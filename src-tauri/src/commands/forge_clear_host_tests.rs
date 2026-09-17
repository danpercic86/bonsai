//! Runtime-free tests for `forge_clear_token_for_host_inner` — one per user-ruled
//! outcome (2026-09-17, security-audit MEDIUM-1): a keychain that refuses ANY of
//! the N+1 deletes changes nothing, a retry after such a failure completes, and
//! a failing settings save is reported with the credentials-are-gone asymmetry.
//! Uses the `_with` seam so the real OS keychain is never touched (and no tauri
//! "test" feature is needed — it crashes with `STATUS_ENTRYPOINT_NOT_FOUND`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::*;
use bonsai_forge::ForgeKind;

fn acct(id: &str, host: &str) -> settings::ForgeAccountRecord {
    settings::ForgeAccountRecord {
        account_id: id.to_string(),
        keychain_key: id.to_string(),
        host: host.to_string(),
        kind: ForgeKind::GitHub,
        login: Some(id.to_string()),
        avatar_url: None,
    }
}

/// Seed a settings file holding TWO github.com accounts (plus a host default, a
/// repo override pointing at one of them, and the legacy host mirror) and one
/// unrelated gitlab.com account that must never be touched.
fn seed(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("settings.json");
    let mut s = settings::Settings::default();
    settings::upsert_forge_account(&mut s, acct("a", "github.com"));
    settings::upsert_forge_account(&mut s, acct("b", "github.com"));
    settings::upsert_forge_account(&mut s, acct("g", "gitlab.com"));
    settings::set_host_default(&mut s, "github.com", "a");
    settings::set_repo_override(&mut s, "D:\\repo", "b");
    settings::upsert_forge_host(&mut s, "github.com", ForgeKind::GitHub, Some("a".into()));
    settings::save_to(&file, &s).expect("seed settings");
    file
}

/// Scratch root under `D:\Data\Temp\bonsai-scratch` on Windows (MEMORY rule —
/// never C:, which is critically full). Mirrors `forge_remove_account_tests.rs`;
/// on macOS/Linux there is no such constraint, so it falls back to the OS temp.
#[cfg(windows)]
fn scratch_root() -> std::path::PathBuf {
    std::path::PathBuf::from("D:\\Data\\Temp\\bonsai-scratch")
}

#[cfg(not(windows))]
fn scratch_root() -> std::path::PathBuf {
    std::env::temp_dir().join("bonsai-scratch")
}

/// RAII scratch dir: `TempDir`'s `Drop` cleans up even when an assert panics.
fn scratch_dir() -> tempfile::TempDir {
    let root = scratch_root();
    std::fs::create_dir_all(&root).expect("create scratch root");
    tempfile::Builder::new()
        .prefix("bonsai-clear-host-")
        .tempdir_in(&root)
        .expect("scratch dir")
}

/// A `delete_token` seam that records every key it saw and fails the ones in
/// `fail`.
fn recording_delete(seen: Arc<Mutex<Vec<String>>>, fail: &'static [&'static str]) -> DeleteTokenFn {
    Box::new(move |k: &str| {
        if let Ok(mut v) = seen.lock() {
            v.push(k.to_string());
        }
        if fail.contains(&k) {
            Err(AppError::Other(format!("access denied for {k}")))
        } else {
            Ok(())
        }
    })
}

/// THE central test (audit MEDIUM-1): k of N deletes refused. The old loop
/// swallowed every failure and dropped all records anyway, orphaning k live
/// PATs with nothing left naming their `keychain_key`. Nothing may change.
#[test]
fn partial_delete_failure_errors_and_changes_nothing() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let wrote = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&wrote);
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "GitHub.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &["b"]),
            update_settings: Box::new(move |_, _| {
                flag.store(true, Ordering::SeqCst);
                Ok(())
            }),
        },
    ));
    let msg = r
        .expect_err("a refused delete must fail the sign-out")
        .to_string();
    assert_eq!(
        msg,
        "Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. Details: access denied for b"
    );
    assert!(
        !wrote.load(Ordering::SeqCst),
        "no settings write may be attempted when a delete failed"
    );
    // Every key was still ATTEMPTED, so one retry can sweep the rest.
    let keys = seen.lock().expect("seen").clone();
    assert_eq!(keys, vec!["b", "a", "github.com"]);
    let s = settings::load_from(&file);
    assert_eq!(s.forge_accounts.len(), 3, "no record may be dropped");
    assert_eq!(s.forge_host_defaults.len(), 1, "host default must survive");
    assert_eq!(s.repo_forge_overrides.len(), 1, "override must survive");
    assert_eq!(s.forge_hosts.len(), 1, "legacy mirror must survive");
}

