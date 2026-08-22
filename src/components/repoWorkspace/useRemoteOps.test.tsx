/** T3.2a — useRemoteOps: fetch / pull (non-FF gate) / push / force-push-with-lease. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useRemoteOps } from './useRemoteOps';
import { COMMIT_HOOK_CANCELED } from '../commitPushSignal';
import { appErr, asyncFn, base, passthroughGate, REPO } from '../../test/actionHookKit';

afterEach(() => vi.restoreAllMocks());

type Deps = Parameters<typeof useRemoteOps>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    // Accepted-but-unused after P85 A1 (see useRemoteOps deps note); still
    // required by the deps shape until P86 drops them from RepoWorkspace.
    refetchBranches: asyncFn(),
    refetchGraph: asyncFn(),
    setRemoteOp: vi.fn(),
    setPendingForcePush: vi.fn(),
    setPendingNonFfPull: vi.fn(),
    runWithHookGate: passthroughGate(),
    ...over,
  };
}

/** The busy indicator the toolbar relies on: op set at start, cleared at end. */
function expectRemoteOpCycle(deps: Deps, op: 'fetch' | 'pull' | 'push') {
  expect(deps.setRemoteOp).toHaveBeenNthCalledWith(1, op);
  expect(deps.setRemoteOp).toHaveBeenLastCalledWith(null);
  expect(deps.setMutating).toHaveBeenNthCalledWith(1, true);
  expect(deps.setMutating).toHaveBeenLastCalledWith(false);
}

describe('handleFetch', () => {
  it('toasts remote + updated-ref counts and refreshes via refreshAll', async () => {
    const fetch = vi.spyOn(mockIpc, 'fetch').mockResolvedValue({
      remotes: [
        { remote: 'origin', receivedObjects: 10, updatedRefs: 2 },
        { remote: 'fork', receivedObjects: 0, updatedRefs: 1 },
      ],
    });
    const deps = makeDeps();
    await useRemoteOps(deps).handleFetch();
    expect(fetch).toHaveBeenCalledWith(REPO);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Fetched 2 remotes — 3 refs updated');
    // P85 A1: one echo-armed refreshAll (tag counts now arrive via tag-auto-sync).
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectRemoteOpCycle(deps, 'fetch');
  });

  it('errors toast and still clear the busy state', async () => {
    vi.spyOn(mockIpc, 'fetch').mockRejectedValue(appErr('networkError', 'offline'));
    const deps = makeDeps();
    await useRemoteOps(deps).handleFetch();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'offline');
    expectRemoteOpCycle(deps, 'fetch');
  });
});

