//! M2d perf gate (contract §5.3). Release-mode only — debug git2 is far
//! slower. Run explicitly:
//!
//! ```text
//! cargo test --release --test perf_gate -- --ignored --nocapture
//! ```

use std::time::Instant;

use bonsai_core::fixture::ensure_default_fixture;
use bonsai_core::graph::{compute_graph, compute_graph_with, GraphFilter};

#[test]
#[ignore] // release-mode gate; see module docs for the invocation
fn layout_31k_under_500ms() {
    // P52: ensure_default_fixture now writes `.git/objects/info/commit-graph`
    // (once, when git is available), so this gate measures compute_graph with
    // the commit-graph present — libgit2 reads generation numbers + inline
    // commit metadata from it instead of inflating 31k commit objects, giving
    // the layout revwalk more margin under the 500 ms ceiling.
    let path = ensure_default_fixture().expect("fixture generation failed");

    // Warm-up (page cache, odb).
    let warm = compute_graph(&path).expect("compute_graph failed");
    assert!(!warm.truncated);
    assert_eq!(warm.nodes.len(), 31_000, "fixture should have 31k commits");

    let mut timings_ms: Vec<f64> = Vec::with_capacity(3);
    for _ in 0..3 {
        let t = Instant::now();
        let layout = compute_graph(&path).expect("compute_graph failed");
        timings_ms.push(t.elapsed().as_secs_f64() * 1e3);
        assert_eq!(layout.nodes.len(), 31_000);
    }

    let min = timings_ms
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    println!("[perf-gate] compute_graph timings: {timings_ms:.1?} ms (best {min:.1} ms)");
    assert!(
        min < 500.0,
        "layout gate failed: minimum of 3 runs was {min:.1} ms (limit 500 ms)"
    );
}

/// Spec-003 perf bullet: a first-parent walk must not be SLOWER than the full
/// walk on the large fixture (it walks a subset — `simplify_first_parent`
/// prunes the node set). Comparative (min-of-3 vs min-of-3), so machine speed
/// cancels; the 1.25× headroom absorbs scheduler noise. NOTE: how much the
/// fixture actually shrinks under first-parent depends on its live-ref
/// topology (live tips keep their first-parent lines) — the assertion is
/// deliberately only "not slower", exactly what the plan requires.
#[test]
#[ignore] // release-mode gate; see module docs for the invocation
fn first_parent_layout_not_slower_than_full() {
    let path = ensure_default_fixture().expect("fixture generation failed");
    let fp = GraphFilter {
        first_parent: true,
        seed_refs: None,
    };

    // Warm-up both paths (page cache, odb, commit-graph).
    let full_warm = compute_graph(&path).expect("compute_graph failed");
    let fp_warm = compute_graph_with(&path, &fp).expect("compute_graph_with failed");
    assert!(
        fp_warm.nodes.len() <= full_warm.nodes.len(),
        "first-parent must never grow the node set"
    );

    let time3 = |f: &dyn Fn() -> usize| -> (Vec<f64>, f64) {
        let mut timings_ms = Vec::with_capacity(3);
        for _ in 0..3 {
            let t = Instant::now();
            let n = f();
            timings_ms.push(t.elapsed().as_secs_f64() * 1e3);
            assert!(n > 0);
        }
        let min = timings_ms.iter().copied().fold(f64::INFINITY, f64::min);
        (timings_ms, min)
    };
    let (full_t, full_min) =
        time3(&|| compute_graph(&path).expect("compute_graph failed").nodes.len());
    let (fp_t, fp_min) = time3(&|| {
        compute_graph_with(&path, &fp)
            .expect("compute_graph_with failed")
            .nodes
            .len()
    });

    println!(
        "[perf-gate] full walk: {full_t:.1?} ms (best {full_min:.1} ms, {} nodes); \
         first-parent: {fp_t:.1?} ms (best {fp_min:.1} ms, {} nodes)",
        full_warm.nodes.len(),
        fp_warm.nodes.len()
    );
    assert!(
        fp_min <= full_min * 1.25,
        "first-parent walk slower than full: {fp_min:.1} ms vs {full_min:.1} ms full \
         (limit = full × 1.25)"
    );
}

#[test]
#[ignore] // release-mode gate; see module docs for the invocation
fn serialize_31k_report() {
    let path = ensure_default_fixture().expect("fixture generation failed");
    let layout = compute_graph(&path).expect("compute_graph failed");

    // Warm-up.
    let _ = serde_json::to_string(&layout).expect("serialize failed");

    let mut timings_ms: Vec<f64> = Vec::with_capacity(3);
    let mut bytes = 0usize;
    for _ in 0..3 {
        let t = Instant::now();
        let json = serde_json::to_string(&layout).expect("serialize failed");
        timings_ms.push(t.elapsed().as_secs_f64() * 1e3);
        bytes = json.len();
    }
    println!(
        "[perf-gate] serialize timings: {timings_ms:.1?} ms, size: {bytes} bytes ({:.2} MB)",
        bytes as f64 / 1e6
    );

    let min = timings_ms
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    assert!(
        min < 250.0,
        "serialize soft ceiling breached: {min:.1} ms (limit 250 ms) — \
         consider the additive stream_graph fallback (contract §1.1)"
    );
}
