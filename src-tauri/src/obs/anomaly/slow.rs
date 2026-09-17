//! P91 §5.1 — the duration & saturation rules, factored out of `anomaly.rs` to
//! keep each file focused and under the size limit.
//!
//! Rules here: `slow-command` (self-calibrating per `cmd`), `slow-phase`,
//! `queue-delay`, `pool-saturation`, `watchdog-pressure`, `cache-collapse`, and
//! `jank-trace` (frame → overlapping span attribution). All of them key strictly
//! off `cmd` names, timings and cache/pool counts — never off repo content or a
//! redaction ordinal (§7.2 (c)).

use super::build_anomaly;
use crate::obs::histogram::Histogram;
use crate::obs::record::{AnomalySeverity, LogPayload, LogRecord};

/// Minimum samples before `slow-command`'s calibrated branch is trusted (§5.1).
const MIN_SAMPLES: u64 = 20;
/// Absolute catch-all: fires even before `MIN_SAMPLES`, severity `error` (§5.1).
const HARD_MS: f64 = 10_000.0;
/// At most one `slow-command` per `cmd` per this window (§5.1).
const SLOW_RATE_MS: i64 = 10_000;
/// Cap on the per-`cmd` baseline map; LRU eviction beyond it (§11).
const BASELINE_CAP: usize = 200;

/// `slow-command` constants, in ONE table (§5.1). Prefix match on `cmd`; the
/// most specific (longest) matching prefix wins, with the `""` default row as the
/// floor. `k` multiplies the rolling p95; `floor_ms` is the absolute floor below
/// which a command is never "slow" regardless of baseline.
pub const SLOW_RULES: &[(&str, f64, f64)] = &[
    ("", 150.0, 3.0),           // default row
    ("get_graph", 1200.0, 3.0), // a 20k-commit walk is legitimately costly
    ("commit_create", 2000.0, 3.0),
];

fn slow_rule_for(cmd: &str) -> (f64, f64) {
    SLOW_RULES
        .iter()
        .filter(|(prefix, _, _)| cmd.starts_with(prefix))
        .max_by_key(|(prefix, _, _)| prefix.len())
        .map(|(_, floor, k)| (*floor, *k))
        .unwrap_or((150.0, 3.0))
}

/// `queue-delay` / `pool-saturation` windows (§5).
const W_SATURATION_MS: i64 = 5_000;
/// `cache-collapse` window (§5.1).
const W_CACHE_MS: i64 = 10_000;
/// `slow-phase` keeps recent spans this long to correlate with a late `ipc.result`.
const W_SPAN_MS: i64 = 10_000;

const QUEUE_DELAY_MIN: usize = 3;
const QUEUE_MS_THRESHOLD: u32 = 100;
const POOL_SATURATION_MIN: usize = 3;
const CACHE_COLLAPSE_MIN: usize = 5;
const CACHE_HIT_FLOOR: f64 = 0.2;
const DEADLINE_FRAC_THRESHOLD: f32 = 0.8;
const SLOW_PHASE_SHARE: f64 = 0.70;
const JANK_FRAME_MS: f64 = 100.0;

/// A recently-completed span, kept so a `slow-command` on the same `trace` can
/// attribute its cost to a phase (`slow-phase`).
struct SpanInfo {
    trace: Option<String>,
    seq: u64,
    ts: i64,
    ms: f64,
    /// `(phase_name, ms)` — allow-listed names only, never user-derived.
    phases: Vec<(String, f64)>,
}

/// A graph.get cache outcome inside the `cache-collapse` window.
struct CacheSample {
    ts: i64,
    seq: u64,
    /// `hit` | `miss` | `redecorate` (from `span.cache`).
    kind: String,
}

/// Per-`cmd` rolling baseline plus its LRU recency stamp (session `seq`).
struct Baseline {
    cmd: String,
    hist: Histogram,
    last_seq: u64,
}

