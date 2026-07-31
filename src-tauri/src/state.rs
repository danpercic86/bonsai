use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use crate::watcher::WatcherHandle;

/// One open repository: its canonical workdir root and its own file watcher.
///
/// NOT `Clone`/`Debug` (`WatcherHandle` is neither) — entries live in the map
/// and callers clone `path` out under the lock, never the whole entry.
///
/// Future perf lever (M2 note): a cached `git2::Repository` handle could live
/// here per entry. Out of scope for P3e — every `git/*.rs` fn still opens from
/// `path`; the shape simply makes per-repo caching a localized future change.
pub struct RepoEntry {
    pub path: PathBuf,
    pub watcher: Option<WatcherHandle>,
}

/// Shared app state: every open repo, keyed by `repoId` (canonical workdir
/// path string, P3e contract §2). One Mutex guards the whole map — safe because
/// handlers only hold the lock long enough to clone a `PathBuf` out (or
/// insert/remove an entry), never across the `spawn_blocking` git work.
#[derive(Default)]
pub struct AppState {
    pub repos: Mutex<HashMap<String, RepoEntry>>,
    /// The app's focused-tab repoId (or `None` when no repo is focused). Set by
    /// the frontend on tab switch / open / close and once on startup after
    /// session restore (P16 §5). Distinct from a per-MCP-session selection: this
    /// only *seeds* a new embedded-MCP session's initial repo; the AI may then
    /// re-point its own session without disturbing this value.
    pub active_repo: Mutex<Option<String>>,
}
