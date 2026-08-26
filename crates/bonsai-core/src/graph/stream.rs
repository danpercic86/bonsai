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

use super::fold::{FoldScan, FoldSpan};
use super::{collect_seed, open_no_search, seeded_revwalk, GraphFilter, LaneWalker, RefLabel};

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
        /// Spec-003: ANY declutter filter took effect
        /// (`seed_refs_applied || first_parent`). Additive; `false` == the
        /// full, unfiltered graph.
        filtered: bool,
        /// Spec-003: the seed-ref restriction specifically took effect —
        /// `false` under the stale-refs fallback even when a non-empty
        /// `seedRefs` was requested (the UI's stale-warning truth signal).
        seed_refs_applied: bool,
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
        /// Spec-004: foldable-run metadata (`foldSpans` on the wire; OMITTED
        /// when empty — fold off or no spans found). Rides `Done` because the
        /// rule needs the full edge set. Per-request, never trusted from a
        /// cached replay (the cache injects fresh spans at replay time).
        #[serde(skip_serializing_if = "Vec::is_empty")]
        fold_spans: Vec<FoldSpan>,
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
        &GraphFilter::default(),
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
    filter: &GraphFilter,
    first_batch: usize,
    batch: usize,
    max_commits: usize,
    emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    let mut repo = open_no_search(workdir)?;
    stream_graph_from_repo_with(&mut repo, filter, first_batch, batch, max_commits, emit)
}

/// Blocking. P88b/B2b round handle cache: stream the walk from an ALREADY-OPEN
/// handle so the `stream_graph` command opens the repo ONCE for the cheap seed
/// probe AND the walk, instead of re-opening for each. Byte-identical to
/// [`stream_graph_core`] — the `&Path` entry points above open then delegate
/// here. `&mut` is required because `collect_seed` runs `stash_foreach`.
pub fn stream_graph_from_repo(
    repo: &mut git2::Repository,
    filter: &GraphFilter,
    emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    stream_graph_from_repo_with(
        repo,
        filter,
        STREAM_FIRST_BATCH,
        STREAM_BATCH,
        STREAM_MAX_COMMITS,
        emit,
    )
}

/// [`stream_graph_from_repo`] with the batch/cap constants parameterized (test +
/// `&Path`-wrapper seam). See [`stream_graph_core_with`].
pub(crate) fn stream_graph_from_repo_with(
    repo: &mut git2::Repository,
    filter: &GraphFilter,
    first_batch: usize,
    batch: usize,
    max_commits: usize,
    emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    stream_graph_inner(repo, filter, first_batch, batch, max_commits, None, emit)
}

/// Spec-004 cache seam: identical to [`stream_graph_from_repo`], additionally
/// collecting the walk's REAL-merge rows (filtered parent count > 1, recorded
/// before the first-parent truncate) into `merge_rows` — the layout cache
/// stores them so cache-hit span recomputation never re-touches libgit2.
pub fn stream_graph_from_repo_collect(
    repo: &mut git2::Repository,
    filter: &GraphFilter,
    merge_rows: &mut Vec<u32>,
    emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    stream_graph_inner(
        repo,
        filter,
        STREAM_FIRST_BATCH,
        STREAM_BATCH,
        STREAM_MAX_COMMITS,
        Some(merge_rows),
        emit,
    )
}

/// The single walk body behind every streaming entry point.
fn stream_graph_inner(
    repo: &mut git2::Repository,
    filter: &GraphFilter,
    first_batch: usize,
    batch: usize,
    max_commits: usize,
    mut merge_out: Option<&mut Vec<u32>>,
    mut emit: impl FnMut(GraphChunk) -> bool,
) -> Result<(), AppError> {
    let (mut refs, tips, head_oid, hide, seed_refs_applied) = collect_seed(repo, filter)?;
    // Downgrade to a shared borrow for the walk (the seed pass above needed
    // `&mut` for `stash_foreach`; the revwalk + lane stepping only read).
    let repo: &git2::Repository = repo;
    let head_hex = head_oid.map(|h| h.to_string());

    if !emit(GraphChunk::Meta {
        total: cheap_total(repo)?,
        head_oid: head_hex,
        // Spec-003 truth flags, set right after `collect_seed` (before the
        // empty-tips early return — hide-all on a tiny repo hits that path).
        filtered: seed_refs_applied || filter.first_parent,
        seed_refs_applied,
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
            fold_spans: Vec::new(),
        });
        return Ok(());
    }

    let revwalk = seeded_revwalk(repo, &tips, filter.first_parent)?;
    let hidden: HashSet<git2::Oid> = hide.iter().copied().collect();
    let mut walker = LaneWalker::new(hidden, filter.first_parent);

    let mut buf_nodes: Vec<StreamNode> = Vec::new();
    let mut buf_edges: Vec<GraphStreamEdge> = Vec::new();
    let mut start_row: u32 = 0;
    let mut row: u32 = 0;
    let mut truncated = false;
    let mut limit = first_batch; // small first flush = instant paint
    // Spec-004: spans are computed only when requested; merge rows only when
    // a collector was passed (the layout-cache store path).
    let mut fold: Option<FoldScan> = filter.fold_linear.then(FoldScan::new);

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
        let (node, edges) = walker.step(repo, oid, row, &mut refs)?;
        let real_merge = walker.last_was_merge();
        if real_merge {
            if let Some(out) = merge_out.as_deref_mut() {
                out.push(row);
            }
        }
        if let Some(scan) = fold.as_mut() {
            scan.push_row(node.lane, node.refs.is_empty(), real_merge);
            for e in &edges {
                scan.push_edge(e.from, e.to, e.lane);
            }
        }
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
    let fold_spans = match fold {
        Some(scan) => scan.finish(head_index, filter.first_parent),
        None => Vec::new(),
    };
    emit(GraphChunk::Done {
        total_rows: row,
        lane_count: walker.lane_count(),
        head_index,
        truncated,
        fold_spans,
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

/// Wire-shape guard tests live in `stream_wire_tests.rs` (size ratchet).
#[cfg(test)]
#[path = "stream_wire_tests.rs"]
mod tests;
