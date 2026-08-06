//! `profiles` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// The context-profile store for `repo_id` (P24 §6). Lazy default when absent.
#[tauri::command]
pub async fn list_profiles(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<ProfileStore, AppError> {
    list_profiles_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_profiles` (unit-testable without a Tauri app).
pub(crate) async fn list_profiles_inner(state: &AppState, repo_id: &str) -> Result<ProfileStore, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::list_profiles(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Insert-or-replace a profile keyed by name, then persist (P24 §5.2). Rejects
/// invalid names / non-single-file targets with `invalidName`.
#[tauri::command]
pub async fn save_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    profile: ContextProfile,
) -> Result<ProfileStore, AppError> {
    save_profile_inner(state.inner(), &repo_id, profile).await
}

/// Runtime-free core of `save_profile`.
pub(crate) async fn save_profile_inner(
    state: &AppState,
    repo_id: &str,
    profile: ContextProfile,
) -> Result<ProfileStore, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::save_profile(&workdir, profile))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Remove a profile (no-op if absent), clearing `activeProfile` if it matched
/// (P24 §5.2). Returns the updated store.
#[tauri::command]
pub async fn delete_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<ProfileStore, AppError> {
    delete_profile_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_profile`.
pub(crate) async fn delete_profile_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<ProfileStore, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::delete_profile(&workdir, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-target before/after preview for a profile's activation (P24 §5.2).
/// Writes nothing — the UI's diff-preview safety gate.
#[tauri::command]
pub async fn preview_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    preview_profile_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `preview_profile`.
pub(crate) async fn preview_profile_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::preview_profile(&workdir, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Activate a profile: write each target's content to its mapped file (atomic
/// temp+rename), set `activeProfile`, persist (P24 §5.2). The one write path;
/// UI-gated behind confirm + preview.
#[tauri::command]
pub async fn activate_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<ProfileActivation, AppError> {
    activate_profile_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `activate_profile`.
pub(crate) async fn activate_profile_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<ProfileActivation, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::activate_profile(&workdir, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-worktree AI-context matrix: every worktree row joined with its active
/// profile + drift/missing counts (P31 §5). Read-only. Errors: `noRepo` |
/// `git` | `other` | `io`.
#[tauri::command]
pub async fn list_worktree_contexts(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<WorktreeContextStatus>, AppError> {
    list_worktree_contexts_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_worktree_contexts` (unit-testable without a Tauri app).
pub(crate) async fn list_worktree_contexts_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<WorktreeContextStatus>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::list_worktree_contexts(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Per-target before/after preview for activating profile `name` onto
/// WORKTREE `worktree_key` (P31 §5). Writes nothing — the UI's diff-preview
/// safety gate. Enforces D6 eligibility (locked/invalid/prunable → `git`).
/// Errors: `noRepo` | `git` | `other` | `io`.
#[tauri::command]
pub async fn preview_worktree_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    worktree_key: String,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    preview_worktree_profile_inner(state.inner(), &repo_id, worktree_key, name).await
}

/// Runtime-free core of `preview_worktree_profile`.
pub(crate) async fn preview_worktree_profile_inner(
    state: &AppState,
    repo_id: &str,
    worktree_key: String,
    name: String,
) -> Result<Vec<ProfilePreviewEntry>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        assets::preview_profile_for_worktree(&workdir, &worktree_key, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Activate profile `name` onto WORKTREE `worktree_key` — THE one write path
/// (P31 §4), UI-gated behind confirm + preview like `activate_profile`. The
/// core enforces D6 eligibility and the D7 dirty-target guard (all targets
/// checked before any write). Errors: `noRepo` | `invalidName` | `git` |
/// `other` | `io`.
#[tauri::command]
pub async fn activate_worktree_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    worktree_key: String,
    name: String,
) -> Result<ProfileActivation, AppError> {
    activate_worktree_profile_inner(state.inner(), &repo_id, worktree_key, name).await
}

/// Runtime-free core of `activate_worktree_profile`.
pub(crate) async fn activate_worktree_profile_inner(
    state: &AppState,
    repo_id: &str,
    worktree_key: String,
    name: String,
) -> Result<ProfileActivation, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        assets::activate_profile_for_worktree(&workdir, &worktree_key, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
