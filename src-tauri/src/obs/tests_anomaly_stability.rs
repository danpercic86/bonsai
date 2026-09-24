//! P120 §7 T-R1–T-R7 — the `unprompted-shift` and `focus-steal` rules
//! (`anomaly/stability.rs`), driven through the real [`super::AnomalyDetector`].
//! T-R8 (strict vs raw on disk) lives in `tests_stability_redaction.rs`.

use super::tests_anomaly_support::*;
use crate::obs::record::{AnomalySeverity, LogPayload, LogRecord, StabilityPhase};

fn detail_of(r: &LogRecord) -> &str {
    match &r.payload {
        LogPayload::Anomaly { detail, .. } => detail,
        _ => "",
    }
}

fn traces_of(r: &LogRecord) -> Vec<String> {
    match &r.payload {
        LogPayload::Anomaly { traces, .. } => traces.clone(),
        _ => vec![],
    }
}

fn gesture(ts: i64) -> LogRecord {
    base(
        ts,
        LogPayload::Gesture {
            origin: "sidebar".into(),
            gesture: "click".into(),
        },
    )
}

// ---- T-R1 / T-R2 — unprompted-shift -----------------------------------------

#[test]
fn t_r1_interactive_unprompted_shift_warns_once_with_its_seq() {
    let mut h = H::new();
    let seq = h.feed(layout_shift(
        1_000,
        StabilityPhase::Interactive,
        false,
        false,
    ));
    assert_eq!(h.count("unprompted-shift"), 1);
    let a = h.find("unprompted-shift").expect("anomaly");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Warn));
    assert_eq!(refs_of(a), &[seq]);
    assert_eq!(detail_of(a), "graph: 3 shift(s), score 0.0412, over 412 ms");
    assert_eq!(a.ts, 1_000);
}

#[test]
fn t_r2_prompted_resizing_or_boot_shifts_do_not_warn() {
    let mut h = H::new();
    h.feed(layout_shift(
        1_000,
        StabilityPhase::Interactive,
        true,
        false,
    ));
    h.feed(layout_shift(
        2_000,
        StabilityPhase::Interactive,
        false,
        true,
    ));
    h.feed(layout_shift(3_000, StabilityPhase::Boot, false, false));
    h.end();
    assert_eq!(h.count("unprompted-shift"), 0);
    assert!(
        h.out.is_empty(),
        "no other rule may fire either: {:?}",
        h.out
    );
}

// ---- T-R3–T-R5 — focus-steal, caused ----------------------------------------

#[test]
fn t_r3_loss_before_its_settling_refresh_warns_anchored_at_the_loss() {
    let mut h = H::new();
    let loss = h.feed(focus_lost(10_000));
    assert_eq!(h.count("focus-steal"), 0, "held until the refresh settles");
    // Round [9_900, 10_300] ⇒ window [9_850, 10_550] contains lostAt 10_000.
    let refresh = h.feed(refresh_round(10_300, 400.0, &["t-a", "t-b"]));
    assert_eq!(h.count("focus-steal"), 1);
    let a = h.find("focus-steal").expect("anomaly");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Warn));
    assert_eq!(refs_of(a), &[loss, refresh]);
    assert_eq!(traces_of(a), vec!["t-a".to_string(), "t-b".to_string()]);
    assert_eq!(a.ts, 10_050, "anchored at the loss record's ts");
    assert_eq!(
        detail_of(a),
        "focus lost from details (detached, interactive); causes: 1 refresh, 0 watcher"
    );
    h.end();
    assert_eq!(h.count("focus-steal"), 1, "exactly one anomaly per loss");
}

#[test]
fn t_r3_a_refresh_outside_the_window_still_matches_on_a_shared_trace() {
    let mut h = H::new();
    let loss = h.feed(with_trace(focus_lost(10_000), "t-x"));
    let refresh = h.feed(refresh_round(14_000, 10.0, &["t-x"]));
    let a = h.find("focus-steal").expect("trace hit");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Warn));
    assert_eq!(refs_of(a), &[loss, refresh]);
    assert_eq!(traces_of(a), vec!["t-x".to_string()]);
}

#[test]
fn t_r4_fired_watcher_400ms_before_the_loss_is_a_cause() {
    let mut h = H::new();
    let w = h.feed(watcher_sup(9_600, true, false));
    let loss = h.feed(focus_lost(10_000));
    let a = h.find("focus-steal").expect("found by look-back");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Warn));
    assert_eq!(refs_of(a), &[loss, w]);
    assert!(detail_of(a).ends_with("causes: 0 refresh, 1 watcher"));
}

