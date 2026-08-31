import type { ComponentProps, RefObject } from 'react';
import { AiOutputPanel } from './AiOutputPanel';
import { BlameView } from './BlameView';
import { CommitSearchBar } from './CommitSearchBar';
import type { ComboboxOption } from './Combobox';
import { HistorySearchPanel } from './HistorySearchPanel';
import { DiffBrowser } from './DiffBrowser';
import { DiffOverlay } from './DiffOverlay';
import type { DiffOverlayMeta } from './DiffOverlay';
import { ErrorBoundary } from './ErrorBoundary';
import { FileHistoryView } from './FileHistoryView';
import { ReflogView } from './ReflogView';
import { shortcutLabel } from '../utils/platform';
import type { DiffSlot } from './StatusPanel';
import type { UseCommitSearch } from './repoWorkspace/useCommitSearch';
import type { UseHistorySearch } from './repoWorkspace/useHistorySearch';
import { GraphCanvas } from '../graph/GraphCanvas';
import { GraphSelectionAnnouncer } from './GraphSelectionAnnouncer';
import type { GraphCanvasHandle } from '../graph/GraphCanvas';
import type {
  BlameLine,
  FileHistoryEntry,
  GraphLayout,
  HeadInfo,
  ReflogEntry,
  ResetMode,
} from '../ipc';

type GraphCanvasProps = ComponentProps<typeof GraphCanvas>;
type DiffOverlayProps = ComponentProps<typeof DiffOverlay>;
type DiffBrowserProps = ComponentProps<typeof DiffBrowser>;

export interface WorkspaceGraphPaneProps {
  graphError: string | null;
  graph: GraphLayout | null;
  head: HeadInfo | null;
  graphRef: RefObject<GraphCanvasHandle | null>;
  selectedIndex: number | null;
  compare: { oid: string } | null;
  clearCompare(): void;
  setSelectedIndex(i: number | null): void;
  wip: GraphCanvasProps['wip'];
  themeVersion: number;
  active: boolean;
  onContextMenu: GraphCanvasProps['onContextMenu'];
  metrics: GraphCanvasProps['metrics'];
  metricsVersion: number;
  /** P51b: per-row display toggles + date basis + ahead/behind, forwarded to
   *  GraphCanvas (built from graphPrefs/branches in RepoWorkspace). */
  display: GraphCanvasProps['display'];
  /** P58c: oid → signature verdict for the LIT badge (visible rows, cached). */
  verifyStatus: GraphCanvasProps['verifyStatus'];
  /** P58c: visible-window callback driving the debounced verify request. */
  onVisibleRangeChange: GraphCanvasProps['onVisibleRangeChange'];
  /** P63: a graph PR-badge click → open that PR in the right-pane PR panel. */
  onOpenPr: GraphCanvasProps['onOpenPr'];
  /** P65b: the stream assembler's incremental edge index — forwarded to
   *  GraphCanvas so it skips the O(n)-per-batch one-shot rebuild. */
  edgeIndex: GraphCanvasProps['edgeIndex'];
  /** P65b: total row count for the scroll extent while rows are still arriving. */
  totalRows: GraphCanvasProps['totalRows'];
  /** P84: nonce-driven reveal flash + reduced-motion flag, forwarded to GraphCanvas. */
  revealFlash: GraphCanvasProps['revealFlash'];
  reducedMotion: GraphCanvasProps['reducedMotion'];

  /** P50b: commit-search state (bar + graph highlight + next/prev jump). */
  search: UseCommitSearch;
  /** Branch/ref scope options for the search bar; `value: ''` == all refs. */
  searchScopeOptions: ComboboxOption[];
  /** P57c: semantic-history "Ask history" state (overlay + graph match rings). */
  historySearch: UseHistorySearch;

