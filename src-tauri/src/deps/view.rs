//! `deps_get_view`'s blocking body (contract §8, §6.5, §6.6): the report is
//! re-read from the cache per call; staleness is probed here too (the hook
//! refetches on window focus, which is the focus rescan for this surface).

use std::path::Path;
use std::sync::atomic::Ordering;

use bonsai_deps::discover::probe_stale;

use super::{interval_ms, lock, store, DepTrackerView, DepsState};

/// BLOCKING (cache read + git2). A disabled tracker returns the disabled view
/// without touching the repo or the cache (AC1).
pub fn view(state: &DepsState, repo_id: &str, workdir: &Path) -> DepTrackerView {
    if !state.is_enabled() {
        return DepTrackerView::disabled(repo_id.to_owned());
    }
    let epoch = state.epoch.load(Ordering::SeqCst);
    let key = store::key_for(workdir);
    let cached = store::load(state.data_root.as_deref(), &key, repo_id);
    let probed = cached.as_ref().map(|c| {
        git2::Repository::open(workdir)
            .map(|r| probe_stale(&r, &c.stamps))
            .unwrap_or(true)
    });
    let cfg = *lock(&state.cfg);
    let mut repos = lock(&state.repos);
    if !state.is_enabled() || state.epoch.load(Ordering::SeqCst) != epoch {
        // Disabled while we read: never serve a report from a wiped cache.
        return DepTrackerView::disabled(repo_id.to_owned());
    }
    let rd = repos.get_mut(repo_id);
    let (running, last_error, last_attempt, mut stale) = match &rd {
        Some(rd) => (
            rd.running.as_ref().map(|r| r.check),
            rd.last_error.clone(),
            rd.last_attempt_ms,
            rd.stale,
        ),
        None => (None, None, None, false),
    };
    if probed == Some(true) {
        stale = true;
        if let Some(rd) = rd {
            rd.stale = true;
        }
    }
    let report = cached.map(|c| c.report);
    DepTrackerView {
        repo_id: repo_id.to_owned(),
        enabled: true,
        manifests_changed: stale && report.is_some(),
        report,
        running,
        last_error,
        next_check_ms: last_attempt.map(|t| t.saturating_add(interval_ms(&cfg))),
    }
}
