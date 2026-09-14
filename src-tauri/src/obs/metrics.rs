//! P91 §8 — durable local metrics: rolled-up daily counters, duration
//! histograms and error tallies, persisted to `metrics/usage.json`.
//!
//! **Two structural guarantees this module exists to keep:**
//! 1. *No key is user-derived.* Every metric key comes from the fixed allow-list
//!    below (`<domain>.<action>`), or from a command name validated to be a bare
//!    code identifier. Repo content — paths, refs, messages — can never become a
//!    key, so metrics need no redaction pass. They ARE in the
//!    `logs_delete_all` scope since §F6 — deletability is a remedy, not a reason
//!    to relax the key guard that stops a user-derived key being written at all.
//! 2. *No network sink.* Nothing here reaches an HTTP client; a test asserts the
//!    whole `obs/` tree names no HTTP-client crate at all.
//!
//! The percentiles (`p50Ms`/`p95Ms`) are DERIVED at [`MetricsState::snapshot`]
//! time from the frozen [`Histogram`] buckets and are **never persisted** —
//! `usage.json` stores only the 8 bucket counters + aggregates, so its size is
//! independent of how many observations folded through (§8.1).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::obs::histogram::Histogram;
use crate::obs::record::PhaseTiming;
use crate::obs::writer;
use crate::perf::PerfCounters;

use super::metrics_cmds::is_known_cmd;
use super::metrics_file;
use super::metrics_keys::{is_valid_cmd_name, is_valid_counter_key, is_valid_err_code};
use super::metrics_map;

/// Current on-disk metrics schema. Bumped only if an EXISTING field changes
/// shape; additive growth keeps it at 1 (mirrors `OBS_SCHEMA_VERSION`).
pub const METRICS_SCHEMA_VERSION: u32 = 1;

