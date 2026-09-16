//! Shared test harness + record builders for the anomaly-detector tests
//! (`tests_anomaly.rs` and `tests_anomaly_slow.rs`). No `#[test]`s live here.

use super::AnomalyDetector;
use crate::obs::record::{
    AnomalySeverity, FrameDim, IpcOutcome, LogLevel, LogPayload, LogRecord, LogSource, PhaseTiming,
};

/// A test harness: assigns monotone `seq`s (as the writer does) and accumulates
/// every anomaly the detector emits.
pub(super) struct H {
    det: AnomalyDetector,
    seq: u64,
    pub(super) out: Vec<LogRecord>,
}

impl H {
    pub(super) fn new() -> Self {
        H {
            det: AnomalyDetector::new(None),
            seq: 0,
            out: Vec::new(),
        }
    }

    /// Feeds one record, returns its assigned `seq`.
    pub(super) fn feed(&mut self, rec: LogRecord) -> u64 {
        self.seq += 1;
        let s = self.seq;
        let anomalies = self.det.observe(&rec, s);
        self.out.extend(anomalies);
        s
    }

    pub(super) fn batch_mark(&mut self) {
        let ts = self.out.last().map(|r| r.ts).unwrap_or(0);
        let anomalies = self.det.on_batch_mark(ts, self.seq + 1);
        self.out.extend(anomalies);
    }

    pub(super) fn end(&mut self) {
        let anomalies = self.det.on_session_end();
        self.out.extend(anomalies);
    }

    pub(super) fn count(&self, rule: &str) -> usize {
        self.out.iter().filter(|r| rule_of(r) == Some(rule)).count()
    }

    pub(super) fn find(&self, rule: &str) -> Option<&LogRecord> {
        self.out.iter().find(|r| rule_of(r) == Some(rule))
    }

    pub(super) fn detector(&mut self) -> &mut AnomalyDetector {
        &mut self.det
    }
}

pub(super) fn rule_of(r: &LogRecord) -> Option<&str> {
    match &r.payload {
        LogPayload::Anomaly { rule, .. } => Some(rule.as_str()),
        _ => None,
    }
}

pub(super) fn refs_of(r: &LogRecord) -> &[u64] {
    match &r.payload {
        LogPayload::Anomaly { refs, .. } => refs,
        _ => &[],
    }
}

pub(super) fn severity_of(r: &LogRecord) -> Option<AnomalySeverity> {
    match &r.payload {
        LogPayload::Anomaly { severity, .. } => Some(*severity),
        _ => None,
    }
}

// ---- record builders -------------------------------------------------------

pub(super) fn base(ts: i64, payload: LogPayload) -> LogRecord {
    LogRecord {
        seq: 0,
        ts,
        mono: 0,
        src: LogSource::Ui,
        lvl: LogLevel::Debug,
        trace: None,
        span: None,
        caused_by: None,
        payload,
    }
}

pub(super) fn with_trace(mut r: LogRecord, trace: &str) -> LogRecord {
    r.trace = Some(trace.to_string());
    r
}

pub(super) fn ipc_call(ts: i64, cmd: &str, hash: &str) -> LogRecord {
    base(
        ts,
        LogPayload::IpcCall {
            cmd: cmd.to_string(),
            args_hash: hash.to_string(),
            args_shape: None,
            args: None,
            args_omitted: None,
        },
    )
}

pub(super) fn ipc_result(ts: i64, cmd: &str, ms: f64) -> LogRecord {
    base(
        ts,
        LogPayload::IpcResult {
            cmd: cmd.to_string(),
            args_hash: "h".to_string(),
            ms,
            outcome: IpcOutcome::Ok,
            err_code: None,
            result_shape: None,
        },
    )
}

pub(super) fn ipc_result_hash(ts: i64, cmd: &str, hash: &str) -> LogRecord {
    base(
        ts,
        LogPayload::IpcResult {
            cmd: cmd.to_string(),
            args_hash: hash.to_string(),
            ms: 1.0,
            outcome: IpcOutcome::Ok,
            err_code: None,
            result_shape: None,
        },
    )
}

pub(super) fn refresh(ts: i64, scope: &str) -> LogRecord {
    base(
        ts,
        LogPayload::Refresh {
            round: 1,
            scope: scope.to_string(),
            origins: vec![],
            contributing_traces: vec![],
            collapsed: 0,
            ms: 1.0,
        },
    )
}

pub(super) fn effect(ts: i64, component: &str, effect: &str) -> LogRecord {
    base(
        ts,
        LogPayload::Effect {
            component: component.to_string(),
            effect: effect.to_string(),
            run: 1,
            changed_deps: vec![],
            dep_count: 0,
        },
    )
}

pub(super) fn event(ts: i64, name: &str) -> LogRecord {
    base(
        ts,
        LogPayload::Event {
            name: name.to_string(),
            reason: None,
            delivered: true,
            listeners: 1,
        },
    )
}

pub(super) fn watcher(ts: i64, fired: bool) -> LogRecord {
    base(
        ts,
        LogPayload::Watcher {
            paths: 1,
            relevant: 1,
            debounce_ms: 300,
            fired,
            suppressed: false,
            suppress_reason: None,
            burst_class: None,
        },
    )
}

pub(super) fn render_tally(ts: i64, component: &str, renders: u64, instances: u64) -> LogRecord {
    base(
        ts,
        LogPayload::RenderTally {
            component: component.to_string(),
            window_ms: 1000.0,
            renders,
            instances,
            changed_props: None,
            traces: vec![],
        },
    )
}

pub(super) fn frame(ts: i64, worst_ms: f64) -> LogRecord {
    base(
        ts,
        LogPayload::Frame {
            dim: FrameDim::Paint,
            paint_ms: worst_ms,
            gap_ms: 0.0,
            over33: 0,
            over100: 0,
            worst_ms,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn span(
    ts: i64,
    op: &str,
    ms: f64,
    phases: Option<Vec<(&str, f64)>>,
    queued_ms: Option<u32>,
    pool: Option<(u32, u32)>,
    deadline_frac: Option<f32>,
    cache: Option<&str>,
    outcome: Option<&str>,
) -> LogRecord {
    base(
        ts,
        LogPayload::Span {
            op: op.to_string(),
            ms,
            phases: phases.map(|ps| {
                ps.into_iter()
                    .map(|(n, m)| PhaseTiming {
                        name: n.to_string(),
                        ms: m,
                        n: None,
                    })
                    .collect()
            }),
            queued_ms,
            pool_inflight: pool.map(|(i, _)| i),
            pool_max: pool.map(|(_, m)| m),
            deadline_frac,
            cache: cache.map(str::to_string),
            items: None,
            outcome: outcome.map(str::to_string),
        },
    )
}
