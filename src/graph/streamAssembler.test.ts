import { describe, expect, it } from 'vitest';

import type { GraphChunk, GraphEdge, GraphLayout, GraphNode, RefLabel, StreamEdge, StreamNode } from '../ipc';
import { createGraphStream } from './streamAssembler';

const oid = (i: number): string => i.toString(16).padStart(2, '0').repeat(20);

/** A hand-built layout mirroring `compute_graph` output: parents at HIGHER rows,
 *  merges (multi-parent), branch lanes, refs on some rows, a resolved headIndex,
 *  and edges sorted ascending by (from, to). */
function buildFixture(): GraphLayout {
  const raw: { lane: number; parents: number[]; refs?: RefLabel[] }[] = [
    { lane: 0, parents: [1, 2], refs: [{ name: 'main', kind: 'localBranch', isHead: true }] }, // 0 merge
    { lane: 0, parents: [3] }, // 1
    { lane: 1, parents: [3], refs: [{ name: 'v1.0', kind: 'tag', isHead: false }] }, // 2 branch tip
    { lane: 0, parents: [4, 5] }, // 3 merge
    { lane: 0, parents: [6] }, // 4
    { lane: 2, parents: [6] }, // 5 second branch
    { lane: 0, parents: [7] }, // 6
    { lane: 0, parents: [8] }, // 7
    { lane: 0, parents: [9] }, // 8
    { lane: 0, parents: [] }, // 9 root
  ];
  const nodes: GraphNode[] = raw.map((r, i) => {
    const n: GraphNode = {
      id: oid(i),
      lane: r.lane,
      parents: r.parents,
      summary: `commit ${i}`,
      author: i % 2 === 0 ? 'Ada Lovelace' : 'Grace Hopper',
      ts: 2000 - i,
      committerTs: 2000 - i,
    };
    if (r.refs !== undefined) n.refs = r.refs;
    return n;
  });
  const edges: GraphEdge[] = [];
  nodes.forEach((n, from) => {
    n.parents.forEach((to, ord) => {
      // Arbitrary-but-deterministic lane per edge; must round-trip as a set.
      edges.push({ from, to, lane: (from + to + ord) % 3 });
    });
  });
  edges.sort((a, b) => a.from - b.from || a.to - b.to);
  return { nodes, edges, laneCount: 3, headIndex: 0, truncated: false };
}

/** Split a full layout into a Meta -> Batch* -> Done chunk sequence, mirroring
 *  the mock/Rust batching. Boundaries are arbitrary (firstBatch/batch sizes). */
function toChunks(layout: GraphLayout, firstBatch: number, batch: number): GraphChunk[] {
  const chunks: GraphChunk[] = [];
  const total = layout.nodes.length;
  chunks.push({
    kind: 'meta',
    total,
    headOid: layout.headIndex !== null ? layout.nodes[layout.headIndex].id : null,
  });
  let start = 0;
  let limit = firstBatch;
  let maxLane = 0;
  while (start < total) {
    const end = Math.min(total, start + limit);
    const nodes: StreamNode[] = [];
    for (let r = start; r < end; r++) {
      const n = layout.nodes[r];
      if (n.lane > maxLane) maxLane = n.lane;
      const sn: StreamNode = {
        id: n.id,
        lane: n.lane,
        summary: n.summary,
        author: n.author,
        ts: n.ts,
        committerTs: n.committerTs,
      };
      if (n.refs !== undefined && n.refs.length > 0) sn.refs = n.refs;
      nodes.push(sn);
    }
    const edges: StreamEdge[] = [];
    for (const e of layout.edges) {
      if (e.to >= start && e.to < end) {
        if (e.lane > maxLane) maxLane = e.lane;
        edges.push({ from: e.from, to: e.to, lane: e.lane, ord: layout.nodes[e.from].parents.indexOf(e.to) });
      }
    }
    chunks.push({ kind: 'batch', startRow: start, laneCountSoFar: maxLane + 1, nodes, edges });
    start = end;
    limit = batch;
  }
  chunks.push({
    kind: 'done',
    totalRows: total,
    laneCount: layout.laneCount,
    headIndex: layout.headIndex,
    truncated: layout.truncated,
  });
  return chunks;
}

