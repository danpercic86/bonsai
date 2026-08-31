//! `branches` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// One snapshot of local branches + remote-tracking branches + tags + HEAD
/// (M5 contract §2.2/§2.8). Errors: `noRepo` | `git`.
#[tauri::command]
pub async fn list_branches(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<BranchesSnapshot, AppError> {
    list_branches_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `list_branches` (unit-testable without a Tauri app).
pub(crate) async fn list_branches_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<BranchesSnapshot, AppError> {
    // P88b/B2b: route through the per-thread handle cache. Refs/tags/HEAD are
    // re-read on demand, so a reused handle reads current on-disk state.
    let (path, generation) = repo_path_and_gen(state, repo_id)?;
    let perf = state.perf.clone();
    let repo_id = repo_id.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        crate::repo_handle::with_repo(&repo_id, generation, &path, &perf, |repo| {
            branches::list_refs_with(repo)
        })
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates a local branch at the current HEAD commit — does NOT check out
/// (M5 contract §2.4). Errors: `invalidName` | `branchExists` | `git` | `noRepo`.
/// Does NOT emit `repo-changed` — the frontend refetches imperatively.
#[tauri::command]
pub async fn create_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    create_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `create_branch` (unit-testable without a Tauri app).
pub(crate) async fn create_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::create_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Creates local branch `name` at commit `oid`, auto-stashing/re-applying
/// uncommitted work across the checkout (P11 §1). Errors: `invalidName` |
/// `branchExists` | `operationInProgress` | `configMissing` | `checkoutConflict`
/// | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn create_branch_here(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    oid: String,
) -> Result<CreateBranchHereResult, AppError> {
    create_branch_here_inner(state.inner(), &repo_id, name, oid).await
}

/// Runtime-free core of `create_branch_here` (unit-testable without a Tauri app).
pub(crate) async fn create_branch_here_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    oid: String,
) -> Result<CreateBranchHereResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::create_branch_here(&path, &name, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Dirty-safe checkout of a LOCAL branch (P33): auto-stash -> switch -> auto FF
/// to upstream (no fetch) -> re-apply stash. A conflicted re-apply is a SUCCESS
/// carrying `apply: Some(conflicts)` (stash retained). Errors: `branchNotFound`
/// | `operationInProgress` | `configMissing` | `checkoutConflict` | `git` |
/// `noRepo`. Does NOT emit `repo-changed` (frontend calls refreshAll).
#[tauri::command]
pub async fn checkout_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<CheckoutResult, AppError> {
    checkout_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `checkout_branch` (unit-testable without a Tauri app).
pub(crate) async fn checkout_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<CheckoutResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_branch_autostash(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Dirty-safe checkout of an arbitrary commit → DETACHED HEAD: auto-stash ->
/// safe checkout -> set_head_detached -> re-apply stash. No auto-FF. A conflicted
/// re-apply is a SUCCESS carrying `apply: Some(conflicts)` (stash retained).
/// Errors: `invalidName` | `git` | `operationInProgress` | `configMissing` |
/// `checkoutConflict` | `noRepo`. Does NOT emit `repo-changed` (frontend calls
/// refreshAll).
#[tauri::command]
pub async fn checkout_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CheckoutResult, AppError> {
    checkout_commit_inner(state.inner(), &repo_id, oid).await
}

/// Runtime-free core of `checkout_commit` (unit-testable without a Tauri app).
pub(crate) async fn checkout_commit_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CheckoutResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_commit_detached(&path, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes a LOCAL, fully merged, non-current branch (M5 contract §2.6 —
/// unmerged deletion is blocked; no force-delete in v1).
/// Errors: `branchNotFound` | `unmergedBranch` | `git` | `noRepo`.
#[tauri::command]
pub async fn delete_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    delete_branch_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_branch` (unit-testable without a Tauri app).
pub(crate) async fn delete_branch_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::delete_branch(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Renames LOCAL branch `old_name` → `new_name` (git `branch -m`, non-force,
/// P60a). Preserves upstream + reflog; rewrites HEAD when `old_name` is the
/// checked-out branch. Errors: `invalidName` | `branchNotFound` | `branchExists`
/// | `git` | `noRepo`. Does NOT emit `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn rename_branch(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    old_name: String,
    new_name: String,
) -> Result<RenameBranchResult, AppError> {
    rename_branch_inner(state.inner(), &repo_id, old_name, new_name).await
}

/// Runtime-free core of `rename_branch` (unit-testable without a Tauri app).
pub(crate) async fn rename_branch_inner(
    state: &AppState,
    repo_id: &str,
    old_name: String,
    new_name: String,
) -> Result<RenameBranchResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        branches::rename_branch(&path, &old_name, &new_name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// GitKraken-style remote checkout: create/reuse a local tracking branch for
/// `name` ("<remote>/<branch>") and safe-checkout it (P6 §2.2).
/// Errors: `invalidName` | `branchNotFound` | `checkoutConflict` | `git` | `noRepo`.
#[tauri::command]
pub async fn checkout_remote(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    checkout_remote_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `checkout_remote` (unit-testable without a Tauri app).
pub(crate) async fn checkout_remote_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_remote(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes the LOCAL remote-tracking ref `name` — does NOT touch the server
/// (P6 §2.3). Errors: `branchNotFound` | `git` | `noRepo`.
#[tauri::command]
pub async fn delete_remote_tracking(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    delete_remote_tracking_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_remote_tracking` (unit-testable without a Tauri app).
pub(crate) async fn delete_remote_tracking_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::delete_remote_tracking(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Classifies local branches safe to delete (merged into `base` OR
/// upstream-gone) — read-only, touches nothing (P25 §4.1). `base` auto-resolves
/// when omitted. Pure git; NO consent gate. Errors: `git` | `noRepo`.
#[tauri::command]
pub async fn list_stale_branches(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    base: Option<String>,
) -> Result<StaleReport, AppError> {
    list_stale_branches_inner(state.inner(), &repo_id, base).await
}

/// Runtime-free core of `list_stale_branches` (unit-testable without a Tauri app).
pub(crate) async fn list_stale_branches_inner(
    state: &AppState,
    repo_id: &str,
    base: Option<String>,
) -> Result<StaleReport, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stale::find_stale_branches(&path, base.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Batch-deletes the caller-supplied branch names that are STILL safe against a
/// freshly-recomputed stale set — refusing the current branch, the base, and
/// anything not re-verified as stale (P25 §4.3). Per-branch outcomes are DATA,
/// never thrown; a partial batch returns `Ok(results)`. Pure git; NO consent
/// gate. Does NOT emit `repo-changed` — the frontend refetches imperatively.
/// Errors (whole-call): `git` (bad base) | `noRepo`.
#[tauri::command]
pub async fn delete_branches(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    names: Vec<String>,
    base: Option<String>,
) -> Result<Vec<BranchDeleteResult>, AppError> {
    delete_branches_inner(state.inner(), &repo_id, names, base).await
}

/// Runtime-free core of `delete_branches` (unit-testable without a Tauri app).
pub(crate) async fn delete_branches_inner(
    state: &AppState,
    repo_id: &str,
    names: Vec<String>,
    base: Option<String>,
) -> Result<Vec<BranchDeleteResult>, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        stale::delete_branches(&path, &names, base.as_deref())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
