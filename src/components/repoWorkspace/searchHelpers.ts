import type { GraphLayout, SearchQuery, SearchResults } from '../../ipc';

/** P50b: pure helpers for commit-search graph highlight + next/prev jump. Kept
 *  separate from the hook so the index math is trivially unit-testable and
 *  RepoWorkspace never grows inline search logic. */

/** Map a search result set onto row indices in the current graph layout,
 *  dropping matches whose oid is not present (off-view / different repo). Order
 *  follows `results.matches` (newest-first). Pure — the caller passes the graph
 *  snapshot the search ran against. */
export function deriveMatchRows(
  results: SearchResults | null,
  graph: GraphLayout | null,
): number[] {
  if (results === null || graph === null) return [];
  const index = new Map<string, number>();
  for (let i = 0; i < graph.nodes.length; i += 1) index.set(graph.nodes[i].id, i);
  const rows: number[] = [];
  for (const m of results.matches) {
    const row = index.get(m.oid);
    if (row !== undefined) rows.push(row);
  }
  return rows;
}

/** Serialized query identity — drives the content-mode "needs (re)submit" flag
 *  so Enter runs the pickaxe once, then advances matches (browser-find feel).
 *  Joined with `'\0'` (never appears in the fields) so queries whose text
 *  contains the separator cannot collide. Pure. */
export function queryKey(q: SearchQuery): string {
  return [
    q.field,
    q.text,
    q.regex ? 1 : 0,
    q.caseSensitive ? 1 : 0,
    q.scopeRef ?? '',
    q.maxResults,
  ].join('\0');
}

/** Next match index with wrap-around. `cur < 0` (no current match) starts at
 *  the first (dir +1) or last (dir -1). Returns -1 for an empty set. Pure. */
export function nextMatchIndex(cur: number, dir: 1 | -1, len: number): number {
  if (len <= 0) return -1;
  if (cur < 0) return dir === 1 ? 0 : len - 1;
  return (cur + dir + len) % len;
}
