// P11c §3.2: the persisted-UI-settings state machine, extracted from App so the
// container stays readable. Every setting in here rides ONE shared path: a
// partial `UiSettingsPatch` is applied to local state immediately (live
// preview), then a single merged `ipc.setUiSettings` write is debounced (~300 ms)
// so a burst of knob changes reaches disk once.
//
// Owned here: the per-field state, the patch merge (`handleSettingsChange`), the
// debounced coalescing write (`pendingSettingsPatchRef` + `settingsSaveTimerRef`),
// and launch-time hydration (`hydrateUiSettings`).
//
// NOT owned here: `theme`, `listView` and `paneWidths`. Those have their own
// bespoke persist paths in App (toolbar toggles that write immediately, and the
// drag-commit debounce) and are hydrated alongside this hook, not by it.
//
// Adding a setting is therefore two edits — a `useState` and a patch arm — plus
// whatever prop threads it to a child.

import { useCallback, useRef, useState } from 'react';

import type { ToastTone } from '../components/Toasts';
import { ipc } from '../ipc';
import type {
  AiAutonomy,
  AutoFetchSettings,
  GraphPrefs,
  HealthRefreshSettings,
  IdentityProfile,
  PanelDensity,
  UiSettings,
  UiSettingsPatch,
} from '../ipc';
import { errorMessage } from '../utils/errors';

/** App's toast pusher. Passed in (rather than re-derived from context here) so
 *  the save-failure copy and this hook's callback stability both stay App's. */
type PushToast = (tone: ToastTone, text: string) => void;

export interface UiSettingsController {
  panelDensity: PanelDensity;
  autoFetch: AutoFetchSettings;
  healthRefresh: HealthRefreshSettings;
  graph: GraphPrefs;
  /** P11d §4.3: bumped on every graph-knob change → GraphCanvas full re-measure. */
  metricsVersion: number;
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  mcpConsented: boolean;
  mcpWriteConsented: boolean;
  autoCheckUpdates: boolean;
  profiles: IdentityProfile[];
  terminalCommand: string;
  editorCommand: string;
  aiDockHeight: number;
  aiDockCollapsed: boolean;
  aiStreamLog: boolean;
  /** Apply a Settings patch: live preview now, one debounced merged persist.
   *  Referentially stable for as long as `pushToast` is — it is handed to
   *  children as a prop, so it must not churn every render. */
  handleSettingsChange(patch: UiSettingsPatch): void;
  /** Seed every field from the launch-time `getUiSettings()` read (§6.2). */
  hydrateUiSettings(settings: UiSettings): void;
}

export function useUiSettings(pushToast: PushToast): UiSettingsController {
  // P67 §4: right-panel density. No toolbar button (unlike theme/listView), so
  // it rides the debounced `handleSettingsChange` patch path only.
  const [panelDensity, setPanelDensity] = useState<PanelDensity>('cozy');
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
  // P16: `mcpConsented` is the one-time consent gate for the embedded-MCP enable
  // toggle; the dialog (App-owned) defers enabling until it is recorded.
  const [mcpConsented, setMcpConsented] = useState(false);
  // P16c: the write-gate has its own one-time consent (a stronger grant than
  // read) and its own defer-to-dialog flow.
  const [mcpWriteConsented, setMcpWriteConsented] = useState(false);
  // P42b: auto-check-for-updates-on-launch preference (persisted; default OFF).
  const [autoCheckUpdates, setAutoCheckUpdates] = useState(false);
  // P44: named identity profiles (global). Source of truth for the Settings
  // section; persisted via handleSettingsChange like every other setting.
  const [profiles, setProfiles] = useState<IdentityProfile[]>([]);
  // P49b: external-tool command templates ('' ⇒ backend auto-detects per-OS).
  // Threaded into the Settings section; persisted via handleSettingsChange.
  const [terminalCommand, setTerminalCommand] = useState('');
  const [editorCommand, setEditorCommand] = useState('');
  // P11c §3.2: debounced settings persist — accumulates partial patches so a
  // burst of knob changes within the window all reach disk in one write.
  const settingsSaveTimerRef = useRef<number | null>(null);
  const pendingSettingsPatchRef = useRef<UiSettingsPatch>({});

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

  // Launch-time hydration (§6.2). Same setter order as the single read it
  // replaces, including the metricsVersion bump that follows setGraph.
  const hydrateUiSettings = useCallback((s: UiSettings) => {
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
  }, []);

  return {
    panelDensity,
    autoFetch,
    healthRefresh,
    graph,
    metricsVersion,
    aiEnabled,
    aiConflictAutonomy,
    aiConsented,
    mcpConsented,
    mcpWriteConsented,
    autoCheckUpdates,
    profiles,
    terminalCommand,
    editorCommand,
    aiDockHeight,
    aiDockCollapsed,
    aiStreamLog,
    handleSettingsChange,
    hydrateUiSettings,
  };
}
