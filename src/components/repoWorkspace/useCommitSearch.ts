import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { isGitNotFound } from '../../ipc/errors';
import { GIT_NOT_FOUND_TOAST_KEY, noteGitNotFound } from '../../ipc/gitNotFound';

import type { GraphLayout, SearchQuery, SearchResults } from '../../ipc';
import type { PushToast } from '../../ToastContext';
import { deriveMatchRows, nextMatchIndex, queryKey } from './searchHelpers';

/** UI §5.6: the commit-search surface's line when git is unavailable. */
const SEARCH_NEEDS_GIT = 'Search needs Git — see the notice at the top of the window.';

/** ~250 ms — matches the useReadOverlays / refetchStatus debounce feel. */
const DEBOUNCE_MS = 250;

/** Cheap fields live-search on debounced input; `content` (`-S`/`-G` pickaxe) is
 *  submit-only — never fired per keystroke (P50 §5.1). */
function isLiveField(field: SearchQuery['field']): boolean {
  return field !== 'content';
}

const DEFAULT_QUERY: SearchQuery = {
  text: '',
  field: 'message',
  regex: false,
  caseSensitive: false,
  maxResults: 0,
  scopeRef: null,
};

export interface UseCommitSearch {
  open: boolean;
  openSearch(initialText?: string): void;
  close(): void;
  /** For the workspace Esc-layering (read without a re-subscribe). */
  openRef: { current: boolean };
  query: SearchQuery;
  patchQuery(patch: Partial<SearchQuery>): void;
  /** Force a run now (required for content mode; cheap modes also fire live). */
  submit(): void;
  results: SearchResults | null;
  loading: boolean;
  error: string | null;
  /** Index into `results.matches`; -1 when there is no current match. */
  currentMatch: number;
  /** Node indices in the current layout carrying a match (for GraphCanvas). */
  matchRows: number[];
  /** content mode only: the current query has not been run yet (Enter = submit). */
  needsSubmit: boolean;
  next(): void;
  prev(): void;
  /** Jump to a specific match index (results-list click). */
  goToMatch(index: number): void;
  /** Bumped on every openSearch() call so the bar refocuses its input even when
   *  already open (Ctrl/Cmd-F from elsewhere behaves like native find). */
  openNonce: number;
}

/** P50b: commit-search state. Cheap modes (all/message/author/path) live-search
 *  on a debounced query; content is submit-only. A last-wins reqId guard drops
 *  stale responses (mirrors useReadOverlays). The "current match" IS the normal
 *  graph selection: next/prev/goToMatch call `revealCommitByOid`, reusing the
 *  single-selection + scroll-into-view path — no competing selection model. */
