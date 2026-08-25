//! Tick loop + job execution (P30 contract §3, D2–D10). The per-tick body
//! (`tick_once`), the detached per-job runner (`execute_job`), the manual
//! run-now entry (`start_job_now`), and the production loop (`run_scheduler`).
//! Split out of the module root for size; the pure planner + config + event
//! types live in `super`.

use std::path::PathBuf;
use std::sync::PoisonError;

use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::remote::fetch_all;
use tauri::Manager;

use crate::commands::RepoChangedPayload;
use crate::state::AppState;

use super::*;

const ALL_JOBS: [JobKind; 2] = [JobKind::AutoFetch, JobKind::HealthRefresh];

/// One scheduler tick over a snapshot of the open repos (D2: the repo map IS
/// the membership source of truth — closed repos are pruned here). Spawns a
/// detached future per due job (D3) and returns the join handles so tests can
/// await completion; the production loop drops them.
pub(crate) fn tick_once(
    repos: &[(String, PathBuf)],
    sched: &SchedulerState,
    now_ms: i64,
    emit: &EmitFn,
) -> Vec<tauri::async_runtime::JoinHandle<()>> {
    // S1: poison-recovering locks — a panicked job future must not silently
    // kill the scheduler (see `lock_recover`).
    let cfg = *lock_recover(&sched.cfg);
    let mut handles = Vec::new();
    let mut jobs = lock_recover(&sched.jobs);

    // Prune bookkeeping for repos that are no longer open.
    jobs.retain(|(repo_id, _), _| repos.iter().any(|(id, _)| id == repo_id));

    for (repo_id, path) in repos {
        for job in ALL_JOBS {
            let (enabled, base_ms) = cfg.job_params(job);
            let entry = jobs.entry((repo_id.clone(), job)).or_default();

            // D13: the first time an ENABLED job is seen, baseline it to
            // "ran at now" — first real run lands a full interval later.
            if enabled && entry.last_run_ms.is_none() {
                entry.last_run_ms = Some(now_ms);
            }

            match plan(
                enabled,
                base_ms,
                now_ms,
                entry.last_run_ms,
                entry.running,
                entry.consecutive_failures,
            ) {
                PlanDecision::Wait { .. } => {}
                PlanDecision::SkipOverlap => {
                    // N1: signal only the FIRST skip of a given in-flight
                    // run — a slow fetch must not emit Skipped every tick.
                    if entry.skip_signaled {
                        continue;
                    }
                    entry.skip_signaled = true;
                    // D4: record + signal, count no failure, don't touch
                    // last_run (the job stays due; the running run's own
                    // completion re-baselines it).
                    entry.last_outcome = Some(JobOutcome::Skipped);
                    let payload = JobStatusChangedPayload {
                        repo_id: repo_id.clone(),
                        job,
                        outcome: JobOutcome::Skipped,
                        updated_refs: None,
                        error: None,
                        consecutive_failures: entry.consecutive_failures,
                        in_backoff: entry.consecutive_failures >= BACKOFF_THRESHOLD,
                        entered_backoff: false,
                        ts_ms: now_ms,
                        next_run_ms: next_run_estimate_ms(
                            enabled,
                            base_ms,
                            entry.last_run_ms,
                            entry.consecutive_failures,
                        ),
                    };
                    emit(SchedulerEvent::JobStatus(payload));
                }
                PlanDecision::Run => {
                    entry.running = true;
                    entry.skip_signaled = false;
                    handles.push(tauri::async_runtime::spawn(execute_job(
                        sched.clone(),
                        repo_id.clone(),
                        path.clone(),
                        job,
                        enabled,
                        base_ms,
                        now_ms,
                        emit.clone(),
                    )));
                }
            }
        }
    }
    handles
}

/// What one job run produced (before bookkeeping).
enum RunResult {
    /// `updated_refs` is `Some` for autoFetch; `emit_repo_changed` per D8.
    Success {
        updated_refs: Option<u32>,
        emit_repo_changed: bool,
    },
    Suppressed,
    Failed(String),
}

