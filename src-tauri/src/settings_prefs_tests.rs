//! Unit tests for [`super`] (`settings.rs`): auto-fetch, graph preferences, and
//! health-refresh — clamping, round-trips, and on-disk back-compat.
//!
//! Kept in a sibling file so `settings.rs` stays closer to the ~500-line soft
//! limit. Declared with `#[path]` as a child module of `settings`, so
//! `super::*` still reaches the private items without widening their
//! visibility (the `external_tests` / `session_drain_tests` convention).

use super::*;
use super::tests::settings_path;

/// Below-min and above-max clamp to their bounds on each graph knob and on
/// the auto-fetch interval; in-range values pass through (P11 §2.1/§2.4).
#[test]
fn clamp_auto_fetch_and_graph_prefs_clamp_ranges() {
    // Interval below-min (0) and above-max (999) clamp to 1/120; `enabled`
    // is carried through untouched.
    assert_eq!(
        clamp_auto_fetch(AutoFetch {
            enabled: true,
            interval_minutes: 0,
        }),
        AutoFetch {
            enabled: true,
            interval_minutes: AUTO_FETCH_INTERVAL_MIN,
        }
    );
    assert_eq!(
        clamp_auto_fetch(AutoFetch {
            enabled: false,
            interval_minutes: 999,
        }),
        AutoFetch {
            enabled: false,
            interval_minutes: AUTO_FETCH_INTERVAL_MAX,
        }
    );
    let in_range = AutoFetch {
        enabled: true,
        interval_minutes: 30,
    };
    assert_eq!(clamp_auto_fetch(in_range), in_range);

    // Each graph knob below-min clamps to its min (toggles pass through).
    assert_eq!(
        clamp_graph_prefs(GraphPrefs {
            avatar_radius: 0,
            row_height: 0,
            lane_width: 0,
            ..GraphPrefs::default()
        }),
        GraphPrefs {
            avatar_radius: AVATAR_RADIUS_MIN,
            row_height: ROW_HEIGHT_MIN,
            lane_width: LANE_WIDTH_MIN,
            ..GraphPrefs::default()
        }
    );
    // Each graph knob above-max clamps to its max.
    assert_eq!(
        clamp_graph_prefs(GraphPrefs {
            avatar_radius: 9999,
            row_height: 9999,
            lane_width: 9999,
            ..GraphPrefs::default()
        }),
        GraphPrefs {
            avatar_radius: AVATAR_RADIUS_MAX,
            row_height: ROW_HEIGHT_MAX,
            lane_width: LANE_WIDTH_MAX,
            ..GraphPrefs::default()
        }
    );
    // In-range graph knobs pass through unchanged.
    let g_in = GraphPrefs {
        avatar_radius: 12,
        row_height: 36,
        lane_width: 20,
        ..GraphPrefs::default()
    };
    assert_eq!(clamp_graph_prefs(g_in), g_in);
}

/// Save/load a `Settings` with non-default `auto_fetch` + `graph` round-trips
/// exactly (P11 §2.4).
#[test]
fn auto_fetch_and_graph_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        auto_fetch: AutoFetch {
            enabled: true,
            interval_minutes: 15,
        },
        graph: GraphPrefs {
            avatar_radius: 14,
            row_height: 40,
            lane_width: 24,
            ..GraphPrefs::default()
        },
        ..Default::default()
    };

    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert!(loaded.auto_fetch.enabled);
    assert_eq!(loaded.auto_fetch.interval_minutes, 15);
    assert_eq!(loaded.graph.avatar_radius, 14);
    assert_eq!(loaded.graph.lane_width, 24);
}

/// A hand-edited file with out-of-range `autoFetch`/`graph` values is clamped
/// on load rather than left out-of-range (P11 §2.1 — `load_from` clamps both
/// new structs on read).
#[test]
fn corrupt_auto_fetch_and_graph_on_disk_are_clamped_on_load() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "dark",
            "paneWidths": { "sidebar": 240, "rightPanel": 380 },
            "autoFetch": { "enabled": true, "intervalMinutes": 0 },
            "graph": { "avatarRadius": 99, "rowHeight": 1, "laneWidth": 999 }
        }"#;
    std::fs::write(&file, json).expect("write out-of-range settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.auto_fetch.interval_minutes, AUTO_FETCH_INTERVAL_MIN);
    assert!(loaded.auto_fetch.enabled);
    assert_eq!(loaded.graph.avatar_radius, AVATAR_RADIUS_MAX);
    assert_eq!(loaded.graph.row_height, ROW_HEIGHT_MIN);
    assert_eq!(loaded.graph.lane_width, LANE_WIDTH_MAX);
}

