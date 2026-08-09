// T3.4 shared helpers for mock-IPC layer tests. The mock handlers hide their
// latency behind `delay(...)` setTimeout calls; these helpers pair each call
// with a fake-timer advance so suites stay fast and deterministic.
// Lives under src/test/ (coverage-excluded test infrastructure).
import { vi } from 'vitest';

import type { AppError } from '../ipc/types';

/** Resolve a mock-IPC promise under fake timers (advances up to 10 s). */
export async function run<T>(p: Promise<T>): Promise<T> {
  await vi.advanceTimersByTimeAsync(10_000);
  return p;
}

/** Await a REJECTING mock-IPC promise under fake timers and return the
 *  AppError-shaped rejection. Attaches the handler BEFORE advancing so no
 *  unhandled-rejection warning fires. */
export async function runErr(p: Promise<unknown>): Promise<AppError> {
  const guarded = p.then(
    () => {
      throw new Error('expected the mock call to reject, but it resolved');
    },
    (e: unknown) => e as AppError,
  );
  await vi.advanceTimersByTimeAsync(10_000);
  return guarded;
}

let repoSeq = 0;

/** A unique, seam-free mock repo path (avoids the reserved substrings
 *  'merge' | 'rebase' | 'detached' | 'unborn' | 'not-a-repo' | 'bare' | 'error'). */
export function freshRepoPath(label = 'fixture'): string {
  repoSeq += 1;
  return `/mock/t34-${label}-${repoSeq}`;
}
