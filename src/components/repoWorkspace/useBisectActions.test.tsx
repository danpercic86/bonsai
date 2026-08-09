/** T3.2a — useBisectActions: start / mark / skip / reset. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useBisectActions } from './useBisectActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';

afterEach(() => vi.restoreAllMocks());

const BAD = 'b'.repeat(40);
const GOOD = '9'.repeat(40);
const FIRST_BAD = '0123456789abcdef0123456789abcdef01234567';

type Deps = Parameters<typeof useBisectActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    setPendingBisectBad: vi.fn(),
    ...over,
  };
}

describe('handleStartBisect', () => {
  it('testing outcome → clears the pending-bad, info toast with progress, refreshAll', async () => {
    const start = vi.spyOn(mockIpc, 'startBisect').mockResolvedValue({
      kind: 'testing',
      current: GOOD,
      revisionsRemaining: 8,
      estimatedSteps: 3,
    });
    const deps = makeDeps();
    await useBisectActions(deps).handleStartBisect(BAD, GOOD);
    expect(start).toHaveBeenCalledWith(REPO, BAD, [GOOD]);
    expect(deps.setPendingBisectBad).toHaveBeenCalledWith(null);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      'Bisecting: 8 revision(s) left, ~3 step(s)',
    );
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('error (non-ancestor good / dirty tree) → toast and KEEPS the pending-bad for retry', async () => {
    vi.spyOn(mockIpc, 'startBisect').mockRejectedValue(appErr('git', 'good is not an ancestor'));
    const deps = makeDeps();
    await useBisectActions(deps).handleStartBisect(BAD, GOOD);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'good is not an ancestor');
    expect(deps.setPendingBisectBad).not.toHaveBeenCalled();
    expect(deps.refreshAll).not.toHaveBeenCalled();
  });
});

describe('handleBisectMark / handleBisectSkip', () => {
  it('found → success toast with the short first-bad oid', async () => {
    const mark = vi
      .spyOn(mockIpc, 'bisectMark')
      .mockResolvedValue({ kind: 'found', firstBad: FIRST_BAD });
    const deps = makeDeps();
    await useBisectActions(deps).handleBisectMark(true);
    expect(mark).toHaveBeenCalledWith(REPO, true);
    expect(deps.pushToast).toHaveBeenCalledWith(
      'success',
      `Bisect found first bad commit ${FIRST_BAD.slice(0, 7)}`,
    );
  });

  it('skip → cannotDetermine info toast with skipped count', async () => {
    vi.spyOn(mockIpc, 'bisectSkip').mockResolvedValue({
      kind: 'cannotDetermine',
      skipped: [BAD, GOOD],
    });
    const deps = makeDeps();
    await useBisectActions(deps).handleBisectSkip();
    expect(deps.pushToast).toHaveBeenCalledWith('info', expect.stringContaining('(2)'));
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('mark errors toast and never throw', async () => {
    vi.spyOn(mockIpc, 'bisectMark').mockRejectedValue(appErr('git', 'not bisecting'));
    const deps = makeDeps();
    await useBisectActions(deps).handleBisectMark(false);
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'not bisecting');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('handleBisectReset (confirm-gated upstream)', () => {
  it('resets, refreshes, success toast; errors toast', async () => {
    const reset = vi.spyOn(mockIpc, 'bisectReset').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useBisectActions(deps).handleBisectReset();
    expect(reset).toHaveBeenCalledWith(REPO);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Bisect reset');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);

    reset.mockRejectedValue(appErr('io', 'sequencer locked'));
    await useBisectActions(deps).handleBisectReset();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'sequencer locked');
  });
});
