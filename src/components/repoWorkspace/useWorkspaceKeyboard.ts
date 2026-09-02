import { useEffect, useRef } from 'react';
import type { GraphLayout } from '../../ipc';
import type { GraphCanvasHandle } from '../../graph/GraphCanvas';
import { foldPillRow, foldRowAt, foldToDisplay } from '../../hooks/useGraphFold';
import type { GraphFoldController } from '../../hooks/useGraphFold';
import type { DiffSlot } from '../StatusPanel';
import type { Setter } from './types';
import { useWorkspaceEscLayer } from './useWorkspaceEscLayer';

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
  // PR-mode DiffBrowser peel layer (mutually exclusive with commit mode).
  prBrowserOpenRef: { readonly current: boolean };
  closePrBrowser: () => void;
  // P54c: the commit composer is a top-level modal — Esc peels it (preview
  // first, then the dialog) before the diff/compare layers; a no-op while
  // applying (op in flight). `composerOpen` also gates graph-nav below.
  composerOpenRef: { current: boolean };
  closeComposer: () => void;
  composerOpen: boolean;
  searchOpenRef: { current: boolean };
  closeSearch: () => void;
  // P57c: the "Ask history" overlay peels just below the P50 search layer.
  historySearchOpenRef: { current: boolean };
  closeHistorySearch: () => void;
  // P50c: the command palette is a top-level modal — Esc peels it first.
  paletteOpenRef: { current: boolean };
  closePalette: () => void;
  // Spec-007: replay overlay. Its own capture handler normally wins; these are
  // the focus-elsewhere backstops so Esc/arrows never mutate the state UNDER
  // the overlay (exact-restore guarantee).
  replayOpenRef: { readonly current: boolean };
  closeReplay: () => void;
  replayOpen: boolean;
  diffSlotRef: { current: DiffSlot | null };
  compareRef: { current: { oid: string } | null };
  setSelectedIndex: Setter<number | null>;
  setCommitBrowserOpen: Setter<boolean>;
  // Shortcuts
  searchOpen: boolean;
  openSearch: () => void;
  // P57c: gates nav/fetch/pull/push while the Ask-history overlay owns keys.
  historySearchOpen: boolean;
  // P50c: Ctrl/Cmd-K toggles the palette; paletteOpen gates graph-nav keys.
  paletteOpen: boolean;
  togglePalette: () => void;
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
  /** Spec-004: fold controller — while its model is active, graph nav runs in
   *  DISPLAY space with the UI-contract §3 pill semantics (land-don't-select,
   *  Enter/Space/→ expand, ← collapse). Absent/identity ⇒ legacy nav verbatim. */
  fold?: GraphFoldController;
  /** P68e §4.4: `Ctrl/Cmd+Shift+A` — expand the AI activity dock and focus the reply
   *  box if a run is blocked, else the log. Bound BEFORE the typing guard on purpose:
   *  Claude's question can arrive while the user is mid-commit-message, and this is
   *  the DELIBERATE way in that never steals the caret by itself. */
  onAiActivity: () => void;
  /** P87b §5: `Ctrl/Cmd+Shift+L` — toggle the git activity dock. Bound BEFORE the
   *  typing guard (the `+Shift+A` precedent) so it works from the commit box. */
  onGitActivity: () => void;
  handleRefresh: () => Promise<void> | void;
  handleFetch: () => Promise<void> | void;
  handlePull: () => Promise<void> | void;
  handlePush: () => Promise<void> | void;
}) {
  const {
    active,
    globalModalOpen,
    composerOpen,
    replayOpenRef,
    replayOpen,
    setSelectedIndex,
    searchOpen,
    openSearch,
    historySearchOpen,
    paletteOpen,
    togglePalette,
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
    fold,
    onAiActivity,
    onGitActivity,
    handleRefresh,
    handleFetch,
    handlePull,
    handlePush,
  } = deps;

  // Esc-layering effect (active tab only; global modals win). Extracted verbatim
  // into its own hook; called FIRST so effect registration order is unchanged.
  useWorkspaceEscLayer(deps);

  // Spec-004 §3: after Enter-expand the next arrow must resume FROM the pill's
  // display index (not teleport back to a far-away selection). A ref — never
  // render-visible state — so it can't paint as an active-but-unselected row.
  const navAnchorRef = useRef<number | null>(null);
  useEffect(() => {
    navAnchorRef.current = null; // any selection change invalidates the anchor
  }, [selectedIndex]);

  // Per-repo shortcut effect (active tab only, §5.1): refresh / fetch / pull /
  // push / graph nav. Global modals + this repo's own dialogs suppress it.
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (globalModalOpen) return;
      const ctrl = e.ctrlKey || e.metaKey;

      if (e.key === 'F5' || (ctrl && e.key.toLowerCase() === 'r')) {
        e.preventDefault();
        // Also suppressed while a confirm dialog is pending — a refresh could
        // invalidate the state the dialog is about to act on.
        const canRefresh =
          !refreshing &&
          !statusLoading &&
          !graphLoading &&
          !mutating &&
          !dialogOpen &&
          !abortConfirmOpen;
        if (canRefresh) void handleRefresh();
        return;
      }

      // P50b (OQ1): Ctrl/Cmd-F opens commit search (preventDefault the webview
      // find). Ctrl+Shift+F stays fetch (handled below). Runs before the typing
      // guard so it works from the commit box too; suppressed under a dialog.
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        // Spec-007: not under the replay overlay — the bar would open invisibly
        // beneath it, steal focus from the trap, and eat the first Esc.
        if (!dialogOpen && !abortConfirmOpen && !composerOpen && !replayOpenRef.current)
          openSearch();
        return;
      }

      // P50c: Ctrl/Cmd-K toggles the command palette (no Shift; distinct from
      // Ctrl/Cmd-F search). Runs before the typing guard so it toggles from the
      // commit box / search bar too; suppressed under a dialog or abort confirm.
      // Not gated on paletteOpen/searchOpen so a second Ctrl/Cmd-K closes it.
      if (ctrl && !e.shiftKey && e.key.toLowerCase() === 'k') {
        e.preventDefault();
        if (!dialogOpen && !abortConfirmOpen && !composerOpen) togglePalette();
        return;
      }

      // P68e §4.4: also before the typing guard (the Ctrl+F / Ctrl+K precedent
      // above), so a user with a half-typed commit message can reach the reply box.
      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'a') {
        e.preventDefault();
        if (!dialogOpen && !abortConfirmOpen) onAiActivity();
        return;
      }

      // P87b §5: Ctrl/Cmd+Shift+L toggles the git activity dock. Free in the map
      // (F/P/U remote ops, R refresh, K palette, F find, A AI dock); before the
      // typing guard so it toggles from the commit box too.
      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'l') {
        e.preventDefault();
        if (!dialogOpen && !abortConfirmOpen) onGitActivity();
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

      // P50b/P50c/P54c/P57c: nav/fetch/pull/push are inert while the search bar,
      // the command palette, the commit composer, or the Ask-history overlay is
      // open (each owns its own keys).
      if (
        dialogOpen ||
        abortConfirmOpen ||
        searchOpen ||
        paletteOpen ||
        composerOpen ||
        historySearchOpen ||
        replayOpen // spec-007: the transport owns arrows/Home/End while up
      )
        return;

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

      // P95 §2.1: another focused widget with its own arrow handling already consumed
      // this key (it called preventDefault without stopPropagation). Do not move the
      // graph selection, and above all do not yank focus out of that widget.
      if (e.defaultPrevented) return;

      // Spec-004 (UI contract §3): while fold is active, graph nav operates on
      // DISPLAY rows. Arrows LAND on fold-pill rows (active-descendant only —
      // the commit selection never changes); Enter/Space/ArrowRight expand the
      // active pill; ArrowLeft collapses the expanded run containing the
      // selection (the restored pill becomes the active row).
      const foldModel = fold !== undefined && fold.model !== null ? fold.model : null;
      if (foldModel !== null && graph !== null && graph.nodes.length > 0) {
        const displayCount = foldModel.displayRowCount;
        const activePill = fold!.activePillStart;
        if (activePill !== null && (e.key === 'Enter' || e.key === ' ' || e.key === 'ArrowRight')) {
          e.preventDefault();
          // Anchor the NEXT arrow at the pill's former display index (== the
          // run's first revealed row post-expand).
          navAnchorRef.current = activePill;
          fold!.toggleSpan(activePill); // a pill row is always collapsed → expand
          return;
        }
        if (e.key === 'ArrowLeft') {
          if (selectedIndex !== null && fold!.collapseRunContaining(selectedIndex)) {
            e.preventDefault();
          }
          return; // no-op on rows outside an expanded run (reserved)
        }
        if (
          e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'PageDown' ||
          e.key === 'PageUp' || e.key === 'Home' || e.key === 'End'
        ) {
          e.preventDefault();
          const activeDisplay = foldPillRow(foldModel, activePill);
          const anchor = navAnchorRef.current;
          navAnchorRef.current = null; // consumed by this nav step
          const cur =
            activeDisplay ??
            (anchor !== null
              ? foldToDisplay(foldModel, anchor)
              : selectedIndex !== null
                ? foldToDisplay(foldModel, selectedIndex)
                : null);
          const lastRow = displayCount - 1;
          let next: number;
          if (cur === null) {
            // Seed anchors (M2 rule, display space; anchor rows are never hidden).
            const headDisplay =
              graph.headIndex !== null ? foldToDisplay(foldModel, graph.headIndex) : 0;
            if (e.key === 'ArrowDown' || e.key === 'PageDown' || e.key === 'Home') next = headDisplay;
            else if (e.key === 'ArrowUp' || e.key === 'End') next = lastRow;
            else next = 0; // PageUp with none → 0
          } else if (e.key === 'Home') next = 0;
          else if (e.key === 'End') next = lastRow;
          else {
            const page = graphRef.current?.getVisibleRowCount() ?? 10;
            const delta =
              e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : e.key === 'PageDown' ? page : -page;
            next = Math.max(0, Math.min(cur + delta, lastRow));
          }
          const at = foldRowAt(foldModel, next);
          if (at.kind === 'fold') {
            fold!.setActivePill(at.span.start); // land, don't select
          } else {
            fold!.setActivePill(null);
            setSelectedIndex(at.row);
            graphRef.current?.focusScroller();
          }
          return;
        }
      }

      // M2 (graph review): the first Arrow/Page/Home/End with no prior selection
      // seeds an anchor so keyboard nav works without a mouse click. Down/PageDown/
      // Home anchor at headIndex (in range) else 0; Up/End anchor at the last row;
      // PageUp anchors at 0 (contract M2, line 43).
      if (graph !== null && graph.nodes.length > 0 && selectedIndex === null) {
        const lastRow = graph.nodes.length - 1;
        const headAnchor =
          graph.headIndex !== null && graph.headIndex >= 0 && graph.headIndex <= lastRow
            ? graph.headIndex
            : 0;
        let seed: number | null = null;
        if (e.key === 'ArrowDown' || e.key === 'PageDown' || e.key === 'Home') seed = headAnchor;
        else if (e.key === 'ArrowUp' || e.key === 'End') seed = lastRow;
        else if (e.key === 'PageUp') seed = 0; // contract M2: PageUp with none → 0
        if (seed !== null) {
          e.preventDefault();
          setSelectedIndex(seed);
          graphRef.current?.focusScroller();
          return;
        }
      }

      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        setSelectedIndex((cur) => {
          if (cur === null) return cur;
          const next = e.key === 'ArrowDown' ? cur + 1 : cur - 1;
          return Math.max(0, Math.min(next, graph.nodes.length - 1));
        });
        graphRef.current?.focusScroller();
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
        graphRef.current?.focusScroller();
        return;
      }

      if (e.key === 'Home' || e.key === 'End') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        setSelectedIndex(e.key === 'Home' ? 0 : graph.nodes.length - 1);
        graphRef.current?.focusScroller();
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
    historySearchOpen,
    paletteOpen,
    togglePalette,
    composerOpen,
    replayOpen,
    onAiActivity,
    onGitActivity,
    selectedIndex,
    graph,
    fold,
  ]);
}
