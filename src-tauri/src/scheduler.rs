//! Background-job scheduler (P30 contract).
//!
//! ONE global tick loop (D2) drives two strictly NON-DESTRUCTIVE jobs per open
//! repo: autoFetch (reuses `fetch_all` — remote-tracking refs only) and
//! healthRefresh (a pure `repo-changed` signal, no git work). Jobs are
//! suppressed while `read_op_state != None` (D5), never overlap their own
//! previous run (D4), back off exponentially after repeated failures (D6),
//! and NEVER prompt for credentials — the M6 credential chain in `fetch_all`
//! returns an error on exhaustion instead of prompting, which lands here as
//! an ordinary `Failed` outcome (D9).
//!
//! The planner (`plan` / `effective_interval_ms` / `next_run_estimate_ms`) is
//! pure over injected `now_ms` so the state machine is unit-testable without
//! real time (D14); the per-tick body (`tick_once`) takes a repo snapshot and
//! an injected emitter so integration tests can drive ticks without an
//! `AppHandle` (§9).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bonsai_core::git::opstate::{read_op_state, RepoOpState};
use bonsai_core::git::remote::fetch_all;
use tauri::Manager;

use crate::commands::RepoChangedPayload;
use crate::settings::{AutoFetch, HealthRefresh};
use crate::state::AppState;

/// Coarse tick period of the global loop (D2). Due-ness is evaluated per
/// (repo, job) each tick; the tick itself is cheap (map scan, no git work).
pub const TICK_SECONDS: u64 = 15;
/// Consecutive-failure count at which backoff starts (D6).
pub const BACKOFF_THRESHOLD: u32 = 3;
/// Backoff cap: effective interval never exceeds `8 * base` (D6).
pub const BACKOFF_MAX_FACTOR: i64 = 8;

/// The two background jobs (contract §3). Wire: camelCase variant names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobKind {
    AutoFetch,
    HealthRefresh,
}

/// Outcome of one job run. `Skipped` = the overlap guard fired (D4);
/// `Suppressed` = an operation (merge/rebase/…) was in progress (D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JobOutcome {
    Success,
    Failed,
    Suppressed,
    Skipped,
}

/// Per-(repo, job) runtime record. All timestamps unix ms.
#[derive(Debug, Clone, Default)]
pub struct JobRuntime {
    pub running: bool,
    pub last_run_ms: Option<i64>,
    pub last_outcome: Option<JobOutcome>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    /// P30a review N1: `true` once a `Skipped` event has been emitted for the
    /// CURRENT in-flight run — later ticks stay quiet until that run finishes
    /// (otherwise a slow fetch produces a Skipped event every 15 s).
    pub skip_signaled: bool,
}

/// Global scheduler config snapshot (mirrors the `Settings` fields, D7).
#[derive(Debug, Clone, Copy, Default)]
pub struct JobsConfig {
    pub auto_fetch: AutoFetch,
    pub health_refresh: HealthRefresh,
}

impl JobsConfig {
    /// (enabled, base interval in ms) for `job`. Intervals are stored in
    /// minutes in settings; the planner works in ms (D14).
    pub fn job_params(&self, job: JobKind) -> (bool, i64) {
        match job {
            JobKind::AutoFetch => (
                self.auto_fetch.enabled,
                i64::from(self.auto_fetch.interval_minutes) * 60_000,
            ),
            JobKind::HealthRefresh => (
                self.health_refresh.enabled,
                i64::from(self.health_refresh.interval_minutes) * 60_000,
            ),
        }
    }
}

/// Shared scheduler bookkeeping, `.manage()`d in `lib.rs`.
///
/// Deviation from the contract's literal field layout (flagged for review):
/// the mutexes live behind an inner `Arc` so detached job futures (D3) can
/// hold `'static` access without an `AppHandle` — command signatures are
/// unchanged (`tauri::State<'_, SchedulerState>` works verbatim via `Deref`).
#[derive(Default, Clone)]
pub struct SchedulerState(Arc<SchedulerInner>);

