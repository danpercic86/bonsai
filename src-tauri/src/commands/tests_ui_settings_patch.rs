//! T2 Area 1 — `set_ui_settings_patch` partial-update semantics, part 1:
//! theme/pane_widths/list_view, AI run fields, and auto-fetch + graph
//! patches each mutate independently, leaving every other field untouched
//! (P2a contract §3.4.3; P3b contract §2.1; P67 §7.2 for `panel_density`).

use super::*;

/// Patching only `theme` leaves `pane_widths`/`list_view` untouched, and
/// each other single-field patch is equally partial (P2a contract §3.4.3;
/// P3b contract §2.1; P67 §7.2 for `panel_density`).
#[test]
fn set_ui_settings_patch_is_partial() {
    let mut s = settings::Settings::default();
    let original_widths = s.pane_widths;

    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            pane_widths: None,
            list_view: None,
            ..Default::default()
        },
    );
    assert_eq!(s.theme, ThemeChoice::Light);
    assert_eq!(s.pane_widths, original_widths);
    assert_eq!(s.list_view, settings::ListView::Tree);

    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: None,
            pane_widths: Some(PaneWidths {
                sidebar: 300,
                right_panel: 400,
            }),
            list_view: None,
            ..Default::default()
        },
    );
    assert_eq!(s.theme, ThemeChoice::Light); // untouched by the second patch
    assert_eq!(s.list_view, settings::ListView::Tree);
    assert_eq!(
        s.pane_widths,
        PaneWidths {
            sidebar: 300,
            right_panel: 400,
        }
    );

    // Patching only `list_view` leaves theme + pane widths untouched.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: None,
            pane_widths: None,
            list_view: Some(settings::ListView::Flat),
            ..Default::default()
        },
    );
    assert_eq!(s.list_view, settings::ListView::Flat);
    assert_eq!(s.theme, ThemeChoice::Light);
    assert_eq!(
        s.pane_widths,
        PaneWidths {
            sidebar: 300,
            right_panel: 400,
        }
    );

    // Give `graph` a NON-default value first. Without this, asserting
    // `graph == GraphPrefs::default()` after the density patch would only
    // prove the patch never *introduced* a graph change — not that it left
    // an existing one alone. 40 is inside the row-height clamp range, so
    // the write-side clamp cannot rewrite it and muddy the assertion.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            graph: Some(settings::GraphPrefs {
                row_height: 40,
                ..settings::GraphPrefs::default()
            }),
            ..Default::default()
        },
    );
    let graph_before = s.graph; // GraphPrefs is Copy
    assert_ne!(graph_before, settings::GraphPrefs::default());

    // P67 §7.2: patching only `panel_density` leaves list_view / graph /
    // theme untouched — the independence claim behind D6 (density is NOT a
    // graph pref and is never routed through `clamp_graph_prefs`).
    apply_patch(
        &mut s,
        UiSettingsPatch {
            panel_density: Some(settings::PanelDensity::Compact),
            ..Default::default()
        },
    );
    assert_eq!(s.panel_density, settings::PanelDensity::Compact);
    assert_eq!(s.list_view, settings::ListView::Flat); // from the patch above
    assert_eq!(s.graph, graph_before); // the non-default survives untouched
    assert_eq!(s.theme, ThemeChoice::Light);

    // Out-of-range pane widths in a patch get clamped on write.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: None,
            pane_widths: Some(PaneWidths {
                sidebar: 5,
                right_panel: 5000,
            }),
            list_view: None,
            ..Default::default()
        },
    );
    assert_eq!(s.pane_widths.sidebar, settings::SIDEBAR_MIN);
    assert_eq!(s.pane_widths.right_panel, settings::RIGHT_PANEL_MAX);
}

