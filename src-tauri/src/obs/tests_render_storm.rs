//! `render-storm` under React StrictMode (user ruling 2026-09-28).
//!
//! `renders` on a `render.tally` counts render-BODY invocations; StrictMode runs
//! each body twice in a dev build while `instances` is minted once per instance.
//! The rule is `3×` REAL renders, so a record whose producer states
//! `strictModeDoubling: true` is judged against a doubled budget. Records
//! without the flag (non-StrictMode builds, every pre-flag record) keep the
//! original `renders > 3 * instances`.

use super::tests_anomaly_support::*;
use crate::obs::record::{LogPayload, LogRecord};

fn detail_of(r: &LogRecord) -> &str {
    match &r.payload {
        LogPayload::Anomaly { detail, .. } => detail,
        other => panic!("expected an anomaly, got {}", other.kind()),
    }
}

fn storms(renders: u64, instances: u64, doubling: bool) -> usize {
    let mut h = H::new();
    h.feed(render_tally_with(1000, "Row", renders, instances, doubling));
    h.count("render-storm")
}

/// The 423-firings case: one real commit per instance per window reads as 2×
/// raw under StrictMode. It must not fire.
#[test]
fn doubled_one_real_render_per_instance_does_not_fire() {
    assert_eq!(storms(4, 2, true), 0);
}

/// Two real renders per instance (4× raw) — the case the old rule tripped on
/// (`react.ts` documented it) — is well inside a 3× real budget.
#[test]
fn doubled_two_real_renders_per_instance_does_not_fire() {
    assert_eq!(storms(8, 2, true), 0);
}

/// Exactly 3× real (6× raw) is the boundary: `>` not `>=`, as before.
#[test]
fn doubled_exactly_three_real_renders_per_instance_does_not_fire() {
    assert_eq!(storms(12, 2, true), 0);
}

/// Above 3× real renders fires, and the detail reports REAL renders plus the
/// raw count so the message no longer overstates by 2×.
#[test]
fn doubled_over_three_real_renders_per_instance_fires_with_real_count() {
    let mut h = H::new();
    h.feed(render_tally_with(1000, "Row", 14, 2, true)); // 7 real > 3*2
    assert_eq!(h.count("render-storm"), 1);
    let a = h.find("render-storm").expect("fires");
    assert_eq!(
        detail_of(a),
        "Row: 7 renders vs 2 instances in window (14 raw, StrictMode-doubled)"
    );
}

/// Non-StrictMode behaviour is unchanged: the same raw counts are judged 1:1.
#[test]
fn undoubled_threshold_is_unchanged() {
    assert_eq!(storms(6, 2, false), 0);
    assert_eq!(storms(7, 2, false), 1);
    let mut h = H::new();
    h.feed(render_tally(1000, "Row", 7, 2));
    let a = h.find("render-storm").expect("fires");
    assert_eq!(detail_of(a), "Row: 7 renders vs 2 instances in window");
}

/// Hostile counts from the webview cannot overflow the budget arithmetic.
#[test]
fn huge_instance_counts_saturate_instead_of_overflowing() {
    assert_eq!(storms(u64::MAX, u64::MAX, true), 0);
    assert_eq!(storms(u64::MAX, u64::MAX / 2, false), 0);
}

/// Wire contract: the field is optional (absent ⇒ `false`, i.e. every
/// pre-flag record), and it is omitted on output when `false`.
#[test]
fn flag_is_optional_on_the_wire_and_omitted_when_false() {
    let wire = serde_json::json!({
        "seq": 0, "ts": 1000, "mono": 1, "src": "ui", "lvl": "debug",
        "kind": "render.tally", "component": "Row", "windowMs": 500.0,
        "renders": 4, "instances": 2, "traces": [],
    });
    let rec: LogRecord = serde_json::from_value(wire).expect("absent flag deserializes");
    match &rec.payload {
        LogPayload::RenderTally {
            strict_mode_doubling,
            ..
        } => assert!(!strict_mode_doubling),
        other => panic!("expected render.tally, got {}", other.kind()),
    }
    let out = serde_json::to_value(&rec).expect("serialize");
    assert!(
        out.get("strictModeDoubling").is_none(),
        "false is omitted: {out}"
    );

    let on = render_tally_with(1000, "Row", 4, 2, true);
    let out = serde_json::to_value(&on).expect("serialize");
    assert_eq!(out["strictModeDoubling"], serde_json::json!(true));
}
