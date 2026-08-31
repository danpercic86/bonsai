//! Diff core (M4 contract §2).
//!
//! Pure git2 logic, no Tauri types. Three modes share one engine:
//! workdir unstaged (index vs workdir), staged (HEAD tree vs index, unborn ->
//! empty tree), and commit vs FIRST parent (root -> empty tree, merge ->
//! first parent only). Per-file hunk payloads are capped at
//! [`MAX_FILE_DIFF_LINES`] emitted lines, all-or-nothing (`too_large`).
//!
//! Split by concern into focused submodules — the wire `types`, the git2
//! `collect` walk + shared option/tree helpers, and the public `api` entry
//! points; every item is re-exported so `crate::git::diff::<item>` paths are
//! unchanged.

/// Total emitted [`DiffLine`]s per file (contract §2.6). Exceeded -> abort
/// iteration, `too_large: true`, `hunks: []` (all-or-nothing).
pub const MAX_FILE_DIFF_LINES: usize = 5_000;

/// Context (and interhunk) line count for a full-context "File View" diff
/// (§2.6). A large FINITE value — `u32::MAX` overflows libgit2's xdiff context
/// math. Comfortably larger than [`MAX_FILE_DIFF_LINES`], so any file that is
/// not already `too_large` collapses to one whole-file hunk.
const FULL_CONTEXT_LINES: u32 = 1_000_000;

mod api;
mod collect;
mod types;

pub use types::{
    CommitDetails, CommitDiff, CompareDiff, CompareEndpoint, DiffLine, FileDiff, FileDiffHeader,
    Hunk, LineKind,
};

pub(crate) use collect::{
    apply_find_similar, build_diff_options, collect_file_diff, collect_file_diffs, collect_headers,
    head_tree, lossy,
};
// `map_status` / `normalize_content` are exercised only by the sibling test
// modules (via `use super::*`), so their re-export is test-only.
#[cfg(test)]
pub(crate) use collect::{map_status, normalize_content};

pub use api::{
    commit_diff, commit_file_diff, compare_head_diff, compare_head_file_diff, workdir_file_diff,
};
pub(crate) use api::{commit_trees, head_endpoint, maybe_annotate, pathspecs};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod compare_tests;
