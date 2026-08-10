/** P65b: folds a `GraphChunk` stream (meta -> batch* -> done) into a growing
 *  `GraphLayout` (contract §4.2). Produces the SAME shape the renderer already
 *  consumes, so ALL downstream consumers (drawGraph, search match-rings,
 *  reveal, CommitPanel) work unchanged once assembly reaches them and degrade
 *  gracefully mid-stream. Pure + unit-testable — no React, no IPC.
 *
 *  INVARIANT: after `done` on a COMPLETE (non-truncated) walk, `layout`
 *  deep-equals `ipc.getGraph`'s output — nodes incl. ordered `parents`, edges as
 *  a SET (streamed edges arrive ascending-`to`, not `(from,to)`-sorted),
 *  `laneCount`, `headIndex`, `truncated`. This is the frontend half of the
 *  lane-color stability guarantee. */

import type { GraphChunk, GraphLayout, GraphNode } from '../ipc';
import { createIncrementalEdgeIndex } from './incrementalEdgeIndex';
import type { IncrementalEdgeIndex } from './incrementalEdgeIndex';

export interface GraphStream {
  /** Grows in place; the caller bumps its object identity per applied batch to
   *  drive GraphCanvas's layout-dep repaint (§4.3). */
  layout: GraphLayout;
  edgeIndex: IncrementalEdgeIndex;
  /** Shared reveal/search index, built as rows arrive. */
  oidToRow: Map<string, number>;
  /** Meta.total (spacer sizing); null when the backend cannot cheaply count. */
  total: number | null;
  loadedRows: number;
  done: boolean;
  truncated: boolean;
  /** Increments on every applied chunk (repaint trigger). */
  version: number;
  apply(chunk: GraphChunk): void;
}

export function createGraphStream(): GraphStream {
  const edgeIndex = createIncrementalEdgeIndex();
  // `layout.edges` ALIASES the index's backing store: `edgeIndex.insert` appends
  // to it, so the two never diverge and edges are pushed exactly once. (The
  // streamed path always feeds the index prop to GraphCanvas, so the one-shot
  // `buildEdgeIndex(layout)` — which needs a (from,to)-sorted array — is unused.)
  const layout: GraphLayout = {
    nodes: [],
    edges: edgeIndex.edges,
    laneCount: 0,
    headIndex: null,
    truncated: false,
  };
  const oidToRow = new Map<string, number>();
  let headOid: string | null = null;

  function resolveHead(): void {
    if (headOid !== null && layout.headIndex === null) {
      const row = oidToRow.get(headOid);
      if (row !== undefined) layout.headIndex = row;
    }
  }

  const stream: GraphStream = {
    layout,
    edgeIndex,
    oidToRow,
    total: null,
    loadedRows: 0,
    done: false,
    truncated: false,
    version: 0,

    apply(chunk: GraphChunk): void {
      switch (chunk.kind) {
        case 'meta': {
          stream.total = chunk.total;
          headOid = chunk.headOid;
          resolveHead();
          stream.version++;
          break;
        }
        case 'batch': {
          // Defensive: rows must arrive strictly appended, contiguous from 0.
          if (chunk.startRow !== layout.nodes.length) {
            throw new Error(
              `streamAssembler: batch startRow ${chunk.startRow} != expected ${layout.nodes.length}`,
            );
          }
          for (let i = 0; i < chunk.nodes.length; i++) {
            const n = chunk.nodes[i];
            const absRow = chunk.startRow + i;
            const node: GraphNode = {
              id: n.id,
              lane: n.lane,
              parents: [],
              summary: n.summary,
              author: n.author,
              ts: n.ts,
              committerTs: n.committerTs,
            };
            if (n.refs !== undefined && n.refs.length > 0) node.refs = n.refs;
            layout.nodes.push(node);
            oidToRow.set(n.id, absRow);
          }
          for (const e of chunk.edges) {
            // Appends to edgeIndex.edges === layout.edges (aliased above).
            edgeIndex.insert({ from: e.from, to: e.to, lane: e.lane });
            // Rebuild the child's ordered parents from the edge ordinal. Sparse
            // until every parent's row arrives; dense (== compute_graph) on a
            // complete walk. Matches CommitPanel's `parents[i] !== undefined`.
            const child = layout.nodes[e.from];
            if (child !== undefined) child.parents[e.ord] = e.to;
          }
          // Monotonic running-max width (never shrinks; §4.3 / OQ5).
          layout.laneCount = Math.max(layout.laneCount, chunk.laneCountSoFar);
          stream.loadedRows = layout.nodes.length;
          resolveHead();
          stream.version++;
          break;
        }
        case 'done': {
          // Authoritative final scalars (redundant with the accumulated stream).
          layout.laneCount = chunk.laneCount;
          layout.headIndex = chunk.headIndex;
          layout.truncated = chunk.truncated;
          stream.truncated = chunk.truncated;
          stream.done = true;
          stream.version++;
          break;
        }
      }
    },
  };
  return stream;
}
