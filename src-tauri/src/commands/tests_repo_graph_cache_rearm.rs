//! P117 §1 — the graph-layout cache survives a same-path `open_repo` re-arm.
//!
//! Before P117, `open_repo_inner` always inserted a `RepoEntry` with a fresh
//! `graph_cache: None`. The `full` refresh scope calls `openRepo` AND refetches
//! the graph in the same round, so every `full` round destroyed the cache it was
//! about to read (measured: 62 of 81 graph requests were guaranteed misses and
//! `HitRedecorate` fired 0 times in a 152-minute session).
//!
//! These tests pin the new rule (§1.3) and the boundaries that keep it sound
//! (§1.4): a carry only ever happens for an entry already present under the
//! exact same key, a `close_repo` still starts cold, a different canonical path
//! gets its own slot, and the watcher self-heal still replaces the watcher on
//! every re-arm.
//!
//! NOTE (contract deviation, reported): AC1-1 names
//! `tests_repo_session_misc.rs`; that file is already over the ~500-line soft
//! limit, so these tests live in their own module instead.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use bonsai_core::graph::GraphFilter;

use super::tests_support::*;
use super::*;
use crate::graph_cache::{stream_graph_cached, GraphCache};
use crate::perf::PerfState;

// ---- helpers -------------------------------------------------------------

/// The `Arc` in the map for `repo_id`, cloned out the same way the `stream_graph`
/// command gets it (`repo_path_and_graph_cache`). `None` when the id isn't open.
fn slot(state: &AppState, repo_id: &str) -> Option<Arc<GraphCache>> {
    repo_path_and_graph_cache(state, repo_id)
        .ok()
        .map(|(_path, cache)| cache)
}

/// One full cache-aware graph pass against the entry's live slot, mirroring the
/// observation seam of `graph_cache/tests.rs::hit_verbatim_on_unchanged_repo`
/// (same driver, same counters — only the cache handle comes from `AppState`).
fn graph_pass(state: &AppState, repo_id: &str, perf: &PerfState) {
    let (path, cache) = repo_path_and_graph_cache(state, repo_id).expect("repo is open");
    stream_graph_cached(&path, &cache, perf, &GraphFilter::default(), |_chunk| true)
        .expect("graph pass");
}

fn watcher_installed(state: &AppState, repo_id: &str) -> bool {
    let repos = state
        .repos
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    repos
        .get(repo_id)
        .map(|e| e.watcher.is_some())
        .unwrap_or(false)
}

/// True when the slot currently holds a cached layout.
fn is_populated(cache: &Arc<GraphCache>) -> bool {
    cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
}

/// Increments the shared counter when dropped — moved into a watcher callback so
/// a test can observe that the PRIOR arm's callback really went away (AC1-5).
struct DropCounter(Arc<AtomicUsize>);

impl Drop for DropCounter {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

// ---- AC1-1 / AC1-2 -------------------------------------------------------

/// AC1-1 (the headline regression) + AC1-2 (the mechanism): a second `open` of
/// the SAME path carries the layout cache over, so the graph pass that follows
/// it is a hit — and the `Arc` read before the re-arm is pointer-identical to
/// the one read after. Pre-P117 the hit count here was 0.
#[test]
fn rearm_same_path_preserves_graph_cache() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let perf = PerfState::default();

    // Cold pass populates the slot.
    graph_pass(&state, &id, &perf);
    let before = slot(&state, &id).expect("slot after first open");
    assert_eq!(perf.snapshot().graph_walks, 1, "first pass is a cold walk");
    assert_eq!(perf.snapshot().graph_cache_hits, 0);
    assert!(is_populated(&before), "cold walk stored a layout");

    // Re-arm: same path, same canonical id.
    let again = open(&state, dir.path()).expect("re-open same path").repo_id;
    assert_eq!(again, id, "same path must re-arm the same id");
    assert_eq!(repo_count(&state), 1, "re-arm must not duplicate the entry");

