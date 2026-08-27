/** Spec-006: parent-highlight target math (UI contract spec-006-ui.md §2).
 *
 *  Pure DISPLAY-space filter — the input is the already-projected
 *  `visibleEdges` array (spec-004 fold projection has dropped edges into
 *  folded spans and remapped indices), so `node.parents` (MODEL indices) is
 *  never consulted and folded parents are excluded by construction. Fold-pill
 *  rows carry no edges after projection, but a `foldRows` guard is applied
 *  anyway so no ring pass can ever run for a pill target (contract §2.2:
 *  no fold-pill highlight in v1). No canvas, no React. */

import type { GraphEdge } from '../ipc';

export interface HighlightTargets {
  /** Display-space parent edges of `targetRow` (re-stroked in pass 3.5). */
  edges: GraphEdge[];
  /** Display rows to ring in pass 4 (may be off-viewport — paint skips those). */
  parentRows: number[];
}

const EMPTY: HighlightTargets = { edges: [], parentRows: [] };

/** Direct parent edges + parent rows of `targetRow`, in display space.
 *  `targetRow = hoverRow ?? selectedRow`; `null` (or a fold-pill row) → empty.
 *  Root commits yield no edges; merges yield one entry per parent edge. */
export function highlightTargets(
  visibleEdges: readonly GraphEdge[],
  targetRow: number | null,
  foldRows?: ReadonlySet<number> | null,
): HighlightTargets {
  if (targetRow === null || foldRows?.has(targetRow) === true) return EMPTY;
  const edges: GraphEdge[] = [];
  const parentRows: number[] = [];
  for (const e of visibleEdges) {
    if (e.from !== targetRow) continue;
    edges.push(e);
    // A parent landing on a pill row is never ringed (conservative v1 rule).
    if (foldRows?.has(e.to) !== true && !parentRows.includes(e.to)) parentRows.push(e.to);
  }
  return edges.length === 0 ? EMPTY : { edges, parentRows };
}
