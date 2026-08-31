//! Remote-tracking checkout and remote-tracking ref deletion (P6 contract §2.2/§2.3).

use std::path::Path;

use crate::error::AppError;
use crate::git::worktree_reuse;

use super::open_repo_at;

/// Blocking. GitKraken-style remote checkout: create (or reuse) a LOCAL tracking
/// branch for the remote-tracking ref `remote_shorthand` ("<remote>/<branch>")
/// and safe-checkout it. SAFE checkout only — never force (P6 contract §2.2).
///
/// When NO local branch of the same short name exists, a new local tracking
/// branch is created at the remote tip with its upstream set, then checked out.
///
/// When a local branch of that name ALREADY exists, its tip is compared to the
/// remote tip by ancestry and the ref is only ever moved forward:
/// - equal tips → check out the local branch as-is (no ref move);
/// - local strictly BEHIND (fast-forwardable) → safe-checkout the remote tip,
///   then fast-forward `refs/heads/<name>` onto it, ending on the local branch
///   at the remote's commit;
/// - local strictly AHEAD → check out the local branch as-is (it already
///   contains everything the remote has; no ref move);
/// - DIVERGED (neither tip is an ancestor of the other) → error, change nothing.
///
/// The diverged/error and "checked out elsewhere" conditions are detected BEFORE
/// any worktree or ref mutation, and the SAFE checkout runs before any ref move,
/// so an error (divergence or conflict) leaves HEAD + worktree + refs untouched
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

    // Decide the checkout target, whether we create, and whether we must
    // fast-forward the local ref — ALL before touching the worktree, so a
    // divergence error or conflict leaves everything untouched and creates
    // nothing.
    let (checkout_oid, created, fast_forward) =
        match repo.find_branch(local_name, git2::BranchType::Local) {
            Ok(existing) => {
                let local_tip = existing.get().target().ok_or_else(|| {
                    AppError::Git(format!("branch '{local_name}' has no target commit"))
                })?;
                if local_tip == remote_tip {
                    // Equal tips: check out the local branch as-is, no ref move.
                    (local_tip, false, false)
                } else {
                    let base = repo.merge_base(local_tip, remote_tip)?;
                    if base == local_tip {
                        // Local strictly BEHIND → fast-forwardable. Check out the
                        // remote tip, then move the local ref onto it.
                        (remote_tip, false, true)
                    } else if base == remote_tip {
                        // Local strictly AHEAD → already contains the remote; keep
                        // the local branch where it is.
                        (local_tip, false, false)
                    } else {
                        // Diverged: neither tip is an ancestor of the other.
                        // Refuse BEFORE any mutation.
                        return Err(AppError::Git(format!(
                            "cannot check out '{remote_shorthand}': local branch \
                             '{local_name}' has diverged from it; not fast-forwardable"
                        )));
                    }
                }
            }
            Err(e) if e.code() == git2::ErrorCode::NotFound => (remote_tip, true, false),
            Err(e) => return Err(e.into()),
        };

    // Reusing an EXISTING local branch: refuse if it is checked out in another
    // worktree (a just-created branch cannot be). Mirrors `checkout_branch`;
    // runs before any side effect. Reuses the already-open handle (P88b/B2a).
    if !created {
        if let Some(other) =
            worktree_reuse::branch_checked_out_elsewhere_with(&repo, workdir, local_name)?
        {
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
    } else if fast_forward {
        // Local was strictly behind and the worktree is now at the remote tip:
        // fast-forward the local ref onto it (force-update refs/heads/<name>).
        repo.find_reference(&format!("refs/heads/{local_name}"))?
            .set_target(
                remote_tip,
                "bonsai: fast-forward on remote checkout",
            )?;
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
