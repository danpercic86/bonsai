//! Full-layout walk (§2.4): drives the shared `LaneWalker` over the seeded
//! revwalk and assembles the finished [`GraphLayout`].

use std::collections::HashSet;

use super::{
    fold, seeded_revwalk, GraphEdge, GraphFilter, GraphLayout, GraphNode, LaneWalker,
    LayoutCounters, RefMap, MAX_COMMITS,
};
use crate::error::AppError;

/// Core lane-assignment walk (§2.4). Tips must be pre-deduped and in
/// deterministic order; `refs` labels are moved into the emitted nodes.
/// `counters` tallies the walk's work (spec-008); the uncounted caller passes
/// a throwaway default.
pub(super) fn layout_walk(
    repo: &git2::Repository,
    tips: &[git2::Oid],
    mut refs: RefMap,
    head_oid: Option<git2::Oid>,
    hide: &[git2::Oid],
    filter: &GraphFilter,
    counters: &mut LayoutCounters,
) -> Result<GraphLayout, AppError> {
    let first_parent = filter.first_parent;
    let revwalk = seeded_revwalk(repo, tips, first_parent)?;
    // Stash synthetic parents (`I` = staged index, `U` = untracked) must never
    // become nodes. We do NOT use `revwalk.hide` for this: `hide(I)` marks I AND
    // its ancestors uninteresting, and `I`'s parent IS the stash base `B`, so it
    // would wrongly exclude `B` (and its history) from the walk — the opposite
    // of the intended single `W → B` edge. Instead the `LaneWalker` skip-emits
    // these oids: they are dropped when reached in the walk and filtered out of
    // any commit's parent list, so `B` stays reachable via the branch/stash
    // tips. A hidden oid absent from the graph is a tolerated no-op.
    let hidden: HashSet<git2::Oid> = hide.iter().copied().collect();
    let mut walker = LaneWalker::new(hidden, first_parent);

    let mut nodes: Vec<GraphNode> = Vec::new();
    let mut edges: Vec<GraphEdge> = Vec::new();
    let mut raw_parents: Vec<Vec<git2::Oid>> = Vec::new();
    let mut truncated = false;
    // Spec-004: fold spans, computed over the SAME walk output when requested.
    let mut fold: Option<fold::FoldScan> = filter.fold_linear.then(fold::FoldScan::new);

    for oid in revwalk {
        let oid = oid?;
        counters.rows_visited += 1;
        // Stash `I`/`U` synthetic parents are never emitted as nodes.
        if walker.is_hidden(&oid) {
            continue;
        }
        if nodes.len() >= MAX_COMMITS {
            truncated = true;
            break;
        }
        // Row == final node index (skipped oids leave no gap).
        let row = nodes.len() as u32;
        let (node, node_edges) = walker.step(repo, oid, row, &mut refs, counters)?;
        if let Some(scan) = fold.as_mut() {
            scan.push_row(node.lane, node.refs.is_empty(), walker.last_was_merge());
            for e in &node_edges {
                scan.push_edge(e.from, e.to, e.lane);
            }
        }
        // Keep this row's raw parent oids for the `index_of` resolution below
        // (byte-identical to the pre-`LaneWalker` code); streaming skips this.
        raw_parents.push(walker.take_last_parents());
        nodes.push(GraphNode {
            id: node.id,
            lane: node.lane,
            parents: Vec::new(), // resolved below
            refs: node.refs,
            summary: node.summary,
            author: node.author,
            ts: node.ts,
            committer_ts: node.committer_ts,
        });
        for e in node_edges {
            edges.push(GraphEdge {
                from: e.from,
                to: e.to,
                lane: e.lane,
            });
        }
    }

    // 6. Resolve parent oids → indices; parents outside the emitted set are
    //    dropped (truncation only — a complete walk emits every ancestor).
    //    Pending edges never finalized are dropped with the walker.
    for (node, ps) in nodes.iter_mut().zip(raw_parents.iter()) {
        node.parents = ps.iter().filter_map(|p| walker.row_of(p)).collect();
    }

    edges.sort_unstable_by_key(|e| (e.from, e.to)); // required wire order (§1.1)
    let head_index = head_oid.and_then(|h| walker.row_of(&h));
    let lane_count = walker.lane_count();
    let fold_spans = match fold {
        Some(scan) => scan.finish(head_index, first_parent),
        None => Vec::new(),
    };

    Ok(GraphLayout {
        nodes,
        edges,
        lane_count,
        head_index,
        truncated,
        fold_spans,
    })
}
