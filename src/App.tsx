import { useCallback, useEffect, useRef, useState } from 'react';
import { RepoWorkspace } from './components/RepoWorkspace';
import { SettingsPanel } from './components/SettingsPanel';
import { ShortcutOverlay } from './components/ShortcutOverlay';
import { TabStrip } from './components/TabStrip';
import type { TabMeta } from './components/TabStrip';
import { Toasts } from './components/Toasts';
import type { Toast, ToastTone } from './components/Toasts';
import { ToastContext } from './ToastContext';
import { ipc } from './ipc';
import type {
  AutoFetchSettings,
  GraphPrefs,
  ListView,
  PaneWidths,
  RecentRepo,
  RepoInfo,
  SessionState,
  Theme,
  UiSettingsPatch,
} from './ipc';
import { errorMessage, isAppError } from './utils/errors';

function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] ?? path;
}

function isUsableRepo(info: RepoInfo): boolean {
  return info.isRepo && !info.bare;
}

function unusableRepoMessage(info: RepoInfo): string {
  return info.isRepo
    ? `Bare repositories are not supported: ${info.path}`
    : `Not a Git repository: ${info.path}`;
}

// P2a §2.5: persisted-sanity clamp ranges (mirrors settings.rs clamp_pane_widths).
const SIDEBAR_MIN = 180;
const SIDEBAR_MAX = 480;
const RIGHT_PANEL_MIN = 280;
const RIGHT_PANEL_MAX = 640;
const GRAPH_MIN_WIDTH = 480;
const DEFAULT_PANE_WIDTHS: PaneWidths = { sidebar: 240, rightPanel: 380 };

/** Live-drag clamp (§2.5): the persisted range intersected with the current
 * window size and the graph pane's floor. */
function clampLive(value: number, side: 'sidebar' | 'rightPanel', otherWidth: number): number {
  const [min, max] = side === 'sidebar' ? [SIDEBAR_MIN, SIDEBAR_MAX] : [RIGHT_PANEL_MIN, RIGHT_PANEL_MAX];
  const dynamicMax = Math.min(max, window.innerWidth - otherWidth - GRAPH_MIN_WIDTH);
  return Math.max(min, Math.min(value, Math.max(min, dynamicMax)));
}

/** P2b §4.2: sets data-theme on <html>. */
function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme === 'light' ? 'light' : 'dark');
}

