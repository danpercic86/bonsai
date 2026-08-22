/**
 * P87b — the git-activity run STATE shape and its pure transforms.
 *
 * Split out of `useGitActivity.ts` (file-size discipline, mirrors `aiRunState.ts`):
 * the store is the only place that interprets the wire event, so the event→state
 * decisions live here as pure functions the store and its tests both import.
 */
import type { GitActivityLine } from './gitActivityLog';
import type {
  GitActivityCategory,
  GitPhase,
  GitTransferProgress,
} from '../../ipc';

export type GitRunStatus = 'running' | 'success' | 'failed';

/** One `hookDone` record — per-hook pass/fail for the row's sub-rows (§3.5-1). */
export interface GitHookRecord {
  hook: string;
  /** `null` = killed / no exit code (defensive; no cancel path exists yet). */
  code: number | null;
  success: boolean;
  at: number;
}

export interface GitActivityRun {
  id: string;
  category: GitActivityCategory;
  /** Current phase — drives View C label + the running row's sub-label. */
  phase: GitPhase;
  status: GitRunStatus;
  code: number | null;
  /** Wall-clock, anchored on event arrival (mirrors the AI dock). */
  startedAt: number;
  endedAt: number | null;
  /** Latest fetch/pull transfer counts (§14.9); null unless a Progress arrived. */
  progress: GitTransferProgress | null;
  /** One per `hookDone` (View D per-hook status). */
  hooks: GitHookRecord[];
  /** Bounded output ring (500 lines); the oldest overflow is counted below. */
  lines: GitActivityLine[];
  /** Count of output lines dropped off the front of the ring (§4.7 chip). */
  linesDropped: number;
  /** Last-seen event `seq` for its id — drop any event whose `seq <= this`. */
  seq: number;
}

/** Session-scoped run cap; newest-first, oldest TERMINAL evicted on overflow. */
export const GIT_ACTIVITY_RUNS_MAX = 200;

export function isTerminalGitStatus(status: GitRunStatus): boolean {
  return status === 'success' || status === 'failed';
}

/** A fresh `running` run from a `started` event. */
export function newGitRun(
  id: string,
  category: GitActivityCategory,
  phase: GitPhase,
  seq: number,
  now: number,
): GitActivityRun {
  return {
    id,
    category,
    phase,
    status: 'running',
    code: null,
    startedAt: now,
    endedAt: null,
    progress: null,
    hooks: [],
    lines: [],
    linesDropped: 0,
    seq,
  };
}

/**
 * Enforce the 200-run cap on a chronological (oldest→newest) list: evict the
 * OLDEST TERMINAL runs first and NEVER a `running` one (store rule §8). Returns
 * the ids that were dropped (so the store can free their line buffers) plus the
 * kept list, or `null` when nothing changed.
 */
export function pruneGitRuns(
  order: string[],
  runs: Map<string, GitActivityRun>,
): { kept: string[]; dropped: string[] } | null {
  if (order.length <= GIT_ACTIVITY_RUNS_MAX) return null;
  const excess = order.length - GIT_ACTIVITY_RUNS_MAX;
  const dropped: string[] = [];
  const kept: string[] = [];
  // Walk oldest→newest, dropping terminal runs until we are back under the cap.
  for (const id of order) {
    const run = runs.get(id);
    if (dropped.length < excess && run !== undefined && isTerminalGitStatus(run.status)) {
      dropped.push(id);
    } else {
      kept.push(id);
    }
  }
  if (dropped.length === 0) return null;
  return { kept, dropped };
}
