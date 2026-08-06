//! `submodules` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Lists every submodule with its classified status (P19 contract §3). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn list_submodules(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<SubmoduleInfo>, AppError> {
    list_submodules_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_submodules` (unit-testable without a Tauri app).
pub(crate) async fn list_submodules_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<SubmoduleInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::list_submodules(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Registers submodule `name` in .git/config — no worktree change (P19 contract
/// §3). Errors: `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn init_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    init_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `init_submodule` (unit-testable without a Tauri app).
pub(crate) async fn init_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::init_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Init-if-needed + fetch + checkout the pinned commit for submodule `name`
/// (P19 contract §3). Reuses the M6 credential chain. Errors: `invalidName` |
/// `authFailed` | `networkError` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn update_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    update_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `update_submodule` (unit-testable without a Tauri app).
pub(crate) async fn update_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::update_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Propagates the .gitmodules URL into config + the submodule remote for
/// submodule `name` (P19 contract §3). No worktree change. Errors:
/// `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn sync_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    sync_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `sync_submodule` (unit-testable without a Tauri app).
pub(crate) async fn sync_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::sync_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
