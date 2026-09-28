//! Restart durability, daily bucketing / retention, `perf.*` absorption.

use super::*;

// ------------------------------------------------------- restart durability

#[test]
fn counters_survive_restart() {
    let path = scratch("restart");
    let day = "2026-08-27";
    {
        let store = MetricsState::for_test(path.clone(), 1_756_000_000);
        store.bump_counter("commit.create", 4, day);
        store.observe_ipc_result("getStatus", 12.0, None, day);
        store.flush(&perf(0, 0, 0), 1_756_000_000).expect("flush");
    }
    // A fresh store loading the SAME file is the "restart".
    let store2 = MetricsState::for_test(path.clone(), 1_756_100_000);
    let snap = store2.snapshot();
    let today = snap.days.iter().find(|d| d.date == day).expect("day");
    assert_eq!(today.totals.counters.get("commit.create"), Some(&4));
    assert!(today.totals.durations.contains_key("cmd.getStatus"));
    let _ = std::fs::remove_file(&path);
}

// -------------------------------------------------------- daily bucketing

#[test]
fn daily_bucketing_splits_across_a_date_change() {
    let path = scratch("dates");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    store.bump_counter("commit.create", 1, "2026-08-27");
    store.bump_counter("commit.create", 2, "2026-08-28");
    let snap = store.snapshot();
    assert_eq!(snap.days.len(), 2);
    assert_eq!(snap.days[0].date, "2026-08-27");
    assert_eq!(snap.days[1].date, "2026-08-28");
    assert_eq!(snap.days[0].totals.counters.get("commit.create"), Some(&1));
    assert_eq!(snap.days[1].totals.counters.get("commit.create"), Some(&2));
    let _ = std::fs::remove_file(&path);
}

/// Outside the retention window a day bucket folds into `lifetime` (§8 / §F6), so
/// the `days` vec never grows without bound. Exercises `MetricTotals::merge` +
/// `Histogram::merge` through the DATE-CHANGE trigger (`totals_for`), which is
/// the only retention trigger a long-running session ever reaches.
///
/// §F6 changed the rule from "the oldest bucket past 400 entries" to "every
/// bucket older than 90 calendar days", so the dates here are REAL consecutive
/// days: under the old positional rule any 401 distinct strings would do, which
/// is exactly why that rule could keep buckets spanning years.
#[test]
fn retention_folds_out_of_window_days_into_lifetime() {
    const BASE: i64 = 1_756_000_000;
    let path = scratch("retain");
    let store = MetricsState::for_test(path.clone(), BASE);
    // 401 consecutive calendar days, each with one counter tick.
    for i in 0..401i64 {
        let date = crate::obs::writer::utc_date(BASE + i * 86_400);
        store.bump_counter("commit.create", 1, &date);
    }
    let snap = store.snapshot();
    assert_eq!(
        snap.days.len(),
        super::super::RETAIN_DAYS,
        "bounded by the window"
    );
    assert_eq!(
        snap.days[0].date,
        crate::obs::writer::utc_date(BASE + (400 - 89) * 86_400),
        "the oldest survivor is exactly today-89"
    );
    // Every day that fell out of the window landed in lifetime.
    assert_eq!(snap.lifetime.counters.get("commit.create"), Some(&311));
    let _ = std::fs::remove_file(&path);
}

// ------------------------------------------------------- perf absorption

#[test]
fn perf_deltas_appear_as_perf_star() {
    let path = scratch("perf");
    let store = MetricsState::for_test(path.clone(), 1_756_000_000);
    let day = "2026-08-27";
    store.fold_perf(&perf(2, 5, 9), day);
    // A second fold records only the DELTA (monotone counters).
    store.fold_perf(&perf(3, 5, 12), day);
    let snap = store.snapshot();
    let c = &snap.days[0].totals.counters;
    assert_eq!(c.get("perf.repo_opens"), Some(&3)); // 2 + 1
    assert_eq!(c.get("perf.graph_walks"), Some(&5)); // 5 + 0
    assert_eq!(c.get("perf.status_scans"), Some(&12)); // 9 + 3
    let _ = std::fs::remove_file(&path);
}
