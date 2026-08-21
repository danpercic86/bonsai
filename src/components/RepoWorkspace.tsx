import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { CommitBoxHandle } from './CommitBox';
import type { ContextMenuItem } from './ContextMenu';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceDialogs } from './WorkspaceDialogs';
import { WorkspaceOverlays } from './WorkspaceOverlays';
import { WorkspaceGraphPane } from './WorkspaceGraphPane';
import { WorkspaceRightPanel } from './WorkspaceRightPanel';
import { isUsableRepo, shortOid } from './workspaceUtils';
import { createWorkspaceMenus } from './workspaceMenus';
import type { DiffOverlayMeta } from './DiffOverlay';
import type { DiffScope } from './DiffFileTree';
import { PaneDivider } from './PaneDivider';
import { Sidebar } from './Sidebar';
import type { DiffSlot, WorkdirSection } from './StatusPanel';
import type { GraphCanvasHandle, GraphContextTarget, WipSummary } from '../graph/GraphCanvas';
import { effectiveMetrics } from '../graph/metrics';
import type { GraphDisplayOptions } from '../graph/rightColumns';
import { createGraphStream } from '../graph/streamAssembler';
import { createGraphStreamApplier } from './repoWorkspace/graphStreamApply';
import type { IncrementalEdgeIndex } from '../graph/incrementalEdgeIndex';
import { ipc } from '../ipc';
import type {
  AiAnalysisMode,
  AiAutonomy,
  AiAvailability,
  AiDiffTarget,
  AiDigestRange,
  BlameLine,
  BranchesSnapshot,
  ChangelogRange,
  CommitDiff,
  CompareDiff,
  ConflictEntry,
  FileDiff,
  FileHistoryEntry,
  FileStatus,
  GraphLayout,
  GraphPrefs,
  HeadInfo,
  ImageDiff,
  ImageDiffRequest,
  JobStatus,
  LineSelection,
  ListView,
  PrNavRequest,
  ProposedOperation,
  RebaseTodoOp,
  PaneWidths,
  PanelDensity,
  PrimaryCommitAction,
  ReflogEntry,
  UndoPlan,
  RemoteInfo,
  RepoInfo,
  RepoOpState,
  ResetMode,
  SigningStatus,
  StashEntry,
  StatusEntry,
  StatusSnapshot,
  SubmoduleInfo,
  UiSettingsPatch,
  Unsubscribe,
  WorktreeInfo,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage, isAppError } from '../utils/errors';
import { isImagePath } from '../utils/imagePaths';

import { useRemoteOps, type NonFfPullInfo } from './repoWorkspace/useRemoteOps';
import { useCommitActions } from './repoWorkspace/useCommitActions';
import { usePartialStaging } from './repoWorkspace/usePartialStaging';
import { useHookGate } from './repoWorkspace/useHookGate';
import { useBranchActions } from './repoWorkspace/useBranchActions';
import { useAiRuns } from './repoWorkspace/useAiRuns';
import { AiActivityPanel } from './AiActivityPanel';
import { useAiDock } from './repoWorkspace/useAiDock';
import { useBulkAiResolve } from './repoWorkspace/useBulkAiResolve';
import { useMergeActions } from './repoWorkspace/useMergeActions';
import { useStashActions } from './repoWorkspace/useStashActions';
import { useSubmoduleActions, type SubmoduleBusy } from './repoWorkspace/useSubmoduleActions';
import { useWorktreeActions } from './repoWorkspace/useWorktreeActions';
import { useTagRemoteActions } from './repoWorkspace/useTagRemoteActions';
import { useRebaseActions } from './repoWorkspace/useRebaseActions';
import { useCherrypickRevertActions } from './repoWorkspace/useCherrypickRevertActions';
import { useBisectActions } from './repoWorkspace/useBisectActions';
import { useReadOverlays } from './repoWorkspace/useReadOverlays';
import { useWorkspaceKeyboard } from './repoWorkspace/useWorkspaceKeyboard';
import { useCommitSearch } from './repoWorkspace/useCommitSearch';
import { useCommitVerification } from './repoWorkspace/useCommitVerification';
import { useForgeSignals } from './repoWorkspace/useForgeSignals';
import { useHistorySearch } from './repoWorkspace/useHistorySearch';
import { useCommitComposer } from './repoWorkspace/useCommitComposer';
import { usePalette } from './repoWorkspace/usePalette';
import { buildPaletteActions, type PaletteAction } from './paletteActions';
import { safeOpDispatch } from './safeOpDispatch';
import type { ComboboxOption } from './Combobox';

