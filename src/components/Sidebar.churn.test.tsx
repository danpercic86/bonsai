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

/**
 * STABLE BY CONSTRUCTION, and that is the point. These were `vi.fn()` literals
 * inside `props()`, so every `rerender` handed the Sidebar nine fresh callbacks
 * and five fresh empty arrays — which no amount of memoisation can survive, and
 * which is NOT what the app does any more: `repoWorkspace/useSidebarCallbacks.ts`
 * and the stable context-menu openers give the real call site exactly these
 * semantics. Note what is deliberately NOT stabilised: `snapshot(bump)` still
 * rebuilds all 500 `BranchInfo` objects, because `list_branches` really does
 * return brand-new serde objects on every round — that is precisely what
 * `rowPropsEqual`'s structural comparison has to see through.
 */
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
  // Stable BECAUSE the app's is: `repoWorkspace/useReveal.ts` latches
  // `handleReveal` behind a ref so replacing the streamed `GraphLayout` (every
  // `full`/`refsOnly`/`remoteMeta`/`stash` round) does not remint it. A fresh
  // one per render costs 1010 renders over 500 BranchRow instances — asserted
  // as the negative control at the bottom of this file.
  onReveal: vi.fn(),
};
const NO_STASHES: SidebarProps['stashes'] = [];
const NO_SUBMODULES: SidebarProps['submodules'] = [];
const NO_WORKTREES: SidebarProps['worktrees'] = [];
const REMOTES: SidebarProps['remotes'] = [{ name: 'origin', url: 'https://example.com/r.git' }];

function props(over: Partial<SidebarProps> = {}): SidebarProps {
  return {
    data: snapshot(0),
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
async function churnOfRefChange(
  bumps = 1,
  /** Evaluated per render — for props that are deliberately UNSTABLE. */
  unstable: () => Partial<SidebarProps> = () => ({}),
): Promise<Churn> {
  const { rerender } = render(<Sidebar {...props(unstable())} />, { wrapper: StrictMode });
  // Discard mount-time churn; measure ONLY the ref change(s).
  await drain();
  sunk = [];
  __resetRenderTally();

  for (let i = 1; i <= bumps; i += 1) {
    await act(async () => {
      rerender(<Sidebar {...props({ data: snapshot(i), ...unstable() })} />);
    });
  }
  await drain();
  return collect();
}

/** Fold everything sunk so far into a Churn. */
function collect(): Churn {
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

/** What MOUNTING the 500-ref sidebar costs — where all 500 row instances really
 *  do render, so it is the honest place to assert the aggregation shape. */
async function churnOfMount(): Promise<Churn> {
  render(<Sidebar {...props()} />, { wrapper: StrictMode });
  await drain();
  return collect();
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

    // A ONE-ref change now renders exactly the ONE row whose data changed — the
    // whole point of the render-storm fix. (It was 500 instances / 1000 renders.)
    const branchTally = churn.tallies.find((r) => r.component === 'BranchRow');
    expect(branchTally?.instances).toBe(1);
    expect(branchTally?.renders).toBe(2); // one real render, doubled by StrictMode
  });

  it('aggregates all 500 row instances into ONE tally record on mount', async () => {
    // The aggregation claim itself (§9.2: one record per component, never per
    // instance) is only observable where every instance renders — i.e. at mount.
    const churn = await churnOfMount();
    const branchTally = churn.tallies.filter((r) => r.component === 'BranchRow');
    expect(branchTally).toHaveLength(1);
    expect(branchTally[0]?.instances).toBe(500);
  });
});

