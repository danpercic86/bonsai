//! Runtime-free tests for `forge_add_account_inner_with` — security-audit
//! MEDIUM-2 source A (a swallowed settings write left an unreachable
//! credential) and source B (a legacy re-add abandoned its bare-host key).
//!
//! Uses the `_with` seam so the real OS keychain is never touched and no
//! network is needed (validation happens in `forge_add_account_inner`, outside
//! the seam). The one real-keychain test lives in
//! `forge_add_account_real_keychain_tests.rs` — it is `#[ignore]`d precisely
//! because it DOES write to the user's credential manager, and it is a separate
//! file so this one stays under the CLAUDE.md 500-line limit.

use super::super::forge_account_test_support::{scratch_dir as support_scratch_dir, Calls};
use super::*;

/// The token the tests store. Never a realistic-looking PAT.
const TOKEN: &str = "mock-token";

fn viewer() -> ForgeViewer {
    ForgeViewer {
        login: "octocat".to_string(),
        avatar_url: None,
    }
}

/// The deterministic three-part key this add stores under.
fn aid() -> String {
    settings::account_id(ForgeKind::GitHub, "github.com", Some("octocat"))
}

/// An empty settings file (no account on the host yet).
fn seed_empty(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("settings.json");
    settings::save_to(&file, &settings::Settings::default()).expect("seed settings");
    file
}

/// A settings file holding ONE account for `octocat@github.com` keyed by
/// `keychain_key`. `keychain_key == aid()` models a MODERN record (the re-add
/// case); `keychain_key == "github.com"` models a MIGRATED legacy one.
fn seed_with_key(dir: &std::path::Path, keychain_key: &str) -> std::path::PathBuf {
    let file = dir.join("settings.json");
    let mut s = settings::Settings::default();
    settings::upsert_forge_account(
        &mut s,
        settings::ForgeAccountRecord {
            account_id: aid(),
            keychain_key: keychain_key.to_string(),
            host: "github.com".to_string(),
            kind: ForgeKind::GitHub,
            login: Some("octocat".to_string()),
            avatar_url: None,
        },
    );
    settings::upsert_forge_host(
        &mut s,
        "github.com",
        ForgeKind::GitHub,
        Some("octocat".into()),
    );
    settings::save_to(&file, &s).expect("seed settings");
    file
}

fn add(file: &Path, deps: AddAccountDeps) -> Result<ForgeViewer, AppError> {
    tauri::async_runtime::block_on(forge_add_account_inner_with(
        file,
        "GitHub.com".to_string(),
        ForgeKind::GitHub,
        TOKEN.to_string(),
        viewer(),
        deps,
    ))
}

/// THE DISCRIMINATOR (audit MEDIUM-2 source A, case 2). A re-add of an
/// EXISTING account whose record already names the three-part key: the store
/// OVERWROTE a live credential, so a failing settings write must NOT delete it
/// — the surviving record still points there, and deleting would leave a record
/// with no token (`connected: false`), the orphan bug in the other direction.
/// An unconditional rollback fails exactly this test.
#[test]
fn settings_failure_on_a_readd_keeps_the_existing_credential() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_with_key(dir.path(), &aid());
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.failing_update(),
        },
    );
    let msg = r.expect_err("settings write failed").to_string();
    assert!(
        msg.starts_with(CREDENTIAL_KEPT_HEAD),
        "expected the credential-kept outcome, got {msg:?}"
    );
    assert_eq!(
        calls.log(),
        vec![format!("store:{}", aid()), "settings".to_string()],
        "no delete may happen: the store overwrote a credential an existing record still names"
    );
}

/// Source A, case 1: nothing referenced the key before the store, so the new
/// credential is referenced by nothing and is withdrawn again.
#[test]
fn settings_failure_on_a_new_account_withdraws_the_new_credential() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_empty(dir.path());
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.failing_update(),
        },
    );
    let msg = r.expect_err("settings write failed").to_string();
    assert!(
        msg.starts_with(ROLLED_BACK_HEAD),
        "expected the rolled-back outcome, got {msg:?}"
    );
    assert_eq!(
        calls.log(),
        vec![
            format!("store:{}", aid()),
            "settings".to_string(),
            format!("delete:{}", aid()),
        ]
    );
}

