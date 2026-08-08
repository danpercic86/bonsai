// P59a: mock blocking-hook gate. Mirrors the backend's `pre-commit` rejection so
// the browser harness can exercise the HookOutputDialog + "Commit anyway (skip
// hooks)" retry + the "Run git hooks" (`bonsai.runHooks`) toggle — with NO real
// git. Shared by the commit / commitAmend / commitMerge mock handlers.
import { runHooksEnabled } from '../fixtures/config';
import type { AppError } from '../types';
import type { MockRepoState } from './repoState';

/** Message sentinel: a commit message containing this triggers a hook rejection
 *  per-call (independent of the `?hooks=fail` RepoState flag). */
export const HOOK_FAIL_SENTINEL = '#hookfail';

/** Realistic multi-line hook output, shaped like the Rust
 *  `HookRejected("<hook> hook failed:\n<combined stdout+stderr>")` so the
 *  HookOutputDialog renders the tool's own output verbatim. */
export const MOCK_HOOK_OUTPUT = [
  'pre-commit hook failed:',
  'gitleaks................................................................Failed',
  '- hook id: gitleaks',
  '- exit code: 1',
  '',
  'Finding:     Potential API key committed',
  'File:        src/config.ts',
  'Line:        42',
  'Secret:      sk_live_************************',
].join('\n');

/** Realistic `pre-push` hook output (P59a-2). The `pre-push hook failed:` prefix
 *  is load-bearing: HookOutputDialog derives its "Push anyway (skip hooks)" label
 *  from a `pre-push` heading, exactly as the backend's `run_hook` message does. */
export const MOCK_PRE_PUSH_OUTPUT = [
  'pre-push hook failed:',
  'gitleaks................................................................Failed',
  '- hook id: gitleaks',
  '- exit code: 1',
  '',
  'Finding:     Potential secret in a commit being pushed',
  'Commit:      a1b2c3d',
].join('\n');

/** Returns a `hookRejected` AppError when a blocking hook would fail this
 *  commit/amend/merge, else `null`. Triggered by the repo's `?hooks=fail` flag
 *  OR a `#hookfail` message sentinel — UNLESS `skipHooks` is true (≡ --no-verify)
 *  or `bonsai.runHooks` is false in the mock config. */
export function hookRejectionFor(
  state: MockRepoState,
  message: string,
  skipHooks?: boolean,
): AppError | null {
  if (skipHooks === true) return null;
  if (!runHooksEnabled(state.config)) return null;
  const triggered = state.hooksFail || message.includes(HOOK_FAIL_SENTINEL);
  if (!triggered) return null;
  return { kind: 'hookRejected', message: MOCK_HOOK_OUTPUT };
}

/** Returns a `hookRejected` for a blocking `pre-push` hook (the `?hooks=failpush`
 *  seam), else `null`. UNLESS `skipHooks` is true (≡ --no-verify) or
 *  `bonsai.runHooks` is false — mirroring `hookRejectionFor`, but for the push
 *  side (no message sentinel — a push has no message). */
export function prePushRejectionFor(
  state: MockRepoState,
  skipHooks?: boolean,
): AppError | null {
  if (skipHooks === true) return null;
  if (!runHooksEnabled(state.config)) return null;
  if (!state.hooksFailPush) return null;
  return { kind: 'hookRejected', message: MOCK_PRE_PUSH_OUTPUT };
}
