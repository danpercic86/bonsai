//! P80 account REMOVAL — `forge_remove_account` and its runtime-free core.
//!
//! Split out of `forge_accounts.rs` (CLAUDE.md file-size discipline) because
//! removal carries injected side effects: the OS-keychain delete and the
//! settings write, both of which must be failable in tests. `forge_clear_host.rs`
//! is the other such command in that layer (sign out of a whole host).
//!
//! User ruling (2026-09-17): deleting the token IS the operation. See
//! [`forge_remove_account_inner_with`] for the three outcomes.

use std::path::Path;

use super::shared::*;

/// Deletes the token stored under a `keychain_key`.
type DeleteTokenFn = Box<dyn Fn(&str) -> Result<(), AppError> + Send + 'static>;

/// Applies a settings mutation as one load→mutate→save transaction.
type UpdateSettingsFn = Box<
    dyn Fn(&Path, &mut dyn FnMut(&mut settings::Settings)) -> Result<(), AppError> + Send + 'static,
>;

/// The user-facing failure messages, as a fixed HEAD (the human sentences,
/// ending `. `) plus the shared [`CAUSE_LEAD`], so every message is
/// `format!("{HEAD}{CAUSE_LEAD}{e}")` and the cause-last ordering rule
/// (P114 §3) lives in exactly one place.
///
/// Consts so ONE Rust definition is the source of truth and the cross-language
/// guard (`forge_remove_account_tests::mock_copy_mirrors_the_rust_copy`) can
/// assert the harness mirror in `src/ipc/mock/handlers/forgeRemoveFailure.ts`
/// contains each whole HEAD as a quoted TS literal — editing the copy here
/// without editing the mock turns that test red.
///
/// P114 rule 1: these are OUTCOMES (which half of a two-step operation
/// happened), so they own the whole sentence and the caller renders them
/// verbatim — `SettingsAccountsSection.confirmRemove` adds no prefix.
const CAUSE_LEAD: &str = "Details: ";
const KEYCHAIN_FAIL_HEAD: &str = "Couldn't remove the account's credential from the OS keychain. Nothing was changed — the account is still listed, so you can try again. ";
/// P114 rule 2 (state, not act): `crates/bonsai-forge/src/auth.rs:53` folds
/// `NoEntry` into `Ok(())`, so on a RETRY nothing is deleted and "was removed"
/// would be false. "is no longer in" stays true either way.
const SETTINGS_FAIL_HEAD: &str = "The credential is no longer in the OS keychain, but the account list couldn't be saved. Try again to finish removing it. ";
/// The `rec == None` variant of the settings-save failure: no `delete_token`
/// call happened (an unknown or already-removed `account_id` has no
/// `keychain_key`), so the message must NOT claim a credential was deleted.
const SETTINGS_FAIL_NO_CREDENTIAL_HEAD: &str =
    "The account list couldn't be saved. Try again to finish removing it. ";
/// P114 N1: `settings::settings_file`'s own error is a bare lowercase CAUSE
/// shared by many commands, so it is wrapped into sentence form HERE (not
/// edited there) to keep "the caller renders verbatim" total for this command.
const CONFIG_DIR_FAIL_HEAD: &str =
    "Couldn't remove the account — Bonsai can't reach its settings folder. ";
/// P114 N2: after a panicked blocking task the real state is genuinely
/// unknown, so this is the one message that must not promise either way.
const TASK_JOIN_FAIL_HEAD: &str = "Couldn't remove the account. It may or may not have been removed — check the list and try again. ";

