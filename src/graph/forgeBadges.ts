/** PURE forge-signal badge subsystem for the commit graph's FORGE column
 *  (PR-badge-placement contract). Defines the per-branch PR / CI badge model,
 *  the state→visual classifiers, the PR-state glyph, the width helper, the
 *  canvas draw fns, the `branchSignals` display-time gate, the row-level
 *  `rowForgeSignal` selector, and the `layoutForgeCell` intra-column geometry.
 *  No React, no IPC — mirrors `verifyBadge.ts` (classifier) + `drawRowText.ts`
 *  (glyph helpers) so it unit-tests headlessly. Both the draw pass (drawRowText)
 *  and the hit-test (GraphCanvas.tsx) consume `rowForgeSignal` + `layoutForgeCell`
 *  so draw and hit-test share one source of truth. */

import type { CheckRollup, GraphNode, PrState, RefLabel } from '../ipc';
import type { Theme } from './colors';
import { FONT_UI, METRICS } from './metrics';
import { groupRefs } from './refLabels';
import type { RefEntity } from './refLabels';
import type { GraphDisplayOptions } from './rightColumns';
import { measure, truncateToWidth } from './textMeasure';

/** Merged/closed PR pill color — fixed across themes (GitHub's merged violet),
 *  same convention as TAG_COLOR / STASH_COLOR. Green (open) + red (closed) come
 *  from the theme badge palette so they track the signature-badge colors. */
const PR_MERGED_COLOR = '#8957e5';

/** Per-branch PR signal (subset of P62 `PrSummary`; `title` feeds the tooltip). */
export interface PrBadge {
  number: number;
  title: string;
  state: PrState;
  isDraft: boolean;
  url: string;
}

/** Per-tip CI signal (subset of P62 `CommitStatus`; counts feed the tooltip). */
export interface CiBadge {
  rollup: CheckRollup;
  passed: number;
  failed: number;
  pending: number;
  total: number;
}

/** Resolved PR-pill visual (mirrors `refLabels.entityStyle`'s shape). A draft PR
 *  (state 'open' + isDraft) reads as a grey OUTLINE pill; otherwise the fill is
 *  tinted by lifecycle state. `label` is always `#<num>`. */
export function prBadgeVisual(
  pr: PrBadge,
  theme: Theme,
): { label: string; fill: string; text: string; border: string | null } {
  const label = `#${pr.number}`;
  if (pr.isDraft) {
    // grey outline — muted, no strong fill (a draft is not yet "real").
    return { label, fill: theme.bg2, text: theme.text2, border: theme.text3 };
  }
  switch (pr.state) {
    case 'open':
      return { label, fill: theme.badgeGood, text: '#ffffff', border: null };
    case 'merged':
      return { label, fill: PR_MERGED_COLOR, text: '#ffffff', border: null };
    case 'closed':
      return { label, fill: theme.badgeWarn, text: '#ffffff', border: null };
  }
}

/** The CI dot's glyph + color, or `null` when NOTHING draws (`none` rollup —
 *  copies `verifyBadgeKind`'s null pattern so an absent-CI tip stays clean). */
export function ciBadgeVisual(
  rollup: CheckRollup,
  theme: Theme,
): { glyph: 'check' | 'x' | 'dot' | 'dash'; color: string } | null {
  switch (rollup) {
    case 'success':
      return { glyph: 'check', color: theme.badgeGood };
    case 'failure':
    case 'error':
      return { glyph: 'x', color: theme.badgeWarn };
    case 'pending':
      return { glyph: 'dot', color: theme.warning };
    case 'neutral':
      return { glyph: 'dash', color: theme.text3 };
    case 'none':
      return null;
  }
}

/** PURE display-time gate: the PR/CI badges a branch entity should carry, from
 *  the display maps. Nulls when the toggle is off, the entity is not a branch,
 *  or nothing is cached. (Compact is already baked in by the caller AND-ing
 *  `!compact` into `showPrBadge`/`showCiStatus`, so no compact plumbing here.) */
