//! `status` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Computes the working-directory status of `repo_id`.
#[tauri::command]
pub async fn get_status(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<StatusSnapshot, AppError> {
    get_status_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_status` (unit-testable without a Tauri app).
pub(crate) async fn get_status_inner(state: &AppState, repo_id: &str) -> Result<StatusSnapshot, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || read_status(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Computes the full commit-graph layout of `repo_id`.
///
/// Unborn-HEAD / zero-ref repos yield an empty layout (M2 contract §2.1),
/// not an error; `NoRepo` when nothing is open under that id.
#[tauri::command]
pub async fn get_graph(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<GraphLayout, AppError> {
    get_graph_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_graph` (unit-testable without a Tauri app).
pub(crate) async fn get_graph_inner(state: &AppState, repo_id: &str) -> Result<GraphLayout, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || compute_graph(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Streams the commit-graph layout of `repo_id` as ordered [`GraphChunk`]
/// batches over `on_chunk` (channel command; mirrors `history_index_build` /
/// `clone_repo`). The heavy git2 walk runs in `spawn_blocking`.
///
/// Wire order: exactly one `Meta`, then N `Batch`, then exactly one `Done`.
/// Unborn / zero-ref repos yield a `Meta` + `Done` pair, NOT an error (parity
/// with `get_graph`). `get_graph` is retained (small-repo / tests / mock reuse).
/// Rejects `NoRepo` when nothing is open under that id; git errors surface as
/// `AppError` (the command rejects instead of sending `Done`).
#[tauri::command]
pub async fn stream_graph(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    on_chunk: tauri::ipc::Channel<GraphChunk>,
) -> Result<(), AppError> {
    let path = repo_path(state.inner(), &repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        // `Channel::send` errs once the frontend drops the channel (component
        // unmount / repo switch / `close_repo`); `is_ok() == false` stops the
        // walk promptly with `Ok` (contract §6 cancellation).
        stream_graph_core(&path, |chunk| on_chunk.send(chunk).is_ok())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
