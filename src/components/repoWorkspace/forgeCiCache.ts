/** Pure CI-verdict cache helpers for {@link ./useForgeSignals} (extracted in
 *  P124a). Everything here is PURE and unit-tested in ./useForgeSignals.test.ts;
 *  the hook owns the timers, the in-flight coalescing and the IPC. */
import type { CommitStatus, GraphLayout } from '../../ipc';
import type { CiBadge } from '../../graph/forgeBadges';

/** Short cache lifetime for forge signals — a fetch/pull or manual Refresh
 *  FORCES a refetch; focus + graph-change are TTL-guarded so returning to the
 *  window doesn't hammer the API. Applies to the PR list and to `pending` CI
 *  verdicts (a fresh push gains checks soon). */
export const TTL_MS = 60_000;
/** P121a: lifetime of a TERMINAL CI verdict (success | failure | error |
 *  neutral, and a settled `none`). The sha is immutable, so its verdict rarely
 *  changes — a CI re-run can flip one, hence finite. `force` still bypasses it. */
export const TTL_TERMINAL_MS = 30 * 60_000;
/** P124a: a `none` observed across at least this span is treated as terminal. */
export const NONE_SETTLE_MS = 2 * 60_000;

/** One cached CI verdict + when it was fetched (for the TTL freshness check). */
export interface CiEntry {
  badge: CiBadge;
  /** When this verdict was fetched (batch landing time). */
  tsMs: number;
  /** P124a: `tsMs` of the FIRST observation in the current unbroken run of `none`
   *  verdicts for this sha; null when the verdict is not `none`. */
  noneSinceMs: number | null;
}

/** Distinct branch-tip shas in a layout: the id of every node carrying a local
 *  or remote branch ref. PURE (unit-tested) — the CI sha set seeds from here. */
export function branchTipShas(layout: GraphLayout): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const node of layout.nodes) {
    const isBranchTip = node.refs?.some((r) => r.kind === 'localBranch' || r.kind === 'remoteBranch');
    if (isBranchTip === true && !seen.has(node.id)) {
      seen.add(node.id);
      out.push(node.id);
    }
  }
  return out;
}

/** A cache entry is fresh when younger than `ttlMs`. PURE. */
export function isFresh(tsMs: number, now: number, ttlMs: number): boolean {
  return now - tsMs < ttlMs;
}

/** P121a/P124a: the freshness TTL for a cached CI verdict. PURE. Terminal
 *  verdicts for an immutable sha rarely change; `pending` may gain checks soon.
 *  `none` is transient right after a push (CI not registered yet), so it is
 *  re-checked ONCE, `NONE_SETTLE_MS` after first sight, and settles (terminal
 *  TTL) once the unbroken `none` run spans that long. The span is measured
 *  between OBSERVATIONS, never against `now`. */
export function ciTtlFor(entry: CiEntry): number {
  switch (entry.badge.rollup) {
    case 'success':
    case 'failure':
    case 'error':
    case 'neutral':
      return TTL_TERMINAL_MS;
    case 'pending':
      return TTL_MS;
    case 'none': {
      const span = entry.tsMs - (entry.noneSinceMs ?? entry.tsMs);
      return span >= NONE_SETTLE_MS ? TTL_TERMINAL_MS : Math.max(TTL_MS, NONE_SETTLE_MS - span);
    }
  }
}

/** The CI shas to actually request: the union of branch tips + open-PR head
 *  shas, minus those already cached-fresh (unless `force`), deduped, capped at
 *  `max`. PURE (unit-tested) — the batching/freshness decision lives here so
 *  the effect body stays thin. */
export function collectCiShas(
  tips: readonly string[],
  prHeadShas: readonly string[],
  cached: ReadonlyMap<string, CiEntry>,
  now: number,
  ttlOf: (entry: CiEntry) => number,
  force: boolean,
  max: number,
): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const sha of [...tips, ...prHeadShas]) {
    if (seen.has(sha)) continue;
    seen.add(sha);
    const entry = cached.get(sha);
    if (!force && entry !== undefined && isFresh(entry.tsMs, now, ttlOf(entry))) continue;
    out.push(sha);
    if (out.length >= max) break;
  }
  return out;
}

/** Rebuild the CI cache after a successful fetch — REPLACE, not merge, so it
 *  stays bounded to the current requested set (branch tips ∪ open-PR heads).
 *  PURE (unit-tested). The result is:
 *   - cached-fresh entries STILL in `currentSet` that we did NOT refetch this
 *     cycle (carried over, all fields, so a partial non-forced fetch keeps fresh
 *     CI), PLUS
 *   - the statuses `fetched` this cycle (fresh values).
 *  A sha that left `currentSet` (branch deleted / PR closed) is dropped, and a
 *  sha we `requested` but the batch OMITTED (404 — force-pushed/gone tip) is
 *  dropped too (showing its stale CI would be wrong).
 *
 *  P113a: `requested` therefore means "asked for AND actually attempted". When a
 *  batch comes back CUT SHORT, the caller passes only the resolved shas — the
 *  ones after the stop were never attempted, and dropping them here would blank
 *  perfectly good badges on a rate limit.
 *
 *  P124a: a fetched `none` continues the previous entry's unbroken `none` run
 *  (keeps its `noneSinceMs`) or starts one at `stamp`; any other verdict ends it. */
export function rebuildCiCache(
  prev: ReadonlyMap<string, CiEntry>,
  currentSet: ReadonlySet<string>,
  requested: ReadonlySet<string>,
  fetched: readonly CommitStatus[],
  stamp: number,
): Map<string, CiEntry> {
  const next = new Map<string, CiEntry>();
  for (const [sha, entry] of prev) {
    if (currentSet.has(sha) && !requested.has(sha)) next.set(sha, entry);
  }
  for (const s of fetched) {
    const p = prev.get(s.sha);
    const noneSinceMs =
      s.state !== 'none'
        ? null
        : p !== undefined && p.badge.rollup === 'none' && p.noneSinceMs !== null
          ? p.noneSinceMs
          : stamp;
    next.set(s.sha, {
      tsMs: stamp,
      noneSinceMs,
      badge: {
        rollup: s.state,
        passed: s.passed,
        failed: s.failed,
        pending: s.pending,
        total: s.total,
      },
    });
  }
  return next;
}
