//! P125 dependency tracker — the src-tauri side (contract
//! `docs/contracts/P125-dependency-tracker.md` §2.2, §6, §7.2, §8).
//!
//! `bonsai-deps` owns discovery, parsing, privacy, the network check and the
//! cache format. This module owns WHEN a check runs (the pure planner in
//! [`plan`], active tab only — Q4), the run lifecycle (`exec`: one
//! `spawn_blocking` per check, the async `CHECK_DEADLINE`, epoch-guarded
//! persist, disable wipe), the per-run [`gate::GatedTransport`] (AC1/AC12),
//! the real `HostEnv` ([`host_env`]) and the view DTO served by
//! `deps_get_view`.
//!
//! No process is ever spawned from here: `crates/bonsai-deps/tests/no_process.rs`
//! scans this directory and `commands/deps.rs` (§7.4).
//!
//! Lock order: `cfg` before `repos`, never the reverse; `active` is only
//! ever taken on its own.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use bonsai_deps::model::DepReport;
use bonsai_deps::registry_ctx::HostEnv;
use bonsai_deps::{CancelToken, Transport, CHECK_DEADLINE};

use crate::settings::DepTracker;

mod exec;
mod failure;
mod gate;
pub mod hooks;
pub mod host_env;
mod host_paths;
pub mod plan;
mod store;
mod view;

pub use exec::{apply_config, on_active_changed, on_fs_change, on_repo_closed, start_run, tick};
pub use view::view;

#[cfg(test)]
mod exec_cancel_tests;
#[cfg(test)]
mod exec_tests;
#[cfg(test)]
mod failure_tests;
#[cfg(test)]
mod host_env_tests;
#[cfg(test)]
mod host_paths_tests;
#[cfg(test)]
mod plan_tests;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod view_sentinel_tests;

/// §5.3 / §2.2 failure kinds (no `Cancelled`: a cancelled run is not a failure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CheckFailureKind {
    Offline,
    RateLimited,
    Service,
    Timeout,
    Internal,
}

/// `[MEDIUM-1]` `message` is rendered from `kind` + `http_status` by the fixed
/// table in `failure.rs`; it carries no free text.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckFailure {
    pub kind: CheckFailureKind,
    pub http_status: Option<u16>,
    pub message: String,
    pub at_ms: i64,
}

/// `[UI-b]` `done` / `total` count WORK UNITS (deps.dev lookups + OSV
/// querybatch requests), not packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunningCheck {
    pub check_id: u64,
    pub started_at_ms: i64,
    pub done: u32,
    pub total: u32,
}

/// The `deps_get_view` response (§2.2).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepTrackerView {
    pub repo_id: String,
    pub enabled: bool,
    /// Last completed report (from the cache), shown while running.
    pub report: Option<DepReport>,
    /// D5 stale.
    pub manifests_changed: bool,
    pub running: Option<RunningCheck>,
    /// Latest FAILED attempt; cleared on success.
    pub last_error: Option<CheckFailure>,
    pub next_check_ms: Option<i64>,
}

