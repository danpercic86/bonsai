/**
 * P68 §8.5 / D15 — the STREAMING conflict resolver in the mock IPC layer.
 *
 * A new file rather than three more handlers in `ai.ts` (485 lines): this one is
 * stateful (a run registry, a pending-reply promise, a cancel flag) where the rest
 * of `ai.ts` is request/response.
 *
 * D15 — every event kind and every terminal state must be reachable in a plain
 * browser: `?aiSlow` (long run, cancellable), `?aiAsk` (mid-run question + reply
 * round trip), `?aiFail` (single ⇒ rejection, bulk ⇒ ONE per-file failure), and
 * `?ai=off` (no CLI). P68d adds two more:
 *   `?aiMarkers` — the proposal comes back with its conflict markers INTACT, which is
 *      the only way to actually exercise the frontend safety gate. The single-path
 *      stream returns the model's body verbatim (P13 parity), so `hasUnresolvedMarkers`
 *      in `useAiRuns` is the ONLY thing standing between a markerful body and a silent
 *      `autoResolve` stage. Without this seam that gate is unprovable end-to-end.
 *   `?aiFlood` — ~700 log lines, one of them exactly `AI_EVENT_TEXT_MAX` chars, to
 *      exercise the 500-line cap, `logDropped`, the truncation chip and jump-to-latest.
 * The `?ai=off` seam is honoured HERE deliberately —
 * `ai.ts:29`'s `aiResolveConflict` ignores it, a known pre-P68 gap left untouched
 * so P68c changes no existing behaviour.
 *
 * WRITES NOTHING (D4), exactly like the real command: the returned bodies are
 * proposals; applying one stays the caller's separate `resolveConflictText` step,
 * and a body that still has markers is a REVIEWABLE proposal, never a clean merge.
 */
import { AI_MAX_CONCURRENT_RUNS } from '../../../settings/ranges';
import { AI_OFF, delay, query, requireRepo, stripConflictMarkers } from '../repoState';
import type {
  AiResolveBatch,
  AiResolveFailure,
  AiResolveProposal,
  AiRunEvent,
  AiRunEventKind,
  AppError,
  IpcApi,
} from '../../types';

// Module-init seams (like `AI_OFF` / `HISTORY_FAIL`): read once so a test can
// re-import the module under a rewritten location.
const AI_SLOW = query('aiSlow') !== null;
const AI_ASK = query('aiAsk') !== null;
const AI_FAIL = query('aiFail') !== null;
/** Return the body with markers INTACT — the frontend safety-gate seam (see above). */
const AI_MARKERS = query('aiMarkers') !== null;
/** Overrun the 500-line log cap in one run. */
const AI_FLOOD = query('aiFlood') !== null;

/** MIRRORS `bonsai_core::ai::stream::MAX_EVENT_TEXT`: Rust truncates to exactly this
 *  many chars and appends `…`, which is what the dock's "truncated" chip detects. */
const MAX_EVENT_TEXT = 2000;

/** Mock costs. Distinct per turn so "LAST value wins within a run" (A10) is
 *  observable in the harness instead of being a comment. */
const COST_FIRST_TURN = 0.018;
const COST_LAST_TURN = 0.0238;
const COST_RUN = 0.0263;

const QUESTION = 'Should the German plural form use "Einträge" or "Eintraege"?';

interface MockRun {
  cancelled: boolean;
  awaiting: boolean;
  /** Resolves the `awaitingInput` wait; null when the run is not blocked. */
  resolveReply: ((text: string) => void) | null;
}

const runs = new Map<string, MockRun>();
let counter = 0;

function err(kind: AppError['kind'], message: string): AppError {
  return { kind, message };
}

/** The event sequencer: one monotonic `seq` per run, a real `elapsedMs`, the
 *  current turn, and the assistant-text accumulation that terminal events echo as
 *  `partialText` (D2 — display-only, never a proposal). */