#[test]
fn t_r4_a_suppressed_watcher_is_not_a_cause() {
    let mut h = H::new();
    h.feed(watcher_sup(9_600, true, true));
    h.feed(focus_lost(10_000));
    assert_eq!(h.count("focus-steal"), 0);
    h.end();
    let a = h.find("focus-steal").expect("drained");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Info));
}

#[test]
fn t_r4_a_settling_refresh_does_not_credit_a_non_matching_watcher() {
    let mut h = H::new();
    let w = h.feed(watcher_sup(9_600, true, false));
    // Outside the watcher's 1 s look-back, so the loss is held…
    let loss = h.feed(focus_lost(10_700));
    assert_eq!(h.count("focus-steal"), 0);
    // …until a refresh settles; the watcher does not match this loss.
    let refresh = h.feed(refresh_round(10_800, 300.0, &[]));
    let a = h.find("focus-steal").expect("warn");
    assert_eq!(refs_of(a), &[loss, refresh]);
    assert!(!refs_of(a).contains(&w));
}

#[test]
fn t_r5_loss_just_after_the_refresh_settled_is_found_by_look_back() {
    let mut h = H::new();
    let refresh = h.feed(refresh_round(10_000, 100.0, &["t-r"]));
    let loss = h.feed(focus_lost(10_200));
    let a = h.find("focus-steal").expect("look-back hit");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Warn));
    assert_eq!(refs_of(a), &[loss, refresh]);
    assert_eq!(a.ts, 10_250);
}

#[test]
fn t_r5_loss_past_the_post_window_is_not_matched() {
    let mut h = H::new();
    h.feed(refresh_round(10_000, 100.0, &[]));
    h.feed(focus_lost(10_251));
    assert_eq!(h.count("focus-steal"), 0);
}

// ---- T-R6 — focus-steal, uncaused -------------------------------------------

#[test]
fn t_r6_uncaused_loss_is_info_only_after_the_hold_expires() {
    let mut h = H::new();
    let loss = h.feed(focus_lost(10_000));
    h.feed(gesture(15_000));
    assert_eq!(h.count("focus-steal"), 0, "not yet past lostAt + 5000");
    h.feed(gesture(15_001));
    assert_eq!(h.count("focus-steal"), 1);
    let a = h.find("focus-steal").expect("info");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Info));
    assert_eq!(refs_of(a), &[loss]);
    assert_eq!(a.ts, 10_050);
    assert_eq!(
        detail_of(a),
        "focus lost from details (detached, interactive); no refresh or watcher round found"
    );
    h.end();
    assert_eq!(
        h.count("focus-steal"),
        1,
        "expired losses are not drained twice"
    );
}

#[test]
fn t_r6_session_end_drains_held_losses_as_info() {
    let mut h = H::new();
    let a = h.feed(focus_lost(10_000));
    let b = h.feed(focus_lost(11_000));
    h.end();
    let infos: Vec<&[u64]> = h
        .out
        .iter()
        .filter(|r| rule_of(r) == Some("focus-steal"))
        .inspect(|r| assert_eq!(severity_of(r), Some(AnomalySeverity::Info)))
        .map(refs_of)
        .collect();
    assert_eq!(infos, vec![&[a][..], &[b][..]]);
}

// ---- T-R7 — bounds (§11) ----------------------------------------------------

#[test]
fn t_r7_pending_losses_are_fifo_capped_and_overflow_is_info() {
    let mut h = H::new();
    // 70 losses inside 5 s, so only the cap (not expiry) can bound them.
    let seqs: Vec<u64> = (0..70).map(|i| h.feed(focus_lost(10_000 + i))).collect();
    assert!(h.detector().stability.pending_len() <= 64);
    assert_eq!(h.count("focus-steal"), 6);
    let overflow: Vec<u64> = h
        .out
        .iter()
        .filter(|r| rule_of(r) == Some("focus-steal"))
        .inspect(|r| assert_eq!(severity_of(r), Some(AnomalySeverity::Info)))
        .map(|r| refs_of(r)[0])
        .collect();
    assert_eq!(overflow, seqs[..6], "the OLDEST losses overflow first");
    h.end();
    assert_eq!(h.count("focus-steal"), 70, "one anomaly per loss overall");
}

#[test]
fn t_r7_causes_are_fifo_capped() {
    let mut h = H::new();
    // 300 fired watchers inside 6 s, so only the cap can bound them.
    for i in 0..300 {
        h.feed(watcher_sup(20_000 + i * 10, true, false));
    }
    assert!(h.detector().stability.causes_len() <= 256);
    assert_eq!(h.count("focus-steal"), 0);
}
