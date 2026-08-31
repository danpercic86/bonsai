//! P88b/B2b — thread-local `git2::Repository` handle cache for the refresh
//! round's READ commands.
//!
//! Before this, every command re-opened the repo from its path: a full refresh
//! round fired ~9–11 `open_ext` calls (one per `invoke`). `git2::Repository` is
//! `Send` but NOT `Sync`, and a round fans out ~11 CONCURRENT `spawn_blocking`
//! tasks (`Promise.all`), so a single shared `Mutex<Repository>` would serialize
//! the fan-out (the slow `status` scan and a graph Miss back-to-back). A
//! thread-local cache keyed `(repoId, generation)` keeps one open handle per
//! blocking-pool thread: nothing is shared across threads (no `Sync` needed, no
//! mutex serializes the round). Bound: ≤ (pool threads × open repos) handles;
//! evicted on a generation bump.
//!
//! WHAT ACTUALLY REUSES (be precise — the win is uneven):
//! - **The list trio** (`list_branches`/`list_stashes`/`list_worktrees`) calls
//!   `with_repo*` DIRECTLY inside `spawn_blocking`, on the tokio blocking-pool
//!   thread whose thread-local PERSISTS across tasks and rounds ⇒ they get the
//!   real CROSS-ROUND reuse (first call on a thread opens, later calls reuse).
//! - **`get_status` and `stream_graph`** run their `with_repo*` call inside
//!   `run_with_git_timeout` (`bonsai_core::git::timeout`), which spawns a FRESH
//!   watchdog OS thread per call. That thread's `HANDLES` starts EMPTY every
//!   call, so these open ONCE PER CALL — NO cross-round reuse. `stream_graph`
//!   still fuses its seed probe + walk + store re-probe to a SINGLE open WITHIN
//!   one call (they share the one `with_repo_mut` handle; was two opens), which
//!   is the graph's real B2b win; `get_status` was already a single open, so its
//!   only B2b change is the index-freshness guard (`read_status_with`), correct
//!   should the handle ever be reused (it isn't, today). Re-fusing status/graph
//!   across rounds would require hoisting `with_repo` OUTSIDE the timeout seam —
//!   a deliberate separate follow-up, not done here.
//!
//! FRESHNESS (the load-bearing correctness point): a reused handle re-reads refs
//! (`references()` re-stats loose refs + reloads `packed-refs`), the odb, and
//! every `Revwalk` ON DEMAND, so `graph_seed` / `list_refs` observe current
//! on-disk topology. The one CACHED datum is the `Index` (`repo.index()` returns
//! a cached handle) — `read_status_with` forces it fresh via `index.read(true)`.
//! Config-reading reads (`list_remotes`) are deliberately LEFT UN-ROUTED (they
//! keep their own fresh open). Generation bumps on `open_repo`/`close_repo`
//! evict every thread's stale handle for the id on its next call.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use bonsai_core::error::AppError;
use bonsai_core::git::timeout::{effective_deadline, run_with_git_timeout_owned_with, GitProgress};

use crate::perf::PerfState;

thread_local! {
    /// One open `Repository` per `(repoId, generation)` on THIS blocking-pool
    /// thread. Never shared across threads (thread-local) ⇒ no `Sync` bound and
    /// no cross-thread contention. Stale-generation entries are evicted lazily
    /// on the next [`with_repo_mut`] for the id.
    static HANDLES: RefCell<HashMap<(String, u64), git2::Repository>> =
        RefCell::new(HashMap::new());
}

/// Opens the repo exactly as every READ command does: `NO_SEARCH` (no walking up
/// to a parent repo, no ceiling dirs). Byte-identical to `read_status`'s open,
/// `graph::open_no_search`, and `branches::open_repo_at`.
fn open_no_search(path: &Path) -> Result<git2::Repository, AppError> {
    Ok(git2::Repository::open_ext(
        path,
        git2::RepositoryOpenFlags::NO_SEARCH,
        std::iter::empty::<&std::ffi::OsStr>(),
    )?)
}

/// Run `f` against a per-thread cached open `Repository` for `(repo_id,
/// generation)`. Opens (bumping `perf.repo_opens`) on the first use per thread
/// for the pair and reuses thereafter; a generation bump (close / open re-arm)
/// drops the id's stale entry and reopens.
///
/// For readers that only need shared access (`read_status_with`, `list_refs_with`,
/// `list_worktrees_with`). Readers that run `stash_foreach` (the graph seed/walk
/// and the stash list) need [`with_repo_mut`].
pub fn with_repo<R>(
    repo_id: &str,
    generation: u64,
    path: &Path,
    perf: &PerfState,
    f: impl FnOnce(&git2::Repository) -> Result<R, AppError>,
) -> Result<R, AppError> {
    with_repo_mut(repo_id, generation, path, perf, |repo| f(repo))
}

