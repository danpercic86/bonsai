//! Runtime-free tests for `forge_remove_account_inner` — one per user-ruled
//! outcome (2026-09-17): a failing keychain delete changes nothing, a
//! not-in-keychain key is success, and a failing settings save is reported with
//! the credential-is-gone asymmetry. Uses the `_with` seam so the real OS
//! keychain is never touched (and no tauri "test" feature is needed).

use super::*;
use bonsai_forge::ForgeKind;

pub(super) fn acct(id: &str, host: &str) -> settings::ForgeAccountRecord {
    settings::ForgeAccountRecord {
        account_id: id.to_string(),
        keychain_key: id.to_string(),
        host: host.to_string(),
        kind: ForgeKind::GitHub,
        login: Some("octocat".to_string()),
        avatar_url: None,
    }
}

/// Seed a settings file holding one GitHub account, in a fresh temp dir.
pub(super) fn seed(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("settings.json");
    let mut s = settings::Settings::default();
    settings::upsert_forge_account(&mut s, acct("a", "github.com"));
    settings::upsert_forge_host(
        &mut s,
        "github.com",
        ForgeKind::GitHub,
        Some("octocat".into()),
    );
    settings::save_to(&file, &s).expect("seed settings");
    file
}

/// Scratch root under `D:\Data\Temp\bonsai-scratch` on Windows (MEMORY rule —
/// never C:, which is critically full). Mirrors
/// `src-tauri/src/scheduler/test_support.rs`; on macOS/Linux there is no such
/// constraint, so it falls back to the OS temp dir.
#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from("D:\\Data\\Temp\\bonsai-scratch")
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// RAII scratch dir: `TempDir`'s `Drop` cleans up even when an assert panics
/// (the earlier explicit `remove_dir_all` after the asserts leaked on failure).
pub(super) fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-remove-acct-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// Records every key `delete_token` is asked to remove, in order.
pub(super) fn recording_deletes(
    log: &std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    refuse: Option<&'static str>,
) -> DeleteTokenFn {
    let log = std::sync::Arc::clone(log);
    Box::new(move |key| {
        log.lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(key.to_string());
        match refuse {
            Some(k) if k == key => Err(AppError::Other("access denied".into())),
            _ => Ok(()),
        }
    })
}
/// Outcome 2: the key is not in the keychain (`delete_token` returns `Ok(())`
/// for that case — `crates/bonsai-forge/src/auth.rs:53`), so the removal
/// SUCCEEDS and the record + legacy host mirror are cleaned.
#[test]
fn missing_key_is_success_and_removes_the_record() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: Box::new(|_| Ok(())),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    let s = settings::load_from(&file);
    assert!(s.forge_accounts.is_empty());
    assert!(s.forge_hosts.is_empty(), "legacy mirror dropped");
}

/// Outcome 1: the keychain refuses the ACCOUNT's own key ⇒ `Err`, and NOTHING
/// else changed — the record and the legacy host mirror both survive so the row
/// stays visible and Remove is retryable.
///
/// Refuses ONLY `"a"`, not every key: since the source-B reorder the bare-host
/// sweep is attempted first, and a blanket refusal would report the LEGACY
/// outcome instead. That is the distinction this test exists to pin, so it must
/// fail the one key it is about.
#[test]
fn failing_delete_errors_and_changes_nothing() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let log = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: recording_deletes(&log, Some("a")),
            ..RemoveAccountDeps::default()
        },
    ));
    let e = r.expect_err("must fail");
    let msg = e.to_string();
    assert_eq!(
        msg,
        "Couldn't remove the account's credential from the OS keychain. Nothing was changed — the account is still listed, so you can try again. Details: access denied"
    );
    assert_eq!(
        log.lock().unwrap_or_else(|p| p.into_inner()).clone(),
        vec!["github.com".to_string(), "a".to_string()],
        "the unreferenced leftover was swept first, then the refused account key"
    );
    let s = settings::load_from(&file);
    assert_eq!(s.forge_accounts.len(), 1, "record must survive");
    assert_eq!(s.forge_hosts.len(), 1, "legacy mirror must survive");
}

