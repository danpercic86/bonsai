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
