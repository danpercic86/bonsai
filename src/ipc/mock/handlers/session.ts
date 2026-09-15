// Split out of the former monolithic mock.ts (pure refactor; no behavior change).
import type { IpcApi } from '../../types';
import { clampAiRunSettings } from '../aiRunSettings';
import { jobStatusListeners, mockMcp, repoChangedListeners, tagAutoSyncListeners } from '../events';
import { clampAutoFetch, clampGraphPrefs, clampHealthRefresh, clampPaneWidths, readRecents, readSession, readUiSettings, writeRecents, writeSession, writeUiSettings } from '../persistence';
import { delay, query as urlParam, requireRepo } from '../repoState';
import { applyToolSeam } from './tools';
import { applyMockJobTimers, completeMockJobRun, seedJobStatuses } from '../scheduler';
import type { AppError, JobKind, JobStatus, JobStatusChangedPayload, RecentRepo, RepoChangedPayload, SessionState, TagAutoSyncEvent, UiSettings, UiSettingsPatch, Unsubscribe } from '../../types';

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

  // P85 A3: the fire-and-forget fetch tag auto-sync completion event. The mock's
  // `fetch` handler dispatches through this registry (see remotesSync.ts).
  async onTagAutoSync(cb: (e: TagAutoSyncEvent) => void): Promise<Unsubscribe> {
    tagAutoSyncListeners.add(cb);
    return () => {
      tagAutoSyncListeners.delete(cb);
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
    // P112 §12: the `?tools=stale` / `?tools=custom` seams need a PERSISTED
    // selection. `applyToolSeam` layers it under the stored blob and yields to
    // any value the harness has since written, so a pick sticks.
    return applyToolSeam(readUiSettings());
  },

  async setUiSettings(patch: UiSettingsPatch): Promise<UiSettings> {
    await delay(150);
    // P113 §14/§17.3: `?settingsSaveFail=1` — nothing in this mock rejected a
    // settings write before, which is why the app's highest-traffic Settings
    // failure (`useUiSettings`'s save-failure channel: EVERY toggle, radio and
    // field goes through this path) had never been seen rendered by anyone.
    //
    // FIDELITY NOTE, verified against `src-tauri/src/settings.rs` `save_to()`:
    // the write is a temp-file write + atomic rename, and its permission-denied
    // branch is `AppError::Io(format!("write {}: {e}", tmp.display()))`. That is
    // mirrored verbatim in shape here — a `.<pid>.<n>.tmp` sibling of
    // `settings.json` under the app config dir, and a real Windows os error 5.
    // The other reachable rejections of `set_ui_settings` are
    // `cannot resolve app config dir: {e}` (settings_file), `create settings dir
    // {parent}: {e}`, `rename {tmp} -> {file}: {e}` and `task join error: {e}`;
    // the write failure is the one the banner's "check that Bonsai can write to
    // its config folder" names, so it is the honest fixture for this knob.
    //
    // The banner does NOT render this text (amendment A4 dropped the raw tail),
    // so the value here is what a developer sees in the console/network view —
    // which is exactly why it must not be invented.
    if (urlParam('settingsSaveFail') === '1') {
      const err: AppError = {
        kind: 'io',
        message:
          'write C:\\Users\\dev\\AppData\\Roaming\\com.bonsai.app\\settings.json.4812.0.tmp: Access is denied. (os error 5)',
      };
      throw err;
    }
    const current = applyToolSeam(readUiSettings());
    // Spec-002 (additive/optional): merge only when a value exists (patch or
    // stored blob), so a write never mints keys the Rust-pinned oracle lacks.
    const graphStyle = patch.graphStyle ?? current.graphStyle;
    const graphSeason = patch.graphSeason ?? current.graphSeason;
    const next: UiSettings = {
      theme: patch.theme ?? current.theme,
      paneWidths:
        patch.paneWidths !== undefined ? clampPaneWidths(patch.paneWidths) : current.paneWidths,
      listView: patch.listView ?? current.listView,
      // P67 §4: patches independently of listView/graph.
      panelDensity: patch.panelDensity ?? current.panelDensity,
      // P80 D1: patches independently of listView/graph/panelDensity.
      primaryCommitAction: patch.primaryCommitAction ?? current.primaryCommitAction,
      autoFetch:
        patch.autoFetch !== undefined ? clampAutoFetch(patch.autoFetch) : current.autoFetch,
      healthRefresh:
        patch.healthRefresh !== undefined
          ? clampHealthRefresh(patch.healthRefresh)
          : current.healthRefresh,
      graph: patch.graph !== undefined ? clampGraphPrefs(patch.graph) : current.graph,
      ...(graphStyle !== undefined ? { graphStyle } : {}),
      ...(graphSeason !== undefined ? { graphSeason } : {}),
      // Spec-003: first-parent + ref-filter intent. `graphRefFilter: null` in a
      // patch is a real value (clear the filter), so `!== undefined` gates it.
      graphFirstParent: patch.graphFirstParent ?? current.graphFirstParent ?? false,
      // Spec-004: fold-linear toggle patches independently (first-parent precedent).
      graphFoldLinear: patch.graphFoldLinear ?? current.graphFoldLinear ?? false,
      // Spec-005: always-show overview rail (same plain-bool precedent).
      graphMinimapAlwaysShow: patch.graphMinimapAlwaysShow ?? current.graphMinimapAlwaysShow ?? false,
      // Spec-006: edge/ring coloring (same plain-enum precedent; default 'lane').
      graphColorMode: patch.graphColorMode ?? current.graphColorMode ?? 'lane',
      graphRefFilter:
        patch.graphRefFilter !== undefined ? patch.graphRefFilter : (current.graphRefFilter ?? null),
      aiEnabled: patch.aiEnabled ?? current.aiEnabled,
      aiConflictAutonomy: patch.aiConflictAutonomy ?? current.aiConflictAutonomy,
      aiConsented: patch.aiConsented ?? current.aiConsented,
      mcpConsented: patch.mcpConsented ?? current.mcpConsented,
      mcpWriteConsented: patch.mcpWriteConsented ?? current.mcpWriteConsented,
      onboardingSeen: patch.onboardingSeen ?? current.onboardingSeen,
      autoCheckUpdates: patch.autoCheckUpdates ?? current.autoCheckUpdates,
      profiles: patch.profiles ?? current.profiles,
      // P112 §5.1: catalog ids, merged like any other scalar. NOT coerced here —
      // Rust's `coerce_tool_id` maps an unknown id to `''` against the
      // compile-time catalog, and that catalog has no TypeScript mirror until
      // P112 sub-inc 4 brings the detected-tool picker (and its `DetectedTool`
      // IPC) over. Inventing a partial allowlist here would be a second,
      // divergent source of truth; until then no UI can write these keys at all.
      terminalTool: patch.terminalTool ?? current.terminalTool,
      editorTool: patch.editorTool ?? current.editorTool,
      // P91 §10: whole-struct patch (mirrors Rust `apply_patch`); the harness
      // sink is (re)started from the merged value, exactly as Rust restarts the
      // real sink after the save.
      dev: patch.dev ?? current.dev,
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
