import type { ComponentProps, RefObject } from 'react';
import { AiOutputPanel } from './AiOutputPanel';
import { BlameView } from './BlameView';
import { DiffBrowser } from './DiffBrowser';
import { DiffOverlay } from './DiffOverlay';
import type { DiffOverlayMeta } from './DiffOverlay';
import { FileHistoryView } from './FileHistoryView';
import { ReflogView } from './ReflogView';
import type { DiffSlot } from './StatusPanel';
import { GraphCanvas } from '../graph/GraphCanvas';
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

  diffSlot: DiffSlot | null;
  overlayMeta: DiffOverlayMeta | null;
  collapseDiffSlot(): void;
  onResolveConflictText: DiffOverlayProps['onResolveConflictText'];
  mutating: boolean;
  overlayExplain: (() => void) | undefined;
  diffViewMode: 'diff' | 'file';
  onSetViewMode: DiffOverlayProps['onSetViewMode'];
  stageable: null | 'stage' | 'unstage';
  onStageLines: DiffOverlayProps['onStageLines'];
  onStageHunk: DiffOverlayProps['onStageHunk'];
  onDiscardHunk: DiffOverlayProps['onStageHunk'];

  blame: { path: string; lines: BlameLine[]; loading: boolean; error: string | null } | null;
  closeBlame(): void;
  revealCommitByOid(oid: string): void;
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
  diffSlot,
  overlayMeta,
  collapseDiffSlot,
  onResolveConflictText,
  mutating,
  overlayExplain,
  diffViewMode,
  onSetViewMode,
  stageable,
  onStageLines,
  onStageHunk,
  onDiscardHunk,
  blame,
  closeBlame,
  revealCommitByOid,
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
  return (
    <main className="graph-pane">
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
        />
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
