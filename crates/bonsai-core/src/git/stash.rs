//! Stash management (P9 contract §2). Wraps every git2 stash primitive:
//! list / create / apply / pop / drop.
//!
//! Pure git2 logic, no Tauri types (runtime-free cores → unit-testable without
//! the Tauri "test" feature, same rule as merge/rebase). All stash APIs
//! (`stash_foreach` / `stash_save2` / `stash_apply` / `stash_pop` /
//! `stash_drop`) require `&mut Repository`, so callers bind `let mut repo`.
//!
//! SAFE by construction: apply/pop use the SAFE checkout default (no
//! REINSTATE_INDEX, OPEN Q#4) and a conflicting apply/pop RETAINS the stash
//! (never lossy). Pop uses the P8 `apply + inspect index + conditional drop`
//! pattern (mirrors `merge::pop_after_success`) because this libgit2 may report
//! a *content* conflict as `Ok(())`, and a naive `stash_pop` would then silently
//! drop the entry — data loss.

use std::path::{Path, PathBuf};

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::stage::open_workdir_repo;

/// One stash stack entry. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    /// Stack index; 0 == most recent (== stash@{0}). SHIFTS after any drop/pop.
    pub index: usize,
    /// Full stash message, e.g. "WIP on main: 1a2b3c4 summary" or a custom message.
    pub message: String,
    /// Full 40-hex oid of the stash commit itself.
    pub oid: String,
    /// Full 40-hex oid of the stash's FIRST parent = the base commit it was
    /// created from (what the graph pill attaches to).
    pub base_oid: String,
    /// Stash commit author time, seconds since epoch (UTC) — drives relative age.
    pub ts: i64,
}

/// Result of apply/pop. Wire: tagged "kind", camelCase (same recipe as MergeOutcome).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ApplyStashOutcome {
    /// Clean apply/pop. (Pop additionally dropped the entry.)
    Applied,
    /// Worktree has <<<<<<< markers, index has conflict entries, and the stash
    /// entry is RETAINED (libgit2 does not drop on GIT_ECONFLICT). `paths` =
    /// sorted conflicted paths (the set `list_conflicts` returns).
    Conflicts { paths: Vec<String> },
}

/// Result of create_stash.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStashResult {
    /// false == nothing to stash (clean worktree) → NOT an error.
    pub created: bool,
}

/// Which changes a `create_stash` call captures. Wire: camelCase (matches the
/// TS `StashScope` union `'all' | 'allWithUntracked' | 'staged'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StashScope {
    /// Staged + unstaged tracked changes; untracked left in place.
    /// → `StashFlags::DEFAULT`.
    All,
    /// Adds untracked files. → `StashFlags::DEFAULT | INCLUDE_UNTRACKED`.
    AllWithUntracked,
    /// Only the staged (index-vs-HEAD) paths. No native libgit2 flag —
    /// hand-rolled by `create_staged_stash` (FOLD semantics, see below).
    Staged,
}

/// Shared precondition: create/apply/pop require a Clean repo state (no
/// in-progress merge/rebase). Drop is exempt (touches only the stash reflog).
fn require_clean(repo: &git2::Repository) -> Result<(), AppError> {
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — finish or abort it first".to_string(),
        ));
    }
    Ok(())
}