/**
 * The churn budget. A RATCHET, NOT A TARGET: these numbers are what the current
 * code does, so they may only be lowered. A failure means the sidebar started
 * re-rendering more per ref change than it used to.
 *
 * Observed 2026-09-16, BEFORE the render-storm fix: 1012 renders across 6
 * tallies — BranchRow 1000 renders over 500 instances, i.e. a one-ref change
 * re-rendered every row in the sidebar.
 *
 * Observed 2026-09-16, AFTER it: **8 renders across 4 tallies** — BranchesSection
 * 2/1, BranchRow 2/1, RemotesSection 2/1, TagsSection 2/1. A 126x cut, and the
 * shape is the claim: BranchRow now reports ONE instance (the single branch whose
 * `ahead` actually moved), and the Remote/ConfiguredRemote/Tag rows bail out
 * entirely — they no longer report at all. The three sections still render
 * because the snapshot they take really did change; only their rows are spared.
 * Everything is 2 renders per instance, which is ONE real render doubled by
 * StrictMode (obs/react.ts documents the 2x dev inflation) — hence
 * MAX_RENDERS_PER_INSTANCE = 2, which cannot improve and must not be raised.
 *
 * Observed 2026-09-23, after P118b: **6 renders across 3 tallies** —
 * BranchesSection 2/1, BranchRow 2/1, TagsSection 2/1. RemotesSection dropped
 * out entirely: the fixture's ref change is LOCAL-only, and the section no
 * longer takes the whole snapshot (`hasRemoteRefs: boolean`) while `Sidebar.tsx`
 * identity-caches `data.remote` structurally, so the fresh-but-equal remote
 * array of a new snapshot no longer reaches it. A component that renders zero
 * times emits NO `render.tally` record at all (obs/renderTally.ts), which is why
 * the tally COUNT drops with it. Ratchet lowered 8 → 6 to match, per the
 * only-lower rule above — at 8 a regression back to RemotesSection 2/1 would
 * have passed silently.
 *
 * Proof this still fails on a regression: with one induced extra prop change
 * (`churnOfRefChange(2)`) it measures 12 renders across 6 tallies — the tally
 * flushes twice, so the second change produces its own set of 3 records, the
 * per-instance ratio stays 2 in each, and the SUM is what fails ("expected 12 to
 * be less than or equal to 6"). The two forms are complementary: the sum catches
 * extra renders across windows, the per-instance ratio catches several commits
 * inside ONE window (the render-storm shape).
 */
const MAX_TALLY_RENDERS = 6;
const MAX_RENDERS_PER_INSTANCE = 2;

/** A local-branch change must reach these — a budget over an empty set of
 *  tallies would be vacuously green. */
const MUST_REPORT = ['BranchRow', 'BranchesSection'];
/** …and must NOT reach the remote rows: nothing about `origin/*` changed, so
 *  they have no business re-rendering, and before the row memos they did.
 *
 *  `TagRow` and `StashRow` are deliberately NOT listed. They would pass
 *  vacuously: the §9.4 refs-only fixture has an empty stash list and the Tags
 *  section defaults to COLLAPSED, so neither row ever mounts here. They are
 *  memoised the same way (TagsSection.tsx / sidebar/rows.tsx); this file just
 *  cannot witness it, and an assertion that cannot fail is worse than none.
 *
 *  `RemotesSection` joined the list in P118b: nothing it renders depends on a
 *  local branch, and it no longer takes the whole snapshot, so a local-only
 *  change must not reach the section either — not just its rows. */
const MUST_NOT_REPORT = ['RemoteRow', 'ConfiguredRemoteRow', 'RemotesSection'];

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

    const reported = new Set(churn.tallies.map((t) => t.component as string));
    for (const c of MUST_REPORT) expect(reported).toContain(c);
    for (const c of MUST_NOT_REPORT) expect(reported).not.toContain(c);

    // The row that changed, and only that row.
    const branchTally = churn.tallies.find((t) => t.component === 'BranchRow');
    expect(branchTally?.instances).toBe(1);
  });

  /**
   * NEGATIVE CONTROL for `onReveal`, the prop that made this whole budget
   * vacuous: it was ABSENT from the fixture (⇒ `undefined` ⇒ trivially stable),
   * so the suite was green whether `useReveal` latched `handleReveal` or handed
   * out a fresh one per graph stream. Handing the rows a fresh function per
   * render is exactly what an unlatched `useReveal` does, and this is what it
   * costs: the pre-fix storm, in full.
   */
  it('FAILS on an unstable onReveal (the useReveal latch is load-bearing)', async () => {
    const churn = await churnOfRefChange(1, () => ({ onReveal: () => {} }));
    expect(churn.tallyRenders).toBeGreaterThan(MAX_TALLY_RENDERS);
    // Every row re-renders: `rowPropsEqual` compares `onReveal` with `Object.is`.
    const branchTally = churn.tallies.find((t) => t.component === 'BranchRow');
    expect(branchTally?.instances).toBe(500);
    expect(branchTally?.renders).toBe(1000);
    expect(churn.tallyRenders).toBe(1010);
  });

  it('FAILS on an induced extra re-render (the ratchet is load-bearing)', async () => {
    // Two prop changes instead of one: double the renders, double the tallies.
    // Asserted explicitly so the budget above is provably not vacuous.
    const churn = await churnOfRefChange(2);
    expect(churn.tallyRenders).toBeGreaterThan(MAX_TALLY_RENDERS);
    expect(churn.tallyRenders).toBe(2 * MAX_TALLY_RENDERS);
  });
});
