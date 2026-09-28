//! P113d §6 — `AppState::stop_all_watchers`: drops every handle, keeps every
//! entry, idempotent.

use std::sync::{Arc, Mutex};

use super::{AppState, RepoEntry};

fn entry(path: &std::path::Path, watcher: Option<crate::watcher::WatcherHandle>) -> RepoEntry {
    RepoEntry {
        path: path.to_path_buf(),
        watcher,
        graph_cache: Arc::new(Mutex::new(None)),
    }
}

#[test]
fn stop_all_watchers_drops_handles_and_keeps_entries() {
    let dir = tempfile::TempDir::new().unwrap();
    git2::Repository::init(dir.path()).unwrap();
    let handle = crate::watcher::spawn_watcher(dir.path(), Box::new(|_| {})).unwrap();

    let state = AppState::default();
    {
        let mut repos = state.repos.lock().unwrap();
        repos.insert("a".into(), entry(std::path::Path::new("a"), None));
        repos.insert("b".into(), entry(std::path::Path::new("b"), None));
        repos.insert("live".into(), entry(dir.path(), Some(handle)));
    }

    state.stop_all_watchers();
    state.stop_all_watchers(); // idempotent

    let repos = state.repos.lock().unwrap();
    assert_eq!(repos.len(), 3, "entries are kept");
    assert!(repos.values().all(|e| e.watcher.is_none()));
    assert!(repos.contains_key("live"));
}
