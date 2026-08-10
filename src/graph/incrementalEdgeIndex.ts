/** P65b: growable, order-independent bucket index over streamed edges (contract
 *  §4.1). The one-shot `edgeIndex.ts` dedupe relies on the global `(from,to)`
 *  sort of `layout.edges`; streamed edges arrive in ascending-`to` order (NOT
 *  `(from,to)`), so that trick is invalid here. This index instead dedupes with
 *  a generation-stamped `Int32Array seen` bumped once per query — correct
 *  regardless of insertion order. View plumbing over Rust-computed geometry — no
 *  layout math. */

import type { GraphEdge } from '../ipc';

export interface IncrementalEdgeIndex {
  bucketSize: number;
  /** Per-row-bucket edge-array indices, push-order (ascending by construction). */
  buckets: number[][];
  /** Backing edge store the assembler appends to; queries read from it. */
  edges: GraphEdge[];
  /** Append `edge` + push its index into every bucket it spans. Order-independent. */
  insert(edge: GraphEdge): void;
  /** Edges overlapping the inclusive row range [firstRow, lastRow]. */
  edgesInRange(firstRow: number, lastRow: number): GraphEdge[];
}

/** Edge `e` occupies buckets `floor(e.from/size) ..= floor(e.to/size)`. */
export function createIncrementalEdgeIndex(bucketSize = 256): IncrementalEdgeIndex {
  const buckets: number[][] = [];
  const edges: GraphEdge[] = [];
  // Generation-stamped dedupe scratch (per-query, never read across queries).
  let seen = new Int32Array(0);
  let gen = 0;

  return {
    bucketSize,
    buckets,
    edges,

    insert(edge: GraphEdge): void {
      const idx = edges.length;
      edges.push(edge);
      const b0 = Math.floor(edge.from / bucketSize);
      const b1 = Math.floor(edge.to / bucketSize);
      // Grow the bucket vector to cover the parent row's bucket.
      while (buckets.length <= b1) buckets.push([]);
      for (let b = b0; b <= b1; b++) buckets[b].push(idx);
    },

    edgesInRange(firstRow: number, lastRow: number): GraphEdge[] {
      const out: GraphEdge[] = [];
      if (buckets.length === 0 || lastRow < firstRow) return out;
      // Grow `seen` (amortized) when the edge store outgrows it. The buffer is
      // per-query scratch, so a fresh zero-filled array + gen reset is correct.
      if (seen.length < edges.length) {
        seen = new Int32Array(Math.max(edges.length, seen.length * 2));
        gen = 0;
      }
      gen++;
      const b0 = Math.max(0, Math.floor(firstRow / bucketSize));
      const b1 = Math.min(buckets.length - 1, Math.floor(lastRow / bucketSize));
      for (let b = b0; b <= b1; b++) {
        const bucket = buckets[b];
        for (let i = 0; i < bucket.length; i++) {
          const idx = bucket[i];
          if (seen[idx] === gen) continue;
          seen[idx] = gen;
          const e = edges[idx];
          if (e.from <= lastRow && e.to >= firstRow) out.push(e);
        }
      }
      return out;
    },
  };
}
