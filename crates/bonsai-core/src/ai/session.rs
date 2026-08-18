//! Lifecycle of ONE streaming `claude` run (P68 §A): argv, spawn, line-reader
//! threads, the 250 ms tick loop with the idle watchdog, mid-run replies, turn
//! accounting and reaping.
//!
//! ALL line interpretation lives in [`super::stream`] (D12) — this module only
//! decides what to do about it. Every line is forwarded as it arrives, which is
//! what makes D2 true: on cancel or watchdog fire the output collected so far is
//! already in the caller's hands, unlike `run_process`' discard-on-timeout
//! (`super::run_process`, the bug this milestone fixes).

use std::path::Path;
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, TryRecvError};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use super::session_argv::build_command;
use super::session_pipes::{send_write, spawn_reader, spawn_writer, turn_line, Msg, WriteTx};
use super::stream::{
    classify_line, permission_denial_lines, sentinel_question, truncate_text, AiRunEvent,
    AiRunEventKind, LineOutcome, MAX_EVENT_TEXT, MAX_PARTIAL_TEXT,
};
use super::{
    kill_child_tree, parse_result_envelope, AiResult, RunLimits, RunOpts, EXIT_GRACE, RECV_TICK,
};
use crate::error::AppError;

/// The post-EOF stderr drain and the failure it composes (the S1 race), plus its
/// deterministic tests. A `#[path]`-included CHILD module so it keeps reaching
/// `ClaudeSession`'s private state without widening anything for the move.
#[path = "session_drain.rs"]
mod session_drain;

/// Appended (via stdin, never argv) to a user reply so the next turn produces a
/// file body rather than more conversation. Single line by construction.
const REPLY_SUFFIX: &str = "\n\n(Answer above. Now output ONLY the merged file contents, with no conflict markers and no commentary.)";
/// How often the graceful-exit poll re-checks `try_wait` (mirrors `run_process`).
const EXIT_POLL: Duration = Duration::from_millis(50);

/// Cancel + reply plumbing handed to a session by [`super::AiRunRegistry`].
pub struct RunControl {
    pub run_id: String,
    pub cancel: Arc<AtomicBool>,
    /// Set by the session so `ai_reply_run` can reject a reply for a run that is
    /// not awaiting input, and so the UI can show the right affordance.
    pub awaiting: Arc<AtomicBool>,
    /// Set by the session right after spawn so the app-exit hook can kill an
    /// orphan (D7). 0 = not spawned.
    pub pid: Arc<AtomicU32>,
    pub replies: Receiver<String>,
}

/// Why the tick loop stopped without a result.
enum LoopEnd {
    Cancelled,
    Failed(String),
}

/// One streaming CLI run. Owns the event sequence, the turn state machine and
/// the accumulated partial text; the child/pipes are locals of [`Self::drive`]
/// so no exit path can forget them.
///
/// Private on purpose: [`run`] (i.e. `super::run_claude_streaming`) is the only
/// way to drive a session, so there is no public surface to misuse.
struct ClaudeSession<'a> {
    /// Borrowed: a bulk run drives SEVERAL sessions sequentially under one
    /// `RunControl` (P68b §6.3), so a session can never own it.
    ctl: &'a RunControl,
    on_event: &'a (dyn Fn(AiRunEvent) + Send + Sync),
    seq: u64,
    started: Instant,
    last_output: Instant,
    turn: u32,
    awaiting: bool,
    /// Assistant prose ONLY (D2/A5) — a plausible truncated body for display.
    partial: String,
    stderr_tail: String,
}

/// Blocking. Drives one streaming run to completion; see
/// [`super::run_claude_streaming`] for the contract.
pub(crate) fn run(
    cwd: &Path,
    prompt: &str,
    payload: &str,
    opts: RunOpts,
    limits: RunLimits,
    ctl: &RunControl,
    on_event: &(dyn Fn(AiRunEvent) + Send + Sync),
) -> Result<AiResult, AppError> {
    ClaudeSession::new(ctl, on_event).drive(cwd, prompt, payload, &opts, &limits)
}

