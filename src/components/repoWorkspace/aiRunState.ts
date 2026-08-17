/**
 * P68d §C — the AI run STATE SHAPE and the pure transforms over it.
 *
 * Split out of `useAiRuns.ts` (which crossed the ~500-line limit the moment it also
 * owned the buffered flush, the elapsed tick and the autonomy routing): everything
 * here is a plain data type or a total function over one, so the settle/derive/prune
 * arithmetic is testable and readable without React in the picture. The hook keeps
 * the refs, the timers and the IPC.
 *
 * `AiRunState` is deliberately generic over the run KEY (`conflict:<path>`,
 * `bulk:<startedAt>`, later `analyze:<oid>`) so the other six AI runners can adopt
 * the same store and the P68e dock without a prop redesign (D14).
 */
import { hasUnresolvedMarkers } from '../../utils/conflictRegions';
import type { AiResolveBatch } from '../../ipc';
import type { AiRunLogLine } from './aiRunLog';

/** One AI run's lifecycle, keyed independently of any UI slot. */
export type AiRunStatus = 'running' | 'awaitingInput' | 'ready' | 'failed' | 'cancelled';

/** `ready` / `failed` / `cancelled` — a terminal status is never overwritten. */
const TERMINAL: readonly AiRunStatus[] = ['ready', 'failed', 'cancelled'];

/** How many finished runs stay in the store (P68e §12-A4). */
export const AI_TERMINAL_RUNS_MAX = 6;

export function isTerminalStatus(status: AiRunStatus): boolean {
  return TERMINAL.includes(status);
}

export function conflictKey(path: string): string {
  return `conflict:${path}`;
}

export interface AiRunFileState {
  path: string;
  /** 'pending' until the batch resolves, then one of the terminal two. */
  status: 'pending' | 'ready' | 'failed';
  proposal: string | null;
  error: string | null;
}

export interface AiRunState {
  /** `conflict:<path>` | `bulk:<startedAt>`; generalises to `analyze:<oid>` (D14). */
  key: string;
  /** Dock header label: the path, or `"<n> conflicts"`. */
  label: string;
  paths: string[];
  /** null until the `started` event lands (D8). */
  runId: string | null;
  /** A cancel asked for before `runId` existed — fired the moment it arrives (D8).
   *  Also the dock's immediate `Stopping…` feedback (P68e §12-A2). */
  cancelRequested: boolean;
  status: AiRunStatus;
  log: AiRunLogLine[];
  /** Lines dropped off the front by the AI_LOG_MAX cap. */
  logDropped: number;
  question: string | null;
  /** Single-run proposal (`paths.length === 1`). */
  proposal: string | null;
  files: AiRunFileState[];
  error: string | null;
  /** LAST result's value within a run; summed across sequential bulk batches (A10). */
  costUsd: number | null;
  /** P68d: live cumulative thinking-token estimate from the CLI's heartbeats — the
   *  only spend signal that exists before the first `costUsd`. Never priced. */
  thinkingTokens: number | null;
  /** Last seen `AiRunEvent.turn` (the dock header's turn counter, P68e §12-A3). */
  turn: number;
  /** Display-only partial assistant text on a cancelled/failed run (D2). */
  partialText: string | null;
  startedAt: number;
  endedAt: number | null;
  /** Stale/duplicate guard: an event whose `seq <= lastSeq` is ignored. */
  lastSeq: number;
}

/** Per-path state for the conflict row's affordance (§5.4). */
export interface AiRowState {
  status: AiRunStatus;
  elapsedSecs: number;
  /** The run's key, so a row can address the dock entry (P68e). */
  key: string;
  error: string | null;
}

export function newRun(key: string, label: string, paths: string[], now: number): AiRunState {
  return {
    key,
    label,
    paths,
    runId: null,
    cancelRequested: false,
    status: 'running',
    log: [],
    logDropped: 0,
    question: null,
    proposal: null,
    files: paths.map((path) => ({ path, status: 'pending', proposal: null, error: null })),
    error: null,
    costUsd: null,
    thinkingTokens: null,
    turn: 0,
    partialText: null,
    startedAt: now,
    endedAt: null,
    lastSeq: -1,
  };
}

/** The outcome of turning a resolved batch into per-file state. */
export interface SettledBatch {
  files: AiRunFileState[];
  /** Files that are ready AND marker-free — the only ones that may be staged. */
  stageable: AiRunFileState[];
  /** Files whose body still carries conflict markers (autoResolve only). */
  markerful: AiRunFileState[];
  status: AiRunStatus;
  error: string | null;
  /** Single-path proposal, or null for a bulk run. */
  proposal: string | null;
}