function sequencer(runId: string, onEvent: (e: AiRunEvent) => void, onlyPath: string | null) {
  const startedAt = Date.now();
  let seq = 0;
  let turn = 0;
  let partial = '';
  return {
    setTurn(next: number): void {
      turn = next;
    },
    emit(kind: AiRunEventKind, extra: Partial<AiRunEvent> = {}): void {
      onEvent({
        runId,
        seq: seq++,
        kind,
        text: null,
        costUsd: null,
        elapsedMs: Date.now() - startedAt,
        path: onlyPath,
        turn,
        partialText: null,
        thinkingTokens: null,
        ...extra,
      });
    },
    /** One log line. `assistant` marks real model prose, which is the only thing
     *  that accumulates into `partialText` (the Rust `StreamLogItem.assistantText`
     *  rule) — decoration like `⚙ Read(...)` would make the echo implausible. */
    log(text: string, path?: string, assistant = false): void {
      if (assistant) partial += `${text}\n`;
      this.emit('log', { text, path: path ?? onlyPath });
    },
    partialText(): string {
      return partial;
    },
    /** P68d: the CLI's `system`/`thinking_tokens` heartbeat as Rust forwards it — a
     *  METRICS-ONLY `log` event (`text: null`), never a log line (A4). This is the
     *  run's only live spend signal before the first `costUsd`. */
    heartbeat(tokens: number): void {
      this.emit('log', { text: null, thinkingTokens: tokens });
    },
  };
}

type Sequencer = ReturnType<typeof sequencer>;

/** Terminal `cancelled`: the log collected so far stands (D2) and the command
 *  rejects `aiCancelled`, so the caller has ONE catch path. */
function cancelNow(ev: Sequencer): never {
  ev.emit('cancelled', { text: 'cancelled', partialText: ev.partialText() });
  throw err('aiCancelled', 'cancelled by user');
}

function failNow(ev: Sequencer, message: string): never {
  ev.emit('failed', { text: message, partialText: ev.partialText() });
  throw err('aiFailed', message);
}

function basename(path: string): string {
  const parts = path.split('/');
  return parts[parts.length - 1] ?? path;
}