/// Source A, case 3: the pre-existing record is LEGACY (its `keychain_key` is
/// the bare host), so the `aid` key is new and unreferenced — case 1 applies.
/// The SUPERSEDED bare-host key must survive: its record survived the failed
/// write and still points at it, so deleting it here is the
/// "delete-then-fail leaves a record naming a key that is gone" hazard.
#[test]
fn settings_failure_on_a_legacy_readd_withdraws_only_the_new_key() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_with_key(dir.path(), "github.com");
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.failing_update(),
        },
    );
    let msg = r.expect_err("settings write failed").to_string();
    assert!(
        msg.starts_with(ROLLED_BACK_HEAD),
        "expected the rolled-back outcome, got {msg:?}"
    );
    let log = calls.log();
    assert!(
        log.contains(&format!("delete:{}", aid())),
        "the new, unreferenced key must be withdrawn: {log:?}"
    );
    assert!(
        !log.contains(&"delete:github.com".to_string()),
        "the superseded bare-host key must survive a FAILED write — its record still names it: {log:?}"
    );
}

/// Source A, the compensating delete is itself refused: both causes are
/// reported, settings first, and the copy names the asymmetry.
#[test]
fn a_refused_rollback_reports_both_causes() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_empty(dir.path());
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(Some("keychain error: access denied")),
            update_settings: calls.failing_update(),
        },
    );
    let msg = r.expect_err("settings write failed").to_string();
    assert_eq!(
        msg,
        format!(
            "{ROLLBACK_FAILED_HEAD}{CAUSE_LEAD}io error: disk full{CAUSE_JOIN}keychain error: access denied"
        )
    );
    assert!(
        !msg.contains(&aid()),
        "a keychain key must never appear in a user-facing message: {msg:?}"
    );
}

/// Source B: a legacy re-add re-keys the record to `aid`, so the abandoned
/// bare-host entry is swept — AFTER the settings write, never before.
#[test]
fn a_successful_legacy_readd_deletes_the_superseded_bare_host_key() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_with_key(dir.path(), "github.com");
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.real_update(),
        },
    );
    assert!(r.is_ok(), "the add must succeed: {:?}", r.err());
    assert_eq!(
        calls.log(),
        vec![
            format!("store:{}", aid()),
            "settings".to_string(),
            "delete:github.com".to_string(),
        ],
        "order matters: the settings write must land BEFORE the superseded key is deleted"
    );
    let s = settings::load_from(&file);
    assert_eq!(s.forge_accounts.len(), 1);
    assert_eq!(s.forge_accounts[0].keychain_key, aid());
}

/// Source B: a REFUSED sweep of the superseded key must not fail the add — the
/// account works, and `forge_remove_account`'s backstop catches the leftover.
#[test]
fn a_refused_superseded_sweep_does_not_fail_the_add() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_with_key(dir.path(), "github.com");
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(Some("keychain error: access denied")),
            update_settings: calls.real_update(),
        },
    );
    assert!(
        r.is_ok(),
        "a refused sweep must not fail the add: {:?}",
        r.err()
    );
    assert_eq!(
        settings::load_from(&file).forge_accounts[0].keychain_key,
        aid()
    );
}

/// A MODERN re-add has no superseded key, so nothing is deleted on success
/// either (the overwrite is the whole update).
#[test]
fn a_successful_modern_readd_deletes_nothing() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_with_key(dir.path(), &aid());
    let calls = Calls::default();
    let r = add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.real_update(),
        },
    );
    assert!(r.is_ok(), "the add must succeed: {:?}", r.err());
    assert_eq!(
        calls.log(),
        vec![format!("store:{}", aid()), "settings".to_string()]
    );
}

/// PIN (audit MEDIUM-2 source B): `migrate_forge_hosts_to_accounts` runs on
/// EVERY `load_from`, and the OD-5 legacy `forge_hosts` mirror is still written
/// by a successful add. So after a legacy re-add re-keys the record, the
/// migration must NOT re-create a bare-host record for that host — which would
/// resurrect the very key the add just deleted. It skips hosts already present
/// in `forge_accounts`, and this is what holds it to that.
#[test]
fn the_migration_does_not_recreate_a_bare_host_record_after_a_readd() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = seed_with_key(dir.path(), "github.com");
    let calls = Calls::default();
    add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.real_update(),
        },
    )
    .expect("add succeeds");
    // `load_from` is the real migration entry point (`settings.rs:339`).
    let s = settings::load_from(&file);
    assert!(
        !s.forge_hosts.is_empty(),
        "the OD-5 legacy mirror is still written, so the migration really does re-run over it"
    );
    assert_eq!(s.forge_accounts.len(), 1, "no second, bare-host record");
    assert_eq!(s.forge_accounts[0].keychain_key, aid());
}