#[derive(Default)]
pub(super) struct SlowState {
    baselines: Vec<Baseline>,
    slow_last_fire: std::collections::HashMap<String, i64>,
    spans: Vec<SpanInfo>,
    queue: Vec<(i64, u64)>,
    pool: Vec<(i64, u64)>,
    cache: Vec<CacheSample>,
    queue_last_fire: Option<i64>,
    pool_last_fire: Option<i64>,
    cache_last_fire: Option<i64>,
}

impl SlowState {
    /// §5.1 `slow-command` (+ the `slow-phase` it triggers). Follows the contract
    /// pseudocode exactly: compare against the baseline, THEN observe.
    pub(super) fn on_ipc_result(
        &mut self,
        ts: i64,
        seq: u64,
        cmd: &str,
        ms: f64,
        trace: Option<&str>,
        out: &mut Vec<LogRecord>,
    ) {
        let (floor, k) = slow_rule_for(cmd);
        let hist = self.baseline(cmd, seq);

        // Decide severity BEFORE observing this sample.
        let mut fire: Option<AnomalySeverity> = None;
        let mut p95_detail: Option<u32> = None;
        if ms > HARD_MS {
            fire = Some(AnomalySeverity::Error);
        } else if hist.count >= MIN_SAMPLES {
            if let Some(p95) = hist.percentile_ms(0.95) {
                let threshold = floor.max(k * p95 as f64);
                if ms > threshold {
                    fire = Some(AnomalySeverity::Warn);
                    p95_detail = Some(p95);
                }
            }
        }
        let samples = hist.count;

        // Rate limit: ≤1 slow-command per cmd per 10 s (shared by both branches).
        if let Some(severity) = fire {
            if self.rate_ok(cmd, ts) {
                out.push(build_anomaly(
                    "slow-command",
                    severity,
                    match p95_detail {
                        Some(p95) => format!(
                            "{cmd}: {ms:.0}ms > max(floor {floor:.0}, {k}×p95 {p95}) over {samples} samples"
                        ),
                        None => format!("{cmd}: {ms:.0}ms exceeds hard cap {HARD_MS:.0}ms"),
                    },
                    vec![seq],
                    trace.into_iter().map(str::to_string).collect(),
                    ts,
                ));
                self.detect_slow_phase(ts, seq, trace, out);
            }
        }

        // observe AFTER comparing (§5.1).
        self.baseline(cmd, seq).observe(ms.max(0.0) as u64);
    }

    /// §5.1 `slow-phase`: when a slow-command fires, if the correlated span (same
    /// `trace`) has a phase ≥70 % of its `ms`, emit `slow-phase` referencing both.
    fn detect_slow_phase(
        &self,
        ts: i64,
        result_seq: u64,
        trace: Option<&str>,
        out: &mut Vec<LogRecord>,
    ) {
        let Some(trace) = trace else { return };
        let span = self
            .spans
            .iter()
            .rev()
            .find(|s| s.trace.as_deref() == Some(trace) && !s.phases.is_empty());
        let Some(span) = span else { return };
        if span.ms <= 0.0 {
            return;
        }
        if let Some((phase, pms)) = span
            .phases
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .filter(|(_, pms)| *pms >= SLOW_PHASE_SHARE * span.ms)
        {
            let share = pms / span.ms;
            out.push(build_anomaly(
                "slow-phase",
                AnomalySeverity::Info,
                format!(
                    "phase {phase} = {pms:.0}ms ({:.0}% of {:.0}ms)",
                    share * 100.0,
                    span.ms
                ),
                vec![span.seq, result_seq],
                vec![trace.to_string()],
                ts,
            ));
        }
    }

