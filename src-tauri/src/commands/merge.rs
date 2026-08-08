//! `merge` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Current operation state (merge / rebase / cherry-pick / revert / none).
/// Part of the frontend refresh batch (P3c contract §6). Errors: `noRepo`
/// | `git`.
#[tauri::command]
pub async fn get_op_state(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RepoOpState, AppError> {
    get_op_state_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_op_state` (unit-testable without a Tauri app).
pub(crate) async fn get_op_state_inner(state: &AppState, repo_id: &str) -> Result<RepoOpState, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || read_op_state(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Merges a local or remote-tracking branch into the current branch (P3c
/// contract §4). Errors: `operationInProgress` | `branchNotFound`
/// | `checkoutConflict` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn merge_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<MergeOutcome, AppError> {
    merge_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `merge_branch` (unit-testable without a Tauri app).
pub(crate) async fn merge_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<MergeOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::merge_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused merge as a 2(+)-parent commit (P3c contract §4.4).
/// `sign` (P58 OQ4): `null`/absent ⇒ follow `commit.gpgsign`; `true`/`false`
/// force. Errors: `noOperationInProgress` | `unresolvedConflicts` |
/// `emptyMessage` | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn commit_merge(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
    sign: Option<bool>,
) -> Result<CommitResult, AppError> {
    commit_merge_inner(state.inner(), &repo_id, message, sign).await
}

/// Runtime-free core of `commit_merge` (unit-testable without a Tauri app).
pub(crate) async fn commit_merge_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
    sign: Option<bool>,
) -> Result<CommitResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::commit_merge(&path, &message, sign))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused merge (worktree-destructive for merge-touched files —
/// the UI confirms first; P3c contract §4.5). Errors: `noOperationInProgress`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn abort_merge(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    abort_merge_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `abort_merge` (unit-testable without a Tauri app).
pub(crate) async fn abort_merge_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::abort_merge(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// All current index conflicts, path-ascending (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_conflicts(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<ConflictEntry>, AppError> {
    list_conflicts_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_conflicts` (unit-testable without a Tauri app).
pub(crate) async fn list_conflicts_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<ConflictEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || conflict::list_conflicts(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Read-only marker view of one conflicted file (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_conflict(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<ConflictFile, AppError> {
    get_conflict_inner(state.inner(), &repo_id, path).await
}

/// Runtime-free core of `get_conflict` (unit-testable without a Tauri app).
pub(crate) async fn get_conflict_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
) -> Result<ConflictFile, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || conflict::get_conflict(&workdir, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resolves one conflicted path per the P3c contract §3.2 matrix.
/// Errors: `noRepo` | `git` | `invalidName` (validate_rel_path).
#[tauri::command]
pub async fn resolve_conflict(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    resolve_conflict_inner(state.inner(), &repo_id, path, resolution).await
}

/// Runtime-free core of `resolve_conflict` (unit-testable without a Tauri app).
pub(crate) async fn resolve_conflict_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict(&workdir, &path, resolution)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stages user-authored resolved text for one conflicted path (P12 §1.2).
/// Errors: `noRepo` | `git` | `invalidName`. Does NOT emit `repo-changed` —
/// the frontend refetches imperatively.
#[tauri::command]
pub async fn resolve_conflict_text(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    content: String,
) -> Result<(), AppError> {
    resolve_conflict_text_inner(state.inner(), &repo_id, path, content).await
}

/// Runtime-free core of `resolve_conflict_text` (unit-testable without a Tauri app).
pub(crate) async fn resolve_conflict_text_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict_text(&workdir, &path, &content)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