/// THE SAFETY CLAUSE of the source-B sweep (`superseded`'s `.filter`), which
/// nothing else exercises: the superseded key is only deleted when NO OTHER
/// record names it. Seeds a legacy record for `aid` PLUS a second, different
/// account whose `keychain_key` is also the bare host — a hand-edited
/// `settings.json`, or a future migration that keys two accounts alike. Without
/// the filter this successful add would delete the sibling's live credential.
#[test]
fn a_key_another_record_also_names_is_never_swept() {
    let dir = support_scratch_dir("bonsai-add-acct-");
    let file = dir.path().join("settings.json");
    let mut s = settings::Settings::default();
    // The record this add REPLACES: still legacy, keyed by the bare host.
    settings::upsert_forge_account(
        &mut s,
        settings::ForgeAccountRecord {
            account_id: aid(),
            keychain_key: "github.com".to_string(),
            host: "github.com".to_string(),
            kind: ForgeKind::GitHub,
            login: Some("octocat".to_string()),
            avatar_url: None,
        },
    );
    // A DIFFERENT account that also names the bare-host key. It survives this
    // add untouched, so that key is still live afterwards.
    settings::upsert_forge_account(
        &mut s,
        settings::ForgeAccountRecord {
            account_id: "gitHub:github.com:hubot".to_string(),
            keychain_key: "github.com".to_string(),
            host: "github.com".to_string(),
            kind: ForgeKind::GitHub,
            login: Some("hubot".to_string()),
            avatar_url: None,
        },
    );
    settings::save_to(&file, &s).expect("seed settings");
    let calls = Calls::default();
    add(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.real_update(),
        },
    )
    .expect("add");
    assert_eq!(
        calls.log(),
        vec![format!("store:{}", aid()), "settings".to_string()],
        "NO delete: `github.com` is still named by the other account"
    );
    let after = settings::load_from(&file);
    assert!(
        after
            .forge_accounts
            .iter()
            .any(|a| a.account_id == "gitHub:github.com:hubot" && a.keychain_key == "github.com"),
        "the sibling record is untouched, so its credential must still exist"
    );
}

/// The cross-language copy guard, in the shape
/// `forge_remove_account_tests::mock_copy_mirrors_the_rust_copy` establishes:
/// each message's ENTIRE fixed text (`HEAD` + [`CAUSE_LEAD`], since the cause is
/// last) must appear in the mock's code as ONE backtick literal, exactly once.
/// `src/ipc/mock/handlers/forgeAddFailure.ts` claims its strings are the
/// backend's output VERBATIM, but Rust and TS share no constant, so nothing else
/// makes editing one side red.
#[test]
fn mock_copy_mirrors_the_rust_copy() {
    const MOCK: &str = include_str!("../../../src/ipc/mock/handlers/forgeAddFailure.ts");
    let code: String = MOCK
        .lines()
        .filter(|l| {
            let t = l.trim_start();
            !(t.starts_with("//") || t.starts_with("/*") || t.starts_with('*'))
        })
        .collect::<Vec<_>>()
        .join("\n");
    for (name, head) in [
        ("ROLLED_BACK_HEAD", ROLLED_BACK_HEAD),
        ("CREDENTIAL_KEPT_HEAD", CREDENTIAL_KEPT_HEAD),
        ("ROLLBACK_FAILED_HEAD", ROLLBACK_FAILED_HEAD),
        // The two-cause separator, which only this command's mock renders.
        ("CAUSE_JOIN", CAUSE_JOIN),
    ] {
        let literal = if name == "CAUSE_JOIN" {
            format!("`{head}`")
        } else {
            format!("`{head}{CAUSE_LEAD}`")
        };
        assert_eq!(
            code.matches(literal.as_str()).count(),
            1,
            "forgeAddFailure.ts must contain the whole {name} text exactly once, as the code literal {literal:?} — update the mock (its header promises verbatim backend text)"
        );
    }
}
