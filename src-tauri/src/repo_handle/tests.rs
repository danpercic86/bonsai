//! Handle-cache tests, split out of `repo_handle.rs` (FU-B2c) to keep the parent
//! module under the size ratchet — mirrors `graph_cache.rs` / `graph_cache/tests.rs`.
//!
//! Two production paths are exercised:
//! - **Direct** (`with_repo`/`with_repo_mut`): the list trio's path — reuse only
//!   holds on the persistent blocking-pool thread (`ac_b1*`, `ac_b3`, `ac_b5*`).
//! - **Timed** (`with_repo_timed`/`with_repo_mut_timed`, FU-B2c): status/graph
//!   run their `with_repo*` seam on the POOL thread and MOVE the handle through
//!   the corrupt-object watchdog, so they reuse ACROSS ROUNDS even though the git
//!   work runs on a fresh watchdog thread (`fu_b2c_*`). These drive the `_timed`
//!   wrappers DIRECTLY on the test thread (NOT through `get_status_inner`'s
//!   `spawn_blocking`, whose pool-thread choice is nondeterministic).

use super::*;
use bonsai_core::git::branches::{list_refs, list_refs_with};
use bonsai_core::git::stash::list_stashes_with;
use bonsai_core::git::status::{read_status, read_status_with, StatusSnapshot};
use bonsai_core::git::worktree::list_worktrees_with;
use bonsai_core::graph::{graph_seed_with, GraphChunk};
use std::sync::{Arc, Mutex};

use crate::graph_cache::GraphCache;

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
/// on the persistent blocking-pool thread and so DO reuse across rounds.
fn run_round(id: &str, gen: u64, path: &Path, perf: &PerfState) {
    with_repo(id, gen, path, perf, read_status_with).expect("status");
    with_repo(id, gen, path, perf, list_refs_with).expect("refs");
    with_repo(id, gen, path, perf, list_worktrees_with).expect("worktrees");
    with_repo_mut(id, gen, path, perf, list_stashes_with).expect("stashes");
    with_repo_mut(id, gen, path, perf, |r| graph_seed_with(r).map(|_| ())).expect("seed");
}

/// Drive a graph stream through `with_repo_mut_timed` (the FU-B2c seam), the
/// exact `stream_graph` command path, collecting the emitted chunks on the test
/// thread. An mpsc channel bridges them out because the emit closure moves into
/// the (watchdog) worker thread; `with_repo_mut_timed` blocks until it returns,
/// so the sender has dropped by the time we drain.
fn graph_chunks_timed(id: &str, gen: u64, path: &Path, perf: &Arc<PerfState>) -> Vec<GraphChunk> {
    let cache: Arc<GraphCache> = Arc::new(Mutex::new(None));
    let (tx, rx) = std::sync::mpsc::channel::<GraphChunk>();
    let perf_walk = perf.clone();
    with_repo_mut_timed("stream_graph", id, gen, path, perf, move |progress, repo| {
        crate::graph_cache::stream_graph_cached_with(repo, &cache, &perf_walk, |chunk| {
            progress.tick();
            tx.send(chunk).is_ok()
        })
    })
    .expect("graph stream");
    rx.into_iter().collect()
}