#[derive(Default)]
pub struct SchedulerInner {
    pub cfg: Mutex<JobsConfig>,
    /// Keyed by (repoId, JobKind); entries are pruned each tick when the repo
    /// no longer appears in `AppState.repos` (D2).
    pub jobs: Mutex<HashMap<(String, JobKind), JobRuntime>>,
}

impl std::ops::Deref for SchedulerState {
    type Target = SchedulerInner;
    fn deref(&self) -> &SchedulerInner {
        &self.0
    }
}

/// Locks a scheduler mutex, RECOVERING from poisoning (P30a review S1).
///
/// Rationale: the scheduler is an unattended background subsystem — if a job
/// future ever panicked while holding a lock, a `return`-on-poison here would
/// permanently wedge every later tick (and leak `running = true`, blocking
/// the job forever) with no user-visible signal. Our guarded data (config
/// copy + per-job bookkeeping) stays structurally valid at every await/panic
/// point (plain field writes, no multi-step invariants), so continuing with
/// the possibly-mid-update values is strictly safer than dying silently.
pub(crate) fn lock_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Replaces the config snapshot (called from `set_ui_settings` after persist
/// and once at startup, D7). Interval/enable changes take effect on the next
/// tick; runtime records (backoff, lastRun) are intentionally preserved.
pub fn apply_config(state: &SchedulerState, cfg: JobsConfig) {
    *lock_recover(&state.cfg) = cfg;
}

// ---------------------------------------------------------------------------
// Pure planner (no IO, no Tauri types, no real time — D14)
// ---------------------------------------------------------------------------

/// What the tick loop should do for one (repo, job) right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanDecision {
    Run,
    SkipOverlap,
    Wait { next_run_ms: i64 },
}

/// `base` for failures below [`BACKOFF_THRESHOLD`];
/// `base * 2^(failures - (BACKOFF_THRESHOLD - 1))` at/above it, capped at
/// [`BACKOFF_MAX_FACTOR`]` * base` (D6: 3 failures → 2×, 4 → 4×, ≥5 → 8×).
pub fn effective_interval_ms(base_ms: i64, consecutive_failures: u32) -> i64 {
    if consecutive_failures < BACKOFF_THRESHOLD {
        return base_ms;
    }
    let shift = consecutive_failures - (BACKOFF_THRESHOLD - 1);
    let factor = if shift >= 3 {
        BACKOFF_MAX_FACTOR
    } else {
        (1i64 << shift).min(BACKOFF_MAX_FACTOR)
    };
    base_ms.saturating_mul(factor)
}

/// Pure due-ness decision (contract §3). `last_run_ms == None` ⇒ first due a
/// full interval from `now` (D13 — the tick loop stamps `last_run_ms = now`
/// the first time an enabled job is seen, so an open never triggers an
/// instant fetch storm). Disabled ⇒ `Wait { i64::MAX }`.
pub fn plan(
    enabled: bool,
    base_interval_ms: i64,
    now_ms: i64,
    last_run_ms: Option<i64>,
    running: bool,
    consecutive_failures: u32,
) -> PlanDecision {
    if !enabled {
        return PlanDecision::Wait {
            next_run_ms: i64::MAX,
        };
    }
    let eff = effective_interval_ms(base_interval_ms, consecutive_failures);
    let last = match last_run_ms {
        Some(l) => l,
        None => {
            return PlanDecision::Wait {
                next_run_ms: now_ms.saturating_add(eff),
            }
        }
    };
    let next = last.saturating_add(eff);
    if now_ms < next {
        PlanDecision::Wait { next_run_ms: next }
    } else if running {
        PlanDecision::SkipOverlap
    } else {
        PlanDecision::Run
    }
}

