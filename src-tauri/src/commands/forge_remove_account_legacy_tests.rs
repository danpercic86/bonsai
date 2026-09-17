//! Legacy bare-host sweep tests for `forge_remove_account` (audit MEDIUM-2
//! source B). Split out of `forge_remove_account_tests.rs` when the
//! best-effort ruling (P114 Addendum A, 2026-09-17) added the R4 outcome and
//! its combination cases, which pushed that file past the ~500-line limit.
//!
//! Shares the seed/recorder helpers with its sibling module rather than
//! cloning them: a second copy of the call recorder is how two suites start
//! disagreeing about what "the same deps" mean.

use super::tests::{acct, recording_deletes, scratch_dir, seed};
use super::*;
use bonsai_forge::ForgeKind;

/// Seed a settings file holding one MIGRATED LEGACY account: its `keychain_key`
/// IS the bare host, which is the shape `migrate_forge_hosts_to_accounts`
/// produces (`src-tauri/src/settings/forge_accounts.rs`).
fn seed_legacy(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("settings.json");
    let mut s = settings::Settings::default();
    let mut rec = acct("a", "github.com");
    rec.keychain_key = "github.com".to_string();
    settings::upsert_forge_account(&mut s, rec);
    settings::upsert_forge_host(
        &mut s,
        "github.com",
        ForgeKind::GitHub,
        Some("octocat".into()),
    );
    settings::save_to(&file, &s).expect("seed settings");
    file
}

/// Audit MEDIUM-2 source B, backstop: removing the LAST account on a host also
/// sweeps the legacy bare-host key, so a P79-era credential abandoned by a
/// re-add cannot survive with no record naming it.
#[test]
fn removing_the_last_account_on_a_host_also_sweeps_the_bare_host_key() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, None),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert_eq!(
        log.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        vec!["github.com".to_string(), "a".to_string()],
        "the legacy bare-host key FIRST, then the account key — that order is          what makes both refusal messages true (the bare-host key is          unreferenced on this branch, so losing it destroys nothing live,          while the account's own credential is still there to be reported as          unchanged)"
    );
    assert!(settings::load_from(&file).forge_accounts.is_empty());
}

/// The dedup: a STILL-LEGACY record's `keychain_key` already IS the host, so
/// the sweep must not ask for the same key twice (the audit INFO-1 noise).
#[test]
fn a_legacy_record_is_not_deleted_twice() {
    let dir = scratch_dir();
    let file = seed_legacy(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, None),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert_eq!(
        log.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        vec!["github.com".to_string()]
    );
}

/// The backstop does NOT fire while another account remains on the host: the
/// bare-host key could still back that sibling (it does, for a migrated one).
#[test]
fn the_bare_host_key_is_kept_while_a_sibling_account_remains() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    settings::update(&file, |s| {
        settings::upsert_forge_account(s, acct("b", "github.com"))
    })
    .expect("add sibling");
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, None),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert_eq!(
        log.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        vec!["a".to_string()]
    );
}

/// Item 6 / the add path's `superseded` filter, mirrored: `last_on_host` checks
/// GLOBALLY. A record on ANOTHER host whose `keychain_key` happens to equal
/// this host's name (reachable only by hand-editing settings.json) still names
/// that key, so the sweep must not fire and clobber its credential.
#[test]
fn a_foreign_record_naming_this_host_as_its_key_blocks_the_sweep() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    settings::update(&file, |s| {
        let mut foreign = acct("b", "gitlab.example.com");
        foreign.keychain_key = "github.com".to_string();
        settings::upsert_forge_account(s, foreign);
    })
    .expect("add foreign record");
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, None),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert_eq!(
        log.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        vec!["a".to_string()],
        "only the account's own key: `github.com` is still named by the record on the other host"
    );
}

