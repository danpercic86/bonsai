//! Local-branch checkout: plain and dirty-safe autostash (M5/P33).

use std::path::Path;

use crate::error::AppError;
use crate::git::stash;
use crate::git::worktree;

use super::open_repo_at;

/// Blocking. Checks out LOCAL branch `name` (v1: local branch names only —
/// no tags, no oids, no remote-tracking checkout; contract §9).
///
/// SAFE checkout only — NEVER force. `checkout_tree` runs before `set_head`,
/// so a conflict leaves both the worktree and HEAD untouched.
pub fn checkout_branch(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_repo_at(workdir)?;

    let branch = match repo.find_branch(name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!("branch '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };

    // No-op when already checked out (UI hides the action; guard the race).
    if branch.is_head() {
        return Ok(());
    }

    // Refuse if the branch is checked out in ANOTHER worktree (git-like:
    // "fatal: '<b>' is already checked out at '<path>'") — two worktrees on
    // one branch corrupt each other's view. Runs before any side effect.
    if let Some(other) = worktree::branch_checked_out_elsewhere(workdir, name)? {
        return Err(AppError::BranchCheckedOutElsewhere(format!(
            "branch '{name}' is already checked out at '{other}'"
        )));
    }

    let target_oid = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;
    let obj = repo.find_object(target_oid, None)?;

    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe(); // DEFAULT SAFE MODE — never .force()
    match repo.checkout_tree(&obj, Some(&mut opts)) {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            return Err(AppError::CheckoutConflict(format!(
                "cannot switch to '{name}': local changes would be overwritten. \
                 Commit or discard them first."
            )));
        }
        Err(e) => return Err(e.into()),
    }

    repo.set_head(&format!("refs/heads/{name}"))?;
    Ok(())
}

/// Result of `checkout_branch_autostash`. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutResult {
    /// true when uncommitted work was auto-stashed and carried across.
    pub stashed: bool,
    /// true when the switched-to branch was fast-forwarded to its upstream
    /// (behind>0 && ahead==0). false when no upstream, up-to-date, ahead, or
    /// diverged.
    pub fast_forwarded: bool,
    /// Present only when `stashed`; the outcome of re-applying the stash on the
    /// (possibly fast-forwarded) target branch. `Applied` = clean carry-over
    /// (stash dropped); `Conflicts{paths}` = carried with markers, stash
    /// RETAINED at stash@{0}. `None` when the worktree was clean.
    pub apply: Option<stash::ApplyStashOutcome>,
}