/// Status via the FU-B2c timed seam (single-shot: no tick).
fn status_timed(id: &str, gen: u64, path: &Path, perf: &Arc<PerfState>) -> StatusSnapshot {
    with_repo_timed("read_status", id, gen, path, perf, move |_p, repo| read_status_with(repo))
        .expect("status")
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

/// FU-B2c AC-(a) (status seam): calling `with_repo_timed`(read_status) THEN
/// `with_repo_mut_timed`(graph) on the SAME thread + generation shares ONE handle
/// keyed `(repo_id, gen)` — cold round `repo_opens == 1` (the status call opens,
/// the graph call reuses), warm round `repo_opens == 0` (cross-round reuse). This
/// REPLACES the old `get_status_opens_once_per_call` limitation test. Driven
/// directly on the test thread (the `_timed` seam runs on the caller's thread).
#[test]
fn fu_b2c_status_reuses_across_rounds() {
    let dir = fixture();
    let perf = Arc::new(PerfState::default());
    let id = "fu-b2c-status";

    // Cold round: status (miss → open) then graph (hit → reuse) share one handle.
    let _ = status_timed(id, 1, dir.path(), &perf);
    let _ = graph_chunks_timed(id, 1, dir.path(), &perf);
    let cold = perf.snapshot().repo_opens;

    // Warm round: the cached handle survives across rounds ⇒ zero opens.
    perf.reset();
    let _ = status_timed(id, 1, dir.path(), &perf);
    let _ = graph_chunks_timed(id, 1, dir.path(), &perf);
    let warm = perf.snapshot().repo_opens;

    assert_eq!(cold, 1, "status+graph on the same thread+gen share ONE handle (cold)");
    assert_eq!(warm, 0, "a warm round reuses the cached handle across rounds");
}

/// FU-B2c AC-(a) (graph seam): `with_repo_mut_timed`(graph) reuses its handle
/// ACROSS rounds — cold round opens once, a warm round on the same thread + gen
/// re-opens nothing. This REPLACES the old `stream_graph_path_opens_once_per_call`
/// limitation test (which asserted two calls ⇒ two opens; FU-B2c fixes that).
#[test]
fn fu_b2c_graph_reuses_across_rounds() {
    let dir = fixture();
    let perf = Arc::new(PerfState::default());
    let id = "fu-b2c-graph";

    let _ = graph_chunks_timed(id, 1, dir.path(), &perf);
    let cold = perf.snapshot().repo_opens;

    perf.reset();
    let _ = graph_chunks_timed(id, 1, dir.path(), &perf);
    let warm = perf.snapshot().repo_opens;

    assert_eq!(cold, 1, "cold graph round opens ONE handle");
    assert_eq!(warm, 0, "a warm graph round reuses the cached handle across rounds");
}

/// FU-B2c AC-(b): a hanging closure through the timed wrapper (tiny deadline)
/// returns the corrupt-odb timeout `AppError::Git` PROMPTLY, and the abandoned
/// handle is NOT re-cached — the NEXT fast call on the same thread + generation
/// reopens (`repo_opens` bumps), proving self-healing. No shared `&mut`.
#[test]
fn fu_b2c_timeout_abandons_handle_and_next_call_reopens() {
    let dir = fixture();
    let perf = Arc::new(PerfState::default());
    let id = "fu-b2c-timeout";

    let hung = with_repo_mut_timed_with(
        "hang",
        Duration::from_millis(300),
        id,
        1,
        dir.path(),
        &perf,
        |_progress, _repo| {
            std::thread::sleep(Duration::from_secs(600));
            Ok::<(), AppError>(())
        },
    );
    match hung {
        Err(AppError::Git(m)) => {
            assert!(m.contains("operation timed out"), "message: {m}");
            assert!(m.contains("hang"), "names the op: {m}");
        }
        other => panic!("expected Git timeout error, got {other:?}"),
    }
    assert_eq!(perf.snapshot().repo_opens, 1, "the hanging call opened once (miss)");

    // The abandoned handle must NOT be in the cache ⇒ the next call reopens.
    perf.reset();
    let _ = status_timed(id, 1, dir.path(), &perf);
    assert_eq!(
        perf.snapshot().repo_opens,
        1,
        "abandoned handle was not re-cached ⇒ next call reopens (self-healing)"
    );
}

/// FU-B2c AC-(c): a generation bump evicts the stale handle through the timed
/// seam — gen 1 opens then reuses (1 → 0); a gen-2 call reopens (1).
#[test]
fn fu_b2c_generation_evict_timed() {
    let dir = fixture();
    let perf = Arc::new(PerfState::default());
    let id = "fu-b2c-gen";

    let _ = status_timed(id, 1, dir.path(), &perf);
    let _ = status_timed(id, 1, dir.path(), &perf);
    assert_eq!(perf.snapshot().repo_opens, 1, "gen 1: one open then reuse (timed)");

    perf.reset();
    let _ = status_timed(id, 2, dir.path(), &perf);
    assert_eq!(perf.snapshot().repo_opens, 1, "generation bump reopens (timed)");

    perf.reset();
    let _ = status_timed(id, 2, dir.path(), &perf);
    assert_eq!(perf.snapshot().repo_opens, 0, "gen 2 handle reused after reopen (timed)");
}

/// FU-B2c AC-(d) (status): `StatusSnapshot` from `with_repo_timed` is byte-
/// identical to a fresh `read_status(path)`.
#[test]
fn fu_b2c_status_timed_byte_identical() {
    let dir = fixture();
    let path = dir.path();
    let perf = Arc::new(PerfState::default());

    let timed = status_timed("fu-b2c-status-eq", 1, path, &perf);
    let fresh = read_status(path).expect("fresh status");
    assert_eq!(timed, fresh, "timed status must equal a fresh open");
}

/// FU-B2c AC-(d) (graph): the `Vec<GraphChunk>` collected through
/// `with_repo_mut_timed` equals a fresh `stream_graph_cached(path, ..)` stream.
/// `GraphChunk` is `Serialize`-only (no `PartialEq`) ⇒ compare via `serde_json`
/// value per chunk.
#[test]
fn fu_b2c_graph_timed_byte_identical() {
    let dir = fixture();
    let path = dir.path();
    let perf = Arc::new(PerfState::default());

    let timed = graph_chunks_timed("fu-b2c-graph-eq", 1, path, &perf);

    let fresh_cache: Arc<GraphCache> = Arc::new(Mutex::new(None));
    let fresh_perf = PerfState::default();
    let mut fresh: Vec<GraphChunk> = Vec::new();
    crate::graph_cache::stream_graph_cached(path, &fresh_cache, &fresh_perf, |chunk| {
        fresh.push(chunk);
        true
    })
    .expect("fresh graph stream");

    assert_eq!(timed.len(), fresh.len(), "same chunk count");
    for (a, b) in timed.iter().zip(fresh.iter()) {
        let va = serde_json::to_value(a).expect("serialize timed chunk");
        let vb = serde_json::to_value(b).expect("serialize fresh chunk");
        assert_eq!(va, vb, "GraphChunk byte-identical (serde value)");
    }
}

/// AC-b3: a generation bump (close+open re-arm) evicts the stale handle and
/// forces a reopen; the next call at the new generation reuses. (Direct path.)
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