/**
 * THE SAFETY GATE lives here (blocking requirement from the P68b review): nothing
 * markerful may ever be presented as clean.
 *
 * `AiResolveBatch.proposedText` is a REVIEWABLE proposal, not a verified-clean merge.
 * Bulk marks a markerful body `failed` on the Rust side, but a SINGLE-path stream
 * returns the model's body VERBATIM (P13 parity), so this `hasUnresolvedMarkers` pass
 * is the only thing between such a body and a silent `autoResolve` stage — exactly
 * what `useMergeActions.ts:126,141-143` did before P68d, preserved.
 *
 * Under `proposeReview` nothing is staged anyway, so markerful bodies simply open for
 * review (the editor is the gate) and `markerful` comes back empty.
 */
export function settleBatch(
  paths: string[],
  batch: AiResolveBatch,
  autonomy: 'proposeReview' | 'autoResolve',
): SettledBatch {
  const byPath = new Map(batch.proposals.map((p) => [p.path, p] as const));
  const failedBy = new Map(batch.failed.map((f) => [f.path, f.reason] as const));

  const files: AiRunFileState[] = paths.map((path) => {
    const proposal = byPath.get(path);
    if (proposal !== undefined) {
      return { path, status: 'ready', proposal: proposal.proposedText, error: null };
    }
    return {
      path,
      status: 'failed',
      proposal: null,
      error: failedBy.get(path) ?? 'no result returned for this file',
    };
  });

  const ready = files.filter((f) => f.status === 'ready' && f.proposal !== null);
  const markerful =
    autonomy === 'autoResolve' ? ready.filter((f) => hasUnresolvedMarkers(f.proposal ?? '')) : [];
  for (const f of markerful) {
    // Demoted, never staged. The message is the P13 copy, kept verbatim.
    f.status = 'failed';
    f.error = `AI left unresolved markers in ${f.path} — opened for review`;
  }

  const stageable = files.filter((f) => f.status === 'ready' && f.proposal !== null);
  const anyReady = stageable.length > 0;
  return {
    files,
    stageable,
    markerful,
    status: anyReady ? 'ready' : 'failed',
    error: anyReady ? null : (files.find((f) => f.error !== null)?.error ?? 'AI resolve failed'),
    proposal: files.length === 1 ? (files[0]?.proposal ?? null) : null,
  };
}

/**
 * Per-path row affordance input. A path covered by several runs shows the LIVE one;
 * a per-file failure inside an otherwise-ready batch shows on ITS row only.
 */
export function deriveRowStates(
  runs: Record<string, AiRunState>,
  now: number,
): Record<string, AiRowState> {
  const out: Record<string, AiRowState> = {};
  for (const run of Object.values(runs)) {
    const elapsedSecs = Math.max(0, Math.floor(((run.endedAt ?? now) - run.startedAt) / 1000));
    for (const path of run.paths) {
      const file = run.files.find((f) => f.path === path);
      const status: AiRunStatus =
        run.status === 'ready' && file?.status === 'failed' ? 'failed' : run.status;
      const prev = out[path];
      if (prev !== undefined && !isTerminalStatus(prev.status)) continue;
      out[path] = { status, elapsedSecs, key: run.key, error: file?.error ?? run.error };
    }
  }
  return out;
}

/**
 * P68e §12-A4: drop a terminal run once NONE of its paths is conflicted any more, and
 * cap retained terminal runs (oldest first) so the dock does not fill with stale chips
 * across a long merge. Live runs are never pruned.
 *
 * Returns `null` when nothing changed, so the caller can skip a commit.
 */
export function pruneRuns(
  runs: Record<string, AiRunState>,
  conflictPaths: readonly string[],
): { kept: Record<string, AiRunState>; dropped: string[] } | null {
  const conflicted = new Set(conflictPaths);
  const dropped: string[] = [];
  const kept: Record<string, AiRunState> = {};
  for (const [key, run] of Object.entries(runs)) {
    if (isTerminalStatus(run.status) && run.paths.every((p) => !conflicted.has(p))) {
      dropped.push(key);
      continue;
    }
    kept[key] = run;
  }
  const terminal = Object.values(kept)
    .filter((r) => isTerminalStatus(r.status))
    .sort((a, b) => (a.endedAt ?? a.startedAt) - (b.endedAt ?? b.startedAt));
  for (const run of terminal.slice(0, Math.max(0, terminal.length - AI_TERMINAL_RUNS_MAX))) {
    delete kept[run.key];
    dropped.push(run.key);
  }
  return dropped.length === 0 ? null : { kept, dropped };
}
