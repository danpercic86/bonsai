//! §8.1 (a)–(d): size independence, derived-on-snapshot percentiles,
//! per-day lane comparability, percentile accuracy.

use super::*;

// --------------------------------------------- §8.1 (b) size independence

#[test]
fn stored_size_is_independent_of_observation_count() {
    let path_a = scratch("size1m");
    let path_b = scratch("size2m");
    let day = "2026-08-27";
    let store_a = MetricsState::for_test(path_a.clone(), 1_756_000_000);
    let store_b = MetricsState::for_test(path_b.clone(), 1_756_000_000);
    for _ in 0..1_000_000u64 {
        store_a.observe_ipc_result("getStatus", 3.0, None, day);
    }
    for _ in 0..2_000_000u64 {
        store_b.observe_ipc_result("getStatus", 3.0, None, day);
    }
    store_a
        .flush(&perf(0, 0, 0), 1_756_000_000)
        .expect("flush a");
    store_b
        .flush(&perf(0, 0, 0), 1_756_000_000)
        .expect("flush b");
    let size_a = std::fs::metadata(&path_a).unwrap().len();
    let size_b = std::fs::metadata(&path_b).unwrap().len();
    // 1,000,000 and 2,000,000 share a digit width, as do their 3× sums, so the
    // file is byte-identical: no raw sample is retained, only the 8 bucket
    // counters + aggregates.
    assert_eq!(
        size_a, size_b,
        "usage.json size must not grow with sample count"
    );
    let _ = std::fs::remove_file(&path_a);
    let _ = std::fs::remove_file(&path_b);
}

// ----------------------------------- §8.1 (c) derived-on-snapshot, not disk

#[test]
fn percentiles_are_on_snapshot_but_absent_from_disk() {
    let path = scratch("derived");
    let day = "2026-08-27";
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    for _ in 0..100 {
        store.observe_ipc_result("getStatus", 42.0, None, day);
    }
    store.flush(&perf(0, 0, 0), 1_756_000_000).expect("flush");
    // Snapshot: p50/p95 present.
    let snap = store.snapshot();
    let h = snap.days[0].totals.durations.get("cmd.getStatus").unwrap();
    assert!(h.p50_ms.is_some() && h.p95_ms.is_some());
    // Disk: neither key appears.
    let raw = std::fs::read_to_string(&path).unwrap();
    assert!(!raw.contains("p50Ms") && !raw.contains("p95Ms"));
    let _ = std::fs::remove_file(&path);
}

// ------------------------- §8.1 (d) two-day op.graph.get.lane comparability

#[test]
fn lane_percentiles_are_per_day_comparable() {
    let path = scratch("twoday");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let lane = |ms: f64| PhaseTiming {
        name: "lane".into(),
        ms,
        n: None,
    };
    // Day 1: fast lane (~5 ms). Day 2: slow lane (~300 ms) — a regression.
    for _ in 0..40 {
        store.observe_span("graph.get", 50.0, &[lane(5.0)], None, "2026-08-27");
    }
    for _ in 0..40 {
        store.observe_span("graph.get", 400.0, &[lane(300.0)], None, "2026-08-28");
    }
    let snap = store.snapshot();
    let d1 = &snap.days[0]
        .totals
        .durations
        .get("op.graph.get.lane")
        .unwrap();
    let d2 = &snap.days[1]
        .totals
        .durations
        .get("op.graph.get.lane")
        .unwrap();
    let p95_1 = d1.p95_ms.unwrap();
    let p95_2 = d2.p95_ms.unwrap();
    assert!(
        p95_2 > p95_1 * 3,
        "day-2 lane p95 ({p95_2}) must dwarf day-1 ({p95_1}) — the regression signal"
    );
    let _ = std::fs::remove_file(&path);
}

// ------------------------------ §8.1 (a) percentile vs brute-force reference

#[test]
fn percentile_matches_brute_force_within_one_bucket() {
    use crate::obs::histogram::{Histogram, BUCKET_BOUNDS_MS};
    // 10k deterministic pseudo-random samples in 0..600 ms (an LCG — no dep).
    let mut samples: Vec<u64> = Vec::with_capacity(10_000);
    let mut s: u64 = 0x1234_5678;
    let mut h = Histogram::default();
    for _ in 0..10_000 {
        s = s
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        let v = (s >> 33) % 600;
        samples.push(v);
        h.observe(v);
    }
    samples.sort_unstable();
    let widest = 1500u32; // the coarsest bucket, (500, 2000]
    for p in [0.5f64, 0.95] {
        let idx = ((p * samples.len() as f64) as usize).min(samples.len() - 1);
        let reference = samples[idx] as i64;
        let est = h.percentile_ms(p as f32).unwrap() as i64;
        assert!(
            (est - reference).unsigned_abs() as u32 <= widest,
            "p{p}: est {est} vs reference {reference} exceeds one bucket width"
        );
    }
    // Sanity: the bucket bounds are the frozen 7 we interpolate within.
    assert_eq!(BUCKET_BOUNDS_MS.len(), 7);
}

/// The `percentile_ms(0.0)` edge (inc-5 review NIT): with an empty `buckets[0]`
/// it must return the LOWER bound of the first non-empty bucket, not `max_ms`.
#[test]
fn percentile_zero_skips_empty_leading_bucket() {
    use crate::obs::histogram::Histogram;
    let mut h = Histogram::default();
    for _ in 0..20 {
        h.observe(20); // lands in bucket (10, 50], index 3
    }
    // Before the fix this returned max_ms (20); now it returns the bucket's lower
    // bound (10).
    assert_eq!(h.percentile_ms(0.0), Some(10));
}
