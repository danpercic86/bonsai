import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ipc } from '../../ipc';
import { isForgeSuppressed, noteForgeRateLimit } from './forgeBackoff';
import { branchTipShas, ciTtlFor, collectCiShas, isFresh, rebuildCiCache, TTL_MS } from './forgeCiCache';
import type { CiEntry } from './forgeCiCache';
import type { CommitStatus, GraphLayout } from '../../ipc';
import type { CiBadge, PrBadge } from '../../graph/forgeBadges';

/** Coalesce focus + graph-change (+ enable) bursts into ONE fetch. */
const DEBOUNCE_MS = 300;
/** Mirror of the Rust `MAX_STATUS_BATCH` — chunk defensively so a huge branch
 *  count can never exceed the backend cap. */
const MAX = 100;

export interface UseForgeSignals {
  /** branch SHORT-name → PR badge (open PRs only). New identity on each fetch. */
  prByBranch: ReadonlyMap<string, PrBadge>;
  /** commit sha → CI badge (branch tips). New identity on each fetch. */
  ciBySha: ReadonlyMap<string, CiBadge>;
  /** Kick a (debounced) refetch. `force` bypasses the TTL (fetch/pull/manual);
   *  otherwise stale-only (focus/graph-change). Disabled ⇒ clears the maps. */
  refresh(reason: string, force?: boolean): void;
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
  // P113a: the host of the last resolved repo context — the key the rate-limit
  // back-off window is stored under (an ACCOUNT-level budget, shared by every
  // repo on that host), read in the catch where `ctx` is out of scope.
  const hostRef = useRef<string | null>(null);
  const debounceRef = useRef<number | null>(null);
  // P63: force is STICKY across a debounce window — if any coalesced call in the
  // window forced (fetch/pull/manual), the resulting fetch bypasses the TTL even
  // when a later focus/graph-change call did not. Reset when the fetch fires.
  const pendingForceRef = useRef(false);
  // P121a in-flight coalescing: the reqId of the running fetch (null when idle),
  // and a trigger that arrived while it ran. Coalesces instead of dropping (cf.
  // useTagSync's P113b guard): at most ONE rerun, after the batch lands, reading
  // the LATEST layout. `force` is sticky across the coalesced triggers.
  const inFlightRef = useRef<number | null>(null);
  const rerunRef = useRef({ pending: false, force: false });
  // Self-reference for the rerun scheduled from runFetch's own `finally`.
  const runFetchRef = useRef<(force: boolean) => Promise<void>>(async () => {});

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
      if (inFlightRef.current !== null) {
        // A batch is on the wire: never request the identical sha set twice
        // concurrently — fold this trigger into one rerun after it lands.
        rerunRef.current.pending = true;
        rerunRef.current.force ||= force;
        return;
      }
      const layout = graphDataRef.current;
      if (layout === null) return;
      const reqId = ++reqIdRef.current;
      inFlightRef.current = reqId;
      const repo = repoIdRef.current;
      try {
        // Cheap, no network (P62): an unsupported / unparseable origin is inert
        // — clear any stale badges and bail WITHOUT surfacing an error. Any
        // KNOWN provider (gitHub | gitLab | …) proceeds to fetch PR + CI signals.
        const ctx = await ipc.forgeRepoContext(repo);
        if (reqIdRef.current !== reqId) return;
        hostRef.current = ctx.host;
        if (ctx.provider === 'unknown') {
          clearAll();
          return;
        }
        // P113a: the host told us to back off — keep the current badges and make
        // NO further forge call until the advertised wait elapses. Checked after
        // `forgeRepoContext` (local, no network) because that is where the host
        // comes from, and before the PR list, which is what most often trips the
        // limit. `force` does not bypass it: the budget belongs to the server.
        if (isForgeSuppressed(ctx.host)) {
          if (import.meta.env.DEV) {
            console.warn('[bonsai] forge signals suppressed — rate limited', ctx.host);
          }
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
          const shas = collectCiShas(tips, prHeadShas, ciCacheRef.current, now, ciTtlFor, force, MAX);
          if (shas.length > 0) {
            const collected: CommitStatus[] = [];
            // P113a: a batch can come back CUT SHORT (rate limit / auth /
            // network part-way through). What it did resolve is kept; the shas
            // after the stop were never attempted, so we stop chunking and —
            // crucially — treat only the RESOLVED shas as "requested" below, so
            // the un-attempted ones keep their previous badge instead of being
            // dropped as phantom 404s.
            let cutShort = false;
            for (let i = 0; i < shas.length; i += MAX) {
              const chunk = shas.slice(i, i + MAX);
              const res = await ipc.forgeCommitStatuses(repo, chunk);
              if (reqIdRef.current !== reqId) return;
              collected.push(...res.statuses);
              if (res.stoppedBy !== null) {
                cutShort = true;
                noteForgeRateLimit(ctx.host, res.stoppedBy);
                if (import.meta.env.DEV) {
                  console.warn('[bonsai] forge CI batch cut short', res.stoppedBy);
                }
                break;
              }
            }
            const currentSet = new Set<string>([...tips, ...prHeadShas]);
            const requested = cutShort ? new Set(collected.map((s) => s.sha)) : new Set(shas);
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
        // P113a: a rate limit is still recorded, so the NEXT round is suppressed
        // instead of re-requesting the same set. `hostRef` is whatever the
        // context resolved to before the failure (null if that call itself
        // failed — nothing to key a window on then).
        if (hostRef.current !== null) noteForgeRateLimit(hostRef.current, e);
        if (import.meta.env.DEV) console.warn('[bonsai] forge signals refresh failed', e);
      } finally {
        // Invalidated (disable / repo switch reset inFlightRef) ⇒ no-op.
        if (inFlightRef.current === reqId) {
          inFlightRef.current = null;
          if (rerunRef.current.pending && reqIdRef.current === reqId) {
            // The rerun's clearDebounce() would swallow a pending debounced
            // trigger — fold its sticky force in so it is not lost.
            const f = rerunRef.current.force || (debounceRef.current !== null && pendingForceRef.current);
            if (debounceRef.current !== null) pendingForceRef.current = false;
            rerunRef.current = { pending: false, force: false };
            void runFetchRef.current(f);
          }
        }
      }
    },
    [clearDebounce, clearAll, graphDataRef],
  );
  runFetchRef.current = runFetch;