export interface RepoWorkspaceProps {
  /** Canonical workdir path (== repoId, P3e §2). */
  repoId: string;
  /** True when this tab is visible (the others are display:none). Gates the
   *  keyboard shortcut + Esc effects, window-focus rescan, GraphCanvas remeasure
   *  and the activation self-heal refresh (§5.1/§7). */
  active: boolean;
  /** App-global display prefs / pane sizing threaded down. */
  listView: ListView;
  /** P67 §4: right-panel vertical density (applied as a `data-density`
   *  attribute on the right panel's `<aside>`). */
  panelDensity: PanelDensity;
  /** P80 D1: which commit button is emphasized in the Working tab footer. */
  primaryCommitAction: PrimaryCommitAction;
  themeVersion: number;
  paneWidths: PaneWidths;
  /** True when a global modal (shortcut overlay / tab menu) is open — the
   *  workspace suppresses its own shortcuts + Esc handling (§5.1). */
  globalModalOpen: boolean;
  /** P11d §3.3/§4: user graph geometry knobs (threaded into the canvas). */
  graph: GraphPrefs;
  /** P11d §4.3: bumped by App on every graph-knob change → GraphCanvas re-measure. */
  metricsVersion: number;
  /** P13 §8: AI assistance settings + CLI health (App owns these + consent). */
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  /** CLI health status; null while App is probing. */
  aiAvailability: AiAvailability | null;
  /** P68e §8: persisted AI-dock geometry + `aiStreamLog`; `onAiDockChange` patches
   *  `aiDockHeight`/`aiDockCollapsed` (App debounces the write). */
  aiDockHeight: number;
  aiDockCollapsed: boolean;
  aiStreamLog: boolean;
  onAiDockChange(patch: UiSettingsPatch): void;
  onSidebarResize(delta: number): void;
  onRightPanelResize(delta: number): void;
  onPaneResizeEnd(): void;
  /** P19 §6.5: open `path` in a new/focused tab (App.openTab). Used by the
   *  submodule "Open in new tab" action; reuses the existing open-repo flow. */
  onOpenRepoPath(path: string): void;
  /** P40b: open Settings → Git config → Identity (commit-error linkage). */
  onOpenIdentitySettings(): void;
  /** P50c: App-level command-palette entries (toggle theme/lists, open Settings
   *  / AI Assets / Health, open repo / clone / new) — merged with the repo-scoped
   *  entries this workspace assembles. Built once in App. */
  appCommands: PaletteAction[];
}

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
  appCommands,
}: RepoWorkspaceProps) {
  const pushToast = usePushToast();
  const repoPath = repoId; // repoId == canonical workdir path (§2)

  // P13 §8.2: AI conflict-resolution is offered only when enabled, consented,
  // and the CLI is actually installed. The backend re-checks enabled+consented.
  const aiEligible = aiEnabled && aiConsented && aiAvailability?.installed === true;

  // P11d §4.1: METRICS overlaid with the user's graph knobs; memoized so the
  // canvas metricsRef only churns when a knob actually changes.
  const metrics = useMemo(() => effectiveMetrics(graphPrefs), [graphPrefs]);

  // RepoInfo is (re)loaded by refreshAll's openRepo; head also arrives via the
  // branches snapshot, so gating works before the first refreshAll.
  const [repo, setRepo] = useState<RepoInfo | null>(null);

  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [statusError, setStatusError] = useState<{ id: number; message: string } | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  // P62c: right-pane tab — the existing working/compare/commit tri-state
  // ('work') vs the pull-request panel ('prs'). PrPanel mounts only under 'prs'.
  const [rightPaneTab, setRightPaneTab] = useState<'work' | 'prs'>('work');

  // P30 D11: background-job status readout (fed by get_job_status on mount +
  // live job-status-changed events); jobNow re-renders the relative label.
  const [jobStatus, setJobStatus] = useState<JobStatus[]>([]);
  const [jobNow, setJobNow] = useState(() => Date.now());
  const [mutating, setMutating] = useState(false);
  // P11e §5: latest `mutating` read by the auto-fetch interval callback WITHOUT
  // resetting the timer on every mutation (it depends only on the settings).
  const mutatingRef = useRef(mutating);
  mutatingRef.current = mutating;

  const [branches, setBranches] = useState<BranchesSnapshot | null>(null);
  const [branchesError, setBranchesError] = useState<string | null>(null);
  const [branchesLoading, setBranchesLoading] = useState(false);

  // P51c: local-branch ahead/behind, keyed by branch name, for the graph's
  // ahead/behind chip. Only branches WITH an upstream (non-null counts) are
  // included; the chip render gates on divergence (>0) + the showAheadBehind
  // toggle. Memoized on `branches` so the canvas display object is stable
  // between refreshes.
  const branchStats = useMemo<Map<string, { ahead: number | null; behind: number | null }>>(() => {
    const m = new Map<string, { ahead: number | null; behind: number | null }>();
    for (const b of branches?.local ?? []) {
      if (b.ahead !== null && b.behind !== null) m.set(b.name, { ahead: b.ahead, behind: b.behind });
    }
    return m;
  }, [branches]);

  const [stashes, setStashes] = useState<StashEntry[]>([]);
  const [submodules, setSubmodules] = useState<SubmoduleInfo[]>([]);
  const [submoduleBusy, setSubmoduleBusy] = useState<SubmoduleBusy | null>(null);
  // P27 §6.3: worktrees (main first), refetched alongside submodules.
  const [worktrees, setWorktrees] = useState<WorktreeInfo[]>([]);
  // P22 §7.1: configured remotes (name + fetch URL), refetched alongside branches.
  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);
  const [remoteOp, setRemoteOp] = useState<'fetch' | 'pull' | 'push' | null>(null);

  const [opState, setOpState] = useState<RepoOpState>({ kind: 'none' });
  const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);
  // Tracks conflict count across renders so we auto-open the first conflicted
  // file exactly once per conflict episode (0 -> >0 edge), not on every refetch.
  const prevConflictCountRef = useRef(0);
  // P15b: explain/review output panel (read-only prose). RepoWorkspace owns the
  // ipc.aiAnalyzeDiff call + the panel's loading/error/result state; the panel is
  // presentational. `null` => not shown. A req-id guards against a stale response
  // overwriting a newer request or a closed panel.
  const [aiPanel, setAiPanel] = useState<{
    title: string;
    text: string | null;
    loading: boolean;
    error: string | null;
    costUsd: number | null;
    /** P56b: opt-in editable body — set only by runChangelog so the notes can be
     *  tweaked before copying. Every other runner omits it (read-only <pre>). */
    editable?: boolean;
  } | null>(null);
  const aiPanelReqId = useRef(0);
  const aiPanelOpenRef = useRef(false);
  aiPanelOpenRef.current = aiPanel !== null;
  const [abortConfirmOpen, setAbortConfirmOpen] = useState(false);
  const commitBoxRef = useRef<CommitBoxHandle>(null);
  // P6 §4.5: pending branch/remote deletes drive the two confirm dialogs; the
  // shortcut effect is suppressed while either is up (derived `dialogOpen`).
  const [pendingDeleteBranch, setPendingDeleteBranch] = useState<string | null>(null);
  // Plain (non-interactive) rebase confirm gate. `name` = the branch rebased
  // onto; `cur` = the current branch whose commits get rewritten (for the copy).
  const [pendingRebase, setPendingRebase] = useState<{ name: string; cur: string } | null>(null);
  const [pendingDeleteRemote, setPendingDeleteRemote] = useState<string | null>(null);
  const [pendingDropStash, setPendingDropStash] = useState<number | null>(null);
  // Reserved-path recovery: a stash apply/pop hit Windows-reserved paths (e.g.
  // `NUL`). Arms a ConfirmDialog offering to apply the rest, skipping those.
  const [pendingReservedStash, setPendingReservedStash] = useState<{
    index: number;
    op: 'apply' | 'pop';
    paths: string[];
  } | null>(null);
  // P20: destructive reset (all three modes confirm; hard warns extra) + discard.
  const [pendingReset, setPendingReset] = useState<{ oid: string; mode: ResetMode } | null>(null);
  const [pendingDiscard, setPendingDiscard] = useState<string[] | null>(null);
  // Bulk "Discard all" (panel + folder): reverts modified tracked files AND
  // deletes new/untracked files. Carries per-kind counts so the confirm dialog
  // can warn precisely about permanent deletion of new files.
  const [pendingDiscardForce, setPendingDiscardForce] = useState<{
    paths: string[];
    modified: number;
    created: number;
    // The untracked (created) subset of `paths` — the files permanently deleted,
    // listed in the confirm dialog so the destruction is spelled out per-path.
    untracked: string[];
  } | null>(null);
  // Commit & Push: when HEAD has no upstream, the message is parked here while a
  // ConfirmDialog asks to set upstream. The pending promise (resolves the
  // CommitBox submit) is held in commitPushResolver.
  const [pendingCommitPush, setPendingCommitPush] = useState<string | null>(null);
  const commitPushResolver = useRef<{
    resolve: () => void;
    reject: (e: unknown) => void;
    // P58c: the sign choice parked alongside the message (forwarded to
    // doCommitAndPush once the set-upstream dialog is answered).
    sign: boolean | null;
    // P59a: the "Skip hooks" choice parked alongside the message.
    skipHooks: boolean;
  } | null>(null);
  // P59a: the shared hook gate — parks a commit/amend/merge behind the
  // HookOutputDialog when a git hook blocks it, with a "Commit anyway" retry.
  const hookGate = useHookGate();
  // P37b: force-push-with-lease confirm gate (targets the current branch).
  const [pendingForcePush, setPendingForcePush] = useState(false);
  // P28: pending "Discard hunk" confirmation (unstaged diffs only).
  const [pendingHunkDiscard, setPendingHunkDiscard] = useState<{
    path: string;
    origPath: string | null;
    hunkIndex: number;
  } | null>(null);
  // P45: pending "Discard line(s)" confirmation (unstaged diffs only). Stores the
  // selection verbatim — arbitrary lines cannot be re-derived from a hunk index.
  const [pendingLineDiscard, setPendingLineDiscard] = useState<{
    path: string;
    origPath: string | null;
    selection: LineSelection[];
  } | null>(null);
  // P20: amend affordance. `amend` toggles the commit box into amend mode;
  // `amendMessage` holds HEAD's message fetched once on toggle-on (prefill).
  const [amend, setAmend] = useState(false);
  const [amendMessage, setAmendMessage] = useState<string | null>(null);
  // P11 §1.4: "Create branch here" target commit → drives the PromptDialog.
  const [pendingCreateBranch, setPendingCreateBranch] = useState<{ oid: string } | null>(null);
  // P60a: "Rename…" a local branch → drives the shared PromptDialog (prefilled).
  const [pendingRenameBranch, setPendingRenameBranch] = useState<{ name: string } | null>(null);
  // P60d: submodule add (url + path) / deinit / remove dialog state.
  const [pendingAddSubmodule, setPendingAddSubmodule] = useState(false);
  const [pendingDeinitSubmodule, setPendingDeinitSubmodule] = useState<string | null>(null);
  const [pendingRemoveSubmodule, setPendingRemoveSubmodule] = useState<string | null>(null);
  // P60b: a non-fast-forward pull → drives NonFfPullDialog (Merge / Rebase).
  const [pendingNonFfPull, setPendingNonFfPull] = useState<NonFfPullInfo | null>(null);
  // P60c: one-click undo. The toolbar Undo button describes the last op
  // (read-only) into this plan; the UndoDialog confirms, then reuses resetBranch.
  const [pendingUndo, setPendingUndo] = useState<UndoPlan | null>(null);
  // P39b: two-click bisect start. Holds the oid marked BAD (via the commit menu)
  // while the user picks an older known-GOOD commit; cleared on start / cancel.
  const [pendingBisectBad, setPendingBisectBad] = useState<string | null>(null);
  // P47d: cherry-pick message dialog. `handleCherrypick` opens this prefilled
  // with the source commit's full message (fetched via getCommitDiff); confirm
  // runs the pick with the edited message. `loading` gates the prefill fetch.
  const [pendingCherrypick, setPendingCherrypick] = useState<{
    oid: string;
    initialMessage: string;
    loading: boolean;
  } | null>(null);
  // P22 §7.1: tag + remote management dialog state.
  const [pendingCreateTag, setPendingCreateTag] = useState<{ oid: string } | null>(null);
  const [pendingDeleteTag, setPendingDeleteTag] = useState<string | null>(null);
  const [pendingAddRemote, setPendingAddRemote] = useState<boolean>(false);
  const [pendingRenameRemote, setPendingRenameRemote] = useState<{ name: string } | null>(null);
  const [pendingEditUrl, setPendingEditUrl] = useState<{ name: string; url: string } | null>(null);
  const [pendingRemoveRemote, setPendingRemoveRemote] = useState<string | null>(null);
  // P25d: B4 stale-branch cleanup dialog (opened from the Branches header).
  const [staleCleanupOpen, setStaleCleanupOpen] = useState(false);
  // P27 §6.5/§6.6: worktree dialogs — create (branch picker), remove confirm
  // (names the directory to delete), lock reason prompt.
  const [newWorktreeOpen, setNewWorktreeOpen] = useState(false);
  // P28 §7: "✨ What changed…" digest range picker (opened from the toolbar).
  const [whatChangedOpen, setWhatChangedOpen] = useState(false);
  // P56b §6: "✨ Release notes…" changelog range picker (opened from the palette).
  const [changelogOpen, setChangelogOpen] = useState(false);
  // P55c: NL → safe-git-op. `askOpen` = the one-line natural-language input;
  // `askBusy` gates it while the READ-ONLY planner runs. `pendingProposedOp` is
  // the resolved proposal shown in ProposedOpDialog — NOTHING mutates until its
  // Confirm; `opDispatching` gates that dialog while the confirmed op runs.
  // `planReqId` is a last-wins guard (mirrors aiPanelReqId) so a slow/superseded
  // or cancelled plan reply is dropped.
  const [askOpen, setAskOpen] = useState(false);
  const [askBusy, setAskBusy] = useState(false);
  const [pendingProposedOp, setPendingProposedOp] = useState<ProposedOperation | null>(null);
  const [opDispatching, setOpDispatching] = useState(false);
  const planReqId = useRef(0);
  const [pendingWorktreeRemove, setPendingWorktreeRemove] = useState<{
    name: string;
    absPath: string;
  } | null>(null);
  const [pendingWorktreeLock, setPendingWorktreeLock] = useState<string | null>(null);
  // P31 §7: the worktree × AI-context matrix (opened from the worktree menu).
  const [worktreeContextOpen, setWorktreeContextOpen] = useState(false);
  // P23b: interactive-rebase plan editor. `rebasePlan` holds the seeded plan +
  // display metadata; `rebasePlanError` shows a failed Start's error in-dialog.
  const [rebasePlan, setRebasePlan] = useState<{
    ontoOid: string;
    ontoLabel: string;
    initialTodos: RebaseTodoOp[];
    summaries: Record<string, string>;
  } | null>(null);
  const [rebasePlanError, setRebasePlanError] = useState<string | null>(null);
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
  const dialogOpen =
    pendingDeleteBranch !== null ||
    pendingRebase !== null ||
    pendingDeleteRemote !== null ||
    pendingDropStash !== null ||
    pendingReservedStash !== null ||
    pendingReset !== null ||
    pendingDiscard !== null ||
    pendingDiscardForce !== null ||
    pendingHunkDiscard !== null ||
    pendingLineDiscard !== null ||
    pendingCreateBranch !== null ||
    pendingRenameBranch !== null ||
    pendingAddSubmodule ||
    pendingDeinitSubmodule !== null ||
    pendingRemoveSubmodule !== null ||
    pendingNonFfPull !== null ||
    pendingUndo !== null ||
    pendingCherrypick !== null ||
    pendingCreateTag !== null ||
    pendingDeleteTag !== null ||
    pendingAddRemote ||
    pendingRenameRemote !== null ||
    pendingEditUrl !== null ||
    pendingRemoveRemote !== null ||
    staleCleanupOpen ||
    newWorktreeOpen ||
    whatChangedOpen ||
    changelogOpen ||
    askOpen ||
    pendingProposedOp !== null ||
    pendingWorktreeRemove !== null ||
    pendingWorktreeLock !== null ||
    worktreeContextOpen ||
    hookGate.pendingHook !== null ||
    rebasePlan !== null;

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

  const [commitDiff, setCommitDiff] = useState<CommitDiff | null>(null);
  const [commitDiffLoading, setCommitDiffLoading] = useState(false);
  const [commitDiffError, setCommitDiffError] = useState<string | null>(null);
  const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null);
  // P17c: File vs Diff view for the center-pane diff overlay. Drives the
  // `fullContext` arg of the primary overlay fetchers; read through a ref by the
  // stable `refetchStatus` callback so toggling never re-creates it.
  const [diffViewMode, setDiffViewMode] = useState<'diff' | 'file' | 'split'>('diff');
  const diffViewModeRef = useRef(diffViewMode);
  diffViewModeRef.current = diffViewMode;
  // P61a: "Highlight changes" (word-level intraline emphasis) for the overlay
  // diff. Drives the `intraline` arg of every overlay fetch; read through a ref
  // by the stable refetch callbacks so toggling never re-creates them.
  const [intraline, setIntraline] = useState(false);
  const intralineRef = useRef(intraline);
  intralineRef.current = intraline;
  // Bug fix: the "Changes" list view mode (tree vs flat), read through a ref by
  // handleStage so the auto-advance target is computed in the SAME order the UI
  // renders. Threaded via ref so toggling never re-creates the stage handler.
  const listViewRef = useRef(listView);
  listViewRef.current = listView;
  // P61b: image-diff data for the open overlay slot when its path is an image
  // (D4). Fetched in parallel with the text slot (getWorkdirFileDiff still runs
  // and returns a cheap binary FileDiff); DiffOverlay renders DiffImageView from
  // this instead of the text diff. `imageDiffReqId` guards against races.
  const [imageDiff, setImageDiff] = useState<ImageDiff | null>(null);
  const [imageDiffLoading, setImageDiffLoading] = useState(false);
  const [imageDiffError, setImageDiffError] = useState<string | null>(null);
  const imageDiffReqId = useRef(0);
  // Last image path fetched: keep the previous image dimmed during a same-file
  // refresh, but clear it when a DIFFERENT image opens (no wrong image under the
  // new header).
  const imageDiffPathRef = useRef<string | null>(null);

  // P5 §5.2: graph right-click context menu (position + prebuilt items).
  const [menu, setMenu] = useState<{ x: number; y: number; items: ContextMenuItem[] } | null>(
    null,
  );

  // P5 §5.3: Compare right-panel mode (HEAD → right-clicked commit). Mirrors the
  // commitDiff cluster; `compare.oid` is a full oid so it survives refetches.
  const [compare, setCompare] = useState<{ oid: string } | null>(null);
  const [compareData, setCompareData] = useState<CompareDiff | null>(null);
  const [compareLoading, setCompareLoading] = useState(false);
  const [compareError, setCompareError] = useState<string | null>(null);
  const compareReqId = useRef(0);

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
  const stashesReqId = useRef(0);
  const submodulesReqId = useRef(0);
  const worktreesReqId = useRef(0);
  const remotesReqId = useRef(0);
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
    () => ({
      showSha: graphPrefs.showSha,
      showAuthor: graphPrefs.showAuthor,
      showDate: graphPrefs.showDate,
      dateBasis: graphPrefs.dateBasis,
      showAheadBehind: graphPrefs.showAheadBehind,
      branchStats,
      showSignatureBadge: graphPrefs.showSignatureBadge,
      showPrBadge: graphPrefs.showPrBadge && !graphPrefs.compact,
      showCiStatus: graphPrefs.showCiStatus && !graphPrefs.compact,
      prByBranch: forgeSignals.prByBranch,
      ciBySha: forgeSignals.ciBySha,
    }),
    [graphPrefs, branchStats, forgeSignals.prByBranch, forgeSignals.ciBySha],
  );

  // P63: right-pane PR navigation request — a graph PR-badge click sets the
  // 'prs' tab and bumps `seq` so PrPanel opens (or re-opens) that PR's detail.
  const [prNav, setPrNav] = useState<PrNavRequest | null>(null);
  const onOpenPr = useCallback((n: number) => {
    setRightPaneTab('prs');
    setPrNav((prev) => ({ number: n, seq: (prev?.seq ?? 0) + 1 }));
  }, []);

  // P58c: effective signing config for the commit-box toggle/indicator. Read
  // once per repo (and on manual Refresh); a read failure just hides the toggle.
  const [signingStatus, setSigningStatus] = useState<SigningStatus | null>(null);
  const refetchSigningStatus = useCallback(async () => {
    try {
      setSigningStatus(await ipc.signingStatus(repoId));
    } catch {
      setSigningStatus(null); // non-critical read — hide the toggle, follow config
    }
  }, [repoId]);
  useEffect(() => {
    void refetchSigningStatus();
  }, [refetchSigningStatus]);

  // Latest compare target read by refetchCompare without widening effect deps.
  const compareRef = useRef(compare);
  compareRef.current = compare;
  // Commit whose diff/panel is currently loaded — lets the selection effect skip
  // a reset+refetch when the selected OID is unchanged (tab switch / watcher tick
  // that only shifts the row index).
  const commitDiffKeyRef = useRef<string | null>(null);

  // Head: prefer the freshly re-opened RepoInfo, fall back to the branches
  // snapshot (available before the first refreshAll, §5.1).
  const head: HeadInfo | null = repo?.head ?? branches?.head ?? null;
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

  const overlayMeta: DiffOverlayMeta | null = useMemo(() => {
    if (diffSlot === null) return null;
    const key = diffSlot.key;
    if (key.startsWith('conflict:')) {
      return {
        path: key.slice('conflict:'.length),
        origPath: null,
        status: 'conflicted',
        kind: 'conflict',
      };
    }
    // P13 §8.3: AI proposal review — reuses the conflict editor (seeded with the
    // markerless proposed body carried on diffSlot.conflict).
    if (key.startsWith('ai-proposal:')) {
      return {
        path: key.slice('ai-proposal:'.length),
        origPath: null,
        status: 'conflicted',
        kind: 'aiProposal',
      };
    }
    const sep = key.indexOf(':');
    const section = key.slice(0, sep) as WorkdirSection;
    const path = key.slice(sep + 1);
    const entry = status?.[section].find((e) => e.path === path) ?? null;
    return {
      path,
      origPath: entry?.origPath ?? null,
      status: entry?.status ?? null,
      kind: section,
    };
  }, [diffSlot, status]);

  // Latest overlay meta read by the partial-staging handlers + the view-mode
  // toggle without widening their (stable) callback deps.
  const overlayMetaRef = useRef(overlayMeta);
  overlayMetaRef.current = overlayMeta;

  // P61b: when the open overlay slot is an image (D4), fetch getImageDiff for
  // the current context and hand it to DiffOverlay. The overlay only ever serves
  // workdir kinds (staged/unstaged/untracked) — commit/compare per-file diffs
  // live in DiffBrowser — so the request is always a Workdir one. Depends on the
  // status snapshot identity so a repo-changed refresh re-reads the image too
  // (mirrors how the text slot refetches on status change). Non-image or
  // conflict/proposal slots clear the image state.
  useEffect(() => {
    const meta = overlayMetaRef.current;
    const isWorkdirKind =
      meta !== null &&
      (meta.kind === 'staged' || meta.kind === 'unstaged' || meta.kind === 'untracked');
    if (meta === null || !isWorkdirKind || !isImagePath(meta.path)) {
      imageDiffReqId.current += 1;
      imageDiffPathRef.current = null;
      setImageDiff(null);
      setImageDiffLoading(false);
      setImageDiffError(null);
      return;
    }
    const request: ImageDiffRequest = {
      kind: 'workdir',
      path: meta.path,
      origPath: meta.origPath,
      staged: meta.kind === 'staged',
    };
    const id = ++imageDiffReqId.current;
    // A different image than the one currently shown -> drop the stale preview.
    if (imageDiffPathRef.current !== meta.path) setImageDiff(null);
    imageDiffPathRef.current = meta.path;
    setImageDiffLoading(true);
    setImageDiffError(null);
    void ipc.getImageDiff(repoId, request).then(
      (d) => {
        if (id !== imageDiffReqId.current) return;
        setImageDiff(d);
        setImageDiffLoading(false);
      },
      (e) => {
        if (id !== imageDiffReqId.current) return;
        setImageDiff(null);
        setImageDiffError(errorMessage(e));
        setImageDiffLoading(false);
      },
    );
    // overlayMeta is read via ref; the primitive deps below capture every change
    // that matters (which file, which section) plus a status-driven refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [repoId, overlayMeta?.path, overlayMeta?.kind, overlayMeta?.origPath, status]);

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
  }, []);

  // P5 §5.3: tear down compare mode. Bumps the req-id so any in-flight fetch is
  // ignored, and collapses an open `compare:` overlay.
  const clearCompare = useCallback(() => {
    compareReqId.current += 1;
    setCompare(null);
    setCompareData(null);
    setCompareLoading(false);
    setCompareError(null);
    if (diffSlotRef.current?.key.startsWith('compare:') === true) {
      collapseDiffSlot();
    }
    // P11g-rev §4.7: the compare DiffBrowser is now derived from
    // compare/compareData, so setting compare=null (above) closes it
    // automatically — no explicit browser teardown needed here.
  }, [collapseDiffSlot]);

  // P5 §5.3 refresh coexistence: re-fetch the active comparison after a repo
  // change (HEAD may have moved). `compare.oid` is a full oid — no row remap. A
  // `git`-error rejection means the compared commit is gone -> clear + inform.
  const refetchCompare = useCallback(async () => {
    const target = compareRef.current;
    if (target === null) return;
    const id = ++compareReqId.current;
    try {
      const cd = await ipc.compareWithHead(repoId, target.oid);
      if (id !== compareReqId.current) return;
      setCompareData(cd);
      setCompareLoading(false);
      setCompareError(null);
    } catch (e) {
      if (id !== compareReqId.current) return;
      // Only a `git`-kind rejection means the compared commit is genuinely gone
      // (contract above). Transient failures (io/network/other) keep compare
      // mode active and surface via the inline compare error state instead.
      if (isAppError(e) && e.kind === 'git') {
        clearCompare();
        pushToast('info', 'Compared commit is no longer in this repository');
      } else {
        setCompareLoading(false);
        setCompareError(errorMessage(e));
      }
    }
  }, [repoId, clearCompare, pushToast]);

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
        !slot.key.startsWith('ai-proposal:')
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
  }, [repoId, fetchDiffSlot, collapseDiffSlot, reportStatusError]);

  const clearStatus = useCallback(() => {
    statusReqId.current += 1;
    setStatus(null);
    setStatusError(null);
    setStatusLoading(false);
    collapseDiffSlot();
  }, [collapseDiffSlot]);

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
      { setGraph, setGraphEdgeIndex, setGraphTotal, setSelectedIndex },
      (e) => {
        if (id === graphReqId.current) setGraphError(errorMessage(e));
      },
    );
    setGraphLoading(true);
    try {
      await ipc.streamGraph(repoId, (chunk) => {
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
  }, [repoId]);

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

  const refetchStashes = useCallback(async () => {
    const id = ++stashesReqId.current;
    try {
      const list = await ipc.listStashes(repoId);
      if (id !== stashesReqId.current) return;
      setStashes(list);
    } catch {
      if (id !== stashesReqId.current) return;
      // Non-fatal: stashes are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearStashes = useCallback(() => {
    stashesReqId.current += 1;
    setStashes([]);
  }, []);

  const refetchSubmodules = useCallback(async () => {
    const id = ++submodulesReqId.current;
    try {
      const list = await ipc.listSubmodules(repoId);
      if (id !== submodulesReqId.current) return;
      setSubmodules(list);
    } catch {
      if (id !== submodulesReqId.current) return;
      // Non-fatal: submodules are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearSubmodules = useCallback(() => {
    submodulesReqId.current += 1;
    setSubmodules([]);
  }, []);

  const refetchWorktrees = useCallback(async () => {
    const id = ++worktreesReqId.current;
    try {
      const list = await ipc.listWorktrees(repoId);
      if (id !== worktreesReqId.current) return;
      setWorktrees(list);
    } catch {
      if (id !== worktreesReqId.current) return;
      // Non-fatal: worktrees are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearWorktrees = useCallback(() => {
    worktreesReqId.current += 1;
    setWorktrees([]);
  }, []);

  const refetchRemotes = useCallback(async () => {
    const id = ++remotesReqId.current;
    try {
      const list = await ipc.listRemotes(repoId);
      if (id !== remotesReqId.current) return;
      setRemotes(list);
    } catch {
      if (id !== remotesReqId.current) return;
      // Non-fatal: remotes are a secondary surface; keep the last-known list.
    }
  }, [repoId]);

  const clearRemotes = useCallback(() => {
    remotesReqId.current += 1;
    setRemotes([]);
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

  /** Composite post-op refresh (P1 §4.6): re-openRepo (refreshes header HEAD +
   *  self-heals the watcher) + refetch status/graph/branches/opstate. Never
   *  throws — failures surface as a sticky error toast. */
  const refreshAll = useCallback(async (): Promise<void> => {
    try {
      const { info } = await ipc.openRepo(repoPath);
      setRepo(info);
      if (isUsableRepo(info)) {
        await Promise.all([
          refetchStatus(),
          refetchGraph(),
          refetchBranches(),
          refetchStashes(),
          refetchSubmodules(),
          refetchWorktrees(),
          refetchRemotes(),
          refetchOpState(),
          refetchCompare(),
        ]);
      } else {
        clearStatus();
        clearGraph();
        clearBranches();
        clearStashes();
        clearSubmodules();
        clearWorktrees();
        clearRemotes();
        clearOpState();
        clearCompare();
        // P23d: drop any blame/history overlay + invalidate in-flight fetches so
        // a stale overlay can't linger over the now-empty pane.
        blameReqId.current += 1;
        setBlame(null);
        historyReqId.current += 1;
        setHistory(null);
      }
    } catch (e) {
      pushToast('error', `Refresh failed: ${errorMessage(e)}`);
    }
  }, [
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
    clearStatus,
    clearGraph,
    clearBranches,
    clearStashes,
    clearSubmodules,
    clearWorktrees,
    clearRemotes,
    clearOpState,
    clearCompare,
    pushToast,
  ]);

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
  const refreshAllRef = useRef(refreshAll);
  refreshAllRef.current = refreshAll;
  const activeFlipRef = useRef(false);
  useEffect(() => {
    if (!activeFlipRef.current) {
      activeFlipRef.current = true;
      return;
    }
    if (active) void refreshAllRef.current();
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

  // repo-changed subscription: filter to THIS repo; refetch regardless of active
  // so a background tab stays fresh when its watcher fires (§7).
  useEffect(() => {
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    const subscribe = async () => {
      const off = await ipc.onRepoChanged((p) => {
        if (p.repoId !== repoId) return;
        void refetchStatus();
        void refetchGraph();
        void refetchBranches();
        void refetchStashes();
        void refetchSubmodules();
        void refetchWorktrees();
        void refetchRemotes();
        void refetchOpState();
        void refetchCompare();
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    };
    // Subscription loss = degraded live refresh only (manual refresh + focus
    // rescan still work) — log, don't crash.
    void subscribe().catch((e: unknown) => {
      console.error('repo-changed subscription failed', e);
    });
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [
    repoId,
    refetchStatus,
    refetchGraph,
    refetchBranches,
    refetchStashes,
    refetchSubmodules,
    refetchWorktrees,
    refetchRemotes,
    refetchOpState,
    refetchCompare,
  ]);

  // Window-focus rescan: ACTIVE tab only (the visible tab is the one the user
  // just returned to; background tabs self-heal on activation, §7).
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    const subscribe = async () => {
      const off = await ipc.onWindowFocus(() => {
        void refetchStatus();
        void refetchGraph();
        void refetchBranches();
        void refetchStashes();
        void refetchSubmodules();
        void refetchWorktrees();
        void refetchRemotes();
        void refetchOpState();
        void refetchCompare();
        forgeSignals.refresh('focus'); // P63: TTL-guarded (not forced)
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
  }, [
    active,
    refetchStatus,
    refetchGraph,
    refetchBranches,
    refetchStashes,
    refetchSubmodules,
    refetchWorktrees,
    refetchRemotes,
    refetchOpState,
    refetchCompare,
    forgeSignals.refresh,
  ]);

  // P63: a new graph layout identity (post fetch/pull/branch-op) may carry new
  // branch tips → TTL-guarded forge-signal refresh so their badges appear. Fires
  // on mount too (graph null→value); runFetch bails while the layout is null.
  useEffect(() => {
    forgeSignals.refresh('graph');
  }, [graph, forgeSignals.refresh]);

  // P30 §6: the P11e frontend auto-fetch timer is GONE — auto-fetch now runs
  // in the Rust scheduler for ALL open repos (scheduler.rs); data refresh
  // arrives via the emitted `repo-changed`. This block only renders status
  // (D11 readout) + toasts — it must NOT double-refresh.
  useEffect(() => {
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];
    // Initial snapshot on mount (D11).
    void ipc
      .getJobStatus(repoId)
      .then((list) => {
        if (!cancelled) setJobStatus(list);
      })
      .catch(() => {
        // Non-fatal: the readout simply stays hidden until the first event.
      });
    const subscribe = async () => {
      const off = await ipc.onJobStatusChanged((p) => {
        if (p.repoId !== repoId) return;
        setJobStatus((prev) => {
          // Upsert: the mount snapshot may predate the user enabling the job
          // (or may have failed) — a run event implies the job is enabled.
          const updated = {
            job: p.job,
            enabled: true,
            lastRunMs: p.tsMs,
            lastOutcome: p.outcome,
            lastError: p.error ?? null,
            consecutiveFailures: p.consecutiveFailures,
            inBackoff: p.inBackoff,
            nextRunMs: p.nextRunMs,
          };
          return prev.some((s) => s.job === p.job)
            ? prev.map((s) => (s.job === p.job ? { ...s, ...updated } : s))
            : [...prev, updated];
        });
        // SINGLE toast on the 2→3 failure transition (D6) — individual
        // background failures stay silent (D9).
        if (p.enteredBackoff) {
          pushToast('warning', 'Auto-fetch failing — backing off');
        }
        // §6.2: the quiet "Fetched N refs" success toast (data refresh itself
        // arrives via the scheduler's repo-changed emit).
        if (
          p.job === 'autoFetch' &&
          p.outcome === 'success' &&
          p.updatedRefs !== undefined &&
          p.updatedRefs > 0
        ) {
          pushToast('info', `Fetched ${p.updatedRefs} ref${p.updatedRefs === 1 ? '' : 's'}`);
        }
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    };
    // Subscription loss = degraded job-status readout only — log, don't crash.
    void subscribe().catch((e: unknown) => {
      console.error('job-status subscription failed', e);
    });
    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [repoId, pushToast]);

  // Keep the relative-time readout fresh (30 s granularity is plenty).
  useEffect(() => {
    const id = window.setInterval(() => setJobNow(Date.now()), 30_000);
    return () => window.clearInterval(id);
  }, []);

  // Manual refresh (button + Ctrl+R/F5). P58c: also drop the signature-verify
  // cache (keyring / allowedSigners may have changed — OQ8) and re-read the
  // signing config so the commit-box toggle reflects a fresh commit.gpgsign.
  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await refreshAll();
      verification.refresh();
      forgeSignals.refresh('manual', true); // P63: forced (bypass TTL)
      void refetchSigningStatus();
    } finally {
      setRefreshing(false);
    }
  }, [refreshing, refreshAll, verification.refresh, forgeSignals.refresh, refetchSigningStatus]);
  const headBranch = branches?.local.find((b) => b.isHead) ?? null;

  // P58c: the selected commit's signature verdict for the CommitPanel line —
  // reuses the shared verify cache (single source; no extra IPC). null when
  // nothing is selected, not yet verified, or the badge is disabled.
  // (`selectedOid` is derived once above, by the scope-reset effect.)
  const commitSignature =
    selectedOid !== null ? (verification.detailsFor(selectedOid) ?? null) : null;

  const { handleFetch, handlePull, pushCurrentBranch, handlePush, handleForcePush, doForcePush } =
    useRemoteOps({
      repoId,
      pushToast,
      setMutating,
      refreshAll,
      refetchBranches,
      refetchGraph,
      setRemoteOp,
      setPendingForcePush,
      setPendingNonFfPull,
      runWithHookGate: hookGate.runWithHookGate,
    });

  // P63: fetch/pull success may land new tips or new CI verdicts → FORCE a
  // forge-signal refresh (bypass TTL) after they complete. Both internally catch
  // + toast, so they resolve normally (the `.then` runs on completion).
  const onFetch = useCallback(() => {
    void handleFetch().then(() => forgeSignals.refresh('remote', true));
  }, [handleFetch, forgeSignals.refresh]);
  const onPull = useCallback(() => {
    void handlePull().then(() => forgeSignals.refresh('remote', true));
  }, [handlePull, forgeSignals.refresh]);

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
    refetchStatus,
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
    refetchBranches,
    refetchGraph,
    branches,
    setBranchesError,
    setPendingCreateBranch,
    setPendingRenameBranch,
  });

  const {
    handleMergeBranch,
    handleResolveConflict,
    handleResolveConflictText,
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
    applyResolution: handleResolveConflictText,
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
    refetchGraph,
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
    refetchSubmodules,
    refetchStatus,
    refetchGraph,
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
    handleAddRemote,
    handleRemoveRemote,
    handleRenameRemote,
    handleSetRemoteUrl,
  } = useTagRemoteActions({
    repoId,
    pushToast,
    setMutating,
    refetchBranches,
    refetchGraph,
    refetchRemotes,
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
    setDiffViewMode,
    setIntraline,
    setPendingHunkDiscard,
    setPendingLineDiscard,
    fetchDiffSlot,
    refetchStatus,
    reportStatusError,
  });

  // P15b: run an explain/review analysis of a diff target and show the prose in
  // the AiOutputPanel. Read-only — writes nothing. Guarded by a req-id so a slow
  // response can't clobber a newer request or a closed panel.
  const runAnalyze = useCallback(
    (target: AiDiffTarget, mode: AiAnalysisMode, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiAnalyzeDiff(repoId, target, mode).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P57c: answer a natural-language history question grounded in the retrieved
  // commits' real diffs, rendering the prose in the shared AiOutputPanel. Shares
  // runAnalyze's last-wins req-id guard so a slow/superseded response can't
  // clobber a newer request or a closed panel. `runHistoryAnswer` is handed to
  // useHistorySearch as its `runAiAnswer` route.
  const runHistoryAnswer = useCallback(
    (question: string, topK: number) => {
      const title = `History: "${question}"`;
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiSearchHistory(repoId, question, topK).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P15c: summarize the commits/diff unique to `target` vs `base` and show the
  // prose in the AiOutputPanel. Read-only — writes nothing. Shares the same
  // req-id guard as runAnalyze so a slow response can't clobber a newer request
  // or a closed panel.
  const runSummarize = useCallback(
    (base: string, target: string) => {
      const title = `Summary: ${base} → ${target}`;
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiSummarizeRange(repoId, base, target).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P28 §7: digest "what changed" over a range and show the prose in the
  // AiOutputPanel. Read-only — writes nothing. Shares the same req-id guard as
  // runAnalyze so a slow response can't clobber a newer request or a closed
  // panel. `title` is range-derived, built by WhatChangedDialog.
  const runDigest = useCallback(
    (range: AiDigestRange, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiDigest(repoId, range).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P56b §6: generate grouped release notes for a tag/ref range and show the
  // Markdown in the AiOutputPanel (editable). Read-only — writes nothing. Shares
  // the same req-id guard as runAnalyze so a slow response can't clobber a newer
  // request or a closed panel. The provisional `title` covers the loading state;
  // on success the header becomes `Release notes: <fromRef>..<toRef>` from the
  // RESOLVED range (e.g. the previous-tag name for sinceLastTag).
  const runChangelog = useCallback(
    (range: ChangelogRange, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null, editable: true });
      ipc.aiChangelog(repoId, range).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({
            title: `Release notes: ${res.fromRef}..${res.toRef}`,
            text: res.text,
            loading: false,
            error: null,
            costUsd: res.costUsd,
            editable: true,
          });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({
            title,
            text: null,
            loading: false,
            error: errorMessage(e),
            costUsd: null,
            editable: true,
          });
        },
      );
    },
    [repoId],
  );

  // P56b: open the general "Release notes…" range picker (palette entry). Stable
  // so the palette-action useMemo doesn't rebuild each render.
  const openChangelog = useCallback(() => setChangelogOpen(true), []);

  // P53a: blame-why — explain WHY a line exists and show the prose in the
  // AiOutputPanel. Read-only — writes nothing. Shares the same req-id guard as
  // runAnalyze so a slow response can't clobber a newer request or a closed
  // panel. `atOid` is the blamed version (null => HEAD in v1).
  const runExplainLine = useCallback(
    (path: string, lineNo: number, atOid: string | null, title: string) => {
      const id = ++aiPanelReqId.current;
      setAiPanel({ title, text: null, loading: true, error: null, costUsd: null });
      ipc.aiExplainLine(repoId, path, lineNo, atOid).then(
        (res) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: res.text, loading: false, error: null, costUsd: res.costUsd });
        },
        (e: unknown) => {
          if (id !== aiPanelReqId.current) return;
          setAiPanel({ title, text: null, loading: false, error: errorMessage(e), costUsd: null });
        },
      );
    },
    [repoId],
  );

  // P53a: BlameView "Why?" entry point — blame is always vs HEAD in v1, so
  // atOid is null. Title mirrors the mock/backend grounding label.
  const onBlameExplain = useCallback(
    (path: string, lineNo: number) => {
      runExplainLine(path, lineNo, null, `Why line ${lineNo} of ${path}`);
    },
    [runExplainLine],
  );

  const closeAiPanel = useCallback(() => {
    aiPanelReqId.current += 1;
    setAiPanel(null);
  }, []);

  // P55c: map a natural-language `request` to ONE allowlisted, previewable op via
  // the READ-ONLY planner. Mirrors runAnalyze's last-wins req-id guard so a slow,
  // superseded, or cancelled reply is dropped. On `proposed` → arm the
  // ProposedOpDialog (NOTHING mutates yet); on `unsupported` → a calm info toast;
  // on error (aiUnavailable / aiFailed / …) → the shared error toast. This call
  // is READ-ONLY — it writes nothing and never emits repo-changed.
  const runPlanOperation = useCallback(
    (request: string) => {
      const id = ++planReqId.current;
      setAskBusy(true);
      ipc.aiPlanOperation(repoId, request).then(
        (plan) => {
          if (id !== planReqId.current) return;
          setAskBusy(false);
          setAskOpen(false);
          if (plan.kind === 'proposed') setPendingProposedOp(plan.operation);
          else pushToast('info', plan.reason);
        },
        (e: unknown) => {
          if (id !== planReqId.current) return;
          setAskBusy(false);
          setAskOpen(false);
          pushToast('error', errorMessage(e));
        },
      );
    },
    [repoId, pushToast],
  );

  const openAskBonsai = useCallback(() => {
    // Drop any in-flight/stale plan and clear the input's busy state on open.
    planReqId.current += 1;
    setAskBusy(false);
    setAskOpen(true);
  }, []);

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
  }, [repoId, pushToast]);

  const cancelAskBonsai = useCallback(() => {
    // Cancel drops any in-flight plan (its reply is ignored by the req-id guard).
    planReqId.current += 1;
    setAskBusy(false);
    setAskOpen(false);
  }, []);

  // P55c: the ONLY mutation in the NL pipeline — runs after the user confirms the
  // ProposedOpDialog. Dispatches the RESOLVED op to its EXISTING typed command
  // (safeOpDispatch, §6), then refreshes; a dispatched-command AppError (e.g.
  // checkoutConflict / unmergedBranch) surfaces in the shared error toast. The
  // op may pause into the existing conflict/autostash flow — no new UI here.
  const confirmProposedOp = useCallback(async () => {
    const operation = pendingProposedOp;
    if (operation === null) return;
    setOpDispatching(true);
    setMutating(true);
    try {
      await safeOpDispatch(ipc, repoId, operation.op);
      await refreshAll();
      pushToast('success', operation.preview.title);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setOpDispatching(false);
      setMutating(false);
      setPendingProposedOp(null);
    }
  }, [pendingProposedOp, repoId, refreshAll, pushToast]);

  const cancelProposedOp = useCallback(() => {
    // Ignore cancel while the confirmed op is dispatching (keep the modal up).
    if (!opDispatching) setPendingProposedOp(null);
  }, [opDispatching]);

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
  });

  // P50b: commit search — state hook drives the search bar + graph match rings;
  // next/prev reuse revealCommitByOid (the single-selection reveal path).
  const search = useCommitSearch({ repoId, graph, revealCommitByOid, pushToast });

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

  // P54c: commit composer. The row "Preview" reuses the EXISTING workdir file-
  // diff IPC — resolve the changed file's section from the latest snapshot
  // (unstaged → untracked → staged) and fetch that file's diff (no new path).
  const previewComposerFileDiff = useCallback(
    (path: string): Promise<FileDiff> => {
      const s = statusRef.current;
      let entry: StatusEntry | undefined;
      let staged = false;
      if (s !== null) {
        entry = s.unstaged.find((e) => e.path === path);
        if (entry === undefined) entry = s.untracked.find((e) => e.path === path);
        if (entry === undefined) {
          entry = s.staged.find((e) => e.path === path);
          staged = entry !== undefined;
        }
      }
      if (entry === undefined) {
        return Promise.reject(new Error(`No working-tree diff available for ${path}`));
      }
      return ipc.getWorkdirFileDiff(repoId, entry.path, entry.origPath, staged, false, false);
    },
    [repoId],
  );
  const composer = useCommitComposer({
    repoId,
    refetchStatus,
    refetchGraph,
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
  const searchScopeOptions = useMemo<ComboboxOption[]>(() => {
    const opts: ComboboxOption[] = [{ value: '', label: 'All refs' }];
    for (const b of branches?.local ?? []) opts.push({ value: b.name, label: b.name });
    for (const r of branches?.remote ?? []) opts.push({ value: r.name, label: r.name });
    return opts;
  }, [branches]);

  // P50c: command palette (Ctrl/Cmd-K). usePalette owns open/close; the
  // accelerator + Esc-layering are wired through useWorkspaceKeyboard below. The
  // entry registry is assembled ONLY while open (its tag lookup scans the whole
  // graph) and merges the repo-scoped actions with App's `appCommands`.
  const palette = usePalette({ active });

  // "New branch…" opens the shared create-branch PromptDialog seeded at HEAD (a
  // dialog — never a raw mutation); disabled when detached/unborn or busy.
  const openNewBranch = useCallback(() => {
    if (headBranch !== null) setPendingCreateBranch({ oid: headBranch.tip });
  }, [headBranch]);
  const openNewWorktree = useCallback(() => setNewWorktreeOpen(true), []);
  const openSearchEmpty = useCallback(() => search.openSearch(), [search.openSearch]);

  // Dynamic palette rows: prefill + open the search bar, or jump to a commit by
  // oid prefix — both reuse the non-mutating single-selection reveal path.
  const paletteRunSearch = useCallback((t: string) => search.openSearch(t), [search.openSearch]);
  const paletteJumpToCommit = useCallback(
    (prefix: string) => {
      const g = graphDataRef.current;
      const p = prefix.toLowerCase();
      const node = g?.nodes.find((n) => n.id.startsWith(p));
      if (node !== undefined) revealCommitByOid(node.id);
      else pushToast('info', `No commit matching ${prefix} in the current view`);
    },
    [revealCommitByOid, pushToast],
  );

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
      branches,
      graph,
      revealCommitByOid,
      appCommands,
    });
    // P55c / P56b lead the Action group; P68e's dock rows trail it. Both registries
    // live in `paletteActions.ts` (§E) so this container stays a composition site.
    actions.unshift(...aiDock.paletteEntries.lead);
    actions.push(...aiDock.paletteEntries.trail);
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

  // P5 §5.3: enter Compare mode (HEAD → the right-clicked commit). Read-only,
  // so it is NOT gated on mutating/opActive. Collapses any open non-compare diff
  // overlay first (its key belongs to another mode).
  function handleCompareWithHead(oid: string) {
    setMenu(null);
    fileDiffReqId.current += 1;
    setDiffSlot(null);
    setCompare({ oid });
    setCompareData(null);
    setCompareLoading(true);
    setCompareError(null);
    const id = ++compareReqId.current;
    ipc.compareWithHead(repoId, oid).then(
      (cd) => {
        if (id !== compareReqId.current) return;
        setCompareData(cd);
        setCompareLoading(false);
      },
      (e: unknown) => {
        if (id !== compareReqId.current) return;
        setCompareError(errorMessage(e));
        setCompareLoading(false);
      },
    );
  }

  // P9 §6.4: right-click a sidebar stash row → open the shared context menu.
  function handleStashContextMenu(index: number, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: menus.stashMenuItems(index) });
  }

  // P19 §6.4: right-click a sidebar submodule row → open the shared context
  // menu. Looks up the SubmoduleInfo by name from state.
  function handleSubmoduleContextMenu(name: string, clientX: number, clientY: number) {
    const sub = submodules.find((s) => s.name === name);
    if (sub === undefined) return;
    setMenu({ x: clientX, y: clientY, items: menus.submoduleMenuItems(sub) });
  }

  // P27 §6.4: right-click a sidebar worktree row → open the shared context
  // menu. Looks up the WorktreeInfo by name from state.
  function handleWorktreeContextMenu(name: string, clientX: number, clientY: number) {
    const wt = worktrees.find((w) => w.name === name);
    if (wt === undefined) return;
    setMenu({ x: clientX, y: clientY, items: menus.worktreeMenuItems(wt) });
  }

  function handleTagContextMenu(name: string, clientX: number, clientY: number) {
    // P47 (F3): sidebar tag rows have no cheap oid → pass null (delete/copy/push
    // only; graph tag pills pass the node oid and get the shared commit actions).
    setMenu({ x: clientX, y: clientY, items: menus.tagMenuItems(name, null) });
  }

  function handleRemoteContextMenu(name: string, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: menus.remoteMenuItems(name) });
  }

  function handleGraphContextMenu(target: GraphContextTarget, clientX: number, clientY: number) {
    const items = menus.buildContextItems(target);
    if (items.length === 0) return; // no valid actions → menu does not open
    setMenu({ x: clientX, y: clientY, items });
  }

  // P6 §4.3: right-click a sidebar branch/remote row → open the SAME shared menu
  // at the cursor. Empty items (current branch, missing entry) → no menu.
  function handleSidebarContextMenu(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
    clientX: number,
    clientY: number,
  ) {
    const items = menus.branchMenuItems(name, kind);
    if (items.length === 0) return;
    setMenu({ x: clientX, y: clientY, items });
  }

  // Stable so ContextMenu's dismiss-listener effect doesn't re-arm on every
  // parent re-render while the menu is open (reviewer NIT).
  const closeMenu = useCallback(() => setMenu(null), []);

  // Per-repo keyboard handling (Esc-layering + shortcut effects), active tab only.
  useWorkspaceKeyboard({
    active,
    globalModalOpen,
    collapseDiffSlot,
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
    // It joins here rather than in the line-~444 disjunction because `aiBulk`
    // is declared after that point.
    dialogOpen: dialogOpen || aiBulk.confirm.open,
    abortConfirmOpen,
    selectedIndex,
    graph,
    graphRef,
    onAiActivity: aiDock.focusDock,
    handleRefresh,
    handleFetch: onFetch, // P63: refresh forge signals after fetch
    handlePull: onPull, // P63: refresh forge signals after pull
    handlePush,
  });

  // P37b: force-push needs a normal-push-capable HEAD with a configured upstream.
  const canForcePush = canPullPush && headBranch?.upstream != null;

  // P49b: launch external tools at a filesystem path (repo / worktree /
  // submodule). Never gated by mutating/opActive — launches touch no git state.
  // Failures surface via the shared AppError→toast path; success is silent (the
  // opened window is its own feedback).
  const handleOpenInTerminal = useCallback(
    (path: string) => {
      void ipc.openInTerminal(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  const handleRevealInFileManager = useCallback(
    (path: string) => {
      void ipc.revealInFileManager(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  const handleOpenInEditor = useCallback(
    (path: string) => {
      void ipc.openInEditor(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );

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
    handleApplyStash,
    handlePopStash,
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
  });

  // P39b: short summaries for the bisect banner's first-bad / current oids,
  // resolved from the loaded graph (missing → the banner falls back to shortOid).
  const bisectSummaries: Record<string, string> | undefined = (() => {
    if (opState.kind !== 'bisect') return undefined;
    const map: Record<string, string> = {};
    const nodes = graph?.nodes ?? [];
    for (const oid of [opState.current, opState.firstBad]) {
      if (oid === null) continue;
      const s = nodes.find((n) => n.id === oid)?.summary;
      if (s !== undefined) map[oid] = s;
    }
    return map;
  })();

  // P38 §7.2/§7.3: reflog restore wiring. Both actions arm the SHARED dialogs
  // (create-branch PromptDialog / reset ConfirmDialog) — no new mutation path.
  // Reset is offered only on an attached, born HEAD (same predicate as
  // resetMenuItems); otherwise the view hides the reset actions.
  const reflogCanReset = head !== null && !head.unborn && !head.detached;
  const reflogResetLabel = headBranch?.name ?? 'HEAD';
  const onReflogCreateBranch = useCallback((newOid: string) => {
    reflogRestoreRef.current = true;
    setPendingCreateBranch({ oid: newOid });
  }, []);
  const onReflogReset = useCallback(
    (newOid: string, mode: ResetMode) => {
      reflogRestoreRef.current = true;
      setPendingReset({ oid: newOid, mode });
    },
    [],
  );

  // P11g-rev §4.4: resolve the DiffBrowser source labels + header list. Compare
  // mode AUTO-OPENS once data has loaded (≥1 file); commit mode is EXPLICIT-open
  // (gated on commitBrowserOpen). null → browser not rendered.
  const diffBrowserView = useMemo(() => {
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
    // Commit mode: EXPLICIT-open only.
    if (selectedIndex !== null && graph !== null && commitBrowserOpen && commitDiff !== null) {
      // Mid-stream partial layout: the selected commit's row is not in the
      // streamed window yet -> fall through to null (no browser) until the
      // refetch remap re-points selectedIndex and this memo re-runs.
      const node = graph.nodes[selectedIndex];
      if (node) {
        const oid = node.id;
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
    }
    return null;
  }, [compare, compareData, selectedIndex, graph, commitBrowserOpen, commitDiff, headBranch, clearCompare]);

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
        onPush={() => void handlePush()}
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
      />

      <div className="panes">
        <Sidebar
          data={branches}
          loading={branchesLoading}
          error={branchesError}
          onDismissError={() => setBranchesError(null)}
          busy={mutating}
          opActive={opActive}
          currentBranch={headBranch?.name ?? null}
          onCheckout={(name) => void handleCheckoutBranch(name)}
          onContextMenu={handleSidebarContextMenu}
          onCreateBranch={handleCreateBranch}
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
          remotes={remotes}
          onRemoteContextMenu={handleRemoteContextMenu}
          onAddRemote={() => setPendingAddRemote(true)}
          onCleanupBranches={() => setStaleCleanupOpen(true)}
        />
        <PaneDivider side="sidebar" onResize={onSidebarResize} onResizeEnd={onPaneResizeEnd} />
        <WorkspaceGraphPane
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
          search={search}
          searchScopeOptions={searchScopeOptions}
          historySearch={historySearch}
          diffSlot={diffSlot}
          overlayMeta={overlayMeta}
          collapseDiffSlot={collapseDiffSlot}
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
          onSelectRightPaneTab={setRightPaneTab}
          prDefaultHead={headBranch?.name ?? null}
          prNav={prNav}
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
          onCommit={handleCommit}
          onCommitAndPush={
            headBranch ? (m, sign, skipHooks) => handleCommitAndPush(m, sign, skipHooks) : undefined
          }
          onGenerate={handleGenerateCommitMessage}
          workingDirty={workingDirty}
          onCompose={() => composer.openComposer()}
          onOpenIdentitySettings={onOpenIdentitySettings}
          signingStatus={signingStatus}
          commitSignature={commitSignature}
        />
      </div>

      {/* P68e: `.workspace-host`'s THIRD child (toolbar → .panes → dock), full width on
          purpose; renders null until the first run exists. */}
      <AiActivityPanel {...aiDock.panelProps} />

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
        handleDeleteBranch={(name) => void handleDeleteBranch(name)}
        pendingRebase={pendingRebase}
        setPendingRebase={setPendingRebase}
        handleRebaseBranch={(name) => void handleRebaseBranch(name)}
        pendingDeleteRemote={pendingDeleteRemote}
        setPendingDeleteRemote={setPendingDeleteRemote}
        handleDeleteRemoteTracking={(name) => void handleDeleteRemoteTracking(name)}
        pendingDropStash={pendingDropStash}
        setPendingDropStash={setPendingDropStash}
        handleDropStash={(index) => void handleDropStash(index)}
        pendingReservedStash={pendingReservedStash}
        setPendingReservedStash={setPendingReservedStash}
        handleApplyStashSkipping={(index) => void handleApplyStash(index, true)}
        handlePopStashSkipping={(index) => void handlePopStash(index, true)}
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
        handleCreateBranchHere={(oid, name) => void handleCreateBranchHere(oid, name)}
        pendingRenameBranch={pendingRenameBranch}
        setPendingRenameBranch={setPendingRenameBranch}
        handleRenameBranch={(oldName, newName) => void handleRenameBranch(oldName, newName)}
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
      />
    </>
  );
}
