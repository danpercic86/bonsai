//! P91 §8 — the persistence half of [`MetricsState`]: `flush`, `reset`, and the
//! single `persist` they share.
//!
//! **The ordering rule this file exists to state (increment-6 review, item 1).**
//! Persisting is two steps that cannot be one lock: snapshot the in-memory
//! `MetricsFile` under the state mutex, then do file IO with the mutex RELEASED
//! (counters keep being bumped while we serialize). `metrics_file`'s `SAVE_LOCK`
//! makes the second step atomic between savers, but it is acquired only *after*
//! the snapshot, so two savers could snapshot A→B and commit B→A — the OLDER
//! bytes landing last.
//!
//! That is not a theoretical loss: the pair that races is exactly `metrics_reset`
//! versus the 60 s flush timer, so the observable failure was **a reset silently
//! undone on disk** by an in-flight flush holding a pre-reset snapshot — and both
//! paths then cleared `dirty`, so nothing rewrote the emptied file until the next
//! counter bump. `usage.json` is durable, is NOT in the `logs_delete_all` scope
//! (§8), and `metrics_reset` is the user's only way to clear it.
//!
//! **Fix: a revision stamp, not a wider lock.** Every accepted mutation bumps
//! `Inner::rev` (`MetricsState::mark_dirty`). A saver stamps the `rev` its
//! snapshot was taken at and, *under `SAVE_LOCK`*, refuses to commit when a
//! higher `rev` has already been committed by this store. Compare-then-commit is
//! therefore atomic between savers, and the newest snapshot always wins.
//!
//! Two alternatives were rejected:
//! * **Widen `SAVE_LOCK` over snapshot + save.** Correct, but it nests
//!   `SAVE_LOCK` → state mutex, adding a second lock order to a module that
//!   already has a non-reentrant re-entry hazard (`fold_perf` holds the state
//!   mutex across its whole loop, which is why `bump_validated` takes
//!   already-locked totals). It would also clone a 400-day `MetricsFile` while
//!   holding the IO lock, and it makes the failing interleaving reproducible only
//!   by racing threads.
//! * **Document the residual staleness.** Rejected on the merits above: a doc
//!   cannot stop a reset from being undone.
//!
//! Scope, unchanged from `SAVE_LOCK`'s: the stamp is per-store. Two
//! `MetricsState` instances (or two processes) sharing one path still conflict at
//! the in-memory level, which no commit-side check can repair.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use bonsai_core::error::AppError;

use super::{metrics_file, MetricTotals, MetricsFile, MetricsState, METRICS_SCHEMA_VERSION};
use crate::obs::writer;
use crate::perf::PerfCounters;

impl MetricsState {
    /// Folds perf deltas + wall time, then persists atomically if dirty. Blocking
    /// (writes a file). `now_secs` fixes both "today" and the wall-time delta.
    pub fn flush(&self, perf: &PerfCounters, now_secs: i64) -> Result<(), AppError> {
        let today = writer::utc_date(now_secs);
        self.fold_perf(perf, &today);
        {
            let mut g = self.lock();
            let wall = (now_secs - g.last_wall_secs).max(0) as u64 * 1000;
            g.last_wall_secs = now_secs;
            if wall > 0 {
                let t = Self::totals_for(&mut g, &today);
                t.session_ms = t.session_ms.saturating_add(wall);
                Self::mark_dirty(&mut g);
            }
        }
        self.persist()
    }

    /// `metrics_reset()` — clears every aggregate to a fresh file and persists.
    /// Headless: exposed as a command but never surfaced in a settings catalog.
    pub fn reset(&self, now_secs: i64) -> Result<(), AppError> {
        {
            let mut g = self.lock();
            g.file = MetricsFile {
                schema: METRICS_SCHEMA_VERSION,
                first_seen: writer::utc_date(now_secs),
                sessions: 0,
                days: Vec::new(),
                lifetime: MetricTotals::default(),
            };
            g.perf_baseline = PerfCounters::default();
            g.last_wall_secs = now_secs;
            Self::mark_dirty(&mut g);
        }
        self.persist()
    }

    /// Snapshot → commit, the ONE door to `usage.json`.
    ///
    /// Both `dirty` and `rev` are captured with the snapshot, so:
    /// * a snapshot older than what is already on disk is dropped instead of
    ///   overwriting it (the module doc's reset-undone case), and
    /// * `dirty` is cleared only if `rev` has not moved since the snapshot — an
    ///   observation recorded *while* we were writing keeps the file dirty and is
    ///   picked up by the next flush, rather than being silently dropped.
    ///
    /// A failed commit leaves `dirty` set, so the next flush retries.
    fn persist(&self) -> Result<(), AppError> {
        let snapshot: Option<(PathBuf, MetricsFile, u64)> = {
            let g = self.lock();
            match (&g.path, g.dirty) {
                // Not initialised (no config dir), or nothing changed — either way
                // there is nothing to write.
                (None, _) | (_, false) => None,
                (Some(path), true) => Some((path.clone(), g.file.clone(), g.rev)),
            }
        };
        let Some((path, file, rev)) = snapshot else {
            return Ok(());
        };
        after_snapshot_hook();

        {
            // Compare-then-commit, atomic between savers. `SAVE_LOCK` is held for
            // the IO only; the state mutex is never taken inside it.
            let guard = metrics_file::begin_save();
            if self.commit_rev.load(Ordering::Acquire) > rev {
                // A newer snapshot beat us to disk. Its bytes supersede ours, and
                // it cleared `dirty` for its own revision — dropping this write is
                // what keeps the newest state on disk.
                return Ok(());
            }
            guard.commit(&path, &file)?;
            self.commit_rev.store(rev, Ordering::Release);
        }

        let mut g = self.lock();
        if g.rev == rev {
            g.dirty = false;
        }
        Ok(())
    }
}

/// Test seam for the interleaving above: runs once, between a saver's snapshot
/// and its commit, so the racing operation can be injected deterministically
/// instead of raced for.
///
/// Thread-local (unit tests run in parallel in one process, so a process-wide
/// hook would fire inside unrelated tests) and **one-shot** — it is `take`n
/// before running, so a hook that itself persists cannot recurse.
#[cfg(test)]
pub(super) fn arm_after_snapshot_hook(f: Box<dyn FnOnce()>) {
    AFTER_SNAPSHOT.with(|h| *h.borrow_mut() = Some(f));
}

#[cfg(test)]
thread_local! {
    static AFTER_SNAPSHOT: std::cell::RefCell<Option<Box<dyn FnOnce()>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn after_snapshot_hook() {
    let armed = AFTER_SNAPSHOT.with(|h| h.borrow_mut().take());
    if let Some(f) = armed {
        f();
    }
}

/// Compiled out of every non-test build — no branch, no atomic, no cost.
#[cfg(not(test))]
#[inline(always)]
fn after_snapshot_hook() {}

#[cfg(test)]
#[path = "tests_metrics_ordering.rs"]
mod tests_metrics_ordering;
