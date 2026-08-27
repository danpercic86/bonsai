//! Spec-003 filter tests that need the crate-private streaming seams
//! (`stream_graph_core_with` with forced batch/cap constants). Sibling of
//! `tests.rs`, split out to respect the size ratchet. The public-API filter
//! matrix lives in `tests/graph_filter.rs`.

use super::tests::{branch, commit, init_repo, set_head};
use super::*;

/// The unborn-repo Meta carries the default (unfiltered) truth flags.
#[test]
fn unborn_repo_meta_flags_default_false() {
    let (dir, _repo) = init_repo();
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
    .expect("stream unborn repo");
    match chunks.first() {
        Some(GraphChunk::Meta {
            filtered,
            seed_refs_applied,
            ..
        }) => {
            assert!(!*filtered, "default filter → not filtered");
            assert!(!*seed_refs_applied, "default filter → no seed restriction");
        }
        other => panic!("first chunk must be Meta, got {other:?}"),
    }
}

/// Spec-003: the streaming cap still sets `truncated` when a filter is active
/// (first-parent at a tiny forced cap).
#[test]
fn stream_truncates_at_cap_under_first_parent_filter() {
    let (dir, repo) = init_repo();
    let c0 = commit(&repo, "C0", &[], 1);
    let c1 = commit(&repo, "C1", &[c0], 2);
    let c2 = commit(&repo, "C2", &[c1], 3);
    branch(&repo, "main", c2);
    set_head(&repo, "main");

    let filter = GraphFilter {
        first_parent: true,
        seed_refs: None,
        ..Default::default()
    };
    let mut chunks: Vec<GraphChunk> = Vec::new();
    super::stream::stream_graph_core_with(dir.path(), &filter, 1, 1, 2, |c| {
        chunks.push(c);
        true
    })
    .expect("stream under filter");
    match chunks.last() {
        Some(GraphChunk::Done {
            total_rows,
            truncated,
            ..
        }) => {
            assert_eq!(*total_rows, 2, "stops at the cap");
            assert!(*truncated, "truncated set under an active filter");
        }
        other => panic!("last chunk must be Done, got {other:?}"),
    }
}
