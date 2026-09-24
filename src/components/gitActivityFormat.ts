/**
 * P87b §1/§4/§8 — the git-activity dock's PURE layer: LOCKED copy + every
 * formatter View C and View D share. Nothing here touches the DOM, so the phase
 * strings, pills, readouts and announcer sentences are directly unit-testable.
 *
 * The backend emits structured `category × phase{kind,hook}` only; every human
 * string is derived HERE (P87-ui §1 is the canonical table).
 */
import { CATEGORY_META, TARGET_PREPOSITION } from './gitActivityCategories';
import type { CategoryMeta } from './gitActivityCategories';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';
import type { GitActivityCategory, GitPhase, GitRunOutcome } from '../ipc';

/** The category's copy + glyph (table in `gitActivityCategories.ts`). */
export function categoryMeta(category: GitActivityCategory): CategoryMeta {
  return CATEGORY_META[category];
}

// ---------------------------------------------------------------- geometry (§3.1)

export const GIT_DOCK_HEIGHT_MIN = 120;
export const GIT_DOCK_HEIGHT_MAX = 600;
export const GIT_DOCK_HEIGHT_DEFAULT = 180;
export const GIT_DOCK_NUDGE_PX = 8;

/** §3.1: the effective max never lets the dock swallow the graph on a short window. */
export function clampGitDockHeight(next: number, viewportHeight: number): number {
  const max = Math.min(
    GIT_DOCK_HEIGHT_MAX,
    Math.max(GIT_DOCK_HEIGHT_MIN, Math.round(viewportHeight * 0.6)),
  );
  return Math.min(max, Math.max(GIT_DOCK_HEIGHT_MIN, Math.round(next)));
}

/** The seven P87 categories whose phase rows are LOCKED byte-for-byte (§1). */
const P87_CATEGORIES: ReadonlySet<GitActivityCategory> = new Set<GitActivityCategory>([
  'commit',
  'amend',
  'mergeCommit',
  'push',
  'forcePush',
  'fetch',
  'pull',
]);

/**
 * §1 LOCKED table. `category × phase{kind,hook}` → the user string, sentence
 * case, trailing `…` while in flight. Generic fallbacks: unknown hook →
 * `Running <hook> hook…`; anything else → `Working…`.
 *
 * P119-ui §4.2-4: every NEW category reads its participle for `preparing` /
 * `finalizing` and `networkLabel ?? participle` for `network` (a 3 s checkout
 * is not "Preparing…"); `runningHook` is shared.
 */
export function phaseLabel(category: GitActivityCategory, phase: GitPhase): string {
  if (!P87_CATEGORIES.has(category) && phase.kind !== 'runningHook') {
    const meta = categoryMeta(category);
    return phase.kind === 'network' ? (meta.networkLabel ?? meta.participle) : meta.participle;
  }
  switch (phase.kind) {
    case 'preparing':
      return 'Preparing…';
    case 'runningHook':
      return phase.hook !== undefined && phase.hook !== ''
        ? `Running ${phase.hook} hook…`
        : 'Working…';
    case 'network':
      if (category === 'push') return 'Sending objects…';
      if (category === 'forcePush') return 'Force-pushing…';
      if (category === 'fetch' || category === 'pull') return 'Fetching…';
      return 'Working…';
    case 'finalizing':
      if (category === 'commit') return 'Writing commit…';
      if (category === 'amend') return 'Amending…';
      if (category === 'mergeCommit') return 'Writing merge commit…';
      if (category === 'pull') return 'Pulling…';
      return 'Finalizing…';
    default:
      return 'Working…';
  }
}

// ---------------------------------------------------------------- status pills

export interface GitStatusPill {
  glyph: string;
  label: string;
  /** `data-status` drives the local `--h` hue in one CSS rule (§4.4). */
  dataStatus: 'running' | 'success' | 'failed' | 'conflicts';
}

/** §4.4: run pill. `●` running (accent) / `✓` success / `⚠` failed / `!`
 *  conflicts (P119-ui §4.5, `--warning`) — word + glyph, colour never alone. */
export function statusPill(status: GitActivityRun['status']): GitStatusPill {
  switch (status) {
    case 'running':
      return { glyph: '●', label: 'Running', dataStatus: 'running' };
    case 'success':
      return { glyph: '✓', label: 'Success', dataStatus: 'success' };
    case 'failed':
      return { glyph: '⚠', label: 'Failed', dataStatus: 'failed' };
    case 'conflicts':
      return { glyph: '!', label: 'Conflicts', dataStatus: 'conflicts' };
  }
}

// ---------------------------------------------------------------- outcome (P119-ui §4.7)

/** The visible outcome detail of a SUCCESSFUL run, or null. `conflicts` has no
 *  detail (its pill already says it). One string set serves merge, the rebase
 *  family and checkout's auto fast-forward — no per-category copy. */
export function outcomeLabel(outcome: GitRunOutcome | null): string | null {
  switch (outcome) {
    case 'fastForwarded':
      return 'Fast-forwarded';
    case 'merged':
      return 'Merge commit created';
    case 'upToDate':
      return 'Already up to date';
    default:
      return null;
  }
}

