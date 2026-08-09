/** T3.2b — useCommitSearch: debounced live search, submit-only content mode,
 *  last-wins reqId, empty-query reset, error path, match stepping. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useCommitSearch } from './useCommitSearch';
import { appErr } from '../../test/actionHookKit';
import type { GraphLayout, GraphNode, SearchMatch, SearchResults } from '../../ipc';

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

type Deps = Parameters<typeof useCommitSearch>[0];

function node(id: string): GraphNode {
  return { id, lane: 0, parents: [], summary: '', author: '', ts: 0, committerTs: 0 };
}
function layout(ids: string[]): GraphLayout {
  return { nodes: ids.map(node), edges: [], laneCount: 1, headIndex: null, truncated: false };
}
function match(oid: string): SearchMatch {
  return { oid, summary: 's', authorName: 'a', authorTs: 0, matched: 'message' };
}
function results(...oids: string[]): SearchResults {
  return { matches: oids.map(match), truncated: false };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    repoId: '/mock/repo',
    graph: layout(['a', 'b', 'c']),
    revealCommitByOid: vi.fn(),
    pushToast: vi.fn(),
    ...over,
  };
}

function mount(deps: Deps) {
  return renderHook((d: Deps) => useCommitSearch(d), { initialProps: deps });
}

/** Flush the debounce window + resulting microtasks. */
async function tick(ms = 250) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

describe('open/close', () => {
  it('openSearch opens, bumps openNonce each call, and seeds initial text', async () => {
    const spy = vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('a'));
    const { result } = mount(makeDeps());
    expect(result.current.open).toBe(false);
    act(() => result.current.openSearch());
    expect(result.current.open).toBe(true);
    expect(result.current.openRef.current).toBe(true);
    const n1 = result.current.openNonce;
    act(() => result.current.openSearch('fix'));
    expect(result.current.openNonce).toBe(n1 + 1);
    expect(result.current.query.text).toBe('fix');
    await tick();
    expect(spy).toHaveBeenCalledTimes(1);
  });

  it('close drops an in-flight response (last-wins bump)', async () => {
    const d = deferred<SearchResults>();
    vi.spyOn(mockIpc, 'searchCommits').mockReturnValue(d.promise);
    const { result } = mount(makeDeps());
    act(() => result.current.openSearch('x'));
    await tick();
    expect(result.current.loading).toBe(true);
    act(() => result.current.close());
    expect(result.current.open).toBe(false);
    expect(result.current.loading).toBe(false);
    await act(async () => {
      d.resolve(results('a'));
      await Promise.resolve();
    });
    expect(result.current.results).toBeNull();
  });
});

describe('debounced live search (cheap fields)', () => {
  it('coalesces rapid typing into one request with the latest query', async () => {
    const spy = vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('b'));
    const { result } = mount(makeDeps());
    act(() => result.current.openSearch());
    act(() => result.current.patchQuery({ text: 'f' }));
    await tick(100);
    act(() => result.current.patchQuery({ text: 'fi' }));
    await tick(100);
    act(() => result.current.patchQuery({ text: 'fix' }));
    await tick(249);
    expect(spy).not.toHaveBeenCalled();
    await tick(1);
    expect(spy).toHaveBeenCalledTimes(1);
    expect(spy.mock.calls[0][1].text).toBe('fix');
  });

  it('does not fire while closed', async () => {
    const spy = vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results());
    const { result } = mount(makeDeps());
    act(() => result.current.patchQuery({ text: 'fix' }));
    await tick(1000);
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.loading).toBe(false);
  });

  it('empty/whitespace text resets immediately without a request', async () => {
    const spy = vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.openSearch('fix'));
    await tick();
    expect(result.current.results).not.toBeNull();
    spy.mockClear();
    act(() => result.current.patchQuery({ text: '   ' }));
    // No debounce wait needed — the reset is synchronous in the effect.
    await act(async () => {});
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.results).toBeNull();
    expect(result.current.error).toBeNull();
    expect(result.current.currentMatch).toBe(-1);
  });
});

