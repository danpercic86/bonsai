import { ConfirmDialog } from '../ConfirmDialog';
import { PromptDialog } from '../PromptDialog';
import { WorktreeCreateDialog } from '../WorktreeCreateDialog';
import { WorktreeContextDialog } from '../WorktreeContextDialog';
import { worktreeContainerPreview } from '../workspaceUtils';
import type { BranchesSnapshot, CopySelection, WorktreeInfo } from '../../ipc';

export interface WorktreeDialogsProps {
  repoId: string;
  mutating: boolean;
  branches: BranchesSnapshot | null;
  worktrees: WorktreeInfo[];

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
}

/** Worktree actions: create, the worktree × AI-context matrix, lock, and
 *  remove-from-disk. */
export function WorktreeDialogs({
  repoId,
  mutating,
  branches,
  worktrees,
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
}: WorktreeDialogsProps) {
  return (
    <>
      {/* P27 §6.5: new worktree — branch picker + derived-path preview. */}
      <WorktreeCreateDialog
        open={newWorktreeOpen}
        busy={mutating}
        repoId={repoId}
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
    </>
  );
}
