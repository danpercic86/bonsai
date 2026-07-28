import type { AppError } from '../ipc';

/** Single source for AppError narrowing (P1 contract §2.2) — previously
 * copied into App.tsx, CommitBox.tsx and Sidebar.tsx. */
export function isAppError(e: unknown): e is AppError {
  return (
    typeof e === 'object' &&
    e !== null &&
    'kind' in e &&
    'message' in e &&
    typeof (e as { message: unknown }).message === 'string'
  );
}

/** AppError.message | Error.message | String(e). */
export function errorMessage(e: unknown): string {
  if (isAppError(e)) return e.message;
  if (e instanceof Error) return e.message;
  return String(e);
}