    let after = slot(&state, &id).expect("slot after re-arm");
    assert!(
        Arc::ptr_eq(&before, &after),
        "AC1-2: the re-arm carries the SAME cache Arc over"
    );

    graph_pass(&state, &id, &perf);
    let c = perf.snapshot();
    assert_eq!(
        c.graph_cache_hits, 1,
        "AC1-1: the post-re-arm pass is a cache hit (0 before P117)"
    );
    assert_eq!(c.graph_walks, 1, "no second cold walk");
}

/// Review follow-up — the risk the carry-over introduces: the carried cache
/// must still correctly FAIL to serve once the topology has moved. Command-level
/// counterpart to `graph_cache/tests.rs::miss_on_new_commit`, asserted at the
/// seam where the carry lives; the `ptr_eq` in the middle is what makes it
/// meaningful — the Miss comes from an invalidated CARRIED cache, not an empty
/// slot.
#[test]
fn rearm_with_changed_topology_still_misses() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let perf = PerfState::default();

    graph_pass(&state, &id, &perf);
    let before = slot(&state, &id).expect("slot after first open");
    assert!(is_populated(&before), "cold walk stored a layout");

    // HEAD moves onto a commit the cached walk never saw.
    write_stage_commit(&state, &id, dir.path(), "b.txt", "more\n", "C1");
    open(&state, dir.path()).expect("re-arm after the new commit");
    let after = slot(&state, &id).expect("slot after re-arm");
    assert!(
        Arc::ptr_eq(&before, &after),
        "the re-arm carried the (now stale) slot over"
    );

    graph_pass(&state, &id, &perf);
    let c = perf.snapshot();
    assert_eq!(
        c.graph_cache_hits, 0,
        "a tip the cached walk never saw must never hit"
    );
    assert_eq!(c.graph_walks, 2, "it re-walks instead");
}

// ---- AC1-3 ---------------------------------------------------------------

/// AC1-3 / §1.4(a): `close_repo` drops the entry (and its `Arc`), so a later
/// open of the same path finds nothing under the key and starts cold — by
/// construction, with no special case in the carry-over.
#[test]
fn close_then_reopen_starts_cold() {
    let state = AppState::default();
    let (dir, id, _c0) = fixture_repo(&state);
    let perf = PerfState::default();

    graph_pass(&state, &id, &perf);
    let before = slot(&state, &id).expect("slot after first open");
    assert_eq!(perf.snapshot().graph_walks, 1);

    tauri::async_runtime::block_on(close_repo_inner(&state, &id)).expect("close");
    assert_eq!(repo_count(&state), 0);

    let reopened = open(&state, dir.path())
        .expect("re-open after close")
        .repo_id;
    assert_eq!(reopened, id);
    let after = slot(&state, &id).expect("slot after re-open");
    assert!(
        !Arc::ptr_eq(&before, &after),
        "a close must not leave the old slot reachable"
    );
    assert!(!is_populated(&after), "the post-close slot starts empty");

    graph_pass(&state, &id, &perf);
    let c = perf.snapshot();
    assert_eq!(
        c.graph_cache_hits, 0,
        "AC1-3: the post-close pass is a Miss from an empty slot"
    );
    assert_eq!(c.graph_walks, 2, "it re-walks");
}

// ---- AC1-4 ---------------------------------------------------------------

