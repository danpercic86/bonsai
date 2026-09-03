/** OID-anchored ("sticky") selected commit.
 *
 *  The selection is stored as a ROW INDEX, but a background refresh re-streams
 *  the whole graph from row 0: between the first paintable chunk and the chunk
 *  that carries the selected commit's row, `graph.nodes[selectedIndex]` is
 *  `undefined`. Consumers that fell back on that gap (right panel -> status
 *  view, commit browser -> closed, scope -> root) visibly flipped back and forth
 *  on EVERY refresh round for a commit deep in history.
 *
 *  The fix is a one-slot anchor: remember the last node the index actually
 *  resolved to and keep rendering it while THAT SAME index is momentarily
 *  unresolvable. It is dropped the moment `selectedIndex` becomes null —
 *  `refetchGraph` already clears the index when a stream completes without the
 *  prior OID, so a rebased/GC'd commit still clears correctly and nothing sticks
 *  forever. */

import { useMemo, useRef } from 'react';

import type { GraphLayout, GraphNode } from '../../ipc';

/** The anchor is the (row index, node) PAIR the index last resolved to — the
 *  index is part of it because a *changed* index means a different target. */
export interface StickyAnchor {
  readonly index: number;
  readonly node: GraphNode;
}

export interface StickySelection {
  /** The selected commit to render, resolved through the anchor (never
   *  `undefined` — every consumer keeps its defensive null check). */
  readonly node: GraphNode | null;
  /** `node.id`, i.e. a VALUE-stable selection identity across a re-stream. */
  readonly oid: string | null;
}

/** Pure resolution step (unit-tested):
 *  - no selection             -> null (drops the anchor),
 *  - row present in `graph`   -> that node (becomes the new anchor),
 *  - row absent, SAME index   -> the previous anchor (the re-stream gap),
 *  - row absent, OTHER index  -> null.
 *
 *  The last rule is what keeps the anchor honest. The progressive remap
 *  (graphStreamApply.ts) only ever sets an index whose row is already present in
 *  the layout published in the same batch, so "index changed AND row missing"
 *  never comes from a refetch — it is a genuinely different target (e.g.
 *  `handleSelectParent` jumping to a parent row past the streamed window) and
 *  must NOT keep showing the previous commit's details or action targets.
 *  Keyboard nav is unaffected either way: it clamps to `nodes.length - 1`. */
export function resolveStickySelection(
  selectedIndex: number | null,
  graph: GraphLayout | null,
  previous: StickyAnchor | null,
): StickyAnchor | null {
  if (selectedIndex === null) return null;
  const node = graph?.nodes[selectedIndex] ?? null;
  if (node === null) {
    return previous !== null && previous.index === selectedIndex ? previous : null;
  }
  // Reuse the anchor object when nothing changed, so `node`/`oid` identities
  // survive an unrelated re-render.
  return previous !== null && previous.index === selectedIndex && previous.node === node
    ? previous
    : { index: selectedIndex, node };
}

export function useStickySelection(
  selectedIndex: number | null,
  graph: GraphLayout | null,
): StickySelection {
  // Render-derived, not state: `resolveStickySelection` is idempotent for the
  // same inputs (re-applying it to its own output is a no-op), so a StrictMode
  // double render cannot drift and no extra render pass is needed.
  // CONSTRAINT for future authors: this is only safe while no `startTransition`
  // / `useDeferredValue` sits above this container — under a transition React
  // may DISCARD a render, and the mutation below would strand the ref on an
  // anchor that was never committed.
  const anchorRef = useRef<StickyAnchor | null>(null);
  const anchor = resolveStickySelection(selectedIndex, graph, anchorRef.current);
  anchorRef.current = anchor;
  const node = anchor?.node ?? null;
  const oid = node?.id ?? null;
  return useMemo(() => ({ node, oid }), [node, oid]);
}
