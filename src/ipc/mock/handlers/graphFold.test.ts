import { describe, expect, it } from 'vitest';
import { computeMockFoldSpans } from './graphFold';
import type { GraphLayout, GraphNode, RefLabel } from '../../types';

function node(id: string, lane = 0, refs?: RefLabel[]): GraphNode {
  const n: GraphNode = { id, lane, parents: [], summary: id, author: 'a', ts: 0, committerTs: 0 };
  if (refs !== undefined) n.refs = refs;
  return n;
}

/** Single-lane chain with contiguous edges; HEAD at row 0. */
function chain(n: number, headIndex: number | null = 0): GraphLayout {
  return {
    nodes: Array.from({ length: n }, (_, i) => node(`c${i}`)),
    edges: Array.from({ length: n - 1 }, (_, i) => ({ from: i, to: i + 1, lane: 0 })),
    laneCount: 1,
    headIndex,
    truncated: false,
  };
}

describe('computeMockFoldSpans (mirrors the Rust rule)', () => {
  it('folds a long linear run; HEAD and the root anchor stay out', () => {
    // Row 0 = HEAD (rule 1); row 9 has no outgoing edge (rule 2) — so the
    // foldable run is rows 1..8 → one span of 8.
    const spans = computeMockFoldSpans(chain(10), [], false);
    expect(spans).toEqual([{ start: 1, count: 8, lane: 0 }]);
  });

  it('a run shorter than MIN_FOLD_RUN (5) never folds', () => {
    // 6 rows → foldable rows 1..4 = 4 < 5.
    expect(computeMockFoldSpans(chain(6), [], false)).toEqual([]);
  });

  it('a ref mid-run splits it', () => {
    const layout = chain(16);
    layout.nodes[8].refs = [{ name: 'v1', kind: 'tag', isHead: false }];
    const spans = computeMockFoldSpans(layout, [], false);
    expect(spans).toEqual([
      { start: 1, count: 7, lane: 0 },
      { start: 9, count: 6, lane: 0 },
    ]);
  });

  it('HEAD mid-run splits it (detached-HEAD safety)', () => {
    const spans = computeMockFoldSpans(chain(16, 8), [], false);
    expect(spans.some((s) => s.start <= 8 && 8 < s.start + s.count)).toBe(false);
  });

  it('a crossing edge blocks the rows it passes through (rule 4)', () => {
    const layout = chain(12);
    // Long edge 1 → 10 on another lane crosses rows 2..9.
    layout.edges.push({ from: 1, to: 10, lane: 1 });
    layout.edges.sort((a, b) => a.from - b.from || a.to - b.to);
    expect(computeMockFoldSpans(layout, [], false)).toEqual([]);
  });

  it('under first-parent a REAL merge row never folds (rule 5)', () => {
    const base = computeMockFoldSpans(chain(12), [], false);
    expect(base).toEqual([{ start: 1, count: 10, lane: 0 }]);
    const spans = computeMockFoldSpans(chain(12), [6], true);
    expect(spans.some((s) => s.start <= 6 && 6 < s.start + s.count)).toBe(false);
    // merge rows only matter under first-parent.
    expect(computeMockFoldSpans(chain(12), [6], false)).toEqual(base);
  });
});