/// AC1-4 / §1.4(d): an open whose canonical id differs from every existing key
/// mints its OWN empty slot and leaves the other entry's cache alone. The
/// case-variant half (case-insensitive filesystems only) is the complement: a
/// different path STRING that canonicalizes onto an existing key hits the
/// dedupe scan's `repo_id = existing` branch and therefore DOES carry the slot —
/// that branch, not a byte-identical string, is what the `full` scope exercises.
#[test]
fn distinct_canonical_path_gets_a_fresh_slot() {
    let state = AppState::default();
    let (dir_a, id_a, _c0) = fixture_repo(&state);
    let perf_a = PerfState::default();
    graph_pass(&state, &id_a, &perf_a);
    let slot_a = slot(&state, &id_a).expect("slot A");
    assert!(is_populated(&slot_a));

    // A genuinely different directory: `read_repo_info` resolves a different
    // canonical id, so nothing exists under that key.
    //
    // (The uppercase-variant construction of `tests_repo_isolation.rs:128`
    // cannot produce this case on any platform: on Windows/macOS it dedupes to
    // the SAME key, and on a case-sensitive FS the uppercased directory does not
    // exist, so `read_repo_info`'s `is_dir()` precheck rejects it.)
    let dir_b = init_repo_with_identity();
    let id_b = open(&state, dir_b.path()).expect("open B").repo_id;
    assert_ne!(id_a, id_b);
    let slot_b = slot(&state, &id_b).expect("slot B");
    assert!(
        !Arc::ptr_eq(&slot_a, &slot_b),
        "AC1-4: a different key never resurrects another entry's slot"
    );
    assert!(
        !is_populated(&slot_b),
        "AC1-4: the new entry's slot starts None"
    );
    assert!(is_populated(&slot_a), "A's cache is untouched by B's open");

    // Complement: a path string that canonicalizes ONTO A's key is a same-path
    // re-arm and carries A's slot.
    #[cfg(any(windows, target_os = "macos"))]
    {
        let variant = path_string(dir_a.path()).to_uppercase();
        assert_ne!(variant, id_a, "the variant is a different path string");
        let deduped = tauri::async_runtime::block_on(open_repo_inner(&state, variant, |_id| {
            Box::new(|_class| {})
        }))
        .expect("open A via case-variant")
        .repo_id;
        assert_eq!(deduped, id_a, "the case-variant dedupes onto A's key");
        let carried = slot(&state, &id_a).expect("slot A after variant re-arm");
        assert!(
            Arc::ptr_eq(&slot_a, &carried),
            "a deduped re-arm carries A's slot over too"
        );
        // …and a request actually SERVES from it. The deduped arm overwrote
        // `entry.path` with a differently-spelled (upper-cased) path string
        // while keeping the carried slot, so this is the one place where "cache
        // carried across a path-string change" is checked end to end.
        graph_pass(&state, &id_a, &perf_a);
        assert_eq!(
            perf_a.snapshot().graph_cache_hits,
            1,
            "the carried slot serves the post-dedupe request"
        );
    }
    drop(dir_a);
}

// ---- AC1-5 ---------------------------------------------------------------

