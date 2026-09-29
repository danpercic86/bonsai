//! E4 AC1 / AC7 / AC8 / AC9 / AC12 + repo switch: the run lifecycle over a
//! counting transport and git2 fixture repos (no Tauri app, no network).

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use bonsai_core::error::AppError;
use bonsai_deps::cache::CACHE_DIR;
use bonsai_deps::DEPSDEV_PARALLEL;

use super::exec::{start_run_inner, DISABLED_MESSAGE};
use super::test_support::*;
use super::*;
use crate::scheduler::unix_now_ms;

const LONG: Duration = Duration::from_secs(600);

fn cfg(enabled: bool) -> DepTracker {
    DepTracker {
        enabled,
        interval_hours: 12,
    }
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tauri::async_runtime::block_on(f)
}

/// Starts a check and waits for its task to end.
fn run_to_end(h: &Harness, repo_id: &str, wd: &std::path::Path) -> u64 {
    let (id, task) = start_run_inner(&h.state, repo_id, wd.to_path_buf()).expect("started");
    block_on(async { task.expect("a new run").await.expect("task") });
    id
}

#[test]
fn disabled_makes_zero_requests() {
    let dir = scratch();
    let wd = repo(dir.path(), &[("package.json", &npm_manifest(3))]);
    let rid = id(&wd);
    let h = harness(dir.path(), false, Arc::default(), TestEnv::default(), LONG);
    on_active_changed(&h.state, None, Some((&rid, wd.clone())));
    for i in 0..10 {
        tick(
            &h.state,
            Some((rid.clone(), wd.clone())),
            unix_now_ms() + i * 86_400_000,
        );
    }
    on_fs_change(&h.state, &rid, wd.clone());
    let v = view(&h.state, &rid, &wd);
    assert_eq!(v, DepTrackerView::disabled(rid.clone()));
    match start_run(&h.state, &rid, wd.clone()) {
        Err(AppError::Other(m)) => assert_eq!(m, DISABLED_MESSAGE),
        other => panic!("expected the disabled refusal, got {other:?}"),
    }
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(h.transport.sends(), 0);
    assert!(h.events.all().is_empty());
    assert!(!h.data_root.join(CACHE_DIR).exists());
}

#[test]
fn view_returns_cached_before_network_and_manual_runs_regardless_of_interval() {
    let dir = scratch();
    let wd = repo(dir.path(), &[("package.json", &npm_manifest(2))]);
    let rid = id(&wd);
    let t = TestTransport::holding();
    t.release();
    let h = harness(dir.path(), true, t.clone(), TestEnv::default(), LONG);

    run_to_end(&h, &rid, &wd);
    assert_eq!(h.transport.sends(), 2);
    let v = view(&h.state, &rid, &wd);
    let report = v.report.clone().expect("a cached report");
    assert_eq!(report.groups.len(), 1);
    assert!(v.last_error.is_none() && v.running.is_none() && !v.manifests_changed);
    assert!(v.next_check_ms.is_some());

    // Just checked, yet a manual refresh runs again; while it is blocked in
    // the transport, the view serves the cached report plus `running`.
    let held = t;
    held.hold();
    let (second, task) = start_run_inner(&h.state, &rid, wd.clone()).expect("started");
    assert!(task.is_some(), "a new run, not the finished one");
    assert!(lock(&h.state.repos)[&rid].last_attempt_ms.is_some());
    wait_until("a blocked send", || {
        held.in_flight.load(Ordering::SeqCst) > 0
    });
    // A second request while running resolves the SAME checkId.
    assert_eq!(start_run(&h.state, &rid, wd.clone()).expect("id"), second);
    let during = view(&h.state, &rid, &wd);
    assert_eq!(during.report, Some(report));
    assert_eq!(during.running.map(|r| r.check_id), Some(second));
    held.release();
    block_on(async { task.expect("new run").await.expect("task") });
    let kinds = h.events.kinds(&rid);
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == DepsChangedKind::Finished)
            .count(),
        2
    );
}

#[test]
fn deadline_times_out_and_discards() {
    let dir = scratch();
    let wd = repo(dir.path(), &[("package.json", &npm_manifest(1))]);
    let rid = id(&wd);
    let t = TestTransport::holding();
    let h = harness(
        dir.path(),
        true,
        t.clone(),
        TestEnv::default(),
        Duration::from_millis(300),
    );
    let (_, task) = start_run_inner(&h.state, &rid, wd.clone()).expect("start");
    block_on(async { task.expect("run").await.expect("task") });
    let v = view(&h.state, &rid, &wd);
    assert_eq!(
        v.last_error.map(|e| e.kind),
        Some(CheckFailureKind::Timeout)
    );
    assert!(v.running.is_none() && v.report.is_none());
    assert_eq!(h.events.kinds(&rid).last(), Some(&DepsChangedKind::Failed));
    // The orphaned worker ends at its next cancel check: no more sends, no
    // persist.
    t.release();
    std::thread::sleep(Duration::from_millis(200));
    assert_eq!(t.sends(), 1);
    assert!(!h.data_root.join(CACHE_DIR).exists());
}

