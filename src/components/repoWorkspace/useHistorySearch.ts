import { useCallback, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type {
  GraphLayout,
  HistoryHit,
  HistoryQuery,
  IndexProgress,
  IndexStatus,
} from '../../ipc';
import type { PushToast } from '../../ToastContext';

const DEFAULT_QUERY: HistoryQuery = { text: '', topK: 0 };

export interface UseHistorySearch {
  open: boolean;
  openPanel(): void;
  close(): void;
  /** For the workspace Esc-layering (read without a re-subscribe). */
  openRef: { current: boolean };

  /** Persisted-index status (built?, count, staleness). `null` until first read. */
  status: IndexStatus | null;
  refreshStatus(): void;
  /** A build is streaming progress. */
  building: boolean;
  progress: IndexProgress | null;
  build(): void;

  query: HistoryQuery;
  setText(t: string): void;

  /** Relevance-ranked retrieval hits (submit-only). */
  hits: HistoryHit[];
  searching: boolean;
  error: string | null;
  /** Whether a retrieval has run for the current open session (drives the
   *  "No relevant commits" empty state vs. the pristine panel). */
  searched: boolean;
  search(): void;

  /** Hit oids present in the current layout → GraphCanvas match rings. */
  matchRows: number[];

  /** Route an AI answer into the shared AiOutputPanel (gated by `canAsk`). */
  askAi(): void;
  /** The "Ask AI" affordance is live (AI eligible AND an index exists). */
  canAsk: boolean;
}

/** P57c: semantic-history search state. Retrieval is SUBMIT-ONLY (Enter / the
 *  Search button — never per-keystroke; it reads a persisted index but is still
 *  an `invoke`). A last-wins reqId guard drops stale responses (mirrors
 *  `useCommitSearch`). The AI answer renders in the shared `AiOutputPanel` via
 *  `runAiAnswer` (RepoWorkspace `aiPanel` req-id state, mirroring `runAnalyze`). */
export function useHistorySearch(deps: {
  repoId: string;
  /** Reactive layout for oid→row (matchRows) — threaded as a VALUE (not a ref)
   *  so the rings re-map when the graph reloads/reorders while hits are shown
   *  (mirrors useCommitSearch). */
  graph: GraphLayout | null;
  revealCommitByOid(oid: string): void;
  aiEligible: boolean;
  runAiAnswer(question: string, topK: number): void;
  pushToast: PushToast;
}): UseHistorySearch {
  const { repoId, graph, revealCommitByOid, aiEligible, runAiAnswer, pushToast } = deps;

  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  openRef.current = open;

  const [status, setStatus] = useState<IndexStatus | null>(null);
  const [building, setBuilding] = useState(false);
  const [progress, setProgress] = useState<IndexProgress | null>(null);

  const [query, setQuery] = useState<HistoryQuery>(DEFAULT_QUERY);
  const [hits, setHits] = useState<HistoryHit[]>([]);
  const [searching, setSearching] = useState(false);
  const [searched, setSearched] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const statusReqId = useRef(0);
  const searchReqId = useRef(0);

  // Cheap status probe with a last-wins guard (a slow status can't clobber a
  // fresher one, e.g. a build that just finished).
  const refreshStatus = useCallback(() => {
    const reqId = ++statusReqId.current;
    ipc.historyIndexStatus(repoId).then(
      (s) => {
        if (statusReqId.current !== reqId) return;
        setStatus(s);
      },
      (e: unknown) => {
        if (statusReqId.current !== reqId) return;
        pushToast('error', errorMessage(e));
      },
    );
  }, [repoId, pushToast]);

  const openPanel = useCallback(() => {
    setOpen(true);
    refreshStatus();
  }, [refreshStatus]);

  const close = useCallback(() => {
    setOpen(false);
    searchReqId.current += 1; // drop any in-flight retrieval
    setSearching(false);
  }, []);

  // Build/refresh the index; the progress channel drives the bar. Guarded so a
  // second click while building is a no-op. On completion, adopt the returned
  // status (fresh: not stale, 0 new).
  const build = useCallback(() => {
    if (building) return;
    setBuilding(true);
    setProgress(null);
    setError(null);
    ipc.historyIndexBuild(repoId, (p) => setProgress(p)).then(
      (s) => {
        setBuilding(false);
        setProgress(null);
        setStatus(s);
      },
      (e: unknown) => {
        setBuilding(false);
        setProgress(null);
        const msg = errorMessage(e);
        setError(msg);
        pushToast('error', msg);
      },
    );
  }, [building, repoId, pushToast]);

  const setText = useCallback((t: string) => {
    setQuery((q) => ({ ...q, text: t }));
  }, []);

  // Submit-only retrieval: last-wins reqId guard. Empty text clears (no request).
  // On success reveal the top hit (reusing the single-selection reveal path).
  const search = useCallback(() => {
    const text = query.text.trim();
    if (text === '') {
      searchReqId.current += 1;
      setHits([]);
      setSearched(false);
      setSearching(false);
      setError(null);
      return;
    }
    const reqId = ++searchReqId.current;
    setSearching(true);
    setError(null);
    ipc.historySearch(repoId, { text, topK: query.topK }).then(
      (res) => {
        if (searchReqId.current !== reqId) return;
        setHits(res.hits);
        setSearching(false);
        setSearched(true);
        if (res.hits.length > 0) revealCommitByOid(res.hits[0].oid);
      },
      (e: unknown) => {
        if (searchReqId.current !== reqId) return;
        const msg = errorMessage(e);
        setHits([]);
        setSearching(false);
        setSearched(true);
        setError(msg);
        pushToast('error', msg);
      },
    );
  }, [repoId, query.text, query.topK, revealCommitByOid, pushToast]);

  // matchRows: hit oids mapped onto the current layout's rows; empty while closed
  // so the ring pass clears on dismiss. Depends on the `graph` VALUE so the rings
  // re-map to the correct rows when the layout reloads/reorders while hits are
  // shown (mirrors useCommitSearch) — a stable ref in the deps would go stale.
  const matchRows = useMemo(() => {
    if (!open || graph === null) return [];
    const index = new Map<string, number>();
    for (let i = 0; i < graph.nodes.length; i += 1) index.set(graph.nodes[i].id, i);
    const rows: number[] = [];
    for (const h of hits) {
      const row = index.get(h.oid);
      if (row !== undefined) rows.push(row);
    }
    return rows;
  }, [open, hits, graph]);

  const canAsk = aiEligible && status?.built === true;

  const askAi = useCallback(() => {
    const text = query.text.trim();
    if (text === '' || !canAsk) return;
    runAiAnswer(text, query.topK);
  }, [query.text, query.topK, canAsk, runAiAnswer]);

  return {
    open,
    openPanel,
    close,
    openRef,
    status,
    refreshStatus,
    building,
    progress,
    build,
    query,
    setText,
    hits,
    searching,
    error,
    searched,
    search,
    matchRows,
    askAi,
    canAsk,
  };
}
