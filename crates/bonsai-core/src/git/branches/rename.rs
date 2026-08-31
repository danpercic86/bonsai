//! Local-branch rename (P60a).

use std::path::Path;

use crate::error::AppError;

use super::{open_repo_at, validate_branch_name};

/// Result of `rename_branch` (P60a). Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameBranchResult {
    /// true when the renamed branch was the checked-out branch (HEAD followed the
    /// rename — libgit2 rewrites the HEAD symref). Tells the frontend to refetch
    /// HEAD/status, not just the branch list.
    pub was_head: bool,
    /// The upstream shorthand still configured after the rename (e.g. "origin/main"),
    /// or None. libgit2 renames the `branch.<name>.*` config section, so tracking
    /// is PRESERVED; surfaced so the UI can confirm it in a toast.
    pub upstream: Option<String>,
}

/// Blocking. Renames LOCAL branch `old_name` → `new_name` (git `branch -m`,
/// non-force). Validates `new_name` (reuses `validate_branch_name`); resolves
/// `old_name` (NotFound → `BranchNotFound`); refuses when `new_name` already
/// exists (git2 `Branch::rename(.., force=false)` → `ErrorCode::Exists` →
/// `BranchExists`). libgit2 moves the ref, its reflog, and the `branch.<name>.*`
/// config section, and rewrites HEAD when `old_name` is checked out — so
/// upstream/tracking survive and no manual config surgery is needed.
///
/// Errors: `invalidName` | `branchNotFound` | `branchExists` | `git` | `noRepo`.
pub fn rename_branch(
    workdir: &Path,
    old_name: &str,
    new_name: &str,
) -> Result<RenameBranchResult, AppError> {
    validate_branch_name(new_name)?;
    let repo = open_repo_at(workdir)?;

    let mut branch = match repo.find_branch(old_name, git2::BranchType::Local) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!(
                "branch '{old_name}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };
    // Capture BEFORE the rename — after it, `branch` points at the moved ref.
    let was_head = branch.is_head();

    // Non-force rename: git2 moves the ref + reflog + `branch.<name>.*` config
    // section, and rewrites HEAD when `old_name` is the checked-out branch. A
    // clash with an existing branch is refused (force=false).
    let renamed = match branch.rename(new_name, /* force */ false) {
        Ok(b) => b,
        Err(e) if e.code() == git2::ErrorCode::Exists => {
            return Err(AppError::BranchExists(format!(
                "branch '{new_name}' already exists"
            )));
        }
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::BranchNotFound(format!(
                "branch '{old_name}' not found"
            )));
        }
        Err(e) => return Err(e.into()),
    };

    // Re-read the upstream from the RENAMED branch — the config section moved
    // with the rename, so tracking is preserved. None when no upstream / gone.
    let upstream = renamed
        .upstream()
        .ok()
        .and_then(|u| u.name().ok().flatten().map(str::to_string));

    Ok(RenameBranchResult { was_head, upstream })
}
