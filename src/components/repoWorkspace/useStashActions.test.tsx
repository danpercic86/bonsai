/** T3.2a — useStashActions: scope-aware create + apply/pop (incl. reserved-path gate) + drop. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useStashActions } from './useStashActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';

afterEach(() => vi.restoreAllMocks());

type Deps = Parameters<typeof useStashActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    refetchStashes: asyncFn(),
    refetchGraph: asyncFn(),
    setPendingReservedStash: vi.fn(),
    ...over,
  };
}

describe('handleCreateStash', () => {
  it('created → scope-specific success toast + refreshAll', async () => {
    const create = vi.spyOn(mockIpc, 'createStash').mockResolvedValue({ created: true });
    const deps = makeDeps();
    await useStashActions(deps).handleCreateStash('staged');
    expect(create).toHaveBeenCalledWith(REPO, null, 'staged');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Stashed staged changes');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('clean tree (created:false) → info toast, still refreshes', async () => {
    vi.spyOn(mockIpc, 'createStash').mockResolvedValue({ created: false });
    const deps = makeDeps();
    await useStashActions(deps).handleCreateStash('all');
    expect(deps.pushToast).toHaveBeenCalledWith('info', 'Nothing to stash — working tree is clean');
  });

  it('errors toast and never throw', async () => {
    vi.spyOn(mockIpc, 'createStash').mockRejectedValue(appErr('git', 'stash failed'));
    const deps = makeDeps();
    await useStashActions(deps).handleCreateStash('allWithUntracked');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'stash failed');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('handleApplyStash', () => {
  it('applied → success toast + refreshAll', async () => {
    const apply = vi.spyOn(mockIpc, 'applyStash').mockResolvedValue({ kind: 'applied' });
    const deps = makeDeps();
    await useStashActions(deps).handleApplyStash(1);
    expect(apply).toHaveBeenCalledWith(REPO, 1, false, undefined);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Applied stash@{1}');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('reservedPaths → arms the confirm dialog, SKIPS refreshAll, clears mutating', async () => {
    vi.spyOn(mockIpc, 'applyStash').mockResolvedValue({
      kind: 'reservedPaths',
      paths: ['NUL', 'aux.txt'],
    });
    const deps = makeDeps();
    await useStashActions(deps).handleApplyStash(0);
    expect(deps.setPendingReservedStash).toHaveBeenCalledWith({
      index: 0,
      op: 'apply',
      paths: ['NUL', 'aux.txt'],
    });
    expect(deps.refreshAll).not.toHaveBeenCalled();
    expect(deps.pushToast).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });

  it('retry with skipReserved → appliedSkippingReserved success toast naming the files', async () => {
    const apply = vi.spyOn(mockIpc, 'applyStash').mockResolvedValue({
      kind: 'appliedSkippingReserved',
      skipped: ['NUL'],
    });
    const deps = makeDeps();
    await useStashActions(deps).handleApplyStash(0, true);
    expect(apply).toHaveBeenCalledWith(REPO, 0, true, undefined);
    expect(deps.pushToast).toHaveBeenCalledWith('success', expect.stringContaining('NUL'));
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('conflicts → INFO toast (stash kept)', async () => {
    vi.spyOn(mockIpc, 'applyStash').mockResolvedValue({ kind: 'conflicts', paths: ['a.ts'] });
    const deps = makeDeps();
    await useStashActions(deps).handleApplyStash(2);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      expect.stringContaining('the stash is kept (stash@{2})'),
    );
  });
});

describe('handlePopStash', () => {
  it('applied → "Popped" toast; conflicts → ERROR toast (stash retained)', async () => {
    const pop = vi.spyOn(mockIpc, 'popStash').mockResolvedValue({ kind: 'applied' });
    const deps = makeDeps();
    const actions = useStashActions(deps);
    await actions.handlePopStash(0);
    expect(pop).toHaveBeenCalledWith(REPO, 0, false, undefined);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Popped stash@{0}');

    pop.mockResolvedValue({ kind: 'conflicts', paths: ['a.ts', 'b.ts'] });
    await actions.handlePopStash(0);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'error',
      expect.stringContaining('still on the stash (stash@{0})'),
    );
  });

  it('reservedPaths → dialog armed with op:pop, no refresh', async () => {
    vi.spyOn(mockIpc, 'popStash').mockResolvedValue({ kind: 'reservedPaths', paths: ['NUL'] });
    const deps = makeDeps();
    await useStashActions(deps).handlePopStash(3);
    expect(deps.setPendingReservedStash).toHaveBeenCalledWith({
      index: 3,
      op: 'pop',
      paths: ['NUL'],
    });
    expect(deps.refreshAll).not.toHaveBeenCalled();
  });

  it('errors toast', async () => {
    vi.spyOn(mockIpc, 'popStash').mockRejectedValue(appErr('git', 'pop failed'));
    const deps = makeDeps();
    await useStashActions(deps).handlePopStash(0);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'pop failed');
  });
});

describe('handleDropStash (confirm-gated upstream)', () => {
  it('drops, toasts, and refetches ONLY stashes + graph (worktree untouched)', async () => {
    const drop = vi.spyOn(mockIpc, 'dropStash').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useStashActions(deps).handleDropStash(1);
    expect(drop).toHaveBeenCalledWith(REPO, 1, undefined);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Dropped stash@{1}');
    expect(deps.refetchStashes).toHaveBeenCalledTimes(1);
    expect(deps.refetchGraph).toHaveBeenCalledTimes(1);
    expect(deps.refreshAll).not.toHaveBeenCalled();
  });

  it('errors toast and clear mutating', async () => {
    vi.spyOn(mockIpc, 'dropStash').mockRejectedValue(appErr('git', 'gone'));
    const deps = makeDeps();
    await useStashActions(deps).handleDropStash(0);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'gone');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
    // Non-guard error must NOT refresh the list (that is reserved for F-A6-B).
    expect(deps.refetchStashes).not.toHaveBeenCalled();
  });
});

// F-A6-B: the wrong-target guard — the UI passes the oid it rendered for the
// stack index; apply/pop/drop forward it verbatim, and a mismatch rejection
// re-syncs the stale list.
describe('F-A6-B wrong-target guard (expectedOid)', () => {
  const GUARD_MSG = 'stash list changed; refresh and retry';

  it('apply/pop/drop forward the rendered expectedOid to the ipc call', async () => {
    const apply = vi.spyOn(mockIpc, 'applyStash').mockResolvedValue({ kind: 'applied' });
    const pop = vi.spyOn(mockIpc, 'popStash').mockResolvedValue({ kind: 'applied' });
    const drop = vi.spyOn(mockIpc, 'dropStash').mockResolvedValue(undefined);
    const deps = makeDeps();
    const actions = useStashActions(deps);

    await actions.handleApplyStash(1, false, 'oidA');
    expect(apply).toHaveBeenCalledWith(REPO, 1, false, 'oidA');

    await actions.handlePopStash(2, false, 'oidB');
    expect(pop).toHaveBeenCalledWith(REPO, 2, false, 'oidB');

    await actions.handleDropStash(3, 'oidC');
    expect(drop).toHaveBeenCalledWith(REPO, 3, 'oidC');
  });

  it('mismatch rejection → error toast AND refetchStashes (apply)', async () => {
    vi.spyOn(mockIpc, 'applyStash').mockRejectedValue(new Error(GUARD_MSG));
    const deps = makeDeps();
    await useStashActions(deps).handleApplyStash(0, false, 'stale');
    expect(deps.pushToast).toHaveBeenCalledWith('error', GUARD_MSG);
    expect(deps.refetchStashes).toHaveBeenCalledTimes(1);
  });

  it('mismatch rejection → error toast AND refetchStashes (pop + drop)', async () => {
    vi.spyOn(mockIpc, 'popStash').mockRejectedValue(new Error(GUARD_MSG));
    const popDeps = makeDeps();
    await useStashActions(popDeps).handlePopStash(0, false, 'stale');
    expect(popDeps.pushToast).toHaveBeenCalledWith('error', GUARD_MSG);
    expect(popDeps.refetchStashes).toHaveBeenCalledTimes(1);

    vi.spyOn(mockIpc, 'dropStash').mockRejectedValue(new Error(GUARD_MSG));
    const dropDeps = makeDeps();
    await useStashActions(dropDeps).handleDropStash(0, 'stale');
    expect(dropDeps.pushToast).toHaveBeenCalledWith('error', GUARD_MSG);
    // Drop already refetches on success; on the guard rejection it also re-syncs.
    expect(dropDeps.refetchStashes).toHaveBeenCalledTimes(1);
  });

  it('reservedPaths carries the rendered oid; the skip-reserved retry forwards it', async () => {
    const apply = vi
      .spyOn(mockIpc, 'applyStash')
      .mockResolvedValueOnce({ kind: 'reservedPaths', paths: ['NUL'] })
      .mockResolvedValueOnce({ kind: 'appliedSkippingReserved', skipped: ['NUL'] });
    const deps = makeDeps();
    const actions = useStashActions(deps);

    // First attempt arms the dialog, stashing the oid the user saw.
    await actions.handleApplyStash(0, false, 'oidReserved');
    expect(deps.setPendingReservedStash).toHaveBeenCalledWith({
      index: 0,
      op: 'apply',
      paths: ['NUL'],
      oid: 'oidReserved',
    });

    // The confirm re-invokes with skipReserved=true and the SAME oid.
    await actions.handleApplyStash(0, true, 'oidReserved');
    expect(apply).toHaveBeenLastCalledWith(REPO, 0, true, 'oidReserved');
  });
});
