//! P91 §5 — the cross-record anomaly detector.
//!
//! ONE concern: turning the unified record stream into derived `anomaly` records.
//! It runs on the sink's **writer thread** (`sink::writer_loop`), the only place
//! that sees both frontend records (arriving via `log_append`) and backend records
//! in `seq` order. As each `Record` message is written, [`AnomalyDetector::observe`]
//! is fed the record plus its just-assigned `seq`; any anomalies it returns are
//! written straight back into the same stream, each getting its own `seq`.
//!
//! **Redaction-independent by construction (§7.2 (c)).** Every rule keys off `cmd`
//! names, `argsHash`, scopes, component ids, counts and timings — never off repo
//! content and **never off a redaction ordinal**. A `strict` log yields byte-
//! identical anomaly output to a `raw` one; `tests_anomaly` asserts it.
//!
//! **Time base.** Every window and rate limit keys off the record's own `ts`
//! (wall-clock ms), never arrival/`Instant::now()` time: frontend records arrive
//! in 500 ms batches, so arrival time would collapse a real 300 ms `dup-ipc` gap
//! and stretch windows arbitrarily. `ts` is also the only clock shared across
//! sides (`mono` bases differ per side), which `jank-trace` needs.
//!
//! **Recursion guard.** Only original `Record` payloads are observed — never the
//! detector's own `anomaly` records, nor writer-minted `session`/`drop` lines
//! (`sink::writer_loop` never routes those through `observe`).
//!
//! **`truncate` is not an anomaly rule (§6.3).** No arm consumes it; the payload
//! variant does not even exist until increment 7. The catch-all `_ => {}` in
//! [`AnomalyDetector::observe`] is that guarantee, pinned by
//! `tests_anomaly::non_consumed_kinds_emit_nothing`.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use super::record::{AnomalySeverity, LogPayload, LogRecord, LogSource};

mod slow;
mod window;

use window::Sliding;

/// The widest window any rule keeps history for, used to bound the mutation
/// timeline (`cache-collapse` looks back 10 s).
const MAX_HISTORY_MS: i64 = 10_000;

/// FIFO cap on the open-call map so `orphan-trace` state stays bounded per §11.
const OPEN_CALLS_CAP: usize = 1024;

/// Session-scoped, in-memory, bounded (§11). Created once per writer thread.
///
/// "Bounded" is literal, and each piece of state pays for it differently:
/// `mutations` and every [`Sliding`]'s event list are pruned to their window;
/// `open_calls` is FIFO-capped at [`OPEN_CALLS_CAP`]; `slow` LRU-caps its
/// baseline map (§11's 200-`cmd`-key row); and each `Sliding::last_fire` debounce
/// map is pruned to the same window as its events — which is what keeps the claim
/// true for `dup-ipc`, the one rule whose key (`cmd\0argsHash`) is not drawn from
/// a finite catalogue.
#[derive(Default)]
pub struct AnomalyDetector {
    /// Wall-clock ts of every mutation-command `ipc.call` seen, pruned to
    /// `MAX_HISTORY_MS`. Shared by `dup-ipc`, `redundant-refresh`, `cache-collapse`.
    mutations: Vec<i64>,

    ipc_calls: Sliding,
    refreshes: Sliding,
    effects: Sliding,
    events: Sliding,
    watchers: Sliding,
    unbatched: Sliding,

    /// `orphan-trace`: traces of `ipc.call`s not yet answered by an `ipc.result`.
    /// FIFO-bounded at `OPEN_CALLS_CAP`.
    open_calls: Vec<(String, u64)>,

    /// §5.1 state (baseline histograms, saturation windows, recent spans).
    slow: slow::SlowState,

    /// Count of anomalies emitted this session, surfaced by `log_session_info`.
    anomalies: Option<Arc<AtomicU64>>,
}

impl AnomalyDetector {
    /// `anomalies` is the shared counter `log_session_info` reads; pass `None` in
    /// pure-unit tests that do not exercise it.
    pub fn new(anomalies: Option<Arc<AtomicU64>>) -> Self {
        AnomalyDetector {
            anomalies,
            ..Default::default()
        }
    }

