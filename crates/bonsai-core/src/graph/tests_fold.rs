//! Spec-004 fold tests that need the crate-private streaming seams
//! (`stream_graph_core_with` with a forced tiny cap). Sibling of
//! `tests_filter.rs`; the public-API fold matrix lives in
//! `tests/graph_fold.rs`.

use super::tests::{branch, commit, init_repo, set_head};
use super::*;

/// A run adjacent to the truncation cap folds; `truncated` stays true. The
/// capped last row has no outgoing edge (its parent was never emitted), so it
/// is never itself foldable — the span keeps a visible bottom anchor.
#[test]
fn run_adjacent_to_cap_folds_and_truncated_set() {
    let (dir, repo) = init_repo();
    let mut tip = commit(&repo, "c0", &[], 1);
    for i in 1..30 {
        tip = commit(&repo, &format!("c{i}"), &[tip], 1 + i);
    }
    branch(&repo, "main", tip);
    set_head(&repo, "main");

    let filter = GraphFilter {
        fold_linear: true,
        ..Default::default()
    };
    let mut chunks: Vec<GraphChunk> = Vec::new();
    super::stream::stream_graph_core_with(dir.path(), &filter, 4, 4, 12, |c| {
        chunks.push(c);
        true
    })
    .expect("capped fold stream");

    match chunks.last() {
        Some(GraphChunk::Done {
            total_rows,
            truncated,
            fold_spans,
            ..
        }) => {
            assert_eq!(*total_rows, 12, "stops at the cap");
            assert!(*truncated, "truncated set at the cap");
            // Row 0 carries the main pill + HEAD; row 11 (cap edge) has no
            // outgoing edge. Rows 1..=10 fold.
            assert_eq!(
                fold_spans,
                &vec![FoldSpan {
                    start: 1,
                    count: 10,
                    lane: 0
                }]
            );
        }
        other => panic!("last chunk must be Done, got {other:?}"),
    }
}

/// Fold off (default filter) ⇒ no spans on the wire even over a long run.
#[test]
fn fold_off_emits_no_spans() {
    let (dir, repo) = init_repo();
    let mut tip = commit(&repo, "c0", &[], 1);
    for i in 1..20 {
        tip = commit(&repo, &format!("c{i}"), &[tip], 1 + i);
    }
    branch(&repo, "main", tip);
    set_head(&repo, "main");

    let mut chunks: Vec<GraphChunk> = Vec::new();
    super::stream::stream_graph_core_with(
        dir.path(),
        &GraphFilter::default(),
        512,
        512,
        STREAM_MAX_COMMITS,
        |c| {
            chunks.push(c);
            true
        },
    )
    .expect("stream fold off");
    match chunks.last() {
        Some(GraphChunk::Done { fold_spans, .. }) => {
            assert!(fold_spans.is_empty(), "fold off must emit no spans");
        }
        other => panic!("last chunk must be Done, got {other:?}"),
    }
}
