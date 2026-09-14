//! P91 §F6 — what a "forget my usage counts" leaves behind on disk.
//!
//! Split out of `metrics.rs` deliberately: the enumerate-then-remove policy below
//! is the security-relevant half of the delete and is easier to review (and to
//! test) when it is not buried in the observation hot path.

use std::path::Path;

/// §F6 §4.2 — which on-disk shape a clear leaves behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearMode {
    /// `metrics_reset` — commit a fresh empty `usage.json` in place (§5). The
    /// `metrics` directory survives, holding one empty aggregate.
    ResetInPlace,
    /// `logs_delete_all` — remove the metrics files AND the directory (§4.3).
    DeleteFiles,
}

/// §F6 — honest counts for the metrics half of a delete. Merged into
/// `LogsDeleteResult` by the command layer.
#[derive(Debug, Default, Clone, Copy)]
pub struct MetricsClearCounts {
    pub deleted_files: u32,
    pub deleted_bytes: u64,
    pub failed_files: u32,
    /// True when the `metrics` directory itself no longer exists.
    pub dir_removed: bool,
}

/// Deletes every FILE directly inside `dir`, then the directory itself.
///
/// **Scope is "every file", not a `usage.json*` glob** (§4.3): a future metrics
/// file must be covered automatically, and a stray name-mismatched file would
/// otherwise survive, block `remove_dir`, and make the signed copy's "Bonsai
/// removes that whole folder" false.
///
/// **Non-recursive by design, and NOT `remove_dir_all`.** This runs from a
/// webview-reachable command on a path built by joining onto the app config
/// directory; a recursive delete there would take `settings.json` and `logs/`
/// with it if `metrics_dir` ever resolved to the config root (an empty join, a
/// refactor). Enumerate-then-`remove_dir` structurally cannot do that, and for a
/// directory that only ever holds `usage.json{,.tmp,.bak}` the two are
/// behaviourally identical. A subdirectory (never created by Bonsai) is counted
/// in `failed_files` and left alone — the §6.1 partial-failure copy path already
/// covers it. **Do not "simplify" this to `remove_dir_all`.**
///
/// Never fails: a missing directory is success (nothing to forget), and an
/// individual failure is reported, not propagated — metrics must never fail the
/// app. Blocking (file IO) — call on the blocking pool.
pub fn purge_metrics_dir(dir: &Path) -> MetricsClearCounts {
    let mut c = MetricsClearCounts::default();
    let Ok(entries) = std::fs::read_dir(dir) else {
        // Missing (or unreadable) — there is nothing left to delete. `dir_removed`
        // reflects reality rather than the read: an unreadable-but-present dir
        // must not claim to be gone.
        c.dir_removed = !dir.exists();
        return c;
    };
    for entry in entries.flatten() {
        // `file_type` avoids following a symlink to a directory elsewhere; an
        // unreadable entry is counted as failed rather than silently skipped.
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        if is_dir {
            c.failed_files = c.failed_files.saturating_add(1);
            continue;
        }
        // Measured immediately before removal, like `writer::purge_scope`.
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        if std::fs::remove_file(entry.path()).is_ok() {
            c.deleted_files = c.deleted_files.saturating_add(1);
            c.deleted_bytes = c.deleted_bytes.saturating_add(size);
        } else {
            c.failed_files = c.failed_files.saturating_add(1);
        }
    }
    // Succeeds iff the directory is now empty — which is exactly the condition
    // under which claiming "the whole folder is gone" is true.
    c.dir_removed = std::fs::remove_dir(dir).is_ok() || !dir.exists();
    c
}

#[cfg(test)]
#[path = "tests_metrics_purge.rs"]
mod tests_metrics_purge;
