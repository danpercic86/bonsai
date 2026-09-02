// P55c: the natural-language → safe-git-op pipeline ("Ask Bonsai"). Owns the
// one-line input's open/busy state, the READ-ONLY planner call with its
// last-wins req-id guard, and the ProposedOpDialog's confirm/cancel — including
// the single mutation in the pipeline (safeOpDispatch after the user confirms).
// Extracted verbatim from RepoWorkspace so the container only wires it.
import { useCallback, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { ProposedOperation } from '../../ipc';
import type { PushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';
import { safeOpDispatch } from '../safeOpDispatch';
import type { RefreshScope } from './refreshScope';

interface UseAskBonsaiDeps {
  repoId: string;
  pushToast: PushToast;
  refreshAll: (scope?: RefreshScope) => Promise<void>;
  setMutating: (v: boolean) => void;
}

export interface UseAskBonsai {
  askOpen: boolean;
  askBusy: boolean;
  pendingProposedOp: ProposedOperation | null;
  opDispatching: boolean;
  runPlanOperation: (request: string) => void;
  openAskBonsai: () => void;
  cancelAskBonsai: () => void;
  confirmProposedOp: () => Promise<void>;
  cancelProposedOp: () => void;
}

export function useAskBonsai({
  repoId,
  pushToast,
  refreshAll,
  setMutating,
}: UseAskBonsaiDeps): UseAskBonsai {
  // P55c: NL → safe-git-op. `askOpen` = the one-line natural-language input;
  // `askBusy` gates it while the READ-ONLY planner runs. `pendingProposedOp` is
  // the resolved proposal shown in ProposedOpDialog — NOTHING mutates until its
  // Confirm; `opDispatching` gates that dialog while the confirmed op runs.
  // `planReqId` is a last-wins guard (mirrors aiPanelReqId) so a slow/superseded
  // or cancelled plan reply is dropped.
  const [askOpen, setAskOpen] = useState(false);
  const [askBusy, setAskBusy] = useState(false);
  const [pendingProposedOp, setPendingProposedOp] = useState<ProposedOperation | null>(null);
  const [opDispatching, setOpDispatching] = useState(false);
  const planReqId = useRef(0);

  // P55c: map a natural-language `request` to ONE allowlisted, previewable op via
  // the READ-ONLY planner. Mirrors runAnalyze's last-wins req-id guard so a slow,
  // superseded, or cancelled reply is dropped. On `proposed` → arm the
  // ProposedOpDialog (NOTHING mutates yet); on `unsupported` → a calm info toast;
  // on error (aiUnavailable / aiFailed / …) → the shared error toast. This call
  // is READ-ONLY — it writes nothing and never emits repo-changed.
  const runPlanOperation = useCallback(
    (request: string) => {
      const id = ++planReqId.current;
      setAskBusy(true);
      ipc.aiPlanOperation(repoId, request).then(
        (plan) => {
          if (id !== planReqId.current) return;
          setAskBusy(false);
          setAskOpen(false);
          if (plan.kind === 'proposed') setPendingProposedOp(plan.operation);
          else pushToast('info', plan.reason);
        },
        (e: unknown) => {
          if (id !== planReqId.current) return;
          setAskBusy(false);
          setAskOpen(false);
          pushToast('error', errorMessage(e));
        },
      );
    },
    [repoId, pushToast],
  );

  const openAskBonsai = useCallback(() => {
    // Drop any in-flight/stale plan and clear the input's busy state on open.
    planReqId.current += 1;
    setAskBusy(false);
    setAskOpen(true);
  }, []);
  const cancelAskBonsai = useCallback(() => {
    // Cancel drops any in-flight plan (its reply is ignored by the req-id guard).
    planReqId.current += 1;
    setAskBusy(false);
    setAskOpen(false);
  }, []);

  // P55c: the ONLY mutation in the NL pipeline — runs after the user confirms the
  // ProposedOpDialog. Dispatches the RESOLVED op to its EXISTING typed command
  // (safeOpDispatch, §6), then refreshes; a dispatched-command AppError (e.g.
  // checkoutConflict / unmergedBranch) surfaces in the shared error toast. The
  // op may pause into the existing conflict/autostash flow — no new UI here.
  const confirmProposedOp = useCallback(async () => {
    const operation = pendingProposedOp;
    if (operation === null) return;
    setOpDispatching(true);
    setMutating(true);
    try {
      await safeOpDispatch(ipc, repoId, operation.op);
      await refreshAll();
      pushToast('success', operation.preview.title);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setOpDispatching(false);
      setMutating(false);
      setPendingProposedOp(null);
    }
  }, [pendingProposedOp, repoId, refreshAll, pushToast, setMutating]);

  const cancelProposedOp = useCallback(() => {
    // Ignore cancel while the confirmed op is dispatching (keep the modal up).
    if (!opDispatching) setPendingProposedOp(null);
  }, [opDispatching]);
  return {
    askOpen,
    askBusy,
    pendingProposedOp,
    opDispatching,
    runPlanOperation,
    openAskBonsai,
    cancelAskBonsai,
    confirmProposedOp,
    cancelProposedOp,
  };
}