impl<'a> ClaudeSession<'a> {
    fn new(ctl: &'a RunControl, on_event: &'a (dyn Fn(AiRunEvent) + Send + Sync)) -> Self {
        let now = Instant::now();
        ClaudeSession {
            ctl,
            on_event,
            seq: 0,
            started: now,
            last_output: now,
            turn: 0,
            awaiting: false,
            partial: String::new(),
            stderr_tail: String::new(),
        }
    }

    fn drive(
        mut self,
        cwd: &Path,
        prompt: &str,
        payload: &str,
        opts: &RunOpts,
        limits: &RunLimits,
    ) -> Result<AiResult, AppError> {
        // D8: the runId reaches the UI on seq 0, BEFORE the spawn can fail.
        let ev = self.event(AiRunEventKind::Started);
        self.send(ev);

        let mut child = match build_command(cwd, prompt, opts, limits).spawn() {
            Ok(c) => c,
            Err(e) => {
                let msg = if e.kind() == std::io::ErrorKind::NotFound {
                    format!("Claude Code CLI not found: {e}")
                } else {
                    e.to_string()
                };
                self.terminal(AiRunEventKind::Failed, msg.clone());
                return Err(AppError::AiUnavailable(msg));
            }
        };
        self.ctl.pid.store(child.id(), Ordering::Relaxed);
        // The idle clock starts HERE, not in `new()`: process creation (cmd.exe +
        // the npm shim + node) is not the child being silent, and charging it to
        // the watchdog made a 1 s limit fire on startup alone under load. A CLI
        // that never says anything is still reaped, from this instant.
        self.last_output = Instant::now();

        let (tx, rx) = channel::<Msg>();
        spawn_reader(child.stdout.take(), tx.clone(), Msg::Out, Msg::OutEof);
        spawn_reader(child.stderr.take(), tx.clone(), Msg::Err, Msg::ErrEof);
        // Readers are live BEFORE the first write, so a payload larger than the OS
        // pipe buffer cannot deadlock: the child stays free to write stdout while
        // it drains our stdin. And the write itself happens on the WRITER thread,
        // so a child that never drains stdin blocks nothing here — the loop below
        // keeps polling cancel and the watchdog either way (the failure class this
        // milestone exists to remove).
        let (wtx, wrx) = channel::<String>();
        spawn_writer(child.stdin.take(), wrx, tx);

        let first_turn = if limits.interactive {
            // D13: the prompt travels on stdin too in interactive mode.
            turn_line(&format!("{prompt}\n\n{payload}"))
        } else {
            payload.to_string()
        };
        let mut writer = Some(wtx);
        if let Err(e) = send_write(&mut writer, first_turn) {
            return Err(self.fail(&mut child, format!("Claude closed its input: {e}")));
        }
        if !limits.interactive {
            // Drop our handle: the writer thread closes stdin once the payload is
            // out, which is the EOF a one-shot run needs.
            writer = None;
        }

        match self.pump(&rx, &mut writer, limits) {
            Ok(res) => Ok(self.complete(&mut child, writer, res)),
            Err(LoopEnd::Cancelled) => Err(self.cancel(&mut child)),
            Err(LoopEnd::Failed(msg)) => Err(self.fail(&mut child, msg)),
        }
    }

