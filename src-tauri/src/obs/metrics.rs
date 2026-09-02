//! P91 §8 — durable local metrics: rolled-up daily counters, duration
//! histograms and error tallies, persisted to `metrics/usage.json`.
//!
//! **Two structural guarantees this module exists to keep:**
//! 1. *No key is user-derived.* Every metric key comes from the fixed allow-list
//!    below (`<domain>.<action>`), or from a command name validated to be a bare
//!    code identifier. Repo content — paths, refs, messages — can never become a
//!    key, so metrics need no redaction and are NOT in the `logs_delete_all`
//!    scope (§8).
//! 2. *No network sink.* Nothing here reaches an HTTP client; a test asserts the
//!    whole `obs/` tree names no HTTP-client crate at all.
//!
//! The percentiles (`p50Ms`/`p95Ms`) are DERIVED at [`MetricsState::snapshot`]
//! time from the frozen [`Histogram`] buckets and are **never persisted** —
//! `usage.json` stores only the 8 bucket counters + aggregates, so its size is
//! independent of how many observations folded through (§8.1).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::obs::histogram::Histogram;
use crate::obs::record::PhaseTiming;
use crate::obs::writer;
use crate::perf::PerfCounters;

use super::metrics_file;
use super::metrics_keys::{is_valid_cmd_name, is_valid_counter_key, is_valid_err_code};

/// Current on-disk metrics schema. Bumped only if an EXISTING field changes
/// shape; additive growth keeps it at 1 (mirrors `OBS_SCHEMA_VERSION`).
pub const METRICS_SCHEMA_VERSION: u32 = 1;

/// Daily buckets kept in full before the oldest folds into `lifetime` (§8).
pub const RETAIN_DAYS: usize = 400;

/// The persisted metrics root (§8). `camelCase` on the wire.
///
/// `metrics_snapshot()` returns a clone of this with each histogram's DERIVED
/// `p50Ms`/`p95Ms` filled; the durable form on disk always omits them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricsFile {
    pub schema: u32,
    /// ISO date of first launch that wrote metrics.
    pub first_seen: String,
    /// App launches that have folded metrics (bumped once per launch at init).
    pub sessions: u64,
    /// Retained daily buckets, oldest first. Length ≤ [`RETAIN_DAYS`].
    pub days: Vec<DayBucket>,
    /// Everything older than [`RETAIN_DAYS`], folded together.
    pub lifetime: MetricTotals,
}

/// One calendar day's aggregates (§8).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DayBucket {
    /// `YYYY-MM-DD`, UTC.
    pub date: String,
    pub totals: MetricTotals,
}

/// The tallies inside a day bucket (or `lifetime`). All maps are keyed by the
/// allow-listed namespace only (§8).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricTotals {
    /// `<domain>.<action>` → count (`perf.repo_opens`, `commit.create`, ...).
    pub counters: BTreeMap<String, u64>,
    /// Duration key (`cmd.<name>`, `op.graph.get`, ...) → bounded histogram.
    pub durations: BTreeMap<String, Histogram>,
    /// Error code → count.
    pub errors: BTreeMap<String, u64>,
    /// Wall time attributed to this day, ms.
    pub session_ms: u64,
}

impl MetricTotals {
    /// Folds `other` into `self` — used for the 400-day → `lifetime` roll-up.
    fn merge(&mut self, other: &MetricTotals) {
        for (k, v) in &other.counters {
            *self.counters.entry(k.clone()).or_insert(0) = self
                .counters
                .get(k)
                .copied()
                .unwrap_or(0)
                .saturating_add(*v);
        }
        for (k, h) in &other.durations {
            self.durations.entry(k.clone()).or_default().merge(h);
        }
        for (k, v) in &other.errors {
            *self.errors.entry(k.clone()).or_insert(0) = self
                .errors
                .get(k)
                .copied()
                .unwrap_or(0)
                .saturating_add(*v);
        }
        self.session_ms = self.session_ms.saturating_add(other.session_ms);
    }
}

// --------------------------------------------------------------- allow-lists

/// The three instrumented span ops (mirrors `obs/phase.rs`) with the phase
/// sub-keys that get their own histogram (§8.1). Anything outside this table is
/// silently dropped — no fallback key, so the key set stays bounded.
fn allowed_phases(op: &str) -> Option<&'static [&'static str]> {
    match op {
        "graph.get" => Some(&["revwalk", "decorate", "lane", "filter", "serialize"]),
        "status.scan" => Some(&["statuses"]),
        "diff.compute" => Some(&["hunks"]),
        _ => None,
    }
}

