/**
 * Render-storm fix, step 1 (fewer commits per round) — the load-bearing claim is
 * that a refresh round which fetched BYTE-IDENTICAL data commits NOTHING: no new
 * state, and so no new prop identity for any sidebar section or row to react to.
 *
 * Each `list_*` command returns a brand-new serde object every call, so the old
 * unconditional `setX(list)` re-rendered the whole sidebar on each of the ~43
 * debounced watcher rounds a 6-minute Dev session produced — changed data or
 * not. These tests assert both halves: no render when nothing moved, and a
 * render (with the new value) the moment something does.
 *
 * The suite runs with VITE_MOCK_IPC=1, so `ipc` IS `mockIpc` — spy on it.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { act, render } from '@testing-library/react';
import { mockIpc } from '../../ipc/mock';
import type { RemoteInfo, StashEntry, SubmoduleInfo, WorktreeInfo } from '../../ipc';
import { useSidebarCollections, type UseSidebarCollections } from './useSidebarCollections';

const REPO = 'repo-1';

const STASHES: StashEntry[] = [
  { index: 0, message: 'wip', oid: 'a'.repeat(40), baseOid: 'd'.repeat(40), ts: 1_700_000_000 },
];
const SUBMODULES: SubmoduleInfo[] = [
  {
    name: 'vendor/lib',
    path: 'vendor/lib',
    absPath: '/repo/vendor/lib',
    url: 'https://example.com/lib.git',
    headOid: 'b'.repeat(40),
    indexOid: 'b'.repeat(40),
    wtOid: 'b'.repeat(40),
    status: 'upToDate',
  },
];
const WORKTREES: WorktreeInfo[] = [
  {
    name: 'main',
    absPath: '/repo',
    relPath: null,
    branch: 'main',
    headOid: 'e'.repeat(40),
    locked: false,
    lockReason: null,
    isMain: true,
    isCurrent: true,
    prunable: false,
    valid: true,
  },
];
const REMOTES: RemoteInfo[] = [{ name: 'origin', url: 'https://example.com/r.git' }];

/** Deep copy, so every fetch hands the hook a FRESH object graph — exactly what
 *  the IPC layer does, and the whole point of the structural comparison. */
function fresh<T>(v: T): T {
  return structuredClone(v);
}

interface Harness {
  /** Re-read on every access — the hook's return value is per-render. */
  api(): UseSidebarCollections;
  renders(): number;
}

function mount(): Harness {
  let renders = 0;
  let latest: UseSidebarCollections | null = null;
  function Probe() {
    renders += 1;
    latest = useSidebarCollections(REPO);
    return null;
  }
  render(<Probe />);
  return {
    api: () => {
      if (latest === null) throw new Error('probe did not render');
      return latest;
    },
    renders: () => renders,
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe('a refresh round that found nothing new commits nothing', () => {
  it('does not re-render when all four collections come back identical', async () => {
    vi.spyOn(mockIpc, 'listStashes').mockImplementation(async () => fresh(STASHES));
    vi.spyOn(mockIpc, 'listSubmodules').mockImplementation(async () => fresh(SUBMODULES));
    vi.spyOn(mockIpc, 'listWorktrees').mockImplementation(async () => fresh(WORKTREES));
    vi.spyOn(mockIpc, 'listRemotes').mockImplementation(async () => fresh(REMOTES));

    const h = mount();
    // Round 1: first data ⇒ four commits (React batches them into one render).
    await act(async () => {
      await Promise.all([
        h.api().refetchStashes(),
        h.api().refetchSubmodules(),
        h.api().refetchWorktrees(),
        h.api().refetchRemotes(),
      ]);
    });
    expect(h.api().stashes).toEqual(STASHES);
    const afterFirst = h.renders();

    // Rounds 2..4: the same data, in fresh objects.
    const perRound: number[] = [];
    for (let i = 0; i < 3; i += 1) {
      const before = h.renders();
      await act(async () => {
        await Promise.all([
          h.api().refetchStashes(),
          h.api().refetchSubmodules(),
          h.api().refetchWorktrees(),
          h.api().refetchRemotes(),
        ]);
      });
      perRound.push(h.renders() - before);
    }

    // The first no-change round after ANY render of the owner still costs one
    // render: React's eager-bailout fast path in `dispatchSetState` is skipped
    // while the fiber's alternate carries lanes from the previous commit, so the
    // update is scheduled, the updater returns `prev`, and React bails DURING
    // that render rather than before it. Consecutive no-change rounds then cost
    // zero, which is what this `[1, 0, 0]` measures.
    //
    // DO NOT read this as "zero renders in the app". RepoWorkspace renders for
    // unrelated reasons (graph stream, selection, toasts) between watcher rounds,
    // so the eager path is usually cold and a no-change round usually costs ONE
    // container render. What the fix removes is the CASCADE: that render now bails
    // out at every memoised section and row instead of re-rendering 500 of them.
    expect(perRound).toEqual([1, 0, 0]);
    expect(h.renders()).toBe(afterFirst + 1);
    // And the state is still the real data, not an emptied placeholder.
    expect(h.api().submodules).toEqual(SUBMODULES);
    expect(h.api().worktrees).toEqual(WORKTREES);
    expect(h.api().remotes).toEqual(REMOTES);
  });

  it('keeps the SAME array reference across a no-change round (so rows can memo)', async () => {
    vi.spyOn(mockIpc, 'listStashes').mockImplementation(async () => fresh(STASHES));
    const h = mount();
    await act(async () => {
      await h.api().refetchStashes();
    });
    const first = h.api().stashes;
    await act(async () => {
      await h.api().refetchStashes();
    });
    expect(h.api().stashes).toBe(first);
  });

  it('still commits — and renders — as soon as the data really changes', async () => {
    let list = STASHES;
    vi.spyOn(mockIpc, 'listStashes').mockImplementation(async () => fresh(list));
    const h = mount();
    await act(async () => {
      await h.api().refetchStashes();
    });
    const before = h.renders();

    list = [
      ...STASHES,
      { index: 1, message: 'wip 2', oid: 'c'.repeat(40), baseOid: 'd'.repeat(40), ts: 1_700_000_100 },
    ];
    await act(async () => {
      await h.api().refetchStashes();
    });
    expect(h.renders()).toBeGreaterThan(before);
    expect(h.api().stashes).toHaveLength(2);
  });

  it('a one-field change deep in a payload is still seen', async () => {
    let list = SUBMODULES;
    vi.spyOn(mockIpc, 'listSubmodules').mockImplementation(async () => fresh(list));
    const h = mount();
    await act(async () => {
      await h.api().refetchSubmodules();
    });
    const before = h.renders();

    list = [{ ...SUBMODULES[0], status: 'modifiedWorkdir' }];
    await act(async () => {
      await h.api().refetchSubmodules();
    });
    expect(h.renders()).toBeGreaterThan(before);
    expect(h.api().submodules[0]?.status).toBe('modifiedWorkdir');
  });
});
