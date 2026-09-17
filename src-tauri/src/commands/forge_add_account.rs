//! P80 account ADD — `forge_add_account` (and its P79 alias) plus the
//! runtime-free, dependency-injected core.
//!
//! Split out of `forge_accounts.rs` (CLAUDE.md file-size discipline) for the
//! same reason `forge_remove_account.rs` and `forge_clear_host.rs` were: this
//! is the third command in that layer whose side effects (the OS-keychain
//! write, a possible compensating delete, and the settings write) must be
//! failable in tests.
//!
//! Security-audit MEDIUM-2 (2026-09-17), source A: the previous body did
//! `bonsai_forge::store_token(&aid, &token)?` and then
//! `let _ = settings::update(&file, …)`. On a failing settings write the
//! credential was in the keychain with NO record naming its `keychain_key` —
//! unreachable from the UI forever, because every sweep iterates RECORDS.
//!
//! The obvious fix (delete the token you just stored) is WRONG on its own:
//! `TokenStore::set` is OVERWRITE (`crates/bonsai-forge/src/auth.rs:113-118`
//! delegates to `Keychain::set`, whose real impl is `keyring::Entry::set_password`
//! at `auth.rs:47-51`, which REPLACES), and `aid` is derived deterministically
//! from `(kind, host, login)`. So on a RE-ADD of an existing account an
//! unconditional rollback would delete the user's previously-working credential
//! while the old record still pointed at `aid` — a record with no token, i.e.
//! the SAME orphan bug in the other direction. Hence the pre-read discriminator
//! in [`forge_add_account_inner_with`].

use std::path::Path;

use super::shared::*;

/// Stores a token under a `keychain_key`. OVERWRITE semantics — see the module
/// doc; the whole rollback discriminator rests on this.
type StoreTokenFn = Box<dyn Fn(&str, &str) -> Result<(), AppError> + Send + 'static>;

/// Deletes the token stored under a `keychain_key`.
type DeleteTokenFn = Box<dyn Fn(&str) -> Result<(), AppError> + Send + 'static>;

/// Applies a settings mutation as one load→mutate→save transaction.
type UpdateSettingsFn = Box<
    dyn Fn(&Path, &mut dyn FnMut(&mut settings::Settings)) -> Result<(), AppError> + Send + 'static,
>;

/// The user-facing failure messages, as a fixed HEAD (the human sentences,
/// ending `. `) plus the shared [`CAUSE_LEAD`], in the shape
/// `forge_remove_account.rs` establishes: every message is
/// `format!("{HEAD}{CAUSE_LEAD}{cause}")`, so the P114 §3 cause-last ordering
/// rule lives in exactly one place per command.
///
/// P114 rule 1: these are OUTCOMES (which half of a two-step operation
/// happened), so they own the whole sentence. There are FOUR render sites now
/// that `forge_set_token` delegates here, and all four render the message
/// verbatim inline: `SettingsAccountAddForm.addError`'s `default` arm returns
/// `e.message`; `SettingsAccountCard.submit`, `PrPanel.handleConnect` and
/// `ChecksPanel.handleConnect` use `errorMessage(e)`. So the const text IS the
/// rendered text.
///
/// CAVEAT, not fixed here: the two panel sites ALSO raise a toast reading
/// `Could not connect: {message}`, which double-frames an outcome sentence
/// ("Could not connect: The credential in the OS keychain is up to date,
/// but…"). Pre-existing, and UI copy belongs to `ui-designer` — reported as a
/// SHOULD-FIX rather than changed here.
///
/// Consts so ONE Rust definition is the source of truth and the cross-language
/// guard (`forge_add_account_tests::mock_copy_mirrors_the_rust_copy`) can
/// assert the harness mirror in `src/ipc/mock/handlers/forgeAddFailure.ts`
/// contains each whole HEAD as a quoted TS literal — editing the copy here
/// without editing the mock turns that test red.
///
/// No message ever interpolates a `keychain_key`: keys are never shown, never
/// logged.
pub(crate) const CAUSE_LEAD: &str = "Details: ";
/// Cases 1 and 3 — the store wrote a NEW, unreferenced credential and the
/// compensating delete succeeded. P114 rule 2 (state, not act): "is not in the
/// OS keychain" describes the end state, which stays true on a retry.
pub(crate) const ROLLED_BACK_HEAD: &str =
    "The account couldn't be saved, and its credential is not in the OS keychain. Add it again. ";