/// Next-run estimate for the status surface. `None` when disabled or the job
/// has never been seen by the loop yet (no `last_run_ms` baseline).
pub fn next_run_estimate_ms(
    enabled: bool,
    base_interval_ms: i64,
    last_run_ms: Option<i64>,
    consecutive_failures: u32,
) -> Option<i64> {
    if !enabled {
        return None;
    }
    last_run_ms
        .map(|l| l.saturating_add(effective_interval_ms(base_interval_ms, consecutive_failures)))
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// Payload of the new `job-status-changed` event (contract §4).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatusChangedPayload {
    pub repo_id: String,
    pub job: JobKind,
    pub outcome: JobOutcome,
    /// autoFetch success only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_refs: Option<u32>,
    /// failed only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub consecutive_failures: u32,
    pub in_backoff: bool,
    /// `true` exactly on the 2→3 failure transition (D6) — the frontend
    /// toasts ONLY then.
    pub entered_backoff: bool,
    pub ts_ms: i64,
    pub next_run_ms: Option<i64>,
}

/// Events the scheduler emits; abstracted so tests can inject a collector
/// instead of an `AppHandle` (§9).
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    JobStatus(JobStatusChangedPayload),
    RepoChanged(RepoChangedPayload),
}

/// Injected event sink. The real one (see [`emitter_for`]) forwards to
/// `AppHandle::emit`; tests collect into a Vec.
pub type EmitFn = Arc<dyn Fn(SchedulerEvent) + Send + Sync>;

/// Builds the production emitter over `app.emit(..)`. Emission failures are
/// ignored (best-effort push signals).
pub fn emitter_for(app: tauri::AppHandle) -> EmitFn {
    use tauri::Emitter;
    Arc::new(move |ev| match ev {
        SchedulerEvent::JobStatus(p) => {
            let _ = app.emit("job-status-changed", p);
        }
        SchedulerEvent::RepoChanged(p) => {
            let _ = app.emit("repo-changed", p);
        }
    })
}

