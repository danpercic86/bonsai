import { useCallback, useEffect, useRef, useState } from 'react';
import { ipc, SUPPORTED_MERGE_METHODS } from '../../ipc';
import type { FileDiffHeader, ForgeKind, MergePrInput, PrDetail, ReviewComment } from '../../ipc';
import { usePushToast } from '../../ToastContext';
import { errorMessage } from '../../utils/errors';
import { ConfirmDialog } from '../ConfirmDialog';
import { closeActionGerund, closeActionLabel, closeActionPast } from '../PrActionsBar';
import { PrDetailView } from '../PrDetailView';
import { PrReviewComments } from '../PrReviewComments';
import { PrChangesSection } from './PrChangesSection';
import type { PrFileDiffOpen } from './PrChangesSection';
import type { PrRestoreFocus } from '../repoWorkspace/usePrFileOverlay';
import { PrMergeDialog } from './PrMergeDialog';
import { usePrDiff } from './usePrDiff';

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
  /** P93: open ONE changed file's diff in the center overlay. */
  onOpenFileDiff?(ctx: PrFileDiffOpen): void;
  /** P93 §6: collapse the center PR overlay — fired when this detail unmounts
   *  (tab leaves Pull requests / Back to the list), when a different PR is
   *  shown, and on a head advance. Must be stable / safe to overfire. */
  onClosePrFileDiff?(): void;
  /** Path of the PR file currently open in the center overlay (null = none). */
  prOverlayPath?: string | null;
  /** P93 §6.1: dismissal-event focus restore for the changed-files list. */
  prRestoreFocusTo?: PrRestoreFocus | null;
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
  onOpenFileDiff,
  onClosePrFileDiff,
  prOverlayPath = null,
  prRestoreFocusTo = null,
}: PrDetailContainerProps) {
  const pushToast = usePushToast();
  const [merging, setMerging] = useState(false);
  const [closing, setClosing] = useState(false);
  const [showMergeDialog, setShowMergeDialog] = useState(false);
  const [showCloseConfirm, setShowCloseConfirm] = useState(false);

  const { summary } = detail;

  // P89: local base…head diff — auto-fetch on open, keyed by repoId + PR number
  // + head sha (re-open cache / head-advance staleness live in the hook). The
  // per-file hunk fetcher is keyed off the resolved merge-base/head oids.
  const prDiff = usePrDiff(repoId, summary.number, summary.headSha);

  // P93 §6.1/§6.2: unmounting this detail (tab switch / Back) or switching to a
  // different PR orphans any open `pr:` overlay — collapse it. Cleanup-only, so
  // it fires exactly once per detail episode.
  useEffect(() => () => onClosePrFileDiff?.(), [summary.number, onClosePrFileDiff]);

  // P93 §6.3: a head advance re-keys the diff (the open file may not even exist
  // at the new head) — collapse rather than dim. The LIST keeps its `.diff-stale`
  // dim while the new stats load (P89 SF2, unchanged).
  //
  // P96 item 4: on a PR switch the new head oid arrives a commit LATE — the
  // hook only calls `setStats` from its fetch effect, so on the switch commit
  // `stats` still holds the OLD PR's oid (a cached target has the same shape,
  // just shorter latency). Recording that stale oid as the baseline made the
  // next commit's old→new flip read as a head advance and fire a SECOND,
  // redundant close on top of the cleanup above. The switch episode is therefore
  // bracketed explicitly: the number change opens it (baseline cleared, nothing
  // fired — C2 already closed the overlay), and the FIRST stats belonging to the
  // new PR closes it by establishing the baseline. That first arrival is
  // detected by the stats OBJECT IDENTITY, not by the oid — two PRs can share a
  // head sha, in which case `headOid` never changes across the switch and an
  // oid-only guard would leave the baseline stuck at `null` and swallow the next
  // genuine advance. `usePrDiff` hands back a different object whenever stats
  // for the newly-keyed PR land (cache hit or fetch resolve), so identity is the
  // reliable signal. Extra runs outside a switch episode are harmless: they see
  // `prev === headOid` and fire nothing.
  const headOid = prDiff.stats?.headOid ?? null;
  const prevHeadOidRef = useRef(headOid);
  const prevNumberRef = useRef(summary.number);
  const awaitingSwitchRef = useRef(false);
  const statsObj = prDiff.stats;
  useEffect(() => {
    if (prevNumberRef.current !== summary.number) {
      prevNumberRef.current = summary.number;
      prevHeadOidRef.current = null;
      awaitingSwitchRef.current = true;
      return;
    }
    if (awaitingSwitchRef.current) {
      // First stats for the new PR — baseline only, never a close.
      awaitingSwitchRef.current = false;
      prevHeadOidRef.current = headOid;
      return;
    }
    const prev = prevHeadOidRef.current;
    prevHeadOidRef.current = headOid;
    if (prev !== null && headOid !== null && prev !== headOid) onClosePrFileDiff?.();
  }, [headOid, statsObj, summary.number, onClosePrFileDiff]);

  const stats = prDiff.stats;
  const handleOpenFile = useCallback(
    (header: FileDiffHeader) => {
      // No resolved oids ⇒ nothing to diff against; never invoke.
      if (stats === null) return;
      onOpenFileDiff?.({
        prNumber: summary.number,
        baseOid: stats.mergeBaseOid,
        headOid: stats.headOid,
        header,
      });
    },
    [stats, summary.number, onOpenFileDiff],
  );
  // Header counts: use the authoritative local stats once the local diff has
  // resolved — ready OR empty, i.e. even when it's 0/0/0 (SF1: an empty local
  // diff must show +0/-0/0 files, not the forge's stale non-zero counts). Only
  // fall back to the forge-reported counts while still loading or on error.
  const localResolved = prDiff.status === 'ready' || prDiff.status === 'empty';
  const headerStats =
    localResolved && prDiff.stats !== null
      ? prDiff.stats
      : {
          additions: detail.additions,
          deletions: detail.deletions,
          changedFiles: detail.changedFiles,
        };

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
        stats={headerStats}
        changesSlot={
          <PrChangesSection
            status={prDiff.status}
            stats={prDiff.stats}
            stale={prDiff.stale}
            errorCause={prDiff.errorCause}
            onRetry={prDiff.retry}
            activePath={prOverlayPath}
            restoreFocusTo={prRestoreFocusTo}
            onOpenFile={handleOpenFile}
          />
        }
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