    /// Feeds one written record (with its assigned `seq`) to every rule and
    /// returns the derived anomalies to append. Cheap and allocation-light on the
    /// common no-anomaly path.
    pub fn observe(&mut self, rec: &LogRecord, seq: u64) -> Vec<LogRecord> {
        let ts = rec.ts;
        let mut out: Vec<LogRecord> = Vec::new();
        match &rec.payload {
            LogPayload::IpcCall { cmd, args_hash, .. } => {
                // dup-ipc filters EXPLICITLY on this arm — the `ipc.call` kind —
                // NOT on `args_hash` happening to be absent elsewhere (§5, hard
                // requirement). A future payload gaining an `args_hash` therefore
                // cannot leak into this rule.
                if is_mutation_cmd(cmd) {
                    self.mutations.push(ts);
                }
                self.detect_dup_ipc(ts, seq, cmd, args_hash, &mut out);
                self.track_open_call(rec.trace.as_deref(), seq);
            }
            LogPayload::IpcResult { cmd, ms, .. } => {
                let trace = rec.trace.clone();
                self.close_open_call(trace.as_deref());
                self.slow
                    .on_ipc_result(ts, seq, cmd, *ms, trace.as_deref(), &mut out);
            }
            LogPayload::Refresh { scope, .. } => {
                self.detect_redundant_refresh(ts, seq, scope, &mut out);
            }
            LogPayload::Effect {
                component, effect, ..
            } => {
                self.detect_effect_thrash(ts, seq, component, effect, &mut out);
            }
            LogPayload::Event { name, .. } => {
                self.detect_event_storm(ts, seq, name, &mut out);
            }
            LogPayload::Watcher { fired, .. } => {
                if *fired {
                    self.detect_watcher_storm(ts, seq, &mut out);
                }
            }
            LogPayload::RenderTally {
                component,
                renders,
                instances,
                ..
            } => {
                if *renders > 3 * (*instances) {
                    out.push(self.anomaly(
                        "render-storm",
                        AnomalySeverity::Warn,
                        format!(
                            "{component}: {renders} renders vs {instances} instances in window"
                        ),
                        vec![seq],
                        traces_of(rec),
                        ts,
                    ));
                }
            }
            LogPayload::Frame { worst_ms, .. } => {
                // jank-trace is inert unless frame capture is on — and a `frame`
                // record only exists when it is (`dev.captureFrames`, or forced by
                // `level:'trace'`). Its mere presence is the enable signal, so no
                // separate flag is needed.
                self.slow.on_frame(ts, seq, *worst_ms, &mut out);
            }
            LogPayload::Span { .. } => {
                self.slow.on_span(ts, seq, rec, &self.mutations, &mut out);
            }
            // session / gesture / ipc.recv / render / state / error / channel /
            // anomaly / drop / (future) truncate — not consumed (§6.3).
            _ => {}
        }

        // Post-emit bookkeeping shared by several rules.
        self.prune(ts);
        self.bump_anomaly_count(out.len());
        out
    }

    /// TEST ONLY — the current size of the `slow-command` baseline map (§11 cap).
    #[cfg(test)]
    pub(super) fn baseline_len(&self) -> usize {
        self.slow.baseline_len()
    }

    /// Session-end pass (§5): every unanswered `ipc.call` is an `orphan-trace`.
    pub fn on_session_end(&mut self) -> Vec<LogRecord> {
        let now = super::writer::now_ms();
        let mut out = Vec::new();
        let open = std::mem::take(&mut self.open_calls);
        for (trace, seq) in open {
            out.push(self.anomaly(
                "orphan-trace",
                AnomalySeverity::Error,
                "ipc.call had no ipc.result".to_string(),
                vec![seq],
                vec![trace],
                now,
            ));
        }
        self.bump_anomaly_count(out.len());
        out
    }

    /// Adds `n` to the shared session anomaly counter (`log_session_info`).
    fn bump_anomaly_count(&self, n: usize) {
        if let Some(counter) = &self.anomalies {
            counter.fetch_add(n as u64, Ordering::Relaxed);
        }
    }

