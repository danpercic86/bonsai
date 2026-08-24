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

use bonsai_core::error::AppError;

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

#[cfg(test)]
mod tests {
    use super::*;
    use bonsai_core::git::branches::{list_refs, list_refs_with};
    use bonsai_core::git::stash::list_stashes_with;
    use bonsai_core::git::status::{read_status, read_status_with};
    use bonsai_core::git::worktree::list_worktrees_with;
    use bonsai_core::graph::graph_seed_with;

    /// init + identity + one commit + a `feature` branch — enough content for
    /// every routed reader to succeed.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let repo = git2::Repository::init(dir.path()).expect("init");
        {
            let mut cfg = repo.config().expect("config");
            cfg.set_str("user.name", "Test User").expect("name");
            cfg.set_str("user.email", "test@example.com").expect("email");
            cfg.set_bool("core.autocrlf", false).expect("autocrlf");
        }
        std::fs::write(dir.path().join("a.txt"), "base\n").expect("write");
        let mut index = repo.index().expect("index");
        index.add_path(Path::new("a.txt")).expect("add");
        index.write().expect("write index");
        let tree = repo.find_tree(index.write_tree().expect("tree")).expect("find tree");
        let sig = git2::Signature::now("Test User", "test@example.com").expect("sig");
        let head = repo
            .commit(Some("HEAD"), &sig, &sig, "C0", &tree, &[])
            .expect("commit");
        let head_commit = repo.find_commit(head).expect("head commit");
        repo.branch("feature", &head_commit, false).expect("branch");
        dir
    }

    /// One refresh round's worth of routed reads, calling `with_repo*` DIRECTLY
    /// (no `run_with_git_timeout` wrapper). This is exactly the production path of
    /// the LIST TRIO (`list_branches`/`list_stashes`/`list_worktrees`), which run
    /// on the persistent blocking-pool thread and so DO reuse across rounds. It is
    /// NOT the path of `get_status`/`stream_graph` — those run inside the timeout
    /// wrapper (fresh watchdog thread per call) and open once per call; that is
    /// covered separately by [`tests::get_status_opens_once_per_call`] /
    /// [`tests::stream_graph_path_opens_once_per_call`].
    fn run_round(id: &str, gen: u64, path: &Path, perf: &PerfState) {
        with_repo(id, gen, path, perf, read_status_with).expect("status");
        with_repo(id, gen, path, perf, list_refs_with).expect("refs");
        with_repo(id, gen, path, perf, list_worktrees_with).expect("worktrees");
        with_repo_mut(id, gen, path, perf, list_stashes_with).expect("stashes");
        with_repo_mut(id, gen, path, perf, |r| graph_seed_with(r).map(|_| ())).expect("seed");
    }

    /// AC-b1 (DIRECT / list-command path): N routed reads on a cold thread share
    /// ONE open; a warm round on the same thread + generation re-opens nothing.
    /// This measures the direct `with_repo*` path (the list trio's production
    /// behaviour), NOT the timeout-wrapped status/graph path — see `run_round`.
    #[test]
    fn ac_b1_round_shares_one_open_warm_round_zero() {
        let dir = fixture();
        let perf = PerfState::default();
        let id = "ac-b1";

        run_round(id, 1, dir.path(), &perf);
        let cold = perf.snapshot().repo_opens;

        perf.reset();
        run_round(id, 1, dir.path(), &perf);
        let warm = perf.snapshot().repo_opens;

        assert_eq!(cold, 1, "5 direct reads must share ONE open on a cold thread");
        assert_eq!(warm, 0, "a warm round (same thread + generation) re-opens nothing");
    }

    /// Production-path honesty (status): `get_status_inner` runs its `with_repo`
    /// call inside `run_with_git_timeout`, which spawns a FRESH watchdog thread
    /// per call ⇒ the `HANDLES` thread-local is empty each call ⇒ status opens
    /// ONCE PER CALL, with NO cross-round reuse. Two calls ⇒ two opens (would be
    /// 1 if the handle were reused across rounds). Documents the limitation.
    #[test]
    fn get_status_opens_once_per_call() {
        use crate::commands::{get_status_inner, open_repo_inner};
        use crate::state::AppState;
        use tauri::async_runtime::block_on;

        let dir = fixture();
        let state = AppState::default();
        let opened = block_on(open_repo_inner(
            &state,
            dir.path().to_string_lossy().into_owned(),
            |_id| Box::new(|| {}),
        ))
        .expect("open");
        let id = opened.repo_id;

        state.perf.reset();
        block_on(get_status_inner(&state, &id)).expect("status 1");
        block_on(get_status_inner(&state, &id)).expect("status 2");

        assert_eq!(
            state.perf.snapshot().repo_opens,
            2,
            "status opens once PER CALL — fresh watchdog thread ⇒ empty HANDLES ⇒ no cross-round reuse"
        );
    }

    /// Production-path honesty (graph): the `stream_graph` command routes through
    /// `run_with_git_timeout` → `with_repo_mut` → `stream_graph_cached_with` (no
    /// runtime-free `stream_graph_inner` exists, so this reconstructs that exact
    /// wrapper). Each call gets a fresh watchdog thread ⇒ opens ONCE PER CALL;
    /// within a call the seed probe + walk + store re-probe SHARE that one open
    /// (the graph's real B2b win). Two calls ⇒ two opens (no cross-round reuse).
    #[test]
    fn stream_graph_path_opens_once_per_call() {
        use bonsai_core::git::timeout::run_with_git_timeout;
        use std::sync::{Arc, Mutex};

        let dir = fixture();
        let path = dir.path().to_path_buf();
        let perf = Arc::new(PerfState::default());
        let cache: Arc<crate::graph_cache::GraphCache> = Arc::new(Mutex::new(None));

        for _ in 0..2 {
            let path = path.clone();
            let perf = perf.clone();
            let cache = cache.clone();
            run_with_git_timeout("stream_graph", move |_progress| {
                with_repo_mut("graph-timeout", 1, &path, &perf, |repo| {
                    crate::graph_cache::stream_graph_cached_with(repo, &cache, &perf, |_chunk| true)
                })
            })
            .expect("graph");
        }

        assert_eq!(
            perf.snapshot().repo_opens,
            2,
            "graph opens once PER CALL (seed+walk+re-probe fused within a call; no cross-round reuse)"
        );
    }

    /// AC-b3: a generation bump (close+open re-arm) evicts the stale handle and
    /// forces a reopen; the next call at the new generation reuses.
    #[test]
    fn ac_b3_generation_bump_forces_reopen() {
        let dir = fixture();
        let perf = PerfState::default();
        let id = "ac-b3";

        with_repo(id, 1, dir.path(), &perf, read_status_with).expect("gen1 a");
        with_repo(id, 1, dir.path(), &perf, read_status_with).expect("gen1 b");
        assert_eq!(perf.snapshot().repo_opens, 1, "gen 1: one open then reuse");

        perf.reset();
        with_repo(id, 2, dir.path(), &perf, read_status_with).expect("gen2 a");
        assert_eq!(perf.snapshot().repo_opens, 1, "generation bump reopens");

        perf.reset();
        with_repo(id, 2, dir.path(), &perf, read_status_with).expect("gen2 b");
        assert_eq!(perf.snapshot().repo_opens, 0, "gen 2 handle reused after reopen");
    }

    /// AC-b5: status through a REUSED handle after an EXTERNAL index write (via a
    /// second `Repository`) equals a fresh open — proves the `index.read(true)`
    /// freshness guard in `read_status_with`.
    #[test]
    fn ac_b5_status_reused_handle_sees_external_index_write() {
        let dir = fixture();
        let path = dir.path();
        let perf = PerfState::default();
        let id = "ac-b5-idx";

        let s1 = with_repo(id, 1, path, &perf, read_status_with).expect("s1");

        // External index write through an INDEPENDENT handle: stage a new file.
        let repo2 = git2::Repository::open(path).expect("open 2");
        std::fs::write(path.join("new.txt"), "x\n").expect("write new");
        let mut idx = repo2.index().expect("index 2");
        idx.add_path(Path::new("new.txt")).expect("add new");
        idx.write().expect("write index 2");
        drop(idx);
        drop(repo2);

        let s2 = with_repo(id, 1, path, &perf, read_status_with).expect("s2");
        let fresh = read_status(path).expect("fresh");

        assert_eq!(
            s2, fresh,
            "reused handle must equal a fresh open after an external index write"
        );
        assert_ne!(s1, s2, "the external stage must be visible through the reused handle");
        assert!(
            s2.staged.iter().any(|e| e.path == "new.txt"),
            "new.txt must show as STAGED (not untracked) ⇒ index was reloaded"
        );
    }

    /// AC-b5 (ref half): `list_refs` through a REUSED handle sees an EXTERNALLY
    /// created branch — refs are re-read on demand, the load-bearing point for
    /// the graph B1 classify and the branch list.
    #[test]
    fn ac_b5_refs_reused_handle_sees_external_branch() {
        let dir = fixture();
        let path = dir.path();
        let perf = PerfState::default();
        let id = "ac-b5-refs";

        let before = with_repo(id, 1, path, &perf, list_refs_with).expect("before");

        {
            let repo2 = git2::Repository::open(path).expect("open 2");
            let head = repo2.head().expect("head").peel_to_commit().expect("commit");
            repo2.branch("externally-added", &head, false).expect("branch");
            // `head` (borrows `repo2`) drops at the end of this block, before `repo2`.
        }

        let after = with_repo(id, 1, path, &perf, list_refs_with).expect("after");
        let fresh = list_refs(path).expect("fresh");

        assert!(
            !before.local.iter().any(|b| b.name == "externally-added"),
            "branch absent before the external create"
        );
        assert!(
            after.local.iter().any(|b| b.name == "externally-added"),
            "reused handle must see the externally created branch"
        );
        assert_eq!(after.local.len(), fresh.local.len(), "matches a fresh open");
    }
}
