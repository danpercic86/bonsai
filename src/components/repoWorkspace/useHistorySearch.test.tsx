/** T3.2b — useHistorySearch: status probe, index build w/ progress stream,
 *  submit-only retrieval, last-wins, matchRows, canAsk/askAi gating. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useHistorySearch } from './useHistorySearch';
import { appErr } from '../../test/actionHookKit';
import type {
  GraphLayout,
  GraphNode,
  HistoryHit,
  HistorySearchResults,
  IndexProgress,
  IndexStatus,
} from '../../ipc';

afterEach(() => vi.restoreAllMocks());

type Deps = Parameters<typeof useHistorySearch>[0];

function node(id: string): GraphNode {
  return { id, lane: 0, parents: [], summary: '', author: '', ts: 0, committerTs: 0 };
}
function layout(ids: string[]): GraphLayout {
  return { nodes: ids.map(node), edges: [], laneCount: 1, headIndex: null, truncated: false };
}
function status(over: Partial<IndexStatus> = {}): IndexStatus {
  return {
    built: true,
    indexedCommits: 10,
    headOid: 'h',
    stale: false,
    newCommits: 0,
    schema: 1,
    builtAt: 1,
    skippedCommits: 0,
    ...over,
  };
}
function hit(oid: string, score = 1): HistoryHit {
  return { oid, summary: 's', authorName: 'a', authorTs: 0, score };
}
function hits(...oids: string[]): HistorySearchResults {
  return { hits: oids.map((o) => hit(o)), indexStale: false, indexedCommits: 10 };
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
    aiEligible: false,
    runAiAnswer: vi.fn(),
    pushToast: vi.fn(),
    ...over,
  };
}

const flush = () => act(async () => {});

function mount(deps: Deps) {
  return renderHook((d: Deps) => useHistorySearch(d), { initialProps: deps });
}

describe('openPanel / status', () => {
  it('opens and probes the index status', async () => {
    const spy = vi.spyOn(mockIpc, 'historyIndexStatus').mockResolvedValue(status());
    const { result } = mount(makeDeps());
    act(() => result.current.openPanel());
    expect(result.current.open).toBe(true);
    expect(result.current.openRef.current).toBe(true);
    await flush();
    expect(spy).toHaveBeenCalledWith('/mock/repo');
    expect(result.current.status?.built).toBe(true);
  });

  it('status failure toasts and leaves status null', async () => {
    vi.spyOn(mockIpc, 'historyIndexStatus').mockRejectedValue(appErr('git', 'no repo'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.openPanel());
    await flush();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'no repo');
    expect(result.current.status).toBeNull();
  });

  it('a slow stale status cannot clobber a fresher one (last-wins)', async () => {
    const d1 = deferred<IndexStatus>();
    const d2 = deferred<IndexStatus>();
    vi.spyOn(mockIpc, 'historyIndexStatus')
      .mockReturnValueOnce(d1.promise)
      .mockReturnValueOnce(d2.promise);
    const { result } = mount(makeDeps());
    act(() => result.current.openPanel());
    act(() => result.current.refreshStatus());
    await act(async () => d2.resolve(status({ indexedCommits: 42 })));
    await act(async () => d1.resolve(status({ indexedCommits: 1 }))); // stale
    expect(result.current.status?.indexedCommits).toBe(42);
  });
});

describe('build', () => {
  it('streams progress, adopts the returned status, and clears the bar', async () => {
    let emit!: (p: IndexProgress) => void;
    const d = deferred<IndexStatus>();
    vi.spyOn(mockIpc, 'historyIndexBuild').mockImplementation((_repo, onProgress) => {
      emit = onProgress;
      return d.promise;
    });
    const { result } = mount(makeDeps());
    act(() => result.current.build());
    expect(result.current.building).toBe(true);
    act(() => emit({ phase: 'extracting', processed: 5, total: 10, newCommits: 5 }));
    expect(result.current.progress?.processed).toBe(5);
    await act(async () => d.resolve(status({ indexedCommits: 10 })));
    expect(result.current.building).toBe(false);
    expect(result.current.progress).toBeNull();
    expect(result.current.status?.indexedCommits).toBe(10);
  });

  it('a second build() while building is a no-op', async () => {
    const d = deferred<IndexStatus>();
    const spy = vi.spyOn(mockIpc, 'historyIndexBuild').mockReturnValue(d.promise);
    const { result } = mount(makeDeps());
    act(() => result.current.build());
    act(() => result.current.build());
    expect(spy).toHaveBeenCalledTimes(1);
    await act(async () => d.resolve(status()));
  });

  it('build failure sets error, toasts, and resets building', async () => {
    vi.spyOn(mockIpc, 'historyIndexBuild').mockRejectedValue(appErr('git', 'index boom'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.build());
    await flush();
    expect(result.current.building).toBe(false);
    expect(result.current.error).toBe('index boom');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'index boom');
  });
});

describe('search (submit-only)', () => {
  it('empty/whitespace text clears without a request', async () => {
    const spy = vi.spyOn(mockIpc, 'historySearch').mockResolvedValue(hits('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.setText('   '));
    act(() => result.current.search());
    await flush();
    expect(spy).not.toHaveBeenCalled();
    expect(result.current.hits).toEqual([]);
    expect(result.current.searched).toBe(false);
  });

  it('typing alone never fires a request (submit-only)', async () => {
    const spy = vi.spyOn(mockIpc, 'historySearch').mockResolvedValue(hits('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.setText('why refactor'));
    await flush();
    expect(spy).not.toHaveBeenCalled();
  });

  it('success stores hits, marks searched, reveals the TOP hit, maps matchRows', async () => {
    const spy = vi.spyOn(mockIpc, 'historySearch').mockResolvedValue(hits('c', 'a', 'zzz'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.openPanel());
    act(() => result.current.setText(' why '));
    act(() => result.current.search());
    await flush();
    expect(spy).toHaveBeenCalledWith('/mock/repo', { text: 'why', topK: 0 });
    expect(result.current.hits.map((h) => h.oid)).toEqual(['c', 'a', 'zzz']);
    expect(result.current.searched).toBe(true);
    expect(deps.revealCommitByOid).toHaveBeenCalledWith('c');
    // 'zzz' not in the layout → dropped from the rings.
    expect(result.current.matchRows).toEqual([2, 0]);
  });

  it('matchRows is empty while the panel is closed', async () => {
    vi.spyOn(mockIpc, 'historySearch').mockResolvedValue(hits('a'));
    const { result } = mount(makeDeps());
    act(() => result.current.setText('q'));
    act(() => result.current.search());
    await flush();
    expect(result.current.hits.length).toBe(1);
    expect(result.current.matchRows).toEqual([]); // open === false
  });

  it('failure clears hits, sets error + searched, and toasts', async () => {
    vi.spyOn(mockIpc, 'historySearch').mockRejectedValue(appErr('git', 'no index'));
    const deps = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.setText('q'));
    act(() => result.current.search());
    await flush();
    expect(result.current.hits).toEqual([]);
    expect(result.current.error).toBe('no index');
    expect(result.current.searched).toBe(true);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'no index');
  });

  it('last-wins: a superseded retrieval is dropped; close() drops in-flight', async () => {
    const d1 = deferred<HistorySearchResults>();
    const d2 = deferred<HistorySearchResults>();
    vi.spyOn(mockIpc, 'historySearch')
      .mockReturnValueOnce(d1.promise)
      .mockReturnValueOnce(d2.promise);
    const { result } = mount(makeDeps());
    act(() => result.current.setText('one'));
    act(() => result.current.search());
    act(() => result.current.setText('two'));
    act(() => result.current.search());
    await act(async () => d2.resolve(hits('b')));
    await act(async () => d1.resolve(hits('a'))); // stale
    expect(result.current.hits.map((h) => h.oid)).toEqual(['b']);

    // close() bumps the reqId: a pending third search resolving after close is dropped.
    const d3 = deferred<HistorySearchResults>();
    vi.spyOn(mockIpc, 'historySearch').mockReturnValue(d3.promise);
    act(() => result.current.search());
    act(() => result.current.close());
    expect(result.current.searching).toBe(false);
    await act(async () => d3.resolve(hits('x')));
    expect(result.current.hits.map((h) => h.oid)).toEqual(['b']);
  });
});

describe('askAi / canAsk', () => {
  it('canAsk requires aiEligible AND a built index; askAi forwards question + topK', async () => {
    vi.spyOn(mockIpc, 'historyIndexStatus').mockResolvedValue(status({ built: true }));
    const deps = makeDeps({ aiEligible: true });
    const { result } = mount(deps);
    expect(result.current.canAsk).toBe(false); // no status yet
    act(() => result.current.openPanel());
    await flush();
    expect(result.current.canAsk).toBe(true);
    act(() => result.current.setText('  why was auth rewritten '));
    act(() => result.current.askAi());
    expect(deps.runAiAnswer).toHaveBeenCalledWith('why was auth rewritten', 0);
  });

  it('askAi is inert with empty text, without eligibility, or an unbuilt index', async () => {
    vi.spyOn(mockIpc, 'historyIndexStatus').mockResolvedValue(status({ built: false }));
    const deps = makeDeps({ aiEligible: true });
    const { result } = mount(deps);
    act(() => result.current.openPanel());
    await flush();
    expect(result.current.canAsk).toBe(false); // built:false
    act(() => result.current.setText('q'));
    act(() => result.current.askAi());
    expect(deps.runAiAnswer).not.toHaveBeenCalled();
  });
});
