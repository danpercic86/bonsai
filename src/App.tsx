import { useCallback, useEffect, useRef, useState } from 'react';
import { ConfirmDialog } from './components/ConfirmDialog';
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
  AiAutonomy,
  AiAvailability,
  AutoFetchSettings,
  GraphPrefs,
  ListView,
  McpStatus,
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
  // P13 §8: AI assistance settings (App-owned; threaded to Settings + each
  // workspace). Consent is a one-time gate — enabling without it opens a dialog.
  const [aiEnabled, setAiEnabled] = useState(true);
  const [aiConflictAutonomy, setAiConflictAutonomy] = useState<AiAutonomy>('proposeReview');
  const [aiConsented, setAiConsented] = useState(false);
  // CLI health probe result; null while probing. Re-fetched on Settings open and
  // on repo open (§8.3). A req-id guards against out-of-order probe resolutions.
  const [aiAvailability, setAiAvailability] = useState<AiAvailability | null>(null);
  const aiProbeIdRef = useRef(0);
  // Consent ConfirmDialog (opened by SettingsPanel's enable toggle when consent
  // has not yet been recorded).
  const [consentOpen, setConsentOpen] = useState(false);
  // P16: embedded MCP server. `mcpStatus` is the live runtime state (from the
  // backend, kept fresh via `mcp-server-changed`); `mcpConsented` is the
  // one-time consent gate for the enable toggle; the dialog defers enabling.
  const [mcpStatus, setMcpStatus] = useState<McpStatus | null>(null);
  const [mcpConsented, setMcpConsented] = useState(false);
  const [mcpConsentOpen, setMcpConsentOpen] = useState(false);
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
      if (patch.aiEnabled !== undefined) setAiEnabled(patch.aiEnabled);
      if (patch.aiConflictAutonomy !== undefined) setAiConflictAutonomy(patch.aiConflictAutonomy);
      if (patch.aiConsented !== undefined) setAiConsented(patch.aiConsented);
      if (patch.mcpConsented !== undefined) setMcpConsented(patch.mcpConsented);
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

  // P13 §8.3: probe the Claude Code CLI. Re-runnable; a req-id guards against a
  // stale probe overwriting a newer result. Never throws (the IPC never rejects
  // for CLI state) — a rejection just leaves the last-known availability.
  const probeAiAvailability = useCallback(() => {
    const id = ++aiProbeIdRef.current;
    void ipc
      .checkAiAvailability()
      .then((a) => {
        if (id === aiProbeIdRef.current) setAiAvailability(a);
      })
      .catch(() => {
        // Non-fatal — keep the last-known availability.
      });
  }, []);

  // Probe on Settings open (fresh status for the AI section) and whenever a repo
  // becomes active (§8.3). Cheap enough to re-run; the req-id dedupes races.
  useEffect(() => {
    if (settingsOpen) probeAiAvailability();
  }, [settingsOpen, probeAiAvailability]);
  useEffect(() => {
    if (activeRepo !== null) probeAiAvailability();
  }, [activeRepo, probeAiAvailability]);

  // Consent flow (§8.4): the Settings enable toggle defers here when consent has
  // not been recorded; confirming records BOTH enable + consent in one patch.
  const handleConfirmConsent = useCallback(() => {
    setConsentOpen(false);
    handleSettingsChange({ aiEnabled: true, aiConsented: true });
  }, [handleSettingsChange]);

  // P16: load the embedded-MCP status once and stay live via `mcp-server-changed`.
  useEffect(() => {
    let unsub: (() => void) | null = null;
    let cancelled = false;
    ipc.getMcpStatus().then(
      (s) => {
        if (!cancelled) setMcpStatus(s);
      },
      () => {
        // Non-fatal — the Settings section renders a stopped placeholder.
      },
    );
    ipc.onMcpServerChanged((s) => setMcpStatus(s)).then(
      (u) => {
        if (cancelled) u();
        else unsub = u;
      },
      () => {},
    );
    return () => {
      cancelled = true;
      if (unsub !== null) unsub();
    };
  }, []);

  // P16: start/stop the embedded MCP server; keep `mcpStatus` in sync (the
  // `mcp-server-changed` subscription also updates it, but this is immediate).
  const handleSetMcpEnabled = useCallback(
    (enabled: boolean) => {
      ipc.setMcpEnabled(enabled).then(
        (s) => setMcpStatus(s),
        (e) => pushToast('error', `Could not ${enabled ? 'start' : 'stop'} MCP server: ${errorMessage(e)}`),
      );
    },
    [pushToast],
  );

  // Enabling the MCP server the first time records consent, then starts it.
  const handleConfirmMcpConsent = useCallback(() => {
    setMcpConsentOpen(false);
    handleSettingsChange({ mcpConsented: true });
    handleSetMcpEnabled(true);
  }, [handleSettingsChange, handleSetMcpEnabled]);

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
        setAiEnabled(s.aiEnabled);
        setAiConflictAutonomy(s.aiConflictAutonomy);
        setAiConsented(s.aiConsented);
        setMcpConsented(s.mcpConsented);
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

  const globalModalOpen =
    overlayOpen || menuOpen || settingsOpen || consentOpen || mcpConsentOpen;

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
                aiEnabled={aiEnabled}
                aiConflictAutonomy={aiConflictAutonomy}
                aiConsented={aiConsented}
                aiAvailability={aiAvailability}
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
          aiEnabled={aiEnabled}
          aiConflictAutonomy={aiConflictAutonomy}
          aiConsented={aiConsented}
          aiAvailability={aiAvailability}
          onRequestEnableAi={() => setConsentOpen(true)}
          mcpStatus={mcpStatus}
          mcpConsented={mcpConsented}
          onSetMcpEnabled={handleSetMcpEnabled}
          onRequestEnableMcp={() => setMcpConsentOpen(true)}
        />
        <ConfirmDialog
          open={consentOpen}
          title="Enable AI features?"
          confirmLabel="Enable"
          busy={false}
          onConfirm={handleConfirmConsent}
          onCancel={() => setConsentOpen(false)}
        >
          <div>
            Bonsai will send the contents of conflicted files to the Claude Code CLI installed on
            this machine, under your Claude subscription. Nothing is sent to Bonsai's own servers,
            and no files are changed without your review. Enable AI features?
          </div>
        </ConfirmDialog>
        <ConfirmDialog
          open={mcpConsentOpen}
          title="Enable MCP server?"
          confirmLabel="Enable"
          busy={false}
          onConfirm={handleConfirmMcpConsent}
          onCancel={() => setMcpConsentOpen(false)}
        >
          <div>
            Bonsai will run a local MCP server on 127.0.0.1 that lets an external AI client (e.g.
            Claude Code) read <strong>any repository you have open in Bonsai</strong>. Access
            requires a secret token shown in Settings; nothing is exposed to the network. The server
            is read-only. Enable the MCP server?
          </div>
        </ConfirmDialog>
        <Toasts toasts={toasts} onDismiss={dismissToast} />
      </div>
    </ToastContext.Provider>
  );
}
