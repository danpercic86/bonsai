/** P121c — the shared, boundary-aligned clock store. ONE timer for the whole
 *  app however many consumers are mounted, and a tick re-renders only the
 *  consumers themselves. Non-StrictMode throughout (the timer-count assertion
 *  is about the steady state, not StrictMode's mount/unmount/remount probe). */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';

import { NOW_PERIOD_MS, __resetNowForTests, useNow } from './useNow';

const T0 = 1_700_000_010_000; // an exact NOW_PERIOD_MS boundary

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
  it('shares one timer across consumers and clears it on the last unmount', () => {
    expect(T0 % NOW_PERIOD_MS).toBe(0);
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
    // Re-armed for the next boundary — still exactly one timer.
    expect(vi.getTimerCount()).toBe(1);

    unmount();
    expect(vi.getTimerCount()).toBe(0);
  });

  it('is quantized: mid-period reads return the boundary and do not re-render', () => {
    vi.setSystemTime(T0 + 12_345);
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
    act(() => {
      vi.advanceTimersByTime(NOW_PERIOD_MS - 12_345 - 1);
    });
    expect(renders).toBe(r);
    act(() => {
      vi.advanceTimersByTime(1);
    });
    expect(renders).toBe(r + 1);
    expect(seen).toBe(T0 + NOW_PERIOD_MS);
  });
});
