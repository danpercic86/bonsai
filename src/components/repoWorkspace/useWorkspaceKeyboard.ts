import { useEffect } from 'react';
import type { GraphLayout } from '../../ipc';
import type { GraphCanvasHandle } from '../../graph/GraphCanvas';
import type { DiffSlot } from '../StatusPanel';
import type { Setter } from './types';

/** Per-repo keyboard handling for the active tab: the Esc-layering effect and
 *  the refresh/fetch/pull/push/graph-nav shortcut effect (P5 §5.4 / §5.1). Both
 *  effects are gated on `active` and suppressed while a global modal or one of
 *  this repo's own dialogs is up. Extracted verbatim; the two `useEffect` calls
 *  keep their original relative order. */
export function useWorkspaceKeyboard(deps: {
  active: boolean;
  globalModalOpen: boolean;
  // Esc-layering
  collapseDiffSlot: () => void;
  clearCompare: () => void;
  closeAiPanel: () => void;
  closeBlame: () => void;
  closeHistory: () => void;
  closeReflog: () => void;
  aiPanelOpenRef: { current: boolean };
  blameOpenRef: { current: boolean };
  historyOpenRef: { current: boolean };
  reflogOpenRef: { current: boolean };
  commitBrowserOpenRef: { current: boolean };
  searchOpenRef: { current: boolean };
  closeSearch: () => void;
  diffSlotRef: { current: DiffSlot | null };
  compareRef: { current: { oid: string } | null };
  setSelectedIndex: Setter<number | null>;
  setCommitBrowserOpen: Setter<boolean>;
  // Shortcuts
  searchOpen: boolean;
  openSearch: () => void;
  refreshing: boolean;
  statusLoading: boolean;
  graphLoading: boolean;
  mutating: boolean;
  canPullPush: boolean;
  dialogOpen: boolean;
  abortConfirmOpen: boolean;
  selectedIndex: number | null;
  graph: GraphLayout | null;
  graphRef: { current: GraphCanvasHandle | null };
  handleRefresh: () => Promise<void> | void;
  handleFetch: () => Promise<void> | void;
  handlePull: () => Promise<void> | void;
  handlePush: () => Promise<void> | void;
}) {
  const {
    active,
    globalModalOpen,
    collapseDiffSlot,
    clearCompare,
    closeAiPanel,
    closeBlame,
    closeHistory,
    closeReflog,
    aiPanelOpenRef,
    blameOpenRef,
    historyOpenRef,
    reflogOpenRef,
    commitBrowserOpenRef,
    searchOpenRef,
    closeSearch,
    diffSlotRef,
    compareRef,
    setSelectedIndex,
    setCommitBrowserOpen,
    searchOpen,
    openSearch,
    refreshing,
    statusLoading,
    graphLoading,
    mutating,
    canPullPush,
    dialogOpen,
    abortConfirmOpen,
    selectedIndex,
    graph,
    graphRef,
    handleRefresh,
    handleFetch,
    handlePull,
    handlePush,
  } = deps;

  // Esc-layering effect (active tab only; global modals win). typing guard ->
  // collapse diff overlay -> exit compare -> deselect commit (P5 §5.4).
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (globalModalOpen) return;
      const target = e.target as HTMLElement | null;
      if (target !== null && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT')) return;
      // P11g-rev §4.7: layering, topmost first. The commit-mode DiffBrowser
      // overlay closes first; then the workdir single-file diffSlot; then
      // compare mode (which also closes its auto-open browser); then deselect.
      // P15b: the AI output panel floats above everything — Esc dismisses it first.
      if (aiPanelOpenRef.current) {
        closeAiPanel();
        return;
      }
      // P23d: blame / file-history overlays close before the diff/commit layers.
      // Use the close helpers so the in-flight fetch reqId is invalidated too.
      if (blameOpenRef.current) {
        closeBlame();
        return;
      }
      if (historyOpenRef.current) {
        closeHistory();
        return;
      }
      if (reflogOpenRef.current) {
        closeReflog();
        return;
      }
      if (commitBrowserOpenRef.current) {
        setCommitBrowserOpen(false);
        return;
      }
      // P50b: the commit-search bar sits below the transient overlays and above
      // the diff/compare layers. When its input is focused the bar's own
      // capture-phase Esc already closed it (this branch handles the
      // focus-elsewhere case).
      if (searchOpenRef.current) {
        closeSearch();
        return;
      }
      if (diffSlotRef.current !== null) {
        collapseDiffSlot();
        return;
      }
      if (compareRef.current !== null) {
        clearCompare();
        return;
      }
      setSelectedIndex((cur) => (cur !== null ? null : cur));
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [
    active,
    globalModalOpen,
    collapseDiffSlot,
    clearCompare,
    closeAiPanel,
    closeBlame,
    closeHistory,
    closeReflog,
    closeSearch,
  ]);

  // Per-repo shortcut effect (active tab only, §5.1): refresh / fetch / pull /
  // push / graph nav. Global modals + this repo's own dialogs suppress it.
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (globalModalOpen) return;
      const ctrl = e.ctrlKey || e.metaKey;

      if (e.key === 'F5' || (ctrl && e.key.toLowerCase() === 'r')) {
        e.preventDefault();
        const canRefresh = !refreshing && !statusLoading && !graphLoading && !mutating;
        if (canRefresh) void handleRefresh();
        return;
      }

      // P50b (OQ1): Ctrl/Cmd-F opens commit search (preventDefault the webview
      // find). Ctrl+Shift+F stays fetch (handled below). Runs before the typing
      // guard so it works from the commit box too; suppressed under a dialog.
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        if (!dialogOpen && !abortConfirmOpen) openSearch();
        return;
      }

      const target = e.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable);
      if (typing) return;

      // P50b: nav/fetch/pull/push are inert while the search bar is open (its
      // own input handles Enter/Shift+Enter for next/prev).
      if (dialogOpen || abortConfirmOpen || searchOpen) return;

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        if (!refreshing && !mutating) void handleFetch();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        if (!refreshing && !mutating && canPullPush) void handlePull();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'u') {
        e.preventDefault();
        if (!refreshing && !mutating && canPullPush) void handlePush();
        return;
      }

      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        setSelectedIndex((cur) => {
          if (cur === null) return cur;
          const next = e.key === 'ArrowDown' ? cur + 1 : cur - 1;
          return Math.max(0, Math.min(next, graph.nodes.length - 1));
        });
        return;
      }

      if (e.key === 'PageDown' || e.key === 'PageUp') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        const n = graphRef.current?.getVisibleRowCount() ?? 10;
        setSelectedIndex((cur) => {
          if (cur === null) return cur;
          const next = e.key === 'PageDown' ? cur + n : cur - n;
          return Math.max(0, Math.min(next, graph.nodes.length - 1));
        });
        return;
      }

      if (e.key === 'Home' || e.key === 'End') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        setSelectedIndex(e.key === 'Home' ? 0 : graph.nodes.length - 1);
        return;
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    active,
    globalModalOpen,
    refreshing,
    statusLoading,
    graphLoading,
    mutating,
    canPullPush,
    dialogOpen,
    abortConfirmOpen,
    searchOpen,
    openSearch,
    selectedIndex,
    graph,
  ]);
}
