/**
 * P119-ui §2 / §4 — the dock's DOM: the determinate bar (`--progress` from the
 * active run, clamped), the indeterminate sweep otherwise, the empty-dock hint,
 * and the new row states (conflicts pill, outcome detail, count noun, blocked
 * noun) as rendered by `GitActivityRow` / `GitActivityHeader`.
 */
import { describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { GitActivityDock } from './GitActivityDock';
import type { GitActivityDockProps } from './GitActivityDock';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

function run(over: Partial<GitActivityRun> = {}): GitActivityRun {
  return {
    id: 'r1',
    category: 'fetch',
    phase: { kind: 'network' },
    status: 'running',
    code: null,
    startedAt: 1_000,
    endedAt: null,
    progress: null,
    hooks: [],
    lines: [],
    linesDropped: 0,
    seq: 0,
    target: 'origin',
    targetCount: null,
    outcome: null,
    ...over,
  };
}

function renderDock(over: Partial<GitActivityDockProps> = {}) {
  const props: GitActivityDockProps = {
    runs: [],
    activeRun: null,
    tick: 2_000,
    collapsed: false,
    onToggleCollapsed: vi.fn(),
    height: 180,
    onResizeHeight: vi.fn(),
    onClear: vi.fn(),
    hasTerminalRuns: false,
    density: 'cozy',
    ...over,
  };
  return render(<GitActivityDock {...props} />);
}

const bar = (c: HTMLElement) => c.querySelector<HTMLElement>('.git-dock-progress');

describe('GitActivityDock — the progress bar (§2)', () => {
  it('is determinate at received/total', () => {
    const r = run({
      progress: { receivedObjects: 50, totalObjects: 100, indexedObjects: 0, receivedBytes: 0 },
    });
    const { container } = renderDock({ runs: [r], activeRun: r });
    const el = bar(container);
    expect(el).toHaveAttribute('data-determinate', 'true');
    expect(el?.style.getPropertyValue('--progress')).toBe('0.5');
    expect(el).toHaveAttribute('aria-hidden', 'true');
  });

  it('clamps an overflowing fraction to 1', () => {
    const r = run({
      progress: { receivedObjects: 130, totalObjects: 100, indexedObjects: 0, receivedBytes: 0 },
    });
    const { container } = renderDock({ runs: [r], activeRun: r });
    expect(bar(container)?.style.getPropertyValue('--progress')).toBe('1');
  });

  it('is the indeterminate sweep without counts', () => {
    const r = run({ category: 'checkoutBranch', target: 'main', phase: { kind: 'preparing' } });
    const { container } = renderDock({ runs: [r], activeRun: r });
    const el = bar(container);
    expect(el).toBeInTheDocument();
    expect(el).not.toHaveAttribute('data-determinate');
    expect(el?.style.getPropertyValue('--progress')).toBe('');
  });

  it('is absent when nothing runs', () => {
    const r = run({ status: 'success', endedAt: 1_500 });
    const { container } = renderDock({ runs: [r], activeRun: null, hasTerminalRuns: true });
    expect(bar(container)).toBeNull();
  });
});

describe('GitActivityDock — P119 row states (§4)', () => {
  it('empty dock reads the new hint', () => {
    const r = run({ status: 'success', endedAt: 1_500 });
    const { rerender } = renderDock({ runs: [r] });
    // A Clear keeps the dock mounted and shows the empty state.
    rerender(
      <GitActivityDock
        runs={[]}
        activeRun={null}
        tick={2_000}
        collapsed={false}
        onToggleCollapsed={vi.fn()}
        height={180}
        onResizeHeight={vi.fn()}
        onClear={vi.fn()}
        hasTerminalRuns={false}
        density="cozy"
      />,
    );
    expect(
      screen.getByText('Every change Bonsai makes to this repository shows up here.'),
    ).toBeInTheDocument();
  });

  it('a conflicts run shows `! Conflicts` and no outcome detail', () => {
    const r = run({
      category: 'merge',
      target: 'feature/x',
      status: 'conflicts',
      outcome: 'conflicts',
      endedAt: 1_300,
    });
    const { container } = renderDock({ runs: [r], hasTerminalRuns: true });
    const pill = container.querySelector('.git-run-pill');
    expect(pill).toHaveAttribute('data-status', 'conflicts');
    expect(pill).toHaveTextContent('!Conflicts');
    expect(container.querySelector('.git-run-subphase')).toBeNull();
    expect(container.querySelector('.git-dock-status')).toHaveAttribute('data-status', 'conflicts');
  });

  it('a fast-forwarded merge shows the detail in the row and the collapsed bar', () => {
    const r = run({
      category: 'merge',
      target: 'feature/x',
      status: 'success',
      outcome: 'fastForwarded',
      endedAt: 1_200,
    });
    const { container } = renderDock({ runs: [r], hasTerminalRuns: true });
    expect(container.querySelector('.git-run-subphase')).toHaveTextContent('Fast-forwarded');
    expect(container.querySelector('.git-dock-detail')).toHaveTextContent('· Fast-forwarded');
  });

  it('a multi-target run puts the count noun in the noun slot and no target', () => {
    const r = run({
      category: 'deleteBranches',
      target: null,
      targetCount: 3,
      status: 'success',
      endedAt: 1_100,
    });
    const { container } = renderDock({ runs: [r], hasTerminalRuns: true });
    expect(container.querySelector('.git-run-noun')).toHaveTextContent('Delete 3 branches');
    expect(container.querySelector('.git-run-target')).toBeNull();
    expect(container.querySelector('.git-dock-noun')).toHaveTextContent('Delete 3 branches');
  });

  it('the blocking-hook note names the category, not `commit`', () => {
    const r = run({
      category: 'rebase',
      target: 'main',
      status: 'failed',
      endedAt: 1_500,
      hooks: [{ hook: 'pre-rebase', code: 1, success: false, at: 1_200 }],
    });
    const { container } = renderDock({ runs: [r], hasTerminalRuns: true });
    const summary = container.querySelector<HTMLElement>('[data-run-row]');
    if (summary === null) throw new Error('row missing');
    fireEvent.click(summary);
    expect(screen.getByText(/This hook blocked the rebase\./)).toBeInTheDocument();
  });
});
