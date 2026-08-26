/** Spec-004: project a model-space `GraphLayout` into DISPLAY space under a
 *  fold model — a pure view transform (deviation from plan.md's letter, same
 *  contract: GraphCanvas/draw keep painting "layout rows", which are now
 *  display rows; all topology decisions stay in Rust's `FoldSpan`s).
 *
 *  - Every collapsed span's hidden rows are replaced by ONE synthetic
 *    placeholder node (never painted as a commit — draw.ts skips `foldRows`;
 *    drawFold.ts paints the pill row instead).
 *  - Edges touching a hidden row are dropped (the span's own chain edges); the
 *    dashed connector on the pill row is painted by drawFold.ts. Rust's fold
 *    rule 4 guarantees no OTHER edge crosses a span, so every surviving edge
 *    remaps exactly (no interior row of a long edge can be hidden).
 *  - `headIndex` is never hidden (fold rule 1), so it remaps directly. */

import type { FoldSpan, GraphEdge, GraphLayout, GraphNode } from '../ipc';
import { displayToModel, modelToDisplay, spanContaining } from './foldModel';
import type { FoldModel } from './foldModel';

export interface ProjectedGraph {
  /** Display-space layout (the INPUT layout object when `identity`). */
  layout: GraphLayout;
  /** Display row → the collapsed span whose pill renders there. */
  foldRows: ReadonlyMap<number, FoldSpan>;
  /** Display row of an EXPANDED span's first revealed row (`span.start`) →
   *  that span (the boundary "Collapse N" pill, UI contract §2). */
  boundaryRows: ReadonlyMap<number, FoldSpan>;
  /** True when nothing is collapsed (layout passed through untouched). */
  identity: boolean;
}

const EMPTY_MAP: ReadonlyMap<number, FoldSpan> = new Map();

/** Synthetic placeholder node for a pill row. Never hit-tested as a commit and
 *  skipped by every draw pass except drawFold. */
function foldNode(span: FoldSpan): GraphNode {
  return {
    id: `__fold:${span.start}`,
    lane: span.lane,
    parents: [],
    summary: '',
    author: '',
    ts: 0,
    committerTs: 0,
  };
}

export function projectLayout(
  layout: GraphLayout,
  model: FoldModel,
  expandedSpans: readonly FoldSpan[],
): ProjectedGraph {
  const boundaryRows = new Map<number, FoldSpan>();
  for (const s of expandedSpans) {
    if (s.start < layout.nodes.length) boundaryRows.set(modelToDisplay(model, s.start), s);
  }
  if (model.identity) {
    return { layout, foldRows: EMPTY_MAP, boundaryRows, identity: true };
  }

  const n = model.displayRowCount;
  const nodes: GraphNode[] = new Array<GraphNode>(n);
  const foldRows = new Map<number, FoldSpan>();
  for (let d = 0; d < n; d++) {
    const m = displayToModel(model, d);
    if (m.kind === 'fold') {
      nodes[d] = foldNode(m.span);
      foldRows.set(d, m.span);
    } else {
      nodes[d] = layout.nodes[m.row];
    }
  }

  const edges: GraphEdge[] = [];
  for (const e of layout.edges) {
    if (spanContaining(model, e.from) !== null || spanContaining(model, e.to) !== null) continue;
    edges.push({ from: modelToDisplay(model, e.from), to: modelToDisplay(model, e.to), lane: e.lane });
  }

  return {
    layout: {
      nodes,
      edges,
      laneCount: layout.laneCount,
      headIndex: layout.headIndex !== null ? modelToDisplay(model, layout.headIndex) : null,
      truncated: layout.truncated,
    },
    foldRows,
    boundaryRows,
    identity: false,
  };
}