/// Accessor for one `PerfCounters` field, paired with its counter key.
type PerfAccessor = fn(&PerfCounters) -> u64;

/// Maps a `PerfCounters` field to its `perf.*` counter key (§8 absorption).
const PERF_KEYS: [(&str, PerfAccessor); 5] = [
    ("perf.repo_opens", |p| p.repo_opens),
    ("perf.graph_walks", |p| p.graph_walks),
    ("perf.graph_cache_hits", |p| p.graph_cache_hits),
    ("perf.graph_redecorates", |p| p.graph_redecorates),
    ("perf.status_scans", |p| p.status_scans),
];

// ------------------------------------------------------------------- state

struct Inner {
    /// Resolved `metrics/usage.json`; `None` until [`MetricsState::init`].
    path: Option<PathBuf>,
    file: MetricsFile,
    /// Last `PerfCounters` snapshot folded, so `fold_perf` records only deltas.
    perf_baseline: PerfCounters,
    dirty: bool,
    /// Epoch secs of the last `session_ms` attribution, for wall-time deltas.
    last_wall_secs: i64,
}

/// Managed observability-metrics state (§8). Always present — unlike the Dev-mode
/// log sink, metrics accumulate `perf.*` counters and `sessions` on every launch;
/// the `op.*`/`cmd.*` duration histograms fill only during Dev-mode sessions
/// (their source `span`/`ipc.result` records exist only then — §11 keeps span
/// recording zero-cost when Dev mode is off).
pub struct MetricsState {
    inner: Mutex<Inner>,
}

impl Default for MetricsState {
    fn default() -> Self {
        MetricsState {
            inner: Mutex::new(Inner {
                path: None,
                file: MetricsFile {
                    schema: METRICS_SCHEMA_VERSION,
                    ..Default::default()
                },
                perf_baseline: PerfCounters::default(),
                dirty: false,
                last_wall_secs: 0,
            }),
        }
    }
}

