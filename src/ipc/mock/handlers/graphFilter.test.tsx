/** Spec-003 — applyGraphFilter (the fixture-side mock filter): BFS kept-set,
 *  index remap, edge dropping, pill stripping + the synthesized HEAD label, and
 *  the meta-flag semantics (hide-all vs stale fallback), mirroring the locked
 *  Rust rules in plan.md "Approach". */
import { describe, expect, it } from 'vitest';

import { applyGraphFilter } from './graphFilter';
import type { GraphLayout, RefLabel } from '../../types';

const oid = (i: number): string => i.toString(16).padStart(2, '0').repeat(20);

function label(name: string, kind: RefLabel['kind'], isHead = false): RefLabel {
  return { name, kind, isHead };
}

/** rows: 0 = main tip (HEAD, merge of 1+2), 1 = mainline, 2 = side, 3 = root.
 *  `sideRef` controls whether row 2 carries a branch label. */
function layout(sideRef: boolean, extra?: { stashRow?: boolean }): GraphLayout {
  const nodes = [
    {
      id: oid(0),
      lane: 0,
      parents: [1, 2],
      refs: [label('main', 'localBranch', true)],
      summary: 'merge side',
      author: 'a',
      ts: 4,
      committerTs: 4,
    },
    { id: oid(1), lane: 0, parents: [3], summary: 'mainline', author: 'a', ts: 3, committerTs: 3 },
    {
      id: oid(2),
      lane: 1,
      parents: [3],
      ...(sideRef ? { refs: [label('side', 'localBranch')] } : {}),
      summary: 'side work',
      author: 'a',
      ts: 2,
      committerTs: 2,
    },
    { id: oid(3), lane: 0, parents: [], summary: 'root', author: 'a', ts: 1, committerTs: 1 },
  ];
  const edges = [
    { from: 0, to: 1, lane: 0 },
    { from: 0, to: 2, lane: 1 },
    { from: 1, to: 3, lane: 0 },
    { from: 2, to: 3, lane: 1 },
  ];
  if (extra?.stashRow === true) {
    // A stash offshoot of the root: never seeded under a restricted seed.
    nodes.push({
      id: oid(9),
      lane: 2,
      parents: [3],
      refs: [label('stash@{0}', 'stash')],
      summary: 'WIP',
      author: 'a',
      ts: 5,
      committerTs: 5,
    });
    // Stash row appended last; edge to root.
    edges.push({ from: 4, to: 3, lane: 2 });
  }
  return { nodes, edges, laneCount: extra?.stashRow === true ? 3 : 2, headIndex: 0, truncated: false };
}

describe('applyGraphFilter', () => {
  it('null / default filter → the layout passes through, unfiltered', () => {
    const l = layout(true);
    expect(applyGraphFilter(l, null)).toEqual({ layout: l, filtered: false, seedRefsApplied: false });
    expect(
      applyGraphFilter(l, { firstParent: false, seedRefs: null }).filtered,
    ).toBe(false);
  });

  it('first-parent with the side ref DELETED → side node absent, indices remapped', () => {
    const { layout: out, filtered, seedRefsApplied } = applyGraphFilter(layout(false), {
      firstParent: true,
      seedRefs: null,
    });
    expect(filtered).toBe(true);
    expect(seedRefsApplied).toBe(false);
    expect(out.nodes.map((n) => n.id)).toEqual([oid(0), oid(1), oid(3)]);
    // Merge node's parents truncated to the first parent, then remapped.
    expect(out.nodes[0].parents).toEqual([1]);
    // Edges: only first-parent runs survive; the (0→2) edge is gone.
    expect(out.edges).toEqual([
      { from: 0, to: 1, lane: 0 },
      { from: 1, to: 2, lane: 0 },
    ]);
    expect(out.laneCount).toBe(1);
    expect(out.headIndex).toBe(0);
  });

  it('first-parent with the side ref PRESENT → its tip + line remain (documented semantics)', () => {
    const { layout: out } = applyGraphFilter(layout(true), { firstParent: true, seedRefs: null });
    expect(out.nodes.map((n) => n.id)).toEqual([oid(0), oid(1), oid(2), oid(3)]);
  });

  it('solo side → ancestry(side) ∪ ancestry(HEAD); other pills stripped; HEAD synthesized', () => {
    const { layout: out, filtered, seedRefsApplied } = applyGraphFilter(layout(true), {
      firstParent: false,
      seedRefs: ['refs/heads/side'],
    });
    expect(filtered).toBe(true);
    expect(seedRefsApplied).toBe(true);
    expect(out.nodes).toHaveLength(4); // HEAD drags the full merge ancestry in
    // main's pill vanished; the HEAD row keeps a synthesized head label.
    expect(out.nodes[0].refs).toEqual([{ name: 'HEAD', kind: 'head', isHead: true }]);
    expect(out.nodes[2].refs).toEqual([label('side', 'localBranch')]);
  });

  it('hide-all (seedRefs []) → HEAD-only seed, seedRefsApplied true', () => {
    const { layout: out, filtered, seedRefsApplied } = applyGraphFilter(layout(true), {
      firstParent: false,
      seedRefs: [],
    });
    expect(filtered).toBe(true);
    expect(seedRefsApplied).toBe(true);
    // HEAD's ancestry still shows (spec edge case) but every branch pill is gone.
    expect(out.nodes[0].refs).toEqual([{ name: 'HEAD', kind: 'head', isHead: true }]);
    expect(out.nodes[2].refs).toBeUndefined();
  });

  it('stale seedRefs (zero matches) → full layout, seedRefsApplied false; filtered == firstParent', () => {
    const l = layout(true);
    const stale = applyGraphFilter(l, { firstParent: false, seedRefs: ['refs/heads/deleted'] });
    expect(stale).toEqual({ layout: l, filtered: false, seedRefsApplied: false });
    const staleFp = applyGraphFilter(l, { firstParent: true, seedRefs: ['refs/heads/deleted'] });
    expect(staleFp.filtered).toBe(true);
    expect(staleFp.seedRefsApplied).toBe(false);
    // First-parent still applied on the fallback walk.
    expect(staleFp.layout.nodes).toHaveLength(4); // side ref present → its line stays
  });

  it('stash tips: seeded under a null-seed first-parent walk, excluded under solo', () => {
    const l = layout(true, { stashRow: true });
    const fp = applyGraphFilter(l, { firstParent: true, seedRefs: null });
    expect(fp.layout.nodes.some((n) => n.id === oid(9))).toBe(true);
    const solo = applyGraphFilter(l, { firstParent: false, seedRefs: ['refs/heads/main'] });
    expect(solo.layout.nodes.some((n) => n.id === oid(9))).toBe(false);
  });
});
