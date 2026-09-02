// P17c/P28/P45: partial (hunk- and line-level) staging, unstaging and discard for
// the file open in the diff overlay, plus the two overlay refetch toggles that
// share the exact same "re-fetch the open workdir slot under the same key" shape
// — File/Diff/Split view mode and "Highlight changes" (intraline).
//
// Extracted verbatim from RepoWorkspace so the container keeps only the state
// these handlers drive. Everything here reads the CURRENT overlay through refs
// (`overlayMetaRef` / `diffSlotRef` / `stageableRef` / `mutatingRef`) rather than
// through render values, which is what keeps each callback stable across the
// per-keystroke re-renders of the diff overlay; the refs and the `useState`
// setters therefore appear in the dependency arrays only as stable identities.
//
// Destructive paths (hunk discard, line discard) NEVER act directly: they arm a
// ConfirmDialog via `setPendingHunkDiscard` / `setPendingLineDiscard`, and the
// container calls the matching `handleConfirm*` when the user confirms.

import { useCallback } from 'react';

import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { LineSelection } from '../../ipc';
import type { DiffOverlayMeta } from '../DiffOverlay';
import type { DiffSlot } from '../StatusPanel';
import type { PendingHunkDiscard, PendingLineDiscard, PrOverlayCtx, Setter } from './types';

export interface PartialStagingDeps {
  repoId: string;
  setMutating: Setter<boolean>;
  /** Guards every mutating path exactly as `mutating` does in the container. */
  mutatingRef: { current: boolean };
  overlayMetaRef: { current: DiffOverlayMeta | null };
  diffSlotRef: { current: DiffSlot | null };
  /** Stage vs unstage direction for the open overlay (null ⇒ not partial-able). */
  stageableRef: { current: null | 'stage' | 'unstage' };
  diffViewModeRef: { current: 'diff' | 'file' | 'split' };
  intralineRef: { current: boolean };
  /** P93: the open `pr:` slot's context (oids + rename origin), needed to refetch
   *  a PR file diff under the same key. null ⇒ no PR slot open. */
  prOverlayCtxRef: { current: PrOverlayCtx | null };
  setDiffViewMode: Setter<'diff' | 'file' | 'split'>;
  setIntraline: Setter<boolean>;
  setPendingHunkDiscard: Setter<PendingHunkDiscard | null>;
  setPendingLineDiscard: Setter<PendingLineDiscard | null>;
  fetchDiffSlot: (key: string, fetcher: () => Promise<import('../../ipc').FileDiff>) => Promise<void>;
  refetchStatus: () => Promise<void>;
  reportStatusError: (message: string) => void;
}

