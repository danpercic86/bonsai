//! Per-repo path lists on [`Settings`]: the recently-opened repos and the
//! git-hook disclosure acks. Split out of `settings.rs` (size split);
//! re-exported from there so every `crate::settings::X` path is unchanged.

use super::Settings;

pub const MAX_RECENT_REPOS: usize = 10;

/// One recently-opened repository.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentRepo {
    /// Absolute workdir path as reported by `read_repo_info` (canonical root).
    pub path: String,
    /// Seconds since epoch (UTC) of the last successful open.
    pub last_opened: i64,
}

/// Upserts `path` at the front of the recents list, stamping `last_opened`,
/// and truncating to [`MAX_RECENT_REPOS`].
///
/// Dedupe goes through [`crate::commands::same_repo_path`] — the same
/// `fs::canonicalize` compare the open-repo scan uses — so the recents list and
/// the repo-registry agree on what "the same repo" is. The previous
/// `eq_ignore_ascii_case` was wrong in BOTH directions: it missed non-ASCII
/// case variants and 8.3 short names on Windows, and on a case-SENSITIVE
/// filesystem (ext4) it merged `/home/u/Repo` and `/home/u/repo`, which are two
/// genuinely different repositories there.
///
/// `same_repo_path` falls back to the old ASCII-case-insensitive compare
/// whenever either side cannot be canonicalized (a recents entry whose folder
/// was deleted or is on a detached drive), so **`record_recent` itself** never
/// drops an entry. Culling stale ones is [`super::prune::prune_stale_paths`]'s job
/// (P115): once at setup, only paths it can prove are gone. This is the one
/// non-pure step here; tests stay injectable because unresolvable temp paths
/// take the string-compare branch.
pub fn record_recent(s: &mut Settings, path: &str, now: i64) {
    s.recent_repos
        .retain(|r| !crate::commands::same_repo_path(&r.path, path));
    s.recent_repos.insert(
        0,
        RecentRepo {
            path: path.to_string(),
            last_opened: now,
        },
    );
    s.recent_repos.truncate(MAX_RECENT_REPOS);
}

/// Whether the user has acknowledged `repo_path`'s one-time git-hook disclosure.
/// Dedupe goes through [`crate::commands::same_repo_path`] (the canonicalizing
/// compare the open-repo scan + `record_recent` use), so this and the repo
/// registry agree on what "the same repo" is. P115's pass removes confirmed-gone
/// paths here and **never migrates an ack** (consent a user did not give must
/// never appear); a moved repo simply re-prompts once.
pub fn hooks_ack_contains(s: &Settings, repo_path: &str) -> bool {
    s.hooks_ack_repos
        .iter()
        .any(|p| crate::commands::same_repo_path(p, repo_path))
}

/// Record that the user acknowledged `repo_path`'s git-hook disclosure.
/// Idempotent: a no-op when an equivalent path (per `same_repo_path`) is already
/// present, so re-acking never grows the list — and P115's pass only removes.
pub fn set_hooks_ack(s: &mut Settings, repo_path: &str) {
    if !hooks_ack_contains(s, repo_path) {
        s.hooks_ack_repos.push(repo_path.to_string());
    }
}
