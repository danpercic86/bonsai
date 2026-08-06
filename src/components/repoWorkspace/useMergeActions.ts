import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { hasUnresolvedMarkers } from '../../utils/conflictRegions';
import type { AiAutonomy, AiResolveProposal, ConflictResolution } from '../../ipc';
import type { DiffSlot } from '../StatusPanel';
import type { BaseActionDeps, Setter } from './types';

/** P3c merge + conflict handling, incl. P13 §8.3 AI conflict resolution. */
export function useMergeActions(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    aiConflictAutonomy: AiAutonomy;
    setAiResolvingPath: Setter<string | null>;
    setDiffSlot: Setter<DiffSlot | null>;
    fileDiffReqId: { current: number };
  },
) {
  const {
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    aiConflictAutonomy,
    setAiResolvingPath,
    setDiffSlot,
    fileDiffReqId,
  } = deps;

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
  async function handleResolveConflictText(path: string, content: string): Promise<void> {
    setMutating(true);
    try {
      await ipc.resolveConflictText(repoId, path, content);
      await refreshAll();
      pushToast('success', `Staged resolution for ${path}`);
    } catch (e) {
      pushToast('error', errorMessage(e));
      throw e;
    } finally {
      setMutating(false);
    }
  }

  // P13 §8.3: AI conflict resolution for one path.
  async function handleAiResolveConflict(path: string) {
    setAiResolvingPath(path);
    let proposal: AiResolveProposal;
    try {
      proposal = await ipc.aiResolveConflict(repoId, path);
    } catch (e) {
      pushToast('error', errorMessage(e));
      setAiResolvingPath(null);
      return;
    }
    // Safety net (P13): never auto-stage a body that still carries conflict
    // markers. The backend resolve_conflict_text trusts its input (git-add
    // model), so a rare markerful model output would otherwise be staged
    // silently in autoResolve. When that happens, fall through to the review
    // editor with a warning instead — the user still resolves it by hand.
    const markerful = hasUnresolvedMarkers(proposal.proposedText);
    if (aiConflictAutonomy === 'autoResolve' && !markerful) {
      setMutating(true);
      try {
        await ipc.resolveConflictText(repoId, path, proposal.proposedText);
        await refreshAll();
        pushToast('success', `Resolved ${path} with AI — review the staged result`);
      } catch (e) {
        pushToast('error', errorMessage(e));
      } finally {
        setMutating(false);
        setAiResolvingPath(null);
      }
      return;
    }
    if (aiConflictAutonomy === 'autoResolve' && markerful) {
      pushToast('error', `AI left unresolved markers in ${path} — opened for review`);
    }
    // proposeReview (or the autoResolve marker fallback): open the proposal in
    // the conflict editor for review/edit.
    // Guard the getConflict await with the shared fileDiffReqId (P13, same
    // recipe as fetchConflictSlot): if the user opens another diff during the
    // fetch, that bumps the id and we bail rather than clobber their slot.
    const id = ++fileDiffReqId.current;
    try {
      const file = await ipc.getConflict(repoId, path);
      if (id !== fileDiffReqId.current) return;
      // Synthesize a ConflictFile carrying the AI's markerless body so the
      // editor shows the proposed result; ours/theirs are kept for split mode.
      const synthesized = { ...file, text: proposal.proposedText };
      setDiffSlot({
        key: `ai-proposal:${path}`,
        state: 'ready',
        diff: null,
        conflict: synthesized,
        error: null,
      });
    } catch (e) {
      if (id !== fileDiffReqId.current) return;
      pushToast('error', errorMessage(e));
    } finally {
      setAiResolvingPath(null);
    }
  }

  async function handleCommitMerge(message: string) {
    setMutating(true);
    try {
      await ipc.commitMerge(repoId, message);
      await refreshAll();
      pushToast('success', 'Merge committed');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
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
    handleAiResolveConflict,
    handleCommitMerge,
    handleAbortMerge,
  };
}
