/**
 * P91 §12 row 4 acceptance (d) — the real Sidebar, mounted with a refs-only
 * 500-ref fixture, must produce ≤8 react records on a single ref change: the
 * `render` (each) records of the `Sidebar` container (two, under StrictMode's
 * double-invoked render body) plus one `render.tally` per
 * instrumented section/row component, and ZERO per-row-instance records.
 *
 * The fixture is deliberately refs-only (no stashes / submodules / worktrees /
 * detached HEAD) per §9.4 — otherwise the Stash/Worktree/Submodule/DetachedHead
 * row tallies push the count past 8 without any per-instance-rule violation.
 *
 * 2026-09-16 — THE RECORD BUDGET CANNOT SEE A RENDER STORM. `≤8 records` was the
 * only churn assertion here, and aggregation folds 500 rows × N renders into ONE
 * `render.tally`, so it passed at ANY render count: the sidebar could re-render
 * ten times per ref change and this file stayed green. The renders budget below
 * is the assertion that actually measures churn (sum of `renders` across tallies,
 * plus renders-per-instance), and everything now mounts under `StrictMode` so it
 * measures what the app really does.
 */
import { StrictMode } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, cleanup, render } from '@testing-library/react';
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
  return { name, isHead: false, upstream: null, ahead: null, behind: null, tip: 'a'.repeat(40), ...over };
}

/** 500 local branches + a handful of remotes; NO stash/submodule/worktree, no
 *  detached HEAD — the §9.4 refs-only fixture. `bump` mutates one branch so a
 *  re-render is a genuine one-ref change. */
function snapshot(bump = 0): BranchesSnapshot {
  const local: BranchInfo[] = [branch('main', { isHead: true })];
  for (let i = 1; i < 500; i += 1) {
    local.push(branch(`feat/b${i}`, i === 1 ? { ahead: bump } : {}));
  }
  return {
    local,
    remote: [
      { name: 'origin/main', tip: 'b'.repeat(40) },
      { name: 'origin/dev', tip: 'c'.repeat(40) },
    ],
    tags: ['v1.0', 'v1.1'],
    head: { branchName: 'main', oid: 'c'.repeat(40), detached: false, unborn: false },
  };
}

function props(over: Partial<SidebarProps> = {}): SidebarProps {
  return {
    data: snapshot(0),
    loading: false,
    error: null,
    onDismissError: vi.fn(),
    busy: false,
    opActive: false,
    currentBranch: 'main',
    onCheckout: vi.fn(),
    onContextMenu: vi.fn(),
    onCreateBranch: vi.fn(async () => {}),
    width: 240,
    listView: 'flat',
    stashes: [],
    onCreateStash: vi.fn(),
    onStashContextMenu: vi.fn(),
    submodules: [],
    onSubmoduleContextMenu: vi.fn(),
    submoduleBusy: null,
    onNewSubmodule: vi.fn(),
    worktrees: [],
    onWorktreeContextMenu: vi.fn(),
    onNewWorktree: vi.fn(),
    onTagContextMenu: vi.fn(),
    tagSyncReport: null,
    tagSyncState: 'idle',
    tagSyncRemote: null,
    tagSyncCheckedAt: null,
    onTagsExpand: vi.fn(),
    remotes: [{ name: 'origin', url: 'https://example.com/r.git' }],
    onRemoteContextMenu: vi.fn(),
    onAddRemote: vi.fn(),
    ...over,
  };
}

