/**
 * P118 — one mutation round must cost AT MOST ONE render of each memoised
 * sidebar surface, and re-render only the rows whose own data changed. A
 * surface whose data did not change must not render at all: since P118b that
 * is RemotesSection, because this round touches only a LOCAL branch.
 *
 * WHAT THIS REPRODUCES. A mutation (`useBranchActions.ts` and friends) is
 * `setMutating(true)` → git call → `await refreshAll(...)` → `setMutating(false)`
 * in the `finally`. That is three container commits inside one 500 ms
 * render-tally window: busy on, the refreshed snapshot, busy off. Before P118
 * `busy`/`actionsDisabled` was a PROP of both sections and of every `BranchRow`,
 * so the two flips re-rendered all of them for zero visual difference — measured
 * in the 2026-09-22 Dev session as `BranchRow: 100 renders vs 25 instances` plus
 * `4 renders` on each section, i.e. the `render-storm` rule
 * (`renders > 3 * instances`, `src-tauri/src/obs/anomaly.rs:136`) firing 24 times.
 *
 * READING THE NUMBERS. `useRenderCount` counts in the render body, so StrictMode
 * doubles `renders` but not `instances` (obs/react.ts): **2 tallied renders = 1
 * real render**, and a singleton component trips the rule at 2 real renders
 * (4 > 3). The fixture is 25 local branches — the same instance count as the
 * logged storm — so the assertions are directly comparable to the session data.
 *
 * The container's own `render` records are the negative control: they prove the
 * three commits really happened, so "the sections rendered once" is not a test
 * that simply did nothing.
 */
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render, screen } from '@testing-library/react';
import { Sidebar } from './Sidebar';
import type { SidebarProps } from './Sidebar';
import type { BranchInfo, BranchesSnapshot } from '../ipc';
import { configureObs, resetObsConfigForTests } from '../obs/enabled';
import { attachSink, flushNow, resetBatcherForTests } from '../obs/batcher';
import { clearSessionSalt, setSessionSalt } from '../obs/redact';
import { __resetRenderTally, flushRenderTally } from '../obs/renderTally';
import type { LogRecord } from '../obs/types';

let sunk: LogRecord[] = [];

function branch(name: string, over: Partial<BranchInfo> = {}): BranchInfo {
  return {
    name,
    isHead: false,
    upstream: null,
    ahead: null,
    behind: null,
    tip: 'a'.repeat(40),
    ...over,
  };
}

/** 25 local branches (the logged storm's instance count) + two tracking refs.
 *  `bump` moves ONE branch's ahead count, as a real mutation round does; every
 *  other `BranchInfo` is still rebuilt from scratch, because `list_branches`
 *  really does hand back fresh serde objects every round. */
function snapshot(bump = 0): BranchesSnapshot {
  const local: BranchInfo[] = [branch('main', { isHead: true })];
  for (let i = 1; i < 25; i += 1) {
    local.push(branch(`feat/b${i}`, i === 1 ? { ahead: bump } : {}));
  }
  return {
    local,
    remote: [
      { name: 'origin/main', tip: 'b'.repeat(40) },
      { name: 'origin/dev', tip: 'c'.repeat(40) },
    ],
    tags: ['v1.0'],
    head: { branchName: 'main', oid: 'c'.repeat(40), detached: false, unborn: false },
  };
}

/** The two snapshots are hoisted so their IDENTITY is stable across rerenders —
 *  that is what the app does: `refetchBranches` stores through `keepIfUnchanged`
 *  (utils/structuralEqual.ts), so a round that fetched the same branches keeps
 *  the previous reference and the busy flips never touch `data`. Building a
 *  fresh snapshot per rerender would model a bug that was fixed in P91. */
const SNAP_BEFORE = snapshot(0);
const SNAP_AFTER = snapshot(1);

/** Stable by construction — exactly what `useSidebarCallbacks` and
 *  `contextMenuOpeners` give the real call site. */
