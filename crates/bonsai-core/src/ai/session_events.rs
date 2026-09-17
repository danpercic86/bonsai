//! The `AiRunEvent` sequence a streaming run emits, and the bounds on the text it
//! carries (P68 §A): sequence numbering, the log/terminal event shapes, and the
//! two accumulator trims that keep the echo from growing without limit.
//!
//! Split from [`super`] so that module is argv/spawn plus the tick state machine
//! and nothing else. Nothing here touches the child, its pipes or the clock's
//! policy decisions — this concern begins once the loop has already decided what
//! happened, and its only job is to shape what the caller sees.
//!
//! A `#[path]`-included CHILD module of `session` on purpose: it reaches
//! `ClaudeSession`'s private state exactly as it did before the split, so
//! `ClaudeSession` stays private to `session` and NOTHING had to be widened for
//! the move (only the methods themselves are `pub(super)`, for the tick loop and
//! for `session_drain`).

use super::{
    truncate_text, AiRunEvent, AiRunEventKind, ClaudeSession, MAX_EVENT_TEXT, MAX_PARTIAL_TEXT,
};

impl<'a> ClaudeSession<'a> {
    /// Bound the accumulator DURING the run, not only on the wire at
    /// [`Self::terminal`] time: streaming has no hard deadline by design, so a long
    /// run would otherwise grow this without limit in RAM. Twice the wire cap and
    /// keeping the HEAD — what `truncate_text` keeps — so the echo is unchanged.
    pub(super) fn trim_partial(&mut self) {
        const KEEP: usize = 2 * MAX_PARTIAL_TEXT;
        if self.partial.chars().count() > KEEP {
            self.partial = self.partial.chars().take(KEEP).collect();
        }
    }

    /// Keep only the last `MAX_EVENT_TEXT` chars of stderr for the failure message.
    pub(super) fn trim_stderr_tail(&mut self) {
        let count = self.stderr_tail.chars().count();
        if count > MAX_EVENT_TEXT {
            self.stderr_tail = self
                .stderr_tail
                .chars()
                .skip(count - MAX_EVENT_TEXT)
                .collect();
        }
    }

    /// Next event in the run's sequence (seq 0 is `Started`).
    pub(super) fn event(&mut self, kind: AiRunEventKind) -> AiRunEvent {
        let elapsed = self
            .clock
            .now()
            .saturating_duration_since(self.started)
            .as_millis() as u64;
        let ev = AiRunEvent::new(&self.ctl.run_id, self.seq, kind, elapsed, self.turn);
        self.seq += 1;
        ev
    }

    pub(super) fn send(&self, ev: AiRunEvent) {
        (self.on_event)(ev);
    }

    pub(super) fn log(&mut self, text: String) {
        self.log_line(text, false);
    }

    /// `notable` marks the lines that survive `ai_stream_log: false` (M6) — what the
    /// model read, what the fence denied. Set by classification, never from text shape.
    pub(super) fn log_line(&mut self, text: String, notable: bool) {
        let mut ev = self.event(AiRunEventKind::Log);
        ev.text = Some(truncate_text(&text, MAX_EVENT_TEXT));
        ev.notable = notable;
        self.send(ev);
    }

    /// `Failed` / `Cancelled` carry the accumulated assistant text for DISPLAY
    /// only — never as a stageable proposal (A5).
    ///
    /// The echo is LOSSY by construction: each block was already truncated to
    /// `MAX_EVENT_TEXT` on the way in, `--include-partial-messages` deltas are
    /// deliberately excluded (they would double-count the final `assistant` line),
    /// and the whole thing is capped here. The dock log — every `Log` event — is
    /// the complete record; `partialText` is only what the panel shows.
    pub(super) fn terminal(&mut self, kind: AiRunEventKind, msg: String) {
        let partial = truncate_text(&self.partial, MAX_PARTIAL_TEXT);
        let mut ev = self.event(kind);
        ev.text = Some(truncate_text(&msg, MAX_EVENT_TEXT));
        ev.partial_text = Some(partial);
        self.send(ev);
    }
}
