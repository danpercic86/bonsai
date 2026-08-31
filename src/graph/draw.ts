/** Pure canvas draw functions for the precomputed GraphLayout — no React.
 * The ctx is already DPR-transformed; every coordinate below is CSS px.
 * Geometry and draw order are normative per contract M2-graph.md §1.3/§3.3.
 *
 * P51b split this file: the LEFT ref-band subsystem moved to `refLabels.ts`,
 * the per-row RIGHT text pass to `drawRowText.ts`, the right-column model to
 * `rightColumns.ts`, cached measurement to `textMeasure.ts`, and the date/oid
 * formatters to `dates.ts`. This module keeps geometry, edges, avatars, the
 * stash/WIP rows, and the `drawGraph` orchestration. T3.6 moved the pure
 * geometry + avatar-identity helpers to `geometry.ts` (re-exported below). */

import type { GraphEdge, GraphLayout, GraphNode, VerifyStatus } from '../ipc';
import { STASH_COLOR } from './colors';
import type { Theme } from './colors';
import { FONT_UI } from './metrics';
import type { EffectiveMetrics } from './metrics';
import { drawRowText } from './drawRowText';
import {
  avatarColor,
  initials,
  laneX,
  refColArea,
  rowY,
  summaryStartX,
} from './geometry';
import { drawRefLabelAt, drawStashIcon, groupRefs, layoutRefLabels } from './refLabels';
import { computeRightColumns } from './rightColumns';
import type { GraphDisplayOptions } from './rightColumns';
import { measure } from './textMeasure';
// P67 §1: the guideline's geometry is computed in viewport.ts (contract D2);
// this module only strokes/fills the result.
import type { HeadGuide } from './viewport';

// P51b: relativeDate lives in dates.ts now; re-export it so the many existing
// `import { relativeDate } from '../graph/draw'` call sites keep working.
export { relativeDate } from './dates';

// T3.6: the pure geometry + avatar-identity helpers moved to geometry.ts;
// re-export so existing `from './draw'` call sites keep working.
export {
  avatarColor,
  avatarHit,
  initials,
  laneX,
  refColArea,
  rowAtPoint,
  rowY,
  summaryStartX,
} from './geometry';
export type { AvatarColor } from './geometry';

export interface Viewport {
  /** Inclusive; M2b: 0. */
  firstRow: number;
  /** Inclusive; M2b: nodes.length - 1. */
  lastRow: number;
  /** CSS px; M2b: 0. */
  scrollTop: number;
  /** CSS px of the canvas. */
  width: number;
  height: number;
  /** P7e §13.2: CSS px reserved on the RIGHT for the vertical scrollbar (the
   *  `.graph-scroll` overlay's native scrollbar paints over the full-width
   *  canvas's right edge). Absent/`undefined` → treated as 0, i.e. identical to
   *  pre-P7e behavior. */
  rightInset?: number;
}

export interface Interaction {
  hoverRow: number | null;
  selectedIndex: number | null;
  /** P50b: rows carrying a commit-search match → an outer `--match-ring` ring.
   *  `null` when search is closed / has no visible matches (no ring pass). */
  matchRows: Set<number> | null;
  /** P58c: oid → signature verdict for the LIT badge (visible rows only,
   *  cached by oid). `null` / a missing oid ⇒ the faint P51 stub. */
  verifyStatus: ReadonlyMap<string, VerifyStatus> | null;
  /** P84: transient reveal flash for one row (row-bg pulse + dot halo, accent
   *  family). `null` when no flash is active. `alpha`/`ringRadius` are
   *  precomputed per frame by GraphCanvas from `revealFlash.ts`. */
  flash?: { row: number; alpha: number; ringRadius: number } | null;
}

/** Long-edge middle segments are clamped to this margin around the canvas. */
const EDGE_CLAMP_MARGIN = 56;

// ---------- edges (§1.3 three-segment render rule) ----------

