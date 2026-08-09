import { describe, expect, it } from 'vitest';

import type { GraphEdge, GraphLayout, GraphNode } from '../ipc';
import { buildEdgeIndex, edgesInRange } from './edgeIndex';

const node = (i: number): GraphNode => ({
  id: String(i).padStart(40, '0'),
  lane: 0,
  parents: [],
  summary: `c${i}`,
  author: 'a',
  ts: i,
  committerTs: i,
});

const layoutOf = (nodeCount: number, edges: GraphEdge[]): GraphLayout => ({
  nodes: Array.from({ length: nodeCount }, (_, i) => node(i)),
  edges, // must be sorted by (from, to) per contract §1.1
  laneCount: 1,
  headIndex: null,
  truncated: false,
});

const e = (from: number, to: number, lane = 0): GraphEdge => ({ from, to, lane });

describe('buildEdgeIndex', () => {
  it('empty layout → zero buckets', () => {
    const ix = buildEdgeIndex(layoutOf(0, []));
    expect(ix.buckets).toEqual([]);
  });

  it('bucket count is ceil(nodes/bucketSize)', () => {
    expect(buildEdgeIndex(layoutOf(10, []), 4).buckets).toHaveLength(3);
    expect(buildEdgeIndex(layoutOf(8, []), 4).buckets).toHaveLength(2);
    expect(buildEdgeIndex(layoutOf(1, []), 256).buckets).toHaveLength(1);
  });

  it('an edge lands in every bucket its [from,to] span touches', () => {
    const ix = buildEdgeIndex(layoutOf(12, [e(1, 9)]), 4);
    expect(ix.buckets).toEqual([[0], [0], [0]]);
  });

  it('an edge inside one bucket appears only there', () => {
    const ix = buildEdgeIndex(layoutOf(12, [e(5, 6)]), 4);
    expect(ix.buckets).toEqual([[], [0], []]);
  });

  it('per-bucket index lists stay ascending for sorted edges', () => {
    const ix = buildEdgeIndex(layoutOf(8, [e(0, 1), e(0, 5), e(2, 3), e(6, 7)]), 4);
    expect(ix.buckets[0]).toEqual([0, 1, 2]);
    expect(ix.buckets[1]).toEqual([1, 3]);
  });

  it('adversarial: edge.to past the last node is clamped to existing buckets (no throw)', () => {
    const ix = buildEdgeIndex(layoutOf(4, [e(0, 999)]), 4);
    expect(ix.buckets).toEqual([[0]]);
  });
});

describe('edgesInRange', () => {
  const edges = [e(0, 1), e(0, 5), e(2, 3), e(4, 9), e(6, 7), e(8, 11)];
  const layout = layoutOf(12, edges);
  const ix = buildEdgeIndex(layout, 4);

  it('returns exactly the overlapping edges, in index order, no duplicates', () => {
    // Rows 4..7 overlap: (0,5), (4,9), (6,7).
    expect(edgesInRange(layout, ix, 4, 7)).toEqual([e(0, 5), e(4, 9), e(6, 7)]);
  });

  it('a spanning edge crossing multiple in-range buckets is emitted once', () => {
    const out = edgesInRange(layout, ix, 0, 11);
    expect(out).toEqual(edges);
  });

  it('touching endpoints count as overlap (inclusive range)', () => {
    expect(edgesInRange(layout, ix, 5, 5)).toEqual([e(0, 5), e(4, 9)]);
    expect(edgesInRange(layout, ix, 1, 1)).toEqual([e(0, 1), e(0, 5)]);
  });

  it('range with no edges → []', () => {
    const sparse = layoutOf(12, [e(0, 1)]);
    const six = buildEdgeIndex(sparse, 4);
    expect(edgesInRange(sparse, six, 8, 11)).toEqual([]);
  });

  it('inverted range (lastRow < firstRow) → []', () => {
    expect(edgesInRange(layout, ix, 7, 4)).toEqual([]);
  });

  it('empty index → []', () => {
    const empty = layoutOf(0, []);
    expect(edgesInRange(empty, buildEdgeIndex(empty), 0, 100)).toEqual([]);
  });

  it('negative firstRow clamps to bucket 0; huge lastRow clamps to the last bucket', () => {
    expect(edgesInRange(layout, ix, -50, 1_000_000)).toEqual(edges);
  });

  it('default bucketSize 256 works on a 20-row layout (single bucket)', () => {
    const ix256 = buildEdgeIndex(layoutOf(20, [e(0, 19), e(3, 4)]));
    expect(ix256.buckets).toHaveLength(1);
    expect(edgesInRange(layoutOf(20, [e(0, 19), e(3, 4)]), ix256, 10, 12)).toEqual([e(0, 19)]);
  });
});
