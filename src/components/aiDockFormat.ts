/**
 * P68e §9 — the AI activity dock's PURE layer: its prop types and every formatter
 * the five React files share.
 *
 * Nothing here touches React or the DOM, so the header/queue/log components stay
 * presentational and the numbers (elapsed, cost, aggregate urgency) are directly
 * unit-testable in `aiDockFormat.test.ts`.
 *
 * §12-A1 is SATISFIED BY THE STORE: `AiRunLogLine.kind` is classified once at
 * ingest in `repoWorkspace/aiRunLog.ts`, so the contract's `classifyLogLine`
 * fallback is NOT re-implemented here — it is re-exported from its one home so the
 * dock and its tests have a single import site and the rule lives in one place.
 */
import { AI_DOCK_HEIGHT_MAX, AI_DOCK_HEIGHT_MIN } from '../settings/ranges';
import { classifyLogLine } from './repoWorkspace/aiRunLog';
import type { AiRunLogLine, AiRunStatus } from './repoWorkspace/useAiRuns';
import type { PanelDensity } from '../ipc';

export { classifyLogLine };
export { AI_EVENT_TEXT_MAX } from './repoWorkspace/aiRunLog';
export { AI_DOCK_HEIGHT_MAX, AI_DOCK_HEIGHT_MIN };

/** §8: the double-click reset value and the persisted default. */
export const AI_DOCK_HEIGHT_DEFAULT = 180;

/** §8 keyboard nudge (ArrowUp grows the dock). */
export const AI_DOCK_NUDGE_PX = 8;

// ---------------------------------------------------------------- prop types

export interface AiActivityFile {
  path: string;
  status: 'pending' | 'ready' | 'failed';
  /** Rendered as the queue row's `reason` column. */
  error: string | null;
  /**
   * P68f — is there a body to look at, INDEPENDENT of `status`? A `failed` file can
   * still carry one: under `autoResolve` the markerful safety gate demotes every
   * marker-carrying body to `failed`, and bulk only auto-opens `markerful[0]`, so files
   * 2..N of a bulk run have a real draft the user paid for and no other way to reach it.
   * The queue row offers `Review` whenever this is true (`Retry` stays alongside).
   */
  hasProposal: boolean;
}

/**
 * One dock entry. Keyed by an OPAQUE run key (`conflict:<path>`, `bulk:<ts>`,
 * later `analyze:<oid>`) so the other six AI runners can adopt the dock without a
 * prop redesign (D14) — which is also why `AiOutputPanel.tsx` is untouched.
 */
export interface AiActivityRun {
  key: string;
  label: string;
  status: AiRunStatus;
  elapsedMs: number;
  costUsd: number | null;
  question: string | null;
  error: string | null;
  partialText: string | null;
  log: AiRunLogLine[];
  logDropped: number;
  files: AiActivityFile[];
  /** Requested paths — the single-run header `Review proposal` button has no queue
   *  row to click (§12-A2). */
  paths: string[];
  /** Drives the immediate `Stopping…` feedback (§6), before any IPC resolves. */
  cancelRequested: boolean;
  /** Last seen `AiRunEvent.turn`; the header counter shows it from 2 up. */
  turn: number;
  /** Live cumulative thinking-token estimate — the only spend signal that exists
   *  before the first `costUsd`. NEVER priced (§12-B1). */
  thinkingTokens: number | null;
  /** M1: did Bonsai actually open this proposal in the center pane? FOLD-IN 1 can
   *  suppress that open, and the dock must not claim otherwise (§5.1-3). */
  openedInPane: boolean;
}

export interface AiActivityPanelProps {
  /** Newest first. `[]` ⇒ the component renders `null` (§13.1-1). */
  runs: AiActivityRun[];
  activeKey: string | null;
  onSelectRun(key: string): void;
  collapsed: boolean;
  onToggleCollapsed(next: boolean): void;
  /** 120..600, persisted as `aiDockHeight`. */
  height: number;
  onResizeHeight(next: number): void;
  onCancel(key: string): void;
  onReply(key: string, text: string): void;
  onDismiss(key: string): void;
  onReviewFile(key: string, path: string): void;
  onRetryFile(key: string, path: string): void;
  /** `panelDensity` → `data-density` on the root (U12). */
  density: PanelDensity;
  /** `UiSettings.aiStreamLog`; without it an empty log reads as "broken again". */
  streamLogEnabled: boolean;
  /** Store `atCapacity` → the `N of N running` chip in the run strip. */
  atCapacity: boolean;
}

