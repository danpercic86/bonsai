/** The debounced session write owned by `useRepoTabs` (§6).
 *
 *  `persistSession` arms a 300 ms `setTimeout`; an unmount effect cancels it so a
 *  pending write cannot land on a gone tree. The second test is the one that
 *  earns its place: `React.StrictMode` is on in `main.tsx`, so every dev mount
 *  runs effects -> cleanups -> effects, and an unmount-effect cancel is only safe
 *  if nothing has armed a timer by the time that intermediate cleanup fires.
 *  The sibling hook `useToastQueue` fails exactly that condition -- see
 *  `useToastQueue.test.tsx` for the fix that had to be reverted because of it --
 *  so this is asserted here rather than assumed. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StrictMode } from 'react';
import { act, renderHook } from '@testing-library/react';
import { ipc } from '../ipc';
import { useRepoTabs } from './useRepoTabs';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe('useRepoTabs session persistence', () => {
  it('cancels a pending debounced write on unmount', () => {
    const setSession = vi.spyOn(ipc, 'setSession').mockResolvedValue(undefined);
    const { result, unmount } = renderHook(() => useRepoTabs(() => {}));

    // The persist effect is gated until App's launch-reopen settles.
    act(() => {
      result.current.sessionReadyRef.current = true;
      result.current.setTabs([{ repoId: '/r', name: 'r', path: '/r' } as never]);
    });
    expect(setSession).not.toHaveBeenCalled();

    unmount();
    act(() => void vi.advanceTimersByTime(1000));
    expect(setSession).not.toHaveBeenCalled();
  });

  it('still writes after a StrictMode double-mount', async () => {
    const setSession = vi.spyOn(ipc, 'setSession').mockResolvedValue(undefined);
    const { result } = renderHook(() => useRepoTabs(() => {}), { wrapper: StrictMode });

    act(() => {
      result.current.sessionReadyRef.current = true;
      result.current.setTabs([{ repoId: '/r', name: 'r', path: '/r' } as never]);
    });

    await act(async () => {
      vi.advanceTimersByTime(300);
    });
    expect(setSession).toHaveBeenCalledWith({ openRepos: ['/r'], activeRepo: null });
  });
});
