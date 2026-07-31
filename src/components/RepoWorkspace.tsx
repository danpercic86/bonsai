import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { AiOutputPanel } from './AiOutputPanel';
import { CommitBox } from './CommitBox';
import type { CommitBoxHandle } from './CommitBox';
import { CommitPanel } from './CommitPanel';
import { ComparePanel } from './ComparePanel';
import { ConfirmDialog } from './ConfirmDialog';
import { PromptDialog } from './PromptDialog';
import { RebasePlanEditor } from './RebasePlanEditor';
import { TagCreateDialog } from './TagCreateDialog';
import { RemoteEditDialog } from './RemoteEditDialog';
import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './ContextMenu';
import {
  BranchIcon,
  CheckoutIcon,
  CompareIcon,
  CopyIcon,
  DeleteIcon,
  MergeIcon,
  RebaseIcon,
  StashApplyIcon,
  StashPopIcon,
  SummarizeIcon,
  TagIcon,
} from './menuIcons';
import { DiffOverlay } from './DiffOverlay';
import type { DiffOverlayMeta } from './DiffOverlay';
import { BlameView } from './BlameView';
import { FileHistoryView } from './FileHistoryView';
import { DiffBrowser } from './DiffBrowser';
import type { DiffScope } from './DiffFileTree';
import { OpBanner } from './OpBanner';
import { PaneDivider } from './PaneDivider';
import { Sidebar } from './Sidebar';
import { StatusPanel } from './StatusPanel';
import type { DiffSlot, WorkdirSection } from './StatusPanel';
import { GraphCanvas } from '../graph/GraphCanvas';
import type { GraphCanvasHandle, GraphContextTarget, WipSummary } from '../graph/GraphCanvas';
import { effectiveMetrics } from '../graph/metrics';
import { ipc } from '../ipc';
import type {
  AiAnalysisMode,
  AiAutonomy,
  AiAvailability,
  AiDiffTarget,
  AiResolveProposal,
  AutoFetchSettings,
  BlameLine,
  BranchInfo,
  BranchesSnapshot,
  CommitDiff,
  CompareDiff,
  ConflictEntry,
  ConflictResolution,
  FileDiff,
  FileHistoryEntry,
  GraphLayout,
  GraphPrefs,
  HeadInfo,
  LineSelection,
  ListView,
  RebaseTodoOp,
  PaneWidths,
  RemoteInfo,
  RepoInfo,
  RepoOpState,
  ResetMode,
  StashEntry,
  StatusEntry,
  StatusSnapshot,
  SubmoduleInfo,
  Unsubscribe,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage, isAppError } from '../utils/errors';
import { hasUnresolvedMarkers } from '../utils/conflictRegions';

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

/** P23d: how many file-history entries to request (backend caps at MAX_HISTORY). */
const MAX_HISTORY_UI = 200;

