// Spec-003: fixture-side graph filter for the browser harness. Mirrors the Rust
// `GraphFilter` semantics closely enough for UI verification — the kept set is a
// BFS from the seeded tip rows over `parents` (first parent only under
// `firstParent`), indices are remapped, edges to removed nodes dropped, and the
// ORIGINAL lane numbers kept (`laneCount = max + 1`). Rust remains the layout
// truth; this is an approximation for `VITE_MOCK_IPC=1` only.
//
// Locked semantics (plan.md "Approach", mirrored verbatim):
//   - `seedRefs: null`  → all ref-bearing tips (stashes included) + HEAD.
//   - `seedRefs: []`    → intentional hide-all: HEAD-only seed, `seedRefsApplied: true`.
//   - non-empty, zero matches → STALE: full seed (as null), `seedRefsApplied: false`,
//     `filtered === firstParent` (first-parent still applies).
//   - stash tips are excluded from the seed whenever `seedRefs` is non-null.
//   - hidden refs' pill labels are stripped; a synthesized HEAD label keeps the
//     HEAD pill resolving when its branch label was filtered out.
import type { GraphFilter, GraphLayout, GraphNode, RefLabel } from '../../types';

export interface FilteredGraph {
  layout: GraphLayout;
  /** Any filter took effect. */
  filtered: boolean;
  /** The seed-ref restriction specifically took effect. */
  seedRefsApplied: boolean;
}

/** Full ref name → the fixture RefLabel shorthand it would carry.
 *  `refs/heads/x` → localBranch "x"; `refs/remotes/origin/x` → remoteBranch
 *  "origin/x"; `refs/tags/t` → tag "t". Unknown prefixes match nothing. */
function labelMatches(fullRef: string, label: RefLabel): boolean {
  if (fullRef.startsWith('refs/heads/')) {
    return label.kind === 'localBranch' && label.name === fullRef.slice('refs/heads/'.length);
  }
  if (fullRef.startsWith('refs/remotes/')) {
    return label.kind === 'remoteBranch' && label.name === fullRef.slice('refs/remotes/'.length);
  }
  if (fullRef.startsWith('refs/tags/')) {
    return label.kind === 'tag' && label.name === fullRef.slice('refs/tags/'.length);
  }
  return false;
}

function refIsSeeded(label: RefLabel, seedRefs: string[]): boolean {
  return seedRefs.some((r) => labelMatches(r, label));
}

/** Apply `filter` to a full fixture layout. Pure. */
export function applyGraphFilter(layout: GraphLayout, filter: GraphFilter | null): FilteredGraph {
  const firstParent = filter?.firstParent === true;
  const seedRefs = filter?.seedRefs ?? null;

  // --- Resolve the seed rows + whether the restriction applied. ---
  let seedRefsApplied = false;
  const seeds = new Set<number>();
  let restrict = false; // strip non-matching pills below
  if (seedRefs === null) {
    for (let i = 0; i < layout.nodes.length; i++) {
      const refs = layout.nodes[i].refs;
      if (refs !== undefined && refs.length > 0) seeds.add(i);
    }
  } else if (seedRefs.length === 0) {
    // Intentional hide-all → HEAD-only seed.
    seedRefsApplied = true;
    restrict = true;
  } else {
    let matched = false;
    for (let i = 0; i < layout.nodes.length; i++) {
      const refs = layout.nodes[i].refs;
      if (refs === undefined) continue;
      if (refs.some((r) => refIsSeeded(r, seedRefs))) {
        seeds.add(i);
        matched = true;
      }
    }
    if (matched) {
      seedRefsApplied = true;
      restrict = true;
    } else {
      // STALE: fall back to the full seed (null semantics).
      for (let i = 0; i < layout.nodes.length; i++) {
        const refs = layout.nodes[i].refs;
        if (refs !== undefined && refs.length > 0) seeds.add(i);
      }
    }
  }
  // Stash tips are excluded from a RESTRICTED seed automatically: a stash label
  // can never match a full ref name, so under `restrict` it is simply not seeded.
  // HEAD is always seeded.
  if (layout.headIndex !== null) seeds.add(layout.headIndex);

  const filtered = seedRefsApplied || firstParent;
  if (!filtered) return { layout, filtered: false, seedRefsApplied: false };

  // --- BFS over parents (first parent only under firstParent). ---
  const kept = new Set<number>();
  const queue = [...seeds];
  while (queue.length > 0) {
    const row = queue.pop() as number;
    if (kept.has(row)) continue;
    kept.add(row);
    const parents = layout.nodes[row].parents;
    const walk = firstParent ? parents.slice(0, 1) : parents;
    for (const p of walk) if (!kept.has(p)) queue.push(p);
  }

  // --- Remap indices (rows keep their relative order). ---
  const keptRows = [...kept].sort((a, b) => a - b);
  const newIndex = new Map<number, number>();
  keptRows.forEach((row, i) => newIndex.set(row, i));

  let maxLane = 0;
  const nodes: GraphNode[] = keptRows.map((row) => {
    const n = layout.nodes[row];
    if (n.lane > maxLane) maxLane = n.lane;
    const parentsWalked = firstParent ? n.parents.slice(0, 1) : n.parents;
    const parents = parentsWalked
      .filter((p) => newIndex.has(p))
      .map((p) => newIndex.get(p) as number);
    const node: GraphNode = { ...n, parents };
    if (restrict && n.refs !== undefined) {
      const keptRefs = n.refs.filter(
        (r) => r.kind === 'head' || (seedRefs !== null && refIsSeeded(r, seedRefs)),
      );
      // Synthesized-Head: keep the HEAD pill resolving when its branch label
      // was filtered out (mirrors Rust collect_refs).
      if (keptRefs.length === 0 && row === layout.headIndex) {
        keptRefs.push({ name: 'HEAD', kind: 'head', isHead: true });
      }
      if (keptRefs.length > 0) node.refs = keptRefs;
      else delete node.refs;
    }
    return node;
  });

  // Keep an edge iff both endpoints survive AND its parent is still one of the
  // child's (possibly truncated) parents.
  const edges = layout.edges
    .filter((e) => {
      const from = newIndex.get(e.from);
      const to = newIndex.get(e.to);
      if (from === undefined || to === undefined) return false;
      return nodes[from].parents.includes(to);
    })
    .map((e) => {
      if (e.lane > maxLane) maxLane = e.lane;
      return {
        from: newIndex.get(e.from) as number,
        to: newIndex.get(e.to) as number,
        lane: e.lane,
      };
    });

  const headIndex =
    layout.headIndex !== null ? (newIndex.get(layout.headIndex) ?? null) : null;

  return {
    layout: { nodes, edges, laneCount: maxLane + 1, headIndex, truncated: layout.truncated },
    filtered,
    seedRefsApplied,
  };
}
