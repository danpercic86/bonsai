import { BulkAiConfirmDialog } from './dialogs/BulkAiConfirmDialog';
import { DestructiveDialogs } from './dialogs/DestructiveDialogs';
import { HookOutputDialog } from './HookOutputDialog';
import { StashDialogs } from './dialogs/StashDialogs';
import { BranchTagDialogs } from './dialogs/BranchTagDialogs';
import { RemoteDialogs } from './dialogs/RemoteDialogs';
import { WorktreeDialogs } from './dialogs/WorktreeDialogs';
import { CleanupDialogs } from './dialogs/CleanupDialogs';
import type { ContextMenuItem } from './ContextMenu';
import type { BulkAiConfirmState } from './repoWorkspace/useBulkAiResolve';
import type {
  AiDigestRange,
  BranchInfo,
  BranchesSnapshot,
  BranchNameProposal,
  CopySelection,
  LineSelection,
  RebaseTodoOp,
  RemoteInfo,
  RepoOpState,
  ResetMode,
  WorktreeInfo,
} from '../ipc';

export interface WorkspaceDialogsProps {
  repoId: string;
  mutating: boolean;
  opState: RepoOpState;
  headBranch: BranchInfo | null;
  branches: BranchesSnapshot | null;
  remotes: RemoteInfo[];
  worktrees: WorktreeInfo[];

  abortConfirmOpen: boolean;
  setAbortConfirmOpen: (v: boolean) => void;
  handleRebaseAbort(): void;
  handleCherrypickAbort(): void;
  handleRevertAbort(): void;
  handleAbortMerge(): void;
  /** P39b: leave bisect + restore the original branch/worktree (confirm-gated). */
  handleBisectReset(): void;

  pendingDeleteBranch: string | null;
  setPendingDeleteBranch: (v: string | null) => void;
  handleDeleteBranch(name: string): void;

  pendingRebase: { name: string; cur: string } | null;
  setPendingRebase: (v: { name: string; cur: string } | null) => void;
  handleRebaseBranch(name: string): void;

  pendingDeleteRemote: string | null;
  setPendingDeleteRemote: (v: string | null) => void;
  handleDeleteRemoteTracking(name: string): void;

  pendingDropStash: { index: number; oid?: string } | null;
  setPendingDropStash: (v: { index: number; oid?: string } | null) => void;
  handleDropStash(index: number, oid?: string): void;

  pendingReservedStash: { index: number; op: 'apply' | 'pop'; paths: string[]; oid?: string } | null;
  setPendingReservedStash: (
    v: { index: number; op: 'apply' | 'pop'; paths: string[]; oid?: string } | null,
  ) => void;
  handleApplyStashSkipping(index: number, oid?: string): void;
  handlePopStashSkipping(index: number, oid?: string): void;

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

  /** P59a: a git hook blocked the commit/amend/merge — its output to show, or
   *  null when closed. `hookRetrying` drives the "Commit anyway" busy state. */
  pendingHook: string | null;
  hookRetrying: boolean;
  onHookSkipRetry(): void;
  onHookCancel(): void;

  pendingHunkDiscard: { path: string; origPath: string | null; hunkIndex: number } | null;
  setPendingHunkDiscard: (v: { path: string; origPath: string | null; hunkIndex: number } | null) => void;
  handleConfirmHunkDiscard(pending: { path: string; origPath: string | null; hunkIndex: number }): void;

  pendingLineDiscard: { path: string; origPath: string | null; selection: LineSelection[] } | null;
  setPendingLineDiscard: (v: { path: string; origPath: string | null; selection: LineSelection[] } | null) => void;
  handleConfirmLineDiscard(pending: { path: string; origPath: string | null; selection: LineSelection[] }): void;

  staleCleanupOpen: boolean;
  setStaleCleanupOpen: (v: boolean) => void;
  refetchBranches(): Promise<void>;
  refetchGraph(): Promise<void>;

  pendingCreateBranch: { oid: string } | null;
  setPendingCreateBranch: (v: { oid: string } | null) => void;
  handleCreateBranchHere(oid: string, name: string): void;

  /** P60a: rename a local branch (prefilled PromptDialog). */
  pendingRenameBranch: { name: string } | null;
  setPendingRenameBranch: (v: { name: string } | null) => void;
  handleRenameBranch(oldName: string, newName: string): void;
  /** P53c: AI "Suggest name" gate + grounding for the branch-create dialog.
   *  `aiEligible` = installed && enabled && consented; `workingDirty` = the
   *  worktree has any staged/unstaged/untracked change; `suggestBranchName`
   *  is the container-bound IPC call (WRITES NOTHING). */
  aiEligible: boolean;
  workingDirty: boolean;
  suggestBranchName(): Promise<BranchNameProposal>;

  pendingCreateTag: { oid: string } | null;
  setPendingCreateTag: (v: { oid: string } | null) => void;
  handleCreateTag(oid: string, name: string, message: string | null): void;

