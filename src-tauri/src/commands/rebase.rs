//! `rebase` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Starts a rebase of the current branch onto `onto` (local or remote-tracking
/// shorthand; P3d contract §3). Errors: `operationInProgress` | `branchNotFound`
/// | `checkoutConflict` | `configMissing` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn rebase_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    onto: String,
) -> Result<RebaseOutcome, AppError> {
    rebase_branch_inner(state.inner(), &repo_id, onto).await
}

/// Runtime-free core of `rebase_branch` (unit-testable without a Tauri app).
pub(crate) async fn rebase_branch_inner(
    state: &AppState,
    repo_id: &str,
    onto: String,
) -> Result<RebaseOutcome, AppError> {
    let subject = arg_target(state, TargetArg::Ref(&onto));
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::Rebase,
        subject,
        LoggedPhase::Local,
        rebase_outcome,
        move |path| rebase::rebase_branch(&path, &onto),
    )
    .await
}

/// Resumes a paused rebase — commits the resolved op, then replays on (P3d
/// contract §3.7). Errors: `noOperationInProgress` | `unresolvedConflicts`
/// | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn rebase_continue(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RebaseOutcome, AppError> {
    rebase_continue_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `rebase_continue` (unit-testable without a Tauri app).
pub(crate) async fn rebase_continue_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<RebaseOutcome, AppError> {
    let subject = activity_target(state, repo_id, GitActivityCategory::RebaseContinue).await;
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::RebaseContinue,
        subject,
        LoggedPhase::Local,
        rebase_outcome,
        move |path| rebase::rebase_continue(&path),
    )
    .await
}

/// Skips the current operation and resumes (P3d contract §3.8). Errors:
/// `noOperationInProgress` | `configMissing` | `git` | `noRepo`.
#[tauri::command]
pub async fn rebase_skip(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RebaseOutcome, AppError> {
    rebase_skip_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `rebase_skip` (unit-testable without a Tauri app).
pub(crate) async fn rebase_skip_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<RebaseOutcome, AppError> {
    let subject = activity_target(state, repo_id, GitActivityCategory::RebaseSkip).await;
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::RebaseSkip,
        subject,
        LoggedPhase::Local,
        rebase_outcome,
        move |path| rebase::rebase_skip(&path),
    )
    .await
}

/// Aborts a paused rebase (worktree-destructive — the UI confirms first; P3d
/// contract §3.10). Errors: `noOperationInProgress` | `git` | `noRepo`.
#[tauri::command]
pub async fn rebase_abort(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    rebase_abort_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `rebase_abort` (unit-testable without a Tauri app).
pub(crate) async fn rebase_abort_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let subject = activity_target(state, repo_id, GitActivityCategory::RebaseAbort).await;
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::RebaseAbort,
        subject,
        LoggedPhase::Local,
        no_outcome,
        move |path| rebase::rebase_abort(&path),
    )
    .await
}

/// Returns the DEFAULT interactive-rebase plan (every commit `pick`, oldest-
/// first) for `base..HEAD`, seeding the plan editor (P23 contract §7). Errors:
/// `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn get_interactive_plan(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base_oid: String,
) -> Result<Vec<RebaseTodoOp>, AppError> {
    get_interactive_plan_inner(state.inner(), &repo_id, base_oid).await
}

/// Runtime-free core of `get_interactive_plan` (unit-testable without a Tauri app).
pub(crate) async fn get_interactive_plan_inner(
    state: &AppState,
    repo_id: &str,
    base_oid: String,
) -> Result<Vec<RebaseTodoOp>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        rebase_interactive::get_interactive_plan(&path, &base_oid)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Starts an interactive rebase of the current branch onto `onto_oid`, replaying
/// `todos` in order (P23 contract §7). Continue/Skip/Abort reuse the existing
/// `rebase_{continue,skip,abort}` commands via the core delegation. Errors:
/// `operationInProgress` | `checkoutConflict` | `configMissing` | `git` |
/// `noRepo`. Does NOT emit `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn start_interactive_rebase(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    onto_oid: String,
    todos: Vec<RebaseTodoOp>,
) -> Result<RebaseOutcome, AppError> {
    start_interactive_rebase_inner(state.inner(), &repo_id, onto_oid, todos).await
}

/// Runtime-free core of `start_interactive_rebase` (unit-testable without a Tauri app).
pub(crate) async fn start_interactive_rebase_inner(
    state: &AppState,
    repo_id: &str,
    onto_oid: String,
    todos: Vec<RebaseTodoOp>,
) -> Result<RebaseOutcome, AppError> {
    let subject = arg_target(state, TargetArg::Commit(&onto_oid));
    logged_blocking(
        state,
        repo_id,
        GitActivityCategory::InteractiveRebase,
        subject,
        LoggedPhase::Local,
        rebase_outcome,
        move |path| rebase_interactive::start_interactive_rebase(&path, &onto_oid, todos),
    )
    .await
}
