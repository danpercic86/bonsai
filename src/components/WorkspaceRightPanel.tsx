import type { ComponentProps, RefObject } from 'react';
import { CommitBox } from './CommitBox';
import type { CommitBoxHandle } from './CommitBox';
import { CommitPanel } from './CommitPanel';
import { ComparePanel } from './ComparePanel';
import { OpBanner } from './OpBanner';
import { StatusPanel } from './StatusPanel';
import { shortOid } from './workspaceUtils';
import type {
  AiAnalysisMode,
  AiDiffTarget,
  BranchInfo,
  GraphLayout,
  HeadInfo,
  ListView,
  RepoOpState,
} from '../ipc';

type OpBannerProps = ComponentProps<typeof OpBanner>;
type ComparePanelProps = ComponentProps<typeof ComparePanel>;
type CommitPanelProps = ComponentProps<typeof CommitPanel>;
type StatusPanelProps = ComponentProps<typeof StatusPanel>;
type CommitBoxProps = ComponentProps<typeof CommitBox>;

export interface WorkspaceRightPanelProps {
  rightPanelWidth: number;

  opState: RepoOpState;
  conflicts: StatusPanelProps['conflicts'];
  mutating: boolean;
  onCommitMerge: OpBannerProps['onCommitMerge'];
  onRebaseContinue(): void;
  onRebaseSkip(): void;
  onCherrypickContinue(): void;
  onRevertContinue(): void;
  onAbort(): void;

  compare: { oid: string } | null;
  compareData: ComparePanelProps['data'];
  compareLoading: boolean;
  compareError: ComparePanelProps['error'];
  headBranch: BranchInfo | null;
  listView: ListView;
  scope: ComparePanelProps['scope'];
  setScope: ComparePanelProps['onSelectScope'];
  clearCompare(): void;

  selectedIndex: number | null;
  graph: GraphLayout | null;
  commitDiff: CommitPanelProps['data'];
  commitDiffLoading: boolean;
  commitDiffError: CommitPanelProps['error'];
  setCommitBrowserOpen(v: boolean): void;
  onSelectParent: CommitPanelProps['onSelectParent'];
  setSelectedIndex(i: number | null): void;
  aiEligible: boolean;
  runAnalyze(target: AiDiffTarget, mode: AiAnalysisMode, title: string): void;

  status: StatusPanelProps['snapshot'];
  statusLoading: boolean;
  statusError: StatusPanelProps['error'];
  diffSlot: StatusPanelProps['diffSlot'];
  aiResolvingPath: StatusPanelProps['aiResolvingPath'];
  aiPanelLoading: boolean;
  onStage: StatusPanelProps['onStage'];
  onUnstage: StatusPanelProps['onUnstage'];
  onDiscard: StatusPanelProps['onDiscard'];
  onToggleDiff: StatusPanelProps['onToggleDiff'];
  onResolveConflict: StatusPanelProps['onResolveConflict'];
  onToggleConflictView: StatusPanelProps['onToggleConflictView'];
  onAiResolve: StatusPanelProps['onAiResolve'];
  onBlame: StatusPanelProps['onBlame'];
  onFileHistory: StatusPanelProps['onFileHistory'];

  head: HeadInfo | null;
  amend: boolean;
  onToggleAmend(next: boolean): void;
  amendMessage: string | null;
  commitBoxRef: RefObject<CommitBoxHandle | null>;
  onCommitAmend: CommitBoxProps['onCommit'];
  onCommitMergeSubmit: CommitBoxProps['onCommit'];
  onCommit: CommitBoxProps['onCommit'];
  onGenerate: CommitBoxProps['onGenerate'];
}

/** P3e: the right panel — op banner + the compare / commit-details / status
 *  (working-dir staging + amend + commit box) tri-state. Presentational: all
 *  state + callbacks are threaded in from RepoWorkspace so DOM/behavior are
 *  identical to the inline block it replaced. */