/// Case 2 — the store OVERWROTE a live credential already referenced by an
/// existing record, so nothing was deleted. Succeeded half first, joined by
/// "but" (P114 rule 3).
pub(crate) const CREDENTIAL_KEPT_HEAD: &str = "The credential in the OS keychain is up to date, but the account details couldn't be saved. Try again to finish adding the account. ";
/// Cases 1 and 3 where the compensating delete ITSELF failed. Names the
/// asymmetry with a retry cue, which is honest: a re-add stores under the same
/// `aid`, so a later successful settings write references exactly this
/// credential.
///
/// Opens with "The credential", not "Its credential": this is rendered
/// VERBATIM into a `role="alert"` utterance with no preceding sentence, so a
/// pronoun here has no antecedent for a screen-reader listener.
pub(crate) const ROLLBACK_FAILED_HEAD: &str = "The credential is in the OS keychain, but the account couldn't be saved. Try again to finish adding it. ";
/// Separator between the two causes of [`ROLLBACK_FAILED_HEAD`] (the settings
/// error and the delete error). A named const so the cross-language guard
/// covers it too, mirroring `forge_clear_host`'s `KEYCHAIN_FAIL_JOIN`.
const CAUSE_JOIN: &str = "; ";

/// P80: validate a pasted PAT against `host`/`kind` directly (no repo), learn the
/// login, store it under a three-part keychain key, and upsert the account; if
/// the host has no default yet, make this the default. Azure DevOps ⇒
/// `forgeUnsupported` (OD-6). Errors: `authFailed` | `forgeUnsupported` |
/// `forgeRateLimited` | `networkError` | `other` (the credential is in the OS
/// keychain but the account could not be saved, or the account could not be
/// saved and the new credential was withdrawn again).
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

/// Injectable side effects of [`forge_add_account_inner`]. The real
/// implementations live in [`Default`]; tests substitute failing ones because
/// the keychain is process-global and `settings::update` cannot be made to fail
/// portably from the outside.
///
/// Deliberately constructed UNCONDITIONALLY by [`forge_add_account_inner`]: no
/// env var, feature or setting can swap the production side effects — the seam
/// exists for callers of [`forge_add_account_inner_with`] only.
pub(crate) struct AddAccountDeps {
    /// Store `token` under a `keychain_key`. OVERWRITES any existing value.
    pub store_token: StoreTokenFn,
    /// Delete the token stored under a `keychain_key`. A key that is NOT in the
    /// keychain is `Ok(())` (`crates/bonsai-forge/src/auth.rs:53` folds
    /// `keyring::Error::NoEntry` into success).
    pub delete_token: DeleteTokenFn,
    /// Load→mutate→save settings as one transaction (keeps `settings::update`'s
    /// process-wide IO lock rather than re-implementing it here).
    pub update_settings: UpdateSettingsFn,
}

impl Default for AddAccountDeps {
    fn default() -> Self {
        Self {
            store_token: Box::new(bonsai_forge::store_token),
            delete_token: Box::new(bonsai_forge::delete_token),
            update_settings: Box::new(|file, mutate| {
                settings::update(file, |s| mutate(s)).map(|_| ())
            }),
        }
    }
}

/// Runtime-free core of `forge_add_account` (also backs the
/// `forge_set_token_for_host` back-compat alias). `forge_set_token` — the
/// per-repo Connect field — reaches the DI core below directly, with its own
/// validation and a wrapped `update_settings`.
///
/// Validation is a blocking network call, so it runs in its OWN
/// `spawn_blocking` here rather than inside [`forge_add_account_inner_with`] —
/// which keeps that function's injectable surface exactly the three local side
/// effects, so every add outcome is testable with no network and no keychain.
pub(crate) async fn forge_add_account_inner(
    settings_file: &Path,
    host: String,
    kind: ForgeKind,
    token: String,
) -> Result<ForgeViewer, AppError> {
    let (vhost, vtoken) = (host.clone(), token.clone());
    let viewer = tauri::async_runtime::spawn_blocking(move || {
        bonsai_forge::validate_host_token(&vhost, kind, &vtoken)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))??;
    forge_add_account_inner_with(
        settings_file,
        host,
        kind,
        token,
        viewer,
        AddAccountDeps::default(),
    )
    .await
}