/// Blocking. Enumerate the stash stack, index 0 (most recent) first.
/// `stash_foreach` is the ONLY enumeration API; its callback receives
/// (index, message, &oid). Empty stack → Ok(vec![]).
///
/// The closure cannot re-borrow `repo` (it is mutably borrowed for the
/// duration), so we collect (index, message) inside it and resolve
/// oid/base_oid/ts AFTER via the stash reflog (`refs/stash`), where entry `i`'s
/// `id_new()` is the stash commit oid.
pub fn list_stashes(workdir: &Path) -> Result<Vec<StashEntry>, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    let mut raw: Vec<(usize, String)> = Vec::new();
    repo.stash_foreach(|index, message, _oid| {
        // NOTE: we intentionally do NOT capture oid here — resolving base/ts
        // needs an immutable repo borrow, impossible inside this &mut closure.
        raw.push((index, message.to_string()));
        true
    })?;

    if raw.is_empty() {
        return Ok(Vec::new());
    }

    // Resolve each entry via the stash reflog (index order; entry 0 == stash@{0}).
    let reflog = repo.reflog("refs/stash")?;
    let mut out = Vec::with_capacity(raw.len());
    for (index, message) in raw {
        let entry = reflog.get(index).ok_or_else(|| {
            AppError::Git(format!("stash reflog entry {index} missing"))
        })?;
        let stash_oid = entry.id_new();
        let commit = repo.find_commit(stash_oid)?;
        let base_oid = commit.parent_id(0)?;
        out.push(StashEntry {
            index,
            message,
            oid: commit.id().to_string(),
            base_oid: base_oid.to_string(),
            ts: commit.author().when().seconds(),
        });
    }
    Ok(out)
}

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
    require_clean(&repo)?;

    // Identity is required to author the stash commit; surface ConfigMissing
    // early, consistent with commit/merge.
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    match scope {
        StashScope::All => native_stash(&mut repo, &sig, message, git2::StashFlags::DEFAULT),
        StashScope::AllWithUntracked => native_stash(
            &mut repo,
            &sig,
            message,
            git2::StashFlags::DEFAULT | git2::StashFlags::INCLUDE_UNTRACKED,
        ),
        StashScope::Staged => create_staged_stash(&mut repo, &sig, message),
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
            // Staged deletion (incl. `git rm --cached` where the file is still on
            // disk) → capture the deletion regardless of worktree presence.
            wt.remove_path(p)?;
            continue;
        }
        let abs = workdir.join(p);
        if abs.is_file() {
            // Staged add/modify with the file present → FOLD the worktree content.
            let bytes = std::fs::read(&abs)
                .map_err(|e| AppError::Git(format!("read {}: {e}", p.display())))?;
            let blob = repo.blob(&bytes)?;
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
    let summary = head_commit.summary().unwrap_or("").to_string();
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
fn make_index_entry(path: &Path, blob: git2::Oid, mode: u32) -> Result<git2::IndexEntry, AppError> {
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

/// Blocking. Apply stash `index` WITHOUT dropping it. Precondition: state Clean
/// else OperationInProgress. Conflicts → Ok(Conflicts{paths}) (stash retained).
pub fn apply_stash(workdir: &Path, index: usize) -> Result<ApplyStashOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    require_clean(&repo)?;

    let mut opts = git2::StashApplyOptions::new(); // SAFE default; NO REINSTATE_INDEX (OPEN Q#4)
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            // This libgit2 may report a CONTENT conflict as Ok(()) with markers
            // in the worktree + conflict entries in the index. Inspect the index
            // rather than trusting the return code (P8 lesson).
            if repo.index()?.has_conflicts() {
                Ok(ApplyStashOutcome::Conflicts {
                    paths: conflict_paths(workdir)?,
                })
            } else {
                Ok(ApplyStashOutcome::Applied)
            }
        }
        Err(e) if e.code() == git2::ErrorCode::Conflict => Ok(ApplyStashOutcome::Conflicts {
            paths: conflict_paths(workdir)?,
        }),
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Apply stash `index` and drop it on clean success only.
/// Precondition: state Clean else OperationInProgress. Conflicts →
/// Ok(Conflicts{paths}) and the entry is RETAINED (libgit2 does not drop).
///
/// Mirrors `merge::pop_after_success`: uses `stash_apply` + inspect index +
/// conditional `stash_drop` so WE control dropping. A naive `stash_pop` would
/// silently drop the entry on this libgit2's false-clean content conflict —
/// data loss.
pub fn pop_stash(workdir: &Path, index: usize) -> Result<ApplyStashOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    require_clean(&repo)?;

    let mut opts = git2::StashApplyOptions::new(); // SAFE default; NO REINSTATE_INDEX (OPEN Q#4)
    match repo.stash_apply(index, Some(&mut opts)) {
        Ok(()) => {
            if repo.index()?.has_conflicts() {
                // Conflicted re-apply: LEAVE the stash on the stack (do NOT
                // drop) → retained for the user to resolve.
                Ok(ApplyStashOutcome::Conflicts {
                    paths: conflict_paths(workdir)?,
                })
            } else {
                // Clean apply → now drop, equivalent to a clean pop.
                repo.stash_drop(index)?;
                Ok(ApplyStashOutcome::Applied)
            }
        }
        // A checkout-level conflict means nothing droppable was applied →
        // stash retained.
        Err(e) if e.code() == git2::ErrorCode::Conflict => Ok(ApplyStashOutcome::Conflicts {
            paths: conflict_paths(workdir)?,
        }),
        Err(e) => Err(e.into()),
    }
}

/// Blocking. Permanently discard stash `index`. Allowed in ANY repo state
/// (touches only the stash reflog). UI confirms first (destructive). An
/// out-of-range index surfaces as the underlying git2 error → AppError::Git.
pub fn drop_stash(workdir: &Path, index: usize) -> Result<(), AppError> {
    let mut repo = open_workdir_repo(workdir)?;
    repo.stash_drop(index)?;
    Ok(())
}

