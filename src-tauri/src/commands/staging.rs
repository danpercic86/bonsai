//! `staging` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Stages the given worktree-relative paths (atomic batch, M3 contract §2.2).
/// Does NOT emit `repo-changed` — the frontend refetches imperatively after
/// every successful mutation (§2.7).
#[tauri::command]
pub async fn stage(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    stage_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `stage` (unit-testable without a Tauri app).
pub(crate) async fn stage_inner(state: &AppState, repo_id: &str, paths: Vec<String>) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || stage_paths(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unstages the given worktree-relative paths (atomic batch). Safe: the
/// worktree is never touched.
#[tauri::command]
pub async fn unstage(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    unstage_inner(state.inner(), &repo_id, paths).await
}

/// Runtime-free core of `unstage` (unit-testable without a Tauri app).
pub(crate) async fn unstage_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || unstage_paths(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a commit from the current index. `sign` (P58): `null`/absent ⇒ follow
/// `commit.gpgsign`; `true` ⇒ force sign; `false` ⇒ force unsigned. `skipHooks`
/// (P59a): `true` ≡ `--no-verify`; `null`/absent/`false` ⇒ run hooks per
/// `bonsai.runHooks` (default true). Errors: `emptyMessage` | `configMissing` |
/// `nothingToCommit` | `hookRejected` | `git` | `noRepo`.
#[tauri::command]
pub async fn commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
    sign: Option<bool>,
    skip_hooks: Option<bool>,
) -> Result<CommitResult, AppError> {
    commit_inner(state.inner(), &repo_id, message, sign, skip_hooks).await
}

/// Runtime-free core of `commit` (unit-testable without a Tauri app). P87:
/// wrapped in `with_activity` (category `Commit`) — pre-commit/commit-msg
/// `RunningHook` phases, `Finalizing`, post-commit; a no-op when nobody is
/// subscribed.
pub(crate) async fn commit_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
    sign: Option<bool>,
    skip_hooks: Option<bool>,
) -> Result<CommitResult, AppError> {
    with_activity(state.git_activity_hub(), GitActivityCategory::Commit, move |emitter| async move {
        let path = repo_path(state, repo_id)?;
        let skip = skip_hooks.unwrap_or(false);
        tauri::async_runtime::spawn_blocking(move || {
            let rec: Option<&dyn GitActivityRecorder> =
                emitter.as_deref().map(|e| e as &dyn GitActivityRecorder);
            create_commit_with_activity(&path, &message, sign, skip, rec)
        })
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
    })
    .await
}

/// Stages only the selected changed lines of one working-dir file (index moves
/// toward the workdir; P17 §2.7). Empty selection is a no-op. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
/// Errors: `noRepo` | `git` | `other` (stale/unsupported/invalid path).
#[tauri::command]
pub async fn stage_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    stage_partial_inner(state.inner(), &repo_id, path, orig_path, selection).await
}

/// Runtime-free core of `stage_partial` (unit-testable without a Tauri app).
pub(crate) async fn stage_partial_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stage_partial_core(&workdir, &path, orig_path.as_deref(), &selection)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Unstages only the selected changed lines of one staged file (index moves
/// toward HEAD; P17 §2.7). Empty selection is a no-op. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
/// Errors: `noRepo` | `git` | `other` (stale/unsupported/invalid path).
#[tauri::command]
pub async fn unstage_partial(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    unstage_partial_inner(state.inner(), &repo_id, path, orig_path, selection).await
}

/// Runtime-free core of `unstage_partial` (unit-testable without a Tauri app).
pub(crate) async fn unstage_partial_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    selection: Vec<LineSelection>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        unstage_partial_core(&workdir, &path, orig_path.as_deref(), &selection)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Amends the current HEAD commit with a new message + the current index
/// (P20 contract §2). Preserves HEAD's parents + original author. `sign` (P58):
/// as `commit`. `skipHooks` (P59a): as `commit`. Errors: `operationInProgress` |
/// `git` | `emptyMessage` | `configMissing` | `hookRejected` | `noRepo`. Does NOT
/// emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn commit_amend(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
    sign: Option<bool>,
    skip_hooks: Option<bool>,
) -> Result<CommitResult, AppError> {
    commit_amend_inner(state.inner(), &repo_id, message, sign, skip_hooks).await
}

/// Runtime-free core of `commit_amend` (unit-testable without a Tauri app). P87:
/// wrapped in `with_activity` (category `Amend`); a no-op when nobody is
/// subscribed.
pub(crate) async fn commit_amend_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
    sign: Option<bool>,
    skip_hooks: Option<bool>,
) -> Result<CommitResult, AppError> {
    with_activity(state.git_activity_hub(), GitActivityCategory::Amend, move |emitter| async move {
        let path = repo_path(state, repo_id)?;
        let skip = skip_hooks.unwrap_or(false);
        tauri::async_runtime::spawn_blocking(move || {
            let rec: Option<&dyn GitActivityRecorder> =
                emitter.as_deref().map(|e| e as &dyn GitActivityRecorder);
            amend_commit_with_activity(&path, &message, sign, skip, rec)
        })
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
    })
    .await
}