describe('handlePull', () => {
  it('fastForwarded → success toast with short target oid + refreshAll', async () => {
    const to = '0123456789abcdef0123456789abcdef01234567';
    vi.spyOn(mockIpc, 'pull').mockResolvedValue({
      kind: 'fastForwarded',
      branch: 'main',
      from: 'f'.repeat(40),
      to,
    });
    const deps = makeDeps();
    await useRemoteOps(deps).handlePull();
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Fast-forwarded main to 0123456');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectRemoteOpCycle(deps, 'pull');
  });

  it('wouldNotFastForward → opens the reconcile dialog, NO error toast, still refreshes (fetch landed)', async () => {
    vi.spyOn(mockIpc, 'pull').mockResolvedValue({
      kind: 'wouldNotFastForward',
      branch: 'main',
      ahead: 2,
      behind: 3,
      upstream: 'origin/main',
    });
    const deps = makeDeps();
    await useRemoteOps(deps).handlePull();
    expect(deps.setPendingNonFfPull).toHaveBeenCalledWith({
      branch: 'main',
      upstream: 'origin/main',
      ahead: 2,
      behind: 3,
    });
    expect(deps.pushToast).not.toHaveBeenCalled();
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('upToDate → success toast; errors → error toast', async () => {
    const pull = vi.spyOn(mockIpc, 'pull').mockResolvedValue({ kind: 'upToDate' });
    const deps = makeDeps();
    const ops = useRemoteOps(deps);
    await ops.handlePull();
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Already up to date');

    pull.mockRejectedValue(appErr('noUpstream', 'no upstream'));
    await ops.handlePull();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'no upstream');
    expect(deps.setRemoteOp).toHaveBeenLastCalledWith(null);
  });
});

describe('handlePush / pushCurrentBranch', () => {
  it('pushed with upstream set → toast notes it; refreshes via refreshAll', async () => {
    const push = vi.spyOn(mockIpc, 'push').mockResolvedValue({
      kind: 'pushed',
      remote: 'origin',
      branch: 'feat',
      setUpstream: true,
    });
    const deps = makeDeps();
    await useRemoteOps(deps).handlePush();
    expect(push).toHaveBeenCalledWith(REPO, false);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      'Pushed feat → origin/feat (upstream set)',
    );
    // P85 A1: one echo-armed refreshAll, not raw refetchBranches/refetchGraph.
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectRemoteOpCycle(deps, 'push');
  });

  it('hook-gate dismissal (COMMIT_HOOK_CANCELED) → NO toast, busy state cleared', async () => {
    const deps = makeDeps({
      runWithHookGate: vi.fn(async () => {
        throw COMMIT_HOOK_CANCELED;
      }),
    });
    await useRemoteOps(deps).pushCurrentBranch();
    expect(deps.pushToast).not.toHaveBeenCalled();
    expectRemoteOpCycle(deps, 'push');
  });

  it('push errors are toasted, never thrown (Commit & Push keeps the commit)', async () => {
    vi.spyOn(mockIpc, 'push').mockRejectedValue(appErr('pushRejected', 'non-fast-forward'));
    const deps = makeDeps();
    await expect(useRemoteOps(deps).pushCurrentBranch()).resolves.toBeUndefined();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'non-fast-forward');
  });
});

describe('force push', () => {
  it('handleForcePush only arms the danger confirm — no IPC', () => {
    const force = vi.spyOn(mockIpc, 'forcePush');
    const deps = makeDeps();
    useRemoteOps(deps).handleForcePush();
    expect(deps.setPendingForcePush).toHaveBeenCalledWith(true);
    expect(force).not.toHaveBeenCalled();
    expect(deps.setMutating).not.toHaveBeenCalled();
  });

  it('doForcePush closes the confirm, pushes with lease, toasts', async () => {
    const force = vi.spyOn(mockIpc, 'forcePush').mockResolvedValue({
      kind: 'pushed',
      remote: 'origin',
      branch: 'feat',
      setUpstream: false,
    });
    const deps = makeDeps();
    await useRemoteOps(deps).doForcePush();
    expect(deps.setPendingForcePush).toHaveBeenCalledWith(false);
    expect(force).toHaveBeenCalledWith(REPO, false);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Force-pushed feat → origin/feat');
    expectRemoteOpCycle(deps, 'push');
  });

  it('lease rejection (pushRejected) → error toast with the fetch-and-retry hint', async () => {
    vi.spyOn(mockIpc, 'forcePush').mockRejectedValue(appErr('pushRejected', 'stale info'));
    const deps = makeDeps();
    await useRemoteOps(deps).doForcePush();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'stale info — fetch and retry');
  });

  it('non-lease errors get no hint; hook-cancel is silent', async () => {
    const force = vi.spyOn(mockIpc, 'forcePush').mockRejectedValue(appErr('authFailed', 'denied'));
    const deps = makeDeps();
    await useRemoteOps(deps).doForcePush();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'denied');
    force.mockRestore();

    const deps2 = makeDeps({
      runWithHookGate: vi.fn(async () => {
        throw COMMIT_HOOK_CANCELED;
      }),
    });
    await useRemoteOps(deps2).doForcePush();
    expect(deps2.pushToast).not.toHaveBeenCalled();
    expect(deps2.setRemoteOp).toHaveBeenLastCalledWith(null);
  });
});
