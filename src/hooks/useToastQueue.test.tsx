/** The auto-dismiss timers owned by `useToastQueue`.
 *
 *  These exist because of a fix that was tried, measured, and REVERTED.
 *
 *  The pending 5-second handle at `useToastQueue.ts:40` is discarded, so nothing
 *  cancels it on unmount. That was filed as a leak. The obvious fix -- track the
 *  handles in a ref and clear them in an unmount effect -- is WRONG here, and the
 *  last test below is what proves it: `React.StrictMode` is on in `main.tsx`, so
 *  every dev mount runs effects -> cleanups -> effects, and a toast pushed
 *  synchronously during the FIRST pass has already armed its handle when the
 *  cleanup fires. With the teardown effect in place that toast never
 *  auto-dismissed -- it sat there until clicked. A dev-only behaviour regression
 *  traded for a dev-only timer leak.
 *
 *  So the leak stays: React 18 makes the late `setToasts` a silent no-op, and the
 *  handles expire on their own in 5 s. These tests pin the dismissal contract that
 *  any future attempt must not break. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { StrictMode, useEffect } from 'react';
import { act, renderHook } from '@testing-library/react';
import { useToastQueue } from './useToastQueue';

/** P113 §13.1: the DEV reachability guard's input. These cases all run with
 *  Settings CLOSED — the guard's own behaviour is covered in
 *  `settingsToastGuard.test.tsx`. */
const CLOSED = false;

beforeEach(() => vi.useFakeTimers());
afterEach(() => vi.useRealTimers());

describe('useToastQueue auto-dismiss timers', () => {
  it('drops a non-error toast after 5s and leaves an error toast sticky', () => {
    const { result } = renderHook(() => useToastQueue(CLOSED));

    act(() => result.current.pushToast('info', 'saved'));
    act(() => result.current.pushToast('error', 'broke'));
    expect(result.current.toasts.map((t) => t.text)).toEqual(['saved', 'broke']);

    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current.toasts.map((t) => t.text)).toEqual(['broke']);

    act(() => void vi.advanceTimersByTime(60_000));
    expect(result.current.toasts.map((t) => t.text)).toEqual(['broke']);
  });


  it('leaves no pending timer behind once a toast has dismissed itself', () => {
    const { result } = renderHook(() => useToastQueue(CLOSED));

    act(() => result.current.pushToast('info', 'a'));
    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current.toasts).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  // The regression guard. This passed only after the teardown effect was
  // reverted; with it in place the first toast never left the screen.
  it('still auto-dismisses a toast pushed synchronously during the StrictMode first mount pass', () => {
    const { result } = renderHook(
      () => {
        const q = useToastQueue(CLOSED);
        const { pushToast } = q;
        useEffect(() => {
          pushToast('info', 'mounted');
        }, [pushToast]);
        return q;
      },
      { wrapper: StrictMode },
    );

    // TWO toasts: the push is unkeyed, so StrictMode's second pass adds a
    // second one rather than coalescing. That is pre-existing dev-only
    // behaviour, unrelated to the timers -- it is asserted so the count below
    // is not mistaken for the thing under test.
    expect(result.current.toasts.map((t) => t.text)).toEqual(['mounted', 'mounted']);

    // The point: the FIRST toast's handle is armed before the StrictMode
    // cleanup pass runs. Anything that cancels handles in an unmount effect
    // strands it here -- one toast left forever, dismissable only by a click.
    act(() => void vi.advanceTimersByTime(5000));
    expect(result.current.toasts).toEqual([]);
  });
});