/** One-row segment: straight vertical if same x, else cubic bézier with
 * vertical tangents — control points (x1, y1+14) and (x2, y2-14). */
function segmentTo(
  ctx: CanvasRenderingContext2D,
  x1: number,
  y1: number,
  x2: number,
  y2: number,
  halfRow: number,
): void {
  ctx.moveTo(x1, y1);
  if (x1 === x2) ctx.lineTo(x2, y2);
  else ctx.bezierCurveTo(x1, y1 + halfRow, x2, y2 - halfRow, x2, y2);
}

function drawEdge(
  ctx: CanvasRenderingContext2D,
  e: GraphEdge,
  nodes: readonly GraphNode[],
  vp: Viewport,
  theme: Theme,
  m: EffectiveMetrics,
): void {
  const halfRow = m.rowHeight / 2;
  const fromLane = nodes[e.from].lane;
  const toLane = nodes[e.to].lane;
  const fx = laneX(fromLane, m);
  const fy = rowY(e.from, vp.scrollTop, m);
  const tx = laneX(toLane, m);
  const ty = rowY(e.to, vp.scrollTop, m);

  ctx.strokeStyle = theme.laneColors[e.lane % 10];
  ctx.beginPath();
  if (e.to === e.from + 1) {
    segmentTo(ctx, fx, fy, tx, ty, halfRow);
  } else {
    const mx = laneX(e.lane, m);
    const yTop = rowY(e.from + 1, vp.scrollTop, m);
    const yBot = rowY(e.to - 1, vp.scrollTop, m);
    const clampTop = -EDGE_CLAMP_MARGIN;
    const clampBot = vp.height + EDGE_CLAMP_MARGIN;
    // top curve fromLane -> e.lane (skip when fully above the clamp window)
    if (yTop >= clampTop) segmentTo(ctx, fx, fy, mx, yTop, halfRow);
    // middle straight run, y-range clamped — never emit far-off-canvas coords
    const runTop = Math.max(yTop, clampTop);
    const runBot = Math.min(yBot, clampBot);
    if (runBot > runTop) {
      ctx.moveTo(mx, runTop);
      ctx.lineTo(mx, runBot);
    }
    // bottom curve e.lane -> toLane (skip when fully below the clamp window)
    if (yBot <= clampBot) segmentTo(ctx, mx, yBot, tx, ty, halfRow);
  }
  ctx.stroke();
}

// ---------- stash node (P10 §2.1) ----------

/** P10 §2.1: a stash node — violet disc + centered white stash glyph + violet
 *  lane ring. Draws in place of the author avatar; the caller has ALREADY drawn
 *  the bg-ring halo and will draw HEAD/selection rings afterwards. The ring
 *  matches the disc color (STASH_COLOR) rather than the row's lane color, so the
 *  stash reads as "not a real branch commit" (a deliberate choice). */
export function drawStashNode(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  m: EffectiveMetrics,
): void {
  // disc
  ctx.beginPath();
  ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
  ctx.fillStyle = STASH_COLOR;
  ctx.fill();

  // lane ring (violet, matches disc)
  ctx.beginPath();
  ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
  ctx.strokeStyle = STASH_COLOR;
  ctx.lineWidth = m.avatarRingWidth;
  ctx.stroke();

  // glyph (white, legible on the violet disc)
  const S = m.avatarRadius * 1.4;
  ctx.strokeStyle = '#ffffff';
  drawStashIcon(ctx, x - S / 2, y - S / 2, S);
}

// ---------- WIP (uncommitted changes) row (P1 §9.3) ----------

export interface WipSummary {
  fileCount: number;
}

/** Draws the frontend-composited WIP row (P1 §9.1/§9.3). `vp.scrollTop` is the
 * RAW (un-offset) scroll position.
 *
 * P67 §1: the dashed connector to the HEAD dot MOVED OUT of this function into
 * `drawHeadGuide` below, so it paints at every scroll position. What remains
 * here — the hover background, the dashed marker circle and the
 * "Uncommitted changes (n)" label — belongs to the WIP row itself and keeps the
 * caller's near-top gate. */
