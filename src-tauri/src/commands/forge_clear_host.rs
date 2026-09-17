//! Sign-out-of-a-HOST — the runtime-free core of what WAS the
//! `forge_clear_token_for_host` command.
//!
//! DORMANT (security audit INFO-2, 2026-09-17): the `#[tauri::command]` wrapper
//! was removed because per-account removal (`forge_remove_account`, P80)
//! replaced its only UI caller, and a credential-deleting command with zero
//! callers is an unnecessary privileged surface. The logic and its tests are
//! kept intact so it is provably correct if the host-wide sign-out is ever
//! rewired: re-add `#[tauri::command] pub async fn forge_clear_token_for_host`
//! over [`forge_clear_token_for_host_inner`], plus the `generate_handler!`,
//! `IpcApi`, mock and `obs` allow-list entries.
//!
//! Split out of `forge_accounts.rs` (CLAUDE.md file-size discipline) for the
//! same reason `forge_remove_account.rs` was: this is the other command in that
//! layer whose side effects (N+1 OS-keychain deletes and the settings write)
//! must be failable in tests.
//!
//! Security-audit MEDIUM-1 (2026-09-17): the previous loop did
//! `for a in &on_host { let _ = delete_token(&a.keychain_key); }` and then
//! dropped every record regardless. If the keychain refused k of N deletes, the
//! UI reported a clean sign-out while k PATs stayed live with NO record left
//! naming their `keychain_key` — unreachable from the UI forever. The user ruled
//! this takes the same shape as `forge_remove_account`: see
//! [`forge_clear_token_for_host_inner_with`] for the three outcomes.

use std::path::Path;

use super::shared::*;

/// Deletes the token stored under one keychain key.
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
/// Three heads name the host, so they carry a literal `{host}` placeholder
/// filled by [`with_host`] (`format!` cannot take a const as its template).
///
/// Consts so ONE Rust definition is the source of truth and the cross-language
/// guard (`forge_clear_host_tests::mock_copy_mirrors_the_rust_copy`) can assert
/// the harness mirror in `src/ipc/mock/handlers/forgeClearHostFailure.ts`
/// contains each whole HEAD (with `{host}` rendered as the TS `${host}`) —
/// editing the copy here without editing the mock turns that test red.
const CAUSE_LEAD: &str = "Details: ";
const KEYCHAIN_FAIL_HEAD: &str = "Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. ";
/// The empty-`on_host` variant of the keychain refusal: with no account on the
/// host only the legacy bare-host delete can be refused, and then there are NO
/// accounts listed — so the standard head would assert a false UI fact (the
/// same overstatement `e583f11` existed to remove). P114 C5 makes this its own
/// head rather than a suffix swap so the sentence can start with the thing that
/// actually failed and can name the host (it also said its failure twice).
const KEYCHAIN_FAIL_NO_ACCOUNT_HEAD: &str = "A leftover credential for {host} couldn't be removed from the OS keychain. Nothing was changed, so you can try again. ";
/// Separator between the causes of multiple refused deletes. A named const so
/// the cross-language guard covers it too (a bare literal could drift out of
/// sync with the mock's joined message unnoticed).
const KEYCHAIN_FAIL_JOIN: &str = "; ";
/// P114 rule 2 (state, not act) and its plural-truth check: this branch is
/// reachable only when every delete — including the legacy bare-host key —
/// succeeded, so "the credentials" (all of them) is true here.
const SETTINGS_FAIL_HEAD: &str = "The credentials are no longer in the OS keychain, but the account list couldn't be saved. Try again to finish signing out of {host}. ";
/// The empty-`on_host` variant of the settings-save failure: no account named a
/// credential, so the message must NOT claim credentials were removed (the
/// `e583f11` lesson — that exact false claim was the bug fixed in the sibling).
/// Deliberately NOT byte-identical to the sibling's R3 any more: the action
/// differs ("signing out of {host}" vs "removing it"), so do not de-duplicate
/// these consts across the two files.
const SETTINGS_FAIL_NO_CREDENTIAL_HEAD: &str =
    "The account list couldn't be saved. Try again to finish signing out of {host}. ";

/// Fills a head's `{host}` placeholder. The placeholder (rather than a
/// prefix/suffix pair) keeps each head ONE contiguous literal, which is what
/// lets the cross-language guard match a whole message instead of two halves
/// that could come from different strings.
fn with_host(head: &str, host: &str) -> String {
    head.replace("{host}", host)
}

/// Injectable side effects of [`forge_clear_token_for_host_inner`]. The real
/// implementations live in [`Default`]; tests substitute failing ones because
/// the keychain is process-global and `settings::update` cannot be made to fail
/// portably from the outside.
pub(crate) struct ClearHostDeps {
    /// Delete the token stored under one keychain key. A key that is NOT in the
    /// keychain is `Ok(())` (`crates/bonsai-forge/src/auth.rs:53` folds
    /// `keyring::Error::NoEntry` into success), which is what keeps this command
    /// re-runnable after a partial failure.
    pub delete_token: DeleteTokenFn,
    /// Load→mutate→save settings as one transaction (keeps `settings::update`'s
    /// process-wide IO lock rather than re-implementing it here).
    pub update_settings: UpdateSettingsFn,
}

impl Default for ClearHostDeps {
    fn default() -> Self {
        Self {
            delete_token: Box::new(bonsai_forge::delete_token),
            update_settings: Box::new(|file, mutate| {
                settings::update(file, |s| mutate(s)).map(|_| ())
            }),
        }
    }
}

