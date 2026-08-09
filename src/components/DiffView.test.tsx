/** T3.5 — DiffView + DiffViewSplit + intraline rendering: hunk/row layout from
 *  a fixture FileDiff, placeholder states, granular stage/discard wiring and
 *  its gating, unified-vs-split content parity, and word-level (intraline)
 *  emphasis. Pure renderers — no IPC. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DiffView, DiffSlotView } from './DiffView';
import type { FileDiff, Hunk } from '../ipc';

const HUNK: Hunk = {
  oldStart: 1,
  oldLines: 3,
  newStart: 1,
  newLines: 3,
  lines: [
    { kind: 'context', oldNo: 1, newNo: 1, content: 'alpha' },
    { kind: 'del', oldNo: 2, newNo: null, content: 'beta old' },
    { kind: 'add', oldNo: null, newNo: 2, content: 'beta new', spans: [[5, 3]] },
    { kind: 'context', oldNo: 3, newNo: 3, content: 'gamma', noNewline: true },
  ],
};

// .txt has no registered grammar -> highlighter stays null -> plain text spans.
function fixture(over: Partial<FileDiff> = {}): FileDiff {
  return {
    path: 'notes.txt',
    origPath: null,
    status: 'modified',
    binary: false,
    tooLarge: false,
    hunks: [HUNK],
    ...over,
  };
}

const contentsOf = (root: ParentNode): string[] =>
  Array.from(root.querySelectorAll('.diff-content')).map((el) => el.textContent ?? '');

describe('DiffView (unified)', () => {
  it('renders the hunk header and every line with old/new numbers + markers', () => {
    const { container } = render(<DiffView diff={fixture()} />);
    expect(screen.getByText('@@ -1,3 +1,3 @@')).toBeInTheDocument();
    expect(contentsOf(container)).toEqual([
      'alpha',
      'beta old',
      'beta new',
      'gamma',
      '\\ No newline at end of file',
    ]);
    const del = container.querySelector('.diff-line-del')!;
    expect(del.querySelectorAll('.diff-lineno')[0]).toHaveTextContent('2');
    expect(del.querySelectorAll('.diff-lineno')[1].textContent).toBe('');
    expect(del.querySelector('.diff-marker')).toHaveTextContent('−');
    const add = container.querySelector('.diff-line-add')!;
    expect(add.querySelectorAll('.diff-lineno')[1]).toHaveTextContent('2');
    expect(add.querySelector('.diff-marker')).toHaveTextContent('+');
  });

  it('placeholders: binary, too-large, and empty diffs', () => {
    render(<DiffView diff={fixture({ binary: true })} />);
    expect(screen.getByText('Binary file')).toBeInTheDocument();
    render(<DiffView diff={fixture({ tooLarge: true, hunks: [] })} />);
    expect(screen.getByText(/Diff too large/)).toBeInTheDocument();
    render(<DiffView diff={fixture({ hunks: [] })} />);
    expect(screen.getByText('No changes')).toBeInTheDocument();
  });

  it('read-only (stageable=null) renders no gutter or hunk buttons', () => {
    const { container } = render(<DiffView diff={fixture()} />);
    expect(container.querySelector('.diff-gutter-btn')).not.toBeInTheDocument();
    expect(container.querySelector('.diff-hunk-stage-btn')).not.toBeInTheDocument();
  });

  it('gutter + stages exactly one line; hunk button forwards its index', () => {
    const onStageLines = vi.fn();
    const onStageHunk = vi.fn();
    render(
      <DiffView diff={fixture()} stageable="stage" onStageLines={onStageLines} onStageHunk={onStageHunk} />,
    );
    const btns = screen.getAllByRole('button', { name: 'Stage this line' });
    expect(btns).toHaveLength(2); // one per changed line
    fireEvent.click(btns[0]);
    expect(onStageLines).toHaveBeenCalledWith([{ kind: 'del', oldNo: 2, newNo: null }]);
    fireEvent.click(screen.getByRole('button', { name: 'Stage hunk' }));
    expect(onStageHunk).toHaveBeenCalledWith(0);
  });

  it('unstage direction flips labels and never offers discard controls', () => {
    const onDiscardLines = vi.fn();
    const onDiscardHunk = vi.fn();
    render(
      <DiffView
        diff={fixture()}
        stageable="unstage"
        onStageLines={vi.fn()}
        onDiscardLines={onDiscardLines}
        onDiscardHunk={onDiscardHunk}
      />,
    );
    expect(screen.getAllByRole('button', { name: 'Unstage this line' })).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Unstage hunk' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Discard this line' })).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Discard hunk' })).not.toBeInTheDocument();
  });

  it('discard controls render for unstaged (stage-direction) diffs and call back', () => {
    const onDiscardLines = vi.fn();
    const onDiscardHunk = vi.fn();
    render(
      <DiffView
        diff={fixture()}
        stageable="stage"
        onStageLines={vi.fn()}
        onDiscardLines={onDiscardLines}
        onDiscardHunk={onDiscardHunk}
      />,
    );
    const discards = screen.getAllByRole('button', { name: 'Discard this line' });
    expect(discards).toHaveLength(2);
    fireEvent.click(discards[1]);
    expect(onDiscardLines).toHaveBeenCalledWith([{ kind: 'add', oldNo: null, newNo: 2 }]);
    fireEvent.click(screen.getByRole('button', { name: 'Discard hunk' }));
    expect(onDiscardHunk).toHaveBeenCalledWith(0);
  });

  it('file view renders all lines without @@ headers', () => {
    const { container } = render(<DiffView diff={fixture()} viewMode="file" />);
    expect(container.querySelector('.diff-hunk-header')).not.toBeInTheDocument();
    expect(contentsOf(container)).toContain('beta new');
  });

  it('intraline: spans render emphasized segments instead of plain content', () => {
    const { container } = render(<DiffView diff={fixture()} intraline />);
    const marks = container.querySelectorAll('.diff-intra');
    expect(marks).toHaveLength(1); // only the add line carries spans
    expect(marks[0]).toHaveClass('diff-intra-add');
    expect(marks[0]).toHaveTextContent('new');
    // Toggle off -> no emphasis markup.
    const off = render(<DiffView diff={fixture()} />);
    expect(off.container.querySelector('.diff-intra')).not.toBeInTheDocument();
  });
});

describe('DiffView (split) vs unified parity', () => {
  it('split renders the same content set; del lands left, add lands right', () => {
    const unified = render(<DiffView diff={fixture()} />);
    const split = render(<DiffView diff={fixture()} viewMode="split" />);
    const unifiedSet = new Set(contentsOf(unified.container).filter((t) => !t.includes('No newline')));
    const left = split.container.querySelector('.diff-split-pane-left')!;
    const right = split.container.querySelector('.diff-split-pane-right')!;
    const splitSet = new Set(
      [...contentsOf(left), ...contentsOf(right)].map((t) => t.replace(/ \\ No newline.*$/, '')),
    );
    expect(splitSet).toEqual(unifiedSet);
    expect(left.textContent).toContain('beta old');
    expect(left.textContent).not.toContain('beta new');
    expect(right.textContent).toContain('beta new');
    expect(right.textContent).not.toContain('beta old');
    // Tinting: del tints only the left cell, add only the right.
    expect(left.querySelector('.diff-line-del')).toBeInTheDocument();
    expect(left.querySelector('.diff-line-add')).not.toBeInTheDocument();
    expect(right.querySelector('.diff-line-add')).toBeInTheDocument();
    expect(right.querySelector('.diff-line-del')).not.toBeInTheDocument();
  });

  it('split pairs del/add on one row and fills the unmatched side', () => {
    const { container } = render(<DiffView diff={fixture()} viewMode="split" />);
    // 4 unified rows pair down to 3 split rows -> each pane has 3 cells + fillers.
    const leftCells = container.querySelectorAll('.diff-split-pane-left .diff-split-cell');
    const rightCells = container.querySelectorAll('.diff-split-pane-right .diff-split-cell');
    expect(leftCells).toHaveLength(rightCells.length);
    expect(container.querySelectorAll('.diff-split-filler')).toHaveLength(0); // del+add pair: no filler
  });

  it('split renders the hunk header in BOTH panes but buttons only on the left', () => {
    const { container } = render(
      <DiffView diff={fixture()} viewMode="split" stageable="stage" onStageLines={vi.fn()} onStageHunk={vi.fn()} />,
    );
    expect(container.querySelectorAll('.diff-hunk-header')).toHaveLength(2);
    expect(
      container.querySelectorAll('.diff-split-pane-left .diff-hunk-stage-btn'),
    ).toHaveLength(1);
    expect(
      container.querySelectorAll('.diff-split-pane-right .diff-hunk-stage-btn'),
    ).toHaveLength(0);
  });

  it('split intraline matches the unified emphasis', () => {
    const { container } = render(<DiffView diff={fixture()} viewMode="split" intraline />);
    const marks = container.querySelectorAll('.diff-intra');
    expect(marks).toHaveLength(1);
    expect(marks[0]).toHaveTextContent('new');
  });

  it('split per-cell stage button uses the same selection payload as unified', () => {
    const onStageLines = vi.fn();
    render(
      <DiffView diff={fixture()} viewMode="split" stageable="stage" onStageLines={onStageLines} onStageHunk={vi.fn()} />,
    );
    const btns = screen.getAllByRole('button', { name: 'Stage this line' });
    fireEvent.click(btns[0]); // left pane del cell renders first
    expect(onStageLines).toHaveBeenCalledWith([{ kind: 'del', oldNo: 2, newNo: null }]);
  });
});

describe('DiffSlotView', () => {
  it('first load shows skeletons; a stale refetch dims the previous diff', () => {
    const { container } = render(
      <DiffSlotView
        slot={{ key: 'unstaged:notes.txt', state: 'loading', diff: null, error: null }}
        onDismissError={vi.fn()}
      />,
    );
    expect(container.querySelector('.diff-slot-loading')).toBeInTheDocument();
    const stale = render(
      <DiffSlotView
        slot={{ key: 'unstaged:notes.txt', state: 'loading', diff: fixture(), error: null }}
        onDismissError={vi.fn()}
      />,
    );
    expect(stale.container.querySelector('.diff-stale')).toBeInTheDocument();
    expect(stale.container.textContent).toContain('beta new');
  });

  it('error state renders the banner and dismiss collapses via the callback', () => {
    const onDismissError = vi.fn();
    render(
      <DiffSlotView
        slot={{ key: 'k', state: 'error', diff: null, error: 'diff failed' }}
        onDismissError={onDismissError}
      />,
    );
    expect(screen.getByRole('alert')).toHaveTextContent('diff failed');
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss error' }));
    expect(onDismissError).toHaveBeenCalledTimes(1);
  });
});