    /// §3.1 span → saturation + cache rules.
    pub(super) fn on_span(
        &mut self,
        ts: i64,
        seq: u64,
        rec: &LogRecord,
        _mutations: &[i64],
        out: &mut Vec<LogRecord>,
    ) {
        let LogPayload::Span {
            op,
            ms,
            phases,
            queued_ms,
            pool_inflight,
            pool_max,
            deadline_frac,
            cache,
            outcome,
            ..
        } = &rec.payload
        else {
            return;
        };

        // Retain for slow-phase correlation.
        self.spans.push(SpanInfo {
            trace: rec.trace.clone(),
            seq,
            ts,
            ms: *ms,
            phases: phases
                .as_ref()
                .map(|ps| ps.iter().map(|p| (p.name.clone(), p.ms)).collect())
                .unwrap_or_default(),
        });

        // queue-delay
        if queued_ms.map(|q| q > QUEUE_MS_THRESHOLD).unwrap_or(false) {
            self.queue.push((ts, seq));
            self.queue.retain(|(t, _)| *t >= ts - W_SATURATION_MS);
            if self.queue.len() >= QUEUE_DELAY_MIN
                && rate_ok(&mut self.queue_last_fire, ts, W_SATURATION_MS)
            {
                let refs: Vec<u64> = self.queue.iter().map(|(_, s)| *s).collect();
                out.push(build_anomaly(
                    "queue-delay",
                    AnomalySeverity::Warn,
                    format!(
                        "{} spans queued > {QUEUE_MS_THRESHOLD}ms within {W_SATURATION_MS}ms",
                        refs.len()
                    ),
                    refs,
                    Vec::new(),
                    ts,
                ));
            }
        }

        // pool-saturation
        if let (Some(inflight), Some(max)) = (pool_inflight, pool_max) {
            if inflight >= max {
                self.pool.push((ts, seq));
                self.pool.retain(|(t, _)| *t >= ts - W_SATURATION_MS);
                if self.pool.len() >= POOL_SATURATION_MIN
                    && rate_ok(&mut self.pool_last_fire, ts, W_SATURATION_MS)
                {
                    let refs: Vec<u64> = self.pool.iter().map(|(_, s)| *s).collect();
                    out.push(build_anomaly(
                        "pool-saturation",
                        AnomalySeverity::Warn,
                        format!(
                            "{} spans at pool cap {max} within {W_SATURATION_MS}ms",
                            refs.len()
                        ),
                        refs,
                        Vec::new(),
                        ts,
                    ));
                }
            }
        }

        // watchdog-pressure (no window: fires per qualifying span)
        let timeout = outcome.as_deref() == Some("timeout");
        if timeout
            || deadline_frac
                .map(|f| f >= DEADLINE_FRAC_THRESHOLD)
                .unwrap_or(false)
        {
            let severity = if timeout {
                AnomalySeverity::Error
            } else {
                AnomalySeverity::Warn
            };
            let frac = deadline_frac.unwrap_or(1.0);
            out.push(build_anomaly(
                "watchdog-pressure",
                severity,
                format!(
                    "{op}: deadlineFrac {frac:.2}{}",
                    if timeout { " (timeout)" } else { "" }
                ),
                vec![seq],
                rec.trace.iter().cloned().collect(),
                ts,
            ));
        }

        // cache-collapse (graph.get spans only)
        if op == "graph.get" {
            if let Some(kind) = cache {
                self.cache.push(CacheSample {
                    ts,
                    seq,
                    kind: kind.clone(),
                });
                self.cache.retain(|c| c.ts >= ts - W_CACHE_MS);
                self.detect_cache_collapse(ts, _mutations, out);
            }
        }
    }

