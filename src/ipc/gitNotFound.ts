// P70: the process-wide "a gitNotFound error was actually observed" latch, plus
// the two strings the remote-op catch sites need.
//
// A module-level store rather than a prop drill: the latch is set deep inside
// the workspace hooks (`useRemoteOps`, clone) and read at the app root by
// `useGitAvailability`. Threading a callback through would have forced new props
// on `RepoWorkspace` / `WorkspaceToolbar`, which the UI contract (§10.3)
// explicitly rules out.
import { isGitNotFound } from './errors';
import { errorMessage } from '../utils/errors';

/** Dedupe key for EVERY gitNotFound toast (UI §10.1). The invariant it buys:
 *  at most one such toast exists at any moment, whatever the press count. */
export const GIT_NOT_FOUND_TOAST_KEY = 'git-not-found';

/** Operation labels used in the coalesced toast text (UI §5.6). */
export type RemoteOpLabel = 'Fetch' | 'Pull' | 'Push' | 'Fetch all' | 'Clone' | 'Push tag';

/** UI §5.6 — plain language: no "authentication", no "credential helper". */
export function gitNotFoundToastText(op: RemoteOpLabel): string {
  return `${op} failed — Bonsai can't run Git to read your saved sign-in.`;
}

let latched = false;
const listeners = new Set<() => void>();

/** Record that a `gitNotFound` error was observed. Idempotent; safe to call
 *  from anywhere, including a background job. */
export function noteGitNotFound(): void {
  if (latched) return;
  latched = true;
  for (const l of [...listeners]) l();
}

/** Cleared by a re-check that finds git (`found === true`). */
export function clearGitNotFoundLatch(): void {
  if (!latched) return;
  latched = false;
  for (const l of [...listeners]) l();
}

export function gitNotFoundLatched(): boolean {
  return latched;
}

/** Subscribe to latch changes; returns the unsubscribe. */
export function subscribeGitNotFound(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

/** Test-only reset so the module-level latch cannot leak between test cases. */
export function resetGitNotFoundLatchForTests(): void {
  latched = false;
  listeners.clear();
}

/** The ONE way a user-pressed remote op reports a failure (UI §10.3).
 *  `gitNotFound` ⇒ latch + exactly one KEYED toast (repeat presses coalesce);
 *  anything else ⇒ the existing unkeyed error toast, untouched. */
export function reportRemoteOpError(
  op: RemoteOpLabel,
  e: unknown,
  pushToast: (tone: 'error', text: string, key?: string) => void,
): void {
  if (isGitNotFound(e)) {
    noteGitNotFound();
    pushToast('error', gitNotFoundToastText(op), GIT_NOT_FOUND_TOAST_KEY);
    return;
  }
  pushToast('error', errorMessage(e));
}
