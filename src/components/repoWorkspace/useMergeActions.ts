import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { COMMIT_HOOK_CANCELED } from '../commitPushSignal';
import type { ConflictResolution } from '../../ipc';
import type { DiffSlot } from '../StatusPanel';
import type { BaseActionDeps, Setter } from './types';

/** P3c merge + conflict handling.
 *
 *  P68d moved AI conflict resolution OUT of here into `useAiRuns.ts`: the old
 *  `handleAiResolveConflict` awaited the CLI *behind* a `fileDiffReqId` bump, so any
 *  diff opened during the run discarded the finished proposal (user item 5, part b).
 *  What is left is `openAiProposal`, which guards only a fast local `getConflict` —
 *  losing that race costs the editor SLOT, never the proposal. */
export function useMergeActions(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    setDiffSlot: Setter<DiffSlot | null>;
    fileDiffReqId: { current: number };
    /** P59a: wrap the merge commit so a `hookRejected` opens the
     *  HookOutputDialog (+ "Commit anyway" retry) instead of surfacing raw. */
    runWithHookGate: (
      attempt: (skipHooks: boolean) => Promise<void>,
      skipHooks: boolean,
    ) => Promise<void>;
  },
) {
  const { repoId, pushToast, setMutating, refreshAll, setDiffSlot, fileDiffReqId, runWithHookGate } =
    deps;

  async function handleMergeBranch(name: string) {
    setMutating(true);
    try {
      const res = await ipc.mergeBranch(repoId, name);
      switch (res.kind) {
        case 'upToDate':
          pushToast('info', `Already up to date with ${name}`);
          break;
        case 'fastForwarded':
          pushToast(
            'success',
            `Fast-forwarded to ${name}` +
              (res.stashed ? ' (local changes stashed and restored)' : ''),
          );
          break;
        case 'merged':
          pushToast(
            'success',
            `Merged ${name}` + (res.stashed ? ' (local changes stashed and restored)' : ''),
          );
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Merge paused: ${res.paths.length} conflict(s) to resolve` +
              (res.stashed
                ? '. Your local changes are safe on the stash (stash@{0}) — apply them after finishing the merge.'
                : ''),
          );
          break;
        case 'stashPopConflicts':
          pushToast(
            'error',
            `Merge done, but re-applying your stashed changes hit ${res.paths.length} conflict(s). ` +
              'Your changes are still on the stash (stash@{0}); resolve the conflicts, then drop the stash.',
          );
          break;
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleResolveConflict(path: string, resolution: ConflictResolution) {
    setMutating(true);
    try {
      await ipc.resolveConflict(repoId, path, resolution);
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P12 §4.3: stage user-authored resolved text from the ConflictEditor.
  //
  // The single WRITER for a resolved body (D4), which is why the AI store routes
  // `autoResolve` through it rather than calling `resolveConflictText` itself.
  // `successMessage` lets that caller keep the P13 copy ("Resolved <path> with AI —
  // review the staged result") instead of adding a second toast; `null` suppresses the
  // success toast entirely, which is how a bulk AI stage replaces N per-file toasts
  // with one summary (errors still toast — a failure is always per-file news).
  //
  // P68f: `deferRefresh` skips the `refreshAll()` so a caller staging SEVERAL files
  // can do ONE refresh after the loop. Before that flag, an N-file bulk `autoResolve`
  // ran N full refreshes (status + graph + branches …) back to back — a P68d nit that
  // only became visible once bulk existed.
  async function handleResolveConflictText(
    path: string,
    content: string,
    successMessage?: string | null,
    deferRefresh = false,
  ): Promise<void> {
    setMutating(true);
    try {
      await ipc.resolveConflictText(repoId, path, content);
      if (!deferRefresh) await refreshAll();
      if (successMessage !== null) {
        pushToast('success', successMessage ?? `Staged resolution for ${path}`);
      }
    } catch (e) {
      pushToast('error', errorMessage(e));
      throw e;
    } finally {
      setMutating(false);
    }
  }

  /**
   * P68d §5.3: open an ALREADY-COMPUTED AI proposal in the center-pane review
   * editor.
   *
   * The `fileDiffReqId` guard wraps ONLY the fast local `getConflict` — never a CLI
   * call. That is the binding rule from §5.1: the guard protects the diff SLOT, so a
   * superseded open simply returns, leaving the proposal in the run store where the
   * row's `✓ review` affordance can re-open it. Before P68d the same guard wrapped
   * the multi-second `aiResolveConflict` await, which is how a file switch destroyed
   * a finished proposal.
   *
   * Slot key `ai-proposal:<path>` is unchanged, so ConflictEditor/DiffOverlay need
   * no change.
   */
  async function openAiProposal(path: string, proposedText: string): Promise<void> {
    const id = ++fileDiffReqId.current;
    try {
      const file = await ipc.getConflict(repoId, path);
      if (id !== fileDiffReqId.current) return;
      // Synthesize a ConflictFile carrying the proposed body so the editor shows the
      // result; ours/theirs are kept for split mode.
      setDiffSlot({
        key: `ai-proposal:${path}`,
        state: 'ready',
        diff: null,
        conflict: { ...file, text: proposedText },
        error: null,
      });
    } catch (e) {
      if (id !== fileDiffReqId.current) return;
      pushToast('error', errorMessage(e));
    }
  }

  // P59a: git runs the commit hooks around a merge commit too. `sign` is unused
  // (merge mode hides the sign toggle → CommitBox passes null); `skipHooks`
  // comes from the "Skip hooks" checkbox / the dialog's "Commit anyway". A hook
  // rejection is parked by the gate; other errors keep the existing toast path;
  // a dialog cancel rethrows the sentinel so CommitBox keeps the merge message.
  async function handleCommitMerge(
    message: string,
    _sign: boolean | null = null,
    skipHooks = false,
  ) {
    try {
      await runWithHookGate(async (sh) => {
        setMutating(true);
        try {
          const res = await ipc.commitMerge(repoId, message, sh);
          if (res.hookWarning !== null) pushToast('warning', res.hookWarning);
          await refreshAll();
          pushToast('success', 'Merge committed');
        } finally {
          setMutating(false);
        }
      }, skipHooks);
    } catch (e) {
      if (e === COMMIT_HOOK_CANCELED) throw e;
      pushToast('error', errorMessage(e));
    }
  }

  async function handleAbortMerge() {
    setMutating(true);
    try {
      await ipc.abortMerge(repoId);
      await refreshAll();
      pushToast('success', 'Merge aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  return {
    handleMergeBranch,
    handleResolveConflict,
    handleResolveConflictText,
    openAiProposal,
    handleCommitMerge,
    handleAbortMerge,
  };
}
