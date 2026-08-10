//! Streaming commit-graph delivery skin (P65a).
//!
//! The one-shot [`compute_graph`](super::compute_graph) returns the whole
//! [`GraphLayout`](super::GraphLayout) in a single response. For huge repos we
//! instead STREAM the identical walk forward in batches through a callback, so
//! the first screenful paints instantly and the remainder arrives in the
//! background. Lane-color stability across batch boundaries is TRUE BY
//! CONSTRUCTION: this walk drives the very same [`LaneWalker`](super::LaneWalker)
//! as `compute_graph`, merely flushing in pieces — batch boundaries touch no
//! lane state (contract §0, §3).

use std::collections::HashSet;

use crate::error::AppError;

use super::{collect_seed, open_no_search, seeded_revwalk, LaneWalker, RefLabel};

/// First flush: the first screenful + generous overscan, kept small so the
/// initial paint is instant.
pub const STREAM_FIRST_BATCH: usize = 512;
/// Steady-state batch size (large — 200k rows ⇒ ~49 events, tiny event count).
pub const STREAM_BATCH: usize = 4096;
/// Streaming walk cap. Larger than the one-shot [`MAX_COMMITS`](super::MAX_COMMITS)
/// (100_000) because streaming exists for huge repos; beyond it the stream ends
/// with `truncated: true` (OQ3).
pub const STREAM_MAX_COMMITS: usize = 1_000_000;

/// A streamed commit row. Identical to [`GraphNode`](super::GraphNode) MINUS
/// `parents`: parent row indices are not known when a child is emitted (parents
/// are always at HIGHER, not-yet-walked rows), so the frontend reconstructs
/// `parents` from edge ordinals (§4.2). Saves the per-node parents bytes.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamNode {
    /// Full 40-char hex oid.
    pub id: String,
    pub lane: u32,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub refs: Vec<RefLabel>,
    pub summary: String,
    pub author: String,
    pub ts: i64,
    pub committer_ts: i64,
}

/// Logical edge as [`GraphEdge`](super::GraphEdge) PLUS the child's parent
/// ordinal (`ord`) so the frontend can rebuild each node's ordered `parents`.
/// `ord == 0` is the first parent (the lane-inheriting edge).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStreamEdge {
    /// Child row (already delivered: `from < to`).
    pub from: u32,
    /// Parent row == this batch's finalizing row.
    pub to: u32,
    /// Vertical-run lane (M2 §1.3) — RUST-owned layout math.
    pub lane: u32,
    /// Parent ordinal on `from`.
    pub ord: u16,
}

/// One channel message. Order on the wire: exactly one `Meta`, then N `Batch`,
/// then exactly one `Done`. On any error the command REJECTS (`AppError`)
/// instead of sending `Done`.
///
/// `rename_all_fields = "camelCase"` maps the struct-variant fields
/// (`head_oid`↔`headOid`, `start_row`↔`startRow`, `lane_count_so_far`↔
/// `laneCountSoFar`, `total_rows`↔`totalRows`, `lane_count`↔`laneCount`,
/// `head_index`↔`headIndex`) — the enum-level `rename_all` only renames the
/// VARIANT tags (`Meta`↔`meta`, …). Both are required to match the TS mirror
/// (contract §2.2); this follows the `BisectOutcome`/`SafeOp` recipe.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum GraphChunk {
    /// First message. `total` = exact reachable-commit count IF cheaply known
    /// (OQ2), else `None` (frontend grows the scroll extent as rows arrive).
    /// `head_oid` lets the frontend resolve `headIndex` the moment HEAD's row
    /// lands.
    Meta {
        total: Option<u32>,
        head_oid: Option<String>,
    },
    /// A run of consecutive rows `[start_row, start_row + nodes.len())` plus the
    /// edges FINALIZED within them (every edge whose parent `to` falls in this
    /// batch; its child `from` was delivered earlier or in this same batch).
    /// `lane_count_so_far` is the running max (`lanes.len()`), monotonic.
    Batch {
        start_row: u32,
        lane_count_so_far: u32,
        nodes: Vec<StreamNode>,
        edges: Vec<GraphStreamEdge>,
    },
    /// Terminal. Authoritative final scalars (redundant with the accumulated
    /// stream, for a clean close). `total_rows == nodes emitted`; `head_index`
    /// resolved; `truncated` set at the cap.
    Done {
        total_rows: u32,
        lane_count: u32,
        head_index: Option<u32>,
        truncated: bool,
    },
}

/// Blocking. Opens `workdir` (NO_SEARCH, same as `compute_graph`) and collects
/// the refs and stash tips identically, then walks forward flushing
/// [`GraphChunk`] batches through `emit`. `emit` returns `false` when the sink
/// is gone (channel dropped / cancelled) so the walk stops promptly with `Ok`.
/// Unborn / zero-ref repos yield a `Meta` then a `Done`, never an error (parity
/// with `compute_graph`). Never resolves `node.parents` (the frontend does, §4.2).
pub fn stream_graph_core(
    workdir: &std::path::Path,
    emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    stream_graph_core_with(
        workdir,
        STREAM_FIRST_BATCH,
        STREAM_BATCH,
        STREAM_MAX_COMMITS,
        emit,
    )
}