  pendingDeleteTag: string | null;
  setPendingDeleteTag: (v: string | null) => void;
  handleDeleteTag(name: string): void;

  pendingAddRemote: boolean;
  setPendingAddRemote: (v: boolean) => void;
  handleAddRemote(name: string, url: string): void;

  pendingEditUrl: { name: string; url: string } | null;
  setPendingEditUrl: (v: { name: string; url: string } | null) => void;
  handleSetRemoteUrl(name: string, url: string): void;

  pendingRenameRemote: { name: string } | null;
  setPendingRenameRemote: (v: { name: string } | null) => void;
  handleRenameRemote(name: string, newName: string): void;

  pendingRemoveRemote: string | null;
  setPendingRemoveRemote: (v: string | null) => void;
  handleRemoveRemote(name: string): void;

  whatChangedOpen: boolean;
  setWhatChangedOpen: (v: boolean) => void;
  runDigest(range: AiDigestRange, title: string): void;

  newWorktreeOpen: boolean;
  setNewWorktreeOpen: (v: boolean) => void;
  handleAddWorktree(branch: string, name: string, selections: CopySelection[]): Promise<void>;

  worktreeContextOpen: boolean;
  setWorktreeContextOpen: (v: boolean) => void;

  pendingWorktreeLock: string | null;
  setPendingWorktreeLock: (v: string | null) => void;
  handleLockWorktree(name: string, reason: string | undefined): void;

  pendingWorktreeRemove: { name: string; absPath: string } | null;
  setPendingWorktreeRemove: (v: { name: string; absPath: string } | null) => void;
  handleRemoveWorktree(name: string): void;

  rebasePlan: {
    ontoOid: string;
    ontoLabel: string;
    initialTodos: RebaseTodoOp[];
    summaries: Record<string, string>;
  } | null;
  setRebasePlan: (
    v: {
      ontoOid: string;
      ontoLabel: string;
      initialTodos: RebaseTodoOp[];
      summaries: Record<string, string>;
    } | null,
  ) => void;
  rebasePlanError: string | null;
  setRebasePlanError: (v: string | null) => void;
  handleStartInteractiveRebase(ontoOid: string, ontoLabel: string, todos: RebaseTodoOp[]): void;

  menu: { x: number; y: number; items: ContextMenuItem[] } | null;
  closeMenu(): void;

  /** P68f: the confirm gate in front of "Resolve all with AI" (one run, N files,
   *  real spend). State lives in `useBulkAiResolve`. */
  bulkAiConfirm: BulkAiConfirmState;
}

/** P3e: the full trailing dialog/modal cluster + graph context menu for a
 *  workspace tab. Purely presentational — every open/pending flag, setter and
 *  handler is threaded in from RepoWorkspace so behavior/DOM are identical.
 *  This container only splits the cluster into per-family child components
 *  (`dialogs/*`); it neither owns state nor regroups the flat prop list. */
