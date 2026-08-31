import type { ComponentProps, RefObject } from 'react';
import type { ComboboxOption } from './Combobox';
import { CommitBox } from './CommitBox';
import type { CommitBoxHandle } from './CommitBox';
import { ChecksPanel } from './checksPanel/ChecksPanel';
import type { ChecksTarget } from './checksPanel/checksTarget';
import { CommitPanel } from './CommitPanel';
import { ComparePanel } from './ComparePanel';
import { OpBanner } from './OpBanner';
import { PrPanel } from './PrPanel';
import type { PrFileDiffOpen } from './prPanel/PrChangesSection';
import type { PrRestoreFocus } from './repoWorkspace/usePrFileOverlay';
import { StatusPanel } from './StatusPanel';
import { shortOid } from './workspaceUtils';
import type {
  AiAnalysisMode,
  AiDiffTarget,
  BranchInfo,
  CommitVerification,
  GraphLayout,
  HeadInfo,
  ListView,
  PanelDensity,
  PrimaryCommitAction,
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
  /** P62c/P90: active right-pane tab (owned by RepoWorkspace). */
  rightPaneTab: 'work' | 'prs' | 'checks';
  /** P93: open one PR changed file's diff in the center overlay. */
  onOpenPrFileDiff(ctx: PrFileDiffOpen): void;
  /** P93 §6: collapse the center PR overlay (stable identity required). */
  onClosePrFileDiff(): void;
  /** P93: path of the PR file open in the center overlay (null = none). */
  prOverlayPath: string | null;
  /** P93 §6.1: dismissal-event focus restore for the PR changed-files list. */
  prRestoreFocusTo: PrRestoreFocus | null;
  onSelectRightPaneTab(tab: 'work' | 'prs' | 'checks'): void;
  /** P90: the branch resolved from the last sidebar reveal (or HEAD) → Checks tab. */
  checksTarget: ChecksTarget | null;
  /** P90: bumped on fetch/pull to force a silent Checks refetch. */
  checksRefreshSeq: number;
  /** P90 §4.4: push the checks target (defined only when it is the current
   *  branch); drives the "Push branch" affordance in the no-upstream empty state. */
  onPushChecksBranch?(): void;
  /** P90: reveal a commit oid in the graph (Checks tip-sha affordance). */
  onRevealCommit?(oid: string): void;
  /** P62c: current branch name — seeds the PR create form's compare field. */
  prDefaultHead: string | null;
  /** P78: base-branch hint for the PR create form (upstream target/main). */
  prDefaultBase: string | null;
  /** P78: branch suggestions for the PR create form's Base combobox. */
  prBaseOptions: ComboboxOption[];
  /** P78: branch suggestions for the PR create form's Compare combobox. */
  prCompareOptions: ComboboxOption[];
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
  /** P67 §4: right-panel density — rendered as `data-density` on the `<aside>`
   *  (D7: a prop, not `documentElement.dataset`, so the cascade stays scoped to
   *  this panel and the value is unit-testable by `render()`). */
  panelDensity: PanelDensity;
  /** P80 D1: which commit button is emphasized in the CommitBox footer. */
  primaryCommitAction: PrimaryCommitAction;
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
  aiRows: StatusPanelProps['aiRows'];
  aiAtCapacity: StatusPanelProps['aiAtCapacity'];
  /** P68f: ONE control, rendered by BOTH entry points — the conflicts-section header
   *  and the merge banner (OQ4). Same object ⇒ they can never disagree. */
  aiBulk?: StatusPanelProps['aiBulk'];
  aiPanelLoading: boolean;
  onStage: StatusPanelProps['onStage'];
  onUnstage: StatusPanelProps['onUnstage'];
  onDiscard: StatusPanelProps['onDiscard'];
  onDiscardForce: StatusPanelProps['onDiscardForce'];
  onToggleDiff: StatusPanelProps['onToggleDiff'];
  onResolveConflict: StatusPanelProps['onResolveConflict'];
  onToggleConflictView: StatusPanelProps['onToggleConflictView'];
  onAiResolve: StatusPanelProps['onAiResolve'];
  onAiReview: StatusPanelProps['onAiReview'];
  onAiReveal?: StatusPanelProps['onAiReveal'];
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
  /** P80: open Settings → Accounts (the PR panel's "Manage accounts…"). */
  onOpenAccountSettings?: () => void;
  /** P58c: effective signing config (drives the CommitBox sign toggle + hint). */
  signingStatus: SigningStatus | null;
  /** P58c: the selected commit's signature verdict (CommitPanel line); null when
   *  unverified / disabled / unsigned. */
  commitSignature: CommitVerification | null;
  /** P87b View C §2.2: the active commit-family run's phase readout, forwarded to
   *  CommitBox so the granular phase is visible beside the commit button. */
  commitPhase?: CommitBoxProps['commitPhase'];
  /** P87b §5-3: expand the git activity dock + reveal the active run. */
  onShowGitActivity?: CommitBoxProps['onShowGitActivity'];
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
  panelDensity,
  primaryCommitAction,
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
  aiRows,
  aiAtCapacity,
  aiBulk,
  aiPanelLoading,
  onStage,
  onUnstage,
  onDiscard,
  onDiscardForce,
  onToggleDiff,
  onResolveConflict,
  onToggleConflictView,
  onAiResolve,
  onAiReview,
  onAiReveal,
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
  onOpenAccountSettings,
  signingStatus,
  commitSignature,
  commitPhase,
  onShowGitActivity,
  repoId,
  rightPaneTab,
  onOpenPrFileDiff,
  onClosePrFileDiff,
  prOverlayPath,
  prRestoreFocusTo,
  onSelectRightPaneTab,
  prDefaultHead,
  prDefaultBase,
  prBaseOptions,
  prCompareOptions,
  prNav,
  checksTarget,
  checksRefreshSeq,
  onPushChecksBranch,
  onRevealCommit,
}: WorkspaceRightPanelProps) {
  // Audit §2.2: `selectedIndex` can point PAST the end of `graph.nodes` — a
  // streaming refetch publishes its first partial batch BEFORE the progressive
  // selection remap runs, and a rebased/GC'd commit never comes back at all.
  // `graph.nodes[i]` is then `undefined`, and CommitPanel (non-optional `node`)
  // would deref it → TypeError → the ErrorBoundary tears down the workspace.
  // Derive the node ONCE and gate BOTH uses on it; until the row arrives (or the
  // stream ends and RepoWorkspace clears the selection) we fall back to the
  // status panel, exactly like the commit-diff effect already skips.
  const selectedNode =
    selectedIndex !== null && graph !== null ? (graph.nodes[selectedIndex] ?? null) : null;

  return (
    <aside className="right-panel" data-density={panelDensity} style={{ width: rightPanelWidth }}>
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
        <button
          type="button"
          role="tab"
          aria-selected={rightPaneTab === 'checks'}
          className={`right-pane-tab${rightPaneTab === 'checks' ? ' active' : ''}`}
          onClick={() => onSelectRightPaneTab('checks')}
        >
          Checks
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
        aiBulk={aiBulk}
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
      ) : selectedNode !== null ? (
        <CommitPanel
          node={selectedNode}
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
            const oid = selectedNode.id;
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
            aiRows={aiRows}
            aiAtCapacity={aiAtCapacity}
            aiBulk={aiBulk}
            onStage={onStage}
            onUnstage={onUnstage}
            onDiscard={onDiscard}
            onDiscardForce={onDiscardForce}
            onToggleDiff={onToggleDiff}
            onResolveConflict={onResolveConflict}
            onToggleConflictView={onToggleConflictView}
            onAiResolve={onAiResolve}
            onAiReview={onAiReview}
            onAiReveal={onAiReveal}
            onBlame={onBlame}
            onFileHistory={onFileHistory}
          />
          {/* P80 §2b: the former `.rp-actions` row is gone — amend, stash, sign,
              skip-hooks, compose and the context-scoped review all fold into
              CommitBox's `⋯` menu. Amend stays owned upstream (RepoWorkspace),
              threaded in as a prop; CommitBox reseeds its message via an internal
              effect (no remount), so the menu item keeps focus on toggle. */}
          <CommitBox
            key={opState.kind === 'merge' ? `merge:${opState.incoming}` : 'commit'}
            ref={commitBoxRef}
            stagedCount={status?.staged.length ?? 0}
            busy={mutating}
            mode={opState.kind === 'merge' && !amend ? 'merge' : 'commit'}
            initialMessage={opState.kind === 'merge' ? opState.message : undefined}
            conflictCount={conflicts.length}
            blocked={!amend && opState.kind !== 'none' && opState.kind !== 'merge'}
            amend={amend}
            amendMessage={amendMessage}
            canAmend={opState.kind === 'none' && head !== null && !head.unborn}
            onToggleAmend={onToggleAmend}
            showAmendPushWarning={
              amend &&
              headBranch !== null &&
              headBranch.upstream !== null &&
              headBranch.ahead === 0
            }
            primaryCommitAction={primaryCommitAction}
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
            onReviewStaged={() => runAnalyze({ kind: 'staged' }, 'review', 'Review staged changes')}
            onReviewWorktree={() =>
              runAnalyze({ kind: 'worktree' }, 'review', 'Review working tree')
            }
            aiAnalyzing={aiPanelLoading}
            onStash={onCreateStash}
            canStash={opState.kind === 'none' && head !== null && !head.unborn}
            hasTrackedChanges={
              (status?.staged.length ?? 0) > 0 || (status?.unstaged.length ?? 0) > 0
            }
            hasUntracked={(status?.untracked.length ?? 0) > 0}
            onOpenIdentitySettings={onOpenIdentitySettings}
            signingStatus={signingStatus}
            commitPhase={commitPhase}
            onShowGitActivity={onShowGitActivity}
          />
        </>
      )}
      </div>
      {rightPaneTab === 'prs' && (
        <PrPanel
          repoId={repoId}
          defaultHead={prDefaultHead}
          defaultBase={prDefaultBase}
          baseOptions={prBaseOptions}
          compareOptions={prCompareOptions}
          openToPr={prNav}
          aiEligible={aiEligible}
          onManageAccounts={onOpenAccountSettings}
          onOpenFileDiff={onOpenPrFileDiff}
          onClosePrFileDiff={onClosePrFileDiff}
          prOverlayPath={prOverlayPath}
          prRestoreFocusTo={prRestoreFocusTo}
        />
      )}
      {rightPaneTab === 'checks' && (
        <ChecksPanel
          repoId={repoId}
          target={checksTarget}
          refreshSeq={checksRefreshSeq}
          active={rightPaneTab === 'checks'}
          onRevealCommit={onRevealCommit}
          onPush={onPushChecksBranch}
          onManageAccounts={onOpenAccountSettings}
        />
      )}
    </aside>
  );
}
