/** The frontend-composited WIP (uncommitted changes) row — moved VERBATIM out
 *  of draw.ts (spec-004 size split; draw.ts sits at the file-size cap). draw.ts
 *  re-exports both names so existing call sites keep working. */

import type { GraphLayout } from '../ipc';
import type { Theme } from './colors';
import { FONT_UI } from './metrics';
import type { EffectiveMetrics } from './metrics';
import { laneX, summaryStartX } from './geometry';
import { measure } from './textMeasure';
import type { Viewport } from './draw';

export interface WipSummary {
  fileCount: number;
}

/** Draws the frontend-composited WIP row (P1 §9.1/§9.3). `vp.scrollTop` is the
 * RAW (un-offset) scroll position.
 *
 * P67 §1: the dashed connector to the HEAD dot MOVED OUT of this function into
 * `drawHeadGuide`, so it paints at every scroll position. What remains here —
 * the hover background, the dashed marker circle and the
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
  // Backdrop-colored fill (== bg0 in the standard theme; the paper/soil color in
  // Bonsai, §7) so the dashed WIP marker never punches a bg0 hole in the backdrop.
  ctx.fillStyle = theme.graphBackdrop;
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
