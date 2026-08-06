//! `health` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Collects all four repo-health sections in ONE round-trip (P29 contract
/// §D2/§D4). Per-section failures are folded into `Section.error` inside the
/// payload; the command itself errors only for `noRepo` (unknown id) or a
/// join failure. READ-ONLY — never emits `repo-changed`.
#[tauri::command]
pub async fn get_repo_health(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RepoHealth, AppError> {
    get_repo_health_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_repo_health` (unit-testable without a Tauri app).
pub(crate) async fn get_repo_health_inner(state: &AppState, repo_id: &str) -> Result<RepoHealth, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || collect_repo_health(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}
