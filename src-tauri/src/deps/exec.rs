//! Run / persist / deadline / disable (contract §6.3–§6.4, §6.6).
//!
//! - `start_run` records the run, emits `Started` and detaches ONE async
//!   task: the whole `run_check` in one `spawn_blocking`, raced against the
//!   deadline (`[HIGH-2]`: the deadline lives here, not in the worker).
//! - `finish` runs in `spawn_blocking` with the `repos` lock held: the epoch
//!   check, the cache save and the bookkeeping are one critical section, the
//!   same one the disable wipe uses, so no save can land after a wipe and no
//!   two saves of one repo interleave (§6.5, `[P125d review]`).
//! - Disable (`apply_config` true → false) bumps the epoch, cancels every
//!   run, closes every `GatedTransport` and wipes the cache directory; up to
//!   `DEPSDEV_PARALLEL` requests already in flight may complete, none start
//!   (`[LOW-1]`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};

use bonsai_core::error::AppError;
use bonsai_deps::check::{CheckError, InternalFault};
use bonsai_deps::discover::probe_stale;
use bonsai_deps::model::CachedReport;
use bonsai_deps::{run_check, CancelToken, CheckCtx};

use super::gate::GatedTransport;
use super::plan::{plan_dep_check, DepPlan};
use super::{
    failure, interval_ms, lock, store, ActiveRun, DepsChangedKind, DepsChangedPayload, DepsState,
    RepoDeps, RunningCheck,
};
use crate::scheduler::unix_now_ms;
use crate::settings::DepTracker;

/// Verbatim `AppError::Other` text (mirrored by the mock's `DEPS_DISABLED_MESSAGE`).
pub(crate) const DISABLED_MESSAGE: &str = "dependency tracker is disabled";
/// Progress events per run: at most one per 250 ms (§8: ≤ 4/s).
const PROGRESS_MIN_GAP_MS: i64 = 250;
/// `on_fs_change` probes at most once per 2 s per repo (§6.6).
const PROBE_MIN_GAP_MS: i64 = 2_000;

fn opened() -> RepoDeps {
    RepoDeps {
        opened_unchecked: true,
        ..RepoDeps::default()
    }
}

/// A started (or already running) check: its id, plus the detached task
/// (`None` when the repo was already running). Tests await the task;
/// production drops it.
pub(crate) type Started = (u64, Option<tauri::async_runtime::JoinHandle<()>>);

/// §6.4 `start_run`: resolves the running check's id, or starts one.
pub fn start_run(state: &DepsState, repo_id: &str, workdir: PathBuf) -> Result<u64, AppError> {
    start_run_inner(state, repo_id, workdir).map(|(id, _task)| id)
}

pub(crate) fn start_run_inner(
    state: &DepsState,
    repo_id: &str,
    workdir: PathBuf,
) -> Result<Started, AppError> {
    let (check_id, epoch, token) = {
        let mut repos = lock(&state.repos);
        let rd = repos.entry(repo_id.to_owned()).or_insert_with(opened);
        // [review] a run whose token is already cancelled (tab switch,
        // deadline) is draining, not running: never join it — start anew.
        // Its `finish` then sees a different check id and writes nothing.
        if let Some(r) = rd.running.as_ref().filter(|r| !r.token.is_cancelled()) {
            return Ok((r.check.check_id, None));
        }
        // Read under the `repos` lock: a disable flips it under the same lock.
        if !state.is_enabled() {
            return Err(AppError::Other(DISABLED_MESSAGE.to_owned()));
        }
        let check_id = state.next_check_id.fetch_add(1, Ordering::SeqCst);
        let epoch = state.epoch.load(Ordering::SeqCst);
        let token = CancelToken::new();
        rd.running = Some(ActiveRun {
            check: RunningCheck {
                check_id,
                started_at_ms: unix_now_ms(),
                done: 0,
                total: 0,
            },
            token: token.clone(),
            epoch,
        });
        rd.opened_unchecked = false;
        (check_id, epoch, token)
    };
    state.emit(DepsChangedPayload::of(
        repo_id,
        Some(check_id),
        DepsChangedKind::Started,
    ));
    let run = Run {
        state: state.clone(),
        repo_id: repo_id.to_owned(),
        workdir,
        check_id,
        epoch,
        token,
    };
    Ok((check_id, Some(tauri::async_runtime::spawn(run.drive()))))
}

