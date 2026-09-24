//! P120 §2.5 — the two dev-mode stability rules, `unprompted-shift` and
//! `focus-steal`.
//!
//! ONE concern: correlating the frontend's `layout.shift` / `focus.lost`
//! records with the `refresh` and `watcher` rounds that may have caused them.
//! `unprompted-shift` is stateless (the frontend already buckets 1 s per
//! region). `focus-steal` keeps two bounded queues (§11): recent causes
//! (≤[`CAUSE_HISTORY_CAP`], [`CAUSE_HISTORY_MS`]) and losses held while the
//! settling `refresh` may still arrive (≤[`FOCUS_PENDING_CAP`],
//! [`FOCUS_HOLD_MS`]). Every `focus.lost` yields exactly one `focus-steal`:
//! `warn` when a cause matched, `info` when none did by expiry / session end.
//!
//! **Redaction-independent.** Keys only on the closed region/phase/via enums,
//! booleans, `ts`/`lostAt`/`ms` and trace ids — never `repo`, `scope`,
//! origins or ordinals. `detail` is built only from enum `as_str()` values and
//! numbers, so it never holds `/`, `\` or `://`.

use std::collections::VecDeque;

use super::build_anomaly;
use crate::obs::record::{
    AnomalySeverity, FocusLostPayload, LayoutShiftPayload, LogPayload, LogRecord, StabilityPhase,
};

pub(super) const FOCUS_WATCHER_LOOKBACK_MS: i64 = 1_000;
/// Loss just before the round's measured start.
pub(super) const REFRESH_PRE_SLACK_MS: i64 = 50;
/// React commit lands after `.finally()`.
pub(super) const REFRESH_POST_MS: i64 = 250;
/// Max wait for the settling `refresh`.
pub(super) const FOCUS_HOLD_MS: i64 = 5_000;
pub(super) const FOCUS_PENDING_CAP: usize = 64;
pub(super) const CAUSE_HISTORY_MS: i64 = 6_000;
pub(super) const CAUSE_HISTORY_CAP: usize = 256;

#[derive(Default)]
pub(super) struct StabilityState {
    /// Pruned to `CAUSE_HISTORY_MS`, FIFO-capped at `CAUSE_HISTORY_CAP`.
    causes: VecDeque<Cause>,
    /// FIFO-capped at `FOCUS_PENDING_CAP`; overflow ⇒ the oldest is emitted as info.
    pending: VecDeque<PendingLoss>,
}

struct Cause {
    kind: CauseKind,
    seq: u64,
    start: i64,
    end: i64,
    traces: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CauseKind {
    Refresh,
    Watcher,
}

struct PendingLoss {
    seq: u64,
    lost_at: i64,
    ts: i64,
    p: FocusLostPayload,
    trace: Option<String>,
}

impl StabilityState {
    /// `unprompted-shift` — one warn per qualifying (interactive, unprompted,
    /// non-resize) record.
    pub(super) fn on_shift(
        &mut self,
        ts: i64,
        seq: u64,
        rec: &LogRecord,
        p: &LayoutShiftPayload,
        out: &mut Vec<LogRecord>,
    ) {
        if p.phase != StabilityPhase::Interactive || p.had_recent_input || p.resizing {
            return;
        }
        out.push(build_anomaly(
            "unprompted-shift",
            AnomalySeverity::Warn,
            format!(
                "{}: {} shift(s), score {:.4}, over {} ms",
                p.region.as_str(),
                p.shifts,
                p.value,
                p.span_ms
            ),
            vec![seq],
            rec.trace.iter().cloned().collect(),
            ts,
        ));
    }

    pub(super) fn on_focus_lost(
        &mut self,
        ts: i64,
        seq: u64,
        rec: &LogRecord,
        p: &FocusLostPayload,
        out: &mut Vec<LogRecord>,
    ) {
        let loss = PendingLoss {
            seq,
            lost_at: p.lost_at,
            ts,
            p: p.clone(),
            trace: rec.trace.clone(),
        };
        let hits: Vec<&Cause> = self.causes.iter().filter(|c| matches(c, &loss)).collect();
        if !hits.is_empty() {
            out.push(emit_warn(&loss, &hits));
            return;
        }
        self.pending.push_back(loss);
        if self.pending.len() > FOCUS_PENDING_CAP {
            if let Some(oldest) = self.pending.pop_front() {
                out.push(emit_info(&oldest));
            }
        }
    }

