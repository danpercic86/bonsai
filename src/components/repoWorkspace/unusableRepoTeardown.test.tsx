/** P38 follow-up — the repo-went-unusable teardown.
 *
 *  `runRefreshRound`'s `full`-scope usability check runs this when the open repo
 *  stops being a usable repo (`.git` deleted / converted to bare). RepoWorkspace
 *  stays MOUNTED in that case (the other two `isUsableRepo` checks only refuse to
 *  *open* one), so anything rendered purely off state lingers over the emptied
 *  pane. These tests pin down that every slice is cleared and every overlay is
 *  closed — the gap that let the reflog overlay stay missing for milestones.
 *
 *  The last describes drive the REAL `useAiPanel` / `useHistorySearch` /
 *  `useCommitSearch` / `useReplayController` close helpers (not spies) to prove
 *  each leak was reachable and that the helpers' own guards suffice — a response
 *  landing after the teardown must not pop the panel back open.
 *
 *  The container-side wiring (that `runRefreshRound` actually calls this, with
 *  every close mirror assigned) is pinned separately in
 *  `unusableRepoTeardown.wiring.test.tsx`. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { effectiveMetrics } from '../../graph/metrics';
import { graphDisplayOf } from './displayModels';
import { mockIpc } from '../../ipc/mock';
import { CLOSE_MIRRORS, tearDownUnusableRepo } from './unusableRepoTeardown';
import { useAiPanel } from './useAiPanel';
import { useCommitSearch } from './useCommitSearch';
import { useCommitComposer } from './useCommitComposer';
import { useHistorySearch } from './useHistorySearch';
import { usePalette } from './usePalette';
import { useReplayController } from './replayProps';
import { stateSetter } from '../../test/actionHookKit';
import type {
  ComposeProposal,
  GraphLayout,
  GraphNode,
  HistoryAnswer,
  HistorySearchResults,
  IndexStatus,
  SearchResults,
} from '../../ipc';
import type { BlameState, HistoryState, ReflogState } from './types';
import type { UnusableRepoTeardownDeps } from './unusableRepoTeardown';

afterEach(() => vi.restoreAllMocks());

const SLICES = [
  'clearStatus',
  'clearGraph',
  'clearBranches',
  'clearStashes',
  'clearSubmodules',
  'clearWorktrees',
  'clearRemotes',
  'clearTagSync',
  'clearOpState',
  'clearCompare',
] as const;

function node(id: string): GraphNode {
  return { id, lane: 0, parents: [], summary: 's', author: 'a', ts: 0, committerTs: 0 };
}

/** A non-empty layout — `canReplay` and the search rings need real rows. */
function layoutOf(ids: string[]): GraphLayout {
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

function makeDeps(over: Partial<UnusableRepoTeardownDeps> = {}) {
  const blame = stateSetter<BlameState | null>(null);
  const history = stateSetter<HistoryState | null>(null);
  const reflog = stateSetter<ReflogState | null>(null);
  const historySearchClose = vi.fn();
  const commitSearchClose = vi.fn();
  const replayExit = vi.fn();
  const composerClose = vi.fn();
  const paletteClose = vi.fn();
  const deps: UnusableRepoTeardownDeps = {
    clearStatus: vi.fn(),
    clearGraph: vi.fn(),
    clearBranches: vi.fn(),
    clearStashes: vi.fn(),
    clearSubmodules: vi.fn(),
    clearWorktrees: vi.fn(),
    clearRemotes: vi.fn(),
    clearTagSync: vi.fn(),
    clearOpState: vi.fn(),
    clearCompare: vi.fn(),
    setBlame: blame.set,
    setHistory: history.set,
    setReflog: reflog.set,
    blameReqId: { current: 0 },
    historyReqId: { current: 0 },
    reflogReqId: { current: 0 },
    closeAiPanel: vi.fn(),
    collapseDiffSlot: vi.fn(),
    historySearchCloseRef: { current: historySearchClose },
    commitSearchCloseRef: { current: commitSearchClose },
    replayExitRef: { current: replayExit },
    composerCloseRef: { current: composerClose },
    paletteCloseRef: { current: paletteClose },
    ...over,
  };
  return {
    deps, blame, history, reflog,
    historySearchClose, commitSearchClose, replayExit, composerClose, paletteClose,
  };
}

describe('tearDownUnusableRepo — state slices', () => {
  it('clears all ten repo-scoped slices exactly once', () => {
    const { deps } = makeDeps();
    tearDownUnusableRepo(deps);
    for (const name of SLICES) {
      expect(deps[name], `${name} must run on the teardown`).toHaveBeenCalledTimes(1);
    }
  });
});

describe('tearDownUnusableRepo — read overlays', () => {
  it('nulls blame/history/reflog and bumps each reqId', () => {
    const { deps, blame, history, reflog } = makeDeps({
      blameReqId: { current: 2 },
      historyReqId: { current: 5 },
      reflogReqId: { current: 9 },
    });
    tearDownUnusableRepo(deps);
    expect(blame.set).toHaveBeenCalledWith(null);
    expect(history.set).toHaveBeenCalledWith(null);
    expect(reflog.set).toHaveBeenCalledWith(null);
    expect(deps.blameReqId.current).toBe(3);
    expect(deps.historyReqId.current).toBe(6);
    expect(deps.reflogReqId.current).toBe(10);
  });
});

describe('tearDownUnusableRepo — AI output panel', () => {
  it('calls the AI panel close helper', () => {
    const { deps } = makeDeps();
    tearDownUnusableRepo(deps);
    expect(deps.closeAiPanel).toHaveBeenCalledTimes(1);
  });
});

describe('tearDownUnusableRepo — mirrored overlays', () => {
  it('calls every ref-mirrored close helper', () => {
    const { deps } = makeDeps();
    tearDownUnusableRepo(deps);
    // Two halves, because driving purely off CLOSE_MIRRORS would make DELETING
    // an entry invisible (the loop would just iterate less): pin the list
    // contents, then assert every listed mirror actually fired. A 6th mirror
    // added to the module fails the first half until it is wired here too.
    expect([...CLOSE_MIRRORS]).toEqual([
      'historySearchCloseRef',
      'commitSearchCloseRef',
      'replayExitRef',
      'composerCloseRef',
      'paletteCloseRef',
    ]);
    for (const name of CLOSE_MIRRORS) {
      expect(deps[name].current, name).toHaveBeenCalledTimes(1);
    }
  });
});

describe('tearDownUnusableRepo — center diff overlay', () => {
  it('collapses the diff slot in its OWN right, not via clearStatus', () => {
    // The container's `clearStatus` also calls `collapseDiffSlot` today, so the
    // overlay is not currently leaking (see the module doc). This asserts the
    // teardown does not DEPEND on that: `makeDeps`'s `clearStatus` is a bare
    // spy — the status-only contract this interface actually promises — and the
    // slot still collapses. `conflict:` / `ai-proposal:` / `pr:` slots read
    // nothing from `status`, so without this call they would outlive the repo.
    const { deps } = makeDeps();
    tearDownUnusableRepo(deps);
    expect(deps.collapseDiffSlot).toHaveBeenCalledTimes(1);
  });
});

describe('tearDownUnusableRepo — an unwired close mirror is loud', () => {
  it('throws naming the unwired mirror instead of silently no-op-ing', () => {
    // The failure mode this guards: someone deletes the container's
    // `commitSearchCloseRef.current = search.close` line. A `useRef(() => {})`
    // default would leave every test green while the bar lingered.
    const { deps } = makeDeps({ commitSearchCloseRef: { current: null } });
    expect(() => tearDownUnusableRepo(deps)).toThrow(/commitSearchCloseRef/);
  });

  it('lists every unwired mirror in one error', () => {
    const { deps } = makeDeps({
      historySearchCloseRef: { current: null },
      replayExitRef: { current: null },
    });
    expect(() => tearDownUnusableRepo(deps)).toThrow(
      /historySearchCloseRef, replayExitRef/,
    );
  });

  it('still empties the slices and runs the WIRED mirrors before throwing', () => {
    // Non-fatal by design: `runRefreshRound` catches this into a
    // `Refresh failed: …` toast, so the rest of the teardown must have happened.
    const { deps, commitSearchClose, replayExit } = makeDeps({
      historySearchCloseRef: { current: null },
    });
    expect(() => tearDownUnusableRepo(deps)).toThrow();
    expect(deps.clearGraph).toHaveBeenCalledTimes(1);
    expect(deps.closeAiPanel).toHaveBeenCalledTimes(1);
    expect(commitSearchClose).toHaveBeenCalledTimes(1);
    expect(replayExit).toHaveBeenCalledTimes(1);
  });
});

describe('the AI panel leak (real useAiPanel.closeAiPanel)', () => {
  it('closes an open panel and drops the in-flight analysis', async () => {
    const d = deferred<HistoryAnswer>();
    vi.spyOn(mockIpc, 'aiSearchHistory').mockReturnValue(d.promise);
    const { result } = renderHook(() => useAiPanel('/mock/repo'));

    act(() => result.current.runHistoryAnswer('why?', 5));
    // Reachable: WorkspaceGraphPane renders the panel off `aiPanel !== null`.
    expect(result.current.aiPanel?.loading).toBe(true);

    const { deps } = makeDeps({ closeAiPanel: result.current.closeAiPanel });
    act(() => tearDownUnusableRepo(deps));
    expect(result.current.aiPanel).toBeNull();

    // closeAiPanel bumped aiPanelReqId, so the late reply is dropped.
    await act(async () => {
      d.resolve({ text: 'because', cited: [], retrieved: [], costUsd: 0 });
      await d.promise;
    });
    expect(result.current.aiPanel).toBeNull();
  });
});

describe('the history-search leak (real useHistorySearch.close)', () => {
  const layout: GraphLayout = {
    nodes: [],
    edges: [],
    laneCount: 0,
    headIndex: null,
    truncated: false,
  };
  const built: IndexStatus = {
    built: true,
    indexedCommits: 10,
    headOid: 'h',
    stale: false,
    newCommits: 0,
    schema: 1,
    builtAt: 1,
    skippedCommits: 0,
  };

  it('closes an open panel and drops the in-flight retrieval', async () => {
    vi.spyOn(mockIpc, 'historyIndexStatus').mockResolvedValue(built);
    const d = deferred<HistorySearchResults>();
    vi.spyOn(mockIpc, 'historySearch').mockReturnValue(d.promise);
    const reveal = vi.fn();
    const { result } = renderHook(() =>
      useHistorySearch({
        repoId: '/mock/repo',
        graph: layout,
        revealCommitByOid: reveal,
        aiEligible: false,
        runAiAnswer: vi.fn(),
        pushToast: vi.fn(),
      }),
    );

    await act(async () => {
      result.current.openPanel();
    });
    act(() => result.current.setText('lock contention'));
    act(() => result.current.search());
    // Reachable: WorkspaceGraphPane gates the panel on `historySearch.open`.
    expect(result.current.open).toBe(true);
    expect(result.current.searching).toBe(true);

    const { deps } = makeDeps({ historySearchCloseRef: { current: result.current.close } });
    act(() => tearDownUnusableRepo(deps));
    expect(result.current.open).toBe(false);
    expect(result.current.searching).toBe(false);

    // close() bumped searchReqId, so the late retrieval neither adopts hits nor
    // reveals a row in the (now emptied) graph.
    await act(async () => {
      d.resolve({ hits: [{ oid: 'a', summary: 's', authorName: 'a', authorTs: 0, score: 1 }], indexStale: false, indexedCommits: 10 });
      await d.promise;
    });
    expect(result.current.open).toBe(false);
    expect(result.current.hits).toEqual([]);
    expect(reveal).not.toHaveBeenCalled();
  });
});

describe('the commit-search leak (real useCommitSearch.close)', () => {
  type SearchDeps = Parameters<typeof useCommitSearch>[0];

  it('closes the bar and drops the in-flight search', async () => {
    const d = deferred<SearchResults>();
    vi.spyOn(mockIpc, 'searchCommits').mockReturnValue(d.promise);
    const reveal = vi.fn();
    const initial: SearchDeps = {
      repoId: '/mock/repo',
      graph: layoutOf(['a', 'b']),
      revealCommitByOid: reveal,
      pushToast: vi.fn(),
    };
    const { result, rerender } = renderHook((p: SearchDeps) => useCommitSearch(p), {
      initialProps: initial,
    });

    act(() => result.current.openSearch());
    // `content` is the submit-only field — no debounce timer to fight here.
    act(() => result.current.patchQuery({ field: 'content', text: 'lock contention' }));
    act(() => result.current.submit());
    // Reachable: WorkspaceGraphPane renders CommitSearchBar off `search.open`.
    expect(result.current.open).toBe(true);
    expect(result.current.loading).toBe(true);

    // …and `clearGraph()` alone cannot close it: `open` is its own state, so the
    // bar would sit over an emptied pane. Hence the teardown must close it.
    rerender({ ...initial, graph: null });
    expect(result.current.open).toBe(true);

    const { deps } = makeDeps({ commitSearchCloseRef: { current: result.current.close } });
    act(() => tearDownUnusableRepo(deps));
    expect(result.current.open).toBe(false);
    expect(result.current.loading).toBe(false);

    // close() bumped reqIdRef, so the late matches are neither adopted nor revealed.
    await act(async () => {
      d.resolve({
        matches: [{ oid: 'a', summary: 's', authorName: 'a', authorTs: 0, matched: 'message' }],
        truncated: false,
      });
      await d.promise;
    });
    expect(result.current.open).toBe(false);
    expect(result.current.results).toBeNull();
    expect(reveal).not.toHaveBeenCalled();
  });
});

describe('the replay-overlay leak (real useReplayController.onExit)', () => {
  type ReplayDeps = Parameters<typeof useReplayController>[0];

  const base: Omit<ReplayDeps, 'graph'> = {
    metrics: effectiveMetrics(DEFAULT_UI_SETTINGS.graph),
    metricsVersion: 1,
    display: graphDisplayOf('lane', DEFAULT_UI_SETTINGS.graph, new Map(), new Map(), new Map()),
    graphStyle: 'standard',
    graphSeason: 'living',
    themeVersion: 1,
    reducedMotion: true,
    pushToast: vi.fn(),
  };

  it('drops the entry snapshot that survives clearGraph()', () => {
    const initial: ReplayDeps = { ...base, graph: layoutOf(['a', 'b']) };
    const { result, rerender } = renderHook((p: ReplayDeps) => useReplayController(p), {
      initialProps: initial,
    });

    act(() => result.current.onOpen());
    // Reachable: WorkspaceGraphPane renders ReplayMode off `replay.mode !== null`.
    expect(result.current.mode).not.toBeNull();

    // The overlay renders from the ENTRY SNAPSHOT state, not the live graph — so
    // `clearGraph()` leaves it up. Only the teardown's exit call closes it.
    rerender({ ...initial, graph: null });
    expect(result.current.mode).not.toBeNull();

    const { deps } = makeDeps({ replayExitRef: { current: result.current.onExit } });
    act(() => tearDownUnusableRepo(deps));
    expect(result.current.mode).toBeNull();
    expect(result.current.open).toBe(false);
  });
});

describe('the commit-composer leak (real useCommitComposer.close)', () => {
  type ComposerDeps = Parameters<typeof useCommitComposer>[0];

  const proposal: ComposeProposal = {
    groups: [{ files: ['a.ts'], message: 'feat: a' }],
    unassigned: [],
    notes: [],
    costUsd: null,
  };

  it('closes the dialog and drops the in-flight proposal', async () => {
    const d = deferred<ComposeProposal>();
    vi.spyOn(mockIpc, 'aiComposeCommits').mockReturnValue(d.promise);
    const initial: ComposerDeps = {
      repoId: '/mock/repo',
      refreshAll: vi.fn(),
      pushToast: vi.fn(),
      previewFileDiff: vi.fn(),
    };
    const { result } = renderHook((p: ComposerDeps) => useCommitComposer(p), {
      initialProps: initial,
    });

    act(() => result.current.openComposer());
    // Reachable: WorkspaceOverlays renders ComposerDialog off `composer.open`.
    expect(result.current.open).toBe(true);
    expect(result.current.loading).toBe(true);

    // The hook reads NO repo slice — it takes only repoId + callbacks, and has
    // no effect that could close it — so no slice clear can reach it. A dialog
    // offering "Create N commits" against a dead repo is exactly the leak; only
    // the teardown's close call ends it. (No adversarial rerender here: no prop
    // change could close this dialog, so one would be theater.)

    const { deps } = makeDeps({ composerCloseRef: { current: result.current.close } });
    act(() => tearDownUnusableRepo(deps));
    expect(result.current.open).toBe(false);
    expect(result.current.loading).toBe(false);

    // close() -> resetState() bumped reqIdRef, so the late proposal is dropped.
    // `groups` is the load-bearing half — the propose resolver never calls
    // setOpen, so reopening was never the risk here.
    await act(async () => {
      d.resolve(proposal);
      await d.promise;
    });
    expect(result.current.open).toBe(false);
    expect(result.current.groups).toEqual([]);
  });
});

describe('the command-palette leak (real usePalette.close)', () => {
  it('closes an open palette on an ACTIVE tab', () => {
    const { result, rerender } = renderHook((p: { active: boolean }) => usePalette(p), {
      initialProps: { active: true },
    });

    act(() => result.current.toggle());
    // Reachable: WorkspaceOverlays passes `paletteOpen` straight to CommandPalette.
    expect(result.current.open).toBe(true);

    // The hook's ONE force-close path is deactivation — prove that, then prove
    // it does not cover this case: reopen on a tab that stays active and the
    // palette survives, registry still merging the repo-scoped actions with
    // App's global ones.
    rerender({ active: false });
    expect(result.current.open).toBe(false);
    rerender({ active: true });
    act(() => result.current.toggle());
    expect(result.current.open).toBe(true);

    const { deps } = makeDeps({ paletteCloseRef: { current: result.current.close } });
    act(() => tearDownUnusableRepo(deps));
    expect(result.current.open).toBe(false);
    expect(result.current.openRef.current).toBe(false);
  });
});
