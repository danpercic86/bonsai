//! Unit tests for [`super`] (`settings.rs`): UI settings — theme, list view,
//! panel density, AI conflict tools, session fields, and their back-compat and
//! clamping paths.
//!
//! Kept in a sibling file so `settings.rs` stays closer to the ~500-line soft
//! limit. Declared with `#[path]` as a child module of `settings`, so
//! `super::*` still reaches the private items without widening their
//! visibility (the `external_tests` / `session_drain_tests` convention).

use super::*;
use super::tests::settings_path;

/// Save/load a `Settings` with non-default `theme` + `pane_widths`
/// round-trips exactly (P2a contract §3.4.2).
#[test]
fn ui_settings_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        theme: ThemeChoice::Light,
        pane_widths: PaneWidths {
            sidebar: 300,
            right_panel: 420,
        },
        ..Default::default()
    };

    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.pane_widths.sidebar, 300);
    assert_eq!(loaded.pane_widths.right_panel, 420);
}

/// `Dark` also round-trips explicitly (not just the non-default `Light`
/// case above) — both wire strings ("dark"/"light") deserialize back to
/// the matching enum variant (P2 contract §2.1/§4.1).
#[test]
fn theme_choice_roundtrips_both_variants() {
    let dir = tempfile::TempDir::new().expect("create temp dir");

    let file_dark = settings_path(&dir).with_file_name("dark.json");
    let s_dark = Settings {
        theme: ThemeChoice::Dark,
        ..Default::default()
    };
    save_to(&file_dark, &s_dark).expect("save dark");
    assert_eq!(load_from(&file_dark).theme, ThemeChoice::Dark);
    let raw_dark = std::fs::read_to_string(&file_dark).expect("read dark.json");
    assert!(raw_dark.contains("\"theme\": \"dark\""));

    let file_light = settings_path(&dir).with_file_name("light.json");
    let s_light = Settings {
        theme: ThemeChoice::Light,
        ..Default::default()
    };
    save_to(&file_light, &s_light).expect("save light");
    assert_eq!(load_from(&file_light).theme, ThemeChoice::Light);
    let raw_light = std::fs::read_to_string(&file_light).expect("read light.json");
    assert!(raw_light.contains("\"theme\": \"light\""));
}

/// Both `ListView` wire strings ("tree"/"flat") round-trip through
/// save/load, and the raw JSON uses the documented camelCase key +
/// lowercase values (P3b contract §2.1).
#[test]
fn list_view_roundtrips_both_variants() {
    let dir = tempfile::TempDir::new().expect("create temp dir");

    let file_tree = settings_path(&dir).with_file_name("tree.json");
    let s_tree = Settings {
        list_view: ListView::Tree,
        ..Default::default()
    };
    save_to(&file_tree, &s_tree).expect("save tree");
    assert_eq!(load_from(&file_tree).list_view, ListView::Tree);
    let raw_tree = std::fs::read_to_string(&file_tree).expect("read tree.json");
    assert!(raw_tree.contains("\"listView\": \"tree\""));

    let file_flat = settings_path(&dir).with_file_name("flat.json");
    let s_flat = Settings {
        list_view: ListView::Flat,
        ..Default::default()
    };
    save_to(&file_flat, &s_flat).expect("save flat");
    assert_eq!(load_from(&file_flat).list_view, ListView::Flat);
    let raw_flat = std::fs::read_to_string(&file_flat).expect("read flat.json");
    assert!(raw_flat.contains("\"listView\": \"flat\""));
}

/// Both `PanelDensity` wire strings ("cozy"/"compact") round-trip through
/// save/load, and the raw JSON uses the documented camelCase key +
/// lowercase values (P67 §4.1).
#[test]
fn panel_density_roundtrips_both_variants() {
    let dir = tempfile::TempDir::new().expect("create temp dir");

    let file_cozy = settings_path(&dir).with_file_name("cozy.json");
    let s_cozy = Settings {
        panel_density: PanelDensity::Cozy,
        ..Default::default()
    };
    save_to(&file_cozy, &s_cozy).expect("save cozy");
    assert_eq!(load_from(&file_cozy).panel_density, PanelDensity::Cozy);
    let raw_cozy = std::fs::read_to_string(&file_cozy).expect("read cozy.json");
    assert!(raw_cozy.contains("\"panelDensity\": \"cozy\""));

    let file_compact = settings_path(&dir).with_file_name("compact.json");
    let s_compact = Settings {
        panel_density: PanelDensity::Compact,
        ..Default::default()
    };
    save_to(&file_compact, &s_compact).expect("save compact");
    assert_eq!(load_from(&file_compact).panel_density, PanelDensity::Compact);
    let raw_compact = std::fs::read_to_string(&file_compact).expect("read compact.json");
    assert!(raw_compact.contains("\"panelDensity\": \"compact\""));
}

