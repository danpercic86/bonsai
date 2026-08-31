/** Audit 2026-08-18 §4.2 — usePartialStaging (extracted in cb85e55 with zero
 *  guard tests). Covers the full public surface: the two overlay refetch
 *  toggles (view mode / intraline), hunk+line stage/unstage, the two-step
 *  arm-then-confirm discard flows, the busy/stale guards, and error routing
 *  to reportStatusError. House pattern: actionHookKit + mockIpc spies (T3.2a)
 *  with renderHook mounting (T3.2b) — this hook is useCallback-based. */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { usePartialStaging } from './usePartialStaging';
import { appErr, asyncFn, expectMutatingCycle, REPO } from '../../test/actionHookKit';
import type { FileDiff, LineSelection } from '../../ipc';
import type { DiffOverlayMeta } from '../DiffOverlay';
import type { DiffSlot } from '../StatusPanel';

afterEach(() => vi.restoreAllMocks());

type Deps = Parameters<typeof usePartialStaging>[0];

/* ── fixtures ────────────────────────────────────────────────────────────── */

function meta(kind: DiffOverlayMeta['kind'], origPath: string | null = null): DiffOverlayMeta {
  return { path: 'src/app.ts', origPath, status: 'modified', kind };
}

/** One hunk: context / add / del / context — the add+del pair is what every
 *  hunk-level selection builder must extract (context dropped). */
function mkDiff(): FileDiff {
  return {
    path: 'src/app.ts',
    origPath: null,
    status: 'modified',
    binary: false,
    tooLarge: false,
    hunks: [
      {
        oldStart: 1,
        oldLines: 3,
        newStart: 1,
        newLines: 3,
        lines: [
          { kind: 'context', oldNo: 1, newNo: 1, content: 'a' },
          { kind: 'add', oldNo: null, newNo: 2, content: 'b' },
          { kind: 'del', oldNo: 2, newNo: null, content: 'c' },
          { kind: 'context', oldNo: 3, newNo: 3, content: 'd' },
        ],
      },
      // A context-only hunk: its built selection is empty (guards must bail).
      {
        oldStart: 10,
        oldLines: 1,
        newStart: 10,
        newLines: 1,
        lines: [{ kind: 'context', oldNo: 10, newNo: 10, content: 'e' }],
      },
    ],
  };
}

/** The selection handleStageHunk/handleConfirmHunkDiscard build from hunk 0. */
const HUNK0_SELECTION: LineSelection[] = [
  { kind: 'add', oldNo: null, newNo: 2 },
  { kind: 'del', oldNo: 2, newNo: null },
];

function slot(diff: FileDiff | null = mkDiff()): DiffSlot {
  return { key: 'unstaged:src/app.ts', state: 'ready', diff, error: null };
}

/** fetchDiffSlot spy that RUNS the fetcher so the inner ipc call is observable. */
function runFetcher() {
  return vi.fn(async (_key: string, fetcher: () => Promise<FileDiff>) => {
    await fetcher();
  });
}

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    repoId: REPO,
    setMutating: vi.fn(),
    mutatingRef: { current: false },
    overlayMetaRef: { current: meta('unstaged') },
    diffSlotRef: { current: slot() },
    stageableRef: { current: 'stage' },
    diffViewModeRef: { current: 'diff' },
    intralineRef: { current: false },
    prOverlayCtxRef: { current: null },
    setDiffViewMode: vi.fn(),
    setIntraline: vi.fn(),
    setPendingHunkDiscard: vi.fn(),
    setPendingLineDiscard: vi.fn(),
    fetchDiffSlot: runFetcher(),
    refetchStatus: asyncFn(),
    reportStatusError: vi.fn(),
    ...over,
  };
}

/** The hook is useCallback-based (unlike the plain-function T3.2a hooks), so it
 *  must run inside a component — renderHook, per the T3.2b suites. It has no
 *  internal React state (all effects flow through the deps' setters/refs), so
 *  the returned handlers can be awaited directly without act(). */
function mount(deps: Deps) {
  return renderHook((d: Deps) => usePartialStaging(d), { initialProps: deps }).result.current;
}

/* ── handleSetViewMode ───────────────────────────────────────────────────── */

