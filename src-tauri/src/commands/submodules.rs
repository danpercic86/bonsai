//! `submodules` commands — split from the former monolithic `commands.rs`.

use super::shared::*;
use bonsai_core::git::search::SpawnGitRunner;

/// Lists every submodule with its classified status (P19 contract §3). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn list_submodules(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<SubmoduleInfo>, AppError> {
    list_submodules_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_submodules` (unit-testable without a Tauri app).
pub(crate) async fn list_submodules_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<SubmoduleInfo>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::list_submodules(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Registers submodule `name` in .git/config — no worktree change (P19 contract
/// §3). Errors: `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn init_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    init_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `init_submodule` (unit-testable without a Tauri app).
pub(crate) async fn init_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::init_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Init-if-needed + fetch + checkout the pinned commit for submodule `name`
/// (P19 contract §3). Reuses the M6 credential chain. Errors: `invalidName` |
/// `authFailed` | `networkError` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn update_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    update_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `update_submodule` (unit-testable without a Tauri app).
pub(crate) async fn update_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::update_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Propagates the .gitmodules URL into config + the submodule remote for
/// submodule `name` (P19 contract §3). No worktree change. Errors:
/// `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn sync_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    sync_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `sync_submodule` (unit-testable without a Tauri app).
pub(crate) async fn sync_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::sync_submodule(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Adds a submodule at repo-relative `path` from `url` (P60d §D4): git2 clone
/// via the shared M6 credential chain, then stage .gitmodules + the gitlink.
/// Errors: `invalidName` | `git` (incl. network/auth) | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches submodules + status + graph.
#[tauri::command]
pub async fn add_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    url: String,
    path: String,
) -> Result<SubmoduleInfo, AppError> {
    add_submodule_inner(state.inner(), &repo_id, url, path).await
}

/// Runtime-free core of `add_submodule` (unit-testable without a Tauri app).
pub(crate) async fn add_submodule_inner(
    state: &AppState,
    repo_id: &str,
    url: String,
    path: String,
) -> Result<SubmoduleInfo, AppError> {
    let repo = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || submodule::add_submodule(&repo, &url, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deinits submodule `name` (P60d §D4, shell-out): `git submodule deinit -f --
/// <path>` — clears its config + empties the worktree, KEEPS .gitmodules.
/// Errors: `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn deinit_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    deinit_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `deinit_submodule` (unit-testable without a Tauri app).
pub(crate) async fn deinit_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        submodule::deinit_submodule(&path, &SpawnGitRunner, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Removes submodule `name` entirely (P60d §D4, shell-out): deinit → `git rm -f
/// -- <path>` → best-effort drop of `.git/modules/<name>`. DESTRUCTIVE. Errors:
/// `invalidName` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn remove_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    remove_submodule_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `remove_submodule` (unit-testable without a Tauri app).
pub(crate) async fn remove_submodule_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        submodule::remove_submodule(&path, &SpawnGitRunner, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