/// Both `AiConflictTools` wire strings ("readOnly"/"none") round-trip through
/// save/load with the documented camelCase key + values (P68 §8.3). The `none`
/// variant must NOT serialize as JSON `null` — that would silently reload as
/// the default and re-grant repo access the user turned off.
#[test]
fn ai_conflict_tools_roundtrips_both_variants() {
    let dir = tempfile::TempDir::new().expect("create temp dir");

    let file_ro = settings_path(&dir).with_file_name("readonly.json");
    let s_ro = Settings { ai_conflict_tools: AiConflictTools::ReadOnly, ..Default::default() };
    save_to(&file_ro, &s_ro).expect("save readOnly");
    assert_eq!(load_from(&file_ro).ai_conflict_tools, AiConflictTools::ReadOnly);
    let raw_ro = std::fs::read_to_string(&file_ro).expect("read readonly.json");
    assert!(raw_ro.contains("\"aiConflictTools\": \"readOnly\""), "{raw_ro}");

    let file_none = settings_path(&dir).with_file_name("none.json");
    let s_none = Settings { ai_conflict_tools: AiConflictTools::None, ..Default::default() };
    save_to(&file_none, &s_none).expect("save none");
    assert_eq!(load_from(&file_none).ai_conflict_tools, AiConflictTools::None);
    let raw_none = std::fs::read_to_string(&file_none).expect("read none.json");
    assert!(raw_none.contains("\"aiConflictTools\": \"none\""), "{raw_none}");
}

/// THE no-version-bump guard for P68 (§8.3): a settings.json written before
/// P68 (no `ai*` run keys at all) loads every new field at its documented
/// default — including the two LOCKED user decisions, `aiHardCapSecs = 0`
/// (unbounded) and `aiMaxBudgetUsd = 0.0` (no `--max-budget-usd` flag) —
/// while every pre-existing field survives untouched.
#[test]
fn old_settings_file_without_ai_run_fields_loads_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "listView": "flat",
            "panelDensity": "compact",
            "aiEnabled": true,
            "aiConsented": true,
            "aiConflictAutonomy": "autoResolve"
        }"#;
    std::fs::write(&file, json).expect("write pre-P68 settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.ai_idle_timeout_secs, AI_IDLE_TIMEOUT_DEFAULT);
    assert_eq!(loaded.ai_hard_cap_secs, 0, "0 = unbounded (locked decision)");
    assert_eq!(loaded.ai_max_turns, bonsai_core::ai::DEFAULT_MAX_TURNS);
    assert!(loaded.ai_stream_log);
    assert!(!loaded.ai_include_partial_messages);
    assert_eq!(loaded.ai_conflict_tools, AiConflictTools::ReadOnly);
    assert_eq!(loaded.ai_bulk_max_bytes, AI_BULK_MAX_BYTES_DEFAULT);
    assert_eq!(loaded.ai_max_budget_usd, 0.0, "0.0 = no cap (locked decision)");
    assert_eq!(loaded.ai_dock_height, AI_DOCK_HEIGHT_DEFAULT);
    assert!(!loaded.ai_dock_collapsed);
    // Pre-existing fields are untouched by the addition.
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.panel_density, PanelDensity::Compact);
    assert_eq!(loaded.ai_conflict_autonomy, AiAutonomy::AutoResolve);
    assert_eq!(loaded.recent_repos.len(), 1);
}

