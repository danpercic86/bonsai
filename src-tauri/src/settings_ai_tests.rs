//! Unit tests for [`super`] (`settings.rs`): AI settings, onboarding, auto-check
//! updates, identity profiles, and external-command settings.
//!
//! Kept in a sibling file so `settings.rs` stays closer to the ~500-line soft
//! limit. Declared with `#[path]` as a child module of `settings`, so
//! `super::*` still reaches the private items without widening their
//! visibility (the `external_tests` / `session_drain_tests` convention).

use super::*;
use super::tests::settings_path;

/// Save/load a `Settings` with non-default AI fields round-trips exactly
/// (P13 §4.1). Also asserts the raw JSON uses the documented camelCase keys
/// + values.
#[test]
fn ai_settings_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        ai_enabled: false,
        ai_conflict_autonomy: AiAutonomy::AutoResolve,
        ai_consented: true,
        ..Default::default()
    };

    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert!(!loaded.ai_enabled);
    assert_eq!(loaded.ai_conflict_autonomy, AiAutonomy::AutoResolve);
    assert!(loaded.ai_consented);

    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"aiEnabled\": false"));
    assert!(raw.contains("\"aiConflictAutonomy\": \"autoResolve\""));
    assert!(raw.contains("\"aiConsented\": true"));
}

/// An old `settings.json` written before P13 (no `aiEnabled`/
/// `aiConflictAutonomy`/`aiConsented` keys) loads with the type defaults
/// for the new fields (`true` / `ProposeReview` / `false`) and preserves
/// existing ones — the additive-field guarantee for the P13 settings
/// specifically (P13 §4.1), mirroring
/// `old_settings_file_without_list_view_loads_default`.
#[test]
fn old_settings_file_without_ai_fields_loads_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, json).expect("write pre-P13 settings.json");

    let loaded = load_from(&file);
    assert!(loaded.ai_enabled);
    assert_eq!(loaded.ai_conflict_autonomy, AiAutonomy::ProposeReview);
    assert!(!loaded.ai_consented);
    // Existing fields untouched.
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.recent_repos.len(), 1);
}

/// An old `settings.json` written before P43 (no `onboardingSeen` key) loads
/// with the default (`false` ⇒ show onboarding once) and preserves existing
/// fields — the additive-field guarantee for the P43 setting specifically,
/// mirroring `old_settings_file_without_ai_fields_loads_defaults`.
#[test]
fn old_settings_file_without_onboarding_seen_loads_default() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, json).expect("write pre-P43 settings.json");

    let loaded = load_from(&file);
    assert!(!loaded.onboarding_seen);
    // Existing fields untouched.
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.recent_repos.len(), 1);
}

/// A non-default `onboarding_seen` round-trips and serializes to the
/// documented camelCase key (P43 §6).
#[test]
fn onboarding_seen_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        onboarding_seen: true,
        ..Default::default()
    };
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert!(loaded.onboarding_seen);
    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"onboardingSeen\": true"));
}

/// An old `settings.json` written before P42 (no `autoCheckUpdates` key)
/// loads with the default (`false` ⇒ no auto-check on launch) and preserves
/// existing fields — the additive-field guarantee for the P42 setting
/// specifically, mirroring `old_settings_file_without_onboarding_seen_loads_default`.
#[test]
fn old_settings_file_without_auto_check_updates_loads_default() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, json).expect("write pre-P42 settings.json");

    let loaded = load_from(&file);
    assert!(!loaded.auto_check_updates);
    // Existing fields untouched.
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.recent_repos.len(), 1);
}

/// A non-default `auto_check_updates` round-trips and serializes to the
/// documented camelCase key (P42 D4).
#[test]
fn auto_check_updates_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        auto_check_updates: true,
        ..Default::default()
    };
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert!(loaded.auto_check_updates);
    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"autoCheckUpdates\": true"));
}

/// A couple of `profiles` round-trip through save/load intact (incl. the
/// optional signing key both Some and None); AND a legacy `settings.json`
/// WITHOUT the `profiles` key loads with `profiles == vec![]` — the
/// additive-field back-compat guarantee for the P44 setting specifically,
/// mirroring `old_settings_file_without_auto_check_updates_loads_default`.
#[test]
fn profiles_roundtrip_and_backcompat() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        profiles: vec![
            IdentityProfile {
                id: "id-work".to_string(),
                label: "Work".to_string(),
                user_name: "Ada Lovelace".to_string(),
                user_email: "work@example.com".to_string(),
                signing_key: None,
            },
            IdentityProfile {
                id: "id-personal".to_string(),
                label: "Personal".to_string(),
                user_name: "Ada".to_string(),
                user_email: "me@personal.dev".to_string(),
                signing_key: Some("KEY123".to_string()),
            },
        ],
        ..Default::default()
    };
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert_eq!(loaded.profiles.len(), 2);
    assert_eq!(loaded.profiles[0].label, "Work");
    assert_eq!(loaded.profiles[0].signing_key, None);
    assert_eq!(loaded.profiles[1].signing_key.as_deref(), Some("KEY123"));
    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"profiles\""));
    // camelCase wire mirror for the IdentityProfile fields.
    assert!(raw.contains("\"userEmail\""));
    assert!(raw.contains("\"signingKey\""));

    // Back-compat: a pre-P44 file without `profiles` loads as an empty Vec
    // and preserves the existing fields.
    let legacy = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, legacy).expect("write pre-P44 settings.json");
    let loaded = load_from(&file);
    assert!(loaded.profiles.is_empty());
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.recent_repos.len(), 1);
}

/// An old `settings.json` written before P49 (no `terminalCommand`/
/// `editorCommand` keys) loads both as `""` and preserves the existing
/// fields — the additive-field back-compat guarantee for the P49 settings.
/// Also asserts a round-trip through the documented camelCase keys.
#[test]
fn external_commands_roundtrip_and_backcompat() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);

    // Round-trip non-default values + camelCase wire keys.
    let s = Settings {
        terminal_command: "wt -d {path}".to_string(),
        editor_command: "code {path}".to_string(),
        ..Default::default()
    };
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert_eq!(loaded.terminal_command, "wt -d {path}");
    assert_eq!(loaded.editor_command, "code {path}");
    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"terminalCommand\""));
    assert!(raw.contains("\"editorCommand\""));

    // Back-compat: a pre-P49 file without the keys loads both as "" and
    // preserves existing fields.
    let legacy = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, legacy).expect("write pre-P49 settings.json");
    let loaded = load_from(&file);
    assert_eq!(loaded.terminal_command, "");
    assert_eq!(loaded.editor_command, "");
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.recent_repos.len(), 1);
}