/// P51: non-default detail toggles + a `Committer` date basis round-trip
/// through save/load, and the raw JSON carries the documented camelCase
/// keys + the lowercase basis value.
#[test]
fn graph_prefs_toggles_roundtrip() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        graph: GraphPrefs {
            avatar_radius: 12,
            row_height: 30,
            lane_width: 18,
            show_sha: false,
            show_author: true,
            show_date: false,
            date_basis: GraphDateBasis::Committer,
            show_ahead_behind: false,
            compact: true,
            show_signature_badge: false,
            // P63: both forge-badge toggles flipped to their NON-default
            // (true) so the round-trip proves they persist.
            show_pr_badge: true,
            show_ci_status: true,
        },
        ..Default::default()
    };
    save_to(&file, &s).expect("save settings");
    let loaded = load_from(&file);
    assert_eq!(loaded, s);
    assert!(!loaded.graph.show_sha);
    assert!(loaded.graph.show_author);
    assert!(!loaded.graph.show_date);
    assert_eq!(loaded.graph.date_basis, GraphDateBasis::Committer);
    assert!(!loaded.graph.show_ahead_behind);
    assert!(loaded.graph.compact);
    assert!(!loaded.graph.show_signature_badge);
    assert!(loaded.graph.show_pr_badge);
    assert!(loaded.graph.show_ci_status);

    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"showSha\": false"));
    assert!(raw.contains("\"dateBasis\": \"committer\""));
    assert!(raw.contains("\"compact\": true"));
    assert!(raw.contains("\"showSignatureBadge\": false"));
    assert!(raw.contains("\"showPrBadge\": true"));
    assert!(raw.contains("\"showCiStatus\": true"));
}

/// P51 D7 back-compat: a legacy `graph` object that still carries the
/// removed `dotRadius` field and NONE of the new P51 toggle keys loads
/// without error — `dotRadius` is silently ignored (serde has no
/// `deny_unknown_fields`) and every new toggle falls back to its
/// `#[serde(default)]` default. Pins both the dead-field removal and the
/// additive-toggle guarantee.
#[test]
fn old_graph_prefs_with_dot_radius_ignored() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "graph": { "dotRadius": 4, "avatarRadius": 12, "rowHeight": 30, "laneWidth": 18 }
        }"#;
    std::fs::write(&file, json).expect("write legacy graph settings.json");

    let loaded = load_from(&file);
    // Geometry from the legacy file is preserved (and clamped in-range).
    assert_eq!(loaded.graph.avatar_radius, 12);
    assert_eq!(loaded.graph.row_height, 30);
    assert_eq!(loaded.graph.lane_width, 18);
    // Every new P51 toggle falls back to its default (the `dotRadius` key
    // in the file is ignored, not an error).
    assert!(loaded.graph.show_sha);
    assert!(!loaded.graph.show_author);
    assert!(loaded.graph.show_date);
    assert_eq!(loaded.graph.date_basis, GraphDateBasis::Author);
    assert!(loaded.graph.show_ahead_behind);
    assert!(!loaded.graph.compact);
    // P58c: a legacy `graph` object without `showSignatureBadge` loads with
    // it defaulted true (the badge is on unless explicitly turned off).
    assert!(loaded.graph.show_signature_badge);
    // P63: a legacy `graph` object without the forge-badge keys loads with
    // BOTH defaulted false (network+auth-gated — opt-in, so a fresh/legacy
    // file never fires surprise forge API calls).
    assert!(!loaded.graph.show_pr_badge);
    assert!(!loaded.graph.show_ci_status);
}

