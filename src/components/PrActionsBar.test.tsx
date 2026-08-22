/** PrActionsBar — presentational PR actions footer (P83, UI contract §1). Renders
 *  only for an OPEN PR: a per-forge Close/Decline/Abandon + a primary Merge that
 *  is disabled (with a visible reason) whenever the PR is not mergeable. All state
 *  is props; no IPC. The per-forge label + verb helpers are pinned too. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import {
  PrActionsBar,
  closeActionLabel,
  closeActionGerund,
  closeActionPast,
} from './PrActionsBar';
import type { ForgeKind } from '../ipc';
import { SUPPORTED_MERGE_METHODS } from '../ipc';

function renderBar(over: Partial<Parameters<typeof PrActionsBar>[0]> = {}) {
  const onMerge = vi.fn();
  const onClose = vi.fn();
  const kind: ForgeKind = over.kind ?? 'gitHub';
  const utils = render(
    <PrActionsBar
      state="open"
      kind={kind}
      mergeable={true}
      supportedMethods={SUPPORTED_MERGE_METHODS[kind]}
      busy={false}
      onMerge={onMerge}
      onClose={onClose}
      {...over}
    />,
  );
  return { ...utils, onMerge, onClose };
}

const merge = () => screen.getByRole('button', { name: /Merge/ });
const mergeMaybe = () => screen.queryByRole('button', { name: /Merge/ });

describe('PrActionsBar', () => {
  it('renders nothing unless the PR is open', () => {
    const closed = renderBar({ state: 'closed' });
    expect(closed.container).toBeEmptyDOMElement();
    closed.unmount();
    const merged = renderBar({ state: 'merged' });
    expect(merged.container).toBeEmptyDOMElement();
  });

  it('the primary Merge action invokes onMerge (opens the merge dialog)', () => {
    const { onMerge } = renderBar();
    fireEvent.click(merge());
    expect(onMerge).toHaveBeenCalledTimes(1);
  });

  it('Merge is enabled with no reason tooltip when the PR is mergeable', () => {
    renderBar({ mergeable: true });
    const btn = merge();
    expect(btn).toBeEnabled();
    expect(btn).not.toHaveAttribute('title');
  });

  it('Merge is DISABLED with a "still checking" reason while mergeability is unknown', () => {
    const { onMerge } = renderBar({ mergeable: null });
    const btn = merge();
    expect(btn).toBeDisabled();
    expect(btn.getAttribute('title')).toMatch(/still checking/i);
    fireEvent.click(btn);
    expect(onMerge).not.toHaveBeenCalled();
  });

  it('Merge is DISABLED with a conflicts reason when the PR is not mergeable', () => {
    renderBar({ mergeable: false });
    const btn = merge();
    expect(btn).toBeDisabled();
    expect(btn.getAttribute('title')).toMatch(/conflicts/i);
  });

  it('hides Merge entirely for a forge that exposes no methods', () => {
    renderBar({ kind: 'unknown', supportedMethods: [] });
    expect(mergeMaybe()).toBeNull();
    // The close action is still present.
    expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
  });

  it('busy disables both actions', () => {
    renderBar({ busy: true });
    expect(merge()).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Close' })).toBeDisabled();
  });

  it.each([
    ['gitHub', 'Close'],
    ['gitLab', 'Close'],
    ['bitbucket', 'Decline'],
    ['azureDevOps', 'Abandon'],
  ] as const)('shows the %s close verb "%s" and wires it to onClose', (kind, label) => {
    const { onClose } = renderBar({ kind });
    const btn = screen.getByRole('button', { name: label });
    fireEvent.click(btn);
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});

describe('close-action label helpers', () => {
  it.each([
    ['gitHub', 'Close', 'Closed', 'Closing'],
    ['gitLab', 'Close', 'Closed', 'Closing'],
    ['unknown', 'Close', 'Closed', 'Closing'],
    ['bitbucket', 'Decline', 'Declined', 'Declining'],
    ['azureDevOps', 'Abandon', 'Abandoned', 'Abandoning'],
  ] as const)('%s → %s / %s / %s', (kind, label, past, gerund) => {
    expect(closeActionLabel(kind)).toBe(label);
    expect(closeActionPast(kind)).toBe(past);
    expect(closeActionGerund(kind)).toBe(gerund);
  });
});
