import { ipc } from '../../ipc';
import { errorMessage } from '../../utils/errors';
import { shortOid } from '../workspaceUtils';
import type { GraphLayout, RebaseTodoOp } from '../../ipc';
import type { BaseActionDeps, RebasePlan, Setter } from './types';

/** P3d plain rebase + P23b interactive-rebase plan editor. */
export function useRebaseActions(
  deps: BaseActionDeps & {
    refreshAll: () => Promise<void>;
    graph: GraphLayout | null;
    setRebasePlan: Setter<RebasePlan | null>;
    setRebasePlanError: Setter<string | null>;
  },
) {
  const { repoId, pushToast, setMutating, refreshAll, graph, setRebasePlan, setRebasePlanError } =
    deps;

  async function handleRebaseBranch(onto: string) {
    setMutating(true);
    try {
      const res = await ipc.rebaseBranch(repoId, onto);
      switch (res.kind) {
        case 'upToDate':
          pushToast('info', `Already up to date with ${onto}`);
          break;
        case 'fastForwarded':
          pushToast('success', `Fast-forwarded onto ${onto}`);
          break;
        case 'rebased':
          pushToast('success', `Rebased onto ${onto} (${res.steps} commit(s))`);
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Rebase paused at step ${res.currentStep}/${res.totalSteps}: ` +
              `${res.paths.length} conflict(s) to resolve`,
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

  async function handleRebaseContinue() {
    setMutating(true);
    try {
      const res = await ipc.rebaseContinue(repoId);
      if (res.kind === 'conflicts') {
        pushToast('info', `Rebase paused at step ${res.currentStep}/${res.totalSteps}`);
      } else if (res.kind === 'rebased') {
        pushToast('success', 'Rebase complete');
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRebaseSkip() {
    setMutating(true);
    try {
      const res = await ipc.rebaseSkip(repoId);
      if (res.kind === 'conflicts') {
        pushToast('info', `Rebase paused at step ${res.currentStep}/${res.totalSteps}`);
      } else if (res.kind === 'rebased') {
        pushToast('success', 'Rebase complete');
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRebaseAbort() {
    setMutating(true);
    try {
      await ipc.rebaseAbort(repoId);
      await refreshAll();
      pushToast('success', 'Rebase aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Seed the plan editor: fetch the default todo list (base..HEAD, all `pick`,
  // oldest-first) and build a per-oid summaries map from the loaded graph nodes.
  // On error → toast, no editor.
  async function openRebasePlan(target: { ontoOid: string; ontoLabel: string }) {
    try {
      const initialTodos = await ipc.getInteractivePlan(repoId, target.ontoOid);
      const nodes = graph?.nodes ?? [];
      const summaries: Record<string, string> = {};
      for (const t of initialTodos) {
        summaries[t.oid] = nodes.find((n) => n.id === t.oid)?.summary ?? shortOid(t.oid);
      }
      setRebasePlanError(null);
      setRebasePlan({ ...target, initialTodos, summaries });
    } catch (e) {
      pushToast('error', errorMessage(e));
    }
  }

  // Start the interactive rebase. Success/conflict close the editor; a backend
  // error keeps it open and surfaces the message in-dialog (plus a sticky toast).
  async function handleStartInteractiveRebase(
    ontoOid: string,
    ontoLabel: string,
    todos: RebaseTodoOp[],
  ) {
    setMutating(true);
    try {
      const res = await ipc.startInteractiveRebase(repoId, ontoOid, todos);
      setRebasePlan(null);
      setRebasePlanError(null);
      // Interactive rebase only ever returns `rebased` or `conflicts`
      // (contract §0 #11 — it always rewrites; no up-to-date/fast-forward path).
      if (res.kind === 'rebased') {
        pushToast('success', `Rebased onto ${ontoLabel} (${res.steps} commit(s))`);
        for (const w of res.warnings ?? []) pushToast('info', w);
      } else if (res.kind === 'conflicts') {
        pushToast(
          'info',
          `Rebase paused at step ${res.currentStep}/${res.totalSteps}: ` +
            `${res.paths.length} conflict(s) to resolve`,
        );
      }
      await refreshAll();
    } catch (e) {
      // Keep the editor open so the error is visible in-context (§8.1 scope).
      const msg = errorMessage(e);
      setRebasePlanError(msg);
      pushToast('error', msg);
    } finally {
      setMutating(false);
    }
  }

  return {
    handleRebaseBranch,
    handleRebaseContinue,
    handleRebaseSkip,
    handleRebaseAbort,
    openRebasePlan,
    handleStartInteractiveRebase,
  };
}
