//! P125 §8: the dependency tracker's two commands. Thin: the view is built by
//! `deps::view` (blocking — cache read + git2 — so it runs in
//! `spawn_blocking`); a check is started by `deps::start_run`, which only
//! records it and detaches the worker. Settings go through
//! `get_ui_settings` / `set_ui_settings` (`depTracker`).
//!
//! Scanned by `crates/bonsai-deps/tests/no_process.rs` (§7.4): no process
//! API, and `AppError` is the only core-crate item allowed here.

use bonsai_core::error::AppError;

use super::shared::repo_path;
use crate::deps::{DepTrackerView, DepsState};
use crate::state::AppState;

/// The tracker view for one open repo. When the tracker is disabled it never
/// touches the repo or the cache. Rejects `noRepo`.
#[tauri::command]
pub async fn deps_get_view(
    state: tauri::State<'_, AppState>,
    deps: tauri::State<'_, DepsState>,
    repo_id: String,
) -> Result<DepTrackerView, AppError> {
    let workdir = repo_path(&state, &repo_id)?;
    let deps = deps.inner().clone();
    if !deps.is_enabled() {
        return Ok(DepTrackerView::disabled(repo_id));
    }
    tauri::async_runtime::spawn_blocking(move || crate::deps::view(&deps, &repo_id, &workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

/// Manual refresh: resolves the running check's id, or starts one. Rejects
/// `noRepo` and `other("dependency tracker is disabled")`.
#[tauri::command]
pub async fn deps_check_now(
    state: tauri::State<'_, AppState>,
    deps: tauri::State<'_, DepsState>,
    repo_id: String,
) -> Result<u64, AppError> {
    let workdir = repo_path(&state, &repo_id)?;
    crate::deps::start_run(&deps, &repo_id, workdir)
}
