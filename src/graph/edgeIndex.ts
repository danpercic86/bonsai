/** Bucket interval index over the (from,to)-sorted edge array (contract
 * M2-graph.md §4.4). Built ONCE per layout object; queries are O(visible + k).
 * View plumbing over Rust-computed geometry — no layout math here. */

import type { GraphEdge, GraphLayout } from '../ipc';

export interface EdgeIndex {
  bucketSize: number;
  /** Edge indices (into layout.edges) per row bucket, ascending per bucket. */
  buckets: number[][];
}

/** Edge e occupies buckets floor(e.from/size) ..= floor(e.to/size). */
export function buildEdgeIndex(layout: GraphLayout, bucketSize = 256): EdgeIndex {
  const bucketCount = Math.ceil(layout.nodes.length / bucketSize);
  const buckets: number[][] = [];
  for (let i = 0; i < bucketCount; i++) buckets.push([]);
  for (let idx = 0; idx < layout.edges.length; idx++) {
    const e = layout.edges[idx];
    const b0 = Math.floor(e.from / bucketSize);
    const b1 = Math.floor(e.to / bucketSize);
    for (let b = b0; b <= b1 && b < bucketCount; b++) buckets[b].push(idx);
  }
  return { bucketSize, buckets };
}

/**
 * Edges overlapping the inclusive row range [firstRow, lastRow].
 * Dedupe across buckets uses a monotonic "lastEmitted" check: bucket lists are
 * ascending because layout.edges is sorted by (from, to) (§1.1), and any index
 * <= lastEmitted seen in a later bucket is provably a duplicate of an edge
 * already visited in an earlier in-range bucket.
 */
export function edgesInRange(
  layout: GraphLayout,
  ix: EdgeIndex,
  firstRow: number,
  lastRow: number,
): GraphEdge[] {
  const out: GraphEdge[] = [];
  if (ix.buckets.length === 0 || lastRow < firstRow) return out;
  const b0 = Math.max(0, Math.floor(firstRow / ix.bucketSize));
  const b1 = Math.min(ix.buckets.length - 1, Math.floor(lastRow / ix.bucketSize));
  let lastEmitted = -1;
  for (let b = b0; b <= b1; b++) {
    for (const idx of ix.buckets[b]) {
      if (idx <= lastEmitted) continue;
      lastEmitted = idx;
      const e = layout.edges[idx];
      if (e.from <= lastRow && e.to >= firstRow) out.push(e);
    }
  }
  return out;
}
