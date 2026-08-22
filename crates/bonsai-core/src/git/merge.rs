//! Merge a local or remote-tracking branch into the current branch.
//! Clean merges auto-commit; conflicts pause into RepoOpState::Merge.
//! Pure git2, no Tauri types, no network (merging origin/x uses the local
//! remote-tracking ref — Fetch first is the user's job, same as GitKraken).
//! (P3c contract §4.)

use std::path::Path;

use crate::error::AppError;
use crate::git::activity::GitActivityRecorder;
use crate::git::autostash::{self, PopResult};
use crate::git::bisect::require_no_bisect;
use crate::git::commit::{self, resolve_signature, CommitResult};
use crate::git::conflict::list_conflicts;
use crate::git::exec::SpawnGitExec;
use crate::git::hooks::{
    hooks_enabled, run_hook_nonblocking_streaming, run_hook_streaming, HookName,
};
use crate::git::signing::{self, resolve_signing};
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;
// P87: the activity-recording merge-commit core lives in `merge_activity`
// (file-size split); re-exported so `merge::commit_merge_with_activity` keeps
// resolving for the command layer.
pub use crate::git::merge_activity::commit_merge_with_activity;

/// Wire: tagged "kind", camelCase (same recipe as PullResult).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MergeOutcome {
    /// Incoming is already reachable from HEAD. Nothing changed.
    UpToDate,
    /// HEAD branch fast-forwarded to `to` (full oid). No merge commit.
    /// `stashed` = an autostash was created (and restored) for this operation.
    FastForwarded {
        branch: String,
        to: String,
        stashed: bool,
    },
    /// Clean merge, auto-committed. `oid` = the new 2-parent merge commit.
    /// `stashed` = an autostash was created (and restored) for this operation.
    Merged { oid: String, stashed: bool },
    /// Conflicts recorded in index + worktree; MERGE_HEAD/MERGE_MSG written;
    /// repo paused in state Merge. Sorted conflicted paths (same set
    /// list_conflicts returns). `stashed` = an autostash was created and is
    /// RETAINED on the stack (deferred re-apply, OPEN Q #2).
    Conflicts { paths: Vec<String>, stashed: bool },
    /// FF / merge-commit landed, but re-applying the autostash conflicted.
    /// The stash entry is RETAINED at stash@{0}. `head` = FF target or new
    /// merge-commit oid; `paths` = conflicted paths from the stash apply.
    StashPopConflicts { head: String, paths: Vec<String> },
}

/// Prepared MERGE_MSG first line (contract §4.3, byte-exact for the oracle):
/// `Merge branch '<name>'` / `Merge remote-tracking branch '<name>'` —
/// no `into <branch>` suffix (locked decision §11.4).
fn prepared_merge_message(name: &str, incoming_is_remote: bool) -> String {
    if incoming_is_remote {
        format!("Merge remote-tracking branch '{name}'")
    } else {
        format!("Merge branch '{name}'")
    }
}

