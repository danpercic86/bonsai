//! P125 §2.2 / §8 — the `depTracker` setting through `set_ui_settings`: the
//! whole-struct patch is clamped, echoed back by `ui_settings_of`, leaves
//! every other field untouched, and an absent patch field changes nothing
//! (`[P125f review]` echo test, next to `tests_ui_settings_patch.rs`).

use super::*;

#[test]
fn a_dep_tracker_patch_is_clamped_echoed_and_partial() {
    let mut s = settings::Settings::default();
    let before = s.clone();
    apply_patch(
        &mut s,
        UiSettingsPatch {
            dep_tracker: Some(DepTracker {
                enabled: true,
                interval_hours: 500,
            }),
            ..Default::default()
        },
    );
    let want = DepTracker {
        enabled: true,
        interval_hours: 168,
    };
    assert_eq!(s.dep_tracker, want);
    assert_eq!(ui_settings_of(&s).dep_tracker, want);
    // Nothing else moved.
    let mut rest = s.clone();
    rest.dep_tracker = before.dep_tracker;
    assert_eq!(rest, before);

    // A patch without `depTracker` leaves it alone.
    apply_patch(
        &mut s,
        UiSettingsPatch {
            theme: Some(ThemeChoice::Light),
            ..Default::default()
        },
    );
    assert_eq!(s.dep_tracker, want);
}

#[test]
fn the_wire_patch_and_echo_use_camel_case() {
    let patch: UiSettingsPatch =
        serde_json::from_str(r#"{"depTracker":{"enabled":true,"intervalHours":0}}"#)
            .expect("patch deserializes");
    let mut s = settings::Settings::default();
    apply_patch(&mut s, patch);
    let echo = serde_json::to_value(ui_settings_of(&s)).expect("serialize");
    assert_eq!(
        echo["depTracker"],
        serde_json::json!({ "enabled": true, "intervalHours": 1 })
    );
}
