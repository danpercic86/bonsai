import { useState } from 'react';
import { ipc, SUPPORTED_MERGE_METHODS } from '../../ipc';
import type { ForgeKind, MergePrInput, PrDetail, ReviewComment } from '../../ipc';
import { usePushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';
import { ConfirmDialog } from '../ConfirmDialog';
import { closeActionGerund, closeActionLabel, closeActionPast } from '../PrActionsBar';
import { PrDetailView } from '../PrDetailView';
import { PrReviewComments } from '../PrReviewComments';
import { PrMergeDialog } from './PrMergeDialog';

// P83 — PR detail sub-container: owns the merge/close busy + dialog state and
// the two mutating IPC calls, and mounts PrDetailView + the merge/close dialogs.
// Extracted from PrPanel to keep that container focused. On success it hands the
// updated PrDetail back up (so the parent replaces its `detail` + refreshes the
// list); on error it surfaces the message and asks the parent to refetch.

export interface PrDetailContainerProps {
  repoId: string;
  detail: PrDetail;
  kind: ForgeKind;
  host: string;
  comments: ReviewComment[];
  commentsLoading: boolean;
  commentsError: string | null;
  onBack(): void;
  onOpenUrl(url: string): void;
  /** Replace the parent's detail with the updated (merged/closed) PR. */
  onDetailReplaced(detail: PrDetail): void;
  /** Refresh the list behind the detail (state pill changed). */
  onListChanged(): void;
  /** Refetch the PR (error path — nothing changed remotely). */
  onReload(number: number): void;
  /** Route an `authFailed` error to the parent's reauth flow. Returns true when
   *  handled (caller then suppresses the extra toast). */
  onAuthFailed(e: unknown): boolean;
}

export function PrDetailContainer({
  repoId,
  detail,
  kind,
  host,
  comments,
  commentsLoading,
  commentsError,
  onBack,
  onOpenUrl,
  onDetailReplaced,
  onListChanged,
  onReload,
  onAuthFailed,
}: PrDetailContainerProps) {
  const pushToast = usePushToast();
  const [merging, setMerging] = useState(false);
  const [closing, setClosing] = useState(false);
  const [showMergeDialog, setShowMergeDialog] = useState(false);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);

  const { summary } = detail;
  const supportedMethods = SUPPORTED_MERGE_METHODS[kind];
  const actionBusy = merging || closing;
  const closeVerb = closeActionLabel(kind); // Close | Decline | Abandon
  const source = (
    <span className="mono" title={summary.sourceBranch}>
      {summary.sourceBranch}
    </span>
  );
  const target = (
    <span className="mono" title={summary.targetBranch}>
      {summary.targetBranch}
    </span>
  );

  function handleMerge(input: MergePrInput) {
    const number = summary.number;
    setMerging(true);
    void ipc.forgeMergePr(repoId, number, input).then(
      (d) => {
        setMerging(false);
        setShowMergeDialog(false);
        onDetailReplaced(d);
        onListChanged();
        pushToast('success', `Merged pull request #${number}`);
      },
      (e: unknown) => {
        setMerging(false);
        setShowMergeDialog(false);
        if (onAuthFailed(e)) return; // → reauth (no toast, OD-3)
        pushToast('error', `Could not merge pull request #${number}: ${errorMessage(e)}`);
        onReload(number); // refetch — nothing changed remotely
      },
    );
  }

  function handleClose() {
    const number = summary.number;
    setClosing(true);
    void ipc.forgeClosePr(repoId, number).then(
      (d) => {
        setClosing(false);
        setShowCloseConfirm(false);
        onDetailReplaced(d);
        onListChanged();
        pushToast('success', `${closeActionPast(kind)} pull request #${number}`);
      },
      (e: unknown) => {
        setClosing(false);
        setShowCloseConfirm(false);
        if (onAuthFailed(e)) return; // → reauth (no toast, OD-3)
        pushToast(
          'error',
          `Could not ${closeActionLabel(kind).toLowerCase()} pull request #${number}: ${errorMessage(e)}`,
        );
        onReload(number);
      },
    );
  }

  return (
    <>
      <PrDetailView
        detail={detail}
        onBack={onBack}
        onOpenUrl={onOpenUrl}
        kind={kind}
        supportedMethods={supportedMethods}
        busy={actionBusy}
        onMerge={() => setShowMergeDialog(true)}
        onClose={() => setShowCloseConfirm(true)}
      >
        <PrReviewComments comments={comments} loading={commentsLoading} error={commentsError} />
      </PrDetailView>
      <PrMergeDialog
        open={showMergeDialog}
        number={summary.number}
        kind={kind}
        host={host}
        sourceBranch={summary.sourceBranch}
        targetBranch={summary.targetBranch}
        supportedMethods={supportedMethods}
        busy={merging}
        onConfirm={handleMerge}
        onCancel={() => setShowMergeDialog(false)}
      />
      <ConfirmDialog
        open={showCloseConfirm}
        title={`${closeVerb} pull request #${summary.number}?`}
        confirmLabel={closing ? `${closeActionGerund(kind)}…` : `${closeVerb} pull request`}
        busy={closing}
        onConfirm={handleClose}
        onCancel={() => setShowCloseConfirm(false)}
      >
        <p>
          This {closeVerb.toLowerCase()}s {source} → {target} without merging its changes. Nothing
          in your local repository changes.
        </p>
      </ConfirmDialog>
    </>
  );
}
