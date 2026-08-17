//! The RUN-level event funnel for a streaming conflict resolve (P68 §6.3).
//!
//! Split out of `ai_resolve_stream.rs` (P68c): sequencing/relabelling events is a
//! distinct concern from orchestrating batches, and keeping it here leaves both
//! files comfortably inside the ~500-line rule while P68d/P68e grow the surface.
//! Pure bookkeeping — it never touches a process, a repo or a payload.
//!
//! Private module with `pub(super)` items (the `ai/session_argv.rs` convention):
//! `ai_resolve_stream` is the only consumer, and nothing outside `git` should be
//! able to fabricate run events.

use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use crate::ai::stream::{truncate_text, MAX_EVENT_TEXT};
use crate::ai::{AiRunEvent, AiRunEventKind};

/// Run-level event funnel.
///
/// A bulk resolve is SEVERAL child processes under ONE run id, and every session
/// numbers its own events from 0 with its own clock — so forwarding them raw would
/// (a) make the frontend's `seq <= lastSeq` stale guard drop every event from batch
/// 2 onwards, and (b) show one `Started`/`Done` per batch, i.e. a run that "ends"
/// three times. This funnel therefore owns the run's sequence, the run's clock and
/// its lifecycle events: exactly one `Started` and exactly one terminal event, no
/// matter how many children ran.
pub(super) struct RunEvents<'a> {
    run_id: String,
    sink: &'a (dyn Fn(AiRunEvent) + Send + Sync),
    started: Instant,
    stream_log: bool,
    state: Mutex<EventState>,
}

#[derive(Default)]
struct EventState {
    seq: u64,
    /// Highest turn number any batch reported (§6.3: turns = max across batches).
    max_turn: u32,
    /// The last terminal `partialText` a session produced, re-attached to the run's
    /// own terminal event (D2 — display-only, never a proposal).
    partial: Option<String>,
    /// Set for a single-file run so every event carries its path.
    only_path: Option<String>,
}

impl<'a> RunEvents<'a> {
    pub(super) fn new(
        run_id: String,
        sink: &'a (dyn Fn(AiRunEvent) + Send + Sync),
        stream_log: bool,
    ) -> Self {
        RunEvents {
            run_id,
            sink,
            started: Instant::now(),
            stream_log,
            state: Mutex::new(EventState::default()),
        }
    }

    /// Poison recovery: the state is plain counters, so a panicking sink must not
    /// silence every later event.
    fn lock(&self) -> MutexGuard<'_, EventState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub(super) fn max_turn(&self) -> u32 {
        self.lock().max_turn
    }

    pub(super) fn set_only_path(&self, path: String) {
        self.lock().only_path = Some(path);
    }

    /// Next event in the RUN's sequence, stamped with the RUN's elapsed time.
    fn next_event(&self, kind: AiRunEventKind, turn: u32) -> AiRunEvent {
        let (seq, only_path) = {
            let mut st = self.lock();
            let seq = st.seq;
            st.seq += 1;
            (seq, st.only_path.clone())
        };
        let elapsed = self.started.elapsed().as_millis() as u64;
        let mut ev = AiRunEvent::new(&self.run_id, seq, kind, elapsed, turn);
        ev.path = only_path;
        ev
    }

    pub(super) fn log(&self, text: String) {
        if !self.stream_log {
            return;
        }
        let turn = self.max_turn();
        let mut ev = self.next_event(AiRunEventKind::Log, turn);
        ev.text = Some(truncate_text(&text, MAX_EVENT_TEXT));
        (self.sink)(ev);
    }

    /// `Started` / `Done` / `Failed` / `Cancelled` — emitted by the RUN, exactly
    /// once each. `Failed`/`Cancelled` carry the last session's `partialText`
    /// (display-only, D2/A5).
    pub(super) fn emit_run_level(
        &self,
        kind: AiRunEventKind,
        text: Option<String>,
        cost: Option<f64>,
    ) {
        let turn = self.max_turn();
        let mut ev = self.next_event(kind, turn);
        ev.text = text.map(|t| truncate_text(&t, MAX_EVENT_TEXT));
        ev.cost_usd = cost;
        if matches!(kind, AiRunEventKind::Failed | AiRunEventKind::Cancelled) {
            ev.partial_text = Some(self.lock().partial.clone().unwrap_or_default());
        }
        (self.sink)(ev);
    }

    /// One event from a session (P68 §6.3). `Started` and the per-batch terminal
    /// events are swallowed — the run owns those — and everything else is
    /// re-stamped onto the run's sequence and clock with its payload intact.
    pub(super) fn forward(&self, ev: AiRunEvent) {
        match ev.kind {
            AiRunEventKind::Started => {}
            AiRunEventKind::Log => {
                if self.stream_log {
                    self.relabel(ev);
                }
            }
            AiRunEventKind::TurnEnd | AiRunEventKind::AwaitingInput => {
                self.note_turn(ev.turn);
                self.relabel(ev);
            }
            AiRunEventKind::Done | AiRunEventKind::Failed | AiRunEventKind::Cancelled => {
                self.note_turn(ev.turn);
                let mut st = self.lock();
                if let Some(partial) = ev.partial_text {
                    st.partial = Some(partial);
                }
            }
        }
    }

    fn note_turn(&self, turn: u32) {
        let mut st = self.lock();
        st.max_turn = st.max_turn.max(turn);
    }

    /// Re-stamp a session event onto the run's sequence/clock, keeping its text,
    /// cost, turn and (for bulk attribution) any path it already carries.
    fn relabel(&self, ev: AiRunEvent) {
        let stamped = self.next_event(ev.kind, ev.turn);
        let mut out = ev;
        out.seq = stamped.seq;
        out.elapsed_ms = stamped.elapsed_ms;
        out.run_id = stamped.run_id;
        if out.path.is_none() {
            out.path = stamped.path;
        }
        (self.sink)(out);
    }
}

#[cfg(test)]
#[path = "ai_resolve_stream_events_tests.rs"]
mod tests;
