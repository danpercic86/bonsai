import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CommitBox } from './components/CommitBox';
import { CommitPanel } from './components/CommitPanel';
import { DiffOverlay } from './components/DiffOverlay';
import type { DiffOverlayMeta } from './components/DiffOverlay';
import { PaneDivider } from './components/PaneDivider';
import { RepoSwitcher } from './components/RepoSwitcher';
import { ShortcutOverlay } from './components/ShortcutOverlay';
import { Sidebar } from './components/Sidebar';
import { StatusPanel } from './components/StatusPanel';
import type { DiffSlot, WorkdirSection } from './components/StatusPanel';
import { Toasts } from './components/Toasts';
import type { Toast, ToastTone } from './components/Toasts';
import { GraphCanvas } from './graph/GraphCanvas';
import type { GraphCanvasHandle, WipSummary } from './graph/GraphCanvas';
import { ipc } from './ipc';
import type {
  BranchesSnapshot,
  CommitDiff,
  FileDiff,
  FileDiffHeader,
  GraphLayout,
  ListView,
  PaneWidths,
  RecentRepo,
  RepoInfo,
  StatusEntry,
  StatusSnapshot,
  Theme,
  Unsubscribe,
} from './ipc';
import { errorMessage, isAppError } from './utils/errors';

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

function shortOid(oid: string): string {
  return oid.slice(0, 7);
}

function isUsableRepo(info: RepoInfo): boolean {
  return info.isRepo && !info.bare;
}

// P2a §2.5: persisted-sanity clamp ranges (mirrors settings.rs clamp_pane_widths).
const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 480;
const RIGHT_PANEL_MIN = 280;
const RIGHT_PANEL_MAX = 640;
const GRAPH_MIN_WIDTH = 480;
const DEFAULT_PANE_WIDTHS: PaneWidths = { sidebar: 240, rightPanel: 380 };

/** Live-drag clamp (§2.5): the persisted range intersected with the current
 * window size and the graph pane's floor — a deliberately different check
 * from the persisted-sanity range above (that one alone could let a resize
 * squeeze the graph pane on a small window). */
function clampLive(value: number, side: 'sidebar' | 'rightPanel', otherWidth: number): number {
  const [min, max] = side === 'sidebar' ? [SIDEBAR_MIN, SIDEBAR_MAX] : [RIGHT_PANEL_MIN, RIGHT_PANEL_MAX];
  const dynamicMax = Math.min(max, window.innerWidth - otherWidth - GRAPH_MIN_WIDTH);
  return Math.max(min, Math.min(value, Math.max(min, dynamicMax)));
}

/** P2b §4.2: sets data-theme on <html> (not <body> — matches the
 * :root/[data-theme] selector scope). 'dark' also sets the attribute
 * explicitly (rather than removing it) so [data-theme="light"] and a
 * default :root both work identically regardless of prior state. */
function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme === 'light' ? 'light' : 'dark');
}