/// One started check.
#[derive(Clone)]
struct Run {
    state: DepsState,
    repo_id: String,
    workdir: PathBuf,
    check_id: u64,
    epoch: u64,
    token: CancelToken,
}

impl Run {
    async fn drive(self) {
        let worker = self.clone();
        let task = tauri::async_runtime::spawn_blocking(move || worker.check());
        let res = match tokio::time::timeout(self.state.deadline, task).await {
            // [HIGH-2] the worker stops at its next cancel check; whatever it
            // returns later is dropped with the orphaned join handle.
            // A token cancelled BEFORE the deadline (tab switch / disable)
            // makes this a cancellation, not a timeout.
            Err(_elapsed) if self.token.is_cancelled() => Err(CheckError::Cancelled),
            Err(_elapsed) => {
                self.token.cancel();
                Err(CheckError::Timeout)
            }
            // A panicking worker (the crate already catches parse panics).
            Ok(Err(_join)) => Err(CheckError::Internal(InternalFault::Io)),
            Ok(Ok(r)) => r,
        };
        let _ = tauri::async_runtime::spawn_blocking(move || self.finish(res)).await;
    }

    /// Whether this run may still publish anything (`[LOW-1]`).
    fn live(&self) -> bool {
        self.state.epoch.load(Ordering::SeqCst) == self.epoch && !self.token.is_cancelled()
    }

    /// Blocking: the whole §5.1 pipeline.
    fn check(&self) -> Result<(String, CachedReport), CheckError> {
        let key = store::key_for(&self.workdir);
        let root = self.state.data_root.as_deref();
        let prev = store::load(root, &key, &self.repo_id);
        let gated = GatedTransport::new(&self.state, self.epoch, self.token.clone());
        let last_emit = AtomicI64::new(i64::MIN / 2);
        let progress = |done: u32, total: u32| self.progress(&last_emit, done, total);
        let ctx = CheckCtx {
            transport: &gated,
            env: &*self.state.env,
            cancel: &self.token,
            progress: &progress,
            prev: prev.as_ref().map(|c| &c.report),
            now_ms: unix_now_ms(),
        };
        run_check(&self.workdir, &self.repo_id, &ctx).map(|c| (key, c))
    }