/// `&mut` twin of [`with_repo`] for readers that require a mutable handle
/// (`git2` demands `&mut self` for `stash_foreach`, used by the graph seed/walk
/// and the stash list). Identical caching / eviction / open-count semantics.
pub fn with_repo_mut<R>(
    repo_id: &str,
    generation: u64,
    path: &Path,
    perf: &PerfState,
    f: impl FnOnce(&mut git2::Repository) -> Result<R, AppError>,
) -> Result<R, AppError> {
    HANDLES.with(|cell| {
        let mut map = cell.borrow_mut();
        let key = (repo_id.to_string(), generation);
        if !map.contains_key(&key) {
            // A close / open re-arm bumped the generation ⇒ this id's older-gen
            // handle is stale. Drop EVERY entry for the id before opening the
            // new one, so the map never grows unbounded across reopens and can
            // never serve a handle from before a close.
            map.retain(|(id, _), _| id != repo_id);
            let repo = open_no_search(path)?;
            perf.inc_repo_opens(); // count the ACTUAL open only (AC-b1)
            map.insert(key.clone(), repo);
        }
        // Present by construction (inserted above, or already cached). The
        // `ok_or_else` is a never-taken defensive branch — no `unwrap`/`expect`
        // on repo-derived state.
        let repo = map.get_mut(&key).ok_or_else(|| {
            AppError::Other("repo handle cache: entry missing after insert".to_string())
        })?;
        f(repo)
    })
}

/// FU-B2c: `&mut` handle reuse ACROSS the corrupt-object watchdog. Runs on the
/// caller's (blocking-pool) thread, OUTSIDE the watchdog. Takes the cached handle
/// out of `HANDLES` for `(repo_id, generation)` — opening + `perf.inc_repo_opens()`
/// only on a miss / stale generation — MOVES it THROUGH `run_with_git_timeout_owned`
/// (so `Repository`'s `Send`-but-`!Sync` handle is owned by exactly one thread at
/// every instant), and puts it back on a `Some(_)` return. On timeout/panic the
/// handle is abandoned with the worker and the entry stays absent ⇒ the next call
/// reopens (self-healing).
///
/// This is the status/graph twin of [`with_repo_mut`]: the direct variant reuses
/// only when called on the persistent blocking-pool thread (the list trio), while
/// this one reuses even though the git work runs on a fresh watchdog thread.
pub fn with_repo_mut_timed<R>(
    op: &str,
    repo_id: &str,
    generation: u64,
    path: &Path,
    perf: &PerfState,
    f: impl FnOnce(&GitProgress, &mut git2::Repository) -> Result<R, AppError> + Send + 'static,
) -> Result<R, AppError>
where
    R: Send + 'static,
{
    with_repo_mut_timed_with(op, effective_deadline(), repo_id, generation, path, perf, f)
}

/// Shared twin of [`with_repo_mut_timed`] (narrows `&mut` → `&`) for readers that
/// only need shared access (`read_status_with`). Delegates to the `&mut` variant.
pub fn with_repo_timed<R>(
    op: &str,
    repo_id: &str,
    generation: u64,
    path: &Path,
    perf: &PerfState,
    f: impl FnOnce(&GitProgress, &git2::Repository) -> Result<R, AppError> + Send + 'static,
) -> Result<R, AppError>
where
    R: Send + 'static,
{
    with_repo_mut_timed(op, repo_id, generation, path, perf, move |progress, repo| {
        f(progress, repo)
    })
}

/// Explicit-deadline internal variant so timeout behaviour is testable env-free.
/// [`with_repo_mut_timed`] delegates here with [`effective_deadline`]. Takes the
/// handle out, moves it through the owned watchdog, and re-caches it iff it came
/// back (Ok OR inner Err); a `None` (timeout / panic) leaves the entry absent.
fn with_repo_mut_timed_with<R>(
    op: &str,
    deadline: Duration,
    repo_id: &str,
    generation: u64,
    path: &Path,
    perf: &PerfState,
    f: impl FnOnce(&GitProgress, &mut git2::Repository) -> Result<R, AppError> + Send + 'static,
) -> Result<R, AppError>
where
    R: Send + 'static,
{
    // 1. TAKE the handle out of THIS pool thread's thread-local (open + count only
    //    on a miss / stale-generation evict). Borrows `repo_id`/`path`/`perf` on
    //    the pool thread only.
    let repo = HANDLES.with(|cell| {
        let mut map = cell.borrow_mut();
        let key = (repo_id.to_string(), generation);
        if let Some(r) = map.remove(&key) {
            return Ok(r); // exact-gen hit → take ownership
        }
        // A close / open re-arm bumped the generation ⇒ drop the id's stale entry.
        map.retain(|(id, _), _| id != repo_id);
        let r = open_no_search(path)?;
        perf.inc_repo_opens(); // count the ACTUAL open only (AC-b1)
        Ok::<_, AppError>(r)
    })?;

    // 2. MOVE it through the watchdog; `f` gets it by `&mut`, returns it by value.
    //    Only `f` and `repo` cross to the watchdog thread; the pool thread blocks
    //    here until it returns, so no aliasing and no `Sync` bound is needed.
    let (returned, result) = run_with_git_timeout_owned_with(op, deadline, repo, move |progress, mut repo| {
        let res = f(progress, &mut repo);
        (repo, res)
    });

    // 3. Re-cache iff it came back (Ok OR inner Err). `None` ⇒ abandoned with the
    //    wedged worker ⇒ leave absent so the next call reopens cleanly.
    if let Some(repo) = returned {
        HANDLES.with(|cell| {
            cell.borrow_mut()
                .insert((repo_id.to_string(), generation), repo);
        });
    }
    result
}

#[cfg(test)]
mod tests;
