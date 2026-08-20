//! Branch creation: `create_branch` and `create_branch_here` (M5/P11).

use std::path::Path;

use crate::error::AppError;
use crate::git::stash;

use super::{checkout_branch, delete_branch, open_repo_at, validate_branch_name};

/// Blocking. Creates local branch `name` at the current HEAD commit.
/// Does NOT check out.
pub fn create_branch(workdir: &Path, name: &str) -> Result<(), AppError> {
    validate_branch_name(name)?;
    let repo = open_repo_at(workdir)?;

    let head_commit = match repo.head().and_then(|h| h.peel_to_commit()) {
        Ok(commit) => commit,
        Err(e)
            if matches!(
                e.code(),
                git2::ErrorCode::UnbornBranch | git2::ErrorCode::NotFound
            ) =>
        {
            return Err(AppError::Git(
                "cannot create a branch: the repository has no commits yet".to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    };

    if let Err(e) = repo.branch(name, &head_commit, /* force */ false) {
        if e.code() == git2::ErrorCode::Exists {
            return Err(AppError::BranchExists(format!(
                "branch '{name}' already exists"
            )));
        }
        return Err(e.into());
    }
    Ok(())
}

/// Result of `create_branch_here`. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchHereResult {
    /// true when uncommitted work was auto-stashed and carried across.
    pub stashed: bool,
    /// Present only when `stashed`; the outcome of re-applying the stash on the
    /// new branch (`Applied` = clean carry-over, `Conflicts{paths}` = carried
    /// with markers, stash retained). `None` when the worktree was clean.
    pub apply: Option<stash::ApplyStashOutcome>,
}

/// Blocking. Create local branch `name` at commit `oid`, then check it out,
/// carrying any uncommitted work across via auto-stash. Composes existing
/// primitives; NEVER lossy (working changes are recovered on every failure path).
///
/// Ordered algorithm (P11 §1.1). Errors: `invalidName` | `branchExists` |
/// `operationInProgress` (via `create_stash`) | `configMissing` (via
/// `create_stash`) | `checkoutConflict` (defensive, via `checkout_branch`) |
/// `git` (bad/unknown oid, or any other libgit2 error).
pub fn create_branch_here(
    workdir: &Path,
    name: &str,
    oid: &str,
) -> Result<CreateBranchHereResult, AppError> {
    // 1. Validate & resolve FIRST — zero side effects on any failure here.
    validate_branch_name(name)?;
    let repo = open_repo_at(workdir)?;

    let target_oid = git2::Oid::from_str(oid).map_err(|_| {
        AppError::Git(format!(
            "cannot create branch: '{oid}' is not a valid commit id"
        ))
    })?;
    let target = repo.find_commit(target_oid).map_err(|_| {
        AppError::Git(format!("cannot create branch: commit '{oid}' not found"))
    })?;

    // 2. Pre-check branch existence BEFORE any side effect, so a `BranchExists`
    //    can never strand a stash.
    if repo
        .find_branch(name, git2::BranchType::Local)
        .is_ok()
    {
        return Err(AppError::BranchExists(format!(
            "branch '{name}' already exists"
        )));
    }

    // 3. Auto-stash. `create_stash` owns the dirty-vs-clean decision (clean tree
    //    → created:false) AND the mid-merge/rebase guard (OperationInProgress).
    //    `configMissing` may surface here (stash authors a commit) — let it
    //    propagate. `stashed == true` means work must be re-applied afterwards.
    let stashed =
        stash::create_stash(workdir, None, stash::StashScope::AllWithUntracked)?.created;

    // 4. Create the branch ref at the resolved commit. On failure, restore the
    //    stashed work onto the original branch (best-effort) before returning.
    if let Err(e) = repo.branch(name, &target, /* force */ false) {
        if stashed {
            let _ = stash::pop_stash(workdir, 0, false, None);
        }
        if e.code() == git2::ErrorCode::Exists {
            return Err(AppError::BranchExists(format!(
                "branch '{name}' already exists"
            )));
        }
        return Err(e.into());
    }

    // 5. SAFE checkout the new branch. On failure, roll back so nothing is
    //    stranded: delete the just-created ref and restore stashed work (both
    //    best-effort). Post-stash the worktree is clean, so this is defensive.
    if let Err(e) = checkout_branch(workdir, name) {
        let _ = delete_branch(workdir, name);
        if stashed {
            let _ = stash::pop_stash(workdir, 0, false, None);
        }
        return Err(e);
    }

    // 6. Re-apply the carried work iff stashed. `pop_stash` drops on clean apply
    //    and RETAINS on conflict (never lossy). A `Conflicts` outcome is a
    //    SUCCESS return (branch created & checked out; changes present w/ markers).
    if stashed {
        let outcome = stash::pop_stash(workdir, 0, false, None)?;
        return Ok(CreateBranchHereResult {
            stashed: true,
            apply: Some(outcome),
        });
    }

    // 7. Clean case.
    Ok(CreateBranchHereResult {
        stashed: false,
        apply: None,
    })
}
