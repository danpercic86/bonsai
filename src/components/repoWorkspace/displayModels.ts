import type {
  BranchesSnapshot,
  CommitDiff,
  CompareDiff,
  GraphColorMode,
  GraphNode,
  GraphPrefs,
} from '../../ipc';
import type { GraphDisplayOptions } from '../../graph/rightColumns';
import { shortOid } from '../workspaceUtils';

/** Ahead/behind map keyed by local branch name, for the graph's ahead/behind
 *  column. Extracted verbatim from RepoWorkspace (size ratchet). */
export function branchStatsOf(
  branches: BranchesSnapshot | null,
): GraphDisplayOptions['branchStats'] {
  const m = new Map<string, { ahead: number | null; behind: number | null }>();
  for (const b of branches?.local ?? []) {
    if (b.ahead !== null && b.behind !== null) m.set(b.name, { ahead: b.ahead, behind: b.behind });
  }
  return m;
}

/** Graph geometry/coloring knobs threaded into the canvas. Extracted verbatim
 *  from RepoWorkspace (size ratchet). */
export function graphDisplayOf(
  graphColorMode: GraphColorMode,
  graphPrefs: GraphPrefs,
  branchStats: GraphDisplayOptions['branchStats'],
  prByBranch: GraphDisplayOptions['prByBranch'],
  ciBySha: GraphDisplayOptions['ciBySha'],
): GraphDisplayOptions {
  return {
    // Spec-006: paint-only edge/ring coloring (lane palette vs author hue).
    colorMode: graphColorMode,
    showSha: graphPrefs.showSha,
    showAuthor: graphPrefs.showAuthor,
    showDate: graphPrefs.showDate,
    dateBasis: graphPrefs.dateBasis,
    showAheadBehind: graphPrefs.showAheadBehind,
    branchStats,
    showSignatureBadge: graphPrefs.showSignatureBadge,
    showPrBadge: graphPrefs.showPrBadge && !graphPrefs.compact,
    showCiStatus: graphPrefs.showCiStatus && !graphPrefs.compact,
    prByBranch,
    ciBySha,
  };
}

/** Default PR base: current upstream, else main/master, else empty. Extracted
 *  verbatim from RepoWorkspace. */
export function prDefaultBaseOf(
  headBranch: { upstream?: string | null } | null,
  branches: BranchesSnapshot | null,
): string | null {
  if (headBranch?.upstream != null && headBranch.upstream !== '') return headBranch.upstream;
  const localNames = (branches?.local ?? []).map((b) => b.name);
  if (localNames.includes('main')) return 'main';
  if (localNames.includes('master')) return 'master';
  return '';
}

/** The center-pane DiffBrowser branch selection (compare > PR > commit), pure.
 *  Extracted verbatim from RepoWorkspace (size ratchet); return type inferred
 *  exactly as the original inline memo. */
export function diffBrowserViewOf(args: {
  compare: { oid: string } | null;
  compareData: CompareDiff | null;
  /** The OID-anchored selected commit (see useStickySelection) — sticky across a
   *  re-stream, so an open commit browser survives a background refetch. */
  selectedNode: GraphNode | null;
  commitBrowserOpen: boolean;
  commitDiff: CommitDiff | null;
  headBranch: { name?: string | null } | null;
  clearCompare: () => void;
  setCommitBrowserOpen: (open: boolean) => void;
}) {
  const {
    compare,
    compareData,
    selectedNode,
    commitBrowserOpen,
    commitDiff,
    headBranch,
    clearCompare,
    setCommitBrowserOpen,
  } = args;
  // Compare mode: AUTO-OPEN once data has loaded and there is at least one file.
  if (compare !== null && compareData !== null && compareData.files.length > 0) {
    const fromLabel = `HEAD${headBranch?.name != null ? ` (${headBranch.name})` : ''}`;
    const toLabel = `${shortOid(compareData.to.oid)} · ${compareData.to.summary}`;
    return {
      source: { mode: 'compare' as const, oid: compare.oid, fromLabel, toLabel },
      files: compareData.files,
      onClose: clearCompare, // × in compare mode exits compare (compare IS the diff)
    };
  }
  // PR mode: AUTO-OPENED by the PR panel (beats commit; compare beats it).
  // Commit mode: EXPLICIT-open only.
  if (selectedNode !== null && commitBrowserOpen && commitDiff !== null) {
    const oid = selectedNode.id;
    return {
      source: {
        mode: 'commit' as const,
        oid,
        title: `${shortOid(oid)} · ${commitDiff.details.summary}`,
      },
      files: commitDiff.files,
      onClose: () => setCommitBrowserOpen(false),
    };
  }
  return null;
}
