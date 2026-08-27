import { useCallback, useMemo, useState } from 'react';
import type { PrDiffStats } from '../../ipc';
import type { DiffBrowserSource } from '../DiffBrowser';
import type { DiffScope } from '../DiffFileTree';

// PR-mode center-pane diff browser state (extracted from RepoWorkspace). The
// PR panel calls `openPrDiff` once a PR detail's locally-computed diff resolves
// (auto-open, like compare mode) and `closePrDiff` when the detail goes away.
// `prBrowserView` is the ready-made diffBrowserView branch: RepoWorkspace's
// memo returns it after the compare branch (compare wins) and before the
// commit branch (an open PR wins over a stale commit selection).

export interface PrBrowserView {
  source: DiffBrowserSource;
  files: PrDiffStats['files'];
  onClose(): void;
}

export function usePrDiffBrowser(
  setScope: (scope: DiffScope) => void,
  setCommitBrowserOpen: (open: boolean) => void,
  compareActiveRef: { readonly current: object | null },
): {
  openPrDiff(stats: PrDiffStats, prNumber: number, title: string): void;
  closePrDiff(): void;
  prBrowserView: PrBrowserView | null;
} {
  const [prBrowser, setPrBrowser] = useState<{
    stats: PrDiffStats;
    prNumber: number;
    title: string;
  } | null>(null);

  const openPrDiff = useCallback(
    (stats: PrDiffStats, prNumber: number, title: string) => {
      // Compare beats the PR browser in the diffBrowserView memo. Opening the
      // PR browser while compare is active would render nothing yet clobber
      // compare's file scope / commit-browser flag — suppress it entirely.
      // Once compare closes, "View diffs" / a row click can open it then.
      if (compareActiveRef.current !== null) return;
      setScope({ kind: 'root' }); // a stale file/dir scope would filter the PR's files
      setCommitBrowserOpen(false); // pr wins over a stale explicit commit browser
      setPrBrowser({ stats, prNumber, title });
    },
    [setScope, setCommitBrowserOpen, compareActiveRef],
  );
  const closePrDiff = useCallback(() => setPrBrowser(null), []);

  const prBrowserView = useMemo<PrBrowserView | null>(() => {
    if (prBrowser === null || prBrowser.stats.files.length === 0) return null;
    return {
      source: {
        mode: 'pr' as const,
        mergeBaseOid: prBrowser.stats.mergeBaseOid,
        headOid: prBrowser.stats.headOid,
        prNumber: prBrowser.prNumber,
        title: prBrowser.title,
      },
      files: prBrowser.stats.files,
      onClose: closePrDiff,
    };
  }, [prBrowser, closePrDiff]);

  return { openPrDiff, closePrDiff, prBrowserView };
}
