//! P91 §F6 — the two ways `usage.json` loses data: the 90-day window
//! (`prune_days`) and the user-driven clear (`MetricsState::clear`).
//!
//! Kept out of `metrics.rs` so the observation hot path and the forget path are
//! separately readable (and so neither file crosses the 500-line cap).
//!
//! **Both are lossy and neither may ever fail the app**, so the invariants they
//! must not break are stated where they are implemented:
//! * the window folds `days[]` into `lifetime` and touches nothing else —
//!   `first_seen`, `sessions` and `lifetime` are LIFETIME figures and survive it
//!   (user ruling 2026-09-11, §3.5);
//! * the clear removes all four, in memory AND on disk, because either half
//!   alone is a no-op the next flush undoes (§4.1).

use std::path::PathBuf;

use bonsai_core::error::AppError;

use super::{metrics_file, MetricTotals, MetricsFile, MetricsState, METRICS_SCHEMA_VERSION};
use crate::obs::metrics_purge::{purge_metrics_dir, ClearMode, MetricsClearCounts};
use crate::obs::writer;
use crate::perf::PerfCounters;

use super::RETAIN_DAYS;

/// Loads `path` (with `.bak` recovery), normalises the header fields, bumps
/// `sessions`, and applies the §F6 window. Shared by [`MetricsState::init`] and
/// the test constructor so the MIGRATION is exercised by both.
///
/// **This IS the migration (§3.4).** An installed build can hold up to 400 day
/// buckets, and the load path is the only thing that prunes an install that sat
/// unused for months; the pruned file is persisted at the first flush. No schema
/// bump — the shape is unchanged, only the number of elements.
///
/// Returns the file plus `bak_stale`: true iff the window folded something, which
/// means the copy `save_locked` is about to rotate into `usage.json.bak` is a
/// PRE-window profile that `load` would happily restore.
pub(super) fn load_pruned(path: &std::path::Path, now_secs: i64) -> (MetricsFile, bool) {
    let mut file = metrics_file::load(path);
    if file.schema == 0 {
        file.schema = METRICS_SCHEMA_VERSION;
    }
    if file.first_seen.is_empty() {
        file.first_seen = writer::utc_date(now_secs);
    }
    file.sessions = file.sessions.saturating_add(1);
    let folded = prune_days(&mut file, now_secs);
    (file, folded > 0)
}

/// Folds every day bucket older than the retention window into `lifetime`.
/// Returns the number of buckets folded (0 = nothing to do).
///
/// `now_secs` fixes "today". Comparison is LEXICOGRAPHIC: [`writer::utc_date`]
/// emits zero-padded `YYYY-MM-DD`, so string order IS chronological order and no
/// date parsing is needed.
///
/// **Age, not bucket count (§3.2).** The old rule was purely positional
/// (`while days.len() >= RETAIN_DAYS`), and the app is not used every day — 90
/// buckets can span years of calendar time, while the signed privacy copy says
/// "kept for 90 days". The count rule survives in `totals_for` as a structural
/// backstop for a clock regression or a hand-edited file, not as the mechanism.
///
/// **`retain`, not a prefix drain:** `days` is only *expected* to be sorted (see
/// `MetricsState::totals_for`'s append-only clock assumption); a backwards UTC
/// step can append an out-of-order bucket, which a prefix drain would keep
/// forever.
///
/// Boundary, pinned by test: with `RETAIN_DAYS = 90`, a bucket dated
/// `today − 89` survives and one dated `today − 90` folds — 90 days inclusive of
/// today.
pub(super) fn prune_days(file: &mut MetricsFile, now_secs: i64) -> usize {
    let window = (RETAIN_DAYS as i64 - 1).max(0) * 86_400;
    prune_before(file, &writer::utc_date(now_secs - window))
}

/// [`prune_days`] from the `YYYY-MM-DD` "today" the observation path already
/// carries, so `totals_for` need not thread epoch seconds through five public
/// signatures. An unparseable date folds NOTHING (metrics never fail the app);
/// the count backstop in `totals_for` still bounds the vec in that case.
pub(super) fn prune_days_for(file: &mut MetricsFile, today: &str) -> usize {
    match date_minus_days(today, RETAIN_DAYS as i64 - 1) {
        Some(cutoff) => prune_before(file, &cutoff),
        None => 0,
    }
}

/// The fold itself. `cutoff` is the INCLUSIVE lower bound of the window.
fn prune_before(file: &mut MetricsFile, cutoff: &str) -> usize {
    let mut folded = 0usize;
    for bucket in &file.days {
        if bucket.date.as_str() < cutoff {
            file.lifetime.merge(&bucket.totals);
            folded += 1;
        }
    }
    if folded > 0 {
        file.days.retain(|b| b.date.as_str() >= cutoff);
    }
    folded
}