describe('handleSetViewMode', () => {
  it('file mode on an unstaged workdir slot → refetch under the SAME key with fullContext', async () => {
    const get = vi.spyOn(mockIpc, 'getWorkdirFileDiff').mockResolvedValue(mkDiff());
    const deps = makeDeps();
    mount(deps).handleSetViewMode('file');
    expect(deps.setDiffViewMode).toHaveBeenCalledWith('file');
    expect(deps.fetchDiffSlot).toHaveBeenCalledWith('unstaged:src/app.ts', expect.any(Function));
    await Promise.resolve(); // let the void fetch settle
    expect(get).toHaveBeenCalledWith(REPO, 'src/app.ts', null, false, true, false);
  });

  it('staged meta → staged=true; diff mode → fullContext=false; intraline read from its ref', async () => {
    const get = vi.spyOn(mockIpc, 'getWorkdirFileDiff').mockResolvedValue(mkDiff());
    const deps = makeDeps({
      overlayMetaRef: { current: meta('staged') },
      intralineRef: { current: true },
    });
    mount(deps).handleSetViewMode('diff');
    await Promise.resolve();
    expect(get).toHaveBeenCalledWith(REPO, 'src/app.ts', null, true, false, true);
  });

  it('no open slot → sets the mode but never fetches', () => {
    const get = vi.spyOn(mockIpc, 'getWorkdirFileDiff');
    const deps = makeDeps({ diffSlotRef: { current: null } });
    mount(deps).handleSetViewMode('split');
    expect(deps.setDiffViewMode).toHaveBeenCalledWith('split');
    expect(deps.fetchDiffSlot).not.toHaveBeenCalled();
    expect(get).not.toHaveBeenCalled();
  });

  it('conflict/commit slots are not FileDiffs → no refetch', () => {
    const deps = makeDeps({ overlayMetaRef: { current: meta('conflict') } });
    mount(deps).handleSetViewMode('file');
    expect(deps.fetchDiffSlot).not.toHaveBeenCalled();
  });
});

/* ── handleToggleIntraline ───────────────────────────────────────────────── */

describe('handleToggleIntraline', () => {
  it('refetches the open workdir slot with the NEW intraline flag and the current view mode', async () => {
    const get = vi.spyOn(mockIpc, 'getWorkdirFileDiff').mockResolvedValue(mkDiff());
    const deps = makeDeps({ diffViewModeRef: { current: 'file' } });
    mount(deps).handleToggleIntraline(true);
    expect(deps.setIntraline).toHaveBeenCalledWith(true);
    expect(deps.fetchDiffSlot).toHaveBeenCalledWith('unstaged:src/app.ts', expect.any(Function));
    await Promise.resolve();
    expect(get).toHaveBeenCalledWith(REPO, 'src/app.ts', null, false, true, true);
  });

  it('no overlay open → toggles the flag only', () => {
    const deps = makeDeps({ overlayMetaRef: { current: null } });
    mount(deps).handleToggleIntraline(true);
    expect(deps.setIntraline).toHaveBeenCalledWith(true);
    expect(deps.fetchDiffSlot).not.toHaveBeenCalled();
  });
});

/* ── P93: pr slot refetch ────────────────────────────────────────────────── */

/** A `pr:` slot + its ctx side-channel (the key alone loses status/origPath). */
const PR_SLOT_KEY = `pr:${'a'.repeat(40)}:${'b'.repeat(40)}:src/app.ts`;
function prDeps(over: Partial<Deps> = {}): Deps {
  return makeDeps({
    overlayMetaRef: { current: meta('pr', 'src/old.ts') },
    diffSlotRef: { current: { ...slot(), key: PR_SLOT_KEY } },
    prOverlayCtxRef: {
      current: {
        prNumber: 42,
        baseOid: 'a'.repeat(40),
        headOid: 'b'.repeat(40),
        path: 'src/app.ts',
        origPath: 'src/old.ts',
        status: 'renamed',
      },
    },
    ...over,
  });
}

describe('pr slot toggles (P93 §5.3)', () => {
  it('File view refetches forgePrFileDiff under the SAME key with fullContext', async () => {
    const get = vi.spyOn(mockIpc, 'forgePrFileDiff').mockResolvedValue(mkDiff());
    const deps = prDeps();
    mount(deps).handleSetViewMode('file');
    expect(deps.fetchDiffSlot).toHaveBeenCalledWith(PR_SLOT_KEY, expect.any(Function));
    await Promise.resolve();
    expect(get).toHaveBeenCalledWith(
      REPO,
      'a'.repeat(40),
      'b'.repeat(40),
      'src/app.ts',
      'src/old.ts',
      true,
      false,
    );
  });

  it('Highlight changes refetches with the new intraline flag', async () => {
    const get = vi.spyOn(mockIpc, 'forgePrFileDiff').mockResolvedValue(mkDiff());
    const deps = prDeps({ diffViewModeRef: { current: 'file' } });
    mount(deps).handleToggleIntraline(true);
    await Promise.resolve();
    expect(get).toHaveBeenCalledWith(
      REPO,
      'a'.repeat(40),
      'b'.repeat(40),
      'src/app.ts',
      'src/old.ts',
      true,
      true,
    );
  });

  it('a pr slot with no ctx never refetches (and never as a workdir diff)', () => {
    const workdir = vi.spyOn(mockIpc, 'getWorkdirFileDiff');
    const deps = prDeps({ prOverlayCtxRef: { current: null } });
    mount(deps).handleSetViewMode('file');
    expect(deps.fetchDiffSlot).not.toHaveBeenCalled();
    expect(workdir).not.toHaveBeenCalled();
  });
});

