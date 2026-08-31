import type { GraphLayout } from '../ipc';
import type { Theme } from './colors';
import { avatarHit, laneX, refColArea } from './draw';
import type { WipSummary } from './draw';
import { entityStyle, groupRefs, layoutRefLabels } from './refLabels';
import { formatAbsolute } from './dates';
import { chipHitAt, forgeTooltipTarget, hiddenEntities, pillHitAt } from './hitTest';
import type { TooltipState } from './hitTest';
import { layoutForgeCell, rowForgeSignal } from './forgeBadges';
import { computeRightColumns } from './rightColumns';
import type { GraphDisplayOptions } from './rightColumns';
import type { EffectiveMetrics } from './metrics';

/** Inputs for {@link resolveHoverTarget}: the cursor position + the container's
 *  live props/measurements, passed explicitly so the resolution stays pure. */
export interface HoverTargetArgs {
  x: number;
  y: number;
  scrollTop: number;
  /** Hover-ref encoding: row index, `-1` for the WIP row, or `null`. */
  row: number | null;
  layout: GraphLayout;
  wip: WipSummary | null;
  display: GraphDisplayOptions;
  m: EffectiveMetrics;
  ctx: CanvasRenderingContext2D | null;
  theme: Theme | null;
  rightInset: number;
  cssWidth: number;
}

// P7 §6.1: resolve the hover tooltip target from a cursor position (scroller/
// host CSS coords). Avatar disc → author-name tooltip; a "+n" chip in the LEFT
// ref column → the hidden-entity list. Pure over current props/refs; returns
// null for empty area, the WIP row, or when no ctx/theme is available.
export function resolveHoverTarget(args: HoverTargetArgs): TooltipState | null {
  const { x, y, row, layout: lay, wip, display, m, ctx, theme, rightInset, cssWidth } = args;
  if (row === null || row < 0) return null; // none, or the WIP row (-1)
  const node = lay.nodes[row];
  if (ctx === null || theme === null) return null;
  const wipOffset = wip !== null ? 1 : 0;
  const cy = (row + wipOffset) * m.rowHeight + m.rowHeight / 2 - args.scrollTop;
  const cx = laneX(node.lane, m);
  if (avatarHit(x, y, cx, cy, m)) {
    const r = m.avatarRadius + m.avatarBgRingExtra;
    return {
      kind: 'avatar',
      text: node.author,
      anchor: { left: cx - r, top: cy - r, width: 2 * r, height: 2 * r },
    };
  }
  if (node.refs !== undefined && node.refs.length > 0 && x < m.refColWidth) {
    const { startX, budget } = refColArea(m);
    const entities = groupRefs(node.refs);
    const laid = layoutRefLabels(ctx, entities, node, theme, startX, budget, display);
    const chip = chipHitAt(laid, x);
    if (chip !== undefined) {
      const lines = hiddenEntities(entities, laid).map((e) => entityStyle(e, node, theme).label);
      return {
        kind: 'overflow',
        lines,
        anchor: { left: chip.x, top: cy - m.pillHeight / 2, width: chip.w, height: m.pillHeight },
      };
    }
    // §14.2: hovering a SHOWN branch pill → full branch-name tooltip.
    // Precedence: avatar (earlier) → chip (above) → shown pill.
    const hitLabel = pillHitAt(laid, x);
    if (hitLabel !== undefined && hitLabel.entity !== null && hitLabel.entity.kind === 'branch') {
      return {
        kind: 'ref',
        text: hitLabel.entity.name,
        anchor: { left: hitLabel.x, top: cy - m.pillHeight / 2, width: hitLabel.w, height: m.pillHeight },
      };
    }
  }
  // P51b: hovering the date column → FULL absolute timestamps (authored +
  // committed), one per line; the inline date stays relative. Recompute the
  // column geometry with the SAME pure helper the draw pass uses so the hit
  // box matches the drawn column exactly. (`display` is read at the top.)
  const effRight = cssWidth - rightInset;
  const cols = computeRightColumns(effRight, display, m);
  // PR-badge-placement §6: forge column — the PR pill (tooltip) or CI dot.
  // Same pure helpers the draw pass uses, so the hit boxes match the pixels.
  if (cols.forge !== null && x >= cols.forge.leftX && x <= cols.forge.rightX) {
    const signal = rowForgeSignal(node.refs, node, display);
    if (signal !== null) {
      const cell = layoutForgeCell(ctx, cols.forge.leftX, signal);
      const t = forgeTooltipTarget(cell, x, m.ciBadgeSize, cy, m.pillHeight);
      if (t !== null) return t;
    }
  }
  if (cols.date !== null && x >= cols.date.leftX && x <= cols.date.rightX) {
    return {
      kind: 'date',
      lines: [`Authored  ${formatAbsolute(node.ts)}`, `Committed ${formatAbsolute(node.committerTs)}`],
      anchor: {
        left: cols.date.leftX,
        top: cy - m.pillHeight / 2,
        width: cols.date.width,
        height: m.pillHeight,
      },
    };
  }
  return null;
}