/// P68 §8.3: each of the ten streaming-AI knobs patches INDEPENDENTLY of
/// `graph` / `listView` / `panelDensity` (and of each other), is clamped on
/// write, and keeps its documented `0` sentinels — a plain `clamp` would turn
/// "no hard cap" into 60 s and quietly restore the deadline P68 removes.
#[test]
fn set_ui_settings_patch_ai_run_fields() {
    let mut s = settings::Settings::default();
    // Non-default neighbours first, so "untouched" means something.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            list_view: Some(settings::ListView::Flat),
            panel_density: Some(settings::PanelDensity::Compact),
            graph: Some(settings::GraphPrefs {
                row_height: 40,
                ..settings::GraphPrefs::default()
            }),
            ..Default::default()
        },
    );
    let graph_before = s.graph;

    apply_patch(
        &mut s,
        UiSettingsPatch {
            ai_idle_timeout_secs: Some(600),
            ..Default::default()
        },
    );
    assert_eq!(s.ai_idle_timeout_secs, 600);
    // Nothing else moved — not even the other nine AI fields.
    assert_eq!(s.ai_hard_cap_secs, 0);
    assert_eq!(s.ai_max_turns, bonsai_core::ai::DEFAULT_MAX_TURNS);
    assert!(s.ai_stream_log);
    assert_eq!(s.ai_conflict_tools, settings::AiConflictTools::ReadOnly);
    assert_eq!(s.list_view, settings::ListView::Flat);
    assert_eq!(s.panel_density, settings::PanelDensity::Compact);
    assert_eq!(s.graph, graph_before);

    // Each remaining field patches on its own.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            ai_conflict_tools: Some(settings::AiConflictTools::None),
            ai_stream_log: Some(false),
            ai_include_partial_messages: Some(true),
            ai_dock_collapsed: Some(true),
            ai_dock_height: Some(320),
            ai_bulk_max_bytes: Some(250_000),
            ai_max_budget_usd: Some(2.5),
            ai_hard_cap_secs: Some(900),
            ai_max_turns: Some(3),
            ..Default::default()
        },
    );
    assert_eq!(s.ai_conflict_tools, settings::AiConflictTools::None);
    assert!(!s.ai_stream_log);
    assert!(s.ai_include_partial_messages);
    assert!(s.ai_dock_collapsed);
    assert_eq!(s.ai_dock_height, 320);
    assert_eq!(s.ai_bulk_max_bytes, 250_000);
    assert_eq!(s.ai_max_budget_usd, 2.5);
    assert_eq!(s.ai_hard_cap_secs, 900);
    assert_eq!(s.ai_max_turns, 3);
    assert_eq!(s.ai_idle_timeout_secs, 600, "the earlier patch survives");

    // Out-of-range values are clamped on write; the `0` sentinels are not.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            ai_idle_timeout_secs: Some(0),
            ai_hard_cap_secs: Some(0),
            ai_max_turns: Some(999),
            ai_bulk_max_bytes: Some(1),
            ai_max_budget_usd: Some(1e9),
            ai_dock_height: Some(9_000),
            ..Default::default()
        },
    );
    assert_eq!(s.ai_idle_timeout_secs, 0, "0 = watchdog disabled");
    assert_eq!(s.ai_hard_cap_secs, 0, "0 = unbounded");
    assert_eq!(s.ai_max_turns, settings::AI_MAX_TURNS_MAX);
    assert_eq!(s.ai_bulk_max_bytes, settings::AI_BULK_MAX_BYTES_MIN);
    assert_eq!(s.ai_max_budget_usd, settings::AI_MAX_BUDGET_USD_MAX);
    assert_eq!(s.ai_dock_height, settings::AI_DOCK_HEIGHT_MAX);
}

/// `auto_fetch` and `graph` patch independently, leave the other fields
/// unchanged when `None`, and are clamped on write (P11 §2.4).
#[test]
fn set_ui_settings_patch_auto_fetch_and_graph() {
    let mut s = settings::Settings::default();
    let original_af = s.auto_fetch;
    let original_graph = s.graph;

    // Only `auto_fetch` changes auto-fetch; everything else untouched.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            auto_fetch: Some(AutoFetch {
                enabled: true,
                interval_minutes: 20,
            }),
            ..Default::default()
        },
    );
    assert_eq!(
        s.auto_fetch,
        AutoFetch {
            enabled: true,
            interval_minutes: 20,
        }
    );
    assert_eq!(s.graph, original_graph);
    assert_eq!(s.theme, ThemeChoice::default());

    // Only `graph` changes graph; auto-fetch preserved from the prior patch.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            graph: Some(GraphPrefs {
                avatar_radius: 12,
                row_height: 36,
                lane_width: 20,
                ..GraphPrefs::default()
            }),
            ..Default::default()
        },
    );
    assert_eq!(
        s.graph,
        GraphPrefs {
            avatar_radius: 12,
            row_height: 36,
            lane_width: 20,
            ..GraphPrefs::default()
        }
    );
    assert_eq!(
        s.auto_fetch,
        AutoFetch {
            enabled: true,
            interval_minutes: 20,
        }
    );

    // An empty patch leaves both new fields unchanged.
    apply_patch(&mut s, UiSettingsPatch::default());
    assert_eq!(
        s.auto_fetch,
        AutoFetch {
            enabled: true,
            interval_minutes: 20,
        }
    );
    assert_eq!(
        s.graph,
        GraphPrefs {
            avatar_radius: 12,
            row_height: 36,
            lane_width: 20,
            ..GraphPrefs::default()
        }
    );

    // Out-of-range interval (0) clamps to the min on write.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            auto_fetch: Some(AutoFetch {
                enabled: true,
                interval_minutes: 0,
            }),
            ..Default::default()
        },
    );
    assert_eq!(s.auto_fetch.interval_minutes, settings::AUTO_FETCH_INTERVAL_MIN);

    // Out-of-range interval (999) clamps to the max on write.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            auto_fetch: Some(AutoFetch {
                enabled: false,
                interval_minutes: 999,
            }),
            ..Default::default()
        },
    );
    assert_eq!(s.auto_fetch.interval_minutes, settings::AUTO_FETCH_INTERVAL_MAX);

    // Below-min / above-max graph knobs clamp to their bounds on write.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            graph: Some(GraphPrefs {
                avatar_radius: 9999,
                row_height: 0,
                lane_width: 9999,
                ..GraphPrefs::default()
            }),
            ..Default::default()
        },
    );
    assert_eq!(s.graph.avatar_radius, settings::AVATAR_RADIUS_MAX);
    assert_eq!(s.graph.row_height, settings::ROW_HEIGHT_MIN);
    assert_eq!(s.graph.lane_width, settings::LANE_WIDTH_MAX);

    // Sanity: the `original_*` snapshots were genuinely the defaults.
    assert_eq!(original_af, AutoFetch::default());
    assert_eq!(original_graph, GraphPrefs::default());
}