export function drawWipRow(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  wip: WipSummary,
  vp: Viewport,
  theme: Theme,
  hovered: boolean,
  m: EffectiveMetrics,
): void {
  const RH = m.rowHeight;
  const headIndex = layout.headIndex;
  const headLane = headIndex !== null ? layout.nodes[headIndex].lane : 0;
  const x = laneX(headLane, m);
  const y = RH / 2 - vp.scrollTop;

  if (hovered) {
    ctx.fillStyle = theme.bg2;
    ctx.fillRect(0, -vp.scrollTop, vp.width, RH);
  }

  ctx.save();
  ctx.setLineDash([3, 3]);
  ctx.lineWidth = 1.5;
  ctx.beginPath();
  ctx.arc(x, y, 4, 0, Math.PI * 2);
  ctx.fillStyle = theme.bg0;
  ctx.fill();
  ctx.strokeStyle = theme.warning;
  ctx.stroke();
  ctx.restore();

  ctx.textBaseline = 'middle';
  ctx.textAlign = 'left';
  // P7 §7: WIP label moves to the summary zone; the LEFT ref band stays empty.
  const textX = summaryStartX(layout.laneCount, m);
  ctx.font = `italic ${m.summaryFont} ${FONT_UI}`;
  ctx.fillStyle = theme.text2;
  const label = 'Uncommitted changes';
  ctx.fillText(label, textX, y);
  const labelW = measure(ctx, label);

  ctx.font = `${m.metaFont} ${FONT_UI}`;
  ctx.fillStyle = theme.text3;
  const count = `(${wip.fileCount} file${wip.fileCount === 1 ? '' : 's'})`;
  ctx.fillText(count, textX + labelW + 6, y);
}

// ---------- HEAD guideline (P67 §1) ----------

/** P67 §1: edge-marker geometry (CSS px). Local paint constants only. */
const HEAD_EDGE_MARKER = { halfWidth: 5, height: 6, inset: 2 } as const;

/** P67 §1: the dashed guideline from the WIP dot (or the top edge on a clean
 *  tree) to the checked-out commit. MOVED OUT of `drawWipRow` (P1 §9.3, now
 *  superseded) so it paints at EVERY scroll position. Contains no scroll, row,
 *  clamp or dash-phase arithmetic — `headGuide()` in viewport.ts owns all of it
 *  (contract D2); the only expressions here are the existing `laneX(lane, m)`
 *  helper and the `lane % 10` palette index, both moved verbatim from
 *  `drawWipRow`. Call AFTER `drawGraph` and BEFORE `drawWipRow`.
 *
 *  No-op when `guide.segment === false` (A5 / §1.1a: the segment collapsed to
 *  under 1 px, so only `drawHeadEdgeMarker` has anything to say). The decision is
 *  the boolean from `headGuide()` — never arithmetic here (D2). */
export function drawHeadGuide(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  guide: HeadGuide,
  theme: Theme,
  m: EffectiveMetrics,
): void {
  if (!guide.segment) return;
  const lane = layout.nodes[guide.headIndex]?.lane ?? 0;
  const x = laneX(lane, m);
  ctx.save();
  ctx.setLineDash([3, 3]);
  ctx.lineDashOffset = guide.dashOffset;
  ctx.lineWidth = 2;
  ctx.strokeStyle = theme.laneColors[lane % 10];
  ctx.beginPath();
  ctx.moveTo(x, guide.y0);
  ctx.lineTo(x, guide.y1);
  ctx.stroke();
  ctx.restore();
}

/** P67 §1 (D3): small filled triangle in the HEAD lane colour at the top or
 *  bottom viewport edge, pointing the way to an off-screen HEAD row. No-op when
 *  `guide.edge === null`. Same lane `x` as the guideline, so the two read as one
 *  pointer. */
