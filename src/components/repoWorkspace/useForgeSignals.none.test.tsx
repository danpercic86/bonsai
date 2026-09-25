/** P124a — `none` CI verdicts settle (streak rule). A tip with no statuses/checks
 *  used to keep the 60 s TTL forever, so every focus/graph round re-requested it.
 *  Now: one re-check ≥ NONE_SETTLE_MS after first sight, then the terminal TTL.
 *  Forced refreshes still bypass every TTL; steady `pending` still polls at 60 s.
 *  Mode: non-StrictMode, fake timers (harness as ./useForgeSignals.coalesce.test.tsx).
 *  The pure streak/TTL cases live in ./useForgeSignals.test.ts. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { FORGE_REPO_CONTEXT } from '../../ipc/fixtures/forge';
import { resetForgeBackoff } from './forgeBackoff';
import { useForgeSignals } from './useForgeSignals';
import type { CheckRollup, CommitStatus, CommitStatusBatch, GraphLayout, GraphNode } from '../../ipc';

const TIPS = ['a', 'b', 'c'].map((ch) => ch.repeat(40));
const MIN = 60_000;

beforeEach(() => {
  vi.useFakeTimers();
  resetForgeBackoff();
});
afterEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

function node(id: string, branch: string): GraphNode {
  return {
    id,
    lane: 0,
    parents: [],
    refs: [{ name: branch, kind: 'localBranch', isHead: false }],
    summary: '',
    author: '',
    ts: 0,
    committerTs: 0,
  };
}
const LAYOUT: GraphLayout = {
  nodes: TIPS.map((sha, i) => node(sha, `b${i}`)),
  edges: [],
  laneCount: 1,
  headIndex: null,
  truncated: false,
};

function batch(state: CheckRollup): CommitStatusBatch {
  const statuses: CommitStatus[] = TIPS.map((sha) => ({
    sha,
    state,
    total: state === 'none' ? 0 : 1,
    passed: 0,
    failed: 0,
    pending: state === 'pending' ? 1 : 0,
    contexts: [],
  }));
  return { statuses, stoppedBy: null };
}

function spies(state: CheckRollup) {
  vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({ ...FORGE_REPO_CONTEXT, authenticated: true });
  return vi.spyOn(mockIpc, 'forgeCommitStatuses').mockResolvedValue(batch(state));
}

function mount() {
  return renderHook(() =>
    useForgeSignals({
      repoId: '/mock/repo',
      graphDataRef: { current: LAYOUT },
      showPrBadge: false, // CI only — the PR list is a separate call
      showCiStatus: true,
      compact: false,
    }),
  );
}

/** Fake-clock position relative to mount. */
let elapsed = 0;
async function advance(ms: number) {
  elapsed += ms;
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}
async function advanceTo(sinceMount: number) {
  await advance(sinceMount - elapsed);
}

describe('useForgeSignals `none` streak (P124a)', () => {
  beforeEach(() => {
    elapsed = 0;
  });

  it('a `none` tip is re-checked once ≥ NONE_SETTLE_MS after first sight, then settles for 30 min', async () => {
    const statuses = spies('none');
    const { result } = mount();
    await advance(300); // enable batch → `none` #1 (landed at ≈300 ms)
    expect(statuses).toHaveBeenCalledTimes(1);

    await advance(61_000);
    act(() => result.current.refresh('focus'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1); // unsettled TTL stretched to NONE_SETTLE_MS

    await advanceTo(2 * MIN + 1_000);
    act(() => result.current.refresh('graph'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(2); // the one re-check → settled
    const secondAt = elapsed;

    await advance(3 * MIN);
    act(() => result.current.refresh('focus'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(2); // settled ⇒ terminal TTL

    await advanceTo(secondAt + 30 * MIN + 1_000);
    act(() => result.current.refresh('graph'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(3); // terminal TTL expired
  });

  it('a manual (forced) refresh still refetches a settled `none`', async () => {
    const statuses = spies('none');
    const { result } = mount();
    await advance(300);
    await advanceTo(2 * MIN + 1_000);
    act(() => result.current.refresh('graph'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(2); // settled

    await advance(5_000);
    act(() => result.current.refresh('manual', true));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(3);
  });

  it('a steady `pending` tip still polls on every non-forced trigger > 60 s apart', async () => {
    const statuses = spies('pending');
    const { result } = mount();
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1);
    for (let i = 2; i <= 4; i++) {
      await advance(61_000);
      act(() => result.current.refresh(i % 2 === 0 ? 'focus' : 'graph'));
      await advance(300);
      expect(statuses).toHaveBeenCalledTimes(i);
    }
  });
});
