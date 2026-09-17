//! `diff` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Diff of one working-dir file (M4 contract §2.2/§2.8).
/// `staged == false`: index vs workdir; `staged == true`: HEAD vs index.
/// `orig_path`: pass `StatusEntry.origPath` for renames.
#[tauri::command]
pub async fn get_workdir_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    orig_path: Option<String>,
    staged: bool,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    get_workdir_file_diff_inner(
        state.inner(),
        &repo_id,
        path,
        orig_path,
        staged,
        full_context,
        intraline,
    )
    .await
}

/// Runtime-free core of `get_workdir_file_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_workdir_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    orig_path: Option<String>,
    staged: bool,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    // P91 §3.1.2/§3.1.3: queue delay + pool saturation for the `diff.compute` span.
    let queued_at = std::time::Instant::now();
    tauri::async_runtime::spawn_blocking(move || {
        let pool = crate::obs::phase::PoolGuard::enter();
        let queued_ms = queued_at.elapsed().as_millis().min(u32::MAX as u128) as u32;
        let mut recorder =
            crate::obs::phase::PhaseRecorder::start(crate::obs::phase::OP_DIFF_COMPUTE);
        recorder.note_queue(queued_ms, pool.inflight(), pool.max());
        // `workdir_file_diff` is one core call (bonsai-core owns no `obs`
        // dependency, §3.1.2), so the whole diff is one `hunks` phase.
        let res = {
            let _p = recorder.phase("hunks");
            workdir_file_diff(
                &workdir,
                &path,
                orig_path.as_deref(),
                staged,
                full_context,
                intraline,
            )
        };
        let outcome = if res.is_ok() {
            crate::obs::phase::SpanOutcome::Ok
        } else {
            crate::obs::phase::SpanOutcome::Err
        };
        recorder.finish(&crate::obs::TraceMeta::root("backend"), outcome);
        res
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Commit details + per-file headers for `oid` vs its first parent
/// (M4 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_commit_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CommitDiff, AppError> {
    get_commit_diff_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `get_commit_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_commit_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CommitDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || commit_diff(&workdir, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for ONE file of a commit's first-parent diff (M4 contract §2.2/§2.8).
#[tauri::command]
pub async fn get_commit_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    get_commit_file_diff_inner(
        state.inner(),
        &repo_id,
        oid,
        path,
        orig_path,
        full_context,
        intraline,
    )
    .await
}

/// Runtime-free core of `get_commit_file_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_commit_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        commit_file_diff(
            &workdir,
            &oid,
            &path,
            orig_path.as_deref(),
            full_context,
            intraline,
        )
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// HEAD → `oid` tree comparison (P5 §1.2). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn compare_with_head(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CompareDiff, AppError> {
    compare_with_head_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `compare_with_head` (unit-testable without a Tauri app).
pub(crate) async fn compare_with_head_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CompareDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || compare_head_diff(&workdir, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Hunks for one file of the HEAD → `oid` comparison. Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn compare_with_head_file_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    compare_with_head_file_diff_inner(
        state.inner(),
        &repo_id,
        oid,
        path,
        orig_path,
        full_context,
        intraline,
    )
    .await
}

/// Runtime-free core of `compare_with_head_file_diff`.
pub(crate) async fn compare_with_head_file_diff_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
    path: String,
    orig_path: Option<String>,
    full_context: bool,
    intraline: bool,
) -> Result<FileDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        compare_head_file_diff(
            &workdir,
            &oid,
            &path,
            orig_path.as_deref(),
            full_context,
            intraline,
        )
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Both sides of an image comparison as base64 (P61b §D2). Read-only; emits no
/// `repo-changed`. Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_image_diff(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    request: ImageDiffRequest,
) -> Result<ImageDiff, AppError> {
    get_image_diff_inner(state.inner(), &repo_id, request).await
}

/// Runtime-free core of `get_image_diff` (unit-testable without a Tauri app).
pub(crate) async fn get_image_diff_inner(
    state: &AppState,
    repo_id: &str,
    request: ImageDiffRequest,
) -> Result<ImageDiff, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || image_diff::get_image_diff(&workdir, &request))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
