// §5.2 / §6: the open-repo tab strip and everything that opens a repo into it —
// tab list + active tab, the recents list, the empty-state error/loading pair,
// and the debounced whole-session persist. Extracted verbatim from App so the
// container only wires TabStrip / EmptyState to it; `pushToast` is the sole
// input because every failure here surfaces either as a toast (tabs open) or as
// the empty-state error (no tabs).
import { useCallback, useEffect, useRef, useState } from 'react';
import type { Dispatch, MutableRefObject, SetStateAction } from 'react';
import type { TabMeta } from '../components/TabStrip';
import { ipc } from '../ipc';
import type { RecentRepo } from '../ipc';
import type { PushToast } from '../ToastContext';
import { errorMessage, isAppError } from '../utils/errors';
import { isUsableRepo, unusableRepoMessage } from '../appHelpers';

export interface UseRepoTabs {
  error: string | null;
  loading: boolean;
  recents: RecentRepo[];
  setRecents: Dispatch<SetStateAction<RecentRepo[]>>;
  refreshRecents: () => Promise<void>;
  tabs: TabMeta[];
  setTabs: Dispatch<SetStateAction<TabMeta[]>>;
  activeRepo: string | null;
  setActiveRepo: Dispatch<SetStateAction<string | null>>;
  tabsRef: MutableRefObject<TabMeta[]>;
  /** Flipped true by App's launch-reopen effect once the restore has settled;
   *  until then the persist effect must not write over the stored session. */
  sessionReadyRef: MutableRefObject<boolean>;
  openTab: (path: string) => Promise<void>;
  closeTab: (repoId: string) => void;
  reorderTabs: (from: number, to: number) => void;
  handleOpenRepository: () => Promise<void>;
  handleInitRepository: () => Promise<void>;
}

