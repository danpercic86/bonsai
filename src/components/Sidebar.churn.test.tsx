/**
 * P91 §12 row 4 acceptance (d) — the real Sidebar, mounted with a refs-only
 * 500-ref fixture, must produce ≤8 react records on a single ref change: one
 * `render` (each) for the `Sidebar` container plus one `render.tally` per
 * instrumented section/row component, and ZERO per-row-instance records.
 *
 * The fixture is deliberately refs-only (no stashes / submodules / worktrees /
 * detached HEAD) per §9.4 — otherwise the Stash/Worktree/Submodule/DetachedHead
 * row tallies push the count past 8 without any per-instance-rule violation.
 */
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

describe('acceptance (d) — 500-ref sidebar, one ref change ≤ 8 react records', () => {
  it('collapses to the container each + one tally per component, zero per-row records', async () => {
    const { rerender } = render(<Sidebar {...props()} />);
    // Discard mount-time churn; measure ONLY the ref change.
    await drain();
    sunk = [];
    __resetRenderTally();

    await act(async () => {
      rerender(<Sidebar {...props({ data: snapshot(1) })} />);
    });
    await drain();

    const react = sunk.filter(
      (r) => r.kind === 'render' || r.kind === 'render.tally' || r.kind === 'effect',
    );
    // The budget: ≤8 react records.
    expect(react.length).toBeLessThanOrEqual(8);

    // Exactly one `render` (each) record, and it is the Sidebar container.
    const eachRecords = sunk.filter((r) => r.kind === 'render');
    expect(eachRecords).toHaveLength(1);
    expect(eachRecords[0]?.component).toBe('Sidebar');

    // ZERO per-row-instance records: rows are aggregate-only, never `render`.
    const rowComponents = new Set([
      'BranchRow',
      'RemoteRow',
      'ConfiguredRemoteRow',
      'TagRow',
    ]);
    expect(eachRecords.some((r) => rowComponents.has(r.component as string))).toBe(false);

    // At most one tally per component (never one per instance).
    const tallies = sunk.filter((r) => r.kind === 'render.tally');
    const perComponent = new Map<string, number>();
    for (const t of tallies) {
      const c = t.component as string;
      perComponent.set(c, (perComponent.get(c) ?? 0) + 1);
    }
    for (const [, n] of perComponent) expect(n).toBe(1);

    // The BranchRow tally aggregates all 500 instances into a single record.
    const branchTally = tallies.find((r) => r.component === 'BranchRow');
    expect(branchTally?.instances).toBe(500);
  });
});
