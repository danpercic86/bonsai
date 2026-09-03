/** The graph's uncommitted-changes (WIP) row summary, derived from the status
 *  snapshot.
 *
 *  Split out of RepoWorkspace because its OBJECT IDENTITY matters: `wip` is a
 *  dependency of `useScrollSelectionIntoView`, so a fresh object on every
 *  background status refetch re-ran the scroll-into-view adjustment even when
 *  nothing changed. The count is derived as a primitive first, so the returned
 *  object only changes when its VALUE changes. */

import { useMemo } from 'react';

import type { WipSummary } from '../../graph/GraphCanvas';
import type { HeadInfo, StatusSnapshot } from '../../ipc';

/** Distinct paths touched in the working tree, or `null` when there is no WIP
 *  row to draw (no snapshot yet, unborn HEAD, or a clean tree). Pure. */
export function wipFileCountOf(
  status: StatusSnapshot | null,
  head: HeadInfo | null,
): number | null {
  if (status === null || head?.unborn === true) return null;
  const paths = new Set<string>();
  for (const s of [status.staged, status.unstaged, status.untracked, status.conflicted]) {
    for (const e of s) paths.add(e.path);
  }
  return paths.size > 0 ? paths.size : null;
}

export function useWipSummary(
  status: StatusSnapshot | null,
  head: HeadInfo | null,
): WipSummary | null {
  // Two memos on purpose: the first keeps the O(files) scan off every render,
  // the second is keyed on the PRIMITIVE count so a refetch that yields the same
  // count reuses the exact same object.
  const fileCount = useMemo(() => wipFileCountOf(status, head), [status, head]);
  return useMemo(() => (fileCount === null ? null : { fileCount }), [fileCount]);
}
