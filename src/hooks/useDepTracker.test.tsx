/** P125 §8 — `useDepTracker`: fetches on mount, repo change, window focus and
 *  every non-progress `deps-changed` for the CURRENT repo; ignores other repos'
 *  events and stale answers; never polls; releases both listeners on unmount.
 *
 *  House pattern: the `dom` vitest project runs with VITE_MOCK_IPC=1, so `ipc`
 *  IS `mockIpc` — spy on it directly (see useGitAvailability.test.tsx). */
import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../ipc/mock';
import { buildDepView } from '../ipc/fixtures/depTracker';
import { useDepTracker } from './useDepTracker';
import type { DepsChangedPayload, DepTrackerView } from '../ipc/types/depTracker';

type DepsCb = (p: DepsChangedPayload) => void;

let depsCb: DepsCb | null = null;
let focusCb: (() => void) | null = null;
const offDeps = vi.fn();
const offFocus = vi.fn();

function viewOf(repoId: string, mode: Parameters<typeof buildDepView>[0] = 'populated'): DepTrackerView {
  return buildDepView(mode, repoId, Date.now());
}

function payload(repoId: string, kind: DepsChangedPayload['kind'], done: number | null = null, total: number | null = null): DepsChangedPayload {
  return { repoId, checkId: 1, kind, done, total };
}

beforeEach(() => {
  depsCb = null;
  focusCb = null;
  offDeps.mockReset();
  offFocus.mockReset();
  vi.spyOn(mockIpc, 'onDepsChanged').mockImplementation(async (cb) => {
    depsCb = cb;
    return offDeps;
  });
  vi.spyOn(mockIpc, 'onWindowFocus').mockImplementation(async (cb) => {
    focusCb = cb;
    return offFocus;
  });
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('useDepTracker', () => {
  it('fetches on mount and exposes the view', async () => {
    const get = vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    const { result } = renderHook(() => useDepTracker('r1'));
    await waitFor(() => expect(result.current.view?.repoId).toBe('r1'));
    expect(get).toHaveBeenCalledTimes(1);
    expect(get).toHaveBeenCalledWith('r1');
    expect(result.current.progress).toBeNull();
  });

  it('with no repo: no fetch, null view', async () => {
    const get = vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    const { result } = renderHook(() => useDepTracker(null));
    await waitFor(() => expect(depsCb).not.toBeNull());
    expect(get).not.toHaveBeenCalled();
    expect(result.current.view).toBeNull();
  });

  it('refetches on repo change and never shows the previous repo in between', async () => {
    vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    const { result, rerender } = renderHook(({ id }) => useDepTracker(id), { initialProps: { id: 'r1' } });
    await waitFor(() => expect(result.current.view?.repoId).toBe('r1'));
    rerender({ id: 'r2' });
    expect(result.current.view).toBeNull();
    await waitFor(() => expect(result.current.view?.repoId).toBe('r2'));
  });

  it('discards a slow answer for a repo that is no longer current', async () => {
    let releaseR1: (v: DepTrackerView) => void = () => {};
    vi.spyOn(mockIpc, 'getDepTracker').mockImplementation((id) =>
      id === 'r1' ? new Promise((res) => {
        releaseR1 = res;
      }) : Promise.resolve(viewOf(id)),
    );
    const { result, rerender } = renderHook(({ id }) => useDepTracker(id), { initialProps: { id: 'r1' } });
    rerender({ id: 'r2' });
    await waitFor(() => expect(result.current.view?.repoId).toBe('r2'));
    await act(async () => releaseR1(viewOf('r1')));
    expect(result.current.view?.repoId).toBe('r2');
  });

  it('refetches on window focus', async () => {
    const get = vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    renderHook(() => useDepTracker('r1'));
    await waitFor(() => expect(focusCb).not.toBeNull());
    await waitFor(() => expect(get).toHaveBeenCalledTimes(1));
    act(() => focusCb?.());
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
  });

  it('progress updates numbers without a fetch; other kinds refetch; other repos are ignored', async () => {
    const get = vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    const { result } = renderHook(() => useDepTracker('r1'));
    await waitFor(() => expect(get).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(depsCb).not.toBeNull());

    act(() => depsCb?.(payload('r1', 'progress', 19, 57)));
    expect(result.current.progress).toEqual({ done: 19, total: 57 });
    expect(get).toHaveBeenCalledTimes(1);

    act(() => depsCb?.(payload('other', 'finished')));
    act(() => depsCb?.(payload('other', 'progress', 1, 2)));
    expect(get).toHaveBeenCalledTimes(1);
    expect(result.current.progress).toEqual({ done: 19, total: 57 });

    // A stale probe mid-check refetches but keeps the live numbers.
    act(() => depsCb?.(payload('r1', 'stale')));
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));
    expect(result.current.progress).toEqual({ done: 19, total: 57 });

    for (const kind of ['finished', 'failed', 'cancelled', 'cleared', 'started'] as const) {
      const before = get.mock.calls.length;
      act(() => depsCb?.(payload('r1', 'progress', 5, 9)));
      act(() => depsCb?.(payload('r1', kind)));
      expect(result.current.progress, kind).toBeNull();
      await waitFor(() => expect(get).toHaveBeenCalledTimes(before + 1));
    }
  });

  it('never polls on a timer', async () => {
    vi.useFakeTimers();
    const get = vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    renderHook(() => useDepTracker('r1'));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(10);
    });
    expect(get).toHaveBeenCalledTimes(1);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(24 * 3_600_000);
    });
    expect(get).toHaveBeenCalledTimes(1);
  });

  it('refresh() starts a check and re-reads the view; a refusal re-syncs instead of throwing', async () => {
    const get = vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    const check = vi.spyOn(mockIpc, 'depsCheckNow').mockResolvedValueOnce(3);
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const { result } = renderHook(() => useDepTracker('r1'));
    await waitFor(() => expect(get).toHaveBeenCalledTimes(1));

    await act(async () => result.current.refresh());
    expect(check).toHaveBeenCalledWith('r1');
    await waitFor(() => expect(get).toHaveBeenCalledTimes(2));

    check.mockRejectedValueOnce({ kind: 'other', message: 'dependency tracker is disabled' });
    get.mockImplementation(async (id) => viewOf(id, 'off'));
    await act(async () => result.current.refresh());
    await waitFor(() => expect(result.current.view?.enabled).toBe(false));
    expect(warn).toHaveBeenCalled();
  });

  it('releases both listeners on unmount', async () => {
    vi.spyOn(mockIpc, 'getDepTracker').mockImplementation(async (id) => viewOf(id));
    const { unmount } = renderHook(() => useDepTracker('r1'));
    await waitFor(() => expect(depsCb).not.toBeNull());
    await waitFor(() => expect(focusCb).not.toBeNull());
    await act(async () => {});
    unmount();
    expect(offDeps).toHaveBeenCalledTimes(1);
    expect(offFocus).toHaveBeenCalledTimes(1);
  });
});