export function WorkspaceDialogs(props: WorkspaceDialogsProps) {
  return (
    <>
      <HookOutputDialog
        open={props.pendingHook !== null}
        message={props.pendingHook ?? ''}
        busy={props.hookRetrying}
        onSkipRetry={props.onHookSkipRetry}
        onCancel={props.onHookCancel}
      />

      <DestructiveDialogs
        mutating={props.mutating}
        opState={props.opState}
        headBranch={props.headBranch}
        abortConfirmOpen={props.abortConfirmOpen}
        setAbortConfirmOpen={props.setAbortConfirmOpen}
        handleRebaseAbort={props.handleRebaseAbort}
        handleBisectReset={props.handleBisectReset}
        handleCherrypickAbort={props.handleCherrypickAbort}
        handleRevertAbort={props.handleRevertAbort}
        handleAbortMerge={props.handleAbortMerge}
        pendingReset={props.pendingReset}
        setPendingReset={props.setPendingReset}
        handleResetBranch={props.handleResetBranch}
        pendingDiscard={props.pendingDiscard}
        setPendingDiscard={props.setPendingDiscard}
        handleDiscard={props.handleDiscard}
        pendingDiscardForce={props.pendingDiscardForce}
        setPendingDiscardForce={props.setPendingDiscardForce}
        handleDiscardForce={props.handleDiscardForce}
        pendingCommitPush={props.pendingCommitPush}
        handleConfirmCommitPush={props.handleConfirmCommitPush}
        handleCancelCommitPush={props.handleCancelCommitPush}
        pendingForcePush={props.pendingForcePush}
        setPendingForcePush={props.setPendingForcePush}
        doForcePush={props.doForcePush}
        remoteOp={props.remoteOp}
        pendingHunkDiscard={props.pendingHunkDiscard}
        setPendingHunkDiscard={props.setPendingHunkDiscard}
        handleConfirmHunkDiscard={props.handleConfirmHunkDiscard}
        pendingLineDiscard={props.pendingLineDiscard}
        setPendingLineDiscard={props.setPendingLineDiscard}
        handleConfirmLineDiscard={props.handleConfirmLineDiscard}
      />

      <StashDialogs
        mutating={props.mutating}
        pendingDropStash={props.pendingDropStash}
        setPendingDropStash={props.setPendingDropStash}
        handleDropStash={props.handleDropStash}
        pendingReservedStash={props.pendingReservedStash}
        setPendingReservedStash={props.setPendingReservedStash}
        handleApplyStashSkipping={props.handleApplyStashSkipping}
        handlePopStashSkipping={props.handlePopStashSkipping}
      />

      <BranchTagDialogs
        mutating={props.mutating}
        branches={props.branches}
        pendingDeleteBranch={props.pendingDeleteBranch}
        setPendingDeleteBranch={props.setPendingDeleteBranch}
        handleDeleteBranch={props.handleDeleteBranch}
        pendingRebase={props.pendingRebase}
        setPendingRebase={props.setPendingRebase}
        handleRebaseBranch={props.handleRebaseBranch}
        pendingCreateBranch={props.pendingCreateBranch}
        setPendingCreateBranch={props.setPendingCreateBranch}
        handleCreateBranchHere={props.handleCreateBranchHere}
        pendingRenameBranch={props.pendingRenameBranch}
        setPendingRenameBranch={props.setPendingRenameBranch}
        handleRenameBranch={props.handleRenameBranch}
        aiEligible={props.aiEligible}
        workingDirty={props.workingDirty}
        suggestBranchName={props.suggestBranchName}
        pendingCreateTag={props.pendingCreateTag}
        setPendingCreateTag={props.setPendingCreateTag}
        handleCreateTag={props.handleCreateTag}
        pendingDeleteTag={props.pendingDeleteTag}
        setPendingDeleteTag={props.setPendingDeleteTag}
        handleDeleteTag={props.handleDeleteTag}
      />

      <RemoteDialogs
        mutating={props.mutating}
        remotes={props.remotes}
        pendingDeleteRemote={props.pendingDeleteRemote}
        setPendingDeleteRemote={props.setPendingDeleteRemote}
        handleDeleteRemoteTracking={props.handleDeleteRemoteTracking}
        pendingAddRemote={props.pendingAddRemote}
        setPendingAddRemote={props.setPendingAddRemote}
        handleAddRemote={props.handleAddRemote}
        pendingEditUrl={props.pendingEditUrl}
        setPendingEditUrl={props.setPendingEditUrl}
        handleSetRemoteUrl={props.handleSetRemoteUrl}
        pendingRenameRemote={props.pendingRenameRemote}
        setPendingRenameRemote={props.setPendingRenameRemote}
        handleRenameRemote={props.handleRenameRemote}
        pendingRemoveRemote={props.pendingRemoveRemote}
        setPendingRemoveRemote={props.setPendingRemoveRemote}
        handleRemoveRemote={props.handleRemoveRemote}
      />

      <WorktreeDialogs
        repoId={props.repoId}
        mutating={props.mutating}
        branches={props.branches}
        worktrees={props.worktrees}
        newWorktreeOpen={props.newWorktreeOpen}
        setNewWorktreeOpen={props.setNewWorktreeOpen}
        handleAddWorktree={props.handleAddWorktree}
        worktreeContextOpen={props.worktreeContextOpen}
        setWorktreeContextOpen={props.setWorktreeContextOpen}
        pendingWorktreeLock={props.pendingWorktreeLock}
        setPendingWorktreeLock={props.setPendingWorktreeLock}
        handleLockWorktree={props.handleLockWorktree}
        pendingWorktreeRemove={props.pendingWorktreeRemove}
        setPendingWorktreeRemove={props.setPendingWorktreeRemove}
        handleRemoveWorktree={props.handleRemoveWorktree}
      />

      <CleanupDialogs
        repoId={props.repoId}
        mutating={props.mutating}
        headBranch={props.headBranch}
        branches={props.branches}
        staleCleanupOpen={props.staleCleanupOpen}
        setStaleCleanupOpen={props.setStaleCleanupOpen}
        refetchBranches={props.refetchBranches}
        refetchGraph={props.refetchGraph}
        whatChangedOpen={props.whatChangedOpen}
        setWhatChangedOpen={props.setWhatChangedOpen}
        runDigest={props.runDigest}
        rebasePlan={props.rebasePlan}
        setRebasePlan={props.setRebasePlan}
        rebasePlanError={props.rebasePlanError}
        setRebasePlanError={props.setRebasePlanError}
        handleStartInteractiveRebase={props.handleStartInteractiveRebase}
        menu={props.menu}
        closeMenu={props.closeMenu}
      />

      <BulkAiConfirmDialog {...props.bulkAiConfirm} />
    </>
  );
}
