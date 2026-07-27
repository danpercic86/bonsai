use crate::error::AppError;
use crate::git::repo::{read_repo_info, RepoInfo};
use crate::state::{AppState, OpenRepo};

/// Opens the folder at `path` as a repository and reports its state.
///
/// M0a stub wiring: delegates to the `read_repo_info` stub via
/// `spawn_blocking`; M0b fills in the real repo inspection.
#[tauri::command]
pub async fn open_repo(
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, AppError> {
    let path_buf = std::path::PathBuf::from(&path);
    let info = tauri::async_runtime::spawn_blocking(move || read_repo_info(&path_buf))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    if info.is_repo {
        let mut repo = state
            .repo
            .lock()
            .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
        *repo = Some(OpenRepo {
            path: std::path::PathBuf::from(&info.path),
        });
    }
    Ok(info)
}
