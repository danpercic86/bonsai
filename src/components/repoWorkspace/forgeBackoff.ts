// P113a — the forge rate-limit back-off registry.
//
// A measured 124-minute session recorded 8 `forgeRateLimited` rejections, the
// first 4.8 minutes in, because nothing changed after one: the badge refresh
// kept its stale maps, logged a DEV warning, and the next round re-requested
// the identical set — re-triggering the limit. This module is the missing
// memory: once a host says "slow down", every forge refresh for that host stops
// until the advertised wait elapses.
//
// **Keyed by HOST, deliberately not by repo.** Five open repos on one Azure
// DevOps organisation share ONE rate-limit budget; a per-repo window would let
// the other four keep hammering the same account through the fifth's back-off.
//
// Module-level state (not a hook/context): the callers are several independent
// hooks across repos, and the whole point is that they share one window. Tests
// call `resetForgeBackoff()` in `beforeEach`.

import { isAppError } from '../../utils/errors';

/** Wait applied when the provider rate-limited us but advertised NO usable hint
 *  (no header, an HTTP-date `Retry-After`, or a reset epoch already in the
 *  past). One minute matches the forge-signal TTL: we lose at most one refresh
 *  cycle when the guess is too long. */
export const DEFAULT_BACKOFF_MS = 60_000;
/** Ceiling on an advertised wait. A provider (or a proxy) that says "come back
 *  in 24 h" must not silently disable forge badges for the rest of the session;
 *  we cap, retry once, and take a fresh 429 if it really meant it. */
export const MAX_BACKOFF_MS = 15 * 60_000;
/** Floor, so a `retryAfterSecs: 1` cannot degenerate into a tight retry loop. */
const MIN_BACKOFF_MS = 5_000;

/** host → epoch ms until which forge calls for that host are suppressed. */
const suppressedUntil = new Map<string, number>();

/** How long to wait for a rate-limit error, in ms. PURE (unit-tested): the
 *  advertised hint clamped into [MIN, MAX], or {@link DEFAULT_BACKOFF_MS} when
 *  the provider gave none. */
export function backoffMsFor(retryAfterSecs: number | undefined): number {
  if (retryAfterSecs === undefined || !Number.isFinite(retryAfterSecs) || retryAfterSecs <= 0) {
    return DEFAULT_BACKOFF_MS;
  }
  return Math.min(MAX_BACKOFF_MS, Math.max(MIN_BACKOFF_MS, Math.round(retryAfterSecs * 1000)));
}

/** Is `e` a forge rate-limit rejection (or a batch's `stoppedBy`)? PURE. */
export function isRateLimit(e: unknown): boolean {
  return isAppError(e) && e.kind === 'forgeRateLimited';
}

/** Record a rate limit for `host` and open the suppression window.
 *
 *  Non-rate-limit errors are ignored on purpose: a 404/auth/network failure is
 *  not a budget signal, and suppressing on those would blank badges for reasons
 *  the user never sees. Returns the epoch-ms the window closes at, or null when
 *  `e` was not a rate limit.
 *
 *  The LONGER window wins when one is already open, so a second 429 arriving
 *  with a shorter hint cannot shorten an active back-off. */
export function noteForgeRateLimit(host: string, e: unknown, now = Date.now()): number | null {
  if (!isRateLimit(e)) return null;
  const retryAfterSecs = isAppError(e) ? e.retryAfterSecs : undefined;
  const until = now + backoffMsFor(retryAfterSecs);
  const current = suppressedUntil.get(host);
  const next = current !== undefined && current > until ? current : until;
  suppressedUntil.set(host, next);
  return next;
}

/** Epoch ms the host's back-off window closes at, or null when none is open.
 *  Expired windows are dropped as they are read (the map stays bounded by the
 *  number of hosts, which is tiny). */
export function forgeBackoffUntil(host: string, now = Date.now()): number | null {
  const until = suppressedUntil.get(host);
  if (until === undefined) return null;
  if (until <= now) {
    suppressedUntil.delete(host);
    return null;
  }
  return until;
}

/** Should a forge refresh for `host` be skipped right now?
 *
 *  `force` does NOT bypass this: a 429 is the server's budget, not a staleness
 *  heuristic, and the forcing paths (manual refresh, post-fetch) are exactly the
 *  ones that produced the original storm. */
export function isForgeSuppressed(host: string, now = Date.now()): boolean {
  return forgeBackoffUntil(host, now) !== null;
}

/** Clear the window for one host, or (no argument) for all of them.
 *
 *  Only tests call this today: an OPEN window cannot be disproved by a
 *  successful call, because while it is open nothing calls the forge at all,
 *  and an EXPIRED one is dropped as it is read. It exists so a test suite can
 *  start from a known state — module-level state is shared across tests. */
export function resetForgeBackoff(host?: string): void {
  if (host === undefined) suppressedUntil.clear();
  else suppressedUntil.delete(host);
}
