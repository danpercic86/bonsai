//! Tick/job integration tests (real repos, injected time): fetch, suppression,
//! overlap guard, poison recovery, and backoff progression.

use std::process::Command;

use super::test_support::*;
use super::*;

/// A due autoFetch tick against a local bare remote updates the
/// remote-tracking ref, records Success, and emits repo-changed +
/// job-status-changed (§AI-gate).
#[test]
fn auto_fetch_updates_remote_tracking_ref() {
    let dir = scratch_dir();
    let (work, _bare, other) = fetch_fixture(dir.path());

    // New commit lands in the bare remote from the second clone.
    commit_file(&other, "b.txt", "two\n", "c2");
    git(&other, &["push", "origin", "main"]);
    let pushed = rev_parse(&other, "HEAD");
    assert_ne!(rev_parse(&work, "refs/remotes/origin/main"), pushed);

    let sched = sched_with_auto_fetch(1); // base 60_000 ms
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];

    // Tick 0: first sight — baseline only, nothing runs (D13).
    drive_tick(&repos, &sched, 0, &emit);
    assert!(job_statuses(&events).is_empty());
    assert_eq!(rev_parse(&work, "refs/remotes/origin/main"), rev_parse(&work, "HEAD"));

    // One interval later: the fetch runs and moves origin/main.
    drive_tick(&repos, &sched, MIN, &emit);
    assert_eq!(rev_parse(&work, "refs/remotes/origin/main"), pushed);

    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    let s = &statuses[0];
    assert_eq!(s.outcome, JobOutcome::Success);
    assert_eq!(s.job, JobKind::AutoFetch);
    assert_eq!(s.repo_id, "work");
    assert!(s.updated_refs.is_some_and(|n| n > 0));
    assert_eq!(s.consecutive_failures, 0);
    assert!(!s.in_backoff && !s.entered_backoff);
    assert_eq!(s.next_run_ms, Some(2 * MIN));
    assert_eq!(repo_changed_count(&events), 1);

    // A success with zero updated refs emits NO repo-changed (D8).
    drive_tick(&repos, &sched, 2 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[1].outcome, JobOutcome::Success);
    assert_eq!(statuses[1].updated_refs, Some(0));
    assert_eq!(repo_changed_count(&events), 1);
}

/// A real conflicted merge opstate suppresses the fetch: outcome
/// Suppressed, remote-tracking ref NOT updated, no failure counted (D5).
#[test]
fn auto_fetch_suppressed_during_conflicted_merge() {
    let dir = scratch_dir();
    let (work, _bare, other) = fetch_fixture(dir.path());

    // Conflicting histories: both sides edit a.txt.
    git(&work, &["checkout", "-b", "feature"]);
    commit_file(&work, "a.txt", "feature\n", "feature edit");
    git(&work, &["checkout", "main"]);
    commit_file(&work, "a.txt", "main\n", "main edit");
    // Real conflicted merge (git exits non-zero — run without assert).
    let out = Command::new("git")
        .current_dir(&work)
        .args(["merge", "feature"])
        .output()
        .expect("run git merge");
    assert!(!out.status.success(), "merge should conflict");
    assert!(work.join(".git").join("MERGE_HEAD").exists());

    // Meanwhile the remote moved.
    commit_file(&other, "b.txt", "two\n", "c2");
    git(&other, &["push", "origin", "main"]);
    let pushed = rev_parse(&other, "HEAD");
    let before = rev_parse(&work, "refs/remotes/origin/main");
    assert_ne!(before, pushed);

    let sched = sched_with_auto_fetch(1);
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline
    drive_tick(&repos, &sched, MIN, &emit); // due → suppressed

    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].outcome, JobOutcome::Suppressed);
    assert_eq!(statuses[0].consecutive_failures, 0);
    assert_eq!(repo_changed_count(&events), 0);
    // Ref untouched.
    assert_eq!(rev_parse(&work, "refs/remotes/origin/main"), before);
    // Normal reschedule: next run one interval after the suppressed run.
    assert_eq!(statuses[0].next_run_ms, Some(2 * MIN));
}

/// While a run is in flight (running flag held), a due tick records
/// Skipped and counts no failure (D4). Simulated via the contract's
/// long-running-flag mechanism.
#[test]
fn overlap_guard_skips_while_running() {
    let dir = scratch_dir();
    let (work, _bare, _other) = fetch_fixture(dir.path());

    let sched = sched_with_auto_fetch(1);
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline

    // Simulate a slow fetch still in flight.
    sched
        .jobs
        .lock()
        .expect("jobs lock")
        .get_mut(&("work".to_string(), JobKind::AutoFetch))
        .expect("entry")
        .running = true;

    drive_tick(&repos, &sched, MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].outcome, JobOutcome::Skipped);
    assert_eq!(statuses[0].consecutive_failures, 0);
    assert!(!statuses[0].entered_backoff);
    {
        let jobs = sched.jobs.lock().expect("jobs lock");
        let entry = &jobs[&("work".to_string(), JobKind::AutoFetch)];
        assert_eq!(entry.last_outcome, Some(JobOutcome::Skipped));
        assert!(entry.running, "flag untouched by the skip");
        assert!(entry.skip_signaled, "first skip recorded");
    }

    // N1: further due ticks during the SAME in-flight run stay quiet —
    // only the first skip of a given run is signaled.
    drive_tick(&repos, &sched, 2 * MIN, &emit);
    drive_tick(&repos, &sched, 3 * MIN, &emit);
    assert_eq!(job_statuses(&events).len(), 1, "no repeat Skipped events");

    // The run completes (flag cleared) → the job runs for real, which
    // resets skip_signaled; a NEW overlapping run may signal once again.
    sched
        .jobs
        .lock()
        .expect("jobs lock")
        .get_mut(&("work".to_string(), JobKind::AutoFetch))
        .expect("entry")
        .running = false;
    drive_tick(&repos, &sched, 4 * MIN, &emit); // real run → Success
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 2);
    assert_eq!(statuses[1].outcome, JobOutcome::Success);
    {
        let mut jobs = sched.jobs.lock().expect("jobs lock");
        let entry = jobs
            .get_mut(&("work".to_string(), JobKind::AutoFetch))
            .expect("entry");
        assert!(!entry.skip_signaled, "cleared by run completion");
        entry.running = true; // simulate the next slow run
    }
    drive_tick(&repos, &sched, 6 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[2].outcome, JobOutcome::Skipped);
}