/// `YYYY-MM-DD` minus `n` days, in UTC. `None` on any malformed input — the
/// caller treats that as "do not prune" rather than guessing a window.
fn date_minus_days(date: &str, n: i64) -> Option<String> {
    let b = date.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return None;
    }
    let y: i64 = date.get(0..4)?.parse().ok()?;
    let m: u32 = date.get(5..7)?.parse().ok()?;
    let d: u32 = date.get(8..10)?.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) {
        return None;
    }
    let days = days_from_civil(y, m, d).checked_sub(n)?;
    Some(writer::utc_date(days.checked_mul(86_400)?))
}

/// Howard Hinnant's `days_from_civil` — the exact inverse of the
/// `civil_from_days` that `writer::utc_date` already uses, so the two agree by
/// construction. Same reasoning as `writer_files.rs`: not worth a dependency.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let mp = i64::from((m + 9) % 12); // March = 0
    let doy = (153 * mp + 2) / 5 + i64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

impl MetricsState {
    /// THE door for "forget everything" (§4.1). Blocking (file IO) — call on the
    /// blocking pool.
    ///
    /// Clearing is **two inseparable halves**: the in-memory `MetricsFile` is
    /// replaced first, then the disk is touched. Doing only the disk half is a
    /// no-op the next 60 s flush undoes, with the OLD numbers, and a test that
    /// asserts only "the file is gone" passes while the button does nothing.
    ///
    /// `perf` must be the **current** [`crate::perf::PerfState::snapshot`], not a
    /// default: `fold_perf` records `snapshot - perf_baseline` and `PerfState`'s
    /// counters are process-lifetime and are NOT reset here, so a zeroed baseline
    /// would re-add every pre-clear repo open, graph walk and status scan at the
    /// next fold — resurrecting the counts the user just cleared, ~60 s later.
    pub fn clear(
        &self,
        perf: &PerfCounters,
        now_secs: i64,
        mode: ClearMode,
    ) -> Result<MetricsClearCounts, AppError> {
        // --- 1. memory first, under the state mutex ---------------------------
        // `fresh` is cloned OUT here on purpose: the commit below runs under
        // `SAVE_LOCK`, which must never re-take the state mutex (§8.3's single
        // lock order).
        let (path, fresh, rev): (Option<PathBuf>, MetricsFile, u64) = {
            let mut g = self.lock();
            g.file = MetricsFile {
                schema: METRICS_SCHEMA_VERSION,
                first_seen: writer::utc_date(now_secs),
                sessions: 0,
                days: Vec::new(),
                lifetime: MetricTotals::default(),
            };
            g.perf_baseline = perf.clone();
            g.last_wall_secs = now_secs;
            // The pre-clear `.bak` is handled explicitly below, in both modes.
            g.bak_stale = false;
            Self::mark_dirty(&mut g);
            // Nothing scheduled may re-write the data we are about to destroy.
            g.dirty = false;
            (g.path.clone(), g.file.clone(), g.rev)
        };
        let Some(path) = path else {
            // Uninitialised (no config dir): the aggregate is cleared, there is no
            // file to remove, and that is a COMPLETE success — hence
            // `dir_removed: true`, which is what makes the command report
            // `metricsCleared` honestly instead of claiming a failure.
            return Ok(MetricsClearCounts {
                dir_removed: true,
                ..MetricsClearCounts::default()
            });
        };

        // --- 2. fence every in-flight saver, then touch the disk --------------
        // A saver already inside `commit` finishes first (we block here); a saver
        // that snapshotted BEFORE this clear now has `rev < commit_rev`, so
        // `persist`'s compare-then-commit drops it. That is the same §8.3
        // mechanism the reset-undone-on-disk case uses, reused rather than
        // re-invented. A save that starts AFTER this one snapshots the fresh
        // empty file and `save_locked` re-creates the directory, so removing it
        // is safe: no writer ever sees a missing directory as an error.
        let guard = metrics_file::begin_save();
        self.commit_rev
            .store(rev, std::sync::atomic::Ordering::Release);
        let counts = match mode {
            ClearMode::ResetInPlace => {
                guard.commit(&path, &fresh)?;
                // Without this the FULL pre-reset profile lives on in `.bak` and
                // `metrics_file::load` restores it the moment the primary is torn.
                let _ = std::fs::remove_file(metrics_file::bak_path(&path));
                // `metrics_reset` reports no counts to any UI (it is headless), so
                // the file tallies stay at their defaults; `dir_removed` is the
                // one field with a meaning here, and the directory survives.
                MetricsClearCounts::default()
            }
            ClearMode::DeleteFiles => match path.parent() {
                Some(dir) => purge_metrics_dir(dir),
                // A path with no parent cannot happen (it is always
                // `<config>/metrics/usage.json`), but it must not panic: the
                // in-memory half already succeeded.
                None => MetricsClearCounts::default(),
            },
        };
        drop(guard);
        Ok(counts)
    }
}

#[cfg(test)]
#[path = "tests_metrics_clear.rs"]
mod tests_metrics_clear;
