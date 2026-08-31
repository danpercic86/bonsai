/** P93: the PR changed-files list is a plain OPEN-in-the-center-overlay list.
 *  Pinned here: no inline body, no Expand/Collapse all, one active row marker,
 *  binary rows are inert, and the §6.1 dismissal-token focus restore (fires once
 *  per token; only when the row still exists AND focus fell back to <body>). */
import { describe, expect, it, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import type { FileDiffHeader, PrDiffStats } from '../../ipc';
import { PrChangesSection } from './PrChangesSection';
import type { PrChangesSectionProps } from './PrChangesSection';

const HEADERS: FileDiffHeader[] = [
  { path: 'src/app.ts', origPath: null, status: 'modified', additions: 3, deletions: 1, binary: false },
  { path: 'assets/logo.png', origPath: null, status: 'modified', additions: 0, deletions: 0, binary: true },
  { path: 'docs/a:b.md', origPath: null, status: 'added', additions: 2, deletions: 0, binary: false },
];

const STATS: PrDiffStats = {
  additions: 5,
  deletions: 1,
  changedFiles: HEADERS.length,
  mergeBaseOid: 'a'.repeat(40),
  baseOid: 'b'.repeat(40),
  headOid: 'c'.repeat(40),
  files: HEADERS,
};

function renderSection(over: Partial<PrChangesSectionProps> = {}) {
  const onOpenFile = vi.fn();
  const props: PrChangesSectionProps = {
    status: 'ready',
    stats: STATS,
    stale: false,
    errorCause: 'generic',
    onRetry: vi.fn(),
    activePath: null,
    restoreFocusTo: null,
    onOpenFile,
    ...over,
  };
  const view = render(<PrChangesSection {...props} />);
  return { onOpenFile, view, props };
}

describe('PrChangesSection — P93 center-overlay rows', () => {
  it('renders no Expand/Collapse all control and keeps the file count', () => {
    renderSection();
    expect(screen.queryByRole('button', { name: /Expand all|Collapse all/ })).toBeNull();
    expect(screen.getByText('3 files')).toBeInTheDocument();
  });

  it('clicking a row asks the container to open it — no inline diff body', () => {
    const { onOpenFile } = renderSection();
    fireEvent.click(screen.getByRole('button', { name: /src\/app\.ts/ }));
    expect(onOpenFile).toHaveBeenCalledTimes(1);
    expect(onOpenFile).toHaveBeenCalledWith(HEADERS[0]);
    expect(document.querySelector('.diff-card-body')).toBeNull();
  });

  it('marks exactly the active row, with aria-expanded', () => {
    renderSection({ activePath: 'docs/a:b.md' });
    const active = document.querySelectorAll('.pr-file-row-active');
    expect(active).toHaveLength(1);
    expect(active[0]?.textContent).toContain('docs/a:b.md');
    expect(screen.getByRole('button', { name: /docs\/a:b\.md/ })).toHaveAttribute(
      'aria-expanded',
      'true',
    );
    expect(screen.getByRole('button', { name: /src\/app\.ts/ })).toHaveAttribute(
      'aria-expanded',
      'false',
    );
  });

  it('a binary row is a non-button with the bin chip and no fetch affordance', () => {
    const { onOpenFile } = renderSection();
    expect(screen.queryByRole('button', { name: /logo\.png/ })).toBeNull();
    const row = document.querySelector('.pr-file-row-binary');
    expect(row?.tagName).toBe('SPAN');
    expect(row?.getAttribute('title')).toBe('Binary file — no text diff');
    expect(row?.querySelector('.file-count-bin')?.textContent).toBe('bin');
    if (row instanceof HTMLElement) fireEvent.click(row);
    expect(onOpenFile).not.toHaveBeenCalled();
  });

  it('restores focus to the opener row on a dismissal token', () => {
    const { view, props } = renderSection({ activePath: 'src/app.ts' });
    view.rerender(
      <PrChangesSection
        {...props}
        activePath={null}
        restoreFocusTo={{ path: 'src/app.ts', token: 1 }}
      />,
    );
    expect(document.activeElement).toBe(screen.getByRole('button', { name: /src\/app\.ts/ }));
  });

  it('does not restore focus on an activePath transition alone (C5 slot replacement)', () => {
    const { view, props } = renderSection({ activePath: 'src/app.ts' });
    view.rerender(<PrChangesSection {...props} activePath={null} />);
    expect(document.activeElement).toBe(document.body);
  });

  it('does not steal focus when the opener row is gone (PR switch / head advance)', () => {
    const { view, props } = renderSection({ activePath: 'src/gone.ts' });
    view.rerender(
      <PrChangesSection
        {...props}
        activePath={null}
        restoreFocusTo={{ path: 'src/gone.ts', token: 4 }}
      />,
    );
    expect(document.activeElement).toBe(document.body);
  });

  it('does not take focus back when the user already focused something else', () => {
    const { view, props } = renderSection({ activePath: 'src/app.ts' });
    const elsewhere = document.createElement('button');
    document.body.appendChild(elsewhere);
    elsewhere.focus();
    view.rerender(
      <PrChangesSection
        {...props}
        activePath={null}
        restoreFocusTo={{ path: 'src/app.ts', token: 7 }}
      />,
    );
    expect(document.activeElement).toBe(elsewhere);
    elsewhere.remove();
  });

  it('fires once per token — a re-render with the same token does not re-focus', () => {
    const { view, props } = renderSection({ activePath: 'src/app.ts' });
    const restoreFocusTo = { path: 'src/app.ts', token: 2 };
    view.rerender(<PrChangesSection {...props} activePath={null} restoreFocusTo={restoreFocusTo} />);
    const row = screen.getByRole('button', { name: /src\/app\.ts/ });
    expect(document.activeElement).toBe(row);
    row.blur();
    view.rerender(
      <PrChangesSection {...props} activePath={null} restoreFocusTo={{ ...restoreFocusTo }} />,
    );
    expect(document.activeElement).toBe(document.body);
  });

  it('a mount-time token is treated as already handled (remount does not steal focus)', () => {
    renderSection({ activePath: null, restoreFocusTo: { path: 'src/app.ts', token: 9 } });
    expect(document.activeElement).toBe(document.body);
  });
});
