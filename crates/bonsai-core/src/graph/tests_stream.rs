//! P65a streaming-equivalence tests. Exercises `stream.rs`: the streamed
//! walk must reproduce `compute_graph` byte-for-byte at every batch size, so
//! batch boundaries can never move a lane or a color. Sibling of
//! `tests_lane.rs`; shared fixture builders live in `tests.rs`.

use super::{branch, build_criss_cross, commit, init_repo, lanes, set_head};
use crate::graph::*;

// ---------- P65a: streaming lane-stability (equivalence) ----------

/// Runs the streaming walk with the batch/cap constants forced to the given
/// values and captures every emitted chunk, in wire order.
fn capture_stream(
    dir: &std::path::Path,
    first: usize,
    batch: usize,
    max: usize,
) -> Vec<GraphChunk> {
    let mut chunks: Vec<GraphChunk> = Vec::new();
    crate::graph::stream::stream_graph_core_with(dir, &GraphFilter::default(), first, batch, max, |c| {
        chunks.push(c);
        true
    })
    .expect("stream_graph_core_with");
    chunks
}

/// Folds a `Meta -> Batch* -> Done` chunk sequence back into a `GraphLayout`,
/// exactly as the P65b frontend assembler will (§4.2): nodes pushed in row
/// order, `parents` rebuilt from each edge's `from`/`to`/`ord`. Test-only
/// mirror that lets us assert the streamed walk reproduces `compute_graph`.
fn assemble(chunks: &[GraphChunk]) -> GraphLayout {
    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    // Per node: (ord, parent_row) pairs, to rebuild ordered `parents`.
    let mut parent_edges: Vec<Vec<(u16, u32)>> = Vec::new();
    let mut oid_to_row: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    let mut meta_head_oid: Option<String> = None;
    let mut lane_count = 0u32;
    let mut head_index: Option<u32> = None;
    let mut truncated = false;

    for chunk in chunks {
        match chunk {
            GraphChunk::Meta { head_oid, .. } => {
                meta_head_oid = head_oid.clone();
            }
            GraphChunk::Batch {
                start_row,
                lane_count_so_far,
                nodes: bn,
                edges: be,
            } => {
                assert_eq!(
                    *start_row as usize,
                    nodes.len(),
                    "batch start_row must be contiguous with prior rows"
                );
                for sn in bn {
                    let row = nodes.len() as u32;
                    oid_to_row.insert(sn.id.clone(), row);
                    nodes.push(GraphNode {
                        id: sn.id.clone(),
                        lane: sn.lane,
                        parents: Vec::new(), // filled after the full stream
                        refs: sn.refs.clone(),
                        summary: sn.summary.clone(),
                        author: sn.author.clone(),
                        ts: sn.ts,
                        committer_ts: sn.committer_ts,
                    });
                    parent_edges.push(Vec::new());
                }
                for se in be {
                    edges.push(GraphEdge {
                        from: se.from,
                        to: se.to,
                        lane: se.lane,
                    });
                    parent_edges[se.from as usize].push((se.ord, se.to));
                }
                lane_count = lane_count.max(*lane_count_so_far);
            }
            GraphChunk::Done {
                total_rows, lane_count: lc, head_index: hi, truncated: tr, ..
            } => {
                assert_eq!(
                    *total_rows as usize,
                    nodes.len(),
                    "Done.total_rows must equal the emitted node count"
                );
                lane_count = *lc;
                head_index = *hi;
                truncated = *tr;
            }
        }
    }

    // Rebuild ordered parents: sort each node's edges by `ord`, take the
    // parent rows. A parent dropped by truncation simply has no edge and is
    // skipped — identical COMPACTION to compute_graph's `index_of`
    // filter_map on a complete walk (contract §3.1).
    for (node, pe) in nodes.iter_mut().zip(parent_edges.iter_mut()) {
        pe.sort_by_key(|(ord, _)| *ord);
        node.parents = pe.iter().map(|(_, to)| *to).collect();
    }

    // The Meta.head_oid → row resolution must agree with Done.head_index
    // (the frontend resolves head from head_oid; §4.2).
    let head_from_oid = meta_head_oid.and_then(|h| oid_to_row.get(&h).copied());
    assert_eq!(
        head_from_oid, head_index,
        "Meta.head_oid resolves to the same row as Done.head_index"
    );

    GraphLayout {
        nodes,
        edges,
        lane_count,
        head_index,
        truncated,
        fold_spans: Vec::new(),
    }
}

