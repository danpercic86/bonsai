import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import type { CommitStatus, GraphLayout } from '../../ipc';
import type { CiBadge, PrBadge } from '../../graph/forgeBadges';

/** Short cache lifetime for forge signals — a fetch/pull or manual Refresh
 *  FORCES a refetch; focus + graph-change are TTL-guarded so returning to the
 *  window doesn't hammer the API. */
const TTL_MS = 60_000;
/** Coalesce focus + graph-change (+ enable) bursts into ONE fetch. */
const DEBOUNCE_MS = 300;
/** Mirror of the Rust `MAX_STATUS_BATCH` — chunk defensively so a huge branch
 *  count can never exceed the backend cap. */
const MAX = 100;

/** One cached CI verdict + when it was fetched (for the TTL freshness check). */
interface CiEntry {
  badge: CiBadge;
  tsMs: number;
}

export interface UseForgeSignals {
  /** branch SHORT-name → PR badge (open PRs only). New identity on each fetch. */
  prByBranch: ReadonlyMap<string, PrBadge>;
  /** commit sha → CI badge (branch tips). New identity on each fetch. */
  ciBySha: ReadonlyMap<string, CiBadge>;
  /** Kick a (debounced) refetch. `force` bypasses the TTL (fetch/pull/manual);
   *  otherwise stale-only (focus/graph-change). Disabled ⇒ clears the maps. */
  refresh(reason: string, force?: boolean): void;
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

/** The CI shas to actually request: the union of branch tips + open-PR head
 *  shas, minus those already cached-fresh (unless `force`), deduped, capped at
 *  `max`. PURE (unit-tested) — the batching/freshness decision lives here so
 *  the effect body stays thin. */
export function collectCiShas(
  tips: readonly string[],
  prHeadShas: readonly string[],
  cached: ReadonlyMap<string, CiEntry>,
  now: number,
  ttlMs: number,
  force: boolean,
  max: number,
): string[] {
  const out: string[] = [];
  const seen = new Set<string>();
  for (const sha of [...tips, ...prHeadShas]) {
    if (seen.has(sha)) continue;
    seen.add(sha);
    const entry = cached.get(sha);
    if (!force && entry !== undefined && isFresh(entry.tsMs, now, ttlMs)) continue;
    out.push(sha);
    if (out.length >= max) break;
  }
  return out;
}

/** Rebuild the CI cache after a successful fetch — REPLACE, not merge, so it
 *  stays bounded to the current requested set (branch tips ∪ open-PR heads).
 *  PURE (unit-tested). The result is:
 *   - cached-fresh entries STILL in `currentSet` that we did NOT refetch this
 *     cycle (carried over so a partial, non-forced fetch keeps fresh CI), PLUS
 *   - the statuses `fetched` this cycle (fresh values).
 *  A sha that left `currentSet` (branch deleted / PR closed) is dropped, and a
 *  sha we `requested` but the batch OMITTED (404 — force-pushed/gone tip) is
 *  dropped too (showing its stale CI would be wrong). */
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
    next.set(s.sha, {
      tsMs: stamp,
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

/** P63: per-branch forge-signal cache feeding the graph's PR + CI badges.
 *  Mirrors {@link ../repoWorkspace/useCommitVerification} — a last-wins `reqId`
 *  guard, a debounced fetch, and cache → new map identity → repaint. Differences
 *  from verify: NOT scroll-virtualized (branch tips are a small bounded set,
 *  fetched wholesale), and failures are SILENT (badges are decoration — log and
 *  keep the stale maps, never toast, never block paint). Runs only while
 *  enabled = `(showPrBadge || showCiStatus) && !compact`. */
export function useForgeSignals(deps: {
  repoId: string;
  /** Ref to the current layout — read at fetch time (not a reactive dep) so a
   *  post-remote refresh sees the freshest tips without re-subscribing. */
  graphDataRef: { current: GraphLayout | null };
  showPrBadge: boolean;
  showCiStatus: boolean;
  compact: boolean;
}): UseForgeSignals {
  const { repoId, graphDataRef, showPrBadge, showCiStatus, compact } = deps;

  const [prByBranch, setPrByBranch] = useState<ReadonlyMap<string, PrBadge>>(() => new Map());
  const [ciBySha, setCiBySha] = useState<ReadonlyMap<string, CiBadge>>(() => new Map());

  // Internal caches read by the fetch closures (avoid reactive deps / stale
  // captures). `ciCacheRef` carries fetch timestamps; `ciBySha` state is the
  // badge-only projection the graph consumes.
  const ciCacheRef = useRef<Map<string, CiEntry>>(new Map());
  const prTsRef = useRef<number | null>(null);

  const enabled = (showPrBadge || showCiStatus) && !compact;

  // Latest scalars for the debounced async closures.
  const enabledRef = useRef(enabled);
  enabledRef.current = enabled;
  const showPrRef = useRef(showPrBadge);
  showPrRef.current = showPrBadge;
  const showCiRef = useRef(showCiStatus);
  showCiRef.current = showCiStatus;
  const repoIdRef = useRef(repoId);
  repoIdRef.current = repoId;

  const reqIdRef = useRef(0);
  const debounceRef = useRef<number | null>(null);
  // P63: force is STICKY across a debounce window — if any coalesced call in the
  // window forced (fetch/pull/manual), the resulting fetch bypasses the TTL even
  // when a later focus/graph-change call did not. Reset when the fetch fires.
  const pendingForceRef = useRef(false);

  const clearDebounce = useCallback(() => {
    if (debounceRef.current !== null) {
      window.clearTimeout(debounceRef.current);
      debounceRef.current = null;
    }
  }, []);

  /** Drop every cache + published map (a repo switch / disable / manual clear). */
  const clearAll = useCallback(() => {
    ciCacheRef.current = new Map();
    prTsRef.current = null;
    setPrByBranch((prev) => (prev.size === 0 ? prev : new Map()));
    setCiBySha((prev) => (prev.size === 0 ? prev : new Map()));
  }, []);

  const runFetch = useCallback(
    async (force: boolean) => {
      clearDebounce();
      if (!enabledRef.current) return;
      const layout = graphDataRef.current;
      if (layout === null) return;
      const reqId = ++reqIdRef.current;
      const repo = repoIdRef.current;
      try {
        // Cheap, no network (P62): an unsupported / unparseable origin is inert
        // — clear any stale badges and bail WITHOUT surfacing an error. Any
        // KNOWN provider (gitHub | gitLab | …) proceeds to fetch PR + CI signals.
        const ctx = await ipc.forgeRepoContext(repo);
        if (reqIdRef.current !== reqId) return;
        if (ctx.provider === 'unknown') {
          clearAll();
          return;
        }

        // PR badges (open only, OQ-3): refetch on force or when the PR list is
        // stale. Keyed by sourceBranch (last wins). Capture head shas for the CI
        // union below.
        let prHeadShas: string[] = [];
        const now = Date.now();
        if (showPrRef.current && (force || prTsRef.current === null || !isFresh(prTsRef.current, now, TTL_MS))) {
          const page = await ipc.forgeListPrs(repo, { state: 'open', page: 1, perPage: 50 });
          if (reqIdRef.current !== reqId) return;
          const next = new Map<string, PrBadge>();
          for (const pr of page.items) {
            next.set(pr.sourceBranch, {
              number: pr.number,
              title: pr.title,
              state: pr.state,
              isDraft: pr.isDraft,
              url: pr.url,
            });
          }
          prHeadShas = page.items.map((pr) => pr.headSha);
          prTsRef.current = Date.now();
          setPrByBranch(next);
        }
        // (When the PR list is still fresh we skip the refetch; prHeadShas stays
        // [] and the CI union below proceeds from branch tips alone.)

        // CI dots: union of branch tips + open-PR head shas, minus cached-fresh,
        // capped + chunked. On success REBUILD the cache to the current set
        // (replace, not merge) so it never grows unbounded over a long session
        // and gone/404'd tips drop; the previous cache stays shown until the
        // atomic replace (stale-while-revalidate).
        if (showCiRef.current) {
          const tips = branchTipShas(layout);
          const shas = collectCiShas(tips, prHeadShas, ciCacheRef.current, now, TTL_MS, force, MAX);
          if (shas.length > 0) {
            const collected: CommitStatus[] = [];
            for (let i = 0; i < shas.length; i += MAX) {
              const chunk = shas.slice(i, i + MAX);
              const res = await ipc.forgeCommitStatuses(repo, chunk);
              if (reqIdRef.current !== reqId) return;
              collected.push(...res);
            }
            const currentSet = new Set<string>([...tips, ...prHeadShas]);
            const requested = new Set(shas);
            const rebuilt = rebuildCiCache(
              ciCacheRef.current,
              currentSet,
              requested,
              collected,
              Date.now(),
            );
            ciCacheRef.current = rebuilt;
            const badges = new Map<string, CiBadge>();
            for (const [sha, entry] of rebuilt) badges.set(sha, entry.badge);
            setCiBySha(badges);
          }
        }
      } catch (e) {
        // SILENT (decoration, not a user action): keep stale maps, no toast.
        if (import.meta.env.DEV) console.warn('[bonsai] forge signals refresh failed', e);
      }
    },
    [clearDebounce, clearAll, graphDataRef],
  );

  const refresh = useCallback(
    (_reason: string, force = false) => {
      if (!enabledRef.current) {
        reqIdRef.current += 1; // drop any in-flight
        pendingForceRef.current = false;
        clearDebounce();
        clearAll();
        return;
      }
      if (force) pendingForceRef.current = true;
      clearDebounce();
      debounceRef.current = window.setTimeout(() => {
        debounceRef.current = null;
        const f = pendingForceRef.current;
        pendingForceRef.current = false;
        void runFetch(f);
      }, DEBOUNCE_MS);
    },
    [clearDebounce, clearAll, runFetch],
  );

  // Enable / repo transitions: disabling (or switching repo) drops the cache and
  // stops fetching; enabling fetches the current tips. A repo switch always
  // clears first so the previous repo's badges never bleed through.
  useEffect(() => {
    reqIdRef.current += 1;
    clearDebounce();
    clearAll();
    if (enabled) refresh('enable', true);
    return clearDebounce;
    // `refresh`/`clearAll`/`clearDebounce` are stable; re-run only on the gate.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, repoId]);

  return useMemo(() => ({ prByBranch, ciBySha, refresh }), [prByBranch, ciBySha, refresh]);
}
