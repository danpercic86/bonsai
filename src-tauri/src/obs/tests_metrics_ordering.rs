//! P91 §8 — snapshot-ordering regression tests for `usage.json`.
//!
//! The interleaving under test is **injected, not raced**: `arm_after_snapshot_hook`
//! runs the competing operation at the exact point a saver has taken its snapshot
//! but has not committed it, so these tests are deterministic and single-threaded.

use std::sync::Arc;

use super::arm_after_snapshot_hook;
use crate::obs::metrics::MetricsState;
use crate::obs::metrics_file;
use crate::obs::writer;
use crate::perf::PerfCounters;

fn scratch(name: &str) -> std::path::PathBuf {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "bonsai-metrics-ord-{}-{}-{:?}",
        name,
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::create_dir_all(&p);
    p.push("usage.json");
    let _ = std::fs::remove_file(&p);
    p
}

const T0: i64 = 1_756_000_000;

/// The day bucket `flush(_, T0)` itself writes into. Observations must be filed
/// under the SAME date, or `totals_for`'s append-only clock rule puts them in a
/// second bucket and the assertions below read the wrong one.
fn day() -> String {
    writer::utc_date(T0)
}

/// The failure the revision stamp exists to stop: a flush snapshots the
/// pre-reset file, `metrics_reset` runs to completion (emptying memory AND
/// disk), and the flush then commits its stale bytes — silently restoring
/// everything the user just asked to be deleted. `usage.json` is durable, so the
/// resurrection is durable too. Since §F6 the same stamp fences
/// `MetricsState::clear`; `tests_metrics_clear.rs` is this test's mirror for the
/// delete path.
#[test]
fn an_in_flight_flush_cannot_undo_a_reset() {
    let path = scratch("reset-undo");
    let store = Arc::new(MetricsState::for_test(path.clone(), T0));
    store.bump_counter("commit.create", 7, &day());
    store.observe_ipc_result("getStatus", 12.0, None, &day());

    // Inject the race: the flush below has its (non-empty) snapshot in hand and
    // has not committed yet when the reset runs end to end.
    let resetter = Arc::clone(&store);
    arm_after_snapshot_hook(Box::new(move || {
        resetter
            .reset(&PerfCounters::default(), T0 + 10)
            .expect("reset");
    }));
    store.flush(&PerfCounters::default(), T0).expect("flush");

    let on_disk = metrics_file::load(&path);
    assert!(
        on_disk.days.is_empty(),
        "the stale flush resurrected {} day bucket(s) the reset had cleared",
        on_disk.days.len()
    );
    assert_eq!(on_disk.sessions, 0, "reset must leave sessions cleared");
    assert!(on_disk.lifetime.counters.is_empty());
    let _ = std::fs::remove_file(&path);
}

/// Dropping a stale write must not wedge the store: the very next observation
/// still reaches disk. (Guards against a stamp that only ever rejects.)
#[test]
fn a_dropped_stale_write_does_not_block_later_saves() {
    let path = scratch("stale-then-fresh");
    let store = Arc::new(MetricsState::for_test(path.clone(), T0));
    store.bump_counter("commit.create", 3, &day());
    let resetter = Arc::clone(&store);
    arm_after_snapshot_hook(Box::new(move || {
        resetter
            .reset(&PerfCounters::default(), T0 + 10)
            .expect("reset");
    }));
    store.flush(&PerfCounters::default(), T0).expect("flush");

    store.bump_counter("commit.amend", 2, &day());
    store.flush(&PerfCounters::default(), T0 + 20).expect("flush 2");

    let on_disk = metrics_file::load(&path);
    let bucket = on_disk.days.iter().find(|d| d.date == day()).expect("day bucket");
    assert_eq!(bucket.totals.counters.get("commit.amend"), Some(&2));
    assert!(
        !bucket.totals.counters.contains_key("commit.create"),
        "the pre-reset counter must stay gone"
    );
    let _ = std::fs::remove_file(&path);
}

/// An observation recorded *between* a saver's snapshot and its commit must keep
/// the file dirty, so the next flush writes it. Before the revision stamp both
/// savers cleared `dirty` unconditionally and such an observation sat in memory
/// until some later bump happened to re-dirty the file.
#[test]
fn an_observation_during_a_save_survives_to_the_next_flush() {
    let path = scratch("dirty-window");
    let store = Arc::new(MetricsState::for_test(path.clone(), T0));
    store.bump_counter("commit.create", 1, &day());

    let late = Arc::clone(&store);
    arm_after_snapshot_hook(Box::new(move || {
        // In-memory only: this bump is NOT in the snapshot being committed.
        late.bump_counter("commit.amend", 5, &day());
    }));
    store.flush(&PerfCounters::default(), T0).expect("flush");
    assert!(
        !metrics_file::load(&path)
            .days
            .iter()
            .any(|d| d.totals.counters.contains_key("commit.amend")),
        "sanity: the late bump cannot be in the committed snapshot"
    );

    // No new observation — only a flush. It must still write, i.e. `dirty` was
    // never cleared for a revision the commit did not contain.
    store.flush(&PerfCounters::default(), T0).expect("flush 2");
    let on_disk = metrics_file::load(&path);
    let bucket = on_disk.days.iter().find(|d| d.date == day()).expect("day bucket");
    assert_eq!(bucket.totals.counters.get("commit.amend"), Some(&5));
    let _ = std::fs::remove_file(&path);
}
