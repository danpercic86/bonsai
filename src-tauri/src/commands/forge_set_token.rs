//! `forge_set_token` — the PER-REPO "Connect" token field (`PrPanel` and
//! `ChecksPanel`), split out of `forge.rs` (CLAUDE.md file-size discipline:
//! `forge.rs` sat at 478 lines and this command grows a test seam).
//!
//! Security-audit MEDIUM-2 (2026-09-17), the LIVE occurrence: this command's
//! body was the same swallow as the account-ADD path —
//! `bonsai_forge::store_token(&aid, &token)?` followed by
//! `let _ = settings::update(&file, …)` and `Ok(viewer)`. A failing settings
//! write (read-only dir, disk full, an AV lock — `os error 5`) left the PAT in
//! the OS keychain with NO record naming its `keychain_key`, and every sweep in
//! the app iterates RECORDS, so that credential was unreachable from the UI
//! forever. Source B recurred here too: `upsert_forge_account` re-keys a
//! migrated legacy record to `aid` and abandons its bare-host entry.
//!
//! The fix does NOT re-implement the rollback discriminator. Everything after a
//! successful validation is DELEGATED to
//! [`super::forge_add_account::forge_add_account_inner_with`] — this command is
//! the same operation plus one extra settings mutation (pinning the repo
//! override, OD-3), so that mutation is injected INSIDE the `update_settings`
//! dependency and therefore commits in the SAME transaction as the account
//! record. Two copies of a credential-rollback discriminator is how they drift
//! apart.

use std::path::{Path, PathBuf};

use super::forge_add_account::{forge_add_account_inner_with, AddAccountDeps};
use super::shared::*;

/// Validate a pasted PAT (`GET /user`) for the open repo's origin and, on
/// success, store it in the OS keychain under a three-part account key, upsert
/// the account, and pin it as this repo's override; returns the authenticated
/// viewer. A rejected token stores NOTHING. The token is never logged, never
/// placed in a URL, and never echoed back. Errors: `noRepo` | `authFailed` |
/// `forgeUnsupported` | `noRemote` | `forgeRateLimited` | `networkError` |
/// `git` | `other` (the credential is in the OS keychain but the account could
/// not be saved, or the account could not be saved and the new credential was
/// withdrawn again — see [`forge_add_account_inner_with`]).
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
/// then hand the whole local transaction to [`forge_set_token_with`].
/// Validation is the blocking network call, so it gets its OWN
/// `spawn_blocking` — which keeps the delegate's injectable surface exactly the
/// three local side effects.
pub(crate) async fn forge_set_token_inner(
    state: &AppState,
    settings_file: &Path,
    repo_id: &str,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let workdir = repo_path(state, repo_id)?;
    let (vwork, vtoken) = (workdir.clone(), token.clone());
    let (viewer, host, kind) = tauri::async_runtime::spawn_blocking(move || {
        bonsai_forge::validate_repo_token(&vwork, &vtoken)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))??;
    if host.is_empty() {
        // Unparseable origin: no host to key an account by, so the token cannot
        // be stored under an account and NOTHING is written — not the keychain,
        // not settings. Return the validated viewer unchanged (the pre-split
        // behavior of this early return, preserved exactly).
        //
        // UNREACHABLE IN PRACTICE, at HEAD as much as here — kept because
        // preserving the split's behavior exactly is worth more than the dead
        // branch costs, but do not go looking for a caller. An empty `host` can
        // only come from `resolve_target`'s unparseable-origin fallback
        // (`crates/bonsai-forge/src/lib.rs:82-91`), which pairs it with
        // `ForgeKind::Unknown`; `validate_repo_token` runs BEFORE this line and
        // goes through `validate_token` → `build_provider` (Unknown ⇒
        // `GitHubProvider`) → `viewer()` → `require_supported`
        // (`crates/bonsai-forge/src/github/mod.rs:46-56`), which rejects every
        // non-`GitHub` kind with `ForgeUnsupported`. So an empty host always
        // errors out of the `??` above and never gets here.
        return Ok(viewer);
    }
    forge_set_token_with(
        settings_file,
        workdir,
        host,
        kind,
        token,
        viewer,
        AddAccountDeps::default(),
    )
    .await
}

/// Dependency-injected core of [`forge_set_token_inner`]: everything after a
/// SUCCESSFUL token validation, for a repo whose origin host is known.
///
/// Delegates to [`forge_add_account_inner_with`] with the caller's `deps`
/// WRAPPED: the injected `update_settings` runs the add's own mutation and then
/// [`settings::set_repo_override`], so the repo pin lands in the same
/// load→mutate→save transaction as the account record. Consequences, all of them
/// the point of delegating:
///
/// - the MEDIUM-2 rollback discriminator (was the stored credential NEW, or did
///   it OVERWRITE one an existing record already names?) applies verbatim here,
///   with the same user-facing copy;
/// - a failing settings write no longer returns `Ok` — and it leaves NO repo
///   override either, because the pin was part of the failed transaction;
/// - source B's superseded bare-host key is swept on success.
pub(crate) async fn forge_set_token_with(
    settings_file: &Path,
    workdir: PathBuf,
    host: String,
    kind: ForgeKind,
    token: String,
    viewer: ForgeViewer,
    deps: AddAccountDeps,
) -> Result<ForgeViewer, AppError> {
    // The key the add will store under, recomputed here (pure, no IO) because
    // the override must name exactly that account. `account_id` is derived from
    // the LOWERCASED host, matching what the delegate does with `host`.
    let aid = settings::account_id(kind, &host.to_ascii_lowercase(), Some(&viewer.login));
    let workdir_str = workdir.to_string_lossy().to_string();
    let AddAccountDeps {
        store_token,
        delete_token,
        update_settings,
    } = deps;
    let deps = AddAccountDeps {
        store_token,
        delete_token,
        update_settings: Box::new(move |file, mutate| {
            update_settings(file, &mut |s| {
                mutate(s);
                // OD-3: the newly-connected account is what this repo uses.
                settings::set_repo_override(s, &workdir_str, &aid);
            })
        }),
    };
    forge_add_account_inner_with(settings_file, host, kind, token, viewer, deps).await
}

#[cfg(test)]
#[path = "forge_set_token_tests.rs"]
mod tests;