    /// The tick loop (§3.3). Returns the LAST turn's result, or why it stopped.
    fn pump(
        &mut self,
        rx: &Receiver<Msg>,
        writer: &mut Option<WriteTx>,
        limits: &RunLimits,
    ) -> Result<AiResult, LoopEnd> {
        loop {
            // Checked here as well as on the tick so a chatty child (which never
            // lets `recv_timeout` expire) still cancels promptly.
            if self.ctl.cancel.load(Ordering::Relaxed) {
                return Err(LoopEnd::Cancelled);
            }
            match rx.recv_timeout(RECV_TICK) {
                Ok(Msg::Out(line)) => {
                    self.last_output = Instant::now();
                    if let Some(res) = self.on_stdout(&line, limits)? {
                        return Ok(res);
                    }
                }
                Ok(Msg::Err(line)) => {
                    self.last_output = Instant::now();
                    self.stderr_tail.push_str(&line);
                    self.stderr_tail.push('\n');
                    self.trim_stderr_tail();
                    self.log(format!("stderr: {line}"));
                }
                // stdout EOF before any `result` means the child died mid-turn.
                Ok(Msg::OutEof) => return self.ended_without_result(rx, None, limits),
                Ok(Msg::ErrEof) => {}
                // A failed stdin write is fatal (§3.3), but the child's own stderr
                // is a far better message than our `BrokenPipe`, so compose it the
                // same way — the two race by construction (different senders).
                Ok(Msg::WriteErr(e)) => return self.ended_without_result(rx, Some(e), limits),
                Err(RecvTimeoutError::Timeout) => self.on_tick(writer, limits)?,
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(LoopEnd::Failed(
                        "Claude output stream closed unexpectedly".to_string(),
                    ));
                }
            }
        }
    }

    /// One stdout line. `Ok(Some(res))` = the run is done (a non-question result).
    fn on_stdout(&mut self, line: &str, limits: &RunLimits) -> Result<Option<AiResult>, LoopEnd> {
        match classify_line(line) {
            // A4: the watchdog reset (done by the caller for every stdout line) and
            // NOTHING in the log — one heartbeat per second would drown the dock.
            //
            // P68d: when the line carried a cumulative `estimated_tokens`, forward it
            // as a METRICS-ONLY event: `kind: Log` with `text: None`. Deliberately
            // not an 8th event kind — the 7-kind union is locked (§3.2) — and
            // deliberately not a log line, so the dock stays readable while still
            // having a live number to show before the first `cost_usd` exists.
            // Consumers MUST treat a `log` event with `text == None` as metrics.
            LineOutcome::Heartbeat(tokens) => {
                if let Some(n) = tokens {
                    let mut ev = self.event(AiRunEventKind::Log);
                    ev.thinking_tokens = Some(n);
                    self.send(ev);
                }
            }
            LineOutcome::Log(items) => {
                for item in items {
                    if item.assistant_text {
                        self.partial.push_str(&item.text);
                        self.partial.push('\n');
                        self.trim_partial();
                    }
                    self.log_line(item.text, item.notable);
                }
            }
            LineOutcome::Result => {
                self.turn = self.turn.saturating_add(1);
                // The read fence's ONLY report channel (audit H1/M6): `--permission-mode
                // manual` records what it refused here and nowhere else. Before the
                // parse, so an error envelope cannot swallow the evidence.
                for item in permission_denial_lines(line) {
                    self.log_line(item.text, item.notable);
                }
                // spike §1.3: the streaming `result` line IS the one-shot envelope,
                // so both share one copy of the is_error/empty/fence logic.
                let res = match parse_result_envelope(line, true, &self.stderr_tail) {
                    Ok(r) => r,
                    Err(e) => return Err(LoopEnd::Failed(e.to_string())),
                };
                let mut ev = self.event(AiRunEventKind::TurnEnd);
                ev.cost_usd = res.cost_usd;
                self.send(ev);

                match sentinel_question(&res.text) {
                    None => return Ok(Some(res)),
                    Some(question) => {
                        if !limits.interactive {
                            return Err(LoopEnd::Failed(
                                "Claude needs more information but the run is not interactive"
                                    .to_string(),
                            ));
                        }
                        if self.turn >= limits.max_turns {
                            return Err(LoopEnd::Failed(format!(
                                "Claude asked {} questions without producing a resolution",
                                self.turn
                            )));
                        }
                        // Drop anything already sitting in the reply channel BEFORE
                        // arming `awaiting` (P68c FIX 1). Such a reply can only be
                        // STALE: `AiRunRegistry::reply` refuses unless `awaiting` is
                        // set, so a queued one belongs to a question that was
                        // already answered — two replies landing inside one 250 ms
                        // tick leave the second behind. The channel OUTLIVES a
                        // session (a bulk run drives several children through one
                        // `RunControl`), so a leftover would otherwise silently
                        // answer the NEXT question, i.e. batch 2 answered with batch
                        // 1's text. Order matters: drain first, arm second — a reply
                        // sent AFTER the store is a legitimate answer to THIS
                        // question and must never be dropped.
                        let stale = self.drain_stale_replies();
                        if stale > 0 {
                            self.log(format!(
                                "discarded {stale} stale repl{} queued before this question",
                                if stale == 1 { "y" } else { "ies" }
                            ));
                        }
                        self.awaiting = true;
                        self.ctl.awaiting.store(true, Ordering::Relaxed);
                        let mut ev = self.event(AiRunEventKind::AwaitingInput);
                        ev.text = Some(truncate_text(&question, MAX_EVENT_TEXT));
                        self.send(ev);
                    }
                }
            }
        }
        Ok(None)
    }

    /// Empty the reply channel, returning how many messages were thrown away.
    /// Called ONLY on the transition into the awaiting state, where every queued
    /// message is by definition stale (see the call site). Never blocks; a
    /// disconnected channel simply ends the loop — `on_tick` is what reports that.
    fn drain_stale_replies(&self) -> usize {
        let mut dropped = 0usize;
        while self.ctl.replies.try_recv().is_ok() {
            dropped += 1;
        }
        dropped
    }

    /// The 250 ms tick: pump a reply, or consult the watchdog / hard cap.
    fn on_tick(&mut self, writer: &mut Option<WriteTx>, limits: &RunLimits) -> Result<(), LoopEnd> {
        if self.awaiting {
            match self.ctl.replies.try_recv() {
                Ok(text) => {
                    let bytes = text.len();
                    // D13: reply text goes through stdin, NEVER argv. Handing it to
                    // the writer thread never blocks, so a child that stopped
                    // reading cannot wedge this loop; the failure comes back as
                    // `Msg::WriteErr`.
                    let line = turn_line(&format!("{text}{REPLY_SUFFIX}"));
                    if let Err(e) = send_write(writer, line) {
                        return Err(LoopEnd::Failed(format!("Claude closed its input: {e}")));
                    }
                    self.awaiting = false;
                    self.ctl.awaiting.store(false, Ordering::Relaxed);
                    self.last_output = Instant::now();
                    self.log(format!("» answered ({bytes} bytes)"));
                }
                Err(TryRecvError::Empty) => {}
                // Nobody can ever answer now, and the watchdog is paused — fail
                // rather than block forever.
                Err(TryRecvError::Disconnected) => {
                    return Err(LoopEnd::Failed(
                        "Claude asked a question but the reply channel is closed".to_string(),
                    ));
                }
            }
            // D3: NEITHER the idle watchdog NOR the hard cap is consulted while a
            // human is being waited on.
            return Ok(());
        }

        if !limits.idle_timeout.is_zero() && self.last_output.elapsed() > limits.idle_timeout {
            return Err(LoopEnd::Failed(format!(
                "Claude produced no output for {}s — stopped",
                limits.idle_timeout.as_secs()
            )));
        }
        if let Some(cap) = limits.hard_cap {
            if self.started.elapsed() > cap {
                return Err(LoopEnd::Failed(format!(
                    "Claude exceeded the {}s cap — stopped",
                    cap.as_secs()
                )));
            }
        }
        Ok(())
    }

    /// Success exit: close stdin (dropping the last `WriteTx` makes the writer
    /// thread release the pipe, so the child sees EOF and exits), give it
    /// `EXIT_GRACE`, then kill the tree if it is still alive. Always reaped.
    fn complete(&mut self, child: &mut Child, writer: Option<WriteTx>, res: AiResult) -> AiResult {
        drop(writer);
        let deadline = Instant::now() + EXIT_GRACE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => {
                    if Instant::now() >= deadline {
                        kill_child_tree(child);
                        let _ = child.wait();
                        break;
                    }
                    thread::sleep(EXIT_POLL);
                }
                Err(_) => {
                    // try_wait itself failed — liveness unknowable. Kill+wait
                    // like the deadline path so clearing the pid below can't
                    // orphan a still-live child from cancel_all's reach.
                    kill_child_tree(child);
                    let _ = child.wait();
                    break;
                }
            }
        }
        // §3.7: the child is reaped — clear the shared pid so a late
        // `cancel_all` cannot taskkill a pid the OS has already recycled.
        self.ctl.pid.store(0, Ordering::Relaxed);
        let mut ev = self.event(AiRunEventKind::Done);
        ev.cost_usd = res.cost_usd;
        self.send(ev);
        res
    }

    fn cancel(&mut self, child: &mut Child) -> AppError {
        self.reap(child);
        self.terminal(AiRunEventKind::Cancelled, "cancelled".to_string());
        AppError::AiCancelled("cancelled by user".to_string())
    }

    fn fail(&mut self, child: &mut Child, msg: String) -> AppError {
        self.reap(child);
        self.terminal(AiRunEventKind::Failed, msg.clone());
        AppError::AiFailed(msg)
    }

    /// Kill the whole tree, then `wait()` so no zombie survives. The reader
    /// threads are deliberately NOT joined: a surviving grandchild can hold the
    /// inherited pipe (the reason `run_process` detaches them too), and nothing is
    /// lost because every line was already forwarded (D2).
    ///
    /// The WRITER thread is the least obvious of the three and is detached for one
    /// more reason: it may still be blocked in `write_all` on a child that never
    /// drained its stdin, so joining it would reintroduce the very unkillable-run bug
    /// this milestone removes. Safe because it is the SOLE owner of the pipe —
    /// `child.stdin` was `take()`n at spawn, so `Child::wait()` below has no stdin to
    /// drop and cannot deadlock; after the tree kill the write errors, the thread
    /// reports `WriteErr` into a funnel nobody reads any more, and returns.
    fn reap(&mut self, child: &mut Child) {
        kill_child_tree(child);
        let _ = child.wait();
        // §3.7: pid 0 = "no live child" — prevents a pid-reuse kill from
        // `AiRunRegistry::cancel_all` after the wait has completed.
        self.ctl.pid.store(0, Ordering::Relaxed);
        self.ctl.awaiting.store(false, Ordering::Relaxed);
    }

    /// Bound the accumulator DURING the run, not only on the wire at
    /// [`Self::terminal`] time: streaming has no hard deadline by design, so a long
    /// run would otherwise grow this without limit in RAM. Twice the wire cap and
    /// keeping the HEAD — what `truncate_text` keeps — so the echo is unchanged.
    fn trim_partial(&mut self) {
        const KEEP: usize = 2 * MAX_PARTIAL_TEXT;
        if self.partial.chars().count() > KEEP {
            self.partial = self.partial.chars().take(KEEP).collect();
        }
    }

    /// Keep only the last `MAX_EVENT_TEXT` chars of stderr for the failure message.
    fn trim_stderr_tail(&mut self) {
        let count = self.stderr_tail.chars().count();
        if count > MAX_EVENT_TEXT {
            self.stderr_tail = self.stderr_tail.chars().skip(count - MAX_EVENT_TEXT).collect();
        }
    }

    /// Next event in the run's sequence (seq 0 is `Started`).
    fn event(&mut self, kind: AiRunEventKind) -> AiRunEvent {
        let elapsed = self.started.elapsed().as_millis() as u64;
        let ev = AiRunEvent::new(&self.ctl.run_id, self.seq, kind, elapsed, self.turn);
        self.seq += 1;
        ev
    }

    fn send(&self, ev: AiRunEvent) {
        (self.on_event)(ev);
    }

    fn log(&mut self, text: String) {
        self.log_line(text, false);
    }

    /// `notable` marks the lines that survive `ai_stream_log: false` (M6) — what the
    /// model read, what the fence denied. Set by classification, never from text shape.
    fn log_line(&mut self, text: String, notable: bool) {
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
    fn terminal(&mut self, kind: AiRunEventKind, msg: String) {
        let partial = truncate_text(&self.partial, MAX_PARTIAL_TEXT);
        let mut ev = self.event(kind);
        ev.text = Some(truncate_text(&msg, MAX_EVENT_TEXT));
        ev.partial_text = Some(partial);
        self.send(ev);
    }
}
