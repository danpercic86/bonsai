import { useCallback, useEffect, useRef, useState } from 'react';
import { CloneDialog } from './components/CloneDialog';
import { ContextMenu } from './components/ContextMenu';
import { RepoWorkspace } from './components/RepoWorkspace';
import { SettingsPanel } from './components/SettingsPanel';
import { externalToolsItems } from './components/workspaceMenus';
import { AiConsentDialog } from './components/dialogs/AiConsentDialog';
import { McpConsentDialog } from './components/dialogs/McpConsentDialog';
import { McpWriteConsentDialog } from './components/dialogs/McpWriteConsentDialog';
import { AiAssetsPanel } from './components/AiAssetsPanel';
import { RepoHealthPanel } from './components/RepoHealthPanel';
import { OnboardingOverlay } from './components/OnboardingOverlay';
import { EmptyState } from './components/EmptyState';
import { GitMissingBanner } from './components/GitMissingBanner';
import { HeaderToolbar } from './components/HeaderToolbar';
import { ShortcutOverlay } from './components/ShortcutOverlay';
import { TabStrip, type TabMeta } from './components/TabStrip';
import { Toasts } from './components/Toasts';
import { AppUpdateSurfaces } from './components/AppUpdateSurfaces';
import { useAiAvailability } from './hooks/useAiAvailability';
import { useAppCommands } from './hooks/useAppCommands';
import { useCloneFlow } from './hooks/useCloneFlow';
import { useExternalTools } from './hooks/useExternalTools';
import { useMcpControls } from './hooks/useMcpControls';
import { usePaneWidthState } from './hooks/usePaneWidthState';
import { useRepoTabs } from './hooks/useRepoTabs';
import { useSettingsRequest } from './hooks/useSettingsRequest';
import { useToastQueue } from './hooks/useToastQueue';
import { useUpdateController } from './hooks/useUpdateController';
import { useGitAvailability } from './hooks/useGitAvailability';
import { useUiSettings } from './hooks/useUiSettings';
import { ToastContext } from './ToastContext';
import { ipc } from './ipc';
import type { ListView, RecentRepo, SessionState, Theme } from './ipc';
import { errorMessage } from './utils/errors';
import { applyGraphStyle, applyTheme, folderName, isUsableRepo } from './appHelpers';
import { useAppShortcuts } from './hooks/useAppShortcuts';

