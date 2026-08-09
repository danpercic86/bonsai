/** T3.2b — useReadOverlays: blame/file-history/reflog open+close state machine,
 *  cross-invalidation, reqId stale-guards, reveal-by-oid, reflog-restore refetch. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { useReadOverlays } from './useReadOverlays';
import { appErr, stateSetter } from '../../test/actionHookKit';
import type { BlameLine, GraphLayout, GraphNode, ReflogEntry } from '../../ipc';
import type { BlameState, HistoryState, ReflogState } from './types';

afterEach(() => vi.restoreAllMocks());

type Deps = Parameters<typeof useReadOverlays>[0];

function node(id: string): GraphNode {
  return { id, lane: 0, parents: [], summary: '', author: '', ts: 0, committerTs: 0 };
}
function layout(ids: string[]): GraphLayout {
  return { nodes: ids.map(node), edges: [], laneCount: 1, headIndex: null, truncated: false };
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

function makeDeps(over: Partial<Deps> = {}) {
  const blame = stateSetter<BlameState | null>(null);
  const history = stateSetter<HistoryState | null>(null);
  const reflog = stateSetter<ReflogState | null>(null);
  const deps: Deps = {
    repoId: '/mock/repo',
    pushToast: vi.fn(),
    mutating: false,
    setBlame: blame.set,
    setHistory: history.set,
    setReflog: reflog.set,
    blameReqId: { current: 0 },
    historyReqId: { current: 0 },
    reflogReqId: { current: 0 },
    reflogRef: { current: null },
    reflogRestoreRef: { current: false },
    graphDataRef: { current: layout(['a', 'b', 'c']) },
    compareRef: { current: null },
    clearCompare: vi.fn(),
    setSelectedIndex: vi.fn(),
    ...over,
  };
  return { deps, blame, history, reflog };
}

function mount(deps: Deps) {
  return renderHook((d: Deps) => useReadOverlays(d), { initialProps: deps });
}

describe('handleBlame', () => {
  it('loads: sets loading, cross-invalidates siblings, then stores lines', async () => {
    const lines = [{ path: 'f.ts' } as unknown as BlameLine];
    vi.spyOn(mockIpc, 'blameFile').mockResolvedValue(lines);
    const { deps, blame, history, reflog } = makeDeps({
      historyReqId: { current: 3 },
      reflogReqId: { current: 7 },
    });
    const { result } = mount(deps);
    await act(async () => result.current.handleBlame('f.ts'));
    // Siblings invalidated + closed.
    expect(deps.historyReqId.current).toBe(4);
    expect(deps.reflogReqId.current).toBe(8);
    expect(history.set).toHaveBeenCalledWith(null);
    expect(reflog.set).toHaveBeenCalledWith(null);
    // Loading state first, then the data.
    expect(blame.set).toHaveBeenNthCalledWith(1, {
      path: 'f.ts',
      lines: [],
      loading: true,
      error: null,
    });
    expect(blame.box.current).toEqual({ path: 'f.ts', lines, loading: false, error: null });
    expect(mockIpc.blameFile).toHaveBeenCalledWith('/mock/repo', 'f.ts', null);
  });

  it('failure stores the error message in the overlay state (no toast)', async () => {
    vi.spyOn(mockIpc, 'blameFile').mockRejectedValue(appErr('git', 'binary file'));
    const { deps, blame } = makeDeps();
    const { result } = mount(deps);
    await act(async () => result.current.handleBlame('img.png'));
    expect(blame.box.current).toEqual({
      path: 'img.png',
      lines: [],
      loading: false,
      error: 'binary file',
    });
    expect(deps.pushToast).not.toHaveBeenCalled();
  });

  it('closeBlame during the fetch drops the result (reqId stale-guard)', async () => {
    const d = deferred<BlameLine[]>();
    vi.spyOn(mockIpc, 'blameFile').mockReturnValue(d.promise);
    const { deps, blame } = makeDeps();
    const { result } = mount(deps);
    let p!: Promise<void>;
    act(() => {
      p = result.current.handleBlame('f.ts');
    });
    act(() => result.current.closeBlame());
    expect(blame.box.current).toBeNull();
    await act(async () => {
      d.resolve([]);
      await p;
    });
    expect(blame.box.current).toBeNull(); // closed overlay must not pop back open
  });
});

describe('handleFileHistory', () => {
  it('loads entries and cross-invalidates blame + reflog', async () => {
    vi.spyOn(mockIpc, 'fileHistory').mockResolvedValue([]);
    const { deps, blame, history, reflog } = makeDeps({ blameReqId: { current: 1 } });
    const { result } = mount(deps);
    await act(async () => result.current.handleFileHistory('f.ts'));
    expect(deps.blameReqId.current).toBe(2);
    expect(blame.set).toHaveBeenCalledWith(null);
    expect(reflog.set).toHaveBeenCalledWith(null);
    expect(history.box.current).toEqual({ path: 'f.ts', entries: [], loading: false, error: null });
  });

  it('failure lands in the overlay error field', async () => {
    vi.spyOn(mockIpc, 'fileHistory').mockRejectedValue(appErr('git', 'gone'));
    const { deps, history } = makeDeps();
    const { result } = mount(deps);
    await act(async () => result.current.handleFileHistory('f.ts'));
    expect(history.box.current?.error).toBe('gone');
    expect(history.box.current?.loading).toBe(false);
  });
});

describe('openReflog', () => {
  it('loads entries for the ref and cross-invalidates blame + history', async () => {
    const entries = [{ message: 'reset' } as unknown as ReflogEntry];
    vi.spyOn(mockIpc, 'readReflog').mockResolvedValue(entries);
    const { deps, blame, history, reflog } = makeDeps();
    const { result } = mount(deps);
    await act(async () => result.current.openReflog('HEAD'));
    expect(blame.set).toHaveBeenCalledWith(null);
    expect(history.set).toHaveBeenCalledWith(null);
    expect(reflog.box.current).toEqual({ refName: 'HEAD', entries, loading: false, error: null });
    expect(mockIpc.readReflog).toHaveBeenCalledWith('/mock/repo', 'HEAD');
  });

  it('failure lands in the overlay error field; closeReflog drops in-flight', async () => {
    vi.spyOn(mockIpc, 'readReflog').mockRejectedValue(appErr('git', 'no reflog'));
    const { deps, reflog } = makeDeps();
    const { result } = mount(deps);
    await act(async () => result.current.openReflog('feat'));
    expect(reflog.box.current?.error).toBe('no reflog');

    const d = deferred<ReflogEntry[]>();
    vi.spyOn(mockIpc, 'readReflog').mockReturnValue(d.promise);
    let p!: Promise<void>;
    act(() => {
      p = result.current.openReflog('feat');
    });
    act(() => result.current.closeReflog());
    await act(async () => {
      d.resolve([]);
      await p;
    });
    expect(reflog.box.current).toBeNull();
  });
});

describe('revealCommitByOid', () => {
  it('selects the row, exits compare, and closes any open overlay', () => {
    const { deps } = makeDeps({ compareRef: { current: { oid: 'a' } } });
    const { result } = mount(deps);
    const before = {
      blame: deps.blameReqId.current,
      history: deps.historyReqId.current,
      reflog: deps.reflogReqId.current,
    };
    act(() => result.current.revealCommitByOid('b'));
    expect(deps.clearCompare).toHaveBeenCalledTimes(1);
    expect(deps.setSelectedIndex).toHaveBeenCalledWith(1);
    // close helpers ran → every overlay reqId bumped.
    expect(deps.blameReqId.current).toBe(before.blame + 1);
    expect(deps.historyReqId.current).toBe(before.history + 1);
    expect(deps.reflogReqId.current).toBe(before.reflog + 1);
    expect(deps.pushToast).not.toHaveBeenCalled();
  });

  it('does not clear compare when not comparing', () => {
    const { deps } = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.revealCommitByOid('a'));
    expect(deps.clearCompare).not.toHaveBeenCalled();
    expect(deps.setSelectedIndex).toHaveBeenCalledWith(0);
  });

  it('unknown oid → info toast, no selection change', () => {
    const { deps } = makeDeps();
    const { result } = mount(deps);
    act(() => result.current.revealCommitByOid('nope'));
    expect(deps.pushToast).toHaveBeenCalledWith('info', 'Commit not in the current view');
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
  });

  it('no graph yet → no-op', () => {
    const { deps } = makeDeps({ graphDataRef: { current: null } });
    const { result } = mount(deps);
    act(() => result.current.revealCommitByOid('a'));
    expect(deps.setSelectedIndex).not.toHaveBeenCalled();
    expect(deps.pushToast).not.toHaveBeenCalled();
  });
});

describe('reflog-restore refetch effect', () => {
  it('re-fetches the OPEN reflog when mutating falls with the restore flag armed', async () => {
    const spy = vi.spyOn(mockIpc, 'readReflog').mockResolvedValue([]);
    const { deps, reflog } = makeDeps({
      mutating: true,
      reflogRestoreRef: { current: true },
      reflogRef: { current: { refName: 'HEAD', entries: [], loading: false, error: null } },
    });
    const h = mount(deps);
    expect(spy).not.toHaveBeenCalled();
    await act(async () => h.rerender({ ...deps, mutating: false }));
    expect(spy).toHaveBeenCalledWith('/mock/repo', 'HEAD');
    expect(deps.reflogRestoreRef.current).toBe(false); // one-shot
    expect(reflog.box.current?.refName).toBe('HEAD');
  });

  it('does nothing when the flag is not armed or no reflog is open', async () => {
    const spy = vi.spyOn(mockIpc, 'readReflog').mockResolvedValue([]);
    // Flag off:
    const a = makeDeps({ mutating: true });
    const ha = mount(a.deps);
    await act(async () => ha.rerender({ ...a.deps, mutating: false }));
    // Flag on but overlay closed (flag still consumed):
    const b = makeDeps({ mutating: true, reflogRestoreRef: { current: true } });
    const hb = mount(b.deps);
    await act(async () => hb.rerender({ ...b.deps, mutating: false }));
    expect(spy).not.toHaveBeenCalled();
    expect(b.deps.reflogRestoreRef.current).toBe(false);
  });
});
