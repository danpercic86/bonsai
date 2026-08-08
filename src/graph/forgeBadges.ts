/** P63: PURE forge-signal badge subsystem for the commit graph's LEFT ref band.
 *  Defines the per-branch PR / CI badge model, the state→visual classifiers, the
 *  width helper, the canvas draw fns, and the `branchSignals` display-time gate.
 *  No React, no IPC — mirrors `verifyBadge.ts` (classifier) + `drawRowText.ts`
 *  (glyph helpers) so it unit-tests headlessly. The renderer (refLabels.ts) and
 *  the hit-test (GraphCanvas.tsx) both consume the geometry laid out from here,
 *  so draw and hit-test share one source of truth. */

import type { CheckRollup, GraphNode, PrState } from '../ipc';
import type { Theme } from './colors';
import { FONT_UI, METRICS } from './metrics';
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

/** Measured width of a PR badge pill (`2*padX + measure("#num")`, capped at
 *  `prBadgeMaxWidth`). Sets `ctx.font` to `pillFont` (== the ref-band loop
 *  invariant) to measure — safe to call inside `layoutRefLabels`. PURE. */
export function prBadgeWidth(ctx: CanvasRenderingContext2D, pr: PrBadge): number {
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  const raw = 2 * METRICS.prBadgePadX + Math.ceil(measure(ctx, `#${pr.number}`));
  return Math.min(raw, METRICS.prBadgeMaxWidth);
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
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  ctx.fillStyle = v.text;
  ctx.textAlign = 'left';
  ctx.fillText(truncateToWidth(ctx, v.label, w - 2 * METRICS.prBadgePadX), x + METRICS.prBadgePadX, cy);
}
