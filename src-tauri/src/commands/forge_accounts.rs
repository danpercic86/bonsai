//! P80 multi-account forge command layer: account RESOLUTION (per-repo override
//! → owner match → host default → single → first) and the global
//! account-management commands (add / remove / set-host-default /
//! set-repo-account / list / sign-out-host).
//!
//! Split from `forge.rs` (which keeps the PR-data commands) to hold the
//! resolution algorithm + its unit tests in a focused module (CLAUDE.md
//! file-size discipline). Tokens live ONLY in the OS keychain — every record
//! here carries a `keychain_key`, never a token.

use std::path::Path;

use super::shared::*;

/// The outcome of [`resolve_account`]: the chosen account record (if any) and
/// how it was chosen. `None` account ⇒ unauthenticated (no account on the host).
pub(crate) struct AccountResolution {
    pub account: Option<settings::ForgeAccountRecord>,
    pub source: AccountSource,
}

/// P80 §4 resolution: pick the account backing `repo_path` on `host` for `owner`.
/// Order: per-repo override → owner match (single login==owner) → host default →
/// single account → first (most-recent, UI nudges). A deleted pinned account
/// falls through (never errors). Pure — no git, no network, no keychain.
pub(crate) fn resolve_account(
    s: &settings::Settings,
    repo_path: &str,
    host: &str,
    owner: &str,
) -> AccountResolution {
    let host_l = host.to_ascii_lowercase();
    let accts: Vec<settings::ForgeAccountRecord> = s
        .forge_accounts
        .iter()
        .filter(|a| a.host == host_l)
        .cloned()
        .collect();
    if accts.is_empty() {
        return AccountResolution {
            account: None,
            source: AccountSource::None,
        };
    }

    // 1. per-repo override — a MANUAL pin always wins over an owner match.
    if let Some(ov) = s
        .repo_forge_overrides
        .iter()
        .find(|o| crate::commands::same_repo_path(&o.repo_path, repo_path))
    {
        if let Some(a) = accts.iter().find(|a| a.account_id == ov.account_id) {
            return AccountResolution {
                account: Some(a.clone()),
                source: AccountSource::Override,
            };
        }
        // pinned account was deleted → fall through (never error).
    }

    // 2. owner match (login-based, NO API calls; both sides lowercased).
    if !owner.is_empty() {
        let matches: Vec<&settings::ForgeAccountRecord> = accts
            .iter()
            .filter(|a| {
                a.login
                    .as_deref()
                    .is_some_and(|l| l.eq_ignore_ascii_case(owner))
            })
            .collect();
        if matches.len() == 1 {
            return AccountResolution {
                account: Some(matches[0].clone()),
                source: AccountSource::OwnerMatch,
            };
        }
        // 0 or >1 matches → fall through (never error).
    }

    // 3. host default.
    if let Some(d) = s.forge_host_defaults.iter().find(|d| d.host == host_l) {
        if let Some(a) = accts.iter().find(|a| a.account_id == d.account_id) {
            return AccountResolution {
                account: Some(a.clone()),
                source: AccountSource::HostDefault,
            };
        }
    }

    // 4. single account.
    if accts.len() == 1 {
        return AccountResolution {
            account: Some(accts[0].clone()),
            source: AccountSource::Single,
        };
    }

    // 5. multiple accounts, no usable default (OD-4): first (most-recent); the
    // UI nudges the user to pick a default. `accts` is non-empty here (checked
    // above), so `into_iter().next()` always yields Some.
    AccountResolution {
        account: accts.into_iter().next(),
        source: AccountSource::HostDefault,
    }
}

/// Resolved forge identity + keychain key for a repo, computed inside a blocking
/// task (reads `origin`, then the pure [`resolve_account`]).
pub(crate) struct ForgeResolved {
    pub keychain_key: Option<String>,
    pub account_id: Option<String>,
    pub source: AccountSource,
}

/// Blocking: resolve the repo's forge identity from `origin`, load settings, and
/// run [`resolve_account`]. An unparseable origin yields empty host/owner ⇒
/// `None` account (unauthenticated), matching the crate's friendly degradation.
pub(crate) fn resolve_forge_blocking(
    workdir: &Path,
    settings_file: &Path,
) -> Result<ForgeResolved, AppError> {
    let (host, owner, _kind) = bonsai_forge::resolve_forge_identity(workdir)?;
    let s = settings::load_from(settings_file);
    let workdir_str = workdir.to_string_lossy().to_string();
    let res = resolve_account(&s, &workdir_str, &host, &owner);
    let (keychain_key, account_id) = match &res.account {
        Some(a) => (Some(a.keychain_key.clone()), Some(a.account_id.clone())),
        None => (None, None),
    };
    Ok(ForgeResolved {
        keychain_key,
        account_id,
        source: res.source,
    })
}

