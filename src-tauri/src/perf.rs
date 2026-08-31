//! Backend perf counters (P86 instrumentation).
//!
//! Atomic tallies incremented at the caching seams so the tester can assert —
//! from the outside, via `debug_perf_counters` — that an unchanged-topology
//! refresh serves the layout cache (hit / redecorate) instead of re-walking,
//! and that a real topology change re-walks (Miss). Cheap `Relaxed` loads/stores:
//! these are diagnostics, never a synchronization primitive.

use std::sync::atomic::{AtomicU64, Ordering};

/// Serializable snapshot returned by `debug_perf_counters`. Field docs mirror
/// the seams that bump each counter.
#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfCounters {
    /// git2 repo opens on the touched command paths (graph seed probe + walk,
    /// status scan). A follow-on handle cache (B2) would drive this toward 1.
    pub repo_opens: u64,
    /// Full O(commits) revwalks actually performed (`stream_graph` Miss +
    /// every `get_graph`, which is intentionally uncached — see graph_cache.rs).
    pub graph_walks: u64,
    /// `stream_graph` HitVerbatim: cached chunks replayed with no git work.
    pub graph_cache_hits: u64,
    /// `stream_graph` HitRedecorate: cached chunks re-pilled, no revwalk.
    pub graph_redecorates: u64,
    /// `get_status` worktree scans entered (O(worktree)).
    pub status_scans: u64,
}

/// Live atomic counters held in `AppState` (behind an `Arc` so blocking-pool
/// tasks can bump them). `Default` == all zero.
#[derive(Default)]
pub struct PerfState {
    repo_opens: AtomicU64,
    graph_walks: AtomicU64,
    graph_cache_hits: AtomicU64,
    graph_redecorates: AtomicU64,
    status_scans: AtomicU64,
}

impl PerfState {
    pub fn inc_repo_opens(&self) {
        self.repo_opens.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_graph_walks(&self) {
        self.graph_walks.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_graph_cache_hits(&self) {
        self.graph_cache_hits.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_graph_redecorates(&self) {
        self.graph_redecorates.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_status_scans(&self) {
        self.status_scans.fetch_add(1, Ordering::Relaxed);
    }

    /// Consistent-enough snapshot for diagnostics (fields read independently).
    pub fn snapshot(&self) -> PerfCounters {
        PerfCounters {
            repo_opens: self.repo_opens.load(Ordering::Relaxed),
            graph_walks: self.graph_walks.load(Ordering::Relaxed),
            graph_cache_hits: self.graph_cache_hits.load(Ordering::Relaxed),
            graph_redecorates: self.graph_redecorates.load(Ordering::Relaxed),
            status_scans: self.status_scans.load(Ordering::Relaxed),
        }
    }

    /// Zero every counter (test/harness reset before a measured scenario).
    pub fn reset(&self) {
        self.repo_opens.store(0, Ordering::Relaxed);
        self.graph_walks.store(0, Ordering::Relaxed);
        self.graph_cache_hits.store(0, Ordering::Relaxed);
        self.graph_redecorates.store(0, Ordering::Relaxed);
        self.status_scans.store(0, Ordering::Relaxed);
    }
}