/// [`stream_graph_core`] with the batch/cap constants parameterized. The public
/// wrapper delegates with the `STREAM_*` defaults; tests drive it with tiny
/// batch sizes to prove batch boundaries never move a lane (contract §7).
pub(crate) fn stream_graph_core_with(
    workdir: &std::path::Path,
    first_batch: usize,
    batch: usize,
    max_commits: usize,
    mut emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    let mut repo = open_no_search(workdir)?;
    let (mut refs, tips, head_oid, hide) = collect_seed(&mut repo)?;
    let head_hex = head_oid.map(|h| h.to_string());

    if !emit(GraphChunk::Meta {
        total: cheap_total(&repo)?,
        head_oid: head_hex,
    }) {
        return Ok(()); // sink gone before the first row
    }
    if tips.is_empty() {
        // Unborn / zero-ref: a Meta+Done pair, never an error (§2.1).
        emit(GraphChunk::Done {
            total_rows: 0,
            lane_count: 0,
            head_index: None,
            truncated: false,
        });
        return Ok(());
    }

    let revwalk = seeded_revwalk(&repo, &tips)?;
    let hidden: HashSet<git2::Oid> = hide.iter().copied().collect();
    let mut walker = LaneWalker::new(hidden);

    let mut buf_nodes: Vec<StreamNode> = Vec::new();
    let mut buf_edges: Vec<GraphStreamEdge> = Vec::new();
    let mut start_row: u32 = 0;
    let mut row: u32 = 0;
    let mut truncated = false;
    let mut limit = first_batch; // small first flush = instant paint

    for oid in revwalk {
        let oid = oid?;
        // Stash `I`/`U` synthetic parents are never emitted as nodes.
        if walker.is_hidden(&oid) {
            continue;
        }
        if row as usize >= max_commits {
            truncated = true;
            break;
        }
        let (node, edges) = walker.step(&repo, oid, row, &mut refs)?;
        buf_nodes.push(node);
        buf_edges.extend(edges);
        row += 1;
        if buf_nodes.len() >= limit {
            if !emit(GraphChunk::Batch {
                start_row,
                lane_count_so_far: walker.lane_count(),
                nodes: std::mem::take(&mut buf_nodes),
                edges: std::mem::take(&mut buf_edges),
            }) {
                return Ok(()); // sink gone mid-stream
            }
            start_row = row;
            limit = batch; // steady-state from the second batch on
        }
    }
    if !buf_nodes.is_empty()
        && !emit(GraphChunk::Batch {
            start_row,
            lane_count_so_far: walker.lane_count(),
            nodes: std::mem::take(&mut buf_nodes),
            edges: std::mem::take(&mut buf_edges),
        })
    {
        return Ok(());
    }

    let head_index = head_oid.and_then(|h| walker.row_of(&h));
    emit(GraphChunk::Done {
        total_rows: row,
        lane_count: walker.lane_count(),
        head_index,
        truncated,
    });
    Ok(())
}

/// Exact reachable-commit count for `Meta.total` IF cheaply known, else `None`.
///
/// OQ2 (accepted): v1 grows the scroll extent as rows arrive rather than paying
/// a full pre-count walk. The P52 commit-graph file does not expose a trivially
/// cheap reachable-from-tips count, so this returns `None`. Kept fallible so a
/// future cheap count (e.g. from the commit-graph file) can slot in without a
/// signature change.
fn cheap_total(_repo: &git2::Repository) -> Result<Option<u32>, AppError> {
    Ok(None)
}

/// Wire-shape guards for the P65a→P65b seam. These assert the EXACT camelCase
/// JSON the frontend `GraphChunk` mirror (contract §2.2) folds. They fail loudly
/// if anyone ever drops `#[serde(rename_all_fields = "camelCase")]` /
/// `rename_all = "camelCase"` and a snake_case key leaks onto the wire.
#[cfg(test)]
mod tests {
    use super::{GraphChunk, GraphStreamEdge, StreamNode};
    use crate::graph::{RefKind, RefLabel};

    /// `Meta` with both fields populated → exact camelCase object.
    #[test]
    fn meta_some_wire_shape() {
        let v = serde_json::to_value(GraphChunk::Meta {
            total: Some(3),
            head_oid: Some("abc".to_string()),
        })
        .expect("serialize Meta");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "meta", "total": 3, "headOid": "abc" })
        );
    }

    /// `None` scalars serialize as JSON `null` (present, not omitted): the mirror
    /// types them `number | null` / `string | null`.
    #[test]
    fn meta_none_wire_shape() {
        let v = serde_json::to_value(GraphChunk::Meta {
            total: None,
            head_oid: None,
        })
        .expect("serialize Meta");
        assert_eq!(
            v,
            serde_json::json!({ "kind": "meta", "total": null, "headOid": null })
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
}