export default function App() {
  // ----- App-global state (§5.1) -----
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  const [toasts, setToasts] = useState<Toast[]>([]);
  const toastId = useRef(0);

  const [overlayOpen, setOverlayOpen] = useState(false);
  // TabStrip's `+` menu lift — suppresses global shortcuts + the consumed Esc.
  const [menuOpen, setMenuOpen] = useState(false);

  const [recents, setRecents] = useState<RecentRepo[]>([]);

  const [paneWidths, setPaneWidths] = useState<PaneWidths>(DEFAULT_PANE_WIDTHS);
  const paneWidthsRef = useRef(paneWidths);
  paneWidthsRef.current = paneWidths;
  const saveTimerRef = useRef<number | null>(null);

  const [theme, setTheme] = useState<Theme>('dark');
  const [themeVersion, setThemeVersion] = useState(0);
  const [listView, setListView] = useState<ListView>('tree');

  // P11c §3.2: Settings page + the live-preview knob state it drives.
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [autoFetch, setAutoFetch] = useState<AutoFetchSettings>({
    enabled: false,
    intervalMinutes: 5,
  });
  const [graph, setGraph] = useState<GraphPrefs>({
    dotRadius: 4,
    avatarRadius: 10,
    rowHeight: 32,
    laneWidth: 16,
  });
  // P11d §4.3: bumped on every graph-knob change → GraphCanvas full re-measure.
  const [metricsVersion, setMetricsVersion] = useState(0);
  // P11c §3.2: debounced settings persist — accumulates partial patches so a
  // burst of knob changes within the window all reach disk in one write.
  const settingsSaveTimerRef = useRef<number | null>(null);
  const pendingSettingsPatchRef = useRef<UiSettingsPatch>({});

  // ----- Tab state (§5.2) -----
  const [tabs, setTabs] = useState<TabMeta[]>([]);
  const [activeRepo, setActiveRepo] = useState<string | null>(null);
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  const dismissToast = useCallback((id: number) => {
    setToasts((cur) => cur.filter((t) => t.id !== id));
  }, []);

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

  // Persist on any tab / active change once launch reopen has settled.
  useEffect(() => {
    if (!sessionReadyRef.current) return;
    persistSession(tabs.map((t) => t.repoId), activeRepo);
  }, [tabs, activeRepo, persistSession]);

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
          void ipc.removeRecentRepo(path).then(setRecents);
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

  // P11c §3.2: apply a Settings patch — update local state immediately (live
  // preview; graph changes bump metricsVersion so the canvas re-measures), then
  // debounce a single merged persist (~300 ms, mirrors commitPaneWidths).
  const handleSettingsChange = useCallback(
    (patch: UiSettingsPatch) => {
      if (patch.autoFetch !== undefined) setAutoFetch(patch.autoFetch);
      if (patch.graph !== undefined) {
        setGraph(patch.graph);
        setMetricsVersion((v) => v + 1);
      }
      pendingSettingsPatchRef.current = { ...pendingSettingsPatchRef.current, ...patch };
      if (settingsSaveTimerRef.current !== null) {
        window.clearTimeout(settingsSaveTimerRef.current);
      }
      settingsSaveTimerRef.current = window.setTimeout(() => {
        const merged = pendingSettingsPatchRef.current;
        pendingSettingsPatchRef.current = {};
        void ipc
          .setUiSettings(merged)
          .catch((e) => pushToast('error', `Could not save settings: ${errorMessage(e)}`));
      }, 300);
    },
    [pushToast],
  );

  const handleSidebarResize = useCallback((delta: number) => {
    setPaneWidths((w) => ({ ...w, sidebar: clampLive(w.sidebar + delta, 'sidebar', w.rightPanel) }));
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

  // Picker path (Ctrl+O + TabStrip Browse…): pick a folder, open it as a tab.
  const handleOpenRepository = useCallback(async () => {
    setError(null);
    setLoading(true);
    try {
      const path = await ipc.pickFolder();
      if (path === null) return; // cancelled
      await openTab(path);
    } finally {
      setLoading(false);
    }
  }, [openTab]);

  // ----- Reopen-all-on-launch (§6.2) -----
  const launchedRef = useRef(false);
  useEffect(() => {
    if (launchedRef.current) return;
    launchedRef.current = true;
    (async () => {
      // UI settings first (theme/panes/listView).
      try {
        const s = await ipc.getUiSettings();
        setPaneWidths(s.paneWidths);
        setTheme(s.theme);
        applyTheme(s.theme);
        setThemeVersion((v) => v + 1);
        setListView(s.listView);
        setAutoFetch(s.autoFetch);
        setGraph(s.graph);
        setMetricsVersion((v) => v + 1);
      } catch {
        // Non-fatal — keep defaults.
      }

      let recentsList: RecentRepo[] = [];
      try {
        recentsList = await ipc.getRecentRepos();
        setRecents(recentsList);
      } catch {
        // Non-fatal.
      }

      let session: SessionState = { openRepos: [], activeRepo: null };
      try {
        session = await ipc.getSession();
      } catch {
        // Non-fatal — defaults to empty.
      }

      // Back-compat (§6.2.5): no persisted session → reopen the most-recent repo.
      const usingSession = session.openRepos.length > 0;
      const pathsToOpen = usingSession
        ? session.openRepos
        : recentsList.length > 0
          ? [recentsList[0].path]
          : [];

      const opened: TabMeta[] = [];
      for (const path of pathsToOpen) {
        try {
          const { repoId, info } = await ipc.openRepo(path);
          if (!isUsableRepo(info)) {
            pushToast('warning', `Could not reopen ${folderName(path)}: not a usable repository`);
            continue;
          }
          if (!opened.some((t) => t.repoId === repoId)) {
            opened.push({ repoId, path: info.path });
          }
        } catch (e) {
          pushToast('warning', `Could not reopen ${folderName(path)}: ${errorMessage(e)}`);
        }
      }

      const openedIds = opened.map((t) => t.repoId);
      const active =
        usingSession && session.activeRepo !== null && openedIds.includes(session.activeRepo)
          ? session.activeRepo
          : (openedIds[0] ?? null);

      setTabs(opened);
      setActiveRepo(active);
      if (opened.length > 0) void refreshRecents();

      // Prune dead paths from disk (§6.2.4); also seed the session file for
      // existing users migrating in via the back-compat recents[0] path so
      // their tab is persisted from launch (not only after they touch tabs).
      if (usingSession || opened.length > 0) {
        try {
          await ipc.setSession({ openRepos: openedIds, activeRepo: active });
        } catch {
          // Non-fatal.
        }
      }
      sessionReadyRef.current = true;
    })();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Esc: close the shortcut overlay (TabStrip's own Esc handles its menu; skip
  // when its menu consumed the keypress). Workspace Esc-layering is separate.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (menuOpen) return;
      if (settingsOpen) setSettingsOpen(false);
      if (overlayOpen) setOverlayOpen(false);
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [menuOpen, overlayOpen, settingsOpen]);

  // Global shortcuts (§5.1): Ctrl+O open, ? overlay, Ctrl+Tab / Ctrl+Shift+Tab
  // cycle tabs, Ctrl+W close active tab.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const ctrl = e.ctrlKey || e.metaKey;

      const target = e.target as HTMLElement | null;
      const typing =
        target !== null &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.tagName === 'SELECT' ||
          target.isContentEditable);

      if (menuOpen) return;

      if (ctrl && e.key.toLowerCase() === 'o') {
        e.preventDefault();
        void handleOpenRepository();
        return;
      }

      if (ctrl && e.key === 'Tab') {
        e.preventDefault();
        const cur = tabsRef.current;
        if (cur.length === 0) return;
        const idx = cur.findIndex((t) => t.repoId === activeRepo);
        const base = idx === -1 ? 0 : idx;
        const nextIdx = (base + (e.shiftKey ? -1 : 1) + cur.length) % cur.length;
        setActiveRepo(cur[nextIdx].repoId);
        return;
      }

      if (typing) return;

      // Ctrl+W gated behind the typing guard: word-delete muscle memory in the
      // commit box must not close the tab (and lose the unsent message).
      if (ctrl && e.key.toLowerCase() === 'w') {
        e.preventDefault();
        if (activeRepo !== null) closeTab(activeRepo);
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
  }, [menuOpen, activeRepo, handleOpenRepository, closeTab]);

  const globalModalOpen = overlayOpen || menuOpen || settingsOpen;

  return (
    <ToastContext.Provider value={pushToast}>
      <div className="app">
        <header className="header">
          <span className="app-name">Bonsai</span>
          <TabStrip
            tabs={tabs}
            activeRepo={activeRepo}
            recents={recents}
            disabled={loading}
            onSelect={setActiveRepo}
            onClose={closeTab}
            onOpenPath={(path) => void openTab(path)}
            onBrowse={() => void handleOpenRepository()}
            onMenuOpenChange={setMenuOpen}
          />
          <div className="header-toolbar">
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
              className="btn-icon settings-toggle"
              onClick={() => setSettingsOpen(true)}
              title="Settings"
              aria-label="Settings"
            >
              {'⚙'}
            </button>
          </div>
        </header>

        {tabs.length > 0 ? (
          tabs.map((t) => (
            <div
              key={t.repoId}
              className="workspace-host"
              style={{ display: t.repoId === activeRepo ? 'flex' : 'none' }}
            >
              <RepoWorkspace
                repoId={t.repoId}
                active={t.repoId === activeRepo}
                listView={listView}
                themeVersion={themeVersion}
                paneWidths={paneWidths}
                globalModalOpen={globalModalOpen}
                graph={graph}
                metricsVersion={metricsVersion}
                autoFetch={autoFetch}
                onSidebarResize={handleSidebarResize}
                onRightPanelResize={handleRightPanelResize}
                onPaneResizeEnd={handlePaneResizeEnd}
              />
            </div>
          ))
        ) : (
          <div className="empty-state">
            <h1 className="empty-title">Bonsai</h1>
            <p className="empty-tagline">A tidy Git client</p>
            {error !== null && <div className="error-banner">{error}</div>}
            <button
              type="button"
              className="btn-primary"
              onClick={() => void handleOpenRepository()}
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
                    onClick={() => void openTab(r.path)}
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
        <SettingsPanel
          open={settingsOpen}
          onClose={() => setSettingsOpen(false)}
          theme={theme}
          listView={listView}
          autoFetch={autoFetch}
          graph={graph}
          onChange={handleSettingsChange}
          onToggleTheme={toggleTheme}
          onToggleListView={toggleListView}
        />
        <Toasts toasts={toasts} onDismiss={dismissToast} />
      </div>
    </ToastContext.Provider>
  );
}
