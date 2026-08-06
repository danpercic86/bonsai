//! `discard` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Discards the selected changed lines of one tracked working-dir file: the
/// WORKTREE moves toward the INDEX; the index is never modified (P28 §2.1).
/// DESTRUCTIVE — the UI confirms first. Empty selection is a no-op. Does NOT
/// emit `repo-changed` — the frontend refetches imperatively.
/// Errors: `noRepo` | `git` (untracked) | `other` (stale/unsupported/invalid path).
#[tauri::command]
pub async fn discard_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    discard_partial_inner(state.inner(), &repo_id, path, orig_path, selection).await
}

/// Runtime-free core of `discard_partial` (unit-testable without a Tauri app).
pub(crate) async fn discard_partial_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        discard_partial_core(&workdir, &path, orig_path.as_deref(), &selection)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Restores each tracked path's worktree content to the index version,
/// discarding unstaged edits (P20 contract §4). Destructive — the UI confirms
/// first. Errors: `other` (invalid path) | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn discard_paths(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    discard_paths_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `discard_paths` (unit-testable without a Tauri app).
pub(crate) async fn discard_paths_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || discard_paths_core(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Force-discards a mixed set: tracked paths restored to index, untracked paths
/// deleted from disk. Destructive — the UI confirms first. Errors: `other`
/// (invalid path) | `io` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn discard_paths_force(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    discard_paths_force_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `discard_paths_force` (unit-testable without a Tauri app).
pub(crate) async fn discard_paths_force_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || discard_paths_force_core(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