/// Outcome 3: the credential IS gone but settings could not be saved ⇒ `Err`
/// with a message distinguishable from outcome 1 (it must not claim nothing
/// changed).
#[test]
fn failing_settings_save_reports_the_asymmetry() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let deleted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let seen = std::sync::Arc::clone(&deleted);
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "a".to_string(),
        RemoveAccountDeps {
            delete_token: Box::new(move |_| {
                seen.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }),
            update_settings: Box::new(|_, _| Err(AppError::Io("disk full".into()))),
        },
    ));
    let e = r.expect_err("must fail");
    assert!(
        deleted.load(std::sync::atomic::Ordering::SeqCst),
        "the token delete must have run first"
    );
    assert_eq!(
        e.to_string(),
        "The credential is no longer in the OS keychain, but the account list couldn't be saved. Try again to finish removing it. Details: io error: disk full"
    );
}

/// An unknown `account_id` still runs the settings pass (a no-op) and succeeds —
/// the delete seam is never consulted because there is no `keychain_key`.
#[test]
fn unknown_account_is_ok() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "nope".to_string(),
        RemoveAccountDeps {
            delete_token: Box::new(|_| Err(AppError::Other("must not be called".into()))),
            ..RemoveAccountDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert_eq!(settings::load_from(&file).forge_accounts.len(), 1);
}

/// Outcome 3, the `rec == None` half: no credential was ever touched (the
/// delete seam is not consulted for an unknown / already-removed id), so a
/// failing settings save must NOT claim one was deleted. This is the
/// double-click / retry-after-partial path ruling 1's "you can try again"
/// depends on, which is why a false claim here would be the worst one.
#[test]
fn unknown_account_settings_fail_does_not_claim_a_credential_was_removed() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let r = tauri::async_runtime::block_on(forge_remove_account_inner_with(
        &file,
        "nope".to_string(),
        RemoveAccountDeps {
            delete_token: Box::new(|_| Err(AppError::Other("must not be called".into()))),
            update_settings: Box::new(|_, _| Err(AppError::Io("disk full".into()))),
        },
    ));
    let msg = r.expect_err("must fail").to_string();
    assert_eq!(
        msg,
        "The account list couldn't be saved. Try again to finish removing it. Details: io error: disk full"
    );
    assert!(
        !msg.contains("keychain"),
        "nothing was removed from the keychain here: {msg}"
    );
}

/// The mock's source with COMMENT lines stripped. The previous guard searched
/// the whole file, so prose quoting a message could mask a real drift; a
/// literal must now appear in code.
fn mock_code(src: &str) -> String {
    src.lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The cross-language copy guard. `src/ipc/mock/handlers/forgeRemoveFailure.ts`
/// claims its strings are the backend's output VERBATIM, but Rust and TS share
/// no constant, so nothing else makes editing one side red (the `2a0b8f1`
/// class of defect: a comment asserting a guarantee it does not provide).
///
/// P114 hardening: each message's ENTIRE fixed text (`HEAD` + [`CAUSE_LEAD`],
/// since the cause is last) must appear in the mock's code as ONE backtick
/// literal, exactly once. The old shape checked a prefix and a suffix
/// separately, which could in principle be satisfied by halves of two
/// DIFFERENT strings, and searched comments too. Change the copy here and this
/// test fails naming the const the mock is missing.
#[test]
fn mock_copy_mirrors_the_rust_copy() {
    const MOCK: &str = include_str!("../../../src/ipc/mock/handlers/forgeRemoveFailure.ts");
    let code = mock_code(MOCK);
    // Owned before the loop so the array can hold `&str` uniformly.
    let legacy = LEGACY_LEFTOVER_HEAD.replace("{host}", "${LEGACY_HOST}");
    for (name, head) in [
        ("KEYCHAIN_FAIL_HEAD", KEYCHAIN_FAIL_HEAD),
        (
            "LEGACY_LEFTOVER_HEAD",
            // The mock fills the `{host}` placeholder from its own
            // `LEGACY_HOST` const, so the guard compares the same rendered
            // shape (the `forge_clear_host` convention, whose mock uses
            // `${host}`).
            legacy.as_str(),
        ),
        ("SETTINGS_FAIL_HEAD", SETTINGS_FAIL_HEAD),
        (
            "SETTINGS_FAIL_NO_CREDENTIAL_HEAD",
            SETTINGS_FAIL_NO_CREDENTIAL_HEAD,
        ),
        ("CONFIG_DIR_FAIL_HEAD", CONFIG_DIR_FAIL_HEAD),
        ("TASK_JOIN_FAIL_HEAD", TASK_JOIN_FAIL_HEAD),
    ] {
        let literal = format!("`{head}{CAUSE_LEAD}`");
        assert_eq!(
            code.matches(literal.as_str()).count(),
            1,
            "forgeRemoveFailure.ts must contain the whole {name} message text exactly once, as the code literal {literal:?} — update the mock (its header promises verbatim backend text)"
        );
    }
}
