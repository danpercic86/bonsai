//! Remote-tracking checkout and remote-tracking ref deletion (P6 contract §2.2/§2.3).

use std::path::Path;

use crate::error::AppError;
use crate::git::worktree;

use super::open_repo_at;

/// Blocking. GitKraken-style remote checkout: create (or reuse) a LOCAL tracking
/// branch for the remote-tracking ref `remote_shorthand` ("<remote>/<branch>")
/// and safe-checkout it. SAFE checkout only — never force (P6 contract §2.2).
///
/// A name collision (a local branch of the same short name already exists) just
/// switches to the existing local branch — it is NOT repointed. Safe checkout
/// runs before any ref mutation, so a conflict leaves HEAD + worktree untouched
/// and creates nothing.
pub fn checkout_remote(workdir: &Path, remote_shorthand: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    // Split on the FIRST '/': remote names contain no '/'. The remote segment
    // is validated non-empty but not otherwise needed here.
    let local_name = match remote_shorthand.split_once('/') {
        Some((r, l)) if !r.is_empty() && !l.is_empty() => l,
        _ => {
            return Err(AppError::InvalidName(format!(
                "invalid remote branch name: '{remote_shorthand}'"
            )));
        }
    };

    // Find the remote-tracking ref and its tip.
    let remote_branch = match repo.find_branch(remote_shorthand, git2::BranchType::Remote) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!(
                "remote-tracking branch '{remote_shorthand}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    let remote_tip = remote_branch.get().target().ok_or_else(|| {
        AppError::Git(format!(
            "remote-tracking branch '{remote_shorthand}' has no target commit"
        ))
    })?;

    // Decide the checkout target + whether we create — BEFORE touching the
    // worktree, so a conflict leaves everything untouched and creates nothing.
    let (checkout_oid, created) = match repo.find_branch(local_name, git2::BranchType::Local) {
        Ok(existing) => {
            let oid = existing.get().target().ok_or_else(|| {
                AppError::Git(format!("branch '{local_name}' has no target commit"))
            })?;
            (oid, false)
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => (remote_tip, true),
        Err(e) => return Err(e.into()),
    };

    // Reusing an EXISTING local branch: refuse if it is checked out in another
    // worktree (a just-created branch cannot be). Mirrors `checkout_branch`;
    // runs before any side effect.
    if !created {
        if let Some(other) = worktree::branch_checked_out_elsewhere(workdir, local_name)? {
            return Err(AppError::BranchCheckedOutElsewhere(format!(
                "branch '{local_name}' is already checked out at '{other}'"
            )));
        }
    }

    // SAFE checkout FIRST (matches `checkout_branch`): a conflict leaves HEAD +
    // worktree untouched AND nothing has been created yet.
    let obj = repo.find_object(checkout_oid, None)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe(); // DEFAULT SAFE MODE — never .force()
    match repo.checkout_tree(&obj, Some(&mut opts)) {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            return Err(AppError::CheckoutConflict(format!(
                "cannot switch to '{local_name}': local changes would be overwritten. \
                 Commit or discard them first."
            )));
        }
        Err(e) => return Err(e.into()),
    }

    // Checkout succeeded — only now mutate refs.
    if created {
        let remote_commit = repo.find_commit(remote_tip)?;
        match repo.branch(local_name, &remote_commit, /* force */ false) {
            Ok(mut new_branch) => {
                // Best-effort upstream — a set failure is still a successful
                // checkout; log and continue, do NOT roll back.
                if let Err(e) = new_branch.set_upstream(Some(remote_shorthand)) {
                    eprintln!(
                        "bonsai: checked out '{local_name}' but failed to set upstream \
                         '{remote_shorthand}': {e}"
                    );
                }
            }
            // Race: created between our probe and now — just proceed to set_head.
            Err(e) if e.code() == git2::ErrorCode::Exists => {}
            Err(e) => return Err(e.into()),
        }
    }

    repo.set_head(&format!("refs/heads/{local_name}"))?;
    Ok(())
}

/// Blocking. Deletes the LOCAL remote-tracking ref `name` ("origin/feature").
/// Local-only: does NOT contact the server. No merged-check (a local-branch
/// concept only) (P6 contract §2.3).
pub fn delete_remote_tracking(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    let mut branch = match repo.find_branch(name, git2::BranchType::Remote) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!(
                "remote-tracking branch '{name}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };

    branch.delete()?;
    Ok(())
}