/// Dependency-injected core of [`forge_add_account_inner`]: everything after a
/// SUCCESSFUL token validation. ALSO the core of `forge_set_token`
/// (`forge_set_token.rs`), which injects the repo-override mutation into
/// `update_settings` so its pin commits in this same transaction — there is
/// exactly ONE implementation of the rollback discriminator below.
///
/// Audit MEDIUM-2 source A — the settings write is no longer swallowed, and the
/// compensating delete is conditional on a settings PRE-READ:
///
/// 1. No pre-existing record named `aid` ⇒ the store wrote a NEW credential
///    nobody references, so a failed settings write withdraws it again.
/// 2. A pre-existing record already had `keychain_key == aid` ⇒ the store
///    OVERWROTE a live credential. It must NOT be deleted: the old record still
///    points at `aid`, so deleting would leave a record with no token
///    (`connected: false`) — the orphan bug in the other direction.
/// 3. A pre-existing LEGACY record for the same `account_id` (its
///    `keychain_key` is the bare host) ⇒ the `aid` key is new and unreferenced,
///    so case 1 applies.
///
/// Audit MEDIUM-2 source B — on SUCCESS the superseded key of a re-keyed record
/// is deleted, because `upsert_forge_account` replaces by `account_id` and
/// silently rewrites `keychain_key` to `aid`, abandoning the bare-host entry.
/// ORDER MATTERS: settings write FIRST, then the delete. Delete-then-fail would
/// leave a still-legacy record pointing at a key that is already gone. A FAILED
/// delete after a successful write must NOT fail the add — the account works,
/// and `forge_remove_account`'s last-account-on-host backstop is what sweeps
/// the leftover.
pub(crate) async fn forge_add_account_inner_with(
    settings_file: &Path,
    host: String,
    kind: ForgeKind,
    token: String,
    viewer: ForgeViewer,
    deps: AddAccountDeps,
) -> Result<ForgeViewer, AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let host_l = host.to_ascii_lowercase();
        let login = viewer.login.clone();
        let aid = settings::account_id(kind, &host_l, Some(&login));

        // PRE-READ, before the store: afterwards it is impossible to tell an
        // overwritten live credential from a newly created one.
        let pre = settings::load_from(&file);
        let key_already_referenced = pre.forge_accounts.iter().any(|a| a.keychain_key == aid);
        // The key this add SUPERSEDES: the record that `upsert_forge_account`
        // will replace (same `account_id`) currently names a different key, and
        // no OTHER record names it, so after the write nothing references it.
        // In practice this only ever fires for a migrated legacy record
        // (`keychain_key == <bare host>`), which is the audit's source B; the
        // "no other record names it" filter makes it safe in general.
        let superseded = pre
            .forge_accounts
            .iter()
            .find(|a| a.account_id == aid && a.keychain_key != aid)
            .map(|a| a.keychain_key.clone())
            .filter(|k| {
                !pre.forge_accounts
                    .iter()
                    .any(|a| a.account_id != aid && a.keychain_key == *k)
            });

        // Store under the three-part key ONLY after successful validation.
        (deps.store_token)(&aid, &token)?;
        let rec = settings::ForgeAccountRecord {
            account_id: aid.clone(),
            keychain_key: aid.clone(),
            host: host_l.clone(),
            kind,
            login: Some(login.clone()),
            avatar_url: viewer.avatar_url.clone(),
        };
        let write = (deps.update_settings)(&file, &mut |s| {
            settings::upsert_forge_account(s, rec.clone());
            if !s.forge_host_defaults.iter().any(|d| d.host == host_l) {
                settings::set_host_default(s, &host_l, &aid);
            }
            // OD-5: keep the legacy known-hosts index mirrored for one release.
            settings::upsert_forge_host(s, &host_l, kind, Some(login.clone()));
        });
        if let Err(e) = write {
            if key_already_referenced {
                // Case 2 — a live credential was overwritten. Deleting it would
                // orphan the record that still points at it.
                return Err(AppError::Other(format!(
                    "{CREDENTIAL_KEPT_HEAD}{CAUSE_LEAD}{e}"
                )));
            }
            // Cases 1 and 3 — the credential just written is referenced by
            // nothing, so withdraw it rather than leave it unreachable. The
            // SUPERSEDED key is deliberately left alone: its record survived
            // the failed write and still points at it.
            return match (deps.delete_token)(&aid) {
                Ok(()) => Err(AppError::Other(format!(
                    "{ROLLED_BACK_HEAD}{CAUSE_LEAD}{e}"
                ))),
                Err(de) => Err(AppError::Other(format!(
                    "{ROLLBACK_FAILED_HEAD}{CAUSE_LEAD}{e}{CAUSE_JOIN}{de}"
                ))),
            };
        }
        // Settings are saved, so the record now names `aid`. Sweep the key it
        // abandoned (best effort — the account works either way).
        if let Some(old) = superseded {
            let _ = (deps.delete_token)(&old);
        }
        Ok(viewer)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

#[cfg(test)]
#[path = "forge_add_account_tests.rs"]
mod tests;

// The single real-keychain test, split out so neither file crosses the
// CLAUDE.md 500-line limit (and so the one module that can touch the
// developer's credential manager is obvious from the module list).
#[cfg(test)]
#[path = "forge_add_account_real_keychain_tests.rs"]
mod real_keychain_tests;
