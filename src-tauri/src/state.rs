use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use bonsai_core::git::activity::GitActivityEvent;

use crate::graph_cache::GraphCache;
use crate::perf::PerfState;
use crate::watcher::WatcherHandle;

/// P87: fan-out hub for the ONE git-activity stream. Holds every long-lived
/// `git_activity_subscribe` channel; ops emit onto ALL of them. A subscriber
/// dropping (HMR/reload/window close) is pruned lazily on the next send failure.
///
/// CLONE-able over a shared `Arc` so `with_activity` can move a handle into the
/// emitter closure while `git_activity_subscribe` mutates the same list.
#[derive(Clone, Default)]
pub struct GitActivityHub {
    subs: Arc<Mutex<Vec<tauri::ipc::Channel<GitActivityEvent>>>>,
}

impl GitActivityHub {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<tauri::ipc::Channel<GitActivityEvent>>> {
        // Plain Vec push/retain — a poisoned lock is recoverable (no invariant
        // spans a panic), so recover rather than fail every later emit.
        self.subs.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Register a long-lived channel (called once per app/repo mount; re-invoked
    /// after HMR — stale channels are pruned on send failure in [`Self::emit`]).
    pub fn subscribe(&self, ch: tauri::ipc::Channel<GitActivityEvent>) {
        self.lock().push(ch);
    }

    /// Fan out one event to every registered channel; drop any whose `send`
    /// errors (the frontend dropped it). A cheap no-op when no one is listening.
    pub fn emit(&self, ev: GitActivityEvent) {
        let mut subs = self.lock();
        if subs.is_empty() {
            return;
        }
        subs.retain(|ch| ch.send(ev.clone()).is_ok());
    }

    /// Cheap "anyone listening?" check so `with_activity` can skip the emitter +
    /// the streaming exec path entirely and keep the buffered path (contract §10).
    pub fn is_active(&self) -> bool {
        !self.lock().is_empty()
    }
}

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
    /// P87: the ONE git-activity fan-out hub. Long-lived subscription (Option B):
    /// every git op emits onto whatever channels `git_activity_subscribe`
    /// registered; no subscriber ⇒ emit is a no-op and cores take the buffered
    /// path. Not per-command, so future ops appear on the log automatically.
    pub git_activity: GitActivityHub,
}

impl AppState {
    /// A cloned handle to the git-activity hub (for moving into `with_activity`'s
    /// emitter closure — `tauri::State` only yields a borrow).
    pub fn git_activity_hub(&self) -> GitActivityHub {
        self.git_activity.clone()
    }
}
