/** T3.5 — WorkspaceToolbar: the button enable/disable matrix vs op-state props
 *  (mutating / refreshing / canPullPush / canForcePush / headBorn), in-flight
 *  labels, push-title upstream logic, the force-push caret menu, AI gating, and
 *  the auto-fetch readout. Presentational only. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import type { WorkspaceToolbarProps } from './WorkspaceToolbar';
import type { BranchInfo, JobStatus } from '../ipc';
import { shortcutLabel } from '../utils/platform';

const HEAD: BranchInfo = {
  name: 'main',
  isHead: true,
  upstream: 'origin/main',
  ahead: 0,
  behind: 0,
  tip: 'a'.repeat(40),
};

function renderBar(over: Partial<WorkspaceToolbarProps> = {}) {
  const props: WorkspaceToolbarProps = {
    remoteOp: null,
    refreshing: false,
    mutating: false,
    statusLoading: false,
    graphLoading: false,
    canPullPush: true,
    canForcePush: true,
    aiEligible: false,
    aiPanelLoading: false,
    headBranch: HEAD,
    jobStatus: [],
    jobNow: 0,
    onFetch: vi.fn(),
    onPull: vi.fn(),
    onPush: vi.fn(),
    onForcePush: vi.fn(),
    onWhatChanged: vi.fn(),
    onAskBonsai: vi.fn(),
    onUndo: vi.fn(),
    onViewHeadReflog: vi.fn(),
    headBorn: true,
    onRefresh: vi.fn(),
    externalItems: [{ label: 'Open in Terminal', onSelect: vi.fn() }],
    ...over,
  };
  return { ...render(<WorkspaceToolbar {...props} />), props };
}

const btn = (name: string | RegExp) => screen.getByRole('button', { name });

describe('WorkspaceToolbar', () => {
  it('idle state: remote ops + undo/reflog/refresh enabled and wired', () => {
    const { props } = renderBar();
    fireEvent.click(btn('↓ Fetch'));
    fireEvent.click(btn('⇣ Pull'));
    fireEvent.click(btn('↑ Push'));
    fireEvent.click(btn('↶ Undo'));
    fireEvent.click(btn('↺ Reflog'));
    fireEvent.click(btn('Refresh'));
    expect(props.onFetch).toHaveBeenCalledTimes(1);
    expect(props.onPull).toHaveBeenCalledTimes(1);
    expect(props.onPush).toHaveBeenCalledTimes(1);
    expect(props.onUndo).toHaveBeenCalledTimes(1);
    expect(props.onViewHeadReflog).toHaveBeenCalledTimes(1);
    expect(props.onRefresh).toHaveBeenCalledTimes(1);
  });

  it('mutating disables fetch/pull/push/undo/caret/refresh', () => {
    renderBar({ mutating: true });
    for (const name of ['↓ Fetch', '⇣ Pull', '↑ Push', '↶ Undo', 'More push actions', 'Refresh']) {
      expect(btn(name)).toBeDisabled();
    }
    // Reflog + external-open never touch git state — they stay enabled.
    expect(btn('↺ Reflog')).toBeEnabled();
    expect(btn('Open externally')).toBeEnabled();
  });

  it('refreshing disables the remote ops and shows the progress bar', () => {
    const { container } = renderBar({ refreshing: true });
    expect(btn('↓ Fetch')).toBeDisabled();
    expect(container.querySelector('.header-progress')).toBeInTheDocument();
  });

  it('canPullPush=false disables pull/push but not fetch', () => {
    renderBar({ canPullPush: false });
    expect(btn('⇣ Pull')).toBeDisabled();
    expect(btn('↑ Push')).toBeDisabled();
    expect(btn('↓ Fetch')).toBeEnabled();
  });

  it('headBorn=false disables undo + reflog', () => {
    renderBar({ headBorn: false });
    expect(btn('↶ Undo')).toBeDisabled();
    expect(btn('↺ Reflog')).toBeDisabled();
  });

  it('in-flight remote op flips the matching label', () => {
    renderBar({ remoteOp: 'fetch' });
    expect(screen.getByText('Fetching…')).toBeInTheDocument();
    renderBar({ remoteOp: 'pull' });
    expect(screen.getByText('Pulling…')).toBeInTheDocument();
    renderBar({ remoteOp: 'push' });
    expect(screen.getByText('Pushing…')).toBeInTheDocument();
  });

  it('push title: existing upstream vs set-upstream-on-first-push', () => {
    renderBar();
    expect(btn('↑ Push')).toHaveAttribute(
      'title',
      `Push main to origin/main (${shortcutLabel('Mod+Shift+U')})`,
    );
    renderBar({ headBranch: { ...HEAD, upstream: null } });
    expect(screen.getAllByRole('button', { name: '↑ Push' })[1]).toHaveAttribute(
      'title',
      `Push main to origin/main and set upstream (${shortcutLabel('Mod+Shift+U')})`,
    );
  });

  it('force-push: caret disabled without an upstream; menu item fires onForcePush', () => {
    renderBar({ canForcePush: false });
    expect(btn('More push actions')).toBeDisabled();
    const { props } = renderBar();
    const caret = screen.getAllByRole('button', { name: 'More push actions' })[1];
    fireEvent.click(caret);
    fireEvent.click(screen.getByText('Force-push with lease…'));
    expect(props.onForcePush).toHaveBeenCalledTimes(1);
  });

  it('AI buttons render only when eligible; What-changed disabled while loading', () => {
    renderBar();
    expect(screen.queryByText(/What changed/)).not.toBeInTheDocument();
    expect(screen.queryByText('✨ Ask…')).not.toBeInTheDocument();
    const { props } = renderBar({ aiEligible: true, aiPanelLoading: true });
    expect(btn(/What changed/)).toBeDisabled();
    fireEvent.click(btn('✨ Ask…'));
    expect(props.onAskBonsai).toHaveBeenCalledTimes(1);
  });

  it('external dropdown opens the provided items', () => {
    const onSelect = vi.fn();
    renderBar({ externalItems: [{ label: 'Open in Terminal', onSelect }] });
    fireEvent.click(btn('Open externally'));
    fireEvent.click(screen.getByText('Open in Terminal'));
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it('auto-fetch readout: last-run text and the backoff notice', () => {
    const base: JobStatus = {
      job: 'autoFetch',
      enabled: true,
      lastRunMs: 0,
      lastOutcome: 'success',
      lastError: null,
      consecutiveFailures: 0,
      inBackoff: false,
      nextRunMs: null,
    };
    renderBar({ jobStatus: [base], jobNow: 5 * 60_000 });
    expect(screen.getByText(/Fetched .* ago/)).toBeInTheDocument();
    renderBar({
      jobStatus: [{ ...base, inBackoff: true, lastError: 'auth failed', nextRunMs: 10 * 60_000 }],
      jobNow: 0,
    });
    const paused = screen.getByText(/Auto-fetch paused/);
    expect(paused).toHaveAttribute('title', 'auth failed');
    // Disabled job or no status -> no readout.
    renderBar({ jobStatus: [{ ...base, enabled: false }] });
    expect(screen.getAllByText(/Fetched|paused/)).toHaveLength(2); // only the two above
  });
});
