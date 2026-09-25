/** P121a — in-flight coalescing in the forge-signal cache, at hook level.
 *  While a `forgeCommitStatuses` batch is on the wire, a second trigger must NOT
 *  start a duplicate batch for the identical sha set; it is folded into ONE rerun
 *  after the batch lands (which normally finds everything cached-fresh → 0 calls).
 *  A forced trigger still refetches — but only after the held batch resolved.
 *  Disable/repo-switch invalidates the held batch so it never blocks a new fetch.
 *  Mode: non-StrictMode, fake timers. The pure helpers live in
 *  ./useForgeSignals.test.ts; the rate-limit behaviour in ./useForgeSignals.backoff.test.tsx. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { FORGE_REPO_CONTEXT } from '../../ipc/fixtures/forge';
import { resetForgeBackoff } from './forgeBackoff';
import { useForgeSignals } from './useForgeSignals';
import type { CheckRollup, CommitStatus, CommitStatusBatch, GraphLayout, GraphNode } from '../../ipc';

const TIPS = ['a', 'b', 'c'].map((ch) => ch.repeat(40));

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
    total: 1,
    passed: state === 'success' ? 1 : 0,
    failed: state === 'failure' ? 1 : 0,
    pending: 0,
    contexts: [],
  }));
  return { statuses, stoppedBy: null };
}

function deferred<T>() {
  let resolve!: (v: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

function spies() {
  const ctx = vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({
    ...FORGE_REPO_CONTEXT,
    authenticated: true,
  });
  const held = deferred<CommitStatusBatch>();
  const statuses = vi
    .spyOn(mockIpc, 'forgeCommitStatuses')
    .mockReturnValueOnce(held.promise)
    .mockResolvedValue(batch('success'));
  return { ctx, statuses, held };
}

function mount(graphDataRef: { current: GraphLayout | null } = { current: LAYOUT }) {
  return renderHook(
    ({ showCiStatus }: { showCiStatus: boolean }) =>
      useForgeSignals({
        repoId: '/mock/repo',
        graphDataRef,
        showPrBadge: false, // CI only — the PR list is a separate call
        showCiStatus,
        compact: false,
      }),
    { initialProps: { showCiStatus: true } },
  );
}

async function advance(ms: number) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

async function resolveHeld(held: ReturnType<typeof deferred<CommitStatusBatch>>, b: CommitStatusBatch) {
  await act(async () => {
    held.resolve(b);
    await vi.advanceTimersByTimeAsync(0);
  });
}

describe('useForgeSignals in-flight coalescing (P121a)', () => {
  it('a non-forced trigger during a held batch never re-requests the same shas', async () => {
    const { ctx, statuses, held } = spies();
    const { result } = mount();
    await advance(300); // enable batch A on the wire (held)
    expect(statuses).toHaveBeenCalledTimes(1);

    await advance(1_000);
    act(() => result.current.refresh('graph'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1); // coalesced, not duplicated

    await resolveHeld(held, batch('success'));
    await advance(0);
    expect(statuses).toHaveBeenCalledTimes(1);
    // The rerun DID run (forgeRepoContext twice) and found every sha cached-fresh.
    expect(ctx).toHaveBeenCalledTimes(2);
    expect(result.current.ciBySha.size).toBe(3);
  });

  it('a forced trigger during a held batch refetches only after it resolved', async () => {
    const { statuses, held } = spies();
    const { result } = mount();
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1);

    await advance(1_000);
    act(() => result.current.refresh('manual', true));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1); // max concurrent pending = 1

    await resolveHeld(held, batch('pending'));
    await advance(0);
    expect(statuses).toHaveBeenCalledTimes(2);
    expect(result.current.ciBySha.get(TIPS[0])?.rollup).toBe('success');
  });

  it('disable while a batch is held invalidates it: re-enable fetches at once, the stale result is dropped', async () => {
    const { statuses, held } = spies();
    const view = mount();
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1);

    view.rerender({ showCiStatus: false });
    view.rerender({ showCiStatus: true });
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(2); // not blocked by the held batch
    expect(view.result.current.ciBySha.get(TIPS[0])?.rollup).toBe('success');

    await resolveHeld(held, batch('failure'));
    await advance(0);
    expect(statuses).toHaveBeenCalledTimes(2);
    expect(view.result.current.ciBySha.get(TIPS[0])?.rollup).toBe('success');
  });

  it('unmount during a held batch with a coalesced trigger issues no further IPC', async () => {
    const { ctx, statuses, held } = spies();
    const view = mount();
    await advance(300);
    act(() => view.result.current.refresh('manual', true)); // queues a forced rerun
    await advance(300);
    expect(ctx).toHaveBeenCalledTimes(1);
    expect(statuses).toHaveBeenCalledTimes(1);

    view.unmount();
    await resolveHeld(held, batch('success'));
    await advance(1_000);
    expect(ctx).toHaveBeenCalledTimes(1);
    expect(statuses).toHaveBeenCalledTimes(1);
  });

  it('the rerun reads the NEWER graph that arrived mid-batch and requests only its new tip', async () => {
    const { statuses, held } = spies();
    const graphDataRef: { current: GraphLayout | null } = { current: LAYOUT };
    const { result } = mount(graphDataRef);
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1);

    const NEW_TIP = 'd'.repeat(40);
    graphDataRef.current = { ...LAYOUT, nodes: [...LAYOUT.nodes, node(NEW_TIP, 'b3')] };
    act(() => result.current.refresh('graph'));
    await advance(300);
    expect(statuses).toHaveBeenCalledTimes(1);

    await resolveHeld(held, batch('success'));
    await advance(0);
    expect(statuses).toHaveBeenCalledTimes(2);
    expect(statuses.mock.calls[1]?.[1]).toEqual([NEW_TIP]);
  });
});