export function branchSignals(
  entity: RefEntity,
  node: GraphNode,
  display: GraphDisplayOptions,
): { pr: PrBadge | null; ci: CiBadge | null } {
  const isBranch = entity.kind === 'branch';
  const pr =
    display.showPrBadge && isBranch ? (display.prByBranch.get(entity.name) ?? null) : null;
  const ci = display.showCiStatus && isBranch ? (display.ciBySha.get(node.id) ?? null) : null;
  return { pr, ci };
}

/** PR-badge-placement §3: leading PR-state glyph, the NON-COLOUR carrier of PR
 *  lifecycle (colour alone was a house-rule violation). Drawn inside the pill,
 *  before `#num`, in the pill's label colour. ○ open / ◆ merged / ✕ closed;
 *  a draft is an open-but-unready PR ⇒ same ○ family, distinguished by the
 *  grey outline fill. PURE. */
export function prStateGlyph(pr: PrBadge): string {
  if (pr.isDraft) return '○';
  switch (pr.state) {
    case 'open':
      return '○';
    case 'merged':
      return '◆';
    case 'closed':
      return '✕';
  }
}

/** Gap (px) between the PR-state glyph and the `#num` label inside the pill. */
const PR_GLYPH_GAP = 3;

/** Measured width of a PR badge pill (`2*padX + glyphW + gap + measure("#num")`,
 *  capped at `prBadgeMaxWidth`). Sets `ctx.font` to `pillFont` to measure. PURE. */
export function prBadgeWidth(ctx: CanvasRenderingContext2D, pr: PrBadge): number {
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  const glyphW = Math.ceil(measure(ctx, prStateGlyph(pr)));
  const raw =
    2 * METRICS.prBadgePadX + glyphW + PR_GLYPH_GAP + Math.ceil(measure(ctx, `#${pr.number}`));
  return Math.min(raw, METRICS.prBadgeMaxWidth);
}

/** PR-badge-placement §2.4: the forge signals a ROW should carry. A row may hold
 *  several branch entities; the cell shows the signals of the FIRST branch
 *  entity (in `groupRefs` order: detached-head, then branches local-first) that
 *  has any. Returns `null` when no branch entity carries a signal. Single source
 *  of truth for both `drawRowText` and `forgeHitAt`. PURE. */
export function rowForgeSignal(
  refs: readonly RefLabel[] | undefined,
  node: GraphNode,
  display: GraphDisplayOptions,
  /** PERF: the `groupRefs(refs)` result already computed by the caller (the ref
   *  band pass computes one per row per frame). Pass it to avoid a second
   *  allocation; omitted ⇒ grouped here. */
  groups?: readonly RefEntity[],
): { pr: PrBadge | null; ci: CiBadge | null } | null {
  // A ref-less row can carry no branch signal — skip grouping entirely (the
  // common case: most rows have no refs, so this avoids a Map+array alloc/frame).
  if (refs === undefined || refs.length === 0) return null;
  for (const e of groups ?? groupRefs(refs)) {
    const sig = branchSignals(e, node, display);
    if (sig.pr !== null || sig.ci !== null) return sig;
  }
  return null;
}

/** PR-badge-placement §2.3: laid-out geometry of a row's forge cell, anchored at
 *  the column's `leftX` (pills line up on their LEFT edges). The CI dot is
 *  centered at `cx`; the PR pill spans `[x, x+w]`. Either may be null. */
export interface ForgeCellLayout {
  ci: { badge: CiBadge; cx: number } | null;
  pr: { badge: PrBadge; x: number; w: number } | null;
}

/** PR-badge-placement §2.3: place a row's forge signals inside the column.
 *  CI dot centered at `leftX + ciBadgeSize/2`; the PR pill's left edge follows
 *  the dot+gap (or hugs `leftX` when there is no CI). Sets `ctx.font` (via
 *  `prBadgeWidth`) to measure the pill. PURE. */
export function layoutForgeCell(
  ctx: CanvasRenderingContext2D,
  leftX: number,
  signal: { pr: PrBadge | null; ci: CiBadge | null },
): ForgeCellLayout {
  const ci = signal.ci !== null ? { badge: signal.ci, cx: leftX + METRICS.ciBadgeSize / 2 } : null;
  const prX = leftX + (signal.ci !== null ? METRICS.ciBadgeSize + METRICS.signalGap : 0);
  const pr =
    signal.pr !== null ? { badge: signal.pr, x: prX, w: prBadgeWidth(ctx, signal.pr) } : null;
  return { ci, pr };
}

