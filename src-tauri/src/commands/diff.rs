//! `diff` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Diff of one working-dir file (M4 contract §2.2/§2.8).
/// `staged == false`: index vs workdir; `staged == true`: HEAD vs index.
/// `orig_path`: pass `StatusEntry.origPath` for renames.
#[tauri::command]
pub async fn get_workdir_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    staged: bool,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    get_workdir_file_diff_inner(state.inner(), &repo_id, path, orig_path, staged, full_context).await
}

/// Runtime-free core of `get_workdir_file_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_workdir_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    staged: bool,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        workdir_file_diff(&workdir, &path, orig_path.as_deref(), staged, full_context)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commit details + per-file headers for `oid` vs its first parent
/// (M4 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_commit_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CommitDiff, AppError> {
    get_commit_diff_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `get_commit_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_commit_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CommitDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || commit_diff(&workdir, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for ONE file of a commit's first-parent diff (M4 contract §2.2/§2.8).
#[tauri::command]
pub async fn get_commit_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    get_commit_file_diff_inner(state.inner(), &repo_id, oid, path, orig_path, full_context).await
}

/// Runtime-free core of `get_commit_file_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_commit_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        commit_file_diff(&workdir, &oid, &path, orig_path.as_deref(), full_context)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// HEAD → `oid` tree comparison (P5 §1.2). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn compare_with_head(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CompareDiff, AppError> {
    compare_with_head_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `compare_with_head` (unit-testable without a Tauri app).
pub(crate) async fn compare_with_head_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CompareDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || compare_head_diff(&workdir, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for one file of the HEAD → `oid` comparison. Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn compare_with_head_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    compare_with_head_file_diff_inner(state.inner(), &repo_id, oid, path, orig_path, full_context)
        .await
}

/// Runtime-free core of `compare_with_head_file_diff`.
pub(crate) async fn compare_with_head_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        compare_head_file_diff(&workdir, &oid, &path, orig_path.as_deref(), full_context)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
