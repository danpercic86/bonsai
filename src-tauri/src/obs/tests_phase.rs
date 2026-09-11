//! P91 §3.1 — `PhaseRecorder` mechanism tests: the pool gauge, the zero-cost
//! no-op path, and one end-to-end span emitted through a real sink (phase sum ≤
//! `ms`, `queuedMs` present, `cache` present).

use std::sync::Arc;

use crate::obs::phase::{CacheOutcome, PhaseRecorder, PoolGuard, SpanOutcome, OP_GRAPH_GET};
use crate::obs::record::{LogLevel, RedactionMode};
use crate::obs::sink::Sink;
use crate::obs::trace::{self, TraceMeta};
use crate::obs::writer::{list_log_files, Limits, WriterConfig};

// Serializes span-emitting tests via `trace::test_sink_lock`.
fn cfg(dir: &std::path::Path) -> WriterConfig {
    WriterConfig {
        dir: dir.to_path_buf(),
        session_id: "sphase01".into(),
        started_secs: 1_787_839_391,
        app_version: "1.5.0".into(),
        os: "windows".into(),
        level: LogLevel::Debug,
        redaction: RedactionMode::Strict,
        home_mask: None,
        limits: Limits::default(),
    }
}

fn span_lines(dir: &std::path::Path) -> Vec<serde_json::Value> {
    let mut out = Vec::new();
    for (name, _) in list_log_files(dir) {
        let text = std::fs::read_to_string(dir.join(name)).expect("read log part");
        for line in text.lines() {
            let v: serde_json::Value = serde_json::from_str(line).expect("valid JSON line");
            if v["kind"] == "span" {
                out.push(v);
            }
        }
    }
    out
}

/// The gauge tracks in-flight `PoolGuard`s and drops back to zero.
#[test]
fn pool_guard_counts_and_releases() {
    let base = crate::obs::phase::pool_inflight_now();
    {
        let g1 = PoolGuard::enter();
        assert!(g1.inflight() > base as u32);
        let g2 = PoolGuard::enter();
        assert!(g2.inflight() >= g1.inflight());
        assert_eq!(g2.max(), crate::obs::phase::POOL_MAX);
    }
    assert_eq!(crate::obs::phase::pool_inflight_now(), base, "gauge must release");
}

/// Dev mode off ⇒ the recorder is inactive and `finish` is a silent no-op (no
/// allocation, nothing emitted).
#[test]
fn recorder_is_noop_when_no_active_sink() {
    let _serial = trace::test_sink_lock();
    trace::set_active_sink(None);
    let mut rec = PhaseRecorder::start(OP_GRAPH_GET);
    assert!(!rec.is_active());
    {
        let _p = rec.phase("revwalk");
    }
    rec.note_queue(5, 1, 512);
    rec.finish(&TraceMeta::root("backend"), SpanOutcome::Ok);
}

