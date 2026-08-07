// Commit/content search mock (P50a). Filters the ACTIVE mock graph fixtures by
// the query so P50b can drive graph highlight + next/prev jump in the browser
// harness. This is UI-plumbing only: fixtures carry no file data, so path /
// content modes reuse the message heuristic (documented below).
import type {
  AppError,
  GraphNode,
  IpcApi,
  MatchedField,
  SearchField,
  SearchMatch,
  SearchQuery,
  SearchResults,
} from '../../types';
import { delay, requireRepo } from '../repoState';
import { resolveLayout } from './layout';

/** A `text` containing this substring rejects with a `git` AppError so the
 *  harness can drive the error-toast path (mirrors external.ts's `#fail`). */
const FAIL_SENTINEL = '#fail';
/** Hard cap mirroring the backend's `MAX_SEARCH_RESULTS`. */
const MAX_SEARCH_RESULTS = 1000;

/** Which field a node matches for `field`, or null. Case folded per the query.
 *  path/content have no file data in fixtures ⇒ they reuse the summary
 *  heuristic (enough to exercise highlight/jump; NOT real pickaxe/pathspec). */
function matchNode(
  node: GraphNode,
  field: SearchField,
  needle: string,
  fold: (s: string) => string,
): MatchedField | null {
  const msgHit = fold(node.summary).includes(needle);
  const authorHit = fold(node.author).includes(needle);
  switch (field) {
    case 'message':
      return msgHit ? 'message' : null;
    case 'author':
      return authorHit ? 'author' : null;
    case 'all':
      return msgHit ? 'message' : authorHit ? 'author' : null;
    case 'path':
    case 'content':
      return msgHit ? field : null;
  }
}

export const searchHandlers = {
  async searchCommits(repoId: string, query: SearchQuery): Promise<SearchResults> {
    await delay(120);
    const state = requireRepo(repoId);
    if (query.text.includes(FAIL_SENTINEL)) {
      const err: AppError = { kind: 'git', message: 'Mock: search failed' };
      throw err;
    }
    const text = query.text.trim();
    if (text === '') return { matches: [], truncated: false };

    const cap =
      query.maxResults > 0 ? Math.min(query.maxResults, MAX_SEARCH_RESULTS) : MAX_SEARCH_RESULTS;
    const fold = (s: string): string => (query.caseSensitive ? s : s.toLowerCase());
    const needle = fold(text);

    const layout = resolveLayout(state);
    const hits: SearchMatch[] = [];
    for (const node of layout.nodes) {
      const matched = matchNode(node, query.field, needle, fold);
      if (matched === null) continue;
      const hit: SearchMatch = {
        oid: node.id,
        summary: node.summary,
        authorName: node.author,
        authorTs: node.ts,
        matched,
      };
      if (matched === 'path') hit.snippet = text; // path-only snippet (OQ4)
      hits.push(hit);
    }
    // cap+1 truncation trick, exactly like the backend: a broad query over the
    // 20k fixture (`?fixture=20k`) exceeds the cap ⇒ truncated:true.
    return { matches: hits.slice(0, cap), truncated: hits.length > cap };
  },
} satisfies Partial<IpcApi>;
