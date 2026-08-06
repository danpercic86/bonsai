//! `bisect` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Starts a git bisect: `bad` = known-bad commit, `good` = known-good
/// ancestor(s). Detaches HEAD onto the first midpoint (P39 contract §5). Errors:
/// `operationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed` — the
/// frontend refetches imperatively (op-state refresh).
#[tauri::command]
pub async fn start_bisect(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    bad: String,
    good: Vec<String>,
) -> Result<BisectOutcome, AppError> {
    start_bisect_inner(state.inner(), &repo_id, bad, good).await
}

/// Runtime-free core of `start_bisect` (unit-testable without a Tauri app).
pub(crate) async fn start_bisect_inner(
    state: &AppState,
    repo_id: &str,
    bad: String,
    good: Vec<String>,
) -> Result<BisectOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bisect::start_bisect(&path, &bad, &good))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Marks the current bisect midpoint good (`is_good = true`) or bad, then checks
/// out the next midpoint or converges (P39 contract §5). Errors:
/// `noOperationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn bisect_mark(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    is_good: bool,
) -> Result<BisectOutcome, AppError> {
    bisect_mark_inner(state.inner(), &repo_id, is_good).await
}

/// Runtime-free core of `bisect_mark` (unit-testable without a Tauri app).
pub(crate) async fn bisect_mark_inner(
    state: &AppState,
    repo_id: &str,
    is_good: bool,
) -> Result<BisectOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bisect::bisect_mark(&path, is_good))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Skips the current (untestable) bisect midpoint (P39 contract §5). Errors:
/// `noOperationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn bisect_skip(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<BisectOutcome, AppError> {
    bisect_skip_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `bisect_skip` (unit-testable without a Tauri app).
pub(crate) async fn bisect_skip_inner(state: &AppState, repo_id: &str) -> Result<BisectOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bisect::bisect_skip(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts/finishes a bisect: force-restore the original HEAD/branch + worktree
/// (worktree-destructive — the UI confirms first; P39 contract §5). Errors:
/// `noOperationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn bisect_reset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    bisect_reset_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `bisect_reset` (unit-testable without a Tauri app).
pub(crate) async fn bisect_reset_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bisect::bisect_reset(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