/// AC1-5: the watcher self-heal is untouched by the cache carry-over. Two
/// same-path opens call the `make_on_change` factory twice, each arm installs a
/// watcher, and the SECOND arm drops the first arm's callback (the replaced
/// entry is dropped off-lock, which joins the old debounce thread and with it
/// the closure that owns the callback).
#[test]
fn rearm_replaces_watcher_and_drops_prior_callback() {
    let state = AppState::default();
    let dir = init_repo_with_identity();
    let path = path_string(dir.path());

    let factory_calls = Arc::new(AtomicUsize::new(0));
    let drops = Arc::new(AtomicUsize::new(0));

    /// Builds a one-shot `make_on_change` that counts its own invocation and,
    /// when `drop_counter` is `Some`, keeps a [`DropCounter`] alive for exactly
    /// as long as the callback it returns.
    fn factory(
        calls: Arc<AtomicUsize>,
        drop_counter: Option<Arc<AtomicUsize>>,
    ) -> impl FnOnce(String) -> Box<dyn Fn(crate::watcher::BurstClass) + Send + 'static> {
        move |_id| {
            calls.fetch_add(1, Ordering::SeqCst);
            let guard = drop_counter.map(DropCounter);
            Box::new(move |_class| {
                let _ = &guard;
            })
        }
    }

    let first = tauri::async_runtime::block_on(open_repo_inner(
        &state,
        path.clone(),
        factory(Arc::clone(&factory_calls), Some(Arc::clone(&drops))),
    ))
    .expect("first open")
    .repo_id;
    assert_eq!(factory_calls.load(Ordering::SeqCst), 1);
    // If `spawn_watcher` had failed, the callback (and its DropCounter) would
    // have been dropped inside it — this assert turns that into a loud failure
    // instead of a false pass on the drop count below.
    assert!(
        watcher_installed(&state, &first),
        "the first arm installs a watcher"
    );
    assert_eq!(
        drops.load(Ordering::SeqCst),
        0,
        "the first arm's callback is still alive"
    );

    let again = tauri::async_runtime::block_on(open_repo_inner(
        &state,
        path,
        factory(Arc::clone(&factory_calls), None),
    ))
    .expect("re-arm")
    .repo_id;
    assert_eq!(again, first, "the re-arm keeps the id");
    assert_eq!(
        factory_calls.load(Ordering::SeqCst),
        2,
        "AC1-5: the factory runs on every re-arm"
    );
    assert!(
        watcher_installed(&state, &first),
        "AC1-5: the re-arm installs a fresh watcher"
    );
    assert_eq!(
        drops.load(Ordering::SeqCst),
        1,
        "AC1-5: the prior arm's callback is dropped by the re-arm"
    );
}

// ---- AC1-6 ---------------------------------------------------------------

/// AC1-6: per-repo isolation survives the carry-over. Two repos, interleaved
/// open/graph rounds: each repo's own counters are unaffected by the other's
/// re-arms, and the two slots are never the same allocation.
#[test]
fn per_repo_isolation_survives_rearm() {
    let state = AppState::default();
    let (dir_a, id_a, _ca) = fixture_repo(&state);
    let (dir_b, id_b, _cb) = fixture_repo(&state);
    assert_ne!(id_a, id_b);

    // Separate counters so each repo's hits/misses are attributable.
    let perf_a = PerfState::default();
    let perf_b = PerfState::default();

    graph_pass(&state, &id_a, &perf_a);
    graph_pass(&state, &id_b, &perf_b);
    let slot_a = slot(&state, &id_a).expect("slot A");
    let slot_b = slot(&state, &id_b).expect("slot B");
    assert!(!Arc::ptr_eq(&slot_a, &slot_b), "distinct slots");

    // Re-arm A only: A hits, B is untouched.
    open(&state, dir_a.path()).expect("re-arm A");
    graph_pass(&state, &id_a, &perf_a);
    assert_eq!(
        perf_a.snapshot().graph_cache_hits,
        1,
        "A hits after its own re-arm"
    );
    assert_eq!(perf_b.snapshot().graph_cache_hits, 0, "B unaffected");
    assert!(
        Arc::ptr_eq(&slot_b, &slot(&state, &id_b).expect("slot B")),
        "A's re-arm does not touch B's slot"
    );

    // Re-arm B only: B hits; A's counters do not move.
    open(&state, dir_b.path()).expect("re-arm B");
    graph_pass(&state, &id_b, &perf_b);
    let (a, b) = (perf_a.snapshot(), perf_b.snapshot());
    assert_eq!(b.graph_cache_hits, 1, "B hits after its own re-arm");
    assert_eq!(b.graph_walks, 1, "B never re-walked");
    assert_eq!(a.graph_cache_hits, 1, "A's counters did not move");
    assert_eq!(a.graph_walks, 1, "A never re-walked");

    assert!(
        !Arc::ptr_eq(
            &slot(&state, &id_a).expect("slot A"),
            &slot(&state, &id_b).expect("slot B")
        ),
        "AC1-6: the two slots are never the same allocation"
    );
    assert_eq!(repo_count(&state), 2, "two entries throughout");
}
