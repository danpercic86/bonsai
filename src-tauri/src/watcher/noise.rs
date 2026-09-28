//! P113d — the `relevant == 0` notify batches (`.git`-internals noise) that
//! never enter a debounce burst. The notify handler counts them here; the
//! debounce thread drains the counts onto its next watcher record (a fired
//! record, or a standalone noise summary when no burst fires within
//! [`NOISE_FLUSH`] / at shutdown).

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

/// Upper bound on how long undrained noise waits for a fired record to carry
/// it: the idle debounce thread wakes this often and emits a noise summary.
pub(super) const NOISE_FLUSH: Duration = Duration::from_secs(60);

/// `relevant == 0` notify batches not yet reported. Written by the notify
/// handler, drained by the debounce thread. Wait-free: two relaxed adds on the
/// producer, two relaxed swaps on the drain. No lock, no allocation, no IO
/// (obs/mod.rs guarantee 1: producers never block a git or UI path).
///
/// Ordering caveat: the two atomics are independent, so a drain interleaved
/// between a producer's two adds may attribute one batch's paths to the next
/// record. Totals stay exact.
///
/// Saturation: plain wrapping `fetch_add` on `AtomicU32` is deliberate —
/// 4.29 G batches (or paths) between two drains at most `NOISE_FLUSH` apart is
/// unreachable, and a CAS-loop saturating add would cost the producer more.
#[derive(Default)]
pub(super) struct NoiseCounters {
    batches: AtomicU32,
    paths: AtomicU32,
}

/// One drained noise tally.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SkippedNoise {
    pub(super) batches: u32,
    pub(super) paths: u32,
}

impl NoiseCounters {
    /// Notify thread: counts one irrelevant batch of `paths` paths.
    pub(super) fn absorb(&self, paths: u32) {
        self.batches.fetch_add(1, Ordering::Relaxed);
        self.paths.fetch_add(paths, Ordering::Relaxed);
    }

    /// Debounce thread: drains both counters; `None` when no batch was counted.
    pub(super) fn take(&self) -> Option<SkippedNoise> {
        let batches = self.batches.swap(0, Ordering::Relaxed);
        let paths = self.paths.swap(0, Ordering::Relaxed);
        if batches == 0 {
            // A drain interleaved with a producer's two adds already reported
            // that batch; its late paths stay counted for the next record
            // instead of being swallowed with this `None` (keeps totals exact).
            if paths > 0 {
                self.paths.fetch_add(paths, Ordering::Relaxed);
            }
            return None;
        }
        Some(SkippedNoise { batches, paths })
    }
}
