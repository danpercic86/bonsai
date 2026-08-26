/** Right-click target resolution for the graph canvas — moved VERBATIM out of
 *  GraphCanvas.tsx's handleContextMenu (spec-004 size split). Resolves a ref
 *  pill in the LEFT ref band (shared layoutRefLabels layout — single source of
 *  truth with the draw pass), then the P18b whole-row branch fallback, then the
 *  bare commit target. */

import type { GraphNode, RefLabel } from '../ipc';
import type { Theme } from './colors';
import type { EffectiveMetrics } from './metrics';
import type { GraphDisplayOptions } from './rightColumns';
import { refColArea } from './geometry';
import { groupRefs, layoutRefLabels } from './refLabels';
import { fallbackBranchRef, pillHitAt, targetRefOf } from './hitTest';

/** Right-click target on the graph: a ref pill, or a bare commit row. `index`
 *  is the MODEL row (wire layout index) — fold display rows never reach here. */
export type GraphContextTarget =
  | { kind: 'ref'; ref: RefLabel; oid: string }
  | { kind: 'commit'; index: number; oid: string };

export function resolveContextTarget(args: {
  x: number;
  /** MODEL row index reported in the commit target. */
  index: number;
  node: GraphNode;
  ctx: CanvasRenderingContext2D | null;
  theme: Theme | null;
  m: EffectiveMetrics;
  display: GraphDisplayOptions;
}): GraphContextTarget {
  const { x, index, node, ctx, theme, m, display } = args;
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
      // tag/head resolve to a ref whose branchMenuItems is [] → no menu opens;
      // fall through to the whole-row / commit target (matches today's behavior).
    }
  }
  // P18b: whole-row branch fallback. If no SPECIFIC pill was hit but the row
  // carries a branch/remoteBranch, open that branch's menu (the superset).
  if (node.refs !== undefined && node.refs.length > 0) {
    const ref = fallbackBranchRef(groupRefs(node.refs));
    if (ref !== null) return { kind: 'ref', ref, oid: node.id };
  }
  // Empty band OR the "+n" chip OR a non-branch entity → commit target.
  return { kind: 'commit', index, oid: node.id };
}
