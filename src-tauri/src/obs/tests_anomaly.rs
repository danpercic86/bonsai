//! §5 cross-record rule tests (the windowed sink rules) + the redaction-
//! independence and non-consumed-kind guarantees. The §5.1 duration/saturation
//! rules live in `tests_anomaly_slow.rs`; both share the [`H`] harness here.

use super::is_mutation_cmd;
use super::tests_anomaly_support::*;
use crate::obs::record::{AnomalySeverity, LogPayload};


// ---- dup-ipc ---------------------------------------------------------------

#[test]
fn dup_ipc_true_positive() {
    let mut h = H::new();
    let s1 = h.feed(ipc_call(1000, "get_status", "abc"));
    let s2 = h.feed(ipc_call(1100, "get_status", "abc"));
    assert_eq!(h.count("dup-ipc"), 1);
    assert_eq!(refs_of(h.find("dup-ipc").unwrap()), &[s1, s2]);
}

#[test]
fn dup_ipc_true_negative_intervening_mutation() {
    let mut h = H::new();
    h.feed(ipc_call(1000, "get_status", "abc"));
    h.feed(ipc_call(1050, "commit", "x")); // mutation between
    h.feed(ipc_call(1100, "get_status", "abc"));
    assert_eq!(h.count("dup-ipc"), 0, "a mutation between suppresses dup-ipc");
}

#[test]
fn dup_ipc_true_negative_out_of_window() {
    let mut h = H::new();
    h.feed(ipc_call(1000, "get_status", "abc"));
    h.feed(ipc_call(1400, "get_status", "abc")); // 400ms > 300ms window
    assert_eq!(h.count("dup-ipc"), 0);
}

#[test]
fn dup_ipc_true_negative_different_hash() {
    let mut h = H::new();
    h.feed(ipc_call(1000, "get_status", "abc"));
    h.feed(ipc_call(1100, "get_status", "def"));
    assert_eq!(h.count("dup-ipc"), 0);
}

/// §5 / §13 row 20 — the mandated explicit-kind test: a NON-`ipc.call` record
/// carrying an `argsHash` (here an `ipc.result`, which has one) must NOT
/// contribute to `dup-ipc`.
#[test]
fn dup_ipc_ignores_non_ipc_call_records_with_args_hash() {
    let mut h = H::new();
    h.feed(ipc_result_hash(1000, "get_status", "abc"));
    h.feed(ipc_result_hash(1100, "get_status", "abc"));
    assert_eq!(
        h.count("dup-ipc"),
        0,
        "ipc.result carries argsHash but is not ipc.call — must not fire dup-ipc"
    );
}

// ---- redundant-refresh -----------------------------------------------------

#[test]
fn redundant_refresh_true_positive() {
    let mut h = H::new();
    h.feed(refresh(1000, "graph"));
    h.feed(refresh(1200, "graph"));
    assert_eq!(h.count("redundant-refresh"), 1);
}

#[test]
fn redundant_refresh_true_negative_mutation_between() {
    let mut h = H::new();
    h.feed(refresh(1000, "graph"));
    h.feed(ipc_call(1050, "commit", "x"));
    h.feed(refresh(1200, "graph"));
    assert_eq!(h.count("redundant-refresh"), 0);
}

// ---- effect-thrash ---------------------------------------------------------

#[test]
fn effect_thrash_true_positive() {
    let mut h = H::new();
    for i in 0..5 {
        h.feed(effect(1000 + i * 10, "Sidebar", "sync"));
    }
    assert_eq!(h.count("effect-thrash"), 1);
}

#[test]
fn effect_thrash_true_negative() {
    let mut h = H::new();
    for i in 0..4 {
        h.feed(effect(1000 + i * 10, "Sidebar", "sync"));
    }
    assert_eq!(h.count("effect-thrash"), 0);
}

// ---- render-storm ----------------------------------------------------------

