//! The streaming session's TIME SOURCE — the seam that makes the idle watchdog
//! and the hard cap testable by ORDERING instead of by wall clock (P91).
//!
//! Why this exists: every watchdog assertion used to be a race between two real
//! durations — the test's `idle_timeout` and however long `cmd.exe` + the batch
//! stub took to print its first line. On a loaded box the second one wins, the
//! watchdog fires during process startup, and the test fails on an unrelated
//! assertion (`watchdog must keep the lines already read: ["Claude produced no
//! output for 2s — stopped"]`). Widening the timeout only moves that cliff.
//!
//! With a [`Clock`] the session never reads the wall clock for a POLICY decision.
//! Production passes [`SystemClock`] and behaves exactly as before; tests pass a
//! [`TestClock`] that stands still until the test advances it, so "the watchdog
//! fires" becomes a consequence of an explicit test action rather than of elapsed
//! time. Everything that waits on the OPERATING SYSTEM instead (the post-exit
//! grace in `session::complete`, the stderr drain's `recv_timeout`) deliberately
//! keeps using real time — virtualising those would just deadlock on a real
//! child.

use std::time::Instant;

/// Where the streaming session reads "now" for its idle watchdog, its hard cap
/// and its event timestamps. `Sync` because a test holds the same clock the
/// session thread is reading.
pub(crate) trait Clock: Send + Sync {
    fn now(&self) -> Instant;
}

/// Production time source: the real monotonic clock.
pub(crate) struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

#[cfg(test)]
pub(crate) use test_clock::TestClock;

#[cfg(test)]
mod test_clock {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};

    use super::Clock;

    /// A clock that moves ONLY when the test moves it.
    ///
    /// Two things a test needs, and both are ordering facts rather than
    /// durations:
    ///
    /// - [`TestClock::advance`] — make the session see idle time. Nothing else
    ///   can, so a watchdog firing is proof the test asked for it.
    /// - [`TestClock::reads`] — how many times the session has consulted the
    ///   clock. The tick loop reads it exactly once per tick, so waiting for this
    ///   counter to grow is a precise "the loop has now looked at the advanced
    ///   clock N times" signal. That is what lets the D3 test (the watchdog must
    ///   NOT fire while awaiting input) assert on the loop having had its chance,
    ///   instead of on a `sleep` that a loaded box could swallow.
    pub(crate) struct TestClock {
        base: Instant,
        offset_ms: AtomicU64,
        reads: AtomicU64,
    }

    impl TestClock {
        pub(crate) fn new() -> Self {
            TestClock {
                base: Instant::now(),
                offset_ms: AtomicU64::new(0),
                reads: AtomicU64::new(0),
            }
        }

        /// Move virtual time forward. Monotonic by construction (only ever added).
        pub(crate) fn advance(&self, by: Duration) {
            self.offset_ms.fetch_add(by.as_millis() as u64, Ordering::SeqCst);
        }

        /// How many times the session has read this clock so far.
        pub(crate) fn reads(&self) -> u64 {
            self.reads.load(Ordering::SeqCst)
        }
    }

    impl Clock for TestClock {
        fn now(&self) -> Instant {
            self.reads.fetch_add(1, Ordering::SeqCst);
            self.base + Duration::from_millis(self.offset_ms.load(Ordering::SeqCst))
        }
    }
}
