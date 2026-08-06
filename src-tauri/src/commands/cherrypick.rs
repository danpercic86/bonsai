//! `cherrypick` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Cherry-picks a single commit onto the current branch (P20 contract §5).
/// Clean → auto-commits; conflict → pauses into RepoOpState::CherryPick.
/// Errors: `operationInProgress` | `git` | `checkoutConflict` | `configMissing`
/// | `nothingToCommit` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn cherrypick_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    message: Option<String>,
) -> Result<CherrypickOutcome, AppError> {
    cherrypick_commit_inner(state.inner(), &repo_id, oid, message).await
}

/// Runtime-free core of `cherrypick_commit` (unit-testable without a Tauri app).
pub(crate) async fn cherrypick_commit_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    message: Option<String>,
) -> Result<CherrypickOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        cherrypick::cherrypick_commit(&path, &oid, message.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused (resolved) cherry-pick (P20 contract §5). Errors:
/// `noOperationInProgress` | `unresolvedConflicts` | `configMissing`
/// | `nothingToCommit` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn cherrypick_continue(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<CherrypickOutcome, AppError> {
    cherrypick_continue_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `cherrypick_continue`.
pub(crate) async fn cherrypick_continue_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<CherrypickOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || cherrypick::cherrypick_continue(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused cherry-pick (reset --hard to HEAD; destructive — the UI
/// confirms first; P20 contract §5). Errors: `noOperationInProgress` | `git`
/// | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn cherrypick_abort(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    cherrypick_abort_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `cherrypick_abort`.
pub(crate) async fn cherrypick_abort_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || cherrypick::cherrypick_abort(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
