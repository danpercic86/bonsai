/** T3.2a — useCommitActions: stage/unstage/commit/amend/reset/discard + Commit & Push. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useCommitActions } from './useCommitActions';
import { COMMIT_PUSH_CANCELED } from '../commitPushSignal';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  passthroughGate,
  REPO,
} from '../../test/actionHookKit';
import type { CommitDiff, CommitResult, FileDiff, StatusSnapshot } from '../../ipc';
import type { DiffSlot } from '../StatusPanel';

afterEach(() => vi.restoreAllMocks());

const COMMIT_RES: CommitResult = {
  oid: 'a'.repeat(40),
  summary: 's',
  branch: 'main',
  hookWarning: null,
};
const entry = (path: string) => ({ path, origPath: null, status: 'modified' as const });
const FILE_DIFF: FileDiff = {
  path: 'b.ts',
  origPath: null,
  status: 'modified',
  binary: false,
  tooLarge: false,
  hunks: [],
};

type Deps = Parameters<typeof useCommitActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    refetchStatus: asyncFn(),
    reportStatusError: vi.fn(),
    fetchDiffSlot: vi.fn(async (_key: string, fetcher: () => Promise<FileDiff>) => {
      await fetcher();
    }),
    pushCurrentBranch: asyncFn(),
    status: null,
    statusRef: { current: null },
    diffSlotRef: { current: null },
    diffViewModeRef: { current: 'diff' },
    intralineRef: { current: false },
    head: { branchName: 'main', oid: 'h'.repeat(40), detached: false, unborn: false },
    headBranch: {
      name: 'main',
      isHead: true,
      upstream: 'origin/main',
      ahead: 0,
      behind: 0,
      tip: 'h'.repeat(40),
    },
    setAmend: vi.fn(),
    setAmendMessage: vi.fn(),
    pendingCommitPush: null,
    setPendingCommitPush: vi.fn(),
    commitPushResolver: { current: null },
    setPendingDiscardForce: vi.fn(),
    refreshVerification: vi.fn(),
    runWithHookGate: passthroughGate(),
    ...over,
  };
}

describe('handleStage', () => {
  it('stages, refetches status, toggles mutating', async () => {
    const stage = vi.spyOn(mockIpc, 'stage').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useCommitActions(deps).handleStage(['a.ts']);
    expect(stage).toHaveBeenCalledWith(REPO, ['a.ts']);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('auto-advances the open diff to the next changed file after staging it', async () => {
    vi.spyOn(mockIpc, 'stage').mockResolvedValue(undefined);
    const wdDiff = vi.spyOn(mockIpc, 'getWorkdirFileDiff').mockResolvedValue(FILE_DIFF);
    const status: StatusSnapshot = {
      staged: [],
      unstaged: [entry('a.ts'), entry('b.ts')],
      untracked: [],
      conflicted: [],
    };
    const postStage: StatusSnapshot = { ...status, unstaged: [entry('b.ts')] };
    const slot: DiffSlot = { key: 'unstaged:a.ts', state: 'ready', diff: null, error: null };
    const deps = makeDeps({
      status,
      statusRef: { current: postStage },
      diffSlotRef: { current: slot },
    });
    await useCommitActions(deps).handleStage(['a.ts']);
    expect(deps.fetchDiffSlot).toHaveBeenCalledWith('unstaged:b.ts', expect.any(Function));
    // args: (repoId, path, origPath, staged, wholeFile, intraline)
    expect(wdDiff).toHaveBeenCalledWith(REPO, 'b.ts', null, false, false, false);
  });

  it('does not advance when the target vanished from the fresh snapshot', async () => {
    vi.spyOn(mockIpc, 'stage').mockResolvedValue(undefined);
    const status: StatusSnapshot = {
      staged: [],
      unstaged: [entry('a.ts'), entry('b.ts')],
      untracked: [],
      conflicted: [],
    };
    const slot: DiffSlot = { key: 'unstaged:a.ts', state: 'ready', diff: null, error: null };
    const deps = makeDeps({
      status,
      statusRef: { current: { ...status, unstaged: [] } }, // b.ts gone post-stage
      diffSlotRef: { current: slot },
    });
    await useCommitActions(deps).handleStage(['a.ts']);
    expect(deps.fetchDiffSlot).not.toHaveBeenCalled();
  });

  it('reports a status error (no toast, no throw) on failure', async () => {
    vi.spyOn(mockIpc, 'stage').mockRejectedValue(appErr('git', 'index locked'));
    const deps = makeDeps();
    await useCommitActions(deps).handleStage(['a.ts']);
    expect(deps.reportStatusError).toHaveBeenCalledWith('index locked');
    expect(deps.pushToast).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('handleUnstage', () => {
  it('unstages then refetches status; errors go to reportStatusError', async () => {
    const unstage = vi.spyOn(mockIpc, 'unstage').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useCommitActions(deps).handleUnstage(['a.ts']);
    expect(unstage).toHaveBeenCalledWith(REPO, ['a.ts']);
    expect(deps.refetchStatus).toHaveBeenCalledTimes(1);

    unstage.mockRejectedValue(appErr('git'));
    await useCommitActions(deps).handleUnstage(['a.ts']);
    expect(deps.reportStatusError).toHaveBeenCalledWith('boom');
  });
});

describe('handleCommit', () => {
  it('commits through the hook gate, refreshes all + verification', async () => {
    const commit = vi.spyOn(mockIpc, 'commit').mockResolvedValue(COMMIT_RES);
    const deps = makeDeps();
    await useCommitActions(deps).handleCommit('msg', true, false);
    expect(commit).toHaveBeenCalledWith(REPO, 'msg', true, false);
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expect(deps.refreshVerification).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('rethrows commit errors (CommitBox owns the banner) but still clears mutating', async () => {
    vi.spyOn(mockIpc, 'commit').mockRejectedValue(appErr('configMissing', 'user.name unset'));
    const deps = makeDeps();
    await expect(useCommitActions(deps).handleCommit('msg')).rejects.toMatchObject({
      kind: 'configMissing',
    });
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
    expect(deps.refreshAll).not.toHaveBeenCalled();
  });
});

describe('handleCommitAndPush', () => {
  it('with an upstream: commits then pushes directly (no dialog)', async () => {
    const commit = vi.spyOn(mockIpc, 'commit').mockResolvedValue(COMMIT_RES);
    const deps = makeDeps();
    await useCommitActions(deps).handleCommitAndPush('msg');
    expect(commit).toHaveBeenCalledWith(REPO, 'msg', null, false);
    expect(deps.pushCurrentBranch).toHaveBeenCalledTimes(1);
    expect(deps.setPendingCommitPush).not.toHaveBeenCalled();
  });

  it('without an upstream: parks the message behind the confirm dialog; confirm commits+pushes', async () => {
    const commit = vi.spyOn(mockIpc, 'commit').mockResolvedValue(COMMIT_RES);
    const deps = makeDeps({
      headBranch: {
        name: 'feat',
        isHead: true,
        upstream: null,
        ahead: null,
        behind: null,
        tip: 'f'.repeat(40),
      },
    });
    const pending = useCommitActions(deps).handleCommitAndPush('msg', true, false);
    expect(deps.setPendingCommitPush).toHaveBeenCalledWith('msg');
    expect(commit).not.toHaveBeenCalled();
    // "Re-render" with the parked message and answer the dialog.
    const confirmed = useCommitActions({ ...deps, pendingCommitPush: 'msg' });
    confirmed.handleConfirmCommitPush();
    await expect(pending).resolves.toBeUndefined();
    expect(commit).toHaveBeenCalledWith(REPO, 'msg', true, false); // parked sign kept
    expect(deps.pushCurrentBranch).toHaveBeenCalledTimes(1);
  });

  it('cancel rejects with the COMMIT_PUSH_CANCELED sentinel and commits nothing', async () => {
    const commit = vi.spyOn(mockIpc, 'commit').mockResolvedValue(COMMIT_RES);
    const deps = makeDeps({
      headBranch: {
        name: 'feat',
        isHead: true,
        upstream: null,
        ahead: null,
        behind: null,
        tip: 'f'.repeat(40),
      },
    });
    const pending = useCommitActions(deps).handleCommitAndPush('msg');
    useCommitActions({ ...deps, pendingCommitPush: 'msg' }).handleCancelCommitPush();
    await expect(pending).rejects.toBe(COMMIT_PUSH_CANCELED);
    expect(commit).not.toHaveBeenCalled();
    expect(deps.setPendingCommitPush).toHaveBeenLastCalledWith(null);
  });
});

describe('handleCommitAmend / handleToggleAmend', () => {
  it('amends, exits amend mode, refreshes, toasts success', async () => {
    const amend = vi.spyOn(mockIpc, 'commitAmend').mockResolvedValue(COMMIT_RES);
    const deps = makeDeps();
    await useCommitActions(deps).handleCommitAmend('new msg', null, true);
    expect(amend).toHaveBeenCalledWith(REPO, 'new msg', null, true);
    expect(deps.setAmend).toHaveBeenCalledWith(false);
    expect(deps.setAmendMessage).toHaveBeenCalledWith(null);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Amended last commit');
  });

  it('toggle ON prefills from HEAD full message; error toasts and stays off', async () => {
    const diff = vi
      .spyOn(mockIpc, 'getCommitDiff')
      .mockResolvedValue({ details: { message: 'full\n\nbody' }, files: [] } as unknown as CommitDiff);
    const deps = makeDeps();
    await useCommitActions(deps).handleToggleAmend(true);
    expect(diff).toHaveBeenCalledWith(REPO, 'h'.repeat(40));
    expect(deps.setAmendMessage).toHaveBeenCalledWith('full\n\nbody');
    expect(deps.setAmend).toHaveBeenCalledWith(true);

    diff.mockRejectedValue(appErr('git'));
    const deps2 = makeDeps();
    await useCommitActions(deps2).handleToggleAmend(true);
    expect(deps2.setAmend).not.toHaveBeenCalled();
    expect(deps2.pushToast).toHaveBeenCalledWith('error', expect.stringContaining('boom'));
  });

  it('toggle ON is a no-op on unborn HEAD; toggle OFF never hits IPC', async () => {
    const diff = vi.spyOn(mockIpc, 'getCommitDiff');
    const deps = makeDeps({
      head: { branchName: null, oid: '', detached: false, unborn: true },
    });
    await useCommitActions(deps).handleToggleAmend(true);
    await useCommitActions(makeDeps()).handleToggleAmend(false);
    expect(diff).not.toHaveBeenCalled();
  });
});

describe('reset / discard', () => {
  it('handleResetBranch toasts branch + short oid + mode; errors toast', async () => {
    const reset = vi.spyOn(mockIpc, 'resetBranch').mockResolvedValue(undefined);
    const oid = '0123456789abcdef0123456789abcdef01234567';
    const deps = makeDeps();
    await useCommitActions(deps).handleResetBranch(oid, 'hard');
    expect(reset).toHaveBeenCalledWith(REPO, oid, 'hard');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Reset main to 0123456 (hard)');

    reset.mockRejectedValue(appErr('git', 'dirty'));
    await useCommitActions(deps).handleResetBranch(oid, 'mixed');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'dirty');
  });

  it('requestDiscardForce partitions modified vs created; empty selection is a no-op', () => {
    const status: StatusSnapshot = {
      staged: [],
      unstaged: [entry('m1.ts'), entry('m2.ts')],
      untracked: [entry('new.ts')],
      conflicted: [],
    };
    const deps = makeDeps({ status });
    const actions = useCommitActions(deps);
    actions.requestDiscardForce(['m1.ts', 'm2.ts', 'new.ts']);
    expect(deps.setPendingDiscardForce).toHaveBeenCalledWith({
      paths: ['m1.ts', 'm2.ts', 'new.ts'],
      modified: 2,
      created: 1,
      untracked: ['new.ts'],
    });
    actions.requestDiscardForce([]);
    expect(deps.setPendingDiscardForce).toHaveBeenCalledTimes(1);
  });

  it('handleDiscard + handleDiscardForce call the right IPC and toast counts', async () => {
    const discard = vi.spyOn(mockIpc, 'discardPaths').mockResolvedValue(undefined);
    const force = vi.spyOn(mockIpc, 'discardPathsForce').mockResolvedValue(undefined);
    const deps = makeDeps();
    const actions = useCommitActions(deps);
    await actions.handleDiscard(['a.ts', 'b.ts']);
    expect(discard).toHaveBeenCalledWith(REPO, ['a.ts', 'b.ts']);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Discarded changes to 2 file(s)');
    await actions.handleDiscardForce(['a.ts']);
    expect(force).toHaveBeenCalledWith(REPO, ['a.ts']);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Discarded 1 file(s)');
    expect(deps.refreshAll).toHaveBeenCalledTimes(2);
  });
});

describe('handleGenerateCommitMessage', () => {
  it('returns the proposal message and rethrows errors (caller owns surfacing)', async () => {
    const gen = vi
      .spyOn(mockIpc, 'generateCommitMessage')
      .mockResolvedValue({ message: 'feat: x', costUsd: null });
    const deps = makeDeps();
    await expect(useCommitActions(deps).handleGenerateCommitMessage()).resolves.toBe('feat: x');
    expect(gen).toHaveBeenCalledWith(REPO);
    gen.mockRejectedValue(appErr('aiUnavailable' as never, 'no cli'));
    await expect(useCommitActions(deps).handleGenerateCommitMessage()).rejects.toMatchObject({
      message: 'no cli',
    });
  });
});