export default function App() {
  // ----- App-global state (§5.1) -----
  const [overlayOpen, setOverlayOpen] = useState(false);
  // TabStrip's `+` menu lift — suppresses global shortcuts + the consumed Esc.
  const [menuOpen, setMenuOpen] = useState(false);

  const [theme, setTheme] = useState<Theme>('dark');
  const [themeVersion, setThemeVersion] = useState(0);
  const [listView, setListView] = useState<ListView>('tree');

  // P11c §3.2: Settings page + the live-preview knob state it drives.
  // P69h §5.3: open state + the deep-link request (category, focus, monotonic
  // seq) that lands even while Settings is already open.
  const settings = useSettingsRequest();
  // P43a: re-open the onboarding overlay (Settings "Show welcome tour"); does
  // NOT reset the seen flag.
  const showOnboarding = useCallback(() => {
    settings.close();
    setOnboardingOpen(true);
  }, [settings]);
  // P24d: AI-asset inventory / drift / context-profile overlay (active repo only).
  const [aiAssetsOpen, setAiAssetsOpen] = useState(false);
  // P29c: read-only repo-health overlay (active repo only).
  const [healthOpen, setHealthOpen] = useState(false);
  // P43a: first-run onboarding overlay. Opened at startup when `onboardingSeen`
  // is false (or `?onboarding=1`); re-openable from Settings. Dismissal persists
  // `onboardingSeen: true` so it does not reappear.
  const [onboardingOpen, setOnboardingOpen] = useState(false);
  // P49b: per-tab "Open externally" context menu (App owns it — the strip spans
  // all tabs). Holds the right-clicked tab's repo path + anchor point.
  const [tabMenu, setTabMenu] = useState<{ path: string; x: number; y: number } | null>(null);
  // P42b: the update state machine (check/notify/download/restart) lives here so
  // App only wires the notification, dialog, and Settings section to it.
  const update = useUpdateController();
  // P70: the git preflight behind the "Git is not available" notice bar. Probes
  // once from an effect (after first paint) — nothing renders is gated on it.
  const git = useGitAvailability();
  // Destructured so the palette memo depends on the STABLE callback, not on the
  // hook's per-render state object (which would rebuild every palette row on
  // every App render).
  const gitRecheck = git.recheck;

  // P3e §5.5 / P70: the one global toast stack; P113 §13.1's DEV toast guard and
  // the settings-open signal it publishes (see hooks/useToastQueue.ts).
  const { toasts, pushToast, dismissToast, settingsOpen } = useToastQueue(settings.open);
  // P11c §3.2: every persisted setting that rides the debounced `setUiSettings`
  // patch path, plus that path itself (see src/hooks/useUiSettings.ts). Declared
  // after `pushToast` because the debounced write reports failures through it;
  // `handleSettingsChange` is as stable as `pushToast` is, so children that take
  // it as a prop do not re-render on its account.
  const {
    panelDensity,
    primaryCommitAction,
    autoFetch,
    healthRefresh,
    graph, graphStyle, graphSeason, graphFirstParent, graphFoldLinear, graphMinimapAlwaysShow, graphColorMode, graphRefFilter,
    metricsVersion,
    aiEnabled,
    aiConflictAutonomy,
    aiConsented,
    mcpConsented,
    mcpWriteConsented,
    autoCheckUpdates,
    profiles,
    terminalTool,
    editorTool,
    dev,
    aiDockHeight,
    aiDockCollapsed,
    aiStreamLog,
    aiRun,
    handleSettingsChange,
    queueSettingsWrite,
    hydrateUiSettings,
    adoptToolSelection,
    settingsSaveFailed,
    retrySettingsSave,
  } = useUiSettings(pushToast, settingsOpen);

  // spec-002 §4.2: reflect the active graph style onto <html> (alongside
  // data-theme) so the --graph-canvas-bg token switches the DOM surface behind
  // the canvas. Covers both launch hydration and live toggles via the one
  // graphStyle value the settings hook owns.
  useEffect(() => {
    applyGraphStyle(graphStyle);
  }, [graphStyle]);

  // §5.2 / §6: open-repo tabs, recents, the empty-state error/loading pair and
  // the debounced session persist (see hooks/useRepoTabs.ts).
  const {
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
  } = useRepoTabs(pushToast);

  // P43a: close onboarding (Skip/Finish/Esc/✕) and persist `onboardingSeen` so
  // it does not reappear on the next launch.
  const closeOnboarding = useCallback(() => {
    setOnboardingOpen(false);
    queueSettingsWrite({ onboardingSeen: true });
  }, [queueSettingsWrite]);

  // §5.1 / P69b: the 3-pane split widths (see hooks/usePaneWidthState.ts).
  const {
    paneWidths,
    applyPaneWidths,
    handleSidebarResize,
    handleRightPanelResize,
    handlePaneResizeEnd,
  } = usePaneWidthState(queueSettingsWrite);

  // P69b: these two (plus `commitPaneWidths` and `closeOnboarding`) each used to
  // fire their own `ipc.setUiSettings`, racing the hook's debounced merge —
  // disjoint key sets today, silent field loss the day they overlap. They now
  // update App's state for the live preview and hand the persist to the ONE
  // coalescing window.
  const toggleTheme = useCallback(() => {
    const next: Theme = theme === 'dark' ? 'light' : 'dark';
    setTheme(next);
    applyTheme(next);
    setThemeVersion((v) => v + 1);
    queueSettingsWrite({ theme: next });
  }, [theme, queueSettingsWrite]);

  const toggleListView = useCallback(() => {
    const next: ListView = listView === 'tree' ? 'flat' : 'tree';
    setListView(next);
    queueSettingsWrite({ listView: next });
  }, [listView, queueSettingsWrite]);

  // P49b: external-tool launchers for the per-tab context menu (the strip is
  // App-owned, spanning all tabs). Shared with RepoWorkspace via the hook —
  // never gated by any repo op; failures surface via the AppError→toast path.
  const {
    handleOpenInTerminal: openInTerminal,
    handleRevealInFileManager: revealInFileManager,
    handleOpenInEditor: openInEditor,
  } = useExternalTools(pushToast);

  // §8.3 / §8.4: the Claude Code CLI probe + the one-time AI consent dialog
  // (see hooks/useAiAvailability.ts).
  const { aiAvailability, consentOpen, setConsentOpen, handleConfirmConsent } = useAiAvailability(
    settings.open,
    activeRepo,
    handleSettingsChange,
  );

  // P16 / P16c: embedded MCP server runtime state + controls (useMcpControls.ts).
  const {
    mcpStatus,
    mcpConsentOpen,
    setMcpConsentOpen,
    mcpWriteConsentOpen,
    setMcpWriteConsentOpen,
    handleSetMcpEnabled,
    handleRegisterMcp,
    handleConfirmMcpConsent,
    handleSetMcpAllowWrite,
    handleConfirmMcpWriteConsent,
    mcpOutcomes,
    mcpAnnounce,
    resetMcpOutcomes,
  } = useMcpControls(activeRepo, handleSettingsChange);

  // P21: the clone dialog's lifecycle (see hooks/useCloneFlow.ts).
  const {
    cloneOpen,
    cloneDest,
    cloneProgress,
    cloneBusy,
    cloneError,
    handleCloneOpen,
    handleCloneCancel,
    handleClonePickDest,
    handleCloneSubmit,
  } = useCloneFlow(openTab);

  /** Stable wrapper so `SettingsPanel`'s action bag (and anything else that
   *  memoises over it) does not churn on every App render. */
  const openRepository = useCallback(() => void handleOpenRepository(), [handleOpenRepository]);

  // EVERY value here must be render-stable (a `useCallback`, a `useState`
  // setter, or a plain value). `useAppCommands` memoises over them and its result
  // is `CommandPalette`'s `actions` array — a new array identity re-lands the
  // palette highlight on row 0 mid-typing. Inline arrows are forbidden here; the
  // closures are built inside the memo instead.
  const appCommands = useAppCommands({
    activeRepo,
    openRepository: handleOpenRepository,
    cloneOpen: handleCloneOpen,
    initRepository: handleInitRepository,
    openSettingsAt: settings.openAt,
    setAiAssetsOpen,
    setHealthOpen,
    setOverlayOpen,
    toggleTheme,
    toggleListView,
    gitRecheck,
    pushToast,
  });

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
        applyPaneWidths(s.paneWidths);
        setTheme(s.theme);
        applyTheme(s.theme);
        setThemeVersion((v) => v + 1);
        setListView(s.listView);
        hydrateUiSettings(s);
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

  const globalModalOpen =
    overlayOpen ||
    menuOpen ||
    settings.open ||
    aiAssetsOpen ||
    healthOpen ||
    onboardingOpen ||
    consentOpen ||
    mcpConsentOpen ||
    mcpWriteConsentOpen ||
    update.dialogOpen;

  useAppShortcuts({
    menuOpen,
    overlayOpen,
    settings,
    aiAssetsOpen,
    healthOpen,
    onboardingOpen,
    closeOnboarding,
    setOverlayOpen,
    setAiAssetsOpen,
    setHealthOpen,
    activeRepo,
    handleOpenRepository,
    closeTab,
    globalModalOpen,
    tabsRef,
    setActiveRepo,
  });

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
          <HeaderToolbar
            theme={theme}
            onToggleTheme={toggleTheme}
            listView={listView}
            onToggleListView={toggleListView}
            activeRepo={activeRepo}
            onOpenAiAssets={() => setAiAssetsOpen(true)}
            onOpenHealth={() => setHealthOpen(true)}
            onOpenSettings={() => settings.openAt(null)}
            onOpenSettingsAt={settings.openAt} devEnabled={dev.enabled}
            onMenuOpenChange={setMenuOpen}
            profiles={profiles}
            onProfilesChange={(next) => handleSettingsChange({ profiles: next })}
          />
        </header>

        {/* P70: app-level, in-flow, directly below the header — git availability
            is a process-global fact, and a per-tab banner would both duplicate
            and disappear on the no-repo empty state. */}
        <GitMissingBanner git={git} onGitAvailable={(text) => pushToast('success', text)} />

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
                primaryCommitAction={primaryCommitAction}
                themeVersion={themeVersion}
                paneWidths={paneWidths}
                globalModalOpen={globalModalOpen}
                graph={graph}
                metricsVersion={metricsVersion}
                graphStyle={graphStyle} graphSeason={graphSeason}
                graphFirstParent={graphFirstParent} graphFoldLinear={graphFoldLinear} graphMinimapAlwaysShow={graphMinimapAlwaysShow} graphColorMode={graphColorMode} graphRefFilter={graphRefFilter} onGraphFilterChange={handleSettingsChange}
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
                onOpenIdentitySettings={settings.openIdentity}
                onOpenAccountSettings={() => settings.openAt('accounts')}
                appCommands={appCommands}
              />
            </div>
          ))
        ) : (
          <EmptyState
            loading={loading}
            error={error}
            recents={recents}
            onOpenRepository={openRepository}
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
          onOpenRepository={openRepository}
          onCloneOpen={handleCloneOpen}
          onInitRepository={() => void handleInitRepository()}
          onOpenRecent={(path) => void openTab(path)}
        />
        <SettingsPanel
          open={settings.open}
          initialCategory={settings.request.category ?? undefined}
          requestSeq={settings.request.seq}
          onClose={settings.close}
          theme={theme}
          listView={listView}
          panelDensity={panelDensity}
          primaryCommitAction={primaryCommitAction}
          autoFetch={autoFetch}
          healthRefresh={healthRefresh}
          graph={graph} graphStyle={graphStyle} graphSeason={graphSeason}
          graphFirstParent={graphFirstParent} graphFoldLinear={graphFoldLinear} graphMinimapAlwaysShow={graphMinimapAlwaysShow} graphColorMode={graphColorMode} graphRefFilter={graphRefFilter} onChange={handleSettingsChange}
          onToggleTheme={toggleTheme}
          onToggleListView={toggleListView}
          aiEnabled={aiEnabled}
          aiConflictAutonomy={aiConflictAutonomy}
          aiConsented={aiConsented}
          aiAvailability={aiAvailability}
          onRequestEnableAi={() => setConsentOpen(true)}
          aiRun={aiRun}
          mcpStatus={mcpStatus}
          mcpOutcomes={mcpOutcomes}
          mcpAnnounce={mcpAnnounce} onResetMcpOutcomes={resetMcpOutcomes}
          settingsSaveFailed={settingsSaveFailed}
          onRetrySettingsSave={retrySettingsSave}
          mcpConsented={mcpConsented}
          onSetMcpEnabled={handleSetMcpEnabled}
          onRequestEnableMcp={() => setMcpConsentOpen(true)}
          mcpWriteConsented={mcpWriteConsented}
          onSetMcpAllowWrite={handleSetMcpAllowWrite}
          onRequestEnableMcpWrite={() => setMcpWriteConsentOpen(true)}
          repoPath={activeRepo}
          configInitialFocus={settings.request.focus}
          focusProfileId={settings.request.focusProfileId}
          profiles={profiles}
          terminalTool={terminalTool}
          editorTool={editorTool}
          onAdoptToolSelection={adoptToolSelection}
          dev={dev}
          onRegisterMcp={handleRegisterMcp}
          onShowOnboarding={showOnboarding}
          onOpenRepository={openRepository}
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
        <AiConsentDialog
          open={consentOpen}
          onConfirm={handleConfirmConsent}
          onCancel={() => setConsentOpen(false)}
        />
        <McpConsentDialog
          open={mcpConsentOpen}
          onConfirm={handleConfirmMcpConsent}
          onCancel={() => setMcpConsentOpen(false)}
        />
        <McpWriteConsentDialog
          open={mcpWriteConsentOpen}
          onConfirm={handleConfirmMcpWriteConsent}
          onCancel={() => setMcpWriteConsentOpen(false)}
        />
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
        {/* The updater's two surfaces, from one controller (own file). */}
        <AppUpdateSurfaces update={update} />
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
