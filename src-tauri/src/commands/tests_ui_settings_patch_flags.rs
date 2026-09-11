//! T2 Area 1 — `set_ui_settings_patch` partial-update semantics, part 2:
//! AI enable/consent, onboarding-seen, auto-check-updates, and external
//! commands patches each mutate independently, leaving every other field
//! untouched. Split out of `tests_ui_settings_patch.rs` (same contract) to
//! keep both files under the file-size limit.

use super::*;

/// The three AI fields patch independently: patching only `ai_enabled`
/// leaves autonomy + consent untouched (and vice versa), and an empty
/// patch mutates nothing (P13 §4.2).
#[test]
fn set_ui_settings_patch_ai_is_partial() {
    let mut s = settings::Settings::default();
    // Defaults sanity: enabled true, ProposeReview, not consented.
    assert!(s.ai_enabled);
    assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);
    assert!(!s.ai_consented);

    // Only `ai_enabled` changes; autonomy + consent untouched.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            ai_enabled: Some(false),
            ..Default::default()
        },
    );
    assert!(!s.ai_enabled);
    assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);
    assert!(!s.ai_consented);
    // Unrelated fields untouched too.
    assert_eq!(s.theme, ThemeChoice::default());

    // Only `ai_consented` changes; enabled + autonomy preserved.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            ai_consented: Some(true),
            ..Default::default()
        },
    );
    assert!(s.ai_consented);
    assert!(!s.ai_enabled);
    assert_eq!(s.ai_conflict_autonomy, AiAutonomy::ProposeReview);

    // Only `ai_conflict_autonomy` changes; enabled + consent preserved.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            ai_conflict_autonomy: Some(AiAutonomy::AutoResolve),
            ..Default::default()
        },
    );
    assert_eq!(s.ai_conflict_autonomy, AiAutonomy::AutoResolve);
    assert!(!s.ai_enabled);
    assert!(s.ai_consented);

    // An empty patch leaves all three AI fields unchanged.
    apply_patch(&mut s, UiSettingsPatch::default());
    assert!(!s.ai_enabled);
    assert_eq!(s.ai_conflict_autonomy, AiAutonomy::AutoResolve);
    assert!(s.ai_consented);
}

/// `onboarding_seen` patches partially like every other field (P43 §6):
/// the default is `false`; a `Some(true)` patch flips it while leaving
/// unrelated fields untouched; and a subsequent empty patch (the common
/// case where the frontend saves an unrelated pref) does NOT reset it back
/// to `false` — pinning the "apply only when Some" property for the field
/// the AI harness can't verify (the mock store resets per browser load).
#[test]
fn set_ui_settings_patch_onboarding_seen_is_partial() {
    let mut s = settings::Settings::default();
    // Default: onboarding not yet seen (⇒ show once).
    assert!(!s.onboarding_seen);

    // Only `onboarding_seen` changes; unrelated fields untouched.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            onboarding_seen: Some(true),
            ..Default::default()
        },
    );
    assert!(s.onboarding_seen);
    assert_eq!(s.theme, ThemeChoice::default());
    assert!(s.ai_enabled);

    // An empty patch (frontend saving some other pref) must NOT clear the
    // persisted flag — this is what keeps onboarding from reappearing.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        },
    );
    assert!(s.onboarding_seen);
    assert_eq!(s.theme, ThemeChoice::Light);

    // A totally empty patch is equally non-destructive.
    apply_patch(&mut s, UiSettingsPatch::default());
    assert!(s.onboarding_seen);
}

/// `auto_check_updates` (P42 D4/INV-4) patches partially like every other
/// bool field: the default is `false`; a `Some(true)` patch flips it while
/// leaving unrelated fields untouched; and a subsequent unrelated patch (or
/// an empty one) does NOT reset it — pinning the "apply only when Some"
/// property for the auto-check-on-launch flag the AI harness can't verify
/// (the mock settings store resets per browser load). Mirrors
/// `set_ui_settings_patch_onboarding_seen_is_partial`.
#[test]
fn set_ui_settings_patch_auto_check_updates_is_partial() {
    let mut s = settings::Settings::default();
    // Default: auto-check OFF (D4 — no surprise outbound call on launch).
    assert!(!s.auto_check_updates);

    // Only `auto_check_updates` changes; unrelated fields untouched.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            auto_check_updates: Some(true),
            ..Default::default()
        },
    );
    assert!(s.auto_check_updates);
    assert_eq!(s.theme, ThemeChoice::default());
    assert!(!s.onboarding_seen);

    // An unrelated patch (frontend saving some other pref) must NOT clear
    // the persisted flag.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        },
    );
    assert!(s.auto_check_updates);
    assert_eq!(s.theme, ThemeChoice::Light);

    // A totally empty patch is equally non-destructive.
    apply_patch(&mut s, UiSettingsPatch::default());
    assert!(s.auto_check_updates);

    // And it can be explicitly turned back off via `Some(false)`.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            auto_check_updates: Some(false),
            ..Default::default()
        },
    );
    assert!(!s.auto_check_updates);
}

