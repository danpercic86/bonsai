//! STREAMING AI conflict resolution over 1..n paths (P68 §A/§D).
//!
//! One entry point, [`resolve_conflicts_streaming`], serves both cases: a single
//! file is literally `paths.len() == 1` (A1). That keeps the bulk split and the
//! per-file attribution in Rust (D1), keeps the command count at +3, and — the
//! important half — leaves the PROVEN single-file prompt and payload untouched
//! (§6.1): only the two P68 clauses (read-only tools, question sentinel) are
//! appended to the P13 system prompt.
//!
//! WRITES NOTHING (D4). This returns proposed BYTES; applying them stays the
//! caller's separate, explicit `resolve_conflict_text` step, exactly as in P13.
//! The tool allowlist is read-only (D10), so the child cannot write either.
//!
//! The pure payload/attribution rules live in [`super::ai_resolve_bulk`]; this
//! module is orchestration only.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use crate::ai::stream::{truncate_text, MAX_EVENT_TEXT};
use crate::ai::{
    self, AiResult, AiRunEvent, AiRunEventKind, RunControl, RunLimits, RunOpts,
};
use crate::error::AppError;

use super::ai_resolve::{
    build_single_payload, read_conflict_sides, AiResolveProposal, ConflictSides, RESOLVE_PROMPT,
    SYSTEM_PROMPT,
};
use super::ai_resolve_bulk::{
    build_bulk_payload, bulk_system_prompt, pack_batches, parse_bulk_response, part_bytes,
    BULK_PROMPT, READ_ONLY_CLAUSE, SENTINEL_CLAUSE,
};

/// One path that could not be resolved. NEVER fatal to a batch (D11) — a single
/// bad file must not cost the user the other files' work.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResolveFailure {
    pub path: String,
    pub reason: String,
}

/// The outcome of ONE streaming resolve run over 1..n paths (P68 §8.2).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiResolveBatch {
    /// Echo of the run id. Also delivered on the FIRST channel event (D8) —
    /// this field cannot be the UI's source of it, because the command promise
    /// only settles when the run is over.
    pub run_id: String,
    /// One entry per successfully attributed path (the P13 type, verbatim).
    /// `cost_usd` is per-RUN, not per-file, so a bulk proposal carries `None`
    /// there and the batch's `cost_usd` below is the whole story.
    pub proposals: Vec<AiResolveProposal>,
    /// Per-file failures; never fatal to the batch (D11).
    pub failed: Vec<AiResolveFailure>,
    /// Last value WITHIN a run, SUMMED across sequential bulk batches (A10):
    /// separate processes have independent totals, so summing is correct there,
    /// while within one process the observed value climbs per turn.
    pub cost_usd: Option<f64>,
    /// Max turns used across batches (1 when no question was asked).
    pub turns: u32,
}

/// Everything the command layer derives from settings for one streaming resolve
/// (P68 §8.3). Separate from [`RunLimits`] because these three are Bonsai-side
/// policy, not CLI limits.
#[derive(Debug, Clone)]
pub struct StreamResolveOpts {
    /// `model` is honoured; `system_prompt` is REPLACED per mode; `timeout` is
    /// ignored by streaming (see `ai::run_claude_streaming`).
    pub opts: RunOpts,
    pub limits: RunLimits,
    /// `ai_bulk_max_bytes`: the payload cap that triggers batch splitting (§6.3).
    pub bulk_max_bytes: usize,
    /// `ai_stream_log == false` suppresses `Log` events AT THE SOURCE, so a user
    /// who turns the log off pays no IPC cost (§8.3). Status-changing events are
    /// never suppressed.
    pub stream_log: bool,
}

/// The system prompt for a single-file streaming run: the PROVEN P13 text plus
/// the two P68 clauses, nothing else (§6.1). Single line by construction (D13).
fn single_system_prompt() -> String {
    format!("{SYSTEM_PROMPT}{READ_ONLY_CLAUSE}{SENTINEL_CLAUSE}")
}

/// Blocking (callers invoke under `spawn_blocking`). Resolves every path in
/// `paths` and returns proposals + per-file failures, pushing progress through
/// `on_event`.
///
/// Shape of a run:
/// 1. `Started` (seq 0) is emitted BEFORE any git or process work, so the UI has
///    the run id in time to cancel (D8);
/// 2. every path is read + eligibility-guarded ([`read_conflict_sides`]);
/// 3. one path ⇒ the P13 payload/prompt; several ⇒ the bulk delimiter format,
///    split into as many sequential batches as the byte cap requires (§6.3);
/// 4. exactly ONE terminal event (`Done` / `Failed` / `Cancelled`) closes the run,
///    whatever the batch count.
///
/// Errors: `aiUnavailable` (CLI missing) | `aiFailed` | `aiCancelled` | `git` /
/// `invalidName` (single-path requests only — a bad path in a bulk request is an
/// individual failure, D11).
pub fn resolve_conflicts_streaming(
    workdir: &Path,
    paths: &[String],
    cfg: StreamResolveOpts,
    ctl: &RunControl,
    on_event: &(dyn Fn(AiRunEvent) + Send + Sync),
) -> Result<AiResolveBatch, AppError> {
    let events = RunEvents::new(ctl.run_id.clone(), on_event, cfg.stream_log);
    // A one-path run is about that path from its very FIRST event, so the UI can
    // attribute even a spawn failure to the right row.
    if let [only] = paths {
        events.set_only_path(only.clone());
    }
    // D8: seq 0 goes out before ANYTHING can fail, including opening the repo.
    events.emit_run_level(AiRunEventKind::Started, None, None);

    match resolve_batches(workdir, paths, &cfg, ctl, &events) {
        Ok(batch) => {
            events.emit_run_level(AiRunEventKind::Done, None, batch.cost_usd);
            Ok(batch)
        }
        Err(err) => {
            let kind = match &err {
                AppError::AiCancelled(_) => AiRunEventKind::Cancelled,
                _ => AiRunEventKind::Failed,
            };
            events.emit_run_level(kind, Some(err.to_string()), None);
            Err(err)
        }
    }
}