export function WorkspaceRightPanel({
  rightPanelWidth,
  opState,
  conflicts,
  mutating,
  onCommitMerge,
  onRebaseContinue,
  onRebaseSkip,
  onCherrypickContinue,
  onRevertContinue,
  onAbort,
  compare,
  compareData,
  compareLoading,
  compareError,
  headBranch,
  listView,
  scope,
  setScope,
  clearCompare,
  selectedIndex,
  graph,
  commitDiff,
  commitDiffLoading,
  commitDiffError,
  setCommitBrowserOpen,
  onSelectParent,
  setSelectedIndex,
  aiEligible,
  runAnalyze,
  status,
  statusLoading,
  statusError,
  diffSlot,
  aiResolvingPath,
  aiPanelLoading,
  onStage,
  onUnstage,
  onDiscard,
  onToggleDiff,
  onResolveConflict,
  onToggleConflictView,
  onAiResolve,
  onBlame,
  onFileHistory,
  head,
  amend,
  onToggleAmend,
  amendMessage,
  commitBoxRef,
  onCommitAmend,
  onCommitMergeSubmit,
  onCommit,
  onGenerate,
}: WorkspaceRightPanelProps) {
  return (
    <aside className="right-panel" style={{ width: rightPanelWidth }}>
      <OpBanner
        op={opState}
        conflictCount={conflicts.length}
        mutating={mutating}
        onCommitMerge={onCommitMerge}
        onRebaseContinue={() => onRebaseContinue()}
        onRebaseSkip={() => onRebaseSkip()}
        onOpContinue={() =>
          void (opState.kind === 'cherryPick' ? onCherrypickContinue() : onRevertContinue())
        }
        onAbort={onAbort}
      />
      {compare !== null ? (
        <ComparePanel
          data={compareData}
          loading={compareLoading}
          error={compareError}
          headBranchName={headBranch?.name ?? null}
          listView={listView}
          scope={scope}
          onSelectScope={setScope}
          onClose={clearCompare}
        />
      ) : selectedIndex !== null && graph !== null ? (
        <CommitPanel
          node={graph.nodes[selectedIndex]}
          data={commitDiff}
          loading={commitDiffLoading}
          error={commitDiffError}
          listView={listView}
          scope={scope}
          onSelectScope={(s) => {
            setScope(s);
            setCommitBrowserOpen(true);
          }}
          onSelectParent={onSelectParent}
          onClose={() => setSelectedIndex(null)}
          aiEligible={aiEligible}
          onExplain={() => {
            const oid = graph.nodes[selectedIndex].id;
            runAnalyze({ kind: 'commit', oid }, 'explain', `Explain commit ${shortOid(oid)}`);
          }}
        />
      ) : (
        <>
          <StatusPanel
            snapshot={status}
            loading={statusLoading}
            error={statusError}
            busy={mutating}
            diffSlot={diffSlot}
            listView={listView}
            conflicts={conflicts}
            aiEligible={aiEligible}
            aiResolvingPath={aiResolvingPath}
            aiAnalyzing={aiPanelLoading}
            onStage={onStage}
            onUnstage={onUnstage}
            onDiscard={onDiscard}
            onReviewStaged={() =>
              runAnalyze({ kind: 'staged' }, 'review', 'Review staged changes')
            }
            onReviewWorktree={() =>
              runAnalyze({ kind: 'worktree' }, 'review', 'Review working tree')
            }
            onToggleDiff={onToggleDiff}
            onResolveConflict={onResolveConflict}
            onToggleConflictView={onToggleConflictView}
            onAiResolve={onAiResolve}
            onBlame={onBlame}
            onFileHistory={onFileHistory}
          />
          {opState.kind === 'none' && head !== null && !head.unborn && (
            <div className="amend-affordance">
              <label className="amend-toggle">
                <input
                  type="checkbox"
                  checked={amend}
                  disabled={mutating}
                  onChange={(e) => void onToggleAmend(e.target.checked)}
                />
                <span>Amend last commit</span>
              </label>
              {amend &&
                headBranch !== null &&
                headBranch.upstream !== null &&
                headBranch.ahead === 0 && (
                  <div className="amend-push-warning" role="note">
                    This commit is already pushed — amending rewrites published history.
                  </div>
                )}
            </div>
          )}
          <CommitBox
            key={
              amend
                ? 'amend'
                : opState.kind === 'merge'
                  ? `merge:${opState.incoming}`
                  : 'commit'
            }
            ref={commitBoxRef}
            stagedCount={status?.staged.length ?? 0}
            busy={mutating}
            mode={opState.kind === 'merge' && !amend ? 'merge' : 'commit'}
            initialMessage={
              amend
                ? (amendMessage ?? undefined)
                : opState.kind === 'merge'
                  ? opState.message
                  : undefined
            }
            conflictCount={conflicts.length}
            blocked={!amend && opState.kind !== 'none' && opState.kind !== 'merge'}
            amend={amend}
            onCommit={
              amend
                ? onCommitAmend
                : opState.kind === 'merge'
                  ? onCommitMergeSubmit
                  : onCommit
            }
            aiEligible={aiEligible}
            onGenerate={onGenerate}
          />
        </>
      )}
    </aside>
  );
}