    fn track_open_call(&mut self, trace: Option<&str>, seq: u64) {
        let Some(trace) = trace else { return };
        self.open_calls.push((trace.to_string(), seq));
        // FIFO cap: an unbounded map of unanswered calls would grow for the whole
        // session (§11 "bounded").
        if self.open_calls.len() > OPEN_CALLS_CAP {
            let over = self.open_calls.len() - OPEN_CALLS_CAP;
            self.open_calls.drain(..over);
        }
    }

    fn close_open_call(&mut self, trace: Option<&str>) {
        let Some(trace) = trace else { return };
        if let Some(pos) = self.open_calls.iter().position(|(t, _)| t == trace) {
            self.open_calls.remove(pos);
        }
    }

    fn prune(&mut self, now: i64) {
        let cutoff = now - MAX_HISTORY_MS;
        self.mutations.retain(|&m| m >= cutoff);
        self.slow.prune(now);
    }

    fn anomaly(
        &self,
        rule: &str,
        severity: AnomalySeverity,
        detail: String,
        refs: Vec<u64>,
        traces: Vec<String>,
        ts: i64,
    ) -> LogRecord {
        build_anomaly(rule, severity, detail, refs, traces, ts)
    }
}

/// Builds one `anomaly` record. `seq`/`mono` are left 0 because THIS builder has
/// neither counter: the sink's writer assigns both in `LogWriter::append_record`
/// — `seq` from its running ordinal, `mono` from the session `Instant` it
/// captured at open (it stamps any `src: "rust"` record that arrives at 0).
/// So an `anomaly` line on disk carries a real session-relative `mono`.
pub(super) fn build_anomaly(
    rule: &str,
    severity: AnomalySeverity,
    detail: String,
    refs: Vec<u64>,
    traces: Vec<String>,
    ts: i64,
) -> LogRecord {
    LogRecord {
        seq: 0,
        ts,
        mono: 0,
        src: LogSource::Rust,
        lvl: match severity {
            AnomalySeverity::Error => super::record::LogLevel::Error,
            AnomalySeverity::Warn => super::record::LogLevel::Warn,
            AnomalySeverity::Info => super::record::LogLevel::Info,
        },
        trace: None,
        span: None,
        caused_by: None,
        payload: LogPayload::Anomaly {
            rule: rule.to_string(),
            severity,
            detail,
            refs,
            traces,
        },
    }
}

/// The `traces` list for an anomaly implicating a single record: its own trace,
/// if any.
fn traces_of(rec: &LogRecord) -> Vec<String> {
    rec.trace.iter().cloned().collect()
}

/// Repo-mutating commands (§5 "no intervening mutation"; §5.1 `cache-collapse`).
/// One shared table so `dup-ipc`, `redundant-refresh` and `cache-collapse` cannot
/// disagree about what a mutation is. Prefix match — the command catalogue is
/// consistently `<verb>_<noun>` and every verb here only names writes.
pub fn is_mutation_cmd(cmd: &str) -> bool {
    const MUTATION_PREFIXES: &[&str] = &[
        "commit",
        "stage",
        "unstage",
        "discard",
        "checkout",
        "create_branch",
        "delete_branch",
        "create_tag",
        "delete_tag",
        "create_stash",
        "apply_stash",
        "drop_stash",
        "merge",
        "commit_merge",
        "abort_merge",
        "rebase",
        "cherrypick",
        "revert",
        "reset",
        "fetch",
        "pull",
        "push",
        "force_push",
        "clone_repo",
        "init_repo",
        "add_remote",
        "remove_remote",
        "rename_remote",
        "add_submodule",
        "deinit_submodule",
        "init_submodule",
        "add_worktree",
        "bisect_",
        "force_refresh_tag",
        "auto_sync_tags",
        "delete_remote",
        "apply_composed_commits",
        "apply_identity_profile",
    ];
    MUTATION_PREFIXES.iter().any(|p| cmd.starts_with(p))
}

#[cfg(test)]
#[path = "tests_anomaly_support.rs"]
mod tests_anomaly_support;

#[cfg(test)]
#[path = "tests_anomaly.rs"]
mod tests_anomaly;

#[cfg(test)]
#[path = "tests_anomaly_slow.rs"]
mod tests_anomaly_slow;

#[cfg(test)]
#[path = "tests_histogram.rs"]
mod tests_histogram;