/* ── handleStageLines ────────────────────────────────────────────────────── */

describe('handleStageLines', () => {
  it("direction 'stage' → stagePartial + refetchStatus + mutating cycle", async () => {
    const stage = vi.spyOn(mockIpc, 'stagePartial').mockResolvedValue(undefined);
    const deps = makeDeps();
    await mount(deps).handleStageLines(HUNK0_SELECTION);
    expect(stage).toHaveBeenCalledWith(REPO, 'src/app.ts', null, HUNK0_SELECTION);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it("direction 'unstage' → unstagePartial with the overlay's rename origPath", async () => {
    const unstage = vi.spyOn(mockIpc, 'unstagePartial').mockResolvedValue(undefined);
    const deps = makeDeps({
      overlayMetaRef: { current: meta('staged', 'src/old.ts') },
      stageableRef: { current: 'unstage' },
    });
    await mount(deps).handleStageLines(HUNK0_SELECTION);
    expect(unstage).toHaveBeenCalledWith(REPO, 'src/app.ts', 'src/old.ts', HUNK0_SELECTION);
  });

  it('empty selection → no IPC, no mutating flip', async () => {
    const stage = vi.spyOn(mockIpc, 'stagePartial');
    const deps = makeDeps();
    await mount(deps).handleStageLines([]);
    expect(stage).not.toHaveBeenCalled();
    expect(deps.setMutating).not.toHaveBeenCalled();
  });

  it('busy (mutatingRef) → serialized: the second call is dropped', async () => {
    const stage = vi.spyOn(mockIpc, 'stagePartial');
    const deps = makeDeps({ mutatingRef: { current: true } });
    await mount(deps).handleStageLines(HUNK0_SELECTION);
    expect(stage).not.toHaveBeenCalled();
    expect(deps.setMutating).not.toHaveBeenCalled();
  });

  it('overlay closed or not partial-able (meta/stageable null) → no-op', async () => {
    const stage = vi.spyOn(mockIpc, 'stagePartial');
    await mount(makeDeps({ overlayMetaRef: { current: null } })).handleStageLines(
      HUNK0_SELECTION,
    );
    await mount(makeDeps({ stageableRef: { current: null } })).handleStageLines(
      HUNK0_SELECTION,
    );
    expect(stage).not.toHaveBeenCalled();
  });

  it('error → routed to reportStatusError, mutating cleared, status NOT refetched', async () => {
    vi.spyOn(mockIpc, 'stagePartial').mockRejectedValue(appErr('git', 'stale selection'));
    const deps = makeDeps();
    await mount(deps).handleStageLines(HUNK0_SELECTION);
    expect(deps.reportStatusError).toHaveBeenCalledWith('stale selection');
    expect(deps.refetchStatus).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

/* ── handleStageHunk ─────────────────────────────────────────────────────── */

describe('handleStageHunk', () => {
  it('builds the add/del selection from the hunk (context dropped) and stages it', async () => {
    const stage = vi.spyOn(mockIpc, 'stagePartial').mockResolvedValue(undefined);
    const deps = makeDeps();
    mount(deps).handleStageHunk(0);
    await vi.waitFor(() => expect(stage).toHaveBeenCalled());
    expect(stage).toHaveBeenCalledWith(REPO, 'src/app.ts', null, HUNK0_SELECTION);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);
  });

  it('unknown hunk index / no diff → nothing happens', () => {
    const stage = vi.spyOn(mockIpc, 'stagePartial');
    mount(makeDeps()).handleStageHunk(99);
    mount(makeDeps({ diffSlotRef: { current: slot(null) } })).handleStageHunk(0);
    expect(stage).not.toHaveBeenCalled();
  });
});

/* ── discard: arm + confirm (hunk) ───────────────────────────────────────── */

describe('handleDiscardHunk / handleConfirmHunkDiscard', () => {
  it('arm: NEVER acts directly — only arms the ConfirmDialog payload', () => {
    const discard = vi.spyOn(mockIpc, 'discardPartial');
    const deps = makeDeps({ overlayMetaRef: { current: meta('unstaged', 'src/old.ts') } });
    mount(deps).handleDiscardHunk(0);
    expect(deps.setPendingHunkDiscard).toHaveBeenCalledWith({
      path: 'src/app.ts',
      origPath: 'src/old.ts',
      hunkIndex: 0,
    });
    expect(discard).not.toHaveBeenCalled();
    expect(deps.setMutating).not.toHaveBeenCalled();
  });

  it('arm with no overlay → nothing armed', () => {
    const deps = makeDeps({ overlayMetaRef: { current: null } });
    mount(deps).handleDiscardHunk(0);
    expect(deps.setPendingHunkDiscard).not.toHaveBeenCalled();
  });

  it('confirm: rebuilds the hunk selection and discards it + refetch + mutating cycle', async () => {
    const discard = vi.spyOn(mockIpc, 'discardPartial').mockResolvedValue(undefined);
    const deps = makeDeps();
    await mount(deps).handleConfirmHunkDiscard({
      path: 'src/app.ts',
      origPath: null,
      hunkIndex: 0,
    });
    expect(discard).toHaveBeenCalledWith(REPO, 'src/app.ts', null, HUNK0_SELECTION);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('confirm guards: busy / overlay moved to another file / hunk gone / context-only hunk', async () => {
    const discard = vi.spyOn(mockIpc, 'discardPartial');
    const pending = { path: 'src/app.ts', origPath: null, hunkIndex: 0 };
    await mount(makeDeps({ mutatingRef: { current: true } })).handleConfirmHunkDiscard(
      pending,
    );
    await mount(
      makeDeps({ overlayMetaRef: { current: meta('unstaged') } }),
    ).handleConfirmHunkDiscard({ ...pending, path: 'src/OTHER.ts' });
    await mount(makeDeps()).handleConfirmHunkDiscard({ ...pending, hunkIndex: 99 });
    await mount(makeDeps()).handleConfirmHunkDiscard({ ...pending, hunkIndex: 1 });
    expect(discard).not.toHaveBeenCalled();
  });

  it('confirm error → reportStatusError + mutating cleared', async () => {
    vi.spyOn(mockIpc, 'discardPartial').mockRejectedValue(appErr('other', 'diff changed'));
    const deps = makeDeps();
    await mount(deps).handleConfirmHunkDiscard({
      path: 'src/app.ts',
      origPath: null,
      hunkIndex: 0,
    });
    expect(deps.reportStatusError).toHaveBeenCalledWith('diff changed');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

/* ── discard: arm + confirm (lines) ──────────────────────────────────────── */

describe('handleDiscardLines / handleConfirmLineDiscard', () => {
  it('arm: captures the selection VERBATIM (lines cannot be re-derived later)', () => {
    const deps = makeDeps();
    mount(deps).handleDiscardLines(HUNK0_SELECTION);
    expect(deps.setPendingLineDiscard).toHaveBeenCalledWith({
      path: 'src/app.ts',
      origPath: null,
      selection: HUNK0_SELECTION,
    });
  });

  it('arm guards: empty selection / no overlay → nothing armed', () => {
    const a = makeDeps();
    mount(a).handleDiscardLines([]);
    expect(a.setPendingLineDiscard).not.toHaveBeenCalled();
    const b = makeDeps({ overlayMetaRef: { current: null } });
    mount(b).handleDiscardLines(HUNK0_SELECTION);
    expect(b.setPendingLineDiscard).not.toHaveBeenCalled();
  });

  it('confirm: discards exactly the stored selection + refetch + mutating cycle', async () => {
    const discard = vi.spyOn(mockIpc, 'discardPartial').mockResolvedValue(undefined);
    const deps = makeDeps();
    await mount(deps).handleConfirmLineDiscard({
      path: 'src/app.ts',
      origPath: null,
      selection: HUNK0_SELECTION,
    });
    expect(discard).toHaveBeenCalledWith(REPO, 'src/app.ts', null, HUNK0_SELECTION);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('confirm guards: busy / stale path / empty selection', async () => {
    const discard = vi.spyOn(mockIpc, 'discardPartial');
    const pending = { path: 'src/app.ts', origPath: null, selection: HUNK0_SELECTION };
    await mount(makeDeps({ mutatingRef: { current: true } })).handleConfirmLineDiscard(
      pending,
    );
    await mount(makeDeps()).handleConfirmLineDiscard({
      ...pending,
      path: 'src/OTHER.ts',
    });
    await mount(makeDeps()).handleConfirmLineDiscard({ ...pending, selection: [] });
    expect(discard).not.toHaveBeenCalled();
  });

  it('confirm error → reportStatusError + mutating cleared, no refetch', async () => {
    vi.spyOn(mockIpc, 'discardPartial').mockRejectedValue(appErr('git', 'untracked'));
    const deps = makeDeps();
    await mount(deps).handleConfirmLineDiscard({
      path: 'src/app.ts',
      origPath: null,
      selection: HUNK0_SELECTION,
    });
    expect(deps.reportStatusError).toHaveBeenCalledWith('untracked');
    expect(deps.refetchStatus).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});