/** The run's outcome detail as rendered (success runs only), or null. */
export function runOutcomeDetail(run: GitActivityRun): string | null {
  return run.status === 'success' ? outcomeLabel(run.outcome) : null;
}

export interface GitHookPill {
  glyph: string;
  label: string;
  dataStatus: 'success' | 'failed';
}

/** §4.4: hook verdict pill — the exit code is INSIDE the label so it is never
 *  colour-only. `code === null` → `⊘ killed` (defensive; no cancel path yet). */
export function hookPill(code: number | null, success: boolean): GitHookPill {
  if (code === null) return { glyph: '⊘', label: 'killed', dataStatus: 'failed' };
  return success
    ? { glyph: '✓', label: `exit ${code}`, dataStatus: 'success' }
    : { glyph: '⚠', label: `exit ${code}`, dataStatus: 'failed' };
}

// ---------------------------------------------------------------- progress

/** §2.3/§14.10: the determinate bar fraction, or `null` (→ indeterminate). Always
 *  guards `totalObjects === 0`. */
export function progressFraction(run: GitActivityRun): number | null {
  const p = run.progress;
  return p !== null && p.totalObjects > 0 ? p.receivedObjects / p.totalObjects : null;
}

/** P119-ui §2.2: the dock bar's `--progress`, clamped to [0, 1] (`received` can
 *  momentarily exceed `total` on some servers), or null → indeterminate sweep. */
export function dockBarFraction(run: GitActivityRun | null): number | null {
  const f = run !== null ? progressFraction(run) : null;
  return f === null ? null : Math.min(1, Math.max(0, f));
}

/** §8: `4.2 MB`, thousands not shown for bytes (SI-ish, base 1024). */
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let value = bytes;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  const rounded = unit === 0 ? Math.round(value) : Math.round(value * 10) / 10;
  return `${rounded} ${units[unit]}`;
}

/**
 * §2.3/§14.10: the count/byte readout, or `null` (caller falls back to
 * `phaseLabel`). `12,340 / 50,000 objects` when totals are known, else
 * `4.2 MB received`, else `null`.
 */
export function objectsReadout(run: GitActivityRun): string | null {
  const p = run.progress;
  if (p === null) return null;
  if (p.totalObjects > 0) {
    return `${p.receivedObjects.toLocaleString()} / ${p.totalObjects.toLocaleString()} objects`;
  }
  if (p.receivedBytes > 0) return `${formatBytes(p.receivedBytes)} received`;
  return null;
}

// ---------------------------------------------------------------- time

/** Live/terminal elapsed. `<0.1s` floor (P119-ui §4.2-5 — an instant op must
 *  not read `0.0s`, "did not run"), `2.4s` under a minute, then `m:ss`. */
export function durationLabel(run: GitActivityRun, now: number): string {
  const ms = Math.max(0, (run.endedAt ?? now) - run.startedAt);
  if (ms < 100) return '<0.1s';
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  const total = Math.floor(ms / 1000);
  const mins = Math.floor(total / 60);
  const secs = String(total % 60).padStart(2, '0');
  return `${mins}:${secs}`;
}

/** `HH:MM` local (§3.4). */
export function timeLabel(ms: number): string {
  const d = new Date(ms);
  return `${String(d.getHours()).padStart(2, '0')}:${String(d.getMinutes()).padStart(2, '0')}`;
}

/** Full local date-time for the timestamp `title`. */
export function timeTitle(ms: number): string {
  return new Date(ms).toLocaleString();
}

// ---------------------------------------------------------------- run target (FU-1)

/**
 * FU-1 §3.3 — the row/bar target text, or null for "this run has no target worth
 * showing".
 *
 * PURE FORMATTER. It never sanitizes, truncates or rewrites `run.target`: the
 * backend (and the mock, which mirrors it) owns that funnel, so a regression
 * there must surface here rather than being masked (run-target §7).
 *
 * The only derived string is `all remotes` — the backend sends `null` for a
 * fetch-all because it must never send a human phrase (§3.3, §3.6-1).
 */
export function runTarget(run: GitActivityRun): string | null {
  if (run.target !== null) return run.target;
  return run.category === 'fetch' ? 'all remotes' : null;
}

/** P119-ui §4.4 — the noun slot: `countNoun(n)` when the run acted on ≥2 items
 *  (`Delete 3 branches`), else the category noun. */
export function runNoun(run: GitActivityRun): string {
  return countWords(run) ?? categoryMeta(run.category).noun;
}

/** The count phrase for a multi-target run, or null. */
function countWords(run: GitActivityRun): string | null {
  const countNoun = categoryMeta(run.category).countNoun;
  return run.targetCount !== null && countNoun !== undefined ? countNoun(run.targetCount) : null;
}

/** Join a noun/verb to the run's target with the category's preposition
 *  (`Push to origin/main`), or side by side when it has none (`Delete branch
 *  topic`). No target → the bare word. */
function withTarget(run: GitActivityRun, word: string): string {
  const target = runTarget(run);
  if (target === null) return word;
  const prep = TARGET_PREPOSITION[run.category];
  return prep === null ? `${word} ${target}` : `${word} ${prep} ${target}`;
}

