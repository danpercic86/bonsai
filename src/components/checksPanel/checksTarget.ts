// P90: PURE resolver — maps a sidebar reveal click to a CI-checks target branch,
// or null when the click is not a branch (tag / stash / oid) or the ref is not in
// the branches snapshot. No React, no IPC — unit-tested headlessly.
import type { RevealTarget } from '../../graph/reveal';
import type { BranchesSnapshot } from '../../ipc';

export interface ChecksTarget {
  /** Display name: "main" | "origin/main". */
  name: string;
  /** Full 40-hex tip oid to query forge status for. */
  tip: string;
  /** False for local branches with no configured upstream (drives a soft hint). */
  hasUpstream: boolean;
}

/**
 * Resolve a sidebar reveal click to a checks target, or null.
 *
 * Rules (contract §TypeScript surface):
 * - `kind:'oid'` (stashes / raw oids) → null.
 * - `kind:'ref'`: match `branches.local` by name (hasUpstream = upstream !== null,
 *   tip from BranchInfo.tip); else match `branches.remote` by name (hasUpstream =
 *   true, tip from RemoteBranchInfo.tip); tag names (in neither list) → null.
 */
export function resolveChecksTarget(
  target: RevealTarget,
  branches: BranchesSnapshot | null,
): ChecksTarget | null {
  if (target.kind !== 'ref' || branches === null) return null;

  const local = branches.local.find((b) => b.name === target.name);
  if (local !== undefined) {
    return { name: local.name, tip: local.tip, hasUpstream: local.upstream !== null };
  }

  const remote = branches.remote.find((r) => r.name === target.name);
  if (remote !== undefined) {
    return { name: remote.name, tip: remote.tip, hasUpstream: true };
  }

  // A tag (or any ref not present as a branch) has no checks target.
  return null;
}
