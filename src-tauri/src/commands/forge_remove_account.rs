//! P80 account REMOVAL — `forge_remove_account` and its runtime-free core.
//!
//! Split out of `forge_accounts.rs` (CLAUDE.md file-size discipline) because
//! removal carries injected side effects: the OS-keychain delete and the
//! settings write, both of which must be failable in tests. `forge_clear_host.rs`
//! is the other such command in that layer (sign out of a whole host).
//!
//! User ruling (2026-09-17): deleting the token IS the operation. See
//! [`forge_remove_account_inner_with`] for the outcomes. The LEGACY bare-host
//! sweep is the one exception (P114 Addendum A): it is BEST EFFORT and its
//! refusal is returned as data on the success value, never as an error.

use std::path::Path;

use serde::Serialize;

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
/// P114 Addendum A / outcome R4 — the account WAS removed, but the host's
/// leftover legacy (bare-host) credential could not be. The sweep is best
/// effort (user ruling 2026-09-17): fail-closed could permanently strand a
/// user, because on a retry the account's own token is already gone
/// (`NoEntry → Ok`) while the legacy key refuses again, leaving a listed,
/// disconnected, UNREMOVABLE account fixable only by hand in the OS credential
/// store.
///
/// So this string is NOT a failure: it rides the FULFILLED value
/// ([`ForgeRemoveOutcome::leftover`]), and the absence of `FAIL` in its name is
/// deliberate — every sibling is `*_FAIL_HEAD` because the command failed;
/// here it succeeded, and naming it `..._FAIL_...` would repeat the act/state
/// lie at the identifier level and mislead the next reader into routing it with
/// the failures. No retry cue: retrying is not merely non-idempotent, it is
/// impossible (the account is gone, so a second Remove never reaches the
/// sweep). `{host}` is filled at the `format!` site — `login` would be wrong,
/// nothing named `login` is left behind.
///
/// Its predecessor `LEGACY_KEYCHAIN_FAIL_HEAD` is RETIRED: all three clauses of
/// "Nothing was changed — the account is still listed, so you can try again"
/// are false under best effort.
const LEGACY_LEFTOVER_HEAD: &str = "The account is no longer listed, but a leftover credential for {host} is still in the OS keychain. Bonsai can't remove it — clear it there by hand if you want it gone. ";
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

/// P114 Addendum A §A.2 option 1 — what a SUCCESSFUL removal reports back.
///
/// A named struct rather than a bare `Option<String>` for two reasons: it
/// matches the sibling forge commands, which all resolve with named types
/// (`ForgeViewer`, `ForgeAccount`), and this payload is the natural place for a
/// second field if the sweep ever reports more than one leftover key.
///
/// The leftover is a FULFILLED value, not an `Err`, because the removal
/// succeeded: an `Err` that means success is counted as a failure by every
/// generic layer above it (obs `ipc.result` / `error` payloads, the DEV toast
/// guard, any retry wrapper) and would keep the Remove dialog open on an
/// account that no longer exists.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForgeRemoveOutcome {
    /// The whole R4 sentence ([`LEGACY_LEFTOVER_HEAD`] + [`CAUSE_LEAD`] + the
    /// cause), or `None` when nothing was left behind. The caller renders it
    /// verbatim as a section-level `--warn` outcome note (P114 rule 1).
    pub leftover: Option<String>,
}

/// P80: delete an account's token (by its `keychain_key`), remove the record, and
/// clean references (promote/clear host default, drop repo overrides).
///
/// Idempotent: a `keychain_key` that is no longer in the keychain is success, so
/// this is safely re-runnable after a partial failure. Deleting the token is the
/// operation — if the keychain refuses, no settings or UI state is changed and
/// the account stays listed. When this is the LAST account on its host, the
/// legacy bare-host key is swept first, BEST EFFORT (audit MEDIUM-2 source B +
/// P114 Addendum A): a refusal there does NOT block the removal, and is
/// reported on the success value as
/// [`ForgeRemoveOutcome::leftover`]. Errors: `other` — the keychain refused the
/// account's credential; the credential was deleted but settings could not be
/// saved; or (unknown / already-removed id, so no credential was touched)
/// settings could not be saved.
#[tauri::command]
pub async fn forge_remove_account(
    app: tauri::AppHandle,
    account_id: String,
) -> Result<ForgeRemoveOutcome, AppError> {
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
) -> Result<ForgeRemoveOutcome, AppError> {
    forge_remove_account_inner_with(settings_file, account_id, RemoveAccountDeps::default()).await
}