    /// Updates the view's counters and emits `Progress`, throttled; nothing
    /// at all once the run is cancelled or its epoch is stale.
    fn progress(&self, last_emit: &AtomicI64, done: u32, total: u32) {
        if !self.live() {
            return;
        }
        {
            let mut repos = lock(&self.state.repos);
            let Some(run) = repos
                .get_mut(&self.repo_id)
                .and_then(|rd| rd.running.as_mut())
                .filter(|r| r.check.check_id == self.check_id)
            else {
                return;
            };
            run.check.done = done;
            run.check.total = total;
        }
        let now = unix_now_ms();
        let prev = last_emit.load(Ordering::SeqCst);
        // The last unit always reports, so the bar can reach its end.
        let last = done == total;
        if !last && now.saturating_sub(prev) < PROGRESS_MIN_GAP_MS {
            return;
        }
        if last_emit
            .compare_exchange(prev, now, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
            && !last
        {
            return;
        }
        self.state.emit(DepsChangedPayload {
            done: Some(done),
            total: Some(total),
            ..DepsChangedPayload::of(
                &self.repo_id,
                Some(self.check_id),
                DepsChangedKind::Progress,
            )
        });
    }

    /// Blocking; the §6.4 "lock repos" tail.
    fn finish(&self, res: Result<(String, CachedReport), CheckError>) {
        let now = unix_now_ms();
        let kind = {
            let mut repos = lock(&self.state.repos);
            let stale_epoch = self.state.epoch.load(Ordering::SeqCst) != self.epoch;
            let timed_out = matches!(res, Err(CheckError::Timeout));
            let cancelled = (self.token.is_cancelled() && !timed_out)
                || matches!(res, Err(CheckError::Cancelled));
            let rd = repos
                .get_mut(&self.repo_id)
                .filter(|rd| rd.running.as_ref().map(|r| r.check.check_id) == Some(self.check_id));
            match rd {
                // Closed / disabled / superseded: write nothing.
                None => DepsChangedKind::Cancelled,
                Some(rd) if stale_epoch || cancelled => {
                    rd.running = None;
                    DepsChangedKind::Cancelled
                }
                Some(rd) => {
                    let kind = match res {
                        Ok((key, c)) => {
                            match store::save(self.state.data_root.as_deref(), &key, &c) {
                                Ok(()) => {
                                    rd.last_error = None;
                                    rd.stale = false;
                                    DepsChangedKind::Finished
                                }
                                Err(_) => {
                                    rd.last_error = Some(failure::internal(now));
                                    DepsChangedKind::Failed
                                }
                            }
                        }
                        Err(e) => {
                            rd.last_error = failure::render(e, now);
                            DepsChangedKind::Failed
                        }
                    };
                    rd.last_attempt_ms = Some(now);
                    rd.running = None;
                    kind
                }
            }
        };
        self.state.emit(DepsChangedPayload::of(
            &self.repo_id,
            Some(self.check_id),
            kind,
        ));
    }
}

/// §6.4 `apply_config`. BLOCKING (the disable path removes the cache
/// directory) — call it from `spawn_blocking`. `open_repos` is every open
/// tab: each gets its own `Cleared` (the hook drops non-matching payloads).
pub fn apply_config(state: &DepsState, new: DepTracker, open_repos: &[String]) {
    let old = std::mem::replace(&mut *lock(&state.cfg), new);
    if old.enabled && !new.enabled {
        let mut ids: Vec<String> = {
            let mut repos = lock(&state.repos);
            state.enabled.store(false, Ordering::SeqCst);
            state.epoch.fetch_add(1, Ordering::SeqCst);
            for run in repos.values().filter_map(|rd| rd.running.as_ref()) {
                run.token.cancel();
            }
            let ids = repos.keys().cloned().collect();
            repos.clear();
            // Under the lock: no save can land after this.
            store::wipe(state.data_root.as_deref());
            ids
        };
        ids.extend(open_repos.iter().cloned());
        ids.sort();
        ids.dedup();
        for id in ids {
            state.emit(DepsChangedPayload::of(&id, None, DepsChangedKind::Cleared));
        }
    } else if !old.enabled && new.enabled {
        {
            let _repos = lock(&state.repos);
            state.enabled.store(true, Ordering::SeqCst);
        }
        let active = lock(&state.active).clone();
        if let Some((id, workdir)) = active {
            // A failure here is only "disabled again meanwhile".
            let _ = start_run(state, &id, workdir);
        }
    }
}

/// §6.1 `tick`: plans the ACTIVE repo only (Q4). Cheap: no IO.
pub fn tick(state: &DepsState, active: Option<(String, PathBuf)>, now_ms: i64) {
    // `active` comes from `AppState` each tick: it also heals the mirror when a
    // `set_active_repo` arrived before its `open_repo` had registered the path.
    let known = lock(&state.active).as_ref().map(|(id, _)| id.clone());
    if active.as_ref().map(|(id, _)| id) != known.as_ref() {
        *lock(&state.active) = active.clone();
    }
    let cfg = *lock(&state.cfg);
    let Some((id, workdir)) = active else {
        return;
    };
    if !cfg.enabled {
        return;
    }
    let plan = {
        let mut repos = lock(&state.repos);
        let rd = repos.entry(id.clone()).or_insert_with(opened);
        plan_dep_check(
            state.is_enabled(),
            true,
            rd.running.as_ref().is_some_and(|r| !r.token.is_cancelled()),
            rd.opened_unchecked,
            interval_ms(&cfg),
            now_ms,
            rd.last_attempt_ms,
        )
    };
    if let DepPlan::Run(_) = plan {
        let _ = start_run(state, &id, workdir);
    }
}

/// §6.3: the focused tab changed. Cancels the old tab's run, records the
/// new one, marks a first activation "opened" and ticks it at once.
pub fn on_active_changed(state: &DepsState, old: Option<&str>, new: Option<(&str, PathBuf)>) {
    let new_id = new.as_ref().map(|(id, _)| *id);
    if let Some(old) = old.filter(|o| Some(*o) != new_id) {
        if let Some(run) = lock(&state.repos)
            .get(old)
            .and_then(|rd| rd.running.as_ref())
        {
            run.token.cancel();
        }
    }
    let new = new.map(|(id, p)| (id.to_owned(), p));
    *lock(&state.active) = new.clone();
    if let Some((id, _)) = &new {
        lock(&state.repos).entry(id.clone()).or_insert_with(opened);
    }
    tick(state, new, unix_now_ms());
}

/// §6.3: a tab closed. Its run is cancelled and its bookkeeping dropped, so a
/// reopen counts as a fresh "opened".
pub fn on_repo_closed(state: &DepsState, repo_id: &str) {
    if let Some(rd) = lock(&state.repos).remove(repo_id) {
        if let Some(run) = rd.running {
            run.token.cancel();
        }
    }
    let mut active = lock(&state.active);
    if active.as_ref().is_some_and(|(id, _)| id == repo_id) {
        *active = None;
    }
}

/// §6.6 (D5, AC9): a debounced watcher burst. Probes staleness at most every
/// 2 s, only for the active, enabled, cached, not-yet-stale repo, and emits
/// `Stale` on false → true. Never starts a check.
pub fn on_fs_change(state: &DepsState, repo_id: &str, workdir: PathBuf) {
    if !state.is_enabled() {
        return;
    }
    if !lock(&state.active)
        .as_ref()
        .is_some_and(|(id, _)| id == repo_id)
    {
        return;
    }
    let now = unix_now_ms();
    {
        let mut repos = lock(&state.repos);
        let Some(rd) = repos.get_mut(repo_id) else {
            return;
        };
        if rd.stale || now.saturating_sub(rd.last_probe_ms) < PROBE_MIN_GAP_MS {
            return;
        }
        rd.last_probe_ms = now;
    }
    let epoch = state.epoch.load(Ordering::SeqCst);
    let state = state.clone();
    let repo_id = repo_id.to_owned();
    tauri::async_runtime::spawn_blocking(move || probe(&state, &repo_id, &workdir, epoch));
}

/// Blocking half of `on_fs_change`.
pub(crate) fn probe(state: &DepsState, repo_id: &str, workdir: &std::path::Path, epoch: u64) {
    let key = store::key_for(workdir);
    let Some(cached) = store::load(state.data_root.as_deref(), &key, repo_id) else {
        return;
    };
    let stale = git2::Repository::open(workdir)
        .map(|r| probe_stale(&r, &cached.stamps))
        .unwrap_or(true);
    if !stale {
        return;
    }
    let flipped = {
        let mut repos = lock(&state.repos);
        let current = state.is_enabled() && state.epoch.load(Ordering::SeqCst) == epoch;
        match repos.get_mut(repo_id) {
            Some(rd) if current && !rd.stale => {
                rd.stale = true;
                true
            }
            _ => false,
        }
    };
    if flipped {
        state.emit(DepsChangedPayload::of(
            repo_id,
            None,
            DepsChangedKind::Stale,
        ));
    }
}
