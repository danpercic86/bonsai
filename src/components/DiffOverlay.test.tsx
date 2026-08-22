/** T3.7 — DiffOverlay: presentational full-pane diff shell. App owns slot/meta/
 *  Esc. Covers: header (badge/path/rename/kind label), Explain gating, File/
 *  Diff/Split toggle wiring, intraline toggle, image-mode switcher, conflict
 *  slot routing (non-text-mergeable → marker view), close wiring. No IPC. */
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { DiffOverlay, type DiffOverlayProps, type DiffOverlayMeta } from './DiffOverlay';
import type { DiffSlot } from './DiffView';
import type { ConflictFile, FileDiff, ImageDiff } from '../ipc';

const FILE_DIFF: FileDiff = {
  path: 'notes.txt',
  origPath: null,
  status: 'modified',
  binary: false,
  tooLarge: false,
  hunks: [
    {
      oldStart: 1,
      oldLines: 1,
      newStart: 1,
      newLines: 1,
      lines: [{ kind: 'context', oldNo: 1, newNo: 1, content: 'x' }],
    },
  ],
};

const readySlot: DiffSlot = {
  key: 'commit:notes.txt',
  state: 'ready',
  diff: FILE_DIFF,
  error: null,
};

function meta(over: Partial<DiffOverlayMeta> = {}): DiffOverlayMeta {
  return { path: 'notes.txt', origPath: null, status: 'modified', kind: 'commit', ...over };
}

function props(over: Partial<DiffOverlayProps> = {}): DiffOverlayProps {
  return {
    slot: readySlot,
    meta: meta(),
    onClose: vi.fn(),
    onResolveConflictText: vi.fn(async () => {}),
    mutating: false,
    viewMode: 'diff',
    onSetViewMode: vi.fn(),
    intraline: false,
    onSetIntraline: vi.fn(),
    stageable: null,
    onStageLines: vi.fn(),
    onStageHunk: vi.fn(),
    ...over,
  };
}

describe('DiffOverlay header', () => {
  it('renders the path, status badge and kind label', () => {
    render(<DiffOverlay {...props()} />);
    expect(screen.getByRole('region', { name: 'Diff: notes.txt' })).toBeInTheDocument();
    expect(screen.getByText('M')).toBeInTheDocument(); // modified badge
    expect(screen.getByText('Commit')).toBeInTheDocument();
  });

  it('shows "orig → path" for a rename and no badge when status is null', () => {
    render(<DiffOverlay {...props({ meta: meta({ origPath: 'old.txt', status: null }) })} />);
    expect(screen.getByTitle('old.txt → notes.txt')).toBeInTheDocument();
    expect(screen.queryByText('M')).not.toBeInTheDocument();
  });

  it('close button fires onClose', () => {
    const p = props();
    render(<DiffOverlay {...p} />);
    fireEvent.click(screen.getByRole('button', { name: 'Close diff' }));
    expect(p.onClose).toHaveBeenCalledTimes(1);
  });
});

describe('DiffOverlay Explain gating', () => {
  it('hides Explain when onExplain is undefined', () => {
    render(<DiffOverlay {...props()} />);
    expect(screen.queryByText('Explain')).not.toBeInTheDocument();
  });

  it('shows Explain and forwards the click when onExplain is provided', () => {
    const onExplain = vi.fn();
    render(<DiffOverlay {...props({ onExplain })} />);
    fireEvent.click(screen.getByText('Explain'));
    expect(onExplain).toHaveBeenCalledTimes(1);
  });
});

describe('DiffOverlay view-mode + intraline toggles', () => {
  it('marks the active view-mode button and routes File/Split clicks', () => {
    const p = props({ viewMode: 'diff' });
    render(<DiffOverlay {...p} />);
    expect(screen.getByRole('button', { name: 'Diff' })).toHaveAttribute('aria-pressed', 'true');
    fireEvent.click(screen.getByRole('button', { name: 'File' }));
    expect(p.onSetViewMode).toHaveBeenCalledWith('file');
    fireEvent.click(screen.getByRole('button', { name: 'Split' }));
    expect(p.onSetViewMode).toHaveBeenCalledWith('split');
  });

  it('Highlight changes toggles intraline to the opposite of the current value', () => {
    const p = props({ intraline: false });
    render(<DiffOverlay {...p} />);
    const btn = screen.getByRole('button', { name: 'Highlight changes' });
    expect(btn).toHaveAttribute('aria-pressed', 'false');
    fireEvent.click(btn);
    expect(p.onSetIntraline).toHaveBeenCalledWith(true);
  });
});

describe('DiffOverlay image slot', () => {
  const imageDiff: ImageDiff = {
    path: 'logo.png',
    old: null,
    new: { mime: 'image/png', base64: 'AAAA', width: 2, height: 2, bytes: 3 } as never,
    oldTooLarge: false,
    newTooLarge: false,
  };

  it('swaps the view-mode group for an image-compare switcher and toggles mode', () => {
    render(
      <DiffOverlay
        {...props({
          meta: meta({ path: 'logo.png', kind: 'commit' }),
          imageDiff,
        })}
      />,
    );
    // No File/Diff/Split for images.
    expect(screen.queryByRole('button', { name: 'Diff' })).not.toBeInTheDocument();
    const sbs = screen.getByRole('button', { name: 'Side-by-side' });
    expect(sbs).toHaveAttribute('aria-pressed', 'true');
    const onion = screen.getByRole('button', { name: 'Onion' });
    fireEvent.click(onion);
    expect(onion).toHaveAttribute('aria-pressed', 'true');
    expect(sbs).toHaveAttribute('aria-pressed', 'false');
  });
});

describe('DiffOverlay conflict slot', () => {
  it('hides the view-mode toggle and renders the marker view for a binary conflict', () => {
    const conflict: ConflictFile = {
      path: 'bin.dat',
      kind: 'deletedByThem',
      binary: true,
      tooLarge: false,
      missing: false,
      text: '',
      ours: '',
      theirs: '',
    };
    const slot: DiffSlot = {
      key: 'conflict:bin.dat',
      state: 'ready',
      diff: null,
      error: null,
      conflict,
    };
    render(
      <DiffOverlay
        {...props({ slot, meta: meta({ path: 'bin.dat', kind: 'conflict', status: 'conflicted' }) })}
      />,
    );
    expect(screen.queryByRole('button', { name: 'Diff' })).not.toBeInTheDocument();
    expect(screen.getByText('Binary file')).toBeInTheDocument();
  });

  it('renders an error banner for a conflict slot in the error state', () => {
    const p = props({
      slot: { key: 'conflict:x', state: 'error', diff: null, error: 'load failed', conflict: null },
      meta: meta({ path: 'x', kind: 'conflict' }),
    });
    render(<DiffOverlay {...p} />);
    expect(screen.getByRole('alert')).toHaveTextContent('load failed');
    fireEvent.click(screen.getByRole('button', { name: 'Dismiss error' }));
    expect(p.onClose).toHaveBeenCalled();
  });
});
