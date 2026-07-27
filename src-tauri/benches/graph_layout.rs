//! Criterion benchmark for the commit-graph layout engine (contract §5.2).
//!
//! Measures `compute_graph` over the shared 31k-commit cached fixture, plus
//! the serde_json wire-size / serialization cost (§1.1 measurement). Criterion
//! reports; the hard assertions live in `tests/perf_gate.rs`.

use criterion::{criterion_group, criterion_main, Criterion};

use bonsai_lib::fixture::ensure_default_fixture;
use bonsai_lib::graph::compute_graph;

fn bench_graph(c: &mut Criterion) {
    let path = ensure_default_fixture().expect("fixture generation failed");

    let mut group = c.benchmark_group("graph");
    group.sample_size(10);

    group.bench_function("compute_graph_31k", |b| {
        b.iter(|| compute_graph(&path).expect("compute_graph failed"));
    });

    let layout = compute_graph(&path).expect("compute_graph failed");
    let json = serde_json::to_string(&layout).expect("serialize failed");
    eprintln!(
        "[bench] wire size: {} bytes ({:.2} MB) for {} nodes / {} edges",
        json.len(),
        json.len() as f64 / 1e6,
        layout.nodes.len(),
        layout.edges.len()
    );

    group.bench_function("serialize_31k", |b| {
        b.iter(|| serde_json::to_string(&layout).expect("serialize failed"));
    });

    group.finish();
}

criterion_group!(benches, bench_graph);
criterion_main!(benches);
