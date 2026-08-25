/** Per-row RIGHT text content of the commit graph (P51b §6.4) — the flexing
 *  summary plus the optional author / SHA / date columns packed by
 *  {@link computeRightColumns}. Extracted from draw.ts pass 5 so the new column
 *  logic does not inflate draw.ts. The LEFT ref band is still painted by draw.ts
 *  (see refLabels.ts). `cols`, `summaryStartX` and `now` are computed ONCE
 *  before the row loop and passed in — this function is called per visible row
 *  with `ctx.textBaseline === 'middle'` already set by the caller. */

import type { GraphNode, VerifyStatus } from '../ipc';
import type { Theme } from './colors';
import { relativeDate, shortSha } from './dates';
import { drawCiBadge, drawPrBadge, layoutForgeCell, rowForgeSignal } from './forgeBadges';
import { FONT_MONO, FONT_UI } from './metrics';
import type { EffectiveMetrics } from './metrics';
import type { RefEntity } from './refLabels';
import type { GraphDisplayOptions, RightColumns } from './rightColumns';
import { truncateToWidth } from './textMeasure';
import { verifyBadgeKind } from './verifyBadge';

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
  /** P58c: this commit's signature verdict, or `undefined` when not yet
   *  verified (⇒ the faint P51 stub). Looked up by oid in draw.ts. */
  status: VerifyStatus | undefined,
  /** PERF: the row's `groupRefs(node.refs)` result, already computed by the
   *  band pass — reused for the forge cell to avoid a second grouping/frame. */
  groups: readonly RefEntity[],
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

  // sha — 7-char short SHA (mono), right-aligned; the signature badge sits in
  // the slot at the column's LEFT (lit when known + enabled, else the faint stub).
  if (cols.sha !== null) {
    drawBadge(
      ctx,
      cols.sha.leftX + m.badgeSlotWidth / 2,
      cy,
      theme,
      status,
      display.showSignatureBadge,
    );
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

  // forge — leftmost pack column (PR-badge-placement): the row's CI dot + PR pill
  // for the first branch entity that carries a signal, left-aligned at the
  // column's leftX. The column is reserved only when forge data is present, and
  // is compact-suppressed upstream (toggles arrive false), so nothing draws in
  // compact mode. drawCiBadge save/restores; drawPrBadge leaves textAlign 'left'.
  if (cols.forge !== null) {
    const signal = rowForgeSignal(node.refs, node, display, groups);
    if (signal !== null) {
      const cell = layoutForgeCell(ctx, cols.forge.leftX, signal);
      if (cell.ci !== null) drawCiBadge(ctx, cell.ci.cx, cy, cell.ci.badge, theme);
      if (cell.pr !== null) drawPrBadge(ctx, cell.pr.x, cy, cell.pr.w, cell.pr.badge, theme);
    }
  }
}

/** P58c §7.2: the signature badge, centered in the SHA column's badge slot. A
 *  pure draw swap over the P51 stub — the slot geometry (P51 §6.5) is unchanged.
 *
 *  - `showBadge` && a KNOWN status ⇒ a LIT glyph per {@link verifyBadgeKind}
 *    (green check = good; red triangle = bad/expired/expiredKey/revoked; solid
 *    neutral disc = goodUnknown/cannotCheck; NOTHING for unsigned).
 *  - otherwise (badge off, or the status is not yet verified) ⇒ the faint P51
 *    hollow stub, rendering exactly as before P58. */
function drawBadge(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  theme: Theme,
  status: VerifyStatus | undefined,
  showBadge: boolean,
): void {
  if (showBadge && status !== undefined) {
    const kind = verifyBadgeKind(status);
    if (kind === null) return; // unsigned ⇒ nothing (no clutter)
    // save/restore so the glyphs' lineJoin/lineCap/fillStyle never leak into
    // the SHA text or the next paint's edge pass.
    ctx.save();
    if (kind === 'good') drawGoodBadge(ctx, cx, cy, theme);
    else if (kind === 'warn') drawWarnBadge(ctx, cx, cy, theme);
    else drawUnknownBadge(ctx, cx, cy, theme);
    ctx.restore();
    return;
  }
  // Faint P51 stub — not yet verified, or the badge is toggled off.
  ctx.beginPath();
  ctx.arc(cx, cy, 4, 0, Math.PI * 2);
  ctx.strokeStyle = theme.text3;
  ctx.lineWidth = 1;
  ctx.stroke();
}

/** Good — a filled green disc with a white check (the "verified" glyph). */
function drawGoodBadge(ctx: CanvasRenderingContext2D, cx: number, cy: number, theme: Theme): void {
  ctx.beginPath();
  ctx.arc(cx, cy, 4.6, 0, Math.PI * 2);
  ctx.fillStyle = theme.badgeGood;
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(cx - 2.2, cy + 0.1);
  ctx.lineTo(cx - 0.6, cy + 1.9);
  ctx.lineTo(cx + 2.4, cy - 2.1);
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 1.3;
  ctx.lineJoin = 'round';
  ctx.stroke();
}

/** Bad/expired/revoked — a filled red warning triangle with a white "!". */
function drawWarnBadge(ctx: CanvasRenderingContext2D, cx: number, cy: number, theme: Theme): void {
  ctx.beginPath();
  ctx.moveTo(cx, cy - 4.6);
  ctx.lineTo(cx + 4.4, cy + 3.6);
  ctx.lineTo(cx - 4.4, cy + 3.6);
  ctx.closePath();
  ctx.fillStyle = theme.badgeWarn;
  ctx.fill();
  ctx.beginPath();
  ctx.moveTo(cx, cy - 1.6);
  ctx.lineTo(cx, cy + 1.0);
  ctx.strokeStyle = '#ffffff';
  ctx.lineWidth = 1.2;
  ctx.lineCap = 'round';
  ctx.stroke();
  ctx.beginPath();
  ctx.arc(cx, cy + 2.5, 0.7, 0, Math.PI * 2);
  ctx.fillStyle = '#ffffff';
  ctx.fill();
}

/** goodUnknown/cannotCheck — a solid neutral disc (signed, trust not
 *  established). Solid so it never reads as the hollow "not yet checked" stub. */
function drawUnknownBadge(
  ctx: CanvasRenderingContext2D,
  cx: number,
  cy: number,
  theme: Theme,
): void {
  ctx.beginPath();
  ctx.arc(cx, cy, 4.2, 0, Math.PI * 2);
  ctx.fillStyle = theme.badgeUnknown;
  ctx.fill();
}
