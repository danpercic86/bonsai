//! `merge` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Current operation state (merge / rebase / cherry-pick / revert / none).
/// Part of the frontend refresh batch (P3c contract §6). Errors: `noRepo`
/// | `git`.
#[tauri::command]
pub async fn get_op_state(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<RepoOpState, AppError> {
    get_op_state_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `get_op_state` (unit-testable without a Tauri app).
pub(crate) async fn get_op_state_inner(state: &AppState, repo_id: &str) -> Result<RepoOpState, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || read_op_state(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Merges a local or remote-tracking branch into the current branch (P3c
/// contract §4). `skipHooks` (F-A4-2): `true` ≡ `--no-verify` for the clean
/// auto-merge's `commit-msg` hook; absent ⇒ `false` (run hooks per
/// `bonsai.runHooks`) so existing callers are unchanged on the wire. Errors:
/// `operationInProgress` | `branchNotFound` | `checkoutConflict`
/// | `configMissing` | `hookRejected` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn merge_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    skip_hooks: Option<bool>,
) -> Result<MergeOutcome, AppError> {
    merge_branch_inner(state.inner(), &repo_id, name, skip_hooks).await
}

/// Runtime-free core of `merge_branch` (unit-testable without a Tauri app).
pub(crate) async fn merge_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    skip_hooks: Option<bool>,
) -> Result<MergeOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    let skip = skip_hooks.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || merge::merge_branch(&path, &name, skip))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Finalizes a paused merge as a 2(+)-parent commit (P3c contract §4.4).
/// `sign` (P58 OQ4): `null`/absent ⇒ follow `commit.gpgsign`; `true`/`false`
/// force. `skipHooks` (P59a): `true` ≡ `--no-verify`; else run hooks per
/// `bonsai.runHooks` (default true). Errors: `noOperationInProgress` |
/// `unresolvedConflicts` | `emptyMessage` | `configMissing` | `hookRejected` |
/// `git` | `noRepo`.
#[tauri::command]
pub async fn commit_merge(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: String,
    sign: Option<bool>,
    skip_hooks: Option<bool>,
) -> Result<CommitResult, AppError> {
    commit_merge_inner(state.inner(), &repo_id, message, sign, skip_hooks).await
}

/// Runtime-free core of `commit_merge` (unit-testable without a Tauri app).
pub(crate) async fn commit_merge_inner(
    state: &AppState,
    repo_id: &str,
    message: String,
    sign: Option<bool>,
    skip_hooks: Option<bool>,
) -> Result<CommitResult, AppError> {
    let path = repo_path(state, repo_id)?;
    let skip = skip_hooks.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || merge::commit_merge(&path, &message, sign, skip))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Aborts a paused merge (worktree-destructive for merge-touched files —
/// the UI confirms first; P3c contract §4.5). Errors: `noOperationInProgress`
/// | `git` | `noRepo`.
#[tauri::command]
pub async fn abort_merge(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    abort_merge_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `abort_merge` (unit-testable without a Tauri app).
pub(crate) async fn abort_merge_inner(state: &AppState, repo_id: &str) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || merge::abort_merge(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// All current index conflicts, path-ascending (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_conflicts(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<Vec<ConflictEntry>, AppError> {
    list_conflicts_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_conflicts` (unit-testable without a Tauri app).
pub(crate) async fn list_conflicts_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<Vec<ConflictEntry>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || conflict::list_conflicts(&path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Read-only marker view of one conflicted file (P3c contract §3).
/// Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn get_conflict(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<ConflictFile, AppError> {
    get_conflict_inner(state.inner(), &repo_id, path).await
}

/// Runtime-free core of `get_conflict` (unit-testable without a Tauri app).
pub(crate) async fn get_conflict_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
) -> Result<ConflictFile, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || conflict::get_conflict(&workdir, &path))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Resolves one conflicted path per the P3c contract §3.2 matrix.
/// Errors: `noRepo` | `git` | `invalidName` (validate_rel_path).
#[tauri::command]
pub async fn resolve_conflict(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    resolve_conflict_inner(state.inner(), &repo_id, path, resolution).await
}

/// Runtime-free core of `resolve_conflict` (unit-testable without a Tauri app).
pub(crate) async fn resolve_conflict_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    resolution: ConflictResolution,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict(&workdir, &path, resolution)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stages user-authored resolved text for one conflicted path (P12 §1.2).
/// Errors: `noRepo` | `git` | `invalidName`. Does NOT emit `repo-changed` —
/// the frontend refetches imperatively.
#[tauri::command]
pub async fn resolve_conflict_text(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    content: String,
) -> Result<(), AppError> {
    resolve_conflict_text_inner(state.inner(), &repo_id, path, content).await
}

/// Runtime-free core of `resolve_conflict_text` (unit-testable without a Tauri app).
pub(crate) async fn resolve_conflict_text_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        conflict::resolve_conflict_text(&workdir, &path, &content)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Stages an AI-proposed resolution, GATED server-side by the novel-content check
/// (P68 #7 / H1). This is the AUTHORITATIVE layer: it re-reads the conflict sides
/// from the still-conflicted index and refuses a body carrying a line present in no
/// version — it never trusts a frontend-passed flag. A clean body funnels through
/// the SAME `conflict::resolve_conflict_text` writer as the manual editor (D4:
/// exactly one write body, so the marker gate + symlink guard are unchanged). The
/// manual ConflictEditor Save deliberately stays on the ungated `resolve_conflict_text`,
/// where novel lines are legitimate. Errors: `noRepo` | `git` | `invalidName`
/// | `aiFailed` (ineligible/binary/too-large, from `read_conflict_sides`)
/// | `aiNeedsReview`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn ai_apply_resolution(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    path: String,
    content: String,
) -> Result<(), AppError> {
    ai_apply_resolution_inner(state.inner(), &repo_id, path, content).await
}

/// Runtime-free core of `ai_apply_resolution` (unit-testable without a Tauri app).
pub(crate) async fn ai_apply_resolution_inner(
    state: &AppState,
    repo_id: &str,
    path: String,
    content: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        // Re-read the sides server-side — `read_conflict_sides` requires the path to
        // still be conflicted, so the gate always classifies against the real trio.
        let sides = ai_resolve::read_conflict_sides(&workdir, &path)?;
        if ai_resolve::resolution_is_novel(&sides, &content) {
            return Err(AppError::AiNeedsReview(format!(
                "AI introduced content not present in any version of '{path}' — opened for review"
            )));
        }
        // The SAME single core writer as the manual editor (D4).
        conflict::resolve_conflict_text(&workdir, &path, &content)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
