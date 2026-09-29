//! `run_check` — the blocking check pipeline (contract §5.1). It runs inside
//! ONE `spawn_blocking` in src-tauri (P125e); the 10-minute
//! [`CHECK_DEADLINE`] is enforced by that async caller, which cancels the
//! [`CancelToken`] on expiry `[HIGH-2]`. The worker polls the token before
//! every send and between parse steps.
//!
//! The check is atomic: a full [`CachedReport`] or a [`CheckError`], never a
//! partial report.

pub mod error;
mod groups;
mod pool;
mod rows;
mod verdict;
mod vulns;

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub use error::{CheckError, InternalFault, ServiceFault};

use crate::discover::{discover_full, stamps, workdir_canon, DiscoverError};
use crate::model::{summarize, CachedReport, DepReport, CACHE_SCHEMA, MAX_PARSE_BYTES};
use crate::registry_ctx::{HostEnv, RegistryContext};
use crate::remote::osv::OSV_BATCH_MAX;
use crate::remote::transport::Transport;

/// deps.dev lookups (and advisory GETs) in flight at once (§5.1 step 8).
pub const DEPSDEV_PARALLEL: usize = 6;
/// Enforced by the async caller (§6.4), not inside the worker.
pub const CHECK_DEADLINE: Duration = Duration::from_secs(10 * 60);
/// P125h §2 breaker: per-package oversize responses (deps.dev bodies or
/// version lists, advisory records) tolerated in one check. The one past
/// this aborts with `Service(TooLarge)` — a proxy inflating every body must
/// not produce a report of all-Unknown rows.
pub const MAX_DEGRADED_RESPONSES: u32 = 32;

/// Cooperative cancellation; clones share the flag.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

pub struct CheckCtx<'a> {
    pub transport: &'a dyn Transport,
    pub env: &'a dyn HostEnv,
    pub cancel: &'a CancelToken,
    /// `(done, total)` in `[UI-b]` work units: deps.dev lookups + OSV
    /// querybatch chunks. Called from worker threads.
    pub progress: &'a (dyn Fn(u32, u32) + Sync),
    /// The previous report: advisory DATA is reused when `id` and
    /// `modified` match (§5.1 step 10).
    pub prev: Option<&'a DepReport>,
    pub now_ms: i64,
}

impl CheckCtx<'_> {
    fn check_cancel(&self) -> Result<(), CheckError> {
        if self.cancel.is_cancelled() {
            Err(CheckError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Progress in `[UI-b]` units, shared by the worker threads.
pub(crate) struct Progress<'a> {
    done: AtomicU32,
    total: u32,
    cb: &'a (dyn Fn(u32, u32) + Sync),
}

impl Progress<'_> {
    fn tick(&self) {
        let d = self.done.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        (self.cb)(d.min(self.total), self.total);
    }
}

/// The per-check degraded-response counter behind
/// [`MAX_DEGRADED_RESPONSES`], shared by the worker threads.
#[derive(Default)]
pub(crate) struct Breaker(AtomicU32);

impl Breaker {
    /// Records one degraded response; the one past the limit is an error.
    fn degraded(&self) -> Result<(), CheckError> {
        let n = self.0.fetch_add(1, Ordering::SeqCst).saturating_add(1);
        if n > MAX_DEGRADED_RESPONSES {
            Err(CheckError::Service(ServiceFault::TooLarge))
        } else {
            Ok(())
        }
    }
}

fn discover_err(e: DiscoverError) -> CheckError {
    match e {
        DiscoverError::Bare => CheckError::Internal(InternalFault::GitOpen),
        DiscoverError::Workdir => CheckError::Internal(InternalFault::Io),
        DiscoverError::Index | DiscoverError::Status => {
            CheckError::Internal(InternalFault::GitIndex)
        }
    }
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// §5.1.
pub fn run_check(
    workdir: &Path,
    repo_id: &str,
    ctx: &CheckCtx<'_>,
) -> Result<CachedReport, CheckError> {
    ctx.check_cancel()?;
    // 1. discovery
    let repo = git2::Repository::open(workdir)
        .map_err(|_| CheckError::Internal(InternalFault::GitOpen))?;
    let files = discover_full(&repo).map_err(discover_err)?;
    let wd = workdir_canon(&repo).map_err(discover_err)?;
    ctx.check_cancel()?;
    // 2. registry context + local names
    let reg = RegistryContext::load(&wd, files.manifests(), ctx.env);
    let local = crate::adapter::local_names(&wd, &files);
    ctx.check_cancel()?;
    // 3. parse (catch_unwind per parse, MAX_PARSE_BYTES budget)
    let parsed = groups::parse_all(&wd, &files, MAX_PARSE_BYTES, ctx.cancel)?;
    // 4–5. classify + the two query sets
    let classified = rows::classify_all(parsed, &reg, &local);
    let latest_q = rows::latest_queries(&classified);
    let vuln_q = rows::vuln_queries(&classified);
    // 6–7. progress total in [UI-b] units
    let progress = Progress {
        done: AtomicU32::new(0),
        total: count(latest_q.len() + vuln_q.len().div_ceil(OSV_BATCH_MAX)),
        cb: ctx.progress,
    };
    (ctx.progress)(0, progress.total);
    let breaker = Breaker::default();
    // 8. deps.dev (6 in flight; an oversize body degrades only its package,
    //    any other non-404 failure aborts)
    let latest = vulns::fetch_all_versions(ctx, &latest_q, &progress, &breaker)?;
    // 9–10. OSV querybatch chunks, then advisory details (reused from prev)
    let advisories = vulns::fetch_advisories(ctx, &vuln_q, &progress, &breaker)?;
    ctx.check_cancel()?;
    // 11. assemble, sort, summarize
    let groups = rows::assemble(classified, &latest, &advisories);
    let report = DepReport {
        repo_id: repo_id.to_string(),
        checked_at_ms: ctx.now_ms,
        summary: summarize(&groups),
        groups,
        // P125h §4b: the exact flag (R3), text-only, no I/O.
        network_profile: ctx.env.profile_on_network(),
    };
    // 12. stamps
    Ok(CachedReport {
        schema: CACHE_SCHEMA,
        repo_id: repo_id.to_string(),
        stamps: stamps(&wd, &files),
        report,
    })
}

#[cfg(test)]
#[path = "check_tests.rs"]
mod tests;
