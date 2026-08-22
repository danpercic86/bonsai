/**
 * P68d shared helpers for the `useAiRuns` test files (split per the ~500-line rule).
 *
 * Lives in `src/test/` so it stays out of coverage, matching `actionHookKit.ts`.
 */
import { vi } from 'vitest';

import { mockIpc } from '../ipc/mock';
import { REPO } from './actionHookKit';
import type { AiResolveBatch, AiRunEvent } from '../ipc';
import type { AiRunsDeps } from '../components/repoWorkspace/useAiRuns';

export const CLEAN = 'merged body\n';
export const MARKERFUL = [
  '<<<<<<< HEAD',
  'ours',
  '=======',
  'theirs',
  '>>>>>>> feat',
  '',
].join('\n');

export function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

export function makeDeps(over: Partial<AiRunsDeps> = {}): AiRunsDeps {
  return {
    repoId: REPO,
    pushToast: vi.fn(),
    aiConflictAutonomy: 'proposeReview',
    aiEligible: true,
    applyResolution: vi.fn(async () => {}),
    // P68f: the store batches the staging refresh itself, so this is called ONCE per
    // settle that stages anything — never once per file.
    refreshAll: vi.fn(async () => {}),
    openAiProposal: vi.fn(async () => {}),
    conflictPaths: ['a.ts', 'b.ts'],
    ...over,
  };
}

export function batch(over: Partial<AiResolveBatch> = {}): AiResolveBatch {
  return {
    runId: 'run-1',
    proposals: [{ path: 'a.ts', proposedText: CLEAN, costUsd: null, needsReview: false }],
    failed: [],
    costUsd: 0.0263,
    turns: 1,
    ...over,
  };
}

/** One event with the run-level fields filled in, so tests state only what matters. */
export function ev(over: Partial<AiRunEvent> & Pick<AiRunEvent, 'seq' | 'kind'>): AiRunEvent {
  return {
    runId: 'run-1',
    text: null,
    costUsd: null,
    elapsedMs: 0,
    path: null,
    turn: 0,
    partialText: null,
    thinkingTokens: null,
    ...over,
  };
}

/** Take control of the stream: capture its `onEvent` sink and settle it by hand. */
export function stubStream() {
  const gate = deferred<AiResolveBatch>();
  let emit: ((e: AiRunEvent) => void) | null = null;
  const spy = vi
    .spyOn(mockIpc, 'aiResolveConflictStream')
    .mockImplementation(async (_repo, _paths, onEvent) => {
      emit = onEvent;
      return gate.promise;
    });
  return {
    spy,
    gate,
    send(e: AiRunEvent) {
      if (emit === null) throw new Error('stream not started');
      emit(e);
    },
  };
}

/** A stream that never settles — for the pure "is it live?" assertions. */
export function neverSettles() {
  return vi
    .spyOn(mockIpc, 'aiResolveConflictStream')
    .mockImplementation(async () => new Promise<AiResolveBatch>(() => {}));
}

/** A stream whose single promise the caller settles, with no event sink needed. */
export function gatedStream() {
  const gate = deferred<AiResolveBatch>();
  vi.spyOn(mockIpc, 'aiResolveConflictStream').mockImplementation(async () => gate.promise);
  return gate;
}