/// A refused delete of the LEGACY bare-host key alone also blocks: it is the
/// only path that ever sweeps bare-host entries, so a live PAT there is exactly
/// as orphaned as a per-account one.
#[test]
fn bare_host_delete_failure_alone_blocks() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &["github.com"]),
            ..ClearHostDeps::default()
        },
    ));
    let msg = r.expect_err("must fail").to_string();
    assert_eq!(
        msg,
        "Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. Details: access denied for github.com"
    );
    assert_eq!(settings::load_from(&file).forge_accounts.len(), 3);
}

/// Every cause is reported, not just the first — one retry, all the reasons.
#[test]
fn all_delete_failures_are_reported() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &["a", "github.com"]),
            ..ClearHostDeps::default()
        },
    ));
    assert_eq!(
        r.expect_err("must fail").to_string(),
        "Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. Details: access denied for a; access denied for github.com"
    );
}

/// All deletes succeed (which INCLUDES the not-in-keychain case — `delete_token`
/// folds `keyring::Error::NoEntry` into `Ok(())`,
/// `crates/bonsai-forge/src/auth.rs:53`) ⇒ proceed exactly as before: records,
/// default, override, and legacy mirror for that host all go; other hosts stay.
#[test]
fn all_deletes_succeed_clears_the_host() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &[]),
            ..ClearHostDeps::default()
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert_eq!(
        seen.lock().expect("seen").clone(),
        vec!["b", "a", "github.com"],
        "both accounts plus the legacy bare-host key"
    );
    let s = settings::load_from(&file);
    assert_eq!(s.forge_accounts.len(), 1, "only gitlab.com remains");
    assert_eq!(s.forge_accounts[0].host, "gitlab.com");
    assert!(s.forge_host_defaults.is_empty());
    assert!(s.repo_forge_overrides.is_empty());
    assert!(s.forge_hosts.is_empty(), "legacy mirror dropped");
}

/// The retry ruling 1 buys: attempt 1 refuses `b` and changes nothing, attempt 2
/// (keychain unlocked) succeeds end to end. This is the harness
/// `?forgeClearHostFail=keychain-then-ok` seam in Rust.
#[test]
fn retry_after_a_partial_failure_completes() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let first = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::new(Mutex::new(Vec::new())), &["b"]),
            ..ClearHostDeps::default()
        },
    ));
    assert!(first.is_err());
    let second = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: Box::new(|_| Ok(())),
            ..ClearHostDeps::default()
        },
    ));
    assert!(second.is_ok(), "got {second:?}");
    let s = settings::load_from(&file);
    assert_eq!(s.forge_accounts.len(), 1, "only gitlab.com remains");
}

/// Outcome 3: the credentials ARE gone but settings could not be saved ⇒ `Err`
/// with copy distinguishable from the delete-failure case (it must not claim
/// nothing changed).
#[test]
fn failing_settings_save_reports_the_asymmetry() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &[]),
            update_settings: Box::new(|_, _| Err(AppError::Io("disk full".into()))),
        },
    ));
    assert_eq!(
        r.expect_err("must fail").to_string(),
        "The credentials are no longer in the OS keychain, but the account list couldn't be saved. Try again to finish signing out of github.com. Details: io error: disk full"
    );
    assert_eq!(
        seen.lock().expect("seen").len(),
        3,
        "all deletes ran before the save"
    );
}