export default function App() {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [status, setStatus] = useState<StatusSnapshot | null>(null);
  // Id-wrapped (P1 §4.5) so StatusPanel's dismissal is per-occurrence, not
  // per-message — identical errors from distinct operations re-surface.
  const [statusError, setStatusError] = useState<{ id: number; message: string } | null>(null);
  const [statusLoading, setStatusLoading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  // Single flag for stage/unstage/commit (M3 §4.4): pessimistic UI — controls
  // disable in flight, state comes back via refetch.
  const [mutating, setMutating] = useState(false);

  const [branches, setBranches] = useState<BranchesSnapshot | null>(null);
  const [branchesError, setBranchesError] = useState<string | null>(null);
  const [branchesLoading, setBranchesLoading] = useState(false);

  // Which remote op is in flight — drives the per-button busy label.
  const [remoteOp, setRemoteOp] = useState<'fetch' | 'pull' | 'push' | null>(null);

  // P1 §5: toast stack. Remote-op feedback (M6) and composite-refresh failures
  // surface here; contextual banners (status/commit/sidebar/graph) stay inline.
  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);

  // P1 §6: keyboard shortcuts — "?" overlay + ConfirmDialog-open lift from the
  // Sidebar (shortcuts are inert while the dialog is up).
  const [overlayOpen, setOverlayOpen] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);
  // P1 reviewer SHOULD-FIX: RepoSwitcher's dropdown lifted the same way as
  // Sidebar's ConfirmDialog (onDialogOpenChange) — global shortcuts go inert
  // and the Esc effect below skips a keypress the switcher already consumed.
  const [switcherOpen, setSwitcherOpen] = useState(false);

  // P1 §10: recent repos — persisted list + reopen-last-on-launch.
  const [recents, setRecents] = useState<RecentRepo[]>([]);

  // P2a: pane widths — loaded once from ipc.getUiSettings() (§3.3), persisted
  // debounced on drag-end/keyboard-nudge.
  const [paneWidths, setPaneWidths] = useState<PaneWidths>(DEFAULT_PANE_WIDTHS);
  const paneWidthsRef = useRef(paneWidths);
  paneWidthsRef.current = paneWidths;
  const saveTimerRef = useRef<number | null>(null);

  // P2b: theme — loaded once from ipc.getUiSettings() (mount effect below),
  // persisted on toggle. themeVersion increments on every change so
  // GraphCanvas knows to re-resolve its cached CSS-variable colors (§4.4).
  const [theme, setTheme] = useState<Theme>('dark');
  const [themeVersion, setThemeVersion] = useState(0);

  // P3b: flat vs tree-grouped lists — loaded from the same getUiSettings call,
  // persisted on toggle. Consumed by Sidebar/StatusPanel/CommitPanel from
  // P3b-2/3 onward; until then the toggle just flips + persists the state.
  const [listView, setListView] = useState<ListView>('tree');

  const [graph, setGraph] = useState<GraphLayout | null>(null);
  const [graphError, setGraphError] = useState<string | null>(null);
  const [graphLoading, setGraphLoading] = useState(false);
  const [selectedIndex, setSelectedIndex] = useState<number | null>(null);
  // P2c §5.2: imperative handle to read the DOM-measured visible row count
  // for PageUp/PageDown deltas — pure index arithmetic, no lane/edge math.
  const graphRef = useRef<GraphCanvasHandle>(null);

  // M4: commit details (mode B) + the shared per-file diff expansion slot.
  const [commitDiff, setCommitDiff] = useState<CommitDiff | null>(null);
  const [commitDiffLoading, setCommitDiffLoading] = useState(false);
  const [commitDiffError, setCommitDiffError] = useState<string | null>(null);
  const [diffSlot, setDiffSlot] = useState<DiffSlot | null>(null); // shared by both modes

  // Request-id last-wins guards: only the latest in-flight request may apply
  // its result (M1 contract §5 — no frontend debounce beyond this).
  const statusReqId = useRef(0);
  const graphReqId = useRef(0);
  const branchesReqId = useRef(0);
  const commitDiffReqId = useRef(0);
  const fileDiffReqId = useRef(0);
  // Current slot, readable from stable callbacks without re-subscribing.
  const diffSlotRef = useRef<DiffSlot | null>(null);
  diffSlotRef.current = diffSlot;
  // Monotonic id for statusError occurrences (P1 §4.5).
  const statusErrorId = useRef(0);

  const repoPath = repo !== null && isUsableRepo(repo) ? repo.path : null;
  const repoOpen = repoPath !== null;
  // Convenience gating only — the backend guards detached/unborn itself (§4.2).
  const canPullPush = repo?.head != null && !repo.head.detached && !repo.head.unborn;

  // P1 §9.2: WIP row summary derived from the already-fetched status snapshot
  // — no new IPC call, no Rust layout change.
  const wip: WipSummary | null = useMemo(() => {
    if (status === null || repo?.head?.unborn === true) return null;
    const paths = new Set<string>();
    for (const s of [status.staged, status.unstaged, status.untracked, status.conflicted]) {
      for (const e of s) paths.add(e.path);
    }
    return paths.size > 0 ? { fileCount: paths.size } : null;
  }, [status, repo]);

  // P3a §2.3: overlay header meta, derived (never stored) from the slot key +
  // the current snapshot/commitDiff so it can't go stale. Lookup miss (entry
  // gone from a newer snapshot in the brief window before refetchStatus
  // collapses the slot, or commitDiff cleared mid-flight): path from the key,
  // no badge — never throw, never hide the close button.
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
  }, [diffSlot, status, commitDiff]);

  const reportStatusError = useCallback((message: string) => {
    setStatusError({ id: ++statusErrorId.current, message });
  }, []);

  const dismissToast = useCallback((id: number) => {
    setToasts((cur) => cur.filter((t) => t.id !== id));
  }, []);

  /** P1 §5.1: sticky = tone === 'error' (stays until dismissed); non-sticky
   * auto-dismiss after 5 s (timeout captured per toast id). Stack cap 5 —
   * pushing the 6th drops the oldest NON-sticky toast (oldest sticky if none). */
  const pushToast = useCallback(
    (tone: ToastTone, text: string) => {
      const id = ++toastId.current;
      const sticky = tone === 'error';
      setToasts((cur) => {
        const next = [...cur, { id, tone, text, sticky }];
        if (next.length <= 5) return next;
        const dropIdx = next.findIndex((t) => !t.sticky && t.id !== id);
        return next.filter((_, i) => i !== (dropIdx !== -1 ? dropIdx : 0));
      });
      if (!sticky) window.setTimeout(() => dismissToast(id), 5000);
    },
    [dismissToast],
  );

  /** Debounced persist (300ms) so rapid successive small nudges (keyboard)
   * don't spam IPC; the drag path already only calls this once per
   * onResizeEnd, but keyboard nudges are per-keypress (P2a §3.2).
   * Reads paneWidthsRef at fire time, not call time: onResizeEnd runs in the
   * same event handler as the setPaneWidths that changed the width, so the
   * ref is still one render stale there — by the time the debounce fires the
   * re-render has happened and the ref holds the post-nudge value. */
  const commitPaneWidths = useCallback(() => {
    if (saveTimerRef.current !== null) window.clearTimeout(saveTimerRef.current);
    saveTimerRef.current = window.setTimeout(() => {
      void ipc
        .setUiSettings({ paneWidths: paneWidthsRef.current })
        .catch((e) => pushToast('error', `Could not save pane widths: ${errorMessage(e)}`));
    }, 300);
  }, [pushToast]);

  const toggleTheme = useCallback(() => {
    const next: Theme = theme === 'dark' ? 'light' : 'dark';
    setTheme(next);
    applyTheme(next);
    setThemeVersion((v) => v + 1);
    void ipc
      .setUiSettings({ theme: next })
      .catch((e) => pushToast('error', `Could not save theme: ${errorMessage(e)}`));
  }, [theme, pushToast]);

  const toggleListView = useCallback(() => {
    const next: ListView = listView === 'tree' ? 'flat' : 'tree';
    setListView(next);
    void ipc
      .setUiSettings({ listView: next })
      .catch((e) => pushToast('error', `Could not save list view: ${errorMessage(e)}`));
  }, [listView, pushToast]);

  const handleSidebarResize = useCallback((delta: number) => {
    setPaneWidths((w) => ({
      ...w,
      sidebar: clampLive(w.sidebar + delta, 'sidebar', w.rightPanel),
    }));
  }, []);

  const handleRightPanelResize = useCallback((delta: number) => {
    setPaneWidths((w) => ({
      ...w,
      rightPanel: clampLive(w.rightPanel + delta, 'rightPanel', w.sidebar),
    }));
  }, []);

  const handlePaneResizeEnd = useCallback(() => {
    commitPaneWidths();
  }, [commitPaneWidths]);

  /** Fetch (or re-fetch) the expanded diff for `key`; last-wins guarded.
   * A same-key refetch keeps the stale diff visible (P1 §4.1) — first-time
   * expansions load with `diff: null` (skeleton). */
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
    fileDiffReqId.current += 1; // invalidate any in-flight fetch
    setDiffSlot(null);
  }, []);

  const refetchStatus = useCallback(async () => {
    const id = ++statusReqId.current;
    setStatusLoading(true);
    try {
      const snapshot = await ipc.getStatus();
      if (id !== statusReqId.current) return;
      setStatus(snapshot);
      setStatusError(null);
      // M4 §4.4: a new snapshot invalidates the mode-A expansion — entry gone
      // -> collapse; still present -> re-fetch (content may have changed).
      const slot = diffSlotRef.current;
      if (slot !== null && !slot.key.startsWith('commit:')) {
        const sep = slot.key.indexOf(':');
        const section = slot.key.slice(0, sep) as WorkdirSection;
        const path = slot.key.slice(sep + 1);
        const entry = snapshot[section].find((en) => en.path === path);
        if (entry === undefined) {
          collapseDiffSlot();
        } else {
          void fetchDiffSlot(slot.key, () =>
            ipc.getWorkdirFileDiff(entry.path, entry.origPath, section === 'staged'),
          );
        }
      }
    } catch (e) {
      if (id !== statusReqId.current) return;
      reportStatusError(errorMessage(e));
    } finally {
      if (id === statusReqId.current) setStatusLoading(false);
    }
  }, [fetchDiffSlot, collapseDiffSlot, reportStatusError]);

  const clearStatus = useCallback(() => {
    statusReqId.current += 1; // invalidate any in-flight request
    setStatus(null);
    setStatusError(null);
    setStatusLoading(false);
    collapseDiffSlot();
  }, [collapseDiffSlot]);

  // Refetches keep showing the previous layout until the new one arrives.
  const refetchGraph = useCallback(async () => {
    const id = ++graphReqId.current;
    setGraphLoading(true);
    try {
      const layout = await ipc.getGraph();
      if (id !== graphReqId.current) return;
      setGraph(layout);
      setGraphError(null);
      setSelectedIndex(null); // indices are only valid within one layout
    } catch (e) {
      if (id !== graphReqId.current) return;
      setGraphError(errorMessage(e));
    } finally {
      if (id === graphReqId.current) setGraphLoading(false);
    }
  }, []);

  const refetchBranches = useCallback(async () => {
    const id = ++branchesReqId.current;
    setBranchesLoading(true);
    try {
      const snapshot = await ipc.listBranches();
      if (id !== branchesReqId.current) return;
      setBranches(snapshot);
      setBranchesError(null);
    } catch (e) {
      if (id !== branchesReqId.current) return;
      setBranchesError(errorMessage(e));
    } finally {
      if (id === branchesReqId.current) setBranchesLoading(false);
    }
  }, []);

  const clearBranches = useCallback(() => {
    branchesReqId.current += 1; // invalidate any in-flight request
    setBranches(null);
    setBranchesError(null);
    setBranchesLoading(false);
  }, []);

  const clearGraph = useCallback(() => {
    graphReqId.current += 1; // invalidate any in-flight request
    setGraph(null);
    setGraphError(null);
    setGraphLoading(false);
    setSelectedIndex(null);
  }, []);

  /** Composite post-op refresh (P1 §4.6): openRepo on the current path
   * (refreshes header HEAD, self-heals the watcher) + refetch status/graph/
   * branches. Never throws — failures surface as a sticky error toast
   * "Refresh failed: <message>" (P1c §5.3). The `refetch*` helpers keep their
   * own pane-scoped error states. */
  const refreshAll = useCallback(async (): Promise<void> => {
    if (repoPath === null) return;
    try {
      const info = await ipc.openRepo(repoPath);
      setRepo(info);
      if (isUsableRepo(info)) {
        await Promise.all([refetchStatus(), refetchGraph(), refetchBranches()]);
      } else {
        clearStatus();
        clearGraph();
        clearBranches();
      }
    } catch (e) {
      pushToast('error', `Refresh failed: ${errorMessage(e)}`);
    }
  }, [
    repoPath,
    refetchStatus,
    refetchGraph,
    refetchBranches,
    clearStatus,
    clearGraph,
    clearBranches,
    pushToast,
  ]);

  // P1 §10.1: recent-repos list, refetched after every successful open.
  const refreshRecents = useCallback(async () => {
    try {
      setRecents(await ipc.getRecentRepos());
    } catch {
      // Non-fatal — recents are best-effort UI sugar.
    }
  }, []);

  /** Open a specific path with no folder picker — shared by the switcher, the
   * empty-state recents list, and reopen-on-launch (P1 §10.1). */
  const openPath = useCallback(
    async (path: string, opts: { fromRecents: boolean }) => {
      const hadRepoOpen = repoPath !== null;
      setError(null);
      setLoading(true);
      try {
        const info = await ipc.openRepo(path);
        setRepo(info);
        if (isUsableRepo(info)) {
          void refetchStatus();
          void refetchGraph();
          void refetchBranches();
        } else {
          clearStatus();
          clearGraph();
          clearBranches();
        }
        void refreshRecents();
      } catch (e) {
        if (opts.fromRecents && isAppError(e) && e.kind === 'io') {
          // Path moved/deleted: drop it from recents (never resurrect it).
          void ipc.removeRecentRepo(path).then(setRecents);
        }
        if (hadRepoOpen) {
          pushToast('error', errorMessage(e));
        } else {
          setError(errorMessage(e));
          setRepo(null); // a failed open leaves no repo open (matches backend)
        }
        clearStatus();
        clearGraph();
        clearBranches();
      } finally {
        setLoading(false);
      }
    },
    [repoPath, refetchStatus, refetchGraph, refetchBranches, clearStatus, clearGraph, clearBranches, refreshRecents, pushToast],
  );

  // Mount effect (once): load recents; if none is currently open, reopen the
  // most-recently-used repo (locked "reopen last repo on launch" product
  // decision, P1 §12.2). Guarded against StrictMode double-invoke via a ref.
  const launchedRef = useRef(false);
  useEffect(() => {
    if (launchedRef.current) return;
    launchedRef.current = true;
    (async () => {
      try {
        const list = await ipc.getRecentRepos();
        setRecents(list);
        if (repoPath === null && list.length > 0) {
          void openPath(list[0].path, { fromRecents: true });
        }
      } catch {
        // Non-fatal.
      }
      try {
        const s = await ipc.getUiSettings();
        setPaneWidths(s.paneWidths);
        setTheme(s.theme);
        applyTheme(s.theme);
        setThemeVersion((v) => v + 1);
        setListView(s.listView);
      } catch {
        // Non-fatal — keep defaults.
      }
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // M4 §4.4: selection -> commit diff. Every selection change also resets the
  // shared expansion slot (its keys belong to the previous mode/commit).
  useEffect(() => {
    if (selectedIndex !== null && graph !== null) {
      fileDiffReqId.current += 1;
      setDiffSlot(null);
      const oid = graph.nodes[selectedIndex].id;
      const id = ++commitDiffReqId.current;
      setCommitDiff(null);
      setCommitDiffLoading(true);
      setCommitDiffError(null);
      ipc.getCommitDiff(oid).then(
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
      commitDiffReqId.current += 1; // invalidate any in-flight commit diff
      setCommitDiff(null);
      setCommitDiffLoading(false);
      setCommitDiffError(null);
      if (diffSlotRef.current?.key.startsWith('commit:') === true) {
        fileDiffReqId.current += 1;
        setDiffSlot(null);
      }
    }
  }, [selectedIndex, graph]);

  // Esc precedence (P3a §2.4), top wins — one layer per keypress: switcher →
  // shortcut "?" overlay → typing guard → diff overlay → deselect commit.
  // Skip entirely while the switcher dropdown is open — RepoSwitcher's own Esc
  // listener already closes it; without this guard the same keypress would
  // ALSO close the overlay/deselect the commit underneath.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (switcherOpen) return;
      if (overlayOpen) {
        setOverlayOpen(false);
        return;
      }
      const target = e.target as HTMLElement | null;
      if (target !== null && (target.tagName === 'TEXTAREA' || target.tagName === 'INPUT')) return;
      if (diffSlotRef.current !== null) {
        collapseDiffSlot();
        return;
      }
      setSelectedIndex((cur) => (cur !== null ? null : cur));
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [overlayOpen, switcherOpen, collapseDiffSlot]);

  // P1 §6.2: global shortcut handler. Guard order: refresh (always
  // preventDefault, even as a no-op) -> typing guard -> dialog-open guard ->
  // remaining bindings (each gated by the same enablement as its button).
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;

      if (e.key === 'F5' || (ctrl && e.key.toLowerCase() === 'r')) {
        e.preventDefault();
        const canRefresh =
          repoOpen && !refreshing && !statusLoading && !graphLoading && !mutating;
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

      if (dialogOpen || switcherOpen) return;

      if (ctrl && e.key.toLowerCase() === 'o') {
        e.preventDefault();
        void handleOpenRepository();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'f') {
        e.preventDefault();
        if (repoOpen && !refreshing && !mutating) void handleFetch();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'p') {
        e.preventDefault();
        if (repoOpen && !refreshing && !mutating && canPullPush) void handlePull();
        return;
      }

      if (ctrl && e.shiftKey && e.key.toLowerCase() === 'u') {
        e.preventDefault();
        if (repoOpen && !refreshing && !mutating && canPullPush) void handlePush();
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

      if (e.key === '?') {
        e.preventDefault();
        setOverlayOpen((cur) => !cur);
        return;
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [
    repoOpen,
    refreshing,
    statusLoading,
    graphLoading,
    mutating,
    canPullPush,
    dialogOpen,
    switcherOpen,
    selectedIndex,
    graph,
  ]);

  // Subscriptions only (per React rules): repo-changed events + window focus
  // both trigger a status refetch while a usable repo is open.
  useEffect(() => {
    if (repoPath === null) return;
    let cancelled = false;
    const unsubs: Unsubscribe[] = [];

    const subscribe = async () => {
      const offChanged = await ipc.onRepoChanged(() => {
        console.debug('[bonsai] repo-changed → refetch status+graph+branches');
        void refetchStatus();
        void refetchGraph();
        void refetchBranches();
      });
      if (cancelled) {
        offChanged();
        return;
      }
      unsubs.push(offChanged);

      const offFocus = await ipc.onWindowFocus(() => {
        console.debug('[bonsai] window focus → refetch status+graph+branches');
        void refetchStatus();
        void refetchGraph();
        void refetchBranches();
      });
      if (cancelled) {
        offFocus();
        return;
      }
      unsubs.push(offFocus);
    };
    void subscribe();

    return () => {
      cancelled = true;
      for (const unsub of unsubs) unsub();
    };
  }, [repoPath, refetchStatus, refetchGraph, refetchBranches]);

  // Picker path: delegates to the shared openPath (P1 §10.1).
  async function handleOpenRepository() {
    setError(null);
    setLoading(true);
    try {
      const path = await ipc.pickFolder();
      if (path === null) {
        return; // user cancelled; keep current state
      }
      await openPath(path, { fromRecents: false });
    } finally {
      setLoading(false);
    }
  }

  // Manual refresh button: the shared composite refresh, with a busy flag.
  async function handleRefresh() {
    if (repoPath === null || refreshing) return;
    setRefreshing(true);
    try {
      await refreshAll(); // never throws (§4.6)
    } finally {
      setRefreshing(false);
    }
  }

  async function handleStage(paths: string[]) {
    setMutating(true);
    try {
      await ipc.stage(paths);
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
      await ipc.unstage(paths);
      await refetchStatus();
    } catch (e) {
      reportStatusError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // Commit errors are RETHROWN so CommitBox displays them inline; errors from
  // the post-commit refresh (commit already succeeded) surface via refreshAll.
  async function handleCommit(message: string) {
    setMutating(true);
    try {
      await ipc.commit(message);
      // Post-commit composite refresh (§4.6) — never throws, so a refresh
      // failure cannot masquerade as a commit failure in CommitBox.
      await refreshAll();
    } finally {
      setMutating(false);
    }
  }

  // Errors RETHROWN so the Sidebar's create input shows them inline
  // (CommitBox pattern).
  async function handleCreateBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.createBranch(name);
      await refetchBranches();
      void refetchGraph(); // new ref pill appears
    } finally {
      setMutating(false);
    }
  }

  async function handleCheckoutBranch(name: string) {
    setBranchesError(null);
    setMutating(true);
    try {
      await ipc.checkoutBranch(name);
      // Composite refresh (§4.6): never throws, so the catch below only sees
      // checkout failures (which belong to the sidebar banner).
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
      await ipc.deleteBranch(name);
      await Promise.all([refetchBranches(), refetchGraph()]);
    } catch (e) {
      setBranchesError(errorMessage(e));
    } finally {
      setMutating(false);
    }
  }

  // ----- M6: remote operations (fetch / pull / push) -----
  // P1 §5.3: remote notice/error migrated to the toast stack — 'ok' -> success
  // toast (auto-dismiss), 'warn' -> warning toast (auto-dismiss), errors ->
  // sticky error toast. Copy is byte-identical to the pre-P1 banner strings.

  /** Common entry for every remote-op handler: mark busy. */
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
      const res = await ipc.fetch();
      const n = res.remotes.length;
      const k = res.remotes.reduce((sum, r) => sum + r.updatedRefs, 0);
      pushToast(
        'success',
        `Fetched ${n} remote${n === 1 ? '' : 's'}` +
          (k > 0 ? ` — ${k} ref${k === 1 ? '' : 's'} updated` : ''),
      );
      await Promise.all([refetchBranches(), refetchGraph()]); // status unaffected
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  async function handlePull() {
    beginRemoteOp('pull');
    try {
      const res = await ipc.pull();
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
      // Composite refresh (§4.6): the branch tip may have moved.
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
      const res = await ipc.push();
      if (res.kind === 'upToDate') {
        pushToast('success', 'Already up to date');
      } else {
        pushToast(
          'success',
          `Pushed ${res.branch} → ${res.remote}/${res.branch}` +
            (res.setUpstream ? ' (upstream set)' : ''),
        );
      }
      await Promise.all([refetchBranches(), refetchGraph()]); // ahead badge -> 0
    } catch (e) {
      pushToast('error', errorMessage(e));
    } finally {
      endRemoteOp();
    }
  }

  // Mode-A accordion toggle: staged rows -> staged diff; unstaged/untracked ->
  // unstaged diff (M4 §4.2).
  function handleToggleWorkdirDiff(section: WorkdirSection, entry: StatusEntry) {
    const key = `${section}:${entry.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () =>
      ipc.getWorkdirFileDiff(entry.path, entry.origPath, section === 'staged'),
    );
  }

  // Mode-B accordion toggle: hunks for one file of the selected commit.
  function handleToggleCommitDiff(file: FileDiffHeader) {
    if (selectedIndex === null || graph === null) return;
    const oid = graph.nodes[selectedIndex].id;
    const key = `commit:${file.path}`;
    if (diffSlotRef.current?.key === key) {
      collapseDiffSlot();
      return;
    }
    void fetchDiffSlot(key, () => ipc.getCommitFileDiff(oid, file.path, file.origPath));
  }

  // Parent short-oid clicked: GraphNode.parents are node indices, ordinal-
  // matched to CommitDetails.parents (both first-parent-first).
  function handleSelectParent(parentOrdinal: number) {
    if (selectedIndex === null || graph === null) return;
    const parentIndex = graph.nodes[selectedIndex].parents[parentOrdinal];
    if (parentIndex !== undefined) setSelectedIndex(parentIndex);
  }

  const headBranch = branches?.local.find((b) => b.isHead) ?? null;
  const pushTitle =
    headBranch === null
      ? 'Push'
      : headBranch.upstream !== null
        ? `Push ${headBranch.name} to ${headBranch.upstream}`
        : `Push ${headBranch.name} to origin/${headBranch.name} and set upstream`;

  return (
    <div className="app">
      <header className="header">
        <span className="app-name">Bonsai</span>
        {repoOpen && repo !== null && (
          <RepoSwitcher
            repo={repo}
            recents={recents}
            disabled={refreshing || mutating}
            onOpenPath={(path) => void openPath(path, { fromRecents: true })}
            onBrowse={() => void handleOpenRepository()}
            onOpenChange={setSwitcherOpen}
          />
        )}
        <div className="header-toolbar">
          <button
            type="button"
            className="toolbar-btn"
            disabled={!repoOpen || refreshing || mutating}
            onClick={() => void handleFetch()}
            title="Fetch all remotes (Ctrl+Shift+F)"
          >
            {remoteOp === 'fetch' ? 'Fetching…' : '↓ Fetch'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={!repoOpen || refreshing || mutating || !canPullPush}
            onClick={() => void handlePull()}
            title="Pull (fast-forward only) (Ctrl+Shift+P)"
          >
            {remoteOp === 'pull' ? 'Pulling…' : '⇣ Pull'}
          </button>
          <button
            type="button"
            className="toolbar-btn"
            disabled={!repoOpen || refreshing || mutating || !canPullPush}
            onClick={() => void handlePush()}
            title={`${pushTitle} (Ctrl+Shift+U)`}
          >
            {remoteOp === 'push' ? 'Pushing…' : '↑ Push'}
          </button>
          <button
            type="button"
            className="btn-icon theme-toggle"
            onClick={toggleTheme}
            title={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
            aria-label={theme === 'dark' ? 'Switch to light theme' : 'Switch to dark theme'}
          >
            {theme === 'dark' ? '☀' : '☾'}
          </button>
          <button
            type="button"
            className="btn-icon list-view-toggle"
            onClick={toggleListView}
            title={listView === 'tree' ? 'Switch to flat lists' : 'Switch to tree lists'}
            aria-label={listView === 'tree' ? 'Switch to flat lists' : 'Switch to tree lists'}
          >
            {listView === 'tree' ? '☰' : '⋔'}
          </button>
          <button
            type="button"
            className="btn-icon"
            disabled={!repoOpen || refreshing || statusLoading || graphLoading || mutating}
            onClick={handleRefresh}
            title="Refresh (Ctrl+R)"
            aria-label="Refresh"
          >
            {'⟳'}
          </button>
        </div>
      </header>
      {(remoteOp !== null || refreshing) && (
        <div className="header-progress" aria-hidden="true" />
      )}

      {repoOpen && repo !== null ? (
        <div className="panes">
          <Sidebar
            data={branches}
            loading={branchesLoading}
            error={branchesError}
            onDismissError={() => setBranchesError(null)}
            busy={mutating}
            onCheckout={(name) => void handleCheckoutBranch(name)}
            onDelete={(name) => void handleDeleteBranch(name)}
            onCreateBranch={handleCreateBranch}
            onDialogOpenChange={setDialogOpen}
            width={paneWidths.sidebar}
          />
          <PaneDivider
            side="sidebar"
            onResize={handleSidebarResize}
            onResizeEnd={handlePaneResizeEnd}
          />
          <main className="graph-pane">
            {graphError !== null && (
              <div className="error-banner graph-error-banner">{graphError}</div>
            )}
            {graph !== null && graph.truncated && (
              <div className="graph-truncated-banner">
                History truncated to the most recent 100,000 commits
              </div>
            )}
            {repo.head?.unborn ? (
              <div className="graph-pane-empty">
                <p className="pane-empty">No commits yet</p>
              </div>
            ) : graph !== null ? (
              // Loading first layout: nothing over the canvas area (no spinners).
              <GraphCanvas
                ref={graphRef}
                layout={graph}
                selectedIndex={selectedIndex}
                onSelect={setSelectedIndex}
                wip={wip}
                themeVersion={themeVersion}
              />
            ) : null}
            {/* P3a §2.1 lifecycle (all pre-existing behavior, no new code):
               refetchStatus collapses the slot when the file disappears (overlay
               closes) or same-key refetches it (stale content dimmed); any
               selection change resets the slot (overlay closes); clicking a
               different file row switches the overlay content in place. */}
            {diffSlot !== null && overlayMeta !== null && (
              <DiffOverlay slot={diffSlot} meta={overlayMeta} onClose={collapseDiffSlot} />
            )}
          </main>
          <PaneDivider
            side="right-panel"
            onResize={handleRightPanelResize}
            onResizeEnd={handlePaneResizeEnd}
          />
          <aside className="right-panel" style={{ width: paneWidths.rightPanel }}>
            {selectedIndex !== null && graph !== null ? (
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
                  onStage={(paths) => void handleStage(paths)}
                  onUnstage={(paths) => void handleUnstage(paths)}
                  onToggleDiff={handleToggleWorkdirDiff}
                />
                <CommitBox
                  stagedCount={status?.staged.length ?? 0}
                  busy={mutating}
                  onCommit={handleCommit}
                />
              </>
            )}
          </aside>
        </div>
      ) : (
        <div className="empty-state">
          <h1 className="empty-title">Bonsai</h1>
          <p className="empty-tagline">A tidy Git client</p>
          {error !== null && <div className="error-banner">{error}</div>}
          {repo !== null && !repo.isRepo && (
            <div className="error-banner">
              Not a Git repository: <span className="mono">{repo.path}</span>
            </div>
          )}
          {repo !== null && repo.isRepo && repo.bare && (
            <div className="error-banner">
              Bare repositories are not supported: <span className="mono">{repo.path}</span>
            </div>
          )}
          <button
            type="button"
            className="btn-primary"
            onClick={handleOpenRepository}
            disabled={loading}
          >
            {loading ? 'Opening…' : 'Open repository'}
          </button>
          {recents.length > 0 && (
            <div className="recents-list">
              <p className="section-label recents-label">Recent</p>
              {recents.map((r) => (
                <button
                  key={r.path}
                  type="button"
                  className="recents-item"
                  disabled={loading}
                  onClick={() => void openPath(r.path, { fromRecents: true })}
                >
                  <span className="recents-item-name">{folderName(r.path)}</span>
                  <span className="recents-item-path" title={r.path}>
                    {r.path}
                  </span>
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      <ShortcutOverlay open={overlayOpen} onClose={() => setOverlayOpen(false)} />
      <Toasts toasts={toasts} onDismiss={dismissToast} />
    </div>
  );
}
