/** T5b §5.3 — rapid-fire interactions. Gaps the contract lists beyond the
 *  existing T3.3a suites: double-click / hotkey-spam on the CommitBox submit
 *  (exactly ONE IPC commit), palette open/close/execute spam through a
 *  stateful harness (single dispatch, no desync), and 10× interleaved
 *  stage/unstage against the mock IPC (last-wins, coherent final snapshot). */
import { useState } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { fireEvent, render, screen, waitFor } from '@testing-library/react';

import { CommitBox } from '../CommitBox';
import type { CommitBoxProps } from '../CommitBox';
import { CommandPalette } from '../CommandPalette';
import type { PaletteAction } from '../paletteActions';
import { freshRepoPath, run } from '../../test/mockIpcKit';
import { repoHandlers } from '../../ipc/mock/handlers/repo';
import { statusHandlers } from '../../ipc/mock/handlers/status';

// ---------------------------------------------------------------------------
// CommitBox — double submit
// ---------------------------------------------------------------------------

interface Deferred {
  promise: Promise<void>;
  resolve(): void;
}

function deferred(): Deferred {
  let resolve!: () => void;
  const promise = new Promise<void>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe('CommitBox rapid-fire submit', () => {
  function setup(extra: Partial<CommitBoxProps> = {}) {
    const d = deferred();
    const onCommit = vi.fn(() => d.promise);
    const utils = render(
      <CommitBox stagedCount={1} busy={false} onCommit={onCommit} {...extra} />,
    );
    fireEvent.change(screen.getByPlaceholderText('Commit message'), {
      target: { value: 'msg' },
    });
    return { ...utils, d, onCommit };
  }

  it('double-click on Commit fires exactly one commit call', async () => {
    const { d, onCommit } = setup();
    const btn = screen.getByRole('button', { name: 'Commit' });
    fireEvent.click(btn);
    fireEvent.click(btn); // second click of the double-click
    fireEvent.dblClick(btn);
    expect(onCommit).toHaveBeenCalledTimes(1);
    expect(btn).toBeDisabled();
    d.resolve();
    // On resolve the box clears its message and re-enables (single commit).
    await waitFor(() =>
      expect(screen.getByPlaceholderText('Commit message')).toHaveValue(''),
    );
    expect(onCommit).toHaveBeenCalledTimes(1);
  });

  it('Ctrl+Enter spam (5×) while the first commit is in flight fires exactly once', () => {
    const { onCommit } = setup();
    const box = screen.getByPlaceholderText('Commit message');
    for (let i = 0; i < 5; i++) {
      fireEvent.keyDown(box, { key: 'Enter', ctrlKey: true });
    }
    expect(onCommit).toHaveBeenCalledTimes(1);
  });

  it('split control: hammering Commit & Push then Commit yields ONE total call', () => {
    const d2 = deferred();
    const onCommitAndPush = vi.fn(() => d2.promise);
    const { onCommit } = setup({ onCommitAndPush });
    const push = screen.getByRole('button', { name: 'Commit & Push' });
    const commit = screen.getByRole('button', { name: 'Commit' });
    fireEvent.click(push);
    fireEvent.click(commit);
    fireEvent.click(push);
    expect(onCommitAndPush).toHaveBeenCalledTimes(1);
    expect(onCommit).not.toHaveBeenCalled();
    expect(commit).toBeDisabled();
  });
});

// ---------------------------------------------------------------------------
// CommandPalette — open/close/execute spam
// ---------------------------------------------------------------------------

describe('CommandPalette open/close/execute spam', () => {
  function makeActions(): PaletteAction[] {
    return [
      { id: 'a1', title: 'Fetch', group: 'action', run: vi.fn() },
      { id: 'a2', title: 'Pull', group: 'action', run: vi.fn() },
    ];
  }

  /** Stateful harness mirroring usePalette's close wiring: onClose actually
   *  flips `open`, exactly like the app (so Enter-spam sees the real
   *  closed-after-first-dispatch sequence). */
  function Harness({ actions }: { actions: PaletteAction[] }) {
    const [open, setOpen] = useState(true);
    return (
      <>
        <button type="button" onClick={() => setOpen((v) => !v)}>
          toggle-palette
        </button>
        <CommandPalette
          open={open}
          actions={actions}
          onClose={() => setOpen(false)}
          onRunSearch={() => {}}
          onJumpToCommit={() => {}}
        />
      </>
    );
  }

  it('Enter spam dispatches the highlighted action exactly once (palette closes first)', () => {
    const actions = makeActions();
    render(<Harness actions={actions} />);
    const input = screen.getByRole('combobox');
    fireEvent.keyDown(input, { key: 'Enter' });
    // Palette closed after the first Enter; further Enters have no target.
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: 'Enter' });
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(actions[0].run).toHaveBeenCalledTimes(1);
    expect(actions[1].run).not.toHaveBeenCalled();
  });

  it('10× open/close toggle + type/arrow/Escape spam: no throw, fresh query each open', () => {
    const actions = makeActions();
    render(<Harness actions={actions} />);
    const toggle = screen.getByRole('button', { name: 'toggle-palette' });

    for (let i = 0; i < 10; i++) {
      // Close (odd) / reopen (even) rapidly.
      fireEvent.click(toggle);
    }
    // Even number of toggles from open -> still open.
    let input = screen.getByRole('combobox');
    // Type spam + navigation spam.
    for (const ch of 'pullpullpull') {
      fireEvent.change(input, { target: { value: (input as HTMLInputElement).value + ch } });
      fireEvent.keyDown(input, { key: 'ArrowDown' });
      fireEvent.keyDown(input, { key: 'ArrowUp' });
    }
    // Escape closes; reopen resets the query (no stale filter desync).
    fireEvent.keyDown(window, { key: 'Escape' });
    expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
    fireEvent.click(toggle);
    input = screen.getByRole('combobox');
    expect((input as HTMLInputElement).value).toBe('');
    expect(screen.getByRole('option', { name: 'Fetch' })).toBeInTheDocument();
    // Nothing ran during the spam.
    expect(actions[0].run).not.toHaveBeenCalled();
    expect(actions[1].run).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// Mock IPC — stage/unstage the same file 10× fast
// ---------------------------------------------------------------------------

describe('mock IPC: interleaved stage/unstage storm', () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it('10 alternating stage/unstage calls settle last-wins with a coherent snapshot', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('t5rapid')));

    // Fire all 10 without awaiting (the UI equivalent of hammering +/− on one
    // row). Mock latency is a fixed 150 ms, so resolution order == call order
    // and the LAST call (unstage) must win.
    const ops: Promise<void>[] = [];
    for (let i = 0; i < 5; i++) {
      ops.push(statusHandlers.stage(repoId, ['README.md']));
      ops.push(statusHandlers.unstage(repoId, ['README.md']));
    }
    await vi.advanceTimersByTimeAsync(10_000);
    await expect(Promise.all(ops)).resolves.toBeDefined();

    const s = await run(statusHandlers.getStatus(repoId));
    const inStaged = s.staged.filter((e) => e.path === 'README.md').length;
    const inUnstaged = s.unstaged.filter((e) => e.path === 'README.md').length;
    const inUntracked = s.untracked.filter((e) => e.path === 'README.md').length;
    // Coherence: the file lives in EXACTLY one section, and (last op = unstage)
    // that section is `unstaged`, with its status preserved.
    expect([inStaged, inUnstaged, inUntracked]).toEqual([0, 1, 0]);
    expect(s.unstaged.find((e) => e.path === 'README.md')?.status).toBe('modified');
  });

  it('storm ending on stage leaves the file staged exactly once', async () => {
    const { repoId } = await run(repoHandlers.openRepo(freshRepoPath('t5rapid')));
    const ops: Promise<void>[] = [];
    for (let i = 0; i < 5; i++) {
      ops.push(statusHandlers.unstage(repoId, ['README.md']));
      ops.push(statusHandlers.stage(repoId, ['README.md']));
    }
    await vi.advanceTimersByTimeAsync(10_000);
    await Promise.all(ops);
    const s = await run(statusHandlers.getStatus(repoId));
    expect(s.staged.filter((e) => e.path === 'README.md')).toHaveLength(1);
    expect(s.unstaged.some((e) => e.path === 'README.md')).toBe(false);
  });
});
