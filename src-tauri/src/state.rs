use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::activity_hub::{GitActivityHub, ScopedActivityHub};
use crate::graph_cache::GraphCache;
use crate::perf::PerfState;
use crate::watcher::WatcherHandle;

/// One open repository: its canonical workdir root, its own file watcher, and
/// its commit-graph layout cache (P86 B1).
///
/// NOT `Clone`/`Debug` (`WatcherHandle` is neither) — entries live in the map
/// and callers clone `path` (and the `graph_cache` `Arc`) out under the lock,
/// never the whole entry.
pub struct RepoEntry {
    pub path: PathBuf,
    pub watcher: Option<WatcherHandle>,
    /// Per-repo graph-layout cache (P86 B1). A fresh entry starts empty; the
    /// slot is cloned out under the map lock and locked only for classify +
    /// replay/store, never across a cold walk. Dropped on `close_repo`.
    ///
    /// P117 §1.3 (supersedes P86 B1's "a re-arm starts `None` again"): an
    /// `open_repo` re-arm of an entry ALREADY present under the same key
    /// CARRIES this slot over. Safe because `classify` re-proves the cache on
    /// every single request — an exact-set comparison of (tips, head, hide)
    /// against a seed probed fresh from the live repository — so a changed
    /// topology already Misses, wipe or no wipe. A `close_repo` drops the entry
    /// (and this `Arc`), so a later open starts `None` by construction.
    pub graph_cache: Arc<GraphCache>,
}

/// Shared app state: every open repo, keyed by `repoId` (canonical workdir
/// path string, P3e contract §2). One Mutex guards the whole map — safe because
/// handlers only hold the lock long enough to clone a `PathBuf` out (or
/// insert/remove an entry), never across the `spawn_blocking` git work.
#[derive(Default)]
pub struct AppState {
    pub repos: Mutex<HashMap<String, RepoEntry>>,
    /// P88b/B2b: per-`repoId` generation counter for the thread-local
    /// `git2::Repository` handle cache (`repo_handle::with_repo`). Bumped on every
    /// `open_repo` re-arm and on `close_repo` so a reopen/close lazily EVICTS every
    /// blocking-pool thread's stale cached handle on its next `with_repo` call.
    /// Distinct from `repos` (which is removed on close): this map is only ever
    /// INCREMENTED and entries persist across close, so a stale handle keyed to an
    /// old generation can never be silently reused after a re-open. Small +
    /// bounded by the distinct repoIds opened in a session.
    pub repo_generations: Mutex<HashMap<String, u64>>,
    /// Backend perf counters (P86 instrumentation) — shared behind an `Arc` so
    /// blocking-pool tasks can bump them. Read via `debug_perf_counters`.
    pub perf: Arc<PerfState>,
    /// The app's focused-tab repoId (or `None` when no repo is focused). Set by
    /// the frontend on tab switch / open / close and once on startup after
    /// session restore (P16 §5). Distinct from a per-MCP-session selection: this
    /// only *seeds* a new embedded-MCP session's initial repo; the AI may then
    /// re-point its own session without disturbing this value.
    pub active_repo: Mutex<Option<String>>,
    /// P87/P123: the ONE git-activity fan-out hub. Each `git_activity_subscribe`
    /// channel listens to ONE repo; a run emits only onto its repo's channels
    /// (via [`Self::git_activity_for`]); no subscriber ⇒ cores take the buffered
    /// path.
    pub git_activity: GitActivityHub,
}

impl AppState {
    /// The hub bound to one repo's subscribers (P123): a run emits only onto the
    /// channels of the tabs of `repo_id`.
    pub fn git_activity_for(&self, repo_id: &str) -> ScopedActivityHub {
        ScopedActivityHub::new(self.git_activity.clone(), Some(repo_id.to_owned()))
    }

    /// P123 D3 / Q1: the hub for a run with no repo (clone/init). THE single
    /// switch point for Q1 — today (a) broadcast to every open tab.
    pub fn git_activity_unscoped(&self) -> ScopedActivityHub {
        ScopedActivityHub::new(self.git_activity.clone(), None)
    }
}
