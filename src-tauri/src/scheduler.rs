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
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::commands::RepoChangedPayload;
use crate::settings::{AutoFetch, HealthRefresh};

mod exec;

pub use exec::run_scheduler;
pub(crate) use exec::start_job_now;
// `tick_once` is exercised only by the integration tests (the production loop
// in `exec` calls it directly); re-export it test-only.
#[cfg(test)]
pub(crate) use exec::tick_once;

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

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod tick_tests;
#[cfg(test)]
mod run_now_tests;