const CALLBACKS = {
  onDismissError: vi.fn(),
  onCheckout: vi.fn(),
  onContextMenu: vi.fn(),
  onCreateBranch: vi.fn(async () => {}),
  onCreateStash: vi.fn(),
  onStashContextMenu: vi.fn(),
  onSubmoduleContextMenu: vi.fn(),
  onNewSubmodule: vi.fn(),
  onWorktreeContextMenu: vi.fn(),
  onNewWorktree: vi.fn(),
  onTagContextMenu: vi.fn(),
  onTagsExpand: vi.fn(),
  onRemoteContextMenu: vi.fn(),
  onAddRemote: vi.fn(),
  onCleanupBranches: vi.fn(),
  onReveal: vi.fn(),
};
const NO_STASHES: SidebarProps['stashes'] = [];
const NO_SUBMODULES: SidebarProps['submodules'] = [];
const NO_WORKTREES: SidebarProps['worktrees'] = [];
const REMOTES: SidebarProps['remotes'] = [{ name: 'origin', url: 'https://example.com/r.git' }];

function props(over: Partial<SidebarProps> = {}): SidebarProps {
  return {
    data: SNAP_BEFORE,
    loading: false,
    error: null,
    busy: false,
    opActive: false,
    currentBranch: 'main',
    width: 240,
    listView: 'flat',
    stashes: NO_STASHES,
    submodules: NO_SUBMODULES,
    submoduleBusy: null,
    worktrees: NO_WORKTREES,
    tagSyncReport: null,
    tagSyncState: 'idle',
    tagSyncRemote: null,
    tagSyncCheckedAt: null,
    now: 1_700_000_000,
    remotes: REMOTES,
    ...CALLBACKS,
    ...over,
  };
}

beforeEach(() => {
  // The render-tally window is a REAL 500 ms `setTimeout` (renderTally.ts). Under
  // full-suite load one StrictMode round over 25 rows can outlast it, so the
  // window flushed MID-round and `talliesByComponent` (last record wins) saw
  // only the tail — the CONTROL then read as "no storm". Faking the timer means
  // a window closes only at `drain()`, i.e. exactly one window per round.
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] });
  sunk = [];
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  clearSessionSalt();
  attachSink({
    async logAppend(records) {
      sunk.push(...records);
    },
    async logSessionInfo() {
      return { salt: '00112233445566778899aabbccddeeff' };
    },
  });
  setSessionSalt('00112233445566778899aabbccddeeff');
  configureObs({
    enabled: true,
    level: 'trace',
    captureIpc: true,
    captureReact: true,
    captureFrames: false,
    includeRawNames: false,
  });
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  resetObsConfigForTests();
  resetBatcherForTests();
  __resetRenderTally();
  clearSessionSalt();
});

async function drain(): Promise<void> {
  flushRenderTally();
  await flushNow();
}

interface Tally {
  renders: number;
  instances: number;
}

function talliesByComponent(): Map<string, Tally> {
  const out = new Map<string, Tally>();
  for (const r of sunk) {
    if (r.kind !== 'render.tally') continue;
    out.set(r.component as string, {
      renders: r.renders as number,
      instances: r.instances as number,
    });
  }
  return out;
}

/** The rule the Dev session fired: `renders > 3 * instances`. */
function isStorm(t: Tally): boolean {
  return t.renders > 3 * t.instances;
}

/**
 * Mount, discard mount churn, then replay one mutation round: busy on, the
 * refreshed snapshot (one branch changed), busy off — the three commits
 * `setMutating(true) … await refreshAll() … setMutating(false)` produces.
 */
async function mutationRound(): Promise<Map<string, Tally>> {
  const { rerender } = render(<Sidebar {...props()} />, { wrapper: StrictMode });
  await drain();
  sunk = [];
  __resetRenderTally();

  await act(async () => {
    rerender(<Sidebar {...props({ busy: true })} />);
  });
  await act(async () => {
    rerender(<Sidebar {...props({ busy: true, data: SNAP_AFTER })} />);
  });
  await act(async () => {
    rerender(<Sidebar {...props({ busy: false, data: SNAP_AFTER })} />);
  });
  await drain();
  return talliesByComponent();
}

