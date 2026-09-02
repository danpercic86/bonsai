/** Pure hit-test resolution for the graph canvas (T3.6 split from
 *  GraphCanvas.tsx). Plain data in (coordinates + laid-out label geometry) →
 *  plain data out; zero canvas/DOM imports. All logic moved VERBATIM from the
 *  component — behavior-preserving. */

import type { RefLabel } from '../ipc';
import type { ForgeCellLayout } from './forgeBadges';
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

/** P92 §1.5: minimum comfortable pointer target for the (canvas-drawn) chip. A
 *  narrow `+2` chip paints ~16px wide; the HIT box — not the paint — is widened
 *  to this. Shared by hover (tooltip) and click (ref picker) for free. */
const CHIP_MIN_HIT_W = 24;

/** The "+n" overflow chip under `x`, if any (entity === null ⇒ the chip).
 *  P92: the hit box is padded outward (up to {@link CHIP_MIN_HIT_W}) when the
 *  painted chip is narrower; the paint is untouched. */
export function chipHitAt(laid: readonly LaidRefLabel[], x: number): LaidRefLabel | undefined {
  return laid.find((l) => {
    if (l.entity !== null) return false;
    const pad = Math.max(0, (CHIP_MIN_HIT_W - l.w) / 2);
    return x >= l.x - pad && x <= l.x + l.w + pad;
  });
}

/** P92: the entities the row's ref band could NOT show — the ones the "+n" chip
 *  stands for. `laid` is the output of `layoutRefLabels` for the same entities;
 *  shown pills carry an `entity`, so the hidden slice starts at their count.
 *  Shared by the hover tooltip and the chip's ref-picker menu. PURE. */
export function hiddenEntities(
  entities: readonly RefEntity[],
  laid: readonly LaidRefLabel[],
): RefEntity[] {
  const shown = laid.filter((l) => l.entity !== null).length;
  return entities.slice(shown);
}

/** The SHOWN entity pill under `x`, if any (first match in laid order). */
export function pillHitAt(laid: readonly LaidRefLabel[], x: number): LaidRefLabel | undefined {
  return laid.find((l) => l.entity !== null && x >= l.x && x <= l.x + l.w);
}

// ---------- forge-column hit resolution (PR-badge-placement §6) ----------

/** A forge-signal hit inside the FORGE column: the PR pill (checked first — the
 *  click/tooltip target) or the CI dot (tooltip-only). The rects never overlap
 *  (the PR pill sits right of the dot), so PR-before-CI is only a tie-breaker. */
export type ForgeHit =
  | { kind: 'pr'; pr: NonNullable<ForgeCellLayout['pr']> }
  | { kind: 'ci'; ci: NonNullable<ForgeCellLayout['ci']> };

/** PR-badge-placement §6: resolve a forge-signal under `x` within a row's laid
 *  forge cell (from `layoutForgeCell`). The PR pill is `[x, x+w]`; the CI dot is
 *  a `ciBadgeSize`-wide box centered at `cx`. Returns `null` when `x` falls in
 *  neither (blank rail cell, or gap between the dot and the pill). PURE. */
export function forgeHitAt(
  cell: ForgeCellLayout,
  x: number,
  ciBadgeSize: number,
): ForgeHit | null {
  const pr = cell.pr;
  if (pr !== null && x >= pr.x && x <= pr.x + pr.w) return { kind: 'pr', pr };
  const ci = cell.ci;
  if (ci !== null) {
    const half = ciBadgeSize / 2;
    if (x >= ci.cx - half && x <= ci.cx + half) return { kind: 'ci', ci };
  }
  return null;
}

/** PR-badge-placement §6: build the hover-tooltip target for a forge-column hit
 *  (PR pill → 2-line `PR #n (state)` + title; CI dot → 1-line checks rollup), or
 *  `null` when `x` hits neither. Keeps the tooltip copy + anchor geometry out of
 *  the component. PURE. */
export function forgeTooltipTarget(
  cell: ForgeCellLayout,
  x: number,
  ciBadgeSize: number,
  cy: number,
  pillHeight: number,
): TooltipState | null {
  const hit = forgeHitAt(cell, x, ciBadgeSize);
  if (hit === null) return null;
  if (hit.kind === 'pr') {
    const pr = hit.pr;
    const state = pr.badge.isDraft ? 'draft' : pr.badge.state;
    return {
      kind: 'pr',
      lines: [`PR #${pr.badge.number} (${state})`, pr.badge.title],
      anchor: { left: pr.x, top: cy - pillHeight / 2, width: pr.w, height: pillHeight },
    };
  }
  const half = ciBadgeSize / 2;
  const b = hit.ci.badge;
  return {
    kind: 'ci',
    lines: [`Checks: ${b.passed} passed, ${b.failed} failed, ${b.pending} pending`],
    anchor: { left: hit.ci.cx - half, top: cy - half, width: ciBadgeSize, height: ciBadgeSize },
  };
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
