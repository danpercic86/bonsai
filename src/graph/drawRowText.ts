/** Per-row RIGHT text content of the commit graph (P51b §6.4) — the flexing
 *  summary plus the optional author / SHA / date columns packed by
 *  {@link computeRightColumns}. Extracted from draw.ts pass 5 so the new column
 *  logic does not inflate draw.ts. The LEFT ref band is still painted by draw.ts
 *  (see refLabels.ts). `cols`, `summaryStartX` and `now` are computed ONCE
 *  before the row loop and passed in — this function is called per visible row
 *  with `ctx.textBaseline === 'middle'` already set by the caller. */

import type { GraphNode } from '../ipc';
import type { Theme } from './colors';
import { relativeDate, shortSha } from './dates';
import { FONT_MONO, FONT_UI } from './metrics';
import type { EffectiveMetrics } from './metrics';
import type { GraphDisplayOptions, RightColumns } from './rightColumns';
import { truncateToWidth } from './textMeasure';

export function drawRowText(
  ctx: CanvasRenderingContext2D,
  node: GraphNode,
  cy: number,
  summaryStartX: number,
  cols: RightColumns,
  display: GraphDisplayOptions,
  theme: Theme,
  m: EffectiveMetrics,
  now: number,
): void {
  // summary — flexes from summaryStartX to cols.summaryEndX, reclaiming the
  // space of any disabled right column. With only the date column shown this
  // width equals the pre-P51 summaryMax exactly (behavior-preserving).
  const summaryMax = cols.summaryEndX - summaryStartX;
  if (summaryMax > 0) {
    ctx.font = `${m.summaryFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text1;
    ctx.textAlign = 'left';
    ctx.fillText(truncateToWidth(ctx, node.summary, summaryMax), summaryStartX, cy);
  }

  // author — optional full-name column (default off). The initials avatar is a
  // separate element (the commit node) and is unaffected by this toggle.
  if (cols.author !== null) {
    ctx.font = `${m.metaFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text3;
    ctx.textAlign = 'right';
    ctx.fillText(truncateToWidth(ctx, node.author, m.authorColWidth), cols.author.rightX, cy);
  }

  // sha — 7-char short SHA (mono), right-aligned; the verified-badge stub sits
  // in the slot at the column's LEFT.
  if (cols.sha !== null) {
    drawBadgeStub(ctx, cols.sha.leftX + m.badgeSlotWidth / 2, cy, theme);
    ctx.font = `${m.shaFont} ${FONT_MONO}`;
    ctx.fillStyle = theme.text2;
    ctx.textAlign = 'right';
    ctx.fillText(shortSha(node.id), cols.sha.rightX, cy);
  }

  // date — relative inline; the basis picks author vs committer time. The full
  // absolute timestamp is shown only on hover (see GraphCanvas date tooltip).
  if (cols.date !== null) {
    const ts = display.dateBasis === 'committer' ? node.committerTs : node.ts;
    ctx.font = `${m.metaFont} ${FONT_UI}`;
    ctx.fillStyle = theme.text3;
    ctx.textAlign = 'right';
    ctx.fillText(truncateToWidth(ctx, relativeDate(ts, now), m.dateColWidth), cols.date.rightX, cy);
  }
}

/** P51b §6.5 / D6: verified-badge STUB — a faint unlit hollow glyph centered in
 *  the SHA column's badge slot. Placeholder only; it carries no meaning and no
 *  GraphNode verification field exists yet.
 *  P58 lights this (verified / unverified / unknown) as a pure draw swap — the
 *  badge-slot geometry does NOT change, so no layout is affected. */
function drawBadgeStub(ctx: CanvasRenderingContext2D, cx: number, cy: number, theme: Theme): void {
  ctx.beginPath();
  ctx.arc(cx, cy, 4, 0, Math.PI * 2);
  ctx.strokeStyle = theme.text3;
  ctx.lineWidth = 1;
  ctx.stroke();
}
