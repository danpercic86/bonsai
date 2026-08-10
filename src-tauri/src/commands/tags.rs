//! `tags` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Creates a tag at `target_oid` (P22 contract §2.2). `message: Some(_)` →
/// annotated (needs a git identity); `message: None` → lightweight. `force`
/// overwrites an existing tag (the v1 UI passes `false`). `sign` (F-A7-8, optional
/// / wire-compatible with the frozen `types.ts`): absent/`false` ⇒ config-driven
/// (git `tag.gpgSign` still signs an annotated tag); `true` ⇒ force-sign the
/// annotated tag. Ignored for lightweight tags. Errors:
/// `noRepo` | `invalidName` | `configMissing` | `git`. Does NOT emit
/// `repo-changed` — the frontend refetches.
#[tauri::command]
pub async fn create_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    target_oid: String,
    message: Option<String>,
    force: bool,
    sign: Option<bool>,
) -> Result<(), AppError> {
    create_tag_inner(state.inner(), &repo_id, name, target_oid, message, force, sign).await
}

/// Runtime-free core of `create_tag` (unit-testable without a Tauri app).
pub(crate) async fn create_tag_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
    target_oid: String,
    message: Option<String>,
    force: bool,
    sign: Option<bool>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    // Absent `sign` ⇒ config-driven (false): core still honours `tag.gpgSign`.
    let sign = sign.unwrap_or(false);
    tauri::async_runtime::spawn_blocking(move || {
        tags::create_tag(&path, &name, &target_oid, message, force, sign)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Deletes a LOCAL tag (P22 contract §2.3). Does NOT contact any remote.
/// Errors: `noRepo` | `invalidName` | `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn delete_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    delete_tag_inner(state.inner(), &repo_id, name).await
}

/// Runtime-free core of `delete_tag` (unit-testable without a Tauri app).
pub(crate) async fn delete_tag_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || tags::delete_tag(&path, &name))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Pushes `refs/tags/<tag_name>` to `remote` over the M6 credential path
/// (P22 contract §2.4). `force` is `false` in the v1 UI. Errors:
/// `noRepo` | `noRemote` | `authFailed` | `networkError` | `pushRejected` |
/// `git`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn push_tag(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    remote: String,
    tag_name: String,
    force: bool,
) -> Result<(), AppError> {
    push_tag_inner(state.inner(), &repo_id, remote, tag_name, force).await
}

/// Runtime-free core of `push_tag` (unit-testable without a Tauri app).
pub(crate) async fn push_tag_inner(
    state: &AppState,
    repo_id: &str,
    remote: String,
    tag_name: String,
    force: bool,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || tags::push_tag(&path, &remote, &tag_name, force))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

// ============================================================ P22 §3 remotes
// management (list / add / remove / rename / set-url). Local-only config ops —
// none emit `repo-changed`; the frontend refetches imperatively.