/// Read the conflicts, then run one or more sessions. Split from the public entry
/// point so the terminal event is emitted on EVERY exit path, in one place.
fn resolve_batches(
    workdir: &Path,
    paths: &[String],
    cfg: &StreamResolveOpts,
    ctl: &RunControl,
    events: &RunEvents<'_>,
) -> Result<AiResolveBatch, AppError> {
    if paths.is_empty() {
        return Err(AppError::AiFailed("no conflicted paths given".to_string()));
    }
    // A single-path request keeps P13's error surface: an ineligible path (binary,
    // too large, not conflicted, traversal) REJECTS with that path's own error
    // instead of resolving to an empty batch, so the existing UI messages and the
    // `invalidName` / `git` kinds are unchanged.
    let single_request = paths.len() == 1;
    let mut sides = Vec::with_capacity(paths.len());
    let mut failed: Vec<AiResolveFailure> = Vec::new();
    for path in paths {
        match read_conflict_sides(workdir, path) {
            Ok(s) => sides.push(s),
            Err(e) if single_request => return Err(e),
            Err(e) => {
                // D11: skip this file, keep the run.
                events.log(format!("skipping {path}: {e}"));
                failed.push(AiResolveFailure { path: path.clone(), reason: e.to_string() });
            }
        }
    }
    if sides.is_empty() {
        return Err(AppError::AiFailed(
            "AI resolution is not available for these files".to_string(),
        ));
    }

    if sides.len() == 1 {
        // Also the "2 requested, 1 eligible" case: one file always uses the proven
        // single-file format rather than a one-element bulk payload.
        resolve_single(workdir, &sides[0], cfg, ctl, events, failed)
    } else {
        resolve_bulk(workdir, &sides, cfg, ctl, events, failed)
    }
}

/// ONE file, today's payload and prompt (§6.1).
fn resolve_single(
    workdir: &Path,
    sides: &ConflictSides,
    cfg: &StreamResolveOpts,
    ctl: &RunControl,
    events: &RunEvents<'_>,
    failed: Vec<AiResolveFailure>,
) -> Result<AiResolveBatch, AppError> {
    // Every event of this run is about this one file, so attribute them all.
    events.set_only_path(sides.path.clone());
    let payload = build_single_payload(sides);
    events.log(format!("resolving {} ({} B)", sides.path, payload.len()));

    let res = run_session(
        workdir,
        RESOLVE_PROMPT,
        &payload,
        single_system_prompt(),
        cfg,
        ctl,
        events,
    )?;

    // The body travels VERBATIM, exactly as `ai_resolve_conflict` has returned it
    // since P13 — including a body that still has markers. That is deliberate: the
    // frontend's `hasUnresolvedMarkers` gate refuses to stage such a body but still
    // opens it for review, so the user keeps a nearly-good merge to fix by hand.
    // (A markerful body inside a BULK reply cannot be reviewed that way, which is
    // why §6.2 marks it `failed` there instead.) Nothing markerful is ever
    // presented as clean on either path.
    Ok(AiResolveBatch {
        run_id: ctl.run_id.clone(),
        proposals: vec![AiResolveProposal {
            path: sides.path.clone(),
            proposed_text: res.text,
            cost_usd: res.cost_usd,
        }],
        failed,
        cost_usd: res.cost_usd,
        turns: events.max_turn().max(1),
    })
}