function isUsableRepo(info: RepoInfo): boolean {
  return info.isRepo && !info.bare;
}

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
  /** P11e §5: auto-fetch preference; drives the active-tab-only interval timer. */
  autoFetch: AutoFetchSettings;
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
  autoFetch,
  aiEnabled,
  aiConflictAutonomy,
  aiConsented,
  aiAvailability,
  onSidebarResize,
  onRightPanelResize,
  onPaneResizeEnd,
  onOpenRepoPath,
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
  const [pendingDeleteRemote, setPendingDeleteRemote] = useState<string | null>(null);
  const [pendingDropStash, setPendingDropStash] = useState<number | null>(null);
  // P20: destructive reset (all three modes confirm; hard warns extra) + discard.
  const [pendingReset, setPendingReset] = useState<{ oid: string; mode: ResetMode } | null>(null);
  const [pendingDiscard, setPendingDiscard] = useState<string[] | null>(null);
  // P20: amend affordance. `amend` toggles the commit box into amend mode;
  // `amendMessage` holds HEAD's message fetched once on toggle-on (prefill).
  const [amend, setAmend] = useState(false);
  const [amendMessage, setAmendMessage] = useState<string | null>(null);
  // P11 §1.4: "Create branch here" target commit → drives the PromptDialog.
  const [pendingCreateBranch, setPendingCreateBranch] = useState<{ oid: string } | null>(null);
  // P22 §7.1: tag + remote management dialog state.
  const [pendingCreateTag, setPendingCreateTag] = useState<{ oid: string } | null>(null);
  const [pendingDeleteTag, setPendingDeleteTag] = useState<string | null>(null);
  const [pendingAddRemote, setPendingAddRemote] = useState<boolean>(false);
  const [pendingRenameRemote, setPendingRenameRemote] = useState<{ name: string } | null>(null);
  const [pendingEditUrl, setPendingEditUrl] = useState<{ name: string; url: string } | null>(null);
  const [pendingRemoveRemote, setPendingRemoveRemote] = useState<string | null>(null);
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
  const blameReqId = useRef(0);
  const historyReqId = useRef(0);
  const blameOpenRef = useRef(false);
  blameOpenRef.current = blame !== null;
  const historyOpenRef = useRef(false);
  historyOpenRef.current = history !== null;
  const dialogOpen =
    pendingDeleteBranch !== null ||
    pendingDeleteRemote !== null ||
    pendingDropStash !== null ||
    pendingReset !== null ||
    pendingDiscard !== null ||
    pendingCreateBranch !== null ||
    pendingCreateTag !== null ||
    pendingDeleteTag !== null ||
    pendingAddRemote ||
    pendingRenameRemote !== null ||
    pendingEditUrl !== null ||
    pendingRemoveRemote !== null ||
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
  const [diffViewMode, setDiffViewMode] = useState<'diff' | 'file'>('diff');
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
  const remotesReqId = useRef(0);
  const commitDiffReqId = useRef(0);
  const fileDiffReqId = useRef(0);
  const opStateReqId = useRef(0);
  const diffSlotRef = useRef<DiffSlot | null>(null);
  diffSlotRef.current = diffSlot;
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
        op.kind === 'merge' || op.kind === 'rebase' ? await ipc.listConflicts(repoId) : [];
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
    refetchRemotes,
    refetchOpState,
    refetchCompare,
    clearStatus,
    clearGraph,
    clearBranches,
    clearStashes,
    clearSubmodules,
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
    refetchRemotes,
    refetchOpState,
    refetchCompare,
  ]);

  // P11e §5: auto-fetch timer — ACTIVE tab only, OFF by default. Gated on
  // `active && autoFetch.enabled`; the interval reschedules only when the tab
  // activation or the settings change (NOT on every mutation — `mutating` is read
  // through `mutatingRef`). A tick skips while a mutation is in flight; otherwise
  // it fetches and, only when refs actually moved, refreshes + shows a quiet info
  // toast. No-ops are silent; errors surface as a quiet warning (never a banner).
  useEffect(() => {
    if (!active || !autoFetch.enabled) return;
    const tick = () => {
      if (mutatingRef.current) return;
      void ipc
        .fetch(repoId)
        .then((res) => {
          const updated = res.remotes.reduce((n, r) => n + r.updatedRefs, 0);
          if (updated > 0) {
            void refreshAllRef.current();
            pushToast('info', `Fetched ${updated} ref${updated === 1 ? '' : 's'}`);
          }
        })
        .catch((e) => pushToast('warning', `Auto-fetch failed: ${errorMessage(e)}`));
    };
    const id = window.setInterval(tick, autoFetch.intervalMinutes * 60000);
    return () => window.clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, autoFetch.enabled, autoFetch.intervalMinutes, repoId]);

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

  async function handleStage(paths: string[]) {
    setMutating(true);
    try {
      await ipc.stage(repoId, paths);
      await refetchStatus();
    } catch (e) {
      reportStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleUnstage(paths: string[]) {
    setMutating(true);
    try {
      await ipc.unstage(repoId, paths);
      await refetchStatus();
    } catch (e) {
      reportStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleCommit(message: string) {
    setMutating(true);
    try {
      await ipc.commit(repoId, message);
      await refreshAll();
    } finally {
      setMutating(false);
    }
  }

  // P20 §2: amend the current tip. Rethrows so CommitBox surfaces
  // configMissing/emptyMessage in its own error banner (like handleCommit's
  // implicit rethrow). On success, clear amend mode + refresh.
  async function handleCommitAmend(message: string) {
    setMutating(true);
    try {
      await ipc.commitAmend(repoId, message);
      setAmend(false);
      setAmendMessage(null);
      await refreshAll();
      pushToast('success', 'Amended last commit');
    } finally {
      setMutating(false);
    }
  }

  // P20 §2.3: toggle amend on/off. Toggling ON fetches HEAD's full message once
  // (reusing getCommitDiff().details.message — no dedicated backend getter) so
  // the box remounts prefilled. Toggling OFF drops back to the normal commit box.
  async function handleToggleAmend(next: boolean) {
    if (!next) {
      setAmend(false);
      setAmendMessage(null);
      return;
    }
    if (head === null || head.unborn) return;
    try {
      const diff = await ipc.getCommitDiff(repoId, head.oid);
      setAmendMessage(diff.details.message);
      setAmend(true);
    } catch (e) {
      pushToast('error', `Could not load the last commit message: ${errorMessage(e)}`);
    }
  }

  // P20 §3: reset the current branch (called after the shared ConfirmDialog).
  async function handleResetBranch(oid: string, mode: ResetMode) {
    setMutating(true);
    try {
      await ipc.resetBranch(repoId, oid, mode);
      await refreshAll();
      const branchLabel = headBranch?.name ?? 'HEAD';
      pushToast('success', `Reset ${branchLabel} to ${shortOid(oid)} (${mode})`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P20 §4: discard unstaged edits to tracked files (called after ConfirmDialog).
  async function handleDiscard(paths: string[]) {
    setMutating(true);
    try {
      await ipc.discardPaths(repoId, paths);
      await refreshAll();
      pushToast('success', `Discarded changes to ${paths.length} file(s)`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P17c: switch File/Diff view. When a workdir file diff is open, re-fetch it
  // with the new `fullContext` (File View = one whole-file hunk); the same key
  // keeps the stale content visible during the swap. Conflict/ai-proposal slots
  // are not FileDiffs (they use getConflict), so they need no refetch.
  const handleSetViewMode = useCallback(
    (m: 'diff' | 'file') => {
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

  // P15a: ask the backend for a proposed commit message from the staged diff.
  // Returns the text for CommitBox to drop into its textarea; errors surface in
  // the box's own error-banner (CommitBox catches). Never commits.
  async function handleGenerateCommitMessage(): Promise<string> {
    const proposal = await ipc.generateCommitMessage(repoId);
    return proposal.message;
  }

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

  async function handleCreateBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.createBranch(repoId, name);
      await refetchBranches();
      void refetchGraph();
    } finally {
      setMutating(false);
    }
  }

  async function handleCheckoutBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.checkoutBranch(repoId, name);
      await refreshAll();
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P11 §1.4: create a local branch at `oid` + check it out, carrying any
  // uncommitted work across via auto-stash. HEAD moves, so refreshAll.
  async function handleCreateBranchHere(oid: string, name: string): Promise<void> {
    setMutating(true);
    try {
      const res = await ipc.createBranchHere(repoId, name, oid);
      await refreshAll();
      if (!res.stashed) {
        pushToast('success', `Created and checked out ${name}`);
      } else if (res.apply?.kind === 'applied') {
        pushToast('success', `Created ${name} and carried your changes over`);
      } else {
        pushToast(
          'warning',
          `Created ${name}; your changes were carried over with conflicts — resolve them in the status panel`,
        );
      }
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
      setPendingCreateBranch(null);
    }
  }

  async function handleDeleteBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.deleteBranch(repoId, name);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P6 §4.4: GitKraken-style remote checkout — create/reuse a local tracking
  // branch and switch to it (HEAD moves, so refreshAll like handleCheckoutBranch).
  async function handleCheckoutRemote(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.checkoutRemoteBranch(repoId, name);
      await refreshAll();
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P6 §4.4: delete the LOCAL remote-tracking ref only (does not touch the
  // server); refetch branches + graph like handleDeleteBranch.
  async function handleDeleteRemoteTracking(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.deleteRemoteBranch(repoId, name);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- M6: remote operations -----
  function beginRemoteOp(op: 'fetch' | 'pull' | 'push') {
    setMutating(true);
    setRemoteOp(op);
  }

  function endRemoteOp() {
    setMutating(false);
    setRemoteOp(null);
  }

  async function handleFetch() {
    beginRemoteOp('fetch');
    try {
      const res = await ipc.fetch(repoId);
      const n = res.remotes.length;
      const k = res.remotes.reduce((sum, r) => sum + r.updatedRefs, 0);
      pushToast(
        'success',
        `Fetched ${n} remote${n === 1 ? '' : 's'}` +
          (k > 0 ? ` — ${k} ref${k === 1 ? '' : 's'} updated` : ''),
      );
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  async function handlePull() {
    beginRemoteOp('pull');
    try {
      const res = await ipc.pull(repoId);
      switch (res.kind) {
        case 'upToDate':
          pushToast('success', 'Already up to date');
          break;
        case 'fastForwarded':
          pushToast('success', `Fast-forwarded ${res.branch} to ${shortOid(res.to)}`);
          break;
        case 'wouldNotFastForward':
          pushToast(
            'warning',
            `Cannot fast-forward: '${res.branch}' has ${res.ahead} local commit(s) not on ` +
              'upstream. Bonsai v1 does not merge — push your commits or reconcile via the CLI.',
          );
          break;
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  async function handlePush() {
    beginRemoteOp('push');
    try {
      const res = await ipc.push(repoId);
      if (res.kind === 'upToDate') {
        pushToast('success', 'Already up to date');
      } else {
        pushToast(
          'success',
          `Pushed ${res.branch} → ${res.remote}/${res.branch}` +
            (res.setUpstream ? ' (upstream set)' : ''),
        );
      }
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  // ----- P3c: merge + conflict handling -----
  async function handleMergeBranch(name: string) {
    setMutating(true);
    try {
      const res = await ipc.mergeBranch(repoId, name);
      switch (res.kind) {
        case 'upToDate':
          pushToast('info', `Already up to date with ${name}`);
          break;
        case 'fastForwarded':
          pushToast(
            'success',
            `Fast-forwarded to ${name}` +
              (res.stashed ? ' (local changes stashed and restored)' : ''),
          );
          break;
        case 'merged':
          pushToast(
            'success',
            `Merged ${name}` + (res.stashed ? ' (local changes stashed and restored)' : ''),
          );
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Merge paused: ${res.paths.length} conflict(s) to resolve` +
              (res.stashed
                ? '. Your local changes are safe on the stash (stash@{0}) — apply them after finishing the merge.'
                : ''),
          );
          break;
        case 'stashPopConflicts':
          pushToast(
            'error',
            `Merge done, but re-applying your stashed changes hit ${res.paths.length} conflict(s). ` +
              'Your changes are still on the stash (stash@{0}); resolve the conflicts, then drop the stash.',
          );
          break;
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleResolveConflict(path: string, resolution: ConflictResolution) {
    setMutating(true);
    try {
      await ipc.resolveConflict(repoId, path, resolution);
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // P12 §4.3: stage user-authored resolved text from the ConflictEditor. Same
  // refresh batch as handleResolveConflict — refreshAll drops the resolved path
  // and collapses the slot when the path is no longer conflicted. The editor
  // surfaces a rejection inline via its onResolve promise, so re-throw on error.
  async function handleResolveConflictText(path: string, content: string): Promise<void> {
    setMutating(true);
    try {
      await ipc.resolveConflictText(repoId, path, content);
      await refreshAll();
      pushToast('success', `Staged resolution for ${path}`);
    } catch (e) {
      pushToast('error', errorMessage(e));
      throw e;
    } finally {
      setMutating(false);
    }
  }

  // P13 §8.3: AI conflict resolution for one path. Fetches a proposal (writes
  // nothing), then branches on the autonomy setting: proposeReview opens the
  // proposal in the conflict editor (reused, seeded with the markerless body) so
  // the user reviews/edits before Accept; autoResolve stages it immediately and
  // the user reviews the staged diff before commit_merge. A per-path busy flag
  // gates the row's button (the AI call takes seconds) without freezing the
  // whole panel. Errors surface via the sticky error toast; manual buttons stay.
  async function handleAiResolveConflict(path: string) {
    setAiResolvingPath(path);
    let proposal: AiResolveProposal;
    try {
      proposal = await ipc.aiResolveConflict(repoId, path);
    } catch (e) {
      pushToast('error', errorMessage(e));
      setAiResolvingPath(null);
      return;
    }
    // Safety net (P13): never auto-stage a body that still carries conflict
    // markers. The backend resolve_conflict_text trusts its input (git-add
    // model), so a rare markerful model output would otherwise be staged
    // silently in autoResolve. When that happens, fall through to the review
    // editor with a warning instead — the user still resolves it by hand.
    const markerful = hasUnresolvedMarkers(proposal.proposedText);
    if (aiConflictAutonomy === 'autoResolve' && !markerful) {
      setMutating(true);
      try {
        await ipc.resolveConflictText(repoId, path, proposal.proposedText);
        await refreshAll();
        pushToast('success', `Resolved ${path} with AI — review the staged result`);
      } catch (e) {
        pushToast('error', errorMessage(e));
      } finally {
        setMutating(false);
        setAiResolvingPath(null);
      }
      return;
    }
    if (aiConflictAutonomy === 'autoResolve' && markerful) {
      pushToast('error', `AI left unresolved markers in ${path} — opened for review`);
    }
    // proposeReview (or the autoResolve marker fallback): open the proposal in
    // the conflict editor for review/edit.
    // Guard the getConflict await with the shared fileDiffReqId (P13, same
    // recipe as fetchConflictSlot): if the user opens another diff during the
    // fetch, that bumps the id and we bail rather than clobber their slot.
    const id = ++fileDiffReqId.current;
    try {
      const file = await ipc.getConflict(repoId, path);
      if (id !== fileDiffReqId.current) return;
      // Synthesize a ConflictFile carrying the AI's markerless body so the
      // editor shows the proposed result; ours/theirs are kept for split mode.
      const synthesized = { ...file, text: proposal.proposedText };
      setDiffSlot({
        key: `ai-proposal:${path}`,
        state: 'ready',
        diff: null,
        conflict: synthesized,
        error: null,
      });
    } catch (e) {
      if (id !== fileDiffReqId.current) return;
      pushToast('error', errorMessage(e));
    } finally {
      setAiResolvingPath(null);
    }
  }

  async function handleCommitMerge(message: string) {
    setMutating(true);
    try {
      await ipc.commitMerge(repoId, message);
      await refreshAll();
      pushToast('success', 'Merge committed');
    } finally {
      setMutating(false);
    }
  }

  async function handleAbortMerge() {
    setMutating(true);
    try {
      await ipc.abortMerge(repoId);
      await refreshAll();
      pushToast('success', 'Merge aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- P9: stash handling -----
  async function handleCreateStash() {
    setMutating(true);
    try {
      const res = await ipc.createStash(repoId, null, /* includeUntracked */ true);
      pushToast(
        res.created ? 'success' : 'info',
        res.created ? 'Changes stashed' : 'Nothing to stash — working tree is clean',
      );
      await refreshAll(); // status + graph (pills) + stashes
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleApplyStash(index: number) {
    setMutating(true);
    try {
      const res = await ipc.applyStash(repoId, index);
      if (res.kind === 'applied') pushToast('success', `Applied stash@{${index}}`);
      else
        pushToast(
          'info',
          `Stash applied with ${res.paths.length} conflict(s) to resolve — the stash is kept (stash@{${index}}).`,
        );
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handlePopStash(index: number) {
    setMutating(true);
    try {
      const res = await ipc.popStash(repoId, index);
      if (res.kind === 'applied') pushToast('success', `Popped stash@{${index}}`);
      else
        pushToast(
          'error',
          `Pop hit ${res.paths.length} conflict(s); your changes are still on the stash (stash@{${index}}). ` +
            'Resolve the conflicts, then drop it.',
        );
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleDropStash(index: number) {
    // called after ConfirmDialog
    setMutating(true);
    try {
      await ipc.dropStash(repoId, index);
      pushToast('success', `Dropped stash@{${index}}`);
      await Promise.all([refetchStashes(), refetchGraph()]); // pills change; worktree does not
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- P19: submodule handling -----
  // Init/update/sync are non-destructive to the superproject → no confirm
  // dialog. refetchSubmodules suffices (submodule ops don't change the
  // superproject status/graph in v1).
  async function handleInitSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.initSubmodule(repoId, name);
      pushToast('success', `Initialized ${name}`);
      await refetchSubmodules();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleUpdateSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.updateSubmodule(repoId, name);
      pushToast('success', `Updated ${name}`);
      await refetchSubmodules();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleSyncSubmodule(name: string) {
    setMutating(true);
    try {
      await ipc.syncSubmodule(repoId, name);
      pushToast('success', `Synced URL for ${name}`);
      await refetchSubmodules();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- P22: tag + remote management -----
  // Create create/delete refetch branches (tag list, §2.0) + graph (pill).
  async function handleCreateTag(oid: string, name: string, message: string | null) {
    setMutating(true);
    try {
      await ipc.createTag(repoId, name, oid, message, /* force */ false);
      pushToast('success', `Created tag ${name}`);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleDeleteTag(name: string) {
    setMutating(true);
    try {
      await ipc.deleteTag(repoId, name);
      pushToast('success', `Deleted tag ${name}`);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handlePushTag(remote: string, name: string) {
    setMutating(true);
    try {
      await ipc.pushTag(repoId, remote, name, /* force */ false);
      pushToast('success', `Pushed tag ${name} → ${remote}`);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Add/remove/rename move remote-tracking refs (and thus graph pills), so those
  // refetch remotes + branches + graph; set-url changes only the RemoteInfo list.
  async function handleAddRemote(name: string, url: string) {
    setMutating(true);
    try {
      await ipc.addRemote(repoId, name, url);
      pushToast('success', `Added remote ${name}`);
      await Promise.all([refetchRemotes(), refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRemoveRemote(name: string) {
    setMutating(true);
    try {
      await ipc.removeRemote(repoId, name);
      pushToast('success', `Removed remote ${name}`);
      await Promise.all([refetchRemotes(), refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRenameRemote(name: string, newName: string) {
    setMutating(true);
    try {
      await ipc.renameRemote(repoId, name, newName);
      pushToast('success', `Renamed remote ${name} → ${newName}`);
      await Promise.all([refetchRemotes(), refetchBranches(), refetchGraph()]);
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleSetRemoteUrl(name: string, url: string) {
    setMutating(true);
    try {
      await ipc.setRemoteUrl(repoId, name, url);
      pushToast('success', `Updated URL for ${name}`);
      await refetchRemotes();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- P3d: rebase handling -----
  async function handleRebaseBranch(onto: string) {
    setMutating(true);
    try {
      const res = await ipc.rebaseBranch(repoId, onto);
      switch (res.kind) {
        case 'upToDate':
          pushToast('info', `Already up to date with ${onto}`);
          break;
        case 'fastForwarded':
          pushToast('success', `Fast-forwarded onto ${onto}`);
          break;
        case 'rebased':
          pushToast('success', `Rebased onto ${onto} (${res.steps} commit(s))`);
          break;
        case 'conflicts':
          pushToast(
            'info',
            `Rebase paused at step ${res.currentStep}/${res.totalSteps}: ` +
              `${res.paths.length} conflict(s) to resolve`,
          );
          break;
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRebaseContinue() {
    setMutating(true);
    try {
      const res = await ipc.rebaseContinue(repoId);
      if (res.kind === 'conflicts') {
        pushToast('info', `Rebase paused at step ${res.currentStep}/${res.totalSteps}`);
      } else if (res.kind === 'rebased') {
        pushToast('success', 'Rebase complete');
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRebaseSkip() {
    setMutating(true);
    try {
      const res = await ipc.rebaseSkip(repoId);
      if (res.kind === 'conflicts') {
        pushToast('info', `Rebase paused at step ${res.currentStep}/${res.totalSteps}`);
      } else if (res.kind === 'rebased') {
        pushToast('success', 'Rebase complete');
      }
      await refreshAll();
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRebaseAbort() {
    setMutating(true);
    try {
      await ipc.rebaseAbort(repoId);
      await refreshAll();
      pushToast('success', 'Rebase aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- P23b: interactive rebase -----
  // Seed the plan editor: fetch the default todo list (base..HEAD, all `pick`,
  // oldest-first) and build a per-oid summaries map from the loaded graph nodes.
  // On error → toast, no editor.
  async function openRebasePlan(target: { ontoOid: string; ontoLabel: string }) {
    try {
      const initialTodos = await ipc.getInteractivePlan(repoId, target.ontoOid);
      const nodes = graph?.nodes ?? [];
      const summaries: Record<string, string> = {};
      for (const t of initialTodos) {
        summaries[t.oid] = nodes.find((n) => n.id === t.oid)?.summary ?? shortOid(t.oid);
      }
      setRebasePlanError(null);
      setRebasePlan({ ...target, initialTodos, summaries });
    } catch (e) {
      pushToast('error', errorMessage(e));
    }
  }

  // Start the interactive rebase. Success/conflict close the editor; a backend
  // error keeps it open and surfaces the message in-dialog (plus a sticky toast).
  // On Conflicts the existing OpBanner + rebaseContinue/Skip/Abort drive the rest.
  async function handleStartInteractiveRebase(
    ontoOid: string,
    ontoLabel: string,
    todos: RebaseTodoOp[],
  ) {
    setMutating(true);
    try {
      const res = await ipc.startInteractiveRebase(repoId, ontoOid, todos);
      setRebasePlan(null);
      setRebasePlanError(null);
      // Interactive rebase only ever returns `rebased` or `conflicts`
      // (contract §0 #11 — it always rewrites; no up-to-date/fast-forward path).
      if (res.kind === 'rebased') {
        pushToast('success', `Rebased onto ${ontoLabel} (${res.steps} commit(s))`);
      } else if (res.kind === 'conflicts') {
        pushToast(
          'info',
          `Rebase paused at step ${res.currentStep}/${res.totalSteps}: ` +
            `${res.paths.length} conflict(s) to resolve`,
        );
      }
      await refreshAll();
    } catch (e) {
      // Keep the editor open so the error is visible in-context (§8.1 scope).
      const msg = errorMessage(e);
      setRebasePlanError(msg);
      pushToast('error', msg);
    } finally {
      setMutating(false);
    }
  }

  // ----- P23d: blame + file history -----
  // Close helpers bump the matching reqId so a still-in-flight blameFile/
  // fileHistory promise is dropped (its `reqId.current !== reqId` check fails)
  // and the closed overlay can't pop back open.
  const closeBlame = useCallback(() => {
    blameReqId.current += 1;
    setBlame(null);
  }, []);
  const closeHistory = useCallback(() => {
    historyReqId.current += 1;
    setHistory(null);
  }, []);

  // Reveal a commit in the graph by oid: reuse the select-by-oid path. Setting
  // `selectedIndex` opens CommitPanel AND triggers GraphCanvas's §6.3 effect,
  // which scrolls the row into the virtualized viewport — so this is select+
  // scroll, no extra graph API needed. Close the blame/history overlay first so
  // the revealed row is actually visible (the overlay covers the graph pane).
  const revealCommitByOid = useCallback(
    (oid: string) => {
      const g = graphDataRef.current;
      if (g === null) return;
      const idx = g.nodes.findIndex((n) => n.id === oid);
      if (idx < 0) {
        pushToast('info', 'Commit not in the current view');
        return;
      }
      if (compareRef.current !== null) clearCompare();
      closeBlame();
      closeHistory();
      setSelectedIndex(idx);
    },
    [pushToast, clearCompare, closeBlame, closeHistory],
  );

  // Blame is against the committed HEAD version (atOid=null) in v1 (P23 OPEN #8
  // + orchestrator decision). Cross-invalidate the sibling (history) so only one
  // overlay is ever pending/open: bumping historyReqId drops any in-flight
  // fileHistory response.
  async function handleBlame(path: string) {
    historyReqId.current += 1;
    setHistory(null);
    const reqId = ++blameReqId.current;
    setBlame({ path, lines: [], loading: true, error: null });
    try {
      const lines = await ipc.blameFile(repoId, path, null);
      if (blameReqId.current !== reqId) return;
      setBlame({ path, lines, loading: false, error: null });
    } catch (e) {
      if (blameReqId.current !== reqId) return;
      setBlame({ path, lines: [], loading: false, error: errorMessage(e) });
    }
  }

  async function handleFileHistory(path: string) {
    blameReqId.current += 1;
    setBlame(null);
    const reqId = ++historyReqId.current;
    setHistory({ path, entries: [], loading: true, error: null });
    try {
      const entries = await ipc.fileHistory(repoId, path, MAX_HISTORY_UI);
      if (historyReqId.current !== reqId) return;
      setHistory({ path, entries, loading: false, error: null });
    } catch (e) {
      if (historyReqId.current !== reqId) return;
      setHistory({ path, entries: [], loading: false, error: errorMessage(e) });
    }
  }

  // ----- P20 §5/§6: cherry-pick + revert handling -----
  // An empty pick/revert (nothingToCommit) is an info, not an error toast (§8.1);
  // every other failure surfaces via the sticky error toast.
  function surfacePickRevertError(e: unknown) {
    if (isAppError(e) && e.kind === 'nothingToCommit') {
      pushToast('info', 'Nothing to apply — the change is already present');
    } else {
      pushToast('error', errorMessage(e));
    }
  }

  async function handleCherrypick(oid: string) {
    setMutating(true);
    try {
      const res = await ipc.cherrypickCommit(repoId, oid);
      if (res.kind === 'committed') {
        pushToast('success', `Cherry-picked ${shortOid(res.oid)}`);
      } else {
        pushToast('info', `Cherry-pick paused: ${res.paths.length} conflict(s) to resolve`);
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleRevert(oid: string) {
    setMutating(true);
    try {
      const res = await ipc.revertCommit(repoId, oid);
      if (res.kind === 'committed') {
        pushToast('success', `Reverted ${shortOid(res.oid)}`);
      } else {
        pushToast('info', `Revert paused: ${res.paths.length} conflict(s) to resolve`);
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleCherrypickContinue() {
    setMutating(true);
    try {
      const res = await ipc.cherrypickContinue(repoId);
      // Conflicts can't recur on a single-pick continue, but map defensively.
      if (res.kind === 'committed') {
        pushToast('success', `Cherry-picked ${shortOid(res.oid)}`);
      } else {
        pushToast('info', `Cherry-pick paused: ${res.paths.length} conflict(s) to resolve`);
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleRevertContinue() {
    setMutating(true);
    try {
      const res = await ipc.revertContinue(repoId);
      if (res.kind === 'committed') {
        pushToast('success', `Reverted ${shortOid(res.oid)}`);
      } else {
        pushToast('info', `Revert paused: ${res.paths.length} conflict(s) to resolve`);
      }
      await refreshAll();
    } catch (e) {
      surfacePickRevertError(e);
    } finally {
      setMutating(false);
    }
  }

  async function handleCherrypickAbort() {
    setMutating(true);
    try {
      await ipc.cherrypickAbort(repoId);
      await refreshAll();
      pushToast('success', 'Cherry-pick aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  async function handleRevertAbort() {
    setMutating(true);
    try {
      await ipc.revertAbort(repoId);
      await refreshAll();
      pushToast('success', 'Revert aborted');
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

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

  // P6 §4.1: the single shared builder for a branch/remote-tracking ref menu,
  // used identically by the graph pills AND the sidebar rows. Resolves tip +
  // isHead from the current `branches` snapshot by name so the two surfaces can
  // never diverge. Returns [] (menu does not open) when: no snapshot; the entry
  // is missing; or the entry is the current local HEAD branch.
  function branchMenuItems(
    name: string,
    kind: 'localBranch' | 'remoteBranch',
  ): ContextMenuItem[] {
    const snapshot = branches;
    if (snapshot === null) return [];
    const cur = headBranch?.name ?? null;
    const gate = mutating || opActive;
    const headUnborn = head === null || head.unborn;
    const entry =
      kind === 'localBranch'
        ? snapshot.local.find((b) => b.name === name)
        : snapshot.remote.find((r) => r.name === name);
    if (entry === undefined) return [];
    const isHead = kind === 'localBranch' ? (entry as BranchInfo).isHead : false;
    if (isHead) return [];
    const tip = entry.tip;
    const items: ContextMenuItem[] = [
      {
        label: 'Checkout',
        icon: <CheckoutIcon />,
        disabled: gate,
        onSelect: () =>
          void (kind === 'remoteBranch'
            ? handleCheckoutRemote(name)
            : handleCheckoutBranch(name)),
      },
      {
        label: 'Create branch here',
        icon: <BranchIcon />,
        disabled: gate,
        onSelect: () => setPendingCreateBranch({ oid: tip }),
      },
      {
        label: 'Copy branch name',
        icon: <CopyIcon />,
        disabled: false,
        onSelect: () => {
          const p =
            navigator.clipboard?.writeText(name) ??
            Promise.reject(new Error('Clipboard unavailable'));
          void p
            .then(() => pushToast('success', 'Copied branch name'))
            .catch((e) => pushToast('error', `Copy failed: ${errorMessage(e)}`));
        },
      },
    ];
    // P15c: "Summarize branch…" (local branches only, AI-eligible only). Base
    // selection is a frontend policy (§7.5): the repo's primary branch (main,
    // else master, else the current HEAD branch) UNLESS the target IS that
    // primary, in which case the base is the target's upstream. When no usable
    // base can be resolved (primary missing, or target == primary with no
    // upstream), the item is omitted.
    if (kind === 'localBranch' && aiEligible) {
      const localEntry = entry as BranchInfo;
      const primary = snapshot.local.some((b) => b.name === 'main')
        ? 'main'
        : snapshot.local.some((b) => b.name === 'master')
          ? 'master'
          : (headBranch?.name ?? null);
      const summaryBase = name === primary ? localEntry.upstream : primary;
      if (summaryBase !== null && summaryBase !== name) {
        items.push({
          label: 'Summarize branch…',
          icon: <SummarizeIcon />,
          disabled: false,
          onSelect: () => runSummarize(summaryBase, name),
        });
      }
      // P25b: "Review branch…" (local branches only, AI-eligible only). Reviews
      // the branch's diff vs its auto-resolved base (backend resolves
      // upstream→origin/HEAD→main→master), so no base is passed. Guarded by
      // runAnalyze's req-id, hence disabled:false.
      items.push({
        label: 'Review branch…',
        icon: <SummarizeIcon />,
        disabled: false,
        onSelect: () =>
          runAnalyze({ kind: 'branch', name }, 'review', `Review branch ${name}`),
      });
    }
    if (cur !== null) {
      items.push({
        label: `Merge ${name} into ${cur}`,
        icon: <MergeIcon />,
        disabled: gate,
        onSelect: () => void handleMergeBranch(name),
      });
      items.push({
        label: `Rebase ${cur} onto ${name}`,
        icon: <RebaseIcon />,
        disabled: gate,
        onSelect: () => void handleRebaseBranch(name),
      });
      // P23b §8.2: interactive rebase of the current branch onto this ref's tip.
      items.push({
        label: `Rebase ${cur} onto ${name} (interactive)…`,
        icon: <RebaseIcon />,
        disabled: gate,
        onSelect: () => void openRebasePlan({ ontoOid: tip, ontoLabel: name }),
      });
    }
    if (!headUnborn) {
      items.push({
        label: 'Compare with HEAD',
        icon: <CompareIcon />,
        disabled: false,
        onSelect: () => handleCompareWithHead(tip),
      });
    }
    items.push({
      label: 'Delete',
      icon: <DeleteIcon />,
      disabled: gate,
      onSelect: () =>
        kind === 'remoteBranch' ? setPendingDeleteRemote(name) : setPendingDeleteBranch(name),
    });
    // P20 §3.3: reset the CURRENT branch to this ref's tip (gated internally).
    items.push(...resetMenuItems(tip));
    return items;
  }

  // P9 §6.4: build the right-click menu for a stash row. Apply/Pop need a clean,
  // idle repo (gated on mutating || opActive); Drop is allowed mid-op (it only
  // edits the stash reflog) → routes through the ConfirmDialog.
  function stashMenuItems(index: number): ContextMenuItem[] {
    const gate = mutating || opActive;
    return [
      {
        label: 'Apply',
        icon: <StashApplyIcon />,
        disabled: gate,
        onSelect: () => void handleApplyStash(index),
      },
      {
        label: 'Pop',
        icon: <StashPopIcon />,
        disabled: gate,
        onSelect: () => void handlePopStash(index),
      },
      {
        label: 'Drop',
        icon: <DeleteIcon />,
        disabled: mutating,
        onSelect: () => setPendingDropStash(index),
      },
    ];
  }

  // P9 §6.4: right-click a sidebar stash row → open the shared context menu.
  function handleStashContextMenu(index: number, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: stashMenuItems(index) });
  }

  // P19 §6.4: submodule row menu. "Update" on an uninitialized row
  // init-then-updates (backend §OPEN-4), so it is always enabled; "Init" is a
  // no-op once initialized → disabled unless uninitialized. "Open in new tab"
  // needs a checked-out worktree → disabled while uninitialized.
  function submoduleMenuItems(sub: SubmoduleInfo): ContextMenuItem[] {
    const gate = mutating || opActive;
    return [
      {
        label: 'Init',
        icon: <BranchIcon />,
        disabled: gate || sub.status !== 'uninitialized',
        onSelect: () => void handleInitSubmodule(sub.name),
      },
      {
        label: 'Update',
        icon: <StashApplyIcon />,
        disabled: gate,
        onSelect: () => void handleUpdateSubmodule(sub.name),
      },
      {
        label: 'Sync',
        icon: <RebaseIcon />,
        disabled: gate,
        onSelect: () => void handleSyncSubmodule(sub.name),
      },
      {
        label: 'Open in new tab',
        icon: <CompareIcon />,
        disabled: sub.status === 'uninitialized',
        onSelect: () => onOpenRepoPath(sub.absPath),
      },
    ];
  }

  // P19 §6.4: right-click a sidebar submodule row → open the shared context
  // menu. Looks up the SubmoduleInfo by name from state.
  function handleSubmoduleContextMenu(name: string, clientX: number, clientY: number) {
    const sub = submodules.find((s) => s.name === name);
    if (sub === undefined) return;
    setMenu({ x: clientX, y: clientY, items: submoduleMenuItems(sub) });
  }

  // P22 §7.2: the shared tag menu — used by the graph tag pill AND the sidebar
  // tag rows. Delete (ConfirmDialog) + Copy + one "Push tag to <remote>" per
  // configured remote (§OPEN-7: 0 → no push item; 1 → single; >1 → one each).
  function tagMenuItems(name: string): ContextMenuItem[] {
    const gate = mutating || opActive;
    const items: ContextMenuItem[] = [
      {
        label: 'Delete tag',
        icon: <DeleteIcon />,
        disabled: gate,
        onSelect: () => setPendingDeleteTag(name),
      },
      {
        label: 'Copy tag name',
        icon: <CopyIcon />,
        disabled: false,
        onSelect: () => {
          const p =
            navigator.clipboard?.writeText(name) ??
            Promise.reject(new Error('Clipboard unavailable'));
          void p
            .then(() => pushToast('success', 'Copied tag name'))
            .catch((e) => pushToast('error', `Copy failed: ${errorMessage(e)}`));
        },
      },
    ];
    for (const r of remotes) {
      items.push({
        label: `Push tag to ${r.name}`,
        icon: <TagIcon />,
        disabled: gate,
        onSelect: () => void handlePushTag(r.name, name),
      });
    }
    return items;
  }

  // P22 §7.2: the configured-remote management menu (sidebar rows only).
  function remoteMenuItems(name: string): ContextMenuItem[] {
    const gate = mutating || opActive;
    const url = remotes.find((r) => r.name === name)?.url ?? '';
    return [
      {
        label: 'Rename…',
        icon: <BranchIcon />,
        disabled: gate,
        onSelect: () => setPendingRenameRemote({ name }),
      },
      {
        label: 'Edit URL…',
        icon: <CompareIcon />,
        disabled: gate,
        onSelect: () => setPendingEditUrl({ name, url }),
      },
      {
        label: 'Remove…',
        icon: <DeleteIcon />,
        disabled: gate,
        onSelect: () => setPendingRemoveRemote(name),
      },
    ];
  }

  function handleTagContextMenu(name: string, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: tagMenuItems(name) });
  }

  function handleRemoteContextMenu(name: string, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: remoteMenuItems(name) });
  }

  // P5 §5.2 / P6 §4.2: the commit-row menu — "Create branch here" + "Compare
  // with HEAD" (both read-only entry points; unavailable when HEAD is unborn,
  // §1.3). Factored out (P18b) so the whole-row ref fallback can reuse it.
  // P20 §3.3: the three "Reset <branch> to here" items, gated on an attached
  // born HEAD, an idle repo, and a target that is not already the current tip.
  // Hard is suffixed "…" (opens the extra-warning ConfirmDialog). Returns [] when
  // reset is not offered (so callers can spread unconditionally).
  function resetMenuItems(targetOid: string): ContextMenuItem[] {
    if (head === null || head.unborn || head.detached) return [];
    if (targetOid === head.oid) return [];
    const gate = mutating || opActive;
    const b = headBranch?.name ?? 'HEAD';
    const make = (mode: ResetMode, label: string): ContextMenuItem => ({
      label,
      icon: <RebaseIcon />,
      disabled: gate,
      onSelect: () => setPendingReset({ oid: targetOid, mode }),
    });
    return [
      make('soft', `Reset ${b} to here (soft)`),
      make('mixed', `Reset ${b} to here (mixed)`),
      make('hard', `Reset ${b} to here (hard)…`),
    ];
  }

  function commitMenuItems(oid: string): ContextMenuItem[] {
    if (head === null || head.unborn) return [];
    const gate = mutating || opActive;
    return [
      {
        label: 'Create branch here',
        icon: <BranchIcon />,
        disabled: gate,
        onSelect: () => setPendingCreateBranch({ oid }),
      },
      {
        label: 'Create tag here',
        icon: <TagIcon />,
        disabled: gate,
        onSelect: () => setPendingCreateTag({ oid }),
      },
      {
        label: 'Compare with HEAD',
        icon: <CompareIcon />,
        disabled: false,
        onSelect: () => handleCompareWithHead(oid),
      },
      // P20 §5.2/§6: cherry-pick / revert onto the current branch. Gated on an
      // attached born HEAD (excluded on detached HEAD, which the backend rejects
      // — mirrors resetMenuItems) and an idle repo. On Conflicts the existing
      // OpBanner/conflict flow takes over.
      ...(head.detached
        ? []
        : [
            {
              label: 'Cherry-pick onto current',
              icon: <RebaseIcon />,
              disabled: gate,
              onSelect: () => void handleCherrypick(oid),
            },
            {
              label: 'Revert commit',
              icon: <RebaseIcon />,
              disabled: gate,
              onSelect: () => void handleRevert(oid),
            },
            // P23b §8.2: interactive rebase replaying THIS commit..HEAD onto the
            // selected commit (it becomes the `onto` base). Gated like cherry-pick.
            {
              label: 'Interactive rebase from here…',
              icon: <RebaseIcon />,
              disabled: gate,
              onSelect: () => void openRebasePlan({ ontoOid: oid, ontoLabel: shortOid(oid) }),
            },
          ]),
      ...resetMenuItems(oid),
    ];
  }

  // P5 §5.2 / P6 §4.2: build the right-click menu items for a graph target. Ref
  // pills delegate to the shared branchMenuItems builder; commit rows offer
  // "Compare with HEAD" (read-only; unavailable when HEAD is unborn).
  function buildContextItems(target: GraphContextTarget): ContextMenuItem[] {
    if (target.kind === 'ref') {
      const r = target.ref;
      // P10 §5: a stash pill → Apply/Pop/Drop menu (parse the index from the name).
      if (r.kind === 'stash') {
        const m = /^stash@\{(\d+)\}$/.exec(r.name);
        if (m === null) return []; // malformed name → no menu (defensive)
        return stashMenuItems(Number(m[1]));
      }
      if (r.kind === 'head') return [];
      // P22 §7.2: the graph tag pill opens the same menu as the sidebar tag row.
      if (r.kind === 'tag') return tagMenuItems(r.name);
      const kind = r.kind === 'remoteBranch' ? 'remoteBranch' : 'localBranch';
      const items = branchMenuItems(r.name, kind);
      if (items.length > 0) return items;
      // P18b: whole-row right-click resolved to a branch whose branch menu is
      // empty — the current HEAD branch. Fall back to the commit menu (resolving
      // the row's oid from the branch tip) so the row still opens a useful menu.
      const snapshot = branches;
      if (snapshot === null) return [];
      const entry =
        kind === 'localBranch'
          ? snapshot.local.find((b) => b.name === r.name)
          : snapshot.remote.find((b) => b.name === r.name);
      if (entry === undefined) return [];
      return commitMenuItems(entry.tip);
    }
    // Commit row → Create branch here + Compare with HEAD.
    return commitMenuItems(target.oid);
  }

  function handleGraphContextMenu(target: GraphContextTarget, clientX: number, clientY: number) {
    const items = buildContextItems(target);
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
    const items = branchMenuItems(name, kind);
    if (items.length === 0) return;
    setMenu({ x: clientX, y: clientY, items });
  }

  // Stable so ContextMenu's dismiss-listener effect doesn't re-arm on every
  // parent re-render while the menu is open (reviewer NIT).
  const closeMenu = useCallback(() => setMenu(null), []);

  // Esc-layering effect (active tab only; global modals win). typing guard ->
  // collapse diff overlay -> exit compare -> deselect commit (P5 §5.4).
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (globalModalOpen) return;
      const target = e.target as HTMLElement | null;
      if (target !== null && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT')) return;
      // P11g-rev §4.7: layering, topmost first. The commit-mode DiffBrowser
      // overlay closes first; then the workdir single-file diffSlot; then
      // compare mode (which also closes its auto-open browser); then deselect.
      // P15b: the AI output panel floats above everything — Esc dismisses it first.
      if (aiPanelOpenRef.current) {
        closeAiPanel();
        return;
      }
      // P23d: blame / file-history overlays close before the diff/commit layers.
      // Use the close helpers so the in-flight fetch reqId is invalidated too.
      if (blameOpenRef.current) {
        closeBlame();
        return;
      }
      if (historyOpenRef.current) {
        closeHistory();
        return;
      }
      if (commitBrowserOpenRef.current) {
        setCommitBrowserOpen(false);
        return;
      }
      if (diffSlotRef.current !== null) {
        collapseDiffSlot();
        return;
      }
      if (compareRef.current !== null) {
        clearCompare();
        return;
      }
      setSelectedIndex((cur) => (cur !== null ? null : cur));
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [active, globalModalOpen, collapseDiffSlot, clearCompare, closeAiPanel, closeBlame, closeHistory]);

  // Per-repo shortcut effect (active tab only, §5.1): refresh / fetch / pull /
  // push / graph nav. Global modals + this repo's own dialogs suppress it.
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (globalModalOpen) return;
      const ctrl = e.ctrlKey || e.metaKey;

      if (e.key === 'F5' || (ctrl && e.key.toLowerCase() === 'r')) {
        e.preventDefault();
        const canRefresh = !refreshing && !statusLoading && !graphLoading && !mutating;
        if (canRefresh) void handleRefresh();
        return;
      }

      const target = e.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable);
      if (typing) return;

      if (dialogOpen || abortConfirmOpen) return;

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        if (!refreshing && !mutating) void handleFetch();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        if (!refreshing && !mutating && canPullPush) void handlePull();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'u') {
        e.preventDefault();
        if (!refreshing && !mutating && canPullPush) void handlePush();
        return;
      }

      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        setSelectedIndex((cur) => {
          if (cur === null) return cur;
          const next = e.key === 'ArrowDown' ? cur + 1 : cur - 1;
          return Math.max(0, Math.min(next, graph.nodes.length - 1));
        });
        return;
      }

      if (e.key === 'PageDown' || e.key === 'PageUp') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        const n = graphRef.current?.getVisibleRowCount() ?? 10;
        setSelectedIndex((cur) => {
          if (cur === null) return cur;
          const next = e.key === 'PageDown' ? cur + n : cur - n;
          return Math.max(0, Math.min(next, graph.nodes.length - 1));
        });
        return;
      }

      if (e.key === 'Home' || e.key === 'End') {
        if (selectedIndex === null || graph === null) return;
        e.preventDefault();
        setSelectedIndex(e.key === 'Home' ? 0 : graph.nodes.length - 1);
        return;
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [
    active,
    globalModalOpen,
    refreshing,
    statusLoading,
    graphLoading,
    mutating,
    canPullPush,
    dialogOpen,
    abortConfirmOpen,
    selectedIndex,
    graph,
  ]);

  const headBranch = branches?.local.find((b) => b.isHead) ?? null;

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

  const pushTitle =
    headBranch === null
      ? 'Push'
      : headBranch.upstream !== null
        ? `Push ${headBranch.name} to ${headBranch.upstream}`
        : `Push ${headBranch.name} to origin/${headBranch.name} and set upstream`;

  return (
    <>
      <div className="workspace-toolbar">
        <div className="toolbar-center">
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating}
            onClick={() => void handleFetch()}
            title="Fetch all remotes (Ctrl+Shift+F)"
          >
            {remoteOp === 'fetch' ? 'Fetching…' : '↓ Fetch'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating || !canPullPush}
            onClick={() => void handlePull()}
            title="Pull (fast-forward only) (Ctrl+Shift+P)"
          >
            {remoteOp === 'pull' ? 'Pulling…' : '⇣ Pull'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={refreshing || mutating || !canPullPush}
            onClick={() => void handlePush()}
            title={`${pushTitle} (Ctrl+Shift+U)`}
          >
            {remoteOp === 'push' ? 'Pushing…' : '↑ Push'}
          </button>
        </div>
        <button
          type="button"
          className="btn-icon toolbar-refresh"
          disabled={refreshing || statusLoading || graphLoading || mutating}
          onClick={() => void handleRefresh()}
          title="Refresh (Ctrl+R)"
          aria-label="Refresh"
        >
          {'⟳'}
        </button>
      </div>
      {(remoteOp !== null || refreshing) && <div className="header-progress" aria-hidden="true" />}

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
          onCreateStash={() => void handleCreateStash()}
          onStashContextMenu={handleStashContextMenu}
          submodules={submodules}
          onSubmoduleContextMenu={handleSubmoduleContextMenu}
          onTagContextMenu={handleTagContextMenu}
          remotes={remotes}
          onRemoteContextMenu={handleRemoteContextMenu}
          onAddRemote={() => setPendingAddRemote(true)}
        />
        <PaneDivider side="sidebar" onResize={onSidebarResize} onResizeEnd={onPaneResizeEnd} />
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
              <p className="pane-empty">No commits yet</p>
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
              onContextMenu={handleGraphContextMenu}
              metrics={metrics}
              metricsVersion={metricsVersion}
            />
          ) : null}
          {diffSlot !== null && overlayMeta !== null && (
            <DiffOverlay
              slot={diffSlot}
              meta={overlayMeta}
              onClose={collapseDiffSlot}
              onResolveConflictText={handleResolveConflictText}
              mutating={mutating}
              onExplain={overlayExplain}
              viewMode={diffViewMode}
              onSetViewMode={handleSetViewMode}
              stageable={stageable}
              onStageLines={handleStageLines}
              onStageHunk={handleStageHunk}
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
        <PaneDivider
          side="right-panel"
          onResize={onRightPanelResize}
          onResizeEnd={onPaneResizeEnd}
        />
        <aside className="right-panel" style={{ width: paneWidths.rightPanel }}>
          <OpBanner
            op={opState}
            conflictCount={conflicts.length}
            mutating={mutating}
            onCommitMerge={handleBannerCommitMerge}
            onRebaseContinue={() => void handleRebaseContinue()}
            onRebaseSkip={() => void handleRebaseSkip()}
            onOpContinue={() =>
              void (opState.kind === 'cherryPick'
                ? handleCherrypickContinue()
                : handleRevertContinue())
            }
            onAbort={() => setAbortConfirmOpen(true)}
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
              onSelectParent={handleSelectParent}
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
                aiAnalyzing={aiPanel?.loading === true}
                onStage={(paths) => void handleStage(paths)}
                onUnstage={(paths) => void handleUnstage(paths)}
                onDiscard={(paths) => setPendingDiscard(paths)}
                onReviewStaged={() =>
                  runAnalyze({ kind: 'staged' }, 'review', 'Review staged changes')
                }
                onReviewWorktree={() =>
                  runAnalyze({ kind: 'worktree' }, 'review', 'Review working tree')
                }
                onToggleDiff={handleToggleWorkdirDiff}
                onResolveConflict={(path, r) => void handleResolveConflict(path, r)}
                onToggleConflictView={handleToggleConflictView}
                onAiResolve={(path) => void handleAiResolveConflict(path)}
                onBlame={(path) => void handleBlame(path)}
                onFileHistory={(path) => void handleFileHistory(path)}
              />
              {opState.kind === 'none' && head !== null && !head.unborn && (
                <div className="amend-affordance">
                  <label className="amend-toggle">
                    <input
                      type="checkbox"
                      checked={amend}
                      disabled={mutating}
                      onChange={(e) => void handleToggleAmend(e.target.checked)}
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
                blocked={!amend && opActive && opState.kind !== 'merge'}
                amend={amend}
                onCommit={
                  amend
                    ? handleCommitAmend
                    : opState.kind === 'merge'
                      ? handleCommitMerge
                      : handleCommit
                }
                aiEligible={aiEligible}
                onGenerate={handleGenerateCommitMessage}
              />
            </>
          )}
        </aside>
      </div>

      <ConfirmDialog
        open={abortConfirmOpen}
        title={
          opState.kind === 'rebase'
            ? 'Abort rebase?'
            : opState.kind === 'cherryPick'
              ? 'Abort cherry-pick?'
              : opState.kind === 'revert'
                ? 'Abort revert?'
                : 'Abort merge?'
        }
        confirmLabel={
          opState.kind === 'rebase'
            ? 'Abort rebase'
            : opState.kind === 'cherryPick'
              ? 'Abort cherry-pick'
              : opState.kind === 'revert'
                ? 'Abort revert'
                : 'Abort merge'
        }
        busy={mutating}
        onConfirm={() => {
          const kind = opState.kind;
          setAbortConfirmOpen(false);
          if (kind === 'rebase') {
            void handleRebaseAbort();
          } else if (kind === 'cherryPick') {
            void handleCherrypickAbort();
          } else if (kind === 'revert') {
            void handleRevertAbort();
          } else {
            void handleAbortMerge();
          }
        }}
        onCancel={() => setAbortConfirmOpen(false)}
      >
        {opState.kind === 'rebase' ? (
          <div>
            This restores your branch and working tree to their pre-rebase state. Replayed commits
            and conflict resolutions will be lost.
          </div>
        ) : opState.kind === 'cherryPick' || opState.kind === 'revert' ? (
          <div>
            This resets your branch and working tree to HEAD. The in-progress{' '}
            {opState.kind === 'cherryPick' ? 'cherry-pick' : 'revert'} and any conflict resolutions
            will be lost.
          </div>
        ) : (
          <div>
            This restores the files touched by the merge to their pre-merge state. Conflict
            resolutions will be lost.
          </div>
        )}
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDeleteBranch !== null}
        title="Delete branch"
        confirmLabel="Delete branch"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteBranch;
          setPendingDeleteBranch(null);
          if (name !== null) void handleDeleteBranch(name);
        }}
        onCancel={() => setPendingDeleteBranch(null)}
      >
        <div>Delete branch "<span className="mono">{pendingDeleteBranch ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          The branch is fully merged, but this cannot be undone from Bonsai.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDeleteRemote !== null}
        title="Delete remote-tracking reference"
        confirmLabel="Delete reference"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteRemote;
          setPendingDeleteRemote(null);
          if (name !== null) void handleDeleteRemoteTracking(name);
        }}
        onCancel={() => setPendingDeleteRemote(null)}
      >
        <div>Delete the remote-tracking reference "<span className="mono">{pendingDeleteRemote ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          This removes only Bonsai's local copy of the remote branch. It does NOT delete the branch on
          the server — a future fetch may recreate it.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDropStash !== null}
        title="Drop stash"
        confirmLabel="Drop stash"
        busy={mutating}
        onConfirm={() => {
          const i = pendingDropStash;
          setPendingDropStash(null);
          if (i !== null) void handleDropStash(i);
        }}
        onCancel={() => setPendingDropStash(null)}
      >
        <div>Drop <span className="mono">stash@{`{${pendingDropStash ?? 0}}`}</span>?</div>
        <div className="dialog-body-note">
          This permanently discards the stashed changes and cannot be undone.
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingReset !== null}
        title={pendingReset?.mode === 'hard' ? 'Hard reset' : 'Reset branch'}
        confirmLabel={
          pendingReset === null
            ? 'Reset'
            : `Reset (${pendingReset.mode})`
        }
        busy={mutating}
        onConfirm={() => {
          const p = pendingReset;
          setPendingReset(null);
          if (p !== null) void handleResetBranch(p.oid, p.mode);
        }}
        onCancel={() => setPendingReset(null)}
      >
        <div>
          Move <span className="mono">{headBranch?.name ?? 'HEAD'}</span> to{' '}
          <span className="mono">{shortOid(pendingReset?.oid ?? '')}</span> ({pendingReset?.mode})?
        </div>
        <div className="dialog-body-note">
          Commits after the target are no longer on this branch (recoverable via the reflog).
          {pendingReset?.mode === 'hard' && (
            <> Uncommitted changes in your working tree will be permanently discarded.</>
          )}
        </div>
      </ConfirmDialog>

      <ConfirmDialog
        open={pendingDiscard !== null}
        title="Discard changes"
        confirmLabel="Discard changes"
        busy={mutating}
        onConfirm={() => {
          const paths = pendingDiscard;
          setPendingDiscard(null);
          if (paths !== null) void handleDiscard(paths);
        }}
        onCancel={() => setPendingDiscard(null)}
      >
        <div>Discard changes to {pendingDiscard?.length ?? 0} file(s)?</div>
        <div className="dialog-body-note">
          This permanently reverts them to the last staged/committed version and cannot be undone.
        </div>
      </ConfirmDialog>

      <PromptDialog
        open={pendingCreateBranch !== null}
        title="Create branch here"
        label="Branch name"
        placeholder="feature/my-branch"
        confirmLabel="Create branch"
        busy={mutating}
        validate={(v) => {
          const t = v.trim();
          if (t === '' || t.startsWith('-')) return 'Enter a valid branch name';
          if (branches?.local.some((b) => b.name === t) === true)
            return 'A branch with that name already exists';
          return null;
        }}
        onSubmit={(v) => void handleCreateBranchHere(pendingCreateBranch!.oid, v.trim())}
        onCancel={() => setPendingCreateBranch(null)}
      />

      {/* P22: create tag at the right-clicked commit. */}
      <TagCreateDialog
        open={pendingCreateTag !== null}
        targetOid={pendingCreateTag?.oid ?? ''}
        busy={mutating}
        existingTags={branches?.tags ?? []}
        onSubmit={(name, message) => {
          const oid = pendingCreateTag?.oid ?? null;
          setPendingCreateTag(null);
          if (oid !== null) void handleCreateTag(oid, name, message);
        }}
        onCancel={() => setPendingCreateTag(null)}
      />

      {/* P22: delete tag (local only). */}
      <ConfirmDialog
        open={pendingDeleteTag !== null}
        title="Delete tag"
        confirmLabel="Delete tag"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeleteTag;
          setPendingDeleteTag(null);
          if (name !== null) void handleDeleteTag(name);
        }}
        onCancel={() => setPendingDeleteTag(null)}
      >
        <div>Delete tag "<span className="mono">{pendingDeleteTag ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          Deletes the local tag only; a tag already pushed to a remote is not removed there.
        </div>
      </ConfirmDialog>

      {/* P22: add a new remote (name + url both editable). */}
      <RemoteEditDialog
        open={pendingAddRemote}
        title="Add remote"
        confirmLabel="Add remote"
        busy={mutating}
        existingNames={remotes.map((r) => r.name)}
        onSubmit={(name, url) => {
          setPendingAddRemote(false);
          void handleAddRemote(name, url);
        }}
        onCancel={() => setPendingAddRemote(false)}
      />

      {/* P22: edit an existing remote's fetch URL (name read-only). */}
      <RemoteEditDialog
        open={pendingEditUrl !== null}
        title="Edit remote URL"
        confirmLabel="Save URL"
        busy={mutating}
        nameReadOnly
        initialName={pendingEditUrl?.name}
        initialUrl={pendingEditUrl?.url}
        existingNames={remotes.map((r) => r.name)}
        onSubmit={(_name, url) => {
          const target = pendingEditUrl;
          setPendingEditUrl(null);
          if (target !== null) void handleSetRemoteUrl(target.name, url);
        }}
        onCancel={() => setPendingEditUrl(null)}
      />

      {/* P22: rename a remote (single-field → reuse PromptDialog). */}
      <PromptDialog
        open={pendingRenameRemote !== null}
        title="Rename remote"
        label="New remote name"
        placeholder="origin"
        initialValue={pendingRenameRemote?.name}
        confirmLabel="Rename"
        busy={mutating}
        validate={(v) => {
          const t = v.trim();
          if (t === '') return 'Enter a remote name';
          if (/\s/.test(t)) return 'Remote name cannot contain whitespace';
          if (t !== pendingRenameRemote?.name && remotes.some((r) => r.name === t))
            return 'A remote with that name already exists';
          return null;
        }}
        onSubmit={(v) => {
          const target = pendingRenameRemote;
          setPendingRenameRemote(null);
          if (target !== null) void handleRenameRemote(target.name, v.trim());
        }}
        onCancel={() => setPendingRenameRemote(null)}
      />

      {/* P22: remove a remote (drops its tracking refs locally). */}
      <ConfirmDialog
        open={pendingRemoveRemote !== null}
        title="Remove remote"
        confirmLabel="Remove remote"
        busy={mutating}
        onConfirm={() => {
          const name = pendingRemoveRemote;
          setPendingRemoveRemote(null);
          if (name !== null) void handleRemoveRemote(name);
        }}
        onCancel={() => setPendingRemoveRemote(null)}
      >
        <div>Remove remote "<span className="mono">{pendingRemoveRemote ?? ''}</span>"?</div>
        <div className="dialog-body-note">
          Removes the remote and its remote-tracking branches from this repo. The server is not
          affected.
        </div>
      </ConfirmDialog>

      {/* P23b: interactive-rebase plan editor. */}
      <RebasePlanEditor
        open={rebasePlan !== null}
        ontoLabel={rebasePlan?.ontoLabel ?? ''}
        ontoOid={rebasePlan?.ontoOid ?? ''}
        initialTodos={rebasePlan?.initialTodos ?? []}
        summaries={rebasePlan?.summaries ?? {}}
        mutating={mutating}
        error={rebasePlanError}
        onCancel={() => {
          setRebasePlan(null);
          setRebasePlanError(null);
        }}
        onStart={(todos) => {
          if (rebasePlan !== null) {
            void handleStartInteractiveRebase(rebasePlan.ontoOid, rebasePlan.ontoLabel, todos);
          }
        }}
      />

      {menu !== null && (
        <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={closeMenu} />
      )}
    </>
  );
}