/// Byte-identity assertion: nodes (incl. ordered `parents`), edges AS A SET,
/// `lane_count`, `head_index`, `truncated`.
fn assert_layouts_eq(got: &GraphLayout, want: &GraphLayout, label: &str, bs: usize) {
    assert_eq!(got.nodes, want.nodes, "{label} (batch={bs}): nodes");
    assert_eq!(
        got.lane_count, want.lane_count,
        "{label} (batch={bs}): lane_count"
    );
    assert_eq!(
        got.head_index, want.head_index,
        "{label} (batch={bs}): head_index"
    );
    assert_eq!(
        got.truncated, want.truncated,
        "{label} (batch={bs}): truncated"
    );
    let mut ge = got.edges.clone();
    let mut we = want.edges.clone();
    ge.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
    we.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
    assert_eq!(ge, we, "{label} (batch={bs}): edges as a set");
}

/// Forced batch sizes for the equivalence sweep. Varying these proves batch
/// boundaries never move a lane / color (the key P65 test, §7 item 1).
const BATCH_SIZES: [usize; 5] = [1, 2, 3, 7, 512];

/// Streams `dir` at every `BATCH_SIZES` value and asserts each assembled
/// layout is byte-identical to `compute_graph`'s one-shot output.
fn check_equivalence(label: &str, dir: &std::path::Path) {
    let oracle = compute_graph(dir).expect("compute_graph");
    assert!(
        !oracle.truncated,
        "{label}: fixture must be a complete (non-truncated) walk"
    );
    for &bs in &BATCH_SIZES {
        let chunks = capture_stream(dir, bs, bs, STREAM_MAX_COMMITS);
        let assembled = assemble(&chunks);
        assert_layouts_eq(&assembled, &oracle, label, bs);
    }
}

/// The crux: the streamed walk reproduces `compute_graph` byte-for-byte on
/// every M2 fixture (E1–E6) AND a mid-size generated fixture, at five batch
/// sizes. If this fails, the `LaneWalker` extraction changed lane behavior.
#[test]
fn stream_matches_compute_graph_across_batch_sizes() {
    // E1 — linear chain.
    {
        let (dir, repo) = init_repo();
        let c0 = commit(&repo, "C0", &[], 1);
        let c1 = commit(&repo, "C1", &[c0], 2);
        let c2 = commit(&repo, "C2", &[c1], 3);
        branch(&repo, "main", c2);
        set_head(&repo, "main");
        check_equivalence("E1-linear", dir.path());
    }
    // E2 — fork + merge.
    {
        let (dir, repo) = init_repo();
        let c0 = commit(&repo, "C0", &[], 1);
        let c1 = commit(&repo, "C1", &[c0], 2);
        let c2 = commit(&repo, "C2", &[c1], 3);
        let f1 = commit(&repo, "F1", &[c1], 4);
        let c3 = commit(&repo, "C3", &[c2], 5);
        let f2 = commit(&repo, "F2", &[f1], 6);
        let m = commit(&repo, "M", &[c3, f2], 7);
        branch(&repo, "main", m);
        set_head(&repo, "main");
        check_equivalence("E2-fork-merge", dir.path());
    }
    // E3 — two parallel branches, no merge.
    {
        let (dir, repo) = init_repo();
        let c1 = commit(&repo, "C1", &[], 1);
        let t1 = commit(&repo, "T1", &[c1], 2);
        let c2 = commit(&repo, "C2", &[c1], 3);
        let t2 = commit(&repo, "T2", &[t1], 4);
        let c3 = commit(&repo, "C3", &[c2], 5);
        branch(&repo, "main", c3);
        branch(&repo, "topic", t2);
        set_head(&repo, "main");
        check_equivalence("E3-parallel", dir.path());
    }
    // E4 — criss-cross (shared builder).
    {
        let (dir, repo) = init_repo();
        let _ = build_criss_cross(&repo, dir.path());
        check_equivalence("E4-criss-cross", dir.path());
    }
    // E5 — octopus merge (3 parents).
    {
        let (dir, repo) = init_repo();
        let r = commit(&repo, "R", &[], 1);
        let c = commit(&repo, "C", &[r], 2);
        let b = commit(&repo, "B", &[r], 3);
        let a = commit(&repo, "A", &[r], 4);
        let m = commit(&repo, "M", &[a, b, c], 5);
        branch(&repo, "main", m);
        set_head(&repo, "main");
        check_equivalence("E5-octopus", dir.path());
    }
    // E6 — two orphan roots.
    {
        let (dir, repo) = init_repo();
        let p0 = commit(&repo, "P0", &[], 1);
        let p1 = commit(&repo, "P1", &[p0], 2);
        let c0 = commit(&repo, "C0", &[], 3);
        let c1 = commit(&repo, "C1", &[c0], 4);
        let c2 = commit(&repo, "C2", &[c1], 5);
        branch(&repo, "main", c2);
        branch(&repo, "pages", p1);
        set_head(&repo, "main");
        check_equivalence("E6-two-orphans", dir.path());
    }
    // Mid-size generated fixture: parallel lanes, merges, tags, long
    // branches (git2 objects only — fast).
    {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let spec = crate::fixture::FixtureSpec {
            main_len: 300,
            branch_every: 25,
            branch_len: 8,
            merge_after: 12,
            long_branches: 2,
            long_branch_len: 30,
            tag_every: 100,
            keep_branch_ref_every: 3,
        };
        crate::fixture::generate_fixture(dir.path(), &spec).expect("generate_fixture");
        check_equivalence("mid-size", dir.path());
    }
}

