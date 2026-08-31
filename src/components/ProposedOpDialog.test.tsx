/** ProposedOpDialog — the approval GATE for an AI-proposed git operation (P55c,
 *  safety layer L6). It is purely presentational: it dispatches NOTHING itself.
 *  The load-bearing invariant pinned here is that the op NEVER auto-applies —
 *  onConfirm fires only on an explicit click, never on mount/render, and never on
 *  a stray Enter (initial focus is Cancel, inherited from ConfirmDialog). */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ProposedOpDialog } from './ProposedOpDialog';
import type { ProposedOperation } from '../ipc';

const DESTRUCTIVE_OP: ProposedOperation = {
  op: { kind: 'reset', targetOid: 'b'.repeat(40), targetShort: 'bbbbbbb', mode: 'mixed' },
  preview: {
    title: 'Reset current branch',
    summary: 'Move main back to bbbbbbb, dropping 2 commits.',
    danger: 'destructive',
    refChanges: [{ name: 'main', fromShort: 'aaaaaaa', toShort: 'bbbbbbb' }],
    droppedCommits: [
      { short: 'ccccccc', summary: 'Add feature X' },
      { short: 'ddddddd', summary: 'Fix bug Y' },
    ],
    addedCommits: 0,
    worktreeWarning: 'You have uncommitted changes that could be affected.',
    confirmLabel: 'Reset branch',
  },
  rationale: 'Interpreted "undo my last two commits" as a mixed reset.',
  costUsd: null,
};

const SAFE_OP: ProposedOperation = {
  op: { kind: 'createBranch', name: 'feature/x', atOid: null },
  preview: {
    title: 'Create branch',
    summary: 'Create feature/x at HEAD.',
    danger: 'safe',
    refChanges: [],
    droppedCommits: [],
    addedCommits: 1,
    worktreeWarning: null,
    confirmLabel: 'Create branch',
  },
  rationale: '',
  costUsd: null,
};

function renderDialog(over: Partial<Parameters<typeof ProposedOpDialog>[0]> = {}) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  const utils = render(
    <ProposedOpDialog
      open
      operation={DESTRUCTIVE_OP}
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
      {...over}
    />,
  );
  return { ...utils, onConfirm, onCancel };
}

describe('ProposedOpDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = renderDialog({ open: false });
    expect(container).toBeEmptyDOMElement();
  });

  it('renders nothing when there is no operation (no preview)', () => {
    const { container } = renderDialog({ operation: null });
    expect(container).toBeEmptyDOMElement();
  });

  it('renders the proposed operation details from props', () => {
    renderDialog();
    // Title (via the ConfirmDialog dialog label) + summary.
    expect(screen.getByRole('dialog', { name: 'Reset current branch' })).toBeInTheDocument();
    expect(
      screen.getByText('Move main back to bbbbbbb, dropping 2 commits.'),
    ).toBeInTheDocument();
    // Danger badge.
    const badge = screen.getByText('Destructive');
    expect(badge).toHaveClass('danger-badge', 'destructive');
    // Ref change from → to.
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.getByText('aaaaaaa')).toBeInTheDocument();
    expect(screen.getByText('bbbbbbb')).toBeInTheDocument();
    // Dropped commits (plural label + each summary).
    expect(screen.getByText('2 commits leave the branch')).toBeInTheDocument();
    expect(screen.getByText('Add feature X')).toBeInTheDocument();
    expect(screen.getByText('Fix bug Y')).toBeInTheDocument();
    // Worktree warning + rationale.
    expect(
      screen.getByText('You have uncommitted changes that could be affected.'),
    ).toBeInTheDocument();
    expect(
      screen.getByText('Interpreted "undo my last two commits" as a mixed reset.'),
    ).toBeInTheDocument();
  });

  it('a destructive op uses a danger confirm button labelled from the preview', () => {
    renderDialog();
    const apply = screen.getByRole('button', { name: 'Reset branch' });
    expect(apply).toHaveClass('btn-danger');
  });

  it('a safe op uses a primary (non-danger) confirm button and the Safe badge', () => {
    renderDialog({ operation: SAFE_OP });
    expect(screen.getByText('Safe')).toHaveClass('danger-badge', 'safe');
    const apply = screen.getByRole('button', { name: 'Create branch' });
    expect(apply).toHaveClass('btn-primary');
    expect(apply).not.toHaveClass('btn-danger');
    // addedCommits summary line.
    expect(screen.getByText('Adds 1 new commit.')).toBeInTheDocument();
  });

  it('Apply (confirm) invokes onConfirm exactly once; nothing else', () => {
    const { onConfirm, onCancel } = renderDialog();
    fireEvent.click(screen.getByRole('button', { name: 'Reset branch' }));
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onCancel).not.toHaveBeenCalled();
  });

  it('the Cancel button invokes onCancel without confirming', () => {
    const { onConfirm, onCancel } = renderDialog();
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('Escape cancels without confirming', () => {
    const { onConfirm, onCancel } = renderDialog();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('NEVER auto-applies: no confirm fires on mount, and a stray Enter cancels instead', async () => {
    const { onConfirm, onCancel } = renderDialog();
    // Mount alone must not dispatch.
    expect(onConfirm).not.toHaveBeenCalled();
    // Initial focus is Cancel (a stray Enter must never approve a git op).
    expect(screen.getByRole('button', { name: 'Cancel' })).toHaveFocus();
    await userEvent.keyboard('{Enter}');
    expect(onConfirm).not.toHaveBeenCalled();
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it('busy disables Apply (approval cannot be double-dispatched)', () => {
    renderDialog({ busy: true });
    expect(screen.getByRole('button', { name: 'Reset branch' })).toBeDisabled();
  });
});