/// Outcome 3', the empty-`on_host` half: no account on the host named a
/// credential, so a failing settings save must NOT claim credentials were
/// removed. (`e583f11` fixed exactly this false claim in the sibling; the
/// best-effort legacy bare-host sweep is not a named credential.)
#[test]
fn empty_host_settings_fail_does_not_claim_credentials_were_removed() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "bitbucket.org".to_string(),
        ClearHostDeps {
            delete_token: Box::new(|_| Ok(())),
            update_settings: Box::new(|_, _| Err(AppError::Io("disk full".into()))),
        },
    ));
    let msg = r.expect_err("must fail").to_string();
    assert_eq!(
        msg,
        "The account list couldn't be saved. Try again to finish signing out of bitbucket.org. Details: io error: disk full"
    );
    assert!(
        !msg.contains("keychain"),
        "nothing named a credential here: {msg}"
    );
}

/// A host with no accounts is plain success, and the settings pass still runs
/// (it prunes nothing, but the command stays idempotent).
#[test]
fn empty_host_is_ok() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let wrote = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&wrote);
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "bitbucket.org".to_string(),
        ClearHostDeps {
            delete_token: Box::new(|_| Ok(())),
            update_settings: Box::new(move |_, _| {
                flag.store(true, Ordering::SeqCst);
                Ok(())
            }),
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    assert!(wrote.load(Ordering::SeqCst), "the settings pass still runs");
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

/// The cross-language copy guard, mirroring
/// `forge_remove_account_tests::mock_copy_mirrors_the_rust_copy`:
/// `src/ipc/mock/handlers/forgeClearHostFailure.ts` claims its strings are the
/// backend's output VERBATIM, but Rust and TS share no constant, so nothing
/// else makes editing one side red.
///
/// P114 hardening: each message's ENTIRE fixed text (`HEAD` + [`CAUSE_LEAD`])
/// must appear in the mock's code as ONE backtick literal, exactly once — not
/// a prefix and a suffix, which two DIFFERENT strings could satisfy between
/// them, and not anywhere in a comment. Heads that name the host are matched
/// with `{host}` rendered as the TS `${host}`, which also pins the
/// interpolation point.
#[test]
fn mock_copy_mirrors_the_rust_copy() {
    const MOCK: &str = include_str!("../../../src/ipc/mock/handlers/forgeClearHostFailure.ts");
    let code = mock_code(MOCK);
    for (name, head) in [
        ("KEYCHAIN_FAIL_HEAD", KEYCHAIN_FAIL_HEAD),
        (
            "KEYCHAIN_FAIL_NO_ACCOUNT_HEAD",
            KEYCHAIN_FAIL_NO_ACCOUNT_HEAD,
        ),
        ("SETTINGS_FAIL_HEAD", SETTINGS_FAIL_HEAD),
        (
            "SETTINGS_FAIL_NO_CREDENTIAL_HEAD",
            SETTINGS_FAIL_NO_CREDENTIAL_HEAD,
        ),
    ] {
        let literal = format!("`{}{CAUSE_LEAD}`", with_host(head, "${host}"));
        assert_eq!(
            code.matches(literal.as_str()).count(),
            1,
            "forgeClearHostFailure.ts must contain the whole {name} message text exactly once, as the code literal {literal:?} — update the mock (its header promises verbatim backend text)"
        );
    }
    // `KEYCHAIN_FAIL_JOIN` is a two-character literal, so only its interpolation
    // shape in the mock's joined-cause message (`${CAUSE}; ${CAUSE}`) can guard it.
    let joined = format!("}}{KEYCHAIN_FAIL_JOIN}${{");
    assert!(
        code.contains(&joined),
        "forgeClearHostFailure.ts joins its causes with something other than {KEYCHAIN_FAIL_JOIN:?} — the all-refused message no longer mirrors the backend"
    );
}

/// Copy-honesty (the fourth instance of this family): with NO accounts on the
/// host, only the legacy bare-host delete can be refused — and then there are no
/// accounts listed, so the standard suffix's "the accounts are still listed"
/// asserts a UI fact that is false. Coverage: pre-fix this message was
/// unconditional.
#[test]
fn empty_host_keychain_failure_does_not_claim_accounts_are_listed() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let wrote = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&wrote);
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "BitBucket.org".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &["bitbucket.org"]),
            update_settings: Box::new(move |_, _| {
                flag.store(true, Ordering::SeqCst);
                Ok(())
            }),
        },
    ));
    let msg = r
        .expect_err("a refused legacy delete must fail")
        .to_string();
    assert_eq!(
        msg,
        "A leftover credential for bitbucket.org couldn't be removed from the OS keychain. Nothing was changed, so you can try again. Details: access denied for bitbucket.org"
    );
    assert!(
        !msg.contains("still listed"),
        "no account is listed for this host: {msg}"
    );
    assert!(!wrote.load(Ordering::SeqCst), "nothing may be written");
    assert_eq!(seen.lock().expect("seen").clone(), vec!["bitbucket.org"]);
}

