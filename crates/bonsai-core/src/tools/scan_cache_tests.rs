//! The coalescing rule (audit LOW-2, 2026-09-15): N concurrent probes ⇒ ONE
//! probe, one panicking probe must not wedge the cell, and — the property the
//! second of those two enables and so has to be pinned separately — a follower
//! of a probe that panicked must not be handed the PREVIOUS probe's rows.
//!
//! Every case builds its OWN [`ScanCell`], so the `scan_tests` house rule holds
//! here too: nothing touches the process-global cache, and nothing spawns
//! `reg.exe` — the probe is a closure, and these pass fake ones.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Barrier;
use std::time::{Duration, Instant};

use crate::external::TargetOs;
use crate::tools::{catalog, Resolution, ToolKind, ToolSource};

use super::{Rows, ScanCell};

/// One real catalog row, so "the follower got the LEADER's rows" is a comparison
/// with content rather than two empty vectors.
fn rows() -> Rows {
    let entry = catalog::find_for(ToolKind::Editor, "vscode", TargetOs::Windows)
        .expect("the catalog has a Windows vscode row (AC8 pins totality)");
    vec![(
        entry,
        Resolution {
            program: r"C:\Code.exe".to_string(),
            bundle: None,
            source: ToolSource::Path,
        },
    )]
}

/// The fix itself: four callers that all bypass the cache at once (four Rescans)
/// run ONE probe between them, and all four get that probe's rows and its
/// `at_ms` — so `scanned_at_ms` still advances for every caller that asked.
///
/// **What this case does NOT prove:** `claim`'s check-and-set atomicity. The
/// leader's sleep is this case's only synchronisation after the barrier, so a
/// TOCTOU `claim` (two callers both observing `probing == false` before either
/// sets it) would still pass it on most runs. That property is established by
/// *reading* `claim` — one write acquisition around both the check and the set —
/// not by this assertion.
#[test]
fn concurrent_probes_coalesce_onto_a_single_probe() {
    const CALLERS: usize = 4;
    let cell = ScanCell::new();
    let probes = AtomicUsize::new(0);
    let gate = Barrier::new(CALLERS);

    let results: Vec<(u64, Rows)> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..CALLERS)
            .map(|_| {
                s.spawn(|| {
                    gate.wait();
                    cell.probe(|| {
                        probes.fetch_add(1, Ordering::SeqCst);
                        // Long enough that every follower is inside
                        // `await_leader` before this publishes — a follower
                        // descheduled past it would become a SECOND leader and
                        // fail the assertion below. Well under `PROBE_WAIT`
                        // (3 s), so the margin is free.
                        std::thread::sleep(Duration::from_millis(300));
                        rows()
                    })
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("probe thread"))
            .collect()
    });

    assert_eq!(
        probes.load(Ordering::SeqCst),
        1,
        "{CALLERS} callers must produce ONE probe"
    );
    let (at_ms, first) = results.first().expect("one result per caller");
    assert_eq!(
        first.len(),
        1,
        "the leader's rows came back, not an empty scan"
    );
    for (other_ms, other) in &results {
        assert_eq!(
            other_ms, at_ms,
            "every caller got the leader's scan identity"
        );
        assert_eq!(other, first, "every caller got the leader's rows");
    }
}

/// A published probe is what `cached_rows` serves, and `probe` (the Rescan path)
/// deliberately does NOT consult it — it replaces it.
#[test]
fn a_probe_publishes_what_the_cache_then_serves_and_a_refresh_replaces_it() {
    let cell = ScanCell::new();
    assert!(
        cell.cached().is_none(),
        "nothing is cached before the first probe"
    );

    let (at_ms, found) = cell.probe(rows);
    assert_eq!(found.len(), 1);
    let (hit_ms, hit) = cell.cached().expect("the probe published its rows");
    assert_eq!(hit_ms, at_ms);
    assert_eq!(hit, found);

    let (next_ms, next) = cell.probe(Vec::new);
    assert!(
        next.is_empty(),
        "the refresh returns its OWN probe, not the cache"
    );
    assert!(next_ms >= at_ms, "the scan identity never moves backwards");
    assert!(
        cell.cached().expect("republished").1.is_empty(),
        "the cache was replaced"
    );
}

/// A probe that panics clears the in-flight flag on the way out (that is what
/// `Lease`'s `Drop` is for). Without it, `probing` would stay `true` forever and
/// EVERY later Rescan would wait out `PROBE_WAIT` for a probe that will never
/// publish — turning one bad probe into a permanently degraded picker.
#[test]
fn a_panicking_probe_does_not_wedge_the_cell() {
    let cell = ScanCell::new();
    let blew_up = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cell.probe(|| panic!("probe blew up"));
    }));
    assert!(
        blew_up.is_err(),
        "the panic propagates to the caller, as before"
    );

    let started = Instant::now();
    let (_, found) = cell.probe(rows);
    assert_eq!(found.len(), 1, "the next caller LEADS instead of waiting");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "it waited for a dead leader: {:?}",
        started.elapsed()
    );
}

/// The stale HANDOFF that `Lease`'s panic-path clear enables, and that
/// `ScanState::generation` closes: a leader that panics clears `probing`
/// WITHOUT publishing, so a follower wakes on the next poll, sees "done", and —
/// before the generation check — returned the PREVIOUS probe's rows with the
/// PREVIOUS `at_ms`. A Rescan that silently answered with pre-Rescan data, and
/// with nothing in flight to replace it.
///
/// [`a_panicking_probe_does_not_wedge_the_cell`] cannot catch this: it has no
/// follower and an empty cache, so it exercises only the wedge property — never
/// the handoff that property enables. This is the case that was missing.
#[test]
fn a_follower_reprobes_when_the_leader_panics_instead_of_serving_stale_rows() {
    let cell = ScanCell::new();
    // What a stale handoff would hand back.
    let (stale_ms, stale) = cell.probe(rows);
    assert_eq!(
        stale.len(),
        1,
        "the first probe published rows that can go stale"
    );

    // Opened from INSIDE the leader's probe closure, i.e. only once the lease is
    // held — so the caller below is guaranteed to be a follower.
    let gate = Barrier::new(2);
    let follower_probes = AtomicUsize::new(0);

    let (at_ms, found) = std::thread::scope(|s| {
        let leader = s.spawn(|| {
            let blew_up = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                cell.probe(|| -> Rows {
                    gate.wait();
                    // Long enough that the follower's `claim` fails and it is
                    // inside `await_leader` when this unwinds. Without the sleep
                    // the follower could lead, and the case would pass vacuously.
                    std::thread::sleep(Duration::from_millis(200));
                    panic!("probe blew up before publishing")
                });
            }));
            assert!(blew_up.is_err(), "this case needs the leader to panic");
        });
        gate.wait();
        let got = cell.probe(|| {
            follower_probes.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        });
        leader.join().expect("leader thread");
        got
    });

    assert_eq!(
        follower_probes.load(Ordering::SeqCst),
        1,
        "the follower must run its OWN probe: the dead leader published nothing, \
         so nothing is coming to replace the stale rows"
    );
    assert!(
        found.is_empty(),
        "the follower got the dead leader's PREDECESSOR's rows"
    );
    assert!(
        at_ms > stale_ms,
        "the follower got the stale scan identity, so its Rescan looks like it never landed"
    );
    assert!(
        cell.cached().expect("the follower published").1.is_empty(),
        "the follower's own probe replaced the stale cache"
    );
}