#[test]
fn render_storm_true_positive() {
    let mut h = H::new();
    h.feed(render_tally(1000, "Row", 10, 2)); // 10 > 3*2
    assert_eq!(h.count("render-storm"), 1);
}

#[test]
fn render_storm_true_negative() {
    let mut h = H::new();
    h.feed(render_tally(1000, "Row", 5, 2)); // 5 <= 6
    assert_eq!(h.count("render-storm"), 0);
}

// ---- event-storm -----------------------------------------------------------

#[test]
fn event_storm_true_positive() {
    let mut h = H::new();
    for i in 0..20 {
        h.feed(event(1000 + i, "focus"));
    }
    assert_eq!(h.count("event-storm"), 1);
}

#[test]
fn event_storm_true_negative() {
    let mut h = H::new();
    for i in 0..19 {
        h.feed(event(1000 + i, "focus"));
    }
    assert_eq!(h.count("event-storm"), 0);
}

// ---- watcher-storm ---------------------------------------------------------

#[test]
fn watcher_storm_true_positive() {
    let mut h = H::new();
    for i in 0..5 {
        h.feed(watcher(1000 + i, true));
    }
    assert_eq!(h.count("watcher-storm"), 1);
}

#[test]
fn watcher_storm_true_negative_not_fired() {
    let mut h = H::new();
    for i in 0..5 {
        h.feed(watcher(1000 + i, false)); // suppressed firings do not count
    }
    assert_eq!(h.count("watcher-storm"), 0);
}

// ---- jank-trace ------------------------------------------------------------

#[test]
fn jank_trace_true_positive() {
    let mut h = H::new();
    let s = h.feed(with_trace(
        span(1000, "graph.get", 200.0, None, None, None, None, None, None),
        "t1",
    ));
    h.feed(frame(1000, 150.0)); // overlaps [800,1000]
    let a = h.find("jank-trace").expect("jank-trace fires");
    assert!(refs_of(a).contains(&s));
}

#[test]
fn jank_trace_true_negative_small_frame() {
    let mut h = H::new();
    h.feed(with_trace(
        span(1000, "graph.get", 200.0, None, None, None, None, None, None),
        "t1",
    ));
    h.feed(frame(1000, 50.0));
    assert_eq!(h.count("jank-trace"), 0);
}

// ---- unbatched-sink --------------------------------------------------------

#[test]
fn unbatched_sink_true_positive() {
    let mut h = H::new();
    // Seed a record so batch_mark has a ts, then 10 marks in the same instant.
    h.feed(event(1000, "x"));
    for _ in 0..10 {
        h.batch_mark();
    }
    assert_eq!(h.count("unbatched-sink"), 1);
}

#[test]
fn unbatched_sink_true_negative_below_threshold() {
    let mut h = H::new();
    h.feed(event(1000, "x"));
    for _ in 0..9 {
        h.batch_mark();
    }
    assert_eq!(h.count("unbatched-sink"), 0);
}

// ---- orphan-trace ----------------------------------------------------------

#[test]
fn orphan_trace_true_positive() {
    let mut h = H::new();
    let s = h.feed(with_trace(ipc_call(1000, "get_graph", "a"), "t1"));
    h.end();
    let a = h.find("orphan-trace").expect("unanswered call is an orphan");
    assert_eq!(severity_of(a), Some(AnomalySeverity::Error));
    assert_eq!(refs_of(a), &[s]);
}

#[test]
fn orphan_trace_true_negative_answered() {
    let mut h = H::new();
    h.feed(with_trace(ipc_call(1000, "get_graph", "a"), "t1"));
    h.feed(with_trace(ipc_result(1010, "get_graph", 5.0), "t1"));
    h.end();
    assert_eq!(h.count("orphan-trace"), 0);
}

// ---- §7.2(c): redaction independence ---------------------------------------