/// One end-to-end span: revwalk/decorate/lane phases, `queuedMs`, a cache
/// outcome, and phase-sum ≤ `ms` (§12 row-3 acceptance a/b).
#[test]
fn span_records_phases_queue_and_cache() {
    let _serial = trace::test_sink_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Arc::new(Sink::start(cfg(dir.path())).expect("start"));
    trace::set_active_sink(Some(Arc::clone(&sink)));

    let mut rec = PhaseRecorder::start(OP_GRAPH_GET);
    assert!(rec.is_active());
    rec.note_queue(3, 1, 512);
    rec.note_cache(CacheOutcome::Miss);
    rec.note_items(42);
    // Real timed phases so the `sum ≤ ms` invariant is exercised honestly
    // (add_phase with fabricated durations could exceed the real elapsed time).
    for name in ["decorate", "revwalk", "lane"] {
        let _p = rec.phase(name);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    // Capture our own trace so we can single out our span: the active sink is
    // process-wide, and parallel graph tests may deposit their own graph.get
    // spans into it during our window.
    let meta = TraceMeta::root("backend");
    let my_trace = meta.trace.clone();
    rec.finish(&meta, SpanOutcome::Ok);

    trace::set_active_sink(None);
    sink.shutdown();

    let spans: Vec<serde_json::Value> = span_lines(dir.path())
        .into_iter()
        .filter(|s| s["trace"] == serde_json::Value::String(my_trace.clone()))
        .collect();
    assert_eq!(spans.len(), 1, "exactly one span for our trace");
    let s = &spans[0];
    assert_eq!(s["op"], OP_GRAPH_GET);
    assert_eq!(s["cache"], "miss");
    assert_eq!(s["queuedMs"], 3);
    assert_eq!(s["items"], 42);
    assert!(s.get("argsHash").is_none(), "span must carry no argsHash");

    let phases = s["phases"].as_array().expect("phases array");
    let names: Vec<&str> = phases.iter().map(|p| p["name"].as_str().unwrap()).collect();
    for want in ["decorate", "revwalk", "lane"] {
        assert!(names.contains(&want), "missing phase {want}: {names:?}");
    }
    let sum: f64 = phases.iter().map(|p| p["ms"].as_f64().unwrap()).sum();
    let ms = s["ms"].as_f64().unwrap();
    assert!(sum <= ms + 0.001, "phase sum {sum} must be ≤ span ms {ms}");
}

/// §12 row-3 (b): a saturated pool yields `poolInflight >= poolMax` on the span.
/// Gauge-level (holds `POOL_MAX` guards) — real 512-blocking-task saturation is
/// tester/integration territory.
#[test]
fn saturated_pool_reports_inflight_ge_max() {
    let _serial = trace::test_sink_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Arc::new(Sink::start(cfg(dir.path())).expect("start"));
    trace::set_active_sink(Some(Arc::clone(&sink)));

    let held: Vec<PoolGuard> = (0..crate::obs::phase::POOL_MAX).map(|_| PoolGuard::enter()).collect();
    let mine = PoolGuard::enter();
    let mut rec = PhaseRecorder::start(OP_GRAPH_GET);
    rec.note_queue(1, mine.inflight(), mine.max());
    let meta = TraceMeta::root("backend");
    let my_trace = meta.trace.clone();
    rec.finish(&meta, SpanOutcome::Ok);
    drop(mine);
    drop(held);

    trace::set_active_sink(None);
    sink.shutdown();

    let s = span_lines(dir.path())
        .into_iter()
        .find(|s| s["trace"] == serde_json::Value::String(my_trace.clone()))
        .expect("our span");
    let inflight = s["poolInflight"].as_u64().unwrap();
    let max = s["poolMax"].as_u64().unwrap();
    assert!(inflight >= max, "poolInflight {inflight} >= poolMax {max}");
    assert!(s["queuedMs"].as_u64().is_some(), "queuedMs present and ≥0");
}

/// §12 row-3 (c): a near-timeout op records `deadlineFrac ≥ 0.8`.
#[test]
fn near_timeout_reports_deadline_frac() {
    let _serial = trace::test_sink_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Arc::new(Sink::start(cfg(dir.path())).expect("start"));
    trace::set_active_sink(Some(Arc::clone(&sink)));

    let mut rec = PhaseRecorder::start(OP_GRAPH_GET);
    rec.note_deadline(0.9);
    let meta = TraceMeta::root("backend");
    let my_trace = meta.trace.clone();
    rec.finish(&meta, SpanOutcome::Ok);

    trace::set_active_sink(None);
    sink.shutdown();

    let s = span_lines(dir.path())
        .into_iter()
        .find(|s| s["trace"] == serde_json::Value::String(my_trace.clone()))
        .expect("our span");
    assert!(s["deadlineFrac"].as_f64().unwrap() >= 0.8, "deadlineFrac ≥ 0.8");
}

/// §12 row-3 acceptance (e): recorder overhead. `#[ignore]` by default so a busy
/// CI box can never flake the gate on a microbenchmark; run with
/// `cargo test -- --ignored` to check the < 5 µs budget locally.
#[test]
#[ignore = "microbenchmark — run explicitly; timing-sensitive"]
fn recorder_overhead_is_small() {
    let _serial = trace::test_sink_lock();
    let dir = tempfile::tempdir().expect("tempdir");
    let sink = Arc::new(Sink::start(cfg(dir.path())).expect("start"));
    trace::set_active_sink(Some(Arc::clone(&sink)));

    let iters = 10_000u32;
    let start = std::time::Instant::now();
    for _ in 0..iters {
        let mut rec = PhaseRecorder::start(OP_GRAPH_GET);
        {
            let _p = rec.phase("revwalk");
        }
        {
            let _p = rec.phase("lane");
        }
        rec.note_queue(1, 1, 512);
        // Deliberately drop WITHOUT finish so we time only the recorder, not the
        // sink write.
        drop(rec);
    }
    let per = start.elapsed().as_secs_f64() / iters as f64 * 1e6; // µs
    trace::set_active_sink(None);
    sink.shutdown();
    assert!(per < 5.0, "recorder overhead {per:.2} µs/op exceeds 5 µs budget");
}