/// Blocking: the resolved keychain key for a repo's forge (for the PR-data
/// commands that only need to `open_with_key`).
pub(crate) fn resolved_key(
    workdir: &Path,
    settings_file: &Path,
) -> Result<Option<String>, AppError> {
    Ok(resolve_forge_blocking(workdir, settings_file)?.keychain_key)
}

// ---------------------------------------------------------------------------
// P80: global forge account management.
// ---------------------------------------------------------------------------

/// P80: all forge accounts across all hosts (the settings index), each with live
/// `connected` (keychain presence for its `keychain_key`) + best-effort
/// login/avatar + `isHostDefault`. NO network. Errors: `other`.
#[tauri::command]
pub async fn forge_list_accounts(app: tauri::AppHandle) -> Result<Vec<ForgeAccount>, AppError> {
    let file = settings::settings_file(&app)?;
    forge_list_accounts_inner(&file).await
}

/// Runtime-free core of `forge_list_accounts`.
pub(crate) async fn forge_list_accounts_inner(
    settings_file: &Path,
) -> Result<Vec<ForgeAccount>, AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        let accounts = s
            .forge_accounts
            .iter()
            .map(|r| {
                let cached = bonsai_forge::auth::cached_viewer(&r.host);
                let login = r
                    .login
                    .clone()
                    .or_else(|| cached.as_ref().map(|v| v.login.clone()));
                let avatar_url = r
                    .avatar_url
                    .clone()
                    .or_else(|| cached.as_ref().and_then(|v| v.avatar_url.clone()));
                let is_host_default = s
                    .forge_host_defaults
                    .iter()
                    .any(|d| d.host == r.host && d.account_id == r.account_id);
                ForgeAccount {
                    account_id: r.account_id.clone(),
                    host: r.host.clone(),
                    kind: r.kind,
                    login,
                    avatar_url,
                    connected: bonsai_forge::auth::global().has(&r.keychain_key),
                    is_host_default,
                }
            })
            .collect();
        Ok(accounts)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P80: validate a pasted PAT against `host`/`kind` directly (no repo), learn the
/// login, store it under a three-part keychain key, and upsert the account; if
/// the host has no default yet, make this the default. Azure DevOps ⇒
/// `forgeUnsupported` (OD-6). Errors: `authFailed` | `forgeUnsupported` |
/// `forgeRateLimited` | `networkError` | `other`.
#[tauri::command]
pub async fn forge_add_account(
    app: tauri::AppHandle,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let file = settings::settings_file(&app)?;
    forge_add_account_inner(&file, host, kind, token).await
}