  diffSlot: DiffSlot | null;
  overlayMeta: DiffOverlayMeta | null;
  collapseDiffSlot(): void;
  onResolveConflictText: DiffOverlayProps['onResolveConflictText'];
  mutating: boolean;
  overlayExplain: (() => void) | undefined;
  diffViewMode: 'diff' | 'file' | 'split';
  onSetViewMode: DiffOverlayProps['onSetViewMode'];
  intraline: boolean;
  onSetIntraline: DiffOverlayProps['onSetIntraline'];
  /** P93: PR number for a `pr:` overlay slot's computed kind chip (pass-through
   *  only — the overlay is mounted here, not in RepoWorkspace). */
  prNumber?: number | null;
  imageDiff: DiffOverlayProps['imageDiff'];
  imageDiffLoading: boolean;
  imageDiffError: string | null;
  stageable: null | 'stage' | 'unstage';
  onStageLines: DiffOverlayProps['onStageLines'];
  onStageHunk: DiffOverlayProps['onStageHunk'];
  onDiscardHunk: DiffOverlayProps['onStageHunk'];
  onDiscardLines: DiffOverlayProps['onDiscardLines'];

  blame: { path: string; lines: BlameLine[]; loading: boolean; error: string | null } | null;
  closeBlame(): void;
  revealCommitByOid(oid: string): void;
  /** P53a: gate the per-block "Why?" affordance in BlameView (aiEligible). */
  blameAiEligible: boolean;
  /** P53a: explain WHY a blame line exists (AI); `path` is the blamed file. */
  onBlameExplain(path: string, lineNo: number): void;
  history: {
    path: string;
    entries: FileHistoryEntry[];
    loading: boolean;
    error: string | null;
  } | null;
  closeHistory(): void;
  reflog: {
    refName: string;
    entries: ReflogEntry[];
    loading: boolean;
    error: string | null;
  } | null;
  closeReflog(): void;
  reflogBusy: boolean;
  reflogResetLabel: string;
  onReflogCreateBranch(newOid: string): void;
  /** Undefined when reset is not allowed (detached/unborn HEAD) → view hides it. */
  onReflogReset?: (newOid: string, mode: ResetMode) => void;
  aiPanel: {
    title: string;
    text: string | null;
    loading: boolean;
    error: string | null;
    costUsd: number | null;
    /** P56b: opt-in editable body (changelog only); other callers omit it. */
    editable?: boolean;
  } | null;
  closeAiPanel(): void;

  diffBrowserView: {
    source: DiffBrowserProps['source'];
    files: DiffBrowserProps['files'];
    onClose: () => void;
  } | null;
  repoId: string;
  scope: DiffBrowserProps['scope'];
  listView: DiffBrowserProps['listView'];
  /** P43b: unborn-HEAD empty state offers a shortcut to set Git identity
   *  (a repo is open here, so SettingsPanel's repo-scoped identity works). */
  onOpenIdentitySettings(): void;
}

/** P3e: the center graph pane — error/truncated banners, the virtualized
 *  GraphCanvas, and the layered diff/blame/history/AI/diff-browser overlays.
 *  Presentational: all state + callbacks are threaded in from RepoWorkspace so
 *  DOM/behavior are identical to the inline block it replaced. */
