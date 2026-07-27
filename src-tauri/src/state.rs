/// The repository currently open in the app.
#[derive(Debug, Clone)]
pub struct OpenRepo {
    /// Workdir root of the opened repo.
    pub path: std::path::PathBuf,
}

/// Shared application state, registered via `.manage(AppState::default())`.
///
/// The watcher lives behind its own lock (M1 contract §4) so `OpenRepo`
/// stays `Clone`/`Debug`; `WatcherHandle` is neither.
#[derive(Default)]
pub struct AppState {
    pub repo: std::sync::Mutex<Option<OpenRepo>>,
    pub watcher: std::sync::Mutex<Option<crate::watcher::WatcherHandle>>,
}
