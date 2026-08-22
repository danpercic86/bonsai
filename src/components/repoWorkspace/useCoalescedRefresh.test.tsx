// P81 §9 — hook-level tests: origin routing + echo suppression against the
// coalescer, using fake timers + a controllable `run` (no real 300/600 ms waits).
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { useCoalescedRefresh } from './useCoalescedRefresh';
import type { RefreshScope } from './refreshScope';
import {
  ECHO_TAIL_MS,
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
      await result.current.refresh('mutation', 'full');
    });
    expect(run).toHaveBeenCalledTimes(1);
    expect(isEchoSuppressed('r1')).toBe(true);

    vi.setSystemTime(300); // echo arrives inside the window
    await act(async () => {
      await result.current.refresh('watcher', 'full');
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
        await result.current.refresh('mutation', 'full');
      });
      vi.setSystemTime(base + 300);
      await act(async () => {
        await result.current.refresh('watcher', 'full'); // suppressed
      });
    }
    expect(run).toHaveBeenCalledTimes(3);
  });

  it('AC3: watcher with no preceding mutation ⇒ run once', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));
    await act(async () => {
      await result.current.refresh('watcher', 'full');
    });
    expect(run).toHaveBeenCalledTimes(1);
  });

  it('AC4: watcher after the settle tail ⇒ second round fires', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    vi.setSystemTime(0);
    await act(async () => {
      await result.current.refresh('mutation', 'full'); // round settles at t=0 → tail to 600
    });
    expect(run).toHaveBeenCalledTimes(1);

    vi.setSystemTime(ECHO_TAIL_MS); // tail expired (boundary is exclusive)
    await act(async () => {
      await result.current.refresh('watcher', 'full');
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
        await result.current.refresh('mutation', 'full');
      });
      expect(run).toHaveBeenCalledTimes(1);

      vi.setSystemTime(100); // inside the armed window
      await act(async () => {
        await result.current.refresh(origin, 'full');
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
      leading = result.current.refresh('manual', 'full'); // starts round 0 (in flight)
    });
    expect(run).toHaveBeenCalledTimes(1);

    const collapsed: Promise<void>[] = [];
    act(() => {
      for (let i = 0; i < 4; i += 1) collapsed.push(result.current.refresh('manual', 'full'));
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
      await hookA.result.current.refresh('mutation', 'full'); // arms A only
    });
    vi.setSystemTime(100);
    await act(async () => {
      await hookB.result.current.refresh('watcher', 'full'); // B not gated
    });
    expect(runB).toHaveBeenCalledTimes(1);
  });

  it('clears the suppression window for its repoId on unmount', async () => {
    const run = makeRun();
    const { result, unmount } = renderHook(() => useCoalescedRefresh('r1', run));
    vi.setSystemTime(0);
    await act(async () => {
      await result.current.refresh('mutation', 'full');
    });
    expect(isEchoSuppressed('r1', 100)).toBe(true);
    unmount();
    expect(isEchoSuppressed('r1', 100)).toBe(false);
  });

  it('keeps refreshAll semantics: a lone mutation resolves after exactly one round', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));
    await act(async () => {
      await result.current.refresh('mutation', 'full');
    });
    expect(run).toHaveBeenCalledTimes(1);
    await flush();
  });

  it('AC-A2a: a slow round suppresses the echo for ANY duration (span open while running)', async () => {
    // Deferred run so the round stays in flight (a slow re-open + full walk on a
    // large repo). The self-echo landing mid-round — far past P81's 600 ms
    // wall-clock window — must still be dropped because the span is OPEN.
    const box: { resolve: (() => void) | null } = { resolve: null };
    const run = vi.fn<() => Promise<void>>(
      () =>
        new Promise<void>((r) => {
          box.resolve = r;
        }),
    );
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    vi.setSystemTime(0);
    let mutation!: Promise<void>;
    act(() => {
      mutation = result.current.refresh('mutation', 'full'); // round in flight
    });
    expect(run).toHaveBeenCalledTimes(1);

    // Echo arrives 5 s later — P81's fixed 600 ms-from-arm window is long gone,
    // but the span is still OPEN (round not settled) so it is suppressed.
    vi.setSystemTime(5_000);
    expect(isEchoSuppressed('r1')).toBe(true);
    let echo!: Promise<void>;
    act(() => {
      echo = result.current.refresh('watcher', 'full');
    });
    await act(async () => {
      await echo;
    });
    expect(run).toHaveBeenCalledTimes(1); // suppressed regardless of elapsed time

    // Settle the leading round; the tail begins now (t = 5_000), anchored to
    // settle, not to arm.
    const settle = box.resolve;
    box.resolve = null;
    await act(async () => {
      settle?.();
      await mutation;
    });

    expect(isEchoSuppressed('r1', 5_000 + ECHO_TAIL_MS - 1)).toBe(true); // inside tail
    expect(isEchoSuppressed('r1', 5_000 + ECHO_TAIL_MS)).toBe(false); // tail expired

    // After the tail a genuine external change runs a fresh round.
    vi.setSystemTime(5_000 + ECHO_TAIL_MS);
    act(() => {
      void result.current.refresh('watcher', 'full');
    });
    expect(run).toHaveBeenCalledTimes(2);
    // Resolve the fresh round so nothing dangles into the next test.
    await act(async () => {
      box.resolve?.();
      await Promise.resolve();
    });
  });

  it('P85 measurement: bumps window.__bonsaiRefreshRounds once per executed round', async () => {
    const g = globalThis as { __bonsaiRefreshRounds?: number };
    g.__bonsaiRefreshRounds = 0;
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    await act(async () => {
      await result.current.refresh('mutation', 'full');
    });
    // A mutation + its (suppressed) watcher echo ⇒ exactly one round counted.
    vi.setSystemTime(100);
    await act(async () => {
      await result.current.refresh('watcher', 'full');
    });
    expect(g.__bonsaiRefreshRounds).toBe(1);
  });

  it('P86a CI-1: an external (backend-confirmed) event bypasses the armed echo window', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    vi.setSystemTime(0);
    await act(async () => {
      // A fetch arms echo suppression for its own remoteMeta round.
      await result.current.refresh('mutation', 'remoteMeta');
    });
    expect(run).toHaveBeenCalledTimes(1);
    expect(isEchoSuppressed('r1')).toBe(true); // inside the settle tail

    // A raw notify echo (reason "fs") landing inside the window is DROPPED…
    await act(async () => {
      await result.current.refresh('watcher', 'full');
    });
    expect(run).toHaveBeenCalledTimes(1);

    // …but a backend-confirmed reason:"tags" event (origin 'external') is NOT our
    // own fs echo, so it refreshes anyway — the CI-1 regression fix.
    await act(async () => {
      await result.current.refresh('external', 'refsOnly');
    });
    expect(run).toHaveBeenCalledTimes(2);
  });

  it('P86a: external origin never arms echo (a later watcher echo is not swallowed)', async () => {
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    vi.setSystemTime(0);
    await act(async () => {
      await result.current.refresh('external', 'refsOnly');
    });
    expect(run).toHaveBeenCalledTimes(1);
    // No span was opened, so an unrelated watcher event runs a fresh round.
    expect(isEchoSuppressed('r1')).toBe(false);
    await act(async () => {
      await result.current.refresh('watcher', 'full');
    });
    expect(run).toHaveBeenCalledTimes(2);
  });

  it('P86a: records the executed round scope in window.__bonsaiRefreshScopes', async () => {
    const g = globalThis as { __bonsaiRefreshScopes?: Record<string, number> };
    g.__bonsaiRefreshScopes = {};
    const run = makeRun();
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    vi.setSystemTime(0);
    await act(async () => {
      await result.current.refresh('mutation', 'worktree');
    });
    vi.setSystemTime(10_000); // clear the first tail
    await act(async () => {
      await result.current.refresh('mutation', 'refsOnly');
    });
    expect(g.__bonsaiRefreshScopes).toMatchObject({ worktree: 1, refsOnly: 1 });
  });

  it('P86a: the trailing round runs the UNION of collapsed scopes (distinct → full)', async () => {
    const g = globalThis as { __bonsaiRefreshScopes?: Record<string, number> };
    g.__bonsaiRefreshScopes = {};
    const box: { resolve: (() => void) | null } = { resolve: null };
    const run = vi.fn<(scope: RefreshScope) => Promise<void>>(
      () =>
        new Promise<void>((r) => {
          box.resolve = r;
        }),
    );
    const { result } = renderHook(() => useCoalescedRefresh('r1', run));

    let leading!: Promise<void>;
    act(() => {
      leading = result.current.refresh('manual', 'refsOnly'); // leading round (in flight)
    });
    expect(run).toHaveBeenNthCalledWith(1, 'refsOnly');

    // Two DIFFERENT scopes collapse into the single trailing round → union = full.
    act(() => {
      void result.current.refresh('mutation', 'worktree');
      void result.current.refresh('mutation', 'remoteMeta');
    });

    const first = box.resolve;
    box.resolve = null;
    await act(async () => {
      first?.();
      await leading;
    });
    expect(run).toHaveBeenNthCalledWith(2, 'full');

    // Settle the trailing round so nothing dangles.
    await act(async () => {
      box.resolve?.();
      await Promise.resolve();
    });
    expect(g.__bonsaiRefreshScopes).toMatchObject({ refsOnly: 1, full: 1 });
  });
});
