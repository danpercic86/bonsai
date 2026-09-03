/** P87b FU-3 — the git-activity dock row's disclosure semantics.
 *
 *  The row's roving focus target used to be a role-less `<div>` with the
 *  `aria-expanded` state parked on a nested chevron button, so a screen reader
 *  announced neither that the row was actionable nor whether it was open.
 *  These tests pin the fixed contract: the roving element IS the button, it
 *  carries the expanded state, it answers Enter AND Space, and the chevron is
 *  decorative (hidden from AT and out of the tab order).
 *
 *  Also covers the dock's mount latch (the former render-phase ref write): once
 *  a run has been seen the dock stays mounted even after Clear empties `runs`,
 *  and that must hold under StrictMode's double render.
 */
import { StrictMode } from 'react';
import { describe, expect, it } from 'vitest';
import { fireEvent, render, screen } from '@testing-library/react';

import { GitActivityDock } from './GitActivityDock';
import { GitActivityRow } from './GitActivityRow';
import type { GitActivityRun } from './repoWorkspace/useGitActivity';

function run(over: Partial<GitActivityRun> = {}): GitActivityRun {
  return {
    id: 'run-1',
    category: 'push',
    phase: { kind: 'network' },
    status: 'success',
    code: 0,
    startedAt: 1_000,
    endedAt: 3_400,
    progress: null,
    hooks: [],
    lines: [{ seq: 0, stream: 'stdout', text: 'Everything up-to-date' }],
    linesDropped: 0,
    seq: 4,
    ...over,
  };
}

/** The roving focus target — the element the dock's ArrowUp/Down moves focus to. */
function rovingRow(): HTMLElement {
  const el = document.querySelector<HTMLElement>('[data-run-row]');
  if (el === null) throw new Error('no [data-run-row] rendered');
  return el;
}

describe('GitActivityRow disclosure semantics (FU-3)', () => {
  it('puts role=button + aria-expanded on the roving focus target itself', () => {
    render(<GitActivityRow run={run()} tick={5_000} />);

    const row = rovingRow();
    expect(row.getAttribute('role')).toBe('button');
    expect(row.getAttribute('aria-expanded')).toBe('false');
    // The roving element is the one queryable as a button — not a wrapper.
    expect(screen.getByRole('button', { expanded: false })).toBe(row);
  });

  it('flips aria-expanded on the same element when opened', () => {
    render(<GitActivityRow run={run()} tick={5_000} />);

    fireEvent.click(rovingRow());

    expect(rovingRow().getAttribute('aria-expanded')).toBe('true');
    expect(screen.getByRole('button', { expanded: true })).toBe(rovingRow());
  });

  it.each([
    ['Enter', 'Enter'],
    ['Space', ' '],
  ])('toggles on %s (role=button gets neither key for free)', (_label, key) => {
    render(<GitActivityRow run={run()} tick={5_000} />);

    fireEvent.keyDown(rovingRow(), { key });
    expect(rovingRow().getAttribute('aria-expanded')).toBe('true');

    fireEvent.keyDown(rovingRow(), { key });
    expect(rovingRow().getAttribute('aria-expanded')).toBe('false');
  });

  it('hides the chevron from AT and keeps it out of the tab order', () => {
    render(<GitActivityRow run={run()} tick={5_000} />);

    const chevron = document.querySelector<HTMLElement>('.git-run-chevron');
    expect(chevron).not.toBeNull();
    expect(chevron?.getAttribute('aria-hidden')).toBe('true');
    // An aria-hidden element must never be focusable.
    expect(chevron?.getAttribute('tabindex')).toBe('-1');
    // Exactly one exposed button in the collapsed row: the row itself.
    expect(screen.getAllByRole('button')).toHaveLength(1);
  });

  it('still toggles when the (decorative) chevron is clicked', () => {
    render(<GitActivityRow run={run()} tick={5_000} />);

    const chevron = document.querySelector<HTMLElement>('.git-run-chevron');
    if (chevron === null) throw new Error('no chevron');
    fireEvent.click(chevron);

    expect(rovingRow().getAttribute('aria-expanded')).toBe('true');
  });
});

describe('GitActivityDock mount latch (render purity)', () => {
  const dockProps = {
    activeRun: null,
    tick: 5_000,
    collapsed: false,
    onToggleCollapsed: () => {},
    height: 200,
    onResizeHeight: () => {},
    onClear: () => {},
    hasTerminalRuns: true,
    density: 'cozy' as const,
  };

  it('renders nothing before the first run, then stays mounted after Clear', () => {
    const { rerender } = render(
      <StrictMode>
        <GitActivityDock runs={[]} {...dockProps} />
      </StrictMode>,
    );
    expect(screen.queryByRole('region', { name: 'Git activity' })).toBeNull();

    // First run arrives: the dock must appear on THIS render, not one later.
    rerender(
      <StrictMode>
        <GitActivityDock runs={[run()]} {...dockProps} />
      </StrictMode>,
    );
    expect(screen.getByRole('region', { name: 'Git activity' })).not.toBeNull();

    // Clear empties the list; the region (and its live region) survives.
    rerender(
      <StrictMode>
        <GitActivityDock runs={[]} {...dockProps} />
      </StrictMode>,
    );
    expect(screen.getByRole('region', { name: 'Git activity' })).not.toBeNull();
    expect(screen.getByText('No git activity yet.')).not.toBeNull();
  });
});