export function usePartialStaging(deps: PartialStagingDeps) {
  const {
    repoId,
    setMutating,
    mutatingRef,
    overlayMetaRef,
    diffSlotRef,
    stageableRef,
    diffViewModeRef,
    intralineRef,
    prOverlayCtxRef,
    setDiffViewMode,
    setIntraline,
    setPendingHunkDiscard,
    setPendingLineDiscard,
    fetchDiffSlot,
    refetchStatus,
    reportStatusError,
  } = deps;

  // P17c: switch File/Diff view. When a workdir file diff is open, re-fetch it
  // with the new `fullContext` (File View = one whole-file hunk); the same key
  // keeps the stale content visible during the swap. Conflict/ai-proposal slots
  // are not FileDiffs (they use getConflict), so they need no refetch.
  const handleSetViewMode = useCallback(
    (m: 'diff' | 'file' | 'split') => {
      setDiffViewMode(m);
      const meta = overlayMetaRef.current;
      const slot = diffSlotRef.current;
      if (slot === null || meta === null) return;
      if (meta.kind === 'pr') {
        // P93 §5.3: a PR file diff refetches base…head under the SAME key; the
        // oids/origPath come from the ctx side-channel (the key alone loses the
        // rename origin).
        const ctx = prOverlayCtxRef.current;
        if (ctx === null) return;
        void fetchDiffSlot(slot.key, () =>
          ipc.forgePrFileDiff(
            repoId,
            ctx.baseOid,
            ctx.headOid,
            ctx.path,
            ctx.origPath,
            m === 'file',
            intralineRef.current,
          ),
        );
        return;
      }
      if (meta.kind === 'staged' || meta.kind === 'unstaged' || meta.kind === 'untracked') {
        const staged = meta.kind === 'staged';
        void fetchDiffSlot(slot.key, () =>
          ipc.getWorkdirFileDiff(
            repoId,
            meta.path,
            meta.origPath,
            staged,
            m === 'file',
            intralineRef.current,
          ),
        );
      }
    },
    [
      repoId,
      fetchDiffSlot,
      setDiffViewMode,
      overlayMetaRef,
      diffSlotRef,
      intralineRef,
      prOverlayCtxRef,
    ],
  );

  // P61a: flip "Highlight changes" and refetch the open workdir slot with the
  // new `intraline` flag (same refetch pattern as handleSetViewMode; the same
  // key keeps stale content visible during the swap). Commit/compare diffs live
  // in DiffBrowser, not the overlay slot, so nothing else refetches here.
  const handleToggleIntraline = useCallback(
    (next: boolean) => {
      setIntraline(next);
      const meta = overlayMetaRef.current;
      const slot = diffSlotRef.current;
      if (slot === null || meta === null) return;
      if (meta.kind === 'pr') {
        // P93 §5.3: same refetch shape as handleSetViewMode, new `intraline`.
        const ctx = prOverlayCtxRef.current;
        if (ctx === null) return;
        void fetchDiffSlot(slot.key, () =>
          ipc.forgePrFileDiff(
            repoId,
            ctx.baseOid,
            ctx.headOid,
            ctx.path,
            ctx.origPath,
            diffViewModeRef.current === 'file',
            next,
          ),
        );
        return;
      }
      if (meta.kind === 'staged' || meta.kind === 'unstaged' || meta.kind === 'untracked') {
        const staged = meta.kind === 'staged';
        void fetchDiffSlot(slot.key, () =>
          ipc.getWorkdirFileDiff(
            repoId,
            meta.path,
            meta.origPath,
            staged,
            diffViewModeRef.current === 'file',
            next,
          ),
        );
      }
    },
    [
      repoId,
      fetchDiffSlot,
      setIntraline,
      overlayMetaRef,
      diffSlotRef,
      diffViewModeRef,
      prOverlayCtxRef,
    ],
  );

  // P17c: stage/unstage exactly `selection` (already Context-dropped) for the
  // file open in the overlay. Direction + path/origPath come from the current
  // stageable/overlay meta. Guarded by the `mutating` flag like handleStage.
  // refetchStatus re-fetches the matching mode-A workdir slot by path in the new
  // snapshot (honoring the current view mode), so no extra slot fetch is needed;
  // a src/main.rs-style file persists in its section (and may now appear in both
  // staged & unstaged). If the entry leaves its section, refetchStatus collapses.
  const handleStageLines = useCallback(
    async (selection: LineSelection[]) => {
      if (selection.length === 0) return; // empty selection -> skip
      if (mutatingRef.current) return;
      const meta = overlayMetaRef.current;
      const dir = stageableRef.current;
      if (meta === null || dir === null) return;
      setMutating(true);
      try {
        if (dir === 'stage') {
          await ipc.stagePartial(repoId, meta.path, meta.origPath, selection);
        } else {
          await ipc.unstagePartial(repoId, meta.path, meta.origPath, selection);
        }
        await refetchStatus();
      } catch (e) {
        reportStatusError(errorMessage(e));
      } finally {
        setMutating(false);
      }
    },
    [repoId, refetchStatus, reportStatusError, mutatingRef, overlayMetaRef, stageableRef, setMutating],
  );

  // P17c: stage/unstage every add/del line of hunk `hunkIndex` from the open
  // diff (Diff View hunk-header button). Builds the selection then delegates.
  const handleStageHunk = useCallback(
    (hunkIndex: number) => {
      const d = diffSlotRef.current?.diff ?? null;
      const hunk = d?.hunks[hunkIndex];
      if (hunk === undefined) return;
      const selection: LineSelection[] = hunk.lines
        .filter((l) => l.kind === 'add' || l.kind === 'del')
        .map((l) => ({ kind: l.kind, oldNo: l.oldNo, newNo: l.newNo }));
      void handleStageLines(selection);
    },
    [handleStageLines, diffSlotRef],
  );

  // P28: request a hunk discard — just arms the ConfirmDialog (destructive ops
  // always confirm first). Passed to DiffOverlay only for unstaged tracked
  // diffs (see the render-site gating), so meta here is the unstaged file.
  const handleDiscardHunk = useCallback((hunkIndex: number) => {
    const meta = overlayMetaRef.current;
    if (meta === null) return;
    setPendingHunkDiscard({ path: meta.path, origPath: meta.origPath, hunkIndex });
  }, [overlayMetaRef, setPendingHunkDiscard]);

  // P28: confirmed hunk discard — build the LineSelection from the open diff's
  // hunk (same rule as handleStageHunk) and revert it in the worktree, then
  // refetch like handleStageLines does. Guarded by `mutating`.
  const handleConfirmHunkDiscard = useCallback(
    async (pending: { path: string; origPath: string | null; hunkIndex: number }) => {
      if (mutatingRef.current) return;
      // The slot must still show the file the dialog was armed for.
      if (overlayMetaRef.current?.path !== pending.path) return;
      const d = diffSlotRef.current?.diff ?? null;
      const hunk = d?.hunks[pending.hunkIndex];
      if (hunk === undefined) return; // stale click; diff changed underneath
      const selection: LineSelection[] = hunk.lines
        .filter((l) => l.kind === 'add' || l.kind === 'del')
        .map((l) => ({ kind: l.kind, oldNo: l.oldNo, newNo: l.newNo }));
      if (selection.length === 0) return;
      setMutating(true);
      try {
        await ipc.discardPartial(repoId, pending.path, pending.origPath, selection);
        await refetchStatus();
      } catch (e) {
        reportStatusError(errorMessage(e));
      } finally {
        setMutating(false);
      }
    },
    [repoId, refetchStatus, reportStatusError, mutatingRef, overlayMetaRef, diffSlotRef, setMutating],
  );

  // P45: request a per-line discard — just arms the ConfirmDialog (destructive
  // ops always confirm first). The selection is captured verbatim because
  // arbitrary lines can't be re-derived after the diff refetches (unlike a hunk
  // index). Passed to DiffOverlay only for unstaged tracked diffs (see gating).
  const handleDiscardLines = useCallback((selection: LineSelection[]) => {
    if (selection.length === 0) return;
    const meta = overlayMetaRef.current;
    if (meta === null) return;
    setPendingLineDiscard({ path: meta.path, origPath: meta.origPath, selection });
  }, [overlayMetaRef, setPendingLineDiscard]);

  // P45: confirmed per-line discard — revert exactly the stored selection in the
  // worktree, then refetch like handleConfirmHunkDiscard. Guarded by `mutating`;
  // the backend's stale() guard rejects a selection whose coordinates moved.
  const handleConfirmLineDiscard = useCallback(
    async (pending: { path: string; origPath: string | null; selection: LineSelection[] }) => {
      if (mutatingRef.current) return;
      // The slot must still show the file the dialog was armed for.
      if (overlayMetaRef.current?.path !== pending.path) return;
      if (pending.selection.length === 0) return;
      setMutating(true);
      try {
        await ipc.discardPartial(repoId, pending.path, pending.origPath, pending.selection);
        await refetchStatus();
      } catch (e) {
        reportStatusError(errorMessage(e));
      } finally {
        setMutating(false);
      }
    },
    [repoId, refetchStatus, reportStatusError, mutatingRef, overlayMetaRef, setMutating],
  );

  return {
    handleSetViewMode,
    handleToggleIntraline,
    handleStageLines,
    handleStageHunk,
    handleDiscardHunk,
    handleConfirmHunkDiscard,
    handleDiscardLines,
    handleConfirmLineDiscard,
  };
}
