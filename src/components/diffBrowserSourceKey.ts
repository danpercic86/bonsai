import type { DiffBrowserSource } from './DiffBrowser';

/** Stable identity for a DiffBrowser source's diff target — the cache-key
 *  prefix AND the remount `key` seed in WorkspaceGraphPane. For commit/compare
 *  it is the oid; for a PR it is `#N:mergeBase..head` so a different PR (or a
 *  head advance) remounts fresh while a same-target refetch keeps the cache.
 *  Own module (not DiffBrowser.tsx) to keep that file component-only for HMR. */
export function diffBrowserSourceKey(src: DiffBrowserSource): string {
  return src.mode === 'pr' ? `#${src.prNumber}:${src.mergeBaseOid}..${src.headOid}` : src.oid;
}
