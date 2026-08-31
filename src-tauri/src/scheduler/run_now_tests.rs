//! Run-now, health-refresh, and P30 tester-gap integration tests.

use super::test_support::*;
use super::*;


/// run-now (D10): runs immediately even mid-backoff and while not due;
/// rejects when already running; a successful run-now clears backoff.
#[test]
fn run_now_ignores_backoff_and_rejects_overlap() {
    let dir = scratch_dir();
    let (work, _bare, _other) = fetch_fixture(dir.path());

    let sched = sched_with_auto_fetch(1);
    // Seed a deep-backoff state, not due for a long time.
    {
        let mut jobs = sched.jobs.lock().expect("jobs lock");
        jobs.insert(
            ("work".to_string(), JobKind::AutoFetch),
            JobRuntime {
                running: false,
                last_run_ms: Some(0),
                last_outcome: Some(JobOutcome::Failed),
                last_error: Some("boom".to_string()),
                consecutive_failures: 5,
                skip_signaled: false,
            },
        );
    }
    let (emit, events) = collecting_emitter();

    // Overlap rejection.
    sched
        .jobs
        .lock()
        .expect("jobs lock")
        .get_mut(&("work".to_string(), JobKind::AutoFetch))
        .expect("entry")
        .running = true;
    let err = match start_job_now(
        &sched,
        "work",
        work.clone(),
        JobKind::AutoFetch,
        MIN,
        emit.clone(),
    ) {
        Err(e) => e,
        Ok(_) => panic!("run-now while running must error"),
    };
    assert_eq!(err.to_string(), "job already running");
    sched
        .jobs
        .lock()
        .expect("jobs lock")
        .get_mut(&("work".to_string(), JobKind::AutoFetch))
        .expect("entry")
        .running = false;

    // Immediate run despite backoff; success clears it.
    let handle = start_job_now(&sched, "work", work.clone(), JobKind::AutoFetch, MIN, emit)
        .expect("run-now starts");
    tauri::async_runtime::block_on(async {
        let _ = handle.await;
    });
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].outcome, JobOutcome::Success);
    assert_eq!(statuses[0].consecutive_failures, 0);
    assert!(!statuses[0].in_backoff);
}

/// healthRefresh does no git work and emits repo-changed + Success on its
/// interval; disabled jobs never run; pruning drops closed repos (D2/D8).
#[test]
fn health_refresh_signal_and_pruning() {
    let dir = scratch_dir();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("mkdir");
    init_repo(&work);
    commit_file(&work, "a.txt", "one\n", "c1");

    let sched = SchedulerState::default();
    apply_config(
        &sched,
        JobsConfig {
            auto_fetch: AutoFetch {
                enabled: false,
                interval_minutes: 5,
            },
            health_refresh: HealthRefresh {
                enabled: true,
                interval_minutes: 1,
            },
        },
    );
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline
    drive_tick(&repos, &sched, MIN, &emit);

    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].job, JobKind::HealthRefresh);
    assert_eq!(statuses[0].outcome, JobOutcome::Success);
    assert_eq!(statuses[0].updated_refs, None);
    assert_eq!(repo_changed_count(&events), 1);
    // Disabled autoFetch never baselined a run.
    {
        let jobs = sched.jobs.lock().expect("jobs lock");
        assert_eq!(
            jobs[&("work".to_string(), JobKind::AutoFetch)].last_run_ms,
            None
        );
    }

    // Repo closed → entries pruned on the next tick.
    drive_tick(&[], &sched, 2 * MIN, &emit);
    assert!(sched.jobs.lock().expect("jobs lock").is_empty());
}

// ---------------- P30 tester gap tests ----------------

