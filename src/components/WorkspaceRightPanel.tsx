import type { ComponentProps, RefObject } from 'react';
import { CommitBox } from './CommitBox';
import type { CommitBoxHandle } from './CommitBox';
import { CommitPanel } from './CommitPanel';
import { ComparePanel } from './ComparePanel';
import { OpBanner } from './OpBanner';
import { PrPanel } from './PrPanel';
import { StatusPanel } from './StatusPanel';
import { StashSplitButton } from './StashSplitButton';
import { shortOid } from './workspaceUtils';
import type {
  AiAnalysisMode,
  AiDiffTarget,
  BranchInfo,
  CommitVerification,
  GraphLayout,
  HeadInfo,
  ListView,
  PrNavRequest,
  RepoOpState,
  SigningStatus,
  StashScope,
} from '../ipc';

type OpBannerProps = ComponentProps<typeof OpBanner>;
type ComparePanelProps = ComponentProps<typeof ComparePanel>;
type CommitPanelProps = ComponentProps<typeof CommitPanel>;
type StatusPanelProps = ComponentProps<typeof StatusPanel>;
type CommitBoxProps = ComponentProps<typeof CommitBox>;

export interface WorkspaceRightPanelProps {
  rightPanelWidth: number;

  /** P62c: canonical repo id — threaded to the PR panel under the 'prs' tab. */
  repoId: string;
  /** P62c: active right-pane tab (owned by RepoWorkspace). */
  rightPaneTab: 'work' | 'prs';
  onSelectRightPaneTab(tab: 'work' | 'prs'): void;
  /** P62c: current branch name — seeds the PR create form's compare field. */
  prDefaultHead: string | null;
  /** P63: external "open PR N" request from a graph PR-badge click (bumped
   *  `seq` re-opens the same PR). Threaded into PrPanel's `openToPr`. */
  prNav: PrNavRequest | null;

  opState: RepoOpState;
  conflicts: StatusPanelProps['conflicts'];
  mutating: boolean;
  onCommitMerge: OpBannerProps['onCommitMerge'];
  onRebaseContinue(): void;
  onRebaseSkip(): void;
  onCherrypickContinue(): void;
  onRevertContinue(): void;
  onAbort(): void;
  /** P39b: bisect Good/Bad + Skip; Reset reuses onAbort's confirm. */
  onBisectMark: OpBannerProps['onBisectMark'];
  onBisectSkip: OpBannerProps['onBisectSkip'];
  bisectSummaries: OpBannerProps['bisectSummaries'];

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
  onDiscardForce: StatusPanelProps['onDiscardForce'];
  onToggleDiff: StatusPanelProps['onToggleDiff'];
  onResolveConflict: StatusPanelProps['onResolveConflict'];
  onToggleConflictView: StatusPanelProps['onToggleConflictView'];
  onAiResolve: StatusPanelProps['onAiResolve'];
  onBlame: StatusPanelProps['onBlame'];
  onFileHistory: StatusPanelProps['onFileHistory'];
  /** P34: stash the worktree per scope (staging-panel split button + sidebar). */
  onCreateStash(scope: StashScope): void;

  head: HeadInfo | null;
  amend: boolean;
  onToggleAmend(next: boolean): void;
  amendMessage: string | null;
  commitBoxRef: RefObject<CommitBoxHandle | null>;
  onCommitAmend: CommitBoxProps['onCommit'];
  onCommitMergeSubmit: CommitBoxProps['onCommit'];
  onCommit: CommitBoxProps['onCommit'];
  /** Normal-commit path only: commit then push (threaded to CommitBox's split control). */
  onCommitAndPush: CommitBoxProps['onCommitAndPush'];
  onGenerate: CommitBoxProps['onGenerate'];
  /** P54c: working tree has changes — gates the "Compose commits ✨" affordance. */
  workingDirty: boolean;
  /** P54c: open the commit composer (working-changes affordance). */
  onCompose: () => void;
  /** P40b: open Settings → Git config → Identity from a `configMissing` commit
   *  error banner. */
  onOpenIdentitySettings: CommitBoxProps['onOpenIdentitySettings'];
  /** P58c: effective signing config (drives the CommitBox sign toggle + hint). */
  signingStatus: SigningStatus | null;
  /** P58c: the selected commit's signature verdict (CommitPanel line); null when
   *  unverified / disabled / unsigned. */
  commitSignature: CommitVerification | null;
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
  onBisectMark,
  onBisectSkip,
  bisectSummaries,
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
  onDiscardForce,
  onToggleDiff,
  onResolveConflict,
  onToggleConflictView,
  onAiResolve,
  onBlame,
  onFileHistory,
  onCreateStash,
  head,
  amend,
  onToggleAmend,
  amendMessage,
  commitBoxRef,
  onCommitAmend,
  onCommitMergeSubmit,
  onCommit,
  onCommitAndPush,
  onGenerate,
  workingDirty,
  onCompose,
  onOpenIdentitySettings,
  signingStatus,
  commitSignature,
  repoId,
  rightPaneTab,
  onSelectRightPaneTab,
  prDefaultHead,
  prNav,
}: WorkspaceRightPanelProps) {
  return (
    <aside className="right-panel" style={{ width: rightPanelWidth }}>
      <div className="right-pane-tabs" role="tablist" aria-label="Right panel view">
        <button
          type="button"
          role="tab"
          aria-selected={rightPaneTab === 'work'}
          className={`right-pane-tab${rightPaneTab === 'work' ? ' active' : ''}`}
          onClick={() => onSelectRightPaneTab('work')}
        >
          Working
        </button>
        <button
          type="button"
          role="tab"
          aria-selected={rightPaneTab === 'prs'}
          className={`right-pane-tab${rightPaneTab === 'prs' ? ' active' : ''}`}
          onClick={() => onSelectRightPaneTab('prs')}
        >
          Pull requests
        </button>
      </div>
      <div className="right-panel-work" hidden={rightPaneTab !== 'work'}>
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
        onBisectMark={onBisectMark}
        onBisectSkip={onBisectSkip}
        bisectSummaries={bisectSummaries}
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
          signature={commitSignature}
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
            onDiscardForce={onDiscardForce}
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
            <StashSplitButton
              disabled={
                mutating ||
                ((status?.staged.length ?? 0) === 0 &&
                  (status?.unstaged.length ?? 0) === 0 &&
                  (status?.untracked.length ?? 0) === 0)
              }
              stagedCount={status?.staged.length ?? 0}
              hasTrackedChanges={
                (status?.staged.length ?? 0) > 0 || (status?.unstaged.length ?? 0) > 0
              }
              hasUntracked={(status?.untracked.length ?? 0) > 0}
              onStash={onCreateStash}
            />
          )}
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
            onCommitAndPush={amend || opState.kind === 'merge' ? undefined : onCommitAndPush}
            aiEligible={aiEligible}
            onGenerate={onGenerate}
            workingDirty={workingDirty}
            onCompose={onCompose}
            onOpenIdentitySettings={onOpenIdentitySettings}
            signingStatus={signingStatus}
          />
        </>
      )}
      </div>
      {rightPaneTab === 'prs' && (
        <PrPanel repoId={repoId} defaultHead={prDefaultHead} openToPr={prNav} />
      )}
    </aside>
  );
}
