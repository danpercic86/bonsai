//! §5.1 duration & saturation rule tests: `slow-command` (self-calibrating),
//! `slow-phase`, `queue-delay`, `pool-saturation`, `watchdog-pressure`,
//! `cache-collapse`. Shares the [`H`] harness and builders from `tests_anomaly`.

use super::tests_anomaly_support::{
    ipc_call, ipc_result, refs_of, severity_of, span, with_trace, H,
};
use crate::obs::record::AnomalySeverity;

// ---- §12 row-5 (a): one outlier after a stable baseline fires exactly once ---

#[test]
fn slow_command_fires_once_on_outlier() {
    let mut h = H::new();
    for i in 0..50 {
        h.feed(ipc_result(i, "get_graph", 900.0));
    }
    h.feed(ipc_result(100, "get_graph", 4000.0));
    assert_eq!(h.count("slow-command"), 1, "exactly one slow-command");
    assert_eq!(
        severity_of(h.find("slow-command").unwrap()),
        Some(AnomalySeverity::Warn)
    );
}

/// Pins the FORMULA composition (not just the clamp): with 50 `get_graph`@300ms
/// the clamped p95 is 300, so `3×p95 = 900 < floor 1200` and the floor dominates
/// — a 1100ms call must NOT fire, a 1300ms call must. This is what distinguishes
/// `threshold = max(floor, k × clamped_p95)` from a hardcoded constant.
#[test]
fn slow_command_threshold_uses_clamped_p95_with_floor_and_k() {
    // Each probe gets a FRESH baseline so the probe itself does not pollute p95.
    // Baseline: 50 × get_graph @ 300ms → clamped p95 = 300, so 3×p95 = 900 and the
    // floor (1200) dominates: threshold = max(1200, 900) = 1200.
    fn probe(outlier: f64) -> usize {
        let mut h = H::new();
        for i in 0..50 {
            h.feed(ipc_result(i, "get_graph", 300.0));
        }
        h.feed(ipc_result(100, "get_graph", outlier));
        h.count("slow-command")
    }
    assert_eq!(probe(1100.0), 0, "1100ms < floor 1200 must not fire");
    assert_eq!(probe(1300.0), 1, "1300ms > floor 1200 fires");
}

// ---- §12 row-5 (b): a uniformly high baseline never fires (large-repo) --------

#[test]
fn slow_command_silent_on_uniform_high_baseline() {
    let mut h = H::new();
    for i in 0..50 {
        h.feed(ipc_result(i, "get_graph", 900.0));
    }
    assert_eq!(
        h.count("slow-command"),
        0,
        "a big repo's normal cost is the baseline"
    );
}

// ---- §12 row-5 (c): below MIN_SAMPLES only the >10s catch-all fires ----------

#[test]
fn slow_command_cold_start_only_hard_catch_all() {
    let mut h = H::new();
    for i in 0..3 {
        h.feed(ipc_result(i, "diff_compute", 900.0));
    }
    assert_eq!(
        h.count("slow-command"),
        0,
        "cold start fires nothing on the calibrated path"
    );
    h.feed(ipc_result(10, "diff_compute", 11_000.0));
    assert_eq!(h.count("slow-command"), 1);
    assert_eq!(
        severity_of(h.find("slow-command").unwrap()),
        Some(AnomalySeverity::Error)
    );
}

// ---- §12 row-5 (d): rate limit caps repeats at 1 per cmd per 10s -------------

#[test]
fn slow_command_rate_limited_to_one_per_10s() {
    let mut h = H::new();
    h.feed(ipc_result(0, "get_graph", 11_000.0)); // fires
    h.feed(ipc_result(5_000, "get_graph", 11_000.0)); // within 10s → suppressed
    assert_eq!(h.count("slow-command"), 1);
    h.feed(ipc_result(11_000, "get_graph", 11_000.0)); // >10s later → fires again
    assert_eq!(h.count("slow-command"), 2);
}

// ---- §12 row-5 (e): a phase-dominated slow span fires slow-phase -------------

#[test]
fn slow_phase_attributes_to_dominant_phase() {
    let mut h = H::new();
    let span_seq = h.feed(with_trace(
        span(
            1000,
            "graph.get",
            1000.0,
            Some(vec![("lane", 800.0)]),
            None,
            None,
            None,
            None,
            None,
        ),
        "t1",
    ));
    let result_seq = h.feed(with_trace(ipc_result(1010, "get_graph", 11_000.0), "t1"));
    let a = h.find("slow-phase").expect("dominant phase → slow-phase");
    assert!(
        matches!(&a.payload, crate::obs::record::LogPayload::Anomaly { detail, .. } if detail.contains("lane"))
    );
    assert_eq!(
        refs_of(a),
        &[span_seq, result_seq],
        "references both the span and the result"
    );
}

