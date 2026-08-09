//! `config` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// Read the config view for `level` ("local" | "global") of `repo_id`: curated
/// keys (effective value + level + target-level value) + advanced entries at the
/// target level. Read-only (P40 §5.1). Errors: `git` | `noRepo`. Does NOT emit
/// `repo-changed`.
#[tauri::command]
pub async fn get_config(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    level: ConfigLevelArg,
) -> Result<ConfigView, AppError> {
    get_config_inner(state.inner(), &repo_id, level).await
}

pub(crate) async fn get_config_inner(
    state: &AppState,
    repo_id: &str,
    level: ConfigLevelArg,
) -> Result<ConfigView, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || config::read_config(&workdir, level))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Write `value` to `key` at `level` of `repo_id`. Validated server-side (key
/// shape, enum value). Errors: `invalidName` | `git` | `noRepo`. Does NOT emit
/// `repo-changed` (the Settings section re-fetches — P40 §5.1).
#[tauri::command]
pub async fn set_config(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    level: ConfigLevelArg,
    key: String,
    value: String,
) -> Result<(), AppError> {
    set_config_inner(state.inner(), &repo_id, level, key, value).await
}

pub(crate) async fn set_config_inner(
    state: &AppState,
    repo_id: &str,
    level: ConfigLevelArg,
    key: String,
    value: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || config::set_config(&workdir, level, &key, &value))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Remove `key` at `level` of `repo_id` (idempotent). Errors: `invalidName` |
/// `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn unset_config(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    level: ConfigLevelArg,
    key: String,
) -> Result<(), AppError> {
    unset_config_inner(state.inner(), &repo_id, level, key).await
}

pub(crate) async fn unset_config_inner(
    state: &AppState,
    repo_id: &str,
    level: ConfigLevelArg,
    key: String,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || config::unset_config(&workdir, level, &key))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Apply an identity to `repo_id`'s LOCAL git config (P44): writes user.name +
/// user.email + (if set) user.signingkey, returns the refreshed Local
/// `ConfigView`. The identity fields are passed by the caller (App's live
/// in-memory profile state), NOT re-resolved from persisted settings — profile
/// CRUD only persists after a 300 ms debounce, so a read-by-id here would race
/// (edit-then-Apply within the window would apply the STALE persisted identity,
/// and a freshly-added profile would not be found). Does NOT emit `repo-changed`
/// (mirrors `set_config`; identity does not change tree/graph). Errors: `noRepo`
/// (unknown repo) | `invalidName` | `git` (write failure).
#[tauri::command]
pub async fn apply_identity_profile(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    user_name: String,
    user_email: String,
    signing_key: Option<String>,
) -> Result<ConfigView, AppError> {
    apply_identity_profile_inner(state.inner(), &repo_id, user_name, user_email, signing_key)
        .await
}

/// Runtime-free core of `apply_identity_profile` (unit-testable without a
/// Tauri app).
pub(crate) async fn apply_identity_profile_inner(
    state: &AppState,
    repo_id: &str,
    user_name: String,
    user_email: String,
    signing_key: Option<String>,
) -> Result<ConfigView, AppError> {
    let workdir = repo_path(state, repo_id)?; // NoRepo if unknown
    tauri::async_runtime::spawn_blocking(move || {
        config::apply_identity_profile(
            &workdir,
            &user_name,
            &user_email,
            signing_key.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