/// The detector keys only off fields that are NEVER redacted (command names,
/// scopes, argsHash, counts, timings). Proof: two runs identical in those fields
/// but differing in the redaction-affected `args` payload (raw path vs `ref#N`
/// ordinal) produce byte-identical anomaly output.
#[test]
fn anomalies_are_redaction_independent() {
    fn run(arg: serde_json::Value) -> String {
        let mut h = H::new();
        let mut a = ipc_call(1000, "get_status", "abc");
        let mut b = ipc_call(1100, "get_status", "abc");
        if let LogPayload::IpcCall { args, .. } = &mut a.payload {
            *args = Some(arg.clone());
        }
        if let LogPayload::IpcCall { args, .. } = &mut b.payload {
            *args = Some(arg);
        }
        h.feed(a);
        h.feed(b);
        serde_json::to_string(&h.out).unwrap()
    }
    let raw = run(serde_json::json!(["/home/dan/secret-repo/file.rs"]));
    let redacted = run(serde_json::json!(["path#7"]));
    assert_eq!(raw, redacted, "anomaly output must not depend on redaction");
    // And the rule actually fired (otherwise the assertion is vacuous).
    assert!(raw.contains("dup-ipc"));
}

/// §6.3 — `truncate` is not an anomaly rule and no detector consumes it. The
/// variant does not exist until increment 7; this pins the catch-all arm that is
/// that guarantee by feeding every currently non-consumed kind and asserting no
/// anomaly is produced.
#[test]
fn non_consumed_kinds_emit_nothing() {
    let mut h = H::new();
    h.feed(base(
        1000,
        LogPayload::Gesture {
            origin: "sidebar".into(),
            gesture: "click".into(),
        },
    ));
    h.feed(base(1001, LogPayload::IpcRecv { cmd: "get_graph".into() }));
    h.feed(base(
        1002,
        LogPayload::Render {
            component: "Row".into(),
            count: 1,
            since_ms: 0.0,
            changed_props: None,
        },
    ));
    h.feed(base(
        1003,
        LogPayload::State {
            store: "s".into(),
            field: "f".into(),
            from: "a".into(),
            to: "b".into(),
        },
    ));
    h.feed(base(
        1004,
        LogPayload::Error {
            location: "x".into(),
            code: None,
            message: "boom".into(),
            stack_hash: None,
        },
    ));
    h.feed(base(
        1005,
        LogPayload::Drop {
            dropped: 3,
            since_seq: 1,
        },
    ));
    assert!(h.out.is_empty(), "non-consumed kinds produce no anomalies");
}

#[test]
fn mutation_table_is_shared_and_sane() {
    assert!(is_mutation_cmd("commit"));
    assert!(is_mutation_cmd("stage_file"));
    assert!(is_mutation_cmd("bisect_mark"));
    assert!(!is_mutation_cmd("get_graph"));
    assert!(!is_mutation_cmd("get_status"));
}

/// §11 "bounded" is a literal claim, and `dup-ipc` is the one rule whose key
/// (`cmd\0argsHash`) is drawn from an UNBOUNDED space: every distinct argument
/// set mints a new debounce entry. The `last_fire` map must therefore be pruned
/// with the event window, or a long Dev-mode session grows it forever.
#[test]
fn dup_ipc_debounce_map_stays_bounded_over_a_long_session() {
    let mut h = H::new();
    // 2000 distinct arg hashes, each fired as a duplicate pair (so each one
    // really does stamp `last_fire`), spread across 200 s of session time.
    for i in 0..2000u64 {
        let ts = 1000 + i as i64 * 100;
        let hash = format!("h{i}");
        h.feed(ipc_call(ts, "get_status", &hash));
        h.feed(ipc_call(ts + 50, "get_status", &hash));
    }
    assert_eq!(h.count("dup-ipc"), 2000, "every pair is a real duplicate");
    let len = h.detector().ipc_calls.last_fire_len();
    assert!(
        len <= 4,
        "dup-ipc debounce map must stay within one window's worth of keys, got {len}"
    );
}
