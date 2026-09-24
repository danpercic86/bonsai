//! `reset` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Moves the current branch (HEAD) to `oid` in the given `mode` (P20 contract
/// §3). Hard is destructive — the UI confirms first. Errors:
/// `operationInProgress` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn reset_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    mode: ResetMode,
) -> Result<(), AppError> {
    reset_branch_command_inner(state.inner(), &repo_id, oid, mode).await
}

/// Runtime-free core of `reset_branch` (unit-testable without a Tauri app).
pub(crate) async fn reset_branch_command_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    mode: ResetMode,
) -> Result<(), AppError> {
    // P119 §1 rows 22-24: the mode picks the category (Undo executes here too).
    let category = match mode {
        ResetMode::Soft => GitActivityCategory::ResetSoft,
        ResetMode::Mixed => GitActivityCategory::ResetMixed,
        ResetMode::Hard => GitActivityCategory::ResetHard,
    };
    let subject = arg_target(state, TargetArg::Commit(&oid));
    logged_blocking(
        state,
        repo_id,
        category,
        subject,
        LoggedPhase::Local,
        no_outcome,
        move |path| reset_branch_core(&path, &oid, mode),
    )
    .await
}
