// P49: external-tool launch mock (terminal / file manager / editor).
// No real process is spawned in the browser harness — this only proves the UI
// wiring: the success path resolves silently (a window "opening" is its own
// feedback in the real app) and a `#fail` sentinel path rejects with the exact
// AppError shape the frontend's error→toast path expects.
import type { AppError, IpcApi } from '../../types';
import { delay } from '../repoState';

/** A path containing this substring makes every external action reject, so the
 *  harness can drive the error-toast path (mirrors the `?remote=` failure
 *  triggers). Any other path resolves. */
const FAIL_SENTINEL = '#fail';

/** Resolve on success or throw an `externalToolFailed` AppError on the sentinel,
 *  exactly as the real backend rejects when no candidate launches. */
async function simulate(path: string, what: string): Promise<void> {
  await delay(120);
  if (path.includes(FAIL_SENTINEL)) {
    const err: AppError = {
      kind: 'externalToolFailed',
      message: `Mock: could not launch ${what} for "${path}"`,
    };
    throw err;
  }
  console.info(`[mock] open ${what}: ${path}`);
}

export const externalHandlers = {
  async openInTerminal(path: string): Promise<void> {
    return simulate(path, 'terminal');
  },

  async revealInFileManager(path: string): Promise<void> {
    return simulate(path, 'file manager');
  },

  async openInEditor(path: string): Promise<void> {
    return simulate(path, 'editor');
  },
} satisfies Partial<IpcApi>;
