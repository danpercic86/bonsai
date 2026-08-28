//! P91 §8 — persistence tests for `metrics/usage.json`: atomic round-trip,
//! `.bak` recovery on a corrupt primary, and the "derived percentiles never hit
//! disk" invariant.

use super::{load, save};
use crate::obs::metrics::{DayBucket, MetricTotals, MetricsFile};
use crate::obs::histogram::Histogram;

fn scratch(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!("bonsai-metrics-{}-{}", name, std::process::id()));
    let _ = std::fs::create_dir_all(&p);
    p.push("usage.json");
    p
}

fn sample_file() -> MetricsFile {
    let mut counters = std::collections::BTreeMap::new();
    counters.insert("perf.repo_opens".to_string(), 3u64);
    let mut durations = std::collections::BTreeMap::new();
    let mut h = Histogram::default();
    for _ in 0..10 {
        h.observe(42);
    }
    durations.insert("op.graph.get".to_string(), h);
    MetricsFile {
        schema: 1,
        first_seen: "2026-08-01".into(),
        sessions: 5,
        days: vec![DayBucket {
            date: "2026-08-27".into(),
            totals: MetricTotals {
                counters,
                durations,
                errors: Default::default(),
                session_ms: 1000,
            },
        }],
        lifetime: MetricTotals::default(),
    }
}

#[test]
fn round_trips_atomically() {
    let path = scratch("roundtrip");
    let file = sample_file();
    save(&path, &file).expect("save");
    let back = load(&path);
    assert_eq!(back, file);
    let _ = std::fs::remove_file(&path);
}

#[test]
fn recovers_from_bak_when_primary_is_corrupt() {
    let path = scratch("bak");
    let good = sample_file();
    // First good write creates the primary.
    save(&path, &good).expect("first save");
    // A second write rotates the (good) primary to `.bak`, so `.bak` now holds a
    // valid file. Corrupt the primary and confirm load falls back to `.bak`.
    save(&path, &good).expect("second save");
    std::fs::write(&path, b"{ this is not valid json").expect("corrupt primary");
    let back = load(&path);
    assert_eq!(back, good, "load must recover the good `.bak`");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn missing_file_loads_empty() {
    let path = scratch("missing");
    let _ = std::fs::remove_file(&path);
    let back = load(&path);
    assert_eq!(back.days.len(), 0);
    assert_eq!(back.schema, 1);
}

#[test]
fn stored_histograms_omit_derived_percentiles() {
    // A normally-built stored file leaves p50/p95 as None, so the JSON on disk
    // carries neither key (they are `skip_serializing_if = Option::is_none`).
    // This is the disk half of §12(c); the snapshot half is in tests_metrics.rs.
    let path = scratch("no-derived");
    let file = sample_file();
    save(&path, &file).expect("save");
    let raw = std::fs::read_to_string(&path).expect("read");
    assert!(!raw.contains("p50Ms"), "usage.json must not carry p50Ms");
    assert!(!raw.contains("p95Ms"), "usage.json must not carry p95Ms");
    let _ = std::fs::remove_file(&path);
}
