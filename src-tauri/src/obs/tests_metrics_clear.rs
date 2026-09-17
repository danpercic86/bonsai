//! P91 §F6 §8 items 1–7 — the 90-day window, its migration, and the delete.
//!
//! Kept out of `tests_metrics.rs` (already ~470 lines) so neither file has to be
//! read in full to work on either concern.
//!
//! Scratch dirs are `tempfile::TempDir`s, deliberately: the §8 item 11 guard in
//! `tests_metrics_purge.rs` forbids `remove_dir_all` anywhere under `obs/`, and
//! that includes the cleanup code of these tests.

use super::{prune_days, prune_days_for};
use crate::obs::metrics::{DayBucket, MetricTotals, MetricsFile, MetricsState, RETAIN_DAYS};
use crate::obs::metrics_file;
use crate::obs::metrics_purge::ClearMode;
use crate::obs::writer;
use crate::perf::PerfCounters;

/// A fixed "now" so every date below is deterministic.
const NOW: i64 = 1_789_000_000;

/// A scratch `metrics/usage.json` inside a temp dir that cleans itself up. The
/// `TempDir` must stay alive for the whole test — hence the tuple.
fn scratch() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let metrics = dir.path().join("metrics");
    std::fs::create_dir_all(&metrics).expect("mkdir");
    let path = metrics.join("usage.json");
    (dir, path)
}

/// A bucket dated `NOW - days_ago`, carrying one counter worth `n`.
fn bucket(days_ago: i64, key: &str, n: u64) -> DayBucket {
    let mut totals = MetricTotals::default();
    totals.counters.insert(key.to_string(), n);
    totals.session_ms = 1_000;
    DayBucket {
        date: writer::utc_date(NOW - days_ago * 86_400),
        totals,
    }
}

fn seeded(days: Vec<DayBucket>) -> MetricsFile {
    MetricsFile {
        schema: 1,
        first_seen: writer::utc_date(NOW - 400 * 86_400),
        sessions: 37,
        days,
        lifetime: MetricTotals::default(),
    }
}

/// The inclusive lower bound of the window around `NOW`.
fn window_start(now: i64) -> String {
    writer::utc_date(now - (RETAIN_DAYS as i64 - 1) * 86_400)
}

// ---------------------------------------------------------------- the window

/// §8 item 1 — the boundary, and §3.5's ruling in one assertion: `today-89`
/// SURVIVES, `today-90` FOLDS, and neither `first_seen` nor `sessions` moves.
#[test]
fn prune_folds_day_90_keeps_day_89_and_leaves_the_lifetime_figures_alone() {
    let mut file = seeded(vec![
        bucket(90, "commit.create", 7),
        bucket(89, "commit.create", 5),
        bucket(1, "commit.create", 3),
        bucket(0, "commit.create", 1),
    ]);
    let first_seen = file.first_seen.clone();

    let folded = prune_days(&mut file, NOW);

    assert_eq!(folded, 1, "exactly the day-90 bucket folds");
    assert_eq!(file.days.len(), 3, "89, 1 and 0 survive");
    assert_eq!(
        file.lifetime.counters.get("commit.create"),
        Some(&7),
        "the folded bucket's counters land in lifetime"
    );
    assert_eq!(file.lifetime.session_ms, 1_000);
    // §3.5, user ruling: the window applies to days[] ONLY.
    assert_eq!(
        file.first_seen, first_seen,
        "first_seen is a lifetime figure"
    );
    assert_eq!(file.sessions, 37, "sessions is a lifetime figure");
}

/// §8 item 3 — out-of-order buckets (a backwards UTC step appended an older date
/// after a newer one) are pruned by AGE, not position. A prefix drain would keep
/// the stale bucket forever, because it is not at index 0.
#[test]
fn prune_is_age_based_not_positional() {
    let stale = writer::utc_date(NOW - 365 * 86_400);
    let mut file = seeded(vec![
        bucket(0, "commit.create", 1),
        bucket(365, "commit.create", 9), // appended after a clock step backwards
        bucket(2, "commit.create", 2),
    ]);

    let folded = prune_days(&mut file, NOW);

    assert_eq!(folded, 1);
    assert_eq!(file.days.len(), 2);
    assert!(
        file.days.iter().all(|b| b.date != stale),
        "the mid-vec stale bucket is gone: {:?}",
        file.days.iter().map(|b| &b.date).collect::<Vec<_>>()
    );
    assert_eq!(file.lifetime.counters.get("commit.create"), Some(&9));
}

