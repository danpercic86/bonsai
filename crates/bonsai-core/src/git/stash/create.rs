//! Stash creation: native (`All`/`AllWithUntracked`) and the hand-rolled
//! staged-only fold (`Staged`), plus their private helpers (P9/P34).

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::bisect::require_no_bisect;
use crate::git::commit::resolve_signature;
use crate::git::stage::open_workdir_repo;

use super::{require_clean, CreateStashResult, StashScope};

/// Blocking. Stash the worktree per `scope`. `message: None` → git default
/// ("WIP on <branch>: <short> <summary>"). Precondition: state Clean else
/// OperationInProgress. Nothing in that scope → Ok(CreateStashResult{created:false}).
///
/// - `All` / `AllWithUntracked` → native `stash_save2` (DEFAULT [| INCLUDE_UNTRACKED]).
/// - `Staged` → hand-rolled `create_staged_stash` (no native flag exists).
pub fn create_stash(
    workdir: &Path,
    message: Option<&str>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    create_stash_with(&mut repo, message, scope)
}

/// Handle-reusing twin of [`create_stash`] (P88b/B2a): runs against an
/// already-open `&mut Repository` so a composite mutation opens the repo once.
/// Byte-identical to `create_stash` minus the `open_workdir_repo` call. The
/// bare-repo guard that `open_workdir_repo` performs is NOT re-run here, so each
/// caller preserves it at the original point: the worktree-probing composite
/// (`checkout_branch_autostash`) hits it via `ensure_not_bare` inside
/// `branch_checked_out_elsewhere_with` before reaching this, while the
/// `open_repo_at`-based composites (`create_branch_here`,
/// `checkout_commit_detached`) reinstate `stage::ensure_not_bare` explicitly
/// immediately before calling this.
pub fn create_stash_with(
    repo: &mut git2::Repository,
    message: Option<&str>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError> {
    require_clean(repo)?;
    // A clean detached-HEAD bisect is invisible to `require_clean` — refuse
    // (covers both native and staged scopes, incl. `create_staged_stash`).
    require_no_bisect(repo)?;

    // Identity is required to author the stash commit; surface ConfigMissing
    // early, consistent with commit/merge.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    match scope {
        StashScope::All => native_stash(repo, &sig, message, git2::StashFlags::DEFAULT),
        StashScope::AllWithUntracked => native_stash(
            repo,
            &sig,
            message,
            git2::StashFlags::DEFAULT | git2::StashFlags::INCLUDE_UNTRACKED,
        ),
        StashScope::Staged => create_staged_stash(repo, &sig, message),
    }
}

/// Native libgit2 stash (`All` / `AllWithUntracked`). NotFound → created:false.
fn native_stash(
    repo: &mut git2::Repository,
    sig: &git2::Signature,
    message: Option<&str>,
    flags: git2::StashFlags,
) -> Result<CreateStashResult, AppError> {
    match repo.stash_save2(sig, message, Some(flags)) {
        Ok(_oid) => Ok(CreateStashResult { created: true }),
        // libgit2 returns GIT_ENOTFOUND when there is nothing to stash.
        Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(CreateStashResult { created: false }),
        Err(e) => Err(e.into()),
    }
}

