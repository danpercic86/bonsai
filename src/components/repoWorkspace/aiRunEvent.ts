/**
 * P68d §5.2 — the event mapping table, as a PURE decision function.
 *
 * Split out of `useAiRuns.ts` for the same reason `ai/stream.rs` is split out of
 * `ai/session.rs` on the Rust side (D12): interpreting an event is separable from the
 * refs, timers and IPC that act on the interpretation, and separating them is what
 * makes the table readable and directly testable. The hook applies the returned
 * `Decision`; it makes none of the choices here.
 */
import { classifyLogLine, type AiRunLogLine } from './aiRunLog';
import { isTerminalStatus, type AiRunState } from './aiRunState';
import type { AiRunEvent } from '../../ipc';

export interface AiRunDecision {
  /** Fields to merge into the run entry. Empty object = no field change. */
  patch: Partial<AiRunState>;
  /** Buffer this line (50 ms flush) instead of committing now (D5). */
  logLine: AiRunLogLine | null;
  /** Buffer this live thinking-token count (same 50 ms flush). */
  thinkingTokens: number | null;
  /** Commit the render mirror NOW — status-changing events are rare and must land. */
  flushNow: boolean;
  /** `started` carried the id: fire any cancel the user already asked for (D8). */
  fireQueuedCancel: string | null;
}

/** Drop the event entirely (stale, duplicate, or after a terminal status). */
const IGNORE: AiRunDecision = {
  patch: {},
  logLine: null,
  thinkingTokens: null,
  flushNow: false,
  fireQueuedCancel: null,
};

function decide(patch: Partial<AiRunState>): AiRunDecision {
  return { ...IGNORE, patch, flushNow: true };
}

/**
 * What one channel event means for one run entry (PURE). `now` is injected so the
 * caller owns the clock.
 *
 * Two guards come first and are the reason this returns a decision rather than
 * mutating: `seq <= lastSeq` drops stales and duplicates (D8), and a run that already
 * reached a terminal status is never resurrected — the command PROMISE, not the event
 * stream, is authoritative for the final data, so a late `awaitingInput` arriving
 * after a cancel must not un-cancel the run.
 *
 * `log` is the one exception, and only for its TEXT: lines that were already in
 * flight still belong in the record (D2). A metrics-only heartbeat arriving after a
 * terminal status used to slip through the same door and could still move
 * `thinkingTokens` after `done` — harmless (monotonic, display-only) but inconsistent
 * with "a terminal status is never resurrected", so it is now dropped explicitly
 * (P68e FOLD-IN 3).
 */
export function decideEvent(entry: AiRunState, ev: AiRunEvent, now: number): AiRunDecision {
  if (ev.seq <= entry.lastSeq) return IGNORE;
  const terminal = isTerminalStatus(entry.status);
  if (terminal && (ev.kind !== 'log' || ev.text === null)) return IGNORE;
  const seen: Partial<AiRunState> = { lastSeq: ev.seq };

  switch (ev.kind) {
    case 'started':
      return {
        ...IGNORE,
        patch: { ...seen, runId: ev.runId },
        flushNow: true,
        fireQueuedCancel: entry.cancelRequested ? ev.runId : null,
      };

    case 'log':
      // A `log` with NO text is a METRICS-ONLY heartbeat (P68d): record the live
      // thinking-token estimate and add nothing to the log — one heartbeat per second
      // would drown the dock (A4). The two fields are mutually exclusive by contract.
      if (ev.text === null) {
        return { ...IGNORE, patch: seen, thinkingTokens: ev.thinkingTokens };
      }
      return {
        ...IGNORE,
        patch: seen,
        logLine: { seq: ev.seq, text: ev.text, kind: classifyLogLine(ev.text) },
      };

    // Cost is LAST-value-wins within a run, never summed (A10).
    case 'turnEnd':
    case 'done':
      return decide({ ...seen, costUsd: ev.costUsd ?? entry.costUsd, turn: ev.turn });

    case 'awaitingInput':
      return decide({ ...seen, status: 'awaitingInput', question: ev.text, turn: ev.turn });

    case 'failed':
      return decide({
        ...seen,
        status: 'failed',
        error: ev.text,
        partialText: ev.partialText,
        endedAt: now,
      });

    case 'cancelled':
      return decide({ ...seen, status: 'cancelled', partialText: ev.partialText, endedAt: now });
  }
}
