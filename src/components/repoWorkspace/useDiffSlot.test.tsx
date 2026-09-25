/**
 * P124c — `useDiffSlot`: a background refetch of the shown diff that returns an
 * identical diff commits NOTHING (no 'loading' flash, same slot object); a
 * changed diff commits once; a foreground refetch still shows 'loading'; and
 * overlapping fetches land only the latest.
 */
import { Profiler } from 'react';
import type { ReactNode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import type { DiffSlot } from '../DiffView';
import type { FileDiff } from '../../ipc';
import { useDiffSlot } from './useDiffSlot';

const KEY = 'unstaged:README.md';

/** A fresh object every call — what the IPC layer returns per round. */
function diffOf(line: string): FileDiff {
  return {
    path: 'README.md',
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
        lines: [{ kind: 'add', content: line, oldNo: null, newNo: 1 }],
      },
    ],
  };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

/** Counts React COMMITS (Profiler `onRender` fires per commit only — a render
 *  React bails out of after an identical `setState` is not a commit) and the
 *  slot state each committed render observed. */
function mount() {
  let commits = 0;
  const states: Array<DiffSlot['state'] | null> = [];
  const wrapper = ({ children }: { children: ReactNode }) => (
    <Profiler id="useDiffSlot" onRender={() => (commits += 1)}>
      {children}
    </Profiler>
  );
  const view = renderHook(
    () => {
      const slot = useDiffSlot();
      states.push(slot.diffSlot?.state ?? null);
      return slot;
    },
    { wrapper },
  );
  return { view, commits: () => commits, states };
}

describe('useDiffSlot', () => {
  it('background refetch with an identical diff keeps the slot object and commits nothing', async () => {
    const { view, commits, states } = mount();
    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a')));
    const s1 = view.result.current.diffSlot;
    expect(s1?.state).toBe('ready');
    const before = commits();
    states.length = 0;

    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a'), { background: true }));

    expect(view.result.current.diffSlot).toBe(s1);
    expect(commits()).toBe(before);
    expect(states).not.toContain('loading');
  });

  it('background refetch with a changed diff commits once, straight to ready', async () => {
    const { view, commits, states } = mount();
    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a')));
    const before = commits();
    states.length = 0;

    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('b'), { background: true }));

    expect(commits()).toBe(before + 1);
    expect(states).not.toContain('loading');
    expect(view.result.current.diffSlot?.state).toBe('ready');
    expect(view.result.current.diffSlot?.diff).toEqual(diffOf('b'));
  });

  it('foreground refetch of the same key shows loading (stale diff kept) then ready', async () => {
    const { view, states } = mount();
    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a')));
    const s1 = view.result.current.diffSlot;
    states.length = 0;
    const d = deferred<FileDiff>();

    let pending: Promise<void> = Promise.resolve();
    act(() => {
      pending = view.result.current.fetchDiffSlot(KEY, () => d.promise);
    });
    expect(view.result.current.diffSlot?.state).toBe('loading');
    expect(view.result.current.diffSlot?.diff).toBe(s1?.diff);

    await act(async () => {
      d.resolve(diffOf('a'));
      await pending;
    });
    expect(view.result.current.diffSlot?.state).toBe('ready');
    expect(states).toEqual(['loading', 'ready']);
  });

  it('foreground same-key fetch resolving before React renders loading still ends ready', async () => {
    const { view } = mount();
    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a')));
    // The fetcher resolves immediately with an EQUAL diff, inside the same act —
    // the 'loading' update is dispatched but not yet rendered when it lands.
    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a')));
    expect(view.result.current.diffSlot?.state).toBe('ready');
  });

  it('a background fetch of a DIFFERENT key still shows loading', async () => {
    const { view } = mount();
    await act(() => view.result.current.fetchDiffSlot(KEY, async () => diffOf('a')));
    const d = deferred<FileDiff>();
    let pending: Promise<void> = Promise.resolve();
    act(() => {
      pending = view.result.current.fetchDiffSlot('staged:README.md', () => d.promise, { background: true });
    });
    expect(view.result.current.diffSlot).toMatchObject({ key: 'staged:README.md', state: 'loading', diff: null });
    await act(async () => {
      d.resolve(diffOf('a'));
      await pending;
    });
    expect(view.result.current.diffSlot?.state).toBe('ready');
  });

  it('overlapping fetches: only the latest lands', async () => {
    const { view } = mount();
    const first = deferred<FileDiff>();
    const second = deferred<FileDiff>();
    let p1: Promise<void> = Promise.resolve();
    let p2: Promise<void> = Promise.resolve();
    act(() => {
      p1 = view.result.current.fetchDiffSlot(KEY, () => first.promise);
    });
    act(() => {
      p2 = view.result.current.fetchDiffSlot(KEY, () => second.promise);
    });
    await act(async () => {
      second.resolve(diffOf('second'));
      await p2;
    });
    await act(async () => {
      first.resolve(diffOf('first'));
      await p1;
    });
    expect(view.result.current.diffSlot?.state).toBe('ready');
    expect(view.result.current.diffSlot?.diff).toEqual(diffOf('second'));
  });

  it('an error result replaces the slot with the error state', async () => {
    const { view } = mount();
    await act(() =>
      view.result.current.fetchDiffSlot(KEY, async () => {
        throw new Error('boom');
      }),
    );
    expect(view.result.current.diffSlot).toMatchObject({ key: KEY, state: 'error', diff: null, error: 'boom' });
  });
});
