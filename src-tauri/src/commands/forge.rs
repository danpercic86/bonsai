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
/// P79 lazy backfill (OD-1): when the resolved host has a token in the keychain
/// but no record in the known-hosts index, add one (best-effort — a backfill
/// write failure never fails the context read).
pub(crate) async fn forge_repo_context_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
) -> Result<ForgeRepoContext, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let ctx = bonsai_forge::open(&workdir)?.repo_context();
        if ctx.authenticated && !ctx.host.is_empty() {
            let host = ctx.host.clone();
            let kind = ctx.provider;
            let login = ctx.viewer.as_ref().map(|v| v.login.clone());
            // `update_if` skips the disk write unless a record was actually
            // inserted — this read path runs on every PR-panel open.
            let _ = settings::update_if(&file, |s| {
                settings::backfill_forge_host(s, &host, kind, login)
            });
        }
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
/// P79: after the crate validates+stores the token, upsert the known-hosts index
/// record for the resolved host (host + kind + the validated login) so the
/// global Accounts list stays in sync. Done inside the SAME `spawn_blocking`.
pub(crate) async fn forge_set_token_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let viewer = bonsai_forge::set_token(&workdir, &token)?;
        // Best-effort index upsert (never fails the successful set-token).
        if let Ok((host, kind)) = bonsai_forge::resolve_forge_host(&workdir) {
            if !host.is_empty() {
                let login = Some(viewer.login.clone());
                let _ = settings::update(&file, |s| {
                    settings::upsert_forge_host(s, &host, kind, login);
                });
            }
        }
        Ok(viewer)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// Sign out: delete the origin host's PAT from the keychain and evict the
/// cached viewer. Idempotent. Errors: `noRepo` | `noRemote` | `git` | `other`.
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
///
/// P79: after the crate deletes the token, remove the resolved host from the
/// known-hosts index so the global Accounts list stays in sync.
pub(crate) async fn forge_clear_token_inner(
    state: &AppState,
    settings_file: &std::path::Path,
    repo_id: &str,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        // Resolve the host BEFORE clearing so an unparseable origin is handled
        // the same way `clear_token` already tolerates it.
        let host = bonsai_forge::resolve_forge_host(&workdir)
            .ok()
            .map(|(h, _)| h);
        bonsai_forge::clear_token(&workdir)?;
        if let Some(host) = host {
            if !host.is_empty() {
                let _ = settings::update(&file, |s| settings::remove_forge_host(s, &host));
            }
        }
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
    state: tauri::State<'_, AppState>,
    repo_id: String,
    shas: Vec<String>,
) -> Result<Vec<CommitStatus>, AppError> {
    forge_commit_statuses_inner(state.inner(), &repo_id, shas).await
}

/// Runtime-free core of `forge_commit_statuses`.
pub(crate) async fn forge_commit_statuses_inner(
    state: &AppState,
    repo_id: &str,
    shas: Vec<String>,
) -> Result<Vec<CommitStatus>, AppError> {
    let workdir = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::open(&workdir)?.commit_statuses(&shas))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

// ---------------------------------------------------------------------------
// P79: global forge account management (repo-independent).
// ---------------------------------------------------------------------------

/// P79: all forge hosts Bonsai knows a token for (the settings index), each with
/// live `connected` (keychain presence) + cache-warm viewer identity. NO
/// network. Errors: `other`.
#[tauri::command]
pub async fn forge_list_accounts(
    app: tauri::AppHandle,
) -> Result<Vec<ForgeAccount>, AppError> {
    let file = settings::settings_file(&app)?;
    forge_list_accounts_inner(&file).await
}

/// Runtime-free core of `forge_list_accounts`.
pub(crate) async fn forge_list_accounts_inner(
    settings_file: &std::path::Path,
) -> Result<Vec<ForgeAccount>, AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        let accounts = s
            .forge_hosts
            .iter()
            .map(|r| {
                let cached = bonsai_forge::auth::cached_viewer(&r.host);
                let login = r
                    .login
                    .clone()
                    .or_else(|| cached.as_ref().map(|v| v.login.clone()));
                let avatar_url = cached.as_ref().and_then(|v| v.avatar_url.clone());
                ForgeAccount {
                    host: r.host.clone(),
                    kind: r.kind,
                    login,
                    avatar_url,
                    connected: bonsai_forge::auth::global().has(&r.host),
                }
            })
            .collect();
        Ok(accounts)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P79: validate a pasted PAT against `host`/`kind` directly (no repo needed)
/// and, on success, store it + upsert the known-hosts index. Returns the viewer.
/// Errors: `authFailed` | `forgeUnsupported` | `forgeRateLimited` |
/// `networkError` | `other`.
#[tauri::command]
pub async fn forge_set_token_for_host(
    app: tauri::AppHandle,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let file = settings::settings_file(&app)?;
    forge_set_token_for_host_inner(&file, host, kind, token).await
}

/// Runtime-free core of `forge_set_token_for_host`.
pub(crate) async fn forge_set_token_for_host_inner(
    settings_file: &std::path::Path,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let viewer = bonsai_forge::set_token_for_host(&host, kind, &token)?;
        // Best-effort index upsert (never fails a successful set-token).
        let login = Some(viewer.login.clone());
        let _ = settings::update(&file, |s| {
            settings::upsert_forge_host(s, &host, kind, login);
        });
        Ok(viewer)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P79: sign out a host globally — delete its PAT, evict its viewer, remove it
/// from the known-hosts index. Idempotent. Errors: `other`.
#[tauri::command]
pub async fn forge_clear_token_for_host(
    app: tauri::AppHandle,
    host: String,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    forge_clear_token_for_host_inner(&file, host).await
}

/// Runtime-free core of `forge_clear_token_for_host`.
pub(crate) async fn forge_clear_token_for_host_inner(
    settings_file: &std::path::Path,
    host: String,
) -> Result<(), AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        bonsai_forge::clear_token_for_host(&host)?;
        let _ = settings::update(&file, |s| settings::remove_forge_host(s, &host));
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P79: evict the cached viewer for `host` WITHOUT deleting the token (expiry
/// flow). Keeps the keychain entry + the index record; only stops surfacing a
/// warm "connected" identity so the panel routes to re-auth. Infallible.
#[tauri::command]
pub async fn forge_invalidate_viewer(
    _state: tauri::State<'_, AppState>,
    host: String,
) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::invalidate_viewer(&host))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}
