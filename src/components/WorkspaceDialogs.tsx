import { ConfirmDialog } from './ConfirmDialog';
import { PromptDialog } from './PromptDialog';
import { RebasePlanEditor } from './RebasePlanEditor';
import { TagCreateDialog } from './TagCreateDialog';
import { RemoteEditDialog } from './RemoteEditDialog';
import { WorktreeCreateDialog } from './WorktreeCreateDialog';
import { WorktreeContextDialog } from './WorktreeContextDialog';
import { WhatChangedDialog } from './WhatChangedDialog';
import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './ContextMenu';
import { StaleBranchesDialog } from './StaleBranchesDialog';
import { shortOid, worktreeContainerPreview } from './workspaceUtils';
import type {
  AiDigestRange,
  BranchInfo,
  BranchesSnapshot,
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

  pendingDeleteBranch: string | null;
  setPendingDeleteBranch: (v: string | null) => void;
  handleDeleteBranch(name: string): void;

  pendingDeleteRemote: string | null;
  setPendingDeleteRemote: (v: string | null) => void;
  handleDeleteRemoteTracking(name: string): void;

  pendingDropStash: number | null;
  setPendingDropStash: (v: number | null) => void;
  handleDropStash(index: number): void;

  pendingReset: { oid: string; mode: ResetMode } | null;
  setPendingReset: (v: { oid: string; mode: ResetMode } | null) => void;
  handleResetBranch(oid: string, mode: ResetMode): void;

  pendingDiscard: string[] | null;
  setPendingDiscard: (v: string[] | null) => void;
  handleDiscard(paths: string[]): void;

  pendingHunkDiscard: { path: string; origPath: string | null; hunkIndex: number } | null;
  setPendingHunkDiscard: (v: { path: string; origPath: string | null; hunkIndex: number } | null) => void;
  handleConfirmHunkDiscard(pending: { path: string; origPath: string | null; hunkIndex: number }): void;

  staleCleanupOpen: boolean;
  setStaleCleanupOpen: (v: boolean) => void;
  refetchBranches(): Promise<void>;
  refetchGraph(): Promise<void>;

  pendingCreateBranch: { oid: string } | null;
  setPendingCreateBranch: (v: { oid: string } | null) => void;
  handleCreateBranchHere(oid: string, name: string): void;

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
  handleAddWorktree(branch: string, name: string): Promise<void>;

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
}

/** P3e: the full trailing dialog/modal cluster + graph context menu for a
 *  workspace tab. Purely presentational — every open/pending flag, setter and
 *  handler is threaded in from RepoWorkspace so behavior/DOM are identical. */