/// Empty / unborn repo: the stream is exactly `Meta{total:None, head:None}`
/// then `Done{0,0,None,false}` — never an error (contract §2.1).
#[test]
fn stream_unborn_repo_emits_meta_then_done() {
    let (dir, _repo) = init_repo();
    let chunks = capture_stream(dir.path(), 512, 512, STREAM_MAX_COMMITS);
    assert_eq!(chunks.len(), 2, "exactly Meta + Done");
    match &chunks[0] {
        GraphChunk::Meta { total, head_oid, .. } => {
            assert_eq!(*total, None, "v1 grows-as-you-go (OQ2)");
            assert!(head_oid.is_none(), "unborn HEAD");
        }
        other => panic!("first chunk must be Meta, got {other:?}"),
    }
    match &chunks[1] {
        GraphChunk::Done {
            total_rows, lane_count, head_index, truncated, ..
        } => {
            assert_eq!(*total_rows, 0);
            assert_eq!(*lane_count, 0);
            assert!(head_index.is_none());
            assert!(!*truncated);
        }
        other => panic!("second chunk must be Done, got {other:?}"),
    }
    // Assembles to the same empty layout compute_graph produces.
    let oracle = compute_graph(dir.path()).expect("compute_graph");
    assert_layouts_eq(&assemble(&chunks), &oracle, "unborn", 512);
}

/// Truncation is also batch-boundary-invariant: a tiny `STREAM_MAX_COMMITS`
/// stops at the same row for every batch size, with identical lanes / edges
/// / lane_count (parents may differ only by truncation compaction, so they
/// are NOT compared — §7 item 1).
#[test]
fn stream_truncation_is_batch_invariant() {
    // E2 topology (7 nodes); truncate at 4.
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    let f1 = commit(&repo, "F1", &[c1], 4);
    let c3 = commit(&repo, "C3", &[c2], 5);
    let f2 = commit(&repo, "F2", &[f1], 6);
    let m = commit(&repo, "M", &[c3, f2], 7);
    branch(&repo, "main", m);
    set_head(&repo, "main");

    let max = 4usize;
    let mut reference: Option<GraphLayout> = None;
    for &bs in &[1usize, 2, 3, 512] {
        let chunks = capture_stream(dir.path(), bs, bs, max);
        let a = assemble(&chunks);
        assert!(a.truncated, "batch={bs}: truncated flag set at the cap");
        assert_eq!(a.nodes.len(), max, "batch={bs}: stopped at the cap");
        match &reference {
            None => reference = Some(a),
            Some(r) => {
                assert_eq!(
                    lanes(&a),
                    lanes(r),
                    "batch={bs}: lanes stable under truncation"
                );
                assert_eq!(a.lane_count, r.lane_count, "batch={bs}: lane_count");
                let mut ae = a.edges.clone();
                let mut re = r.edges.clone();
                ae.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
                re.sort_unstable_by_key(|e| (e.from, e.to, e.lane));
                assert_eq!(ae, re, "batch={bs}: edges stable under truncation");
            }
        }
    }
}
