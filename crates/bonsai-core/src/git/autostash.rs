//! Shared autostash for operations that need a clean tree (merge / cherry-pick /
//! revert). Stashes TRACKED changes (index + worktree), reset to HEAD, so the
//! subsequent checkout cannot clobber the user's edits. On any failure the stash
//! is RETAINED at stash@{0} — never silently dropped (no data loss).
//!
//! Extracted from `merge.rs` (P3c) so cherry-pick, revert AND merge share ONE
//! implementation (P47 §2.1, flag F1). `stash_save` is parameterized on the
//! stash `label`; the behavior is otherwise byte-identical to the original merge
//! helpers.

use std::path::Path;

use crate::error::AppError;
use crate::git::conflict::list_conflicts;

/// Result of re-applying the autostash after a successful operation.
pub enum PopResult {
    /// Clean re-apply; the stash was dropped (equivalent to a clean pop).
    Restored,
    /// Re-apply produced conflict markers; the stash is RETAINED at stash@{0}.
    Conflicted(Vec<String>),
}

/// True iff the working tree has any TRACKED change (staged or unstaged).
/// Untracked and ignored files are excluded (mirrors git's autostash default).
pub fn is_dirty(repo: &git2::Repository) -> Result<bool, AppError> {
    let mut so = git2::StatusOptions::new();
    so.include_untracked(false).include_ignored(false);
    Ok(!repo.statuses(Some(&mut so))?.is_empty())
}

/// Autostash tracked (index + worktree) changes with `label`, resetting the
/// tree to HEAD so a subsequent SAFE checkout cannot conflict with the user's
/// own edits. NOT KEEP_INDEX (would leave the tree dirty), NOT INCLUDE_UNTRACKED
/// (matches git's autostash default).
pub fn stash_save(
    repo: &mut git2::Repository,
    sig: &git2::Signature,
    label: &str,
) -> Result<(), AppError> {
    repo.stash_save2(sig, Some(label), Some(git2::StashFlags::DEFAULT))?;
    Ok(())
}

/// On a mutation failure AFTER `stash_save` but BEFORE the terminal outcome,
/// try to restore the user's original dirty state, then return the original
/// error. Drops the stash ONLY on a genuinely clean restore; if the restore
/// conflicts or fails, the stash is left on the stack and the error message is
/// augmented to say so — never a silent success or a silent drop.
///
/// No-op passthrough when `!stashed`.
pub fn rollback_and_map(repo: &mut git2::Repository, stashed: bool, err: AppError) -> AppError {
    if !stashed {
        return err;
    }
    // Attempt to restore the user's original dirty state. Most callers reach
    // here with a clean tree (just-stashed, or an error path that reset the
    // index), so the apply is clean. A caller that already checked out an
    // incoming tree could conflict on restore.
    //
    // Use stash_apply (NOT stash_pop): this libgit2 applies a *content*
    // conflict as Ok(()) with markers and stash_pop would then silently DROP
    // the stash (data loss). We inspect the index after Ok and only drop on a
    // genuinely clean restore; otherwise the stash is RETAINED at stash@{0} and
    // we augment the error to say so — never a silent success.
    let augment = |err: AppError| -> AppError {
        let base = match &err {
            AppError::CheckoutConflict(m) | AppError::Git(m) => m.clone(),
            other => other.to_string(),
        };
        AppError::Git(format!("{base} (your changes are safe at stash@{{0}})"))
    };
    match repo.stash_apply(0, Some(&mut git2::StashApplyOptions::new())) {
        Ok(()) => match repo.index() {
            Ok(index) if !index.has_conflicts() => {
                // Clean restore → drop the now-redundant stash and return the
                // original error (state is as if nothing happened).
                let _ = repo.stash_drop(0);
                err
            }
            // Conflicted (or unreadable) restore: LEAVE the stash on the stack
            // (never drop) and tell the user where their changes are.
            _ => augment(err),
        },
        // Could not auto-restore: stash_apply never drops → stash retained.
        Err(_) => augment(err),
    }
}

/// Re-apply the autostash after a SUCCESSFUL finalize.
///
/// Uses `stash_apply` (NOT `stash_pop`) so WE control dropping. This libgit2
/// version applies a *content* conflict as `Ok(())` — writing conflict markers
/// into the worktree and conflict entries into the index — rather than
/// returning `GIT_ECONFLICT`, and `stash_pop` would then DROP the stash on that
/// silent conflict (data loss). So we must inspect the index after `Ok`, not
/// trust the return code, before deciding to drop. No REINSTATE_INDEX: staged
/// changes return as unstaged.
pub fn pop_after_success(
    repo: &mut git2::Repository,
    workdir: &Path,
) -> Result<PopResult, AppError> {
    let mut opts = git2::StashApplyOptions::new();
    match repo.stash_apply(0, Some(&mut opts)) {
        Ok(()) => {
            if repo.index()?.has_conflicts() {
                // Conflicted re-apply: LEAVE the stash on the stack (do NOT
                // drop) → retained at stash@{0} for the user to resolve.
                let paths = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
                Ok(PopResult::Conflicted(paths))
            } else {
                // Clean apply → now drop, equivalent to a clean pop.
                repo.stash_drop(0)?;
                Ok(PopResult::Restored)
            }
        }
        // A checkout-level conflict (rare) means nothing droppable was applied
        // → stash retained.
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            let paths = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
            Ok(PopResult::Conflicted(paths))
        }
        // Rare non-conflict failure: the operation HAS ALREADY landed and
        // stash_apply never drops, so the stash is retained. Report success and
        // point the user at their safe stash.
        Err(e) => Err(AppError::Git(format!(
            "operation succeeded, but re-applying your stashed changes failed: {e}. \
             Your changes are safe at stash@{{0}}."
        ))),
    }
}