/// P49: `terminal_command`/`editor_command` patch independently — a `Some`
/// overwrites, a `None` (including an empty/unrelated patch) leaves the
/// stored value untouched, and `Some("")` explicitly resets to auto-detect.
#[test]
fn set_ui_settings_patch_external_commands_is_partial() {
    let mut s = settings::Settings::default();
    assert_eq!(s.terminal_command, "");
    assert_eq!(s.editor_command, "");

    // Only `terminal_command` changes; the editor + unrelated fields stay.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            terminal_command: Some("wt".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "wt");
    assert_eq!(s.editor_command, "");
    assert_eq!(s.theme, ThemeChoice::default());

    // Only `editor_command` changes; the terminal value is preserved.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_command: Some("code".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "wt");
    assert_eq!(s.editor_command, "code");

    // An unrelated patch does NOT clear either command.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "wt");
    assert_eq!(s.editor_command, "code");

    // An empty patch is equally non-destructive.
    apply_patch(&mut s, UiSettingsPatch::default());
    assert_eq!(s.terminal_command, "wt");
    assert_eq!(s.editor_command, "code");

    // `Some("")` explicitly resets a command back to auto-detect.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            terminal_command: Some(String::new()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "");
    assert_eq!(s.editor_command, "code");

    // 2026-09-11 (audit MEDIUM-2): `apply_patch` deliberately does NOT validate
    // the shape — a value the LAUNCHER will refuse still persists verbatim. The
    // settings writer merges every pending key into ONE patch and re-queues it
    // on failure, so rejecting one key here would wedge every later settings
    // write; the refusal belongs at the consumption point
    // (`external_cmd::validate_command_setting`), where it names the setting.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_command: Some("powershell -c calc".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.editor_command, "powershell -c calc", "stored as typed, refused at launch");
}

