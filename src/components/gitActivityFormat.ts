/**
 * P87b §1/§4/§8 — the git-activity dock's PURE layer: LOCKED copy + every
 * formatter View C and View D share. Nothing here touches the DOM, so the phase
 * strings, pills, readouts and announcer sentences are directly unit-testable.
 *
 * The backend emits structured `category × phase{kind,hook}` only; every human
 * string is derived HERE (P87-ui §1 is the canonical table).
 */
import type { ComponentType } from 'react';

import { FetchIcon, PullIcon, PushIcon, RefDotIcon } from './appIcons';
import { MergeIcon } from './menuIcons';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';
import type { GitActivityCategory, GitPhase } from '../ipc';

type IconComponent = ComponentType;

export interface CategoryMeta {
  /** Idle button + palette verb ("Push"). */
  verb: string;
  /** Layout-stable busy button participle ("Pushing…"). */
  participle: string;
  /** Terminal row noun ("Push", "Merge commit"). */
  noun: string;
  /** The category glyph icon (reused graph/menu icon). */
  glyph: IconComponent;
}

const CATEGORY_META: Record<GitActivityCategory, CategoryMeta> = {
  push: { verb: 'Push', participle: 'Pushing…', noun: 'Push', glyph: PushIcon },
  forcePush: { verb: 'Force-push', participle: 'Force-pushing…', noun: 'Force-push', glyph: PushIcon },
  fetch: { verb: 'Fetch', participle: 'Fetching…', noun: 'Fetch', glyph: FetchIcon },
  pull: { verb: 'Pull', participle: 'Pulling…', noun: 'Pull', glyph: PullIcon },
  commit: { verb: 'Commit', participle: 'Committing…', noun: 'Commit', glyph: RefDotIcon },
  amend: { verb: 'Amend', participle: 'Amending…', noun: 'Amend', glyph: RefDotIcon },
  mergeCommit: { verb: 'Merge', participle: 'Merging…', noun: 'Merge commit', glyph: MergeIcon },
};

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

/**
 * §1 LOCKED table. `category × phase{kind,hook}` → the user string, sentence
 * case, trailing `…` while in flight. Generic fallbacks: unknown hook →
 * `Running <hook> hook…`; anything else → `Working…`.
 */
export function phaseLabel(category: GitActivityCategory, phase: GitPhase): string {
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
  dataStatus: 'running' | 'success' | 'failed';
}

/** §4.4: run pill. `●` running (accent) / `✓` success / `⚠` failed — word + glyph,
 *  colour never alone. */
export function statusPill(status: GitActivityRun['status']): GitStatusPill {
  switch (status) {
    case 'running':
      return { glyph: '●', label: 'Running', dataStatus: 'running' };
    case 'success':
      return { glyph: '✓', label: 'Success', dataStatus: 'success' };
    case 'failed':
      return { glyph: '⚠', label: 'Failed', dataStatus: 'failed' };
  }
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

/** Live/terminal elapsed. `2.4s` under a minute, then `m:ss`. */
export function durationLabel(run: GitActivityRun, now: number): string {
  const ms = Math.max(0, (run.endedAt ?? now) - run.startedAt);
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
  const meta = categoryMeta(run.category);
  if (run.status === 'success') return `${meta.verb} finished — success`;
  if (run.status === 'failed') return `${meta.verb} failed`;
  // running: announce the meaningful phase transitions, not the initial preparing.
  if (run.phase.kind === 'preparing') return null;
  return phaseLabel(run.category, run.phase).replace(/…$/, '');
}
