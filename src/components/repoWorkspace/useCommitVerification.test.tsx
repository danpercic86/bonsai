/** T3.2b — useCommitVerification: debounced batched verify of the visible window,
 *  session cache, enable/disable transitions, chunking, refresh, last-wins. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useCommitVerification } from './useCommitVerification';
import { appErr } from '../../test/actionHookKit';
import type { CommitVerification, GraphLayout, GraphNode, VerifyResults } from '../../ipc';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

type Deps = Parameters<typeof useCommitVerification>[0];

function node(id: string): GraphNode {
  return { id, lane: 0, parents: [], summary: '', author: '', ts: 0, committerTs: 0 };
}
function layout(ids: string[]): GraphLayout {
  return { nodes: ids.map(node), edges: [], laneCount: 1, headIndex: null, truncated: false };
}
function good(oid: string): CommitVerification {
  return { oid, status: 'good', signer: 'dev', key: 'K1' };
}
function res(...oids: string[]): VerifyResults {
  return { verifications: oids.map(good) };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res_, rej) => {
    resolve = res_;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    repoId: '/mock/repo',
    graphDataRef: { current: layout(['a', 'b', 'c', 'd', 'e']) },
    enabled: true,
    pushToast: vi.fn(),
    ...over,
  };
}

function mount(deps: Deps) {
  return renderHook((d: Deps) => useCommitVerification(d), { initialProps: deps });
}

/** Advance past the 150 ms debounce + flush the async fetch. */
async function tick(ms = 150) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe('debounced window verify', () => {
  it('coalesces a scroll flick into ONE batched request for the visible oids', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('b', 'c', 'd'));
    const { result } = mount(makeDeps());
    act(() => result.current.onVisibleRangeChange(0, 1));
    await tick(100); // debounce not elapsed
    expect(spy).not.toHaveBeenCalled();
    act(() => result.current.onVisibleRangeChange(1, 3)); // flick continues
    await tick(150);
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy).toHaveBeenCalledWith('/mock/repo', ['b', 'c', 'd']); // latest window only
    expect(result.current.verifyStatus.get('b')).toBe('good');
    expect(result.current.detailsFor('c')?.signer).toBe('dev');
    expect(result.current.detailsFor('zz')).toBeUndefined();
  });

  it('clamps an out-of-bounds window to the layout', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a', 'e'));
    const { result } = mount(makeDeps());
    act(() => result.current.onVisibleRangeChange(-10, 99));
    await tick();
    expect(spy).toHaveBeenCalledWith('/mock/repo', ['a', 'b', 'c', 'd', 'e']);
  });

  it('cached oids are not re-requested; fully-cached window makes NO request', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a', 'b'));
    const { result } = mount(makeDeps());
    act(() => result.current.onVisibleRangeChange(0, 1));
    await tick();
    expect(spy).toHaveBeenCalledTimes(1);
    spy.mockResolvedValue(res('c'));
    act(() => result.current.onVisibleRangeChange(0, 2)); // a,b cached → only c
    await tick();
    expect(spy).toHaveBeenLastCalledWith('/mock/repo', ['c']);
    act(() => result.current.onVisibleRangeChange(0, 2)); // all cached
    await tick();
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('chunks a window larger than 512 into multiple requests, merged into one cache', async () => {
    const ids = Array.from({ length: 600 }, (_, i) => `c${i}`);
    const spy = vi
      .spyOn(mockIpc, 'verifyCommits')
      .mockImplementation(async (_repo, oids) => ({ verifications: oids.map(good) }));
    const { result } = mount(makeDeps({ graphDataRef: { current: layout(ids) } }));
    act(() => result.current.onVisibleRangeChange(0, 599));
    await tick();
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy.mock.calls[0][1].length).toBe(512);
    expect(spy.mock.calls[1][1].length).toBe(88);
    expect(result.current.verifyStatus.size).toBe(600);
  });

  it('an all-omitted (unresolvable) response leaves the cache untouched', async () => {
    vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue({ verifications: [] });
    const { result } = mount(makeDeps());
    act(() => result.current.onVisibleRangeChange(0, 2));
    await tick();
    expect(result.current.verifyStatus.size).toBe(0);
  });

  it('failure toasts once and caches nothing', async () => {
    vi.spyOn(mockIpc, 'verifyCommits').mockRejectedValue(appErr('git', 'gpg missing'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.onVisibleRangeChange(0, 2));
    await tick();
    expect(deps.pushToast).toHaveBeenCalledTimes(1);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'gpg missing');
    expect(result.current.verifyStatus.size).toBe(0);
  });
});

