/** P113a — the rate-limit behaviour of the forge-signal cache, at hook level:
 *   1. a batch cut short by a 429 KEEPS what it resolved (no discarded API work)
 *      and leaves the un-attempted shas on their previous badge;
 *   2. the next refresh — forced or not — makes NO forge call while the host's
 *      advertised window is open;
 *   3. once the window closes, refreshing resumes.
 *  The pure helpers live in ./useForgeSignals.test.ts; this file drives the hook. */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';

import { mockIpc } from '../../ipc/mock';
import { FORGE_REPO_CONTEXT } from '../../ipc/fixtures/forge';
import { resetForgeBackoff } from './forgeBackoff';
import { useForgeSignals } from './useForgeSignals';
import type { AppError, CommitStatus, CommitStatusBatch, GraphLayout, GraphNode } from '../../ipc';

const TIP_A = 'a'.repeat(40);
const TIP_B = 'b'.repeat(40);
const HOST = FORGE_REPO_CONTEXT.host;

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
  nodes: [node(TIP_A, 'main'), node(TIP_B, 'feat')],
  edges: [],
  laneCount: 1,
  headIndex: null,
  truncated: false,
};

function status(sha: string): CommitStatus {
  return { sha, state: 'success', total: 1, passed: 1, failed: 0, pending: 0, contexts: [] };
}
const RATE_LIMITED: AppError = {
  kind: 'forgeRateLimited',
  message: 'Azure DevOps API rate limit exceeded (retry after 30s)',
  retryAfterSecs: 30,
};
function batch(statuses: CommitStatus[], stoppedBy: AppError | null = null): CommitStatusBatch {
  return { statuses, stoppedBy };
}

function mount(repoId = '/mock/repo') {
  const graphDataRef = { current: LAYOUT };
  return renderHook(() =>
    useForgeSignals({
      repoId,
      graphDataRef,
      showPrBadge: false, // CI only — the PR list is a separate call
      showCiStatus: true,
      compact: false,
    }),
  );
}

/** Run the 300 ms debounce + let the async fetch settle. */
async function settle() {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(400);
  });
}

describe('useForgeSignals rate-limit handling', () => {
  it('keeps the statuses a cut-short batch resolved, and the previous badge for the rest', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({
      ...FORGE_REPO_CONTEXT,
      authenticated: true,
    });
    const spy = vi
      .spyOn(mockIpc, 'forgeCommitStatuses')
      .mockResolvedValue(batch([status(TIP_A), status(TIP_B)]));

    const { result } = mount();
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(result.current.ciBySha.size).toBe(2);

    // Second round: the 429 lands after TIP_A resolved. TIP_B was never
    // attempted, so it must NOT be dropped as a phantom 404.
    spy.mockResolvedValue(batch([status(TIP_A)], RATE_LIMITED));
    act(() => result.current.refresh('manual', true));
    await settle();
    expect(spy).toHaveBeenCalledTimes(2);
    expect(result.current.ciBySha.has(TIP_A)).toBe(true);
    expect(result.current.ciBySha.has(TIP_B)).toBe(true);
  });

  it('suppresses the next refresh — even a forced one — until the window closes', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({
      ...FORGE_REPO_CONTEXT,
      authenticated: true,
    });
    const spy = vi
      .spyOn(mockIpc, 'forgeCommitStatuses')
      .mockResolvedValue(batch([status(TIP_A)], RATE_LIMITED));

    const { result } = mount();
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);

    // Inside the advertised 30 s: no forge call at all.
    act(() => result.current.refresh('manual', true));
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);

    // Past it: refreshing resumes.
    spy.mockResolvedValue(batch([status(TIP_A), status(TIP_B)]));
    await act(async () => {
      await vi.advanceTimersByTimeAsync(31_000);
    });
    act(() => result.current.refresh('manual', true));
    await settle();
    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('records a rate-limit REJECTION (nothing resolved) and backs off too', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({
      ...FORGE_REPO_CONTEXT,
      authenticated: true,
    });
    const spy = vi.spyOn(mockIpc, 'forgeCommitStatuses').mockRejectedValue(RATE_LIMITED);

    const { result } = mount();
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);

    act(() => result.current.refresh('manual', true));
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);
    expect(result.current.ciBySha.size).toBe(0);
  });

  it('a non-rate-limit failure does NOT suppress the next refresh', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({
      ...FORGE_REPO_CONTEXT,
      authenticated: true,
    });
    const spy = vi
      .spyOn(mockIpc, 'forgeCommitStatuses')
      .mockRejectedValueOnce({ kind: 'networkError', message: 'offline' } satisfies AppError)
      .mockResolvedValue(batch([status(TIP_A), status(TIP_B)]));

    const { result } = mount();
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);

    act(() => result.current.refresh('manual', true));
    await settle();
    expect(spy).toHaveBeenCalledTimes(2);
    expect(result.current.ciBySha.size).toBe(2);
  });

  it('shares one window across repos on the same host', async () => {
    vi.spyOn(mockIpc, 'forgeRepoContext').mockResolvedValue({
      ...FORGE_REPO_CONTEXT,
      host: HOST,
      authenticated: true,
    });
    const spy = vi
      .spyOn(mockIpc, 'forgeCommitStatuses')
      .mockResolvedValue(batch([status(TIP_A)], RATE_LIMITED));

    const first = mount();
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);

    // A SECOND repo (different repoId, same host) must inherit the back-off —
    // the rate-limit budget belongs to the account, not the repository.
    const second = mount('/mock/other-repo');
    await settle();
    expect(spy).toHaveBeenCalledTimes(1);

    first.unmount();
    second.unmount();
  });
});