const sortEdges = (edges: GraphEdge[]): GraphEdge[] =>
  [...edges].sort((a, b) => a.from - b.from || a.to - b.to || a.lane - b.lane);

function assemble(chunks: GraphChunk[]): ReturnType<typeof createGraphStream> {
  const s = createGraphStream();
  for (const c of chunks) s.apply(c);
  return s;
}

/** The invariant: the assembled layout deep-equals the un-chunked fixture —
 *  nodes incl. ordered parents, edges AS A SET, laneCount, headIndex, truncated. */
function expectEquivalent(s: ReturnType<typeof createGraphStream>, fixture: GraphLayout): void {
  expect(s.layout.nodes).toEqual(fixture.nodes);
  expect(sortEdges(s.layout.edges)).toEqual(sortEdges(fixture.edges));
  expect(s.layout.laneCount).toBe(fixture.laneCount);
  expect(s.layout.headIndex).toBe(fixture.headIndex);
  expect(s.layout.truncated).toBe(fixture.truncated);
}

describe('createGraphStream', () => {
  const fixture = buildFixture();

  it('reconstructs the fixture across a multi-batch split (first=3, batch=4)', () => {
    const s = assemble(toChunks(fixture, 3, 4));
    expectEquivalent(s, fixture);
    expect(s.done).toBe(true);
    expect(s.total).toBe(fixture.nodes.length);
    expect(s.loadedRows).toBe(fixture.nodes.length);
    expect(s.truncated).toBe(false);
  });

  it('reconstructs the fixture from a single giant batch', () => {
    const chunks = toChunks(fixture, 1000, 1000);
    // meta + exactly one batch + done
    expect(chunks.filter((c) => c.kind === 'batch')).toHaveLength(1);
    expectEquivalent(assemble(chunks), fixture);
  });

  it('is batch-boundary-invariant: many arbitrary split sizes reconstruct identically', () => {
    for (const [first, batch] of [
      [1, 1],
      [1, 2],
      [2, 2],
      [2, 3],
      [3, 5],
      [5, 7],
      [7, 512],
      [512, 512],
    ] as const) {
      expectEquivalent(assemble(toChunks(fixture, first, batch)), fixture);
    }
  });

  it('builds oidToRow for every row', () => {
    const s = assemble(toChunks(fixture, 2, 3));
    for (let r = 0; r < fixture.nodes.length; r++) {
      expect(s.oidToRow.get(oid(r))).toBe(r);
    }
  });

  it('resolves headIndex from headOid as soon as HEAD lands (before Done)', () => {
    // Feed only meta + the first batch (row 0 = HEAD): headIndex is already set.
    const chunks = toChunks(fixture, 3, 4);
    const s = createGraphStream();
    s.apply(chunks[0]); // meta
    s.apply(chunks[1]); // first batch (contains row 0)
    expect(s.layout.headIndex).toBe(0);
  });

  it('handles an empty (unborn) stream: meta(total 0) + done', () => {
    const empty: GraphLayout = { nodes: [], edges: [], laneCount: 0, headIndex: null, truncated: false };
    const s = assemble(toChunks(empty, 512, 4096));
    expect(s.layout.nodes).toEqual([]);
    expect(s.layout.edges).toEqual([]);
    expect(s.layout.headIndex).toBeNull();
    expect(s.done).toBe(true);
    expect(s.total).toBe(0);
  });

  it('bumps version on every applied chunk', () => {
    const chunks = toChunks(fixture, 3, 4);
    const s = createGraphStream();
    let prev = s.version;
    for (const c of chunks) {
      s.apply(c);
      expect(s.version).toBeGreaterThan(prev);
      prev = s.version;
    }
  });

  it('throws on a non-contiguous batch startRow (defensive)', () => {
    const s = createGraphStream();
    expect(() =>
      s.apply({ kind: 'batch', startRow: 5, laneCountSoFar: 1, nodes: [], edges: [] }),
    ).toThrow(/startRow/);
  });
});