/// Audit LOW-1: the settings read happens outside the `SETTINGS_IO` lock, so a
/// concurrent `forge_add_account` on the SAME host can land between the read and
/// the mutation. A host-keyed retain drops that record even though its key was
/// never deleted — a live PAT with nothing naming its `keychain_key` (the
/// MEDIUM-1 state, reached by a race). The retain must be keyed by the deleted
/// ids. Coverage: pre-fix the interleaved record "c" was dropped.
#[test]
fn a_record_added_after_the_read_survives_the_retain() {
    let dir = scratch_dir();
    let file = seed(dir.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &[]),
            // Stand in for `settings::update`: re-`load_from` INSIDE the lock,
            // with a concurrent add having landed in between.
            update_settings: Box::new(|f, mutate| {
                let mut s = settings::load_from(f);
                settings::upsert_forge_account(&mut s, acct("c", "github.com"));
                mutate(&mut s);
                settings::save_to(f, &s)
            }),
        },
    ));
    assert!(r.is_ok(), "got {r:?}");
    let s = settings::load_from(&file);
    let mut ids: Vec<&str> = s
        .forge_accounts
        .iter()
        .map(|a| a.account_id.as_str())
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, vec!["c", "g"], "the un-deleted record must survive");
    let keys = seen.lock().expect("seen").clone();
    assert!(
        !keys.contains(&"c".to_string()),
        "c's key was never deleted: {keys:?}"
    );
}

/// Audit INFO-1: a MIGRATED legacy account carries `keychain_key == <bare host>`,
/// so the account chain and the legacy sweep name the SAME key. `delete` is
/// idempotent, but on refusal the joined causes showed the identical cause twice
/// — which a user reads. Coverage: pre-fix `seen` had the key twice and the
/// message repeated the cause.
#[test]
fn a_migrated_legacy_key_is_attempted_once_and_reported_once() {
    let dir = scratch_dir();
    let file = dir.path().join("settings.json");
    let mut s = settings::Settings::default();
    let mut legacy = acct("legacy", "github.com");
    legacy.keychain_key = "github.com".to_string();
    settings::upsert_forge_account(&mut s, legacy);
    settings::save_to(&file, &s).expect("seed settings");
    let seen = Arc::new(Mutex::new(Vec::new()));
    let r = tauri::async_runtime::block_on(forge_clear_token_for_host_inner_with(
        &file,
        "github.com".to_string(),
        ClearHostDeps {
            delete_token: recording_delete(Arc::clone(&seen), &["github.com"]),
            ..ClearHostDeps::default()
        },
    ));
    assert_eq!(
        r.expect_err("must fail").to_string(),
        "Couldn't remove this host's credentials from the OS keychain. Nothing was changed — the accounts are still listed, so you can try again. Details: access denied for github.com",
        "the cause must appear exactly once"
    );
    assert_eq!(
        seen.lock().expect("seen").clone(),
        vec!["github.com"],
        "the duplicated key must be attempted once"
    );
}