impl MetricsState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Loads `usage.json` (with `.bak` recovery) from `dir`, bumps `sessions` and
    /// sets `first_seen` if unset. Blocking (reads a file) — call on the blocking
    /// pool. `now_secs` fixes the launch date deterministically for tests.
    pub fn init(&self, dir: PathBuf, now_secs: i64) {
        let path = dir.join("usage.json");
        let mut file = metrics_file::load(&path);
        if file.schema == 0 {
            file.schema = METRICS_SCHEMA_VERSION;
        }
        if file.first_seen.is_empty() {
            file.first_seen = writer::utc_date(now_secs);
        }
        file.sessions = file.sessions.saturating_add(1);
        let mut g = self.lock();
        g.path = Some(path);
        g.file = file;
        g.perf_baseline = PerfCounters::default();
        g.dirty = true;
        g.last_wall_secs = now_secs;
    }

    /// TEST ONLY — a store bound to `path` with no prior file. Loads whatever is
    /// already at `path` (so `.bak`/restart tests can pre-seed it).
    #[cfg(test)]
    pub fn for_test(path: PathBuf, now_secs: i64) -> Self {
        let state = MetricsState::default();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // `init` appends `usage.json`; here `path` IS the file, so seed directly.
        let mut file = metrics_file::load(&path);
        if file.schema == 0 {
            file.schema = METRICS_SCHEMA_VERSION;
        }
        if file.first_seen.is_empty() {
            file.first_seen = writer::utc_date(now_secs);
        }
        file.sessions = file.sessions.saturating_add(1);
        {
            let mut g = state.lock();
            g.path = Some(path);
            g.file = file;
            g.last_wall_secs = now_secs;
            g.dirty = true;
        }
        state
    }

    // ---------------------------------------------------------- observation

    /// Returns today's day bucket, creating it (and folding the oldest into
    /// `lifetime` past [`RETAIN_DAYS`]) on a date change.
    ///
    /// **Append-only clock assumption:** `today` is expected to be monotone
    /// non-decreasing across a session (it comes from the wall clock). Only the
    /// LAST bucket is compared, so a UTC step *backwards* mid-session would append
    /// a duplicate bucket for an older date. Real clock skew is sub-second and
    /// harmless here; a backwards step across a day boundary is not a case the §8
    /// contract addresses.
    fn totals_for<'a>(inner: &'a mut Inner, today: &str) -> &'a mut MetricTotals {
        if inner.file.days.last().map(|d| d.date.as_str()) != Some(today) {
            // A new day started. Enforce retention BEFORE pushing so the vec never
            // exceeds RETAIN_DAYS: fold the oldest into `lifetime`.
            while inner.file.days.len() >= RETAIN_DAYS {
                let oldest = inner.file.days.remove(0);
                let lifetime = &mut inner.file.lifetime;
                lifetime.merge(&oldest.totals);
            }
            inner.file.days.push(DayBucket {
                date: today.to_string(),
                totals: MetricTotals::default(),
            });
        }
        &mut inner
            .file
            .days
            .last_mut()
            .expect("day bucket just ensured")
            .totals
    }

    /// Increments a counter key. The caller MUST pass an allow-listed
    /// `<domain>.<action>` literal — the counter map is only bounded, and only
    /// free of user-derived content, because every call site uses a fixed string
    /// (today: `fold_perf` and tests).
    ///
    /// The `debug_assert` below is the enforcement: a key carrying a path
    /// separator, whitespace or an uppercase letter is what a branch name, a path
    /// or a ref would look like, and such a key would be persisted verbatim into
    /// `usage.json` — a file that carries NO user content by construction (§8).
    /// Debug-only on purpose: this is a programmer mistake to catch in tests, not
    /// a runtime condition to branch on, and metrics must never fail the app.
    pub fn bump_counter(&self, key: &str, n: u64, today: &str) {
        debug_assert!(
            is_valid_counter_key(key),
            "counter keys are `<domain>.<action>` literals, never user-derived: {key:?}"
        );
        let mut g = self.lock();
        let t = Self::totals_for(&mut g, today);
        *t.counters.entry(key.to_string()).or_insert(0) =
            t.counters.get(key).copied().unwrap_or(0).saturating_add(n);
        g.dirty = true;
    }

    /// Folds one `ipc.result` into the day bucket: `cmd.<name>` duration plus, on
    /// an error outcome, the `errCode` tally. In-memory ONLY — never touches disk
    /// (so it is safe inside the no-IO `log_append` command).
    pub fn observe_ipc_result(&self, cmd: &str, ms: f64, err_code: Option<&str>, today: &str) {
        let mut g = self.lock();
        let t = Self::totals_for(&mut g, today);
        if is_valid_cmd_name(cmd) {
            t.durations
                .entry(format!("cmd.{cmd}"))
                .or_default()
                .observe(ms.max(0.0).round() as u64);
        }
        if let Some(code) = err_code {
            if is_valid_err_code(code) {
                *t.errors.entry(code.to_string()).or_insert(0) =
                    t.errors.get(code).copied().unwrap_or(0).saturating_add(1);
            }
        }
        g.dirty = true;
    }

    /// Folds one backend `span` into the day bucket: `op.<op>` total, allow-listed
    /// `op.<op>.<phase>` sub-keys, and `queue.blocking` for the queued delay.
    /// In-memory only.
    pub fn observe_span(
        &self,
        op: &str,
        ms: f64,
        phases: &[PhaseTiming],
        queued_ms: Option<u32>,
        today: &str,
    ) {
        let Some(allowed) = allowed_phases(op) else {
            return; // op not on the allow-list — drop it, no key created.
        };
        let mut g = self.lock();
        let t = Self::totals_for(&mut g, today);
        t.durations
            .entry(format!("op.{op}"))
            .or_default()
            .observe(ms.max(0.0).round() as u64);
        for p in phases {
            if allowed.contains(&p.name.as_str()) {
                t.durations
                    .entry(format!("op.{op}.{}", p.name))
                    .or_default()
                    .observe(p.ms.max(0.0).round() as u64);
            }
        }
        if let Some(q) = queued_ms {
            t.durations
                .entry("queue.blocking".to_string())
                .or_default()
                .observe(q as u64);
        }
        g.dirty = true;
    }

    /// Folds `PerfState::snapshot()` DELTAS into `perf.*` counters (§8). Monotone
    /// counters yield non-negative deltas; a perf reset (snapshot < baseline) is
    /// absorbed by `saturating_sub` and re-baselined.
    pub fn fold_perf(&self, snap: &PerfCounters, today: &str) {
        let mut g = self.lock();
        let baseline = g.perf_baseline.clone();
        let t = Self::totals_for(&mut g, today);
        for (key, get) in PERF_KEYS {
            let delta = get(snap).saturating_sub(get(&baseline));
            if delta > 0 {
                *t.counters.entry(key.to_string()).or_insert(0) = t
                    .counters
                    .get(key)
                    .copied()
                    .unwrap_or(0)
                    .saturating_add(delta);
            }
        }
        g.perf_baseline = snap.clone();
        g.dirty = true;
    }

    // ------------------------------------------------------------- flushing

    /// Folds perf deltas + wall time, then persists atomically if dirty. Blocking
    /// (writes a file). `now_secs` fixes both "today" and the wall-time delta.
    pub fn flush(&self, perf: &PerfCounters, now_secs: i64) -> Result<(), bonsai_core::error::AppError> {
        let today = writer::utc_date(now_secs);
        self.fold_perf(perf, &today);
        {
            let mut g = self.lock();
            let wall = (now_secs - g.last_wall_secs).max(0) as u64 * 1000;
            g.last_wall_secs = now_secs;
            if wall > 0 {
                let t = Self::totals_for(&mut g, &today);
                t.session_ms = t.session_ms.saturating_add(wall);
            }
        }
        let (path, file, dirty) = {
            let g = self.lock();
            (g.path.clone(), g.file.clone(), g.dirty)
        };
        let Some(path) = path else {
            return Ok(()); // not initialised (no config dir) — nothing to write.
        };
        if !dirty {
            return Ok(());
        }
        metrics_file::save(&path, &file)?;
        self.lock().dirty = false;
        Ok(())
    }

    /// Read API for the future Statistics page (§8). Returns a clone of the
    /// persisted file with every histogram's DERIVED `p50Ms`/`p95Ms` filled in —
    /// those fields are NEVER written to disk.
    pub fn snapshot(&self) -> MetricsFile {
        let mut file = self.lock().file.clone();
        for day in &mut file.days {
            derive_totals(&mut day.totals);
        }
        derive_totals(&mut file.lifetime);
        file
    }

    /// `metrics_reset()` — clears every aggregate to a fresh file and persists.
    /// Headless: exposed as a command but never surfaced in a settings catalog.
    pub fn reset(&self, now_secs: i64) -> Result<(), bonsai_core::error::AppError> {
        let path = {
            let mut g = self.lock();
            g.file = MetricsFile {
                schema: METRICS_SCHEMA_VERSION,
                first_seen: writer::utc_date(now_secs),
                sessions: 0,
                days: Vec::new(),
                lifetime: MetricTotals::default(),
            };
            g.perf_baseline = PerfCounters::default();
            g.last_wall_secs = now_secs;
            g.dirty = true;
            g.path.clone()
        };
        if let Some(path) = path {
            let file = self.lock().file.clone();
            metrics_file::save(&path, &file)?;
            self.lock().dirty = false;
        }
        Ok(())
    }
}