export const aiStreamHandlers = {
  async aiResolveConflictStream(
    repoId: string,
    paths: string[],
    onEvent: (e: AiRunEvent) => void,
  ): Promise<AiResolveBatch> {
    const state = requireRepo(repoId);
    // `?ai=off` = no CLI on PATH. Refused BEFORE a run id exists, so the UI never
    // shows a dock entry for it (the real command's consent/availability gate).
    if (AI_OFF) {
      throw err('aiUnavailable', 'Claude Code CLI not found on PATH');
    }
    if (paths.length === 0) {
      throw err('aiFailed', 'no conflicted paths given');
    }
    // Mirrors the BACKEND cap (`bonsai_core::ai::AI_MAX_CONCURRENT_RUNS`), message
    // prefix included, so the harness can drive the "too many AI runs" path.
    if (runs.size >= AI_MAX_CONCURRENT_RUNS) {
      throw err(
        'aiFailed',
        `too many AI runs in progress (${runs.size} of ${AI_MAX_CONCURRENT_RUNS} allowed) — cancel one and try again`,
      );
    }

    // Only text-mergeable kinds are eligible; anything else is an INDIVIDUAL
    // failure and never costs the other files their run (D11).
    const eligible: string[] = [];
    const failed: AiResolveFailure[] = [];
    for (const path of paths) {
      const entry = state.conflicts.find((c) => c.path === path);
      if (entry !== undefined && (entry.kind === 'bothModified' || entry.kind === 'bothAdded')) {
        eligible.push(path);
      } else {
        failed.push({ path, reason: 'AI resolution is not available for this file' });
      }
    }
    if (eligible.length === 0) {
      throw err('aiFailed', 'AI resolution is not available for these files');
    }

    const runId = `mock-run-${++counter}`;
    const run: MockRun = { cancelled: false, awaiting: false, resolveReply: null };
    runs.set(runId, run);
    // A single-path run is about that path from its FIRST event (Rust's
    // `set_only_path`), so even a spawn failure lands on the right row.
    const ev = sequencer(runId, onEvent, paths.length === 1 ? paths[0] : null);
    try {
      // D8: the runId reaches the UI here, not via the return value.
      ev.emit('started');
      ev.log('session mock-sess · model sonnet · tools: Read, Grep, Glob');
      // Proof that the read-only allowlist lets the model look around the repo
      // (D10 — the actual fix for the reported "AI never understood the app").
      ev.log(`⚙ Grep(pattern: "${basename(eligible[0])}")`);
      for (const path of eligible) {
        ev.log(`⚙ Read(${path})`, path);
      }

      if (AI_FAIL && eligible.length === 1) {
        failNow(ev, 'Claude exited without a result');
      }

      if (AI_ASK) {
        ev.setTurn(1);
        ev.emit('turnEnd', { costUsd: COST_FIRST_TURN });
        ev.emit('awaitingInput', { text: QUESTION });
        run.awaiting = true;
        const answer = await new Promise<string>((resolve) => {
          run.resolveReply = resolve;
        });
        run.awaiting = false;
        run.resolveReply = null;
        // A cancel while awaiting resolves the same promise (see aiCancelRun): the
        // watchdog is paused there (D3), so Cancel is the ONLY way out.
        if (run.cancelled) cancelNow(ev);
        ev.log(`» answered (${answer.length} bytes)`);
        ev.setTurn(2);
      } else {
        ev.setTurn(1);
      }

      // `?aiFlood` deliberately overruns AI_LOG_MAX (500) so the cap, `logDropped`
      // and the dock's jump-to-latest affordance are all reachable in the harness.
      const ticks = AI_FLOOD ? 700 : AI_SLOW ? 12 : 3;
      const gap = AI_FLOOD ? 1 : AI_SLOW ? 1500 : 200;
      let tokens = 0;
      for (let i = 1; i <= ticks; i++) {
        await delay(gap);
        if (run.cancelled) cancelNow(ev);
        // Assistant prose ⇒ it accumulates into `partialText`.
        ev.log(`analysing… (${i}/${ticks})`, undefined, true);
        // One heartbeat every few lines, cumulative and monotonic like the real CLI.
        if (i % 3 === 0) {
          tokens += 150;
          ev.heartbeat(tokens);
        }
      }
      if (AI_FLOOD) {
        // Exactly MAX_EVENT_TEXT chars with a trailing `…` — Rust's truncate_text
        // shape, which is how the dock knows to show the "truncated" chip.
        ev.log(`${'x'.repeat(MAX_EVENT_TEXT - 1)}…`);
      }

      ev.emit('turnEnd', { costUsd: COST_LAST_TURN });

      const proposals: AiResolveProposal[] = eligible.map((path) => {
        const file = state.conflictTexts.get(path);
        return {
          path,
          // Derived from the marker fixture; state is NOT mutated (D4).
          // `?aiMarkers` hands back the markerful body VERBATIM — exactly what the
          // real single-path stream does with a model that failed to merge — so the
          // frontend's `hasUnresolvedMarkers` gate can be proven, not assumed.
          proposedText:
            file === undefined ? '' : AI_MARKERS ? file.text : stripConflictMarkers(file.text),
          // Per-file cost is unknowable: one run covered them all.
          costUsd: null,
        };
      });
      // Bulk `?aiFail`: ONE path comes back unusable and the rest still resolve.
      if (AI_FAIL && proposals.length > 1) {
        const dropped = proposals.splice(1, 1)[0];
        if (dropped !== undefined) {
          ev.log(`${dropped.path}: no result block returned`, dropped.path);
          failed.push({ path: dropped.path, reason: 'no result block returned' });
        }
      }

      ev.emit('done', { costUsd: COST_RUN });
      return { runId, proposals, failed, costUsd: COST_RUN, turns: AI_ASK ? 2 : 1 };
    } finally {
      // Always released, on every exit path (the Rust FinishGuard).
      runs.delete(runId);
    }
  },

  // IDEMPOTENT (D7): an unknown or already-finished id resolves — a cancel racing a
  // completion is normal, and the UI must not error for clicking a moment too late.
  async aiCancelRun(runId: string): Promise<void> {
    await delay(30);
    const run = runs.get(runId);
    if (run === undefined) return;
    run.cancelled = true;
    // Unblock a run parked on a question; the resolver's `cancelled` check turns
    // this into the cancelled terminal state rather than an answer.
    const pending = run.resolveReply;
    run.resolveReply = null;
    run.awaiting = false;
    pending?.('');
  },

  async aiReplyRun(runId: string, text: string): Promise<void> {
    await delay(20);
    const run = runs.get(runId);
    // A stray reply must never be swallowed into a channel nobody reads.
    if (run === undefined || run.resolveReply === null) {
      throw err('aiFailed', 'run is not awaiting input');
    }
    const resolve = run.resolveReply;
    run.resolveReply = null;
    resolve(text);
  },
} satisfies Partial<IpcApi>;
