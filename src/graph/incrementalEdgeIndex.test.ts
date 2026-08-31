import { describe, expect, it } from 'vitest';

import type { GraphEdge } from '../ipc';
import { createIncrementalEdgeIndex } from './incrementalEdgeIndex';

/** Deterministic PRNG so the random-edge property tests are reproducible. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const sortEdges = (edges: GraphEdge[]): GraphEdge[] =>
  [...edges].sort((a, b) => a.from - b.from || a.to - b.to || a.lane - b.lane);

/** Brute-force oracle: every edge overlapping the inclusive [first,last] range. */
const oracle = (edges: GraphEdge[], first: number, last: number): GraphEdge[] =>
  edges.filter((e) => e.from <= last && e.to >= first);

function randomEdges(rand: () => number, count: number, maxRow: number): GraphEdge[] {
  const edges: GraphEdge[] = [];
  for (let i = 0; i < count; i++) {
    const a = Math.floor(rand() * maxRow);
    const b = Math.floor(rand() * maxRow);
    // Enforce from < to (parent always at a higher row than its child).
    if (a === b) edges.push({ from: a, to: a + 1, lane: i % 5 });
    else edges.push({ from: Math.min(a, b), to: Math.max(a, b), lane: i % 5 });
  }
  return edges;
}

describe('createIncrementalEdgeIndex', () => {
  it('exposes the backing edge store, growing one entry per insert', () => {
    const ix = createIncrementalEdgeIndex();
    expect(ix.edges).toHaveLength(0);
    ix.insert({ from: 0, to: 1, lane: 0 });
    ix.insert({ from: 1, to: 3, lane: 1 });
    expect(ix.edges).toHaveLength(2);
  });

  it('grows buckets to cover an edge whose parent is past the current extent', () => {
    const ix = createIncrementalEdgeIndex(4);
    ix.insert({ from: 1, to: 9, lane: 0 }); // spans buckets 0..2
    expect(ix.buckets.length).toBeGreaterThanOrEqual(3);
    // Emitted exactly once despite spanning three in-range buckets.
    expect(ix.edgesInRange(0, 11)).toEqual([{ from: 1, to: 9, lane: 0 }]);
  });

  it('inverted range or empty index returns []', () => {
    const ix = createIncrementalEdgeIndex();
    expect(ix.edgesInRange(5, 3)).toEqual([]);
    expect(ix.edgesInRange(0, 100)).toEqual([]);
    ix.insert({ from: 0, to: 2, lane: 0 });
    expect(ix.edgesInRange(3, 2)).toEqual([]); // lastRow < firstRow
    expect(ix.edgesInRange(5, 9)).toEqual([]); // out of range
  });

  it('matches the oracle for ascending-to inserts (the streamed arrival order)', () => {
    const rand = mulberry32(0x1234);
    const edges = randomEdges(rand, 500, 800);
    const streamed = [...edges].sort((a, b) => a.to - b.to); // as delivered by the walk
    const ix = createIncrementalEdgeIndex(64);
    for (const e of streamed) ix.insert(e);
    for (let q = 0; q < 300; q++) {
      const r1 = Math.floor(rand() * 850);
      const r2 = Math.floor(rand() * 850);
      const first = Math.min(r1, r2);
      const last = Math.max(r1, r2);
      expect(sortEdges(ix.edgesInRange(first, last))).toEqual(
        sortEdges(oracle(edges, first, last)),
      );
    }
  });

  it('is order-independent: shuffled inserts give identical query results', () => {
    const rand = mulberry32(0x999);
    const edges = randomEdges(rand, 300, 400);
    const shuffled = [...edges];
    for (let i = shuffled.length - 1; i > 0; i--) {
      const j = Math.floor(rand() * (i + 1));
      [shuffled[i], shuffled[j]] = [shuffled[j], shuffled[i]];
    }
    const ix = createIncrementalEdgeIndex(32);
    for (const e of shuffled) ix.insert(e);
    for (let q = 0; q < 200; q++) {
      const a = Math.floor(rand() * 420);
      const b = Math.floor(rand() * 420);
      const [first, last] = a <= b ? [a, b] : [b, a];
      expect(sortEdges(ix.edgesInRange(first, last))).toEqual(
        sortEdges(oracle(edges, first, last)),
      );
    }
  });

  it('grows the dedupe buffer correctly across interleaved insert + query batches', () => {
    const rand = mulberry32(7);
    const ix = createIncrementalEdgeIndex(16);
    const all: GraphEdge[] = [];
    for (let batch = 0; batch < 25; batch++) {
      const es = randomEdges(rand, 40, 500).sort((a, b) => a.to - b.to);
      for (const e of es) {
        ix.insert(e);
        all.push(e);
      }
      // Query after each insert batch → forces the `seen` buffer to grow while
      // the edge store keeps expanding (the interesting incremental case).
      const first = batch * 8;
      const last = first + 60;
      expect(sortEdges(ix.edgesInRange(first, last))).toEqual(
        sortEdges(oracle(all, first, last)),
      );
    }
  });
});