/// Spec-004: `graphFoldLinear` patches independently (camelCase on the wire)
/// and an absent key leaves it untouched.
#[test]
fn set_ui_settings_patch_graph_fold_linear_is_partial() {
    let mut s = settings::Settings::default();
    assert!(!s.graph_fold_linear);

    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{ "graphFoldLinear": true }"#).expect("fold patch");
    apply_patch(&mut s, patch);
    assert!(s.graph_fold_linear);
    assert!(!s.graph_first_parent, "sibling pref untouched");

    // An absent key leaves it unchanged.
    let patch: UiSettingsPatch = serde_json::from_str(r#"{ "theme": "light" }"#).expect("patch");
    apply_patch(&mut s, patch);
    assert!(s.graph_fold_linear);
}

/// Spec-005: `graphMinimapAlwaysShow` patches independently (camelCase on the
/// wire) and an absent key leaves it untouched.
#[test]
fn set_ui_settings_patch_graph_minimap_always_show_is_partial() {
    let mut s = settings::Settings::default();
    assert!(!s.graph_minimap_always_show);

    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{ "graphMinimapAlwaysShow": true }"#).expect("minimap patch");
    apply_patch(&mut s, patch);
    assert!(s.graph_minimap_always_show);
    assert!(!s.graph_fold_linear, "sibling pref untouched");

    // An absent key leaves it unchanged.
    let patch: UiSettingsPatch = serde_json::from_str(r#"{ "theme": "light" }"#).expect("patch");
    apply_patch(&mut s, patch);
    assert!(s.graph_minimap_always_show);
}

/// Spec-006: `graphColorMode` patches independently (camelCase key, lowercase
/// value on the wire) and an absent key leaves it untouched.
#[test]
fn set_ui_settings_patch_graph_color_mode_is_partial() {
    let mut s = settings::Settings::default();
    assert_eq!(s.graph_color_mode, settings::GraphColorMode::Lane);

    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{ "graphColorMode": "author" }"#).expect("color-mode patch");
    apply_patch(&mut s, patch);
    assert_eq!(s.graph_color_mode, settings::GraphColorMode::Author);
    assert!(!s.graph_fold_linear, "sibling pref untouched");

    // An absent key leaves it unchanged.
    let patch: UiSettingsPatch = serde_json::from_str(r#"{ "theme": "light" }"#).expect("patch");
    apply_patch(&mut s, patch);
    assert_eq!(s.graph_color_mode, settings::GraphColorMode::Author);
}

/// Spec-003: the two declutter prefs patch independently, and the
/// `graphRefFilter` double-option distinguishes ABSENT (leave unchanged) from
/// an explicit `null` (clear) on the wire.
#[test]
fn set_ui_settings_patch_graph_declutter_is_partial() {
    let mut s = settings::Settings::default();
    assert!(!s.graph_first_parent);
    assert_eq!(s.graph_ref_filter, None);

    // Only `graph_first_parent` changes; the ref filter untouched.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            graph_first_parent: Some(true),
            ..Default::default()
        },
    );
    assert!(s.graph_first_parent);
    assert_eq!(s.graph_ref_filter, None);

    // Set the ref filter; first-parent preserved.
    let filter = GraphRefFilter {
        mode: settings::RefFilterMode::Hide,
        refs: vec!["refs/heads/wip".to_string()],
    };
    apply_patch(
        &mut s,
        UiSettingsPatch {
            graph_ref_filter: Some(Some(filter.clone())),
            ..Default::default()
        },
    );
    assert_eq!(s.graph_ref_filter, Some(filter.clone()));
    assert!(s.graph_first_parent);

    // An ABSENT key on the wire leaves the filter unchanged...
    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{ "theme": "light" }"#).expect("patch without graphRefFilter");
    assert_eq!(patch.graph_ref_filter, None, "missing key → don't touch");
    apply_patch(&mut s, patch);
    assert_eq!(s.graph_ref_filter, Some(filter));

    // ...while an explicit `null` CLEARS it.
    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{ "graphRefFilter": null }"#).expect("patch with null");
    assert_eq!(patch.graph_ref_filter, Some(None), "null → clear");
    apply_patch(&mut s, patch);
    assert_eq!(s.graph_ref_filter, None);
    assert!(s.graph_first_parent, "sibling pref untouched by the clear");

    // And a full value round-trips through the wire patch too.
    let patch: UiSettingsPatch = serde_json::from_str(
        r#"{ "graphRefFilter": { "mode": "solo", "refs": ["refs/heads/main"] } }"#,
    )
    .expect("patch with value");
    apply_patch(&mut s, patch);
    assert_eq!(
        s.graph_ref_filter,
        Some(GraphRefFilter {
            mode: settings::RefFilterMode::Solo,
            refs: vec!["refs/heads/main".to_string()],
        })
    );
}

/// P91 §10: `dev` is a WHOLE-STRUCT patch (the `autoFetch` precedent) with
/// camelCase keys, it patches independently of its siblings, and an absent key
/// leaves the persisted value alone. A pre-P91 settings blob (no `dev` key)
/// loads Dev mode OFF with strict redaction.
#[test]
fn set_ui_settings_patch_dev_is_partial() {
    let mut s = settings::Settings::default();
    assert!(!s.dev.enabled, "Dev mode is off by default");
    assert!(!s.dev.include_raw_names, "strict redaction by default");
    assert_eq!(s.dev.level, crate::obs::record::LogLevel::Debug);

    let patch: UiSettingsPatch = serde_json::from_str(
        r#"{ "dev": { "enabled": true, "level": "trace", "captureIpc": false,
                      "captureReact": true, "captureFrames": true,
                      "includeRawNames": true } }"#,
    )
    .expect("dev patch");
    apply_patch(&mut s, patch);
    assert!(s.dev.enabled);
    assert_eq!(s.dev.level, crate::obs::record::LogLevel::Trace);
    assert!(!s.dev.capture_ipc);
    assert!(s.dev.capture_frames);
    assert!(s.dev.include_raw_names);
    assert_eq!(s.theme, settings::ThemeChoice::Dark, "sibling untouched");

    // An absent key leaves the whole struct unchanged.
    let patch: UiSettingsPatch = serde_json::from_str(r#"{ "theme": "light" }"#).expect("patch");
    apply_patch(&mut s, patch);
    assert!(s.dev.enabled);
    assert_eq!(s.dev.level, crate::obs::record::LogLevel::Trace);

    // A partial `dev` object relies on the struct-level `#[serde(default)]`:
    // omitted sub-fields load their defaults, never garbage.
    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{ "dev": { "enabled": true } }"#).expect("partial dev");
    apply_patch(&mut s, patch);
    assert!(s.dev.enabled);
    assert!(!s.dev.include_raw_names, "omitted sub-field takes its default");
}

/// P91 §10 back-compat: a settings.json written before P91 loads with Dev mode
/// off — the additive `#[serde(default)]` promise, asserted rather than assumed.
#[test]
fn legacy_settings_without_dev_key_loads_dev_off() {
    let s: settings::Settings =
        serde_json::from_str(r#"{ "version": 1, "recentRepos": [] }"#).expect("legacy blob");
    assert_eq!(s.dev, settings::DevSettings::default());
    assert!(!s.dev.enabled);
}
