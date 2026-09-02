import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { CommitBoxHandle } from './CommitBox';
import type { ContextMenuState } from './ContextMenu';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceDialogs } from './WorkspaceDialogs';
import { WorkspaceOverlays } from './WorkspaceOverlays';
import { WorkspaceGraphPane } from './WorkspaceGraphPane';
import { WorkspaceRightPanel } from './WorkspaceRightPanel';
import { isUsableRepo } from './workspaceUtils';
import { prBaseRefOptions, prCompareRefOptions } from './repoWorkspace/prRefOptions';
import { createWorkspaceMenus } from './workspaceMenus';
import type { DiffOverlayMeta } from './DiffOverlay';
import type { DiffScope } from './DiffFileTree';
import { PaneDivider } from './PaneDivider';
import { Sidebar } from './Sidebar';
import type { DiffSlot, WorkdirSection } from './StatusPanel';
import type { GraphCanvasHandle, WipSummary } from '../graph/GraphCanvas';
import { useChecksTab } from './repoWorkspace/useChecksTab';
import { useAiPanel } from './repoWorkspace/useAiPanel';
import { useReveal } from './repoWorkspace/useReveal';
import { RevealAnnouncer } from './RevealAnnouncer';
import { effectiveMetrics } from '../graph/metrics';
import type { GraphDisplayOptions } from '../graph/rightColumns';
import { createGraphStream } from '../graph/streamAssembler';
import { createGraphStreamApplier } from './repoWorkspace/graphStreamApply';
import { composerPreviewFileDiff } from './repoWorkspace/composerPreview';
import { useRailInput } from './repoWorkspace/railProps';
import { useReplayController } from './repoWorkspace/replayProps';
import { usePaletteCallbacks } from './repoWorkspace/paletteCallbacks';
import { useCoalescedRefresh, type RefreshOrigin } from './repoWorkspace/useCoalescedRefresh';
import { useRenderCount } from '../obs/react';
import { traced, GESTURES } from '../obs/gesture';
import type { TraceId } from '../obs/types';
import { type RefreshScope, slicesForScope } from './repoWorkspace/refreshScope';
import { useRepoChangeSubscription } from './repoWorkspace/useRepoChangeSubscription';
import { useJobStatus } from './repoWorkspace/useJobStatus';
import { useImageDiff } from './repoWorkspace/useImageDiff';
import { useSidebarCollections } from './repoWorkspace/useSidebarCollections';
import { useAskBonsai } from './repoWorkspace/useAskBonsai';
import { createContextMenuOpeners } from './repoWorkspace/contextMenuOpeners';
import { useCompareMode } from './repoWorkspace/useCompareMode';
import { useWorkspaceDialogState } from './repoWorkspace/useWorkspaceDialogState';
import { useDiffOverlayView } from './repoWorkspace/useDiffOverlayView';
import { useSigningStatus } from './repoWorkspace/useSigningStatus';
import type { IncrementalEdgeIndex } from '../graph/incrementalEdgeIndex';
import { ipc } from '../ipc';
import type {
  BlameLine,
  BranchesSnapshot,
  CommitDiff,
  ConflictEntry,
  FileDiff,
  FileHistoryEntry,
  FileStatus,
  GraphLayout,
  HeadInfo,
  PrNavRequest,
  ReflogEntry,
  RepoOpState,
  ResetMode,
  StatusEntry,
  StatusSnapshot,
  Unsubscribe,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

import { useRemoteOps } from './repoWorkspace/useRemoteOps';
import { useCommitActions } from './repoWorkspace/useCommitActions';
import { usePartialStaging } from './repoWorkspace/usePartialStaging';
import { isPrSlotKey } from './repoWorkspace/prSlotKey';
import { deriveOverlayMeta } from './repoWorkspace/overlayMeta';
import { usePrFileOverlay } from './repoWorkspace/usePrFileOverlay';
import { useHookGate } from './repoWorkspace/useHookGate';
import { useHookDisclosure } from './repoWorkspace/useHookDisclosure';
import { useBranchActions } from './repoWorkspace/useBranchActions';
import { useAiRuns } from './repoWorkspace/useAiRuns';
import { AiActivityPanel } from './AiActivityPanel';
import { useAiDock } from './repoWorkspace/useAiDock';
import { useGitDock } from './repoWorkspace/useGitDock';
import { GitActivityDock } from './GitActivityDock';
import { useBulkAiResolve } from './repoWorkspace/useBulkAiResolve';
import { useMergeActions } from './repoWorkspace/useMergeActions';
import { useStashActions } from './repoWorkspace/useStashActions';
import { useSubmoduleActions, type SubmoduleBusy } from './repoWorkspace/useSubmoduleActions';
import { useWorktreeActions } from './repoWorkspace/useWorktreeActions';
import { useTagRemoteActions } from './repoWorkspace/useTagRemoteActions';
import {
  TagSyncDialogs,
} from './dialogs/TagSyncDialogs';
import { useTagSync } from './repoWorkspace/useTagSync';
import { useRebaseActions } from './repoWorkspace/useRebaseActions';
import { useCherrypickRevertActions } from './repoWorkspace/useCherrypickRevertActions';
import { useBisectActions } from './repoWorkspace/useBisectActions';
import { clearReadOverlays, useReadOverlays } from './repoWorkspace/useReadOverlays';
import { useWorkspaceKeyboard } from './repoWorkspace/useWorkspaceKeyboard';
import { useCommitSearch } from './repoWorkspace/useCommitSearch';
import { useCommitVerification } from './repoWorkspace/useCommitVerification';
import { useForgeSignals } from './repoWorkspace/useForgeSignals';
import { useHistorySearch } from './repoWorkspace/useHistorySearch';
import { useCommitComposer } from './repoWorkspace/useCommitComposer';
import { usePalette } from './repoWorkspace/usePalette';
import { useGraphFilterRefetch } from '../hooks/useGraphFilter';
import { useGraphFilterWiring } from './repoWorkspace/useGraphFilterWiring';
import { useExternalTools } from './repoWorkspace/useExternalTools';
import { bisectSummariesOf } from './repoWorkspace/bisectSummaries';
import {
  graphFilterPaletteEntries,
  refFilterMenuItems,
} from './workspaceMenusFilter';
import { RefFilterMarkerContext } from './sidebar/refFilterMarkerContext';
import { buildPaletteActions, type PaletteAction } from './paletteActions';
import type { ComboboxOption } from './Combobox';
import { searchScopeOptionsOf } from './repoWorkspace/searchHelpers';
import { branchStatsOf, diffBrowserViewOf, graphDisplayOf, prDefaultBaseOf } from './repoWorkspace/displayModels';

export type { RepoWorkspaceProps } from './repoWorkspace/RepoWorkspaceProps';
import type { RepoWorkspaceProps } from './repoWorkspace/RepoWorkspaceProps';

/** P3e §5.1: the entire per-repo state cluster + handlers + render tree, one
 *  instance per open tab (keyed by repoId in App). Consumes toasts via
 *  ToastContext; receives only app-global prefs + pane callbacks as props. */
export function RepoWorkspace({
  repoId,
  active,
  listView,
  panelDensity,
  primaryCommitAction,
  themeVersion,
  paneWidths,
  globalModalOpen,
  graph: graphPrefs,
  metricsVersion,
  graphStyle,
  graphSeason,
  graphFirstParent,
  graphFoldLinear,
  graphMinimapAlwaysShow,
  graphColorMode,
  graphRefFilter,
  onGraphFilterChange,
  aiEnabled,
  aiConflictAutonomy,
  aiConsented,
  aiAvailability,
  aiDockHeight,
  aiDockCollapsed,
  aiStreamLog,
  onAiDockChange,
  onSidebarResize,
  onRightPanelResize,
  onPaneResizeEnd,
  onOpenRepoPath,
  onOpenIdentitySettings,
  onOpenAccountSettings,
  appCommands,
}: RepoWorkspaceProps) {
  const pushToast = usePushToast();
  // P91 §9.2 surface 1 — container render churn (each mode). Above every early
  // return so hook order is unconditional (§9.5).
  useRenderCount('RepoWorkspace', { repoId, active, panelDensity, themeVersion, globalModalOpen });
  const repoPath = repoId; // repoId == canonical workdir path (§2)

  // P13 §8.2: AI conflict-resolution is offered only when enabled, consented,
  // and the CLI is actually installed. The backend re-checks enabled+consented.
  const aiEligible = aiEnabled && aiConsented && aiAvailability?.installed === true;

  // P11d §4.1: METRICS overlaid with the user's graph knobs; memoized so the
  // canvas metricsRef only churns when a knob actually changes.
  const metrics = useMemo(() => effectiveMetrics(graphPrefs), [graphPrefs]);

  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [statusError, setStatusError] = useState<{ id: number; message: string } | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  // P62c: right-pane tab — the existing working/compare/commit tri-state
  // ('work') vs the pull-request panel ('prs'). PrPanel mounts only under 'prs'.
  const [rightPaneTab, setRightPaneTab] = useState<'work' | 'prs' | 'checks'>('work');

  const [mutating, setMutating] = useState(false);
  // P11e §5: latest `mutating` read by the auto-fetch interval callback WITHOUT
  // resetting the timer on every mutation (it depends only on the settings).
  const mutatingRef = useRef(mutating);
  mutatingRef.current = mutating;

  const [branches, setBranches] = useState<BranchesSnapshot | null>(null);
  const [branchesError, setBranchesError] = useState<string | null>(null);
  const [branchesLoading, setBranchesLoading] = useState(false);
  const checksTab = useChecksTab(branches, branches?.local.find((b) => b.isHead)?.name ?? null); // P90

  // P51c: local-branch ahead/behind, keyed by branch name, for the graph's
  // ahead/behind chip. Only branches WITH an upstream (non-null counts) are
  // included; the chip render gates on divergence (>0) + the showAheadBehind
  // toggle. Memoized on `branches` so the canvas display object is stable
  // between refreshes.
  const branchStats = useMemo(() => branchStatsOf(branches), [branches]);

  // The four secondary sidebar collections + their last-wins fetch/clear pairs
  // (extracted to useSidebarCollections.ts).
  const {
    stashes,
    submodules,
    worktrees,
    remotes,
    refetchStashes,
    clearStashes,
    refetchSubmodules,
    clearSubmodules,
    refetchWorktrees,
    clearWorktrees,
    refetchRemotes,
    clearRemotes,
  } = useSidebarCollections(repoId);
  const [submoduleBusy, setSubmoduleBusy] = useState<SubmoduleBusy | null>(null);
  const [remoteOp, setRemoteOp] = useState<'fetch' | 'pull' | 'push' | null>(null);

  const [opState, setOpState] = useState<RepoOpState>({ kind: 'none' });
  const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);
  // Tracks conflict count across renders so we auto-open the first conflicted
  // file exactly once per conflict episode (0 -> >0 edge), not on every refetch.
  const prevConflictCountRef = useRef(0);
  // P15b/P15c/P28/P53a/P56b/P57c: the shared read-only AI-output panel + every
  // runner that fills it now live in useAiPanel (repoWorkspace/useAiPanel.ts).
  const {
    aiPanel,
    aiPanelOpenRef,
    runAnalyze,
    runHistoryAnswer,
    runSummarize,
    runDigest,
    runChangelog,
    onBlameExplain,
    closeAiPanel,
  } = useAiPanel(repoId);
  // Every armed-dialog flag for this repo + the disjunction over them
  // (extracted to useWorkspaceDialogState.ts). Destructured under the
  // original names, so every handler / hook call site below is unchanged.
  const {
    abortConfirmOpen,
    setAbortConfirmOpen,
    pendingDeleteBranch,
    setPendingDeleteBranch,
    pendingRebase,
    setPendingRebase,
    pendingDeleteRemote,
    setPendingDeleteRemote,
    pendingDropStash,
    setPendingDropStash,
    pendingReservedStash,
    setPendingReservedStash,
    pendingReset,
    setPendingReset,
    pendingDiscard,
    setPendingDiscard,
    pendingDiscardForce,
    setPendingDiscardForce,
    pendingCommitPush,
    setPendingCommitPush,
    commitPushResolver,
    pendingForcePush,
    setPendingForcePush,
    pendingHunkDiscard,
    setPendingHunkDiscard,
    pendingLineDiscard,
    setPendingLineDiscard,
    pendingCreateBranch,
    setPendingCreateBranch,
    pendingRenameBranch,
    setPendingRenameBranch,
    pendingAddSubmodule,
    setPendingAddSubmodule,
    pendingDeinitSubmodule,
    setPendingDeinitSubmodule,
    pendingRemoveSubmodule,
    setPendingRemoveSubmodule,
    pendingForceSubmodule,
    setPendingForceSubmodule,
    pendingNonFfPull,
    setPendingNonFfPull,
    pendingUndo,
    setPendingUndo,
    pendingCherrypick,
    setPendingCherrypick,
    pendingCreateTag,
    setPendingCreateTag,
    pendingDeleteTag,
    setPendingDeleteTag,
    pendingDeleteRemoteTag,
    setPendingDeleteRemoteTag,
    pendingForceMoveTag,
    setPendingForceMoveTag,
    pendingAddRemote,
    setPendingAddRemote,
    pendingRenameRemote,
    setPendingRenameRemote,
    pendingEditUrl,
    setPendingEditUrl,
    pendingRemoveRemote,
    setPendingRemoveRemote,
    staleCleanupOpen,
    setStaleCleanupOpen,
    newWorktreeOpen,
    setNewWorktreeOpen,
    whatChangedOpen,
    setWhatChangedOpen,
    changelogOpen,
    setChangelogOpen,
    pendingWorktreeRemove,
    setPendingWorktreeRemove,
    pendingWorktreeLock,
    setPendingWorktreeLock,
    worktreeContextOpen,
    setWorktreeContextOpen,
    rebasePlan,
    setRebasePlan,
    rebasePlanError,
    setRebasePlanError,
    anyDialogArmed,
  } = useWorkspaceDialogState();
  const commitBoxRef = useRef<CommitBoxHandle>(null);
  // First-time per-repo git-hook execution disclosure — sits at the TOP of the
  // shared hook gate (before any hook could run), so all four hook-bearing ops
  // (commit/amend/merge-commit/push) disclose once with zero per-call-site change.
  const hookDisclosure = useHookDisclosure(repoId);
  // P59a: the shared hook gate — parks a commit/amend/merge behind the
  // HookOutputDialog when a git hook blocks it, with a "Commit anyway" retry.
  const hookGate = useHookGate(hookDisclosure.ensureHooksDisclosed);
  // P20: amend affordance. `amend` toggles the commit box into amend mode;
  // `amendMessage` holds HEAD's message fetched once on toggle-on (prefill).
  const [amend, setAmend] = useState(false);
  const [amendMessage, setAmendMessage] = useState<string | null>(null);
  // P39b: two-click bisect start. Holds the oid marked BAD (via the commit menu)
  // while the user picks an older known-GOOD commit; cleared on start / cancel.
  const [pendingBisectBad, setPendingBisectBad] = useState<string | null>(null);
  // P77: live tag-sync report + its ls-remote lifecycle (owned by useTagSync).
  // Best-effort — the tags list never blocks on it; a rejection degrades to
  // `unavailable` (no badges).
  const {
    report: tagSyncReport,
    state: tagSyncState,
    remote: tagSyncRemote,
    checkedAt: tagSyncCheckedAt,
    refetch: refetchTagSync,
    clear: clearTagSync,
  } = useTagSync(repoId, remotes);
  // P23d: blame + file-history center-pane overlays. Each holds its own
  // loading/error so the overlay can render skeletons then data. A req-id guards
  // against a stale async response overwriting a newer request or a closed view.
  const [blame, setBlame] = useState<{
    path: string;
    lines: BlameLine[];
    loading: boolean;
    error: string | null;
  } | null>(null);
  const [history, setHistory] = useState<{
    path: string;
    entries: FileHistoryEntry[];
    loading: boolean;
    error: string | null;
  } | null>(null);
  // P38: reflog viewer overlay — a sibling read overlay with its own req-id
  // stale-guard. Restore actions reuse the shared create-branch / reset dialogs.
  const [reflog, setReflog] = useState<{
    refName: string;
    entries: ReflogEntry[];
    loading: boolean;
    error: string | null;
  } | null>(null);
  const blameReqId = useRef(0);
  const historyReqId = useRef(0);
  const reflogReqId = useRef(0);
  const blameOpenRef = useRef(false);
  blameOpenRef.current = blame !== null;
  const historyOpenRef = useRef(false);
  historyOpenRef.current = history !== null;
  const reflogOpenRef = useRef(false);
  reflogOpenRef.current = reflog !== null;
  const reflogRef = useRef(reflog);
  reflogRef.current = reflog;
  // Set when a restore is armed from the reflog overlay, so the completion
  // effect knows to re-fetch the (now stale) reflog after refreshAll.
  const reflogRestoreRef = useRef(false);
  const [graph, setGraph] = useState<GraphLayout | null>(null);
  // P65b: the stream assembler's incremental edge index + total row count for the
  // active graph, threaded into GraphCanvas alongside `graph` (set together with
  // it per applied batch so they never disagree).
  const [graphEdgeIndex, setGraphEdgeIndex] = useState<IncrementalEdgeIndex | null>(null);
  const [graphTotal, setGraphTotal] = useState<number | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const graphRef = useRef<GraphCanvasHandle>(null);

  // P84: reveal-in-graph — flash descriptor + a11y announcement + reduced-motion
  // flag + oid/refName→row lookups now live in useReveal (repoWorkspace/useReveal.ts).
  const { revealFlash, revealMessage, reducedMotion, handleReveal } = useReveal({
    graph,
    setSelectedIndex,
    revealBranch: checksTab.revealBranch,
    pushToast,
  });
  const [commitDiff, setCommitDiff] = useState<CommitDiff | null>(null);
  const [commitDiffLoading, setCommitDiffLoading] = useState(false);
  const [commitDiffError, setCommitDiffError] = useState<string | null>(null);
  const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null);
  // How the center-pane diff overlay is DISPLAYED — File/Diff/Split mode, the
  // intraline "Highlight changes" flag and the PR-file context, each mirrored
  // into a ref for the stable refetch callbacks (extracted to
  // useDiffOverlayView.ts).
  const {
    prOverlayCtx,
    setPrOverlayCtx,
    prOverlayCtxRef,
    diffViewMode,
    setDiffViewMode,
    diffViewModeRef,
    intraline,
    setIntraline,
    intralineRef,
  } = useDiffOverlayView();
  // Bug fix: the "Changes" list view mode (tree vs flat), read through a ref by
  // handleStage so the auto-advance target is computed in the SAME order the UI
  // renders. Threaded via ref so toggling never re-creates the stage handler.
  const listViewRef = useRef(listView);
  listViewRef.current = listView;
  // P5 §5.2: graph right-click context menu (position + prebuilt items).
  const [menu, setMenu] = useState<ContextMenuState | null>(null);

  // P11g-rev §4.1: ONE lifted scope drives BOTH the right-pane DiffFileTree
  // highlight AND the DiffBrowser's visible cards. Reset to root whenever the
  // active source (compare target / selected commit) changes.
  const [scope, setScope] = useState<DiffScope>({ kind: 'root' });
  // Commit mode ONLY: explicit-open flag (compare mode auto-opens, needs no flag).
  const [commitBrowserOpen, setCommitBrowserOpen] = useState(false);
  const commitBrowserOpenRef = useRef(commitBrowserOpen);
  commitBrowserOpenRef.current = commitBrowserOpen;

  const statusReqId = useRef(0);
  const graphReqId = useRef(0);
  const branchesReqId = useRef(0);
  const commitDiffReqId = useRef(0);
  const fileDiffReqId = useRef(0);
  const opStateReqId = useRef(0);
  const diffSlotRef = useRef<DiffSlot | null>(null);
  diffSlotRef.current = diffSlot;
  // Latest status snapshot, read by handleStage AFTER `await refetchStatus()` to
  // confirm the auto-advance target still exists (the closure `status` is the
  // pre-stage value; P46 WS3).
  const statusRef = useRef(status);
  statusRef.current = status;
  const statusErrorId = useRef(0);
  // Latest selection/graph read by refetchGraph without widening its deps (would
  // churn identity + re-subscribe the repo-changed / window-focus effects).
  const selectedIndexRef = useRef(selectedIndex);
  selectedIndexRef.current = selectedIndex;
  const graphDataRef = useRef(graph);
  graphDataRef.current = graph;

  // Spec-003/004: graph-declutter controller + fold state + meta truth flags.
  const { graphFilter, fold, graphFilterRef, setGraphFilterFlags, stale: graphFilterStale } =
    useGraphFilterWiring({ graphFirstParent, graphFoldLinear, graphRefFilter, onGraphFilterChange,
      branches, repoId, graphTotalRows: graph?.nodes.length ?? 0, selectedIndexRef, selectedIndex });

  // P58c: per-oid signature verify cache, keyed on the graph's visible range;
  // gated on the showSignatureBadge pref (off ⇒ empty map, NO verify requests).
  const verification = useCommitVerification({
    repoId,
    graphDataRef,
    enabled: graphPrefs.showSignatureBadge,
    pushToast,
  });

  // P63: per-branch forge-signal cache (PR + CI badges). Gated on the two prefs
  // AND !compact (badges are compact-suppressed); failures are silent. Feeds the
  // graphDisplay maps below; a completed fetch produces new map identities → a
  // new display identity → the canvas repaints and the badges fill in.
  const forgeSignals = useForgeSignals({
    repoId,
    graphDataRef,
    showPrBadge: graphPrefs.showPrBadge,
    showCiStatus: graphPrefs.showCiStatus,
    compact: graphPrefs.compact,
  });

  // P51b/P51c/P63: per-row display toggles (SHA/author/date column + date basis)
  // + the ahead/behind chip data + the forge-signal maps, derived from
  // graphPrefs/branchStats/forgeSignals and threaded into GraphCanvas. The
  // compact rule is enforced HERE (AND-ed into the two forge toggles) so the
  // pure layer never sees `compact`.
  const graphDisplay = useMemo<GraphDisplayOptions>(
    () => graphDisplayOf(graphColorMode, graphPrefs, branchStats, forgeSignals.prByBranch, forgeSignals.ciBySha),
    [graphColorMode, graphPrefs, branchStats, forgeSignals.prByBranch, forgeSignals.ciBySha],
  );

  // P63: right-pane PR navigation request — a graph PR-badge click sets the
  // 'prs' tab and bumps `seq` so PrPanel opens (or re-opens) that PR's detail.
  const [prNav, setPrNav] = useState<PrNavRequest | null>(null);
  const onOpenPr = useCallback((n: number) => {
    setRightPaneTab('prs');
    setPrNav((prev) => ({ number: n, seq: (prev?.seq ?? 0) + 1 }));
  }, []);

  // P58c: the effective signing config + its per-repo read (extracted to
  // useSigningStatus.ts).
  const { signingStatus, refetchSigningStatus } = useSigningStatus(repoId);

  // Commit whose diff/panel is currently loaded — lets the selection effect skip
  // a reset+refetch when the selected OID is unchanged (tab switch / watcher tick
  // that only shifts the row index).
  const commitDiffKeyRef = useRef<string | null>(null);

  // P99: the branches snapshot is the SINGLE source for HEAD. `openRepo`'s
  // RepoInfo.head is no longer mirrored into local state — the backend derives
  // both from one shared `read_head_info`, so they cannot disagree, and the
  // snapshot is already available before the first refresh round.
  const head: HeadInfo | null = branches?.head ?? null;
  const opActive = opState.kind !== 'none';
  const canPullPush =
    head != null && !head.detached && !head.unborn && !opActive;

  const wip: WipSummary | null = useMemo(() => {
    if (status === null || head?.unborn === true) return null;
    const paths = new Set<string>();
    for (const s of [status.staged, status.unstaged, status.untracked, status.conflicted]) {
      for (const e of s) paths.add(e.path);
    }
    return paths.size > 0 ? { fileCount: paths.size } : null;
  }, [status, head]);

  // P53c: is there any working change to name a branch from? Gates the AI
  // "Suggest name" affordance in the branch-create dialog (clean tree => no
  // grounding => disabled, OQ6).
  const workingDirty =
    status !== null &&
    (status.staged.length > 0 || status.unstaged.length > 0 || status.untracked.length > 0);

  // P53c: container-bound branch-name suggestion (working-tree grounding). The
  // dialog owns no IPC; the actual branch is created by the confirmed create
  // path — naming WRITES NOTHING.
  const suggestBranchName = useCallback(
    () => ipc.aiSuggestBranchName(repoId, { kind: 'working' }),
    [repoId],
  );

  const overlayMeta: DiffOverlayMeta | null = useMemo(
    () => (diffSlot === null ? null : deriveOverlayMeta(diffSlot.key, status, prOverlayCtx)),
    [diffSlot, status, prOverlayCtx],
  );

  // Latest overlay meta read by the partial-staging handlers + the view-mode
  // toggle without widening their (stable) callback deps.
  // P93: the ctx only while its slot is open (active-row marker + `PR #n` chip).
  const prOverlaySlot = diffSlot !== null && isPrSlotKey(diffSlot.key) ? prOverlayCtx : null;
  const overlayMetaRef = useRef(overlayMeta);
  overlayMetaRef.current = overlayMeta;

  // P61b: the image-diff fetch for an image overlay slot (extracted to
  // useImageDiff.ts). Non-image or conflict/proposal slots clear the state.
  const { imageDiff, imageDiffLoading, imageDiffError } = useImageDiff({
    repoId,
    overlayMeta,
    status,
  });

  // P17c: which granular action the open overlay offers, or null (read-only).
  // Workdir kinds only; renamed/binary/tooLarge/no-diff fall back to whole-file
  // staging (the chevron +/− action), so no gutter/hunk/range controls render.
  const stageable = useMemo<null | 'stage' | 'unstage'>(() => {
    if (overlayMeta === null || diffSlot === null) return null;
    let base: 'stage' | 'unstage';
    if (overlayMeta.kind === 'unstaged' || overlayMeta.kind === 'untracked') base = 'stage';
    else if (overlayMeta.kind === 'staged') base = 'unstage';
    else return null;
    const d = diffSlot.diff;
    if (d === null || d.binary || d.tooLarge || d.status === 'renamed') return null;
    return base;
  }, [overlayMeta, diffSlot]);
  const stageableRef = useRef(stageable);
  stageableRef.current = stageable;

  const reportStatusError = useCallback((message: string) => {
    setStatusError({ id: ++statusErrorId.current, message });
  }, []);

  const fetchDiffSlot = useCallback(async (key: string, fetcher: () => Promise<FileDiff>) => {
    const id = ++fileDiffReqId.current;
    const prev = diffSlotRef.current;
    const stale = prev !== null && prev.key === key ? prev.diff : null;
    setDiffSlot({ key, state: 'loading', diff: stale, error: null });
    try {
      const diff = await fetcher();
      if (id !== fileDiffReqId.current) return;
      setDiffSlot({ key, state: 'ready', diff, error: null });
    } catch (e) {
      if (id !== fileDiffReqId.current) return;
      setDiffSlot({ key, state: 'error', diff: null, error: errorMessage(e) });
    }
  }, []);

  const collapseDiffSlot = useCallback(() => {
    fileDiffReqId.current += 1;
    setDiffSlot(null);
    setPrOverlayCtx(null); // P93: only meaningful while a `pr:` slot is open
  }, [setPrOverlayCtx]);

  // P5 §5.3: Compare mode (HEAD → a right-clicked commit) — target, data,
  // loading/error, teardown, refetch and the enter handler (extracted to
  // useCompareMode.ts).
  const {
    compare,
    compareData,
    compareLoading,
    compareError,
    compareRef,
    clearCompare,
    refetchCompare,
    handleCompareWithHead,
  } = useCompareMode({
    repoId,
    pushToast,
    collapseDiffSlot,
    diffSlotRef,
    setDiffSlot,
    fileDiffReqId,
    setMenu,
  });

  const fetchConflictSlot = useCallback(
    async (path: string) => {
      const key = `conflict:${path}`;
      const id = ++fileDiffReqId.current;
      const prev = diffSlotRef.current;
      const stale = prev !== null && prev.key === key ? (prev.conflict ?? null) : null;
      setDiffSlot({ key, state: 'loading', diff: null, conflict: stale, error: null });
      try {
        const file = await ipc.getConflict(repoId, path);
        if (id !== fileDiffReqId.current) return;
        setDiffSlot({ key, state: 'ready', diff: null, conflict: file, error: null });
      } catch (e) {
        if (id !== fileDiffReqId.current) return;
        setDiffSlot({ key, state: 'error', diff: null, conflict: null, error: errorMessage(e) });
      }
    },
    [repoId],
  );

  const refetchOpState = useCallback(async () => {
    const id = ++opStateReqId.current;
    try {
      const op = await ipc.getOpState(repoId);
      const list =
        op.kind === 'merge' ||
        op.kind === 'rebase' ||
        op.kind === 'cherryPick' ||
        op.kind === 'revert'
          ? await ipc.listConflicts(repoId)
          : [];
      if (id !== opStateReqId.current) return;
      setOpState(op);
      setConflicts(list);
      const slot = diffSlotRef.current;
      if (slot !== null && slot.key.startsWith('conflict:')) {
        const path = slot.key.slice('conflict:'.length);
        if (list.some((c) => c.path === path)) {
          void fetchConflictSlot(path);
        } else {
          collapseDiffSlot();
        }
      } else if (slot !== null && slot.key.startsWith('ai-proposal:')) {
        // P13 §8.3: keep the proposal overlay as long as the path is still
        // conflicted (do NOT re-fetch — that would replace the proposed body
        // with the marker view). Once resolved (Accept), the path leaves the
        // conflict list and the slot collapses (same post-resolve rule).
        const path = slot.key.slice('ai-proposal:'.length);
        if (!list.some((c) => c.path === path)) collapseDiffSlot();
      }
    } catch (e) {
      if (id !== opStateReqId.current) return;
      pushToast('error', `Could not read operation state: ${errorMessage(e)}`);
    }
  }, [repoId, fetchConflictSlot, collapseDiffSlot, pushToast]);

  // Auto-open the first conflicted file once per conflict episode. Fires only on
  // the 0 -> >0 transition (a fresh conflict from merge/rebase/cherry-pick/revert/
  // stash-pop), and only if no conflict/proposal slot is already open — so it
  // never re-opens a slot the user just closed (count stays >0, no new edge).
  useEffect(() => {
    const prev = prevConflictCountRef.current;
    if (prev === 0 && conflicts.length > 0) {
      const slot = diffSlotRef.current;
      const alreadyOpen =
        slot !== null &&
        (slot.key.startsWith('conflict:') || slot.key.startsWith('ai-proposal:'));
      if (!alreadyOpen) {
        void fetchConflictSlot(conflicts[0].path);
      }
    }
    prevConflictCountRef.current = conflicts.length;
  }, [conflicts, fetchConflictSlot]);

  const clearOpState = useCallback(() => {
    opStateReqId.current += 1;
    setOpState({ kind: 'none' });
    setConflicts([]);
  }, []);

  const refetchStatus = useCallback(async () => {
    const id = ++statusReqId.current;
    setStatusLoading(true);
    try {
      const snapshot = await ipc.getStatus(repoId);
      if (id !== statusReqId.current) return;
      setStatus(snapshot);
      setStatusError(null);
      const slot = diffSlotRef.current;
      if (
        slot !== null &&
        !slot.key.startsWith('commit:') &&
        !slot.key.startsWith('conflict:') &&
        !slot.key.startsWith('ai-proposal:') &&
        // P93: a `pr:` slot is a forge diff — never refetch it as a workdir one.
        !isPrSlotKey(slot.key)
      ) {
        const sep = slot.key.indexOf(':');
        const section = slot.key.slice(0, sep) as WorkdirSection;
        const path = slot.key.slice(sep + 1);
        const entry = snapshot[section].find((en) => en.path === path);
        if (entry === undefined) {
          collapseDiffSlot();
        } else {
          void fetchDiffSlot(slot.key, () =>
            ipc.getWorkdirFileDiff(
              repoId,
              entry.path,
              entry.origPath,
              section === 'staged',
              diffViewModeRef.current === 'file',
              intralineRef.current,
            ),
          );
        }
      }
    } catch (e) {
      if (id !== statusReqId.current) return;
      reportStatusError(errorMessage(e));
    } finally {
      if (id === statusReqId.current) setStatusLoading(false);
    }
  }, [repoId, fetchDiffSlot, collapseDiffSlot, reportStatusError, diffViewModeRef, intralineRef]);

  const clearStatus = useCallback(() => {
    statusReqId.current += 1;
    setStatus(null);
    setStatusError(null);
    setStatusLoading(false);
    collapseDiffSlot();
  }, [collapseDiffSlot]);

  // Spec-005: bumped once per stream `done` — the rail's bucket-rebuild key.
  const [railGeneration, setRailGeneration] = useState(0);
  const refetchGraph = useCallback(async () => {
    // The `graphReqId` generation is the cancellation crux (P65 §6): it now gates
    // chunk APPLICATION — chunks from a superseded stream (repo switch / new
    // refetch) are dropped before they ever touch the assembler.
    const id = ++graphReqId.current;
    // Preserve selection across refetches (activation self-heal, focus rescan,
    // watcher ticks) by commit OID: capture it BEFORE the stream, remap during.
    const prevSelectedId =
      selectedIndexRef.current != null
        ? (graphDataRef.current?.nodes[selectedIndexRef.current]?.id ?? null)
        : null;
    const stream = createGraphStream();
    // Chunk application + audit-§3.8 throw containment live in
    // graphStreamApply.ts: the assembler throws on a non-contiguous batch (a
    // correct invariant guard), but this callback runs inside Channel.onmessage
    // where an escaped throw never reaches the catch below. The first throw
    // surfaces via setGraphError and poisons the stream (later chunks drop).
    const applier = createGraphStreamApplier(
      stream,
      prevSelectedId,
      { setGraph, setGraphEdgeIndex, setGraphTotal, setSelectedIndex, setFilterFlags: setGraphFilterFlags, setFoldSpans: fold.setSpans, onDone: () => setRailGeneration((g) => g + 1) },
      (e) => {
        if (id === graphReqId.current) setGraphError(errorMessage(e));
      },
    );
    setGraphLoading(true);
    try {
      await ipc.streamGraph(repoId, graphFilterRef.current, (chunk) => {
        if (id !== graphReqId.current) return; // stale / superseded stream
        applier.handle(chunk);
      });
      if (id !== graphReqId.current) return;
      // A poisoned stream already surfaced its error — don't clear it, and
      // don't resolve the selection against the partial layout.
      if (applier.poisoned) return;
      setGraphError(null);
      // Post-stream selection resolution: a prior selection that never reappeared
      // is gone -> clear; no prior selection -> null.
      if (prevSelectedId === null || !applier.remapped) setSelectedIndex(null);
    } catch (e) {
      if (id !== graphReqId.current) return;
      setGraphError(errorMessage(e));
      // Poison the rejected stream like a superseded one: a timed-out backend
      // worker is detached, not stopped — late chunks must not pass the gate.
      graphReqId.current++;
      setGraphLoading(false);
    } finally {
      if (id === graphReqId.current) setGraphLoading(false);
    }
  }, [repoId, graphFilterRef, setGraphFilterFlags, fold.setSpans]); // all hook-stable

  const refetchBranches = useCallback(async () => {
    const id = ++branchesReqId.current;
    setBranchesLoading(true);
    try {
      const snapshot = await ipc.listBranches(repoId);
      if (id !== branchesReqId.current) return;
      setBranches(snapshot);
      setBranchesError(null);
    } catch (e) {
      if (id !== branchesReqId.current) return;
      setBranchesError(errorMessage(e));
    } finally {
      if (id === branchesReqId.current) setBranchesLoading(false);
    }
  }, [repoId]);

  const clearBranches = useCallback(() => {
    branchesReqId.current += 1;
    setBranches(null);
    setBranchesError(null);
    setBranchesLoading(false);
  }, []);

  const clearGraph = useCallback(() => {
    graphReqId.current += 1;
    setGraph(null);
    setGraphEdgeIndex(null);
    setGraphTotal(null);
    setGraphError(null);
    setGraphLoading(false);
    setSelectedIndex(null);
  }, []);

  // P81: origin-forced tag-sync flag for the NEXT round. Set synchronously by
  // `refresh(origin)` before the coalescer starts a round; read+cleared at the
  // start of `runRefreshRound`. `manual`/`activation`/`focus`/`mutation` origins
  // force an ls-remote tag-drift re-check; the `watcher` (external repo-changed)
  // origin leaves it false → NON-forced tagSync (P77: no-op until Tags opened),
  // so an external FS event never forces a network ls-remote (Flag 2).
  const pendingTagForceRef = useRef(false);

  /** Composite post-op refresh (P1 §4.6): the canonical refresh round (P81 §2);
   *  all origins funnel through the coalescer to it. P86a: SCOPED — only the
   *  slices `scope` implies run, so a ref-only mutation never pays the O(worktree)
   *  `get_status` scan and non-`full` scopes skip `openRepo` (they never move
   *  HEAD). `full` still re-openRepos — for the usability check (clears
   *  everything if the repo went unusable) + the watcher self-heal; P99: the
   *  header HEAD comes from the branches snapshot, not from here. Never throws —
   *  failures surface as a sticky error toast. */
  const runRefreshRound = useCallback(
    async (scope: RefreshScope): Promise<void> => {
      const forceTagSync = pendingTagForceRef.current;
      pendingTagForceRef.current = false;
      const slices = slicesForScope(scope);
      try {
        if (slices.openRepo) {
          const { info } = await ipc.openRepo(repoPath);
          if (!isUsableRepo(info)) {
            clearStatus();
            clearGraph();
            clearBranches();
            clearStashes();
            clearSubmodules();
            clearWorktrees();
            clearRemotes();
            clearTagSync();
            clearOpState();
            clearCompare();
            // P23d + P38: drop any blame/history/reflog overlay + invalidate
            // in-flight fetches so a stale overlay can't linger over the
            // now-empty pane. All three siblings go together — an open reflog
            // is just as stale as an open blame once the repo is unusable.
            clearReadOverlays({
              setBlame,
              setHistory,
              setReflog,
              blameReqId,
              historyReqId,
              reflogReqId,
            });
            return;
          }
        }
        const tasks: Promise<void>[] = [];
        if (slices.status) tasks.push(refetchStatus());
        if (slices.graph) tasks.push(refetchGraph());
        if (slices.branches) tasks.push(refetchBranches());
        if (slices.stashes) tasks.push(refetchStashes());
        if (slices.submodules) tasks.push(refetchSubmodules());
        if (slices.worktrees) tasks.push(refetchWorktrees());
        if (slices.remotes) tasks.push(refetchRemotes());
        if (slices.opState) tasks.push(refetchOpState());
        if (slices.compare) tasks.push(refetchCompare());
        if (slices.tagSync) {
          // P77: re-check tag drift (no-op until the Tags section has been opened
          // once this session). P81 Flag 2 / P86a: only `full` from a forcing
          // origin (manual/focus/mutation) pays the ls-remote; `remoteMeta` and
          // watcher-driven rounds run non-forced.
          const forced = slices.tagSyncForcable && forceTagSync;
          tasks.push(refetchTagSync(forced ? { force: true } : undefined));
        }
        await Promise.all(tasks);
      } catch (e) {
        pushToast('error', `Refresh failed: ${errorMessage(e)}`);
      }
    },
    [
      repoPath,
      refetchStatus,
      refetchGraph,
      refetchBranches,
      refetchStashes,
      refetchSubmodules,
      refetchWorktrees,
      refetchRemotes,
      refetchOpState,
      refetchCompare,
      refetchTagSync,
      clearStatus,
      clearGraph,
      clearBranches,
      clearStashes,
      clearSubmodules,
      clearWorktrees,
      clearRemotes,
      clearTagSync,
      clearOpState,
      clearCompare,
      pushToast,
    ],
  );

  // P81 §2/§3: coalesce every refresh entry point onto the one canonical round.
  // The coalescer collapses overlapping requests (leading + at-most-one-trailing)
  // and the shared echo registry drops the self-caused watcher echo within TTL.
  const { refresh: coalescedRefresh } = useCoalescedRefresh(repoId, runRefreshRound);
  const refresh = useCallback(
    (origin: RefreshOrigin, scope: RefreshScope, trace?: TraceId): Promise<void> => {
      // Forced tag-drift re-check for user-initiated origins (mutation writes,
      // manual refresh, activation self-heal, focus rescan). Set BEFORE enqueuing
      // so the round about to start reads it (P81 Flag 2). `watcher` (raw fs echo)
      // and `external` (backend-confirmed change — the backend already ran the
      // tag-sync) do NOT force a fresh ls-remote.
      if (origin !== 'watcher' && origin !== 'external') pendingTagForceRef.current = true;
      return coalescedRefresh(origin, scope, trace);
    },
    [coalescedRefresh],
  );
  // Name preserved: the mutation call sites + hook deps stay untouched (they now
  // pass an explicit scope; the default keeps unscoped callers on `full`).
  // A mutation is a local write → arms echo suppression + (for `full`) forces tagSync.
  // P91 §2.5: `trace` is the arming gesture's TraceId, captured at the action's
  // synchronous entry and threaded by value (see obs/gesture.ts).
  const refreshAll = useCallback(
    (scope: RefreshScope = 'full', trace?: TraceId): Promise<void> =>
      refresh('mutation', scope, trace),
    [refresh],
  );

  // Initial load on mount: fetch state for repoId (the repo is already opened by
  // App — do NOT openRepo again here, §5.1). Runs for active AND background tabs.
  const mountedRef = useRef(false);
  useEffect(() => {
    if (mountedRef.current) return;
    mountedRef.current = true;
    void refetchStatus();
    void refetchGraph();
    void refetchBranches();
    void refetchStashes();
    void refetchSubmodules();
    void refetchWorktrees();
    void refetchRemotes();
    void refetchOpState();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Activation self-heal (§7): on every flip TO active AFTER mount, refreshAll —
  // catches events missed while the tab was display:none. Skips the mount run
  // (the initial load above already covers first paint).
  const refreshRef = useRef(refresh);
  refreshRef.current = refresh;
  const activeFlipRef = useRef(false);
  useEffect(() => {
    if (!activeFlipRef.current) {
      activeFlipRef.current = true;
      return;
    }
    // P81: activation ALWAYS refreshes (never echo-gated) — catches events
    // missed while the tab was display:none. Full scope for the self-heal.
    if (active) void refreshRef.current('activation', 'full');
  }, [active]);

  // Selection -> commit diff (M4 §4.4). Every selection change resets the shared
  // expansion slot (its keys belong to the previous mode/commit).
  useEffect(() => {
    if (selectedIndex !== null && graph !== null) {
      const node = graph.nodes[selectedIndex];
      // Mid-stream partial layout: the selected commit's row is not in the
      // streamed window yet. Skip — leave the current panel untouched until the
      // refetch remap re-points selectedIndex and this effect re-runs.
      if (!node) return;
      const oid = node.id;
      const key = `${repoId}:${oid}`;
      // Same commit as already loaded (a refetch only shifted its row, or the
      // graph object churned) -> keep the panel + open file diff untouched.
      if (commitDiffKeyRef.current === key) return;
      commitDiffKeyRef.current = key;
      fileDiffReqId.current += 1;
      setDiffSlot(null);
      const id = ++commitDiffReqId.current;
      setCommitDiff(null);
      setCommitDiffLoading(true);
      setCommitDiffError(null);
      ipc.getCommitDiff(repoId, oid).then(
        (cd) => {
          if (id !== commitDiffReqId.current) return;
          setCommitDiff(cd);
          setCommitDiffLoading(false);
        },
        (e: unknown) => {
          if (id !== commitDiffReqId.current) return;
          setCommitDiffError(errorMessage(e));
          setCommitDiffLoading(false);
        },
      );
    } else {
      commitDiffKeyRef.current = null;
      commitDiffReqId.current += 1;
      setCommitDiff(null);
      setCommitDiffLoading(false);
      setCommitDiffError(null);
    }
  }, [selectedIndex, graph, repoId]);

  // P11g-rev §4.2: reset scope + close the commit browser whenever the active
  // source changes (new compare target, or a DIFFERENT commit selected). Keyed
  // on the selected commit's OID — not the row index — so a background refetch
  // that merely shifts rows never closes an open browser (same-OID preservation,
  // mirroring the commit-diff effect above). Compare auto-open then renders at
  // root; commit mode returns to closed.
  const selectedOid =
    selectedIndex !== null && graph !== null ? (graph.nodes[selectedIndex]?.id ?? null) : null;
  useEffect(() => {
    setScope({ kind: 'root' });
    setCommitBrowserOpen(false);
  }, [compare?.oid, selectedOid]);

  // P86a: repo-changed + tag-auto-sync subscriptions (reason-aware refresh routing
  // + the CI-3 tag-count toast) live in their own hook so the container stays thin.
  useRepoChangeSubscription(repoId, refresh, pushToast);

  // Window-focus rescan: ACTIVE tab only (the visible tab is the one the user
  // just returned to; background tabs self-heal on activation, §7).
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    const subscribe = async () => {
      const off = await ipc.onWindowFocus(() => {
        // P81 §7: focus rescan ALWAYS refreshes (never echo-gated). Full self-heal.
        void refresh('focus', 'full');
        forgeSignals.refresh('focus'); // P63: TTL-guarded (not forced)
        checksTab.bumpRefresh(); // P90: refresh-on-focus for the Checks tab
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    };
    // Subscription loss = degraded live refresh only (manual refresh works).
    void subscribe().catch((e: unknown) => {
      console.error('window-focus subscription failed', e);
    });
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [active, refresh, forgeSignals.refresh, checksTab.bumpRefresh]);

  // P63: a new graph layout identity (post fetch/pull/branch-op) may carry new
  // branch tips → TTL-guarded forge-signal refresh so their badges appear. Fires
  // on mount too (graph null→value); runFetch bails while the layout is null.
  useEffect(() => {
    forgeSignals.refresh('graph');
  }, [graph, forgeSignals.refresh]);

  // P30 D11 / P30 §6: background-job status readout + its 30 s ticker
  // (extracted to useJobStatus.ts). Auto-fetch itself runs in the Rust scheduler.
  const { jobStatus, jobNow } = useJobStatus(repoId, pushToast);

  // Manual refresh (button + Ctrl+R/F5). P58c: also drop the signature-verify
  // cache (keyring / allowedSigners may have changed — OQ8) and re-read the
  // signing config so the commit-box toggle reflects a fresh commit.gpgsign.
  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await refresh('manual', 'full'); // P81: always runs (never echo-gated)
      verification.refresh();
      forgeSignals.refresh('manual', true); // P63: forced (bypass TTL)
      void refetchSigningStatus();
    } finally {
      setRefreshing(false);
    }
  }, [refreshing, refresh, verification.refresh, forgeSignals.refresh, refetchSigningStatus]);
  const headBranch = branches?.local.find((b) => b.isHead) ?? null;

  // P78: branch suggestions + base hint for the PR create form. Compare = local
  // branches; Base = local + remote-tracking branches. Base hint prefers the head
  // branch's upstream, then a local main/master, else empty.
  // P78/P100: PR ref-field options (short-oid hints) live in ./repoWorkspace/prRefOptions.
  const prCompareOptions = useMemo<ComboboxOption[]>(
    () => prCompareRefOptions(branches),
    [branches],
  );
  const prBaseOptions = useMemo<ComboboxOption[]>(() => prBaseRefOptions(branches), [branches]);
  const prDefaultBase = useMemo<string | null>(
    () => prDefaultBaseOf(headBranch, branches),
    [headBranch, branches],
  );

  // P58c: the selected commit's signature verdict for the CommitPanel line —
  // reuses the shared verify cache (single source; no extra IPC). null when
  // nothing is selected, not yet verified, or the badge is disabled.
  // (`selectedOid` is derived once above, by the scope-reset effect.)
  const commitSignature =
    selectedOid !== null ? (verification.detailsFor(selectedOid) ?? null) : null;

  // P63/P90: force forge-signal + Checks refetch after any remote op (fetch/pull + push).
  const bumpForgeAndChecks = useCallback(() => {
    forgeSignals.refresh('remote', true);
    checksTab.bumpRefresh();
  }, [forgeSignals.refresh, checksTab.bumpRefresh]);
  const { handleFetch, handlePull, pushCurrentBranch, handlePush, handleForcePush, doForcePush } =
    useRemoteOps({
      repoId,
      pushToast,
      setMutating,
      refreshAll,
      setRemoteOp,
      setPendingForcePush,
      setPendingNonFfPull,
      runWithHookGate: hookGate.runWithHookGate,
      onPushComplete: bumpForgeAndChecks,
    });

  const onFetch = useCallback(
    () => void traced('click', GESTURES.fetch, () => handleFetch().then(bumpForgeAndChecks))(),
    [handleFetch, bumpForgeAndChecks],
  );
  const onPull = useCallback(
    () => void traced('click', GESTURES.pull, () => handlePull().then(bumpForgeAndChecks))(),
    [handlePull, bumpForgeAndChecks],
  );

  const {
    handleStage,
    handleUnstage,
    handleCommit,
    handleCommitAndPush,
    handleConfirmCommitPush,
    handleCancelCommitPush,
    handleCommitAmend,
    handleToggleAmend,
    handleResetBranch,
    handleDiscard,
    requestDiscardForce,
    handleDiscardForce,
    handleGenerateCommitMessage,
  } = useCommitActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    reportStatusError,
    fetchDiffSlot,
    pushCurrentBranch,
    status,
    statusRef,
    diffSlotRef,
    diffViewModeRef,
    intralineRef,
    listViewRef,
    head,
    headBranch,
    setAmend,
    setAmendMessage,
    pendingCommitPush,
    setPendingCommitPush,
    commitPushResolver,
    setPendingDiscardForce,
    refreshVerification: verification.refresh,
    runWithHookGate: hookGate.runWithHookGate,
  });

  const {
    handleCreateBranch,
    handleCheckoutBranch,
    handleCheckoutCommit,
    handleCreateBranchHere,
    handleDeleteBranch,
    handleRenameBranch,
    handleCheckoutRemote,
    handleDeleteRemoteTracking,
  } = useBranchActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    branches,
    setBranchesError,
    setPendingCreateBranch,
    setPendingRenameBranch,
  });

  const {
    handleMergeBranch,
    handleResolveConflict,
    handleResolveConflictText,
    handleAiApplyResolution,
    openAiProposal,
    handleCommitMerge,
    handleAbortMerge,
  } = useMergeActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    setDiffSlot,
    fileDiffReqId,
    runWithHookGate: hookGate.runWithHookGate,
  });

  // P68d §C: the per-path AI run store — THE item-5 fix (rationale in the hook's header).
  const conflictPaths = useMemo(() => conflicts.map((c) => c.path), [conflicts]);
  const aiRuns = useAiRuns({
    repoId,
    pushToast,
    aiConflictAutonomy,
    aiEligible,
    applyResolution: handleAiApplyResolution, // P68 #7 / H1: GATED writer (not the manual save)
    refreshAll, // P68f: ONE refresh after a multi-file autoResolve stage, not N.
    openAiProposal,
    conflictPaths,
    // FOLD-IN 1: never steal the center pane from a user who navigated away while
    // the run worked (the rationale lives on `AiRunsDeps.diffSlotKey`).
    diffSlotKey: () => diffSlotRef.current?.key ?? null,
  });

  // P68f §6.4: "Resolve all with AI" — ONE run over every AI-eligible conflict, confirm-gated.
  const aiBulk = useBulkAiResolve({ conflicts, aiEligible, aiConflictAutonomy, aiRuns });

  const { handleCreateStash, handleApplyStash, handlePopStash, handleDropStash } = useStashActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    refetchStashes,
    setPendingReservedStash,
  });

  const {
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
    handleAddSubmodule,
    handleDeinitSubmodule,
    handleRemoveSubmodule,
  } = useSubmoduleActions({
    repoId,
    pushToast,
    setMutating,
    setSubmoduleBusy,
    refreshAll,
    refetchSubmodules,
    onSubmoduleDirtyRefused: (name, op) => setPendingForceSubmodule({ name, op }),
  });

  const { handleAddWorktree, handleLockWorktree, handleUnlockWorktree, handleRemoveWorktree } =
    useWorktreeActions({
      repoId,
      pushToast,
      setMutating,
      refetchWorktrees,
      setNewWorktreeOpen,
    });

  const {
    handleCreateTag,
    handleDeleteTag,
    handlePushTag,
    handleForceRefreshTag,
    handleFetchRemoteTag,
    handleDeleteRemoteTag,
    handleForceMoveRemoteTag,
    handleAddRemote,
    handleRemoveRemote,
    handleRenameRemote,
    handleSetRemoteUrl,
  } = useTagRemoteActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    refetchRemotes,
    refetchTagSync,
  });

  const {
    handleRebaseBranch,
    handleRebaseContinue,
    handleRebaseSkip,
    handleRebaseAbort,
    openRebasePlan,
    handleStartInteractiveRebase,
  } = useRebaseActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    graph,
    setRebasePlan,
    setRebasePlanError,
  });

  const {
    handleCherrypick,
    confirmCherrypick,
    handleRevert,
    handleCherrypickContinue,
    handleRevertContinue,
    handleCherrypickAbort,
    handleRevertAbort,
  } = useCherrypickRevertActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    setPendingCherrypick,
  });

  const { handleStartBisect, handleBisectMark, handleBisectSkip, handleBisectReset } =
    useBisectActions({
      repoId,
      pushToast,
      setMutating,
      refreshAll,
      setPendingBisectBad,
    });
  // P17c/P28/P45: partial staging + hunk/line discard + the two overlay refetch
  // toggles, all in one hook (see repoWorkspace/usePartialStaging.ts). The state
  // they drive stays here because the render body and `opActive` read it.
  const {
    handleSetViewMode,
    handleToggleIntraline,
    handleStageLines,
    handleStageHunk,
    handleDiscardHunk,
    handleConfirmHunkDiscard,
    handleDiscardLines,
    handleConfirmLineDiscard,
  } = usePartialStaging({
    repoId,
    setMutating,
    mutatingRef,
    overlayMetaRef,
    diffSlotRef,
    stageableRef,
    diffViewModeRef,
    intralineRef,
    prOverlayCtxRef,
    setDiffViewMode,
    setIntraline,
    setPendingHunkDiscard,
    setPendingLineDiscard,
    fetchDiffSlot,
    refetchStatus,
    reportStatusError,
  });

  // P93 §6.1: `handleDismissDiffOverlay` replaces `collapseDiffSlot` at the
  // overlay's dismissal affordances (× / Esc / error banner) — the only arm point.
  const {
    handleOpenPrFileDiff, handleClosePrFileDiff, handleDismissDiffOverlay, prRestoreFocusTo,
  } = usePrFileOverlay({
    repoId, diffSlotRef, diffViewModeRef, intralineRef, prOverlayCtxRef, setPrOverlayCtx,
    fetchDiffSlot, collapseDiffSlot,
  });

  // P56b: open the general "Release notes…" range picker (palette entry). Stable
  // so the palette-action useMemo doesn't rebuild each render.
  const openChangelog = useCallback(() => setChangelogOpen(true), [setChangelogOpen]);

  // P55c: the NL → safe-git-op pipeline (extracted to useAskBonsai.ts). Sits
  // HERE, before `aiDock`, so `openAskBonsai` can be passed BY REFERENCE.
  const {
    askOpen,
    askBusy,
    pendingProposedOp,
    opDispatching,
    runPlanOperation,
    openAskBonsai,
    cancelAskBonsai,
    confirmProposedOp,
    cancelProposedOp,
  } = useAskBonsai({ repoId, pushToast, refreshAll, setMutating });

  // P68e: all of the dock's container-side glue lives in the hook (§9). It sits HERE,
  // after `openChangelog`/`openAskBonsai`, so those two stable `useCallback`s can be
  // passed BY REFERENCE — inline-arrow thunks made `aiDock.paletteEntries` (and so the
  // palette's `actions` array) a fresh object every render, resetting its highlight.
  const aiDock = useAiDock({
    aiRuns,
    height: aiDockHeight,
    collapsed: aiDockCollapsed,
    onChange: onAiDockChange,
    density: panelDensity,
    streamLogEnabled: aiStreamLog,
    aiEligible,
    onAskBonsai: openAskBonsai,
    onChangelog: openChangelog,
  });

  // P87b: the git-activity session store (View C + View D) + its dock container.
  const gitDock = useGitDock({ density: panelDensity });

  // P60c: describe the last HEAD-moving op (READ-ONLY) and open the UndoDialog.
  // Confirming there reuses the shipped resetBranch (handleResetBranch) with the
  // plan's target + mode; the dialog gates on undoable / requiresCleanWorktree.
  const handleRequestUndo = useCallback(async () => {
    try {
      const plan = await ipc.describeLastUndo(repoId);
      setPendingUndo(plan);
    } catch (e) {
      pushToast('error', errorMessage(e));
    }
  }, [repoId, pushToast, setPendingUndo]);

  // P15b: analyze the file diff currently open in the center-pane overlay. Only
  // workdir kinds (staged/unstaged/untracked) map to a `workdirFile` target;
  // `staged` is true only for the staged slot. `undefined` hides the affordance
  // for AI-ineligible or non-workdir (commit/compare/conflict) overlays.
  const overlayExplain = useMemo<(() => void) | undefined>(() => {
    if (!aiEligible || overlayMeta === null) return undefined;
    const meta = overlayMeta;
    if (meta.kind !== 'staged' && meta.kind !== 'unstaged' && meta.kind !== 'untracked') {
      return undefined;
    }
    return () =>
      runAnalyze(
        {
          kind: 'workdirFile',
          path: meta.path,
          origPath: meta.origPath,
          staged: meta.kind === 'staged',
        },
        'explain',
        `Explain ${meta.path}`,
      );
  }, [aiEligible, overlayMeta, runAnalyze]);

  // P23d + P38: blame / file-history / reflog read overlays (state lives above;
  // this hook owns the reqId stale-guards + open/close handlers + restore effect).
  const {
    closeBlame,
    closeHistory,
    closeReflog,
    openReflog,
    revealCommitByOid,
    handleBlame,
    handleFileHistory,
  } = useReadOverlays({
    repoId,
    pushToast,
    mutating,
    setBlame,
    setHistory,
    setReflog,
    blameReqId,
    historyReqId,
    reflogReqId,
    reflogRef,
    reflogRestoreRef,
    graphDataRef,
    compareRef,
    clearCompare,
    setSelectedIndex,
    expandForReveal: fold.expandFor,
  });

  // P50b: commit search — state hook drives the search bar + graph match rings;
  // next/prev reuse revealCommitByOid (the single-selection reveal path).
  const search = useCommitSearch({ repoId, graph, revealCommitByOid, pushToast });

  // Spec-003/004: a WALK change reloads the graph (+ selection reveal); the
  // fold rising edge re-requests for spans (toggle-off stays local, plan lock).
  useGraphFilterRefetch({ filterKey: graphFilter.walkKey, foldLinear: graphFilter.foldLinear, refetchGraph, selectedIndexRef, graphDataRef, revealCommitByOid });

  // P57c: semantic-history "Ask history" — retrieval + AI answer. The answer
  // routes into the shared AiOutputPanel via runHistoryAnswer (aiPanel req-id).
  const historySearch = useHistorySearch({
    repoId,
    graph,
    revealCommitByOid,
    aiEligible,
    runAiAnswer: runHistoryAnswer,
    pushToast,
  });

  // Spec-005: the overview-rail bundle (channel pick + jump resolvers live in
  // railProps.ts; GraphCanvas mounts the rail only while visible).
  const rail = useRailInput({ search, historySearch, graph, revealCommitByOid, generation: railGeneration, alwaysShow: graphMinimapAlwaysShow });

  // Spec-007: replay controller (entry snapshot + fab/palette gate) — replayProps.ts.
  const replay = useReplayController({ graph, metrics, metricsVersion, display: graphDisplay,
    graphStyle, graphSeason, themeVersion, reducedMotion, pushToast });

  // P54c: commit composer row "Preview" — moved to composerPreview.ts.
  const previewComposerFileDiff = useCallback(
    (path: string): Promise<FileDiff> => composerPreviewFileDiff(statusRef.current, repoId, path),
    [repoId],
  );
  const composer = useCommitComposer({
    repoId,
    refreshAll,
    pushToast,
    previewFileDiff: previewComposerFileDiff,
  });
  // Status badge per changed path for the composer file rows.
  const composerStatusByPath = useMemo(() => {
    const m = new Map<string, FileStatus>();
    if (status !== null) {
      for (const e of status.staged) m.set(e.path, e.status);
      for (const e of status.untracked) m.set(e.path, e.status);
      for (const e of status.unstaged) m.set(e.path, e.status);
    }
    return m;
  }, [status]);
  // Branch/ref scope options for the search bar (All refs + local + remote).
  const searchScopeOptions = useMemo<ComboboxOption[]>(() => searchScopeOptionsOf(branches), [branches]);

  // P50c: command palette (Ctrl/Cmd-K). usePalette owns open/close; the
  // accelerator + Esc-layering are wired through useWorkspaceKeyboard below. The
  // entry registry is assembled ONLY while open (its tag lookup scans the whole
  // graph) and merges the repo-scoped actions with App's `appCommands`.
  const palette = usePalette({ active });

  // New-branch/new-worktree/search openers + dynamic palette rows — moved
  // verbatim to paletteCallbacks.ts (spec-007 size offset).
  const { openNewBranch, openNewWorktree, openSearchEmpty, paletteRunSearch, paletteJumpToCommit } =
    usePaletteCallbacks({
      headBranch, setPendingCreateBranch, setNewWorktreeOpen,
      openSearch: search.openSearch, graphDataRef, revealCommitByOid, pushToast,
    });

  const paletteActions = useMemo<PaletteAction[]>(() => {
    if (!palette.open) return [];
    const actions = buildPaletteActions({
      mutating,
      refreshing,
      statusLoading,
      graphLoading,
      opActive,
      canPullPush,
      hasHeadBranch: headBranch !== null,
      onFetch, // P63: wrapped to refresh forge signals after fetch
      onPull, // P63: wrapped to refresh forge signals after pull
      onPush: () => void handlePush(),
      onRefresh: () => void handleRefresh(),
      onNewBranch: openNewBranch,
      onNewWorktree: openNewWorktree,
      onOpenSearch: openSearchEmpty,
      onOpenHistory: historySearch.openPanel,
      onReplayHistory: replay.onOpen,
      canReplay: replay.canReplay,
      branches,
      graph,
      revealCommitByOid,
      appCommands,
    });
    // P55c / P56b lead the Action group; P68e's dock rows trail it. Both registries
    // live in `paletteActions.ts` (§E) so this container stays a composition site.
    actions.unshift(...aiDock.paletteEntries.lead);
    actions.push(...aiDock.paletteEntries.trail);
    actions.push(...gitDock.paletteEntries); // P87b §5: the "Git activity" row.
    actions.push(...graphFilterPaletteEntries(graphFilter)); // Spec-003 §3.2.
    return actions;
  }, [
    palette.open,
    mutating,
    refreshing,
    statusLoading,
    graphLoading,
    opActive,
    canPullPush,
    headBranch,
    onFetch,
    onPull,
    handlePush,
    handleRefresh,
    openNewBranch,
    openNewWorktree,
    openSearchEmpty,
    historySearch.openPanel,
    branches,
    graph,
    revealCommitByOid,
    appCommands,
    aiDock.paletteEntries,
    gitDock.paletteEntries,
    graphFilter,
    replay.onOpen, replay.canReplay,
  ]);

  function handleToggleConflictView(path: string) {
    const key = `conflict:${path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchConflictSlot(path);
  }

  function handleBannerCommitMerge() {
    if (commitBoxRef.current !== null) {
      commitBoxRef.current.submit();
    } else {
      setSelectedIndex(null);
    }
  }

  function handleToggleWorkdirDiff(section: WorkdirSection, entry: StatusEntry) {
    const key = `${section}:${entry.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () =>
      ipc.getWorkdirFileDiff(
        repoId,
        entry.path,
        entry.origPath,
        section === 'staged',
        diffViewMode === 'file',
        intraline,
      ),
    );
  }

  function handleSelectParent(parentOrdinal: number) {
    if (selectedIndex === null || graph === null) return;
    const node = graph.nodes[selectedIndex];
    if (!node) return; // selection not yet in the streamed (partial) window
    const parentIndex = node.parents[parentOrdinal];
    if (parentIndex !== undefined) setSelectedIndex(parentIndex);
  }

  // Stable so ContextMenu's dismiss-listener effect doesn't re-arm on every
  // parent re-render while the menu is open (reviewer NIT).
  const closeMenu = useCallback(() => setMenu(null), []);

  const dialogOpen =
    anyDialogArmed ||
    askOpen ||
    pendingProposedOp !== null ||
    hookGate.pendingHook !== null ||
    hookDisclosure.pendingHookDisclosure;

  // Per-repo keyboard handling (Esc-layering + shortcut effects), active tab only.
  useWorkspaceKeyboard({
    active,
    globalModalOpen,
    collapseDiffSlot: handleDismissDiffOverlay,
    clearCompare,
    closeAiPanel,
    closeBlame,
    closeHistory,
    closeReflog,
    aiPanelOpenRef,
    blameOpenRef,
    historyOpenRef,
    reflogOpenRef,
    commitBrowserOpenRef,
    composerOpenRef: composer.openRef,
    closeComposer: composer.escClose,
    composerOpen: composer.open,
    searchOpenRef: search.openRef,
    closeSearch: search.close,
    historySearchOpenRef: historySearch.openRef,
    closeHistorySearch: historySearch.close,
    paletteOpenRef: palette.openRef,
    closePalette: palette.close,
    // Spec-007: Esc peel + inert-gate backstops while the replay overlay is up.
    replayOpenRef: replay.openRef, closeReplay: replay.onExit, replayOpen: replay.open,
    diffSlotRef,
    compareRef,
    setSelectedIndex,
    setCommitBrowserOpen,
    searchOpen: search.open,
    openSearch: search.openSearch,
    historySearchOpen: historySearch.open,
    paletteOpen: palette.open,
    togglePalette: palette.toggle,
    refreshing,
    statusLoading,
    graphLoading,
    mutating,
    canPullPush,
    // Audit §3.9: the bulk-AI confirm is a sibling modal — suppress workspace
    // shortcuts (Ctrl+K/Ctrl+F/F5, graph navigation) under it like the rest.
    // It joins here rather than in the `dialogOpen` disjunction because `aiBulk`
    // is declared after that point.
    dialogOpen: dialogOpen || aiBulk.confirm.open,
    abortConfirmOpen,
    selectedIndex,
    graph,
    graphRef,
    fold, // spec-004: display-space nav + pill land/expand/collapse semantics
    onAiActivity: aiDock.focusDock,
    onGitActivity: gitDock.toggleDock,
    handleRefresh,
    handleFetch: onFetch, // P63: refresh forge signals after fetch
    handlePull: onPull, // P63: refresh forge signals after pull
    handlePush,
  });

  // P37b: force-push needs a normal-push-capable HEAD with a configured upstream.
  const canForcePush = canPullPush && headBranch?.upstream != null;

  const { handleOpenInTerminal, handleRevealInFileManager, handleOpenInEditor } =
    useExternalTools(pushToast); // P49b launchers (extracted to useExternalTools.ts)

  // P3e §menu-extraction: the context-menu item-array builders live in
  // workspaceMenus.ts now; rebuild them each render over the current state +
  // handlers so the produced arrays stay byte-identical to the old inline ones.
  const menus = createWorkspaceMenus({
    branches,
    headBranch,
    head,
    mutating,
    opActive,
    aiEligible,
    remotes,
    pushToast,
    handleCheckoutRemote,
    handleCheckoutBranch,
    handleCheckoutCommit,
    setPendingCreateBranch,
    runSummarize,
    runAnalyze,
    runChangelog,
    handleMergeBranch,
    setPendingRebase,
    openRebasePlan,
    handleCompareWithHead,
    setPendingDeleteRemote,
    setPendingDeleteBranch,
    setPendingRenameBranch,
    // F-A6-B: the menu passes the rendered oid as arg 2; forward it as the
    // wrong-target guard (skipReserved stays false for the first attempt).
    handleApplyStash: (index, oid) => void handleApplyStash(index, false, oid),
    handlePopStash: (index, oid) => void handlePopStash(index, false, oid),
    setPendingDropStash,
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
    setPendingDeinitSubmodule,
    setPendingRemoveSubmodule,
    onOpenRepoPath,
    setWorktreeContextOpen,
    setPendingWorktreeLock,
    handleUnlockWorktree,
    setPendingWorktreeRemove,
    setPendingDeleteTag,
    handlePushTag,
    tagSync: tagSyncReport,
    handleForceRefreshTag,
    handleFetchRemoteTag,
    setPendingDeleteRemoteTag,
    setPendingForceMoveTag,
    setPendingRenameRemote,
    setPendingEditUrl,
    setPendingRemoveRemote,
    setPendingCreateTag,
    handleCherrypick,
    handleRevert,
    setPendingReset,
    onViewReflog: (name: string) => void openReflog(name),
    pendingBisectBad,
    bisectActive: opState.kind === 'bisect',
    handleMarkBisectBad: (oid: string) => {
      setPendingBisectBad(oid);
      pushToast('info', 'Bisect: now pick an older known-GOOD commit to start');
    },
    handleStartBisect: (bad: string, good: string) => void handleStartBisect(bad, good),
    onOpenInTerminal: handleOpenInTerminal,
    onRevealInFileManager: handleRevealInFileManager,
    onOpenInEditor: handleOpenInEditor,
    refFilterItems: (fullRef, noun) => refFilterMenuItems(graphFilter, fullRef, noun),
  });

  // The row → context-menu adapters that arm the ONE shared ContextMenu state
  // (extracted to repoWorkspace/contextMenuOpeners.ts). Built after `menus`.
  const {
    handleStashContextMenu,
    handleSubmoduleContextMenu,
    handleWorktreeContextMenu,
    handleTagContextMenu,
    handleRemoteContextMenu,
    handleGraphContextMenu,
    handleSidebarContextMenu,
  } = createContextMenuOpeners({
    setMenu,
    menus,
    submodules,
    worktrees,
    headBranch,
    graphFilter,
  });

  // P39b: bisect-banner oid summaries (extracted to bisectSummaries.ts).
  const bisectSummaries = bisectSummariesOf(opState, graph);

  // P38 §7.2/§7.3: reflog restore wiring. Both actions arm the SHARED dialogs
  // (create-branch PromptDialog / reset ConfirmDialog) — no new mutation path.
  // Reset is offered only on an attached, born HEAD (same predicate as
  // resetMenuItems); otherwise the view hides the reset actions.
  const reflogCanReset = head !== null && !head.unborn && !head.detached;
  const reflogResetLabel = headBranch?.name ?? 'HEAD';
  const onReflogCreateBranch = useCallback((newOid: string) => {
    reflogRestoreRef.current = true;
    setPendingCreateBranch({ oid: newOid });
  }, [setPendingCreateBranch]);
  const onReflogReset = useCallback(
    (newOid: string, mode: ResetMode) => {
      reflogRestoreRef.current = true;
      setPendingReset({ oid: newOid, mode });
    },
    [setPendingReset],
  );

  // P11g-rev §4.4: resolve the DiffBrowser source labels + header list. Compare
  // mode AUTO-OPENS once data has loaded (≥1 file); commit mode is EXPLICIT-open
  // (gated on commitBrowserOpen). null → browser not rendered.
  const diffBrowserView = useMemo(
    () => diffBrowserViewOf({ compare, compareData, selectedIndex, graph, commitBrowserOpen, commitDiff, headBranch, clearCompare, setCommitBrowserOpen }),
    [compare, compareData, selectedIndex, graph, commitBrowserOpen, commitDiff, headBranch, clearCompare],
  );

  return (
    <>
      <WorkspaceToolbar
        remoteOp={remoteOp}
        refreshing={refreshing}
        netBusy={submoduleBusy !== null}
        mutating={mutating}
        statusLoading={statusLoading}
        graphLoading={graphLoading}
        canPullPush={canPullPush}
        canForcePush={canForcePush}
        aiEligible={aiEligible}
        aiPanelLoading={aiPanel?.loading === true}
        headBranch={headBranch}
        jobStatus={jobStatus}
        jobNow={jobNow}
        onFetch={onFetch}
        onPull={onPull}
        onPush={traced('click', GESTURES.push, () => void handlePush())}
        onForcePush={() => handleForcePush()}
        onWhatChanged={() => setWhatChangedOpen(true)}
        onAskBonsai={openAskBonsai}
        onUndo={() => void handleRequestUndo()}
        onViewHeadReflog={() =>
          reflog && reflog.refName === 'HEAD' ? closeReflog() : void openReflog('HEAD')
        }
        headBorn={head !== null && !head.unborn}
        onRefresh={() => void handleRefresh()}
        externalItems={menus.externalToolsItems(repoPath)}
        {...gitDock.toolbarProps}
      />

      <div className="panes">
        {/* Spec-003 §3.3: rows read solo/hidden membership from this context. */}
        <RefFilterMarkerContext.Provider value={graphFilter.markerFor}>
        <Sidebar
          data={branches}
          loading={branchesLoading}
          error={branchesError}
          onDismissError={() => setBranchesError(null)}
          busy={mutating}
          opActive={opActive}
          currentBranch={headBranch?.name ?? null}
          onCheckout={traced('click', GESTURES.branchCheckout, (name: string) => {
            void handleCheckoutBranch(name);
          })}
          onContextMenu={handleSidebarContextMenu}
          onCreateBranch={traced('click', GESTURES.branchCreate, handleCreateBranch)}
          width={paneWidths.sidebar}
          listView={listView}
          stashes={stashes}
          onCreateStash={() => void handleCreateStash('allWithUntracked')}
          onStashContextMenu={handleStashContextMenu}
          submodules={submodules}
          onSubmoduleContextMenu={handleSubmoduleContextMenu}
          submoduleBusy={submoduleBusy}
          onNewSubmodule={() => setPendingAddSubmodule(true)}
          worktrees={worktrees}
          onWorktreeContextMenu={handleWorktreeContextMenu}
          onNewWorktree={() => setNewWorktreeOpen(true)}
          onTagContextMenu={handleTagContextMenu}
          tagSyncReport={tagSyncReport}
          tagSyncState={tagSyncState}
          tagSyncRemote={tagSyncRemote}
          tagSyncCheckedAt={tagSyncCheckedAt}
          onTagsExpand={() => void refetchTagSync()}
          remotes={remotes}
          onRemoteContextMenu={handleRemoteContextMenu}
          onAddRemote={() => setPendingAddRemote(true)}
          onCleanupBranches={() => setStaleCleanupOpen(true)}
          onReveal={handleReveal}
        />
        </RefFilterMarkerContext.Provider>
        <PaneDivider side="sidebar" onResize={onSidebarResize} onResizeEnd={onPaneResizeEnd} />
        <WorkspaceGraphPane
          prNumber={prOverlaySlot?.prNumber ?? null}
          graphError={graphError}
          graph={graph}
          head={head}
          graphRef={graphRef}
          selectedIndex={selectedIndex}
          compare={compare}
          clearCompare={clearCompare}
          setSelectedIndex={setSelectedIndex}
          wip={wip}
          themeVersion={themeVersion}
          active={active}
          onContextMenu={handleGraphContextMenu}
          metrics={metrics}
          metricsVersion={metricsVersion}
          display={graphDisplay}
          verifyStatus={verification.verifyStatus}
          onVisibleRangeChange={verification.onVisibleRangeChange}
          onOpenPr={onOpenPr}
          edgeIndex={graphEdgeIndex ?? undefined}
          totalRows={graphTotal ?? undefined}
          revealFlash={revealFlash}
          reducedMotion={reducedMotion}
          graphStyle={graphStyle}
          graphSeason={graphSeason}
          graphFilter={graphFilter}
          replay={replay}
          graphFold={fold}
          rail={rail}
          graphFilterStale={graphFilterStale}
          search={search}
          searchScopeOptions={searchScopeOptions}
          historySearch={historySearch}
          diffSlot={diffSlot}
          overlayMeta={overlayMeta}
          collapseDiffSlot={handleDismissDiffOverlay}
          onResolveConflictText={handleResolveConflictText}
          mutating={mutating}
          overlayExplain={overlayExplain}
          diffViewMode={diffViewMode}
          onSetViewMode={handleSetViewMode}
          intraline={intraline}
          onSetIntraline={handleToggleIntraline}
          imageDiff={imageDiff}
          imageDiffLoading={imageDiffLoading}
          imageDiffError={imageDiffError}
          stageable={stageable}
          onStageLines={handleStageLines}
          onStageHunk={handleStageHunk}
          onDiscardHunk={handleDiscardHunk}
          onDiscardLines={handleDiscardLines}
          blame={blame}
          closeBlame={closeBlame}
          revealCommitByOid={revealCommitByOid}
          blameAiEligible={aiEligible}
          onBlameExplain={onBlameExplain}
          history={history}
          closeHistory={closeHistory}
          reflog={reflog}
          closeReflog={closeReflog}
          reflogBusy={mutating}
          reflogResetLabel={reflogResetLabel}
          onReflogCreateBranch={onReflogCreateBranch}
          onReflogReset={reflogCanReset ? onReflogReset : undefined}
          aiPanel={aiPanel}
          closeAiPanel={closeAiPanel}
          diffBrowserView={diffBrowserView}
          repoId={repoId}
          scope={scope}
          listView={listView}
          onOpenIdentitySettings={onOpenIdentitySettings}
        />
        <PaneDivider
          side="right-panel"
          onResize={onRightPanelResize}
          onResizeEnd={onPaneResizeEnd}
        />
        <WorkspaceRightPanel
          rightPanelWidth={paneWidths.rightPanel}
          repoId={repoId}
          rightPaneTab={rightPaneTab}
          onOpenPrFileDiff={handleOpenPrFileDiff}
          onClosePrFileDiff={handleClosePrFileDiff}
          prOverlayPath={prOverlaySlot?.path ?? null}
          prRestoreFocusTo={prRestoreFocusTo}
          onSelectRightPaneTab={setRightPaneTab}
          prDefaultHead={headBranch?.name ?? null}
          prDefaultBase={prDefaultBase}
          prBaseOptions={prBaseOptions}
          prCompareOptions={prCompareOptions}
          prNav={prNav}
          checksTarget={checksTab.target}
          checksRefreshSeq={checksTab.refreshSeq}
          onPushChecksBranch={checksTab.target?.name === headBranch?.name ? () => void pushCurrentBranch() : undefined}
          onRevealCommit={(oid) => handleReveal({ kind: 'oid', oid })}
          opState={opState}
          conflicts={conflicts}
          mutating={mutating}
          onCommitMerge={handleBannerCommitMerge}
          onRebaseContinue={() => void handleRebaseContinue()}
          onRebaseSkip={() => void handleRebaseSkip()}
          onCherrypickContinue={() => void handleCherrypickContinue()}
          onRevertContinue={() => void handleRevertContinue()}
          onAbort={() => setAbortConfirmOpen(true)}
          onBisectMark={(isGood) => void handleBisectMark(isGood)}
          onBisectSkip={() => void handleBisectSkip()}
          bisectSummaries={bisectSummaries}
          compare={compare}
          compareData={compareData}
          compareLoading={compareLoading}
          compareError={compareError}
          headBranch={headBranch}
          listView={listView}
          panelDensity={panelDensity}
          primaryCommitAction={primaryCommitAction}
          scope={scope}
          setScope={setScope}
          clearCompare={clearCompare}
          selectedIndex={selectedIndex}
          graph={graph}
          commitDiff={commitDiff}
          commitDiffLoading={commitDiffLoading}
          commitDiffError={commitDiffError}
          setCommitBrowserOpen={setCommitBrowserOpen}
          onSelectParent={handleSelectParent}
          setSelectedIndex={setSelectedIndex}
          aiEligible={aiEligible}
          runAnalyze={runAnalyze}
          status={status}
          statusLoading={statusLoading}
          statusError={statusError}
          diffSlot={diffSlot}
          aiRows={aiRuns.rowStates}
          aiAtCapacity={aiRuns.atCapacity}
          aiBulk={aiBulk.control}
          aiPanelLoading={aiPanel?.loading === true}
          onStage={(paths) => void handleStage(paths)}
          onUnstage={(paths) => void handleUnstage(paths)}
          onDiscard={(paths) => setPendingDiscard(paths)}
          onDiscardForce={(paths) => requestDiscardForce(paths)}
          onToggleDiff={handleToggleWorkdirDiff}
          onResolveConflict={(path, r) => void handleResolveConflict(path, r)}
          onToggleConflictView={handleToggleConflictView}
          onAiResolve={(path) => aiRuns.startConflictRun(path)}
          onAiReview={aiDock.reviewForPath}
          onAiReveal={aiDock.revealForPath}
          onBlame={(path) => void handleBlame(path)}
          onFileHistory={(path) => void handleFileHistory(path)}
          onCreateStash={(scope) => void handleCreateStash(scope)}
          head={head}
          amend={amend}
          onToggleAmend={(next) => void handleToggleAmend(next)}
          amendMessage={amendMessage}
          commitBoxRef={commitBoxRef}
          onCommitAmend={handleCommitAmend}
          onCommitMergeSubmit={handleCommitMerge}
          onCommit={traced('click', GESTURES.commitSubmit, handleCommit)}
          onCommitAndPush={
            headBranch ? (m, sign, skipHooks) => handleCommitAndPush(m, sign, skipHooks) : undefined
          }
          onGenerate={handleGenerateCommitMessage}
          workingDirty={workingDirty}
          onCompose={() => composer.openComposer()}
          onOpenIdentitySettings={onOpenIdentitySettings}
          onOpenAccountSettings={onOpenAccountSettings}
          signingStatus={signingStatus}
          commitSignature={commitSignature}
          commitPhase={gitDock.commitPhase}
          onShowGitActivity={gitDock.focusDock}
        />
      </div>

      {/* P68e / P87b: `.workspace-host`'s bottom children — git dock (outermost) →
          AI dock → .panes. Each renders null until its first run of the session. */}
      <AiActivityPanel {...aiDock.panelProps} />
      <GitActivityDock {...gitDock.panelProps} />

      {/* P84: always-mounted a11y live region for reveal announcements. Rendered
          AFTER the dock so `.workspace-host`'s counted toolbar → .panes → dock
          child order (dock = 3rd child) is preserved — a `.sr-only` region is
          position-agnostic for screen readers. */}
      <RevealAnnouncer message={revealMessage} />

      <WorkspaceDialogs
        repoId={repoId}
        mutating={mutating}
        opState={opState}
        headBranch={headBranch}
        branches={branches}
        remotes={remotes}
        worktrees={worktrees}
        abortConfirmOpen={abortConfirmOpen}
        setAbortConfirmOpen={setAbortConfirmOpen}
        handleRebaseAbort={() => void handleRebaseAbort()}
        handleCherrypickAbort={() => void handleCherrypickAbort()}
        handleRevertAbort={() => void handleRevertAbort()}
        handleAbortMerge={() => void handleAbortMerge()}
        handleBisectReset={() => void handleBisectReset()}
        pendingDeleteBranch={pendingDeleteBranch}
        setPendingDeleteBranch={setPendingDeleteBranch}
        handleDeleteBranch={traced('click', GESTURES.branchDelete, (name: string) => {
          void handleDeleteBranch(name);
        })}
        pendingRebase={pendingRebase}
        setPendingRebase={setPendingRebase}
        handleRebaseBranch={(name) => void handleRebaseBranch(name)}
        pendingDeleteRemote={pendingDeleteRemote}
        setPendingDeleteRemote={setPendingDeleteRemote}
        handleDeleteRemoteTracking={traced(
          'click',
          GESTURES.deleteRemoteTracking,
          (name: string) => {
            void handleDeleteRemoteTracking(name);
          },
        )}
        pendingDropStash={pendingDropStash}
        setPendingDropStash={setPendingDropStash}
        handleDropStash={(index, oid) => void handleDropStash(index, oid)}
        pendingReservedStash={pendingReservedStash}
        setPendingReservedStash={setPendingReservedStash}
        handleApplyStashSkipping={(index, oid) => void handleApplyStash(index, true, oid)}
        handlePopStashSkipping={(index, oid) => void handlePopStash(index, true, oid)}
        pendingReset={pendingReset}
        setPendingReset={setPendingReset}
        handleResetBranch={(oid, mode) => void handleResetBranch(oid, mode)}
        pendingDiscard={pendingDiscard}
        setPendingDiscard={setPendingDiscard}
        handleDiscard={(paths) => void handleDiscard(paths)}
        pendingDiscardForce={pendingDiscardForce}
        setPendingDiscardForce={setPendingDiscardForce}
        handleDiscardForce={(paths) => void handleDiscardForce(paths)}
        pendingCommitPush={pendingCommitPush}
        handleConfirmCommitPush={handleConfirmCommitPush}
        handleCancelCommitPush={handleCancelCommitPush}
        pendingForcePush={pendingForcePush}
        setPendingForcePush={setPendingForcePush}
        doForcePush={() => void doForcePush()}
        remoteOp={remoteOp}
        pendingHook={hookGate.pendingHook}
        hookRetrying={hookGate.hookRetrying}
        onHookSkipRetry={hookGate.onHookSkipRetry}
        onHookCancel={hookGate.onHookCancel}
        pendingHookDisclosure={hookDisclosure.pendingHookDisclosure}
        onHookDiscloseConfirm={hookDisclosure.onHookDiscloseConfirm}
        onHookDiscloseCancel={hookDisclosure.onHookDiscloseCancel}
        pendingHunkDiscard={pendingHunkDiscard}
        setPendingHunkDiscard={setPendingHunkDiscard}
        handleConfirmHunkDiscard={(pending) => void handleConfirmHunkDiscard(pending)}
        pendingLineDiscard={pendingLineDiscard}
        setPendingLineDiscard={setPendingLineDiscard}
        handleConfirmLineDiscard={(pending) => void handleConfirmLineDiscard(pending)}
        staleCleanupOpen={staleCleanupOpen}
        setStaleCleanupOpen={setStaleCleanupOpen}
        refetchBranches={refetchBranches}
        refetchGraph={refetchGraph}
        pendingCreateBranch={pendingCreateBranch}
        setPendingCreateBranch={setPendingCreateBranch}
        handleCreateBranchHere={traced(
          'click',
          GESTURES.branchCreateHere,
          (oid: string, name: string) => {
            void handleCreateBranchHere(oid, name);
          },
        )}
        pendingRenameBranch={pendingRenameBranch}
        setPendingRenameBranch={setPendingRenameBranch}
        handleRenameBranch={traced(
          'click',
          GESTURES.branchRename,
          (oldName: string, newName: string) => {
            void handleRenameBranch(oldName, newName);
          },
        )}
        aiEligible={aiEligible}
        workingDirty={workingDirty}
        suggestBranchName={suggestBranchName}
        pendingCreateTag={pendingCreateTag}
        setPendingCreateTag={setPendingCreateTag}
        handleCreateTag={(oid, name, message) => void handleCreateTag(oid, name, message)}
        pendingDeleteTag={pendingDeleteTag}
        setPendingDeleteTag={setPendingDeleteTag}
        handleDeleteTag={(name) => void handleDeleteTag(name)}
        pendingAddRemote={pendingAddRemote}
        setPendingAddRemote={setPendingAddRemote}
        handleAddRemote={(name, url) => void handleAddRemote(name, url)}
        pendingEditUrl={pendingEditUrl}
        setPendingEditUrl={setPendingEditUrl}
        handleSetRemoteUrl={(name, url) => void handleSetRemoteUrl(name, url)}
        pendingRenameRemote={pendingRenameRemote}
        setPendingRenameRemote={setPendingRenameRemote}
        handleRenameRemote={(name, newName) => void handleRenameRemote(name, newName)}
        pendingRemoveRemote={pendingRemoveRemote}
        setPendingRemoveRemote={setPendingRemoveRemote}
        handleRemoveRemote={(name) => void handleRemoveRemote(name)}
        whatChangedOpen={whatChangedOpen}
        setWhatChangedOpen={setWhatChangedOpen}
        runDigest={runDigest}
        newWorktreeOpen={newWorktreeOpen}
        setNewWorktreeOpen={setNewWorktreeOpen}
        handleAddWorktree={handleAddWorktree}
        worktreeContextOpen={worktreeContextOpen}
        setWorktreeContextOpen={setWorktreeContextOpen}
        pendingWorktreeLock={pendingWorktreeLock}
        setPendingWorktreeLock={setPendingWorktreeLock}
        handleLockWorktree={(name, reason) => void handleLockWorktree(name, reason)}
        pendingWorktreeRemove={pendingWorktreeRemove}
        setPendingWorktreeRemove={setPendingWorktreeRemove}
        handleRemoveWorktree={(name) => void handleRemoveWorktree(name)}
        rebasePlan={rebasePlan}
        setRebasePlan={setRebasePlan}
        rebasePlanError={rebasePlanError}
        setRebasePlanError={setRebasePlanError}
        handleStartInteractiveRebase={(ontoOid, ontoLabel, todos) =>
          void handleStartInteractiveRebase(ontoOid, ontoLabel, todos)
        }
        menu={menu}
        closeMenu={closeMenu}
        bulkAiConfirm={aiBulk.confirm}
      />
      <WorkspaceOverlays
        mutating={mutating}
        pendingCherrypick={pendingCherrypick}
        setPendingCherrypick={setPendingCherrypick}
        confirmCherrypick={confirmCherrypick}
        pendingNonFfPull={pendingNonFfPull}
        setPendingNonFfPull={setPendingNonFfPull}
        handleMergeBranch={handleMergeBranch}
        handleRebaseBranch={handleRebaseBranch}
        pendingUndo={pendingUndo}
        setPendingUndo={setPendingUndo}
        handleResetBranch={handleResetBranch}
        paletteOpen={palette.open}
        paletteActions={paletteActions}
        onClosePalette={palette.close}
        paletteRunSearch={paletteRunSearch}
        paletteJumpToCommit={paletteJumpToCommit}
        askOpen={askOpen}
        askBusy={askBusy}
        runPlanOperation={runPlanOperation}
        cancelAskBonsai={cancelAskBonsai}
        pendingProposedOp={pendingProposedOp}
        opDispatching={opDispatching}
        confirmProposedOp={confirmProposedOp}
        cancelProposedOp={cancelProposedOp}
        changelogOpen={changelogOpen}
        branches={branches}
        headBranch={headBranch}
        setChangelogOpen={setChangelogOpen}
        runChangelog={runChangelog}
        composer={composer}
        composerStatusByPath={composerStatusByPath}
        pendingAddSubmodule={pendingAddSubmodule}
        setPendingAddSubmodule={setPendingAddSubmodule}
        handleAddSubmodule={handleAddSubmodule}
        pendingDeinitSubmodule={pendingDeinitSubmodule}
        setPendingDeinitSubmodule={setPendingDeinitSubmodule}
        handleDeinitSubmodule={handleDeinitSubmodule}
        pendingRemoveSubmodule={pendingRemoveSubmodule}
        setPendingRemoveSubmodule={setPendingRemoveSubmodule}
        handleRemoveSubmodule={handleRemoveSubmodule}
        pendingForceSubmodule={pendingForceSubmodule}
        setPendingForceSubmodule={setPendingForceSubmodule}
      />
      {/* P77 §4: destructive remote-tag confirms (delete-on-remote, force-move). */}
      <TagSyncDialogs
        busy={mutating}
        pendingDeleteRemoteTag={pendingDeleteRemoteTag}
        setPendingDeleteRemoteTag={setPendingDeleteRemoteTag}
        handleDeleteRemoteTag={(remote, name) => void handleDeleteRemoteTag(remote, name)}
        pendingForceMoveTag={pendingForceMoveTag}
        setPendingForceMoveTag={setPendingForceMoveTag}
        handleForceMoveRemoteTag={(remote, name, newShort) =>
          void handleForceMoveRemoteTag(remote, name, newShort)
        }
      />
    </>
  );
}
