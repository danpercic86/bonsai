/**
 * P68f — the two entry points (OQ4) and the one button they share.
 *
 * These assertions are about WHERE the affordance appears and WHEN it is usable,
 * because that is the contract: the conflicts-section header and the MERGE banner
 * only, with ≥2 AI-eligible conflicts, respecting `aiEligible` and the concurrency
 * cap exactly as the per-row ✨AI button does.
 */
import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';

import { BulkAiResolveButton } from './BulkAiResolveButton';
import { OpBanner } from './OpBanner';
import { StatusConflictsSection } from './StatusConflictsSection';
import type { BulkAiControl } from './repoWorkspace/useBulkAiResolve';
import type { ConflictEntry, RepoOpState, StatusEntry } from '../ipc';

function control(over: Partial<BulkAiControl> = {}): BulkAiControl {
  return {
    shown: true,
    paths: ['src/auth.ts', 'src/locales/de.json'],
    count: 2,
    active: false,
    disabled: false,
    label: '✨ Resolve all with AI',
    title: 'Resolve all 2 conflicted files in ONE AI run',
    ariaLabel: 'Resolve all 2 conflicts with AI',
    onClick: vi.fn(),
    ...over,
  };
}

const CANCEL = {
  active: true,
  label: 'Cancel all',
  title: 'Stop the one AI run covering all 2 files',
  ariaLabel: 'Cancel the AI run for all 2 files',
} satisfies Partial<BulkAiControl>;

describe('BulkAiResolveButton', () => {
  it('renders nothing when the control is not shown', () => {
    render(<BulkAiResolveButton control={control({ shown: false })} variant="section" />);
    expect(screen.queryByRole('button')).not.toBeInTheDocument();
  });

  it('carries the copy, the title and the aria-label from the control', () => {
    render(<BulkAiResolveButton control={control()} variant="section" />);
    const button = screen.getByRole('button', { name: 'Resolve all 2 conflicts with AI' });
    expect(button).toHaveTextContent('✨ Resolve all with AI');
    expect(button).toHaveAttribute('title', 'Resolve all 2 conflicted files in ONE AI run');
    expect(button).toHaveAttribute('data-state', 'idle');
  });

  it('becomes the danger-styled Cancel all in the banner while a run is live', () => {
    render(<BulkAiResolveButton control={control(CANCEL)} variant="banner" />);
    const button = screen.getByRole('button', { name: 'Cancel the AI run for all 2 files' });
    expect(button).toHaveTextContent('Cancel all');
    expect(button).toHaveAttribute('data-state', 'cancel');
    expect(button.className).toContain('btn-danger');
  });

  it('a busy section blocks STARTING a run but never blocks cancelling one', () => {
    const { unmount } = render(
      <BulkAiResolveButton control={control()} variant="section" busy />,
    );
    expect(screen.getByRole('button')).toBeDisabled();
    unmount();
    render(<BulkAiResolveButton control={control(CANCEL)} variant="section" busy />);
    expect(screen.getByRole('button')).toBeEnabled();
  });
});

// ---------------------------------------------------------------- entry point 1

function entry(path: string): StatusEntry {
  return { path, origPath: null, status: 'conflicted' };
}
function conflict(path: string, kind: ConflictEntry['kind'] = 'bothModified'): ConflictEntry {
  return { path, kind, hasBase: true, hasOurs: true, hasTheirs: true };
}

function renderSection(aiBulk?: BulkAiControl) {
  return render(
    <StatusConflictsSection
      entries={[entry('src/auth.ts'), entry('src/locales/de.json')]}
      conflicts={[conflict('src/auth.ts'), conflict('src/locales/de.json')]}
      disabled={false}
      diffSlot={null}
      aiEligible
      aiRows={{}}
      aiAtCapacity={false}
      aiBulk={aiBulk}
      onResolveConflict={vi.fn()}
      onToggleConflictView={vi.fn()}
      onAiResolve={vi.fn()}
      onAiReview={vi.fn()}
    />,
  );
}

describe('P68f entry point 1 — the conflicts section header', () => {
  it('puts the button in the section header and fires the control', () => {
    const onClick = vi.fn();
    renderSection(control({ onClick }));
    const header = screen.getByText('Conflicts (2)').parentElement;
    const button = screen.getByRole('button', { name: 'Resolve all 2 conflicts with AI' });
    expect(header).toContainElement(button);
    fireEvent.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('adds nothing to the header when no control is passed (fewer than 2 eligible)', () => {
    renderSection(undefined);
    expect(screen.getByText('Conflicts (2)').parentElement?.querySelectorAll('button')).toHaveLength(
      0,
    );
  });
});

// ---------------------------------------------------------------- entry point 2

function renderBanner(op: RepoOpState, aiBulk?: BulkAiControl) {
  return render(
    <OpBanner
      op={op}
      conflictCount={2}
      mutating={false}
      onCommitMerge={vi.fn()}
      onRebaseContinue={vi.fn()}
      onRebaseSkip={vi.fn()}
      onOpContinue={vi.fn()}
      onAbort={vi.fn()}
      onBisectMark={vi.fn()}
      onBisectSkip={vi.fn()}
      aiBulk={aiBulk}
    />,
  );
}

describe('P68f entry point 2 — the merge banner', () => {
  it('sits in the merge actions row, before Commit merge', () => {
    renderBanner({ kind: 'merge', incoming: 'feature/i18n', message: 'Merge feature/i18n' }, control());
    expect(screen.getByText('Merging feature/i18n')).toBeInTheDocument();
    const labels = [...document.querySelectorAll('.op-banner-actions button')].map(
      (b) => b.textContent,
    );
    expect(labels).toEqual(['✨ Resolve all with AI', 'Commit merge', 'Abort']);
  });

  it('is NOT offered for rebase / cherry-pick / revert banners (merge arm only)', () => {
    const { unmount } = renderBanner(
      { kind: 'rebase', headName: 'main', onto: 'origin/main', currentStep: 1, totalSteps: 3 },
      control(),
    );
    expect(
      screen.queryByRole('button', { name: 'Resolve all 2 conflicts with AI' }),
    ).not.toBeInTheDocument();
    unmount();
    renderBanner({ kind: 'cherryPick' }, control());
    expect(
      screen.queryByRole('button', { name: 'Resolve all 2 conflicts with AI' }),
    ).not.toBeInTheDocument();
  });
});
