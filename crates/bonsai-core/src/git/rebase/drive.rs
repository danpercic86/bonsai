//! Rebase drive loop and helpers (extracted from `rebase.rs`, unchanged).
//!
//! The plan-driving loop plus its small git2 helpers. Re-exported to the parent
//! so the public entry points call these paths unchanged.

use std::path::Path;

use crate::error::AppError;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;

/// Result of driving the rebase plan to completion or to the next pause.
pub(super) enum DriveResult {
    Completed {
        head: String,
        steps: u32,
    },
    Paused {
        paths: Vec<String>,
        current_step: u32,
        total_steps: u32,
    },
}

/// Maps a git2 error raised while starting or driving a rebase: a `Conflict`
/// code means the checkout would overwrite local changes (surface the friendly
/// CheckoutConflict), everything else falls through to the generic `From`
/// (`AppError::Git`). Applied at the git2-error origin because `AppError` does
/// not carry git2 codes.
pub(super) fn map_conflict(e: git2::Error) -> AppError {
    if e.code() == git2::ErrorCode::Conflict {
        AppError::CheckoutConflict(super::CONFLICT_MSG.to_string())
    } else {
        e.into()
    }
}

/// 1-based current step (== git `msgnum`) and total (== `end`).
fn steps(rebase: &mut git2::Rebase<'_>) -> (u32, u32) {
    let current = rebase
        .operation_current()
        .map(|c| c as u32 + 1)
        .unwrap_or(0);
    let total = rebase.len() as u32;
    (current, total)
}

/// Commits the CURRENT rebase operation, preserving its original author (the
/// `None` author reuses the pick's name/email/author-time) and stamping the
/// resolved `committer`. An `ErrorCode::Applied` means the pick became EMPTY
/// (its change is already on the new base) → DROP it, matching default
/// `git rebase`. (§3.6)
pub(super) fn commit_current(
    rebase: &mut git2::Rebase<'_>,
    committer: &git2::Signature<'_>,
) -> Result<(), AppError> {
    match rebase.commit(None, committer, None) {
        Ok(_) => Ok(()),
        Err(e) if e.code() == git2::ErrorCode::Applied => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Drives the plan: apply each patch, pause on the first conflict (KEEPING the
/// on-disk rebase state — that is NOT an error), else commit it and continue;
/// when the plan is exhausted, `finish()` reattaches HEAD and moves the branch
/// ref. `committer` is stamped on every replayed commit and on `finish()`. (§3.4)
pub(super) fn run_rebase_loop(
    workdir: &Path,
    repo: &git2::Repository,
    rebase: &mut git2::Rebase<'_>,
    committer: &git2::Signature<'_>,
) -> Result<DriveResult, AppError> {
    loop {
        match rebase.next() {
            None => break,                                 // plan exhausted
            Some(Err(e)) => return Err(map_conflict(e)),   // caller decides abort-vs-keep
            Some(Ok(_op)) => {
                // op.kind() is always Pick for plain rebase.
                if repo.index()?.has_conflicts() {
                    let (current_step, total_steps) = steps(rebase);
                    let paths: Vec<String> = list_conflicts(workdir)?
                        .into_iter()
                        .map(|c| c.path)
                        .collect();
                    return Ok(DriveResult::Paused {
                        paths,
                        current_step,
                        total_steps,
                    });
                }
                commit_current(rebase, committer)?;
            }
        }
    }
    // Plan exhausted -> finalize.
    let total = rebase.len() as u32;
    rebase.finish(Some(committer))?;
    let head = repo.head()?.peel_to_commit()?.id().to_string();
    Ok(DriveResult::Completed { head, steps: total })
}

/// START-only cleanup. GUARANTEE: a failed `rebase_branch` restores
/// `RepositoryState::Clean` and leaves no half-initialized rebase. Best-effort,
/// in order; each step ignores its own error. Because §3.1.5 allows a dirty
/// worktree, the only START failure that can touch the worktree is the initial
/// base checkout, which fails atomically as `CheckoutConflict` before any
/// commit is rewritten. (§3.5)
pub(super) fn cleanup_failed_start(repo: &git2::Repository, head_oid: git2::Oid) {
    if let Ok(mut r) = repo.open_rebase(None) {
        let _ = r.abort(); // normal path: full restore
    }
    if repo.state() != git2::RepositoryState::Clean {
        // belt-and-suspenders
        let _ = repo.cleanup_state();
        if let Ok(commit) = repo.find_commit(head_oid) {
            if let Ok(tree) = commit.tree() {
                if let Ok(mut idx) = repo.index() {
                    let _ = idx.read_tree(&tree);
                    let _ = idx.write();
                }
            }
        }
    }
}

/// True for every on-disk rebase repository state.
pub(super) fn is_rebase_state(s: git2::RepositoryState) -> bool {
    matches!(
        s,
        git2::RepositoryState::RebaseMerge
            | git2::RepositoryState::Rebase
            | git2::RepositoryState::RebaseInteractive
    )
}

/// Best-effort display name of the branch being rebased: read
/// `rebase-merge/head-name` (strip `refs/heads/`); after a completed `finish()`
/// the file is gone, so fall back to HEAD's branch name. NEVER errors — the
/// `branch` field is display-only (toasts). (§3.7)
pub(super) fn read_head_from_rebase(repo: &git2::Repository) -> String {
    let head_name_path = repo.path().join("rebase-merge").join("head-name");
    if let Ok(content) = std::fs::read_to_string(&head_name_path) {
        let trimmed = content.trim();
        let name = trimmed.strip_prefix("refs/heads/").unwrap_or(trimmed);
        if !name.is_empty() {
            return name.to_string();
        }
    }
    read_head_info(repo)
        .ok()
        .and_then(|h| h.branch_name)
        .unwrap_or_default()
}
