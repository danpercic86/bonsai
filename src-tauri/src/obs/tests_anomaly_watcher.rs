//! P113d §4.5 — the anomaly consumers of `watcher` records are blind to the
//! new fields: noise summaries (`fired: false`) never feed `watcher-storm` or
//! the focus-steal cause index, and the P113d aggregates on fired records do
//! not change detection.

use super::tests_anomaly_support::*;
use crate::obs::record::{LogPayload, LogRecord};

/// A noise-summary-shaped record: `fired: false`, `skippedBatches: k`.
fn noise_summary(ts: i64, k: u32) -> LogRecord {
    let mut r = watcher(ts, false);
    if let LogPayload::Watcher {
        paths,
        relevant,
        skipped_batches,
        skipped_paths,
        ..
    } = &mut r.payload
    {
        *paths = 0;
        *relevant = 0;
        *skipped_batches = Some(k);
        *skipped_paths = Some(k * 3);
    }
    r
}

/// `watcher(ts, true)` with every P113d field set.
fn fired_p113d(ts: i64) -> LogRecord {
    let mut r = watcher(ts, true);
    if let LogPayload::Watcher {
        batches,
        burst_ms,
        errors,
        skipped_batches,
        skipped_paths,
        ..
    } = &mut r.payload
    {
        *batches = Some(7);
        *burst_ms = Some(420);
        *errors = Some(1);
        *skipped_batches = Some(2);
        *skipped_paths = Some(9);
    }
    r
}

fn detail_of(r: &LogRecord) -> &str {
    match &r.payload {
        LogPayload::Anomaly { detail, .. } => detail,
        _ => "",
    }
}

#[test]
fn noise_summaries_never_feed_watcher_rules() {
    let mut h = H::new();
    for i in 0..20 {
        h.feed(noise_summary(10_000 + i * 40, i as u32 + 1));
    }
    assert_eq!(h.count("watcher-storm"), 0);

    // A focus loss right after the summaries finds no watcher cause.
    h.feed(focus_lost(10_900));
    h.end();
    let a = h.find("focus-steal").expect("the loss is reported");
    assert!(
        detail_of(a).ends_with("no refresh or watcher round found"),
        "a noise summary was credited as a cause: {}",
        detail_of(a)
    );
}

#[test]
fn watcher_storm_detection_unchanged() {
    // Same ts sequence: a storm (5 in < 1 s), a gap, then a non-storm pair.
    let ts = [1_000, 1_100, 1_200, 1_300, 1_400, 5_000, 5_500];
    let run = |p113d: bool| {
        let mut h = H::new();
        for &t in &ts {
            h.feed(if p113d {
                fired_p113d(t)
            } else {
                watcher(t, true)
            });
        }
        h.end();
        serde_json::to_value(&h.out).unwrap()
    };
    let legacy = run(false);
    assert_eq!(legacy, run(true));
    let mut h = H::new();
    for &t in &ts {
        h.feed(fired_p113d(t));
    }
    assert_eq!(h.count("watcher-storm"), 1);
}
