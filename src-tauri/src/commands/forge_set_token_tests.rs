//! Runtime-free tests for `forge_set_token_with` — security-audit MEDIUM-2 at
//! its LIVE occurrence (the per-repo Connect field in `PrPanel` /
//! `ChecksPanel`), which kept the swallowed settings write for a whole day
//! after the dormant add path was fixed.
//!
//! These exercise the delegation, not a second implementation: the assertions
//! are that the SAME discriminator outcomes and the SAME copy reach a caller
//! through this entry point, plus the one thing only this command does — the
//! repo override must commit in the same transaction as the account record.
//!
//! BASELINE for the red evidence these tests were written against: HEAD
//! (`61af79b`) has no `forge_set_token_with` at all, so none of them compile
//! against it. "Red" here means red against HEAD's command BODY transplanted
//! behind this new seam — a stronger claim than that would not be supported.
//!
//! `forge_set_token_inner`'s empty-host early return is NOT covered here — and
//! cannot be: it is unreachable, because `validate_repo_token` rejects an
//! unparseable origin (`ForgeKind::Unknown`) with `ForgeUnsupported` before the
//! check runs. See the comment on that branch in `forge_set_token.rs` for the
//! `require_supported` cite.

use super::super::forge_account_test_support::{scratch_dir, Calls};
use super::super::forge_add_account::{
    CAUSE_LEAD, CREDENTIAL_KEPT_HEAD, ROLLBACK_FAILED_HEAD, ROLLED_BACK_HEAD,
};
use super::*;

/// The token the tests store. Never a realistic-looking PAT.
const TOKEN: &str = "mock-token";
/// `Calls::failing_update`'s cause, so the expected messages are whole.
const SETTINGS_CAUSE: &str = "io error: disk full";
/// The repo whose Connect field was used. Only ever a string here.
const WORKDIR: &str = "/scratch/repo";

fn viewer() -> ForgeViewer {
    ForgeViewer {
        login: "octocat".to_string(),
        avatar_url: None,
    }
}

/// The deterministic three-part key this connect stores under.
fn aid() -> String {
    settings::account_id(ForgeKind::GitHub, "github.com", Some("octocat"))
}

/// An empty settings file — a first connect on this host.
fn seed_empty(dir: &std::path::Path) -> std::path::PathBuf {
    let file = dir.join("settings.json");
    settings::save_to(&file, &settings::Settings::default()).expect("seed settings");
    file
}

/// A settings file holding ONE account for `octocat@github.com` keyed by
/// `keychain_key` — `aid()` models a RECONNECT of a modern record, the bare
/// host a MIGRATED legacy one.
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
    settings::save_to(&file, &s).expect("seed settings");
    file
}

fn connect(file: &Path, deps: AddAccountDeps) -> Result<ForgeViewer, AppError> {
    tauri::async_runtime::block_on(forge_set_token_with(
        file,
        PathBuf::from(WORKDIR),
        // Mixed case, as `validate_repo_token` could hand back: the key must be
        // derived from the lowercased host or the override would name an
        // account that does not exist.
        "GitHub.com".to_string(),
        ForgeKind::GitHub,
        TOKEN.to_string(),
        viewer(),
        deps,
    ))
}

/// `AppError` has no `PartialEq`, and these messages are CONTRACT text, so the
/// tests compare the whole rendered string rather than matching a variant.
fn message(err: AppError) -> String {
    match err {
        AppError::Other(m) => m,
        other => panic!("expected AppError::Other, got {other:?}"),
    }
}

fn override_for(file: &Path) -> Option<String> {
    settings::load_from(file)
        .repo_forge_overrides
        .iter()
        .find(|o| o.repo_path == WORKDIR)
        .map(|o| o.account_id.clone())
}

/// THE REGRESSION (audit MEDIUM-2, live occurrence). A first connect whose
/// settings write fails used to return `Ok(viewer)` with the PAT sitting in the
/// OS keychain under a key no record names — unreachable from the UI forever.
/// It must now FAIL, withdraw the credential it just wrote, and say so.
#[test]
fn a_failing_settings_write_is_no_longer_swallowed() {
    let dir = scratch_dir("bonsai-set-token-");
    let file = seed_empty(dir.path());
    let calls = Calls::default();
    let err = connect(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.failing_update(),
        },
    )
    .expect_err("a failing settings write must not report success");
    assert_eq!(
        message(err),
        format!("{ROLLED_BACK_HEAD}{CAUSE_LEAD}{SETTINGS_CAUSE}")
    );
    assert_eq!(
        calls.log(),
        vec![
            format!("store:{}", aid()),
            "settings".to_string(),
            format!("delete:{}", aid()),
        ],
        "the credential must be withdrawn AFTER the write failed"
    );
}

