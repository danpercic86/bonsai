/** T3.7 — WorktreeCreateDialog: branch preselect, name↔branch sync + decoupling,
 *  derived-path preview + slugging, used-branch handling, submit payload, and
 *  cancel/Esc. Copy-candidates fetch is kept pending under fake timers (the
 *  section's own behavior is covered by WorktreeCopyCandidates elsewhere). */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorktreeCreateDialog, type WorktreeCreateDialogProps } from './WorktreeCreateDialog';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
});

function props(over: Partial<WorktreeCreateDialogProps> = {}): WorktreeCreateDialogProps {
  return {
    open: true,
    busy: false,
    repoId: '/mock/repo',
    localBranches: ['main', 'dev', 'feature'],
    usedBranches: ['dev'],
    container: '/parent/.worktrees/repo',
    onSubmit: vi.fn(async () => {}),
    onCancel: vi.fn(),
    ...over,
  };
}

describe('WorktreeCreateDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = render(<WorktreeCreateDialog {...props({ open: false })} />);
    expect(container.firstChild).toBeNull();
  });

  it('preselects the first eligible branch and derives the path preview', () => {
    render(<WorktreeCreateDialog {...props()} />);
    expect(screen.getByRole('dialog', { name: 'New worktree' })).toBeInTheDocument();
    // 'main' is first eligible ('dev' is used); name defaults to it.
    expect(screen.getByText('/parent/.worktrees/repo/main')).toBeInTheDocument();
  });

  it('submits (branch, effective name, empty selections) with the defaulted name', () => {
    const p = props();
    render(<WorktreeCreateDialog {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Create worktree' }));
    expect(p.onSubmit).toHaveBeenCalledTimes(1);
    expect(p.onSubmit).toHaveBeenCalledWith('main', 'main', []);
  });

  it('editing the name decouples it from the branch and reslugs the preview', () => {
    render(<WorktreeCreateDialog {...props()} />);
    const nameInput = screen.getByPlaceholderText('main') as HTMLInputElement;
    fireEvent.change(nameInput, { target: { value: 'My Feature!!' } });
    // slugify('My Feature!!') → 'My-Feature'
    expect(screen.getByText('/parent/.worktrees/repo/My-Feature')).toBeInTheDocument();
  });

  it('warns (non-blocking) when the derived name slug matches an existing worktree', () => {
    render(<WorktreeCreateDialog {...props()} />);
    const nameInput = screen.getByPlaceholderText('main') as HTMLInputElement;
    fireEvent.change(nameInput, { target: { value: 'dev' } }); // 'dev' is a used branch
    expect(screen.getByText(/Name in use/)).toBeInTheDocument();
    // Still submittable.
    expect(screen.getByRole('button', { name: 'Create worktree' })).toBeEnabled();
  });

  it('shows the all-used message and blocks submit when every branch is checked out', () => {
    render(
      <WorktreeCreateDialog {...props({ localBranches: ['main'], usedBranches: ['main'] })} />,
    );
    expect(
      screen.getByText('Every local branch is already checked out in a worktree.'),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Create worktree' })).toBeDisabled();
  });

  it('Escape cancels', () => {
    const p = props();
    render(<WorktreeCreateDialog {...p} />);
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(p.onCancel).toHaveBeenCalledTimes(1);
  });

  it('surfaces a rejected submit inline without closing', async () => {
    const onSubmit = vi.fn(async () => {
      throw { kind: 'invalidName', message: 'path collides' };
    });
    render(<WorktreeCreateDialog {...props({ onSubmit })} />);
    fireEvent.click(screen.getByRole('button', { name: 'Create worktree' }));
    // Flush the rejected promise microtask.
    await vi.waitFor(() => expect(screen.getByText('path collides')).toBeInTheDocument());
    // Dialog is still open.
    expect(screen.getByRole('dialog', { name: 'New worktree' })).toBeInTheDocument();
  });

  it('busy disables Cancel and the submit button', () => {
    render(<WorktreeCreateDialog {...props({ busy: true })} />);
    expect(screen.getByRole('button', { name: 'Cancel' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Create worktree' })).toBeDisabled();
  });
});
