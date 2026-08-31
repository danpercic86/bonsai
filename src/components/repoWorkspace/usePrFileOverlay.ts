// P93 §3/§6: the two handlers that drive a `pr:<baseOid>:<headOid>:<path>` diff
// slot — open one PR changed file in the center overlay, and collapse it when
// its originating list goes away. Extracted from RepoWorkspace (which owns the
// slot + the `prOverlayCtx` side-channel state) so the container keeps only the
// state. Everything is read through refs, so both callbacks are stable — the
// close handler is a dependency of a PrDetailContainer cleanup effect and MUST
// NOT change identity per render.

import { useCallback, useRef, useState } from 'react';

import { ipc } from '../../ipc';
import type { FileDiff } from '../../ipc';
import type { PrFileDiffOpen } from '../prPanel/PrChangesSection';
import type { DiffSlot } from '../StatusPanel';
import { isPrSlotKey, prSlotKey } from './prSlotKey';
import type { PrOverlayCtx, Setter } from './types';

/** P93 §6.1: "the user dismissed THIS PR file's overlay". `token` is monotonic so
 *  a repeat dismissal of the same row still fires the consumer effect. */
export interface PrRestoreFocus {
  path: string;
  token: number;
}

export interface PrFileOverlayDeps {
  repoId: string;
  diffSlotRef: { current: DiffSlot | null };
  diffViewModeRef: { current: 'diff' | 'file' | 'split' };
  intralineRef: { current: boolean };
  prOverlayCtxRef: { current: PrOverlayCtx | null };
  setPrOverlayCtx: Setter<PrOverlayCtx | null>;
  fetchDiffSlot: (key: string, fetcher: () => Promise<FileDiff>) => Promise<void>;
  /** Also clears `prOverlayCtx` (RepoWorkspace wires that in). */
  collapseDiffSlot: () => void;
}

export interface PrFileOverlay {
  handleOpenPrFileDiff: (ctx: PrFileDiffOpen) => void;
  handleClosePrFileDiff: () => void;
  /** §6.1: the ONLY place the focus-restore token is bumped. Wire it to every
   *  user-dismissal affordance of the center overlay (the `×`, the Esc layer,
   *  the error-banner dismiss) IN PLACE OF `collapseDiffSlot`. */
  handleDismissDiffOverlay: () => void;
  /** §6.1: armed ONLY by `handleDismissDiffOverlay` on a `pr:` slot. Pass it
   *  straight down to `PrChangesSection.restoreFocusTo`. */
  prRestoreFocusTo: PrRestoreFocus | null;
}

export function usePrFileOverlay(deps: PrFileOverlayDeps): PrFileOverlay {
  const {
    repoId,
    diffSlotRef,
    diffViewModeRef,
    intralineRef,
    prOverlayCtxRef,
    setPrOverlayCtx,
    fetchDiffSlot,
    collapseDiffSlot,
  } = deps;

  // §6.1. Lives here rather than in RepoWorkspace (which calls this hook, so it
  // still owns the state) purely to keep that container from growing.
  const [prRestoreFocusTo, setRestoreFocusTo] = useState<PrRestoreFocus | null>(null);
  const restoreTokenRef = useRef(0);

  // Mirrors handleToggleWorkdirDiff: clicking the already-open row collapses.
  // Binary headers never reach here (the row is not interactive, §4.3).
  const handleOpenPrFileDiff = useCallback(
    (ctx: PrFileDiffOpen) => {
      const key = prSlotKey(ctx.baseOid, ctx.headOid, ctx.header.path);
      if (diffSlotRef.current?.key === key) {
        collapseDiffSlot();
        return;
      }
      // A newly opened slot invalidates any pending restore (§6.1).
      setRestoreFocusTo(null);
      setPrOverlayCtx({
        prNumber: ctx.prNumber,
        baseOid: ctx.baseOid,
        headOid: ctx.headOid,
        path: ctx.header.path,
        origPath: ctx.header.origPath,
        status: ctx.header.status,
      });
      void fetchDiffSlot(key, () =>
        ipc.forgePrFileDiff(
          repoId,
          ctx.baseOid,
          ctx.headOid,
          ctx.header.path,
          ctx.header.origPath,
          diffViewModeRef.current === 'file',
          intralineRef.current,
        ),
      );
    },
    [
      repoId,
      diffSlotRef,
      diffViewModeRef,
      intralineRef,
      setPrOverlayCtx,
      setRestoreFocusTo,
      fetchDiffSlot,
      collapseDiffSlot,
    ],
  );

  // §6: tab leaves Pull requests / Back to the list / a different PR / a head
  // advance. Prefix-checked so no other slot kind is touched; safe to overfire.
  const handleClosePrFileDiff = useCallback(() => {
    const key = diffSlotRef.current?.key;
    if (key !== undefined && isPrSlotKey(key)) collapseDiffSlot();
  }, [diffSlotRef, collapseDiffSlot]);

  // §6.1: user dismissal of the center overlay. Identical to `collapseDiffSlot`
  // for every non-`pr:` slot; for a `pr:` slot it additionally arms the
  // focus-restore token, capturing the path BEFORE collapse clears the ctx.
  // Deliberately NOT used by slot replacement (C5), PR switch (C2), head advance
  // (C3), tab leave (C1) or repo change (C4) — those must not move focus.
  const handleDismissDiffOverlay = useCallback(() => {
    const key = diffSlotRef.current?.key;
    const path = key !== undefined && isPrSlotKey(key) ? prOverlayCtxRef.current?.path : undefined;
    collapseDiffSlot();
    if (path !== undefined) {
      restoreTokenRef.current += 1;
      setRestoreFocusTo({ path, token: restoreTokenRef.current });
    }
  }, [diffSlotRef, prOverlayCtxRef, setRestoreFocusTo, collapseDiffSlot]);

  return {
    handleOpenPrFileDiff,
    handleClosePrFileDiff,
    handleDismissDiffOverlay,
    prRestoreFocusTo,
  };
}
