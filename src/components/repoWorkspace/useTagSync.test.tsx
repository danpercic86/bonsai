/** P77 — useTagSync: the ls-remote reconciliation lifecycle for one open repo:
 *  idle→checking→ready, the last-wins guard, the ~10s in-memory cache, the
 *  repo-switch clear, the no-remote short-circuit, and quiet degrade on error. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useTagSync } from './useTagSync';
import type { RemoteInfo, TagSyncReport } from '../../ipc';

afterEach(() => vi.restoreAllMocks());

const REPO = 'repo-1';
const ORIGIN: RemoteInfo[] = [{ name: 'origin', url: 'https://example.invalid/o.git' }];

function report(remote = 'origin'): TagSyncReport {
  return {
    remote,
    entries: [{ name: 'v1.0', status: 'in-sync', localOid: 'a'.repeat(40), remoteOid: 'a'.repeat(40), annotated: false }],
  };
}

/** P113b: a controllable `Date.now` — the in-flight duplicate floor is a
 *  wall-clock window, so tests that need two SEPARATE checks while one is in
 *  flight move the clock past it instead of waiting 2 real seconds. */
function fakeClock(start = 1_700_000_000_000) {
  let now = start;
  vi.spyOn(Date, 'now').mockImplementation(() => now);
  return {
    advance(ms: number) {
      now += ms;
    },
  };
}

/** A promise whose resolution/rejection the test drives by hand. */
function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe('useTagSync lifecycle', () => {
  it('starts idle and does no fetch until refetch is called', () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync');
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));
    expect(result.current.state).toBe('idle');
    expect(result.current.report).toBeNull();
    expect(result.current.checkedAt).toBeNull();
    expect(spy).not.toHaveBeenCalled();
  });

  it('goes idle→checking→ready and stores the report + checkedAt', async () => {
    const d = deferred<TagSyncReport>();
    vi.spyOn(mockIpc, 'listTagSync').mockReturnValue(d.promise);
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));

    let pending!: Promise<void>;
    act(() => {
      pending = result.current.refetch();
    });
    expect(result.current.state).toBe('checking');

    await act(async () => {
      d.resolve(report());
      await pending;
    });
    expect(result.current.state).toBe('ready');
    expect(result.current.report?.entries[0].name).toBe('v1.0');
    expect(result.current.remote).toBe('origin');
    expect(result.current.checkedAt).not.toBeNull();
  });

  it('last-wins: a stale in-flight result never overwrites the newest', async () => {
    const clock = fakeClock();
    const d1 = deferred<TagSyncReport>();
    const d2 = deferred<TagSyncReport>();
    const spy = vi
      .spyOn(mockIpc, 'listTagSync')
      .mockReturnValueOnce(d1.promise)
      .mockReturnValueOnce(d2.promise);
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));

    let p1!: Promise<void>;
    let p2!: Promise<void>;
    act(() => {
      p1 = result.current.refetch(); // request #1
    });
    // P113b: past the in-flight duplicate floor, so this is a genuinely NEW
    // check rather than the same-tick twin the floor now drops.
    clock.advance(3_000);
    act(() => {
      p2 = result.current.refetch(); // request #2 (while #1 in flight)
    });
    expect(spy).toHaveBeenCalledTimes(2);

    // Resolve the NEWEST first, then the stale one — the stale one must be dropped.
    await act(async () => {
      d2.resolve(report('newest'));
      await p2;
      d1.resolve(report('stale'));
      await p1;
    });
    expect(result.current.report?.remote).toBe('newest');
    expect(result.current.state).toBe('ready');
  });

  it('caches within ~10s: a non-force refetch is suppressed, force bypasses it', async () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync').mockResolvedValue(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));

    await act(async () => {
      await result.current.refetch();
    });
    expect(spy).toHaveBeenCalledTimes(1);
    expect(result.current.state).toBe('ready');

    // Within the cache window → suppressed.
    await act(async () => {
      await result.current.refetch();
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // force=true bypasses the cache.
    await act(async () => {
      await result.current.refetch({ force: true });
    });
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('force is a no-op while still idle (never fetched)', async () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync').mockResolvedValue(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));
    await act(async () => {
      await result.current.refetch({ force: true });
    });
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.state).toBe('idle');
  });

  it('no remote configured: short-circuits to idle with no fetch', async () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync');
    const { result } = renderHook(() => useTagSync(REPO, []));
    await act(async () => {
      await result.current.refetch();
    });
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.state).toBe('idle');
    expect(result.current.report).toBeNull();
    expect(result.current.remote).toBeNull();
  });

  it('degrades to unavailable on error, keeping checkedAt from the last success', async () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync');
    spy.mockResolvedValueOnce(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));

    await act(async () => {
      await result.current.refetch();
    });
    const firstCheckedAt = result.current.checkedAt;
    expect(firstCheckedAt).not.toBeNull();

    spy.mockRejectedValueOnce({ kind: 'networkError', message: 'offline' });
    await act(async () => {
      await result.current.refetch({ force: true });
    });
    expect(result.current.state).toBe('unavailable');
    // checkedAt is retained for the "last checked" tooltip; remote stays named.
    expect(result.current.checkedAt).toBe(firstCheckedAt);
    expect(result.current.remote).toBe('origin');
  });

  it('clear() resets to the pristine no-check state', async () => {
    vi.spyOn(mockIpc, 'listTagSync').mockResolvedValue(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));
    await act(async () => {
      await result.current.refetch();
    });
    expect(result.current.state).toBe('ready');

    act(() => result.current.clear());
    expect(result.current.state).toBe('idle');
    expect(result.current.report).toBeNull();
    expect(result.current.checkedAt).toBeNull();
  });
});