// ------------------------------------------------- process-wide feed handle

/// Process-wide handle to the managed [`MetricsState`], used ONLY by producers
/// that cannot reach an `AppHandle` — the [`super::phase::PhaseRecorder`] on the
/// blocking pool. Same rationale as `trace::ACTIVE_SINK`: threading an
/// `AppHandle` into the `spawn_blocking` git closures would force a
/// command-signature change (§2.2 forbids that).
static ACTIVE_METRICS: Mutex<Option<std::sync::Arc<MetricsState>>> = Mutex::new(None);

/// Installed at `setup` from the managed `Arc<MetricsState>`.
pub fn set_active(state: Option<std::sync::Arc<MetricsState>>) {
    *ACTIVE_METRICS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = state;
}

/// Feeds one completed span into the metrics store, if metrics is initialised.
/// Called from `PhaseRecorder::finish` alongside the sink enqueue; a no-op when
/// no store is active. Derives "today" from the wall clock.
pub fn observe_span_global(op: &str, ms: f64, phases: &[PhaseTiming], queued_ms: Option<u32>) {
    let state = ACTIVE_METRICS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if let Some(state) = state {
        let today = writer::utc_date(writer::now_secs());
        state.observe_span(op, ms, phases, queued_ms, &today);
    }
}

/// Fills the DERIVED percentile fields on every histogram in `totals`.
fn derive_totals(totals: &mut MetricTotals) {
    for h in totals.durations.values_mut() {
        *h = h.clone().with_derived_percentiles();
    }
}

#[cfg(test)]
#[path = "tests_metrics.rs"]
mod tests_metrics;
