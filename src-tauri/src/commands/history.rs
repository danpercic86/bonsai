//! `history` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Per-line blame of `path` as of `at_oid` (`null`/omitted -> HEAD, P23
/// contract §9.1/§10). Errors: `other` (bad path) | `git` | `noRepo`. Does NOT
/// emit `repo-changed`.
#[tauri::command]
pub async fn blame_file(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    at_oid: Option<String>,
) -> Result<Vec<BlameLine>, AppError> {
    blame_file_inner(state.inner(), &repo_id, path, at_oid).await
}

/// Runtime-free core of `blame_file` (unit-testable without a Tauri app).
pub(crate) async fn blame_file_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    at_oid: Option<String>,
) -> Result<Vec<BlameLine>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || blame::blame_file(&workdir, &path, at_oid.as_deref()))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commits that modified `path`, newest-first, best-effort following a single
/// rename (P23 contract §9.2/§10). `limit == 0` -> the built-in `MAX_HISTORY`
/// cap. Errors: `other` (bad path) | `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn file_history(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    limit: u32,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    file_history_inner(state.inner(), &repo_id, path, limit).await
}

/// Runtime-free core of `file_history` (unit-testable without a Tauri app).
pub(crate) async fn file_history_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    limit: u32,
) -> Result<Vec<FileHistoryEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || blame::file_history(&workdir, &path, limit))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Reflog for `ref_name` ("HEAD" or a local branch name), newest-first, capped
/// at `MAX_REFLOG_ENTRIES`. A never-updated ref yields `[]` (not an error).
/// Read-only (P38 contract §5.1). Errors: `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn read_reflog(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    ref_name: String,
) -> Result<Vec<ReflogEntry>, AppError> {
    read_reflog_inner(state.inner(), &repo_id, ref_name).await
}

/// Runtime-free core of `read_reflog` (unit-testable without a Tauri app).
pub(crate) async fn read_reflog_inner(
    state: &AppState,
    repo_id: &str,
    ref_name: String,
) -> Result<Vec<ReflogEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || reflog::read_reflog(&workdir, &ref_name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
