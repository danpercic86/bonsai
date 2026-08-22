//! P87 activity-recording fetch/pull cores (split from `remote.rs` for the
//! ~500-line limit; the push/force siblings live in `remote_push_activity.rs`).
//!
//! `fetch_all_with_activity` / `pull_ff_with_activity` are [`super::remote`]'s
//! `fetch_all` / `pull_ff` PLUS an optional [`GitActivityRecorder`]: they emit
//! `phase(Network)` and throttled structured transfer `Progress` (§14). `activity
//! == None` is the **byte-for-byte** pre-P87 path. The plain names in `remote.rs`
//! are thin `None` wrappers.

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::time::{Duration, Instant};

use crate::error::AppError;
use crate::git::activity::{GitActivityRecorder, GitPhaseKind, GitTransferProgress};
use crate::git::cred::{acquire_cred, evict_fresh_on_auth_fail, map_remote_err, CredAttempts};
use crate::git::remote::{open_repo_at, to_u32, FetchResult, PullResult, RemoteFetchResult};
use crate::git::repo::read_head_info;

/// P87 §14.3: wall-clock coalescing throttle for fetch/pull `transfer_progress`
/// so the activity channel stays ≤~20 events/sec regardless of object count.
pub const PROGRESS_MIN_INTERVAL: Duration = Duration::from_millis(50);

/// P87 §14.3 throttle decision (PURE, so the coalescing is unit-testable without
/// a network fetch): fire at most once per [`PROGRESS_MIN_INTERVAL`], but ALWAYS
/// fire the FIRST terminal (`received == total`) tick so the bar reaches 100%
/// even if the last callback lands inside the window.
fn progress_should_fire(last: Option<Instant>, now: Instant, done: bool, done_emitted: bool) -> bool {
    last.is_none_or(|t| now.duration_since(t) >= PROGRESS_MIN_INTERVAL) || (done && !done_emitted)
}

/// P87 §14.1: map a `git2::Progress` snapshot onto the wire struct.
/// `total_deltas`/`indexed_deltas` are `Some` only during delta-resolution
/// (`total_deltas() > 0`).
fn to_transfer_progress(p: &git2::Progress) -> GitTransferProgress {
    let total_deltas = p.total_deltas();
    GitTransferProgress {
        received_objects: to_u32(p.received_objects()),
        total_objects: to_u32(p.total_objects()),
        indexed_objects: to_u32(p.indexed_objects()),
        received_bytes: u64::try_from(p.received_bytes()).unwrap_or(u64::MAX),
        total_deltas: (total_deltas > 0).then(|| to_u32(total_deltas)),
        indexed_deltas: (total_deltas > 0).then(|| to_u32(p.indexed_deltas())),
    }
}

/// Fetches ONE remote by name: default refspecs, `AutotagOption::Auto`, no
/// prune, fresh `CredAttempts`. Fetch errors mapped via `map_remote_err`. Wires a
/// throttled `transfer_progress` callback ONLY when `activity` is `Some` (§14.3).
fn fetch_remote(
    repo: &git2::Repository,
    name: &str,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<RemoteFetchResult, AppError> {
    let mut remote = match repo.find_remote(name) {
        Ok(r) => r,
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::NoRemote(format!("remote '{name}' not found")));
        }
        Err(e) => return Err(e.into()),
    };

    let attempts = RefCell::new(CredAttempts::default());
    let updated = Cell::new(0u32);

    let received = {
        let mut callbacks = git2::RemoteCallbacks::new();
        callbacks.credentials(|url, username_from_url, allowed| {
            acquire_cred(repo.workdir(), &attempts, url, username_from_url, allowed)
        });
        callbacks.update_tips(|_refname, old, new| {
            if old != new {
                updated.set(updated.get().saturating_add(1));
            }
            true
        });
        // P87 §14.3: throttled structured transfer progress, only when a recorder
        // is present (additive — no callback otherwise). git2 calls this
        // SYNCHRONOUSLY on the fetch thread, so the FnMut captures need no lock.
        if let Some(rec) = activity {
            let mut last: Option<Instant> = None;
            let mut done_emitted = false;
            callbacks.transfer_progress(move |p: git2::Progress| {
                let now = Instant::now();
                let done = p.total_objects() > 0 && p.received_objects() == p.total_objects();
                if progress_should_fire(last, now, done, done_emitted) {
                    rec.progress(to_transfer_progress(&p));
                    last = Some(now);
                    done_emitted |= done;
                }
                true // never abort the transfer
            });
        }

        let mut opts = git2::FetchOptions::new();
        opts.remote_callbacks(callbacks);
        opts.download_tags(git2::AutotagOption::Auto);

        if let Err(e) = remote.fetch(&[] as &[&str], Some(&mut opts), None) {
            return Err(evict_fresh_on_auth_fail(
                repo,
                &attempts,
                map_remote_err(e, name),
            ));
        }
        to_u32(remote.stats().received_objects())
    };

    Ok(RemoteFetchResult {
        remote: name.to_string(),
        received_objects: received,
        updated_refs: updated.get(),
    })
}

