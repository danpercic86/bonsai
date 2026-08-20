/** T3.3a — workspace dialog wrappers, destructive set: DestructiveDialogs,
 *  StashDialogs, UndoDialog, NonFfPullDialog. Representative cases per dialog:
 *  correct copy, confirm fires the handler exactly once (and clears the pending
 *  state), cancel/Esc never calls it, Enter never auto-confirms destructive. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { DestructiveDialogs, type DestructiveDialogsProps } from './DestructiveDialogs';
import { StashDialogs, type StashDialogsProps } from './StashDialogs';
import { UndoDialog } from './UndoDialog';
import { NonFfPullDialog } from './NonFfPullDialog';
import type { BranchInfo, UndoPlan } from '../../ipc';

const headBranch: BranchInfo = {
  name: 'main',
  isHead: true,
  upstream: 'origin/main',
  ahead: 0,
  behind: 0,
  tip: 'a'.repeat(40),
};

function destructiveProps(over: Partial<DestructiveDialogsProps> = {}): DestructiveDialogsProps {
  return {
    mutating: false,
    opState: { kind: 'none' },
    headBranch,
    abortConfirmOpen: false,
    setAbortConfirmOpen: vi.fn(),
    handleRebaseAbort: vi.fn(),
    handleCherrypickAbort: vi.fn(),
    handleRevertAbort: vi.fn(),
    handleAbortMerge: vi.fn(),
    handleBisectReset: vi.fn(),
    pendingReset: null,
    setPendingReset: vi.fn(),
    handleResetBranch: vi.fn(),
    pendingDiscard: null,
    setPendingDiscard: vi.fn(),
    handleDiscard: vi.fn(),
    pendingDiscardForce: null,
    setPendingDiscardForce: vi.fn(),
    handleDiscardForce: vi.fn(),
    pendingCommitPush: null,
    handleConfirmCommitPush: vi.fn(),
    handleCancelCommitPush: vi.fn(),
    pendingForcePush: false,
    setPendingForcePush: vi.fn(),
    doForcePush: vi.fn(),
    remoteOp: null,
    pendingHunkDiscard: null,
    setPendingHunkDiscard: vi.fn(),
    handleConfirmHunkDiscard: vi.fn(),
    pendingLineDiscard: null,
    setPendingLineDiscard: vi.fn(),
    handleConfirmLineDiscard: vi.fn(),
    ...over,
  };
}

describe('DestructiveDialogs', () => {
  it('renders no dialog when nothing is pending', () => {
    render(<DestructiveDialogs {...destructiveProps()} />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('discard: confirm clears pending then calls handleDiscard once with the paths', () => {
    const p = destructiveProps({ pendingDiscard: ['a.txt', 'b.txt'] });
    render(<DestructiveDialogs {...p} />);
    expect(screen.getByText('Discard changes to 2 file(s)?')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Discard changes' }));
    expect(p.setPendingDiscard).toHaveBeenCalledWith(null);
    expect(p.handleDiscard).toHaveBeenCalledTimes(1);
    expect(p.handleDiscard).toHaveBeenCalledWith(['a.txt', 'b.txt']);
  });

  it('discard: a stray Enter cancels (never confirms a destructive dialog)', async () => {
    const p = destructiveProps({ pendingDiscard: ['a.txt'] });
    render(<DestructiveDialogs {...p} />);
    await userEvent.keyboard('{Enter}');
    expect(p.handleDiscard).not.toHaveBeenCalled();
    expect(p.setPendingDiscard).toHaveBeenCalledWith(null); // Cancel had focus
  });

  it('discard: Escape closes without calling the handler', () => {
    const p = destructiveProps({ pendingDiscard: ['a.txt'] });
    render(<DestructiveDialogs {...p} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(p.setPendingDiscard).toHaveBeenCalledWith(null);
    expect(p.handleDiscard).not.toHaveBeenCalled();
  });

  it('hard reset names the branch, target and the working-tree warning; confirm dispatches', () => {
    const oid = 'b'.repeat(40);
    const p = destructiveProps({ pendingReset: { oid, mode: 'hard' } });
    render(<DestructiveDialogs {...p} />);
    expect(screen.getByRole('dialog', { name: 'Hard reset' })).toBeInTheDocument();
    expect(screen.getByText(/permanently discarded/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Reset (hard)' }));
    expect(p.handleResetBranch).toHaveBeenCalledTimes(1);
    expect(p.handleResetBranch).toHaveBeenCalledWith(oid, 'hard');
  });

  it('discard-all phrases modified vs permanently-deleted new files and lists them', () => {
    const p = destructiveProps({
      pendingDiscardForce: {
        paths: ['m.txt', 'n1.txt', 'n2.txt'],
        modified: 1,
        created: 2,
        untracked: ['n1.txt', 'n2.txt'],
      },
    });
    render(<DestructiveDialogs {...p} />);
    expect(screen.getByText('Revert 1 file and permanently delete 2 files?')).toBeInTheDocument();
    expect(screen.getByText('n1.txt')).toBeInTheDocument();
    expect(screen.getByRole('dialog', { name: 'Discard all changes' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Discard all' }));
    expect(p.handleDiscardForce).toHaveBeenCalledWith(['m.txt', 'n1.txt', 'n2.txt']);
  });

  it('a new-files-only set reads as a deletion, not a discard', () => {
    const p = destructiveProps({
      pendingDiscardForce: { paths: ['n1.txt'], modified: 0, created: 1, untracked: ['n1.txt'] },
    });
    render(<DestructiveDialogs {...p} />);
    expect(screen.getByRole('dialog', { name: 'Delete new file' })).toBeInTheDocument();
    expect(screen.getByText('Permanently delete 1 file?')).toBeInTheDocument();
    expect(screen.getByText('n1.txt')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete' }));
    expect(p.handleDiscardForce).toHaveBeenCalledWith(['n1.txt']);
  });

  it('several new files pluralize the delete title', () => {
    const p = destructiveProps({
      pendingDiscardForce: {
        paths: ['n1.txt', 'n2.txt'],
        modified: 0,
        created: 2,
        untracked: ['n1.txt', 'n2.txt'],
      },
    });
    render(<DestructiveDialogs {...p} />);
    expect(screen.getByRole('dialog', { name: 'Delete new files' })).toBeInTheDocument();
  });

  it('abort dialog picks title/handler from opState (rebase)', () => {
    const p = destructiveProps({
      abortConfirmOpen: true,
      opState: { kind: 'rebase', headName: 'main', onto: null, currentStep: 1, totalSteps: 3 },
    });
    render(<DestructiveDialogs {...p} />);
    expect(screen.getByRole('dialog', { name: 'Abort rebase?' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Abort rebase' }));
    expect(p.handleRebaseAbort).toHaveBeenCalledTimes(1);
    expect(p.handleAbortMerge).not.toHaveBeenCalled();
  });

  it('force-push confirm is danger-styled, busy while pushing, cancel never pushes', () => {
    const p = destructiveProps({ pendingForcePush: true, remoteOp: 'push' });
    render(<DestructiveDialogs {...p} />);
    const btn = screen.getByRole('button', { name: 'Force-push' });
    expect(btn).toHaveClass('btn-danger');
    expect(btn).toBeDisabled(); // remoteOp === 'push' ⇒ busy
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(p.setPendingForcePush).toHaveBeenCalledWith(false);
    expect(p.doForcePush).not.toHaveBeenCalled();
  });

  it('commit-&-push set-upstream gate uses the primary (non-danger) variant', () => {
    const p = destructiveProps({ pendingCommitPush: 'msg' });
    render(<DestructiveDialogs {...p} />);
    const btn = screen.getByRole('button', { name: 'Commit & Push' });
    expect(btn).toHaveClass('btn-primary');
    fireEvent.click(btn);
    expect(p.handleConfirmCommitPush).toHaveBeenCalledTimes(1);
  });
});

function stashProps(over: Partial<StashDialogsProps> = {}): StashDialogsProps {
  return {
    mutating: false,
    pendingDropStash: null,
    setPendingDropStash: vi.fn(),
    handleDropStash: vi.fn(),
    pendingReservedStash: null,
    setPendingReservedStash: vi.fn(),
    handleApplyStashSkipping: vi.fn(),
    handlePopStashSkipping: vi.fn(),
    ...over,
  };
}

describe('StashDialogs', () => {
  it('drop stash: confirm calls the handler once with the index; cancel does not', () => {
    const p = stashProps({ pendingDropStash: 2 });
    render(<StashDialogs {...p} />);
    expect(screen.getByRole('dialog', { name: 'Drop stash' })).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Drop stash' }));
    expect(p.handleDropStash).toHaveBeenCalledTimes(1);
    expect(p.handleDropStash).toHaveBeenCalledWith(2);
  });

  it('reserved-files gate routes pop vs apply and is primary-styled', () => {
    const p = stashProps({
      pendingReservedStash: { index: 1, op: 'pop', paths: ['nul.txt'] },
    });
    render(<StashDialogs {...p} />);
    const btn = screen.getByRole('button', { name: 'Apply the rest' });
    expect(btn).toHaveClass('btn-primary');
    fireEvent.click(btn);
    expect(p.handlePopStashSkipping).toHaveBeenCalledWith(1);
    expect(p.handleApplyStashSkipping).not.toHaveBeenCalled();
  });
});

function undoPlan(over: Partial<UndoPlan> = {}): UndoPlan {
  return {
    kind: 'commit',
    summary: 'feat: thing',
    targetOid: 'c'.repeat(40),
    targetShort: 'ccccccc',
    resetMode: 'soft',
    requiresCleanWorktree: false,
    worktreeDirty: false,
    undoable: true,
    reason: null,
    ...over,
  };
}

describe('UndoDialog', () => {
  it('soft undo renders primary Undo; confirm fires once', () => {
    const onConfirm = vi.fn();
    render(<UndoDialog plan={undoPlan()} busy={false} onConfirm={onConfirm} onCancel={vi.fn()} />);
    const btn = screen.getByRole('button', { name: 'Undo' });
    expect(btn).toHaveClass('btn-primary');
    fireEvent.click(btn);
    expect(onConfirm).toHaveBeenCalledTimes(1);
  });

  it('hard undo carries danger styling and the discard warning', () => {
    render(
      <UndoDialog
        plan={undoPlan({ kind: 'merge', resetMode: 'hard' })}
        busy={false}
        onConfirm={vi.fn()}
        onCancel={vi.fn()}
      />,
    );
    expect(screen.getByRole('button', { name: 'Undo' })).toHaveClass('btn-danger');
    expect(screen.getByText(/permanently discards uncommitted changes/)).toBeInTheDocument();
  });

  it('dirty worktree blocks a hard undo with the stash-first reason', () => {
    const onConfirm = vi.fn();
    render(
      <UndoDialog
        plan={undoPlan({ resetMode: 'hard', requiresCleanWorktree: true, worktreeDirty: true })}
        busy={false}
        onConfirm={onConfirm}
        onCancel={vi.fn()}
      />,
    );
    const btn = screen.getByRole('button', { name: 'Undo' });
    expect(btn).toBeDisabled();
    expect(screen.getByText('Commit or stash your changes first.')).toBeInTheDocument();
    fireEvent.click(btn);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('not-undoable plan shows the reason and disables Undo; Esc cancels', () => {
    const onCancel = vi.fn();
    render(
      <UndoDialog
        plan={undoPlan({ undoable: false, reason: 'HEAD moved since.' })}
        busy={false}
        onConfirm={vi.fn()}
        onCancel={onCancel}
      />,
    );
    expect(screen.getByText('HEAD moved since.')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Undo' })).toBeDisabled();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});

describe('NonFfPullDialog', () => {
  const base = {
    open: true,
    branch: 'main',
    upstream: 'origin/main',
    ahead: 2,
    behind: 1,
    busy: false,
  };

  it('names branch/upstream + counts and routes Merge / Rebase / Cancel', () => {
    const onMerge = vi.fn();
    const onRebase = vi.fn();
    const onCancel = vi.fn();
    render(<NonFfPullDialog {...base} onMerge={onMerge} onRebase={onRebase} onCancel={onCancel} />);
    expect(screen.getByText(/2 local commits/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Merge' }));
    expect(onMerge).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Rebase' }));
    expect(onRebase).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('busy disables both actions; Escape cancels without merging', () => {
    const onMerge = vi.fn();
    const onCancel = vi.fn();
    render(
      <NonFfPullDialog {...base} busy onMerge={onMerge} onRebase={vi.fn()} onCancel={onCancel} />,
    );
    expect(screen.getByRole('button', { name: 'Merge' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Rebase' })).toBeDisabled();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onMerge).not.toHaveBeenCalled();
  });
});
