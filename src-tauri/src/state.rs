use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
    /// replay/store, never across a cold walk. Dropped on `close_repo`; a new
    /// `RepoEntry` on `open_repo` re-arm starts `None` again (topology may have
    /// changed while closed).
    pub graph_cache: Arc<GraphCache>,
}

/// Shared app state: every open repo, keyed by `repoId` (canonical workdir
/// path string, P3e contract §2). One Mutex guards the whole map — safe because
/// handlers only hold the lock long enough to clone a `PathBuf` out (or
/// insert/remove an entry), never across the `spawn_blocking` git work.
#[derive(Default)]
pub struct AppState {
    pub repos: Mutex<HashMap<String, RepoEntry>>,
    /// Backend perf counters (P86 instrumentation) — shared behind an `Arc` so
    /// blocking-pool tasks can bump them. Read via `debug_perf_counters`.
    pub perf: Arc<PerfState>,
    /// The app's focused-tab repoId (or `None` when no repo is focused). Set by
    /// the frontend on tab switch / open / close and once on startup after
    /// session restore (P16 §5). Distinct from a per-MCP-session selection: this
    /// only *seeds* a new embedded-MCP session's initial repo; the AI may then
    /// re-point its own session without disturbing this value.
    pub active_repo: Mutex<Option<String>>,
}
