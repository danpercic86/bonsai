/** T3.2a — useCherrypickRevertActions: pick/revert + continue/abort + message dialog. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useCherrypickRevertActions } from './useCherrypickRevertActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
  stateSetter,
} from '../../test/actionHookKit';
import type { CommitDiff } from '../../ipc';
import type { PendingCherrypick } from './types';

afterEach(() => vi.restoreAllMocks());

const OID = '0123456789abcdef0123456789abcdef01234567';

type Deps = Parameters<typeof useCherrypickRevertActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    setPendingCherrypick: vi.fn(),
    ...over,
  };
}

describe('handleCherrypick (dialog prefill)', () => {
  it('opens the dialog loading, then fills the FULL commit message', async () => {
    vi.spyOn(mockIpc, 'getCommitDiff').mockResolvedValue({
      details: { message: 'summary\n\nbody' },
      files: [],
    } as unknown as CommitDiff);
    const pending = stateSetter<PendingCherrypick | null>(null);
    const deps = makeDeps({ setPendingCherrypick: pending.set });
    await useCherrypickRevertActions(deps).handleCherrypick(OID);
    expect(pending.set).toHaveBeenNthCalledWith(1, {
      oid: OID,
      initialMessage: '',
      loading: true,
    });
    expect(pending.box.current).toEqual({
      oid: OID,
      initialMessage: 'summary\n\nbody',
      loading: false,
    });
  });

  it('message fetch error → dialog closed + error toast (never a silent empty dialog)', async () => {
    vi.spyOn(mockIpc, 'getCommitDiff').mockRejectedValue(appErr('git', 'missing'));
    const pending = stateSetter<PendingCherrypick | null>(null);
    const deps = makeDeps({ setPendingCherrypick: pending.set });
    await useCherrypickRevertActions(deps).handleCherrypick(OID);
    expect(pending.box.current).toBeNull();
    expect(deps.pushToast).toHaveBeenCalledWith('error', expect.stringContaining('missing'));
  });

  it('stale-oid guard: a dialog already retargeted to another commit is left alone', async () => {
    vi.spyOn(mockIpc, 'getCommitDiff').mockResolvedValue({
      details: { message: 'old' },
      files: [],
    } as unknown as CommitDiff);
    const other: PendingCherrypick = { oid: 'f'.repeat(40), initialMessage: '', loading: true };
    const pending = stateSetter<PendingCherrypick | null>(null);
    const deps = makeDeps({ setPendingCherrypick: pending.set });
    const p = useCherrypickRevertActions(deps).handleCherrypick(OID);
    pending.box.current = other; // user re-picked a different commit mid-fetch
    await p;
    expect(pending.box.current).toBe(other);
  });
});

describe('confirmCherrypick', () => {
  it('committed → success toast, dialog cleared, refreshAll', async () => {
    const pick = vi
      .spyOn(mockIpc, 'cherrypickCommit')
      .mockResolvedValue({ kind: 'committed', oid: OID, stashed: false });
    const deps = makeDeps();
    await useCherrypickRevertActions(deps).confirmCherrypick(OID, 'edited msg');
    expect(pick).toHaveBeenCalledWith(REPO, OID, 'edited msg');
    expect(deps.pushToast).toHaveBeenCalledWith('success', `Cherry-picked ${OID.slice(0, 7)}`);
    expect(deps.setPendingCherrypick).toHaveBeenCalledWith(null);
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('nothingToCommit → INFO toast (not an error); dialog stays for other errors', async () => {
    const pick = vi
      .spyOn(mockIpc, 'cherrypickCommit')
      .mockRejectedValue(appErr('nothingToCommit', 'empty pick'));
    const deps = makeDeps();
    await useCherrypickRevertActions(deps).confirmCherrypick(OID, 'm');
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      'Nothing to apply — the change is already present',
    );

    pick.mockRejectedValue(appErr('git', 'conflict state'));
    await useCherrypickRevertActions(deps).confirmCherrypick(OID, 'm');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'conflict state');
    expect(deps.setPendingCherrypick).not.toHaveBeenCalled(); // only cleared on success
  });

  it('conflicts → info toast; stashPopConflicts → sticky error toast', async () => {
    const pick = vi
      .spyOn(mockIpc, 'cherrypickCommit')
      .mockResolvedValue({ kind: 'conflicts', paths: ['a.ts', 'b.ts'], stashed: true });
    const deps = makeDeps();
    const actions = useCherrypickRevertActions(deps);
    await actions.confirmCherrypick(OID, 'm');
    expect(deps.pushToast).toHaveBeenCalledWith('info', expect.stringContaining('2 conflict(s)'));

    pick.mockResolvedValue({ kind: 'stashPopConflicts', head: OID, paths: ['a.ts'] });
    await actions.confirmCherrypick(OID, 'm');
    expect(deps.pushToast).toHaveBeenCalledWith('error', expect.stringContaining('stash@{0}'));
  });
});

describe('handleRevert', () => {
  it('committed → success toast with short oid; refreshAll', async () => {
    const revert = vi
      .spyOn(mockIpc, 'revertCommit')
      .mockResolvedValue({ kind: 'committed', oid: OID, stashed: true });
    const deps = makeDeps();
    await useCherrypickRevertActions(deps).handleRevert(OID);
    expect(revert).toHaveBeenCalledWith(REPO, OID);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      `Reverted ${OID.slice(0, 7)} · stashed changes restored`,
    );
  });

  it('conflicts pause → info toast; nothingToCommit → info', async () => {
    const revert = vi
      .spyOn(mockIpc, 'revertCommit')
      .mockResolvedValue({ kind: 'conflicts', paths: ['a.ts'], stashed: false });
    const deps = makeDeps();
    await useCherrypickRevertActions(deps).handleRevert(OID);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      expect.stringContaining('Revert paused: 1 conflict(s)'),
    );

    revert.mockRejectedValue(appErr('nothingToCommit'));
    await useCherrypickRevertActions(deps).handleRevert(OID);
    expect(deps.pushToast).toHaveBeenCalledWith('info', expect.stringContaining('already present'));
  });
});

describe('continue / abort', () => {
  it('cherrypickContinue committed → success; revertContinue conflicts → info', async () => {
    vi.spyOn(mockIpc, 'cherrypickContinue').mockResolvedValue({
      kind: 'committed',
      oid: OID,
      stashed: false,
    });
    vi.spyOn(mockIpc, 'revertContinue').mockResolvedValue({
      kind: 'conflicts',
      paths: ['a.ts'],
      stashed: false,
    });
    const deps = makeDeps();
    const actions = useCherrypickRevertActions(deps);
    await actions.handleCherrypickContinue();
    expect(deps.pushToast).toHaveBeenCalledWith('success', `Cherry-picked ${OID.slice(0, 7)}`);
    await actions.handleRevertContinue();
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      'Revert paused: 1 conflict(s) to resolve',
    );
    expect(deps.refreshAll).toHaveBeenCalledTimes(2);
  });

  it('aborts refresh + toast; abort errors use the plain error toast', async () => {
    vi.spyOn(mockIpc, 'cherrypickAbort').mockResolvedValue(undefined);
    const revertAbort = vi
      .spyOn(mockIpc, 'revertAbort')
      .mockRejectedValue(appErr('git', 'no revert in progress'));
    const deps = makeDeps();
    const actions = useCherrypickRevertActions(deps);
    await actions.handleCherrypickAbort();
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Cherry-pick aborted');
    await actions.handleRevertAbort();
    expect(revertAbort).toHaveBeenCalledWith(REPO);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'no revert in progress');
  });
});
