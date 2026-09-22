// P113a — the mock's `?forgeRateLimit=` sentinel: the harness-side seam for the
// forge rate-limit feedback loop (a 429 mid-batch, and the back-off it now
// triggers). Own module (file-size discipline) so both `forge.ts` handlers and
// any future forge handler can raise the SAME refusal.
//
//   ?forgeRateLimit=batch → forgeCommitStatuses resolves PARTIALLY: the first
//                           half of the requested shas comes back, `stoppedBy`
//                           carries the rate-limit error. This is the shape the
//                           real backend returns after P113a.
//   ?forgeRateLimit=prs   → forgeListPrs REJECTS with the same error (a
//                           list call has nothing partial to hand back).
//   ?forgeRateLimit=all   → both.
//
// The message + `retryAfterSecs` mirror the real provider path VERBATIM: the
// mock's default provider is GitHub, whose `rate_limited_error`
// (crates/bonsai-forge/src/github/rest.rs) formats
// "GitHub API rate limit exceeded (resets at epoch {reset})" and derives
// `retry_after_secs` as the seconds from now until that epoch.
import { query as urlParam } from '../repoState';
import type { AppError } from '../../types';

/** Seconds the mock advertises as the wait — long enough to watch the UI stay
 *  quiet, short enough that a manual harness session is not stuck. */
const RETRY_AFTER_SECS = 45;

const MODE = urlParam('forgeRateLimit');

/** `?forgeRateLimit=batch|all` — `forgeCommitStatuses` returns a cut-short batch. */
export const FORGE_RATE_LIMIT_BATCH = MODE === 'batch' || MODE === 'all';
/** `?forgeRateLimit=prs|all` — `forgeListPrs` rejects. */
export const FORGE_RATE_LIMIT_PRS = MODE === 'prs' || MODE === 'all';

/** The rate-limit AppError, with the structural hint P113a added. */
export function rateLimitError(): AppError {
  const resetEpoch = Math.floor(Date.now() / 1000) + RETRY_AFTER_SECS;
  return {
    kind: 'forgeRateLimited',
    message: `GitHub API rate limit exceeded (resets at epoch ${resetEpoch})`,
    retryAfterSecs: RETRY_AFTER_SECS,
  };
}

/** Throws the rate-limit rejection when `?forgeRateLimit=prs|all` is set. */
export function prsRateLimitGuard(): void {
  if (FORGE_RATE_LIMIT_PRS) throw rateLimitError();
}

/** How many of `shas` the mock resolves before the (simulated) 429 — half,
 *  rounded down, but at least one so the partial-success path is exercised
 *  even for a two-sha batch. A single-sha batch resolves nothing and the
 *  handler rejects instead (mirroring the backend: no partial result ⇒ Err). */
export function rateLimitCutoff(shaCount: number): number {
  return Math.max(1, Math.floor(shaCount / 2));
}
