import { useCallback, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { ComposeGroup, FileDiff } from '../../ipc';
import type { PushToast } from '../../ToastContext';

/** The editable plan working-copy: the ordered groups plus the "left
 *  uncommitted" bucket. The pure reducers below operate on this shape so they
 *  are unit-testable without React. */
export interface PlanState {
  groups: ComposeGroup[];
  unassigned: string[];
}

/** Move target for a file row: a group index, or the unassigned bucket. */
export type MoveTarget = number | 'unassigned';

// ---------------------------------------------------------------------------
// PURE plan reducers (exported for unit tests). Each returns a fresh value and
// never mutates its input — v1 is file-level, so every reducer preserves the
// "each file in at most one place" invariant.
// ---------------------------------------------------------------------------

/** Replace group `gi`'s message. Out-of-range `gi` is a no-op. */
export function reduceEditMessage(
  groups: ComposeGroup[],
  gi: number,
  message: string,
): ComposeGroup[] {
  return groups.map((g, i) => (i === gi ? { ...g, message } : g));
}

/** Append a new, empty group (no files, no message). */
export function reduceAddGroup(groups: ComposeGroup[]): ComposeGroup[] {
  return [...groups, { files: [], message: '' }];
}

/** Move `path` to `target` (a group index or 'unassigned'). First strips it
 *  from wherever it currently lives, then adds it once at the destination — so
 *  the partition invariant holds even if the source and target are the same. */
export function reduceMoveFile(state: PlanState, path: string, target: MoveTarget): PlanState {
  const groups = state.groups.map((g) => ({ ...g, files: g.files.filter((f) => f !== path) }));
  let unassigned = state.unassigned.filter((f) => f !== path);
  if (target === 'unassigned') {
    unassigned = [...unassigned, path];
  } else if (target >= 0 && target < groups.length) {
    groups[target] = { ...groups[target], files: [...groups[target].files, path] };
  }
  return { groups, unassigned };
}

/** Drop group `gi`; its files fall back to the unassigned bucket. */
export function reduceDropGroup(state: PlanState, gi: number): PlanState {
  if (gi < 0 || gi >= state.groups.length) return state;
  const dropped = state.groups[gi];
  return {
    groups: state.groups.filter((_, i) => i !== gi),
    unassigned: [...state.unassigned, ...dropped.files],
  };
}

/** Merge group `gi` into `targetGi`. The combined group takes the LOWER index
 *  (oldest-first order preserved); its files concatenate lower→higher and its
 *  message joins the two non-empty messages with a blank line (no data loss). */
export function reduceMergeInto(
  groups: ComposeGroup[],
  gi: number,
  targetGi: number,
): ComposeGroup[] {
  if (
    gi === targetGi ||
    gi < 0 ||
    targetGi < 0 ||
    gi >= groups.length ||
    targetGi >= groups.length
  ) {
    return groups;
  }
  const lo = Math.min(gi, targetGi);
  const hi = Math.max(gi, targetGi);
  const merged: ComposeGroup = {
    files: [...groups[lo].files, ...groups[hi].files],
    message: [groups[lo].message, groups[hi].message]
      .map((m) => m.trim())
      .filter((m) => m !== '')
      .join('\n\n'),
  };
  const out: ComposeGroup[] = [];
  groups.forEach((g, i) => {
    if (i === lo) out.push(merged);
    else if (i !== hi) out.push(g);
  });
  return out;
}

/** Apply-able iff there is ≥1 group and EVERY group has a non-empty (trimmed)
 *  message and ≥1 file — the same gate the backend/mock validate (OQ5). */
export function planIsApplicable(groups: ComposeGroup[]): boolean {
  return groups.length > 0 && groups.every((g) => g.message.trim() !== '' && g.files.length > 0);
}

/** One file's HEAD→workdir preview, fetched lazily via the EXISTING workdir
 *  file-diff IPC (no new diff path). `diff` is null while loading / on error. */
export interface ComposerPreview {
  path: string;
  diff: FileDiff | null;
  loading: boolean;
  error: string | null;
}

export interface UseCommitComposer {
  open: boolean;
  openComposer(guidance?: string): void;
  close(): void;
  /** Esc-layering: preview-first, then the whole dialog; a no-op while applying. */
  escClose(): void;
  /** For the workspace Esc-layering (read without a re-subscribe). */
  openRef: { current: boolean };
  loading: boolean;
  error: string | null;
  notes: string[];
  groups: ComposeGroup[];
  unassigned: string[];
  editMessage(gi: number, message: string): void;
  moveFile(path: string, target: MoveTarget): void;
  addGroup(): void;
  dropGroup(gi: number): void;
  mergeInto(gi: number, targetGi: number): void;
  applying: boolean;
  canApply: boolean;
  apply(): Promise<void>;
  // Preview (embedded in the dialog — reuses the workdir file-diff IPC):
  preview: ComposerPreview | null;
  previewFile(path: string): void;
  closePreview(): void;
}

/** P54c: commit-composer state. `openComposer` fires the AI PROPOSE call behind
 *  a last-wins reqId guard (mirrors useCommitSearch); the plan reducers mutate
 *  only local state; `apply` builds a ComposePlan from the groups (the
 *  unassigned bucket is intentionally OMITTED → those files stay uncommitted),
 *  applies it, toasts, refetches graph+status and closes. WRITES NOTHING until
 *  the explicit "Create N commits" confirm. */
export function useCommitComposer(deps: {
  repoId: string;
  refetchStatus(): void;
  refetchGraph(): void;
  pushToast: PushToast;
  /** Reuses RepoWorkspace's existing workdir file-diff IPC for the row Preview. */
  previewFileDiff(path: string): Promise<FileDiff>;
}): UseCommitComposer {
  const { repoId, refetchStatus, refetchGraph, pushToast, previewFileDiff } = deps;

  const [open, setOpen] = useState(false);
  const openRef = useRef(false);
  openRef.current = open;

  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notes, setNotes] = useState<string[]>([]);
  // groups + unassigned kept as ONE state so the reducers that touch both (move,
  // drop) update atomically; exposed separately on the public API.
  const [plan, setPlan] = useState<PlanState>({ groups: [], unassigned: [] });
  const { groups, unassigned } = plan;
  const [applying, setApplying] = useState(false);
  const [preview, setPreview] = useState<ComposerPreview | null>(null);

  const reqIdRef = useRef(0);
  const previewReqRef = useRef(0);
  // Synchronous mirrors so the Esc/close guards read the latest values without
  // waiting for a re-render (a double-click can't slip past the applying gate).
  const applyingRef = useRef(false);
  applyingRef.current = applying;
  const previewRef = useRef<ComposerPreview | null>(null);
  previewRef.current = preview;

  // Reset all editable state + drop any in-flight propose/preview response.
  const resetState = useCallback(() => {
    reqIdRef.current += 1;
    previewReqRef.current += 1;
    setLoading(false);
    setError(null);
    setNotes([]);
    setPlan({ groups: [], unassigned: [] });
    setPreview(null);
  }, []);

  const openComposer = useCallback(
    (guidance?: string) => {
      setOpen(true);
      setLoading(true);
      setError(null);
      setNotes([]);
      setPlan({ groups: [], unassigned: [] });
      setPreview(null);
      previewReqRef.current += 1;
      const reqId = ++reqIdRef.current;
      void (async () => {
        try {
          const proposal = await ipc.aiComposeCommits(repoId, guidance ?? null);
          if (reqIdRef.current !== reqId) return; // stale / closed
          setPlan({
            groups: proposal.groups.map((g) => ({ files: [...g.files], message: g.message })),
            unassigned: [...proposal.unassigned],
          });
          setNotes(proposal.notes);
          setLoading(false);
        } catch (e) {
          if (reqIdRef.current !== reqId) return; // stale / closed
          setError(errorMessage(e));
          setLoading(false);
        }
      })();
    },
    [repoId],
  );

  const close = useCallback(() => {
    if (applyingRef.current) return; // op in flight — ignore
    setOpen(false);
    resetState();
  }, [resetState]);

  const editMessage = useCallback((gi: number, message: string) => {
    setPlan((p) => ({ ...p, groups: reduceEditMessage(p.groups, gi, message) }));
  }, []);

  const moveFile = useCallback((path: string, target: MoveTarget) => {
    setPlan((p) => reduceMoveFile(p, path, target));
  }, []);

  const addGroup = useCallback(() => {
    setPlan((p) => ({ ...p, groups: reduceAddGroup(p.groups) }));
  }, []);

  const dropGroup = useCallback((gi: number) => {
    setPlan((p) => reduceDropGroup(p, gi));
  }, []);

  const mergeInto = useCallback((gi: number, targetGi: number) => {
    setPlan((p) => ({ ...p, groups: reduceMergeInto(p.groups, gi, targetGi) }));
  }, []);

  const canApply = planIsApplicable(groups);

  const apply = useCallback(async () => {
    if (applyingRef.current) return;
    if (!planIsApplicable(groups)) return;
    applyingRef.current = true;
    setApplying(true);
    setError(null);
    try {
      const result = await ipc.applyComposedCommits(repoId, { groups });
      applyingRef.current = false;
      setApplying(false);
      pushToast('success', `Created ${result.commits.length} commit(s)`);
      refetchStatus();
      refetchGraph();
      setOpen(false);
      resetState();
    } catch (e) {
      applyingRef.current = false;
      setApplying(false);
      const msg = errorMessage(e);
      setError(msg);
      pushToast('error', msg);
    }
  }, [repoId, groups, pushToast, refetchStatus, refetchGraph, resetState]);

  const previewFile = useCallback(
    (path: string) => {
      const reqId = ++previewReqRef.current;
      setPreview({ path, diff: null, loading: true, error: null });
      void (async () => {
        try {
          const diff = await previewFileDiff(path);
          if (previewReqRef.current !== reqId) return;
          setPreview({ path, diff, loading: false, error: null });
        } catch (e) {
          if (previewReqRef.current !== reqId) return;
          setPreview({ path, diff: null, loading: false, error: errorMessage(e) });
        }
      })();
    },
    [previewFileDiff],
  );

  const closePreview = useCallback(() => {
    previewReqRef.current += 1;
    setPreview(null);
  }, []);

  const escClose = useCallback(() => {
    if (applyingRef.current) return; // op in flight — swallow Esc, change nothing
    if (previewRef.current !== null) {
      closePreview();
      return;
    }
    close();
  }, [close, closePreview]);

  return {
    open,
    openComposer,
    close,
    escClose,
    openRef,
    loading,
    error,
    notes,
    groups,
    unassigned,
    editMessage,
    moveFile,
    addGroup,
    dropGroup,
    mergeInto,
    applying,
    canApply,
    apply,
    preview,
    previewFile,
    closePreview,
  };
}