/// THE RULING (P114 Addendum A): a refused LEGACY orphan delete NO LONGER
/// blocks the removal. Fail-closed could permanently strand a user — on a
/// retry the account's own token is already gone (`NoEntry → Ok`) while the
/// legacy key refuses again, leaving a listed, disconnected, UNREMOVABLE
/// account fixable only by hand in the OS credential store. So the removal
/// completes and the leftover is REPORTED as data.
///
/// RED against HEAD (`61af79b`), whose body returned `Err` here (this file's
/// helpers and `RemoveAccountDeps` both exist there, so it is red vs HEAD
/// proper, not vs a transplant).
#[test]
fn a_refused_legacy_sweep_no_longer_blocks_the_removal() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, Some("github.com")),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "the removal must SUCCEED, got {r:?}");
    assert_eq!(
        log.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        vec!["github.com".to_string(), "a".to_string()],
        "the refused sweep must not skip the account's own key"
    );
    let s = settings::load_from(&file);
    assert!(s.forge_accounts.is_empty(), "the record must be gone");
    assert!(s.forge_hosts.is_empty(), "legacy mirror dropped");
}

/// The R4 payload: the leftover is carried on the FULFILLED value, so a
/// success is typed as a success (§A.2 option 1), and its copy is the signed
/// `LEGACY_LEFTOVER_HEAD` with `{host}` filled and the cause last.
#[test]
fn the_leftover_is_reported_on_the_ok_value_with_the_signed_copy() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let out = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, Some("github.com")),
            ..RemoveAccountDeps::default()
        },
    ))
    .expect("removal succeeds");
    assert_eq!(
        out.leftover.as_deref(),
        Some(
            "The account is no longer listed, but a leftover credential for github.com is still in the OS keychain. Bonsai can't remove it — clear it there by hand if you want it gone. Details: access denied"
        )
    );
}

/// The other half of the payload pin: a clean removal reports NO leftover, so
/// the caller never renders a warn note for a wholly successful remove.
#[test]
fn a_clean_removal_reports_no_leftover() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let out = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, None),
            ..RemoveAccountDeps::default()
        },
    ))
    .expect("removal succeeds");
    assert_eq!(out.leftover, None);
}

/// §A.4 precedence, case 1 — legacy refused AND the account's own key refused:
/// **R1 wins**. It is the actionable outcome and its "Nothing was changed — the
/// account is still listed" is still true, because the settings write never ran.
///
/// RED against HEAD (`61af79b`), whose body short-circuited on the legacy key and
/// reported the LEGACY head instead.
#[test]
fn a_blanket_refusal_reports_r1_not_the_leftover() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: Box::new(|_| Err(AppError::Other("access denied".into()))),
            ..RemoveAccountDeps::default()
        },
    ));
    assert_eq!(
        r.expect_err("must fail").to_string(),
        "Couldn't remove the account's credential from the OS keychain. Nothing was changed — the account is still listed, so you can try again. Details: access denied"
    );
    let s = settings::load_from(&file);
    assert_eq!(s.forge_accounts.len(), 1, "record must survive");
    assert_eq!(s.forge_hosts.len(), 1, "legacy mirror must survive");
}

/// §A.4 precedence, case 2 — legacy refused AND the settings write failed:
/// **R2 wins**. It is the actionable one, and the legacy fact is not lost: the
/// record survives, so the retry R2 recommends re-runs the sweep and surfaces
/// R4 on the attempt that finally saves.
///
/// RED against HEAD (`61af79b`), whose body reported the LEGACY head and never
/// reached the settings write.
#[test]
fn legacy_refused_and_settings_failed_reports_r2_and_keeps_the_record() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, Some("github.com")),
            update_settings: Box::new(|_, _| Err(AppError::Other("access denied".into()))),
        },
    ));
    assert_eq!(
        r.expect_err("must fail").to_string(),
        "The credential is no longer in the OS keychain, but the account list couldn't be saved. Try again to finish removing it. Details: access denied"
    );
    assert_eq!(
        settings::load_from(&file).forge_accounts.len(),
        1,
        "the record survives, so the recommended retry can re-run the sweep"
    );
}
