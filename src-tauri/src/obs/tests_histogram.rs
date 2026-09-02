//! Unit tests for the shared §8 `Histogram` (bucketing + interpolated,
//! max-clamped percentile).

use super::super::histogram::Histogram;

#[test]
fn empty_histogram_has_no_percentile() {
    let h = Histogram::default();
    assert_eq!(h.percentile_ms(0.95), None);
    assert_eq!(h.mean_ms(), None);
}

#[test]
fn observe_fills_expected_buckets() {
    let mut h = Histogram::default();
    for &v in &[0u64, 1, 3, 7, 25, 75, 300, 1500, 9000] {
        h.observe(v);
    }
    assert_eq!(h.count, 9);
    assert_eq!(h.max_ms, 9000);
    // 9000 lands in the +inf bucket.
    assert_eq!(h.buckets[7], 1);
}

#[test]
fn percentile_is_clamped_to_max_ms() {
    // 50 identical 900ms samples: interpolation across the coarse (500,2000]
    // bucket would yield ~1925, but a percentile can never exceed the observed
    // maximum, so the clamp reports 900. This is what lets the §12 row-5(a)
    // slow-command test fire on the 4s outlier.
    let mut h = Histogram::default();
    for _ in 0..50 {
        h.observe(900);
    }
    let p95 = h.percentile_ms(0.95).unwrap();
    assert_eq!(p95, 900, "clamped to max_ms");
}

#[test]
fn percentile_matches_brute_force_within_one_bucket() {
    // A spread across buckets; the estimate must be within one bucket width of the
    // true p50 (§8.1 acceptance shape, checked in miniature here).
    let mut samples: Vec<u64> = Vec::new();
    let mut h = Histogram::default();
    for i in 0..1000u64 {
        let v = i % 400; // 0..399
        samples.push(v);
        h.observe(v);
    }
    samples.sort_unstable();
    let true_p50 = samples[(0.5 * samples.len() as f64) as usize];
    let est = h.percentile_ms(0.5).unwrap() as u64;
    let bucket_width = 500 - 100; // the (100,500] bucket containing ~200
    assert!(
        (est as i64 - true_p50 as i64).unsigned_abs() <= bucket_width,
        "est {est} vs true {true_p50}"
    );
}

#[test]
fn mean_is_sum_over_count() {
    let mut h = Histogram::default();
    for &v in &[10u64, 20, 30] {
        h.observe(v);
    }
    assert_eq!(h.mean_ms(), Some(20));
}

/// The `percentile_ms(0.0)` edge (increment-6 nit): with an EMPTY `buckets[0]`,
/// `next_cum (0) >= target (0)` used to stop in bucket 0 and fall through to the
/// `max_ms` fallback, so p0 reported the MAXIMUM. Empty buckets are skipped, so
/// p0 now lands at the lower bound of the first bucket that holds observations.
/// Inert for the ratified 0.5/0.95 method, but §8.1's percentile helper is shared
/// code and must be correct across the whole `p` range.
#[test]
fn p0_is_the_low_end_not_the_max_when_bucket_zero_is_empty() {
    let mut h = Histogram::default();
    for _ in 0..50 {
        h.observe(900); // all in the coarse (500, 2000] bucket
    }
    assert_eq!(h.buckets[0], 0, "bucket 0 is empty by construction here");
    let p0 = h.percentile_ms(0.0).expect("count > 0");
    assert_eq!(p0, 500, "p0 is the containing bucket's lower bound");
    assert!(p0 < h.max_ms as u32, "p0 must not report the maximum");
    // The ratified §8.1 method is unchanged by the empty-bucket skip.
    assert_eq!(h.percentile_ms(0.95), Some(900), "p95 still clamps to max_ms");
    assert_eq!(h.percentile_ms(1.0), Some(900));
}

/// The only surviving path to the `bucket_count == 0` fallback: a torn/edited
/// `usage.json` deserialized with `count > 0` but no bucket populated. It must
/// degrade to `max_ms`, never panic or divide by zero.
#[test]
fn a_torn_histogram_falls_back_to_max_ms() {
    let h = Histogram {
        count: 7,
        sum_ms: 700,
        max_ms: 123,
        buckets: [0; 8],
        p50_ms: None,
        p95_ms: None,
    };
    assert_eq!(h.percentile_ms(0.95), Some(123));
    assert_eq!(h.percentile_ms(0.0), Some(123));
}
