//! `forge_*` commands — provider-abstracted forge / PR integration (P62b).
//!
//! Thin wiring over the pure `bonsai-forge` crate. Each command mirrors the
//! `compose` house shape EXACTLY: `repo_path(state, repo_id)?` resolves the
//! workdir, then a `spawn_blocking` runs the blocking git2/HTTP work off the UI
//! thread, and a join failure maps to `AppError::Other`. Forge commands are NOT
//! AI-gated and do NOT emit `repo-changed` (`create_pr` mutates the remote, not
//! the local repo — the panel refetches on demand).
//!
//! Auth (`forge_set_token` / `forge_clear_token`) goes through the crate-level
//! `bonsai_forge::{set_token, clear_token}` entry points (the read-only `open()`
//! cannot store): a pasted PAT is validated via `GET /user`, then stored in the
//! OS keychain keyed by host. The token is NEVER logged, NEVER placed in a URL,
//! and NEVER returned to the frontend (only the public viewer identity is).

use super::shared::*;

/// Repo identity from `origin` + keychain presence (NO network). An
/// unrecognized/unparseable origin yields a friendly `Unknown` context rather
/// than an error. Errors: `noRepo` | `noRemote` | `git` | `other`.
#[tauri::command]
pub async fn forge_repo_context(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<ForgeRepoContext, AppError> {
    forge_repo_context_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `forge_repo_context` (unit-testable without a Tauri app).
pub(crate) async fn forge_repo_context_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<ForgeRepoContext, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || Ok(bonsai_forge::open(&workdir)?.repo_context()))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One page of PR summaries for the requested state filter (`per_page` capped
/// at 50 by the provider). Errors: `noRepo` | `forgeUnsupported` | `noRemote` |
/// `forgeRateLimited` | `forgeApi` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_list_prs(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    query: PrListQuery,
) -> Result<PrPage, AppError> {
    forge_list_prs_inner(state.inner(), &repo_id, query).await
}

/// Runtime-free core of `forge_list_prs`.
pub(crate) async fn forge_list_prs_inner(
    state: &AppState,
    repo_id: &str,
    query: PrListQuery,
) -> Result<PrPage, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::open(&workdir)?.list_prs(&query))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// A single PR with body, diff stats, mergeability, and labels. Errors:
/// `noRepo` | `forgeUnsupported` | `noRemote` | `forgeApi` | `forgeRateLimited`
/// | `networkError` | `git`.
#[tauri::command]
pub async fn forge_get_pr(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    number: u64,
) -> Result<PrDetail, AppError> {
    forge_get_pr_inner(state.inner(), &repo_id, number).await
}

/// Runtime-free core of `forge_get_pr`.
pub(crate) async fn forge_get_pr_inner(
    state: &AppState,
    repo_id: &str,
    number: u64,
) -> Result<PrDetail, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::open(&workdir)?.get_pr(number))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Open a new PR from the given input; REQUIRES a stored token. Errors:
/// `noRepo` | `forgeAuthRequired` | `forgeUnsupported` | `noRemote` |
/// `forgeApi` | `forgeRateLimited` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_create_pr(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    input: CreatePrInput,
) -> Result<PrDetail, AppError> {
    forge_create_pr_inner(state.inner(), &repo_id, input).await
}

/// Runtime-free core of `forge_create_pr`.
pub(crate) async fn forge_create_pr_inner(
    state: &AppState,
    repo_id: &str,
    input: CreatePrInput,
) -> Result<PrDetail, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::open(&workdir)?.create_pr(&input))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Merged review (diff-line) + conversation comments for a PR, sorted by
/// creation time. Errors: `noRepo` | `forgeUnsupported` | `noRemote` |
/// `forgeApi` | `forgeRateLimited` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_list_review_comments(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    number: u64,
) -> Result<Vec<ReviewComment>, AppError> {
    forge_list_review_comments_inner(state.inner(), &repo_id, number).await
}

/// Runtime-free core of `forge_list_review_comments`.
pub(crate) async fn forge_list_review_comments_inner(
    state: &AppState,
    repo_id: &str,
    number: u64,
) -> Result<Vec<ReviewComment>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        bonsai_forge::open(&workdir)?.list_review_comments(number)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Validate a pasted PAT (`GET /user`) and, on success, store it in the OS
/// keychain keyed by the origin's host; returns the authenticated viewer. A
/// rejected token stores NOTHING. The token is never logged, never placed in a
/// URL, and never echoed back. Errors: `noRepo` | `authFailed` |
/// `forgeUnsupported` | `noRemote` | `forgeRateLimited` | `networkError` |
/// `git` | `other`.
#[tauri::command]
pub async fn forge_set_token(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    token: String,
) -> Result<ForgeViewer, AppError> {
    forge_set_token_inner(state.inner(), &repo_id, token).await
}

/// Runtime-free core of `forge_set_token`.
pub(crate) async fn forge_set_token_inner(
    state: &AppState,
    repo_id: &str,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::set_token(&workdir, &token))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Sign out: delete the origin host's PAT from the keychain and evict the
/// cached viewer. Idempotent. Errors: `noRepo` | `noRemote` | `git` | `other`.
#[tauri::command]
pub async fn forge_clear_token(
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    forge_clear_token_inner(state.inner(), &repo_id).await
}

/// Runtime-free core of `forge_clear_token`.
pub(crate) async fn forge_clear_token_inner(
    state: &AppState,
    repo_id: &str,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::clear_token(&workdir))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