/// SEVERAL files: one run, batched only if the byte cap demands it (D11/§6.3).
fn resolve_bulk(
    workdir: &Path,
    sides: &[ConflictSides],
    cfg: &StreamResolveOpts,
    ctl: &RunControl,
    events: &RunEvents<'_>,
    mut failed: Vec<AiResolveFailure>,
) -> Result<AiResolveBatch, AppError> {
    let measured: Vec<(String, usize)> =
        sides.iter().map(|s| (s.path.clone(), part_bytes(s))).collect();
    let (batches, oversize) = pack_batches(&measured, cfg.bulk_max_bytes);
    for f in &oversize {
        events.log(format!("skipping {}: {}", f.path, f.reason));
    }
    failed.extend(oversize);
    if batches.is_empty() {
        return Err(AppError::AiFailed(
            "every conflicted file is too large for AI resolution".to_string(),
        ));
    }

    let total_batches = batches.len();
    let mut proposals: Vec<AiResolveProposal> = Vec::new();
    let mut cost: Option<f64> = None;

    for (i, batch) in batches.iter().enumerate() {
        // Cancel BETWEEN batches too: the flag may have been flipped while the
        // previous child was being reaped.
        if ctl.cancel.load(Ordering::Relaxed) {
            return Err(AppError::AiCancelled("cancelled by user".to_string()));
        }
        let parts: Vec<&ConflictSides> = batch.iter().map(|idx| &sides[*idx]).collect();
        let requested: Vec<String> = parts.iter().map(|s| s.path.clone()).collect();
        let payload = build_bulk_payload(&parts);
        events.log(format!(
            "batch {}/{}: {} files ({} B)",
            i + 1,
            total_batches,
            parts.len(),
            payload.len()
        ));

        let res = match run_session(
            workdir,
            BULK_PROMPT,
            &payload,
            bulk_system_prompt(),
            cfg,
            ctl,
            events,
        ) {
            Ok(res) => res,
            // A cancel ends the whole run: the events already emitted stand (D2),
            // but a cancelled run deliberately returns no proposals (§11.4).
            Err(e @ AppError::AiCancelled(_)) => return Err(e),
            // No CLI ⇒ the next batch cannot fare better; fail the run honestly
            // rather than reporting n per-file failures for one missing binary.
            Err(e @ AppError::AiUnavailable(_)) => return Err(e),
            Err(e) => {
                // A batch that fails as a WHOLE must not lose the other batches'
                // work (§6.3): mark its paths and continue.
                events.log(format!("batch {}/{total_batches} failed: {e}", i + 1));
                failed.extend(fail_all(&requested, &e.to_string()));
                continue;
            }
        };
        if let Some(c) = res.cost_usd {
            cost = Some(cost.unwrap_or(0.0) + c);
        }

        match parse_bulk_response(&res.text, &requested) {
            Ok(parsed) => {
                for path in parsed.unknown {
                    events.log(format!("ignoring a result block for an unrequested path: {path}"));
                }
                for (path, body) in parsed.proposals {
                    proposals.push(AiResolveProposal {
                        path,
                        proposed_text: body,
                        // Per-file cost is not knowable: one run covered them all.
                        cost_usd: None,
                    });
                }
                for f in &parsed.failed {
                    events.log(format!("{}: {}", f.path, f.reason));
                }
                failed.extend(parsed.failed);
            }
            Err(e) => {
                events.log(format!("batch {}/{total_batches} unparseable: {e}", i + 1));
                failed.extend(fail_all(&requested, &e.to_string()));
            }
        }
    }

    Ok(AiResolveBatch {
        run_id: ctl.run_id.clone(),
        proposals,
        failed,
        cost_usd: cost,
        turns: events.max_turn().max(1),
    })
}

fn fail_all(paths: &[String], reason: &str) -> Vec<AiResolveFailure> {
    paths
        .iter()
        .map(|p| AiResolveFailure { path: p.clone(), reason: reason.to_string() })
        .collect()
}

/// Drive ONE child process (one batch) with `system_prompt`, funnelling its events
/// through `events` so the whole run keeps a single monotonic sequence.
fn run_session(
    workdir: &Path,
    prompt: &str,
    payload: &str,
    system_prompt: String,
    cfg: &StreamResolveOpts,
    ctl: &RunControl,
    events: &RunEvents<'_>,
) -> Result<AiResult, AppError> {
    let opts = RunOpts { system_prompt: Some(system_prompt), ..cfg.opts.clone() };
    ai::run_claude_streaming(
        workdir,
        prompt,
        payload,
        opts,
        cfg.limits.clone(),
        ctl,
        &|ev| events.forward(ev),
    )
}

/// Run-level event funnel.
///
/// A bulk resolve is SEVERAL child processes under ONE run id, and every session
/// numbers its own events from 0 with its own clock — so forwarding them raw would
/// (a) make the frontend's `seq <= lastSeq` stale guard drop every event from batch
/// 2 onwards, and (b) show one `Started`/`Done` per batch, i.e. a run that "ends"
/// three times. This funnel therefore owns the run's sequence, the run's clock and
/// its lifecycle events: exactly one `Started` and exactly one terminal event, no
/// matter how many children ran.
struct RunEvents<'a> {
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
    fn new(
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

    fn max_turn(&self) -> u32 {
        self.lock().max_turn
    }

    fn set_only_path(&self, path: String) {
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

    fn log(&self, text: String) {
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
    fn emit_run_level(&self, kind: AiRunEventKind, text: Option<String>, cost: Option<f64>) {
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
    fn forward(&self, ev: AiRunEvent) {
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
#[path = "ai_resolve_stream_tests.rs"]
mod tests;
