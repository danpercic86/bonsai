//! P91 §8 — the fixed-shape duration `Histogram` (8 buckets), shared by the
//! anomaly detector's in-memory `slow-command` baseline (§5.1, increment 5) and
//! the durable metrics summaries (§8.1, increment 6).
//!
//! ONE concern: bounded duration summary. 8 counters + `count`/`sum_ms`/`max_ms`,
//! no raw sample ever retained. Bucket boundaries are **frozen** (§8.1) — changing
//! them would break existing `usage.json` files.
//!
//! Living here (not in `anomaly.rs`) is deliberate: it is what makes inc-5's
//! `slow-command` p95 and inc-6's `percentile_ms` snapshot field agree **by
//! construction** rather than by two copies of the same arithmetic.

use serde::{Deserialize, Serialize};

/// Upper bounds of the first 7 buckets, in ms; the 8th bucket is `(2000, +inf)`.
/// Frozen (§8.1).
pub const BUCKET_BOUNDS_MS: [u32; 7] = [1, 5, 10, 50, 100, 500, 2000];

/// A bounded duration summary: 8 monotone buckets plus running aggregates.
///
/// `buckets[i]` counts observations whose value falls in bucket `i`
/// (`(BUCKET_BOUNDS_MS[i-1], BUCKET_BOUNDS_MS[i]]`, with `bucket[0]` = `(0, 1]`
/// and `bucket[7]` = `(2000, +inf)`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Histogram {
    pub count: u64,
    pub sum_ms: u64,
    pub max_ms: u64,
    pub buckets: [u64; 8],
}

impl Histogram {
    /// Records one observation. `ms` is clamped to `>= 0` by the `u64` type.
    pub fn observe(&mut self, ms: u64) {
        self.count += 1;
        self.sum_ms = self.sum_ms.saturating_add(ms);
        if ms > self.max_ms {
            self.max_ms = ms;
        }
        let idx = BUCKET_BOUNDS_MS
            .iter()
            .position(|&b| ms <= b as u64)
            .unwrap_or(7);
        self.buckets[idx] += 1;
    }

    /// Linear interpolation inside the containing bucket (§8.1), **clamped to
    /// `max_ms`**. `p` in `0.0..=1.0`; `None` when `count == 0`.
    ///
    /// The `max_ms` clamp resolves the contract's own §12 row-5 acceptance test
    /// (a): 50 `get_graph` observations at a uniform 900 ms all land in the coarse
    /// `(500, 2000]` bucket, so plain interpolation reports p95 ≈ 1925 ms and the
    /// threshold `max(1200, 3 × 1925) = 5775` would swallow the 4 s outlier the
    /// test requires to fire. A percentile can never exceed the observed maximum,
    /// so clamping to `max_ms` (here 900) yields p95 = 900, threshold 2700, and
    /// the 4 s spike fires — while never worsening §8.1's "within one bucket
    /// width" bound (the true p95 ≤ `max_ms`, so the clamp only moves the estimate
    /// toward truth). §8.1's "top bucket returns `max_ms`" is the special case of
    /// this general rule. Resolves the §5.1-vs-§8.1 percentile nit (report item).
    pub fn percentile_ms(&self, p: f32) -> Option<u32> {
        if self.count == 0 {
            return None;
        }
        let p = p.clamp(0.0, 1.0) as f64;
        let target = p * self.count as f64;
        let mut cum: f64 = 0.0;
        for i in 0..8 {
            let bucket_count = self.buckets[i] as f64;
            let next_cum = cum + bucket_count;
            if next_cum >= target || i == 7 {
                if bucket_count == 0.0 {
                    // Empty containing bucket (only reachable at i == 7): fall back
                    // to the observed maximum.
                    return Some(self.max_ms as u32);
                }
                let lower = if i == 0 {
                    0.0
                } else {
                    BUCKET_BOUNDS_MS[i - 1] as f64
                };
                // The top bucket has no finite upper bound; use `max_ms` as its
                // effective ceiling so interpolation stays finite.
                let upper = if i == 7 {
                    self.max_ms as f64
                } else {
                    BUCKET_BOUNDS_MS[i] as f64
                };
                let frac = ((target - cum) / bucket_count).clamp(0.0, 1.0);
                let val = lower + frac * (upper - lower);
                let clamped = val.min(self.max_ms as f64).max(0.0);
                return Some(clamped.round() as u32);
            }
            cum = next_cum;
        }
        Some(self.max_ms as u32)
    }

    /// Arithmetic mean (§8.1). `None` when `count == 0`.
    pub fn mean_ms(&self) -> Option<u32> {
        if self.count == 0 {
            return None;
        }
        Some((self.sum_ms / self.count) as u32)
    }
}
