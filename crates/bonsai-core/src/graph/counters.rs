//! Deterministic layout operation counters (spec-008 Part a, AC 7/8).
//!
//! Plain `u64` tallies of the genuine units of work the one-shot layout walk
//! performs. They are a function of the repository's history alone (no
//! timing, no hashing order), so two runs over the same repo produce equal
//! counters and a regression test can pin them against a checked-in baseline.
//! Never exported over IPC.

use super::{collect_seed, layout_walk, open_no_search, GraphFilter, GraphLayout};
use crate::error::AppError;

/// Operation counts for one layout walk. Every field is incremented with a
/// plain `+= 1` at the site doing that unit of work.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LayoutCounters {
    /// Oids pulled from the revwalk (including skip-emitted stash parents).
    pub rows_visited: u64,
    /// `LaneWalker::step` invocations (== emitted rows).
    pub lane_steps: u64,
    /// Lane slots examined by linear lane scans (reservation scan, free-lane
    /// search, merge-parent join search).
    pub lane_scans: u64,
    /// Times the active-lane vector grew by one slot (all lanes busy).
    pub lanes_allocated: u64,
    /// Edges finalized when their parent row was emitted.
    pub edges_emitted: u64,
    /// Parent oids read from commits (before the hidden-stash filter).
    pub parent_lookups: u64,
}

/// [`compute_graph_with`](super::compute_graph_with) that also returns the
/// walk's [`LayoutCounters`]. For tests and benches only.
#[doc(hidden)]
pub fn compute_graph_counted(
    workdir: &std::path::Path,
    filter: &GraphFilter,
) -> Result<(GraphLayout, LayoutCounters), AppError> {
    let mut counters = LayoutCounters::default();
    let mut repo = open_no_search(workdir)?;
    let (refs, tips, head_oid, hide, _applied) = collect_seed(&mut repo, filter)?;
    if tips.is_empty() {
        return Ok((GraphLayout::empty(), counters));
    }
    let layout = layout_walk(&repo, &tips, refs, head_oid, &hide, filter, &mut counters)?;
    Ok((layout, counters))
}