export function WorkspaceGraphPane({
  graphError,
  graph,
  head,
  graphRef,
  selectedIndex,
  compare,
  clearCompare,
  setSelectedIndex,
  wip,
  themeVersion,
  active,
  onContextMenu,
  metrics,
  metricsVersion,
  display,
  verifyStatus,
  onVisibleRangeChange,
  onOpenPr,
  edgeIndex,
  totalRows,
  revealFlash,
  reducedMotion,
  search,
  searchScopeOptions,
  historySearch,
  diffSlot,
  overlayMeta,
  collapseDiffSlot,
  onResolveConflictText,
  mutating,
  overlayExplain,
  diffViewMode,
  onSetViewMode,
  intraline,
  onSetIntraline,
  prNumber = null,
  imageDiff,
  imageDiffLoading,
  imageDiffError,
  stageable,
  onStageLines,
  onStageHunk,
  onDiscardHunk,
  onDiscardLines,
  blame,
  closeBlame,
  revealCommitByOid,
  blameAiEligible,
  onBlameExplain,
  history,
  closeHistory,
  reflog,
  closeReflog,
  reflogBusy,
  reflogResetLabel,
  onReflogCreateBranch,
  onReflogReset,
  aiPanel,
  closeAiPanel,
  diffBrowserView,
  repoId,
  scope,
  listView,
  onOpenIdentitySettings,
}: WorkspaceGraphPaneProps) {
  // P50b: the floating search affordance is only shown over a bare graph — hide
  // it while any overlay covers the pane (it would poke through the corner).
  const anyOverlayOpen =
    diffSlot !== null ||
    blame !== null ||
    history !== null ||
    reflog !== null ||
    aiPanel !== null ||
    diffBrowserView !== null;
  return (
    <main className="graph-pane">
      {/* M1: polite live region announcing the settled graph-grid selection
          (canvas is opaque to SR). Permanently mounted for reliable pickup. */}
      <GraphSelectionAnnouncer graph={graph} selectedIndex={selectedIndex} display={display} />
      {/* P50b: search bar at the top of the pane while open; a floating affordance
          otherwise (Ctrl/Cmd-F also opens it — the webview may steal that in the
          browser harness, so the button is the always-reachable entry point). */}
      {search.open ? (
        <CommitSearchBar
          query={search.query}
          patchQuery={search.patchQuery}
          submit={search.submit}
          close={search.close}
          results={search.results}
          loading={search.loading}
          error={search.error}
          currentMatch={search.currentMatch}
          needsSubmit={search.needsSubmit}
          next={search.next}
          prev={search.prev}
          goToMatch={search.goToMatch}
          scopeOptions={searchScopeOptions}
          openNonce={search.openNonce}
        />
      ) : (
        graph !== null &&
        head?.unborn !== true &&
        !anyOverlayOpen &&
        !historySearch.open && (
          <button
            type="button"
            className="graph-search-fab"
            title={`Search commits (${shortcutLabel('Mod+F')})`}
            aria-label="Search commits"
            onClick={() => search.openSearch()}
          >
            ⌕
          </button>
        )
      )}
      {/* P57c: the "Ask history" overlay (semantic search + AI answer). Its own
          top overlay, independent of the P50 literal-search bar. */}
      {historySearch.open && (
        <HistorySearchPanel historySearch={historySearch} revealCommitByOid={revealCommitByOid} />
      )}
      {graphError !== null && (
        <div className="error-banner graph-error-banner">{graphError}</div>
      )}
      {graph !== null && graph.truncated && (
        <div className="graph-truncated-banner">
          History truncated to the most recent 100,000 commits
        </div>
      )}
      {head?.unborn ? (
        <div className="graph-pane-empty">
          <div className="graph-pane-empty-card">
            <span className="graph-pane-empty-mark" aria-hidden="true">
              {'🌱'}
            </span>
            <p className="graph-pane-empty-title">No commits yet</p>
            <p className="pane-empty">
              Stage your changes and write your first commit in the panel on the right — it will
              appear here as the root of your history.
            </p>
            <button
              type="button"
              className="btn-secondary graph-pane-empty-identity"
              onClick={onOpenIdentitySettings}
            >
              Set your Git identity
            </button>
          </div>
        </div>
      ) : graph !== null ? (
        <ErrorBoundary label="Commit graph">
          <GraphCanvas
            ref={graphRef}
            layout={graph}
            selectedIndex={selectedIndex}
            onSelect={(i) => {
              // Left-clicking any row exits Compare mode (P5 §5.4). Scope reset
              // + commit-browser close are handled by the §4.2 effect
              // (selectedIndex dep); selecting does NOT auto-open the browser
              // (P11g-rev Change C asymmetry).
              if (compare !== null) clearCompare();
              setSelectedIndex(i);
            }}
            wip={wip}
            themeVersion={themeVersion}
            active={active}
            onContextMenu={onContextMenu}
            metrics={metrics}
            metricsVersion={metricsVersion}
            // P57c: while the Ask-history overlay is open its hit rings take the
            // shared matchRows channel; otherwise the P50 search rings do. Both
            // are memoized in their hooks, so this stays reference-stable.
            matchRows={historySearch.open ? historySearch.matchRows : search.matchRows}
            display={display}
            verifyStatus={verifyStatus}
            onVisibleRangeChange={onVisibleRangeChange}
            onOpenPr={onOpenPr}
            edgeIndex={edgeIndex}
            totalRows={totalRows}
            revealFlash={revealFlash}
            reducedMotion={reducedMotion}
          />
        </ErrorBoundary>
      ) : null}
      {diffSlot !== null && overlayMeta !== null && (
        <DiffOverlay
          slot={diffSlot}
          meta={overlayMeta}
          onClose={collapseDiffSlot}
          onResolveConflictText={onResolveConflictText}
          mutating={mutating}
          onExplain={overlayExplain}
          viewMode={diffViewMode}
          onSetViewMode={onSetViewMode}
          intraline={intraline}
          onSetIntraline={onSetIntraline}
          prNumber={prNumber}
          imageDiff={imageDiff}
          imageLoading={imageDiffLoading}
          imageError={imageDiffError}
          stageable={stageable}
          onStageLines={onStageLines}
          onStageHunk={onStageHunk}
          onDiscardHunk={
            // P28: unstaged tracked diffs only — never untracked (tracked-only
            // discard) and never binary/tooLarge/renamed (stageable === null).
            overlayMeta.kind === 'unstaged' && stageable === 'stage'
              ? onDiscardHunk
              : undefined
          }
          onDiscardLines={
            // P45: same gate as onDiscardHunk — unstaged tracked diffs only.
            overlayMeta.kind === 'unstaged' && stageable === 'stage'
              ? onDiscardLines
              : undefined
          }
        />
      )}
      {/* P23d: blame + file-history overlays, layered over the graph like the
          diff overlay. Only one of the two is ever set (each handler clears
          the other); they render above DiffOverlay in the DOM. */}
      {blame !== null && (
        <BlameView
          path={blame.path}
          lines={blame.lines}
          loading={blame.loading}
          error={blame.error}
          onClose={closeBlame}
          onRevealCommit={revealCommitByOid}
          aiEligible={blameAiEligible}
          onExplainBlock={(_oid, lineNo) => onBlameExplain(blame.path, lineNo)}
        />
      )}
      {history !== null && (
        <FileHistoryView
          path={history.path}
          entries={history.entries}
          loading={history.loading}
          error={history.error}
          onClose={closeHistory}
          onRevealCommit={revealCommitByOid}
        />
      )}
      {/* P38: reflog overlay — a sibling read overlay to blame/history. Only
          one read overlay is ever open (openReflog clears blame/history). */}
      {reflog !== null && (
        <ReflogView
          refName={reflog.refName}
          entries={reflog.entries}
          loading={reflog.loading}
          error={reflog.error}
          busy={reflogBusy}
          resetBranchLabel={reflogResetLabel}
          onClose={closeReflog}
          onRevealCommit={revealCommitByOid}
          onCreateBranch={onReflogCreateBranch}
          onReset={onReflogReset}
        />
      )}
      {aiPanel !== null && (
        <AiOutputPanel
          title={aiPanel.title}
          text={aiPanel.text}
          loading={aiPanel.loading}
          error={aiPanel.error}
          costUsd={aiPanel.costUsd}
          editable={aiPanel.editable}
          onClose={closeAiPanel}
        />
      )}
      {/* P11g-rev §4.5: all-files DiffBrowser (header + stacked scroll only)
          over the canvas. Compare mode auto-opens; commit mode is
          explicit-open. The `key` on source.oid remounts fresh for a
          DIFFERENT target/commit (clears cache+queue) but survives a refetch
          of the SAME oid. */}
      {diffBrowserView !== null && (
        <DiffBrowser
          key={`${diffBrowserView.source.mode}:${diffBrowserView.source.oid}`}
          repoId={repoId}
          source={diffBrowserView.source}
          files={diffBrowserView.files}
          scope={scope}
          listView={listView}
          onClose={diffBrowserView.onClose}
        />
      )}
    </main>
  );
}
