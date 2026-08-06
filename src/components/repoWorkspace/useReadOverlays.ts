import { useCallback, useEffect, useRef } from 'react';
import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { MAX_HISTORY_UI } from '../workspaceUtils';
import type { GraphLayout } from '../../ipc';
import type { PushToast } from '../../ToastContext';
import type { BlameState, HistoryState, ReflogState, Setter } from './types';

/** P23d + P38: the blame / file-history / reflog center-pane read overlays.
 *  Each holds its own loading/error + a req-id stale-guard; opening one cross-
 *  invalidates the siblings so only one overlay is ever pending/open. The
 *  overlay state itself stays in RepoWorkspace (the render body reads it); this
 *  hook owns the reqId refs, open/close handlers, and the reflog-restore effect. */
export function useReadOverlays(deps: {
  repoId: string;
  pushToast: PushToast;
  mutating: boolean;
  setBlame: Setter<BlameState | null>;
  setHistory: Setter<HistoryState | null>;
  setReflog: Setter<ReflogState | null>;
  blameReqId: { current: number };
  historyReqId: { current: number };
  reflogReqId: { current: number };
  reflogRef: { current: ReflogState | null };
  reflogRestoreRef: { current: boolean };
  graphDataRef: { current: GraphLayout | null };
  compareRef: { current: { oid: string } | null };
  clearCompare: () => void;
  setSelectedIndex: Setter<number | null>;
}) {
  const {
    repoId,
    pushToast,
    mutating,
    setBlame,
    setHistory,
    setReflog,
    blameReqId,
    historyReqId,
    reflogReqId,
    reflogRef,
    reflogRestoreRef,
    graphDataRef,
    compareRef,
    clearCompare,
    setSelectedIndex,
  } = deps;

  // Close helpers bump the matching reqId so a still-in-flight blameFile/
  // fileHistory promise is dropped (its `reqId.current !== reqId` check fails)
  // and the closed overlay can't pop back open.
  const closeBlame = useCallback(() => {
    blameReqId.current += 1;
    setBlame(null);
  }, [blameReqId, setBlame]);
  const closeHistory = useCallback(() => {
    historyReqId.current += 1;
    setHistory(null);
  }, [historyReqId, setHistory]);
  const closeReflog = useCallback(() => {
    reflogReqId.current += 1;
    setReflog(null);
  }, [reflogReqId, setReflog]);

  // P38 §7.2: open the reflog overlay for "HEAD" or a local branch name. Cross-
  // invalidate the sibling blame/history overlays so only one read overlay is
  // ever open, then fetch behind the reflogReqId stale-guard.
  const openReflog = useCallback(
    async (refName: string) => {
      blameReqId.current += 1;
      setBlame(null);
      historyReqId.current += 1;
      setHistory(null);
      const reqId = ++reflogReqId.current;
      setReflog({ refName, entries: [], loading: true, error: null });
      try {
        const entries = await ipc.readReflog(repoId, refName);
        if (reflogReqId.current !== reqId) return;
        setReflog({ refName, entries, loading: false, error: null });
      } catch (e) {
        if (reflogReqId.current !== reqId) return;
        setReflog({ refName, entries: [], loading: false, error: errorMessage(e) });
      }
    },
    [repoId, blameReqId, historyReqId, reflogReqId, setBlame, setHistory, setReflog],
  );

  // Reveal a commit in the graph by oid: reuse the select-by-oid path. Setting
  // `selectedIndex` opens CommitPanel AND triggers GraphCanvas's §6.3 effect,
  // which scrolls the row into the virtualized viewport — so this is select+
  // scroll, no extra graph API needed. Close the blame/history overlay first so
  // the revealed row is actually visible (the overlay covers the graph pane).
  const revealCommitByOid = useCallback(
    (oid: string) => {
      const g = graphDataRef.current;
      if (g === null) return;
      const idx = g.nodes.findIndex((n) => n.id === oid);
      if (idx < 0) {
        pushToast('info', 'Commit not in the current view');
        return;
      }
      if (compareRef.current !== null) clearCompare();
      closeBlame();
      closeHistory();
      closeReflog();
      setSelectedIndex(idx);
    },
    [pushToast, clearCompare, closeBlame, closeHistory, closeReflog, graphDataRef, compareRef, setSelectedIndex],
  );

  // Blame is against the committed HEAD version (atOid=null) in v1. Cross-
  // invalidate the siblings so only one overlay is ever pending/open.
  async function handleBlame(path: string) {
    historyReqId.current += 1;
    setHistory(null);
    reflogReqId.current += 1;
    setReflog(null);
    const reqId = ++blameReqId.current;
    setBlame({ path, lines: [], loading: true, error: null });
    try {
      const lines = await ipc.blameFile(repoId, path, null);
      if (blameReqId.current !== reqId) return;
      setBlame({ path, lines, loading: false, error: null });
    } catch (e) {
      if (blameReqId.current !== reqId) return;
      setBlame({ path, lines: [], loading: false, error: errorMessage(e) });
    }
  }

  async function handleFileHistory(path: string) {
    blameReqId.current += 1;
    setBlame(null);
    reflogReqId.current += 1;
    setReflog(null);
    const reqId = ++historyReqId.current;
    setHistory({ path, entries: [], loading: true, error: null });
    try {
      const entries = await ipc.fileHistory(repoId, path, MAX_HISTORY_UI);
      if (historyReqId.current !== reqId) return;
      setHistory({ path, entries, loading: false, error: null });
    } catch (e) {
      if (historyReqId.current !== reqId) return;
      setHistory({ path, entries: [], loading: false, error: errorMessage(e) });
    }
  }

  // P38 §7.2: after a restore armed from the reflog overlay completes (mutating
  // falls back to false), the reflog is stale (HEAD moved / a branch was created)
  // — re-fetch it so the new "reset: moving to …" entry appears.
  const prevMutatingRef = useRef(mutating);
  useEffect(() => {
    const was = prevMutatingRef.current;
    prevMutatingRef.current = mutating;
    if (was && !mutating && reflogRestoreRef.current) {
      reflogRestoreRef.current = false;
      const open = reflogRef.current;
      if (open !== null) void openReflog(open.refName);
    }
  }, [mutating, openReflog, reflogRestoreRef, reflogRef]);

  return {
    closeBlame,
    closeHistory,
    closeReflog,
    openReflog,
    revealCommitByOid,
    handleBlame,
    handleFileHistory,
  };
}