/// A hand-edited file with out-of-range AI knobs is clamped ON LOAD, and the
/// documented `0` sentinels (watchdog off / no hard cap / no budget flag)
/// SURVIVE — a naive `clamp` would turn them into minimums and quietly
/// re-introduce the deadline the milestone removed.
#[test]
fn clamp_ai_settings_respects_zero_sentinels_and_ranges() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "aiIdleTimeoutSecs": 0,
            "aiHardCapSecs": 0,
            "aiMaxTurns": 0,
            "aiBulkMaxBytes": 10,
            "aiMaxBudgetUsd": -5.0,
            "aiDockHeight": 5000
        }"#;
    std::fs::write(&file, json).expect("write settings.json");
    let loaded = load_from(&file);
    assert_eq!(loaded.ai_idle_timeout_secs, 0, "0 = watchdog disabled, not 30");
    assert_eq!(loaded.ai_hard_cap_secs, 0, "0 = unbounded, not 60");
    assert_eq!(loaded.ai_max_turns, AI_MAX_TURNS_MIN);
    assert_eq!(loaded.ai_bulk_max_bytes, AI_BULK_MAX_BYTES_MIN);
    assert_eq!(loaded.ai_max_budget_usd, 0.0, "a negative budget means no cap");
    assert_eq!(loaded.ai_dock_height, AI_DOCK_HEIGHT_MAX);

    // In-range values pass through, and an over-range one is capped.
    let mut s = Settings {
        ai_idle_timeout_secs: 4_000,
        ai_hard_cap_secs: 600,
        ai_max_turns: 99,
        ai_max_budget_usd: 250.0,
        ..Default::default()
    };
    clamp_ai_settings(&mut s);
    assert_eq!(s.ai_idle_timeout_secs, AI_IDLE_TIMEOUT_MAX);
    assert_eq!(s.ai_hard_cap_secs, 600);
    assert_eq!(s.ai_max_turns, AI_MAX_TURNS_MAX);
    assert_eq!(s.ai_max_budget_usd, AI_MAX_BUDGET_USD_MAX);
}

/// An old `settings.json` written before P2 (only `version`/`recentRepos`,
/// no `theme`/`paneWidths` keys at all) still loads without error and
/// falls back to the type defaults for the new fields — this is the
/// forward-compat guarantee documented in the `Settings` doc comment
/// (`#[serde(default)]` on the whole struct), exercised here against a
/// hand-written legacy JSON string rather than a struct round-trip so a
/// future accidental removal of `default` would actually fail this test.
#[test]
fn old_settings_file_without_new_fields_loads_with_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let legacy_json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ]
        }"#;
    std::fs::write(&file, legacy_json).expect("write legacy settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.recent_repos.len(), 1);
    assert_eq!(loaded.recent_repos[0].path, "D:\\Repos\\legacy");
    assert_eq!(loaded.theme, ThemeChoice::default());
    assert_eq!(loaded.pane_widths, PaneWidths::default());
    assert_eq!(loaded.list_view, ListView::Tree);
}

/// Save/load a `Settings` with non-empty `open_repos` + `active_repo`
/// round-trips exactly (P3e §6.1 / §9.1 "Session (P3e-b)").
#[test]
fn session_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        open_repos: vec![
            "D:\\Repos\\alpha".to_string(),
            "D:\\Repos\\beta".to_string(),
        ],
        active_repo: Some("D:\\Repos\\beta".to_string()),
        ..Default::default()
    };

    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert_eq!(
        loaded.open_repos,
        vec![
            "D:\\Repos\\alpha".to_string(),
            "D:\\Repos\\beta".to_string()
        ]
    );
    assert_eq!(loaded.active_repo.as_deref(), Some("D:\\Repos\\beta"));
}

/// An old `settings.json` written before P3e (no `openRepos`/`activeRepo`
/// keys) loads with empty session defaults and preserves the existing
/// fields — the additive-field guarantee for the P3e session fields
/// specifically (P3e §6.1 / §9.1 "Session (P3e-b)").
#[test]
fn old_settings_file_without_session_fields_loads_with_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, json).expect("write pre-P3e settings.json");

    let loaded = load_from(&file);
    // New session fields fall back to empty defaults.
    assert!(loaded.open_repos.is_empty());
    assert_eq!(loaded.active_repo, None);
    // Existing fields are preserved untouched.
    assert_eq!(loaded.recent_repos.len(), 1);
    assert_eq!(loaded.recent_repos[0].path, "D:\\Repos\\legacy");
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.pane_widths.sidebar, 300);
}

/// A pre-P3b `settings.json` that has `theme`/`paneWidths` but no
/// `listView` key loads with the default (`Tree`) — the additive-field
/// guarantee for the P3b setting specifically (P3b contract §2.1).
#[test]
fn old_settings_file_without_list_view_loads_default() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 }
        }"#;
    std::fs::write(&file, json).expect("write pre-P3b settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.list_view, ListView::Tree);
    assert_eq!(loaded.theme, ThemeChoice::Light); // other fields untouched
    assert_eq!(loaded.pane_widths.sidebar, 300);
}

/// A pre-P67 `settings.json` that has `theme`/`paneWidths`/`listView` but no
/// `panelDensity` key loads with the default (`Cozy`) — this is THE
/// migration guard behind the no-version-bump claim (P67 §4.2): the absence
/// of the key must be indistinguishable from an explicit `"cozy"`.
#[test]
fn old_settings_file_without_panel_density_loads_default() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "light",
            "listView": "flat",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 }
        }"#;
    std::fs::write(&file, json).expect("write pre-P67 settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.panel_density, PanelDensity::Cozy);
    // Other fields untouched — the version is NOT bumped and nothing is
    // rewritten on load.
    assert_eq!(loaded.version, SETTINGS_VERSION);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.pane_widths.sidebar, 300);
}

