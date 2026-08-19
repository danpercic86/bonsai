/**
 * P69 §5.1 — the effective-identity store.
 *
 * Git resolves an identity as **local overrides global** (then system). Every
 * surface that shows "who will author the next commit here" must therefore read the
 * EFFECTIVE identity, not the repo's local block: a repo with no local `user.name`
 * still commits fine using the global one. Reading only `local` is what made the
 * Identity-profiles "Active on this repo" pill stay dark in the ordinary case
 * (UI D6) — that logic is deleted in favour of this module.
 *
 * Design, per contract §5.1:
 *  - ONE `ipc.getConfig(repoId, 'local')` per repo. `CuratedConfigEntry` already
 *    carries `effectiveValue` + `effectiveLevel`, so the global fallback needs no
 *    second call — cheaper than two reads and atomic (one snapshot, not two).
 *  - A module-level cache + listener set read through `useSyncExternalStore`, so the
 *    two (soon three) surfaces that show identity can never disagree.
 *  - Invalidation is EXPLICIT (`invalidateEffectiveIdentity`) — there is deliberately
 *    no `repo-changed` subscription, because `setConfig`/`unsetConfig` do not emit it
 *    (`ipc/types.ts:2263`). Call it after a successful `applyIdentityProfile` or a
 *    `setConfig`/`unsetConfig` of a `user.*` key.
 *  - DEFERRED (P69h): §5.1's "repo close evicts the entry" trigger has no
 *    implementation — the intended API is `forgetEffectiveIdentity(repoId)`, and until
 *    it exists the map keeps one small record per repo visited this session.
 *  - Out-of-band edits (`git config` in a terminal) leave the cache stale until the
 *    next invalidation; the Git-config pane's existing refetch / `Try again` is the
 *    manual refresh. No focus-rescan: that would fire a `getConfig` on every window
 *    focus for a value that changes monthly.
 */
import { useCallback, useEffect, useSyncExternalStore } from 'react';

import { ipc } from '../ipc';
import type { ConfigLevelName, ConfigView } from '../ipc';
import { errorMessage } from '../utils/errors';

export interface EffectiveIdentity {
  name: string | null;
  email: string | null;
  signingKey: string | null;
  /** Where the effective `user.name` came from; `null` => unset everywhere. Falls
   *  back to `user.email`'s level when the name is unset. */
  source: ConfigLevelName | null;
  /** True only while the FIRST read for this repo is in flight. */
  loading: boolean;
  /** `getConfig` rejected: name/email/source are null and the UI shows the
   *  "couldn't read" state instead of a confident (wrong) identity. NOTE: an error
   *  snapshot is cached like any other, so a transient failure sticks until the next
   *  `invalidateEffectiveIdentity` — there is no retry timer and no auto-refetch. */
  error: string | null;
}

/** Stable snapshots — `useSyncExternalStore` compares by reference, so these must
 *  never be rebuilt per render. */
const UNSET: EffectiveIdentity = Object.freeze({
  name: null,
  email: null,
  signingKey: null,
  source: null,
  loading: false,
  error: null,
});
const PENDING: EffectiveIdentity = Object.freeze({ ...UNSET, loading: true });

const cache = new Map<string, EffectiveIdentity>();
/** Repos whose first (or post-invalidation) read is in flight — the dedupe that
 *  keeps a second mount during the fetch from issuing a second `getConfig`. */
const inFlight = new Set<string>();
/** Bumped on invalidation so a late reply from a superseded fetch is discarded. */
const generation = new Map<string, number>();
const listeners = new Set<() => void>();

function emit(): void {
  for (const listener of [...listeners]) listener();
}

/** Blank is unset: a `user.name = ""` is not an identity. */
function nonBlank(value: string | null | undefined): string | null {
  if (value === undefined || value === null) return null;
  return value.trim() === '' ? null : value;
}

function readCurated(view: ConfigView, key: string) {
  const lower = key.toLowerCase();
  return view.curated.find((c) => c.key.toLowerCase() === lower);
}

/** `user.signingkey` is not in `CURATED_KEYS`, so when it is not curated it can only
 *  be seen through the advanced (target-level) list — i.e. a LOCAL signing key. A
 *  global-only signing key is invisible here; it is display-only information and the
 *  Git-config pane remains the place that shows every level. */
function readSigningKey(view: ConfigView): string | null {
  const curated = readCurated(view, 'user.signingkey');
  if (curated !== undefined) return nonBlank(curated.effectiveValue);
  const advanced = view.advanced.find((a) => a.name.toLowerCase() === 'user.signingkey');
  return nonBlank(advanced?.value);
}

export function identityFromConfigView(view: ConfigView): EffectiveIdentity {
  const nameEntry = readCurated(view, 'user.name');
  const emailEntry = readCurated(view, 'user.email');
  const name = nonBlank(nameEntry?.effectiveValue);
  const email = nonBlank(emailEntry?.effectiveValue);
  const source =
    name !== null ? (nameEntry?.effectiveLevel ?? null) : email !== null ? (emailEntry?.effectiveLevel ?? null) : null;
  return {
    name,
    email,
    signingKey: readSigningKey(view),
    source,
    loading: false,
    error: null,
  };
}

