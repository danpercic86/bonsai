/** T3.2a — useRebaseActions: plain rebase lifecycle + interactive-rebase plan editor. */
import { afterEach, describe, expect, it, vi } from 'vitest';

import { mockIpc } from '../../ipc/mock';
import { useRebaseActions } from './useRebaseActions';
import {
  appErr,
  asyncFn,
  base,
  expectMutatingCycle,
  REPO,
} from '../../test/actionHookKit';
import type { GraphLayout, RebaseTodoOp } from '../../ipc';

afterEach(() => vi.restoreAllMocks());

const OID_A = 'a'.repeat(40);
const OID_B = 'b'.repeat(40);
const GRAPH: GraphLayout = {
  nodes: [
    { id: OID_A, lane: 0, parents: [], summary: 'feat: a', author: 'x', ts: 1, committerTs: 1 },
  ],
  edges: [],
  laneCount: 1,
  headIndex: null,
  truncated: false,
};

type Deps = Parameters<typeof useRebaseActions>[0];

function makeDeps(over: Partial<Deps> = {}): Deps {
  return {
    ...base(),
    refreshAll: asyncFn(),
    graph: GRAPH,
    setRebasePlan: vi.fn(),
    setRebasePlanError: vi.fn(),
    ...over,
  };
}

describe('handleRebaseBranch', () => {
  it('rebased → success toast with step count + refreshAll', async () => {
    const rebase = vi.spyOn(mockIpc, 'rebaseBranch').mockResolvedValue({
      kind: 'rebased',
      branch: 'feat',
      head: OID_A,
      steps: 3,
    });
    const deps = makeDeps();
    await useRebaseActions(deps).handleRebaseBranch('main');
    expect(rebase).toHaveBeenCalledWith(REPO, 'main');
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Rebased onto main (3 commit(s))');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
    expectMutatingCycle(deps.setMutating);
  });

  it('conflicts → info toast with step position and count', async () => {
    vi.spyOn(mockIpc, 'rebaseBranch').mockResolvedValue({
      kind: 'conflicts',
      paths: ['a.ts'],
      currentStep: 2,
      totalSteps: 5,
    });
    const deps = makeDeps();
    await useRebaseActions(deps).handleRebaseBranch('main');
    expect(deps.pushToast).toHaveBeenCalledWith(
      'info',
      'Rebase paused at step 2/5: 1 conflict(s) to resolve',
    );
  });

  it('errors toast and never throw', async () => {
    vi.spyOn(mockIpc, 'rebaseBranch').mockRejectedValue(appErr('git', 'dirty tree'));
    const deps = makeDeps();
    await useRebaseActions(deps).handleRebaseBranch('main');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'dirty tree');
    expect(deps.refreshAll).not.toHaveBeenCalled();
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});

describe('continue / skip / abort', () => {
  it('continue → "Rebase complete" on rebased; refreshAll', async () => {
    vi.spyOn(mockIpc, 'rebaseContinue').mockResolvedValue({
      kind: 'rebased',
      branch: 'feat',
      head: OID_A,
      steps: 2,
    });
    const deps = makeDeps();
    await useRebaseActions(deps).handleRebaseContinue();
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Rebase complete');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('skip → paused info on conflicts', async () => {
    vi.spyOn(mockIpc, 'rebaseSkip').mockResolvedValue({
      kind: 'conflicts',
      paths: ['x'],
      currentStep: 1,
      totalSteps: 3,
    });
    const deps = makeDeps();
    await useRebaseActions(deps).handleRebaseSkip();
    expect(deps.pushToast).toHaveBeenCalledWith('info', 'Rebase paused at step 1/3');
  });

  it('abort → success toast; errors toast', async () => {
    const abort = vi.spyOn(mockIpc, 'rebaseAbort').mockResolvedValue(undefined);
    const deps = makeDeps();
    await useRebaseActions(deps).handleRebaseAbort();
    expect(abort).toHaveBeenCalledWith(REPO);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Rebase aborted');

    abort.mockRejectedValue(appErr('git'));
    await useRebaseActions(deps).handleRebaseAbort();
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'boom');
  });
});

describe('openRebasePlan', () => {
  it('seeds the editor with todos + summaries from graph nodes (shortOid fallback)', async () => {
    const todos: RebaseTodoOp[] = [
      { oid: OID_A, action: 'pick', newMessage: null },
      { oid: OID_B, action: 'pick', newMessage: null }, // not in the graph
    ];
    vi.spyOn(mockIpc, 'getInteractivePlan').mockResolvedValue(todos);
    const deps = makeDeps();
    await useRebaseActions(deps).openRebasePlan({ ontoOid: OID_B, ontoLabel: 'main' });
    expect(deps.setRebasePlanError).toHaveBeenCalledWith(null);
    expect(deps.setRebasePlan).toHaveBeenCalledWith({
      ontoOid: OID_B,
      ontoLabel: 'main',
      initialTodos: todos,
      summaries: { [OID_A]: 'feat: a', [OID_B]: OID_B.slice(0, 7) },
    });
  });

  it('plan fetch error → toast, editor never opens', async () => {
    vi.spyOn(mockIpc, 'getInteractivePlan').mockRejectedValue(appErr('git', 'no base'));
    const deps = makeDeps();
    await useRebaseActions(deps).openRebasePlan({ ontoOid: OID_B, ontoLabel: 'main' });
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'no base');
    expect(deps.setRebasePlan).not.toHaveBeenCalled();
  });
});

describe('handleStartInteractiveRebase', () => {
  const todos: RebaseTodoOp[] = [{ oid: OID_A, action: 'reword', newMessage: 'new' }];

  it('success closes the editor, toasts steps + warnings, refreshes', async () => {
    const start = vi.spyOn(mockIpc, 'startInteractiveRebase').mockResolvedValue({
      kind: 'rebased',
      branch: 'feat',
      head: OID_A,
      steps: 1,
      warnings: ['empty commit dropped'],
    });
    const deps = makeDeps();
    await useRebaseActions(deps).handleStartInteractiveRebase(OID_B, 'main', todos);
    expect(start).toHaveBeenCalledWith(REPO, OID_B, todos);
    expect(deps.setRebasePlan).toHaveBeenCalledWith(null);
    expect(deps.pushToast).toHaveBeenCalledWith('success', 'Rebased onto main (1 commit(s))');
    expect(deps.pushToast).toHaveBeenCalledWith('info', 'empty commit dropped');
    expect(deps.refreshAll).toHaveBeenCalledTimes(1);
  });

  it('backend error keeps the editor open: in-dialog error + toast, plan NOT cleared', async () => {
    vi.spyOn(mockIpc, 'startInteractiveRebase').mockRejectedValue(appErr('git', 'bad todo'));
    const deps = makeDeps();
    await useRebaseActions(deps).handleStartInteractiveRebase(OID_B, 'main', todos);
    expect(deps.setRebasePlan).not.toHaveBeenCalled();
    expect(deps.setRebasePlanError).toHaveBeenCalledWith('bad todo');
    expect(deps.pushToast).toHaveBeenCalledWith('error', 'bad todo');
    expect(deps.setMutating).toHaveBeenLastCalledWith(false);
  });
});
