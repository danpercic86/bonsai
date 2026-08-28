//! P91 §3.1 — backend operation spans: phase timing, queue delay, pool
//! saturation and watchdog pressure, emitted as ONE `span` record per completed
//! operation.
//!
//! **No ambient stack** (§3.1.1): the [`PhaseRecorder`] is a value threaded
//! explicitly, exactly like [`super::trace::TraceMeta`]. Phase names and op ids
//! are `&'static str` from the allow-lists below, so no user-derived string can
//! reach a span record.
//!
//! **Zero cost when Dev mode is off.** [`PhaseRecorder::start`] reads the active
//! sink once; with none it returns a recorder holding `None` and every method is
//! a branch-and-return — the phase `Vec` is never allocated.
//!
//! **Crate boundary (§3.1.2):** `crates/bonsai-core` never depends on this
//! module. All timing is taken at the src-tauri caller layer.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use super::record::{LogLevel, LogPayload, PhaseTiming};
use super::sink::Sink;
use super::trace::{self, TraceMeta};

/// The three instrumented operations (§3.1.2). Adding a fourth requires a §13
/// entry — this list is the guard that keeps spans a diagnostic tool, not a
/// profiler.
pub const OP_GRAPH_GET: &str = "graph.get";
pub const OP_STATUS_SCAN: &str = "status.scan";
pub const OP_DIFF_COMPUTE: &str = "diff.compute";

/// Configured tokio blocking-pool cap. Bonsai does not override
/// `max_blocking_threads`, so this is the tokio default (512). Kept here as a
/// named constant read at span start (§3.1.3).
pub const POOL_MAX: u32 = 512;

/// Graph-cache outcome for a `graph.get` span (§5.1 `cache-collapse`).
#[derive(Clone, Copy, Debug)]
pub enum CacheOutcome {
    Hit,
    Redecorate,
    Miss,
}

impl CacheOutcome {
    fn as_str(self) -> &'static str {
        match self {
            CacheOutcome::Hit => "hit",
            CacheOutcome::Redecorate => "redecorate",
            CacheOutcome::Miss => "miss",
        }
    }
}

/// Terminal outcome of a span.
#[derive(Clone, Copy, Debug)]
pub enum SpanOutcome {
    Ok,
    Err,
    Timeout,
}

impl SpanOutcome {
    fn as_str(self) -> &'static str {
        match self {
            SpanOutcome::Ok => "ok",
            SpanOutcome::Err => "err",
            SpanOutcome::Timeout => "timeout",
        }
    }
}

/// Process-wide gauge of blocking-pool tasks currently inside a
/// [`PoolGuard`]. Incremented on closure entry, decremented on exit (§3.1.3).
static POOL_INFLIGHT: AtomicUsize = AtomicUsize::new(0);

/// RAII guard that keeps the [`POOL_INFLIGHT`] gauge accurate. Entered as the
/// first statement inside an instrumented `spawn_blocking` closure; the count it
/// captured at entry is what a span reports as `poolInflight`.
pub struct PoolGuard {
    inflight_at_entry: u32,
}

impl PoolGuard {
    /// Increments the gauge and returns the in-flight count INCLUDING this task.
    pub fn enter() -> Self {
        let prev = POOL_INFLIGHT.fetch_add(1, Ordering::Relaxed);
        PoolGuard {
            inflight_at_entry: (prev + 1).min(u32::MAX as usize) as u32,
        }
    }

    /// In-flight blocking tasks at the moment this one started running.
    pub fn inflight(&self) -> u32 {
        self.inflight_at_entry
    }

    /// The configured pool cap (§3.1.3).
    pub fn max(&self) -> u32 {
        POOL_MAX
    }
}

impl Drop for PoolGuard {
    fn drop(&mut self) {
        POOL_INFLIGHT.fetch_sub(1, Ordering::Relaxed);
    }
}

/// TEST-ONLY read of the raw gauge, for the pool-saturation acceptance test.
#[cfg(test)]
pub fn pool_inflight_now() -> usize {
    POOL_INFLIGHT.load(Ordering::Relaxed)
}

struct Inner {
    op: &'static str,
    start: Instant,
    phases: Vec<PhaseTiming>,
    queued_ms: Option<u32>,
    pool_inflight: Option<u32>,
    pool_max: Option<u32>,
    deadline_frac: Option<f32>,
    cache: Option<CacheOutcome>,
    items: Option<u64>,
}

/// Explicit sub-span recorder (§3.1.1). Holds the live sink, so `finish` can emit
/// with no `AppHandle` in scope.
pub struct PhaseRecorder {
    sink: Option<Arc<Sink>>,
    inner: Option<Inner>,
}

/// Drop-scoped phase handle. On drop it pushes `{name, ms}` onto the recorder.
/// A dotted `name` is just a label — there is NO implicit parent stack (§3.1.1).
pub struct PhaseGuard<'a> {
    rec: &'a mut PhaseRecorder,
    name: &'static str,
    start: Instant,
    n: Option<u64>,
}

