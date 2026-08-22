//! The cheap O(refs) walk seed exposed for the P86 layout cache.
//!
//! Split out of `graph.rs` (file-size discipline). `graph_seed` reuses the
//! private `open_no_search` + `collect_seed` from the parent module — the exact
//! same seed both graph paths compute — WITHOUT walking history.

use super::{collect_seed, open_no_search, RefMap};
use crate::error::AppError;

/// The cheap O(refs) walk SEED exposed for the P86 layout cache: everything that
/// decides whether an unchanged-topology refresh can skip the O(commits) walk.
/// `tips`/`head`/`hide` form the WALK identity (topology); `refs` is the
/// DECORATION (pills). Derived by [`graph_seed`] without walking history.
pub struct GraphSeed {
    pub refs: RefMap,
    /// Deduped revwalk tips in deterministic push order (includes HEAD + stash
    /// tips), exactly as `compute_graph`/`stream_graph_core` push them.
    pub tips: Vec<git2::Oid>,
    pub head: Option<git2::Oid>,
    /// Stash synthetic parents (`I`/`U`) skip-emitted by the walk.
    pub hide: Vec<git2::Oid>,
}

/// Blocking. Opens `workdir` (NO_SEARCH, same as `compute_graph`) and collects
/// the deterministic walk seed WITHOUT walking history — O(refs), not
/// O(commits). This is the cache probe the `stream_graph` command runs on every
/// request to classify Hit / HitRedecorate / Miss (P86 B1). `compute_graph` /
/// `stream_graph_core` are unchanged and still collect their own seed internally.
pub fn graph_seed(workdir: &std::path::Path) -> Result<GraphSeed, AppError> {
    let mut repo = open_no_search(workdir)?;
    let (refs, tips, head, hide) = collect_seed(&mut repo)?;
    Ok(GraphSeed {
        refs,
        tips,
        head,
        hide,
    })
}
