//! The shared lane-assignment engine (P65a).
//!
//! [`LaneWalker`] is the single source of lane truth: it advances one commit
//! per [`step`](LaneWalker::step), assigning lanes, routing edges, and tracking
//! reservations EXACTLY as the pre-P65 inline `layout_walk` did. BOTH the
//! one-shot [`compute_graph`](super::compute_graph) and the streaming
//! [`stream_graph_core`](super::stream_graph_core) drive it, so a streamed walk
//! is byte-for-byte the same computation as the one-shot walk, merely flushed in
//! pieces — batch boundaries touch no lane state (the equivalence guarantee,
//! contract §0/§3/§7).

use std::collections::{HashMap, HashSet};

use crate::error::AppError;

use super::{GraphStreamEdge, RefMap, StreamNode};

/// An edge created at child time, finalized when the parent row is emitted.
struct PendingEdge {
    from: u32,
    lane: u32,
    /// The child's parent ordinal (first parent `p0` → 0, `parents[k]` → k).
    /// Carried onto the finalized [`GraphStreamEdge`] so the streaming frontend
    /// can rebuild each node's ordered `parents` (contract §3.1). The one-shot
    /// path ignores it (it resolves parents from the emitted-row index).
    ord: u16,
}

/// Lowest free lane index; grows the vector when all lanes are busy. Scanning
/// always starts at 0 — simple and deterministic (§8.5).
fn first_free(lanes: &mut Vec<Option<git2::Oid>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// First line of `summary`, char-safe capped at `max` chars.
fn first_line_capped(bytes: Option<&[u8]>, max: usize) -> String {
    let s = String::from_utf8_lossy(bytes.unwrap_or_default());
    let first_line = s.lines().next().unwrap_or("");
    first_line.chars().take(max).collect()
}

/// The per-row lane engine shared by BOTH the one-shot [`compute_graph`] and the
/// streaming [`stream_graph_core`]. Owns the active-lane vector, the pending-edge
/// map, the emitted-row index, and the hidden-oid set.
pub(super) struct LaneWalker {
    /// `lanes[i] == Some(p)`: an edge runs down lane `i`, waiting for commit
    /// `p`. Multiple lanes may wait for the same oid (converging lines). Grows
    /// only.
    lanes: Vec<Option<git2::Oid>>,
    /// Edges created at child time, keyed by the parent oid they await.
    pending: HashMap<git2::Oid, Vec<PendingEdge>>,
    /// oid → emitted row (== node index); populated as each node is emitted.
    index_of: HashMap<git2::Oid, u32>,
    /// Stash synthetic parents (`I`/`U`) that must never become nodes.
    hidden: HashSet<git2::Oid>,
    /// Scratch: the filtered parent oids of the most recent [`step`](Self::step),
    /// in original order. The one-shot path takes it each row (via
    /// [`take_last_parents`](Self::take_last_parents)) to resolve
    /// `GraphNode.parents` from the row index, byte-identically to the
    /// pre-refactor code; the streaming path ignores it. The Vec is built for
    /// routing regardless, so retaining it here costs the stream nothing (moved,
    /// never cloned).
    last_parents: Vec<git2::Oid>,
}

impl LaneWalker {
    pub(super) fn new(hidden: HashSet<git2::Oid>) -> Self {
        LaneWalker {
            lanes: Vec::new(),
            pending: HashMap::new(),
            index_of: HashMap::new(),
            hidden,
            last_parents: Vec::new(),
        }
    }

    /// Number of lanes ever active (== `lanes.len()`; monotonic — drives the
    /// graph-area width and never shrinks).
    pub(super) fn lane_count(&self) -> u32 {
        self.lanes.len() as u32
    }

    /// Whether `oid` is a hidden stash synthetic parent (skip-emitted, never a
    /// node). Callers must test this BEFORE [`step`](Self::step).
    pub(super) fn is_hidden(&self, oid: &git2::Oid) -> bool {
        self.hidden.contains(oid)
    }

    /// Emitted row of `oid`, if it has been walked (used to resolve parent
    /// indices and `head_index`). `None` for a not-yet-walked / truncated oid.
    pub(super) fn row_of(&self, oid: &git2::Oid) -> Option<u32> {
        self.index_of.get(oid).copied()
    }

    /// Moves out the filtered parent oids of the most recent [`step`](Self::step)
    /// so the one-shot caller can resolve `GraphNode.parents` from the row index.
    pub(super) fn take_last_parents(&mut self) -> Vec<git2::Oid> {
        std::mem::take(&mut self.last_parents)
    }

    /// Advance one commit at row `row` (§2.4 steps 1–5). Returns the node
    /// (WITHOUT `parents` — parents always appear at HIGHER, not-yet-walked
    /// rows) and the edges FINALIZED at this row (those whose parent == `oid`),
    /// each carrying its child's parent ordinal `ord`. Mutates lane state
    /// EXACTLY as the pre-refactor `layout_walk` did — the single source of lane
    /// truth. `refs` labels for `oid` are moved into the emitted node.
    pub(super) fn step(
        &mut self,
        repo: &git2::Repository,
        oid: git2::Oid,
        row: u32,
        refs: &mut RefMap,
    ) -> Result<(StreamNode, Vec<GraphStreamEdge>), AppError> {
        let commit = repo.find_commit(oid)?;

        // 1. Which lanes were waiting for this commit? (ascending)
        let reserved: Vec<usize> = self
            .lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| **l == Some(oid))
            .map(|(i, _)| i)
            .collect();

        // 2. Pick this commit's lane.
        let lane = if reserved.is_empty() {
            first_free(&mut self.lanes) // tip / new branch head / orphan root
        } else {
            // Leftmost waiting lane wins; converging lines free their lanes.
            for &i in &reserved[1..] {
                self.lanes[i] = None;
            }
            reserved[0]
        };

        // 3. Finalize every edge that was waiting for this commit.
        let mut finalized: Vec<GraphStreamEdge> = Vec::new();
        for pe in self.pending.remove(&oid).unwrap_or_default() {
            finalized.push(GraphStreamEdge {
                from: pe.from,
                to: row,
                lane: pe.lane,
                ord: pe.ord,
            });
        }

        // 4. Route edges to parents / update reservations. Skip-emitted stash
        //    parents (`I`/`U`) are filtered out so `W` keeps only its base `B`
        //    → a single `W → B` edge and no dangling lane reservation.
        let parents: Vec<git2::Oid> = commit
            .parent_ids()
            .filter(|p| !self.hidden.contains(p))
            .collect();
        if parents.is_empty() {
            self.lanes[lane] = None; // root: line ends here
        } else {
            let p0 = parents[0];
            // First parent inherits the lane — even if p0 is ALSO reserved
            // elsewhere (convergence happens at p0 via leftmost-wins).
            self.lanes[lane] = Some(p0);
            self.pending.entry(p0).or_default().push(PendingEdge {
                from: row,
                lane: lane as u32,
                ord: 0,
            });
            for (k, &pk) in parents.iter().enumerate().skip(1) {
                // Merge parents (octopus-safe): join an existing line if one is
                // already waiting for pk, else open a new lane.
                let j = match self.lanes.iter().position(|l| *l == Some(pk)) {
                    Some(j) => j,
                    None => {
                        let j = first_free(&mut self.lanes);
                        self.lanes[j] = Some(pk);
                        j
                    }
                };
                self.pending.entry(pk).or_default().push(PendingEdge {
                    from: row,
                    lane: j as u32,
                    ord: k as u16,
                });
            }
        }

        // 5. Build the node (parents resolved later by the one-shot caller).
        self.index_of.insert(oid, row);
        let author = commit.author();
        let committer = commit.committer();
        let node = StreamNode {
            id: oid.to_string(),
            lane: lane as u32,
            refs: refs.remove(&oid).unwrap_or_default(),
            summary: first_line_capped(commit.summary_bytes(), 120),
            author: String::from_utf8_lossy(author.name_bytes()).into_owned(),
            ts: author.when().seconds(),
            committer_ts: committer.when().seconds(),
        };
        self.last_parents = parents;
        Ok((node, finalized))
    }
}