/// Daily buckets kept in full before the oldest folds into `lifetime` (§8).
///
/// §F6 (user ruling 2026-09-11): 400 → **90**, and enforced by calendar AGE
/// (`metrics_clear::prune_days`), not by bucket count — the app is not used every
/// day, so 90 buckets can span years while the signed privacy copy says
/// "90 days". This is the single home for the window; no other module may
/// hard-code one.
pub const RETAIN_DAYS: usize = 90;

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
    /// Retained daily buckets, oldest first. Length ≤ [`RETAIN_DAYS`], and no
    /// bucket is older than [`RETAIN_DAYS`] calendar days (§F6 §3.2).
    pub days: Vec<DayBucket>,
    /// Everything older than [`RETAIN_DAYS`], folded together. A LIFETIME figure:
    /// the window never ages it out (§F6 §3.5); only a clear removes it.
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
    /// Folds `other` into `self` — used for the 90-day → `lifetime` roll-up.
    ///
    /// Goes through `metrics_map` like every other writer: `lifetime` accumulates
    /// every folded bucket's key set without bound, so it is the map most able to
    /// grow past the cardinality cap (audit F3).
    fn merge(&mut self, other: &MetricTotals) {
        for (k, v) in &other.counters {
            metrics_map::bump(&mut self.counters, k, *v);
        }
        for (k, h) in &other.durations {
            metrics_map::merge_histogram(&mut self.durations, k, h);
        }
        for (k, v) in &other.errors {
            metrics_map::bump(&mut self.errors, k, *v);
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
    /// Monotone revision of `file`, bumped by [`MetricsState::mark_dirty`] on
    /// every accepted mutation. A saver stamps the revision its snapshot was
    /// taken at, so a snapshot that lost the race to a newer one is recognisable
    /// at commit time (see `metrics_persist.rs`) instead of overwriting it.
    rev: u64,
    /// Epoch secs of the last `session_ms` attribution, for wall-time deltas.
    last_wall_secs: i64,
    /// §F6 §3.4 — in-memory ONLY, never serialized. Set when `prune_days` folded
    /// at least one bucket at load; cleared by the first `persist()` that commits.
    /// While true, `usage.json.bak` may still hold PRE-window data: `save_locked`
    /// rotates the good primary aside before committing, so the migration's own
    /// save is what moves a 400-day file into `.bak`, where `load` would recover
    /// it — silently restoring a profile the user was told is 90 days.
    bak_stale: bool,
}

/// Managed observability-metrics state (§8). Always present — unlike the Dev-mode
/// log sink, metrics accumulate `perf.*` counters and `sessions` on every launch;
/// the `op.*`/`cmd.*` duration histograms fill only during Dev-mode sessions
/// (their source `span`/`ipc.result` records exist only then — §11 keeps span
/// recording zero-cost when Dev mode is off).
pub struct MetricsState {
    inner: Mutex<Inner>,
    /// Highest [`Inner::rev`] whose bytes have been committed to disk by THIS
    /// store. Read and written only under `metrics_file`'s `SAVE_LOCK`, which is
    /// what makes the compare-then-commit atomic between savers; it is an atomic
    /// rather than an `Inner` field precisely so the commit path never nests the
    /// state mutex inside `SAVE_LOCK`.
    commit_rev: AtomicU64,
}

impl Default for MetricsState {
    fn default() -> Self {
        MetricsState {
            commit_rev: AtomicU64::new(0),
            inner: Mutex::new(Inner {
                path: None,
                file: MetricsFile {
                    schema: METRICS_SCHEMA_VERSION,
                    ..Default::default()
                },
                perf_baseline: PerfCounters::default(),
                dirty: false,
                rev: 0,
                last_wall_secs: 0,
                bak_stale: false,
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

    /// Marks `file` unsaved AND advances its revision. Every writer goes through
    /// here: the revision is what lets a saver tell "my snapshot is the newest"
    /// from "someone committed a newer one while I was serializing", so a writer
    /// that set `dirty` without bumping `rev` would be invisible to that check.
    fn mark_dirty(inner: &mut Inner) {
        inner.dirty = true;
        inner.rev = inner.rev.saturating_add(1);
    }

    /// Loads `usage.json` (with `.bak` recovery) from `dir`, bumps `sessions` and
    /// sets `first_seen` if unset. Blocking (reads a file) — call on the blocking
    /// pool. `now_secs` fixes the launch date deterministically for tests.
    pub fn init(&self, dir: PathBuf, now_secs: i64) {
        let path = dir.join("usage.json");
        let (file, bak_stale) = metrics_clear::load_pruned(&path, now_secs);
        let mut g = self.lock();
        g.path = Some(path);
        g.file = file;
        g.perf_baseline = PerfCounters::default();
        g.last_wall_secs = now_secs;
        g.bak_stale = bak_stale;
        Self::mark_dirty(&mut g);
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
        let (file, bak_stale) = metrics_clear::load_pruned(&path, now_secs);
        {
            let mut g = state.lock();
            g.path = Some(path);
            g.file = file;
            g.last_wall_secs = now_secs;
            g.bak_stale = bak_stale;
            Self::mark_dirty(&mut g);
        }
        state
    }

    // ---------------------------------------------------------- observation

    /// Returns today's day bucket, creating it (and folding everything outside
    /// the [`RETAIN_DAYS`] window into `lifetime`) on a date change.
    ///
    /// §F6 §3.3 — this is the SECOND retention trigger. `init` prunes at load and
    /// is the migration, but a long-running session never re-hits it, so a session
    /// running across midnight would otherwise keep buckets past the window.
    ///
    /// **Append-only clock assumption:** `today` is expected to be monotone
    /// non-decreasing across a session (it comes from the wall clock). Only the
    /// LAST bucket is compared, so a UTC step *backwards* mid-session would append
    /// a duplicate bucket for an older date. Real clock skew is sub-second and
    /// harmless here; a backwards step across a day boundary is not a case the §8
    /// contract addresses.
    fn totals_for<'a>(inner: &'a mut Inner, today: &str) -> &'a mut MetricTotals {
        if inner.file.days.last().map(|d| d.date.as_str()) != Some(today) {
            // A new day started. Enforce retention BEFORE pushing, AGE first: fold
            // every bucket outside the calendar window into `lifetime`.
            metrics_clear::prune_days_for(&mut inner.file, today);
            // BACKSTOP ONLY (§F6 §3.3). After an age prune the length is
            // <= RETAIN_DAYS - 1 whenever today's bucket is absent, so this is
            // inert in every healthy case; it survives for a clock regression or a
            // hand-edited file, where it still bounds the vec.
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

    /// The ONE way a `counters` key is minted. Every counter writer — the public
    /// [`Self::bump_counter`] and [`Self::fold_perf`] alike — goes through here,
    /// so [`is_valid_counter_key`] is not a guard one path can walk around.
    ///
    /// It takes the already-locked totals rather than `&self` because `fold_perf`
    /// holds the mutex across its whole loop; re-entering `bump_counter` there
    /// would deadlock on the non-reentrant [`Mutex`].
    ///
    /// The guard is a REAL `if`, not a `debug_assert` (audit F2):
    /// `debug-assertions` is off in release and this crate sets no
    /// `[profile.release]` override, so an assert would vanish from the shipped
    /// binary and let a key carrying a path separator, whitespace or an uppercase
    /// letter — i.e. a branch name, a path or a ref — be persisted verbatim into
    /// `usage.json`, a file that carries NO user content by construction and has
    /// no redaction pass (§8, decision 25). It IS deletable since §F6, which does
    /// not weaken this guard at all: a bad key would still be written unredacted
    /// and would still survive until the user chose to delete.
    /// A rejected key DROPS the observation, silently and in every profile —
    /// identical to `observe_ipc_result`'s two guards; metrics never fail the app.
    /// Returns true iff the observation was recorded, so callers only mark the
    /// file dirty when something actually changed.
    #[must_use]
    fn bump_validated(t: &mut MetricTotals, key: &str, n: u64) -> bool {
        if !is_valid_counter_key(key) {
            return false;
        }
        metrics_map::bump(&mut t.counters, key, n);
        true
    }

    /// Increments a counter key. The caller is expected to pass an allow-listed
    /// `<domain>.<action>` literal; a key failing [`is_valid_counter_key`] is
    /// dropped by [`Self::bump_validated`].
    ///
    /// The only production counter writer today is [`Self::fold_perf`], which
    /// shares the same validated sink. This entry point stays because it is the
    /// intended door for future `<domain>.<action>` counters.
    pub fn bump_counter(&self, key: &str, n: u64, today: &str) {
        let mut g = self.lock();
        let t = Self::totals_for(&mut g, today);
        if Self::bump_validated(t, key, n) {
            Self::mark_dirty(&mut g);
        }
    }

    /// Folds one `ipc.result` into the day bucket: `cmd.<name>` duration plus, on
    /// an error outcome, the `errCode` tally. In-memory ONLY — never touches disk
    /// (so it is safe inside the no-IO `log_append` command).
    pub fn observe_ipc_result(&self, cmd: &str, ms: f64, err_code: Option<&str>, today: &str) {
        // `cmd` arrives from the WEBVIEW (`log_append` takes its records verbatim),
        // so two independent guards apply before it can mint a durable key:
        // the shape predicate, and — authoritatively — membership in the real
        // `IpcApi` method set (audit F3). Shape alone bounds nothing: a buggy or
        // hostile frontend can mint unlimited well-shaped names.
        let known = is_valid_cmd_name(cmd) && is_known_cmd(cmd);
        let mut g = self.lock();
        let t = Self::totals_for(&mut g, today);
        if known {
            metrics_map::observe(
                &mut t.durations,
                &format!("cmd.{cmd}"),
                ms.max(0.0).round() as u64,
            );
        }
        if let Some(code) = err_code {
            if is_valid_err_code(code) {
                metrics_map::bump(&mut t.errors, code, 1);
            }
        }
        Self::mark_dirty(&mut g);
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
        metrics_map::observe(
            &mut t.durations,
            &format!("op.{op}"),
            ms.max(0.0).round() as u64,
        );
        for p in phases {
            if allowed.contains(&p.name.as_str()) {
                metrics_map::observe(
                    &mut t.durations,
                    &format!("op.{op}.{}", p.name),
                    p.ms.max(0.0).round() as u64,
                );
            }
        }
        if let Some(q) = queued_ms {
            metrics_map::observe(&mut t.durations, "queue.blocking", q as u64);
        }
        Self::mark_dirty(&mut g);
    }

    /// Folds `PerfState::snapshot()` DELTAS into `perf.*` counters (§8). Monotone
    /// counters yield non-negative deltas; a perf reset (snapshot < baseline) is
    /// absorbed by `saturating_sub` and re-baselined.
    pub fn fold_perf(&self, snap: &PerfCounters, today: &str) {
        let mut g = self.lock();
        let baseline = g.perf_baseline.clone();
        let t = Self::totals_for(&mut g, today);
        let mut recorded = false;
        for (key, get) in PERF_KEYS {
            let delta = get(snap).saturating_sub(get(&baseline));
            if delta > 0 {
                // Same validated sink as `bump_counter` (audit F2 follow-up): the
                // `PERF_KEYS` literals are const today, but the guard must not be
                // reachable only from the entry point with no production caller.
                recorded |= Self::bump_validated(t, key, delta);
            }
        }
        // Re-baselining alone changes nothing PERSISTED (`perf_baseline` lives in
        // `Inner`, not in `MetricsFile`), so a delta-free fold must not dirty the
        // file: `flush` calls this every 60 s, and marking dirty unconditionally
        // would rewrite `usage.json` forever and make the `dirty` flag meaningless
        // as a "something is unsaved" signal.
        g.perf_baseline = snap.clone();
        if recorded {
            Self::mark_dirty(&mut g);
        }
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

/// §F6 — the 90-day window and the user-driven clear.
#[path = "metrics_clear.rs"]
mod metrics_clear;

/// §8 persistence — `flush`/`reset` and the snapshot-ordering rule they share.
#[path = "metrics_persist.rs"]
mod metrics_persist;

#[cfg(test)]
#[path = "tests_metrics.rs"]
mod tests_metrics;
