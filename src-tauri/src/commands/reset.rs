//! `reset` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Moves the current branch (HEAD) to `oid` in the given `mode` (P20 contract
/// §3). Hard is destructive — the UI confirms first. Errors:
/// `operationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn reset_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    mode: ResetMode,
) -> Result<(), AppError> {
    reset_branch_command_inner(state.inner(), &repo_id, oid, mode).await
}

/// Runtime-free core of `reset_branch` (unit-testable without a Tauri app).
pub(crate) async fn reset_branch_command_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    mode: ResetMode,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || reset_branch_core(&path, &oid, mode))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
