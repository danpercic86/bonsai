//! Handle-reusing twins of the worktree readers (P88b/B2a).
//!
//! Each takes an already-open `&git2::Repository` so a composite mutation
//! (`branches::checkout_branch_autostash` / `create_branch_here`) opens the repo
//! ONCE and threads the handle through its sub-primitives instead of each one
//! re-opening from `&Path`. Behaviour is byte-identical to the `&Path` entry
//! points in `worktree.rs`, which are now thin wrappers over these twins.
//!
//! Split out of `worktree.rs` purely to respect the file-size ratchet (that file
//! is already over the soft limit); no logic changed in the move.

use std::path::Path;

use crate::error::AppError;
use crate::git::stage::ensure_not_bare;

use super::worktree::{build_linked_row, build_main_row, canonical, main_workdir, WorktreeInfo};

/// List the main worktree (synthesized, first) followed by every linked
/// worktree, using an already-open handle. Byte-identical to
/// `worktree::list_worktrees`; `cur` is derived from `repo.workdir()` exactly as
/// before. Reproduces `open_workdir_repo`'s bare-repo guard via `ensure_not_bare`
/// so a handle opened without that check (e.g. `branches::open_repo_at`) still
/// refuses a bare repo at the same point.
pub fn list_worktrees_with(
    repo: &git2::Repository,
) -> Result<Vec<WorktreeInfo>, AppError> {
    ensure_not_bare(repo)?;
    let cur = repo
        .workdir()
        .map(canonical)
        .ok_or_else(|| AppError::Git("repository has no working directory".to_string()))?;
    let main_dir = main_workdir(repo)?;

    let mut out = Vec::new();
    out.push(build_main_row(&main_dir, &cur)?); // synthesized main row, FIRST

    for name in repo.worktrees()?.iter().map(|name| name.ok().flatten()) {
        let name = match name {
            Some(n) => n,
            None => {
                eprintln!("bonsai: skipping worktree with non-UTF-8 name");
                continue;
            }
        };
        let wt = repo.find_worktree(name)?;
        out.push(build_linked_row(&wt, &main_dir, &cur)?);
    }
    Ok(out)
}

/// ABSOLUTE working-dir path of a *different* worktree that has local branch
/// `name` checked out, or `None`, using an already-open handle. Byte-identical to
/// `worktree::branch_checked_out_elsewhere`; `cur` is the passed `workdir` (not
/// `repo.workdir()`), exactly as before.
pub(crate) fn branch_checked_out_elsewhere_with(
    repo: &git2::Repository,
    workdir: &Path,
    name: &str,
) -> Result<Option<String>, AppError> {
    let cur = canonical(workdir);
    for wt in list_worktrees_with(repo)? {
        // The caller's own worktree is never a collision (the `is_head` no-op
        // handles that case), even if it has `name` checked out.
        if canonical(Path::new(&wt.abs_path)) == cur {
            continue;
        }
        // Invalid/unreadable/stale worktrees report `branch == None`; skip
        // defensively rather than erroring.
        if !wt.valid {
            continue;
        }
        if wt.branch.as_deref() == Some(name) {
            return Ok(Some(wt.abs_path.clone()));
        }
    }
    Ok(None)
}
