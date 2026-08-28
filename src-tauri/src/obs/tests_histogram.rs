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
