// P49: external-tool launch mock (terminal / file manager / editor).
// No real process is spawned in the browser harness — this only proves the UI
// wiring: the success path resolves silently (a window "opening" is its own
// feedback in the real app) and a `#fail` sentinel path rejects with the exact
// AppError shape the frontend's error→toast path expects.
//
// Like `validate_web_url` (note below), the mock deliberately does NOT replicate
// the 2026-09-11 `terminalCommand`/`editorCommand` shape rules
// (`external_cmd::validate_command_setting`): there is no launcher here, Rust
// owns the rule, and the IPC surface is unchanged by it. A harness case that
// needs the refusal toast uses the `#fail` sentinel.
import type { AppError, IpcApi } from '../../types';
import { delay, query } from '../repoState';

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

  // P72: `simulate` FIRST, so the `#fail` path never opens a tab. On success the
  // harness really does open one — that keeps the browser behaviour the plain
  // `target="_blank"` anchor used to have. The mock deliberately does NOT
  // replicate `validate_web_url` (Rust owns that rule; there is no launcher
  // here); extend the sentinel triggers instead if a harness case needs it.
  async openUrl(url: string): Promise<void> {
    // P113 §14: `?openUrlFail=1` makes EVERY `openUrl` reject. The `#fail`
    // sentinel above cannot reach the Accounts token-page link, because that URL
    // is BUILT BY THE APP from the host — nothing in the query string can inject
    // the sentinel into it — so the outcome sweep's row 6 was unreachable.
    // The message mirrors `bonsai_core::external::launch_first` VERBATIM
    // (`could not launch {what} ({prog}): {e}`), the shape `open_url` produces
    // when no rung of the browser ladder launches. The frontend appends it raw
    // (P113 §12.2 A1), so this string is what the note actually renders.
    if (query('openUrlFail') === '1') {
      await delay(120);
      const err: AppError = {
        kind: 'externalToolFailed',
        message: 'could not launch browser (rundll32): The system cannot find the file specified. (os error 2)',
      };
      throw err;
    }
    await simulate(url, 'browser');
    window.open(url, '_blank', 'noopener,noreferrer');
  },
} satisfies Partial<IpcApi>;
