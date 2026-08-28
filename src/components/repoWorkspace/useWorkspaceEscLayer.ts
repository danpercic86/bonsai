import { useEffect } from 'react';
import type { DiffSlot } from '../StatusPanel';
import type { Setter } from './types';

/** The Esc-layering half of {@link useWorkspaceKeyboard}: the active-tab Escape
 *  effect that peels overlays topmost-first (P5 §5.4). Extracted verbatim into
 *  its own file; called FIRST inside `useWorkspaceKeyboard` so effect
 *  registration order is unchanged. Accepts the same flat `deps` object (a
 *  superset of what it reads), so the container passes it straight through. */
export function useWorkspaceEscLayer(deps: {
  active: boolean;
  globalModalOpen: boolean;
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
  prBrowserOpenRef: { readonly current: boolean };
  closePrBrowser: () => void;
  composerOpenRef: { current: boolean };
  closeComposer: () => void;
  searchOpenRef: { current: boolean };
  closeSearch: () => void;
  historySearchOpenRef: { current: boolean };
  closeHistorySearch: () => void;
  paletteOpenRef: { current: boolean };
  closePalette: () => void;
  replayOpenRef: { readonly current: boolean };
  closeReplay: () => void;
  diffSlotRef: { current: DiffSlot | null };
  compareRef: { current: { oid: string } | null };
  setSelectedIndex: Setter<number | null>;
  setCommitBrowserOpen: Setter<boolean>;
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
    prBrowserOpenRef,
    closePrBrowser,
    composerOpenRef,
    closeComposer,
    searchOpenRef,
    closeSearch,
    historySearchOpenRef,
    closeHistorySearch,
    paletteOpenRef,
    closePalette,
    replayOpenRef,
    closeReplay,
    diffSlotRef,
    compareRef,
    setSelectedIndex,
    setCommitBrowserOpen,
  } = deps;

  // Esc-layering effect (active tab only; global modals win). typing guard ->
  // collapse diff overlay -> exit compare -> deselect commit (P5 §5.4).
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (globalModalOpen) return;
      // P50c: the command palette is a top-level modal — it closes FIRST, before
      // any transient overlay and before the typing bail below. Its own
      // capture-phase Escape normally handles this while its input is focused;
      // this is the focus-elsewhere fallback and keeps it explicit in the peel
      // order (capture + stopImmediatePropagation means both never both fire).
      if (paletteOpenRef.current) {
        closePalette();
        return;
      }
      // P54c: the composer modal peels next (above the typing bail so Esc closes
      // it from its own message textarea / move-file select). It closes any open
      // file preview first, then the whole dialog; `closeComposer` is a no-op
      // while applying so an in-flight create isn't interrupted.
      if (composerOpenRef.current) {
        closeComposer();
        return;
      }
      // Spec-007: the replay overlay peels next (below the true modals above
      // it, above every covered layer — Esc must never touch those).
      if (replayOpenRef.current) {
        closeReplay();
        return;
      }
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
      // PR-mode browser peels with the commit-mode one (mutually exclusive).
      if (prBrowserOpenRef.current) {
        closePrBrowser();
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
      // P57c: the Ask-history overlay peels just below the search bar (its own
      // capture-phase Esc handles the input-focused case; this is the
      // focus-elsewhere fallback).
      if (historySearchOpenRef.current) {
        closeHistorySearch();
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
    closeHistorySearch,
    closePalette,
    closeComposer,
    closeReplay,
    closePrBrowser,
  ]);
}
