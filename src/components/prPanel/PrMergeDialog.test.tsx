/** PrMergeDialog — the merge form + confirmation (P83, UI contract §2). It IS the
 *  confirmation (no second modal) and holds only local form state. Pinned here:
 *  the merge-method options shown match the forge, confirm yields a well-formed
 *  MergePrInput, commit fields track the chosen method, the delete-source-branch
 *  control is hidden on GitHub, and Cancel/Esc dismiss without merging. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { PrMergeDialog } from './PrMergeDialog';
import type { ForgeKind } from '../../ipc';
import { SUPPORTED_MERGE_METHODS } from '../../ipc';

function renderMerge(over: Partial<Parameters<typeof PrMergeDialog>[0]> = {}) {
  const onConfirm = vi.fn();
  const onCancel = vi.fn();
  const kind: ForgeKind = over.kind ?? 'gitHub';
  const utils = render(
    <PrMergeDialog
      open
      number={128}
      kind={kind}
      host="github.com"
      sourceBranch="feat"
      targetBranch="main"
      supportedMethods={SUPPORTED_MERGE_METHODS[kind]}
      busy={false}
      onConfirm={onConfirm}
      onCancel={onCancel}
      {...over}
    />,
  );
  return { ...utils, onConfirm, onCancel };
}

const confirmBtn = () => screen.getByRole('button', { name: /^Merge pull request$/ });

describe('PrMergeDialog', () => {
  it('renders nothing when closed', () => {
    const { container } = renderMerge({ open: false });
    expect(container).toBeEmptyDOMElement();
  });

  it('titles with the PR number', () => {
    renderMerge({ number: 42 });
    expect(screen.getByText('Merge pull request #42?')).toBeInTheDocument();
  });

  it('shows exactly the GitHub merge methods (merge/squash/rebase, no fast-forward)', () => {
    renderMerge({ kind: 'gitHub' });
    expect(screen.getByRole('radio', { name: 'Merge' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Squash' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Rebase' })).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: 'Fast-forward' })).toBeNull();
  });

  it('shows the Bitbucket method set (merge/squash/fast-forward, no rebase)', () => {
    renderMerge({ kind: 'bitbucket' });
    expect(screen.getByRole('radio', { name: 'Merge' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Squash' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Fast-forward' })).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: 'Rebase' })).toBeNull();
  });

  it('shows the GitLab method set (merge/squash only)', () => {
    renderMerge({ kind: 'gitLab' });
    expect(screen.getByRole('radio', { name: 'Merge' })).toBeInTheDocument();
    expect(screen.getByRole('radio', { name: 'Squash' })).toBeInTheDocument();
    expect(screen.queryByRole('radio', { name: 'Rebase' })).toBeNull();
    expect(screen.queryByRole('radio', { name: 'Fast-forward' })).toBeNull();
  });

  it('defaults to the first supported method and confirms a well-formed MergePrInput', () => {
    const { onConfirm } = renderMerge({ kind: 'gitHub' });
    fireEvent.click(confirmBtn());
    expect(onConfirm).toHaveBeenCalledTimes(1);
    expect(onConfirm).toHaveBeenCalledWith({
      method: 'merge',
      commitTitle: null,
      commitMessage: null,
      deleteSourceBranch: false,
      headSha: null,
    });
  });

  it('carries the selected method + typed commit fields into the input', () => {
    const { onConfirm } = renderMerge({ kind: 'gitHub' });
    fireEvent.click(screen.getByRole('radio', { name: 'Squash' }));
    // Title + message share a placeholder; scope to the "Commit title" field's input.
    const titleInput = screen.getByText('Commit title').closest('label')!.querySelector('input')!;
    fireEvent.change(titleInput, { target: { value: 'My squash title' } });
    fireEvent.click(confirmBtn());
    expect(onConfirm).toHaveBeenCalledWith(
      expect.objectContaining({ method: 'squash', commitTitle: 'My squash title' }),
    );
  });

  it('hides the commit title/message fields for a rebase (no merge commit)', () => {
    renderMerge({ kind: 'gitHub' });
    // Present for the default merge method…
    expect(screen.getByText('Commit title')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('radio', { name: 'Rebase' }));
    // …gone once rebase is selected.
    expect(screen.queryByText('Commit title')).not.toBeInTheDocument();
    expect(screen.queryByText('Commit message')).not.toBeInTheDocument();
  });

  it('hides the delete-source-branch toggle on GitHub (its API ignores it)', () => {
    renderMerge({ kind: 'gitHub' });
    expect(screen.queryByRole('checkbox')).toBeNull();
  });

  it('shows and honours the delete-source-branch toggle on a non-GitHub forge', () => {
    const { onConfirm } = renderMerge({ kind: 'bitbucket' });
    const toggle = screen.getByRole('checkbox');
    fireEvent.click(toggle);
    fireEvent.click(confirmBtn());
    expect(onConfirm).toHaveBeenCalledWith(
      expect.objectContaining({ deleteSourceBranch: true }),
    );
  });

  it('Cancel and Esc dismiss without merging', () => {
    const { onConfirm, onCancel } = renderMerge();
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(onCancel).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
    expect(onCancel).toHaveBeenCalledTimes(2);
    expect(onConfirm).not.toHaveBeenCalled();
  });

  it('busy disables the confirm button and shows the in-flight label', () => {
    renderMerge({ busy: true });
    const btn = screen.getByRole('button', { name: 'Merging…' });
    expect(btn).toBeDisabled();
  });
});
