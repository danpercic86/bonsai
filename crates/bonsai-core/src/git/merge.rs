//! Merge a local or remote-tracking branch into the current branch.
//! Clean merges auto-commit; conflicts pause into RepoOpState::Merge.
//! Pure git2, no Tauri types, no network (merging origin/x uses the local
//! remote-tracking ref — Fetch first is the user's job, same as GitKraken).
//! (P3c contract §4.)
//!
//! Split by concern into focused submodules — `branch` (the `merge_branch`
//! orchestration) and `finalize` (the shared commit core + `commit_merge` /
//! `abort_merge`); the shared `MergeOutcome` / `MergeHooks` types and the
//! prepared-message helper stay here and every public item is re-exported so
//! `crate::git::merge::<item>` paths are unchanged.

// P87: the activity-recording merge-commit core lives in `merge_activity`
// (file-size split); re-exported so `merge::commit_merge_with_activity` keeps
// resolving for the command layer.
pub use crate::git::merge_activity::commit_merge_with_activity;

use crate::error::AppError;

mod branch;
mod finalize;

pub use branch::{merge_branch, merge_branch_gated};
pub(crate) use finalize::finalize_merge_commit;
pub use finalize::{abort_merge, commit_merge};

/// Wire: tagged "kind", camelCase (same recipe as PullResult).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum MergeOutcome {
    /// Incoming is already reachable from HEAD. Nothing changed.
    UpToDate,
    /// HEAD branch fast-forwarded to `to` (full oid). No merge commit.
    /// `stashed` = an autostash was created (and restored) for this operation.
    FastForwarded {
        branch: String,
        to: String,
        stashed: bool,
    },
    /// Clean merge, auto-committed. `oid` = the new 2-parent merge commit.
    /// `stashed` = an autostash was created (and restored) for this operation.
    Merged { oid: String, stashed: bool },
    /// Conflicts recorded in index + worktree; MERGE_HEAD/MERGE_MSG written;
    /// repo paused in state Merge. Sorted conflicted paths (same set
    /// list_conflicts returns). `stashed` = an autostash was created and is
    /// RETAINED on the stack (deferred re-apply, OPEN Q #2).
    Conflicts { paths: Vec<String>, stashed: bool },
    /// FF / merge-commit landed, but re-applying the autostash conflicted.
    /// The stash entry is RETAINED at stash@{0}. `head` = FF target or new
    /// merge-commit oid; `paths` = conflicted paths from the stash apply.
    StashPopConflicts { head: String, paths: Vec<String> },
}

/// Prepared MERGE_MSG first line (contract §4.3, byte-exact for the oracle):
/// `Merge branch '<name>'` / `Merge remote-tracking branch '<name>'` —
/// no `into <branch>` suffix (locked decision §11.4).
fn prepared_merge_message(name: &str, incoming_is_remote: bool) -> String {
    if incoming_is_remote {
        format!("Merge remote-tracking branch '{name}'")
    } else {
        format!("Merge branch '{name}'")
    }
}

/// How [`merge_branch_gated`] treats the `commit-msg` hook of a clean
/// auto-merge commit — the ONE hook that path can fire
/// ([`MergeHooks::MessageOnly`]).
///
/// `Run` / `Skip` are exactly the old `skip_hooks: bool`. `Gate` exists for a
/// caller that may not run undisclosed repository code and must REFUSE rather
/// than silently bypass (audit 2026-09-11 LOW: `bonsai-mcp`'s standalone stdio
/// server, which has no frontend to disclose hook execution through). The
/// refusal TEXT belongs to that caller, not here, so the variant carries the
/// caller's decision function instead of core inventing `--allow-hooks`
/// vocabulary; it is handed the names of the hooks that would really run
/// (`merge_commit_hooks_that_would_run`) and its `Err` aborts the merge.
#[derive(Clone, Copy)]
pub enum MergeHookGate {
    /// Run `commit-msg` when enabled (`bonsai.runHooks`) — the app's default.
    Run,
    /// `--no-verify`: never run it.
    Skip,
    /// Consult this decision function BEFORE anything mutates, on the paths
    /// that would run the hook; `Ok` ⇒ behave like [`MergeHookGate::Run`].
    Gate(fn(&[&str]) -> Result<(), AppError>),
}

impl MergeHookGate {
    /// The `skip_hooks` value this gate implies for [`hooks_enabled`](
    /// crate::git::hooks::hooks_enabled): only `Skip` bypasses — a `Gate` that
    /// returned `Ok` runs the hook exactly like `Run`.
    pub(crate) fn skips_hooks(self) -> bool {
        matches!(self, MergeHookGate::Skip)
    }
}

/// Which commit hooks [`finalize_merge_commit`] fires (F-A4-2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MergeHooks {
    /// No hooks (`skip_hooks` / `bonsai.runHooks=false`).
    Off,
    /// `commit-msg` only — the clean auto-merge path. git's `git merge`
    /// auto-commit runs pre-merge-commit + prepare-commit-msg + commit-msg;
    /// Bonsai supports neither pre-merge-commit nor prepare-commit-msg
    /// (documented v1 divergence, F-A4-3), but honors commit-msg so message
    /// policy hooks apply to merge commits too.
    MessageOnly,
    /// pre-commit + commit-msg + post-commit — `commit_merge` (concluding a
    /// paused merge, like `git commit` with MERGE_HEAD present).
    Full,
}

#[cfg(test)]
mod autostash_tests;
#[cfg(test)]
mod hook_gate_tests;
#[cfg(test)]
mod p8_helpers;
#[cfg(test)]
mod tests;
