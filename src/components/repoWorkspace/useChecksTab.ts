// P90: owns the right-pane Checks tab's branch scoping + refresh counter, kept out
// of RepoWorkspace (a god-file) so it doesn't grow. The tab is scoped to the last
// branch/remote row revealed in the sidebar (else HEAD); `refreshSeq` is bumped on
// fetch/pull/focus to force a silent refetch inside `useBranchChecks`.
import { useCallback, useMemo, useState } from 'react';
import type { RevealTarget } from '../../graph/reveal';
import type { BranchesSnapshot } from '../../ipc';
import { resolveChecksTarget, type ChecksTarget } from '../checksPanel/checksTarget';

export interface ChecksTabState {
  target: ChecksTarget | null;
  refreshSeq: number;
  /** Re-scope the tab from a sidebar reveal (no-op for tags / stashes / oids). */
  revealBranch(t: RevealTarget): void;
  /** Force a silent refetch (fetch / pull / focus). */
  bumpRefresh(): void;
}

export function useChecksTab(
  branches: BranchesSnapshot | null,
  headBranchName: string | null,
): ChecksTabState {
  const [refName, setRefName] = useState<string | null>(null);
  const [refreshSeq, setRefreshSeq] = useState(0);

  const target = useMemo(() => {
    const name = refName ?? headBranchName;
    return name === null ? null : resolveChecksTarget({ kind: 'ref', name }, branches);
  }, [refName, headBranchName, branches]);

  const revealBranch = useCallback(
    (t: RevealTarget) => {
      if (t.kind === 'ref' && resolveChecksTarget(t, branches) !== null) setRefName(t.name);
    },
    [branches],
  );

  const bumpRefresh = useCallback(() => setRefreshSeq((s) => s + 1), []);

  return { target, refreshSeq, revealBranch, bumpRefresh };
}