#[test]
fn slow_phase_true_negative_no_dominant_phase() {
    let mut h = H::new();
    h.feed(with_trace(
        span(
            1000,
            "graph.get",
            1000.0,
            Some(vec![("lane", 400.0), ("revwalk", 400.0)]),
            None,
            None,
            None,
            None,
            None,
        ),
        "t1",
    ));
    h.feed(with_trace(ipc_result(1010, "get_graph", 11_000.0), "t1"));
    assert_eq!(h.count("slow-phase"), 0, "no phase ≥70% ⇒ no slow-phase");
}

// ---- §12 row-5 (f): cache-collapse, and its mutation suppression -------------

#[test]
fn cache_collapse_true_positive() {
    let mut h = H::new();
    for i in 0..5 {
        h.feed(span(
            1000 + i,
            "graph.get",
            10.0,
            None,
            None,
            None,
            None,
            Some("redecorate"),
            None,
        ));
    }
    assert_eq!(h.count("cache-collapse"), 1);
}

#[test]
fn cache_collapse_suppressed_by_intervening_mutation() {
    let mut h = H::new();
    h.feed(ipc_call(900, "commit", "x")); // a real invalidation
    for i in 0..5 {
        h.feed(span(
            1000 + i,
            "graph.get",
            10.0,
            None,
            None,
            None,
            None,
            Some("redecorate"),
            None,
        ));
    }
    assert_eq!(h.count("cache-collapse"), 0);
}

// ---- §12 row-5 (g): queue-delay ---------------------------------------------

#[test]
fn queue_delay_true_positive() {
    let mut h = H::new();
    for i in 0..3 {
        h.feed(span(
            1000 + i * 100,
            "graph.get",
            10.0,
            None,
            Some(150),
            None,
            None,
            None,
            None,
        ));
    }
    assert_eq!(h.count("queue-delay"), 1);
}

#[test]
fn queue_delay_true_negative_below_threshold() {
    let mut h = H::new();
    for i in 0..3 {
        h.feed(span(
            1000 + i * 100,
            "graph.get",
            10.0,
            None,
            Some(50),
            None,
            None,
            None,
            None,
        ));
    }
    assert_eq!(h.count("queue-delay"), 0);
}

// ---- pool-saturation --------------------------------------------------------

#[test]
fn pool_saturation_true_positive() {
    let mut h = H::new();
    for i in 0..3 {
        h.feed(span(
            1000 + i * 100,
            "graph.get",
            10.0,
            None,
            None,
            Some((5, 4)),
            None,
            None,
            None,
        ));
    }
    assert_eq!(h.count("pool-saturation"), 1);
}

#[test]
fn pool_saturation_true_negative() {
    let mut h = H::new();
    for i in 0..3 {
        h.feed(span(
            1000 + i * 100,
            "graph.get",
            10.0,
            None,
            None,
            Some((3, 4)),
            None,
            None,
            None,
        ));
    }
    assert_eq!(h.count("pool-saturation"), 0);
}

// ---- watchdog-pressure ------------------------------------------------------

#[test]
fn watchdog_pressure_true_positive_deadline() {
    let mut h = H::new();
    h.feed(span(
        1000,
        "graph.get",
        10.0,
        None,
        None,
        None,
        Some(0.9),
        None,
        None,
    ));
    let a = h
        .find("watchdog-pressure")
        .expect("deadlineFrac ≥ 0.8 fires");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Warn));
}

#[test]
fn watchdog_pressure_timeout_is_error() {
    let mut h = H::new();
    h.feed(span(
        1000,
        "graph.get",
        10.0,
        None,
        None,
        None,
        None,
        None,
        Some("timeout"),
    ));
    assert_eq!(
        severity_of(h.find("watchdog-pressure").unwrap()),
        Some(AnomalySeverity::Error)
    );
}

#[test]
fn watchdog_pressure_true_negative() {
    let mut h = H::new();
    h.feed(span(
        1000,
        "graph.get",
        10.0,
        None,
        None,
        None,
        Some(0.5),
        None,
        None,
    ));
    assert_eq!(h.count("watchdog-pressure"), 0);
}

// ---- §12 row-5 (h): baseline map stays at the 200-key cap --------------------

#[test]
fn baseline_map_stays_at_cap() {
    let mut h = H::new();
    for i in 0..10_000u64 {
        let cmd = format!("cmd_{i}");
        h.feed(ipc_result(i as i64, &cmd, 5.0));
    }
    assert_eq!(
        h.detector().baseline_len(),
        200,
        "LRU eviction holds the cap"
    );
}