describe('P118 — a mutation round does not storm the sidebar', () => {
  it('re-renders only the section whose data changed, and only the changed row', async () => {
    const tallies = await mutationRound();

    // Negative control: the container really did commit three times (×2 under
    // StrictMode), so the budgets below are measuring suppression, not silence.
    const containerRenders = sunk.filter(
      (r) => r.kind === 'render' && r.component === 'Sidebar',
    ).length;
    expect(containerRenders).toBe(6);

    // BranchesSection: ONE real render (the refreshed snapshot really does
    // change the local list); the two busy flips cost it nothing now that the
    // flag travels by context.
    expect(tallies.get('BranchesSection')).toEqual({ renders: 2, instances: 1 });
    // RemotesSection: ZERO renders. This round moves one LOCAL branch's ahead
    // count, so nothing it displays changed — and since P118b it no longer sees
    // the whole snapshot (`hasRemoteRefs: boolean`) and its refs are identity-
    // cached structurally in Sidebar.tsx, so the fresh-but-equal `data.remote`
    // of a new snapshot no longer reaches it.
    //
    // An ABSENT record IS the zero-render signal: `renderTally` only creates a
    // bucket when a component renders, so zero renders emits no `render.tally`
    // record at all (never `{renders: 0}`). Not vacuous — the `containerRenders`
    // 6 above and `BranchesSection`'s surviving `{renders: 2}` prove the round
    // really happened and that tallies really were collected in this window.
    expect(tallies.get('RemotesSection')).toBeUndefined();

    // Exactly the ONE branch whose ahead count moved — not all 25.
    expect(tallies.get('BranchRow')).toEqual({ renders: 2, instances: 1 });

    // And nothing anywhere in the sidebar trips the anomaly rule.
    for (const [component, t] of tallies) {
      expect(isStorm(t), `${component}: ${t.renders} renders vs ${t.instances}`).toBe(false);
    }
  });

  it('CONTROL — the same round DOES storm when a row callback is unstable', async () => {
    // Teeth for the budgets above: nothing about this scenario is special
    // except one prop identity, and the rule fires immediately. (This is the
    // shape `useSidebarCallbacks`/`contextMenuOpeners` exist to prevent.)
    const { rerender } = render(<Sidebar {...props({ onReveal: () => {} })} />, {
      wrapper: StrictMode,
    });
    await drain();
    sunk = [];
    __resetRenderTally();
    for (const over of [{ busy: true }, { busy: true, data: SNAP_AFTER }, { busy: false }]) {
      await act(async () => {
        rerender(<Sidebar {...props({ ...over, onReveal: () => {} })} />);
      });
    }
    await drain();
    const branchRow = talliesByComponent().get('BranchRow');
    expect(branchRow?.instances).toBe(25);
    expect(isStorm(branchRow as Tally)).toBe(true);
  });

  it('still disables the header actions while the mutation is in flight', async () => {
    const { rerender } = render(<Sidebar {...props()} />, { wrapper: StrictMode });
    const createBranch = () => screen.getByRole('button', { name: 'Create branch' });
    const addRemote = () => screen.getByRole('button', { name: 'Add remote' });
    expect(createBranch()).not.toBeDisabled();
    expect(addRemote()).not.toBeDisabled();

    // The section never re-renders on the flip — the leaf buttons must.
    await act(async () => {
      rerender(<Sidebar {...props({ busy: true })} />);
    });
    expect(createBranch()).toBeDisabled();
    expect(addRemote()).toBeDisabled();

    await act(async () => {
      rerender(<Sidebar {...props({ opActive: true })} />);
    });
    expect(createBranch()).toBeDisabled();

    await act(async () => {
      rerender(<Sidebar {...props()} />);
    });
    expect(createBranch()).not.toBeDisabled();
    await drain();
  });

  it('does not check out a branch double-clicked while busy', async () => {
    const onCheckout = vi.fn();
    const { rerender } = render(<Sidebar {...props({ onCheckout })} />, { wrapper: StrictMode });
    const row = () => screen.getByTitle('feat/b1').closest('li') as HTMLElement;

    await act(async () => {
      rerender(<Sidebar {...props({ onCheckout, busy: true })} />);
    });
    // The row did NOT re-render for the flip; the guard has to read the flag at
    // event time, which is the whole point of the ref-backed context.
    await act(async () => {
      row().dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    });
    expect(onCheckout).not.toHaveBeenCalled();

    await act(async () => {
      rerender(<Sidebar {...props({ onCheckout, busy: false })} />);
    });
    await act(async () => {
      row().dispatchEvent(new MouseEvent('dblclick', { bubbles: true }));
    });
    expect(onCheckout).toHaveBeenCalledWith('feat/b1');
    await drain();
  });
});
