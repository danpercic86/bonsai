//! `revert` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Reverts a single commit on the current branch (P20 contract §6). Clean →
/// auto-commits; conflict → pauses into RepoOpState::Revert. Errors:
/// `operationInProgress` | `git` | `checkoutConflict` | `configMissing`
/// | `nothingToCommit` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn revert_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<RevertOutcome, AppError> {
    revert_commit_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `revert_commit` (unit-testable without a Tauri app).
pub(crate) async fn revert_commit_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<RevertOutcome, AppError> {
    let subject = arg_target(state, TargetArg::Commit(&oid));
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::Revert,
        subject,
        LoggedPhase::Local,
        revert_outcome,
        move |path| revert::revert_commit(&path, &oid),
    )
    .await
}

/// Finalizes a paused (resolved) revert (P20 contract §6). Errors:
/// `noOperationInProgress` | `unresolvedConflicts` | `configMissing`
/// | `nothingToCommit` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn revert_continue(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RevertOutcome, AppError> {
    revert_continue_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `revert_continue`.
pub(crate) async fn revert_continue_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<RevertOutcome, AppError> {
    let subject = activity_target(state, repo_id, GitActivityCategory::RevertContinue).await;
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::RevertContinue,
        subject,
        LoggedPhase::Local,
        revert_outcome,
        move |path| revert::revert_continue(&path),
    )
    .await
}

/// Aborts a paused revert (reset --hard to HEAD; destructive — the UI confirms
/// first; P20 contract §6). Errors: `noOperationInProgress` | `git` | `noRepo`.
/// Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn revert_abort(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    revert_abort_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `revert_abort`.
pub(crate) async fn revert_abort_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let subject = activity_target(state, repo_id, GitActivityCategory::RevertAbort).await;
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::RevertAbort,
        subject,
        LoggedPhase::Local,
        no_outcome,
        move |path| revert::revert_abort(&path),
    )
    .await
}
