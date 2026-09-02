// How the center-pane diff overlay is currently DISPLAYED, as opposed to what it
// shows: the File/Diff/Split view mode, the "Highlight changes" intraline flag,
// and the PR-file context that the `pr:` slot key cannot carry. Each value is
// mirrored into a ref because the stable refetch callbacks read it without
// widening their deps — so toggling never re-creates them. Extracted verbatim
// from RepoWorkspace; the container destructures under the ORIGINAL names, so
// every consumer (usePartialStaging, usePrFileOverlay, the fetchers) is
// unchanged.
import { useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { PrOverlayCtx } from './types';

export type DiffViewMode = 'diff' | 'file' | 'split';

export interface UseDiffOverlayView {
  prOverlayCtx: PrOverlayCtx | null;
  setPrOverlayCtx: Dispatch<SetStateAction<PrOverlayCtx | null>>;
  prOverlayCtxRef: { current: PrOverlayCtx | null };
  diffViewMode: DiffViewMode;
  setDiffViewMode: Dispatch<SetStateAction<DiffViewMode>>;
  diffViewModeRef: { current: DiffViewMode };
  intraline: boolean;
  setIntraline: Dispatch<SetStateAction<boolean>>;
  intralineRef: { current: boolean };
}

export function useDiffOverlayView(): UseDiffOverlayView {
  // P93: the PR file open in the center overlay (the `pr:` key cannot carry its
  // status / rename origin / PR number). Cleared in collapseDiffSlot.
  const [prOverlayCtx, setPrOverlayCtx] = useState<PrOverlayCtx | null>(null);
  const prOverlayCtxRef = useRef(prOverlayCtx); // read by the overlay refetch toggles
  prOverlayCtxRef.current = prOverlayCtx;
  // P17c: File vs Diff view for the center-pane diff overlay. Drives the
  // `fullContext` arg of the primary overlay fetchers; read through a ref by the
  // stable `refetchStatus` callback so toggling never re-creates it.
  const [diffViewMode, setDiffViewMode] = useState<DiffViewMode>('diff');
  const diffViewModeRef = useRef(diffViewMode);
  diffViewModeRef.current = diffViewMode;
  // P61a: "Highlight changes" (word-level intraline emphasis) for the overlay
  // diff. Drives the `intraline` arg of every overlay fetch; read through a ref
  // by the stable refetch callbacks so toggling never re-creates them.
  const [intraline, setIntraline] = useState(false);
  const intralineRef = useRef(intraline);
  intralineRef.current = intraline;

  return {
    prOverlayCtx,
    setPrOverlayCtx,
    prOverlayCtxRef,
    diffViewMode,
    setDiffViewMode,
    diffViewModeRef,
    intraline,
    setIntraline,
    intralineRef,
  };
}