beforeEach(() => {
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
  // `render` (each) is trace-level; the Sidebar container record only appears at
  // 'trace', so acceptance (d)'s "one each for Sidebar" needs this level.
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

/** What one ref change costs, as measured at the sink. */
interface Churn {
  /** `render` + `render.tally` + `effect` records — the §12 row 4 budget. */
  records: LogRecord[];
  /** `each`-mode records (the container only; rows are aggregate-only). */
  each: LogRecord[];
  tallies: LogRecord[];
  /** Sum of `renders` across every tally — the actual render count. */
  tallyRenders: number;
}

/**
 * Mount the 500-ref sidebar under StrictMode, discard the mount churn, then apply
 * `bumps` prop changes and report what they cost. `bumps === 1` is the acceptance
 * scenario (one ref change); >1 exists so the renders budget can be shown to FAIL
 * on an induced extra re-render.
 */
async function churnOfRefChange(bumps = 1): Promise<Churn> {
  const { rerender } = render(<Sidebar {...props()} />, { wrapper: StrictMode });
  // Discard mount-time churn; measure ONLY the ref change(s).
  await drain();
  sunk = [];
  __resetRenderTally();

  for (let i = 1; i <= bumps; i += 1) {
    await act(async () => {
      rerender(<Sidebar {...props({ data: snapshot(i) })} />);
    });
  }
  await drain();

  const tallies = sunk.filter((r) => r.kind === 'render.tally');
  return {
    records: sunk.filter(
      (r) => r.kind === 'render' || r.kind === 'render.tally' || r.kind === 'effect',
    ),
    each: sunk.filter((r) => r.kind === 'render'),
    tallies,
    tallyRenders: tallies.reduce((n, r) => n + (r.renders as number), 0),
  };
}

describe('acceptance (d) — 500-ref sidebar, one ref change ≤ 8 react records', () => {
  it('collapses to the container each + one tally per component, zero per-row records', async () => {
    const churn = await churnOfRefChange();

    // The budget: ≤8 react RECORDS. This bounds log volume, NOT render count —
    // see the renders budget below for the churn assertion.
    expect(churn.records.length).toBeLessThanOrEqual(8);

    // The only `each` records are the Sidebar container's. Two of them, because
    // StrictMode double-invokes the render body and §9.1 counts in the render
    // body deliberately (obs/react.ts).
    expect(churn.each).toHaveLength(2);
    for (const r of churn.each) expect(r.component).toBe('Sidebar');

    // ZERO per-row-instance records: rows are aggregate-only, never `render`.
    const rowComponents = new Set(['BranchRow', 'RemoteRow', 'ConfiguredRemoteRow', 'TagRow']);
    expect(churn.each.some((r) => rowComponents.has(r.component as string))).toBe(false);

    // At most one tally per component (never one per instance).
    const perComponent = new Map<string, number>();
    for (const t of churn.tallies) {
      const c = t.component as string;
      perComponent.set(c, (perComponent.get(c) ?? 0) + 1);
    }
    for (const [, n] of perComponent) expect(n).toBe(1);

    // The BranchRow tally aggregates all 500 instances into a single record.
    const branchTally = churn.tallies.find((r) => r.component === 'BranchRow');
    expect(branchTally?.instances).toBe(500);
  });
});

/**
 * The churn budget. A RATCHET, NOT A TARGET: these numbers are what the current
 * code does, so they may only be lowered (the render-storm work that removes the
 * redundant state commits and memoises the rows should lower both). A failure
 * means the sidebar started re-rendering more per ref change than it used to.
 *
 * Observed 2026-09-16 on one ref change: 1012 renders across 6 tallies
 * (BranchRow 1000/500 instances; the other five tallies also at 2 renders per
 * instance — RemoteRow 4/2) — i.e. exactly TWO renders per instance across the
 * board, which is ONE real render doubled by StrictMode
 * (obs/react.ts documents the 2× dev inflation). Proof this can fail: with one
 * induced extra prop change (`churnOfRefChange(2)`) it measured 2024 renders and 4
 * renders across 12 tallies: the tally flushed twice, so the second change
 * produced its own set of 6 records, the per-instance ratio stayed 2 in each, and
 * the SUM is what failed ("expected 2024 to be less than or equal to 1012"). The two forms are
 * complementary: the sum catches extra renders across windows, the per-instance
 * ratio catches several commits inside ONE window (the render-storm shape).
 */
const MAX_TALLY_RENDERS = 1012;
const MAX_RENDERS_PER_INSTANCE = 2;

describe('render churn budget — one ref change may not cost extra renders', () => {
  it('stays within the renders ratchet, at ≤2 renders per instance', async () => {
    const churn = await churnOfRefChange();

    expect(churn.tallyRenders).toBeLessThanOrEqual(MAX_TALLY_RENDERS);

    // Per-instance is the size-independent form: it stays 2 whether the fixture
    // has 500 rows or 5, and it is what trips when a single ref change causes
    // several state commits inside one tally window.
    for (const t of churn.tallies) {
      const renders = t.renders as number;
      const instances = t.instances as number;
      expect(instances).toBeGreaterThan(0);
      expect(renders / instances).toBeLessThanOrEqual(MAX_RENDERS_PER_INSTANCE);
    }

    // Every instrumented sidebar component reported in — a budget over an empty
    // set of tallies would be vacuously green.
    expect(churn.tallies.length).toBeGreaterThanOrEqual(6);
  });
});