/// Hand-rolled staged-only stash with FOLD semantics (orchestrator decision,
/// simpler than the contract's merge_trees split — no overlap check, no reject):
///
/// For every "staged path" (index differs from HEAD) we capture that path's
/// CURRENT WORKTREE content into the stash tree, then reset that path in BOTH
/// the index and the worktree back to HEAD. Pure-staged files → their staged
/// content is stashed and removed from the tree; mixed files (staged+unstaged) →
/// their FULL worktree content is folded in and the file returns to HEAD.
/// Unstaged-ONLY paths (index==HEAD, worktree changed) and untracked files are
/// left UNTOUCHED.
///
/// The result is a real, libgit2-compatible stash entry: the stash commit `w`
/// has parents `[HEAD, index_commit]` and tree = HEAD ⊕ (worktree content of
/// staged paths), so `diff(HEAD_tree → w.tree)` == exactly those folded changes.
/// The existing SAFE `apply_stash`/`pop_stash` replay that diff into the
/// worktree (as unstaged edits — no index reinstate, F-1).
fn create_staged_stash(
    repo: &mut git2::Repository,
    sig: &git2::Signature,
    message: Option<&str>,
) -> Result<CreateStashResult, AppError> {
    let workdir = repo
        .workdir()
        .ok_or_else(|| AppError::Git("cannot stash in a bare repository".to_string()))?
        .to_path_buf();

    // ---- read-only analysis: NOTHING mutated in this block ----------------
    // Unborn HEAD has no base commit → surface as AppError::Git.
    let head_commit = repo.head()?.peel_to_commit()?;
    let head_tree = head_commit.tree()?;

    let mut index = repo.index()?;
    let index_tree_oid = index.write_tree()?; // HEAD + staged  (unused tree kept for rollback)
    let index_tree = repo.find_tree(index_tree_oid)?;

    // Staged paths = index differs from HEAD. We key the stash-tree build on the
    // STAGED DELTA type (not worktree presence): a path staged as a deletion must
    // be captured as a deletion even when the file is still on disk with HEAD
    // content (the `git rm --cached` case) — otherwise the staged deletion would
    // be silently dropped. No rename detection is requested, so libgit2 reports a
    // rename as a Delete + Add pair, each handled by its own branch below.
    let staged_diff = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None)?;
    let mut staged: Vec<(PathBuf, bool)> = staged_diff
        .deltas()
        .filter_map(|d| {
            let is_delete = d.status() == git2::Delta::Deleted;
            // A deletion's path lives in old_file; every other status in new_file.
            d.new_file()
                .path()
                .or_else(|| d.old_file().path())
                .map(|p| (p.to_path_buf(), is_delete))
        })
        .collect();
    staged.sort();
    staged.dedup();
    drop(staged_diff);

    if staged.is_empty() {
        return Ok(CreateStashResult { created: false }); // AC-5: nothing staged
    }
    let staged_paths: Vec<PathBuf> = staged.iter().map(|(p, _)| p.clone()).collect();

    // Build the stash tree: HEAD tree with each staged path overridden per the
    // staged delta. Untracked paths are never touched → they stay out of every
    // tree. Uses an in-memory index; entries are added by oid so no workdir
    // association is required.
    let mut wt = git2::Index::new()?;
    wt.read_tree(&head_tree)?;
    for (p, is_delete) in &staged {
        if *is_delete {
            // Staged deletion. If the file is STILL on disk with content that
            // DIFFERS from HEAD (`git rm --cached` + rewrite), capturing only
            // the deletion would be data loss (F-A6-A): the mutation window
            // below force-restores HEAD over the path, so the rewritten bytes
            // would then exist nowhere. FOLD semantics (same rule as a mixed
            // staged+unstaged modification): fold the full worktree content
            // into the stash tree — the staged deletion is subsumed by the
            // fold. A file absent from disk, or present with HEAD content
            // (plain `git rm --cached`), still records the deletion.
            let abs = workdir.join(p);
            if abs.is_file() {
                let blob = repo
                    .blob_path(&abs)
                    .map_err(|e| AppError::Git(format!("read {}: {e}", p.display())))?;
                let differs_from_head = head_tree
                    .get_path(p)
                    .map(|te| te.id() != blob)
                    .unwrap_or(true);
                if differs_from_head {
                    let mode = staged_entry_mode(&index, &head_tree, p);
                    wt.add(&make_index_entry(p, blob, mode)?)?;
                    continue;
                }
            }
            wt.remove_path(p)?;
            continue;
        }
        let abs = workdir.join(p);
        if abs.is_file() {
            // Staged add/modify with the file present → FOLD the worktree content.
            // `blob_path` (libgit2 `git_blob_create_fromdisk`) runs the CHECK-IN
            // filter chain (CRLF/ident) for files inside the workdir — raw
            // `fs::read` would embed CRLF blobs in the stash tree under
            // core.autocrlf=true (audit 2026-08-07 §2.2).
            let blob = repo
                .blob_path(&abs)
                .map_err(|e| AppError::Git(format!("read {}: {e}", p.display())))?;
            let mode = staged_entry_mode(&index, &head_tree, p);
            wt.add(&make_index_entry(p, blob, mode)?)?;
        } else if let Some(e) = index.get_path(p, 0) {
            // Staged add/modify but the file is gone from the worktree (an
            // unstaged deletion sits on top) → capture the staged index blob so
            // the staged content is preserved rather than lost.
            wt.add(&e)?;
        } else {
            // No worktree file and no staged index entry → nothing to capture.
            wt.remove_path(p)?;
        }
    }
    let stash_tree_oid = wt.write_tree_to(repo)?;
    let stash_tree = repo.find_tree(stash_tree_oid)?;

    // ---- build the git-standard stash object graph (unreferenced commits) ---
    let branch = current_branch_label(repo);
    let short = short_hex(head_commit.id());
    let summary = head_commit.summary().ok().flatten().unwrap_or("").to_string();
    let i_msg = format!("index on {branch}: {short} {summary}");
    let default_w = format!("WIP on {branch}: {short} {summary}");
    let w_msg = message.unwrap_or(default_w.as_str());

    let i_oid = repo.commit(None, sig, sig, &i_msg, &index_tree, &[&head_commit])?;
    let i_commit = repo.find_commit(i_oid)?;
    let w_oid = repo.commit(None, sig, sig, w_msg, &stash_tree, &[&head_commit, &i_commit])?;

    // ---- MUTATION WINDOW (rollback on any failure) ------------------------
    // Make `w` stash@{0} (F-2). The stash stack IS the refs/stash reflog (what
    // stash_foreach / list_stashes read), so exactly ONE reflog entry must be
    // written per push. libgit2 auto-appends a refs/stash reflog entry on a
    // forced ref update WITH a message WHENEVER the refs/stash reflog file
    // already exists (i.e. any prior stash) — but NOT for the very first stash
    // (no reflog file yet). A blind explicit append therefore double-logs when
    // stacking (stash shows twice, older entries shoved down → drop/apply-by-
    // index hit the wrong entry). So we measure the reflog length across the ref
    // update and only append ourselves when the update did NOT auto-log.
    let reflog_len_before = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    repo.reference("refs/stash", w_oid, true, w_msg)?;
    let reflog_len_after = repo.reflog("refs/stash").map(|r| r.len()).unwrap_or(0);
    if reflog_len_after == reflog_len_before {
        // First stash (no auto-log): append exactly one entry ourselves.
        let mut reflog = repo.reflog("refs/stash")?;
        reflog.append(w_oid, sig, Some(w_msg))?;
        reflog.write()?;
    }

    let mutate = || -> Result<(), AppError> {
        // 1. Worktree: revert staged paths to HEAD (index still the baseline, so an
        //    index-only add reverts to a removal). SAFE-force, path-scoped → unstaged
        //    -only paths and untracked files are untouched.
        let mut co = git2::build::CheckoutBuilder::new();
        co.force().update_index(false).remove_untracked(false);
        for p in &staged_paths {
            co.path(p);
        }
        repo.checkout_tree(head_tree.as_object(), Some(&mut co))?;
        // 2. Index: reset to HEAD (nothing staged). Unstaged-only paths already
        //    equal HEAD in the index, so they are unaffected.
        let mut idx = repo.index()?;
        idx.read_tree(&head_tree)?;
        idx.write()?;
        Ok(())
    };

    if let Err(e) = mutate() {
        // DATA SAFETY: the durable refs/stash entry now holds the user's staged
        // work. mutate() may have PARTIALLY reverted staged paths to HEAD before
        // failing, in which case that folded content survives ONLY inside the
        // stash entry — so we must NOT drop it (dropping here would be the one
        // lossy path; even an uncaught panic would be safer because the stash
        // survives). Best-effort restore the index to its original staged state,
        // KEEP the stash, and tell the user where their work is.
        let index_tree_for_restore = index_tree_oid;
        drop((head_tree, head_commit, index_tree, stash_tree, i_commit, index));
        if let (Ok(mut idx), Ok(tree)) = (repo.index(), repo.find_tree(index_tree_for_restore)) {
            let _ = idx.read_tree(&tree);
            let _ = idx.write();
        }
        let base = match &e {
            AppError::Git(m) => m.clone(),
            other => other.to_string(),
        };
        return Err(AppError::Git(format!(
            "{base} (your staged changes are safe at stash@{{0}})"
        )));
    }

    Ok(CreateStashResult { created: true })
}

