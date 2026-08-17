// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { clampAiRunSettings } from '../aiRunSettings';
import { jobStatusListeners, mockMcp, repoChangedListeners } from '../events';
import { clampAutoFetch, clampGraphPrefs, clampHealthRefresh, clampPaneWidths, readRecents, readSession, readUiSettings, writeRecents, writeSession, writeUiSettings } from '../persistence';
import { delay, requireRepo } from '../repoState';
import { applyMockJobTimers, completeMockJobRun, seedJobStatuses } from '../scheduler';
import type { JobKind, JobStatus, JobStatusChangedPayload, RecentRepo, RepoChangedPayload, SessionState, UiSettings, UiSettingsPatch, Unsubscribe } from '../../types';

export const sessionHandlers = {
  async getRecentRepos(): Promise<RecentRepo[]> {
    await delay(150);
    return readRecents();
  },

  async removeRecentRepo(path: string): Promise<RecentRepo[]> {
    await delay(150);
    const list = readRecents().filter((r) => r.path.toLowerCase() !== path.toLowerCase());
    writeRecents(list);
    return list;
  },

  // No backend watcher in the browser harness, but the P30 mock job ticks
  // dispatch repo-changed through this registry (contract §7).
  async onRepoChanged(cb: (p: RepoChangedPayload) => void): Promise<Unsubscribe> {
    repoChangedListeners.add(cb);
    return () => {
      repoChangedListeners.delete(cb);
    };
  },

  // P30: background-job status surface (mock harness §7).
  async getJobStatus(repoId: string): Promise<JobStatus[]> {
    await delay(80);
    requireRepo(repoId);
    const settings = readUiSettings();
    // Reflect the CURRENT config's enabled flags, like the Rust command.
    return seedJobStatuses(repoId).map((s) => ({
      ...s,
      enabled: (s.job === 'autoFetch' ? settings.autoFetch : settings.healthRefresh).enabled,
    }));
  },

  async runJobNow(repoId: string, job: JobKind): Promise<void> {
    await delay(80);
    requireRepo(repoId);
    // Mock runs are instant, so the D10 overlap rejection never triggers here;
    // fire the same synthetic completion the timers use.
    completeMockJobRun(repoId, job);
  },

  async onJobStatusChanged(cb: (p: JobStatusChangedPayload) => void): Promise<Unsubscribe> {
    jobStatusListeners.add(cb);
    return () => {
      jobStatusListeners.delete(cb);
    };
  },

  // Real browser focus event so the harness exercises the refocus-refetch path.
  async onWindowFocus(cb: () => void): Promise<Unsubscribe> {
    window.addEventListener('focus', cb);
    return () => window.removeEventListener('focus', cb);
  },

  async getUiSettings(): Promise<UiSettings> {
    await delay(150);
    return readUiSettings();
  },

  async setUiSettings(patch: UiSettingsPatch): Promise<UiSettings> {
    await delay(150);
    const current = readUiSettings();
    const next: UiSettings = {
      theme: patch.theme ?? current.theme,
      paneWidths:
        patch.paneWidths !== undefined ? clampPaneWidths(patch.paneWidths) : current.paneWidths,
      listView: patch.listView ?? current.listView,
      // P67 §4: patches independently of listView/graph.
      panelDensity: patch.panelDensity ?? current.panelDensity,
      autoFetch:
        patch.autoFetch !== undefined ? clampAutoFetch(patch.autoFetch) : current.autoFetch,
      healthRefresh:
        patch.healthRefresh !== undefined
          ? clampHealthRefresh(patch.healthRefresh)
          : current.healthRefresh,
      graph: patch.graph !== undefined ? clampGraphPrefs(patch.graph) : current.graph,
      aiEnabled: patch.aiEnabled ?? current.aiEnabled,
      aiConflictAutonomy: patch.aiConflictAutonomy ?? current.aiConflictAutonomy,
      aiConsented: patch.aiConsented ?? current.aiConsented,
      mcpConsented: patch.mcpConsented ?? current.mcpConsented,
      mcpWriteConsented: patch.mcpWriteConsented ?? current.mcpWriteConsented,
      onboardingSeen: patch.onboardingSeen ?? current.onboardingSeen,
      autoCheckUpdates: patch.autoCheckUpdates ?? current.autoCheckUpdates,
      profiles: patch.profiles ?? current.profiles,
      terminalCommand: patch.terminalCommand ?? current.terminalCommand,
      editorCommand: patch.editorCommand ?? current.editorCommand,
      // P68 §8.3: each of the ten AI-run knobs patches independently of
      // graph/listView/panelDensity, then the whole slice is clamped on write
      // (mirrors apply_patch → clamp_ai_settings).
      ...clampAiRunSettings({
        aiIdleTimeoutSecs: patch.aiIdleTimeoutSecs ?? current.aiIdleTimeoutSecs,
        aiHardCapSecs: patch.aiHardCapSecs ?? current.aiHardCapSecs,
        aiMaxTurns: patch.aiMaxTurns ?? current.aiMaxTurns,
        aiStreamLog: patch.aiStreamLog ?? current.aiStreamLog,
        aiIncludePartialMessages:
          patch.aiIncludePartialMessages ?? current.aiIncludePartialMessages,
        aiConflictTools: patch.aiConflictTools ?? current.aiConflictTools,
        aiBulkMaxBytes: patch.aiBulkMaxBytes ?? current.aiBulkMaxBytes,
        aiMaxBudgetUsd: patch.aiMaxBudgetUsd ?? current.aiMaxBudgetUsd,
        aiDockHeight: patch.aiDockHeight ?? current.aiDockHeight,
        aiDockCollapsed: patch.aiDockCollapsed ?? current.aiDockCollapsed,
      }),
    };
    writeUiSettings(next);
    // P30 §7: config round-trip re-arms the synthetic job tick timers.
    applyMockJobTimers(next);
    return next;
  },

  async getSession(): Promise<SessionState> {
    await delay(150);
    return readSession();
  },

  async setSession(session: SessionState): Promise<void> {
    await delay(150);
    writeSession(session);
  },

  // P16: embedded MCP server. No real socket — the harness only proves the
  // Settings UI wiring; canned status mirrors the Rust `McpStatus` shape.
  async setActiveRepo(repoId: string | null): Promise<void> {
    await delay(50);
    mockMcp.activeRepo = repoId;
  },

} satisfies Partial<IpcApi>;
