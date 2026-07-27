/// The repository currently open in the app.
#[derive(Debug, Clone)]
pub struct OpenRepo {
    /// Workdir root of the opened repo.
    pub path: std::path::PathBuf,
}

/// Shared application state, registered via `.manage(AppState::default())`.
#[derive(Debug, Default)]
pub struct AppState {
    pub repo: std::sync::Mutex<Option<OpenRepo>>,
}
