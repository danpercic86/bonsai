//! `undo` command (P60c) — READ-ONLY last-operation classifier.
//!
//! `describe_last_undo` reads HEAD reflog[0] and returns an [`UndoPlan`] naming
//! how to reverse the last op. It mutates NOTHING — execution reuses the shipped
//! `reset_branch` command behind an explicit confirm dialog (P38 invariant), so
//! this command does NOT emit `repo-changed`.

use super::shared::*;
use bonsai_core::git::undo::{self, UndoPlan};

/// Classify the last HEAD-moving operation and describe how to reverse it
/// (read-only, contract §P60c). Errors: `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn describe_last_undo(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<UndoPlan, AppError> {
    describe_last_undo_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `describe_last_undo` (unit-testable without a Tauri app).
pub(crate) async fn describe_last_undo_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<UndoPlan, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || undo::describe_last_undo(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
