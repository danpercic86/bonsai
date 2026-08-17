import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { CloneDialog, deriveRepoName, joinRepoPath } from './components/CloneDialog';
import { ConfirmDialog } from './components/ConfirmDialog';
import { ContextMenu } from './components/ContextMenu';
import { RepoWorkspace } from './components/RepoWorkspace';
import { SettingsPanel } from './components/SettingsPanel';
import { externalToolsItems } from './components/workspaceMenus';
import type { PaletteAction } from './components/paletteActions';
import { AiAssetsPanel } from './components/AiAssetsPanel';
import { RepoHealthPanel } from './components/RepoHealthPanel';
import { OnboardingOverlay } from './components/OnboardingOverlay';
import { EmptyState } from './components/EmptyState';
import { ShortcutOverlay } from './components/ShortcutOverlay';
import { TabStrip } from './components/TabStrip';
import type { TabMeta } from './components/TabStrip';
import { Toasts } from './components/Toasts';
import type { Toast, ToastTone } from './components/Toasts';
import { UpdateNotification } from './components/UpdateNotification';
import { UpdateDialog } from './components/UpdateDialog';
import { useUpdateController } from './hooks/useUpdateController';
import { ToastContext } from './ToastContext';
import { ipc } from './ipc';
import type {
  AiAutonomy,
  AiAvailability,
  AutoFetchSettings,
  CloneProgress,
  GraphPrefs,
  HealthRefreshSettings,
  IdentityProfile,
  ListView,
  McpStatus,
  PaneWidths,
  PanelDensity,
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
  // P67 §4: right-panel density. No toolbar button (unlike theme/listView), so
  // it rides the debounced `handleSettingsChange` patch path only.
  const [panelDensity, setPanelDensity] = useState<PanelDensity>('cozy');

  // P11c §3.2: Settings page + the live-preview knob state it drives.
  const [settingsOpen, setSettingsOpen] = useState(false);
  // P40b: when opened from a `configMissing` commit error, focus the Git-config
  // Identity sub-section; cleared when the panel closes.
  const [configFocus, setConfigFocus] = useState<'identity' | null>(null);
  const openIdentitySettings = useCallback(() => {
    setConfigFocus('identity');
    setSettingsOpen(true);
  }, []);
  // P43a: re-open the onboarding overlay (Settings "Show welcome tour"); does
  // NOT reset the seen flag.
  const showOnboarding = useCallback(() => {
    setSettingsOpen(false);
    setConfigFocus(null);
    setOnboardingOpen(true);
  }, []);
  // P24d: AI-asset inventory / drift / context-profile overlay (active repo only).
  const [aiAssetsOpen, setAiAssetsOpen] = useState(false);
  // P29c: read-only repo-health overlay (active repo only).
  const [healthOpen, setHealthOpen] = useState(false);
  // P43a: first-run onboarding overlay. Opened at startup when `onboardingSeen`
  // is false (or `?onboarding=1`); re-openable from Settings. Dismissal persists
  // `onboardingSeen: true` so it does not reappear.
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  const [autoFetch, setAutoFetch] = useState<AutoFetchSettings>({
    enabled: false,
    intervalMinutes: 5,
  });
  // P30: healthRefresh background job (backend scheduler; Settings UI only).
  const [healthRefresh, setHealthRefresh] = useState<HealthRefreshSettings>({
    enabled: false,
    intervalMinutes: 30,
  });
  const [graph, setGraph] = useState<GraphPrefs>({
    avatarRadius: 10,
    rowHeight: 32,
    laneWidth: 16,
    // P51: per-row detail toggles (mirror GraphPrefs::default in settings.rs).
    showSha: true,
    showAuthor: false,
    showDate: true,
    dateBasis: 'author',
    showAheadBehind: true,
    compact: false,
    showSignatureBadge: true,
    // P63: forge signal badges OFF by default (network+auth-gated, opt-in).
    showPrBadge: false,
    showCiStatus: false,
  });
  // P11d §4.3: bumped on every graph-knob change → GraphCanvas full re-measure.
  const [metricsVersion, setMetricsVersion] = useState(0);
  // P13 §8: AI assistance settings (App-owned; threaded to Settings + each
  // workspace). Consent is a one-time gate — enabling without it opens a dialog.
  const [aiEnabled, setAiEnabled] = useState(true);
  const [aiConflictAutonomy, setAiConflictAutonomy] = useState<AiAutonomy>('proposeReview');
  const [aiConsented, setAiConsented] = useState(false);
  // P68e §8: the AI activity dock's persisted geometry. Both ride the debounced
  // `handleSettingsChange` patch path (one write per drag / per toggle), and
  // `aiStreamLog` is threaded down so the dock can say "live output is off"
  // instead of showing an empty log the user reads as another dead button.
  const [aiDockHeight, setAiDockHeight] = useState(180);
  const [aiDockCollapsed, setAiDockCollapsed] = useState(false);
  const [aiStreamLog, setAiStreamLog] = useState(true);
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
  // P16c: the write-gate has its own one-time consent (a stronger grant than
  // read) and its own defer-to-dialog flow.
  const [mcpWriteConsented, setMcpWriteConsented] = useState(false);
  const [mcpWriteConsentOpen, setMcpWriteConsentOpen] = useState(false);
  // P42b: auto-check-for-updates-on-launch preference (persisted; default OFF).
  const [autoCheckUpdates, setAutoCheckUpdates] = useState(false);
  // P44: named identity profiles (global). Source of truth for the Settings
  // section; persisted via handleSettingsChange like every other setting.
  const [profiles, setProfiles] = useState<IdentityProfile[]>([]);
  // P49b: external-tool command templates ('' ⇒ backend auto-detects per-OS).
  // Threaded into the Settings section; persisted via handleSettingsChange.
  const [terminalCommand, setTerminalCommand] = useState('');
  const [editorCommand, setEditorCommand] = useState('');
  // P49b: per-tab "Open externally" context menu (App owns it — the strip spans
  // all tabs). Holds the right-clicked tab's repo path + anchor point.
  const [tabMenu, setTabMenu] = useState<{ path: string; x: number; y: number } | null>(null);
  // P42b: the update state machine (check/notify/download/restart) lives here so
  // App only wires the notification, dialog, and Settings section to it.
  const update = useUpdateController();
  // P11c §3.2: debounced settings persist — accumulates partial patches so a
  // burst of knob changes within the window all reach disk in one write.
  const settingsSaveTimerRef = useRef<number | null>(null);
  const pendingSettingsPatchRef = useRef<UiSettingsPatch>({});

  // ----- Tab state (§5.2) -----
  const [tabs, setTabs] = useState<TabMeta[]>([]);
  const [activeRepo, setActiveRepo] = useState<string | null>(null);
  const tabsRef = useRef(tabs);
  tabsRef.current = tabs;

  // ----- Clone/init lifecycle (P21) -----
  const [cloneOpen, setCloneOpen] = useState(false);
  const [cloneDest, setCloneDest] = useState<string | null>(null);
  const [cloneProgress, setCloneProgress] = useState<CloneProgress | null>(null);
  const [cloneBusy, setCloneBusy] = useState(false);
  const [cloneError, setCloneError] = useState<string | null>(null);
  // Session token: a late progress tick / resolution from a cancelled (or
  // superseded) clone must not write state for the current dialog session.
  const cloneSessionRef = useRef(0);

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

  // P43a: close onboarding (Skip/Finish/Esc/✕) and persist `onboardingSeen` so
  // it does not reappear on the next launch.
  const closeOnboarding = useCallback(() => {
    setOnboardingOpen(false);
    void ipc
      .setUiSettings({ onboardingSeen: true })
      .catch((e) => pushToast('error', `Could not save onboarding state: ${errorMessage(e)}`));
  }, [pushToast]);

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
      if (patch.panelDensity !== undefined) setPanelDensity(patch.panelDensity);
      if (patch.autoFetch !== undefined) setAutoFetch(patch.autoFetch);
      if (patch.healthRefresh !== undefined) setHealthRefresh(patch.healthRefresh);
      if (patch.graph !== undefined) {
        setGraph(patch.graph);
        setMetricsVersion((v) => v + 1);
      }
      if (patch.aiEnabled !== undefined) setAiEnabled(patch.aiEnabled);
      if (patch.aiConflictAutonomy !== undefined) setAiConflictAutonomy(patch.aiConflictAutonomy);
      if (patch.aiConsented !== undefined) setAiConsented(patch.aiConsented);
      if (patch.mcpConsented !== undefined) setMcpConsented(patch.mcpConsented);
      if (patch.mcpWriteConsented !== undefined) setMcpWriteConsented(patch.mcpWriteConsented);
      if (patch.autoCheckUpdates !== undefined) setAutoCheckUpdates(patch.autoCheckUpdates);
      if (patch.profiles !== undefined) setProfiles(patch.profiles);
      if (patch.terminalCommand !== undefined) setTerminalCommand(patch.terminalCommand);
      if (patch.editorCommand !== undefined) setEditorCommand(patch.editorCommand);
      if (patch.aiDockHeight !== undefined) setAiDockHeight(patch.aiDockHeight);
      if (patch.aiDockCollapsed !== undefined) setAiDockCollapsed(patch.aiDockCollapsed);
      if (patch.aiStreamLog !== undefined) setAiStreamLog(patch.aiStreamLog);
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

  // P49b: external-tool launchers for the per-tab context menu (the strip is
  // App-owned, spanning all tabs). Same shape as RepoWorkspace's — never gated
  // by any repo op; failures surface via the shared AppError→toast path.
  const openInTerminal = useCallback(
    (path: string) => {
      void ipc.openInTerminal(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  const revealInFileManager = useCallback(
    (path: string) => {
      void ipc.revealInFileManager(path).catch((e) => pushToast('error', errorMessage(e)));
    },
    [pushToast],
  );
  const openInEditor = useCallback(
    (path: string) => {
      void ipc.openInEditor(path).catch((e) => pushToast('error', errorMessage(e)));
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

  // P16: run `claude mcp add` for the running server at the chosen scope. Returns
  // the promise so SettingsPanel can clear its in-flight state when it settles.
  const handleRegisterMcp = useCallback(
    (scope: 'user' | 'local'): Promise<void> =>
      ipc.registerMcpWithClaude(scope, activeRepo).then(
        () => pushToast('success', `Registered bonsai with Claude Code (${scope})`),
        (e) => {
          pushToast('error', `Could not register: ${errorMessage(e)}`);
        },
      ),
    [pushToast, activeRepo],
  );

  // Enabling the MCP server the first time records consent, then starts it.
  const handleConfirmMcpConsent = useCallback(() => {
    setMcpConsentOpen(false);
    handleSettingsChange({ mcpConsented: true });
    handleSetMcpEnabled(true);
  }, [handleSettingsChange, handleSetMcpEnabled]);

  // P16c: flip the write-gate; the running server BOUNCES (stop+restart on the
  // same token/port), so `mcpStatus` updates both from this resolve and the
  // `mcp-server-changed` re-emit.
  const handleSetMcpAllowWrite = useCallback(
    (allowWrite: boolean) => {
      ipc.setMcpAllowWrite(allowWrite).then(
        (s) => setMcpStatus(s),
        (e) =>
          pushToast(
            'error',
            `Could not ${allowWrite ? 'enable' : 'disable'} MCP write access: ${errorMessage(e)}`,
          ),
      );
    },
    [pushToast],
  );

  // First enabling write records the stronger write consent, then flips the gate.
  const handleConfirmMcpWriteConsent = useCallback(() => {
    setMcpWriteConsentOpen(false);
    handleSettingsChange({ mcpWriteConsented: true });
    handleSetMcpAllowWrite(true);
  }, [handleSettingsChange, handleSetMcpAllowWrite]);

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
    } catch (e) {
      // openTab handles its own errors; this catches a picker failure so the
      // rejection never escapes the event handler (non-fatal).
      pushToast('error', errorMessage(e));
    } finally {
      setLoading(false);
    }
  }, [openTab, pushToast]);

  // ----- Clone (P21) -----
  const handleCloneOpen = useCallback(() => {
    cloneSessionRef.current += 1; // invalidate any in-flight clone's UI updates
    setCloneDest(null);
    setCloneProgress(null);
    setCloneError(null);
    setCloneBusy(false);
    setCloneOpen(true);
  }, []);

  const handleCloneCancel = useCallback(() => {
    // The backend clone keeps running (no cancellation in v1); we simply stop
    // updating the UI — invalidate the session so late ticks are ignored.
    cloneSessionRef.current += 1;
    setCloneOpen(false);
  }, []);

  const handleClonePickDest = useCallback(async () => {
    try {
      const path = await ipc.pickFolder();
      if (path !== null) setCloneDest(path);
    } catch (e) {
      // Surface in the clone dialog; a picker failure is non-fatal.
      setCloneError(errorMessage(e));
    }
  }, []);

  const handleCloneSubmit = useCallback(
    async (url: string) => {
      if (cloneDest === null) return;
      // Frontend derives the repo name from the URL and computes the full dest
      // = <parent>/<name>; the backend clones INTO an empty/new dest.
      const dest = joinRepoPath(cloneDest, deriveRepoName(url));
      const session = cloneSessionRef.current + 1;
      cloneSessionRef.current = session;
      setCloneBusy(true);
      setCloneError(null);
      setCloneProgress(null);
      try {
        const path = await ipc.cloneRepo(url, dest, (p) => {
          if (cloneSessionRef.current === session) setCloneProgress(p);
        });
        if (cloneSessionRef.current !== session) return; // cancelled/superseded
        setCloneOpen(false);
        await openTab(path);
      } catch (e) {
        if (cloneSessionRef.current === session) setCloneError(errorMessage(e));
      } finally {
        if (cloneSessionRef.current === session) setCloneBusy(false);
      }
    },
    [cloneDest, openTab],
  );

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

  // P50c: App-level command-palette entries — everything valid app-wide. Threaded
  // down to every RepoWorkspace, which merges them with its repo-scoped actions.
  // The setState-based openers are stable; only the useCallback handlers are deps.
  const appCommands = useMemo<PaletteAction[]>(
    () => [
      {
        id: 'app.openRepo',
        title: 'Open repository…',
        hint: 'Ctrl+O',
        group: 'action',
        keywords: 'folder browse',
        run: () => void handleOpenRepository(),
      },
      {
        id: 'app.clone',
        title: 'Clone repository…',
        group: 'action',
        keywords: 'git url download',
        run: handleCloneOpen,
      },
      {
        id: 'app.init',
        title: 'New repository…',
        group: 'action',
        keywords: 'init create',
        run: () => void handleInitRepository(),
      },
      {
        id: 'app.settings',
        title: 'Open Settings',
        group: 'action',
        keywords: 'preferences config options',
        run: () => setSettingsOpen(true),
      },
      {
        id: 'app.aiAssets',
        title: 'AI Assets',
        group: 'action',
        keywords: 'agents claude context',
        run: () => setAiAssetsOpen(true),
      },
      {
        id: 'app.health',
        title: 'Repository Health',
        group: 'action',
        keywords: 'stats status',
        run: () => setHealthOpen(true),
      },
      {
        id: 'app.toggleTheme',
        title: 'Toggle theme (light / dark)',
        group: 'action',
        keywords: 'appearance dark light',
        run: toggleTheme,
      },
      {
        id: 'app.toggleListView',
        title: 'Toggle tree / flat lists',
        group: 'action',
        keywords: 'sidebar view branches',
        run: toggleListView,
      },
      {
        id: 'app.shortcuts',
        title: 'Keyboard shortcuts',
        hint: '?',
        group: 'action',
        keywords: 'help keys',
        run: () => setOverlayOpen(true),
      },
    ],
    [handleOpenRepository, handleCloneOpen, handleInitRepository, toggleTheme, toggleListView],
  );

  // ----- Reopen-all-on-launch (§6.2) -----
  const launchedRef = useRef(false);
  useEffect(() => {
    if (launchedRef.current) return;
    launchedRef.current = true;
    // P43a: `?onboarding=1` force-shows the overlay regardless of the flag —
    // the repeatable harness trigger + manual re-find path.
    const forceOnboarding =
      new URLSearchParams(window.location.search).get('onboarding') === '1';
    let showOnboard = forceOnboarding;
    (async () => {
      // UI settings first (theme/panes/listView).
      try {
        const s = await ipc.getUiSettings();
        setPaneWidths(s.paneWidths);
        setTheme(s.theme);
        applyTheme(s.theme);
        setThemeVersion((v) => v + 1);
        setListView(s.listView);
        setPanelDensity(s.panelDensity);
        setAutoFetch(s.autoFetch);
        setHealthRefresh(s.healthRefresh);
        setGraph(s.graph);
        setMetricsVersion((v) => v + 1);
        setAiEnabled(s.aiEnabled);
        setAiConflictAutonomy(s.aiConflictAutonomy);
        setAiConsented(s.aiConsented);
        setMcpConsented(s.mcpConsented);
        setMcpWriteConsented(s.mcpWriteConsented);
        setAutoCheckUpdates(s.autoCheckUpdates);
        setProfiles(s.profiles);
        setTerminalCommand(s.terminalCommand);
        setEditorCommand(s.editorCommand);
        setAiDockHeight(s.aiDockHeight);
        setAiDockCollapsed(s.aiDockCollapsed);
        setAiStreamLog(s.aiStreamLog);
        if (!s.onboardingSeen) showOnboard = true;
        // P42b D4: auto-check on launch when the setting is on. A `?update=`
        // query (harness) forces one too, mirroring `?onboarding=1`. Silent —
        // only an AVAILABLE result surfaces (the notification); up-to-date and
        // errors are swallowed so launch stays quiet.
        const forceUpdateCheck =
          new URLSearchParams(window.location.search).get('update') !== null;
        if (s.autoCheckUpdates || forceUpdateCheck) void update.check(true);
      } catch {
        // Non-fatal — keep defaults.
      }
      if (showOnboard) setOnboardingOpen(true);

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

  // Esc: close only the TOPMOST global overlay per keypress (LIFO peel:
  // shortcut overlay → settings → AI assets → health → onboarding). TabStrip's
  // own Esc handles its menu; skip when it consumed the keypress. Workspace
  // Esc-layering is separate.
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      if (menuOpen) return;
      if (overlayOpen) {
        setOverlayOpen(false);
        return;
      }
      if (settingsOpen) {
        setSettingsOpen(false);
        setConfigFocus(null);
        return;
      }
      if (aiAssetsOpen) {
        setAiAssetsOpen(false);
        return;
      }
      if (healthOpen) {
        setHealthOpen(false);
        return;
      }
      if (onboardingOpen) closeOnboarding();
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [menuOpen, overlayOpen, settingsOpen, aiAssetsOpen, healthOpen, onboardingOpen, closeOnboarding]);

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
    overlayOpen ||
    menuOpen ||
    settingsOpen ||
    aiAssetsOpen ||
    healthOpen ||
    onboardingOpen ||
    consentOpen ||
    mcpConsentOpen ||
    mcpWriteConsentOpen ||
    update.dialogOpen;

  return (
    <ToastContext.Provider value={pushToast}>
      <div className="app">
        <header className="header">
          <TabStrip
            tabs={tabs}
            activeRepo={activeRepo}
            recents={recents}
            disabled={loading}
            onSelect={setActiveRepo}
            onClose={closeTab}
            onReorder={reorderTabs}
            onOpenPath={(path) => void openTab(path)}
            onBrowse={() => void handleOpenRepository()}
            onClone={handleCloneOpen}
            onInit={() => void handleInitRepository()}
            onMenuOpenChange={setMenuOpen}
            onTabMenu={(path, x, y) => setTabMenu({ path, x, y })}
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
            {activeRepo !== null && (
              <button
                type="button"
                className="btn-icon ai-assets-toggle"
                onClick={() => setAiAssetsOpen(true)}
                title="AI Assets"
                aria-label="AI Assets"
              >
                {'🤖'}
              </button>
            )}
            {activeRepo !== null && (
              <button
                type="button"
                className="btn-icon repo-health-toggle"
                onClick={() => setHealthOpen(true)}
                title="Health"
                aria-label="Health"
              >
                {'📊'}
              </button>
            )}
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
                panelDensity={panelDensity}
                themeVersion={themeVersion}
                paneWidths={paneWidths}
                globalModalOpen={globalModalOpen}
                graph={graph}
                metricsVersion={metricsVersion}
                aiEnabled={aiEnabled}
                aiConflictAutonomy={aiConflictAutonomy}
                aiConsented={aiConsented}
                aiAvailability={aiAvailability}
                aiDockHeight={aiDockHeight}
                aiDockCollapsed={aiDockCollapsed}
                aiStreamLog={aiStreamLog}
                onAiDockChange={handleSettingsChange}
                onSidebarResize={handleSidebarResize}
                onRightPanelResize={handleRightPanelResize}
                onPaneResizeEnd={handlePaneResizeEnd}
                onOpenRepoPath={(path) => void openTab(path)}
                onOpenIdentitySettings={openIdentitySettings}
                appCommands={appCommands}
              />
            </div>
          ))
        ) : (
          <EmptyState
            loading={loading}
            error={error}
            recents={recents}
            onOpenRepository={() => void handleOpenRepository()}
            onCloneOpen={handleCloneOpen}
            onInitRepository={() => void handleInitRepository()}
            onOpenRecent={(path) => void openTab(path)}
          />
        )}

        <ShortcutOverlay open={overlayOpen} onClose={() => setOverlayOpen(false)} />
        <OnboardingOverlay
          open={onboardingOpen}
          onClose={closeOnboarding}
          activeRepo={activeRepo}
          recents={recents}
          loading={loading}
          onOpenRepository={() => void handleOpenRepository()}
          onCloneOpen={handleCloneOpen}
          onInitRepository={() => void handleInitRepository()}
          onOpenRecent={(path) => void openTab(path)}
        />
        <SettingsPanel
          open={settingsOpen}
          onClose={() => {
            setSettingsOpen(false);
            setConfigFocus(null);
          }}
          theme={theme}
          listView={listView}
          panelDensity={panelDensity}
          autoFetch={autoFetch}
          healthRefresh={healthRefresh}
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
          mcpWriteConsented={mcpWriteConsented}
          onSetMcpAllowWrite={handleSetMcpAllowWrite}
          onRequestEnableMcpWrite={() => setMcpWriteConsentOpen(true)}
          repoPath={activeRepo}
          configInitialFocus={configFocus}
          profiles={profiles}
          terminalCommand={terminalCommand}
          editorCommand={editorCommand}
          onRegisterMcp={handleRegisterMcp}
          onShowOnboarding={showOnboarding}
          updateCurrentVersion={update.currentVersion}
          autoCheckUpdates={autoCheckUpdates}
          updateState={update.state}
          onCheckUpdate={() => void update.check(false)}
          onOpenUpdateDialog={update.openDialog}
        />
        {activeRepo !== null && (
          <AiAssetsPanel
            open={aiAssetsOpen}
            onClose={() => setAiAssetsOpen(false)}
            repoId={activeRepo}
            aiEnabled={aiEnabled && aiConsented && aiAvailability?.installed === true}
          />
        )}
        {activeRepo !== null && (
          <RepoHealthPanel
            open={healthOpen}
            onClose={() => setHealthOpen(false)}
            repoId={activeRepo}
          />
        )}
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
        <ConfirmDialog
          open={mcpWriteConsentOpen}
          title="Allow AI to modify repositories?"
          confirmLabel="Allow write access"
          busy={false}
          onConfirm={handleConfirmMcpWriteConsent}
          onCancel={() => setMcpWriteConsentOpen(false)}
        >
          <div>
            This grants the connected AI client the ability to <strong>modify</strong> any
            repository you have open in Bonsai — staging, committing, merging, resolving conflicts,
            and other write operations run without a per-action prompt. Changing this setting
            restarts the server and drops any active connection. Allow write access?
          </div>
        </ConfirmDialog>
        <CloneDialog
          open={cloneOpen}
          busy={cloneBusy}
          progress={cloneProgress}
          error={cloneError}
          dest={cloneDest}
          onPickDest={() => void handleClonePickDest()}
          onSubmit={(u) => void handleCloneSubmit(u)}
          onCancel={handleCloneCancel}
        />
        <UpdateDialog
          open={update.dialogOpen}
          state={update.state}
          onDownload={update.download}
          onRestart={update.restart}
          onClose={update.closeDialog}
        />
        {update.notificationVisible && update.state.status === 'available' && (
          <UpdateNotification
            version={update.state.info.version ?? ''}
            onView={update.openDialog}
            onDismiss={update.dismissNotification}
          />
        )}
        {tabMenu !== null && (
          <ContextMenu
            x={tabMenu.x}
            y={tabMenu.y}
            items={externalToolsItems(tabMenu.path, {
              onOpenInTerminal: openInTerminal,
              onRevealInFileManager: revealInFileManager,
              onOpenInEditor: openInEditor,
            })}
            onClose={() => setTabMenu(null)}
          />
        )}
        <Toasts toasts={toasts} onDismiss={dismissToast} />
      </div>
    </ToastContext.Provider>
  );
}
