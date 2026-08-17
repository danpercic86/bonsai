/**
 * P68e §13.1 — proposal DISCOVERABILITY (§5.1) and multi-run behaviour (§1.6).
 *
 * Split out of `AiActivityPanel.test.tsx` when it crossed the ~500-line limit; the
 * fixtures live in `src/test/aiDockKit.tsx`.
 *
 * The load-bearing test here is the M1 pair: the dock is one of four redundant paths to
 * a finished proposal, and FOLD-IN 1 deliberately does NOT open the pane when the user
 * navigated away — so the hint must say which of the two actually happened. Claiming
 * "open in the center pane" when nothing was opened is the exact bug class P68 exists to
 * eliminate ("I don't see no proposals").
 */
import { describe, expect, it } from 'vitest';
import { fireEvent, screen } from '@testing-library/react';

import { HINT_NOT_OPENED, HINT_OPENED } from './AiActivityPanel';
import { mount, run } from '../test/aiDockKit';
import { AI_MAX_CONCURRENT_RUNS } from '../settings/ranges';

// ---------------------------------------------------------------- 13, 14

describe('bulk runs and proposal discoverability (§5.1)', () => {
  const bulk = run({
    key: 'bulk:1',
    label: '3 conflicts',
    status: 'ready',
    paths: ['a.json', 'b.json', 'c.json'],
    files: [
      { path: 'a.json', status: 'ready', error: null },
      { path: 'b.json', status: 'pending', error: null },
      { path: 'c.json', status: 'failed', error: 'no result block returned' },
    ],
  });

  it('one queue row per file, with Review/Retry only where they apply', () => {
    const { container, p } = mount({ runs: [bulk] });
    expect(container.querySelectorAll('.ai-run-queue-row')).toHaveLength(3);

    fireEvent.click(screen.getByRole('button', { name: 'Review AI proposal for a.json' }));
    expect(p.onReviewFile).toHaveBeenCalledWith('bulk:1', 'a.json');

    fireEvent.click(screen.getByRole('button', { name: 'Retry AI resolution for c.json' }));
    expect(p.onRetryFile).toHaveBeenCalledWith('bulk:1', 'c.json');

    // A pending file offers neither.
    expect(screen.queryByRole('button', { name: /for b\.json/ })).toBeNull();
    const reason = screen.getByText('no result block returned');
    expect(reason).toHaveAttribute('title', 'no result block returned');
  });

  it('a single ready run gets the header Review proposal button; a bulk run does not', () => {
    const { p, unmount } = mount({ runs: [run({ status: 'ready', openedInPane: true })] });
    fireEvent.click(screen.getByRole('button', { name: 'Review proposal' }));
    expect(p.onReviewFile).toHaveBeenCalledWith(run().key, 'src/locales/de.json');
    // §5.1-3: the hint tells the user WHERE it went — the original complaint.
    expect(screen.getByText(HINT_OPENED)).toBeInTheDocument();
    unmount();

    mount({ runs: [bulk] });
    expect(screen.queryByRole('button', { name: 'Review proposal' })).toBeNull();
  });

  /**
   * M1 — THE BRANCH FOLD-IN 1 CREATED. When the user navigated away the proposal is
   * deliberately NOT opened, and the toast says "review it from the AI activity dock".
   * The dock claiming `Proposal is open in the center pane.` in that state is the exact
   * bug class P68 exists to eliminate ("I don't see no proposals" — being told a result
   * is somewhere it isn't), so the suppressed branch gets its own copy and its own test.
   */
  it('says how to OPEN the proposal when the auto-open was suppressed (FOLD-IN 1)', () => {
    const { unmount } = mount({ runs: [run({ status: 'ready', openedInPane: false })] });
    expect(screen.getByText(HINT_NOT_OPENED)).toBeInTheDocument();
    expect(screen.queryByText(HINT_OPENED)).toBeNull();
    // The affordance the hint names is really there.
    expect(screen.getByRole('button', { name: 'Review proposal' })).toBeInTheDocument();
    expect(HINT_NOT_OPENED).not.toContain('is open in the center pane');
    unmount();

    // A live run has no hint at all — it has nothing to point at yet.
    mount({ runs: [run({ status: 'running' })] });
    expect(screen.queryByText(HINT_NOT_OPENED)).toBeNull();
    expect(screen.queryByText(HINT_OPENED)).toBeNull();
  });

  it('a failed run explains that nothing changed', () => {
    mount({ runs: [run({ status: 'failed', error: 'Claude exited without a result' })] });
    expect(screen.getByRole('alert').textContent).toContain('Claude exited without a result');
    expect(
      screen.getByText('Nothing was changed. You can retry, or resolve this file by hand.'),
    ).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------- 16, 17

describe('several runs at once', () => {
  const two = [
    run({ key: 'conflict:a.json', label: 'a.json' }),
    run({ key: 'conflict:b.json', label: 'b.json', status: 'ready' }),
  ];

  it('renders a tablist with exactly one selected tab, and arrows move selection AND focus', () => {
    const { p } = mount({ runs: two, activeKey: 'conflict:a.json' });
    const tabs = screen.getAllByRole('tab');
    expect(tabs).toHaveLength(2);
    expect(tabs.filter((t) => t.getAttribute('aria-selected') === 'true')).toHaveLength(1);
    // Roving tabindex: only the selected chip is in the tab order.
    expect(tabs[0]).toHaveAttribute('tabindex', '0');
    expect(tabs[1]).toHaveAttribute('tabindex', '-1');
    tabs[0]!.focus();
    fireEvent.keyDown(tabs[0]!, { key: 'ArrowRight' });
    expect(p.onSelectRun).toHaveBeenCalledWith('conflict:b.json');
    // The ARIA tabs pattern: focus FOLLOWS selection. Leaving focus on the old chip
    // while it drops to tabIndex=-1 desyncs the focus ring from aria-selected and
    // sends the next Tab from the wrong place.
    expect(document.activeElement).toBe(tabs[1]);

    fireEvent.keyDown(tabs[1]!, { key: 'Home' });
    expect(p.onSelectRun).toHaveBeenLastCalledWith('conflict:a.json');
    expect(document.activeElement).toBe(tabs[0]);
  });

  it('chips use the pill glyph set and the basename (U8 / §1.6)', () => {
    const { container } = mount({
      runs: [
        run({ key: 'conflict:src/a.json', label: 'src/a.json', status: 'failed' }),
        run({ key: 'conflict:src/b.json', label: 'src/b.json', status: 'cancelled' }),
        run({ key: 'bulk:1', label: '3 conflicts', status: 'ready' }),
      ],
      activeKey: 'bulk:1',
    });
    const chips = [...container.querySelectorAll('.ai-dock-run-chip')];
    // ✨ would say "still working" for a run that is anything but.
    expect(chips.map((c) => c.textContent)).toEqual(['⚠ a.json', '⊘ b.json', '✓ 3 conflicts']);
    expect(chips[0]).toHaveAttribute('title', 'src/a.json');
  });

  it('the collapsed bar aggregates: N AI runs and the most urgent status', () => {
    const { container } = mount({
      collapsed: true,
      runs: [two[0]!, run({ key: 'c', label: 'c.json', status: 'awaitingInput' })],
    });
    expect(container.querySelector('.ai-dock-subject')?.textContent).toBe('2 AI runs');
    expect(container.querySelector('.ai-dock-status')?.textContent).toContain('Needs you');
    expect(container.querySelector('.ai-dock')).toHaveAttribute('data-attention', 'true');
  });

  it('shows the concurrency counter at capacity', () => {
    mount({ runs: two, atCapacity: true });
    expect(
      screen.getByText(`${AI_MAX_CONCURRENT_RUNS} of ${AI_MAX_CONCURRENT_RUNS} running`),
    ).toBeInTheDocument();
  });
});
