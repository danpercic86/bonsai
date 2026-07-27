use tauri::Emitter;

use crate::error::AppError;
use crate::git::repo::{read_repo_info, RepoInfo};
use crate::git::status::{read_status, StatusSnapshot};
use crate::state::{AppState, OpenRepo};
use crate::watcher::spawn_watcher;

/// Payload of the `"repo-changed"` event. `reason` is `"fs"` in M1; future
/// reasons (e.g. `"op"` after a commit) reuse this event.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoChangedPayload {
    pub reason: String,
}

/// Opens the folder at `path` as a repository and reports its state.
///
/// Bare repositories are reported (`bare: true`) but NOT stored in state and
/// get no watcher — Bonsai v1 is a working-copy client (M1 contract §3.3).
/// The frontend treats `bare: true` like `isRepo: false`.
///
/// For non-bare repos this (re)starts the file watcher: any previous watcher
/// is dropped first, so re-invoking on the same path is idempotent and
/// self-heals a dead watcher (this is what the refresh button relies on).
#[tauri::command]
pub async fn open_repo(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<RepoInfo, AppError> {
    let path_buf = std::path::PathBuf::from(&path);
    let info = tauri::async_runtime::spawn_blocking(move || read_repo_info(&path_buf))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))??;

    if info.is_repo && !info.bare {
        let workdir = std::path::PathBuf::from(&info.path);

        // Stop any previous watcher BEFORE storing the new repo path: the old
        // handle drops here, its debounce thread joins.
        {
            let mut watcher = state
                .watcher
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            *watcher = None;
        }

        {
            let mut repo = state
                .repo
                .lock()
                .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
            *repo = Some(OpenRepo {
                path: workdir.clone(),
            });
        }

        // Watch failure is non-fatal (M1 contract §4): manual refresh + focus
        // rescan keep the app correct even without filesystem events.
        match spawn_watcher(
            &workdir,
            Box::new(move || {
                let _ = app.emit(
                    "repo-changed",
                    RepoChangedPayload {
                        reason: "fs".to_string(),
                    },
                );
            }),
        ) {
            Ok(handle) => {
                let mut watcher = state
                    .watcher
                    .lock()
                    .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
                *watcher = Some(handle);
            }
            Err(e) => {
                eprintln!("bonsai: file watcher failed to start (falling back to manual refresh): {e}");
            }
        }
    }
    Ok(info)
}

/// Computes the working-directory status of the currently open repository.
#[tauri::command]
pub async fn get_status(state: tauri::State<'_, AppState>) -> Result<StatusSnapshot, AppError> {
    let path = {
        let repo = state
            .repo
            .lock()
            .map_err(|_| AppError::Other("state lock poisoned".to_string()))?;
        repo.as_ref().ok_or(AppError::NoRepo)?.path.clone()
    };

    tauri::async_runtime::spawn_blocking(move || read_status(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
