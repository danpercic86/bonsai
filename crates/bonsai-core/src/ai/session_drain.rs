//! What a streaming run does when it ended WITHOUT a `result` line (P68 §A): the
//! bounded post-EOF stderr drain (review S1) and the failure message it composes.
//!
//! Split from [`super`] so that module is only argv/spawn plus the tick state
//! machine. Nothing here interprets a line (that is `super::super::stream`) and
//! nothing here touches the child or its pipes — this concern begins once the run
//! is already decided, and its only job is to give the user the best available
//! diagnosis. D16 is unaffected: the drain blocks on the same `recv_timeout` the
//! loop already uses, never on I/O.
//!
//! A `#[path]`-included CHILD module of `session` on purpose: it reaches
//! `ClaudeSession`'s private state exactly as it did before the split, so
//! `ClaudeSession` stays private to `session` and NOTHING had to be widened for
//! the move (only the entry point is `pub(super)`, for `pump`).

use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use super::{classify_line, AiResult, ClaudeSession, LineOutcome, LoopEnd, Msg, RunLimits};

/// Per-`recv` grace while draining stderr after the run has already ended.
pub(super) const STDERR_GRACE: Duration = Duration::from_millis(150);
/// Absolute cap on that drain, so a chatty stderr cannot stall shutdown.
pub(super) const STDERR_GRACE_TOTAL: Duration = Duration::from_millis(1000);

impl<'a> ClaudeSession<'a> {
    /// Compose the "ended without a result" failure, giving stderr the last word —
    /// or, on the `WriteErr` path only, the late `result` the drain turned up.
    ///
    /// stderr arrives from a DIFFERENT sender than stdout EOF, and mpsc gives no
    /// ordering guarantee between them: a CLI that prints a usage/auth error and
    /// exits — the most likely real failure — would otherwise lose that text
    /// roughly half the time and report only the generic message. So drain a short
    /// BOUNDED grace first ([`STDERR_GRACE_TOTAL`]) before deciding.
    pub(super) fn ended_without_result(
        &mut self,
        rx: &Receiver<Msg>,
        write_err: Option<String>,
        limits: &RunLimits,
    ) -> Result<AiResult, LoopEnd> {
        // Entered from `WriteErr`, stdout is STILL OPEN, so a `result` can legally
        // land inside the drain window — promote it through the normal path (turn
        // accounting + `TurnEnd`) rather than failing a run that finished. `Ok(None)`
        // = a question, which only the turn that just failed could answer, so it
        // falls through. Unreachable from `OutEof`: per-sender FIFO puts every
        // stdout line ahead of stdout's own EOF.
        if let Some(line) = self.drain_stderr(rx) {
            match self.on_stdout(&line, limits) {
                Ok(Some(res)) => return Ok(res),
                Ok(None) => {}
                Err(end) => return Err(end),
            }
        }
        let tail = self.stderr_tail.trim().to_string();
        let msg = match (tail.is_empty(), write_err) {
            (false, _) => format!("Claude exited without a result: {tail}"),
            (true, Some(e)) => format!("Claude closed its input: {e}"),
            (true, None) => "Claude exited without a result".to_string(),
        };
        Err(LoopEnd::Failed(msg))
    }

    /// Take whatever stderr is still in flight, until `ErrEof`, an empty gap, or the
    /// total cap. Returns a `result` line if one arrives (see
    /// [`Self::ended_without_result`]); every OTHER stdout line is only LOGGED — the
    /// run is already decided, and dropping them silently would break D2.
    fn drain_stderr(&mut self, rx: &Receiver<Msg>) -> Option<String> {
        let deadline = Instant::now() + STDERR_GRACE_TOTAL;
        loop {
            // Clamp each per-recv grace to what is LEFT of the absolute cap, so the
            // total drain never exceeds STDERR_GRACE_TOTAL (a bare `recv_timeout(
            // STDERR_GRACE)` past a `now < deadline` check could overshoot by up to
            // one full STDERR_GRACE). A clamped wait that times out is treated as the
            // same empty-gap stop as before — we are at the cap and stopping anyway.
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            match rx.recv_timeout(remaining.min(STDERR_GRACE)) {
                Ok(Msg::Err(line)) => {
                    self.stderr_tail.push_str(&line);
                    self.stderr_tail.push('\n');
                    self.trim_stderr_tail();
                    self.log(format!("stderr: {line}"));
                }
                Ok(Msg::Out(line)) => {
                    if matches!(classify_line(&line), LineOutcome::Result) {
                        return Some(line);
                    }
                    self.log(line);
                }
                Ok(Msg::ErrEof) => return None,
                Ok(Msg::OutEof) | Ok(Msg::WriteErr(_)) => {}
                Err(_) => return None, // empty gap, or every sender gone
            }
        }
    }
}

/// Deterministic unit tests for the post-EOF stderr drain (the S1 race). A child
/// module so it can reach `Msg`/`ended_without_result` without widening either;
/// kept in its own file to keep this one focused.
#[cfg(test)]
#[path = "session_drain_tests.rs"]
mod drain_tests;
