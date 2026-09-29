//! P125 §2.2 — the `dep_tracker` setting: default, clamp, on-disk back-compat.
//!
//! Declared with `#[path]` from `lib.rs` (the `settings_defaults_parity_tests`
//! precedent): `settings.rs` is at the file-size limit and may not grow by a
//! `mod` line. Everything used here is public.

use crate::settings::{
    clamp_dep_tracker, load_from, DepTracker, Settings, DEP_TRACKER_INTERVAL_MAX,
    DEP_TRACKER_INTERVAL_MIN,
};

#[test]
fn default_is_off_with_a_twelve_hour_interval() {
    let d = Settings::default().dep_tracker;
    assert_eq!(
        d,
        DepTracker {
            enabled: false,
            interval_hours: 12
        }
    );
}

#[test]
fn clamp_keeps_interval_in_one_to_168_hours() {
    let c = |h| {
        clamp_dep_tracker(DepTracker {
            enabled: true,
            interval_hours: h,
        })
    };
    assert_eq!(c(0).interval_hours, DEP_TRACKER_INTERVAL_MIN);
    assert_eq!(c(999).interval_hours, DEP_TRACKER_INTERVAL_MAX);
    assert_eq!(
        c(36),
        DepTracker {
            enabled: true,
            interval_hours: 36
        }
    );
    assert_eq!(
        (DEP_TRACKER_INTERVAL_MIN, DEP_TRACKER_INTERVAL_MAX),
        (1, 168)
    );
}

#[test]
fn a_pre_p125_file_loads_the_default_and_an_out_of_range_one_is_clamped() {
    let dir = tempfile::TempDir::new().expect("create temp dir");
    let file = dir.path().join("settings.json");
    std::fs::write(&file, r#"{"version":1,"theme":"light"}"#).expect("write legacy file");
    assert_eq!(load_from(&file).dep_tracker, DepTracker::default());

    std::fs::write(
        &file,
        r#"{"version":1,"depTracker":{"enabled":true,"intervalHours":500}}"#,
    )
    .expect("write out-of-range file");
    assert_eq!(
        load_from(&file).dep_tracker,
        DepTracker {
            enabled: true,
            interval_hours: 168
        }
    );

    // A partial object fills the missing field from the default (`serde(default)`).
    std::fs::write(&file, r#"{"version":1,"depTracker":{"enabled":true}}"#)
        .expect("write partial file");
    assert_eq!(
        load_from(&file).dep_tracker,
        DepTracker {
            enabled: true,
            interval_hours: 12
        }
    );
}
