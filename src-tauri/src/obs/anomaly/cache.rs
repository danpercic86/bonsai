//! §5.1 `cache-collapse` — the graph-cache hit-rate rule, its window and its
//! per-repo debounce.
//!
//! ONE concern, split out of `slow.rs` in P117 §2.3: it is the only rule in that
//! file whose state is keyed (by repo), so it owns a struct instead of two more
//! fields on the shared `SlowState`. It also keeps `slow.rs` — a deliberate
//! collection of the *duration/saturation* rules — under the size limit.
//!
//! Keys strictly off the span's `cache` outcome and the record's RAW `repo`
//! correlation string (never a redaction ordinal, never repo content), and never
//! interpolates that string into a `detail` (§2.2 point 8).

use std::collections::HashMap;

use super::build_anomaly;
use crate::obs::record::{AnomalySeverity, LogRecord};

/// `cache-collapse` window (§5.1).
const W_CACHE_MS: i64 = 10_000;
/// Minimum spans in the window — PER REPO since P117 §2.3.
const CACHE_COLLAPSE_MIN: usize = 5;
/// Fire below this hit rate.
const CACHE_HIT_FLOOR: f64 = 0.2;

/// A `graph.get` cache outcome inside the window.
struct CacheSample {
    ts: i64,
    seq: u64,
    /// `hit` | `miss` | `redecorate` (from `span.cache`).
    kind: String,
    /// The RAW `repoId` the span was attributed to; `None` for the non-routed
    /// `stream_graph_cached` entry point (tests / diagnostics), which forms its
    /// own bucket rather than joining any repo's (§2.5).
    repo: Option<String>,
}

/// Window + per-repo debounce for `cache-collapse`.
#[derive(Default)]
pub(super) struct CacheRule {
    samples: Vec<CacheSample>,
    /// Last emit per repo (`""` for unattributed), so repo A firing does not
    /// rate-limit repo B out of its own finding. Pruned to `W_CACHE_MS` like
    /// every other debounce map, which is what keeps §11's boundedness claim
    /// true for a key space that is not a finite catalogue.
    last_fire: HashMap<String, i64>,
}

impl CacheRule {
    /// Observes one `graph.get` span that carried a `cache` outcome, then
    /// evaluates the rule for THAT span's repo only.
    ///
    /// P117 §2.3 — the 10 s window is PARTITIONED BY REPO: both the ≥5-span
    /// minimum and the hit rate are computed over the arriving repo's group
    /// alone. Five first-walks from five different repos are five cold caches
    /// doing what a cold cache does, not one cache collapsing — and both firings
    /// in the measured session were exactly that.
    pub(super) fn on_span(
        &mut self,
        ts: i64,
        seq: u64,
        kind: &str,
        repo: Option<&str>,
        mutations: &[(i64, Option<String>)],
        out: &mut Vec<LogRecord>,
    ) {
        self.samples.push(CacheSample {
            ts,
            seq,
            kind: kind.to_string(),
            repo: repo.map(str::to_string),
        });
        self.samples.retain(|c| c.ts >= ts - W_CACHE_MS);

        let group: Vec<&CacheSample> = self
            .samples
            .iter()
            .filter(|c| c.repo.as_deref() == repo)
            .collect();
        let total = group.len();
        if total < CACHE_COLLAPSE_MIN {
            return;
        }
        // A real mutation legitimately invalidates the cache — suppress then, but
        // only for a mutation attributed to THIS repo, or unattributed (§2.4).
        let window_start = ts - W_CACHE_MS;
        if mutations.iter().any(|(m, m_repo)| {
            *m >= window_start && *m <= ts && super::mutation_attributed_to(m_repo.as_deref(), repo)
        }) {
            return;
        }
        let hits = group.iter().filter(|c| c.kind == "hit").count();
        let hit_rate = hits as f64 / total as f64;
        if hit_rate >= CACHE_HIT_FLOOR {
            return;
        }
        let refs: Vec<u64> = group.iter().map(|c| c.seq).collect();
        if !self.arm(repo.unwrap_or(""), ts) {
            return;
        }
        // §2.2 point 8 — "for one repo", never WHICH repo: the value would leak a
        // path in raw mode and break the strict/raw byte-identity of the anomaly
        // stream. `refs` already carries the attribution.
        out.push(build_anomaly(
            "cache-collapse",
            AnomalySeverity::Warn,
            format!(
                "graph cache hit rate {:.0}% over {total} spans for one repo, \
                 no intervening mutation",
                hit_rate * 100.0
            ),
            refs,
            Vec::new(),
            ts,
        ));
    }

    /// True (and stamps) when `repo` may fire now — ≤1 per repo per window.
    fn arm(&mut self, repo: &str, ts: i64) -> bool {
        match self.last_fire.get(repo) {
            Some(&prev) if ts - prev < W_CACHE_MS => false,
            _ => {
                self.last_fire.insert(repo.to_string(), ts);
                true
            }
        }
    }

    pub(super) fn prune(&mut self, now: i64) {
        self.samples.retain(|c| c.ts >= now - W_CACHE_MS);
        // Same cutoff, same premise, same blast radius as `Sliding::prune`'s
        // `last_fire` (see `window.rs`): at worst one duplicate anomaly under a
        // clock regression, never a missed one — and never an unbounded map.
        self.last_fire.retain(|_, t| *t >= now - W_CACHE_MS);
    }

    /// §11 boundedness of the per-repo debounce map.
    #[cfg(test)]
    pub(super) fn last_fire_len(&self) -> usize {
        self.last_fire.len()
    }
}