/** P77 — the auto-fetch trigger. The ruling is "no repo-open network call and no
 *  new network policy", so this rides the cycle the user already enabled; these
 *  two tests are what keep it from quietly becoming either of the things the
 *  ruling forbids. */
describe('useTagSync afterAutoFetch', () => {
  it('re-checks even inside the ~10s cache window (a fetch just changed the answer)', async () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync').mockResolvedValue(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));
    await act(async () => {
      await result.current.refetch();
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // An UNforced refetch inside the window is (correctly) swallowed …
    await act(async () => {
      await result.current.refetch();
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // … but the auto-fetch trigger must not be: it forces.
    await act(async () => {
      result.current.afterAutoFetch();
    });
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('stays silent until the Tags section has been opened once — no repo-open call', async () => {
    const spy = vi.spyOn(mockIpc, 'listTagSync').mockResolvedValue(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));
    expect(result.current.state).toBe('idle');

    await act(async () => {
      result.current.afterAutoFetch();
    });
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.state).toBe('idle');
  });
});

/** P113b — the in-flight duplicate floor. A measured session logged 12 `dup-ipc`
 *  anomalies for `listTagSync`, each a PAIR with an identical argsHash: two
 *  triggers (a refresh round's tagSync slice and `afterAutoFetch`) landing in
 *  the same tick. The floor drops the twin without costing any freshness. */
describe('useTagSync in-flight duplicate floor', () => {
  it('drops the same-tick twin of an in-flight forced check', async () => {
    fakeClock();
    const d = deferred<TagSyncReport>();
    const spy = vi.spyOn(mockIpc, 'listTagSync').mockReturnValue(d.promise);
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));

    // Open the section once so `force` is no longer a no-op.
    let first!: Promise<void>;
    act(() => {
      first = result.current.refetch();
    });
    await act(async () => {
      d.resolve(report());
      await first;
    });
    expect(spy).toHaveBeenCalledTimes(1);

    // The double-fire: two forced triggers in ONE tick (Promise.all-style).
    const d2 = deferred<TagSyncReport>();
    spy.mockReturnValue(d2.promise);
    let pair!: Promise<unknown>;
    act(() => {
      pair = Promise.all([
        result.current.refetch({ force: true }),
        result.current.refetch({ force: true }),
      ]);
    });
    expect(spy).toHaveBeenCalledTimes(2); // ONE new call, not two

    await act(async () => {
      d2.resolve(report());
      await pair;
    });
    expect(result.current.state).toBe('ready');
  });

  it('never suppresses a forced check issued after the previous one settled', async () => {
    fakeClock();
    const spy = vi.spyOn(mockIpc, 'listTagSync').mockResolvedValue(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));

    await act(async () => {
      await result.current.refetch();
    });
    // Same millisecond, but nothing is in flight → the check runs. This is what
    // keeps the floor from costing freshness.
    await act(async () => {
      await result.current.refetch({ force: true });
    });
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('a FAILED check never self-suppresses the next one', async () => {
    fakeClock();
    const spy = vi.spyOn(mockIpc, 'listTagSync');
    spy.mockResolvedValueOnce(report());
    const { result } = renderHook(() => useTagSync(REPO, ORIGIN));
    await act(async () => {
      await result.current.refetch();
    });

    spy.mockRejectedValueOnce({ kind: 'networkError', message: 'offline' });
    await act(async () => {
      await result.current.refetch({ force: true });
    });
    expect(result.current.state).toBe('unavailable');

    // Immediately afterwards, at the SAME clock value: the retry must go out.
    spy.mockResolvedValueOnce(report());
    await act(async () => {
      await result.current.refetch({ force: true });
    });
    expect(spy).toHaveBeenCalledTimes(3);
    expect(result.current.state).toBe('ready');
  });
});