/// Dependency-injected core of [`forge_remove_account_inner`].
///
/// Deleting the token IS the operation (user ruling 2026-09-17): if the
/// ACCOUNT's credential is still in the keychain the removal FAILED, so a
/// failing `delete_token` for `keychain_key` returns `Err` and leaves all
/// settings and UI state untouched —
/// no settings record removed, no legacy host mirror dropped, and no viewer
/// eviction (the token is still live, so the cached viewer is still accurate;
/// evicting it would show a connected account as disconnected). The row stays
/// visible and Remove can be retried in place. (`TokenStore::delete`,
/// `crates/bonsai-forge/src/auth.rs:119-122`, does drop its in-process
/// token-cache entry before the keychain call; that is not settings or UI state,
/// is not user-visible, and self-heals on the next lazy warm.)
///
/// The LEGACY bare-host sweep is the one BEST-EFFORT step (P114 Addendum A):
/// its refusal is captured, the removal continues, and it is returned on the
/// `Ok` value AFTER `update_settings` succeeds — never with `?`. That is what
/// makes §A.4's precedence fall out of the control flow: a refused ACCOUNT key
/// (R1), a failed settings write (R2/R3) and a task panic (N2) all still
/// short-circuit and DROP the legacy fact, which is correct — each is the
/// actionable outcome, and the fact is not lost because the record survives, so
/// the retry those messages recommend re-runs the sweep and surfaces R4 on the
/// attempt that finally saves. R4 is therefore returned ONLY from the tail of a
/// fully successful removal, which is precisely what makes its first clause
/// ("The account is no longer listed") true.
pub(crate) async fn forge_remove_account_inner_with(
    settings_file: &Path,
    account_id: String,
    deps: RemoveAccountDeps,
) -> Result<ForgeRemoveOutcome, AppError> {
    let file = settings_file.to_path_buf();
    tauri::async_runtime::spawn_blocking(move || {
        let s = settings::load_from(&file);
        let rec = s
            .forge_accounts
            .iter()
            .find(|a| a.account_id == account_id)
            .cloned();
        // P114 R4 — the best-effort sweep's refusal, reported on the success
        // value once the removal has actually landed (§A.4).
        let mut leftover: Option<String> = None;
        if let Some(r) = &rec {
            // Audit MEDIUM-2 source B, backstop: when this is the LAST account
            // on its host, also sweep the legacy bare-host key. A P79-era token
            // can survive under it — the migration keys a migrated account to
            // the bare host, and a later re-add re-keys the record to the
            // three-part `aid`, abandoning it. Nothing else in the UI names that
            // key, so a credential left there is exactly as orphaned as a
            // per-account one. `NoEntry` folds to `Ok`, so this is a no-op for
            // modern-only hosts.
            //
            // ORDER: attempted BEFORE the account's own key. On this branch
            // the bare-host key is unreferenced BY CONSTRUCTION —
            // `last_on_host` means no sibling record exists, and
            // `keychain_key != r.host` means it is not this record's key
            // either — so deleting it first destroys nothing live, and a
            // refusal here leaves the account's own credential exactly where
            // it was, which is what keeps R1's "Nothing was changed" true when
            // both keys refuse.
            //
            // `last_on_host` checks GLOBALLY, not just same-host: a record on
            // ANOTHER host whose `keychain_key` happens to equal this host's
            // name (reachable only by hand-editing settings.json) must not be
            // clobbered. This mirrors the add path's `superseded` filter.
            let last_on_host = !s.forge_accounts.iter().any(|a| {
                a.account_id != r.account_id && (a.host == r.host || a.keychain_key == r.host)
            });
            // BEST EFFORT: `.err()`, never `?`. The message is rendered here
            // while `r.host` is in scope, and carried to the tail.
            leftover = if last_on_host && r.keychain_key != r.host {
                (deps.delete_token)(&r.host).err().map(|e| {
                    format!(
                        "{}{CAUSE_LEAD}{e}",
                        LEGACY_LEFTOVER_HEAD.replace("{host}", &r.host)
                    )
                })
            } else {
                None
            };
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
        Ok(ForgeRemoveOutcome { leftover })
    })
    .await
    .map_err(|e| AppError::Other(format!("{TASK_JOIN_FAIL_HEAD}{CAUSE_LEAD}{e}")))?
}

#[cfg(test)]
#[path = "forge_remove_account_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "forge_remove_account_legacy_tests.rs"]
mod legacy_tests;
