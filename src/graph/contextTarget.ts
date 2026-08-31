/** Graph right-click/keyboard target resolution — pure, plain data in → plain
 *  data out (P92; the pill/chip/row resolution moved out of `GraphCanvas.tsx`).
 *
 *  Kept out of `GraphCanvas.tsx` (plain data in → plain data out, no component
 *  state) so the chip's hit box, the hover tooltip and the draw pass all share
 *  the ONE `layoutRefLabels` layout, and the container stays a thin wiring
 *  layer. Zero DOM imports beyond the 2D context used for text measurement.
 */

import type { GraphNode, RefLabel } from '../ipc';
import type { Theme } from './colors';
import { refColArea } from './draw';
import { chipHitAt, hiddenEntities, pillHitAt, targetRefOf } from './hitTest';
import type { EffectiveMetrics } from './metrics';
import { groupRefs, layoutRefLabels } from './refLabels';
import type { RefEntity } from './refLabels';
import type { GraphDisplayOptions } from './rightColumns';

export interface ChipHitArgs {
  node: GraphNode;
  /** Cursor x in scroller CSS coords. */
  x: number;
  m: EffectiveMetrics;
  ctx: CanvasRenderingContext2D | null;
  theme: Theme | null;
  display: GraphDisplayOptions;
}

/** The hidden entities the "+N" chip under `x` stands for, or `null` when `x` is
 *  not on a chip (no refs, outside the ref band, on a pill, or no chip at all).
 *  Never returns an empty array: a chip only exists when something is hidden. */
export function chipHiddenEntitiesAt(args: ChipHitArgs): RefEntity[] | null {
  const { node, x, m, ctx, theme, display } = args;
  if (ctx === null || theme === null) return null;
  if (x >= m.refColWidth || node.refs === undefined || node.refs.length === 0) return null;
  const { startX, budget } = refColArea(m);
  const entities = groupRefs(node.refs);
  const laid = layoutRefLabels(ctx, entities, node, theme, startX, budget, display);
  if (chipHitAt(laid, x) === undefined) return null;
  const hidden = hiddenEntities(entities, laid);
  return hidden.length > 0 ? hidden : null;
}

/** P92 §1.5: client-coords anchor for the keyboard-opened row menu — the ref
 *  band's left edge, just under the selected row, clamped to the scroller's own
 *  box so an off-screen selection still anchors somewhere sensible. */
export function rowMenuAnchor(
  rect: { left: number; top: number; bottom: number },
  scrollTop: number,
  rowIndex: number,
  wipOffset: number,
  m: EffectiveMetrics,
): { x: number; y: number } {
  const rowBottom = (rowIndex + wipOffset + 1) * m.rowHeight - scrollTop;
  return {
    x: rect.left + refColArea(m).startX,
    y: Math.min(Math.max(rect.top + rowBottom, rect.top), rect.bottom),
  };
}

/** Right-click target on the graph: a ref pill, the "+N" overflow chip's hidden
 *  refs (P92 §1.1), or a bare commit row. `commit` carries the row's grouped
 *  entities so the menu builder can offer the P92 §2.2 branch picker when the
 *  row is multi-ref; the field stays optional (absent => pre-P92 commit menu). */
export type GraphContextTarget =
  | { kind: 'ref'; ref: RefLabel; oid: string }
  | { kind: 'refPicker'; entities: RefEntity[]; oid: string }
  | { kind: 'commit'; index: number; oid: string; entities?: RefEntity[] };

/** P5 §4.2 / P7 §5 / P92 §1.1: resolve the target of a right-click on `row`.
 *  Precedence inside the LEFT ref band: a SHOWN pill (its own ref) -> the "+N"
 *  chip (a picker over the hidden entities) -> the whole row. A pill whose ref
 *  resolves to `null` (tag/head shapes) falls through to the row, matching
 *  pre-P92 behaviour. The row target carries the grouped entities so the menu
 *  builder can decide between the picker and the P18b single-branch fallback —
 *  the chip NO LONGER falls through to the row's first branch. */
export function resolveContextTarget(args: ChipHitArgs & { row: number }): GraphContextTarget {
  const { node, x, m, ctx, theme, display, row } = args;
  if (
    ctx !== null &&
    theme !== null &&
    x < m.refColWidth &&
    node.refs !== undefined &&
    node.refs.length > 0
  ) {
    const { startX, budget } = refColArea(m);
    const laid = layoutRefLabels(ctx, groupRefs(node.refs), node, theme, startX, budget, display);
    const hitLabel = pillHitAt(laid, x);
    if (hitLabel !== undefined && hitLabel.entity !== null) {
      const ref = targetRefOf(hitLabel.entity);
      if (ref !== null) return { kind: 'ref', ref, oid: node.id };
    }
  }
  const hidden = chipHiddenEntitiesAt(args);
  if (hidden !== null) return { kind: 'refPicker', entities: hidden, oid: node.id };
  return { kind: 'commit', index: row, oid: node.id, entities: groupRefs(node.refs) };
}