/// D7 config plumbing on a LEGACY settings file: a pre-P30 settings.json
/// (no `healthRefresh` key) loaded via `settings::load_from` and pushed
/// through `apply_config` yields healthRefresh at its disabled default —
/// the job never runs — while the legacy autoFetch value is honored.
/// (The full `set_ui_settings` command needs an AppHandle for the settings
/// path; this covers everything below that seam: load → clamp → apply.)
#[test]
fn legacy_settings_file_through_apply_config() {
    let dir = scratch_dir();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("mkdir");
    init_repo(&work);
    commit_file(&work, "a.txt", "one\n", "c1");

    let file = dir.path().join("settings.json");
    // Pre-P30 shape: autoFetch present, NO healthRefresh key.
    std::fs::write(
        &file,
        r#"{ "theme": "dark", "autoFetch": { "enabled": true, "intervalMinutes": 1 } }"#,
    )
    .expect("write legacy settings");
    let loaded = crate::settings::load_from(&file);
    assert_eq!(loaded.health_refresh, HealthRefresh::default());
    assert!(!loaded.health_refresh.enabled, "default is disabled");

    let sched = SchedulerState::default();
    apply_config(
        &sched,
        JobsConfig {
            auto_fetch: loaded.auto_fetch,
            health_refresh: loaded.health_refresh,
        },
    );

    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline
    // Far in the future: healthRefresh must STILL never run (disabled ⇒
    // Wait{i64::MAX}); autoFetch IS due (it fails — no remote — which is
    // fine: it proves the legacy enabled=true was applied).
    drive_tick(&repos, &sched, 100 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert!(
        statuses.iter().all(|s| s.job == JobKind::AutoFetch),
        "healthRefresh must not run from a legacy file: {statuses:?}"
    );
    assert_eq!(statuses.len(), 1, "autoFetch ran per legacy config");
    // Disabled job was never baselined either.
    let jobs = sched.jobs.lock().expect("jobs lock");
    assert_eq!(
        jobs[&("work".to_string(), JobKind::HealthRefresh)].last_run_ms,
        None
    );
}

/// D2 mid-flight close: the repo is removed from the open set while its
/// job future is still running. No panic; the completion may transiently
/// re-insert its bookkeeping entry, but the next tick prunes it and no
/// entry is left with `running = true` (no ghost/wedged job).
#[test]
fn repo_closed_mid_flight_no_ghost_state() {
    let dir = scratch_dir();
    let (work, _bare, _other) = fetch_fixture(dir.path());

    let sched = sched_with_auto_fetch(1);
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline

    // Start the run but do NOT await it yet; prune first.
    let handles = tick_once(&repos, &sched, MIN, &emit);
    assert_eq!(handles.len(), 1, "autoFetch spawned");
    // Repo closed between plan and completion: an empty-tick prune runs
    // while the job is in flight.
    let pruned = tick_once(&[], &sched, MIN + 1, &emit);
    assert!(pruned.is_empty());
    assert!(
        sched.jobs.lock().expect("jobs lock").is_empty(),
        "in-flight entry pruned with the repo"
    );

    // Completion must not panic and must not leave running=true anywhere.
    tauri::async_runtime::block_on(async {
        for h in handles {
            h.await.expect("job future must not panic");
        }
    });
    {
        let jobs = sched.jobs.lock().expect("jobs lock");
        assert!(
            jobs.values().all(|e| !e.running),
            "no entry stuck running after mid-flight close"
        );
    }
    // Next tick with the repo still closed prunes any transient re-insert.
    drive_tick(&[], &sched, 2 * MIN, &emit);
    assert!(sched.jobs.lock().expect("jobs lock").is_empty());
    // The completed run reported normally (Success against the intact
    // remote) — a stale event for a closed repo is harmless; the frontend
    // filters on repoId.
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].outcome, JobOutcome::Success);
}

/// A remote that VANISHES after a successful fetch (bare repo dir deleted
/// out from under `origin`) → ordinary `Failed` into the backoff path;
/// no repo-changed emission, error recorded, scheduler not wedged (a
/// later tick still runs).
#[test]
fn remote_vanished_fails_into_backoff_without_wedging() {
    let dir = scratch_dir();
    let (work, bare, _other) = fetch_fixture(dir.path());

    let sched = sched_with_auto_fetch(1);
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline
    drive_tick(&repos, &sched, MIN, &emit); // healthy success first
    assert_eq!(job_statuses(&events)[0].outcome, JobOutcome::Success);

    // The remote disappears.
    std::fs::remove_dir_all(&bare).expect("delete bare remote");

    drive_tick(&repos, &sched, 2 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 2);
    let s = &statuses[1];
    assert_eq!(s.outcome, JobOutcome::Failed);
    assert_eq!(s.consecutive_failures, 1);
    assert!(s.error.is_some(), "error string recorded");
    assert!(!s.in_backoff && !s.entered_backoff);
    assert_eq!(
        repo_changed_count(&events),
        0,
        "no repo-changed on failure (first success had 0 updated refs)"
    );

    // Not wedged: the next due tick runs again (failure 2).
    drive_tick(&repos, &sched, 3 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[2].outcome, JobOutcome::Failed);
    assert_eq!(statuses[2].consecutive_failures, 2);
    let jobs = sched.jobs.lock().expect("jobs lock");
    assert!(!jobs[&("work".to_string(), JobKind::AutoFetch)].running);
}

/// run-now on a repo with NO remotes: `fetch_all` returns
/// `AppError::NoRemote`, which the scheduler records as an ordinary
/// `Failed` outcome (contract §8 — job-internal errors land in
/// `lastError`, never returned to the caller); the command-start itself
/// succeeds and the running flag is released.
#[test]
fn run_now_without_remotes_records_failed() {
    let dir = scratch_dir();
    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).expect("mkdir");
    init_repo(&work);
    commit_file(&work, "a.txt", "one\n", "c1");

    let sched = sched_with_auto_fetch(1);
    let (emit, events) = collecting_emitter();
    let handle = start_job_now(&sched, "work", work, JobKind::AutoFetch, MIN, emit)
        .expect("start accepted — error surfaces via the event, not the command");
    tauri::async_runtime::block_on(async {
        let _ = handle.await;
    });

    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    assert_eq!(s.outcome, JobOutcome::Failed);
    assert_eq!(s.consecutive_failures, 1);
    assert!(
        s.error.as_deref().is_some_and(|e| e.contains("no remotes")),
        "NoRemote message surfaced: {:?}",
        s.error
    );
    assert_eq!(repo_changed_count(&events), 0);
    let jobs = sched.jobs.lock().expect("jobs lock");
    assert!(!jobs[&("work".to_string(), JobKind::AutoFetch)].running);
}
