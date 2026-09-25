// P124c — the file-diff slot's state + fetch mechanics, extracted from
// RepoWorkspace. The slot itself must stay in the container (overlay meta,
// staging, compare mode, partial staging, PR overlay and image diff all read
// it); what moves here is HOW a fetch transitions it.
//
// A background (watcher / external / health) round refetches the open workdir
// diff every time. Before P124c each refetch committed twice — a 'loading'
// transition with the stale diff, then a fresh 'ready' object — even when the
// diff was byte-identical. Now a background refetch of the SHOWN key skips the
// 'loading' transition, and a ready result structurally equal to the current
// slot keeps the current object, so an unchanged round commits nothing.
import { useCallback, useRef, useState } from 'react';
import type { Dispatch, MutableRefObject, SetStateAction } from 'react';
import type { DiffSlot } from '../DiffView';
import type { FileDiff } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { structuralEqual } from '../../utils/structuralEqual';

export interface FetchDiffSlotOpts {
  /** Non-user round (watcher / external / health): when the shown slot has the same key,
   *  skip the 'loading' transition and keep it shown until the result lands. */
  background?: boolean;
}

export interface UseDiffSlot {
  diffSlot: DiffSlot | null;
  setDiffSlot: Dispatch<SetStateAction<DiffSlot | null>>;
  diffSlotRef: MutableRefObject<DiffSlot | null>;
  fileDiffReqId: MutableRefObject<number>;
  fetchDiffSlot(key: string, fetcher: () => Promise<FileDiff>, opts?: FetchDiffSlotOpts): Promise<void>;
}

export function useDiffSlot(): UseDiffSlot {
  const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null);
  const fileDiffReqId = useRef(0);
  const diffSlotRef = useRef<DiffSlot | null>(null);
  diffSlotRef.current = diffSlot;

  const fetchDiffSlot = useCallback(
    async (key: string, fetcher: () => Promise<FileDiff>, opts?: FetchDiffSlotOpts) => {
      const id = ++fileDiffReqId.current;
      const prev = diffSlotRef.current;
      const sameKey = prev !== null && prev.key === key;
      const quiet = opts?.background === true && sameKey;
      if (!quiet) {
        // User-initiated (or a different key): show loading, keeping a
        // same-key stale diff visible (dimmed) — P1 §4.1.
        setDiffSlot({ key, state: 'loading', diff: sameKey ? prev.diff : null, error: null });
      }
      try {
        const diff = await fetcher();
        if (id !== fileDiffReqId.current) return;
        const next: DiffSlot = { key, state: 'ready', diff, error: null };
        // Whole-slot compare: a loading→ready still commits; only an identical
        // ready→ready is a no-op. On the QUIET path (no 'loading' dispatched)
        // the ref pre-check skips the setter outright: an updater returning
        // `prev` can still cost one bail-out render + commit (React's
        // eager-state shortcut is off while the fiber carries leftover lanes).
        // NOT on the loading path: a fetcher resolving before React renders
        // the queued 'loading' would still see the old ready slot in the ref
        // and leave the slot stuck in 'loading'. The updater compare keeps
        // both paths race-safe against a not-yet-rendered update.
        if (quiet && structuralEqual(diffSlotRef.current, next)) return;
        setDiffSlot((p) => (structuralEqual(p, next) ? p : next));
      } catch (e) {
        if (id !== fileDiffReqId.current) return;
        setDiffSlot({ key, state: 'error', diff: null, error: errorMessage(e) });
      }
    },
    [],
  );

  return { diffSlot, setDiffSlot, diffSlotRef, fileDiffReqId, fetchDiffSlot };
}