export function useRepoTabs(pushToast: PushToast): UseRepoTabs {
  // ----- App-global state (§5.1) -----
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [recents, setRecents] = useState<RecentRepo[]>([]);

  // ----- Tab state (§5.2) -----
  const [tabs, setTabs] = useState<TabMeta[]>([]);
  const [activeRepo, setActiveRepo] = useState<string | null>(null);
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  // ----- Session persistence (§6): debounced whole-session write -----
  const sessionSaveTimer = useRef<number | null>(null);
  const sessionReadyRef = useRef(false);
  const persistSession = useCallback((openRepos: string[], active: string | null) => {
    if (sessionSaveTimer.current !== null) window.clearTimeout(sessionSaveTimer.current);
    sessionSaveTimer.current = window.setTimeout(() => {
      void ipc
        .setSession({ openRepos, activeRepo: active })
        .catch((e) => pushToast('error', `Could not save session: ${errorMessage(e)}`));
    }, 300);
  }, [pushToast]);

  // Unmount-only: drop a pending debounced write so it cannot land after the
  // tree is gone (`ipc.setSession` plus a `pushToast` with no host left).
  // Empty deps by design — re-running on `persistSession` identity changes
  // would cancel legitimate in-flight saves.
  useEffect(
    () => () => {
      if (sessionSaveTimer.current !== null) window.clearTimeout(sessionSaveTimer.current);
    },
    [],
  );

  // Persist on any tab / active change once launch reopen has settled.
  useEffect(() => {
    if (!sessionReadyRef.current) return;
    persistSession(tabs.map((t) => t.repoId), activeRepo);
  }, [tabs, activeRepo, persistSession]);

  // Tell the backend the focused-tab repoId (P16 §5) so new embedded-MCP
  // sessions seed from it. Fires on tab activation, open, close, and once on
  // startup after session restore (all funnel through `activeRepo`).
  useEffect(() => {
    void ipc.setActiveRepo(activeRepo).catch(() => {
      // Non-fatal: only seeds new MCP sessions; the GUI is unaffected.
    });
  }, [activeRepo]);

  const refreshRecents = useCallback(async () => {
    try {
      setRecents(await ipc.getRecentRepos());
    } catch {
      // Non-fatal — recents are best-effort UI sugar.
    }
  }, []);

  /** Open (or focus) a repo as a tab (§5.2). Non-usable opens surface an error
   *  (empty-state error when no tabs, else a toast) and add no tab. */
  const openTab = useCallback(
    async (path: string): Promise<void> => {
      setError(null);
      try {
        const { repoId, info } = await ipc.openRepo(path);
        if (!isUsableRepo(info)) {
          const msg = unusableRepoMessage(info);
          if (tabsRef.current.length > 0) pushToast('error', msg);
          else setError(msg);
          return;
        }
        void refreshRecents();
        if (tabsRef.current.some((t) => t.repoId === repoId)) {
          setActiveRepo(repoId); // focus existing tab
          return;
        }
        setTabs((cur) =>
          cur.some((t) => t.repoId === repoId) ? cur : [...cur, { repoId, path: info.path }],
        );
        setActiveRepo(repoId);
      } catch (e) {
        const msg = errorMessage(e);
        if (isAppError(e) && e.kind === 'io') {
          void ipc.removeRecentRepo(path).then(setRecents, () => {
            // Non-fatal: the recents prune is best-effort; the stale entry
            // simply survives until the next successful open.
          });
        }
        if (tabsRef.current.length > 0) pushToast('error', msg);
        else setError(msg);
      }
    },
    [pushToast, refreshRecents],
  );

  const closeTab = useCallback((repoId: string) => {
    void ipc.closeRepo(repoId).catch(() => {
      // Idempotent teardown — a failure to close is non-fatal for the UI.
    });
    const cur = tabsRef.current;
    const idx = cur.findIndex((t) => t.repoId === repoId);
    setTabs(cur.filter((t) => t.repoId !== repoId));
    setActiveRepo((act) => {
      if (act !== repoId) return act;
      const next = cur.filter((t) => t.repoId !== repoId);
      if (next.length === 0) return null;
      return next[Math.min(idx, next.length - 1)].repoId;
    });
  }, []);

  // P3e §5.6 (issue 4): reorder open tabs by drag-and-drop. Immutable array
  // move; the tabs-change effect persists the new order via setSession.
  const reorderTabs = useCallback((from: number, to: number) => {
    setTabs((cur) => {
      if (
        from === to ||
        from < 0 ||
        to < 0 ||
        from >= cur.length ||
        to >= cur.length
      ) {
        return cur;
      }
      const next = cur.slice();
      const [moved] = next.splice(from, 1);
      next.splice(to, 0, moved);
      return next;
    });
  }, []);

  // Picker path (Ctrl+O + TabStrip Browse…): pick a folder, open it as a tab.
  const handleOpenRepository = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      const path = await ipc.pickFolder();
      if (path === null) return; // cancelled
      await openTab(path);
    } catch (e) {
      // openTab handles its own errors; this catches a picker failure so the
      // rejection never escapes the event handler (non-fatal).
      pushToast('error', errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [openTab, pushToast]);

  // New repository: folder picker → init → openTab (no dialog needed).
  const handleInitRepository = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      const path = await ipc.pickFolder();
      if (path === null) return; // cancelled
      const repoPath = await ipc.initRepo(path);
      await openTab(repoPath);
    } catch (e) {
      const msg = errorMessage(e);
      if (tabsRef.current.length > 0) pushToast('error', msg);
      else setError(msg);
    } finally {
      setLoading(false);
    }
  }, [openTab, pushToast]);

  return {
    error,
    loading,
    recents,
    setRecents,
    refreshRecents,
    tabs,
    setTabs,
    activeRepo,
    setActiveRepo,
    tabsRef,
    sessionReadyRef,
    openTab,
    closeTab,
    reorderTabs,
    handleOpenRepository,
    handleInitRepository,
  };
}