#[test]
fn manifest_edit_marks_stale_without_check() {
    let dir = scratch();
    let wd = repo(dir.path(), &[("package.json", &npm_manifest(1))]);
    let rid = id(&wd);
    let t = TestTransport::holding();
    t.release();
    let h = harness(dir.path(), true, t.clone(), TestEnv::default(), LONG);
    run_to_end(&h, &rid, &wd);
    *lock(&h.state.active) = Some((rid.clone(), wd.clone()));
    let sends = t.sends();

    std::fs::write(wd.join("package.json"), npm_manifest(4)).expect("edit");
    on_fs_change(&h.state, &rid, wd.clone());
    wait_until("Stale", || {
        h.events.kinds(&rid).contains(&DepsChangedKind::Stale)
    });
    assert!(view(&h.state, &rid, &wd).manifests_changed);
    // A second burst inside 2 s (or once stale) probes nothing more.
    on_fs_change(&h.state, &rid, wd.clone());
    std::thread::sleep(Duration::from_millis(100));
    let kinds = h.events.kinds(&rid);
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == DepsChangedKind::Stale)
            .count(),
        1
    );
    assert_eq!(
        kinds
            .iter()
            .filter(|k| **k == DepsChangedKind::Started)
            .count(),
        1
    );
    assert_eq!(t.sends(), sends, "a stale report never triggers a check");
}

#[test]
fn disable_cancels_and_wipes() {
    let dir = scratch();
    let wd = repo(dir.path(), &[("package.json", &npm_manifest(9))]);
    let rid = id(&wd);
    // A previous report on disk, so the wipe has something to remove.
    let t = TestTransport::holding();
    t.release();
    let h = harness(dir.path(), true, t.clone(), TestEnv::default(), LONG);
    run_to_end(&h, &rid, &wd);
    assert!(h.data_root.join(CACHE_DIR).exists());

    let held = t;
    held.hold();
    let before = held.sends();
    let (_, task) = start_run_inner(&h.state, &rid, wd.clone()).expect("start");
    // [LOW-1] all DEPSDEV_PARALLEL workers are blocked inside the transport.
    wait_until("6 blocked workers", || {
        held.in_flight.load(Ordering::SeqCst) == DEPSDEV_PARALLEL
    });
    let at_disable = h.events.all().len();
    apply_config(&h.state, cfg(false), &[rid.clone(), "other-tab".to_owned()]);
    assert!(
        !h.data_root.join(CACHE_DIR).exists(),
        "wiped under the lock"
    );
    held.release();
    block_on(async { task.expect("run").await.expect("task") });

    assert_eq!(
        held.sends() - before,
        DEPSDEV_PARALLEL,
        "in-flight completed, none started"
    );
    assert!(
        !h.data_root.join(CACHE_DIR).exists(),
        "no persist after the wipe"
    );
    let after: Vec<_> = h.events.all().split_off(at_disable);
    assert!(after.iter().all(|p| p.kind != DepsChangedKind::Progress));
    assert!(after.iter().all(|p| p.kind != DepsChangedKind::Finished));
    for tab in [rid.as_str(), "other-tab"] {
        assert!(
            after
                .iter()
                .any(|p| p.repo_id == tab && p.kind == DepsChangedKind::Cleared),
            "Cleared for {tab}"
        );
    }
    assert!(after
        .iter()
        .any(|p| p.repo_id == rid && p.kind == DepsChangedKind::Cancelled));
    assert_eq!(
        view(&h.state, &rid, &wd),
        DepTrackerView::disabled(rid.clone())
    );

    // The startup retry (§6.4) removes a leftover directory; missing is fine.
    std::fs::create_dir_all(h.data_root.join(CACHE_DIR)).expect("leftover");
    store::wipe(Some(&h.data_root));
    assert!(!h.data_root.join(CACHE_DIR).exists());
    store::wipe(Some(&h.data_root));
}

#[test]
fn switch_cancels_old_run_and_enable_starts_the_active_repo() {
    let dir = scratch();
    let a = repo(&dir.path().join("a"), &[("package.json", &npm_manifest(2))]);
    let b = repo(&dir.path().join("b"), &[("README.md", "no manifests")]);
    let (ra, rb) = (id(&a), id(&b));
    let held = TestTransport::holding();
    let h = harness(dir.path(), false, held.clone(), TestEnv::default(), LONG);

    // Activated while disabled: nothing runs; enabling starts it (Enabled).
    on_active_changed(&h.state, None, Some((&ra, a.clone())));
    assert!(h.events.all().is_empty());
    apply_config(&h.state, cfg(true), &[ra.clone(), rb.clone()]);
    assert_eq!(h.events.kinds(&ra), vec![DepsChangedKind::Started]);
    wait_until("a blocked send", || {
        held.in_flight.load(Ordering::SeqCst) > 0
    });

    // Switching to B cancels A's run; B's first activation runs (Open).
    on_active_changed(&h.state, Some(&ra), Some((&rb, b.clone())));
    held.release();
    wait_until("A cancelled", || {
        h.events.kinds(&ra).contains(&DepsChangedKind::Cancelled)
    });
    wait_until("B finished", || {
        h.events.kinds(&rb).contains(&DepsChangedKind::Finished)
    });
    assert!(!h.events.kinds(&ra).contains(&DepsChangedKind::Finished));
    // A cancelled run wrote nothing; B's (manifest-less) report is cached.
    assert!(view(&h.state, &ra, &a).report.is_none());
    let vb = view(&h.state, &rb, &b).report.expect("B cached");
    assert!(vb.groups.is_empty());

    // Closing B forgets it: a reopen counts as a fresh "opened".
    on_repo_closed(&h.state, &rb);
    assert!(!lock(&h.state.repos).contains_key(&rb));
    assert!(lock(&h.state.active).is_none());
}