describe('enabled gating', () => {
  it('disabled: records the window but makes NO request; map stays empty', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a'));
    const { result } = mount(makeDeps({ enabled: false }));
    act(() => result.current.onVisibleRangeChange(0, 4));
    await tick(1000);
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.verifyStatus.size).toBe(0);
  });

  it('enabling re-verifies the LAST recorded window', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a', 'b'));
    const deps = makeDeps({ enabled: false });
    const h = mount(deps);
    act(() => h.result.current.onVisibleRangeChange(0, 1));
    await tick(500);
    expect(spy).not.toHaveBeenCalled();
    h.rerender({ ...deps, enabled: true });
    await tick();
    expect(spy).toHaveBeenCalledWith('/mock/repo', ['a', 'b']);
    expect(h.result.current.verifyStatus.get('a')).toBe('good');
  });

  it('disabling drops the cache and stops fetching', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a', 'b'));
    const deps = makeDeps();
    const h = mount(deps);
    act(() => h.result.current.onVisibleRangeChange(0, 1));
    await tick();
    expect(h.result.current.verifyStatus.size).toBe(2);
    h.rerender({ ...deps, enabled: false });
    expect(h.result.current.verifyStatus.size).toBe(0);
    act(() => h.result.current.onVisibleRangeChange(0, 4));
    await tick(1000);
    expect(spy).toHaveBeenCalledTimes(1); // no new request while disabled
  });

  it('a repoId switch drops the cache and re-verifies the window for the new repo', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a', 'b'));
    const deps = makeDeps();
    const h = mount(deps);
    act(() => h.result.current.onVisibleRangeChange(0, 1));
    await tick();
    expect(h.result.current.verifyStatus.size).toBe(2);
    h.rerender({ ...deps, repoId: '/other/repo' });
    expect(h.result.current.verifyStatus.size).toBe(0);
    await tick();
    expect(spy).toHaveBeenLastCalledWith('/other/repo', ['a', 'b']);
  });
});

describe('refresh + last-wins', () => {
  it('refresh drops the cache and re-requests the current window (even cached oids)', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a', 'b'));
    const { result } = mount(makeDeps());
    act(() => result.current.onVisibleRangeChange(0, 1));
    await tick();
    expect(spy).toHaveBeenCalledTimes(1);
    act(() => result.current.refresh());
    expect(result.current.verifyStatus.size).toBe(0);
    await tick();
    expect(spy).toHaveBeenCalledTimes(2);
    expect(spy).toHaveBeenLastCalledWith('/mock/repo', ['a', 'b']);
  });

  it('refresh before any window is a no-op (no request, no crash)', async () => {
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockResolvedValue(res('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.refresh());
    await tick(1000);
    expect(spy).not.toHaveBeenCalled();
  });

  it('an in-flight response superseded by refresh is dropped (last-wins)', async () => {
    const d = deferred<VerifyResults>();
    const spy = vi.spyOn(mockIpc, 'verifyCommits').mockReturnValueOnce(d.promise);
    const { result } = mount(makeDeps());
    act(() => result.current.onVisibleRangeChange(0, 1));
    await tick(); // request #1 in flight
    spy.mockResolvedValue(res('a', 'b'));
    act(() => result.current.refresh()); // bumps reqId + schedules request #2
    await act(async () => {
      d.resolve({ verifications: [{ oid: 'a', status: 'bad' }] }); // stale
      await Promise.resolve();
    });
    await tick();
    // The stale 'bad' verdict must not survive; the fresh 'good' one wins.
    expect(result.current.verifyStatus.get('a')).toBe('good');
  });
});