impl DepTrackerView {
    /// The disabled view: built without touching the repo or the cache (AC1).
    pub fn disabled(repo_id: String) -> Self {
        DepTrackerView {
            repo_id,
            enabled: false,
            report: None,
            manifests_changed: false,
            running: None,
            last_error: None,
            next_check_ms: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DepsChangedKind {
    Started,
    Progress,
    Finished,
    Failed,
    Cancelled,
    Stale,
    Cleared,
}

/// The `deps-changed` event payload (§2.2, §8): a small push signal.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepsChangedPayload {
    pub repo_id: String,
    pub check_id: Option<u64>,
    pub kind: DepsChangedKind,
    pub done: Option<u32>,
    pub total: Option<u32>,
}

impl DepsChangedPayload {
    pub(crate) fn of(repo_id: &str, check_id: Option<u64>, kind: DepsChangedKind) -> Self {
        DepsChangedPayload {
            repo_id: repo_id.to_owned(),
            check_id,
            kind,
            done: None,
            total: None,
        }
    }
}

/// The injected event sink: production forwards to
/// `obs::emit_logged("deps-changed")` (see [`hooks`]); tests collect.
pub type DepsEmitFn = Arc<dyn Fn(DepsChangedPayload) + Send + Sync>;

/// The in-flight run of one repo.
pub struct ActiveRun {
    pub check: RunningCheck,
    pub token: CancelToken,
    /// `DepsInner::epoch` when the run started; a later disable bumps it.
    pub epoch: u64,
}

/// Per-repo runtime bookkeeping (§6.1). Only `last_attempt_ms` of the run
/// history is memoized; the report is re-read from disk per view (§6.5).
#[derive(Default)]
pub struct RepoDeps {
    pub running: Option<ActiveRun>,
    pub last_attempt_ms: Option<i64>,
    pub last_error: Option<CheckFailure>,
    /// First activation in this process not yet checked (§6.3 "Opened").
    pub opened_unchecked: bool,
    pub stale: bool,
    pub last_probe_ms: i64,
}

/// Everything a `DepsState` is built from. `transport` is the RAW transport
/// (reqwest in production); every run wraps it in its own `GatedTransport`.
pub struct DepsDeps {
    pub cfg: DepTracker,
    pub transport: Arc<dyn Transport>,
    pub env: Arc<dyn HostEnv + Send + Sync>,
    /// `<app_data_dir>`; `None` when it cannot be resolved (checks then fail
    /// as `internal` instead of persisting).
    pub data_root: Option<PathBuf>,
    pub emit: DepsEmitFn,
}

/// Shared tracker state, `.manage()`d in `lib.rs` setup.
#[derive(Clone)]
pub struct DepsState(Arc<DepsInner>);

pub struct DepsInner {
    pub cfg: Mutex<DepTracker>,
    /// Mirror of `cfg.enabled` for the lock-free transport gate. Written only
    /// under the `repos` lock, together with `epoch`.
    pub enabled: AtomicBool,
    /// Bumped on every disable (§6.4): a run of an older epoch persists
    /// nothing and emits nothing but `Cancelled`.
    pub epoch: AtomicU64,
    pub repos: Mutex<HashMap<String, RepoDeps>>,
    /// The focused tab (repoId, workdir) as last reported by `set_active_repo`.
    pub active: Mutex<Option<(String, PathBuf)>>,
    pub transport: Arc<dyn Transport>,
    pub env: Arc<dyn HostEnv + Send + Sync>,
    pub data_root: Option<PathBuf>,
    pub emit: DepsEmitFn,
    /// `CHECK_DEADLINE` in production; shortened by the deadline test.
    pub deadline: Duration,
    pub next_check_id: AtomicU64,
}

impl std::ops::Deref for DepsState {
    type Target = DepsInner;
    fn deref(&self) -> &DepsInner {
        &self.0
    }
}

impl DepsState {
    pub fn new(d: DepsDeps) -> Self {
        Self::with_deadline(d, CHECK_DEADLINE)
    }

    pub fn with_deadline(d: DepsDeps, deadline: Duration) -> Self {
        DepsState(Arc::new(DepsInner {
            enabled: AtomicBool::new(d.cfg.enabled),
            cfg: Mutex::new(d.cfg),
            epoch: AtomicU64::new(0),
            repos: Mutex::new(HashMap::new()),
            active: Mutex::new(None),
            transport: d.transport,
            env: d.env,
            data_root: d.data_root,
            emit: d.emit,
            deadline,
            next_check_id: AtomicU64::new(1),
        }))
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    pub(crate) fn emit(&self, p: DepsChangedPayload) {
        (self.emit)(p);
    }
}

/// Poison-recovering lock (the scheduler's `lock_recover` policy): every
/// guarded value stays structurally valid at every panic point.
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Interval in ms for the planner.
pub(crate) fn interval_ms(cfg: &DepTracker) -> i64 {
    i64::from(cfg.interval_hours).saturating_mul(3_600_000)
}
