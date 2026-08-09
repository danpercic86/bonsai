/** T3.7 — DiffBrowser: the stacked all-files diff surface. Covers the wiring
 *  RepoWorkspace drives (header title for commit vs compare, File/Diff toggle,
 *  collapse-all, close, scope filtering, tree-vs-flat ordering, per-card
 *  collapse, binary placeholder, empty state). Per-file hunk fetches are kept
 *  pending under fake timers so the suite is deterministic (no reliance on the
 *  mock diff fixtures resolving). */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, within } from '@testing-library/react';
import { DiffBrowser, type DiffBrowserProps, type DiffBrowserSource } from './DiffBrowser';
import type { FileDiffHeader } from '../ipc';

function hdr(over: Partial<FileDiffHeader> = {}): FileDiffHeader {
  return {
    path: 'a.txt',
    origPath: null,
    status: 'modified',
    additions: 3,
    deletions: 1,
    binary: false,
    ...over,
  };
}

const commitSource: DiffBrowserSource = { mode: 'commit', oid: 'a'.repeat(40), title: 'feat: x' };

function props(over: Partial<DiffBrowserProps> = {}): DiffBrowserProps {
  return {
    repoId: '/mock/repo',
    source: commitSource,
    files: [hdr()],
    scope: { kind: 'root' },
    listView: 'flat',
    onClose: vi.fn(),
    ...over,
  };
}

// Keep the per-file diff fetches (setTimeout-delayed mock) pending: cards stay
// in the skeleton state, so nothing depends on the mock resolving.
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.runOnlyPendingTimers();
  vi.useRealTimers();
});

const cards = () => document.querySelectorAll('.diff-card');

describe('DiffBrowser header', () => {
  it('renders a single commit title', () => {
    render(<DiffBrowser {...props()} />);
    expect(screen.getByRole('region', { name: 'All changes' })).toBeInTheDocument();
    expect(screen.getByText('feat: x')).toBeInTheDocument();
  });

  it('renders from → to endpoints for a compare source', () => {
    render(
      <DiffBrowser
        {...props({
          source: { mode: 'compare', oid: 'b'.repeat(40), fromLabel: 'main', toLabel: 'dev' },
        })}
      />,
    );
    expect(screen.getByText('main')).toBeInTheDocument();
    expect(screen.getByText('dev')).toBeInTheDocument();
  });

  it('close button fires onClose', () => {
    const p = props();
    render(<DiffBrowser {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Close all-changes view' }));
    expect(p.onClose).toHaveBeenCalledTimes(1);
  });
});

describe('DiffBrowser File/Diff toggle', () => {
  it('defaults to Diff and flips aria-pressed when File is picked', () => {
    render(<DiffBrowser {...props()} />);
    const group = screen.getByRole('group', { name: 'View mode' });
    expect(within(group).getByRole('button', { name: 'Diff' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    fireEvent.click(within(group).getByRole('button', { name: 'File' }));
    expect(within(group).getByRole('button', { name: 'File' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
  });
});

describe('DiffBrowser collapse controls', () => {
  it('collapse-all flips label and unmounts every card body; expand-all restores', () => {
    render(<DiffBrowser {...props({ files: [hdr({ path: 'a.txt' }), hdr({ path: 'b.txt' })] })} />);
    expect(document.querySelectorAll('.diff-card-body')).toHaveLength(2);
    fireEvent.click(screen.getByRole('button', { name: 'Collapse all' }));
    expect(document.querySelectorAll('.diff-card-body')).toHaveLength(0);
    fireEvent.click(screen.getByRole('button', { name: 'Expand all' }));
    expect(document.querySelectorAll('.diff-card-body')).toHaveLength(2);
  });

  it('a single card header toggles only its own body', () => {
    render(<DiffBrowser {...props({ files: [hdr({ path: 'a.txt' }), hdr({ path: 'b.txt' })] })} />);
    const firstHeader = screen.getByRole('button', { name: /a\.txt/ });
    expect(firstHeader).toHaveAttribute('aria-expanded', 'true');
    fireEvent.click(firstHeader);
    expect(firstHeader).toHaveAttribute('aria-expanded', 'false');
    // The other card stays expanded.
    expect(document.querySelectorAll('.diff-card-body')).toHaveLength(1);
  });
});

describe('DiffBrowser scope filtering', () => {
  const files = [
    hdr({ path: 'src/a.ts' }),
    hdr({ path: 'src/b.ts' }),
    hdr({ path: 'README.md' }),
  ];

  it('root scope renders every file', () => {
    render(<DiffBrowser {...props({ files })} />);
    expect(cards()).toHaveLength(3);
  });

  it('dir scope renders only files under the prefix', () => {
    render(<DiffBrowser {...props({ files, scope: { kind: 'dir', prefix: 'src' } })} />);
    expect(cards()).toHaveLength(2);
    expect(screen.queryByText('README.md')).not.toBeInTheDocument();
  });

  it('file scope renders exactly the one file', () => {
    render(<DiffBrowser {...props({ files, scope: { kind: 'file', path: 'README.md' } })} />);
    expect(cards()).toHaveLength(1);
    expect(screen.getByText('README.md')).toBeInTheDocument();
  });

  it('a scope with no matching files shows the empty state', () => {
    render(<DiffBrowser {...props({ files, scope: { kind: 'dir', prefix: 'nope' } })} />);
    expect(screen.getByText('No changes')).toBeInTheDocument();
    expect(cards()).toHaveLength(0);
  });
});

describe('DiffBrowser per-card body', () => {
  it('a binary (non-image) file renders the placeholder, never a skeleton fetch', () => {
    render(<DiffBrowser {...props({ files: [hdr({ path: 'blob.bin', binary: true })] })} />);
    expect(screen.getByText('Binary file')).toBeInTheDocument();
  });

  it('a non-binary file shows the loading skeleton while its fetch is pending', () => {
    render(<DiffBrowser {...props({ files: [hdr({ path: 'a.txt' })] })} />);
    expect(document.querySelector('.diff-card-loading')).toBeInTheDocument();
  });
});

describe('DiffBrowser tree ordering', () => {
  it('tree view reorders cards to dirs-first leaf order', () => {
    const files = [hdr({ path: 'z.txt' }), hdr({ path: 'src/a.ts' })];
    render(<DiffBrowser {...props({ files, listView: 'tree' })} />);
    const paths = Array.from(document.querySelectorAll('.diff-card-path')).map(
      (el) => el.textContent,
    );
    // Directory group (src/a.ts) sorts before the root-level file (z.txt).
    expect(paths).toEqual(['src/a.ts', 'z.txt']);
  });
});