describe('success + match stepping', () => {
  it('selects and reveals the first match; matchRows maps oids to layout rows', async () => {
    vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('c', 'a'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.openSearch('x'));
    await tick();
    expect(result.current.currentMatch).toBe(0);
    expect(deps.revealCommitByOid).toHaveBeenCalledWith('c');
    expect(result.current.matchRows).toEqual([2, 0]);
    expect(result.current.loading).toBe(false);
  });

  it('zero matches → currentMatch -1, no reveal, next/prev are no-ops', async () => {
    vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results());
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.openSearch('nope'));
    await tick();
    expect(result.current.currentMatch).toBe(-1);
    expect(deps.revealCommitByOid).not.toHaveBeenCalled();
    act(() => result.current.next());
    expect(result.current.currentMatch).toBe(-1);
  });

  it('next/prev wrap around; goToMatch ignores out-of-range indices', async () => {
    vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('a', 'b'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.openSearch('x'));
    await tick();
    act(() => result.current.next());
    expect(result.current.currentMatch).toBe(1);
    act(() => result.current.next());
    expect(result.current.currentMatch).toBe(0); // wrapped
    act(() => result.current.prev());
    expect(result.current.currentMatch).toBe(1); // wrapped back
    expect(deps.revealCommitByOid).toHaveBeenLastCalledWith('b');
    act(() => result.current.goToMatch(5));
    expect(result.current.currentMatch).toBe(1);
    act(() => result.current.goToMatch(-1));
    expect(result.current.currentMatch).toBe(1);
  });

  it('matchRows empty while closed and after a close (ring clear on dismiss)', async () => {
    vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.openSearch('x'));
    await tick();
    expect(result.current.matchRows).toEqual([0]);
    act(() => result.current.close());
    expect(result.current.matchRows).toEqual([]);
  });

  it('matchRows re-map when the graph value changes (reorder while open)', async () => {
    vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('a'));
    const deps = makeDeps();
    const h = mount(deps);
    act(() => h.result.current.openSearch('x'));
    await tick();
    expect(h.result.current.matchRows).toEqual([0]);
    h.rerender({ ...deps, graph: layout(['z', 'a']) });
    expect(h.result.current.matchRows).toEqual([1]);
  });
});

describe('content mode (submit-only)', () => {
  it('never auto-fires; needsSubmit until submit() ran the exact query', async () => {
    const spy = vi.spyOn(mockIpc, 'searchCommits').mockResolvedValue(results('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.openSearch());
    act(() => result.current.patchQuery({ field: 'content', text: 'needle' }));
    await tick(2000);
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.needsSubmit).toBe(true);
    await act(async () => {
      result.current.submit();
      await Promise.resolve();
    });
    expect(spy).toHaveBeenCalledTimes(1);
    expect(result.current.needsSubmit).toBe(false);
    // Editing the query re-arms needsSubmit.
    act(() => result.current.patchQuery({ text: 'needle2' }));
    expect(result.current.needsSubmit).toBe(true);
  });
});

describe('last-wins + errors', () => {
  it('a stale response is dropped when a newer request superseded it', async () => {
    const d1 = deferred<SearchResults>();
    const d2 = deferred<SearchResults>();
    const spy = vi
      .spyOn(mockIpc, 'searchCommits')
      .mockReturnValueOnce(d1.promise)
      .mockReturnValueOnce(d2.promise);
    const { result } = mount(makeDeps());
    act(() => result.current.openSearch('one'));
    await tick();
    act(() => result.current.patchQuery({ text: 'two' }));
    await tick();
    expect(spy).toHaveBeenCalledTimes(2);
    await act(async () => {
      d2.resolve(results('b'));
      await Promise.resolve();
    });
    await act(async () => {
      d1.resolve(results('a')); // stale — must be ignored
      await Promise.resolve();
    });
    expect(result.current.results?.matches[0].oid).toBe('b');
    expect(result.current.loading).toBe(false);
  });

  it('failure surfaces the message, toasts, and clears results', async () => {
    vi.spyOn(mockIpc, 'searchCommits').mockRejectedValue(appErr('git', 'bad regex'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.openSearch('('));
    await tick();
    expect(result.current.error).toBe('bad regex');
    expect(result.current.results).toBeNull();
    expect(result.current.loading).toBe(false);
    expect(result.current.currentMatch).toBe(-1);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'bad regex');
  });
});
