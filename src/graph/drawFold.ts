/** Spec-004 (UI contract §1/§2): fold-pill display rows and the boundary
 *  "Collapse N" pill. Pure canvas painting — sibling of draw.ts (which is at
 *  the size cap and only SKIPS fold rows); called by GraphCanvas after
 *  `drawGraph`, like `drawWipRow`. Works for both graph styles: every colour is
 *  the resolved theme's lane palette or an existing theme var, and the dashed
 *  connector never sways/tapers (contract §1). No motion anywhere. */

import type { FoldSpan } from '../ipc';
import type { Theme } from './colors';
import { FONT_UI, METRICS } from './metrics';
import type { EffectiveMetrics } from './metrics';
import { laneX, refColArea, rowY } from './geometry';
import { measure } from './textMeasure';

/** Interaction flags for the visible fold rows (display indices). */
export interface FoldPaintState {
  hoverRow: number | null;
  /** Mouse-down row (pressed frame, tint 32%). */
  pressedRow: number | null;
  /** Keyboard-active pill row (§3: land-don't-select) — bg-2 + accent bar. */
  activeRow: number | null;
  /** Pill row carrying a HIDDEN selection (§6) — 1.5px accent outer ring. */
  selectionRow: number | null;
}

/** Locale-grouped count (`1,204`), per contract §1. */
export function foldCountLabel(count: number): string {
  return count.toLocaleString();
}

/** Rounded-rect path helper (radius 999 clamps to a stadium). */
function stadium(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number): void {
  const r = Math.min(h / 2, w / 2);
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.arcTo(x + w, y, x + w, y + h, r);
  ctx.arcTo(x + w, y + h, x, y + h, r);
  ctx.arcTo(x, y + h, x, y, r);
  ctx.arcTo(x, y, x + w, y, r);
  ctx.closePath();
}

/** §6 local-branch pill recipe in a lane colour: tinted bg + 1px border + text. */
function drawPill(
  ctx: CanvasRenderingContext2D,
  x: number,
  centerY: number,
  label: string,
  color: string,
  tint: number,
  m: EffectiveMetrics,
): { w: number; h: number; y: number } {
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  const w = Math.min(m.pillMaxWidth, Math.ceil(measure(ctx, label)) + 2 * m.pillPadX);
  const h = m.pillHeight;
  const y = centerY - h / 2;
  const prev = ctx.globalAlpha;
  ctx.globalAlpha = tint;
  ctx.fillStyle = color;
  stadium(ctx, x, y, w, h);
  ctx.fill();
  ctx.globalAlpha = prev;
  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  stadium(ctx, x, y, w, h);
  ctx.stroke();
  ctx.fillStyle = color;
  ctx.textAlign = 'left';
  ctx.textBaseline = 'middle';
  ctx.fillText(label, x + m.pillPadX, centerY);
  return { w, h, y };
}

/** Pill background tint for the row's interaction state (§1). */
function tintFor(row: number, s: FoldPaintState): number {
  if (s.pressedRow === row) return 0.32;
  if (s.hoverRow === row) return 0.28;
  return 0.18;
}

/** Paint every visible fold-pill row + boundary collapse pill. `firstRow`/
 *  `lastRow`/`scrollTop` are the same DISPLAY-space viewport values handed to
 *  `drawGraph`. Row hover/active backgrounds for pill rows are painted here
 *  (drawGraph's hover pass already covers plain hover; the keyboard-active
 *  paint is fold-specific). */
export function drawFoldRows(
  ctx: CanvasRenderingContext2D,
  foldRows: ReadonlyMap<number, FoldSpan>,
  boundaryRows: ReadonlyMap<number, FoldSpan>,
  vp: { firstRow: number; lastRow: number; scrollTop: number; width: number },
  theme: Theme,
  m: EffectiveMetrics,
  s: FoldPaintState,
): void {
  const RH = m.rowHeight;
  for (const [row, span] of foldRows) {
    if (row < vp.firstRow || row > vp.lastRow) continue;
    const top = row * RH - vp.scrollTop;
    const centerY = rowY(row, vp.scrollTop, m);
    const color = theme.laneColors[span.lane % 10];

    // Keyboard-active row (§1): bg-2 fill + a 2px accent bar on the left edge.
    // (Plain hover bg is already painted by drawGraph's hover pass.)
    if (s.activeRow === row) {
      ctx.fillStyle = theme.bg2;
      ctx.fillRect(0, top, vp.width, RH);
      ctx.fillStyle = theme.accent;
      ctx.fillRect(0, top, 2, RH);
    }

    // Dashed lane connector — full row height, 2px, [3,4], round caps (§1).
    const x = laneX(span.lane, m);
    ctx.save();
    ctx.setLineDash([3, 4]);
    ctx.lineCap = 'round';
    ctx.lineWidth = m.edgeWidth;
    ctx.strokeStyle = color;
    ctx.beginPath();
    ctx.moveTo(x, top);
    ctx.lineTo(x, top + RH);
    ctx.stroke();
    ctx.restore();

    // The "⋯ N commits" pill, 8px right of the connector's lane cell (§1).
    const pill = drawPill(
      ctx,
      x + m.laneWidth,
      centerY,
      `⋯ ${foldCountLabel(span.count)} commits`,
      color,
      tintFor(row, s),
      m,
    );

    // Selection-carrying ring (§6): 1.5px accent, 2px outside the pill border.
    if (s.selectionRow === row) {
      ctx.strokeStyle = theme.accent;
      ctx.lineWidth = 1.5;
      stadium(ctx, x + m.laneWidth - 2, pill.y - 2, pill.w + 4, pill.h + 4);
      ctx.stroke();
    }
  }

  // Boundary collapse pills (§2): on an EXPANDED run's first revealed row,
  // right-aligned in the (guaranteed vacant) left ref band.
  for (const [row, span] of boundaryRows) {
    if (row < vp.firstRow || row > vp.lastRow) continue;
    const centerY = rowY(row, vp.scrollTop, m);
    const color = theme.laneColors[span.lane % 10];
    const rect = collapsePillRect(ctx, span, m);
    drawPill(
      ctx,
      rect.x,
      centerY,
      collapsePillLabel(span),
      color,
      s.hoverRow === row ? 0.28 : 0.18,
      m,
    );
  }
}

/** §2 label: `Collapse {N}`. */
export function collapsePillLabel(span: FoldSpan): string {
  return `Collapse ${foldCountLabel(span.count)}`;
}

/** The boundary pill's x/width — shared between paint and hit-test so the hit
 *  box matches the pixels. Right-aligned at the ref band's budget edge. */
export function collapsePillRect(
  ctx: CanvasRenderingContext2D,
  span: FoldSpan,
  m: EffectiveMetrics,
): { x: number; w: number } {
  ctx.font = `${METRICS.pillFont} ${FONT_UI}`;
  const w = Math.min(
    m.pillMaxWidth,
    Math.ceil(measure(ctx, collapsePillLabel(span))) + 2 * m.pillPadX,
  );
  const { startX, budget } = refColArea(m);
  return { x: startX + budget - w, w };
}

/** §2: hit-test the boundary collapse pill (its rect expanded to >= 24px wide;
 *  the row is already the vertical bound). */
export function collapsePillHit(
  ctx: CanvasRenderingContext2D,
  span: FoldSpan,
  m: EffectiveMetrics,
  x: number,
): boolean {
  const r = collapsePillRect(ctx, span, m);
  const pad = Math.max(0, (24 - r.w) / 2);
  return x >= r.x - pad && x <= r.x + r.w + pad;
}
