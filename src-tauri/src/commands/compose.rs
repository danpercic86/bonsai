//! `compose` command — apply a reviewed commit-composer plan (P54b).
//!
//! Pure git, NOT AI-gated (the reviewed plan is the user's own; it must still
//! apply if AI is later toggled off). Mirrors `commit`'s house shape:
//! `repo_path` → `spawn_blocking` → `map_err(join)`. Does NOT emit `repo-changed`
//! — the composer hook refetches graph+status on success.

use super::shared::*;

/// Applies a user-finalized composer plan as an ORDERED stage+commit sequence
/// (contract §5). ATOMIC: validates the whole plan, resets the index to HEAD
/// (working tree UNTOUCHED), then commits each group; ANY mid-sequence failure
/// rolls HEAD+index back so NOTHING is committed. Files in no group are left
/// uncommitted. Errors: `noRepo` | `operationInProgress` | `git` | `emptyMessage`
/// | `configMissing` | `nothingToCommit` | `other`.
#[tauri::command]
pub async fn apply_composed_commits(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    plan: ComposePlan,
) -> Result<ComposeApplyResult, AppError> {
    apply_composed_commits_inner(state.inner(), &repo_id, plan).await
}

/// Runtime-free core of `apply_composed_commits` (unit-testable without a Tauri app).
pub(crate) async fn apply_composed_commits_inner(
    state: &AppState,
    repo_id: &str,
    plan: ComposePlan,
) -> Result<ComposeApplyResult, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        compose_apply::apply_composed_commits(&workdir, &plan)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
