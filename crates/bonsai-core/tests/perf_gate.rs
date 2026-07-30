//! M2d perf gate (contract §5.3). Release-mode only — debug git2 is far
//! slower. Run explicitly:
//!
//! ```text
//! cargo test --release --test perf_gate -- --ignored --nocapture
//! ```

use std::time::Instant;

use bonsai_core::fixture::ensure_default_fixture;
use bonsai_core::graph::compute_graph;

#[test]
#[ignore] // release-mode gate; see module docs for the invocation
fn layout_31k_under_500ms() {
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
    println!("[perf-gate] compute_graph timings: {timings_ms:.1?} ms");

    let min = timings_ms
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    assert!(
        min < 500.0,
        "layout gate failed: minimum of 3 runs was {min:.1} ms (limit 500 ms)"
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
