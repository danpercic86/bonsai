//! P80 resolution precedence tests (runtime-free — `resolve_account` is pure).

use super::*;
use bonsai_forge::ForgeKind;

fn acct(id: &str, host: &str, login: Option<&str>) -> settings::ForgeAccountRecord {
    settings::ForgeAccountRecord {
        account_id: id.to_string(),
        keychain_key: id.to_string(),
        host: host.to_string(),
        kind: ForgeKind::GitHub,
        login: login.map(|l| l.to_string()),
        avatar_url: None,
    }
}

fn settings_with(accts: Vec<settings::ForgeAccountRecord>) -> settings::Settings {
    settings::Settings {
        forge_accounts: accts,
        ..settings::Settings::default()
    }
}

#[test]
fn no_accounts_on_host_is_none() {
    let s = settings_with(vec![]);
    let r = resolve_account(&s, "/repo", "github.com", "owner");
    assert!(r.account.is_none());
    assert_eq!(r.source, AccountSource::None);
}

#[test]
fn single_account_resolves_single() {
    let s = settings_with(vec![acct("a", "github.com", Some("alice"))]);
    let r = resolve_account(&s, "/repo", "github.com", "stranger");
    assert_eq!(r.account.unwrap().account_id, "a");
    assert_eq!(r.source, AccountSource::Single);
}

#[test]
fn override_wins_over_owner_match() {
    let mut s = settings_with(vec![
        acct("a", "github.com", Some("danpercic86")),
        acct("b", "github.com", Some("other")),
    ]);
    s.forge_host_defaults.push(settings::ForgeHostDefault {
        host: "github.com".into(),
        account_id: "b".into(),
    });
    // A manual pin at "b" for the danpercic86-owned repo beats the owner match.
    s.repo_forge_overrides.push(settings::RepoForgeOverride {
        repo_path: "/repo/danpercic86-bonsai".into(),
        account_id: "b".into(),
    });
    let r = resolve_account(&s, "/repo/danpercic86-bonsai", "github.com", "danpercic86");
    assert_eq!(r.account.unwrap().account_id, "b");
    assert_eq!(r.source, AccountSource::Override);
}

#[test]
fn owner_match_beats_host_default_and_is_case_insensitive() {
    let mut s = settings_with(vec![
        acct("a", "github.com", Some("danpercic86")),
        acct("b", "github.com", Some("other")),
    ]);
    s.forge_host_defaults.push(settings::ForgeHostDefault {
        host: "github.com".into(),
        account_id: "b".into(),
    });
    // Different-case owner still matches account "a".
    let r = resolve_account(&s, "/repo", "github.com", "DanPercic86");
    assert_eq!(r.account.unwrap().account_id, "a");
    assert_eq!(r.source, AccountSource::OwnerMatch);
}

#[test]
fn stranger_owner_falls_through_to_host_default() {
    let mut s = settings_with(vec![
        acct("a", "github.com", Some("danpercic86")),
        acct("b", "github.com", Some("other")),
    ]);
    s.forge_host_defaults.push(settings::ForgeHostDefault {
        host: "github.com".into(),
        account_id: "b".into(),
    });
    let r = resolve_account(&s, "/repo", "github.com", "randomuser");
    assert_eq!(r.account.unwrap().account_id, "b");
    assert_eq!(r.source, AccountSource::HostDefault);
}

#[test]
fn host_default_precedence_over_first() {
    let mut s = settings_with(vec![
        acct("a", "github.com", None),
        acct("b", "github.com", None),
    ]);
    s.forge_host_defaults.push(settings::ForgeHostDefault {
        host: "github.com".into(),
        account_id: "b".into(),
    });
    let r = resolve_account(&s, "/repo", "github.com", "");
    assert_eq!(r.account.unwrap().account_id, "b");
    assert_eq!(r.source, AccountSource::HostDefault);
}

#[test]
fn deleted_pin_falls_back_never_errors() {
    // Override points at "b", but "b" no longer exists (deleted account).
    let mut s = settings_with(vec![acct("a", "github.com", Some("danpercic86"))]);
    s.repo_forge_overrides.push(settings::RepoForgeOverride {
        repo_path: "/repo".into(),
        account_id: "b".into(),
    });
    // Owner matches "a" → resolves there instead of erroring.
    let r = resolve_account(&s, "/repo", "github.com", "danpercic86");
    assert_eq!(r.account.unwrap().account_id, "a");
    assert_eq!(r.source, AccountSource::OwnerMatch);
}

#[test]
fn multiple_owner_matches_fall_through() {
    // Two accounts share the same login → ambiguous, fall through to single/first.
    let s = settings_with(vec![
        acct("a", "github.com", Some("dup")),
        acct("b", "github.com", Some("dup")),
    ]);
    let r = resolve_account(&s, "/repo", "github.com", "dup");
    // No default, >1 owner match → first (most-recent), source hostDefault (OD-4).
    assert_eq!(r.account.unwrap().account_id, "a");
    assert_eq!(r.source, AccountSource::HostDefault);
}
