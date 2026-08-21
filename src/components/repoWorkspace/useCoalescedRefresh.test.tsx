// P81 §9 — hook-level tests: origin routing + echo suppression against the
// coalescer, using fake timers + a controllable `run` (no real 300/600 ms waits).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useCoalescedRefresh } from './useCoalescedRefresh';
import {
  ECHO_TTL_MS,
  __resetEchoSuppression,
  isEchoSuppressed,
} from './echoSuppression';

beforeEach(() => {
  __resetEchoSuppression();
  vi.useFakeTimers();
  vi.setSystemTime(0);
});
afterEach(() => {
  vi.useRealTimers();
});

/** A `run` that resolves immediately; each call is awaited by flushing microtasks. */
function makeRun() {
  return vi.fn<() => Promise<void>>(() => Promise.resolve());
}

// Flush the microtask queue so a resolved round settles and the coalescer returns
// to idle before the next request (mimics "each mutation settles before the next").
async function flush(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe('useCoalescedRefresh', () => {
  it('AC1: mutation then watcher echo within TTL ⇒ run once', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    await act(async () => {
      await result.current.refresh('mutation');
    });
    expect(run).toHaveBeenCalledTimes(1);
    expect(isEchoSuppressed('r1')).toBe(true);

    vi.setSystemTime(300); // echo arrives inside the window
    await act(async () => {
      await result.current.refresh('watcher');
    });
    expect(run).toHaveBeenCalledTimes(1); // suppressed — no second round
  });

  it('AC2: N mutation+echo pairs ⇒ run N times', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    for (let i = 0; i < 3; i += 1) {
      const base = i * 10_000; // each pair well outside the previous window
      vi.setSystemTime(base);
      await act(async () => {
        await result.current.refresh('mutation');
      });
      vi.setSystemTime(base + 300);
      await act(async () => {
        await result.current.refresh('watcher'); // suppressed
      });
    }
    expect(run).toHaveBeenCalledTimes(3);
  });

  it('AC3: watcher with no preceding mutation ⇒ run once', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));
    await act(async () => {
      await result.current.refresh('watcher');
    });
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('AC4: watcher after TTL ⇒ second round fires', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    vi.setSystemTime(0);
    await act(async () => {
      await result.current.refresh('mutation');
    });
    expect(run).toHaveBeenCalledTimes(1);

    vi.setSystemTime(ECHO_TTL_MS); // window expired (boundary is exclusive)
    await act(async () => {
      await result.current.refresh('watcher');
    });
    expect(run).toHaveBeenCalledTimes(2);
  });

  it('AC6: forced origins bypass suppression', async () => {
    for (const origin of ['activation', 'manual', 'focus'] as const) {
      __resetEchoSuppression();
      const run = makeRun();
      const { result } = renderHook(() => useCoalescedRefresh('r1', run));

      vi.setSystemTime(0);
      await act(async () => {
        await result.current.refresh('mutation');
      });
      expect(run).toHaveBeenCalledTimes(1);

      vi.setSystemTime(100); // inside the armed window
      await act(async () => {
        await result.current.refresh(origin);
      });
      expect(run, `origin=${origin}`).toHaveBeenCalledTimes(2);
    }
  });

  it('AC5: K watcher-free requests mid-flight ⇒ one trailing round', async () => {
    // Deferred run so requests overlap in flight. Object holder so TS does not
    // narrow the resolver to `null` across the intervening `run` invocations.
    const box: { resolve: (() => void) | null } = { resolve: null };
    const run = vi.fn<() => Promise<void>>(
      () =>
        new Promise<void>((r) => {
          box.resolve = r;
        }),
    );
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    let leading!: Promise<void>;
    act(() => {
      leading = result.current.refresh('manual'); // starts round 0 (in flight)
    });
    expect(run).toHaveBeenCalledTimes(1);

    const collapsed: Promise<void>[] = [];
    act(() => {
      for (let i = 0; i < 4; i += 1) collapsed.push(result.current.refresh('manual'));
    });
    expect(run).toHaveBeenCalledTimes(1); // collapsed, not stacked

    // Settle round 0 → exactly one trailing round starts.
    const first = box.resolve;
    box.resolve = null;
    await act(async () => {
      first?.();
      await leading;
    });
    expect(run).toHaveBeenCalledTimes(2);

    await act(async () => {
      box.resolve?.();
      await Promise.all(collapsed);
    });
    expect(run).toHaveBeenCalledTimes(2); // 2 total, independent of K
  });

  it('AC7: arming repo A does not suppress repo B watcher', async () => {
    const runB = makeRun();
    const hookA = renderHook(() => useCoalescedRefresh('A', makeRun()));
    const hookB = renderHook(() => useCoalescedRefresh('B', runB));

    vi.setSystemTime(0);
    await act(async () => {
      await hookA.result.current.refresh('mutation'); // arms A only
    });
    vi.setSystemTime(100);
    await act(async () => {
      await hookB.result.current.refresh('watcher'); // B not gated
    });
    expect(runB).toHaveBeenCalledTimes(1);
  });

  it('clears the suppression window for its repoId on unmount', async () => {
    const run = makeRun();
    const { result, unmount } = renderHook(() => useCoalescedRefresh('r1', run));
    vi.setSystemTime(0);
    await act(async () => {
      await result.current.refresh('mutation');
    });
    expect(isEchoSuppressed('r1', 100)).toBe(true);
    unmount();
    expect(isEchoSuppressed('r1', 100)).toBe(false);
  });

  it('keeps refreshAll semantics: a lone mutation resolves after exactly one round', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));
    await act(async () => {
      await result.current.refresh('mutation');
    });
    expect(run).toHaveBeenCalledTimes(1);
    await flush();
  });
});
