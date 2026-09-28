//! PB-1 graph-cache memory cap.

use super::*;

// ---- PB-1: graph-cache memory cap ---------------------------------------

/// AC-b6 (pure gate): `should_store` caches at/below the cap and refuses above
/// it (the `<=` boundary the streaming loop keys off).
#[test]
fn should_store_gate_boundary() {
    assert!(should_store(0, GRAPH_CACHE_MAX_NODES));
    assert!(
        should_store(GRAPH_CACHE_MAX_NODES, GRAPH_CACHE_MAX_NODES),
        "at cap stores"
    );
    assert!(
        !should_store(GRAPH_CACHE_MAX_NODES + 1, GRAPH_CACHE_MAX_NODES),
        "over cap skips"
    );
    assert!(should_store(2, 2), "at tiny cap stores");
    assert!(!should_store(3, 2), "over tiny cap skips");
}

/// AC-b6: a walk whose node count EXCEEDS the (test-lowered) cap is NOT stored —
/// the cache slot stays `None` and every subsequent identical request Misses
/// again (`graph_walks` increments each time; no HitVerbatim/Redecorate).
#[test]
fn not_stored_above_cap_rewalks_each_time() {
    let (dir, _repo, _oids) = linear_fixture(); // 3 nodes > cap of 2
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let first = run_capped(dir.path(), &cache, &perf, 2);
    assert!(
        cache.lock().expect("lock").is_none(),
        "over-cap cold walk leaves the cache slot empty"
    );

    let second = run_capped(dir.path(), &cache, &perf, 2);
    assert!(
        cache.lock().expect("lock").is_none(),
        "still empty after a re-walk"
    );

    let c = perf.snapshot();
    assert_eq!(c.graph_walks, 2, "each over-cap request re-walks (Miss)");
    assert_eq!(c.graph_cache_hits, 0, "no verbatim hit above the cap");
    assert_eq!(c.graph_redecorates, 0, "no redecorate above the cap");
    assert_eq!(
        wire(&first),
        wire(&second),
        "re-walked stream is byte-identical"
    );
}

/// AC-b6: a walk AT/BELOW the cap still stores and serves HitVerbatim on the
/// next identical request (the ≤20k target path). Mirrors
/// `hit_verbatim_on_unchanged_repo` but through the capped seam at an exact-cap
/// threshold to prove the `<=` boundary stores.
#[test]
fn stored_at_cap_serves_verbatim() {
    let (dir, _repo, _oids) = linear_fixture(); // exactly 3 nodes
    let cache: GraphCache = Mutex::new(None);
    let perf = PerfState::default();

    let first = run_capped(dir.path(), &cache, &perf, 3);
    assert!(
        cache.lock().expect("lock").is_some(),
        "at-cap cold walk is stored"
    );

    let second = run_capped(dir.path(), &cache, &perf, 3);

    let c = perf.snapshot();
    assert_eq!(
        c.graph_walks, 1,
        "second request served from cache (no re-walk)"
    );
    assert_eq!(c.graph_cache_hits, 1, "at-cap store serves a verbatim hit");
    assert_eq!(c.graph_redecorates, 0);
    assert_eq!(
        wire(&first),
        wire(&second),
        "cached replay is byte-identical"
    );
}

/// AC-b6: the emitted chunk stream is byte-identical whether or not the store is
/// skipped — a store-skipped (capped) run equals a stored (uncapped) run for the
/// same repo state, proving the cap gates only the STORE, never the emit path.
#[test]
fn stream_identical_when_store_skipped() {
    let (dir, _repo, _oids) = linear_fixture();

    // Uncapped (default cap): the cold walk stores.
    let stored = {
        let cache: GraphCache = Mutex::new(None);
        let perf = PerfState::default();
        run(dir.path(), &cache, &perf)
    };
    // Capped below the fixture's node count: the store is skipped.
    let skipped = {
        let cache: GraphCache = Mutex::new(None);
        let perf = PerfState::default();
        run_capped(dir.path(), &cache, &perf, 2)
    };

    assert_eq!(
        wire(&stored),
        wire(&skipped),
        "emit path unaffected by the store cap"
    );
}