/// S1: a poisoned scheduler mutex must not wedge the loop — locks recover
/// via `PoisonError::into_inner` and ticks keep working.
#[test]
fn poisoned_locks_recover() {
    let dir = scratch_dir();
    let (work, _bare, _other) = fetch_fixture(dir.path());

    let sched = sched_with_auto_fetch(1);
    // Poison both mutexes by panicking while holding them.
    let s = sched.clone();
    let _ = std::thread::spawn(move || {
        let _cfg = s.cfg.lock().expect("cfg lock");
        let _jobs = s.jobs.lock().expect("jobs lock");
        panic!("poison");
    })
    .join();
    assert!(sched.cfg.lock().is_err(), "cfg poisoned");
    assert!(sched.jobs.lock().is_err(), "jobs poisoned");

    // apply_config still works.
    apply_config(
        &sched,
        JobsConfig {
            auto_fetch: AutoFetch {
                enabled: true,
                interval_minutes: 1,
            },
            health_refresh: HealthRefresh {
                enabled: false,
                interval_minutes: 30,
            },
        },
    );

    // Ticks still schedule and complete runs.
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline
    drive_tick(&repos, &sched, MIN, &emit); // runs despite poison
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 1);
    assert_eq!(statuses[0].outcome, JobOutcome::Success);

    // run-now also still works on the poisoned state.
    let (emit2, events2) = collecting_emitter();
    let handle = start_job_now(&sched, "work", work, JobKind::AutoFetch, 2 * MIN, emit2)
        .expect("run-now recovers");
    tauri::async_runtime::block_on(async {
        let _ = handle.await;
    });
    assert_eq!(job_statuses(&events2).len(), 1);
}

/// Failure escalation: a broken remote URL fails each run; the 3rd
/// consecutive failure sets enteredBackoff exactly once and the interval
/// stretches per D6. A later success resets everything.
#[test]
fn backoff_progression_and_reset_on_success() {
    let dir = scratch_dir();
    let (work, bare, _other) = fetch_fixture(dir.path());
    let bare_url = file_url(&bare);

    // Point origin at a nonexistent path → every fetch fails.
    let missing = dir.path().join("missing.git");
    let missing_url = file_url(&missing);
    git(&work, &["remote", "set-url", "origin", &missing_url]);

    let sched = sched_with_auto_fetch(1);
    let (emit, events) = collecting_emitter();
    let repos = vec![("work".to_string(), work.clone())];
    drive_tick(&repos, &sched, 0, &emit); // baseline at t=0

    // Failures 1 and 2 at base cadence: no backoff yet.
    drive_tick(&repos, &sched, MIN, &emit);
    drive_tick(&repos, &sched, 2 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 2);
    assert!(statuses.iter().all(|s| s.outcome == JobOutcome::Failed));
    assert!(statuses.iter().all(|s| !s.in_backoff && !s.entered_backoff));
    assert_eq!(statuses[1].consecutive_failures, 2);
    assert!(statuses[1].error.is_some());

    // Failure 3: enteredBackoff EXACTLY here.
    drive_tick(&repos, &sched, 3 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 3);
    assert_eq!(statuses[2].consecutive_failures, 3);
    assert!(statuses[2].in_backoff && statuses[2].entered_backoff);
    // Effective interval now 2×: next estimated at last_run + 2 min.
    assert_eq!(statuses[2].next_run_ms, Some(3 * MIN + 2 * MIN));

    // One base interval later: NOT due (backoff). No new events.
    drive_tick(&repos, &sched, 4 * MIN, &emit);
    assert_eq!(job_statuses(&events).len(), 3);

    // At the backed-off time it runs again; failure 4 has
    // enteredBackoff == false (only the 2→3 transition toasts).
    drive_tick(&repos, &sched, 5 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 4);
    assert_eq!(statuses[3].consecutive_failures, 4);
    assert!(statuses[3].in_backoff && !statuses[3].entered_backoff);

    // Repair the remote; the next (backed-off, 4×) run succeeds and
    // resets the failure count.
    git(&work, &["remote", "set-url", "origin", &bare_url]);
    drive_tick(&repos, &sched, 5 * MIN + 4 * MIN, &emit);
    let statuses = job_statuses(&events);
    assert_eq!(statuses.len(), 5);
    assert_eq!(statuses[4].outcome, JobOutcome::Success);
    assert_eq!(statuses[4].consecutive_failures, 0);
    assert!(!statuses[4].in_backoff && !statuses[4].entered_backoff);
}