/// `prune_days_for` (the `totals_for` entry point) must agree with `prune_days`
/// exactly — same window reached from a `YYYY-MM-DD` instead of epoch seconds. A
/// disagreement would make the two triggers enforce two different windows.
#[test]
fn the_two_prune_entry_points_compute_the_same_window() {
    let days = vec![
        bucket(90, "a.b", 1),
        bucket(89, "a.b", 1),
        bucket(0, "a.b", 1),
    ];
    let mut by_secs = seeded(days.clone());
    let mut by_date = seeded(days);

    let a = prune_days(&mut by_secs, NOW);
    let b = prune_days_for(&mut by_date, &writer::utc_date(NOW));

    assert_eq!(a, b);
    assert_eq!(by_secs.days, by_date.days);
}

/// A malformed "today" must fold NOTHING rather than guess a window — metrics
/// never fail the app, and the count backstop still bounds the vec.
#[test]
fn an_unparseable_today_folds_nothing() {
    let mut file = seeded(vec![bucket(400, "a.b", 1), bucket(0, "a.b", 1)]);
    assert_eq!(prune_days_for(&mut file, "not-a-date"), 0);
    assert_eq!(file.days.len(), 2);
}

/// §8 item 2 — MIGRATION. A pre-seeded 400-bucket file (what an installed build
/// holds) is pruned at load, every folded bucket lands in `lifetime`, and the
/// first flush leaves NO `usage.json.bak` — otherwise `load` would restore the
/// 400-day profile the window change exists to discard.
#[test]
fn init_migrates_a_400_day_file_and_drops_the_rotated_bak() {
    let (_tmp, path) = scratch();
    let days: Vec<DayBucket> = (0..400)
        .map(|i| bucket(399 - i, "commit.create", 1))
        .collect();
    metrics_file::save(&path, &seeded(days)).expect("seed");
    // A pre-existing `.bak` stands in for the one `save_locked` is about to write.
    std::fs::write(metrics_file::bak_path(&path), b"{}").expect("seed bak");

    let store = MetricsState::for_test(path.clone(), NOW);
    let snap = store.snapshot();

    assert_eq!(snap.days.len(), RETAIN_DAYS, "today-89 .. today-0 survive");
    assert!(
        snap.days.iter().all(|b| b.date >= window_start(NOW)),
        "no survivor predates the window"
    );
    assert_eq!(
        snap.lifetime.counters.get("commit.create"),
        Some(&310),
        "400 seeded buckets - 90 survivors = 310 folded into lifetime"
    );
    assert_eq!(
        snap.sessions, 38,
        "sessions survives the fold and still bumps"
    );

    store.flush(&PerfCounters::default(), NOW).expect("flush");
    assert!(
        !metrics_file::bak_path(&path).exists(),
        "the rotated pre-window copy must not survive the migration's own save"
    );
}

/// §8 item 4 — the DATE-CHANGE trigger. A long session never re-hits the load
/// path, so an observation on a later day must itself enforce the window.
#[test]
fn a_date_change_inside_a_live_session_enforces_the_window() {
    let (_tmp, path) = scratch();
    let days: Vec<DayBucket> = (0..RETAIN_DAYS as i64)
        .map(|i| bucket(RETAIN_DAYS as i64 - 1 - i, "commit.create", 1))
        .collect();
    metrics_file::save(&path, &seeded(days)).expect("seed");

    let store = MetricsState::for_test(path.clone(), NOW);
    // "Tomorrow" arrives while the process keeps running.
    let tomorrow = writer::utc_date(NOW + 86_400);
    store.bump_counter("commit.create", 1, &tomorrow);

    let snap = store.snapshot();
    assert!(
        snap.days.len() <= RETAIN_DAYS,
        "still bounded: {}",
        snap.days.len()
    );
    assert_eq!(
        snap.days.last().map(|d| d.date.as_str()),
        Some(tomorrow.as_str())
    );
    assert!(
        snap.days
            .iter()
            .all(|b| b.date >= window_start(NOW + 86_400)),
        "the bucket that fell out of the window folded"
    );
}

