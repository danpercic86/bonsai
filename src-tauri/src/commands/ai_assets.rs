//! `ai_assets` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Full AI-asset inventory + drift for `repo_id` (P24 contract §6.1). Optional
/// `canonical` overrides the drift reference asset id. No events, no channels —
/// the frontend refetches imperatively (and on the existing `repo-changed`).
#[tauri::command]
pub async fn list_ai_assets(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    canonical: Option<String>,
) -> Result<AiAssetInventory, AppError> {
    list_ai_assets_inner(state.inner(), &repo_id, canonical).await
}

/// Runtime-free core of `list_ai_assets` (unit-testable without a Tauri app).
pub(crate) async fn list_ai_assets_inner(
    state: &AppState,
    repo_id: &str,
    canonical: Option<String>,
) -> Result<AiAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        assets::scan_inventory(&workdir, canonical.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Raw content of one AI-asset file under `repo_id` (P24 §3, read path). The
/// path is validated to stay inside the workdir; a missing file yields
/// `exists:false` (not an error).
#[tauri::command]
pub async fn read_ai_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<AssetContent, AppError> {
    read_ai_asset_inner(state.inner(), &repo_id, path).await
}

/// Runtime-free core of `read_ai_asset` (unit-testable without a Tauri app).
pub(crate) async fn read_ai_asset_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
) -> Result<AssetContent, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::read_asset(&workdir, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Managed inventory of the three `.claude/` agent-asset kinds (skills /
/// subagents / slash commands) under `repo_id` (P26 §5, read path). Parses +
/// validates each; a missing `.claude/` yields an empty inventory. No events,
/// no channels — the frontend refetches imperatively (and on `repo-changed`).
#[tauri::command]
pub async fn list_agent_assets(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<AgentAssetInventory, AppError> {
    list_agent_assets_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_agent_assets` (unit-testable without a Tauri app).
pub(crate) async fn list_agent_assets_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<AgentAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::scan_agent_assets(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One parsed agent asset by `(kind, name)` under `repo_id` (P26 §5, read path).
/// The name is validated for filesystem safety; a missing file resolves to an
/// `exists:false` shell (not an error).
#[tauri::command]
pub async fn read_agent_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAsset, AppError> {
    read_agent_asset_inner(state.inner(), &repo_id, kind, name).await
}

/// Runtime-free core of `read_agent_asset` (unit-testable without a Tauri app).
pub(crate) async fn read_agent_asset_inner(
    state: &AppState,
    repo_id: &str,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAsset, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::read_agent_asset(&workdir, kind, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Create or overwrite an agent asset under `repo_id` (P26 §5, write path).
/// Validates the name + computed path; atomic temp+rename with parent-dir
/// creation (incl. the skill's `<name>/` dir). Returns the refreshed inventory.
/// Missing required fields do NOT block the write — they surface as `valid:false`
/// in the returned inventory. No consent gate; no events/channels — the frontend
/// refetches (and the watcher fires `repo-changed` on the `.claude/` write).
#[tauri::command]
pub async fn save_agent_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    asset: AgentAssetInput,
) -> Result<AgentAssetInventory, AppError> {
    save_agent_asset_inner(state.inner(), &repo_id, asset).await
}

/// Runtime-free core of `save_agent_asset`.
pub(crate) async fn save_agent_asset_inner(
    state: &AppState,
    repo_id: &str,
    asset: AgentAssetInput,
) -> Result<AgentAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::save_agent_asset(&workdir, asset))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Delete one agent asset by `(kind, name)` under `repo_id` (P26 §5). A **skill**
/// removes the whole `.claude/skills/<name>/` directory recursively (the UI
/// confirm spells this out); an agent/command removes the single `.md`. A missing
/// target is a no-op. Returns the refreshed inventory. No events/channels.
#[tauri::command]
pub async fn delete_agent_asset(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAssetInventory, AppError> {
    delete_agent_asset_inner(state.inner(), &repo_id, kind, name).await
}

/// Runtime-free core of `delete_agent_asset`.
pub(crate) async fn delete_agent_asset_inner(
    state: &AppState,
    repo_id: &str,
    kind: AgentAssetKind,
    name: String,
) -> Result<AgentAssetInventory, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || assets::delete_agent_asset(&workdir, kind, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