  const refresh = useCallback(
    (_reason: string, force = false) => {
      if (!enabledRef.current) {
        reqIdRef.current += 1; // drop any in-flight
        inFlightRef.current = null;
        rerunRef.current = { pending: false, force: false };
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

  // Unmount-only: invalidate a held batch so its `finally` neither publishes nor
  // fires a coalesced rerun from a dead hook (new IPC after unmount).
  useEffect(
    () => () => {
      reqIdRef.current += 1;
      inFlightRef.current = null;
      rerunRef.current = { pending: false, force: false };
    },
    [],
  );

  // Enable / repo transitions: disabling (or switching repo) drops the cache and
  // stops fetching; enabling fetches the current tips. A repo switch always
  // clears first so the previous repo's badges never bleed through.
  useEffect(() => {
    reqIdRef.current += 1;
    // P121a: a held batch of the previous repo/gate must not block this fetch;
    // its `finally` becomes a no-op.
    inFlightRef.current = null;
    rerunRef.current = { pending: false, force: false };
    clearDebounce();
    clearAll();
    if (enabled) refresh('enable', true);
    return clearDebounce;
    // `refresh`/`clearAll`/`clearDebounce` are stable; re-run only on the gate.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, repoId]);

  return useMemo(() => ({ prByBranch, ciBySha, refresh }), [prByBranch, ciBySha, refresh]);
}