/** `Push` / `Push to origin/main` — the noun with its target, in words. */
function nounWithTarget(run: GitActivityRun): string {
  return withTarget(run, runNoun(run));
}

/** `Push` / `Push to origin/main` using the VERB (the announcer's phrasing). */
function verbWithTarget(run: GitActivityRun): string {
  return withTarget(run, countWords(run) ?? categoryMeta(run.category).verb);
}

/** Elapsed spelled out for a screen reader — `1.2 seconds`, `2 minutes 5 seconds`.
 *  (`durationLabel`'s `2:05` reads as a time of day.) */
function durationWords(run: GitActivityRun, now: number): string {
  const ms = Math.max(0, (run.endedAt ?? now) - run.startedAt);
  if (ms < 100) return 'under 0.1 seconds';
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)} seconds`;
  const total = Math.floor(ms / 1000);
  const mins = Math.floor(total / 60);
  const secs = total % 60;
  const minPart = `${mins} ${mins === 1 ? 'minute' : 'minutes'}`;
  if (secs === 0) return minPart;
  return `${minPart} ${secs} ${secs === 1 ? 'second' : 'seconds'}`;
}

/**
 * §3.7 — the run row's accessible name: the visible row, in words, in reading
 * order. Built EXPLICITLY because the target makes the row two-plus text spans
 * and name computation over siblings can insert a separating space (the
 * `Git config , repository` failure in `ui-reference.md` §11).
 *
 * `Push to origin/main — success, 1.2 seconds, 14:32`
 * `Push to origin/main — running, sending objects, 2.4 seconds`
 * `Push to origin/main — running, pre-push hook, 0.4 seconds`
 *
 * A running row carries the PHASE word, not the live `objectsReadout` counts: a
 * name that changes on every progress tick is announcer churn. The `⋯ trimmed`
 * chip is deliberately absent — §3.1's push row has the chip and §3.7's name for
 * that same row does not.
 */
export function runRowName(run: GitActivityRun, now: number): string {
  const parts = [statusPill(run.status).label.toLowerCase()];
  if (run.status === 'running') {
    // §3.7-1: the status word for a running row IS `running`, and the hook phase
    // labels also start with it (`Running pre-push hook…`), which linearized to
    // `— running, running pre-push hook,`. Drop one leading `running ` from the
    // phase clause. Anchored, so `working`/`preparing`/`sending objects` are
    // untouched. Only the accessible name de-duplicates: the VISIBLE bar keeps
    // the repeat, because there the pill is a chip and the phase is muted text
    // two type steps apart — a linear name has no such chunking.
    parts.push(
      phaseLabel(run.category, run.phase)
        .replace(/…$/, '')
        .toLowerCase()
        .replace(/^running /, ''),
    );
  }
  // P119-ui §4.7: the outcome clause goes right after the status word.
  const detail = runOutcomeDetail(run);
  if (detail !== null) parts.push(detail.toLowerCase());
  parts.push(durationWords(run, now));
  if (run.status !== 'running' && run.endedAt !== null) parts.push(timeLabel(run.endedAt));
  return `${nounWithTarget(run)} — ${parts.join(', ')}`;
}

// ---------------------------------------------------------------- announcer

const PHASE_TOKEN = (phase: GitPhase): string => `${phase.kind}:${phase.hook ?? ''}`;

/**
 * §6 — the ONE polite announcer for both View C and View D. Announces the active
 * run's phase transitions and terminal result ONLY (never output lines, never
 * `progress` ticks). `seen` is a caller-owned accumulator of the last token per
 * run id; returns the single sentence to announce, or `null`.
 */
export function gitAnnounceFor(runs: GitActivityRun[], seen: Map<string, string>): string | null {
  let message: string | null = null;
  const ids = new Set<string>();
  for (const run of runs) {
    ids.add(run.id);
    const token = run.status === 'running' ? `running:${PHASE_TOKEN(run.phase)}` : run.status;
    if (seen.get(run.id) === token) continue;
    seen.set(run.id, token);
    const sentence = sentenceFor(run);
    if (sentence !== null) message = sentence;
  }
  for (const id of [...seen.keys()]) if (!ids.has(id)) seen.delete(id);
  return message;
}

function sentenceFor(run: GitActivityRun): string | null {
  // §3.7: TERMINAL results carry the target (`Push to origin/main failed`);
  // phase transitions stay bare, because repeating the target on every phase
  // change is the hostile verbosity §6 forbids.
  if (run.status === 'success') {
    const detail = runOutcomeDetail(run);
    return `${verbWithTarget(run)} finished — ${detail !== null ? detail.toLowerCase() : 'success'}`;
  }
  if (run.status === 'conflicts') return `${verbWithTarget(run)} stopped — conflicts to resolve`;
  if (run.status === 'failed') return `${verbWithTarget(run)} failed`;
  // running: announce the meaningful phase transitions, not the initial preparing.
  if (run.phase.kind === 'preparing') return null;
  return phaseLabel(run.category, run.phase).replace(/…$/, '');
}