/// Blocking. Dirty-safe checkout of LOCAL branch `name`: auto-stash any
/// uncommitted work, SAFE-checkout the target, auto fast-forward the switched-to
/// branch to its upstream tracking ref **without fetching** (local ref math
/// only, when behind and not diverged), then re-apply the stash. A conflicted
/// re-apply is a SUCCESS carrying `apply: Some(Conflicts{..})` (stash retained,
/// never lossy). Composes existing primitives; mirrors `create_branch_here`
/// minus the branch creation, plus the auto-FF step.
///
/// Errors: `branchNotFound` | `branchCheckedOutElsewhere` |
/// `operationInProgress` (via `create_stash`) | `configMissing` (via
/// `create_stash`) | `checkoutConflict` (defensive, via `checkout_branch`) |
/// `git` | `noRepo`.
pub fn checkout_branch_autostash(
    workdir: &Path,
    name: &str,
) -> Result<CheckoutResult, AppError> {
    // 0. Resolve up-front — zero side effects on failure.
    let repo = open_repo_at(workdir)?;
    let branch = match repo.find_branch(name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!("branch '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };
    // No-op when already checked out (UI hides the action; guard the race).
    if branch.is_head() {
        return Ok(CheckoutResult {
            stashed: false,
            fast_forwarded: false,
            apply: None,
        });
    }

    // 0b. Refuse if the branch is checked out in ANOTHER worktree (git-like:
    //     "fatal: '<b>' is already checked out at '<path>'"). Runs before any
    //     side effect, so a refusal changes nothing.
    if let Some(other) = worktree::branch_checked_out_elsewhere(workdir, name)? {
        return Err(AppError::BranchCheckedOutElsewhere(format!(
            "branch '{name}' is already checked out at '{other}'"
        )));
    }

    // 1. Auto-stash. `create_stash` owns the dirty-vs-clean decision (clean tree
    //    → created:false) AND the mid-merge/rebase guard (OperationInProgress).
    //    `configMissing` may surface here (stash authors a commit) — propagate.
    let stashed =
        stash::create_stash(workdir, None, stash::StashScope::AllWithUntracked)?.created;

    // 2. SAFE checkout. On ANY failure, restore stash (best-effort) then return.
    //    Post-stash the worktree is clean, so a real conflict here is defensive.
    if let Err(e) = checkout_branch(workdir, name) {
        if stashed {
            let _ = stash::pop_stash(workdir, 0, false, None);
        }
        return Err(e);
    }

    // 3. AUTO FAST-FORWARD (no fetch). Runs after the switch, before the stash
    //    re-apply, so carried work lands on the fast-forwarded tip. Best-effort
    //    and INFALLIBLE: skips silently (returns false) on any non-FF condition
    //    OR any internal libgit2 error, so a failed FF can never strand the
    //    carried stash — step 4 always re-applies it.
    let fast_forwarded = try_ff_to_upstream(&repo, name);

    // 4. Re-apply the carried work iff stashed. `pop_stash` drops on clean apply
    //    and RETAINS on conflict (never lossy). A `Conflicts` outcome is a
    //    SUCCESS return (branch switched; changes present w/ markers).
    if stashed {
        let outcome = stash::pop_stash(workdir, 0, false, None)?;
        return Ok(CheckoutResult {
            stashed: true,
            fast_forwarded,
            apply: Some(outcome),
        });
    }

    // 5. Clean case.
    Ok(CheckoutResult {
        stashed: false,
        fast_forwarded,
        apply: None,
    })
}

/// No-fetch fast-forward of LOCAL branch `name` to its upstream tracking ref.
/// Resolves the upstream oid from the already-present remote-tracking ref
/// (`Branch::upstream()` performs no network I/O). Fast-forwards only when
/// behind>0 && ahead==0; every other condition (no upstream, up-to-date,
/// ahead-only, diverged) returns `false` and leaves the ref untouched.
///
/// BEST-EFFORT / INFALLIBLE: the switch has ALREADY succeeded and any carried
/// work is stashed at `stash@{0}` when this runs, so the FF is a pure
/// convenience that MUST NOT propagate errors — an `Err` here would return
/// before the caller's `pop_stash` and silently strand the stash. Every
/// internal libgit2 error (graph math, object lookup, non-conflict
/// `checkout_tree`, ref lookup, `set_target`) therefore collapses to `false`,
/// leaving the ref untouched, rather than an `Err`.
fn try_ff_to_upstream(repo: &git2::Repository, name: &str) -> bool {
    let branch = match repo.find_branch(name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(_) => return false,
    };
    let upstream = match branch.upstream() {
        Ok(u) => u,
        Err(_) => return false, // no upstream / gone -> skip silently
    };
    let upstream_oid = match upstream.get().target() {
        Some(oid) => oid,
        None => return false,
    };
    let local_oid = match branch.get().target() {
        Some(oid) => oid,
        None => return false,
    };

    let (ahead, behind) = match repo.graph_ahead_behind(local_oid, upstream_oid) {
        Ok(counts) => counts,
        Err(_) => return false,
    };
    if behind == 0 {
        return false; // up-to-date or ahead-only
    }
    if ahead > 0 {
        return false; // diverged -> do NOT touch (no merge in v1)
    }

    // Fast-forward (behind>0 && ahead==0). SAFE-FF recipe: checkout_tree BEFORE
    // set_target, identical to remote.rs pull_ff and merge.rs. `obj` is scoped
    // so its borrow of `repo` ends before the &mut set_target call. Any libgit2
    // error (incl. a real conflict) skips the FF via `false` — never propagates.
    {
        let obj = match repo.find_object(upstream_oid, None) {
            Ok(o) => o,
            Err(_) => return false,
        };
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.safe(); // NEVER .force()
        if repo.checkout_tree(&obj, Some(&mut opts)).is_err() {
            return false;
        }
    }
    match repo.find_reference(&format!("refs/heads/{name}")) {
        Ok(mut reference) => reference
            .set_target(
                upstream_oid,
                &format!("checkout: fast-forward {name} to {upstream_oid}"),
            )
            .is_ok(),
        Err(_) => false,
    }
}
