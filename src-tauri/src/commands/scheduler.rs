//! `scheduler` commands — split from the former monolithic `commands.rs`.

use super::shared::*;

/// One background job's status for the UI readout (P30 contract §3).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job: JobKind,
    pub enabled: bool,
    pub last_run_ms: Option<i64>,
    pub last_outcome: Option<JobOutcome>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub in_backoff: bool,
    /// Estimate; `None` when disabled (or never seen by the loop yet).
    pub next_run_ms: Option<i64>,
}

/// Background-job status for one open repo — exactly 2 entries (autoFetch,
/// healthRefresh). Errors: `noRepo` for an unknown repoId (P30 §3).
#[tauri::command]
pub async fn get_job_status(
    state: tauri::State<'_, AppState>,
    sched: tauri::State<'_, SchedulerState>,
    repo_id: String,
) -> Result<Vec<JobStatus>, AppError> {
    get_job_status_inner(state.inner(), &sched, &repo_id)
}

/// Runtime-free core of `get_job_status` (unit-testable without a Tauri app).
pub(crate) fn get_job_status_inner(
    state: &AppState,
    sched: &SchedulerState,
    repo_id: &str,
) -> Result<Vec<JobStatus>, AppError> {
    repo_path(state, repo_id)?; // NoRepo gate only
    // Recover from poison like the scheduler loop itself does (scheduler.rs
    // lock_recover rationale) — a single panicked job must not make this
    // command fail forever.
    let cfg = *crate::scheduler::lock_recover(&sched.cfg);
    let jobs = crate::scheduler::lock_recover(&sched.jobs);
    Ok([JobKind::AutoFetch, JobKind::HealthRefresh]
        .into_iter()
        .map(|job| {
            let (enabled, base_ms) = cfg.job_params(job);
            let rt = jobs
                .get(&(repo_id.to_string(), job))
                .cloned()
                .unwrap_or_default();
            JobStatus {
                job,
                enabled,
                last_run_ms: rt.last_run_ms,
                last_outcome: rt.last_outcome,
                last_error: rt.last_error,
                consecutive_failures: rt.consecutive_failures,
                in_backoff: rt.consecutive_failures >= scheduler::BACKOFF_THRESHOLD,
                next_run_ms: scheduler::next_run_estimate_ms(
                    enabled,
                    base_ms,
                    rt.last_run_ms,
                    rt.consecutive_failures,
                ),
            }
        })
        .collect())
}

/// Manual "run now" (P30 D10): fire-and-forget — `Ok(())` once the job is
/// started; the result arrives via `job-status-changed`. Ignores backoff
/// delay; suppression + backoff-reset rules apply as for a scheduled run.
/// Errors: `noRepo` | `Other("job already running")`.
#[tauri::command]
pub async fn run_job_now(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    sched: tauri::State<'_, SchedulerState>,
    repo_id: String,
    job: JobKind,
) -> Result<(), AppError> {
    let path = repo_path(state.inner(), &repo_id)?;
    scheduler::start_job_now(
        &sched,
        &repo_id,
        path,
        job,
        scheduler::unix_now_ms(),
        scheduler::emitter_for(app),
    )
    .map(|_handle| ()) // detached (fire-and-forget)
}