// ---------------------------------------------------------------- formatters

/**
 * `m:ss`, or `h:mm:ss` past an hour. Known answers (§13.1-5): `0` → `0:00`,
 * `7_400` → `0:07`, `725_000` → `12:05`, `3_723_000` → `1:02:03`.
 */
export function formatElapsed(ms: number): string {
  const total = Number.isFinite(ms) ? Math.max(0, Math.floor(ms / 1000)) : 0;
  const secs = String(total % 60).padStart(2, '0');
  const mins = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  if (hours === 0) return `${mins}:${secs}`;
  return `${hours}:${String(mins).padStart(2, '0')}:${secs}`;
}

/** U13: `$—` while unknown — a guess would be worse than nothing. */
export function formatCost(costUsd: number | null): string {
  if (costUsd === null || !Number.isFinite(costUsd)) return '$—';
  return `$${costUsd.toFixed(costUsd >= 1 ? 2 : 4)}`;
}

/** The `$—` explanation (U13); the real cost only lands on `turnEnd`/`done`. */
export const COST_UNKNOWN_TITLE = 'Cost appears when Claude finishes a turn';

/**
 * §12-B1, THE MITIGATION. `$—` is only acceptable while SOMETHING moves: the user
 * accepted "no default spend cap" on the basis that spend is visible, and `costUsd`
 * does not exist until the first turn boundary — on a 4-minute single-turn run that is
 * four minutes of nothing.
 *
 * The CLI's `thinking_tokens` heartbeat is the only live spend signal that exists
 * before then, so it is rendered beside the cost. THREE HONEST LIMITS, kept
 * deliberately: it counts thinking tokens ONLY (not input/output), it is the CLI's own
 * ESTIMATE (hence `~`), and it is ABSENT on a run that never reports one. It is NEVER
 * priced — no price table, no derived dollar figure, anywhere (§12-B1 rejected option
 * (c): an invented number is worse than none).
 */
export function formatThinkingTokens(tokens: number | null): string | null {
  if (tokens === null || !Number.isFinite(tokens) || tokens <= 0) return null;
  return `~${Math.round(tokens).toLocaleString()} tok`;
}

/** The `~N tok` explanation. States the estimate and refuses to imply a price. */
export const THINKING_TOKENS_TITLE =
  'Thinking tokens so far (Claude’s own estimate, not a price)';

export interface AiDockPill {
  glyph: string;
  label: string;
  /** `data-status` — drives the tint/border hue in one CSS rule (§10). */
  dataStatus: 'running' | 'stopping' | 'awaiting' | 'ready' | 'failed' | 'cancelled';
}

/**
 * §2 LOCKED copy. Colour never carries meaning alone (U8): every state is a word,
 * and the glyph matches the P68d conflict-row glyphs.
 */
export function pillFor(status: AiRunStatus, cancelRequested: boolean): AiDockPill {
  switch (status) {
    case 'running':
      return cancelRequested
        ? { glyph: '✨', label: 'Stopping…', dataStatus: 'stopping' }
        : { glyph: '✨', label: 'Running', dataStatus: 'running' };
    case 'awaitingInput':
      return { glyph: '?', label: 'Needs you', dataStatus: 'awaiting' };
    case 'ready':
      return { glyph: '✓', label: 'Ready', dataStatus: 'ready' };
    case 'failed':
      return { glyph: '⚠', label: 'Failed', dataStatus: 'failed' };
    case 'cancelled':
      return { glyph: '⊘', label: 'Cancelled', dataStatus: 'cancelled' };
  }
}

export function isLiveStatus(status: AiRunStatus): boolean {
  return status === 'running' || status === 'awaitingInput';
}

/** §1.2: which status wins the collapsed bar when several runs exist. */
const URGENCY: Record<AiRunStatus, number> = {
  awaitingInput: 0,
  running: 1,
  failed: 2,
  ready: 3,
  cancelled: 4,
};

