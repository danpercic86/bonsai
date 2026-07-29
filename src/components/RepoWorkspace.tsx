import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CommitBox } from './CommitBox';
import type { CommitBoxHandle } from './CommitBox';
import { CommitPanel } from './CommitPanel';
import { ComparePanel } from './ComparePanel';
import { ConfirmDialog } from './ConfirmDialog';
import { ContextMenu } from './ContextMenu';
import type { ContextMenuItem } from './ContextMenu';
import { DiffOverlay } from './DiffOverlay';
import type { DiffOverlayMeta } from './DiffOverlay';
import { OpBanner } from './OpBanner';
import { PaneDivider } from './PaneDivider';
import { Sidebar } from './Sidebar';
import { StatusPanel } from './StatusPanel';
import type { DiffSlot, WorkdirSection } from './StatusPanel';
import { GraphCanvas } from '../graph/GraphCanvas';
import type { GraphCanvasHandle, GraphContextTarget, WipSummary } from '../graph/GraphCanvas';
import { ipc } from '../ipc';
import type {
  BranchInfo,
  BranchesSnapshot,
  CommitDiff,
  CompareDiff,
  ConflictEntry,
  ConflictResolution,
  FileDiff,
  FileDiffHeader,
  GraphLayout,
  HeadInfo,
  ListView,
  PaneWidths,
  RepoInfo,
  RepoOpState,
  StashEntry,
  StatusEntry,
  StatusSnapshot,
  Unsubscribe,
} from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

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
  onSidebarResize(delta: number): void;
  onRightPanelResize(delta: number): void;
  onPaneResizeEnd(): void;
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
  onSidebarResize,
  onRightPanelResize,
  onPaneResizeEnd,
}: RepoWorkspaceProps) {
  const pushToast = usePushToast();
  const repoPath = repoId; // repoId == canonical workdir path (§2)

  // RepoInfo is (re)loaded by refreshAll's openRepo; head also arrives via the
  // branches snapshot, so gating works before the first refreshAll.
  const [repo, setRepo] = useState<RepoInfo | null>(null);

  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  const [statusError, setStatusError] = useState<{ id: number; message: string } | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [mutating, setMutating] = useState(false);

  const [branches, setBranches] = useState<BranchesSnapshot | null>(null);
  const [branchesError, setBranchesError] = useState<string | null>(null);
  const [branchesLoading, setBranchesLoading] = useState(false);

  const [stashes, setStashes] = useState<StashEntry[]>([]);

  const [remoteOp, setRemoteOp] = useState<'fetch' | 'pull' | 'push' | null>(null);

  const [opState, setOpState] = useState<RepoOpState>({ kind: 'none' });
  const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);
  const [abortConfirmOpen, setAbortConfirmOpen] = useState(false);
  const commitBoxRef = useRef<CommitBoxHandle>(null);
  // P6 §4.5: pending branch/remote deletes drive the two confirm dialogs; the
  // shortcut effect is suppressed while either is up (derived `dialogOpen`).
  const [pendingDeleteBranch, setPendingDeleteBranch] = useState<string | null>(null);
  const [pendingDeleteRemote, setPendingDeleteRemote] = useState<string | null>(null);
  const [pendingDropStash, setPendingDropStash] = useState<number | null>(null);
  const dialogOpen =
    pendingDeleteBranch !== null || pendingDeleteRemote !== null || pendingDropStash !== null;

  const [graph, setGraph] = useState<GraphLayout | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  const graphRef = useRef<GraphCanvasHandle>(null);

  const [commitDiff, setCommitDiff] = useState<CommitDiff | null>(null);
  const [commitDiffLoading, setCommitDiffLoading] = useState(false);
  const [commitDiffError, setCommitDiffError] = useState<string | null>(null);
  const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null);

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

  const statusReqId = useRef(0);
  const graphReqId = useRef(0);
  const branchesReqId = useRef(0);
  const stashesReqId = useRef(0);
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
    if (key.startsWith('commit:')) {
      const path = key.slice('commit:'.length);
      const file = commitDiff?.files.find((f) => f.path === path) ?? null;
      return {
        path,
        origPath: file?.origPath ?? null,
        status: file?.status ?? null,
        kind: 'commit',
      };
    }
    if (key.startsWith('conflict:')) {
      return {
        path: key.slice('conflict:'.length),
        origPath: null,
        status: 'conflicted',
        kind: 'conflict',
      };
    }
    if (key.startsWith('compare:')) {
      const path = key.slice('compare:'.length);
      const file = compareData?.files.find((f) => f.path === path) ?? null;
      return {
        path,
        origPath: file?.origPath ?? null,
        status: file?.status ?? null,
        kind: 'compare',
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
  }, [diffSlot, status, commitDiff, compareData]);

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
      if (slot !== null && !slot.key.startsWith('commit:') && !slot.key.startsWith('conflict:')) {
        const sep = slot.key.indexOf(':');
        const section = slot.key.slice(0, sep) as WorkdirSection;
        const path = slot.key.slice(sep + 1);
        const entry = snapshot[section].find((en) => en.path === path);
        if (entry === undefined) {
          collapseDiffSlot();
        } else {
          void fetchDiffSlot(slot.key, () =>
            ipc.getWorkdirFileDiff(repoId, entry.path, entry.origPath, section === 'staged'),
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
          refetchOpState(),
          refetchCompare(),
        ]);
      } else {
        clearStatus();
        clearGraph();
        clearBranches();
        clearStashes();
        clearOpState();
        clearCompare();
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
    refetchOpState,
    refetchCompare,
    clearStatus,
    clearGraph,
    clearBranches,
    clearStashes,
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
      if (diffSlotRef.current?.key.startsWith('commit:') === true) {
        fileDiffReqId.current += 1;
        setDiffSlot(null);
      }
    }
  }, [selectedIndex, graph, repoId]);

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
    refetchOpState,
    refetchCompare,
  ]);

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
      ipc.getWorkdirFileDiff(repoId, entry.path, entry.origPath, section === 'staged'),
    );
  }

  function handleToggleCommitDiff(file: FileDiffHeader) {
    if (selectedIndex === null || graph === null) return;
    const oid = graph.nodes[selectedIndex].id;
    const key = `commit:${file.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () => ipc.getCommitFileDiff(repoId, oid, file.path, file.origPath));
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

  function handleToggleCompareDiff(file: FileDiffHeader) {
    if (compare === null) return;
    const key = `compare:${file.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () =>
      ipc.compareWithHeadFileDiff(repoId, compare.oid, file.path, file.origPath),
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
        disabled: gate,
        onSelect: () =>
          void (kind === 'remoteBranch'
            ? handleCheckoutRemote(name)
            : handleCheckoutBranch(name)),
      },
      {
        label: 'Copy branch name',
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
    if (cur !== null) {
      items.push({
        label: `Merge ${name} into ${cur}`,
        disabled: gate,
        onSelect: () => void handleMergeBranch(name),
      });
      items.push({
        label: `Rebase ${cur} onto ${name}`,
        disabled: gate,
        onSelect: () => void handleRebaseBranch(name),
      });
    }
    if (!headUnborn) {
      items.push({
        label: 'Compare with HEAD',
        disabled: false,
        onSelect: () => handleCompareWithHead(tip),
      });
    }
    items.push({
      label: 'Delete',
      disabled: gate,
      onSelect: () =>
        kind === 'remoteBranch' ? setPendingDeleteRemote(name) : setPendingDeleteBranch(name),
    });
    return items;
  }

  // P9 §6.4: build the right-click menu for a stash row. Apply/Pop need a clean,
  // idle repo (gated on mutating || opActive); Drop is allowed mid-op (it only
  // edits the stash reflog) → routes through the ConfirmDialog.
  function stashMenuItems(index: number): ContextMenuItem[] {
    const gate = mutating || opActive;
    return [
      { label: 'Apply', disabled: gate, onSelect: () => void handleApplyStash(index) },
      { label: 'Pop', disabled: gate, onSelect: () => void handlePopStash(index) },
      { label: 'Drop', disabled: mutating, onSelect: () => setPendingDropStash(index) },
    ];
  }

  // P9 §6.4: right-click a sidebar stash row → open the shared context menu.
  function handleStashContextMenu(index: number, clientX: number, clientY: number) {
    setMenu({ x: clientX, y: clientY, items: stashMenuItems(index) });
  }

  // P5 §5.2 / P6 §4.2: build the right-click menu items for a graph target. Ref
  // pills delegate to the shared branchMenuItems builder; commit rows offer
  // "Compare with HEAD" (read-only; unavailable when HEAD is unborn).
  function buildContextItems(target: GraphContextTarget): ContextMenuItem[] {
    if (target.kind === 'ref') {
      const r = target.ref;
      if (r.kind === 'tag' || r.kind === 'head' || r.kind === 'stash') return [];
      return branchMenuItems(r.name, r.kind === 'remoteBranch' ? 'remoteBranch' : 'localBranch');
    }
    // Commit row → Compare with HEAD (unavailable for unborn HEAD, §1.3).
    if (head === null || head.unborn) return [];
    return [
      {
        label: 'Compare with HEAD',
        disabled: false,
        onSelect: () => handleCompareWithHead(target.oid),
      },
    ];
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
  }, [active, globalModalOpen, collapseDiffSlot, clearCompare]);

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
                // Left-clicking any row exits Compare mode (P5 §5.4).
                if (compare !== null) clearCompare();
                setSelectedIndex(i);
              }}
              wip={wip}
              themeVersion={themeVersion}
              active={active}
              onContextMenu={handleGraphContextMenu}
            />
          ) : null}
          {diffSlot !== null && overlayMeta !== null && (
            <DiffOverlay slot={diffSlot} meta={overlayMeta} onClose={collapseDiffSlot} />
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
            onAbort={() => setAbortConfirmOpen(true)}
          />
          {compare !== null ? (
            <ComparePanel
              data={compareData}
              loading={compareLoading}
              error={compareError}
              headBranchName={headBranch?.name ?? null}
              diffSlot={diffSlot}
              listView={listView}
              onToggleDiff={handleToggleCompareDiff}
              onClose={clearCompare}
            />
          ) : selectedIndex !== null && graph !== null ? (
            <CommitPanel
              node={graph.nodes[selectedIndex]}
              data={commitDiff}
              loading={commitDiffLoading}
              error={commitDiffError}
              diffSlot={diffSlot}
              listView={listView}
              onToggleDiff={handleToggleCommitDiff}
              onSelectParent={handleSelectParent}
              onClose={() => setSelectedIndex(null)}
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
                onStage={(paths) => void handleStage(paths)}
                onUnstage={(paths) => void handleUnstage(paths)}
                onToggleDiff={handleToggleWorkdirDiff}
                onResolveConflict={(path, r) => void handleResolveConflict(path, r)}
                onToggleConflictView={handleToggleConflictView}
              />
              <CommitBox
                key={opState.kind === 'merge' ? `merge:${opState.incoming}` : 'commit'}
                ref={commitBoxRef}
                stagedCount={status?.staged.length ?? 0}
                busy={mutating}
                mode={opState.kind === 'merge' ? 'merge' : 'commit'}
                initialMessage={opState.kind === 'merge' ? opState.message : undefined}
                conflictCount={conflicts.length}
                blocked={opActive && opState.kind !== 'merge'}
                onCommit={opState.kind === 'merge' ? handleCommitMerge : handleCommit}
              />
            </>
          )}
        </aside>
      </div>

      <ConfirmDialog
        open={abortConfirmOpen}
        title={opState.kind === 'rebase' ? 'Abort rebase?' : 'Abort merge?'}
        confirmLabel={opState.kind === 'rebase' ? 'Abort rebase' : 'Abort merge'}
        busy={mutating}
        onConfirm={() => {
          const isRebase = opState.kind === 'rebase';
          setAbortConfirmOpen(false);
          if (isRebase) {
            void handleRebaseAbort();
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

      {menu !== null && (
        <ContextMenu x={menu.x} y={menu.y} items={menu.items} onClose={closeMenu} />
      )}
    </>
  );
}
