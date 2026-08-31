import { ConfirmDialog } from '../ConfirmDialog';
import { shortOid } from '../workspaceUtils';
import type { BranchInfo, LineSelection, RepoOpState, ResetMode } from '../../ipc';

/** Modified-vs-new confirm copy for the bulk "Discard all" dialog. Phrases for
 *  the mixed / modified-only / new-only cases so the permanent deletion of new
 *  (untracked) files is always spelled out. */
function discardForceQuestion(
  pending: { modified: number; created: number } | null,
): string {
  const modified = pending?.modified ?? 0;
  const created = pending?.created ?? 0;
  const files = (n: number) => `${n} ${n === 1 ? 'file' : 'files'}`;
  if (created === 0) return `Revert ${files(modified)}?`;
  if (modified === 0) return `Permanently delete ${files(created)}?`;
  return `Revert ${files(modified)} and permanently delete ${files(created)}?`;
}

/** Title + confirm label for the same dialog. A new-files-only set (the per-row
 *  🗑 on an untracked row, or a folder/section holding only new files) is a
 *  deletion, not a discard — say so rather than "Discard all changes". */
function discardForceChrome(
  pending: { modified: number; created: number } | null,
): { title: string; confirmLabel: string } {
  const modified = pending?.modified ?? 0;
  const created = pending?.created ?? 0;
  if (created > 0 && modified === 0) {
    return { title: created === 1 ? 'Delete new file' : 'Delete new files', confirmLabel: 'Delete' };
  }
  return { title: 'Discard all changes', confirmLabel: 'Discard all' };
}

export interface DestructiveDialogsProps {
  mutating: boolean;
  opState: RepoOpState;
  headBranch: BranchInfo | null;

  abortConfirmOpen: boolean;
  setAbortConfirmOpen: (v: boolean) => void;
  handleRebaseAbort(): void;
  handleCherrypickAbort(): void;
  handleRevertAbort(): void;
  handleAbortMerge(): void;
  /** P39b: leave bisect + restore the original branch/worktree (confirm-gated). */
  handleBisectReset(): void;

  pendingReset: { oid: string; mode: ResetMode } | null;
  setPendingReset: (v: { oid: string; mode: ResetMode } | null) => void;
  handleResetBranch(oid: string, mode: ResetMode): void;

  pendingDiscard: string[] | null;
  setPendingDiscard: (v: string[] | null) => void;
  handleDiscard(paths: string[]): void;

  /** Bulk "Discard all": counts drive the modified-vs-new confirm copy;
   *  `untracked` lists the permanently-deleted (new) files by path. */
  pendingDiscardForce: {
    paths: string[];
    modified: number;
    created: number;
    untracked: string[];
  } | null;
  setPendingDiscardForce: (
    v: { paths: string[]; modified: number; created: number; untracked: string[] } | null,
  ) => void;
  handleDiscardForce(paths: string[]): void;

  /** Commit & Push: parked message (branch has no upstream) → confirm set-upstream. */
  pendingCommitPush: string | null;
  handleConfirmCommitPush(): void;
  handleCancelCommitPush(): void;

  /** P37b: force-push-with-lease confirm gate. */
  pendingForcePush: boolean;
  setPendingForcePush: (v: boolean) => void;
  doForcePush(): void;
  /** Drives the confirm button's busy state while the push is in flight. */
  remoteOp: 'fetch' | 'pull' | 'push' | null;

  pendingHunkDiscard: { path: string; origPath: string | null; hunkIndex: number } | null;
  setPendingHunkDiscard: (v: { path: string; origPath: string | null; hunkIndex: number } | null) => void;
  handleConfirmHunkDiscard(pending: { path: string; origPath: string | null; hunkIndex: number }): void;

  pendingLineDiscard: { path: string; origPath: string | null; selection: LineSelection[] } | null;
  setPendingLineDiscard: (v: { path: string; origPath: string | null; selection: LineSelection[] } | null) => void;
  handleConfirmLineDiscard(pending: { path: string; origPath: string | null; selection: LineSelection[] }): void;
}

/** Destructive confirmations: abort in-progress op, reset, discard (file/bulk/
 *  hunk/line), force-push-with-lease, and the commit-&-push set-upstream gate. */
