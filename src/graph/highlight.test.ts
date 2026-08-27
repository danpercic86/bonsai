/** Spec-006: parent-highlight target math (display space; UI contract §2). */
import { describe, expect, it } from 'vitest';

import type { GraphEdge } from '../ipc';
import { highlightTargets } from './highlight';

const edge = (from: number, to: number, lane = 0): GraphEdge => ({ from, to, lane });

// A small display-space edge set: row 0 is a merge (parents at 1 and 3),
// row 1 has parent 2, row 4 is a root (no outgoing edges).
const EDGES: readonly GraphEdge[] = [edge(0, 1, 0), edge(0, 3, 1), edge(1, 2, 0)];

describe('highlightTargets', () => {
  it('null target → empty', () => {
    expect(highlightTargets(EDGES, null)).toEqual({ edges: [], parentRows: [] });
  });

  it('single parent → one edge + one parent row', () => {
    expect(highlightTargets(EDGES, 1)).toEqual({ edges: [edge(1, 2, 0)], parentRows: [2] });
  });

  it('merge (2+ parents) → one entry per parent edge', () => {
    const hl = highlightTargets(EDGES, 0);
    expect(hl.edges).toEqual([edge(0, 1, 0), edge(0, 3, 1)]);
    expect(hl.parentRows).toEqual([1, 3]);
  });

  it('root commit (no outgoing edges) → empty', () => {
    expect(highlightTargets(EDGES, 4)).toEqual({ edges: [], parentRows: [] });
  });

  it('fold-pill target row → empty (guard; no highlight on pills)', () => {
    expect(highlightTargets(EDGES, 0, new Set([0]))).toEqual({ edges: [], parentRows: [] });
  });

  it('a parent landing on a pill row keeps its edge but is never ringed', () => {
    const hl = highlightTargets(EDGES, 0, new Set([3]));
    expect(hl.edges).toEqual([edge(0, 1, 0), edge(0, 3, 1)]);
    expect(hl.parentRows).toEqual([1]);
  });

  it('parent inside a folded span is absent from the projected input → excluded by construction', () => {
    // Fold projection drops the 0→3 edge entirely; the filter never sees it.
    const projected = [edge(0, 1, 0), edge(1, 2, 0)];
    const hl = highlightTargets(projected, 0);
    expect(hl.edges).toEqual([edge(0, 1, 0)]);
    expect(hl.parentRows).toEqual([1]);
  });

  it('identity projection behaves like the unfolded graph (no foldRows)', () => {
    expect(highlightTargets(EDGES, 1, null)).toEqual(highlightTargets(EDGES, 1));
  });

  it('deduplicates parent rows if two edges share a target row', () => {
    const dup = [edge(0, 2, 0), edge(0, 2, 1)];
    expect(highlightTargets(dup, 0).parentRows).toEqual([2]);
  });
});
