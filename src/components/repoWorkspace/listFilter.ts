// P50d — pure list-filter helpers for the sidebar Branches / Remotes / Tags
// sections. Display-only: filtering never touches git, it only narrows what the
// existing flat/tree renderers show. Matching is a case-insensitive substring
// test over each row's FULL display name (e.g. `feature/sidebar`, `origin/main`,
// `v1.0`), so typing a mid-path segment keeps the matching leaves.

import type { TreeNode } from '../../utils/pathTree';

/** Trim + lowercase a raw query into a comparison needle. */
function needleOf(query: string): string {
  return query.trim().toLowerCase();
}

/**
 * Contract §7 helper — filter a flat list of display names by a
 * case-insensitive substring. A blank query is the identity (returns the same
 * array). Used for the tag list (plain `string[]`).
 */
export function filterByName(names: string[], query: string): string[] {
  const needle = needleOf(query);
  if (needle === '') return names;
  return names.filter((n) => n.toLowerCase().includes(needle));
}

/**
 * Flat mode for object rows (local branches / remote-tracking branches): keep
 * the items whose `getName(item)` matches. Blank query → a shallow copy of
 * every item (identity semantics; a copy keeps the return type mutable).
 */
export function filterItems<T>(
  items: readonly T[],
  query: string,
  getName: (item: T) => string,
): T[] {
  const needle = needleOf(query);
  if (needle === '') return [...items];
  return items.filter((it) => getName(it).toLowerCase().includes(needle));
}

/**
 * Tree mode — keep a leaf whose `getName(leaf.item)` matches; keep a directory
 * when at least one descendant leaf survives, so matching leaves stay reachable
 * under their ancestor folders. Empty directories are dropped. Blank query →
 * the original nodes unchanged (identity). Pure: surviving dirs are rebuilt as
 * fresh nodes (with `fullPrefix`/`name` preserved so the Tree's collapse keys
 * stay stable); the input is never mutated.
 */
export function filterTree<T>(
  nodes: readonly TreeNode<T>[],
  query: string,
  getName: (item: T) => string,
): TreeNode<T>[] {
  const needle = needleOf(query);
  if (needle === '') return [...nodes];
  const out: TreeNode<T>[] = [];
  for (const node of nodes) {
    if (node.kind === 'leaf') {
      if (getName(node.item).toLowerCase().includes(needle)) out.push(node);
    } else {
      const children = filterTree(node.children, query, getName);
      if (children.length > 0) out.push({ ...node, children });
    }
  }
  return out;
}