/// The discriminator, through this entry point: a RECONNECT whose record
/// already names the three-part key overwrote a live credential, so a failing
/// settings write must NOT delete it — the surviving record still points there.
/// An unconditional rollback fails exactly this test.
#[test]
fn a_failing_write_on_a_reconnect_keeps_the_overwritten_credential() {
    let dir = scratch_dir("bonsai-set-token-");
    let file = seed_with_key(dir.path(), &aid());
    let calls = Calls::default();
    let err = connect(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.failing_update(),
        },
    )
    .expect_err("a failing settings write must not report success");
    assert_eq!(
        message(err),
        format!("{CREDENTIAL_KEPT_HEAD}{CAUSE_LEAD}{SETTINGS_CAUSE}")
    );
    assert_eq!(
        calls.log(),
        vec![format!("store:{}", aid()), "settings".to_string()],
        "no delete: the existing record still names this key"
    );
}

/// The refused-rollback outcome reaches this caller with its own copy too, so
/// the panel never renders a message that contradicts the keychain state.
#[test]
fn a_refused_rollback_reports_both_causes() {
    let dir = scratch_dir("bonsai-set-token-");
    let file = seed_empty(dir.path());
    let calls = Calls::default();
    let err = connect(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(Some("keychain error: Access is denied.")),
            update_settings: calls.failing_update(),
        },
    )
    .expect_err("a failing settings write must not report success");
    let message = message(err);
    assert!(message.starts_with(ROLLBACK_FAILED_HEAD), "got {message:?}");
    assert!(
        message.ends_with("keychain error: Access is denied."),
        "got {message:?}"
    );
}

/// The one thing only this command does (OD-3): the repo pin is part of the
/// SAME load→mutate→save as the account record, so a successful connect leaves
/// both, and the override names the account that actually exists.
#[test]
fn a_successful_connect_pins_the_repo_in_the_same_transaction() {
    let dir = scratch_dir("bonsai-set-token-");
    let file = seed_empty(dir.path());
    let calls = Calls::default();
    connect(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.real_update(),
        },
    )
    .expect("connect");
    let s = settings::load_from(&file);
    assert!(
        s.forge_accounts.iter().any(|a| a.account_id == aid()),
        "the account record must be saved"
    );
    assert_eq!(override_for(&file).as_deref(), Some(aid().as_str()));
    assert!(
        s.forge_host_defaults
            .iter()
            .any(|d| d.host == "github.com" && d.account_id == aid()),
        "first account on the host becomes its default"
    );
    assert_eq!(
        calls.log(),
        vec![format!("store:{}", aid()), "settings".to_string()],
        "one transaction, no compensating delete"
    );
}

/// The corollary: a FAILED connect leaves no half-state for the panel to show.
/// Because the pin travels inside the injected transaction, there is no
/// override pointing at an account that was never saved.
///
/// What makes this an ON-DISK assertion and not a tautology: `override_for`
/// reads the settings FILE, which `Calls::failing_update` never writes. So a pin
/// written outside the injected transaction (a direct `settings::update` in
/// `forge_set_token_with`) turns this test red — verified by mutating exactly
/// that and observing
/// `left: Some("gitHub:github.com:octocat") right: None`.
#[test]
fn a_failed_connect_leaves_no_repo_override() {
    let dir = scratch_dir("bonsai-set-token-");
    let file = seed_empty(dir.path());
    let calls = Calls::default();
    connect(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.failing_update(),
        },
    )
    .expect_err("a failing settings write must not report success");
    assert_eq!(override_for(&file), None);
    assert!(
        settings::load_from(&file).forge_accounts.is_empty(),
        "nothing was saved"
    );
}

/// Audit MEDIUM-2 source B, through this entry point: connecting with a
/// MIGRATED legacy record re-keys it to the three-part `aid`, so the abandoned
/// bare-host entry is swept — AFTER the settings write, never before.
///
/// The defect was REAL, not an artifact of how it is observed: HEAD's body for
/// this command performed no sweep whatsoever, so a legacy reconnect orphaned
/// the bare-host credential with no record naming it. What the seam adds is
/// observability — it is the bug's evidence that needed the seam, not the bug.
#[test]
fn a_legacy_reconnect_sweeps_the_superseded_bare_host_key() {
    let dir = scratch_dir("bonsai-set-token-");
    let file = seed_with_key(dir.path(), "github.com");
    let calls = Calls::default();
    connect(
        &file,
        AddAccountDeps {
            store_token: calls.store_token(),
            delete_token: calls.delete_token(None),
            update_settings: calls.real_update(),
        },
    )
    .expect("connect");
    assert_eq!(
        calls.log(),
        vec![
            format!("store:{}", aid()),
            "settings".to_string(),
            "delete:github.com".to_string(),
        ],
        "settings FIRST, then the sweep"
    );
    assert_eq!(override_for(&file).as_deref(), Some(aid().as_str()));
}
