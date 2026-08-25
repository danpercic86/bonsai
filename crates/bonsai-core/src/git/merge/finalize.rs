//! Concluding a merge (P3c contract §4.4–§4.5): the shared commit core
//! [`finalize_merge_commit`], the paused-merge `commit_merge` entry, and
//! `abort_merge` (approximate `git reset --merge`).

use std::path::Path;

use crate::error::AppError;
use crate::git::activity::GitActivityRecorder;
use crate::git::commit::{self, resolve_signature, CommitResult};
use crate::git::exec::SpawnGitExec;
use crate::git::hooks::{run_hook_nonblocking_streaming, run_hook_streaming, HookName};
use crate::git::signing::{self, resolve_signing};
use crate::git::stage::open_workdir_repo;

use super::{commit_merge_with_activity, MergeHooks};

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