function load(repoId: string): void {
  if (cache.has(repoId) || inFlight.has(repoId)) return;
  const seq = generation.get(repoId) ?? 0;
  inFlight.add(repoId);
  void ipc
    .getConfig(repoId, 'local')
    .then(identityFromConfigView, (e: unknown) => ({ ...UNSET, error: errorMessage(e) }))
    .then((next) => {
      // A newer invalidation superseded this read — drop it rather than resurrect a
      // value that is already known to be stale. The `inFlight` marker is NOT cleared
      // here: it belongs to the newer read now, and clearing it would let the next
      // mount fire a duplicate `getConfig` while that read is still in flight.
      if ((generation.get(repoId) ?? 0) !== seq) return;
      inFlight.delete(repoId);
      cache.set(repoId, next);
      emit();
    });
}

/**
 * P69h — announce that a LOCAL `getConfig` for this repo is already in flight
 * somewhere else (the Git-config pane's own load), so a `useEffectiveIdentity`
 * consumer mounting meanwhile waits for that read instead of firing a second one.
 *
 * Returns the generation the claim was made against. Hand it back to
 * `primeEffectiveIdentity` / `failEffectiveIdentity`: an `invalidateEffectiveIdentity`
 * landing between the claim and its settlement bumps the generation, and without
 * the token the settling call would happily cache its PRE-invalidation snapshot
 * — permanently, since the store's own fresh read then fails ITS generation
 * check and is dropped, leaving a stale identity with nothing in flight to
 * correct it.
 */
export function claimEffectiveIdentity(repoId: string): number {
  // Marking the read in flight is what actually removes the duplicate: a consumer
  // that mounts while the claimed read is still pending sees the PENDING snapshot
  // and issues nothing. Every claim MUST be settled by `primeEffectiveIdentity`
  // or `failEffectiveIdentity`, or that consumer would wait forever.
  inFlight.add(repoId);
  return generation.get(repoId) ?? 0;
}

/** True when a claim has been superseded; also releases its in-flight marker.
 *  Releasing costs at most one duplicate read (the newer owner still holds its
 *  own claim, or `invalidateEffectiveIdentity` has already started a read), and
 *  that is strictly better than the alternative failure — a subscriber stranded
 *  on the loading snapshot forever. */
function claimIsStale(repoId: string, claimSeq: number | undefined): boolean {
  if (claimSeq === undefined || (generation.get(repoId) ?? 0) === claimSeq) return false;
  inFlight.delete(repoId);
  return true;
}

/** Settle a claim that failed. The identity is genuinely unknown, so publish that
 *  (state 3, "couldn't read") rather than leaving subscribers spinning. */
export function failEffectiveIdentity(repoId: string, message: string, claimSeq?: number): void {
  if (claimIsStale(repoId, claimSeq)) return;
  generation.set(repoId, (generation.get(repoId) ?? 0) + 1);
  inFlight.delete(repoId);
  cache.set(repoId, { ...UNSET, error: message });
  emit();
}

/**
 * P69h — publish an identity read from a `ConfigView` that has ALREADY been
 * fetched, instead of fetching one.
 *
 * The Git-config pane loads `getConfig(repoId, 'local')` for its own form, and
 * that response answers the identity question too (`CuratedConfigEntry` carries
 * `effectiveValue` + `effectiveLevel`) — handing it over here is what collapses
 * the pane's three mount reads into one. It is authoritative-now: the generation
 * counter is bumped so an older in-flight `load` cannot resurrect a staler value.
 *
 * The caller MUST pass a LOCAL-level view — a global-level view's `targetValue`s
 * belong to the other file, and `readSigningKey`'s advanced fallback would then
 * report a global key as a local one.
 *
 * `claimSeq` (the token from `claimEffectiveIdentity`) makes the publish
 * conditional: a view fetched before an invalidation is DROPPED rather than
 * being cached over the newer truth.
 */
export function primeEffectiveIdentity(
  repoId: string,
  localView: ConfigView,
  claimSeq?: number,
): void {
  if (claimIsStale(repoId, claimSeq)) return;
  generation.set(repoId, (generation.get(repoId) ?? 0) + 1);
  inFlight.delete(repoId);
  cache.set(repoId, identityFromConfigView(localView));
  emit();
}

/** Drop the cache entry, refetch, notify every subscriber. Subscribers see the
 *  loading snapshot in between, which is honest: the value on screen is no longer
 *  known to be true. */
export function invalidateEffectiveIdentity(repoId: string): void {
  generation.set(repoId, (generation.get(repoId) ?? 0) + 1);
  cache.delete(repoId);
  inFlight.delete(repoId);
  emit();
  load(repoId);
}

/** Test-only: forget every repo (module state outlives a component tree). */
export function resetEffectiveIdentityForTests(): void {
  cache.clear();
  inFlight.clear();
  generation.clear();
}

function subscribe(onStoreChange: () => void): () => void {
  listeners.add(onStoreChange);
  return () => {
    listeners.delete(onStoreChange);
  };
}

/** `repoId === null` => the unset snapshot and NO IPC call. */
export function useEffectiveIdentity(repoId: string | null): EffectiveIdentity {
  const getSnapshot = useCallback(
    () => (repoId === null ? UNSET : (cache.get(repoId) ?? PENDING)),
    [repoId],
  );

  useEffect(() => {
    if (repoId !== null) load(repoId);
  }, [repoId]);

  return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