/// Sorted conflicted paths — the exact set `list_conflicts` returns (P8 reuse).
fn conflict_paths(workdir: &Path) -> Result<Vec<String>, AppError> {
    Ok(list_conflicts(workdir)?
        .into_iter()
        .map(|c| c.path)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire shapes (P9 §8 test 8): serde tag/casing must match the TS mirrors
    /// (ApplyStashOutcome union + StashEntry camelCase).
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(ApplyStashOutcome::Applied).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "applied" }));

        let v = serde_json::to_value(ApplyStashOutcome::Conflicts {
            paths: vec!["src/app.ts".to_string(), "README.md".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["src/app.ts", "README.md"] })
        );

        let v = serde_json::to_value(StashEntry {
            index: 0,
            message: "WIP on main: 1a2b3c4 summary".to_string(),
            oid: "a".repeat(40),
            base_oid: "b".repeat(40),
            ts: 1_700_000_000,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({
                "index": 0,
                "message": "WIP on main: 1a2b3c4 summary",
                "oid": "a".repeat(40),
                "baseOid": "b".repeat(40),
                "ts": 1_700_000_000
            })
        );

        let v = serde_json::to_value(CreateStashResult { created: true }).expect("json");
        assert_eq!(v, serde_json::json!({ "created": true }));
    }

    // ============================================================ P9 §8 matrix
    // Behavioral stash matrix (one test per §8 row, 1..=7). Each asserts BOTH the
    // returned outcome AND the on-disk state via a scratch repo, echoing the
    // merge.rs (P8) fixtures. Fixtures are built with git2 — deterministic, no
    // network, no CLI.

    /// Init a scratch repo with a deterministic identity + autocrlf off (== p8_init).
    fn s9_init(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    /// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
    fn s9_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
        use crate::git::stage::stage_paths;
        for (name, content) in files {
            std::fs::write(dir.join(name), content).expect("write file");
        }
        stage_paths(
            dir,
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir, msg).expect("commit");
    }

    /// Build a commit on `refname` from `parent`'s tree WITHOUT moving HEAD or the
    /// worktree (== p8_commit_on_ref). Used to build a divergent topic tip.
    fn s9_commit_on_ref(
        repo: &git2::Repository,
        refname: &str,
        parent: &git2::Commit,
        files: &[(&str, &str)],
        msg: &str,
    ) -> git2::Oid {
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let mut tb = repo
            .treebuilder(Some(&parent.tree().expect("parent tree")))
            .expect("treebuilder");
        for (name, content) in files {
            let blob = repo.blob(content.as_bytes()).expect("blob");
            tb.insert(name, blob, 0o100644).expect("insert");
        }
        let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
        repo.commit(Some(refname), &sig, &sig, &format!("{msg}\n"), &tree, &[parent])
            .expect("commit on ref")
    }

    fn s9_head_oid(dir: &Path) -> String {
        let repo = git2::Repository::open(dir).expect("open");
        let oid = repo
            .head()
            .expect("HEAD")
            .peel_to_commit()
            .expect("peel")
            .id();
        oid.to_string()
    }

    fn s9_read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).expect("read file")
    }

    // ---- Row 1: Round-trip (apply does NOT drop) ---------------------------

    #[test]
    fn s9_1_round_trip_apply_keeps_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);
        let head = s9_head_oid(d);

        // Dirty: edit a tracked file (unstaged).
        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");

        let res = create_stash(d, None, StashScope::All).expect("create_stash");
        assert!(res.created, "dirty tracked edit must stash");

        // Worktree returned to the committed state (stash reset both index+worktree).
        assert_eq!(s9_read(d, "a.txt"), "base\n", "worktree must be clean after stash");

        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 1, "one entry on the stack");
        assert_eq!(list[0].index, 0);
        assert_eq!(list[0].base_oid, head, "base_oid must == HEAD at stash time");
        assert!(!list[0].message.is_empty(), "default message must be non-empty");

        let outcome = apply_stash(d, 0).expect("apply");
        assert_eq!(outcome, ApplyStashOutcome::Applied, "clean apply");
        assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

        let list = list_stashes(d).expect("list after apply");
        assert_eq!(list.len(), 1, "apply must NOT drop the stash");
    }

    // ---- Row 2: Pop drops --------------------------------------------------

    #[test]
    fn s9_2_pop_drops_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("a.txt"), "edited\n").expect("edit");
        let res = create_stash(d, None, StashScope::All).expect("create_stash");
        assert!(res.created);
        assert_eq!(s9_read(d, "a.txt"), "base\n");

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied, "clean pop");
        assert_eq!(s9_read(d, "a.txt"), "edited\n", "edit restored to worktree");

        let list = list_stashes(d).expect("list after pop");
        assert_eq!(list.len(), 0, "pop must drop the stash on clean apply");
    }

    // ---- Row 3: Nothing to stash -------------------------------------------

    #[test]
    fn s9_3_nothing_to_stash() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        // Clean tree.
        let res = create_stash(d, None, StashScope::All).expect("create_stash");
        assert!(!res.created, "clean tree -> created:false, NOT an error");

        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 0, "nothing pushed onto the stack");
    }

    // ---- Row 4: Include untracked ------------------------------------------

    #[test]
    fn s9_4_include_untracked_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        // An untracked file present.
        std::fs::write(d.join("untracked.txt"), "new\n").expect("write untracked");

        let res = create_stash(d, None, StashScope::AllWithUntracked)
            .expect("create_stash include_untracked");
        assert!(res.created, "untracked file must stash under include_untracked");
        assert!(
            !d.join("untracked.txt").exists(),
            "include_untracked must remove the untracked file from the worktree"
        );

        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 1);

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert!(
            d.join("untracked.txt").exists(),
            "pop must restore the stashed untracked file"
        );
        assert_eq!(s9_read(d, "untracked.txt"), "new\n", "untracked content restored");
        assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops entry");
    }

    // ---- Row 5: Pop conflict retains (P8-lesson data safety) + apply variant

    /// Build a repo where popping/applying stash@{0} necessarily conflicts on X:
    /// base has X="base"; stash records X="stashed"; then HEAD advances X="head".
    /// Returns the scratch dir (kept alive by the caller).
    fn s9_conflict_fixture() -> tempfile::TempDir {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("x.txt", "base\n")]);

        // Stash an unstaged edit to X.
        std::fs::write(d.join("x.txt"), "stashed\n").expect("edit x");
        let res = create_stash(d, None, StashScope::All).expect("create_stash");
        assert!(res.created);
        assert_eq!(s9_read(d, "x.txt"), "base\n", "worktree reset after stash");

        // Advance HEAD with a DIFFERENT change to the same file -> guaranteed
        // 3-way conflict on re-apply (base=base, ours=head, theirs=stashed).
        s9_commit(d, "head change", &[("x.txt", "head\n")]);
        dir
    }

    #[test]
    fn s9_5_pop_conflict_retains_stash() {
        let dir = s9_conflict_fixture();
        let d = dir.path();

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(
            outcome,
            ApplyStashOutcome::Conflicts {
                paths: vec!["x.txt".to_string()]
            },
            "conflicting pop must report Conflicts on x.txt"
        );

        // No merge started; only the index carries conflict entries.
        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Clean,
            "stash apply must not enter a Merge state"
        );

        assert!(
            s9_read(d, "x.txt").contains("<<<<<<<"),
            "worktree X must carry conflict markers"
        );

        let list = list_stashes(d).expect("list after conflicting pop");
        assert_eq!(
            list.len(),
            1,
            "DATA SAFETY: a conflicting pop must RETAIN the stash (never lossy)"
        );
    }

    #[test]
    fn s9_5b_apply_conflict_retains_stash() {
        let dir = s9_conflict_fixture();
        let d = dir.path();

        let outcome = apply_stash(d, 0).expect("apply");
        assert_eq!(
            outcome,
            ApplyStashOutcome::Conflicts {
                paths: vec!["x.txt".to_string()]
            },
            "conflicting apply must report Conflicts on x.txt"
        );

        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert!(s9_read(d, "x.txt").contains("<<<<<<<"));

        let list = list_stashes(d).expect("list after conflicting apply");
        assert_eq!(list.len(), 1, "apply never drops; stash retained on conflict");
    }

    // ---- Row 6: Drop re-indexes the stack ----------------------------------

    #[test]
    fn s9_6_drop_reindexes_stack() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        // Stash A (becomes stash@{1} after the second push).
        std::fs::write(d.join("a.txt"), "edit-A\n").expect("edit A");
        create_stash(d, Some("stash-A"), StashScope::All).expect("stash A");
        // Stash B (most recent, stash@{0}).
        std::fs::write(d.join("a.txt"), "edit-B\n").expect("edit B");
        create_stash(d, Some("stash-B"), StashScope::All).expect("stash B");

        let before = list_stashes(d).expect("list before drop");
        assert_eq!(before.len(), 2);
        assert!(before[0].message.contains("stash-B"), "stash@{{0}} is the newest");
        assert!(before[1].message.contains("stash-A"), "stash@{{1}} is the oldest");
        let survivor_oid = before[1].oid.clone();

        // Drop the most recent (index 0). Entries above shift down by one.
        drop_stash(d, 0).expect("drop");

        let after = list_stashes(d).expect("list after drop");
        assert_eq!(after.len(), 1, "one entry survives");
        assert_eq!(after[0].index, 0, "surviving entry re-indexed to 0 (§2.4 shift)");
        assert!(
            after[0].message.contains("stash-A"),
            "the survivor is the entry previously at index 1 (stash-A)"
        );
        assert_eq!(after[0].oid, survivor_oid, "survivor identity confirmed by oid");
    }

    // ---- Row 7: Op-state guard (Merge in progress) -------------------------

    #[test]
    fn s9_7_op_state_guard_blocks_all_but_drop() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = s9_init(d);

        s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
        let base = repo
            .find_commit(
                repo.head().expect("HEAD").target().expect("oid"),
            )
            .expect("base");
        // topic diverges on x.txt.
        s9_commit_on_ref(&repo, "refs/heads/topic", &base, &[("x.txt", "topic\n")], "topic edits x");
        // main diverges on x.txt (guaranteed conflict).
        s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

        // Dirty unrelated file -> merge autostashes it, then pauses in Merge state.
        // This leaves BOTH a Merge state AND a retained stash on the stack, so we
        // can exercise the guard AND prove drop still works.
        std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");

        crate::git::merge::merge_branch(d, "topic").expect("merge");
        let repo = git2::Repository::open(d).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Merge,
            "conflicting merge over a dirty tree must pause in Merge state"
        );
        assert_eq!(
            list_stashes(d).expect("list").len(),
            1,
            "the retained autostash gives us something to drop"
        );

        // create/apply/pop are all rejected while an operation is in progress.
        match create_stash(d, None, StashScope::All) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("create_stash: expected OperationInProgress, got {other:?}"),
        }
        match apply_stash(d, 0) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("apply_stash: expected OperationInProgress, got {other:?}"),
        }
        match pop_stash(d, 0) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("pop_stash: expected OperationInProgress, got {other:?}"),
        }

        // Drop is allowed in ANY repo state (touches only the stash reflog).
        drop_stash(d, 0).expect("drop must succeed mid-merge");
        assert_eq!(
            list_stashes(d).expect("list after drop").len(),
            0,
            "drop removed the autostash even though a merge is in progress"
        );
    }

    // ======================================================= P34 stash scopes
    // Behavioral matrix for StashScope (all / allWithUntracked / staged), the new
    // data-safety-critical `staged` path (FOLD semantics — orchestrator override:
    // mixed staged+unstaged files are folded WHOLE, not rejected), and the
    // stacking / rm --cached regression guards. Each test asserts BOTH the
    // CreateStashResult/outcome AND the resulting repo state (index, worktree,
    // stash stack). Fixtures reuse the s9_* helpers above.

    /// Stage `names` (delegates to the real staging path used by the app).
    fn p34_stage(dir: &Path, names: &[&str]) {
        use crate::git::stage::stage_paths;
        stage_paths(
            dir,
            &names.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
    }

    /// `git rm --cached <name>`: stage a deletion in the index while the file
    /// STAYS on disk with its HEAD content (index.remove_path, no worktree touch).
    fn p34_rm_cached(dir: &Path, name: &str) {
        let repo = git2::Repository::open(dir).expect("open");
        let mut index = repo.index().expect("index");
        index.remove_path(Path::new(name)).expect("remove_path");
        index.write().expect("write index");
    }

    /// Paths whose INDEX differs from HEAD (== "staged" set). Empty ⇒ index==HEAD.
    fn p34_staged_paths(dir: &Path) -> Vec<String> {
        let repo = git2::Repository::open(dir).expect("open");
        let head_tree = repo
            .head()
            .expect("head")
            .peel_to_tree()
            .expect("head tree");
        let diff = repo
            .diff_tree_to_index(Some(&head_tree), None, None)
            .expect("diff tree->index");
        let mut v: Vec<String> = diff
            .deltas()
            .filter_map(|d| {
                d.new_file()
                    .path()
                    .or_else(|| d.old_file().path())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Paths whose WORKTREE differs from the index (tracked "unstaged" set;
    /// untracked files are excluded by libgit2's default).
    fn p34_unstaged_paths(dir: &Path) -> Vec<String> {
        let repo = git2::Repository::open(dir).expect("open");
        let diff = repo
            .diff_index_to_workdir(None, None)
            .expect("diff index->workdir");
        let mut v: Vec<String> = diff
            .deltas()
            .filter_map(|d| {
                d.new_file()
                    .path()
                    .or_else(|| d.old_file().path())
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }

    fn p34_assert_index_clean(dir: &Path) {
        assert_eq!(
            p34_staged_paths(dir),
            Vec::<String>::new(),
            "index must equal HEAD (nothing staged)"
        );
    }

    // ---- AC-9: wire mapping of the scope union --------------------------------

    #[test]
    fn p34_scope_deserializes_from_camel_case() {
        use serde_json::{from_value, json};
        assert_eq!(
            from_value::<StashScope>(json!("all")).expect("all"),
            StashScope::All
        );
        assert_eq!(
            from_value::<StashScope>(json!("allWithUntracked")).expect("allWithUntracked"),
            StashScope::AllWithUntracked
        );
        assert_eq!(
            from_value::<StashScope>(json!("staged")).expect("staged"),
            StashScope::Staged
        );
        assert!(
            from_value::<StashScope>(json!("bogus")).is_err(),
            "unknown scope must not deserialize"
        );
    }

    // ---- Case 1: `All` == old DEFAULT behavior --------------------------------

    #[test]
    fn p34_all_stashes_tracked_leaves_untracked() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n"), ("b.txt", "b-base\n")]);

        // staged modify (a), unstaged modify (b), untracked (u).
        std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
        p34_stage(d, &["a.txt"]);
        std::fs::write(d.join("b.txt"), "b-unstaged\n").expect("edit b");
        std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

        let res = create_stash(d, None, StashScope::All).expect("create_stash all");
        assert!(res.created, "tracked changes must stash");

        // Tracked worktree changes gone; untracked survives.
        assert_eq!(s9_read(d, "a.txt"), "base\n", "staged a reverted");
        assert_eq!(s9_read(d, "b.txt"), "b-base\n", "unstaged b reverted");
        assert!(d.join("u.txt").exists(), "untracked left in place");
        p34_assert_index_clean(d);
        assert_eq!(list_stashes(d).expect("list").len(), 1, "one entry");
    }

    // ---- Case 2: `AllWithUntracked` also captures untracked -------------------

    #[test]
    fn p34_all_with_untracked_captures_untracked() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
        p34_stage(d, &["a.txt"]);
        std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

        let res = create_stash(d, None, StashScope::AllWithUntracked).expect("create_stash");
        assert!(res.created);
        assert_eq!(s9_read(d, "a.txt"), "base\n", "tracked reverted");
        assert!(
            !d.join("u.txt").exists(),
            "allWithUntracked must sweep the untracked file"
        );
        p34_assert_index_clean(d);
        assert_eq!(list_stashes(d).expect("list").len(), 1);

        // Round-trip restores the untracked file too.
        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert!(d.join("u.txt").exists(), "untracked restored on pop");
        assert_eq!(s9_read(d, "u.txt"), "untracked\n");
        assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
    }

    // ---- Case 3: nothing to stash, per scope ----------------------------------

    #[test]
    fn p34_nothing_to_stash_each_scope() {
        for scope in [
            StashScope::All,
            StashScope::AllWithUntracked,
            StashScope::Staged,
        ] {
            let dir = crate::testutil::scratch_dir();
            let d = dir.path();
            s9_init(d);
            s9_commit(d, "base", &[("a.txt", "base\n")]);
            let head = s9_head_oid(d);

            let res = create_stash(d, None, scope).unwrap_or_else(|e| panic!("{scope:?}: {e:?}"));
            assert!(!res.created, "{scope:?}: clean tree -> created:false");
            assert_eq!(
                list_stashes(d).expect("list").len(),
                0,
                "{scope:?}: no entry pushed"
            );
            assert_eq!(s9_head_oid(d), head, "{scope:?}: HEAD unchanged");
            assert_eq!(s9_read(d, "a.txt"), "base\n", "{scope:?}: worktree unchanged");
        }
    }

    // ---- Case 4: pure-staged modify (the core new path) -----------------------

    #[test]
    fn p34_staged_pure_modify_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
        p34_stage(d, &["a.txt"]); // staged, no further unstaged edit

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(res.created, "staged change must stash");

        // A reverts to HEAD; index == HEAD; one entry.
        assert_eq!(s9_read(d, "a.txt"), "base\n", "worktree reverts to HEAD");
        p34_assert_index_clean(d);
        assert_eq!(
            p34_unstaged_paths(d),
            Vec::<String>::new(),
            "no residual unstaged change"
        );
        assert_eq!(list_stashes(d).expect("list").len(), 1);

        // Pop restores the staged content as an UNSTAGED edit (F-1: no reinstate).
        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert_eq!(s9_read(d, "a.txt"), "a-staged\n", "staged content restored");
        p34_assert_index_clean(d);
        assert_eq!(
            p34_unstaged_paths(d),
            vec!["a.txt".to_string()],
            "restored as UNSTAGED, not re-staged"
        );
        assert_eq!(list_stashes(d).expect("list").len(), 0, "clean pop drops");
    }

    // ---- Case 5: mixed file FOLD (orchestrator override) ----------------------

    #[test]
    fn p34_staged_mixed_file_folds_whole() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("b.txt", "base\n")]);

        std::fs::write(d.join("b.txt"), "b-staged\n").expect("stage-edit b");
        p34_stage(d, &["b.txt"]);
        // Further UNSTAGED edit on the SAME path → mixed file.
        std::fs::write(d.join("b.txt"), "b-staged-then-unstaged\n").expect("unstage-edit b");

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(res.created, "mixed file must fold, not reject");

        // FOLD: B reverts to HEAD, index clean; the FULL worktree content is held.
        assert_eq!(s9_read(d, "b.txt"), "base\n", "b reverted to HEAD");
        p34_assert_index_clean(d);
        assert_eq!(list_stashes(d).expect("list").len(), 1);

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert_eq!(
            s9_read(d, "b.txt"),
            "b-staged-then-unstaged\n",
            "stash held the FULL folded worktree content"
        );
    }

    // ---- Case 6: unstaged-only path is preserved ------------------------------

    #[test]
    fn p34_staged_preserves_unstaged_only_path() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n"), ("c.txt", "c-base\n")]);

        // a.txt staged; c.txt unstaged-only (index == HEAD).
        std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
        p34_stage(d, &["a.txt"]);
        std::fs::write(d.join("c.txt"), "c-unstaged\n").expect("edit c");

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(res.created);

        assert_eq!(s9_read(d, "a.txt"), "base\n", "staged a reverted");
        assert_eq!(
            s9_read(d, "c.txt"),
            "c-unstaged\n",
            "unstaged-only c must NOT be stashed"
        );
        p34_assert_index_clean(d);
        assert_eq!(
            p34_unstaged_paths(d),
            vec!["c.txt".to_string()],
            "c remains an unstaged change"
        );
        assert_eq!(list_stashes(d).expect("list").len(), 1);
    }

    // ---- Case 7: untracked file is preserved ----------------------------------

    #[test]
    fn p34_staged_preserves_untracked() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("a.txt"), "a-staged\n").expect("edit a");
        p34_stage(d, &["a.txt"]);
        std::fs::write(d.join("u.txt"), "untracked\n").expect("write u");

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(res.created);

        assert!(d.join("u.txt").exists(), "untracked survives a staged stash");
        assert_eq!(s9_read(d, "u.txt"), "untracked\n", "untracked content intact");
        assert_eq!(list_stashes(d).expect("list").len(), 1);
    }

    // ---- Case 8: staged ADD ---------------------------------------------------

    #[test]
    fn p34_staged_add_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("new.txt"), "new\n").expect("write new");
        p34_stage(d, &["new.txt"]); // staged add

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(res.created, "staged add must stash");

        assert!(
            !d.join("new.txt").exists(),
            "staged add removed from worktree after stash"
        );
        p34_assert_index_clean(d);
        assert_eq!(list_stashes(d).expect("list").len(), 1);

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert!(d.join("new.txt").exists(), "pop restores the added file");
        assert_eq!(s9_read(d, "new.txt"), "new\n", "added content restored");
    }

    // ---- Case 9: staged DELETE (real worktree deletion) -----------------------

    #[test]
    fn p34_staged_delete_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n"), ("del.txt", "gone\n")]);

        std::fs::remove_file(d.join("del.txt")).expect("rm del");
        p34_stage(d, &["del.txt"]); // stages the deletion (file absent on disk)

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(res.created, "staged deletion must stash");

        // The staged deletion is reverted → file back on disk == HEAD; index clean.
        assert!(
            d.join("del.txt").exists(),
            "staged deletion reverted: file restored to worktree"
        );
        assert_eq!(s9_read(d, "del.txt"), "gone\n", "restored to HEAD content");
        p34_assert_index_clean(d);
        assert_eq!(list_stashes(d).expect("list").len(), 1);

        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert!(
            !d.join("del.txt").exists(),
            "pop reintroduces the staged deletion"
        );
    }

    // ---- Case 10: `git rm --cached` (staged deletion, file still on disk) ------

    #[test]
    fn p34_staged_rm_cached_deletion_captured() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n"), ("rm.txt", "content\n")]);

        p34_rm_cached(d, "rm.txt"); // index deletion; file STAYS on disk w/ HEAD content
        assert_eq!(
            p34_staged_paths(d),
            vec!["rm.txt".to_string()],
            "precondition: rm.txt is a staged deletion"
        );

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(
            res.created,
            "the staged deletion must be captured, not silently dropped"
        );

        // After stash: staged deletion reverted; file present == HEAD; index clean.
        assert!(d.join("rm.txt").exists(), "file back on disk after revert");
        assert_eq!(s9_read(d, "rm.txt"), "content\n");
        p34_assert_index_clean(d);
        assert_eq!(list_stashes(d).expect("list").len(), 1);

        // Pop reintroduces the deletion (proves the deletion was in the entry).
        let outcome = pop_stash(d, 0).expect("pop");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert!(
            !d.join("rm.txt").exists(),
            "pop reintroduces the staged deletion"
        );
    }

    // ---- Case 11: STACKING native + staged (no double-log regression) ---------

    #[test]
    fn p34_stacking_native_then_staged_no_double_log() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "a-base\n"), ("b.txt", "b-base\n")]);

        // 1) Native stash (All) of an unstaged edit to a.txt.
        std::fs::write(d.join("a.txt"), "a-native\n").expect("edit a");
        let r1 = create_stash(d, Some("native-stash"), StashScope::All).expect("native");
        assert!(r1.created);

        // 2) Staged stash of b.txt.
        std::fs::write(d.join("b.txt"), "b-staged\n").expect("edit b");
        p34_stage(d, &["b.txt"]);
        let r2 = create_stash(d, Some("staged-stash"), StashScope::Staged).expect("staged");
        assert!(r2.created);

        // Exactly TWO entries — the hand-rolled push must not double-log.
        let list = list_stashes(d).expect("list");
        assert_eq!(list.len(), 2, "stacking must yield 2 entries, not 3");
        assert!(
            list[0].message.contains("staged-stash"),
            "stash@{{0}} is the staged one, got {:?}",
            list[0].message
        );
        assert!(
            list[1].message.contains("native-stash"),
            "stash@{{1}} is the older native one, got {:?}",
            list[1].message
        );
        let native_oid = list[1].oid.clone();

        // drop@{0} leaves the native survivor intact and re-indexed to 0.
        drop_stash(d, 0).expect("drop 0");
        let after = list_stashes(d).expect("list after drop");
        assert_eq!(after.len(), 1, "native survivor remains");
        assert_eq!(after[0].index, 0, "re-indexed to 0");
        assert_eq!(after[0].oid, native_oid, "survivor is the native entry");
        assert!(after[0].message.contains("native-stash"));

        // apply-by-index resolves the survivor (restores a.txt's native edit).
        let outcome = apply_stash(d, 0).expect("apply survivor");
        assert_eq!(outcome, ApplyStashOutcome::Applied);
        assert_eq!(s9_read(d, "a.txt"), "a-native\n", "native edit re-applied");
    }

    // ---- Case 12: `Staged` with nothing staged (unstaged present) -------------

    #[test]
    fn p34_staged_nothing_staged_but_unstaged_present() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        s9_init(d);
        s9_commit(d, "base", &[("a.txt", "base\n")]);

        std::fs::write(d.join("a.txt"), "a-unstaged\n").expect("edit a"); // unstaged only

        let res = create_stash(d, None, StashScope::Staged).expect("create_stash staged");
        assert!(!res.created, "nothing staged -> created:false");
        assert_eq!(
            s9_read(d, "a.txt"),
            "a-unstaged\n",
            "unstaged change untouched"
        );
        assert_eq!(
            p34_unstaged_paths(d),
            vec!["a.txt".to_string()],
            "still an unstaged change"
        );
        assert_eq!(list_stashes(d).expect("list").len(), 0, "no entry");
    }

    // ---- Case 13: `Staged` rejected mid-merge (require_clean guard) -----------

    #[test]
    fn p34_staged_rejected_mid_merge() {
        let dir = crate::testutil::scratch_dir();
        let d = dir.path();
        let repo = s9_init(d);

        s9_commit(d, "base", &[("x.txt", "base\n"), ("y.txt", "y-base\n")]);
        let base = repo
            .find_commit(repo.head().expect("HEAD").target().expect("oid"))
            .expect("base");
        s9_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("x.txt", "topic\n")],
            "topic edits x",
        );
        s9_commit(d, "main edits x", &[("x.txt", "main\n")]);

        // Dirty unrelated file → conflicting merge pauses in Merge state.
        std::fs::write(d.join("y.txt"), "y-edited\n").expect("edit y");
        crate::git::merge::merge_branch(d, "topic").expect("merge");
        assert_eq!(
            git2::Repository::open(d).expect("reopen").state(),
            git2::RepositoryState::Merge,
            "precondition: mid-merge"
        );

        let before = list_stashes(d).expect("list before");
        match create_stash(d, None, StashScope::Staged) {
            Err(AppError::OperationInProgress(_)) => {}
            other => panic!("expected OperationInProgress, got {other:?}"),
        }
        assert_eq!(
            list_stashes(d).expect("list after").len(),
            before.len(),
            "rejected create must not mutate the stash stack"
        );
    }
}