/// An old `settings.json` written before P11 (no `autoFetch`/`graph` keys)
/// loads with the type defaults for the new fields and preserves existing
/// ones — the additive-field guarantee for the P11 settings specifically
/// (P11 §2.4).
#[test]
fn old_settings_file_without_auto_fetch_graph_loads_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [ { "path": "D:\\Repos\\legacy", "lastOpened": 123 } ],
            "theme": "light",
            "paneWidths": { "sidebar": 300, "rightPanel": 400 },
            "listView": "flat"
        }"#;
    std::fs::write(&file, json).expect("write pre-P11 settings.json");

    let loaded = load_from(&file);
    assert_eq!(loaded.auto_fetch, AutoFetch::default());
    assert_eq!(loaded.graph, GraphPrefs::default());
    // Existing fields untouched.
    assert_eq!(loaded.theme, ThemeChoice::Light);
    assert_eq!(loaded.list_view, ListView::Flat);
    assert_eq!(loaded.recent_repos.len(), 1);
}

/// An unrecognized `theme` string (e.g. from a hypothetical future third
/// theme, or a hand-typo'd file) fails the whole-document parse the same
/// way a malformed field type does — pinned for the same reason as above.
#[test]
fn unknown_theme_string_falls_back_to_full_defaults() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let json = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "solarized",
            "paneWidths": { "sidebar": 240, "rightPanel": 380 }
        }"#;
    std::fs::write(&file, json).expect("write unknown-theme settings.json");

    assert_eq!(load_from(&file), Settings::default());
}

/// `clamp_health_refresh` clamps below-min/above-max intervals, passes
/// in-range through, and carries `enabled` untouched (P30 D12).
#[test]
fn clamp_health_refresh_clamps_range() {
    assert_eq!(
        clamp_health_refresh(HealthRefresh {
            enabled: true,
            interval_minutes: 0,
        }),
        HealthRefresh {
            enabled: true,
            interval_minutes: HEALTH_REFRESH_INTERVAL_MIN,
        }
    );
    assert_eq!(
        clamp_health_refresh(HealthRefresh {
            enabled: false,
            interval_minutes: 9999,
        }),
        HealthRefresh {
            enabled: false,
            interval_minutes: HEALTH_REFRESH_INTERVAL_MAX,
        }
    );
    let in_range = HealthRefresh {
        enabled: true,
        interval_minutes: 60,
    };
    assert_eq!(clamp_health_refresh(in_range), in_range);
}

/// Non-default `health_refresh` round-trips; a pre-P30 file without the
/// key loads the default; an out-of-range on-disk value is clamped on
/// load (P30 §3 — serde back-compat guarantee for the new field).
#[test]
fn health_refresh_roundtrip_backcompat_and_clamp() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = settings_path(&dir);
    let s = Settings {
        health_refresh: HealthRefresh {
            enabled: true,
            interval_minutes: 45,
        },
        ..Default::default()
    };
    save_to(&file, &s).expect("save settings");
    assert_eq!(load_from(&file), s);
    let raw = std::fs::read_to_string(&file).expect("read settings.json");
    assert!(raw.contains("\"healthRefresh\""));

    // Pre-P30 file (no healthRefresh key) → default, other fields kept.
    let legacy = r#"{
            "version": 1,
            "recentRepos": [],
            "theme": "light",
            "autoFetch": { "enabled": true, "intervalMinutes": 15 }
        }"#;
    std::fs::write(&file, legacy).expect("write legacy settings.json");
    let loaded = load_from(&file);
    assert_eq!(loaded.health_refresh, HealthRefresh::default());
    assert!(loaded.auto_fetch.enabled);
    assert_eq!(loaded.theme, ThemeChoice::Light);

    // Out-of-range on-disk value is clamped on load.
    let corrupt = r#"{
            "version": 1,
            "recentRepos": [],
            "healthRefresh": { "enabled": true, "intervalMinutes": 0 }
        }"#;
    std::fs::write(&file, corrupt).expect("write corrupt settings.json");
    let loaded = load_from(&file);
    assert_eq!(
        loaded.health_refresh.interval_minutes,
        HEALTH_REFRESH_INTERVAL_MIN
    );
    assert!(loaded.health_refresh.enabled);
}
