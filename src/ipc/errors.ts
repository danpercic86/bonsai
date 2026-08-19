// P70 §7.4: the shared predicate for the one error kind the UI routes to a
// persistent banner instead of an ordinary toast. Kept in `ipc/` (not
// `utils/errors.ts`) because it is a statement about the IPC error contract.
import { isAppError } from '../utils/errors';

/** True for `AppError { kind: 'gitNotFound' }` — "no runnable git executable",
 *  which is NEVER an authentication failure. Safe on any value (null, a string,
 *  a DOMException). */
export function isGitNotFound(e: unknown): boolean {
  return isAppError(e) && e.kind === 'gitNotFound';
}