    /// A settled refresh round: `ts` is the settle time, `ms` its duration.
    pub(super) fn on_refresh(
        &mut self,
        ts: i64,
        seq: u64,
        rec: &LogRecord,
        out: &mut Vec<LogRecord>,
    ) {
        let LogPayload::Refresh {
            ms,
            contributing_traces,
            ..
        } = &rec.payload
        else {
            return;
        };
        self.push_cause(Cause {
            kind: CauseKind::Refresh,
            seq,
            start: ts - ms.round() as i64,
            end: ts,
            traces: contributing_traces.clone(),
        });
        let causes = &self.causes;
        let Some(c) = causes.back() else { return };
        self.pending.retain(|loss| {
            if !matches(c, loss) {
                return true;
            }
            let watchers = causes
                .iter()
                .filter(|w| w.kind == CauseKind::Watcher && matches(w, loss));
            let hits: Vec<&Cause> = std::iter::once(c).chain(watchers).collect();
            out.push(emit_warn(loss, &hits));
            false
        });
    }

    /// Only a `fired`, non-`suppressed` watcher record ran a round.
    pub(super) fn on_watcher(
        &mut self,
        ts: i64,
        seq: u64,
        rec: &LogRecord,
        _out: &mut Vec<LogRecord>,
    ) {
        let LogPayload::Watcher {
            fired, suppressed, ..
        } = &rec.payload
        else {
            return;
        };
        if !*fired || *suppressed {
            return;
        }
        self.push_cause(Cause {
            kind: CauseKind::Watcher,
            seq,
            start: ts,
            end: ts,
            traces: rec.trace.iter().chain(&rec.caused_by).cloned().collect(),
        });
    }

    /// Drops old causes and emits `info` for held losses past `FOCUS_HOLD_MS`.
    pub(super) fn prune(&mut self, now: i64, out: &mut Vec<LogRecord>) {
        let cutoff = now - CAUSE_HISTORY_MS;
        self.causes.retain(|c| c.end >= cutoff);
        while self.causes.len() > CAUSE_HISTORY_CAP {
            self.causes.pop_front();
        }
        self.pending.retain(|loss| {
            let expired = now > loss.lost_at + FOCUS_HOLD_MS;
            if expired {
                out.push(emit_info(loss));
            }
            !expired
        });
    }

    /// Session end: every still-held loss is an uncaused `info`.
    pub(super) fn drain_pending(&mut self) -> Vec<LogRecord> {
        std::mem::take(&mut self.pending)
            .iter()
            .map(emit_info)
            .collect()
    }

    #[cfg(test)]
    pub(super) fn pending_len(&self) -> usize {
        self.pending.len()
    }

    #[cfg(test)]
    pub(super) fn causes_len(&self) -> usize {
        self.causes.len()
    }

    fn push_cause(&mut self, c: Cause) {
        self.causes.push_back(c);
        if self.causes.len() > CAUSE_HISTORY_CAP {
            self.causes.pop_front();
        }
    }
}

/// Trace equality (when the loss has one) OR the cause's time window.
fn matches(c: &Cause, loss: &PendingLoss) -> bool {
    let trace_hit = loss
        .trace
        .as_ref()
        .is_some_and(|t| c.traces.iter().any(|ct| ct == t));
    let window_hit = match c.kind {
        CauseKind::Watcher => {
            c.start <= loss.lost_at && loss.lost_at <= c.start + FOCUS_WATCHER_LOOKBACK_MS
        }
        CauseKind::Refresh => {
            c.start - REFRESH_PRE_SLACK_MS <= loss.lost_at
                && loss.lost_at <= c.end + REFRESH_POST_MS
        }
    };
    trace_hit || window_hit
}

fn loss_prefix(loss: &PendingLoss) -> String {
    format!(
        "focus lost from {} ({}, {})",
        loss.p.from_region.as_str(),
        loss.p.via.as_str(),
        loss.p.phase.as_str()
    )
}

/// Anchored at the loss record's `ts`, not the cause's.
fn emit_warn(loss: &PendingLoss, hits: &[&Cause]) -> LogRecord {
    let n_refresh = hits.iter().filter(|c| c.kind == CauseKind::Refresh).count();
    let n_watcher = hits.len() - n_refresh;
    let mut refs: Vec<u64> = hits.iter().map(|c| c.seq).collect();
    refs.sort_unstable();
    refs.insert(0, loss.seq);
    let mut traces: Vec<String> = Vec::new();
    for t in hits.iter().flat_map(|c| &c.traces).chain(&loss.trace) {
        if !traces.contains(t) {
            traces.push(t.clone());
        }
    }
    build_anomaly(
        "focus-steal",
        AnomalySeverity::Warn,
        format!(
            "{}; causes: {n_refresh} refresh, {n_watcher} watcher",
            loss_prefix(loss)
        ),
        refs,
        traces,
        loss.ts,
    )
}

fn emit_info(loss: &PendingLoss) -> LogRecord {
    build_anomaly(
        "focus-steal",
        AnomalySeverity::Info,
        format!("{}; no refresh or watcher round found", loss_prefix(loss)),
        vec![loss.seq],
        loss.trace.iter().cloned().collect(),
        loss.ts,
    )
}
