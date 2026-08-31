import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import type { StashScope } from '../../ipc';
import type { RefreshAll } from './refreshScope';
import type { BaseActionDeps, PendingReservedStash, Setter } from './types';

/** Shared copy for a partial restore: the stash is ALWAYS kept, and the files
 *  that did not come back are named (up to 5, then a count) so the user knows
 *  exactly what to recover and from where. */
function unrestoredCopy(index: number, unrestored: string[]): string {
  const shown = unrestored.slice(0, 5).join(', ');
  const more = unrestored.length > 5 ? ` +${unrestored.length - 5} more` : '';
  return (
    `Applied, but ${unrestored.length} new file(s) could not be restored: ${shown}${more}. ` +
    `Nothing was lost — they are kept at stash@{${index}}.`
  );
}

/** Shared copy for a re-apply that failed outright after the ref already moved. */
function notAppliedCopy(index: number, message: string): string {
  return `Your changes could not be re-applied (${message}). Nothing was lost — they are safe at stash@{${index}}.`;
}

/** P9 / P34: scope-aware stash handling. */
export function useStashActions(
  deps: BaseActionDeps & {
    refreshAll: RefreshAll;
    refetchStashes: () => Promise<void>;
    setPendingReservedStash: Setter<PendingReservedStash | null>;
  },
) {
  const { repoId, pushToast, setMutating, refreshAll, refetchStashes, setPendingReservedStash } =
    deps;

  async function handleCreateStash(scope: StashScope) {
    setMutating(true);
    try {
      const res = await ipc.createStash(repoId, null, scope);
      const successCopy =
        scope === 'staged' ? 'Stashed staged changes' : 'Changes stashed';
      pushToast(
        res.created ? 'success' : 'info',
        res.created ? successCopy : 'Nothing to stash — working tree is clean',
      );
      // P88a row 6: narrow the full round to status + graph (pills) + stashes.
      await refreshAll('stash');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // F-A6-B: on the wrong-target guard rejection, refresh the stale list to the
  // current stack (in addition to the error toast) so the next attempt renders
  // fresh oids. Other errors keep today's toast-only behavior.
  function reportStashError(e: unknown) {
    const msg = errorMessage(e);
    pushToast('error', msg);
    if (msg.includes('stash list changed')) void refetchStashes();
  }

  async function handleApplyStash(index: number, skipReserved = false, expectedOid?: string) {
    setMutating(true);
    try {
      const res = await ipc.applyStash(repoId, index, skipReserved, expectedOid);
      switch (res.kind) {
        case 'applied':
          pushToast('success', `Applied stash@{${index}}`);
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Stash applied with ${res.paths.length} conflict(s) to resolve — the stash is kept (stash@{${index}}).`,
          );
          break;
        case 'reservedPaths':
          // Not an error: offer to apply everything except the un-writable paths.
          // Carry `oid` so the retry re-invokes with the SAME wrong-target guard.
          setPendingReservedStash({ index, op: 'apply', paths: res.paths, oid: expectedOid });
          return; // skip refreshAll; nothing changed and the dialog is now up
        case 'appliedSkippingReserved':
          pushToast(
            'success',
            `Applied stash@{${index}} — skipped ${res.skipped.length} file(s) Windows can't restore: ${res.skipped.join(', ')}`,
          );
          break;
        case 'appliedPartially':
          pushToast('info', unrestoredCopy(index, res.unrestored));
          break;
        case 'notApplied':
          pushToast('error', notAppliedCopy(index, res.message));
          break;
      }
      // P88a row 7: apply mutates worktree+index+stash list ⇒ stash scope.
      await refreshAll('stash');
    } catch (e) {
      reportStashError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handlePopStash(index: number, skipReserved = false, expectedOid?: string) {
    setMutating(true);
    try {
      const res = await ipc.popStash(repoId, index, skipReserved, expectedOid);
      switch (res.kind) {
        case 'applied':
          pushToast('success', `Popped stash@{${index}}`);
          break;
        case 'conflicts':
          pushToast(
            'error',
            `Pop hit ${res.paths.length} conflict(s); your changes are still on the stash (stash@{${index}}). ` +
              'Resolve the conflicts, then drop it.',
          );
          break;
        case 'reservedPaths':
          setPendingReservedStash({ index, op: 'pop', paths: res.paths, oid: expectedOid });
          return; // skip refreshAll; nothing changed and the dialog is now up
        case 'appliedSkippingReserved':
          pushToast(
            'success',
            `Applied stash@{${index}} — skipped ${res.skipped.length} file(s) Windows can't restore: ${res.skipped.join(', ')}. ` +
              'The stash was kept because those files could not be restored.',
          );
          break;
        case 'appliedPartially':
          pushToast('info', unrestoredCopy(index, res.unrestored));
          break;
        case 'notApplied':
          pushToast('error', notAppliedCopy(index, res.message));
          break;
      }
      // P88a row 8: pop mutates worktree+index+stash list ⇒ stash scope.
      await refreshAll('stash');
    } catch (e) {
      reportStashError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleDropStash(index: number, expectedOid?: string) {
    // called after ConfirmDialog
    setMutating(true);
    try {
      await ipc.dropStash(repoId, index, expectedOid);
      pushToast('success', `Dropped stash@{${index}}`);
      // P88a row 9: the refs/stash write trips the watcher — route through the
      // echo-armed refreshAll('stash') (one coalesced round) instead of a raw pair.
      await refreshAll('stash');
    } catch (e) {
      reportStashError(e);
    } finally {
      setMutating(false);
    }
  }

  return { handleCreateStash, handleApplyStash, handlePopStash, handleDropStash };
}