/// §8 item 4, DISCRIMINATING — that `totals_for` calls `prune_days_for` at all.
///
/// The sibling test above (and `tests_metrics.rs`'s retention test) both seed
/// ~90 DENSE buckets, so `totals_for`'s count backstop
/// (`while days.len() >= RETAIN_DAYS`) evicts exactly the bucket the age rule
/// would fold and every assertion holds identically with the `prune_days_for`
/// call DELETED. They pin the function, not the wiring; `prune_days ≡
/// prune_days_for` is pinned separately. This one is sparse and discriminates:
///
/// * 3 buckets is far below `RETAIN_DAYS`, so the backstop is INERT — delete the
///   `prune_days_for` call from `MetricsState::totals_for` and this fails with
///   `days.len() == 4`, the three stale buckets still sitting in the window;
/// * the jump is 200 days, so all three are outside a 90-day window and the age
///   rule folds every one of them into `lifetime`.
#[test]
fn a_sparse_date_jump_folds_by_age_where_the_count_backstop_would_not() {
    let (_tmp, path) = scratch();
    // Three consecutive in-window days, so the load-path prune at NOW keeps them.
    let days: Vec<DayBucket> = (0..3i64)
        .map(|i| bucket(2 - i, "commit.create", 1))
        .collect();
    metrics_file::save(&path, &seeded(days)).expect("seed");

    let store = MetricsState::for_test(path.clone(), NOW);
    assert_eq!(
        store.snapshot().days.len(),
        3,
        "precondition: all three loaded"
    );

    // A single observation, 200 days later, inside the SAME live session.
    let far = writer::utc_date(NOW + 200 * 86_400);
    store.bump_counter("commit.create", 1, &far);

    let snap = store.snapshot();
    assert_eq!(
        snap.days.len(),
        1,
        "age folds all three; the count backstop alone would leave 4: {:?}",
        snap.days
            .iter()
            .map(|d| d.date.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(snap.days[0].date, far, "only the new day survives");
    assert_eq!(
        snap.lifetime.counters.get("commit.create"),
        Some(&3),
        "all three stale buckets folded into lifetime, none dropped"
    );
}

// ----------------------------------------------------------------- the clear

/// §8 item 5 (a)(b)(c)(e) — DeleteFiles clears BOTH halves. A test asserting only
/// one of them passes while the button does nothing.
#[test]
fn delete_files_empties_memory_and_removes_the_whole_folder() {
    let (_tmp, path) = scratch();
    let store = MetricsState::for_test(path.clone(), NOW);
    store.bump_counter("commit.create", 4, &writer::utc_date(NOW));
    store.flush(&PerfCounters::default(), NOW).expect("flush");
    std::fs::write(metrics_file::bak_path(&path), b"{}").expect("seed bak");
    assert!(path.exists(), "precondition: something to delete");

    let counts = store
        .clear(&PerfCounters::default(), NOW, ClearMode::DeleteFiles)
        .expect("clear");

    // (a) the in-memory half.
    let snap = store.snapshot();
    assert!(snap.days.is_empty(), "days cleared");
    assert_eq!(snap.lifetime, MetricTotals::default(), "lifetime cleared");
    assert_eq!(snap.sessions, 0, "(e) sessions reset");
    assert_eq!(
        snap.first_seen,
        writer::utc_date(NOW),
        "(e) first_seen is today"
    );
    // (b) + (c) the on-disk half: the files AND the directory.
    assert!(!path.exists(), "usage.json gone");
    assert!(!metrics_file::bak_path(&path).exists(), "(c) .bak gone");
    assert!(
        !path.parent().is_some_and(|d| d.exists()),
        "(b) the metrics folder itself is gone"
    );
    assert!(counts.dir_removed);
    assert_eq!(counts.deleted_files, 2, "usage.json + .bak counted");
}

/// §8 item 5 (d) — THE RE-BASELINE. `PerfState` is process-lifetime and is not
/// reset by a clear, so a zeroed `perf_baseline` would re-add every pre-clear
/// count at the next fold. Driving exactly ONE repo open after the clear must
/// record exactly 1 — asserting merely "not zero" cannot tell a correct
/// re-baseline from a fold that never ran.
#[test]
fn clear_rebaselines_perf_so_pre_clear_counts_do_not_come_back() {
    let (_tmp, path) = scratch();
    let store = MetricsState::for_test(path.clone(), NOW);
    let before = PerfCounters {
        repo_opens: 12,
        graph_walks: 3,
        graph_cache_hits: 0,
        graph_redecorates: 0,
        status_scans: 5,
    };
    store.flush(&before, NOW).expect("flush");

    store
        .clear(&before, NOW, ClearMode::DeleteFiles)
        .expect("clear");

    // Exactly one more repo open happens after the clear; `PerfState`'s counters
    // are cumulative, so the NEXT snapshot reads 13, not 1.
    let after = PerfCounters {
        repo_opens: 13,
        ..before.clone()
    };
    store.flush(&after, NOW + 60).expect("flush");

    let snap = store.snapshot();
    let total: u64 = snap
        .days
        .iter()
        .filter_map(|d| d.totals.counters.get("perf.repo_opens"))
        .sum();
    assert_eq!(
        total, 1,
        "one post-clear repo open — not 13 (zeroed baseline) and not 0 (no fold)"
    );
    // The folder the clear removed is re-created by the next save, containing
    // post-clear data only: no writer ever sees a missing directory as an error.
    assert!(
        path.exists(),
        "collection is always-on and re-creates the file"
    );
}

/// §8 item 7 — ResetInPlace leaves an EMPTY `usage.json` where `DeleteFiles`
/// leaves nothing, removes the `.bak` (without which the full pre-reset profile
/// survives), and keeps the directory.
#[test]
fn reset_in_place_leaves_an_empty_file_and_removes_the_bak() {
    let (_tmp, path) = scratch();
    let store = MetricsState::for_test(path.clone(), NOW);
    store.bump_counter("commit.create", 9, &writer::utc_date(NOW));
    store.flush(&PerfCounters::default(), NOW).expect("flush");
    std::fs::write(metrics_file::bak_path(&path), b"{}").expect("seed bak");

    store
        .clear(&PerfCounters::default(), NOW, ClearMode::ResetInPlace)
        .expect("clear");

    assert!(path.exists(), "the primary is rewritten, not removed");
    assert!(!metrics_file::bak_path(&path).exists(), ".bak removed");
    assert!(
        path.parent().is_some_and(|d| d.exists()),
        "the folder survives"
    );
    let on_disk = metrics_file::load(&path);
    assert!(on_disk.days.is_empty());
    assert_eq!(on_disk.sessions, 0);
    assert_eq!(on_disk.lifetime, MetricTotals::default());
}

/// §8 item 6 — THE FENCE. A saver that snapshotted BEFORE the clear must not be
/// able to write its pre-clear bytes afterwards. Injected deterministically at the
/// `after_snapshot_hook` seam rather than raced for; mirror of the §13 row 32
/// reset-undone-on-disk test.
#[test]
fn a_saver_that_snapshotted_before_the_clear_cannot_resurrect_the_folder() {
    let (_tmp, path) = scratch();
    let store = std::sync::Arc::new(MetricsState::for_test(path.clone(), NOW));
    store.bump_counter("commit.create", 4, &writer::utc_date(NOW));

    let clearer = std::sync::Arc::clone(&store);
    let p = path.clone();
    super::super::metrics_persist::arm_after_snapshot_hook(Box::new(move || {
        clearer
            .clear(&PerfCounters::default(), NOW, ClearMode::DeleteFiles)
            .expect("clear");
        assert!(!p.exists(), "the clear really did remove the file");
    }));

    // This flush snapshotted BEFORE the hook ran the clear, so its bytes are
    // stale and `persist`'s compare-then-commit must drop them.
    store.flush(&PerfCounters::default(), NOW).expect("flush");

    assert!(
        !path.exists(),
        "the pre-clear snapshot must not be committed"
    );
    assert!(
        !path.parent().is_some_and(|d| d.exists()),
        "and the folder must stay gone"
    );
    assert!(
        store.snapshot().days.is_empty(),
        "memory stayed cleared too"
    );
}
