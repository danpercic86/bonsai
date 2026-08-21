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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<ForgeRepoContext, AppError> {
    let file = settings::settings_file(&app)?;
    forge_repo_context_inner(state.inner(), &file, &repo_id).await
}

/// Runtime-free core of `forge_repo_context` (unit-testable without a Tauri app).
///
/// P80: resolves the repo's account (per-repo override → owner match → host
/// default → single → first) and opens the provider with THAT account's keychain
/// key, so `authenticated`/`viewer` reflect the resolved account. Surfaces the
/// resolved `accountId` + `accountSource`.
pub(crate) async fn forge_repo_context_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
) -> Result<ForgeRepoContext, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let resolved = crate::commands::resolve_forge_blocking(&workdir, &file)?;
        let mut ctx =
            bonsai_forge::open_with_key(&workdir, resolved.keychain_key.as_deref())?.repo_context();
        ctx.resolved_account_id = resolved.account_id;
        ctx.account_source = resolved.source;
        Ok(ctx)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// One page of PR summaries for the requested state filter (`per_page` capped
/// at 50 by the provider). Errors: `noRepo` | `forgeUnsupported` | `noRemote` |
/// `forgeRateLimited` | `forgeApi` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_list_prs(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    query: PrListQuery,
) -> Result<PrPage, AppError> {
    let file = settings::settings_file(&app)?;
    forge_list_prs_inner(state.inner(), &file, &repo_id, query).await
}

/// Runtime-free core of `forge_list_prs`.
pub(crate) async fn forge_list_prs_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    query: PrListQuery,
) -> Result<PrPage, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let key = crate::commands::resolved_key(&workdir, &file)?;
        bonsai_forge::open_with_key(&workdir, key.as_deref())?.list_prs(&query)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// A single PR with body, diff stats, mergeability, and labels. Errors:
/// `noRepo` | `forgeUnsupported` | `noRemote` | `forgeApi` | `forgeRateLimited`
/// | `networkError` | `git`.
#[tauri::command]
pub async fn forge_get_pr(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    number: u64,
) -> Result<PrDetail, AppError> {
    let file = settings::settings_file(&app)?;
    forge_get_pr_inner(state.inner(), &file, &repo_id, number).await
}

/// Runtime-free core of `forge_get_pr`.
pub(crate) async fn forge_get_pr_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    number: u64,
) -> Result<PrDetail, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let key = crate::commands::resolved_key(&workdir, &file)?;
        bonsai_forge::open_with_key(&workdir, key.as_deref())?.get_pr(number)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Open a new PR from the given input; REQUIRES a stored token. Errors:
/// `noRepo` | `forgeAuthRequired` | `forgeUnsupported` | `noRemote` |
/// `forgeApi` | `forgeRateLimited` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_create_pr(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    input: CreatePrInput,
) -> Result<PrDetail, AppError> {
    let file = settings::settings_file(&app)?;
    forge_create_pr_inner(state.inner(), &file, &repo_id, input).await
}

/// Runtime-free core of `forge_create_pr`.
pub(crate) async fn forge_create_pr_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    input: CreatePrInput,
) -> Result<PrDetail, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let key = crate::commands::resolved_key(&workdir, &file)?;
        bonsai_forge::open_with_key(&workdir, key.as_deref())?.create_pr(&input)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Merged review (diff-line) + conversation comments for a PR, sorted by
/// creation time. Errors: `noRepo` | `forgeUnsupported` | `noRemote` |
/// `forgeApi` | `forgeRateLimited` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_list_review_comments(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    number: u64,
) -> Result<Vec<ReviewComment>, AppError> {
    let file = settings::settings_file(&app)?;
    forge_list_review_comments_inner(state.inner(), &file, &repo_id, number).await
}

/// Runtime-free core of `forge_list_review_comments`.
pub(crate) async fn forge_list_review_comments_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    number: u64,
) -> Result<Vec<ReviewComment>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let key = crate::commands::resolved_key(&workdir, &file)?;
        bonsai_forge::open_with_key(&workdir, key.as_deref())?.list_review_comments(number)
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
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let file = settings::settings_file(&app)?;
    forge_set_token_inner(state.inner(), &file, &repo_id, token).await
}

/// Runtime-free core of `forge_set_token`.
///
/// P80 (OD-3): validate the pasted PAT for the origin host, learn the login,
/// store the token under a three-part keychain key, upsert the account (setting
/// it as the host default if none exists), AND pin it as this repo's override so
/// the newly-connected account is what the repo uses. The legacy known-hosts
/// index is kept mirrored (OD-5). Done inside the SAME `spawn_blocking`.
pub(crate) async fn forge_set_token_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let (viewer, host, kind) = bonsai_forge::validate_repo_token(&workdir, &token)?;
        if host.is_empty() {
            // Unparseable origin: no host to key an account by; token can't be
            // stored under an account. Return the validated viewer unchanged.
            return Ok(viewer);
        }
        let login = viewer.login.clone();
        let aid = settings::account_id(kind, &host, Some(&login));
        // Store ONLY after successful validation (never persist a rejected token).
        bonsai_forge::store_token(&aid, &token)?;
        let workdir_str = workdir.to_string_lossy().to_string();
        let rec = settings::ForgeAccountRecord {
            account_id: aid.clone(),
            keychain_key: aid.clone(),
            host: host.clone(),
            kind,
            login: Some(login.clone()),
            avatar_url: viewer.avatar_url.clone(),
        };
        let _ = settings::update(&file, |s| {
            settings::upsert_forge_account(s, rec.clone());
            if !s.forge_host_defaults.iter().any(|d| d.host == host) {
                settings::set_host_default(s, &host, &aid);
            }
            settings::set_repo_override(s, &workdir_str, &aid);
            // OD-5: keep the legacy known-hosts index mirrored for one release.
            settings::upsert_forge_host(s, &host, kind, Some(login.clone()));
        });
        Ok(viewer)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P80 (OD-2): clear this repo's account OVERRIDE only — the repo falls back to
/// inheriting (owner match → host default). The account itself stays connected
/// (deletion is `forge_remove_account`). Idempotent. Errors: `noRepo` | `other`.
#[tauri::command]
pub async fn forge_clear_token(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    forge_clear_token_inner(state.inner(), &file, &repo_id).await
}

/// Runtime-free core of `forge_clear_token`.
pub(crate) async fn forge_clear_token_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let workdir_str = workdir.to_string_lossy().to_string();
        let _ = settings::update(&file, |s| settings::clear_repo_override(s, &workdir_str));
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Batch commit/CI statuses (P63): one [`CommitStatus`] per requested sha, in
/// the SAME order (nothing skipped). Runs the whole batch of combined-status
/// lookups inside ONE `spawn_blocking`, mirroring `verify_commits`. Errors:
/// `noRepo` | `forgeUnsupported` | `noRemote` | `forgeApi` | `forgeRateLimited`
/// | `authFailed` | `networkError` | `git`.
#[tauri::command]
pub async fn forge_commit_statuses(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    shas: Vec<String>,
) -> Result<Vec<CommitStatus>, AppError> {
    let file = settings::settings_file(&app)?;
    forge_commit_statuses_inner(state.inner(), &file, &repo_id, shas).await
}

/// Runtime-free core of `forge_commit_statuses`.
pub(crate) async fn forge_commit_statuses_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    shas: Vec<String>,
) -> Result<Vec<CommitStatus>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let key = crate::commands::resolved_key(&workdir, &file)?;
        bonsai_forge::open_with_key(&workdir, key.as_deref())?.commit_statuses(&shas)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