/// Short-hex (7 chars) of an oid, for the git-standard stash message.
fn short_hex(oid: git2::Oid) -> String {
    oid.to_string().chars().take(7).collect()
}

/// Human label for the current branch in the stash message; detached HEAD →
/// "(no branch)" (F-4, matches git's `WIP on (no branch): …`).
fn current_branch_label(repo: &git2::Repository) -> String {
    match repo.head() {
        Ok(h) if h.is_branch() => h.shorthand().unwrap_or("(no branch)").to_string(),
        _ => "(no branch)".to_string(),
    }
}

/// Filemode for a folded worktree blob: prefer the staged index entry's mode,
/// else the HEAD tree's, else a regular blob.
fn staged_entry_mode(index: &git2::Index, head_tree: &git2::Tree, p: &Path) -> u32 {
    if let Some(e) = index.get_path(p, 0) {
        return e.mode;
    }
    if let Ok(te) = head_tree.get_path(p) {
        return te.filemode() as u32;
    }
    0o100644
}

/// Build an in-memory `IndexEntry` for `path` pointing at `blob`. Git index
/// paths use forward slashes; normalize so nested paths round-trip on Windows.
pub(crate) fn make_index_entry(path: &Path, blob: git2::Oid, mode: u32) -> Result<git2::IndexEntry, AppError> {
    let git_path = path
        .to_str()
        .ok_or_else(|| AppError::Git(format!("non-utf8 path {}", path.display())))?
        .replace('\\', "/");
    Ok(git2::IndexEntry {
        ctime: git2::IndexTime::new(0, 0),
        mtime: git2::IndexTime::new(0, 0),
        dev: 0,
        ino: 0,
        mode,
        uid: 0,
        gid: 0,
        file_size: 0,
        id: blob,
        flags: 0,
        flags_extended: 0,
        path: git_path.into_bytes(),
    })
}