/// P87: [`super::remote::fetch_all`] plus an optional recorder (category `Fetch`).
/// Emits `phase(Network)` at entry and structured `Progress` events per remote.
pub fn fetch_all_with_activity(
    workdir: &Path,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<FetchResult, AppError> {
    let repo = open_repo_at(workdir)?;

    let names: Vec<String> = repo
        .remotes()?
        .iter()
        .map(|n| n.ok().flatten())
        .filter_map(|n| match n {
            Some(n) => Some(n.to_string()),
            None => {
                eprintln!("bonsai: skipping remote with non-UTF-8 name");
                None
            }
        })
        .collect();
    if names.is_empty() {
        return Err(AppError::NoRemote("no remotes configured".to_string()));
    }

    if let Some(a) = activity {
        a.phase(GitPhaseKind::Network, None);
    }
    let mut remotes = Vec::with_capacity(names.len());
    for name in &names {
        remotes.push(fetch_remote(&repo, name, activity)?);
    }
    Ok(FetchResult {
        remotes,
        tag_auto_sync: None,
    })
}

/// P87: [`super::remote::pull_ff`] plus an optional recorder (category `Pull`).
/// Emits `phase(Network)` before the fetch + structured `Progress` events.
pub fn pull_ff_with_activity(
    workdir: &Path,
    activity: Option<&dyn GitActivityRecorder>,
) -> Result<PullResult, AppError> {
    let repo = open_repo_at(workdir)?;

    let head = read_head_info(&repo)?;
    if head.unborn {
        return Err(AppError::Git(
            "cannot pull: the repository has no commits yet".to_string(),
        ));
    }
    if head.detached {
        return Err(AppError::Git("cannot pull: HEAD is detached".to_string()));
    }
    let name = head
        .branch_name
        .ok_or_else(|| AppError::Git("cannot pull: HEAD has no branch name".to_string()))?;
    let refname = format!("refs/heads/{name}");

    // Upstream must be configured before we bother fetching.
    let branch = repo.find_branch(&name, git2::BranchType::Local)?;
    match branch.upstream() {
        Ok(_) => {}
        Err(e) if e.code() == git2::ErrorCode::NotFound => {
            return Err(AppError::NoUpstream(format!(
                "cannot pull: branch '{name}' has no upstream configured"
            )));
        }
        Err(e) => return Err(e.into()),
    }

    let remote_buf = repo.branch_upstream_remote(&refname)?;
    let remote_name = remote_buf
        .as_str()
        .ok()
        .ok_or_else(|| AppError::Git("upstream remote name is not valid UTF-8".to_string()))?
        .to_string();

    // Fetch ONLY the upstream's remote (the Fetch button covers the rest).
    if let Some(a) = activity {
        a.phase(GitPhaseKind::Network, None);
    }
    fetch_remote(&repo, &remote_name, activity)?;

    // RE-RESOLVE the upstream after the fetch — it may have moved.
    let branch = repo.find_branch(&name, git2::BranchType::Local)?;
    let upstream = branch.upstream()?;
    let upstream_oid = upstream.get().target().ok_or_else(|| {
        AppError::Git(format!("upstream of '{name}' has no target commit"))
    })?;
    // Resolved tracking shorthand ("origin/main") from the ALREADY-resolved
    // upstream branch (post-fetch) — the exact name the frontend passes back to
    // merge_branch/rebase_branch on a non-FF result. NOT recomputed from config;
    // falls back to "{remote}/{local}" only if the ref name is unreadable.
    let upstream_shorthand = upstream
        .name()
        .ok()
        .flatten()
        .map(str::to_string)
        .unwrap_or_else(|| format!("{remote_name}/{name}"));
    let local_oid = branch
        .get()
        .target()
        .ok_or_else(|| AppError::Git(format!("branch '{name}' has no target commit")))?;

    let (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
    if behind == 0 {
        // Equal or ahead-only: nothing to pull.
        return Ok(PullResult::UpToDate);
    }
    if ahead > 0 {
        // Diverged: change NOTHING (the fetch already landed — that's fine).
        return Ok(PullResult::WouldNotFastForward {
            branch: name,
            ahead: to_u32(ahead),
            behind: to_u32(behind),
            upstream: upstream_shorthand,
        });
    }

    // Fast-forward (ahead == 0 && behind > 0).
    let obj = repo.find_object(upstream_oid, None)?;
    let mut opts = git2::build::CheckoutBuilder::new();
    opts.safe(); // DEFAULT SAFE MODE — never .force()
    match repo.checkout_tree(&obj, Some(&mut opts)) {
        Ok(()) => {}
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            return Err(AppError::CheckoutConflict(
                "cannot pull: local changes would be overwritten by the update. \
                 Commit or discard them first."
                    .to_string(),
            ));
        }
        Err(e) => return Err(e.into()),
    }
    repo.find_reference(&refname)?
        .set_target(upstream_oid, &format!("pull: fast-forward to {upstream_oid}"))?;

    Ok(PullResult::FastForwarded {
        branch: name,
        from: local_oid.to_string(),
        to: upstream_oid.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The throttle coalesces to ≤1 event per `PROGRESS_MIN_INTERVAL` but ALWAYS
    /// lets the first terminal (`received == total`) tick through so the bar hits
    /// 100%.
    #[test]
    fn progress_throttle_coalesces_and_forces_terminal_tick() {
        let base = Instant::now();
        // First tick (no prior) always fires.
        assert!(progress_should_fire(None, base, false, false));
        // Within the window, a non-terminal tick is coalesced away.
        let within = base + Duration::from_millis(10);
        assert!(!progress_should_fire(Some(base), within, false, false));
        // Past the window, a non-terminal tick fires again.
        let past = base + PROGRESS_MIN_INTERVAL + Duration::from_millis(1);
        assert!(progress_should_fire(Some(base), past, false, false));
        // A terminal tick inside the window STILL fires (forced), once.
        assert!(progress_should_fire(Some(base), within, true, false));
        // ...but not a second time once the terminal tick was already emitted.
        assert!(!progress_should_fire(Some(base), within, true, true));
    }
}
