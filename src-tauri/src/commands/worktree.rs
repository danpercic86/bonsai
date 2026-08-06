//! `worktree` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Lists every worktree — the synthesized main row first, then each linked
/// worktree — with resolved branch/oid/badges (P27 contract §3). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn list_worktrees(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<WorktreeInfo>, AppError> {
    list_worktrees_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_worktrees` (unit-testable without a Tauri app).
pub(crate) async fn list_worktrees_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<WorktreeInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::list_worktrees(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a linked worktree checking out the EXISTING local branch `branch` at
/// a derived `<parent>/.worktrees/<repo-name>/<name-slug>` path; the on-disk
/// `name` is user-editable and decoupled from `branch` (P32 Part A — a blank
/// `name` defaults to `branch`). Returns the created row (P27 contract §3).
/// Errors: `noRepo` | `invalidName` | `branchNotFound` | `git` | `io`. Does NOT
/// emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn add_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    branch: String,
    name: String,
) -> Result<WorktreeInfo, AppError> {
    add_worktree_inner(state.inner(), &repo_id, branch, name).await
}

/// Runtime-free core of `add_worktree` (unit-testable without a Tauri app).
pub(crate) async fn add_worktree_inner(
    state: &AppState,
    repo_id: &str,
    branch: String,
    name: String,
) -> Result<WorktreeInfo, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::add_worktree(&path, &branch, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Removes linked worktree `name` — refuses main/current/locked/dirty, then
/// prunes admin files + working directory (P27 contract §3). Errors: `noRepo`
/// | `invalidName` | `git` | `io`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn remove_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    remove_worktree_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `remove_worktree` (unit-testable without a Tauri app).
pub(crate) async fn remove_worktree_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::remove_worktree(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Locks linked worktree `name` with an optional reason (P27 contract §3).
/// Errors: `noRepo` | `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn lock_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    reason: Option<String>,
) -> Result<(), AppError> {
    lock_worktree_inner(state.inner(), &repo_id, name, reason).await
}

/// Runtime-free core of `lock_worktree` (unit-testable without a Tauri app).
pub(crate) async fn lock_worktree_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    reason: Option<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        worktree::lock_worktree(&path, &name, reason.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unlocks linked worktree `name` (P27 contract §3). Errors: `noRepo` |
/// `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn unlock_worktree(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    unlock_worktree_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `unlock_worktree` (unit-testable without a Tauri app).
pub(crate) async fn unlock_worktree_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree::unlock_worktree(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Lists uncommitted + gitignored files eligible to copy into a new worktree
/// (P32 Part B). Groups: staged / unstaged / untracked / ignored; deletions
/// excluded. Errors: `noRepo` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn list_copy_candidates(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<CopyCandidate>, AppError> {
    list_copy_candidates_inner(state.inner(), &repo_id).await
}

/// Runs the blocking core of `list_copy_candidates` under the async runtime
/// (testable directly under a tokio runtime, no Tauri app required).
pub(crate) async fn list_copy_candidates_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<CopyCandidate>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || worktree_copy::list_copy_candidates(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Classifies `paths` against target `branch` (clean/conflict) BEFORE creating
/// the worktree (P32 Part B). Errors: `noRepo` | `branchNotFound` | `git`. Does
/// NOT emit `repo-changed`.
#[tauri::command]
pub async fn preview_worktree_copy(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    branch: String,
    paths: Vec<String>,
) -> Result<Vec<CopyPlanEntry>, AppError> {
    preview_worktree_copy_inner(state.inner(), &repo_id, branch, paths).await
}

/// Runs the blocking core of `preview_worktree_copy` under the async runtime
/// (testable directly under a tokio runtime, no Tauri app required).
pub(crate) async fn preview_worktree_copy_inner(
    state: &AppState,
    repo_id: &str,
    branch: String,
    paths: Vec<String>,
) -> Result<Vec<CopyPlanEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        worktree_copy::classify_copy(&path, &branch, &paths)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates the worktree (Part A branch/name), then copies each `copy` selection's
/// source bytes into it; `skip` selections are not written; empty behaves like a
/// plain `add_worktree` (P32 Part B). Errors: `noRepo` | `invalidName` |
/// `branchNotFound` | `git` | `io`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn add_worktree_with_changes(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    branch: String,
    name: String,
    selections: Vec<CopySelection>,
) -> Result<WorktreeInfo, AppError> {
    add_worktree_with_changes_inner(state.inner(), &repo_id, branch, name, selections).await
}

/// Runs the blocking core of `add_worktree_with_changes` under the async runtime
/// (testable directly under a tokio runtime, no Tauri app required).
pub(crate) async fn add_worktree_with_changes_inner(
    state: &AppState,
    repo_id: &str,
    branch: String,
    name: String,
    selections: Vec<CopySelection>,
) -> Result<WorktreeInfo, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        worktree_copy::add_worktree_with_changes(&path, &branch, &name, &selections)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