    fn detect_cache_collapse(&mut self, ts: i64, mutations: &[i64], out: &mut Vec<LogRecord>) {
        if self.cache.len() < CACHE_COLLAPSE_MIN {
            return;
        }
        // A real mutation legitimately invalidates the cache — suppress then.
        let window_start = ts - W_CACHE_MS;
        if mutations.iter().any(|&m| m >= window_start && m <= ts) {
            return;
        }
        let mut hits = 0usize;
        let mut total = 0usize;
        for c in &self.cache {
            total += 1;
            if c.kind == "hit" {
                hits += 1;
            }
        }
        let hit_rate = hits as f64 / total as f64;
        if hit_rate < CACHE_HIT_FLOOR && rate_ok(&mut self.cache_last_fire, ts, W_CACHE_MS) {
            let refs: Vec<u64> = self.cache.iter().map(|c| c.seq).collect();
            out.push(build_anomaly(
                "cache-collapse",
                AnomalySeverity::Warn,
                format!(
                    "graph cache hit rate {:.0}% over {total} spans, no intervening mutation",
                    hit_rate * 100.0
                ),
                refs,
                Vec::new(),
                ts,
            ));
        }
    }

    /// §5 `jank-trace`: a worst frame > 100 ms is attributed to every trace whose
    /// span overlaps the frame's timestamp.
    pub(super) fn on_frame(&self, ts: i64, seq: u64, worst_ms: f64, out: &mut Vec<LogRecord>) {
        if worst_ms <= JANK_FRAME_MS {
            return;
        }
        let mut refs = vec![seq];
        let mut traces: Vec<String> = Vec::new();
        for s in &self.spans {
            let start = s.ts - s.ms.round() as i64;
            if ts >= start && ts <= s.ts {
                refs.push(s.seq);
                if let Some(t) = &s.trace {
                    if !traces.contains(t) {
                        traces.push(t.clone());
                    }
                }
            }
        }
        out.push(build_anomaly(
            "jank-trace",
            AnomalySeverity::Warn,
            format!(
                "worst frame {worst_ms:.0}ms overlapped {} span(s)",
                refs.len() - 1
            ),
            refs,
            traces,
            ts,
        ));
    }

    pub(super) fn prune(&mut self, now: i64) {
        self.spans.retain(|s| s.ts >= now - W_SPAN_MS);
        self.queue.retain(|(t, _)| *t >= now - W_SATURATION_MS);
        self.pool.retain(|(t, _)| *t >= now - W_SATURATION_MS);
        self.cache.retain(|c| c.ts >= now - W_CACHE_MS);
    }

    /// Per-`cmd` rate limit (1 per `SLOW_RATE_MS`). Stamps on success.
    fn rate_ok(&mut self, cmd: &str, ts: i64) -> bool {
        match self.slow_last_fire.get(cmd) {
            Some(&prev) if ts - prev < SLOW_RATE_MS => false,
            _ => {
                self.slow_last_fire.insert(cmd.to_string(), ts);
                true
            }
        }
    }

    /// Returns the (mutable) baseline histogram for `cmd`, creating it and
    /// evicting the least-recently-used key when the map is at `BASELINE_CAP`.
    fn baseline(&mut self, cmd: &str, seq: u64) -> &mut Histogram {
        if let Some(pos) = self.baselines.iter().position(|b| b.cmd == cmd) {
            self.baselines[pos].last_seq = seq;
            return &mut self.baselines[pos].hist;
        }
        if self.baselines.len() >= BASELINE_CAP {
            if let Some((lru, _)) = self
                .baselines
                .iter()
                .enumerate()
                .min_by_key(|(_, b)| b.last_seq)
                .map(|(i, b)| (i, b.last_seq))
            {
                self.baselines.swap_remove(lru);
            }
        }
        self.baselines.push(Baseline {
            cmd: cmd.to_string(),
            hist: Histogram::default(),
            last_seq: seq,
        });
        let last = self.baselines.len() - 1;
        &mut self.baselines[last].hist
    }

    #[cfg(test)]
    pub(super) fn baseline_len(&self) -> usize {
        self.baselines.len()
    }
}

/// Shared single-slot rate limiter for the windowed saturation/cache rules.
fn rate_ok(last: &mut Option<i64>, ts: i64, window: i64) -> bool {
    match *last {
        Some(prev) if ts - prev < window => false,
        _ => {
            *last = Some(ts);
            true
        }
    }
}
