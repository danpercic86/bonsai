import { describe, expect, it } from 'vitest';
import { buildFoldModel } from './foldModel';
import { projectLayout } from './foldProject';
import type { FoldSpan, GraphLayout, GraphNode } from '../ipc';

function node(id: string, lane = 0): GraphNode {
  return { id, lane, parents: [], summary: id, author: 'a', ts: 0, committerTs: 0 };
}

/** 10-row single-lane chain: edges (r, r+1); HEAD at 0. */
function chain(n: number): GraphLayout {
  return {
    nodes: Array.from({ length: n }, (_, i) => node(`c${i}`)),
    edges: Array.from({ length: n - 1 }, (_, i) => ({ from: i, to: i + 1, lane: 0 })),
    laneCount: 1,
    headIndex: 0,
    truncated: false,
  };
}

const SPAN: FoldSpan = { start: 2, count: 5, lane: 0 };

describe('projectLayout', () => {
  it('identity passthrough when nothing is collapsed', () => {
    const layout = chain(10);
    const p = projectLayout(layout, buildFoldModel(10, [SPAN], new Set([2])), [SPAN]);
    expect(p.identity).toBe(true);
    expect(p.layout).toBe(layout); // same object — zero-cost
    // The expanded span's boundary row still carries the collapse pill.
    expect(p.boundaryRows.get(2)).toEqual(SPAN);
  });

  it('collapses hidden rows into one synthetic pill row and remaps edges', () => {
    const layout = chain(10);
    const p = projectLayout(layout, buildFoldModel(10, [SPAN], new Set()), []);
    expect(p.identity).toBe(false);
    expect(p.layout.nodes).toHaveLength(6);
    expect(p.layout.nodes[1].id).toBe('c1');
    expect(p.layout.nodes[2].id).toBe('__fold:2');
    expect(p.layout.nodes[3].id).toBe('c7');
    expect(p.foldRows.get(2)).toEqual(SPAN);
    // Span-adjacent + interior chain edges dropped; others remapped exactly.
    expect(p.layout.edges).toEqual([
      { from: 0, to: 1, lane: 0 },
      { from: 3, to: 4, lane: 0 },
      { from: 4, to: 5, lane: 0 },
    ]);
    expect(p.layout.headIndex).toBe(0);
    expect(p.layout.laneCount).toBe(1);
  });

  it('remaps headIndex below a collapsed span', () => {
    const layout = { ...chain(10), headIndex: 8 };
    const p = projectLayout(layout, buildFoldModel(10, [SPAN], new Set()), []);
    expect(p.layout.headIndex).toBe(4); // 8 - (5 - 1)
  });
});
