//! Review round 2: a cancelled-but-draining run is never joined or planned
//! as running; a cancel that precedes the deadline stays a cancellation; the
//! gate closes on the run's own token (`[LOW-E3]`).

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use super::exec::start_run_inner;
use super::gate::GatedTransport;
use super::test_support::*;
use super::*;

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

#[test]
fn a_draining_run_is_not_joined_after_switching_away_and_back() {
    let dir = scratch();
    let a = repo(&dir.path().join("a"), &[("package.json", &npm_manifest(2))]);
    let b = repo(&dir.path().join("b"), &[("README.md", "none")]);
    let (ra, rb) = (id(&a), id(&b));
    let held = TestTransport::holding();
    let h = harness(
        dir.path(),
        true,
        held.clone(),
        TestEnv::default(),
        Duration::from_secs(600),
    );

    // A runs (Open) and blocks in the transport.
    on_active_changed(&h.state, None, Some((&ra, a.clone())));
    wait_until("A blocked", || held.in_flight.load(Ordering::SeqCst) > 0);
    let first = lock(&h.state.repos)[&ra]
        .running
        .as_ref()
        .map(|r| r.check.check_id);
    // A → B cancels A's run (still draining); B → A re-plans A.
    on_active_changed(&h.state, Some(&ra), Some((&rb, b.clone())));
    on_active_changed(&h.state, Some(&rb), Some((&ra, a.clone())));
    let replanned = lock(&h.state.repos)[&ra]
        .running
        .as_ref()
        .map(|r| r.check.check_id);
    assert_ne!(replanned, first, "the tick started a fresh run, not waited");
    // "Check now" joins the LIVE run, never the dead one.
    let (joined, task) = start_run_inner(&h.state, &ra, a.clone()).expect("id");
    assert_eq!(Some(joined), replanned);
    assert!(task.is_none());

    held.release();
    wait_until("the live run finished", || {
        h.events
            .all()
            .iter()
            .any(|p| p.check_id == Some(joined) && p.kind == DepsChangedKind::Finished)
    });
    wait_until("the dead run cancelled", || {
        h.events
            .all()
            .iter()
            .any(|p| p.check_id == first && p.kind == DepsChangedKind::Cancelled)
    });
    assert!(view(&h.state, &ra, &a).report.is_some());
}

#[test]
fn a_cancel_before_the_deadline_is_recorded_as_cancelled() {
    let dir = scratch();
    let wd = repo(dir.path(), &[("package.json", &npm_manifest(1))]);
    let rid = id(&wd);
    let held = TestTransport::holding();
    let h = harness(
        dir.path(),
        true,
        held.clone(),
        TestEnv::default(),
        Duration::from_millis(300),
    );
    let (_, task) = start_run_inner(&h.state, &rid, wd.clone()).expect("start");
    wait_until("blocked", || held.in_flight.load(Ordering::SeqCst) > 0);
    // Cancelled (tab switch) while the worker is still blocked.
    on_active_changed(&h.state, Some(&rid), None);
    block_on(async { task.expect("run").await.expect("task") });
    assert_eq!(
        h.events.kinds(&rid).last(),
        Some(&DepsChangedKind::Cancelled)
    );
    let v = view(&h.state, &rid, &wd);
    assert!(v.last_error.is_none(), "not a timeout failure");
    assert!(v.running.is_none());
    assert!(lock(&h.state.repos)[&rid].last_attempt_ms.is_none());
    held.release();
}

#[test]
fn the_gate_closes_on_the_runs_own_token() {
    let dir = scratch();
    let h = harness(
        dir.path(),
        true,
        Arc::default(),
        TestEnv::default(),
        Duration::from_secs(1),
    );
    let token = bonsai_deps::CancelToken::new();
    let epoch = h.state.epoch.load(Ordering::SeqCst);
    let gate = GatedTransport::new(&h.state, epoch, token.clone());
    assert!(gate.is_open());
    token.cancel();
    assert!(
        !gate.is_open(),
        "a timed-out / cancelled run starts no request"
    );
    let other = GatedTransport::new(&h.state, epoch + 1, bonsai_deps::CancelToken::new());
    assert!(!other.is_open(), "a stale epoch stays closed");
}
