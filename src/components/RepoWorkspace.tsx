import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { CommitBoxHandle } from './CommitBox';
import type { ContextMenuItem } from './ContextMenu';
import { WorkspaceToolbar } from './WorkspaceToolbar';
import { WorkspaceDialogs } from './WorkspaceDialogs';
import { CherrypickMessageDialog } from './CherrypickMessageDialog';
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
import { ipc } from '../ipc';
import type {
  AiAnalysisMode,
  AiAutonomy,
  AiAvailability,
  AiDiffTarget,
  AiDigestRange,
  BlameLine,
  BranchesSnapshot,
  CommitDiff,
  CompareDiff,
  ConflictEntry,
  FileDiff,
  FileHistoryEntry,
  GraphLayout,
  GraphPrefs,
  HeadInfo,
  JobStatus,
  LineSelection,
  ListView,
  RebaseTodoOp,
  PaneWidths,
  ReflogEntry,
  RemoteInfo,
  RepoInfo,
  RepoOpState,
  ResetMode,
  StashEntry,
  StatusEntry,
  StatusSnapshot,
  SubmoduleInfo,
  Unsubscribe,
  WorktreeInfo,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

import { useRemoteOps } from './repoWorkspace/useRemoteOps';
import { useCommitActions } from './repoWorkspace/useCommitActions';
import { useBranchActions } from './repoWorkspace/useBranchActions';
import { useMergeActions } from './repoWorkspace/useMergeActions';
import { useStashActions } from './repoWorkspace/useStashActions';
import { useSubmoduleActions } from './repoWorkspace/useSubmoduleActions';
import { useWorktreeActions } from './repoWorkspace/useWorktreeActions';
import { useTagRemoteActions } from './repoWorkspace/useTagRemoteActions';
import { useRebaseActions } from './repoWorkspace/useRebaseActions';
import { useCherrypickRevertActions } from './repoWorkspace/useCherrypickRevertActions';
import { useBisectActions } from './repoWorkspace/useBisectActions';
import { useReadOverlays } from './repoWorkspace/useReadOverlays';
import { useWorkspaceKeyboard } from './repoWorkspace/useWorkspaceKeyboard';

export interface RepoWorkspaceProps {
  /** Canonical workdir path (== repoId, P3e §2). */
  repoId: string;
  /** True when this tab is visible (the others are display:none). Gates the
   *  keyboard shortcut + Esc effects, window-focus rescan, GraphCanvas remeasure
   *  and the activation self-heal refresh (§5.1/§7). */
  active: boolean;
  /** App-global display prefs / pane sizing threaded down. */
  listView: ListView;
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
  onSidebarResize(delta: number): void;
  onRightPanelResize(delta: number): void;
  onPaneResizeEnd(): void;
  /** P19 §6.5: open `path` in a new/focused tab (App.openTab). Used by the
   *  submodule "Open in new tab" action; reuses the existing open-repo flow. */
  onOpenRepoPath(path: string): void;
  /** P40b: open Settings → Git config → Identity (commit-error linkage). */
  onOpenIdentitySettings(): void;
}

/** P3e §5.1: the entire per-repo state cluster + handlers + render tree, one
 *  instance per open tab (keyed by repoId in App). Consumes toasts via
 *  ToastContext; receives only app-global prefs + pane callbacks as props. */
export function RepoWorkspace({
  repoId,
  active,
  listView,
  themeVersion,
  paneWidths,
  globalModalOpen,
  graph: graphPrefs,
  metricsVersion,
  aiEnabled,
  aiConflictAutonomy,
  aiConsented,
  aiAvailability,
  onSidebarResize,
  onRightPanelResize,
  onPaneResizeEnd,
  onOpenRepoPath,
  onOpenIdentitySettings,
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

  const [stashes, setStashes] = useState<StashEntry[]>([]);

  const [submodules, setSubmodules] = useState<SubmoduleInfo[]>([]);

  // P27 §6.3: worktrees (main first), refetched alongside submodules.
  const [worktrees, setWorktrees] = useState<WorktreeInfo[]>([]);

  // P22 §7.1: configured remotes (name + fetch URL), refetched alongside branches.
  const [remotes, setRemotes] = useState<RemoteInfo[]>([]);

  const [remoteOp, setRemoteOp] = useState<'fetch' | 'pull' | 'push' | null>(null);

  const [opState, setOpState] = useState<RepoOpState>({ kind: 'none' });
  const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);
  // P13 §8.3: path whose AI resolution is in flight (calls take seconds). Gates
  // the per-row ✨ AI button without freezing the whole panel like `mutating`.
  const [aiResolvingPath, setAiResolvingPath] = useState<string | null>(null);
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
  const commitPushResolver = useRef<{ resolve: () => void; reject: (e: unknown) => void } | null>(
    null,
  );
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
    pendingWorktreeRemove !== null ||
    pendingWorktreeLock !== null ||
    worktreeContextOpen ||
    rebasePlan !== null;

  const [graph, setGraph] = useState<GraphLayout | null>(null);
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
    } catch {
      if (id !== compareReqId.current) return;
      clearCompare();
      pushToast('info', 'Compared commit is no longer in this repository');
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
    const id = ++graphReqId.current;
    // Preserve selection across refetches (activation self-heal, focus rescan,
    // watcher ticks) by commit OID: capture it BEFORE the fetch, remap after.
    const prevSelectedId =
      selectedIndexRef.current != null
        ? (graphDataRef.current?.nodes[selectedIndexRef.current]?.id ?? null)
        : null;
    setGraphLoading(true);
    try {
      const layout = await ipc.getGraph(repoId);
      if (id !== graphReqId.current) return;
      setGraph(layout);
      setGraphError(null);
      if (prevSelectedId !== null) {
        const idx = layout.nodes.findIndex((n) => n.id === prevSelectedId);
        // Found -> remap to its (possibly shifted) row; gone -> clear.
        setSelectedIndex(idx >= 0 ? idx : null);
      } else {
        setSelectedIndex(null);
      }
    } catch (e) {
      if (id !== graphReqId.current) return;
      setGraphError(errorMessage(e));
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
      const oid = graph.nodes[selectedIndex].id;
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
  // source changes (new compare target, or a different commit selected). Compare
  // auto-open then renders at root; commit mode returns to closed.
  useEffect(() => {
    setScope({ kind: 'root' });
    setCommitBrowserOpen(false);
  }, [compare?.oid, selectedIndex]);

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
    void subscribe();
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
      });
      if (cancelled) {
        off();
        return;
      }
      unsubs.push(off);
    };
    void subscribe();
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
  ]);

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
    void subscribe();
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

  // Manual refresh (button + Ctrl+R/F5).
  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await refreshAll();
    } finally {
      setRefreshing(false);
    }
  }, [refreshing, refreshAll]);
  const headBranch = branches?.local.find((b) => b.isHead) ?? null;

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
    });

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
    head,
    headBranch,
    setAmend,
    setAmendMessage,
    pendingCommitPush,
    setPendingCommitPush,
    commitPushResolver,
    setPendingDiscardForce,
  });

  const {
    handleCreateBranch,
    handleCheckoutBranch,
    handleCreateBranchHere,
    handleDeleteBranch,
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
  });

  const {
    handleMergeBranch,
    handleResolveConflict,
    handleResolveConflictText,
    handleAiResolveConflict,
    handleCommitMerge,
    handleAbortMerge,
  } = useMergeActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    aiConflictAutonomy,
    setAiResolvingPath,
    setDiffSlot,
    fileDiffReqId,
  });

  const { handleCreateStash, handleApplyStash, handlePopStash, handleDropStash } = useStashActions({
    repoId,
    pushToast,
    setMutating,
    refreshAll,
    refetchStashes,
    refetchGraph,
    setPendingReservedStash,
  });

  const { handleInitSubmodule, handleUpdateSubmodule, handleSyncSubmodule } = useSubmoduleActions({
    repoId,
    pushToast,
    setMutating,
    refetchSubmodules,
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
  // P17c: switch File/Diff view. When a workdir file diff is open, re-fetch it
  // with the new `fullContext` (File View = one whole-file hunk); the same key
  // keeps the stale content visible during the swap. Conflict/ai-proposal slots
  // are not FileDiffs (they use getConflict), so they need no refetch.
  const handleSetViewMode = useCallback(
    (m: 'diff' | 'file' | 'split') => {
      setDiffViewMode(m);
      const meta = overlayMetaRef.current;
      const slot = diffSlotRef.current;
      if (slot === null || meta === null) return;
      if (meta.kind === 'staged' || meta.kind === 'unstaged' || meta.kind === 'untracked') {
        const staged = meta.kind === 'staged';
        void fetchDiffSlot(slot.key, () =>
          ipc.getWorkdirFileDiff(repoId, meta.path, meta.origPath, staged, m === 'file'),
        );
      }
    },
    [repoId, fetchDiffSlot],
  );

  // P17c: stage/unstage exactly `selection` (already Context-dropped) for the
  // file open in the overlay. Direction + path/origPath come from the current
  // stageable/overlay meta. Guarded by the `mutating` flag like handleStage.
  // refetchStatus re-fetches the matching mode-A workdir slot by path in the new
  // snapshot (honoring the current view mode), so no extra slot fetch is needed;
  // a src/main.rs-style file persists in its section (and may now appear in both
  // staged & unstaged). If the entry leaves its section, refetchStatus collapses.
  const handleStageLines = useCallback(
    async (selection: LineSelection[]) => {
      if (selection.length === 0) return; // empty selection -> skip
      if (mutatingRef.current) return;
      const meta = overlayMetaRef.current;
      const dir = stageableRef.current;
      if (meta === null || dir === null) return;
      setMutating(true);
      try {
        if (dir === 'stage') {
          await ipc.stagePartial(repoId, meta.path, meta.origPath, selection);
        } else {
          await ipc.unstagePartial(repoId, meta.path, meta.origPath, selection);
        }
        await refetchStatus();
      } catch (e) {
        reportStatusError(errorMessage(e));
      } finally {
        setMutating(false);
      }
    },
    [repoId, refetchStatus, reportStatusError],
  );

  // P17c: stage/unstage every add/del line of hunk `hunkIndex` from the open
  // diff (Diff View hunk-header button). Builds the selection then delegates.
  const handleStageHunk = useCallback(
    (hunkIndex: number) => {
      const d = diffSlotRef.current?.diff ?? null;
      const hunk = d?.hunks[hunkIndex];
      if (hunk === undefined) return;
      const selection: LineSelection[] = hunk.lines
        .filter((l) => l.kind === 'add' || l.kind === 'del')
        .map((l) => ({ kind: l.kind, oldNo: l.oldNo, newNo: l.newNo }));
      void handleStageLines(selection);
    },
    [handleStageLines],
  );

  // P28: request a hunk discard — just arms the ConfirmDialog (destructive ops
  // always confirm first). Passed to DiffOverlay only for unstaged tracked
  // diffs (see the render-site gating), so meta here is the unstaged file.
  const handleDiscardHunk = useCallback((hunkIndex: number) => {
    const meta = overlayMetaRef.current;
    if (meta === null) return;
    setPendingHunkDiscard({ path: meta.path, origPath: meta.origPath, hunkIndex });
  }, []);

  // P28: confirmed hunk discard — build the LineSelection from the open diff's
  // hunk (same rule as handleStageHunk) and revert it in the worktree, then
  // refetch like handleStageLines does. Guarded by `mutating`.
  const handleConfirmHunkDiscard = useCallback(
    async (pending: { path: string; origPath: string | null; hunkIndex: number }) => {
      if (mutatingRef.current) return;
      // The slot must still show the file the dialog was armed for.
      if (overlayMetaRef.current?.path !== pending.path) return;
      const d = diffSlotRef.current?.diff ?? null;
      const hunk = d?.hunks[pending.hunkIndex];
      if (hunk === undefined) return; // stale click; diff changed underneath
      const selection: LineSelection[] = hunk.lines
        .filter((l) => l.kind === 'add' || l.kind === 'del')
        .map((l) => ({ kind: l.kind, oldNo: l.oldNo, newNo: l.newNo }));
      if (selection.length === 0) return;
      setMutating(true);
      try {
        await ipc.discardPartial(repoId, pending.path, pending.origPath, selection);
        await refetchStatus();
      } catch (e) {
        reportStatusError(errorMessage(e));
      } finally {
        setMutating(false);
      }
    },
    [repoId, refetchStatus, reportStatusError],
  );

  // P45: request a per-line discard — just arms the ConfirmDialog (destructive
  // ops always confirm first). The selection is captured verbatim because
  // arbitrary lines can't be re-derived after the diff refetches (unlike a hunk
  // index). Passed to DiffOverlay only for unstaged tracked diffs (see gating).
  const handleDiscardLines = useCallback((selection: LineSelection[]) => {
    if (selection.length === 0) return;
    const meta = overlayMetaRef.current;
    if (meta === null) return;
    setPendingLineDiscard({ path: meta.path, origPath: meta.origPath, selection });
  }, []);

  // P45: confirmed per-line discard — revert exactly the stored selection in the
  // worktree, then refetch like handleConfirmHunkDiscard. Guarded by `mutating`;
  // the backend's stale() guard rejects a selection whose coordinates moved.
  const handleConfirmLineDiscard = useCallback(
    async (pending: { path: string; origPath: string | null; selection: LineSelection[] }) => {
      if (mutatingRef.current) return;
      // The slot must still show the file the dialog was armed for.
      if (overlayMetaRef.current?.path !== pending.path) return;
      if (pending.selection.length === 0) return;
      setMutating(true);
      try {
        await ipc.discardPartial(repoId, pending.path, pending.origPath, pending.selection);
        await refetchStatus();
      } catch (e) {
        reportStatusError(errorMessage(e));
      } finally {
        setMutating(false);
      }
    },
    [repoId, refetchStatus, reportStatusError],
  );

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

  const closeAiPanel = useCallback(() => {
    aiPanelReqId.current += 1;
    setAiPanel(null);
  }, []);

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
      ),
    );
  }

  function handleSelectParent(parentOrdinal: number) {
    if (selectedIndex === null || graph === null) return;
    const parentIndex = graph.nodes[selectedIndex].parents[parentOrdinal];
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
    diffSlotRef,
    compareRef,
    setSelectedIndex,
    setCommitBrowserOpen,
    refreshing,
    statusLoading,
    graphLoading,
    mutating,
    canPullPush,
    dialogOpen,
    abortConfirmOpen,
    selectedIndex,
    graph,
    graphRef,
    handleRefresh,
    handleFetch,
    handlePull,
    handlePush,
  });

  // P37b: force-push needs a normal-push-capable HEAD with a configured upstream.
  const canForcePush = canPullPush && headBranch?.upstream != null;

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
    handleMergeBranch,
    setPendingRebase,
    openRebasePlan,
    handleCompareWithHead,
    setPendingDeleteRemote,
    setPendingDeleteBranch,
    handleApplyStash,
    handlePopStash,
    setPendingDropStash,
    handleInitSubmodule,
    handleUpdateSubmodule,
    handleSyncSubmodule,
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
      const oid = graph.nodes[selectedIndex].id;
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
  }, [compare, compareData, selectedIndex, graph, commitBrowserOpen, commitDiff, headBranch, clearCompare]);

  return (
    <>
      <WorkspaceToolbar
        remoteOp={remoteOp}
        refreshing={refreshing}
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
        onFetch={() => void handleFetch()}
        onPull={() => void handlePull()}
        onPush={() => void handlePush()}
        onForcePush={() => handleForcePush()}
        onWhatChanged={() => setWhatChangedOpen(true)}
        onViewHeadReflog={() =>
          reflog && reflog.refName === 'HEAD' ? closeReflog() : void openReflog('HEAD')
        }
        headBorn={head !== null && !head.unborn}
        onRefresh={() => void handleRefresh()}
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
          diffSlot={diffSlot}
          overlayMeta={overlayMeta}
          collapseDiffSlot={collapseDiffSlot}
          onResolveConflictText={handleResolveConflictText}
          mutating={mutating}
          overlayExplain={overlayExplain}
          diffViewMode={diffViewMode}
          onSetViewMode={handleSetViewMode}
          stageable={stageable}
          onStageLines={handleStageLines}
          onStageHunk={handleStageHunk}
          onDiscardHunk={handleDiscardHunk}
          onDiscardLines={handleDiscardLines}
          blame={blame}
          closeBlame={closeBlame}
          revealCommitByOid={revealCommitByOid}
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
          aiResolvingPath={aiResolvingPath}
          aiPanelLoading={aiPanel?.loading === true}
          onStage={(paths) => void handleStage(paths)}
          onUnstage={(paths) => void handleUnstage(paths)}
          onDiscard={(paths) => setPendingDiscard(paths)}
          onDiscardForce={(paths) => requestDiscardForce(paths)}
          onToggleDiff={handleToggleWorkdirDiff}
          onResolveConflict={(path, r) => void handleResolveConflict(path, r)}
          onToggleConflictView={handleToggleConflictView}
          onAiResolve={(path) => void handleAiResolveConflict(path)}
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
          onCommitAndPush={headBranch ? (m) => handleCommitAndPush(m) : undefined}
          onGenerate={handleGenerateCommitMessage}
          onOpenIdentitySettings={onOpenIdentitySettings}
        />
      </div>

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
      />
      <CherrypickMessageDialog
        open={pendingCherrypick !== null}
        oid={pendingCherrypick?.oid ?? ''}
        initialMessage={pendingCherrypick?.initialMessage ?? ''}
        loading={pendingCherrypick?.loading ?? false}
        busy={mutating}
        onConfirm={(message) => {
          const p = pendingCherrypick;
          if (p !== null) void confirmCherrypick(p.oid, message);
        }}
        onCancel={() => setPendingCherrypick(null)}
      />
    </>
  );
}
