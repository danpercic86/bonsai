/** Sticky (OID-anchored) selection — the fix for the "UI jumps between the
 *  selected commit and Uncommitted changes on every background refresh" bug.
 *
 *  A refresh round re-streams the graph from row 0, so for a commit deep in
 *  history there is a long window where `graph.nodes[selectedIndex]` is
 *  `undefined`. The anchor must bridge exactly that window — and must NOT
 *  outlive a selection that legitimately went away (rebased/GC'd commit). */

import { describe, it, expect } from 'vitest';
import { renderHook } from '@testing-library/react';

import { resolveStickySelection, useStickySelection } from './useStickySelection';
import type { StickyAnchor } from './useStickySelection';
import type { GraphLayout, GraphNode } from '../../ipc';

function node(i: number): GraphNode {
  return {
    id: String(i).padStart(40, '0'),
    lane: 0,
    parents: i === 0 ? [] : [i - 1],
    summary: `commit ${i}`,
    author: 'Test User',
    ts: 1_700_000_000 + i,
    committerTs: 1_700_000_000 + i,
  };
}

/** A partial layout as published by a streamed batch: `rows` rows only. */
function layout(rows: number): GraphLayout {
  return {
    nodes: Array.from({ length: rows }, (_, i) => node(i)),
    edges: [],
    laneCount: 1,
    headIndex: 0,
    truncated: false,
  };
}

function anchor(i: number): StickyAnchor {
  return { index: i, node: node(i) };
}

describe('resolveStickySelection', () => {
  it('resolves the row when it is present in the layout', () => {
    expect(resolveStickySelection(3, layout(512), null)?.node.id).toBe(node(3).id);
  });

  it('keeps the previous anchor while the SAME row is missing from a partial layout', () => {
    const prev = anchor(600);
    expect(resolveStickySelection(600, layout(512), prev)).toBe(prev);
  });

  it('keeps the previous anchor while the layout is still null', () => {
    const prev = anchor(600);
    expect(resolveStickySelection(600, null, prev)).toBe(prev);
  });

  it('drops the anchor when the index CHANGES to a row absent from the layout', () => {
    // SHOULD-FIX 1: `handleSelectParent` can point at a parent row past the
    // streamed window. Holding the old anchor would keep rendering the previous
    // commit's details AND its action targets while `selectedIndex` already
    // points at the parent — a genuinely different target must never stick.
    expect(resolveStickySelection(900, layout(512), anchor(600))).toBeNull();
  });

  it('drops the anchor when the selection is cleared', () => {
    expect(resolveStickySelection(null, layout(512), anchor(600))).toBeNull();
  });

  it('is idempotent (safe to derive during render)', () => {
    const first = resolveStickySelection(600, layout(512), anchor(600));
    expect(resolveStickySelection(600, layout(512), first)).toBe(first);
  });
});

describe('useStickySelection', () => {
  it('keeps the selected commit rendered across a full re-stream', () => {
    const full = layout(1000);
    const { result, rerender } = renderHook(
      ({ i, g }: { i: number | null; g: GraphLayout | null }) => useStickySelection(i, g),
      { initialProps: { i: 600 as number | null, g: full as GraphLayout | null } },
    );
    const selected = result.current.node;
    expect(selected?.summary).toBe('commit 600');

    // Refresh round: the stream republishes from row 0. Row 600 is absent from
    // the early chunks -> the panel must NOT fall back to the status view.
    rerender({ i: 600, g: layout(0) });
    expect(result.current.node?.id).toBe(selected?.id);
    expect(result.current.oid).toBe(selected?.id);
    rerender({ i: 600, g: layout(512) });
    expect(result.current.oid).toBe(selected?.id);

    // The row finally arrives and the index remaps: same commit, no gap.
    rerender({ i: 600, g: layout(1000) });
    expect(result.current.node?.summary).toBe('commit 600');
  });

  it('clears when the stream completes without the selected commit', () => {
    // refetchGraph clears `selectedIndex` when the prior OID never remapped
    // (rebased/GC'd commit) — the anchor must go with it, not stick forever.
    const { result, rerender } = renderHook(
      ({ i, g }: { i: number | null; g: GraphLayout | null }) => useStickySelection(i, g),
      { initialProps: { i: 600 as number | null, g: layout(1000) as GraphLayout | null } },
    );
    expect(result.current.oid).not.toBeNull();

    rerender({ i: 600, g: layout(400) }); // mid-stream: sticky holds
    expect(result.current.oid).not.toBeNull();
    rerender({ i: null, g: layout(400) }); // stream done, selection cleared
    expect(result.current.node).toBeNull();
    expect(result.current.oid).toBeNull();
  });

  it('does NOT hold the anchor when the selection moves to an unstreamed row', () => {
    // Selecting a parent whose row is past the streamed window: the panel must
    // clear rather than keep showing the previous commit (SHOULD-FIX 1).
    const { result, rerender } = renderHook(
      ({ i, g }: { i: number | null; g: GraphLayout | null }) => useStickySelection(i, g),
      { initialProps: { i: 3 as number | null, g: layout(512) as GraphLayout | null } },
    );
    expect(result.current.oid).toBe(node(3).id);
    rerender({ i: 900, g: layout(512) });
    expect(result.current.node).toBeNull();
    expect(result.current.oid).toBeNull();
  });

  it('keeps a stable oid identity while the row shifts, so OID-keyed effects do not re-run', () => {
    const { result, rerender } = renderHook(
      ({ i, g }: { i: number | null; g: GraphLayout | null }) => useStickySelection(i, g),
      { initialProps: { i: 3 as number | null, g: layout(512) as GraphLayout | null } },
    );
    const oid = result.current.oid;
    rerender({ i: 3, g: layout(0) }); // mid-stream gap
    rerender({ i: 3, g: layout(512) }); // fresh layout objects, same rows
    expect(result.current.oid).toBe(oid);
  });
});
