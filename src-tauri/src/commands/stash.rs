//! `stash` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Enumerates the stash stack, index 0 (most recent) first (P9 contract §3).
/// Errors: `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn list_stashes(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<StashEntry>, AppError> {
    list_stashes_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_stashes` (unit-testable without a Tauri app).
pub(crate) async fn list_stashes_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<StashEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::list_stashes(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stashes the dirty worktree (P9 contract §3). `message: None` → git default.
/// `created:false` == nothing to stash (NOT an error). Errors:
/// `operationInProgress` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn create_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: Option<String>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError> {
    create_stash_inner(state.inner(), &repo_id, message, scope).await
}

/// Runtime-free core of `create_stash` (unit-testable without a Tauri app).
pub(crate) async fn create_stash_inner(
    state: &AppState,
    repo_id: &str,
    message: Option<String>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stash::create_stash(&path, message.as_deref(), scope)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Applies stash `index` WITHOUT dropping it (P9 contract §3). Conflicts →
/// `Conflicts{paths}` (stash retained). Errors: `operationInProgress` | `git`
/// | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn apply_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    apply_stash_inner(state.inner(), &repo_id, index, skip_reserved).await
}

/// Runtime-free core of `apply_stash` (unit-testable without a Tauri app).
pub(crate) async fn apply_stash_inner(
    state: &AppState,
    repo_id: &str,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::apply_stash(&path, index, skip_reserved))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Applies stash `index` and drops it on clean success only (P9 contract §3).
/// Conflicts → `Conflicts{paths}` and the entry is RETAINED. Errors:
/// `operationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn pop_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    pop_stash_inner(state.inner(), &repo_id, index, skip_reserved).await
}

/// Runtime-free core of `pop_stash` (unit-testable without a Tauri app).
pub(crate) async fn pop_stash_inner(
    state: &AppState,
    repo_id: &str,
    index: usize,
    skip_reserved: bool,
) -> Result<ApplyStashOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::pop_stash(&path, index, skip_reserved))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Permanently discards stash `index` (P9 contract §3). Allowed in any repo
/// state (the UI confirms first — destructive). Errors: `git` | `noRepo`. Does
/// NOT emit `repo-changed`.
#[tauri::command]
pub async fn drop_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    index: usize,
) -> Result<(), AppError> {
    drop_stash_inner(state.inner(), &repo_id, index).await
}

/// Runtime-free core of `drop_stash` (unit-testable without a Tauri app).
pub(crate) async fn drop_stash_inner(
    state: &AppState,
    repo_id: &str,
    index: usize,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stash::drop_stash(&path, index))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