impl PhaseRecorder {
    /// Starts a recorder for `op`. No-op (no allocation) when Dev mode is off.
    pub fn start(op: &'static str) -> Self {
        let sink = trace::active_sink();
        let inner = sink.as_ref().map(|_| Inner {
            op,
            start: Instant::now(),
            phases: Vec::new(),
            queued_ms: None,
            pool_inflight: None,
            pool_max: None,
            deadline_frac: None,
            cache: None,
            items: None,
        });
        PhaseRecorder { sink, inner }
    }

    /// True when this recorder will emit — lets callers skip building phase
    /// labels entirely when off.
    pub fn is_active(&self) -> bool {
        self.inner.is_some()
    }

    /// Opens a phase; the returned guard closes it on drop (§3.1.1). ≤16 phases
    /// are retained; further phases are timed but not stored (the overhead budget
    /// caps the array, not the operation).
    pub fn phase(&mut self, name: &'static str) -> PhaseGuard<'_> {
        PhaseGuard {
            rec: self,
            name,
            start: Instant::now(),
            n: None,
        }
    }

    /// Records a phase whose duration the caller measured itself — used when a
    /// phase boundary is an EVENT (e.g. "revwalk ends when the first batch
    /// arrives") rather than a lexical scope a [`PhaseGuard`] can bracket.
    /// `&'static str` keeps the allow-list guarantee.
    pub fn add_phase(&mut self, name: &'static str, ms: f64) {
        self.push_phase(name, ms, None);
    }

    fn push_phase(&mut self, name: &'static str, ms: f64, n: Option<u64>) {
        if let Some(inner) = self.inner.as_mut() {
            if inner.phases.len() < 16 {
                inner.phases.push(PhaseTiming {
                    name: name.to_string(),
                    ms,
                    n,
                });
            }
        }
    }

    /// Records queue delay + pool saturation for this op (§3.1.3).
    pub fn note_queue(&mut self, queued_ms: u32, inflight: u32, max: u32) {
        if let Some(inner) = self.inner.as_mut() {
            inner.queued_ms = Some(queued_ms);
            inner.pool_inflight = Some(inflight);
            inner.pool_max = Some(max);
        }
    }

    /// Records watchdog pressure: `elapsed / deadline` (§3.1.3).
    pub fn note_deadline(&mut self, frac: f32) {
        if let Some(inner) = self.inner.as_mut() {
            inner.deadline_frac = Some(frac);
        }
    }

    /// Records the graph-cache outcome (graph.get only).
    pub fn note_cache(&mut self, outcome: CacheOutcome) {
        if let Some(inner) = self.inner.as_mut() {
            inner.cache = Some(outcome);
        }
    }

    /// Records the primary unit count for the whole op.
    pub fn note_items(&mut self, n: u64) {
        if let Some(inner) = self.inner.as_mut() {
            inner.items = Some(n);
        }
    }

    /// Emits the single `span` record. Trace causality is explicit, like
    /// [`super::trace::emit_logged`]. A no-op recorder drops silently.
    pub fn finish(self, meta: &TraceMeta, outcome: SpanOutcome) {
        let (Some(sink), Some(inner)) = (self.sink, self.inner) else {
            return;
        };
        let ms = inner.start.elapsed().as_secs_f64() * 1000.0;
        let phases = if inner.phases.is_empty() {
            None
        } else {
            Some(inner.phases)
        };
        let payload = LogPayload::Span {
            op: inner.op.to_string(),
            ms,
            phases,
            queued_ms: inner.queued_ms,
            pool_inflight: inner.pool_inflight,
            pool_max: inner.pool_max,
            deadline_frac: inner.deadline_frac,
            cache: inner.cache.map(|c| c.as_str().to_string()),
            items: inner.items,
            outcome: Some(outcome.as_str().to_string()),
        };
        let rec = trace::make_record(&sink, LogLevel::Debug, meta, payload);
        sink.enqueue(rec);
    }
}

#[cfg(test)]
impl PhaseRecorder {
    /// An ACTIVE recorder with no sink, so tests can drive phase timing and read
    /// it back deterministically without touching the process-wide sink (which a
    /// parallel test could contaminate). `finish` on one of these is a no-op.
    pub fn start_test(op: &'static str) -> Self {
        PhaseRecorder {
            sink: None,
            inner: Some(Inner {
                op,
                start: Instant::now(),
                phases: Vec::new(),
                queued_ms: None,
                pool_inflight: None,
                pool_max: None,
                deadline_frac: None,
                cache: None,
                items: None,
            }),
        }
    }

    /// Drains a test recorder into `(phase names, cache outcome str)`.
    pub fn into_test_view(self) -> (Vec<String>, Option<&'static str>) {
        let inner = self.inner.expect("test recorder must be active");
        let names = inner.phases.into_iter().map(|p| p.name).collect();
        let cache = inner.cache.map(|c| c.as_str());
        (names, cache)
    }
}

impl PhaseGuard<'_> {
    /// Attaches a unit count (commits walked, files scanned) to this phase.
    pub fn set_n(&mut self, n: u64) {
        self.n = Some(n);
    }
}

impl Drop for PhaseGuard<'_> {
    fn drop(&mut self) {
        let ms = self.start.elapsed().as_secs_f64() * 1000.0;
        let (name, n) = (self.name, self.n);
        self.rec.push_phase(name, ms, n);
    }
}

#[cfg(test)]
#[path = "tests_phase.rs"]
mod tests_phase;