/// P79: a pre-P79 settings.json (no `forgeHosts` key) loads with an empty index
/// and does NOT bump the version or rewrite anything on load.
#[test]
fn old_settings_file_without_forge_hosts_loads_empty() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "dark"
        }"#;
    std::fs::write(&file, json).expect("write pre-P79 settings.json");

    let loaded = load_from(&file);
    assert!(loaded.forge_hosts.is_empty());
    assert_eq!(loaded.version, SETTINGS_VERSION);
}

/// P79: `forgeHosts` survives a load→save→load round-trip with its camelCase
/// wire shape (host + kind + login), and the upsert/remove/backfill helpers
/// behave as the sync rules require.
#[test]
fn forge_hosts_round_trip_and_index_helpers() {
    use bonsai_forge::ForgeKind;

    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);

    // upsert twice on the same host replaces (no duplicate), latest login wins.
    let saved = update(&file, |s| {
        upsert_forge_host(s, "GitHub.com", ForgeKind::GitHub, Some("octocat".into()));
        upsert_forge_host(s, "github.com", ForgeKind::GitHub, Some("octocat2".into()));
        upsert_forge_host(s, "gitlab.com", ForgeKind::GitLab, None);
    })
    .expect("update forge_hosts");
    assert_eq!(saved.forge_hosts.len(), 2);
    let gh = saved
        .forge_hosts
        .iter()
        .find(|r| r.host == "github.com")
        .expect("github record");
    assert_eq!(gh.kind, ForgeKind::GitHub);
    assert_eq!(gh.login.as_deref(), Some("octocat2"));

    // Round-trips through JSON.
    let reloaded = load_from(&file);
    assert_eq!(reloaded.forge_hosts, saved.forge_hosts);

    // backfill only inserts when the host is absent (does not clobber login).
    let after_backfill = update(&file, |s| {
        assert!(!backfill_forge_host(
            s,
            "github.com",
            ForgeKind::GitHub,
            None
        ));
        assert!(backfill_forge_host(
            s,
            "bitbucket.org",
            ForgeKind::Bitbucket,
            Some("bb".into())
        ));
    })
    .expect("backfill");
    assert_eq!(after_backfill.forge_hosts.len(), 3);
    assert_eq!(
        after_backfill
            .forge_hosts
            .iter()
            .find(|r| r.host == "github.com")
            .and_then(|r| r.login.as_deref()),
        Some("octocat2"),
        "backfill must not clobber an existing login"
    );

    // remove drops exactly one host.
    let after_remove = update(&file, |s| remove_forge_host(s, "GITHUB.COM"))
        .expect("remove forge host");
    assert_eq!(after_remove.forge_hosts.len(), 2);
    assert!(!after_remove.forge_hosts.iter().any(|r| r.host == "github.com"));
}

/// A `settings.json` with in-range `recentRepos` but out-of-range/corrupt
/// `paneWidths` values (e.g. hand-edited, or written by a future version
/// with looser bounds) is clamped on load rather than left out-of-range or
/// rejected wholesale (contract §2.1: "load_from calls clamp_pane_widths
/// on the deserialized value before returning").
#[test]
fn corrupt_pane_widths_on_disk_are_clamped_on_load() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "dark",
            "paneWidths": { "sidebar": 0, "rightPanel": 999999 }
        }"#;
    std::fs::write(&file, json).expect("write out-of-range settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.pane_widths.sidebar, SIDEBAR_MIN);
    assert_eq!(loaded.pane_widths.right_panel, RIGHT_PANEL_MAX);
}

/// A `paneWidths.sidebar` field of the wrong JSON type (string instead of
/// number) makes the whole document fail to parse as `Settings` — per the
/// documented "never errors" contract this degrades to full defaults
/// (same as `corrupt_json_defaults`), not a partial/field-level recovery.
/// This pins that behavior so a future switch to field-level tolerant
/// parsing is a deliberate, visible change rather than an accidental
/// regression.
#[test]
fn malformed_field_type_falls_back_to_full_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\x", "lastOpened": 1 } ],
            "theme": "dark",
            "paneWidths": { "sidebar": "not-a-number", "rightPanel": 380 }
        }"#;
    std::fs::write(&file, json).expect("write malformed settings.json");

    assert_eq!(load_from(&file), Settings::default());
}