/// Current unix time in ms.
pub fn unix_now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tick + job execution
// ---------------------------------------------------------------------------

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
            reason: "fs".to_string(),
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
            let guard = match state.repos.lock() {
                Ok(g) => g,
                Err(_) => continue,
            };
            guard
                .iter()
                .map(|(id, entry)| (id.clone(), entry.path.clone()))
                .collect()
        };
        let sched = app.state::<SchedulerState>();
        let _detached = tick_once(&repos, &sched, unix_now_ms(), &emit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use std::sync::Mutex as StdMutex;

    const MIN: i64 = 60_000;

    // ---------------- planner unit tests (pure, fake time) ----------------

    /// Disabled jobs are never due, regardless of state (§9).
    #[test]
    fn plan_disabled_never_due() {
        assert_eq!(
            plan(false, 5 * MIN, 1_000_000_000, Some(0), false, 0),
            PlanDecision::Wait {
                next_run_ms: i64::MAX
            }
        );
        assert_eq!(
            plan(false, 5 * MIN, 0, None, false, 7),
            PlanDecision::Wait {
                next_run_ms: i64::MAX
            }
        );
    }

    /// First sight (`last_run == None`) waits one full interval (D13).
    #[test]
    fn plan_first_seen_waits_full_interval() {
        let now = 10 * MIN;
        assert_eq!(
            plan(true, 5 * MIN, now, None, false, 0),
            PlanDecision::Wait {
                next_run_ms: now + 5 * MIN
            }
        );
    }

    /// Not yet due → Wait with the exact next-run time; due → Run.
    #[test]
    fn plan_due_and_not_due() {
        let last = 100 * MIN;
        // One ms early: wait.
        assert_eq!(
            plan(true, 5 * MIN, last + 5 * MIN - 1, Some(last), false, 0),
            PlanDecision::Wait {
                next_run_ms: last + 5 * MIN
            }
        );
        // Exactly at the boundary: run.
        assert_eq!(
            plan(true, 5 * MIN, last + 5 * MIN, Some(last), false, 0),
            PlanDecision::Run
        );
        // Late: still run.
        assert_eq!(
            plan(true, 5 * MIN, last + 50 * MIN, Some(last), false, 0),
            PlanDecision::Run
        );
    }

    /// Due but still running → SkipOverlap (D4); running but NOT due → Wait.
    #[test]
    fn plan_overlap_guard() {
        let last = 0;
        assert_eq!(
            plan(true, 5 * MIN, 6 * MIN, Some(last), true, 0),
            PlanDecision::SkipOverlap
        );
        assert_eq!(
            plan(true, 5 * MIN, 2 * MIN, Some(last), true, 0),
            PlanDecision::Wait {
                next_run_ms: 5 * MIN
            }
        );
    }

    /// Backoff progression 1×/1×/1×/2×/4×/8×/8× and the D6 formula (base for
    /// failures 0–2; base*2^(f-2) for ≥3; capped 8×).
    #[test]
    fn effective_interval_backoff_progression() {
        let base = 5 * MIN;
        assert_eq!(effective_interval_ms(base, 0), base);
        assert_eq!(effective_interval_ms(base, 1), base);
        assert_eq!(effective_interval_ms(base, 2), base);
        assert_eq!(effective_interval_ms(base, 3), 2 * base);
        assert_eq!(effective_interval_ms(base, 4), 4 * base);
        assert_eq!(effective_interval_ms(base, 5), 8 * base);
        assert_eq!(effective_interval_ms(base, 6), 8 * base); // cap
        assert_eq!(effective_interval_ms(base, 100), 8 * base); // cap, no overflow
    }

    /// In backoff, the job is not due at base interval but IS due at the
    /// backed-off interval; a success resets to base (D6).
    #[test]
    fn plan_respects_backoff_and_reset() {
        let base = 5 * MIN;
        let last = 0;
        // 3 failures → 2× interval: not due at base + 1.
        assert_eq!(
            plan(true, base, base + 1, Some(last), false, 3),
            PlanDecision::Wait {
                next_run_ms: 2 * base
            }
        );
        assert_eq!(plan(true, base, 2 * base, Some(last), false, 3), PlanDecision::Run);
        // After a success (failures reset to 0) the base interval applies.
        assert_eq!(plan(true, base, base, Some(last), false, 0), PlanDecision::Run);
    }

    /// next_run_estimate: None when disabled or never seen; otherwise
    /// last + effective interval.
    #[test]
    fn next_run_estimate_semantics() {
        assert_eq!(next_run_estimate_ms(false, 5 * MIN, Some(0), 0), None);
        assert_eq!(next_run_estimate_ms(true, 5 * MIN, None, 0), None);
        assert_eq!(next_run_estimate_ms(true, 5 * MIN, Some(100), 0), Some(100 + 5 * MIN));
        assert_eq!(
            next_run_estimate_ms(true, 5 * MIN, Some(100), 4),
            Some(100 + 20 * MIN)
        );
    }

    // ---------------- tick/job integration (real repos, injected time) ----

    /// Scratch dir under `D:\Temp\bonsai-scratch` on Windows (MEMORY rule —
    /// never C:). On macOS/Linux there is no such constraint, so scratch
    /// dirs fall back to `std::env::temp_dir()/bonsai-scratch`.
    #[cfg(windows)]
    fn scratch_root() -> std::path::PathBuf {
        std::path::PathBuf::from("D:\\Temp\\bonsai-scratch")
    }

    #[cfg(not(windows))]
    fn scratch_root() -> std::path::PathBuf {
        std::env::temp_dir().join("bonsai-scratch")
    }

    fn scratch_dir() -> tempfile::TempDir {
        let root = scratch_root();
        std::fs::create_dir_all(&root).expect("create scratch root");
        tempfile::Builder::new()
            .prefix("bonsai-sched-")
            .tempdir_in(&root)
            .expect("scratch dir")
    }

    /// Builds a `file://` URL for a local path. On POSIX the path already
    /// starts with `/`, so `file://` + path gives the correct 3-slash form;
    /// prepending a bare `file:///` unconditionally (as Windows drive paths
    /// need) double-slashes it into `file:////...`, which libgit2 rejects as
    /// "not a valid local file URI" even though the real `git` CLI tolerates
    /// it — that mismatch masked as widespread scheduler test failures.
    fn file_url(path: &std::path::Path) -> String {
        let s = path.display().to_string().replace('\\', "/");
        if s.starts_with('/') {
            format!("file://{s}")
        } else {
            format!("file:///{s}")
        }
    }

    fn git(dir: &std::path::Path, args: &[&str]) {
        let out = Command::new("git")
            .current_dir(dir)
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .output()
            .expect("run git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_repo(dir: &std::path::Path) {
        git(dir, &["init", "-b", "main"]);
        git(dir, &["config", "user.name", "Test User"]);
        git(dir, &["config", "user.email", "test@example.com"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
    }

    fn commit_file(dir: &std::path::Path, rel: &str, contents: &str, msg: &str) {
        std::fs::write(dir.join(rel), contents).expect("write file");
        git(dir, &["add", "."]);
        git(dir, &["commit", "-m", msg]);
    }

    /// Collector emitter: appends every event to a shared Vec.
    fn collecting_emitter() -> (EmitFn, Arc<StdMutex<Vec<SchedulerEvent>>>) {
        let events: Arc<StdMutex<Vec<SchedulerEvent>>> = Arc::new(StdMutex::new(Vec::new()));
        let sink = events.clone();
        let emit: EmitFn = Arc::new(move |ev| {
            sink.lock().expect("events lock").push(ev);
        });
        (emit, events)
    }

    fn job_statuses(events: &Arc<StdMutex<Vec<SchedulerEvent>>>) -> Vec<JobStatusChangedPayload> {
        events
            .lock()
            .expect("events lock")
            .iter()
            .filter_map(|e| match e {
                SchedulerEvent::JobStatus(p) => Some(p.clone()),
                _ => None,
            })
            .collect()
    }

    fn repo_changed_count(events: &Arc<StdMutex<Vec<SchedulerEvent>>>) -> usize {
        events
            .lock()
            .expect("events lock")
            .iter()
            .filter(|e| matches!(e, SchedulerEvent::RepoChanged(_)))
            .count()
    }

    fn sched_with_auto_fetch(interval_minutes: u32) -> SchedulerState {
        let sched = SchedulerState::default();
        apply_config(
            &sched,
            JobsConfig {
                auto_fetch: AutoFetch {
                    enabled: true,
                    interval_minutes,
                },
                health_refresh: HealthRefresh {
                    enabled: false,
                    interval_minutes: 30,
                },
            },
        );
        sched
    }

    fn drive_tick(
        repos: &[(String, PathBuf)],
        sched: &SchedulerState,
        now_ms: i64,
        emit: &EmitFn,
    ) {
        let handles = tick_once(repos, sched, now_ms, emit);
        tauri::async_runtime::block_on(async {
            for h in handles {
                let _ = h.await;
            }
        });
    }

    /// Builds work repo + bare `file://` remote; pushes an initial commit and
    /// fetches so `refs/remotes/origin/main` exists. Returns (work, bare, other
    /// clone used to push new commits).
    fn fetch_fixture(root: &std::path::Path) -> (PathBuf, PathBuf, PathBuf) {
        let work = root.join("work");
        let bare = root.join("remote.git");
        let other = root.join("other");
        std::fs::create_dir_all(&work).expect("mkdir work");
        init_repo(&work);
        commit_file(&work, "a.txt", "one\n", "c1");
        git(root, &["init", "--bare", "-b", "main", "remote.git"]);
        let bare_url = file_url(&bare);
        git(&work, &["remote", "add", "origin", &bare_url]);
        git(&work, &["push", "-u", "origin", "main"]);
        git(root, &["clone", &bare_url, "other"]);
        git(&other, &["config", "user.name", "Test User"]);
        git(&other, &["config", "user.email", "test@example.com"]);
        git(&other, &["config", "commit.gpgsign", "false"]);
        (work, bare, other)
    }

    fn rev_parse(dir: &std::path::Path, rev: &str) -> String {
        let out = Command::new("git")
            .current_dir(dir)
            .args(["rev-parse", rev])
            .output()
            .expect("rev-parse");
        assert!(out.status.success(), "rev-parse {rev} failed");
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

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
}
