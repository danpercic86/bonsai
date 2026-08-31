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
            terminal_command: Some("wt -d {path}".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "wt -d {path}");
    assert_eq!(s.editor_command, "");
    assert_eq!(s.theme, ThemeChoice::default());

    // Only `editor_command` changes; the terminal value is preserved.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            editor_command: Some("code {path}".to_string()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "wt -d {path}");
    assert_eq!(s.editor_command, "code {path}");

    // An unrelated patch does NOT clear either command.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "wt -d {path}");
    assert_eq!(s.editor_command, "code {path}");

    // An empty patch is equally non-destructive.
    apply_patch(&mut s, UiSettingsPatch::default());
    assert_eq!(s.terminal_command, "wt -d {path}");
    assert_eq!(s.editor_command, "code {path}");

    // `Some("")` explicitly resets a command back to auto-detect.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            terminal_command: Some(String::new()),
            ..Default::default()
        },
    );
    assert_eq!(s.terminal_command, "");
    assert_eq!(s.editor_command, "code {path}");
}