/// Blocking. Merges `branch_name` (local shorthand "feature/x" OR
/// remote-tracking shorthand "origin/main") into the current branch.
///
/// Preconditions (contract §4.1, checked in order BEFORE anything mutates):
/// state Clean; HEAD attached + born; branch resolvable (local then remote);
/// index matches HEAD (unstaged worktree changes ARE allowed — they only fail
/// as CheckoutConflict if the merge would overwrite them, in which case
/// nothing is left behind); git identity configured (a clean merge
/// auto-commits).
///
/// `skip_hooks` (F-A4-2): the clean auto-merge commit runs the `commit-msg`
/// hook (only — see [`MergeHooks::MessageOnly`]); `true` ≡ `--no-verify`
/// bypasses it, as does `bonsai.runHooks=false`. A commit-msg rejection
/// returns [`AppError::HookRejected`] with the merge left PAUSED (MERGE_HEAD
/// retained — recover via commit_merge or abort_merge), the one deliberate
/// exception to the "failed merge_branch leaves state Clean" guarantee.
pub fn merge_branch(
    workdir: &Path,
    branch_name: &str,
    skip_hooks: bool,
) -> Result<MergeOutcome, AppError> {
    let mut repo = open_workdir_repo(workdir)?;

    // A clean detached-HEAD bisect is invisible to `state()` below — refuse.
    require_no_bisect(&repo)?;

    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — commit or abort it first".to_string(),
        ));
    }

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot merge: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot merge: HEAD is detached".to_string()));
    }
    let head_branch = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot merge: HEAD has no branch name".to_string()))?;

    // Resolve incoming: local first, then remote-tracking. Merging the
    // current branch by name falls out as UpToDate naturally.
    let (incoming, incoming_is_remote) =
        match repo.find_branch(branch_name, git2::BranchType::Local) {
            Ok(b) => (b, false),
            Err(_) => match repo.find_branch(branch_name, git2::BranchType::Remote) {
                Ok(b) => (b, true),
                Err(_) => {
                    return Err(AppError::BranchNotFound(format!(
                        "branch '{branch_name}' not found (local or remote-tracking)"
                    )));
                }
            },
        };

    // Identity check EARLY: a clean merge auto-commits, so ConfigMissing must
    // surface before the worktree is touched. `sig` is also the autostash
    // stasher identity (§2.3 step 5).
    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Analysis BEFORE any stash (§2.3 steps 6-7). Extract the Copy result +
    // Oid, then release every borrow of `repo` (annotated / incoming) so the
    // later &mut stash / set_target / pop calls are legal.
    let (analysis, incoming_id) = {
        let annotated = repo.reference_to_annotated_commit(incoming.get())?;
        let id = annotated.id();
        let (analysis, _pref) = repo.merge_analysis(&[&annotated])?;
        (analysis, id)
    };
    drop(incoming);

    // An up-to-date no-op must never create a stash.
    if analysis.is_up_to_date() {
        return Ok(MergeOutcome::UpToDate);
    }

    // Dirty = any TRACKED change (staged or unstaged); untracked/ignored
    // excluded (§2.2, mirrors git's autostash default §2.4).
    // Keep the saved stash OID so every later apply/drop addresses it by
    // IDENTITY, never by stack position (F-A7-6).
    let stash_oid = if autostash::is_dirty(&repo)? {
        Some(autostash::stash_save(
            &mut repo,
            &sig,
            "bonsai: autostash before merge",
        )?)
    } else {
        None
    };
    let stashed = stash_oid.is_some();

    // SAFETY NOTE (applies to every bare `?` between here and the terminal
    // outcome — find_object, find_annotated_commit, repo.index(), the MERGE_MSG
    // writes, finalize_merge_commit): if any of these fails AFTER stash_save,
    // the autostash is RETAINED at stash@{0} (never dropped). These paths are
    // effectively unreachable — they operate on the already-validated
    // `incoming_id` oid or are plain filesystem writes — so we deliberately do
    // not wire a rollback into each; the stash is recoverable via the CLI.
    if analysis.is_fast_forward() {
        // merge.ff config NOT consulted in v1 — FF whenever possible.
        // Same safe-FF recipe as remote.rs pull_ff: checkout BEFORE set_target.
        // `obj` is scoped so its borrow of `repo` ends before the &mut calls.
        let checkout_res = {
            let obj = repo.find_object(incoming_id, None)?;
            let mut opts = git2::build::CheckoutBuilder::new();
            opts.safe(); // DEFAULT SAFE MODE — never .force()
            repo.checkout_tree(&obj, Some(&mut opts))
        };
        match checkout_res {
            Ok(()) => {}
            Err(e) if e.code() == git2::ErrorCode::Conflict => {
                // Nothing mutated yet (set_target not run); restore the stash.
                let msg = "cannot merge: local changes would be overwritten. \
                     Commit or discard them first.";
                return Err(autostash::rollback_and_map(
                    &mut repo,
                    stash_oid,
                    AppError::CheckoutConflict(msg.to_string()),
                ));
            }
            Err(e) => return Err(autostash::rollback_and_map(&mut repo, stash_oid, e.into())),
        }
        // `.map(|_| ())` discards the returned Reference so no borrow of `repo`
        // is retained across the following &mut rollback / pop calls.
        let ff_res = repo
            .find_reference(&format!("refs/heads/{head_branch}"))
            .and_then(|mut r| {
                r.set_target(incoming_id, &format!("merge {branch_name}: fast-forward"))
            })
            .map(|_| ());
        if let Err(e) = ff_res {
            // The ref move itself failed; worktree already checked out but the
            // branch still points at the old tip — restore the stash so the
            // user's original state is recoverable.
            return Err(autostash::rollback_and_map(&mut repo, stash_oid, e.into()));
        }
        let to = incoming_id.to_string();
        if let Some(oid) = stash_oid {
            return Ok(match autostash::pop_after_success(&mut repo, workdir, oid)? {
                PopResult::Restored => MergeOutcome::FastForwarded {
                    branch: head_branch,
                    to,
                    stashed: true,
                },
                PopResult::Conflicted(paths) => {
                    MergeOutcome::StashPopConflicts { head: to, paths }
                }
            });
        }
        return Ok(MergeOutcome::FastForwarded {
            branch: head_branch,
            to,
            stashed: false,
        });
    }

    // analysis.is_normal(): true merge.
    let mut message = prepared_merge_message(branch_name, incoming_is_remote);
    let mut merge_opts = git2::MergeOptions::new(); // defaults: find_renames on
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout
        .safe()
        .allow_conflicts(true)
        .conflict_style_merge(true); // <<<<<<< ======= >>>>>>> markers

    // Rebuild a NAME-BEARING annotated commit for the merge so libgit2 labels
    // the conflict "theirs" marker with the branch name (e.g. `>>>>>>> topic`)
    // rather than the bare 40-char oid. Must be rebuilt HERE (after stashing)
    // because stash_save2 needed &mut repo and an AnnotatedCommit borrows repo
    // immutably. reference_to_annotated_commit carries the ref name;
    // find_annotated_commit(oid) does NOT. Resolve local-first then
    // remote-tracking, matching the original resolution order. Scoped so both
    // borrows end before the &mut cleanup / rollback below.
    let merge_res = {
        let incoming_branch = repo
            .find_branch(branch_name, git2::BranchType::Local)
            .or_else(|_| repo.find_branch(branch_name, git2::BranchType::Remote))
            .map_err(|_| {
                AppError::BranchNotFound(format!(
                    "branch '{branch_name}' not found (local or remote-tracking)"
                ))
            })?;
        let annotated = repo.reference_to_annotated_commit(incoming_branch.get())?;
        repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))
    };
    if let Err(e) = merge_res {
        // libgit2 may have written MERGE_HEAD/MERGE_MSG/MERGE_MODE before the
        // checkout failed. Guarantee: a failed merge_branch leaves state Clean.
        let _ = repo.cleanup_state();
        if let Ok(head_commit) = repo.head().and_then(|h| h.peel_to_commit()) {
            if let Ok(tree) = head_commit.tree() {
                // Re-open the index to see whatever repo.merge() left on disk.
                if let Ok(mut index) = repo.index() {
                    let _ = index.read_tree(&tree);
                    let _ = index.write();
                }
            }
        }
        let mapped = if e.code() == git2::ErrorCode::Conflict {
            AppError::CheckoutConflict(
                "cannot merge: local changes would be overwritten. \
                 Commit or discard them first."
                    .to_string(),
            )
        } else {
            e.into()
        };
        return Err(autostash::rollback_and_map(&mut repo, stash_oid, mapped));
    }

    let index = repo.index()?;
    if index.has_conflicts() {
        let paths: Vec<String> = list_conflicts(workdir)?
            .into_iter()
            .map(|c| c.path)
            .collect();
        // Conflicts block, exactly like git (contract §4.3); overwrite
        // libgit2's MERGE_MSG so the on-disk message is deterministic.
        message.push_str("\n\nConflicts:\n");
        message.push_str(
            &paths
                .iter()
                .map(|p| format!("\t{p}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        std::fs::write(repo.path().join("MERGE_MSG"), format!("{message}\n"))?;
        // PAUSE. Do NOT pop — reapplying into a conflicted worktree is unsafe
        // (OPEN Q #2). The stash (if any) is RETAINED on the stack.
        return Ok(MergeOutcome::Conflicts { paths, stashed });
    }

    // Clean: auto-commit (git-like), then cleanup. Keep the on-disk message
    // equal to the committed message until cleanup_state removes it.
    std::fs::write(repo.path().join("MERGE_MSG"), format!("{message}\n"))?;
    // `annotated` / `incoming` already dropped above; release the index borrow
    // before the &mut finalize below.
    drop(index);
    // Auto-commit follows commit.gpgsign (None) — a clean merge signs iff the
    // user opted in; byte-identical to pre-P58 when gpgsign is off (the default).
    // Hooks (F-A4-2): the auto-merge commit runs the `commit-msg` hook only
    // (git parity for message policy; pre-merge-commit/prepare-commit-msg are
    // unsupported — see MergeHooks::MessageOnly). If commit-msg REJECTS, the
    // `?` propagates HookRejected and the merge is left PAUSED — MERGE_HEAD
    // retained, merged content staged, HEAD unchanged — exactly git's "Not
    // committing merge; use 'git commit' to complete the merge." state. The
    // user can fix the message via commit_merge (optionally skipping hooks)
    // or abort_merge; a pre-merge autostash stays retained on the stack
    // (same recoverable pause as the Conflicts outcome above).
    let hooks = if hooks_enabled(&repo.config()?.snapshot()?, skip_hooks) {
        MergeHooks::MessageOnly
    } else {
        MergeHooks::Off
    };
    // `merge_branch` is not an activity-wrapped op (no dedicated category); the
    // auto-merge commit records no activity (None).
    let result = finalize_merge_commit(&mut repo, &message, None, hooks, None)?;
    let oid = result.oid;
    if let Some(stash) = stash_oid {
        return Ok(match autostash::pop_after_success(&mut repo, workdir, stash)? {
            PopResult::Restored => MergeOutcome::Merged { oid, stashed: true },
            PopResult::Conflicted(paths) => MergeOutcome::StashPopConflicts { head: oid, paths },
        });
    }
    Ok(MergeOutcome::Merged { oid, stashed: false })
}

/// Which commit hooks [`finalize_merge_commit`] fires (F-A4-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MergeHooks {
    /// No hooks (`skip_hooks` / `bonsai.runHooks=false`).
    Off,
    /// `commit-msg` only — the clean auto-merge path. git's `git merge`
    /// auto-commit runs pre-merge-commit + prepare-commit-msg + commit-msg;
    /// Bonsai supports neither pre-merge-commit nor prepare-commit-msg
    /// (documented v1 divergence, F-A4-3), but honors commit-msg so message
    /// policy hooks apply to merge commits too.
    MessageOnly,
    /// pre-commit + commit-msg + post-commit — `commit_merge` (concluding a
    /// paused merge, like `git commit` with MERGE_HEAD present).
    Full,
}

/// Shared core of commit_merge and the clean-merge auto-commit path
/// (contract §4.4 steps 3–9): normalize the message, resolve the signature,
/// collect HEAD + every MERGE_HEAD as parents, commit, cleanup_state.
///
/// `sign` (P58 D3 / OQ4): `None` ⇒ follow `commit.gpgsign`; `Some(b)` ⇒ `b`.
/// The unsigned path is byte-identical to pre-P58; signing routes the SAME
/// parents through [`signing::create_signed_commit`].
///
/// `hooks` (P59a + F-A4-2): which commit hooks fire around the merge commit.
/// `commit_merge` passes [`MergeHooks::Full`] — git order: `pre-commit`
/// (before `write_tree`) → `commit-msg` (may rewrite the merge message) →
/// create the commit → `post-commit` (non-blocking), exactly as `git commit`
/// concluding a merge would. The clean auto-merge path passes
/// [`MergeHooks::MessageOnly`] — `commit-msg` only, matching git's
/// message-policy behavior for `git merge`'s auto-commit. On ANY blocking
/// hook rejection the merge is left PAUSED (MERGE_HEAD retained), never
/// half-committed.
pub(crate) fn finalize_merge_commit(
    repo: &mut git2::Repository,
    message: &str,
    sign: Option<bool>,
    hooks: MergeHooks,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<CommitResult, AppError> {
    let run_pre_post = hooks == MergeHooks::Full;
    let run_commit_msg = hooks != MergeHooks::Off;
    // Owned workdir: releases the immutable borrow before the &mut
    // mergehead_foreach below, and is the base for any hook run.
    let workdir_buf = repo.workdir().map(std::path::Path::to_path_buf);
    if run_commit_msg && workdir_buf.is_none() {
        return Err(AppError::Git(
            "cannot run commit hooks in a bare repository".to_string(),
        ));
    }

    // pre-commit BEFORE write_tree (git order); non-zero ⇒ HookRejected, abort
    // (no commit, no ref move, merge state left intact for retry/abort).
    if run_pre_post {
        if let Some(wd) = workdir_buf.as_deref() {
            run_hook_streaming(&SpawnGitExec, wd, HookName::PreCommit, &[], None, activity)?;
        }
    }

    // Normalize exactly like create_commit (CRLF/CR -> \n, trim).
    let mut msg = commit::normalize_message(message);
    if msg.is_empty() {
        return Err(AppError::EmptyMessage);
    }
    // commit-msg may rewrite the merge message file.
    if run_commit_msg {
        if let Some(wd) = workdir_buf.as_deref() {
            msg = commit::run_commit_msg_hook(repo, wd, &msg, activity)?;
        }
    }

    let sig = resolve_signature(&repo.config()?.snapshot()?)?;

    // Parents: HEAD commit first, then EVERY MERGE_HEAD oid in file order
    // (v1 UI only produces one, but octopus state written by the CLI must not
    // be silently truncated). MERGE_HEADs are collected before any immutable
    // repo borrow (mergehead_foreach takes &mut self).
    let mut merge_oids: Vec<git2::Oid> = Vec::new();
    repo.mergehead_foreach(|oid| {
        merge_oids.push(*oid);
        true
    })?;
    let head_commit = repo.head()?.peel_to_commit()?;
    if merge_oids.is_empty() {
        return Err(AppError::Git("MERGE_HEAD missing".to_string()));
    }
    let mut parents: Vec<git2::Commit> = vec![head_commit];
    for oid in merge_oids {
        parents.push(repo.find_commit(oid)?);
    }
    let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

    // NO nothing-to-commit check: an empty-diff merge commit is legitimate —
    // it records ancestry.
    let mut index = repo.index()?;
    if run_pre_post {
        index.read(true)?; // pick up any pre-commit hook re-staging
    }
    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;
    let full = format!("{msg}\n");
    let summary = msg.lines().next().unwrap_or(&msg).to_string();

    let signing = resolve_signing(&repo.config()?.snapshot()?, sign);
    let oid = if !signing.sign {
        // Unsigned path: byte-identical to pre-P58.
        repo.commit(Some("HEAD"), &sig, &sig, &full, &tree, &parent_refs)?
    } else {
        let workdir = workdir_buf.as_deref().ok_or_else(|| {
            AppError::Git("cannot sign a merge in a bare repository".to_string())
        })?;
        let parent_oids: Vec<git2::Oid> = parents.iter().map(git2::Commit::id).collect();
        signing::create_signed_commit(
            &SpawnGitExec,
            workdir,
            tree_oid,
            &parent_oids,
            &sig,
            &sig,
            &full,
            Some(parents[0].id()),
            &format!("commit (merge): {summary}"),
        )?
    };

    // post-commit (non-blocking) runs AFTER the commit is made but BEFORE
    // cleanup_state, so a hook that inspects MERGE_HEAD still sees it. Never
    // blocks — the commit already landed; a failure surfaces as a warning
    // (audit #2 §3.3), never an error.
    let mut hook_warning: Option<String> = None;
    if run_pre_post {
        if let Some(wd) = workdir_buf.as_deref() {
            hook_warning =
                run_hook_nonblocking_streaming(&SpawnGitExec, wd, HookName::PostCommit, &[], activity)
                    .warning(HookName::PostCommit);
        }
    }

    repo.cleanup_state()?; // removes MERGE_HEAD/MERGE_MSG/MERGE_MODE -> Clean

    // HEAD's symref (branch NAME) is stable across the signed external ref move,
    // so reading it off the same handle is authoritative for both paths.
    let branch = repo
        .head()
        .ok()
        .filter(|h| h.is_branch())
        .and_then(|h| h.shorthand().ok().map(String::from));
    Ok(CommitResult {
        oid: oid.to_string(),
        summary,
        branch,
        hook_warning,
    })
}

/// Blocking. Finalizes a paused merge as a 2(+)-parent commit
/// (contract §4.4 — cheap checks first). `sign` (P58 D3 / OQ4): `None` ⇒ follow
/// `commit.gpgsign`; `Some(b)` ⇒ `b`. `skip_hooks` (P59a): `true` ≡ `--no-verify`;
/// otherwise the effective toggle is `bonsai.runHooks` (default true), and the
/// commit hooks fire around the merge commit exactly as `git commit` would.
pub fn commit_merge(
    workdir: &Path,
    message: &str,
    sign: Option<bool>,
    skip_hooks: bool,
) -> Result<CommitResult, AppError> {
    commit_merge_with_activity(workdir, message, sign, skip_hooks, None)
}

/// Blocking. Aborts a paused merge; restores pre-merge index + the worktree
/// files the merge touched (approximate `git reset --merge`, contract §4.5 —
/// NOT reset --hard).
///
/// Guarantee: files with unstaged edits made DURING the paused merge that the
/// merge did NOT touch survive an abort byte-identically. Files the merge
/// touched are restored to HEAD. Under P8 autostash, any pre-merge dirty edit
/// (staged or unstaged) was moved onto the autostash before the merge ran, so
/// it is not in the worktree during the paused merge and is safe on the stack
/// (stash@{0}) — abort neither sees nor clobbers it, and the user re-applies it
/// after finishing.
pub fn abort_merge(workdir: &Path) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;

    if repo.state() != git2::RepositoryState::Merge {
        return Err(AppError::NoOperationInProgress(
            "no merge in progress".to_string(),
        ));
    }

    let head_tree = repo.head()?.peel_to_commit()?.tree()?;
    let mut index = repo.index()?;

    // Every path the merge touched: index-vs-HEAD differences UNION all
    // conflicted paths (ours/theirs/base sides). Raw path bytes — a lossy
    // UTF-8 conversion would silently fail to restore non-UTF-8 paths.
    let mut touched: Vec<Vec<u8>> = Vec::new();
    let diff = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None)?;
    for delta in diff.deltas() {
        for f in [delta.old_file(), delta.new_file()] {
            if let Some(p) = f.path_bytes() {
                touched.push(p.to_vec());
            }
        }
    }
    for c in index.conflicts()? {
        let c = c?;
        for e in [c.ancestor, c.our, c.their].into_iter().flatten() {
            touched.push(e.path.clone());
        }
    }
    touched.sort();
    touched.dedup();

    // Force-checkout ONLY the touched paths: restores/deletes exactly the
    // merge-touched files, leaving unrelated unstaged edits alone.
    //
    // CRITICAL: a CheckoutBuilder with ZERO .path() calls matches ALL paths,
    // so an empty `touched` set (e.g. every conflict already resolved as Ours
    // before Abort) must SKIP the checkout entirely — otherwise force()
    // clobbers the whole worktree, the exact data loss §4.5/§11.2 prevents.
    if !touched.is_empty() {
        let mut cb = git2::build::CheckoutBuilder::new();
        cb.force().remove_untracked(false);
        for p in &touched {
            cb.path(p.as_slice());
        }
        repo.checkout_tree(head_tree.as_object(), Some(&mut cb))?;
    }

    // Drop all conflict + merged entries from the index, then clear the op.
    index.read_tree(&head_tree)?;
    index.write()?;
    repo.cleanup_state()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------- wire shape (TS mirrors)

    /// The serde tag/casing must match the TS MergeOutcome union exactly.
    #[test]
    fn wire_shapes_are_camel_case_tagged() {
        let v = serde_json::to_value(MergeOutcome::UpToDate).expect("json");
        assert_eq!(v, serde_json::json!({ "kind": "upToDate" }));

        let v = serde_json::to_value(MergeOutcome::FastForwarded {
            branch: "main".to_string(),
            to: "a".repeat(40),
            stashed: true,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "fastForwarded", "branch": "main", "to": "a".repeat(40), "stashed": true })
        );

        let v = serde_json::to_value(MergeOutcome::Merged {
            oid: "b".repeat(40),
            stashed: false,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "merged", "oid": "b".repeat(40), "stashed": false })
        );

        let v = serde_json::to_value(MergeOutcome::Conflicts {
            paths: vec!["README.md".to_string(), "src/auth.ts".to_string()],
            stashed: true,
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "conflicts", "paths": ["README.md", "src/auth.ts"], "stashed": true })
        );

        let v = serde_json::to_value(MergeOutcome::StashPopConflicts {
            head: "c".repeat(40),
            paths: vec!["src/app.ts".to_string()],
        })
        .expect("json");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "stashPopConflicts", "head": "c".repeat(40), "paths": ["src/app.ts"] })
        );
    }

    // -------------------------------------------- §4.3 prepared MERGE_MSG

    #[test]
    fn prepared_message_is_byte_exact() {
        assert_eq!(
            prepared_merge_message("feature/login", false),
            "Merge branch 'feature/login'"
        );
        assert_eq!(
            prepared_merge_message("origin/main", true),
            "Merge remote-tracking branch 'origin/main'"
        );
    }

    /// Regression (reviewer MUST-FIX): resolving every conflict as Ours
    /// before Abort leaves the index == HEAD tree with zero conflicts, so the
    /// `touched` set is EMPTY. A CheckoutBuilder with zero .path() calls
    /// matches ALL paths — the empty set must skip the force checkout
    /// entirely, or an unrelated pre-merge unstaged edit gets clobbered.
    #[test]
    fn abort_with_empty_touched_set_preserves_unrelated_unstaged_edit() {
        use crate::git::conflict::{resolve_conflict, ConflictResolution};
        use crate::git::stage::stage_paths;

        let dir = crate::testutil::scratch_dir();
        let repo = git2::Repository::init(dir.path()).expect("init repo");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        let commit_all = |msg: &str, files: &[(&str, &str)]| {
            for (name, content) in files {
                std::fs::write(dir.path().join(name), content).expect("write");
            }
            stage_paths(
                dir.path(),
                &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
            )
            .expect("stage");
            crate::git::commit::create_commit(dir.path(), msg, None, false).expect("commit")
        };

        // Base commit with the conflict file + an unrelated file.
        commit_all("base", &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")]);
        let base_oid = repo.head().expect("HEAD").target().expect("oid");

        // topic edits a.txt one way; main edits it another -> guaranteed conflict.
        repo.branch("topic", &repo.find_commit(base_oid).expect("base"), false)
            .expect("branch");
        commit_all("main change", &[("a.txt", "main\n")]);
        {
            // Commit the divergent topic-side change directly on the branch.
            let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
            let base = repo.find_commit(base_oid).expect("base commit");
            let mut tb = repo.treebuilder(Some(&base.tree().expect("tree"))).expect("tb");
            let blob = repo.blob(b"topic\n").expect("blob");
            tb.insert("a.txt", blob, 0o100644).expect("insert");
            let tree = repo.find_tree(tb.write().expect("tree oid")).expect("tree");
            repo.commit(
                Some("refs/heads/topic"),
                &sig,
                &sig,
                "topic change\n",
                &tree,
                &[&base],
            )
            .expect("topic commit");
        }

        // Clean tree at merge time -> no autostash (stashed: false). The
        // unrelated UNSTAGED edit is made AFTER the merge pauses, so it is not
        // captured by the autostash and must survive the abort (this test's
        // regression concern is abort's empty-touched-set guard, not P8).
        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        assert_eq!(
            outcome,
            MergeOutcome::Conflicts {
                paths: vec!["a.txt".to_string()],
                stashed: false,
            }
        );

        // Unrelated UNSTAGED edit (during the paused merge) that must survive.
        std::fs::write(dir.path().join("unrelated.txt"), "edited but not staged\n")
            .expect("edit unrelated");

        // Resolve the ONLY conflict as Ours: index returns to == HEAD tree,
        // zero conflicts -> abort's touched set is empty.
        resolve_conflict(dir.path(), "a.txt", ConflictResolution::Ours).expect("resolve");

        abort_merge(dir.path()).expect("abort");

        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        let unrelated =
            std::fs::read_to_string(dir.path().join("unrelated.txt")).expect("read unrelated");
        assert_eq!(unrelated, "edited but not staged\n", "unstaged edit clobbered");
        let a = std::fs::read_to_string(dir.path().join("a.txt")).expect("read a.txt");
        assert_eq!(a, "main\n", "a.txt must be back at HEAD's version");
    }

    // ------------------------------------------------------- preconditions

    #[test]
    fn merge_preconditions_on_fresh_repo() {
        let dir = crate::testutil::scratch_dir();
        git2::Repository::init(dir.path()).expect("init repo");

        // Unborn HEAD refuses before branch resolution.
        let err = merge_branch(dir.path(), "topic", false).expect_err("unborn");
        match err {
            AppError::Git(m) => assert!(m.contains("no commits yet"), "got: {m}"),
            other => panic!("expected Git, got {other:?}"),
        }

        // commit_merge / abort_merge with no merge in progress.
        let err = commit_merge(dir.path(), "msg", None, false).expect_err("no merge");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
        let err = abort_merge(dir.path()).expect_err("no merge");
        assert!(matches!(err, AppError::NoOperationInProgress(_)));
    }

    // ============================================================ P8 §7 matrix
    // Autostash-aware merge behavioral matrix. One test per §7 row. Each asserts
    // BOTH the returned MergeOutcome AND the on-disk state. Fixtures are scratch
    // repos built with git2 (deterministic, no network, no CLI).

    /// Init a scratch repo with a deterministic identity + autocrlf off.
    fn p8_init(dir: &Path) -> git2::Repository {
        let repo = git2::Repository::init(dir).expect("init repo");
        let mut cfg = repo.config().expect("config");
        cfg.set_str("user.name", "Test User").expect("name");
        cfg.set_str("user.email", "test@example.com").expect("email");
        cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        drop(cfg);
        repo
    }

    /// Stage + commit `files` on the CURRENT branch (moves HEAD + worktree).
    fn p8_commit(dir: &Path, msg: &str, files: &[(&str, &str)]) {
        use crate::git::stage::stage_paths;
        for (name, content) in files {
            std::fs::write(dir.join(name), content).expect("write file");
        }
        stage_paths(
            dir,
            &files.iter().map(|(n, _)| n.to_string()).collect::<Vec<_>>(),
        )
        .expect("stage");
        crate::git::commit::create_commit(dir, msg, None, false).expect("commit");
    }

    /// Build a commit on `refname` from `parent`'s tree with the given top-level
    /// file additions/modifications, WITHOUT moving HEAD or touching the
    /// worktree. Creates `refname` if absent. Used to advance a `topic` branch
    /// (FF fixtures) or to build a divergent tip (non-FF fixtures).
    fn p8_commit_on_ref(
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

    fn p8_head_oid(repo: &git2::Repository) -> git2::Oid {
        repo.head()
            .expect("HEAD")
            .peel_to_commit()
            .expect("peel")
            .id()
    }

    /// Enumerate the stash stack via a FRESH handle (state is read from disk).
    fn p8_stash_count(dir: &Path) -> usize {
        let mut repo = git2::Repository::open(dir).expect("open");
        let mut n = 0usize;
        repo.stash_foreach(|_i, _msg, _oid| {
            n += 1;
            true
        })
        .expect("stash_foreach");
        n
    }

    fn p8_read(dir: &Path, name: &str) -> String {
        std::fs::read_to_string(dir.join(name)).expect("read file")
    }

    // ---- Row 1: Not-dirty FF unchanged (identical to pre-P8) ---------------

    /// FF-able upstream, CLEAN tree -> `FastForwarded { stashed: false }`,
    /// exactly as P3c. No stash created; HEAD moves to the target.
    #[test]
    fn p8_1_not_dirty_ff_unchanged() {
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(dir.path(), "base", &[("a.txt", "base\n")]);
        let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
        // topic descends from base (adds a file) -> FF-able. HEAD stays on main.
        let topic = p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("feature.txt", "feature\n")],
            "topic advance",
        );
        // Read the real default branch name (libgit2 honors init.defaultBranch,
        // so it may be "master" or "main" depending on machine config).
        let branch = repo
            .head()
            .expect("HEAD")
            .shorthand()
            .expect("shorthand")
            .to_string();

        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        assert_eq!(
            outcome,
            MergeOutcome::FastForwarded {
                branch: branch.clone(),
                to: topic.to_string(),
                stashed: false,
            },
            "clean FF must report stashed:false and target = topic tip"
        );

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert_eq!(p8_head_oid(&repo), topic, "HEAD must move to topic tip");
        assert_eq!(p8_read(dir.path(), "feature.txt"), "feature\n");
        assert_eq!(p8_stash_count(dir.path()), 0, "no stash created on clean FF");
    }

    // ---- Row 2 (matrix #2): Dirty (unstaged) FF round-trip -----------------

    /// Unrelated tracked file edited but UNSTAGED; FF-able upstream ->
    /// `FastForwarded { stashed: true }`. HEAD moves to target AND the local
    /// edit is present in the worktree afterward (autostash restored).
    #[test]
    fn p8_2_dirty_unstaged_ff_round_trip() {
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(
            dir.path(),
            "base",
            &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
        );
        let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
        // topic only ADDS feature.txt -> FF checkout won't touch unrelated.txt.
        let topic = p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("feature.txt", "feature\n")],
            "topic advance",
        );

        let branch = repo
            .head()
            .expect("HEAD")
            .shorthand()
            .expect("shorthand")
            .to_string();

        // Dirty: edit an unrelated tracked file, leave it UNSTAGED.
        std::fs::write(dir.path().join("unrelated.txt"), "locally edited\n")
            .expect("edit unrelated");

        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        assert_eq!(
            outcome,
            MergeOutcome::FastForwarded {
                branch: branch.clone(),
                to: topic.to_string(),
                stashed: true,
            },
            "dirty FF must report stashed:true"
        );

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert_eq!(p8_head_oid(&repo), topic, "HEAD must move to topic tip");
        assert_eq!(
            p8_read(dir.path(), "feature.txt"),
            "feature\n",
            "FF must have brought in topic's new file"
        );
        assert_eq!(
            p8_read(dir.path(), "unrelated.txt"),
            "locally edited\n",
            "the stashed unstaged edit must be restored in the worktree"
        );
        assert_eq!(p8_stash_count(dir.path()), 0, "stash applied + dropped");

        // Oracle: replay the SAME history through the real `git` CLI with
        // --autostash and compare BEHAVIOR (not commit oids — those differ
        // across independently-built repos because a commit hash includes the
        // committer timestamp). Parity we assert: the FF landed feature.txt and
        // the unstaged edit was restored byte-identically. Skipped (not a hard
        // failure) if git is unavailable on PATH.
        if let Some((cli_feature, cli_unrelated)) = p8_git_cli_autostash_ff_oracle() {
            assert_eq!(
                "feature\n", cli_feature,
                "`git merge --autostash` FF must also bring in feature.txt"
            );
            assert_eq!(
                "locally edited\n", cli_unrelated,
                "`git merge --autostash` also restores the unstaged edit"
            );
            // Our worktree must match the CLI's for both files.
            assert_eq!(p8_read(dir.path(), "feature.txt"), cli_feature);
            assert_eq!(p8_read(dir.path(), "unrelated.txt"), cli_unrelated);
        }
    }

    /// Optional CLI oracle for row 2: build the identical fixture in a fresh
    /// scratch repo, run real `git merge --autostash topic`, return the
    /// resulting `feature.txt` + restored `unrelated.txt` worktree contents.
    /// Returns None if `git` is not runnable so the test degrades to git2-only
    /// assertions. Commit oids are intentionally NOT returned: they cannot match
    /// across two independently-built repos (timestamp-dependent hashes).
    fn p8_git_cli_autostash_ff_oracle() -> Option<(String, String)> {
        use std::process::Command;
        let dir = crate::testutil::scratch_dir();
        let p = dir.path();
        let git = |args: &[&str]| -> Option<std::process::Output> {
            Command::new("git").current_dir(p).args(args).output().ok()
        };
        // Probe git availability first.
        let probe = git(&["--version"])?;
        if !probe.status.success() {
            return None;
        }
        let run = |args: &[&str]| -> bool {
            git(args).map(|o| o.status.success()).unwrap_or(false)
        };
        if !run(&["init", "-q"]) {
            return None;
        }
        run(&["config", "user.name", "Test User"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "core.autocrlf", "false"]);
        std::fs::write(p.join("a.txt"), "base\n").ok()?;
        std::fs::write(p.join("unrelated.txt"), "orig\n").ok()?;
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "base"]);
        // Capture the real default branch (master/main), don't assume.
        let default_branch = {
            let o = git(&["rev-parse", "--abbrev-ref", "HEAD"])?;
            String::from_utf8_lossy(&o.stdout).trim().to_string()
        };
        // topic = base + feature.txt, built without moving HEAD.
        run(&["branch", "topic"]);
        run(&["checkout", "-q", "topic"]);
        std::fs::write(p.join("feature.txt"), "feature\n").ok()?;
        run(&["add", "-A"]);
        run(&["commit", "-q", "-m", "topic advance"]);
        run(&["checkout", "-q", &default_branch]);
        // Dirty unstaged edit, then autostash FF.
        std::fs::write(p.join("unrelated.txt"), "locally edited\n").ok()?;
        if !run(&["merge", "--autostash", "--ff-only", "topic"]) {
            return None;
        }
        let feature = std::fs::read_to_string(p.join("feature.txt")).ok()?;
        let unrelated = std::fs::read_to_string(p.join("unrelated.txt")).ok()?;
        Some((feature, unrelated))
    }

    // ---- Row 3: Dirty (STAGED) FF round-trip -------------------------------

    /// Stage an unrelated change, then FF -> `FastForwarded { stashed: true }`.
    /// The change CONTENT survives. Per OPEN Q#1 (no REINSTATE_INDEX) it comes
    /// back as an UNSTAGED worktree change, NOT re-staged — asserted explicitly.
    #[test]
    fn p8_3_dirty_staged_ff_round_trip() {
        use crate::git::stage::stage_paths;
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(
            dir.path(),
            "base",
            &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
        );
        let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
        let topic = p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("feature.txt", "feature\n")],
            "topic advance",
        );

        let branch = repo
            .head()
            .expect("HEAD")
            .shorthand()
            .expect("shorthand")
            .to_string();

        // Dirty: edit + STAGE an unrelated tracked file.
        std::fs::write(dir.path().join("unrelated.txt"), "staged edit\n").expect("edit");
        stage_paths(dir.path(), &["unrelated.txt".to_string()]).expect("stage");

        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        assert_eq!(
            outcome,
            MergeOutcome::FastForwarded {
                branch: branch.clone(),
                to: topic.to_string(),
                stashed: true,
            },
            "dirty (staged) FF must report stashed:true"
        );

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        assert_eq!(p8_head_oid(&repo), topic);
        assert_eq!(
            p8_read(dir.path(), "unrelated.txt"),
            "staged edit\n",
            "the staged change CONTENT must survive the autostash round-trip"
        );

        // OPEN Q#1: no REINSTATE_INDEX -> the change returns as UNSTAGED, i.e.
        // worktree-modified, NOT index-modified. Assert the split explicitly.
        let mut so = git2::StatusOptions::new();
        so.include_untracked(false);
        let statuses = repo.statuses(Some(&mut so)).expect("statuses");
        let entry = statuses
            .iter()
            .find(|e| e.path().ok() == Some("unrelated.txt"))
            .expect("unrelated.txt must show a pending change");
        assert!(
            entry.status().contains(git2::Status::WT_MODIFIED),
            "restored change must be an UNSTAGED (worktree) modification"
        );
        assert!(
            !entry.status().contains(git2::Status::INDEX_MODIFIED),
            "OPEN Q#1: without REINSTATE_INDEX the change must NOT be re-staged"
        );
        assert_eq!(p8_stash_count(dir.path()), 0, "stash applied + dropped");
    }

    // ---- Row 4 (matrix #3): Dirty clean normal merge -----------------------

    /// Unrelated dirty edit + a non-FF but cleanly-mergeable branch ->
    /// `Merged { stashed: true }`. Assert a 2-parent merge commit AND the dirty
    /// edit preserved.
    #[test]
    fn p8_4_dirty_clean_normal_merge() {
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(
            dir.path(),
            "base",
            &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
        );
        let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
        // topic diverges from base by ADDING topic-only.txt (from base tree).
        p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("topic-only.txt", "topic\n")],
            "topic side",
        );
        // main advances by ADDING main-only.txt -> divergent (non-FF), clean.
        p8_commit(dir.path(), "main side", &[("main-only.txt", "main\n")]);

        // Dirty: unrelated tracked edit the merge never touches, UNSTAGED.
        std::fs::write(dir.path().join("unrelated.txt"), "locally edited\n").expect("edit");

        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        let oid = match &outcome {
            MergeOutcome::Merged { oid, stashed } => {
                assert!(*stashed, "clean normal merge over dirty tree must be stashed:true");
                oid.clone()
            }
            other => panic!("expected Merged, got {other:?}"),
        };

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        assert_eq!(repo.state(), git2::RepositoryState::Clean);
        let merge_commit = repo
            .find_commit(git2::Oid::from_str(&oid).expect("oid"))
            .expect("merge commit");
        assert_eq!(
            merge_commit.parent_count(),
            2,
            "a normal merge must produce a 2-parent commit"
        );
        assert_eq!(
            p8_head_oid(&repo),
            merge_commit.id(),
            "HEAD must point at the new merge commit"
        );
        // Both sides' files present + the dirty edit restored.
        assert_eq!(p8_read(dir.path(), "main-only.txt"), "main\n");
        assert_eq!(p8_read(dir.path(), "topic-only.txt"), "topic\n");
        assert_eq!(
            p8_read(dir.path(), "unrelated.txt"),
            "locally edited\n",
            "the stashed dirty edit must be restored after the merge commit"
        );
        assert_eq!(p8_stash_count(dir.path()), 0, "stash applied + dropped");
    }

    // ---- Row 5 (matrix #4): Stash-pop conflict -----------------------------

    /// Locally edit file X (unstaged); the FF target ALSO modifies X so the
    /// autostash re-apply conflicts -> `StashPopConflicts { paths: ["x.txt"] }`.
    /// repo.state() == Clean, X has conflict markers, stash RETAINED (count==1).
    #[test]
    fn p8_5_stash_pop_conflict() {
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(dir.path(), "base", &[("x.txt", "line1\nline2\nline3\n")]);
        let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
        // topic (FF-able) modifies line2 -> "TOPIC".
        let topic = p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("x.txt", "line1\nTOPIC\nline3\n")],
            "topic edits x",
        );

        // Local UNSTAGED edit of the SAME line -> conflicts on stash re-apply.
        std::fs::write(dir.path().join("x.txt"), "line1\nLOCAL\nline3\n").expect("edit x");

        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        match &outcome {
            MergeOutcome::StashPopConflicts { head, paths } => {
                assert_eq!(head, &topic.to_string(), "head = FF target");
                assert_eq!(paths, &vec!["x.txt".to_string()], "x.txt conflicted on pop");
            }
            other => panic!("expected StashPopConflicts, got {other:?}"),
        }

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        // A conflicted stash-apply is NOT a merge op: state stays Clean.
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Clean,
            "stash-pop conflict must leave state Clean (not Merge)"
        );
        assert_eq!(
            p8_head_oid(&repo),
            topic,
            "FF already landed: HEAD is at the target"
        );
        let x = p8_read(dir.path(), "x.txt");
        assert!(
            x.contains("<<<<<<<") && x.contains(">>>>>>>"),
            "x.txt must contain conflict markers, got:\n{x}"
        );
        assert_eq!(
            p8_stash_count(dir.path()),
            1,
            "libgit2 does NOT drop the stash on a conflicting pop: it is RETAINED"
        );
    }

    // ---- Row 6 (matrix #5): Normal-merge paused + dirty --------------------

    /// A conflicting merge on file X PLUS an unrelated dirty file Y ->
    /// `Conflicts { stashed: true }`. repo.state() == Merge, MERGE_HEAD present,
    /// stash RETAINED (count==1), Y's worktree content at the HEAD version
    /// (Y was stashed, not restored — deferred re-apply, OPEN Q#2).
    #[test]
    fn p8_6_normal_merge_paused_plus_dirty() {
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(
            dir.path(),
            "base",
            &[("x.txt", "base\n"), ("y.txt", "y-base\n")],
        );
        let base = repo.find_commit(p8_head_oid(&repo)).expect("base");
        // topic diverges: x.txt -> "topic".
        p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("x.txt", "topic\n")],
            "topic edits x",
        );
        // main diverges: x.txt -> "main" (conflict) ; y.txt untouched by both.
        p8_commit(dir.path(), "main edits x", &[("x.txt", "main\n")]);

        // Unrelated dirty file Y (UNSTAGED). The merge never touches Y, so Y
        // lands on the autostash and the paused merge does not restore it.
        std::fs::write(dir.path().join("y.txt"), "y-locally-edited\n").expect("edit y");

        let outcome = merge_branch(dir.path(), "topic", false).expect("merge");
        assert_eq!(
            outcome,
            MergeOutcome::Conflicts {
                paths: vec!["x.txt".to_string()],
                stashed: true,
            },
            "paused conflicting merge over a dirty tree must be stashed:true"
        );

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Merge,
            "a conflicting merge must PAUSE in state Merge"
        );
        assert!(
            repo.path().join("MERGE_HEAD").exists(),
            "MERGE_HEAD must be written for the paused merge"
        );
        assert_eq!(
            p8_stash_count(dir.path()),
            1,
            "deferred re-apply (OPEN Q#2): the autostash is RETAINED on the stack"
        );
        // Y was stashed and NOT re-applied -> worktree Y is at HEAD (main) value.
        assert_eq!(
            p8_read(dir.path(), "y.txt"),
            "y-base\n",
            "Y's dirty edit is on the stash; worktree Y must be at the HEAD version"
        );
    }

    // ---- Row 7 (matrix #6): Rollback on blocked FF -------------------------

    /// A dirty tracked edit + an UNTRACKED file that the FF would create ->
    /// `Err(CheckoutConflict)`. repo.state() == Clean, the dirty tracked edit is
    /// restored in the worktree, and stash_foreach count == 0 (rolled back —
    /// nothing left on the stack).
    #[test]
    fn p8_7_rollback_on_blocked_ff() {
        let dir = crate::testutil::scratch_dir();
        let repo = p8_init(dir.path());

        p8_commit(
            dir.path(),
            "base",
            &[("a.txt", "base\n"), ("unrelated.txt", "orig\n")],
        );
        let base_oid = p8_head_oid(&repo);
        let base = repo.find_commit(base_oid).expect("base");
        // topic (FF-able) ADDS new.txt with committed content "from-topic".
        p8_commit_on_ref(
            &repo,
            "refs/heads/topic",
            &base,
            &[("new.txt", "from-topic\n")],
            "topic adds new.txt",
        );

        // Dirty tracked edit (UNSTAGED) -> triggers the autostash.
        std::fs::write(dir.path().join("unrelated.txt"), "locally edited\n").expect("edit");
        // UNTRACKED file physically in the way of the FF checkout of new.txt.
        // INCLUDE_UNTRACKED is off, so this is NOT stashed and blocks the SAFE
        // checkout with a Conflict.
        std::fs::write(dir.path().join("new.txt"), "untracked in the way\n").expect("untracked");

        let err = merge_branch(dir.path(), "topic", false).expect_err("blocked FF must error");
        assert!(
            matches!(err, AppError::CheckoutConflict(_)),
            "an untracked file blocking the FF checkout must map to CheckoutConflict, got {err:?}"
        );

        let repo = git2::Repository::open(dir.path()).expect("reopen");
        assert_eq!(
            repo.state(),
            git2::RepositoryState::Clean,
            "a failed merge_branch must leave state Clean"
        );
        assert_eq!(
            p8_head_oid(&repo),
            base_oid,
            "the FF set_target never ran: HEAD is unchanged at base"
        );
        assert_eq!(
            p8_read(dir.path(), "unrelated.txt"),
            "locally edited\n",
            "rollback_stash must restore the dirty tracked edit"
        );
        assert_eq!(
            p8_read(dir.path(), "new.txt"),
            "untracked in the way\n",
            "the untracked file must be left untouched"
        );
        assert_eq!(
            p8_stash_count(dir.path()),
            0,
            "rollback popped the stash: nothing left on the stack"
        );
    }
}