/// Runtime-free core of `forge_add_account` (also backs the `forge_set_token_for_host`
/// back-compat alias).
pub(crate) async fn forge_add_account_inner(
    settings_file: &Path,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let viewer = bonsai_forge::validate_host_token(&host, kind, &token)?;
        let host_l = host.to_ascii_lowercase();
        let login = viewer.login.clone();
        let aid = settings::account_id(kind, &host_l, Some(&login));
        // Store under the three-part key ONLY after successful validation.
        bonsai_forge::store_token(&aid, &token)?;
        let rec = settings::ForgeAccountRecord {
            account_id: aid.clone(),
            keychain_key: aid.clone(),
            host: host_l.clone(),
            kind,
            login: Some(login.clone()),
            avatar_url: viewer.avatar_url.clone(),
        };
        let _ = settings::update(&file, |s| {
            settings::upsert_forge_account(s, rec.clone());
            if !s.forge_host_defaults.iter().any(|d| d.host == host_l) {
                settings::set_host_default(s, &host_l, &aid);
            }
            // OD-5: keep the legacy known-hosts index mirrored for one release.
            settings::upsert_forge_host(s, &host_l, kind, Some(login.clone()));
        });
        Ok(viewer)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P79 back-compat alias for [`forge_add_account`] — same behavior, kept so
/// existing callers/mocks keep working.
#[tauri::command]
pub async fn forge_set_token_for_host(
    app: tauri::AppHandle,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let file = settings::settings_file(&app)?;
    forge_add_account_inner(&file, host, kind, token).await
}

/// P80: delete an account's token (by its `keychain_key`), remove the record, and
/// clean references (promote/clear host default, drop repo overrides). Idempotent.
/// Errors: `other`.
#[tauri::command]
pub async fn forge_remove_account(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    forge_remove_account_inner(&file, account_id).await
}

/// Runtime-free core of `forge_remove_account`.
pub(crate) async fn forge_remove_account_inner(
    settings_file: &Path,
    account_id: String,
) -> Result<(), AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        let rec = s
            .forge_accounts
            .iter()
            .find(|a| a.account_id == account_id)
            .cloned();
        if let Some(r) = &rec {
            let _ = bonsai_forge::delete_token(&r.keychain_key);
            bonsai_forge::invalidate_viewer(&r.host);
        }
        let _ = settings::update(&file, |s| {
            settings::remove_forge_account(s, &account_id);
            // OD-5 legacy mirror: drop the known-hosts entry once no account
            // remains on that host.
            if let Some(r) = &rec {
                if !s.forge_accounts.iter().any(|a| a.host == r.host) {
                    settings::remove_forge_host(s, &r.host);
                }
            }
        });
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P80: set/replace the default account for `host`. Errors if `account_id` isn't
/// an account on that host. Errors: `other`.
#[tauri::command]
pub async fn forge_set_host_default(
    app: tauri::AppHandle,
    host: String,
    account_id: String,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    forge_set_host_default_inner(&file, host, account_id).await
}

/// Runtime-free core of `forge_set_host_default`.
pub(crate) async fn forge_set_host_default_inner(
    settings_file: &Path,
    host: String,
    account_id: String,
) -> Result<(), AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let host_l = host.to_ascii_lowercase();
        let s = settings::load_from(&file);
        if !s
            .forge_accounts
            .iter()
            .any(|a| a.host == host_l && a.account_id == account_id)
        {
            return Err(AppError::Other(
                "account is not on the given host".to_string(),
            ));
        }
        let _ = settings::update(&file, |s| settings::set_host_default(s, &host_l, &account_id));
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P80: pin (`account_id`) or clear (`null` ⇒ inherit: owner match → host
/// default) the per-repo account override. Errors: `noRepo` | `other`.
#[tauri::command]
pub async fn forge_set_repo_account(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    repo_id: String,
    account_id: Option<String>,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)?;
    forge_set_repo_account_inner(state.inner(), &file, &repo_id, account_id).await
}

/// Runtime-free core of `forge_set_repo_account`.
pub(crate) async fn forge_set_repo_account_inner(
    state: &AppState,
    settings_file: &Path,
    repo_id: &str,
    account_id: Option<String>,
) -> Result<(), AppError> {
    let workdir = repo_path(state, repo_id)?;
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let workdir_str = workdir.to_string_lossy().to_string();
        let _ = settings::update(&file, |s| match &account_id {
            Some(a) => settings::set_repo_override(s, &workdir_str, a),
            None => settings::clear_repo_override(s, &workdir_str),
        });
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P79 (retained): sign out ALL accounts on `host` — delete each account's
/// keychain entry + the legacy bare-host entry, drop the records, defaults, and
/// any overrides pointing at them. Idempotent. Errors: `other`.
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
    settings_file: &Path,
    host: String,
) -> Result<(), AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let host_l = host.to_ascii_lowercase();
        let s = settings::load_from(&file);
        let on_host: Vec<settings::ForgeAccountRecord> = s
            .forge_accounts
            .iter()
            .filter(|a| a.host == host_l)
            .cloned()
            .collect();
        for a in &on_host {
            let _ = bonsai_forge::delete_token(&a.keychain_key);
        }
        // Legacy bare-host token + cached viewer.
        let _ = bonsai_forge::clear_token_for_host(&host_l);
        let ids: Vec<String> = on_host.iter().map(|a| a.account_id.clone()).collect();
        let _ = settings::update(&file, |s| {
            s.forge_accounts.retain(|a| a.host != host_l);
            s.forge_host_defaults.retain(|d| d.host != host_l);
            s.repo_forge_overrides
                .retain(|o| !ids.contains(&o.account_id));
            settings::remove_forge_host(s, &host_l);
        });
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

/// P79 (retained): evict the cached viewer for `host` WITHOUT deleting the token
/// (expiry flow). Infallible.
#[tauri::command]
pub async fn forge_invalidate_viewer(
    _state: tauri::State<'_, AppState>,
    host: String,
) -> Result<(), AppError> {
    tauri::async_runtime::spawn_blocking(move || bonsai_forge::invalidate_viewer(&host))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))
}

#[cfg(test)]
#[path = "forge_accounts_tests.rs"]
mod tests;