/** Draw the CI dot centered at `(cx, cy)`. save/restore so the glyph stroke
 *  style (lineJoin/cap/width) never leaks into the ref/summary passes. Mirrors
 *  `drawRowText`'s good/warn badge glyphs. Draws NOTHING for a `none` rollup. */
export function drawCiBadge(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  badge: CiBadge,
  theme: Theme,
): void {
  const v = ciBadgeVisual(badge.rollup, theme);
  if (v === null) return;
  ctx.save();
  if (v.glyph === 'check') {
    ctx.beginPath();
    ctx.arc(cx, cy, 4.6, 0, Math.PI * 2);
    ctx.fillStyle = v.color;
    ctx.fill();
    ctx.beginPath();
    ctx.moveTo(cx - 2.2, cy + 0.1);
    ctx.lineTo(cx - 0.6, cy + 1.9);
    ctx.lineTo(cx + 2.4, cy - 2.1);
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 1.3;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    ctx.stroke();
  } else if (v.glyph === 'x') {
    ctx.beginPath();
    ctx.arc(cx, cy, 4.6, 0, Math.PI * 2);
    ctx.fillStyle = v.color;
    ctx.fill();
    ctx.strokeStyle = '#ffffff';
    ctx.lineWidth = 1.3;
    ctx.lineCap = 'round';
    ctx.beginPath();
    ctx.moveTo(cx - 2, cy - 2);
    ctx.lineTo(cx + 2, cy + 2);
    ctx.moveTo(cx + 2, cy - 2);
    ctx.lineTo(cx - 2, cy + 2);
    ctx.stroke();
  } else if (v.glyph === 'dot') {
    ctx.beginPath();
    ctx.arc(cx, cy, 3.6, 0, Math.PI * 2);
    ctx.fillStyle = v.color;
    ctx.fill();
  } else {
    // dash (neutral) — a short muted horizontal bar.
    ctx.strokeStyle = v.color;
    ctx.lineWidth = 1.6;
    ctx.lineCap = 'round';
    ctx.beginPath();
    ctx.moveTo(cx - 3.2, cy);
    ctx.lineTo(cx + 3.2, cy);
    ctx.stroke();
  }
  ctx.restore();
}

/** Draw a PR pill of width `w` with its left edge at `x`, vertically centered on
 *  `cy`. Rounded-rect body (fill + optional border) then the `#num` label,
 *  re-measured to the same cap `prBadgeWidth` used. Mirrors `drawRefLabelAt`'s
 *  pill body. Caller has set `ctx.textBaseline='middle'`. */
export function drawPrBadge(
  ctx: CanvasRenderingContext2D,
  x: number,
  cy: number,
  w: number,
  badge: PrBadge,
  theme: Theme,
): void {
  const v = prBadgeVisual(badge, theme);
  const h = METRICS.pillHeight;
  const y = cy - h / 2;
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, h / 2);
  ctx.fillStyle = v.fill;
  ctx.fill();
  if (v.border !== null) {
    ctx.strokeStyle = v.border;
    ctx.lineWidth = 1;
    ctx.stroke();
  }
  // Leading PR-state glyph (non-colour carrier), then the `#num` label after a
  // small gap; both in the pill's label colour. The `#num` is truncated to the
  // width remaining after the glyph so the pill never overflows its cap.
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  ctx.fillStyle = v.text;
  ctx.textAlign = 'left';
  const glyph = prStateGlyph(badge);
  const glyphW = Math.ceil(measure(ctx, glyph));
  ctx.fillText(glyph, x + METRICS.prBadgePadX, cy);
  const numMax = w - 2 * METRICS.prBadgePadX - glyphW - PR_GLYPH_GAP;
  ctx.fillText(
    truncateToWidth(ctx, v.label, numMax),
    x + METRICS.prBadgePadX + glyphW + PR_GLYPH_GAP,
    cy,
  );
}
