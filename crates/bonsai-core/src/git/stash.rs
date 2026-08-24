//! Stash management (P9 contract §2). Wraps every git2 stash primitive:
//! list / create / apply / pop / drop.
//!
//! Pure git2 logic, no Tauri types (runtime-free cores → unit-testable without
//! the Tauri "test" feature, same rule as merge/rebase). All stash APIs
//! (`stash_foreach` / `stash_save2` / `stash_apply` / `stash_pop` /
//! `stash_drop`) require `&mut Repository`, so callers bind `let mut repo`.
//!
//! SAFE by construction: apply/pop use the SAFE checkout default (no
//! REINSTATE_INDEX, OPEN Q#4) and a conflicting apply/pop RETAINS the stash
//! (never lossy). Pop uses the P8 `apply + inspect index + conditional drop`
//! pattern (mirrors `merge::pop_after_success`) because this libgit2 may report
//! a *content* conflict as `Ok(())`, and a naive `stash_pop` would then silently
//! drop the entry — data loss.
//!
//! Split by operation into focused submodules (list / create / apply); the
//! shared wire types and the `require_clean` precondition stay here and every
//! operation is re-exported so the public path `crate::git::stash::<op>` is
//! unchanged.

use crate::error::AppError;

// T2.6 hardening tests live in a sibling file (this module already carries the
// P9/P34 inline matrices; soft 500-line file discipline).
#[cfg(test)]
#[path = "stash_hardening_tests.rs"]
mod stash_hardening_tests;

mod apply;
mod create;
mod list;

pub use apply::{apply_stash, drop_stash, pop_stash, pop_stash_with};
pub use create::{create_stash, create_stash_with};
pub use list::list_stashes;

// The private helpers below live in the submodules now; the two `#[cfg(test)]`
// sibling test modules reach them (and `Path`) through `use super::*`, so
// re-export them here under `cfg(test)` to keep that path resolving.
#[cfg(test)]
use std::path::Path;
#[cfg(test)]
pub(crate) use self::{
    apply::{escape_pathspec, is_windows_reserved, stash_commit_oid, stash_path_sets},
    create::make_index_entry,
};

/// One stash stack entry. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    /// Stack index; 0 == most recent (== stash@{0}). SHIFTS after any drop/pop.
    pub index: usize,
    /// Full stash message, e.g. "WIP on main: 1a2b3c4 summary" or a custom message.
    pub message: String,
    /// Full 40-hex oid of the stash commit itself.
    pub oid: String,
    /// Full 40-hex oid of the stash's FIRST parent = the base commit it was
    /// created from (what the graph pill attaches to).
    pub base_oid: String,
    /// Stash commit author time, seconds since epoch (UTC) — drives relative age.
    pub ts: i64,
}

/// Result of apply/pop. Wire: tagged "kind", camelCase (same recipe as MergeOutcome).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ApplyStashOutcome {
    /// Clean apply/pop. (Pop additionally dropped the entry.)
    Applied,
    /// Worktree has <<<<<<< markers, index has conflict entries, and the stash
    /// entry is RETAINED (libgit2 does not drop on GIT_ECONFLICT). `paths` =
    /// sorted conflicted paths (the set `list_conflicts` returns).
    Conflicts { paths: Vec<String> },
    /// Blocked pre-apply: the stash holds blobs at Windows-reserved paths (e.g.
    /// `.../NUL`) that NTFS cannot write. Nothing was applied and the stash is
    /// RETAINED. `paths` = the sorted reserved paths, to name in the UI prompt.
    ReservedPaths { paths: Vec<String> },
    /// Applied with `skip_reserved`: every non-reserved change was restored but
    /// these Windows-reserved paths could NOT be written back. On pop the stash
    /// is RETAINED (the reserved blobs live only in the stash). `skipped` =
    /// sorted reserved paths.
    AppliedSkippingReserved { skipped: Vec<String> },
}

/// Result of create_stash.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStashResult {
    /// false == nothing to stash (clean worktree) → NOT an error.
    pub created: bool,
}

/// Which changes a `create_stash` call captures. Wire: camelCase (matches the
/// TS `StashScope` union `'all' | 'allWithUntracked' | 'staged'`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StashScope {
    /// Staged + unstaged tracked changes; untracked left in place.
    /// → `StashFlags::DEFAULT`.
    All,
    /// Adds untracked files. → `StashFlags::DEFAULT | INCLUDE_UNTRACKED`.
    AllWithUntracked,
    /// Only the staged (index-vs-HEAD) paths. No native libgit2 flag —
    /// hand-rolled by `create_staged_stash` (FOLD semantics, see below).
    Staged,
}

/// Shared precondition: create/apply/pop require a Clean repo state (no
/// in-progress merge/rebase). Drop is exempt (touches only the stash reflog).
fn require_clean(repo: &git2::Repository) -> Result<(), AppError> {
    if repo.state() != git2::RepositoryState::Clean {
        return Err(AppError::OperationInProgress(
            "an operation is already in progress — finish or abort it first".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