export function WorkspaceDialogs({
  repoId,
  mutating,
  opState,
  headBranch,
  branches,
  remotes,
  worktrees,
  abortConfirmOpen,
  setAbortConfirmOpen,
  handleRebaseAbort,
  handleCherrypickAbort,
  handleRevertAbort,
  handleAbortMerge,
  pendingDeleteBranch,
  setPendingDeleteBranch,
  handleDeleteBranch,
  pendingDeleteRemote,
  setPendingDeleteRemote,
  handleDeleteRemoteTracking,
  pendingDropStash,
  setPendingDropStash,
  handleDropStash,
  pendingReset,
  setPendingReset,
  handleResetBranch,
  pendingDiscard,
  setPendingDiscard,
  handleDiscard,
  pendingHunkDiscard,
  setPendingHunkDiscard,
  handleConfirmHunkDiscard,
  staleCleanupOpen,
  setStaleCleanupOpen,
  refetchBranches,
  refetchGraph,
  pendingCreateBranch,
  setPendingCreateBranch,
  handleCreateBranchHere,
  pendingCreateTag,
  setPendingCreateTag,
  handleCreateTag,
  pendingDeleteTag,
  setPendingDeleteTag,
  handleDeleteTag,
  pendingAddRemote,
  setPendingAddRemote,
  handleAddRemote,
  pendingEditUrl,
  setPendingEditUrl,
  handleSetRemoteUrl,
  pendingRenameRemote,
  setPendingRenameRemote,
  handleRenameRemote,
  pendingRemoveRemote,
  setPendingRemoveRemote,
  handleRemoveRemote,
  whatChangedOpen,
  setWhatChangedOpen,
  runDigest,
  newWorktreeOpen,
  setNewWorktreeOpen,
  handleAddWorktree,
  worktreeContextOpen,
  setWorktreeContextOpen,
  pendingWorktreeLock,
  setPendingWorktreeLock,
  handleLockWorktree,
  pendingWorktreeRemove,
  setPendingWorktreeRemove,
  handleRemoveWorktree,
  rebasePlan,
  setRebasePlan,
  rebasePlanError,
  setRebasePlanError,
  handleStartInteractiveRebase,
  menu,
  closeMenu,
}: WorkspaceDialogsProps) {
  return (
    <>
      <ConfirmDialog
        open={abortConfirmOpen}
        title={
          opState.kind === 'rebase'
            ? 'Abort rebase?'
            : opState.kind === 'cherryPick'
              ? 'Abort cherry-pick?'
              : opState.kind === 'revert'
                ? 'Abort revert?'
                : 'Abort merge?'
        }
        confirmLabel={
          opState.kind === 'rebase'
            ? 'Abort rebase'
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
        open={pendingDeleteBranch !== null}
        title="Delete branch"
        confirmLabel="Delete branch"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteBranch;
          setPendingDeleteBranch(null);
          if (name !== null) void handleDeleteBranch(name);
        }}
        onCancel={() => setPendingDeleteBranch(null)}
      >
        <div>Delete branch "<span className="mono">{pendingDeleteBranch ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          The branch is fully merged, but this cannot be undone from Bonsai.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDeleteRemote !== null}
        title="Delete remote-tracking reference"
        confirmLabel="Delete reference"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteRemote;
          setPendingDeleteRemote(null);
          if (name !== null) void handleDeleteRemoteTracking(name);
        }}
        onCancel={() => setPendingDeleteRemote(null)}
      >
        <div>Delete the remote-tracking reference "<span className="mono">{pendingDeleteRemote ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          This removes only Bonsai's local copy of the remote branch. It does NOT delete the branch on
          the server — a future fetch may recreate it.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDropStash !== null}
        title="Drop stash"
        confirmLabel="Drop stash"
        busy={mutating}
        onConfirm={() => {
          const i = pendingDropStash;
          setPendingDropStash(null);
          if (i !== null) void handleDropStash(i);
        }}
        onCancel={() => setPendingDropStash(null)}
      >
        <div>Drop <span className="mono">stash@{`{${pendingDropStash ?? 0}}`}</span>?</div>
        <div className="dialog-body-note">
          This permanently discards the stashed changes and cannot be undone.
        </div>
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

      {/* P25d: B4 stale-branch cleanup. The nested ConfirmDialog inside lists the
          exact names before any delete; onDeleted refetches branches + graph. */}
      <StaleBranchesDialog
        open={staleCleanupOpen}
        onClose={() => setStaleCleanupOpen(false)}
        repoId={repoId}
        onDeleted={() => void Promise.all([refetchBranches(), refetchGraph()])}
      />

      <PromptDialog
        open={pendingCreateBranch !== null}
        title="Create branch here"
        label="Branch name"
        placeholder="feature/my-branch"
        confirmLabel="Create branch"
        busy={mutating}
        validate={(v) => {
          const t = v.trim();
          if (t === '' || t.startsWith('-')) return 'Enter a valid branch name';
          if (branches?.local.some((b) => b.name === t) === true)
            return 'A branch with that name already exists';
          return null;
        }}
        onSubmit={(v) => void handleCreateBranchHere(pendingCreateBranch!.oid, v.trim())}
        onCancel={() => setPendingCreateBranch(null)}
      />

      {/* P22: create tag at the right-clicked commit. */}
      <TagCreateDialog
        open={pendingCreateTag !== null}
        targetOid={pendingCreateTag?.oid ?? ''}
        busy={mutating}
        existingTags={branches?.tags ?? []}
        onSubmit={(name, message) => {
          const oid = pendingCreateTag?.oid ?? null;
          setPendingCreateTag(null);
          if (oid !== null) void handleCreateTag(oid, name, message);
        }}
        onCancel={() => setPendingCreateTag(null)}
      />

      {/* P22: delete tag (local only). */}
      <ConfirmDialog
        open={pendingDeleteTag !== null}
        title="Delete tag"
        confirmLabel="Delete tag"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteTag;
          setPendingDeleteTag(null);
          if (name !== null) void handleDeleteTag(name);
        }}
        onCancel={() => setPendingDeleteTag(null)}
      >
        <div>Delete tag "<span className="mono">{pendingDeleteTag ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          Deletes the local tag only; a tag already pushed to a remote is not removed there.
        </div>
      </ConfirmDialog>

      {/* P22: add a new remote (name + url both editable). */}
      <RemoteEditDialog
        open={pendingAddRemote}
        title="Add remote"
        confirmLabel="Add remote"
        busy={mutating}
        existingNames={remotes.map((r) => r.name)}
        onSubmit={(name, url) => {
          setPendingAddRemote(false);
          void handleAddRemote(name, url);
        }}
        onCancel={() => setPendingAddRemote(false)}
      />

      {/* P22: edit an existing remote's fetch URL (name read-only). */}
      <RemoteEditDialog
        open={pendingEditUrl !== null}
        title="Edit remote URL"
        confirmLabel="Save URL"
        busy={mutating}
        nameReadOnly
        initialName={pendingEditUrl?.name}
        initialUrl={pendingEditUrl?.url}
        existingNames={remotes.map((r) => r.name)}
        onSubmit={(_name, url) => {
          const target = pendingEditUrl;
          setPendingEditUrl(null);
          if (target !== null) void handleSetRemoteUrl(target.name, url);
        }}
        onCancel={() => setPendingEditUrl(null)}
      />

      {/* P22: rename a remote (single-field → reuse PromptDialog). */}
      <PromptDialog
        open={pendingRenameRemote !== null}
        title="Rename remote"
        label="New remote name"
        placeholder="origin"
        initialValue={pendingRenameRemote?.name}
        confirmLabel="Rename"
        busy={mutating}
        validate={(v) => {
          const t = v.trim();
          if (t === '') return 'Enter a remote name';
          if (/\s/.test(t)) return 'Remote name cannot contain whitespace';
          if (t !== pendingRenameRemote?.name && remotes.some((r) => r.name === t))
            return 'A remote with that name already exists';
          return null;
        }}
        onSubmit={(v) => {
          const target = pendingRenameRemote;
          setPendingRenameRemote(null);
          if (target !== null) void handleRenameRemote(target.name, v.trim());
        }}
        onCancel={() => setPendingRenameRemote(null)}
      />

      {/* P22: remove a remote (drops its tracking refs locally). */}
      <ConfirmDialog
        open={pendingRemoveRemote !== null}
        title="Remove remote"
        confirmLabel="Remove remote"
        busy={mutating}
        onConfirm={() => {
          const name = pendingRemoveRemote;
          setPendingRemoveRemote(null);
          if (name !== null) void handleRemoveRemote(name);
        }}
        onCancel={() => setPendingRemoveRemote(null)}
      >
        <div>Remove remote "<span className="mono">{pendingRemoveRemote ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          Removes the remote and its remote-tracking branches from this repo. The server is not
          affected.
        </div>
      </ConfirmDialog>

      {/* P28 §7: "What changed" range picker → runDigest → AiOutputPanel. */}
      <WhatChangedDialog
        open={whatChangedOpen}
        branchNames={[
          ...(branches?.local.map((b) => b.name) ?? []),
          ...(branches?.remote.map((b) => b.name) ?? []),
        ]}
        currentBranch={headBranch?.name ?? null}
        onSubmit={(range, title) => {
          setWhatChangedOpen(false);
          runDigest(range, title);
        }}
        onCancel={() => setWhatChangedOpen(false)}
      />

      {/* P27 §6.5: new worktree — branch picker + derived-path preview. */}
      <WorktreeCreateDialog
        open={newWorktreeOpen}
        busy={mutating}
        localBranches={branches?.local.map((b) => b.name) ?? []}
        usedBranches={worktrees.map((w) => w.branch).filter((b): b is string => b !== null)}
        container={worktreeContainerPreview(worktrees, repoId)}
        onSubmit={handleAddWorktree}
        onCancel={() => setNewWorktreeOpen(false)}
      />

      {/* P31 §7: worktree × AI-context matrix. Activation inside routes through
          the ProfileActivateDialog preview gate; the matrix refetches itself. */}
      <WorktreeContextDialog
        open={worktreeContextOpen}
        repoId={repoId}
        onClose={() => setWorktreeContextOpen(false)}
      />

      {/* P27 §6.4: lock a worktree with an optional reason. */}
      <PromptDialog
        open={pendingWorktreeLock !== null}
        title="Lock worktree"
        label="Reason (optional)"
        placeholder="pinned for QA"
        confirmLabel="Lock"
        busy={mutating}
        onSubmit={(v) => {
          const name = pendingWorktreeLock;
          setPendingWorktreeLock(null);
          if (name !== null) {
            const reason = v.trim();
            void handleLockWorktree(name, reason === '' ? undefined : reason);
          }
        }}
        onCancel={() => setPendingWorktreeLock(null)}
      />

      {/* P27 §6.6: remove a worktree — names the exact directory deleted. */}
      <ConfirmDialog
        open={pendingWorktreeRemove !== null}
        title="Remove worktree"
        confirmLabel="Remove worktree"
        busy={mutating}
        onConfirm={() => {
          const target = pendingWorktreeRemove;
          setPendingWorktreeRemove(null);
          if (target !== null) void handleRemoveWorktree(target.name);
        }}
        onCancel={() => setPendingWorktreeRemove(null)}
      >
        <div>
          Remove worktree "<span className="mono">{pendingWorktreeRemove?.name ?? ''}</span>"?
        </div>
        <div className="dialog-body-note">
          This permanently deletes the directory{' '}
          <span className="mono">{pendingWorktreeRemove?.absPath ?? ''}</span> from disk. Bonsai
          refuses if the worktree has uncommitted changes.
        </div>
      </ConfirmDialog>

      {/* P23b: interactive-rebase plan editor. */}
      <RebasePlanEditor
        open={rebasePlan !== null}
        ontoLabel={rebasePlan?.ontoLabel ?? ''}
        ontoOid={rebasePlan?.ontoOid ?? ''}
        initialTodos={rebasePlan?.initialTodos ?? []}
        summaries={rebasePlan?.summaries ?? {}}
        mutating={mutating}
        error={rebasePlanError}
        onCancel={() => {
          setRebasePlan(null);
          setRebasePlanError(null);
        }}
        onStart={(todos) => {
          if (rebasePlan !== null) {
            void handleStartInteractiveRebase(rebasePlan.ontoOid, rebasePlan.ontoLabel, todos);
          }
        }}
      />

      {menu !== null && (
        <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={closeMenu} />
      )}
    </>
  );
}
