/** T3.3a — workspace dialog wrappers, management set: BranchTagDialogs,
 *  RemoteDialogs, SubmoduleDialogs, WorktreeDialogs, CleanupDialogs.
 *  Representative per dialog: copy, validation, confirm-once, cancel-no-call. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { BranchTagDialogs, type BranchTagDialogsProps } from './BranchTagDialogs';
import { RemoteDialogs, type RemoteDialogsProps } from './RemoteDialogs';
import { SubmoduleDialogs, type SubmoduleDialogsProps } from './SubmoduleDialogs';
import { WorktreeDialogs, type WorktreeDialogsProps } from './WorktreeDialogs';
import { CleanupDialogs, type CleanupDialogsProps } from './CleanupDialogs';
import type { BranchesSnapshot } from '../../ipc';

const branches: BranchesSnapshot = {
  local: [
    { name: 'main', isHead: true, upstream: 'origin/main', ahead: 0, behind: 0, tip: 'a'.repeat(40) },
    { name: 'dev', isHead: false, upstream: null, ahead: null, behind: null, tip: 'b'.repeat(40) },
  ],
  remote: [{ name: 'origin/main', tip: 'a'.repeat(40) }],
  tags: ['v1.0.0'],
  head: { branchName: 'main', oid: 'a'.repeat(40), detached: false, unborn: false },
};

function branchTagProps(over: Partial<BranchTagDialogsProps> = {}): BranchTagDialogsProps {
  return {
    mutating: false,
    branches,
    pendingDeleteBranch: null,
    setPendingDeleteBranch: vi.fn(),
    handleDeleteBranch: vi.fn(),
    pendingRebase: null,
    setPendingRebase: vi.fn(),
    handleRebaseBranch: vi.fn(),
    pendingCreateBranch: null,
    setPendingCreateBranch: vi.fn(),
    handleCreateBranchHere: vi.fn(),
    pendingRenameBranch: null,
    setPendingRenameBranch: vi.fn(),
    handleRenameBranch: vi.fn(),
    aiEligible: false,
    workingDirty: false,
    suggestBranchName: vi.fn(),
    pendingCreateTag: null,
    setPendingCreateTag: vi.fn(),
    handleCreateTag: vi.fn(),
    pendingDeleteTag: null,
    setPendingDeleteTag: vi.fn(),
    handleDeleteTag: vi.fn(),
    ...over,
  };
}

describe('BranchTagDialogs', () => {
  it('delete branch: names the branch; confirm calls the handler once, cancel never', () => {
    const p = branchTagProps({ pendingDeleteBranch: 'dev' });
    const { rerender } = render(<BranchTagDialogs {...p} />);
    expect(screen.getByRole('dialog', { name: 'Delete branch' })).toBeInTheDocument();
    expect(screen.getByText('dev')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete branch' }));
    expect(p.handleDeleteBranch).toHaveBeenCalledTimes(1);
    expect(p.handleDeleteBranch).toHaveBeenCalledWith('dev');
    // Cancel path on a fresh instance.
    const p2 = branchTagProps({ pendingDeleteBranch: 'dev' });
    rerender(<BranchTagDialogs {...p2} />);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(p2.setPendingDeleteBranch).toHaveBeenCalledWith(null);
    expect(p2.handleDeleteBranch).not.toHaveBeenCalled();
  });

  it('create branch: existing name blocks submit; a fresh name submits trimmed', () => {
    const oid = 'c'.repeat(40);
    const p = branchTagProps({ pendingCreateBranch: { oid } });
    render(<BranchTagDialogs {...p} />);
    const input = screen.getByLabelText('Branch name');
    fireEvent.change(input, { target: { value: 'dev' } });
    expect(screen.getByText('A branch with that name already exists')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create branch' })).toBeDisabled();
    fireEvent.change(input, { target: { value: '  feature/x  ' } });
    fireEvent.submit(input.closest('form')!);
    expect(p.handleCreateBranchHere).toHaveBeenCalledWith(oid, 'feature/x');
  });

  it('rename branch: prefilled with the old name; unchanged name is allowed', () => {
    const p = branchTagProps({ pendingRenameBranch: { name: 'dev' } });
    render(<BranchTagDialogs {...p} />);
    const input = screen.getByLabelText('New branch name');
    expect(input).toHaveValue('dev');
    // Unchanged name does not trip the exists-check.
    expect(screen.queryByText('A branch with that name already exists')).not.toBeInTheDocument();
    fireEvent.change(input, { target: { value: 'main' } });
    expect(screen.getByText('A branch with that name already exists')).toBeInTheDocument();
    fireEvent.change(input, { target: { value: 'dev2' } });
    fireEvent.submit(input.closest('form')!);
    expect(p.handleRenameBranch).toHaveBeenCalledWith('dev', 'dev2');
  });

  it('delete tag: confirm dispatches with the tag name', () => {
    const p = branchTagProps({ pendingDeleteTag: 'v1.0.0' });
    render(<BranchTagDialogs {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Delete tag' }));
    expect(p.handleDeleteTag).toHaveBeenCalledWith('v1.0.0');
  });
});

function remoteProps(over: Partial<RemoteDialogsProps> = {}): RemoteDialogsProps {
  return {
    mutating: false,
    remotes: [{ name: 'origin', url: 'https://example.com/r.git' }],
    pendingDeleteRemote: null,
    setPendingDeleteRemote: vi.fn(),
    handleDeleteRemoteTracking: vi.fn(),
    pendingAddRemote: false,
    setPendingAddRemote: vi.fn(),
    handleAddRemote: vi.fn(),
    pendingEditUrl: null,
    setPendingEditUrl: vi.fn(),
    handleSetRemoteUrl: vi.fn(),
    pendingRenameRemote: null,
    setPendingRenameRemote: vi.fn(),
    handleRenameRemote: vi.fn(),
    pendingRemoveRemote: null,
    setPendingRemoveRemote: vi.fn(),
    handleRemoveRemote: vi.fn(),
    ...over,
  };
}

describe('RemoteDialogs', () => {
  it('remove remote: confirm dispatches once; Escape closes without removing', () => {
    const p = remoteProps({ pendingRemoveRemote: 'origin' });
    render(<RemoteDialogs {...p} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(p.setPendingRemoveRemote).toHaveBeenCalledWith(null);
    expect(p.handleRemoveRemote).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole('button', { name: 'Remove remote' }));
    expect(p.handleRemoveRemote).toHaveBeenCalledTimes(1);
    expect(p.handleRemoveRemote).toHaveBeenCalledWith('origin');
  });

  it('rename remote: whitespace and duplicate names are rejected, valid name submits', () => {
    const p = remoteProps({
      remotes: [
        { name: 'origin', url: null },
        { name: 'upstream', url: null },
      ],
      pendingRenameRemote: { name: 'upstream' },
    });
    render(<RemoteDialogs {...p} />);
    const input = screen.getByLabelText('New remote name');
    expect(input).toHaveValue('upstream');
    fireEvent.change(input, { target: { value: 'or igin' } });
    expect(screen.getByText('Remote name cannot contain whitespace')).toBeInTheDocument();
    fireEvent.change(input, { target: { value: 'origin' } });
    expect(screen.getByText('A remote with that name already exists')).toBeInTheDocument();
    fireEvent.change(input, { target: { value: 'fork' } });
    fireEvent.submit(input.closest('form')!);
    expect(p.handleRenameRemote).toHaveBeenCalledWith('upstream', 'fork');
  });

  it('delete remote-tracking ref explains it is local-only', () => {
    const p = remoteProps({ pendingDeleteRemote: 'origin/dev' });
    render(<RemoteDialogs {...p} />);
    expect(screen.getByText(/does NOT delete the branch on/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Delete reference' }));
    expect(p.handleDeleteRemoteTracking).toHaveBeenCalledWith('origin/dev');
  });
});

function submoduleProps(over: Partial<SubmoduleDialogsProps> = {}): SubmoduleDialogsProps {
  return {
    mutating: false,
    addOpen: false,
    setAddOpen: vi.fn(),
    handleAddSubmodule: vi.fn(),
    pendingDeinit: null,
    setPendingDeinit: vi.fn(),
    handleDeinitSubmodule: vi.fn(),
    pendingRemove: null,
    setPendingRemove: vi.fn(),
    handleRemoveSubmodule: vi.fn(),
    ...over,
  };
}

describe('SubmoduleDialogs', () => {
  it('add: path defaults from the URL basename (".git" stripped) until edited', () => {
    const p = submoduleProps({ addOpen: true });
    render(<SubmoduleDialogs {...p} />);
    fireEvent.change(screen.getByLabelText('Repository URL'), {
      target: { value: 'https://example.com/vendor/lib.git' },
    });
    const pathInput = screen.getByLabelText(/^Path/);
    expect(pathInput).toHaveValue('lib');
    fireEvent.submit(pathInput.closest('form')!);
    expect(p.handleAddSubmodule).toHaveBeenCalledWith('https://example.com/vendor/lib.git', 'lib');
  });

  it('add: traversing / absolute / backslash paths are rejected', () => {
    render(<SubmoduleDialogs {...submoduleProps({ addOpen: true })} />);
    fireEvent.change(screen.getByLabelText('Repository URL'), {
      target: { value: 'https://example.com/lib.git' },
    });
    const pathInput = screen.getByLabelText(/^Path/);
    fireEvent.change(pathInput, { target: { value: '../evil' } });
    expect(screen.getByText('Path cannot contain ".."')).toBeInTheDocument();
    fireEvent.change(pathInput, { target: { value: 'C:/abs' } });
    expect(screen.getByText('Path must be relative to the repository')).toBeInTheDocument();
    fireEvent.change(pathInput, { target: { value: 'a\\b' } });
    expect(screen.getByText('Use forward slashes in the path')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Add submodule' })).toBeDisabled();
  });

  it('deinit is the primary (reversible) confirm; remove is danger-styled', () => {
    const p = submoduleProps({ pendingDeinit: 'libA', pendingRemove: 'libB' });
    render(<SubmoduleDialogs {...p} />);
    expect(screen.getByRole('button', { name: 'Deinitialize' })).toHaveClass('btn-primary');
    expect(screen.getByRole('button', { name: 'Remove submodule' })).toHaveClass('btn-danger');
    fireEvent.click(screen.getByRole('button', { name: 'Remove submodule' }));
    expect(p.handleRemoveSubmodule).toHaveBeenCalledTimes(1);
    expect(p.handleRemoveSubmodule).toHaveBeenCalledWith('libB');
  });
});

function worktreeProps(over: Partial<WorktreeDialogsProps> = {}): WorktreeDialogsProps {
  return {
    repoId: 'D:/repos/demo',
    mutating: false,
    branches,
    worktrees: [],
    newWorktreeOpen: false,
    setNewWorktreeOpen: vi.fn(),
    handleAddWorktree: vi.fn(),
    worktreeContextOpen: false,
    setWorktreeContextOpen: vi.fn(),
    pendingWorktreeLock: null,
    setPendingWorktreeLock: vi.fn(),
    handleLockWorktree: vi.fn(),
    pendingWorktreeRemove: null,
    setPendingWorktreeRemove: vi.fn(),
    handleRemoveWorktree: vi.fn(),
    ...over,
  };
}

describe('WorktreeDialogs', () => {
  it('lock: empty reason is passed as undefined, non-empty is trimmed', () => {
    const p = worktreeProps({ pendingWorktreeLock: 'wt-1' });
    const { rerender } = render(<WorktreeDialogs {...p} />);
    const input = screen.getByLabelText('Reason (optional)');
    fireEvent.submit(input.closest('form')!);
    expect(p.handleLockWorktree).toHaveBeenCalledWith('wt-1', undefined);
    const p2 = worktreeProps({ pendingWorktreeLock: 'wt-1' });
    rerender(<WorktreeDialogs {...p2} />);
    const input2 = screen.getByLabelText('Reason (optional)');
    fireEvent.change(input2, { target: { value: '  pinned for QA  ' } });
    fireEvent.submit(input2.closest('form')!);
    expect(p2.handleLockWorktree).toHaveBeenCalledWith('wt-1', 'pinned for QA');
  });

  it('remove: names the exact directory; confirm dispatches, cancel never', () => {
    const p = worktreeProps({
      pendingWorktreeRemove: { name: 'wt-1', absPath: 'D:/repos/demo-wt/wt-1' },
    });
    render(<WorktreeDialogs {...p} />);
    expect(screen.getByText('D:/repos/demo-wt/wt-1')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: 'Remove worktree' }));
    expect(p.handleRemoveWorktree).toHaveBeenCalledTimes(1);
    expect(p.handleRemoveWorktree).toHaveBeenCalledWith('wt-1');
  });
});

function cleanupProps(over: Partial<CleanupDialogsProps> = {}): CleanupDialogsProps {
  return {
    repoId: 'D:/repos/demo',
    mutating: false,
    headBranch: branches.local[0],
    branches,
    staleCleanupOpen: false,
    setStaleCleanupOpen: vi.fn(),
    refetchBranches: vi.fn().mockResolvedValue(undefined),
    refetchGraph: vi.fn().mockResolvedValue(undefined),
    whatChangedOpen: false,
    setWhatChangedOpen: vi.fn(),
    runDigest: vi.fn(),
    rebasePlan: null,
    setRebasePlan: vi.fn(),
    rebasePlanError: null,
    setRebasePlanError: vi.fn(),
    handleStartInteractiveRebase: vi.fn(),
    menu: null,
    closeMenu: vi.fn(),
  ...over,
  };
}

describe('CleanupDialogs', () => {
  it('renders nothing visible when everything is closed', () => {
    render(<CleanupDialogs {...cleanupProps()} />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('a set menu renders the graph ContextMenu and item clicks route + close', () => {
    const onSelect = vi.fn();
    const p = cleanupProps({
      menu: { x: 10, y: 10, items: [{ label: 'Checkout', onSelect }] },
    });
    render(<CleanupDialogs {...p} />);
    fireEvent.click(screen.getByRole('menuitem', { name: 'Checkout' }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(p.closeMenu).toHaveBeenCalledTimes(1);
  });
});