/// P80: delete an account's token (by its `keychain_key`), remove the record, and
/// clean references (promote/clear host default, drop repo overrides).
///
/// Idempotent: a `keychain_key` that is no longer in the keychain is success, so
/// this is safely re-runnable after a partial failure. Deleting the token is the
/// operation — if the keychain refuses, no settings or UI state is changed and
/// the account stays listed. Errors: `other` — the keychain refused; the
/// credential was deleted but settings could not be saved; or (unknown /
/// already-removed id, so no credential was touched) settings could not be saved.
#[tauri::command]
pub async fn forge_remove_account(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<(), AppError> {
    let file = settings::settings_file(&app)
        .map_err(|e| AppError::Other(format!("{CONFIG_DIR_FAIL_HEAD}{CAUSE_LEAD}{e}")))?;
    forge_remove_account_inner(&file, account_id).await
}

/// Injectable side effects of [`forge_remove_account_inner`]. The real
/// implementations live in [`Default`]; tests substitute failing ones because
/// `bonsai_forge::delete_token` talks to the process-global OS keychain and
/// `settings::update` cannot be made to fail portably from the outside.
pub(crate) struct RemoveAccountDeps {
    /// Delete the token stored under a `keychain_key`. A key that is NOT in the
    /// keychain is `Ok(())` (`crates/bonsai-forge/src/auth.rs:53` folds
    /// `keyring::Error::NoEntry` into success), which is what keeps this command
    /// re-runnable after a partial failure.
    pub delete_token: DeleteTokenFn,
    /// Load→mutate→save settings as one transaction (keeps `settings::update`'s
    /// process-wide IO lock rather than re-implementing it here).
    pub update_settings: UpdateSettingsFn,
}

impl Default for RemoveAccountDeps {
    fn default() -> Self {
        Self {
            delete_token: Box::new(bonsai_forge::delete_token),
            update_settings: Box::new(|file, mutate| {
                settings::update(file, |s| mutate(s)).map(|_| ())
            }),
        }
    }
}

/// Runtime-free core of `forge_remove_account`.
pub(crate) async fn forge_remove_account_inner(
    settings_file: &Path,
    account_id: String,
) -> Result<(), AppError> {
    forge_remove_account_inner_with(settings_file, account_id, RemoveAccountDeps::default()).await
}

/// Dependency-injected core of [`forge_remove_account_inner`].
///
/// Deleting the token IS the operation (user ruling 2026-09-17): if the
/// credential is still in the keychain the removal FAILED, so a failing
/// `delete_token` returns `Err` and leaves all settings and UI state untouched —
/// no settings record removed, no legacy host mirror dropped, and no viewer
/// eviction (the token is still live, so the cached viewer is still accurate;
/// evicting it would show a connected account as disconnected). The row stays
/// visible and Remove can be retried in place. (`TokenStore::delete`,
/// `crates/bonsai-forge/src/auth.rs:119-122`, does drop its in-process
/// token-cache entry before the keychain call; that is not settings or UI state,
/// is not user-visible, and self-heals on the next lazy warm.)
pub(crate) async fn forge_remove_account_inner_with(
    settings_file: &Path,
    account_id: String,
    deps: RemoveAccountDeps,
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
            (deps.delete_token)(&r.keychain_key)
                .map_err(|e| AppError::Other(format!("{KEYCHAIN_FAIL_HEAD}{CAUSE_LEAD}{e}")))?;
            bonsai_forge::invalidate_viewer(&r.host);
        }
        // Whether a credential was actually deleted decides which settings-save
        // failure message is TRUE: with `rec == None` nothing was removed from the
        // keychain, so the asymmetric wording would be a false claim.
        let had_credential = rec.is_some();
        (deps.update_settings)(&file, &mut |s| {
            settings::remove_forge_account(s, &account_id);
            // OD-5 legacy mirror: drop the known-hosts entry once no account
            // remains on that host.
            if let Some(r) = &rec {
                if !s.forge_accounts.iter().any(|a| a.host == r.host) {
                    settings::remove_forge_host(s, &r.host);
                }
            }
        })
        .map_err(|e| {
            AppError::Other(if had_credential {
                format!("{SETTINGS_FAIL_HEAD}{CAUSE_LEAD}{e}")
            } else {
                format!("{SETTINGS_FAIL_NO_CREDENTIAL_HEAD}{CAUSE_LEAD}{e}")
            })
        })?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("{TASK_JOIN_FAIL_HEAD}{CAUSE_LEAD}{e}")))?
}

#[cfg(test)]
#[path = "forge_remove_account_tests.rs"]
mod tests;