/// P79 (retained): sign out ALL accounts on `host` — delete each account's
/// keychain entry plus the legacy bare-host entry, then drop the records whose
/// keys were deleted, the host default, and any overrides pointing at them.
///
/// Idempotent: a key that is no longer in the keychain is success, so this is
/// safely re-runnable after a partial failure. Deleting the tokens IS the
/// operation — if the keychain refuses ANY of them, nothing is changed and the
/// accounts stay listed. Errors: `other` — the keychain refused one or more
/// deletes (when the host has no accounts listed the copy says so instead of
/// claiming the rows stay listed); the credentials were deleted but settings
/// could not be saved; or
/// (no accounts on the host, so no credential was named) settings could not be
/// saved.
// DORMANT: the only thing that called this was the removed `#[tauri::command]`
// wrapper, and the 12 tests all drive `_inner_with` directly so they can inject
// failures. Kept as the documented rewire entry point — it is the one line a
// restored wrapper calls. Allow is scoped to this fn on purpose: a module-wide
// one would also hide future dead code in a credential-deleting module.
#[allow(dead_code)]
pub(crate) async fn forge_clear_token_for_host_inner(
    settings_file: &Path,
    host: String,
) -> Result<(), AppError> {
    forge_clear_token_for_host_inner_with(settings_file, host, ClearHostDeps::default()).await
}

/// Dependency-injected core of [`forge_clear_token_for_host_inner`].
pub(crate) async fn forge_clear_token_for_host_inner_with(
    settings_file: &Path,
    host: String,
    deps: ClearHostDeps,
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
        // Audit MEDIUM-1: attempt EVERY key (the N accounts plus the legacy
        // bare-host entry) and collect the refusals rather than swallowing them.
        // All are attempted even after the first failure so one retry sweeps
        // whatever it can and the user sees every cause at once.
        //
        // The legacy bare-host key is deleted through the same seam as the
        // accounts, and its refusal blocks identically: it is the ONLY path that
        // ever sweeps bare-host entries, so a PAT left there is exactly as
        // orphaned as a per-account one — and nothing else in the UI names it.
        // (This is why the deleted `bonsai_forge::clear_token_for_host` was not
        // used, and why it was removed outright:
        // it bundles `evict_viewer` into the delete, and evicting the cached
        // viewer on a failure path would show a still-connected account as
        // disconnected. The viewer eviction now happens only on success.)
        //
        // Audit INFO-1: a MIGRATED legacy account carries
        // `keychain_key == <bare host>`, so the account chain and the legacy
        // sweep can name the SAME key. `delete_token` is idempotent, but on
        // refusal the joined causes would show the identical cause twice — which
        // the user reads. Dedup first, preserving order.
        let mut keys: Vec<&str> = Vec::with_capacity(on_host.len() + 1);
        for key in on_host
            .iter()
            .map(|a| a.keychain_key.as_str())
            .chain(std::iter::once(host_l.as_str()))
        {
            if !keys.contains(&key) {
                keys.push(key);
            }
        }
        let mut failures: Vec<String> = Vec::new();
        for key in keys {
            if let Err(e) = (deps.delete_token)(key) {
                failures.push(e.to_string());
            }
        }
        if !failures.is_empty() {
            // Nothing is mutated: no record dropped, no default or override
            // cleared, no legacy mirror removed, no viewer evicted. The host
            // stays listed, so the sign-out is retryable in place.
            // Which head is TRUE depends on whether anything IS listed: with
            // an empty `on_host` the host has no rows at all, so claiming "the
            // accounts are still listed" would be the false-UI-fact bug again.
            let head = if on_host.is_empty() {
                with_host(KEYCHAIN_FAIL_NO_ACCOUNT_HEAD, &host_l)
            } else {
                KEYCHAIN_FAIL_HEAD.to_string()
            };
            return Err(AppError::Other(format!(
                "{head}{CAUSE_LEAD}{}",
                failures.join(KEYCHAIN_FAIL_JOIN)
            )));
        }
        bonsai_forge::invalidate_viewer(&host_l);
        // Whether any account NAMED a credential decides which settings-save
        // failure message is TRUE: with an empty `on_host` only the best-effort
        // legacy bare-host sweep ran, so the asymmetric wording would be a false
        // claim (the `e583f11` bug, fixed in the per-account sibling).
        let had_credentials = !on_host.is_empty();
        let ids: Vec<String> = on_host.iter().map(|a| a.account_id.clone()).collect();
        (deps.update_settings)(&file, &mut |s| {
            // Audit LOW-1: keyed by the ids whose keychain keys were actually
            // deleted, NOT by host. The read at the top of this closure's
            // caller happens outside `settings::update`'s IO lock, so a
            // concurrent `forge_add_account` on the same host can land in
            // between — and it stores the PAT BEFORE its settings write. A
            // host-keyed retain would drop that record while its key was never
            // in the delete set: a live PAT with nothing naming its
            // `keychain_key` (the MEDIUM-1 state, reached by a race).
            s.forge_accounts.retain(|a| !ids.contains(&a.account_id));
            s.forge_host_defaults.retain(|d| d.host != host_l);
            s.repo_forge_overrides
                .retain(|o| !ids.contains(&o.account_id));
            settings::remove_forge_host(s, &host_l);
        })
        .map_err(|e| {
            let head = with_host(
                if had_credentials {
                    SETTINGS_FAIL_HEAD
                } else {
                    SETTINGS_FAIL_NO_CREDENTIAL_HEAD
                },
                &host_l,
            );
            AppError::Other(format!("{head}{CAUSE_LEAD}{e}"))
        })?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

#[cfg(test)]
#[path = "forge_clear_host_tests.rs"]
mod tests;