export function DestructiveDialogs({
  mutating,
  opState,
  headBranch,
  abortConfirmOpen,
  setAbortConfirmOpen,
  handleRebaseAbort,
  handleBisectReset,
  handleCherrypickAbort,
  handleRevertAbort,
  handleAbortMerge,
  pendingReset,
  setPendingReset,
  handleResetBranch,
  pendingDiscard,
  setPendingDiscard,
  handleDiscard,
  pendingDiscardForce,
  setPendingDiscardForce,
  handleDiscardForce,
  pendingCommitPush,
  handleConfirmCommitPush,
  handleCancelCommitPush,
  pendingForcePush,
  setPendingForcePush,
  doForcePush,
  remoteOp,
  pendingHunkDiscard,
  setPendingHunkDiscard,
  handleConfirmHunkDiscard,
  pendingLineDiscard,
  setPendingLineDiscard,
  handleConfirmLineDiscard,
}: DestructiveDialogsProps) {
  return (
    <>
      <ConfirmDialog
        open={abortConfirmOpen}
        title={
          opState.kind === 'rebase'
            ? 'Abort rebase?'
            : opState.kind === 'bisect'
              ? 'Reset bisect?'
              : opState.kind === 'cherryPick'
                ? 'Abort cherry-pick?'
                : opState.kind === 'revert'
                  ? 'Abort revert?'
                  : 'Abort merge?'
        }
        confirmLabel={
          opState.kind === 'rebase'
            ? 'Abort rebase'
            : opState.kind === 'bisect'
              ? 'Reset bisect'
              : opState.kind === 'cherryPick'
                ? 'Abort cherry-pick'
                : opState.kind === 'revert'
                  ? 'Abort revert'
                  : 'Abort merge'
        }
        busy={mutating}
        onConfirm={() => {
          const kind = opState.kind;
          setAbortConfirmOpen(false);
          if (kind === 'rebase') {
            void handleRebaseAbort();
          } else if (kind === 'bisect') {
            void handleBisectReset();
          } else if (kind === 'cherryPick') {
            void handleCherrypickAbort();
          } else if (kind === 'revert') {
            void handleRevertAbort();
          } else {
            void handleAbortMerge();
          }
        }}
        onCancel={() => setAbortConfirmOpen(false)}
      >
        {opState.kind === 'rebase' ? (
          <div>
            This restores your branch and working tree to their pre-rebase state. Replayed commits
            and conflict resolutions will be lost.
          </div>
        ) : opState.kind === 'bisect' ? (
          <div>
            This ends the bisect and restores your original branch and working tree. The recorded
            good/bad marks will be discarded.
          </div>
        ) : opState.kind === 'cherryPick' || opState.kind === 'revert' ? (
          <div>
            This resets your branch and working tree to HEAD. The in-progress{' '}
            {opState.kind === 'cherryPick' ? 'cherry-pick' : 'revert'} and any conflict resolutions
            will be lost.
          </div>
        ) : (
          <div>
            This restores the files touched by the merge to their pre-merge state. Conflict
            resolutions will be lost.
          </div>
        )}
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingReset !== null}
        title={pendingReset?.mode === 'hard' ? 'Hard reset' : 'Reset branch'}
        confirmLabel={
          pendingReset === null
            ? 'Reset'
            : `Reset (${pendingReset.mode})`
        }
        busy={mutating}
        onConfirm={() => {
          const p = pendingReset;
          setPendingReset(null);
          if (p !== null) void handleResetBranch(p.oid, p.mode);
        }}
        onCancel={() => setPendingReset(null)}
      >
        <div>
          Move <span className="mono">{headBranch?.name ?? 'HEAD'}</span> to{' '}
          <span className="mono">{shortOid(pendingReset?.oid ?? '')}</span> ({pendingReset?.mode})?
        </div>
        <div className="dialog-body-note">
          Commits after the target are no longer on this branch (recoverable via the reflog).
          {pendingReset?.mode === 'hard' && (
            <> Uncommitted changes in your working tree will be permanently discarded.</>
          )}
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDiscard !== null}
        title="Discard changes"
        confirmLabel="Discard changes"
        busy={mutating}
        onConfirm={() => {
          const paths = pendingDiscard;
          setPendingDiscard(null);
          if (paths !== null) void handleDiscard(paths);
        }}
        onCancel={() => setPendingDiscard(null)}
      >
        <div>Discard changes to {pendingDiscard?.length ?? 0} file(s)?</div>
        <div className="dialog-body-note">
          This permanently reverts them to the last staged/committed version and cannot be undone.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDiscardForce !== null}
        title={discardForceChrome(pendingDiscardForce).title}
        confirmLabel={discardForceChrome(pendingDiscardForce).confirmLabel}
        busy={mutating}
        onConfirm={() => {
          const p = pendingDiscardForce;
          setPendingDiscardForce(null);
          if (p !== null) void handleDiscardForce(p.paths);
        }}
        onCancel={() => setPendingDiscardForce(null)}
      >
        <div>{discardForceQuestion(pendingDiscardForce)}</div>
        {(pendingDiscardForce?.untracked.length ?? 0) > 0 && (
          <>
            <div className="dialog-body-note">Permanently deleted:</div>
            <ul className="confirm-name-list">
              {(pendingDiscardForce?.untracked ?? []).slice(0, 10).map((p) => (
                <li key={p} className="mono">
                  {p}
                </li>
              ))}
              {(pendingDiscardForce?.untracked.length ?? 0) > 10 && (
                <li className="dialog-body-note">
                  +{(pendingDiscardForce?.untracked.length ?? 0) - 10} more
                </li>
              )}
            </ul>
          </>
        )}
        <div className="dialog-body-note">This cannot be undone.</div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingCommitPush !== null}
        title="Set upstream and push?"
        confirmLabel="Commit & Push"
        confirmVariant="primary"
        busy={mutating}
        onConfirm={handleConfirmCommitPush}
        onCancel={handleCancelCommitPush}
      >
        <div>
          Push <strong>{headBranch?.name ?? 'HEAD'}</strong> to origin/
          {headBranch?.name ?? 'HEAD'} and set it as its upstream?
        </div>
        <div className="dialog-body-note">
          The commit is created first, then pushed.
        </div>
      </ConfirmDialog>

      {/* P37b: force-push with lease — names branch + remote, warns it rewrites
          published history, and documents the client-side-lease limitation. */}
      <ConfirmDialog
        open={pendingForcePush}
        title="Force-push with lease?"
        confirmLabel="Force-push"
        busy={remoteOp === 'push'}
        onConfirm={doForcePush}
        onCancel={() => setPendingForcePush(false)}
      >
        <div>
          This rewrites the published history of{' '}
          <span className="mono">{headBranch?.name ?? 'HEAD'}</span> on{' '}
          <span className="mono">{headBranch?.upstream?.split('/')[0] ?? 'origin'}</span>. Continue?
        </div>
        <div className="dialog-body-note">
          Bonsai first checks the remote hasn&apos;t moved since your last fetch and refuses if
          someone else pushed — strictly safer than a plain force-push. Note this is a client-side
          check with a small race window, not the atomic server-side guarantee of{' '}
          <span className="mono">git push --force-with-lease</span>.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingHunkDiscard !== null}
        title="Discard hunk?"
        confirmLabel="Discard hunk"
        busy={mutating}
        onConfirm={() => {
          const pending = pendingHunkDiscard;
          setPendingHunkDiscard(null);
          if (pending !== null) void handleConfirmHunkDiscard(pending);
        }}
        onCancel={() => setPendingHunkDiscard(null)}
      >
        <div>
          Discard this hunk in <span className="mono">{pendingHunkDiscard?.path ?? ''}</span>?
        </div>
        <div className="dialog-body-note">
          The change in this hunk is permanently reverted in your working tree and cannot be
          undone. Staged changes are not affected.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingLineDiscard !== null}
        title={
          pendingLineDiscard !== null && pendingLineDiscard.selection.length === 1
            ? 'Discard line?'
            : 'Discard lines?'
        }
        confirmLabel={
          pendingLineDiscard !== null && pendingLineDiscard.selection.length === 1
            ? 'Discard line'
            : 'Discard lines'
        }
        busy={mutating}
        onConfirm={() => {
          const pending = pendingLineDiscard;
          setPendingLineDiscard(null);
          if (pending !== null) void handleConfirmLineDiscard(pending);
        }}
        onCancel={() => setPendingLineDiscard(null)}
      >
        <div>
          Discard {pendingLineDiscard?.selection.length ?? 0} selected{' '}
          line{(pendingLineDiscard?.selection.length ?? 0) === 1 ? '' : 's'} in{' '}
          <span className="mono">{pendingLineDiscard?.path ?? ''}</span>?
        </div>
        <div className="dialog-body-note">
          The change{(pendingLineDiscard?.selection.length ?? 0) === 1 ? ' is' : 's are'} permanently
          reverted in your working tree and cannot be undone. Staged changes are not affected.
        </div>
      </ConfirmDialog>
    </>
  );
}