export function useCommitSearch(deps: {
  repoId: string;
  /** Reactive layout — threaded (not a ref) so matchRows recompute when the
   *  graph reorders while search stays open (rings resolve by row index). */
  graph: GraphLayout | null;
  revealCommitByOid(oid: string): void;
  pushToast: PushToast;
}): UseCommitSearch {
  const { repoId, graph, revealCommitByOid, pushToast } = deps;

  const [open, setOpen] = useState(false);
  const [openNonce, setOpenNonce] = useState(0);
  const openRef = useRef(false);
  openRef.current = open;

  const [query, setQuery] = useState<SearchQuery>(DEFAULT_QUERY);
  const [results, setResults] = useState<SearchResults | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [currentMatch, setCurrentMatch] = useState(-1);
  const [ranKey, setRanKey] = useState<string | null>(null);

  const reqIdRef = useRef(0);
  const debounceRef = useRef<number | null>(null);

  const clearDebounce = useCallback(() => {
    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
  }, []);

  // Core search: last-wins reqId guard. On success reset currentMatch to 0 and
  // reveal the first match; on failure surface via the shared error-toast path.
  const run = useCallback(
    async (q: SearchQuery) => {
      clearDebounce();
      if (q.text.trim() === '') {
        reqIdRef.current += 1; // cancel any in-flight
        setResults(null);
        setLoading(false);
        setError(null);
        setCurrentMatch(-1);
        setRanKey(queryKey(q));
        return;
      }
      const reqId = ++reqIdRef.current;
      setLoading(true);
      setError(null);
      try {
        const res = await ipc.searchCommits(repoId, q);
        if (reqIdRef.current !== reqId) return;
        setResults(res);
        setLoading(false);
        setRanKey(queryKey(q));
        if (res.matches.length > 0) {
          setCurrentMatch(0);
          revealCommitByOid(res.matches[0].oid);
        } else {
          setCurrentMatch(-1);
        }
      } catch (e) {
        if (reqIdRef.current !== reqId) return;
        // P70 (UI §10.3): search genuinely CANNOT work without the git program
        // (no SSH caveat here), so point at the notice bar instead of surfacing
        // the raw payload — and latch + coalesce rather than toast per keystroke.
        const gitMissing = isGitNotFound(e);
        const msg = gitMissing ? SEARCH_NEEDS_GIT : errorMessage(e);
        setResults(null);
        setLoading(false);
        setError(msg);
        setCurrentMatch(-1);
        setRanKey(queryKey(q));
        if (gitMissing) {
          noteGitNotFound();
          pushToast('error', msg, GIT_NOT_FOUND_TOAST_KEY);
        } else {
          pushToast('error', msg);
        }
      }
    },
    [repoId, revealCommitByOid, pushToast, clearDebounce],
  );

  // Cheap modes: debounced live search on query change (while open). Content
  // never auto-fires. Empty text clears immediately (no request).
  useEffect(() => {
    if (!open) return;
    if (!isLiveField(query.field)) return;
    if (query.text.trim() === '') {
      void run(query);
      return;
    }
    clearDebounce();
    debounceRef.current = window.setTimeout(() => {
      debounceRef.current = null;
      void run(query);
    }, DEBOUNCE_MS);
    return clearDebounce;
  }, [open, query, run, clearDebounce]);

  const openSearch = useCallback((initialText?: string) => {
    setOpen(true);
    setOpenNonce((n) => n + 1); // signal the bar to (re)focus even if already open
    if (initialText !== undefined) setQuery((q) => ({ ...q, text: initialText }));
  }, []);

  const close = useCallback(() => {
    setOpen(false);
    clearDebounce();
    reqIdRef.current += 1; // drop any in-flight result
    setLoading(false);
  }, [clearDebounce]);

  const patchQuery = useCallback((patch: Partial<SearchQuery>) => {
    setQuery((q) => ({ ...q, ...patch }));
  }, []);

  const submit = useCallback(() => {
    void run(query);
  }, [run, query]);

  // matchRows: highlight rows for the current layout; empty while closed so the
  // ring pass clears on dismiss. Depends on `graph` so the rings re-map to the
  // correct rows when the layout reorders (prepend/checkout/fetch) with results
  // unchanged — otherwise the by-row-index ring pass would draw on stale rows.
  const matchRows = useMemo(
    () => (open ? deriveMatchRows(results, graph) : []),
    [open, results, graph],
  );

  const goToMatch = useCallback(
    (index: number) => {
      const matches = results?.matches ?? [];
      if (index < 0 || index >= matches.length) return;
      setCurrentMatch(index);
      revealCommitByOid(matches[index].oid);
    },
    [results, revealCommitByOid],
  );

  const step = useCallback(
    (dir: 1 | -1) => {
      const matches = results?.matches ?? [];
      if (matches.length === 0) return;
      goToMatch(nextMatchIndex(currentMatch, dir, matches.length));
    },
    [results, currentMatch, goToMatch],
  );
  const next = useCallback(() => step(1), [step]);
  const prev = useCallback(() => step(-1), [step]);

  const needsSubmit = query.field === 'content' && queryKey(query) !== ranKey;

  return {
    open,
    openSearch,
    close,
    openRef,
    query,
    patchQuery,
    submit,
    results,
    loading,
    error,
    currentMatch,
    matchRows,
    needsSubmit,
    next,
    prev,
    goToMatch,
    openNonce,
  };
}
