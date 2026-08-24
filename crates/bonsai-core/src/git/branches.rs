//! Branch operations core (M5 contract §2).
//!
//! Pure git2 logic, no Tauri types — testable against the git CLI oracle
//! (see `tests/branches_cli.rs`). All functions blocking; the command layer
//! wraps them in `spawn_blocking`.
//!
//! Split by operation into focused submodules (list / create / checkout /
//! delete / rename / remote); the shared snapshot types and helpers stay
//! here and every operation is re-exported so the public path
//! `crate::git::branches::<op>` is unchanged.

use std::path::Path;

use crate::error::AppError;
use crate::git::repo::HeadInfo;

mod checkout;
mod create;
mod delete;
mod list;
mod remote;
mod rename;

pub use checkout::{
    checkout_branch, checkout_branch_autostash, checkout_branch_with, checkout_commit_detached,
    CheckoutResult,
};
pub use create::{create_branch, create_branch_here, CreateBranchHereResult};
pub use delete::{delete_branch, delete_branch_with};
pub use list::list_refs;
pub use remote::{checkout_remote, delete_remote_tracking};
pub use rename::{rename_branch, RenameBranchResult};

/// One local branch in the sidebar snapshot.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    /// Shorthand, e.g. "main", "feature/sidebar".
    pub name: String,
    /// True for the branch HEAD points at (always false when detached/unborn).
    pub is_head: bool,
    /// Upstream shorthand, e.g. "origin/main"; None when no upstream
    /// configured or the upstream ref is gone.
    pub upstream: Option<String>,
    /// Commits ahead of / behind upstream. None whenever `upstream` is None.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Full 40-char hex oid of the branch tip.
    pub tip: String,
}

/// One remote-tracking branch (read-only list in M5).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteBranchInfo {
    /// Shorthand incl. remote, e.g. "origin/main".
    pub name: String,
    /// Full 40-char hex oid of the remote-tracking branch tip.
    pub tip: String,
}

/// One snapshot of everything the sidebar renders.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchesSnapshot {
    /// Sorted case-insensitively by name.
    pub local: Vec<BranchInfo>,
    /// Sorted case-insensitively; symbolic "<remote>/HEAD" entries EXCLUDED.
    pub remote: Vec<RemoteBranchInfo>,
    /// Tag names (lightweight + annotated), sorted case-insensitively.
    pub tags: Vec<String>,
    /// Same shape the header already uses — one source of truth for
    /// attached/detached/unborn in the sidebar.
    pub head: HeadInfo,
}

/// Opens the repo at `workdir` with `NO_SEARCH` (same as every git/ module).
fn open_repo_at(workdir: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        workdir,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Case-insensitive name ordering (ties broken case-sensitively so the
/// order is total and stable).
fn ci_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b))
}

/// Backend-authoritative branch-name validation (mirrors
/// `git check-ref-format --branch`): trimmed-empty and leading `-` are our
/// stricter pre-checks (libgit2 accepts `refs/heads/-x` as a valid ref name;
/// the git CLI refuses `-x` as a branch name), the rest is
/// `git2::Branch::name_is_valid`.
///
/// `pub(crate)` so the NL-operation planner (`ai_operation_resolve`) can reuse
/// the SAME validator when resolving a `createBranch` intent (a miss there
/// degrades to a calm `Unsupported`, never a hard error).
pub(crate) fn validate_branch_name(name: &str) -> Result<(), AppError> {
    let invalid = || AppError::InvalidName(format!("invalid branch name: '{name}'"));
    if name.trim().is_empty() || name.starts_with('-') {
        return Err(invalid());
    }
    if !git2::Branch::name_is_valid(name)? {
        return Err(invalid());
    }
    Ok(())
}

#[cfg(test)]
mod create_branch_here_tests;
#[cfg(test)]
mod checkout_autostash_tests;
#[cfg(test)]
mod rename_branch_tests;
#[cfg(test)]
mod checkout_commit_detached_tests;
