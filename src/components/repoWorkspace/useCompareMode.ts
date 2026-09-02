// P5 §5.3: Compare mode (HEAD → a right-clicked commit) for one open repo — the
// target, its CompareDiff, the loading/error pair, the last-wins req-id guard,
// entering (`handleCompareWithHead`), tearing down (`clearCompare`) and the
// refresh-coexistence refetch. Extracted verbatim from RepoWorkspace so the
// container only wires it. Read-only: nothing here mutates the repository.
import { useCallback, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import { ipc } from '../../ipc';
import type { CompareDiff } from '../../ipc';
import type { ContextMenuState } from '../ContextMenu';
import type { DiffSlot } from '../StatusPanel';
import type { PushToast } from '../../ToastContext';
import { errorMessage, isAppError } from '../../utils/errors';

interface UseCompareModeDeps {
  repoId: string;
  pushToast: PushToast;
  /** Collapses the shared center-pane diff slot (used for `compare:` keys). */
  collapseDiffSlot: () => void;
  diffSlotRef: { current: DiffSlot | null };
  setDiffSlot: Dispatch<SetStateAction<DiffSlot | null>>;
  fileDiffReqId: { current: number };
  setMenu: Dispatch<SetStateAction<ContextMenuState | null>>;
}

export interface UseCompareMode {
  compare: { oid: string } | null;
  compareData: CompareDiff | null;
  compareLoading: boolean;
  compareError: string | null;
  /** Latest compare target, read without widening effect/callback deps. */
  compareRef: { current: { oid: string } | null };
  clearCompare: () => void;
  refetchCompare: () => Promise<void>;
  handleCompareWithHead: (oid: string) => void;
}

export function useCompareMode({
  repoId,
  pushToast,
  collapseDiffSlot,
  diffSlotRef,
  setDiffSlot,
  fileDiffReqId,
  setMenu,
}: UseCompareModeDeps): UseCompareMode {
  // P5 §5.3: Compare right-panel mode (HEAD → right-clicked commit). Mirrors the
  // commitDiff cluster; `compare.oid` is a full oid so it survives refetches.
  const [compare, setCompare] = useState<{ oid: string } | null>(null);
  const [compareData, setCompareData] = useState<CompareDiff | null>(null);
  const [compareLoading, setCompareLoading] = useState(false);
  const [compareError, setCompareError] = useState<string | null>(null);
  const compareReqId = useRef(0);

  // Latest compare target read by refetchCompare without widening
  // effect/callback deps.
  const compareRef = useRef(compare);
  compareRef.current = compare;

  // P5 §5.3: tear down compare mode. Bumps the req-id so any in-flight fetch is
  // ignored, and collapses an open `compare:` overlay.
  const clearCompare = useCallback(() => {
    compareReqId.current += 1;
    setCompare(null);
    setCompareData(null);
    setCompareLoading(false);
    setCompareError(null);
    if (diffSlotRef.current?.key.startsWith('compare:') === true) {
      collapseDiffSlot();
    }
    // P11g-rev §4.7: the compare DiffBrowser is now derived from
    // compare/compareData, so setting compare=null (above) closes it
    // automatically — no explicit browser teardown needed here.
  }, [collapseDiffSlot, diffSlotRef]);

  // P5 §5.3 refresh coexistence: re-fetch the active comparison after a repo
  // change (HEAD may have moved). `compare.oid` is a full oid — no row remap. A
  // `git`-error rejection means the compared commit is gone -> clear + inform.
  const refetchCompare = useCallback(async () => {
    const target = compareRef.current;
    if (target === null) return;
    const id = ++compareReqId.current;
    try {
      const cd = await ipc.compareWithHead(repoId, target.oid);
      if (id !== compareReqId.current) return;
      setCompareData(cd);
      setCompareLoading(false);
      setCompareError(null);
    } catch (e) {
      if (id !== compareReqId.current) return;
      // Only a `git`-kind rejection means the compared commit is genuinely gone
      // (contract above). Transient failures (io/network/other) keep compare
      // mode active and surface via the inline compare error state instead.
      if (isAppError(e) && e.kind === 'git') {
        clearCompare();
        pushToast('info', 'Compared commit is no longer in this repository');
      } else {
        setCompareLoading(false);
        setCompareError(errorMessage(e));
      }
    }
  }, [repoId, clearCompare, pushToast]);

  // P5 §5.3: enter Compare mode (HEAD → the right-clicked commit). Read-only,
  // so it is NOT gated on mutating/opActive. Collapses any open non-compare diff
  // overlay first (its key belongs to another mode).
  function handleCompareWithHead(oid: string) {
    setMenu(null);
    fileDiffReqId.current += 1;
    setDiffSlot(null);
    setCompare({ oid });
    setCompareData(null);
    setCompareLoading(true);
    setCompareError(null);
    const id = ++compareReqId.current;
    ipc.compareWithHead(repoId, oid).then(
      (cd) => {
        if (id !== compareReqId.current) return;
        setCompareData(cd);
        setCompareLoading(false);
      },
      (e: unknown) => {
        if (id !== compareReqId.current) return;
        setCompareError(errorMessage(e));
        setCompareLoading(false);
      },
    );
  }

  return {
    compare,
    compareData,
    compareLoading,
    compareError,
    compareRef,
    clearCompare,
    refetchCompare,
    handleCompareWithHead,
  };
}
