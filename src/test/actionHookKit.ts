/**
 * T3.2a shared helpers for the repoWorkspace mutation-hook tests.
 * Lives in src/test/ so it stays out of coverage (vite.config.ts excludes it).
 */
import { expect, vi } from 'vitest';
import type { AppError } from '../ipc';

export const REPO = '/mock/repo';

/** The BaseActionDeps trio with fresh spies. */
export function base() {
  return { repoId: REPO, pushToast: vi.fn(), setMutating: vi.fn() };
}

/** An AppError-shaped rejection (what a real tauri invoke rejects with). */
export function appErr(kind: AppError['kind'], message = 'boom'): AppError {
  return { kind, message };
}

/** Async no-op spy for refetch/refresh callbacks. */
export const asyncFn = () => vi.fn(async () => {});

/** Pass-through hook gate: runs the attempt immediately with the given skipHooks. */
export const passthroughGate = () =>
  vi.fn(async (attempt: (sh: boolean) => Promise<void>, sh: boolean) => attempt(sh));

/** Assert setMutating toggled true then finally false — the flag the UI uses to
 *  disable toolbar/actions while an op is in flight (the hooks themselves do
 *  NOT guard double-invoke; RepoWorkspace relies on this flag). */
export function expectMutatingCycle(setMutating: unknown) {
  expect(setMutating).toHaveBeenCalledWith(true);
  expect(setMutating).toHaveBeenLastCalledWith(false);
}

/** Emulate a React state setter that supports functional updates. */
export function stateSetter<T>(initial: T) {
  const box = { current: initial };
  const set = vi.fn((v: T | ((prev: T) => T)) => {
    box.current = typeof v === 'function' ? (v as (p: T) => T)(box.current) : v;
  });
  return { box, set };
}
