/** P121c — the shared clock store. ONE timer for the whole app however many
 *  consumers are mounted; a tick re-renders only the consumers; the phase is
 *  relative to the first read (never a wall-clock :00/:30 boundary).
 *  Non-StrictMode unless a test says otherwise. */
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { NOW_PERIOD_MS, __resetNowForTests, useNow } from './useNow';

// Deliberately OFF any 30 s wall-clock boundary.
const T0 = 1_700_000_012_345;

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(T0);
  __resetNowForTests();
});

afterEach(() => {
  __resetNowForTests();
  vi.useRealTimers();
});

describe('useNow (P121c)', () => {
  it('shares one timer across consumers, ticks each once, clears on the last unmount', () => {
    const renders = [0, 0, 0];
    const seen: number[] = [];
    function Consumer({ i }: { i: number }) {
      renders[i] += 1;
      const now = useNow();
      seen[i] = now;
      return <span>{now}</span>;
    }
    const { unmount } = render(
      <>
        <Consumer i={0} />
        <Consumer i={1} />
        <Consumer i={2} />
      </>,
    );
    expect(vi.getTimerCount()).toBe(1);
    expect(seen).toEqual([T0, T0, T0]);
    const before = [...renders];

    act(() => {
      vi.advanceTimersByTime(NOW_PERIOD_MS);
    });
    expect(renders).toEqual(before.map((n) => n + 1));
    expect(seen).toEqual([T0 + NOW_PERIOD_MS, T0 + NOW_PERIOD_MS, T0 + NOW_PERIOD_MS]);
    expect(vi.getTimerCount()).toBe(1); // re-armed, still one chain

    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('first tick lands one period after the first read, not at a wall-clock boundary', () => {
    let renders = 0;
    let seen = 0;
    function Consumer() {
      renders += 1;
      seen = useNow();
      return null;
    }
    render(<Consumer />);
    expect(seen).toBe(T0);
    const r = renders;
    // The next wall-clock 30 s boundary passes: nothing happens.
    const toBoundary = NOW_PERIOD_MS - (T0 % NOW_PERIOD_MS);
    act(() => {
      vi.advanceTimersByTime(toBoundary);
    });
    expect(renders).toBe(r);
    act(() => {
      vi.advanceTimersByTime(NOW_PERIOD_MS - toBoundary - 1);
    });
    expect(renders).toBe(r);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(renders).toBe(r + 1);
    expect(seen).toBe(T0 + NOW_PERIOD_MS);
  });

  it('subscribing never re-renders on mount; a late consumer shares the current value', () => {
    let aRenders = 0;
    let bSeen = 0;
    function A() {
      aRenders += 1;
      useNow();
      return null;
    }
    function B() {
      bSeen = useNow();
      return null;
    }
    const { rerender } = render(<A />);
    expect(aRenders).toBe(1);
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    rerender(
      <>
        <A />
        <B />
      </>,
    );
    expect(bSeen).toBe(T0); // same value as A — labels agree
    expect(vi.getTimerCount()).toBe(1);
  });

  it('StrictMode: mount probe + a mid-period remount leave exactly one chain', () => {
    let seen = 0;
    function Consumer() {
      seen = useNow();
      return null;
    }
    const first = render(<Consumer />, { wrapper: StrictMode });
    expect(vi.getTimerCount()).toBe(1);
    act(() => {
      vi.advanceTimersByTime(10_000);
    });
    first.unmount();
    expect(vi.getTimerCount()).toBe(0);
    // Resubscribe mid-period: the chain resumes for the REMAINING 20 s.
    render(<Consumer />, { wrapper: StrictMode });
    expect(vi.getTimerCount()).toBe(1);
    expect(seen).toBe(T0);
    act(() => {
      vi.advanceTimersByTime(NOW_PERIOD_MS - 10_000);
    });
    expect(seen).toBe(T0 + NOW_PERIOD_MS);
    expect(vi.getTimerCount()).toBe(1);
  });

  it('an idle store refreshes a full-period-stale value on the next read', () => {
    let seen = 0;
    function Consumer() {
      seen = useNow();
      return null;
    }
    const first = render(<Consumer />);
    first.unmount();
    vi.setSystemTime(T0 + 5 * NOW_PERIOD_MS);
    render(<Consumer />);
    expect(seen).toBe(T0 + 5 * NOW_PERIOD_MS);
  });
  it('a consumer remounted by the tick itself (mid-tick resubscribe) leaves one chain', () => {
    let childSeen = 0;
    function Child() {
      childSeen = useNow();
      return null;
    }
    function Parent() {
      const now = useNow();
      return <Child key={now} />; // remounts Child on every tick
    }
    render(<Parent />);
    expect(vi.getTimerCount()).toBe(1);
    act(() => {
      vi.advanceTimersByTime(NOW_PERIOD_MS);
    });
    expect(vi.getTimerCount()).toBe(1);
    expect(childSeen).toBe(T0 + NOW_PERIOD_MS);
    act(() => {
      vi.advanceTimersByTime(NOW_PERIOD_MS);
    });
    expect(vi.getTimerCount()).toBe(1);
    expect(childSeen).toBe(T0 + 2 * NOW_PERIOD_MS);
  });
});