/// Executes one job run: opstate suppression check (D5), then the job body,
/// then bookkeeping + events. Runs detached from the tick loop (D3); every
/// blocking git2 call goes through `spawn_blocking`.
#[allow(clippy::too_many_arguments)]
async fn execute_job(
    sched: SchedulerState,
    repo_id: String,
    path: PathBuf,
    job: JobKind,
    enabled: bool,
    base_ms: i64,
    now_ms: i64,
    emit: EmitFn,
) {
    let op_path = path.clone();
    let op = tauri::async_runtime::spawn_blocking(move || read_op_state(&op_path)).await;
    let result = match op {
        Err(e) => RunResult::Failed(format!("task join error: {e}")),
        Ok(Err(e)) => RunResult::Failed(e.to_string()),
        Ok(Ok(state)) if state != RepoOpState::None => RunResult::Suppressed,
        Ok(Ok(_)) => match job {
            // healthRefresh does NO git work — it is a pure refresh signal
            // (D8); the frontend's existing repo-changed refetch recomputes
            // status/health read-only.
            JobKind::HealthRefresh => RunResult::Success {
                updated_refs: None,
                emit_repo_changed: true,
            },
            JobKind::AutoFetch => {
                let fetch_path = path.clone();
                match tauri::async_runtime::spawn_blocking(move || fetch_all(&fetch_path)).await {
                    Err(e) => RunResult::Failed(format!("task join error: {e}")),
                    Ok(Err(e)) => RunResult::Failed(e.to_string()),
                    Ok(Ok(fr)) => {
                        let updated: u32 = fr
                            .remotes
                            .iter()
                            .map(|r| r.updated_refs)
                            .fold(0, u32::saturating_add);
                        // P52: refs advanced ⇒ (re)write the commit-graph off
                        // the tick path (fire-and-forget, best-effort, never
                        // awaited). Gated on `updated > 0` so the common no-op
                        // auto-fetch tick does not pay a pointless rewrite.
                        if updated > 0 {
                            let cg_path = path.clone();
                            tauri::async_runtime::spawn_blocking(move || {
                                let _ = bonsai_core::git::maintenance::write_commit_graph_best_effort(
                                    &cg_path,
                                );
                            });
                        }
                        RunResult::Success {
                            updated_refs: Some(updated),
                            emit_repo_changed: updated > 0,
                        }
                    }
                }
            }
        },
    };

    // Bookkeeping under the lock; events emitted after it is released.
    let (payload, repo_changed) = {
        // S1: recover from poisoning — bailing here would leak running=true.
        let mut jobs = lock_recover(&sched.jobs);
        let entry = jobs.entry((repo_id.clone(), job)).or_default();
        entry.running = false;
        entry.skip_signaled = false; // N1: next overlap may signal again
        entry.last_run_ms = Some(now_ms);

        let (outcome, updated_refs, error, entered_backoff, repo_changed) = match result {
            RunResult::Success {
                updated_refs,
                emit_repo_changed,
            } => {
                entry.consecutive_failures = 0; // D6 reset
                entry.last_error = None;
                (JobOutcome::Success, updated_refs, None, false, emit_repo_changed)
            }
            RunResult::Suppressed => (JobOutcome::Suppressed, None, None, false, false),
            RunResult::Failed(msg) => {
                entry.consecutive_failures = entry.consecutive_failures.saturating_add(1);
                entry.last_error = Some(msg.clone());
                (
                    JobOutcome::Failed,
                    None,
                    Some(msg),
                    entry.consecutive_failures == BACKOFF_THRESHOLD,
                    false,
                )
            }
        };
        entry.last_outcome = Some(outcome);

        let payload = JobStatusChangedPayload {
            repo_id: repo_id.clone(),
            job,
            outcome,
            updated_refs,
            error,
            consecutive_failures: entry.consecutive_failures,
            in_backoff: entry.consecutive_failures >= BACKOFF_THRESHOLD,
            entered_backoff,
            ts_ms: now_ms,
            next_run_ms: next_run_estimate_ms(
                enabled,
                base_ms,
                entry.last_run_ms,
                entry.consecutive_failures,
            ),
        };
        (payload, repo_changed)
    };

    if repo_changed {
        emit(SchedulerEvent::RepoChanged(RepoChangedPayload {
            repo_id: repo_id.clone(),
            reason: (if matches!(job, JobKind::AutoFetch) { "fetch" } else { "fs" }).to_string(),
        }));
    }
    emit(SchedulerEvent::JobStatus(payload));
}

/// Manual "run now" (D10): starts `job` immediately (ignoring backoff delay
/// and due-ness), or errors if it is already running. Suppression and
/// backoff-reset rules apply as for a scheduled run. Returns the join handle
/// so tests can await completion; the command drops it (fire-and-forget).
pub(crate) fn start_job_now(
    sched: &SchedulerState,
    repo_id: &str,
    path: PathBuf,
    job: JobKind,
    now_ms: i64,
    emit: EmitFn,
) -> Result<tauri::async_runtime::JoinHandle<()>, bonsai_core::error::AppError> {
    use bonsai_core::error::AppError;
    // S1: poison-recovering locks (see `lock_recover`) — run-now must keep
    // working even after some job future panicked.
    let (enabled, base_ms) = lock_recover(&sched.cfg).job_params(job);
    {
        let mut jobs = lock_recover(&sched.jobs);
        let entry = jobs.entry((repo_id.to_string(), job)).or_default();
        if entry.running {
            return Err(AppError::Other("job already running".to_string()));
        }
        entry.running = true;
        entry.skip_signaled = false;
    }
    Ok(tauri::async_runtime::spawn(execute_job(
        sched.clone(),
        repo_id.to_string(),
        path,
        job,
        enabled,
        base_ms,
        now_ms,
        emit,
    )))
}

/// The production loop (D2). Never returns; ends with the runtime on app
/// exit. `tick` is injectable so the integration tests can run a fast loop
/// (D14); `lib.rs` passes `Duration::from_secs(TICK_SECONDS)`.
pub async fn run_scheduler(app: tauri::AppHandle, tick: std::time::Duration) {
    let emit = emitter_for(app.clone());
    loop {
        tokio::time::sleep(tick).await;
        let repos: Vec<(String, PathBuf)> = {
            let state = app.state::<AppState>();
            // Poison recovery (audit §3.8) — matches `lock_recover`'s policy:
            // the repos map stays structurally valid, and skipping every tick
            // forever would silently kill auto-fetch/health-refresh.
            let guard = state
                .repos
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            guard
                .iter()
                .map(|(id, entry)| (id.clone(), entry.path.clone()))
                .collect()
        };
        let sched = app.state::<SchedulerState>();
        let _detached = tick_once(&repos, &sched, unix_now_ms(), &emit);
    }
}
