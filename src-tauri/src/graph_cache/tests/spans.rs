//! P91 §3.1 `graph.get` spans.

use super::*;

// ---- P91 §3.1 spans ------------------------------------------------------

/// §12 row-3 (a)+(d): a cold walk records a `graph.get` span whose phases cover
/// revwalk/decorate/lane; a subsequent cache-served graph records `cache:'hit'`
/// with NO `revwalk` phase. Uses owned test recorders (no process-wide sink), so
/// it is deterministic under parallel test load.
#[test]
fn graph_get_span_covers_phases_and_cache() {
    let (dir, _repo, _oids) = linear_fixture();
    let cache: GraphCache = std::sync::Mutex::new(None);
    let perf = PerfState::default();

    let drive = |rec: &mut crate::obs::phase::PhaseRecorder| {
        let mut repo = git2::Repository::open_ext(
            dir.path(),
            git2::RepositoryOpenFlags::NO_SEARCH,
            std::iter::empty::<&std::ffi::OsStr>(),
        )
        .expect("open");
        stream_graph_cached_capped(
            &mut repo,
            &cache,
            &perf,
            &GraphFilter::default(),
            GRAPH_CACHE_MAX_NODES,
            rec,
            |_c| true,
        )
        .expect("stream");
    };

    // First: cold walk (Miss) — revwalk + decorate + lane all present.
    let mut miss = crate::obs::phase::PhaseRecorder::start_test(crate::obs::phase::OP_GRAPH_GET);
    drive(&mut miss);
    let (miss_phases, miss_cache) = miss.into_test_view();
    assert_eq!(miss_cache, Some("miss"));
    for want in ["revwalk", "decorate", "lane"] {
        assert!(
            miss_phases.iter().any(|n| n == want),
            "miss missing {want}: {miss_phases:?}"
        );
    }

    // Second: served from cache (HitVerbatim) — NO revwalk phase.
    let mut hit = crate::obs::phase::PhaseRecorder::start_test(crate::obs::phase::OP_GRAPH_GET);
    drive(&mut hit);
    let (hit_phases, hit_cache) = hit.into_test_view();
    assert_eq!(hit_cache, Some("hit"), "second walk should be a cache hit");
    assert!(
        !hit_phases.iter().any(|n| n == "revwalk"),
        "cache hit must have no revwalk: {hit_phases:?}"
    );
}