export interface AiDockAggregate {
  status: AiRunStatus;
  cancelRequested: boolean;
  elapsedMs: number;
  /** Sum over the listed runs — separate processes, so summing is correct here;
   *  A10 only forbids summing WITHIN one run. */
  costUsd: number | null;
  /** Same rule as `costUsd`: summed ACROSS runs, never within one (§12-B1). */
  thinkingTokens: number | null;
  /** Latest log line of the longest-running active run. */
  latest: string | null;
  /** Present only when exactly one run is active (§1.2). */
  cancelKey: string | null;
}

/**
 * Collapse several runs into the one bar the user cannot miss (U2). `runs` must be
 * non-empty; the caller renders `null` for `[]`.
 */
export function aggregateRuns(runs: AiActivityRun[]): AiDockAggregate {
  const sorted = [...runs].sort((a, b) => URGENCY[a.status] - URGENCY[b.status]);
  const lead = sorted[0];
  const live = runs.filter((r) => isLiveStatus(r.status));
  // The longest-running active run carries elapsed + the activity line; with none
  // active, the most recent run's frozen elapsed stands.
  const longest = live.reduce<AiActivityRun | null>(
    (best, r) => (best === null || r.elapsedMs > best.elapsedMs ? r : best),
    null,
  );
  const clock = longest ?? runs[0];
  const costs = runs.filter((r) => r.costUsd !== null);
  const thinking = runs.filter((r) => r.thinkingTokens !== null);
  return {
    status: lead?.status ?? 'running',
    cancelRequested: lead?.cancelRequested ?? false,
    elapsedMs: clock?.elapsedMs ?? 0,
    costUsd: costs.length === 0 ? null : costs.reduce((sum, r) => sum + (r.costUsd ?? 0), 0),
    thinkingTokens:
      thinking.length === 0
        ? null
        : thinking.reduce((sum, r) => sum + (r.thinkingTokens ?? 0), 0),
    latest: longest === null ? null : (longest.log[longest.log.length - 1]?.text ?? null),
    cancelKey: live.length === 1 ? (live[0]?.key ?? null) : null,
  };
}

/**
 * §11 — the WHOLE announcement list, as a pure-ish transition table.
 *
 * `seen` is a caller-owned accumulator of "status token last announced per run key"
 * (a ref in the panel); it is updated in place so the function stays O(runs) and the
 * component needs no extra state. Returns the ONE sentence to announce, or `null`
 * when nothing transitioned.
 *
 * What is never announced: log lines, the elapsed timer, cost updates, turn changes,
 * queue-row transitions, scroll position (U4).
 */
export function announceFor(runs: AiActivityRun[], seen: Map<string, string>): string | null {
  let message: string | null = null;
  const keys = new Set<string>();
  for (const run of runs) {
    keys.add(run.key);
    // `cancelRequested` is part of the token so "Stopping…" gets its own sentence
    // without a second bookkeeping map.
    const token = `${run.status}:${run.cancelRequested ? 'stopping' : ''}`;
    if (seen.get(run.key) === token) continue;
    const first = !seen.has(run.key);
    seen.set(run.key, token);
    message = sentenceFor(run, first);
  }
  for (const key of [...seen.keys()]) if (!keys.has(key)) seen.delete(key);
  return message;
}

function sentenceFor(run: AiActivityRun, first: boolean): string | null {
  if (run.status === 'running') {
    if (run.cancelRequested) return `Stopping the AI run for ${run.label}`;
    return first ? `AI run started for ${run.label}` : null;
  }
  if (run.status === 'awaitingInput') return `Claude needs your answer about ${run.label}`;
  if (run.status === 'cancelled') return 'AI run cancelled. Nothing was changed.';
  if (run.status === 'failed') return `AI run failed: ${run.error ?? 'unknown error'}`;
  const ready = run.files.filter((f) => f.status === 'ready').length;
  if (run.files.length > 1) return `AI proposals ready for ${ready} of ${run.files.length} files`;
  return `AI proposal ready for ${run.label}`;
}

/** §8: the effective max never lets the dock swallow the graph on a short window. */
export function clampDockHeight(next: number, viewportHeight: number): number {
  const max = Math.min(
    AI_DOCK_HEIGHT_MAX,
    Math.max(AI_DOCK_HEIGHT_MIN, Math.round(viewportHeight * 0.6)),
  );
  return Math.min(max, Math.max(AI_DOCK_HEIGHT_MIN, Math.round(next)));
}