export function drawHeadEdgeMarker(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  guide: HeadGuide,
  viewportHeight: number,
  theme: Theme,
  m: EffectiveMetrics,
): void {
  if (guide.edge === null) return;
  const lane = layout.nodes[guide.headIndex]?.lane ?? 0;
  const x = laneX(lane, m);
  const { halfWidth, height, inset } = HEAD_EDGE_MARKER;
  // Tip sits at the edge it points at; the base trails back into the viewport.
  const up = guide.edge === 'top';
  const tipY = up ? inset : viewportHeight - inset;
  const baseY = up ? tipY + height : tipY - height;
  ctx.save();
  ctx.fillStyle = theme.laneColors[lane % 10];
  ctx.beginPath();
  ctx.moveTo(x, tipY);
  ctx.lineTo(x - halfWidth, baseY);
  ctx.lineTo(x + halfWidth, baseY);
  ctx.closePath();
  ctx.fill();
  ctx.restore();
}

// ---------- main entry ----------

export function drawGraph(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  visibleEdges: readonly GraphEdge[],
  vp: Viewport,
  theme: Theme,
  ix: Interaction,
  display: GraphDisplayOptions,
  m: EffectiveMetrics,
): void {
  const { nodes } = layout;
  const n = nodes.length;
  const firstRow = Math.max(0, vp.firstRow);
  const lastRow = Math.min(n - 1, vp.lastRow);

  // Pass 1: clear.
  ctx.fillStyle = theme.bg0;
  ctx.fillRect(0, 0, vp.width, vp.height);

  // Pass 2: row backgrounds (selection wins over hover).
  const rowBg = (row: number, color: string): void => {
    ctx.fillStyle = color;
    ctx.fillRect(0, row * m.rowHeight - vp.scrollTop, vp.width, m.rowHeight);
  };
  if (ix.hoverRow !== null && ix.hoverRow !== ix.selectedIndex && ix.hoverRow < n) {
    rowBg(ix.hoverRow, theme.bg2);
  }
  if (ix.selectedIndex !== null && ix.selectedIndex < n) {
    rowBg(ix.selectedIndex, theme.selection);
  }

  // Pass 2.5 (P84): reveal row-background pulse — a full-width accent overlay at
  // an animated alpha, layered OVER the selection fill the revealed+selected row
  // already has. Restore globalAlpha immediately.
  if (ix.flash != null && ix.flash.alpha > 0 && ix.flash.row < n) {
    const prevAlpha = ctx.globalAlpha;
    ctx.globalAlpha = ix.flash.alpha;
    rowBg(ix.flash.row, theme.accent);
    ctx.globalAlpha = prevAlpha;
  }

  // Pass 3: edges (under dots).
  ctx.lineWidth = m.edgeWidth;
  ctx.lineCap = 'round';
  for (const e of visibleEdges) drawEdge(ctx, e, nodes, vp, theme, m);

  // Pass 4: author-initials avatars (P7 §2.1 — replaces the plain lane dot).
  // Inner→outer: bg ring → avatar disc → lane ring → initials → HEAD ring →
  // selection ring. Drawn per visible row only (virtualized).
  ctx.textAlign = 'center';
  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const x = laneX(node.lane, m);
    const y = rowY(row, vp.scrollTop, m);
    const laneColor = theme.laneColors[node.lane % 10];
    const ac = avatarColor(node.author);
    const selected = ix.selectedIndex === row;
    // P10 §2.1: a stash node draws a violet disc + glyph instead of the avatar.
    const isStash = node.refs?.some((r) => r.kind === 'stash') ?? false;

    // bg ring — bg0 halo so edges passing under the avatar read cleanly.
    ctx.beginPath();
    ctx.arc(x, y, m.avatarRadius + m.avatarBgRingExtra, 0, Math.PI * 2);
    ctx.fillStyle = theme.bg0;
    ctx.fill();

    if (isStash) {
      drawStashNode(ctx, x, y, m);
    } else {
      // avatar disc — theme-invariant hashed name color.
      ctx.beginPath();
      ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
      ctx.fillStyle = ac.bg;
      ctx.fill();

      // lane ring — ties the avatar to its lane color.
      ctx.beginPath();
      ctx.arc(x, y, m.avatarRadius, 0, Math.PI * 2);
      ctx.strokeStyle = laneColor;
      ctx.lineWidth = m.avatarRingWidth;
      ctx.stroke();

      // initials (centered baseline).
      ctx.font = `${m.avatarFont} ${FONT_UI}`;
      ctx.fillStyle = ac.text;
      ctx.fillText(initials(node.author), x, y);
    }

    if (layout.headIndex === row) {
      ctx.beginPath();
      ctx.arc(x, y, m.avatarHeadRingRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.text1;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    if (selected) {
      ctx.beginPath();
      ctx.arc(x, y, m.avatarSelRingRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.accent;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
    // Pass 4 (P84): reveal dot halo — an expanding accent ring OUTSIDE the
    // selection ring, same animated alpha as the row pulse. Primary attractor
    // when the row background is busy with edges/pills.
    if (ix.flash != null && ix.flash.row === row && ix.flash.alpha > 0) {
      const prevAlpha = ctx.globalAlpha;
      ctx.globalAlpha = ix.flash.alpha;
      ctx.beginPath();
      ctx.arc(x, y, ix.flash.ringRadius, 0, Math.PI * 2);
      ctx.strokeStyle = theme.accent;
      ctx.lineWidth = 2;
      ctx.stroke();
      ctx.globalAlpha = prevAlpha;
    }
    // P50b: search-match ring — an outer ring in --match-ring so matches stay
    // spottable while scrolling (distinct radius + color from head/selection).
    if (ix.matchRows !== null && ix.matchRows.has(row)) {
      ctx.beginPath();
      ctx.arc(x, y, m.avatarSelRingRadius + 1.5, 0, Math.PI * 2);
      ctx.strokeStyle = theme.matchRing;
      ctx.lineWidth = 1.5;
      ctx.stroke();
    }
  }
  // Restore the text-pass expectations (textAlign) and edge lineWidth so the
  // next paint's edges are unaffected (matches the old pass-4 cleanup).
  ctx.textAlign = 'left';
  ctx.lineWidth = m.edgeWidth;

  // Pass 5: text row content — LEFT ref column (refLabels) + the RIGHT summary
  // and optional author/SHA/date columns (drawRowText). The right columns are
  // packed ONCE here (computeRightColumns) and shared with the hover hit-test.
  const { startX, budget } = refColArea(m);
  const sx = summaryStartX(layout.laneCount, m);
  // P7e §13.2: keep the right columns clear of the vertical scrollbar by
  // shrinking the effective right edge by `rightInset`.
  const effRight = vp.width - (vp.rightInset ?? 0);
  const cols = computeRightColumns(effRight, display, m);
  const now = Math.floor(Date.now() / 1000);

  ctx.textBaseline = 'middle';
  for (let row = firstRow; row <= lastRow; row++) {
    const node = nodes[row];
    const y = rowY(row, vp.scrollTop, m);

    // 5a (LEFT): ref column — collapsed entities capped by the fixed band with
    // a trailing "+n" chip. Layout is the shared pure helper (single source of
    // truth with the hit-test); this pass only paints the laid-out labels.
    const groups = groupRefs(node.refs);
    const laid = layoutRefLabels(ctx, groups, node, theme, startX, budget, display);
    for (const l of laid) drawRefLabelAt(ctx, l, y);

    // 5b–5e (RIGHT): summary (flex) + optional author / SHA(+badge) / date
    // columns, packed by `cols`. Toggling a column off reclaims its width.
    // P58c: the SHA-slot badge lights from this row's cached verdict (undefined
    // ⇒ the faint stub, so off-screen/unverified rows stay faint).
    const status = ix.verifyStatus?.get(node.id);
    drawRowText(ctx, node, y, sx, cols, display, theme, m, now, status, groups);
  }
  ctx.textAlign = 'left';
}
