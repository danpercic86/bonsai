/** Pure hit-test resolution for the graph canvas (T3.6 split from
 *  GraphCanvas.tsx). Plain data in (coordinates + laid-out label geometry) →
 *  plain data out; zero canvas/DOM imports. All logic moved VERBATIM from the
 *  component — behavior-preserving. */

import type { RefLabel } from '../ipc';
import type { LaidRefLabel, RefEntity } from './refLabels';
import type { Rect } from './viewport';

/** Row hit-test result: a layout row index, the synthetic WIP row, or none. */
export type HitRow = number | 'wip' | null;

/** raw = floor((y + scrollTop) / RH); raw < wipOffset -> 'wip' (only possible
 * when wipOffset === 1); else row = raw - wipOffset (P1 §9.3). */
export function hitTestRow(
  yCss: number,
  scrollTop: number,
  wipOffset: number,
  nodesLen: number,
  rowHeight: number,
): HitRow {
  const raw = Math.floor((yCss + scrollTop) / rowHeight);
  if (raw < 0) return null;
  if (raw < wipOffset) return 'wip';
  const row = raw - wipOffset;
  return row >= 0 && row < nodesLen ? row : null;
}

/** P7 §5: collapsed-label right-click targeting. A `branch` entity with a local
 *  ref targets the LOCAL branch (its P6 menu is the superset); a remote-only
 *  branch targets its first remote ref; tag/head entities return their own ref
 *  (whose `branchMenuItems` resolves to `[]`, so no menu opens — matches today). */
export function targetRefOf(entity: RefEntity): RefLabel | null {
  if (entity.kind === 'branch') {
    if (entity.hasLocal) return entity.refs.find((r) => r.kind === 'localBranch') ?? null;
    return entity.refs.find((r) => r.kind === 'remoteBranch') ?? null;
  }
  return entity.ref;
}

/** P18b: whole-row branch fallback — the first branch entity's target ref, or
 *  `null` when the row has no branch entity (stash/tag/head-only rows). */
export function fallbackBranchRef(entities: readonly RefEntity[]): RefLabel | null {
  for (const entity of entities) {
    if (entity.kind !== 'branch') continue;
    const ref = targetRefOf(entity);
    if (ref !== null) return ref;
  }
  return null;
}

// ---------- laid-label hit resolution (LEFT ref band) ----------

/** The "+n" overflow chip under `x`, if any (entity === null ⇒ the chip). */
export function chipHitAt(laid: readonly LaidRefLabel[], x: number): LaidRefLabel | undefined {
  return laid.find((l) => l.entity === null && x >= l.x && x <= l.x + l.w);
}

/** The SHOWN entity pill under `x`, if any (first match in laid order). */
export function pillHitAt(laid: readonly LaidRefLabel[], x: number): LaidRefLabel | undefined {
  return laid.find((l) => l.entity !== null && x >= l.x && x <= l.x + l.w);
}

/** P63: the PR badge rect under `x`, if any (the click path — PR only). */
export function prBadgeHitAt(
  laid: readonly LaidRefLabel[],
  x: number,
): NonNullable<NonNullable<LaidRefLabel['signals']>['pr']> | null {
  const hit = laid.find(
    (l) => l.signals?.pr != null && x >= l.signals.pr.x && x <= l.signals.pr.x + l.signals.pr.w,
  );
  return hit?.signals?.pr ?? null;
}

/** P63 hover: a forge-signal badge under `x` — per laid label, the PR pill is
 *  checked before the CI dot (the rects never overlap the pill body; they sit
 *  to its right). Returns the first hit in laid order, or `null`. */
export type SignalHit =
  | { kind: 'pr'; pr: NonNullable<NonNullable<LaidRefLabel['signals']>['pr']> }
  | { kind: 'ci'; ci: NonNullable<NonNullable<LaidRefLabel['signals']>['ci']> };

export function signalHitAt(
  laid: readonly LaidRefLabel[],
  x: number,
  ciBadgeSize: number,
): SignalHit | null {
  for (const l of laid) {
    if (l.signals === null) continue;
    const pr = l.signals.pr;
    if (pr !== null && x >= pr.x && x <= pr.x + pr.w) return { kind: 'pr', pr };
    const ci = l.signals.ci;
    if (ci !== null) {
      const half = ciBadgeSize / 2;
      if (x >= ci.cx - half && x <= ci.cx + half) return { kind: 'ci', ci };
    }
  }
  return null;
}

// ---------- tooltip target identity (P7 §6.1) ----------

/** P7 §6.1: current hover-tooltip target. `avatar` shows the full author name;
 *  `overflow` lists the hidden ref entities of a "+n" chip, one per line;
 *  `ref` shows the full branch name of a shown branch pill; `date` (P51b) shows
 *  the FULL absolute authored + committed timestamps (the inline date is
 *  relative), one per line. */
export type TooltipState =
  | { kind: 'avatar'; text: string; anchor: Rect }
  | { kind: 'overflow'; lines: string[]; anchor: Rect }
  | { kind: 'ref'; text: string; anchor: Rect }
  | { kind: 'date'; lines: string[]; anchor: Rect }
  // P63: PR badge (`["PR #123 (open)", title]`) / CI dot (`["Checks: …"]`).
  | { kind: 'pr'; lines: string[]; anchor: Rect }
  | { kind: 'ci'; lines: string[]; anchor: Rect };

/** P7 §6.1: cheap identity so `setTooltip` only re-renders on a real target
 *  change (kind + content), never per mouse pixel or per scroll frame. */
export function sameTarget(a: TooltipState | null, b: TooltipState | null): boolean {
  if (a === null || b === null) return a === b;
  if (a.kind !== b.kind) return false;
  if (a.kind === 'avatar' && b.kind === 'avatar') return a.text === b.text;
  if (a.kind === 'ref' && b.kind === 'ref') return a.text === b.text;
  if (a.kind === 'overflow' && b.kind === 'overflow') {
    return a.lines.join('␟') === b.lines.join('␟');
  }
  if (a.kind === 'date' && b.kind === 'date') {
    return a.lines.join('␟') === b.lines.join('␟');
  }
  if (a.kind === 'pr' && b.kind === 'pr') {
    return a.lines.join('␟') === b.lines.join('␟');
  }
  if (a.kind === 'ci' && b.kind === 'ci') {
    return a.lines.join('␟') === b.lines.join('␟');
  }
  return false;
}
