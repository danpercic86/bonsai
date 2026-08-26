//! Wire-shape guards for the P65a→P65b seam. These assert the EXACT camelCase
//! JSON the frontend `GraphChunk` mirror (contract §2.2) folds. They fail loudly
//! if anyone ever drops `#[serde(rename_all_fields = "camelCase")]` /
//! `rename_all = "camelCase"` and a snake_case key leaks onto the wire.

use super::{GraphChunk, GraphStreamEdge, StreamNode};
use crate::graph::{RefKind, RefLabel};

/// `Meta` with both fields populated → exact camelCase object.
#[test]
fn meta_some_wire_shape() {
    let v = serde_json::to_value(GraphChunk::Meta {
        total: Some(3),
        head_oid: Some("abc".to_string()),
        filtered: true,
        seed_refs_applied: true,
    })
    .expect("serialize Meta");
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "meta",
            "total": 3,
            "headOid": "abc",
            "filtered": true,
            "seedRefsApplied": true,
        })
    );
}

/// `None` scalars serialize as JSON `null` (present, not omitted): the mirror
/// types them `number | null` / `string | null`.
#[test]
fn meta_none_wire_shape() {
    let v = serde_json::to_value(GraphChunk::Meta {
        total: None,
        head_oid: None,
        filtered: false,
        seed_refs_applied: false,
    })
    .expect("serialize Meta");
    assert_eq!(
        v,
        serde_json::json!({
            "kind": "meta",
            "total": null,
            "headOid": null,
            "filtered": false,
            "seedRefsApplied": false,
        })
    );
}

/// `Batch` + its nested `StreamNode` / `GraphStreamEdge` all serialize
/// camelCase; empty `refs` is OMITTED and `committer_ts` never leaks.
#[test]
fn batch_wire_shape() {
    let chunk = GraphChunk::Batch {
        start_row: 5,
        lane_count_so_far: 3,
        nodes: vec![StreamNode {
            id: "deadbeef".to_string(),
            lane: 2,
            refs: vec![],
            summary: "msg".to_string(),
            author: "Ada".to_string(),
            ts: 100,
            committer_ts: 200,
        }],
        edges: vec![GraphStreamEdge {
            from: 0,
            to: 1,
            lane: 2,
            ord: 1,
        }],
    };
    let v = serde_json::to_value(chunk).expect("serialize Batch");

    assert_eq!(v["kind"], "batch");
    assert_eq!(v["startRow"], 5);
    assert_eq!(v["laneCountSoFar"], 3);

    // StreamNode: whole-object equality pins the exact key set (empty `refs`
    // OMITTED) so any extra/renamed key fails ...
    let node = &v["nodes"][0];
    assert_eq!(
        *node,
        serde_json::json!({
            "id": "deadbeef",
            "lane": 2,
            "summary": "msg",
            "author": "Ada",
            "ts": 100,
            "committerTs": 200,
        })
    );
    // ... and explicit presence/absence checks make a snake_case regression
    // scream with a clear message.
    assert!(node.get("committerTs").is_some(), "committerTs present");
    assert!(
        node.get("committer_ts").is_none(),
        "snake_case committer_ts must be absent"
    );
    assert!(node.get("refs").is_none(), "refs omitted when empty");

    // GraphStreamEdge wire shape.
    assert_eq!(
        v["edges"][0],
        serde_json::json!({ "from": 0, "to": 1, "lane": 2, "ord": 1 })
    );
}

/// A non-empty `refs` vec is PRESENT on the wire (the `skip_serializing_if`
/// only fires when empty), and each `RefLabel` is itself camelCase.
#[test]
fn stream_node_refs_present_when_nonempty() {
    let node = StreamNode {
        id: "abc".to_string(),
        lane: 0,
        refs: vec![RefLabel {
            name: "main".to_string(),
            kind: RefKind::LocalBranch,
            is_head: true,
        }],
        summary: "s".to_string(),
        author: "a".to_string(),
        ts: 1,
        committer_ts: 2,
    };
    let v = serde_json::to_value(&node).expect("serialize StreamNode");
    assert!(v.get("refs").is_some(), "refs present when non-empty");
    assert_eq!(
        v["refs"][0],
        serde_json::json!({ "name": "main", "kind": "localBranch", "isHead": true })
    );
}

/// `Done` terminal scalars all camelCase; covers both `Some`/`None`
/// `head_index` (mirror types it `number | null`).
#[test]
fn done_wire_shape() {
    let with_head = serde_json::to_value(GraphChunk::Done {
        total_rows: 42,
        lane_count: 4,
        head_index: Some(7),
        truncated: false,
        fold_spans: vec![],
    })
    .expect("serialize Done");
    assert_eq!(
        with_head,
        serde_json::json!({
            "kind": "done",
            "totalRows": 42,
            "laneCount": 4,
            "headIndex": 7,
            "truncated": false,
        })
    );

    let no_head = serde_json::to_value(GraphChunk::Done {
        total_rows: 0,
        lane_count: 0,
        head_index: None,
        truncated: true,
        fold_spans: vec![],
    })
    .expect("serialize Done");
    assert_eq!(
        no_head,
        serde_json::json!({
            "kind": "done",
            "totalRows": 0,
            "laneCount": 0,
            "headIndex": null,
            "truncated": true,
        })
    );
}

/// Spec-004: `fold_spans` is OMITTED when empty (both tests above) and
/// present as camelCase `foldSpans` when populated.
#[test]
fn done_fold_spans_wire_shape() {
    let v = serde_json::to_value(GraphChunk::Done {
        total_rows: 20,
        lane_count: 1,
        head_index: Some(0),
        truncated: false,
        fold_spans: vec![crate::graph::FoldSpan {
            start: 1,
            count: 7,
            lane: 0,
        }],
    })
    .expect("serialize Done");
    assert_eq!(
        v["foldSpans"],
        serde_json::json!([{ "start": 1, "count": 7, "lane": 0 }])
    );
    assert!(v.get("fold_spans").is_none(), "snake_case must not leak");
}
