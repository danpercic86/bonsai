// P11c §3.2: the persisted-UI-settings state machine, extracted from App so the
// container stays readable. Every setting in here rides ONE shared path: a
// partial `UiSettingsPatch` is applied to local state immediately (live
// preview), then a single merged `ipc.setUiSettings` write is debounced (~300 ms)
// so a burst of knob changes reaches disk once.
//
// Owned here: the per-field state, the patch merge (`handleSettingsChange`),
// launch-time hydration (`hydrateUiSettings`) and P112's narrow non-writing
// adopt for the two external-tool fields. The debounced coalescing WRITE —
// window, single-writer invariant, retry budget, teardown flush and the P113
// failure surface — lives in `useSettingsWriteQueue`; this hook only feeds it.
//
// NOT owned here: the STATE for `theme`, `listView`, `paneWidths` and
// `onboardingSeen`. Those live in App (toolbar toggles, the resize drag, the
// first-run overlay) and are hydrated alongside this hook, not by it — but
// P69b routed their PERSIST calls through `queueSettingsWrite` so every field
// on disk goes out through this one coalescing window. Nothing else may call
// `ipc.setUiSettings` directly.
//
// Adding a setting is therefore two edits — a `useState` and a patch arm — plus
// whatever prop threads it to a child.

import { useCallback, useEffect, useState } from 'react';

import { configureObs } from '../obs/enabled';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import type {
  AiAutonomy,
  AiConflictTools,
  AutoFetchSettings,
  DevSettings,
  GraphPrefs,
  GraphRefFilter,
  GraphColorMode,
  GraphSeason,
  GraphStyle,
  HealthRefreshSettings,
  IdentityProfile,
  PanelDensity,
  PrimaryCommitAction,
  UiSettings,
  UiSettingsPatch,
} from '../ipc';
import { DEFAULT_SEASON } from '../graph/palettes';
import type { PushToast } from '../ToastContext';
import { useSettingsWriteQueue } from './useSettingsWriteQueue';
import type { SettingsOpenSignal } from './useSettingsOpenSignal';

/**
 * P112 §16.16-5 — the two fields the external-tool picker may adopt from disk
 * without writing, each optional so a caller adopts only the kind it owns.
 *
 * Derived from `UiSettingsPatch` on purpose: these are the same two keys the
 * picker PATCHES through `handleSettingsChange`, and a hand-written pair could
 * drift from them. It is not a patch, though — `adoptToolSelection` never
 * reaches the write queue.
 */
export type ToolSelection = Pick<UiSettingsPatch, 'terminalTool' | 'editorTool'>;

export interface UiSettingsController {
  panelDensity: PanelDensity;
  /** P80 D1: which commit button is emphasized in the Working tab footer. */
  primaryCommitAction: PrimaryCommitAction;
  autoFetch: AutoFetchSettings;
  healthRefresh: HealthRefreshSettings;
  graph: GraphPrefs;
  /** Spec-002: commit-graph visual style (standard vs Bonsai). Ride the debounced
   *  patch path like panelDensity; NO metricsVersion bump (geometry is unchanged —
   *  GraphCanvas re-resolves the palette on its own, task 3). */
  graphStyle: GraphStyle;
  /** Spec-002: seasonal accent for the Bonsai style. */
  graphSeason: GraphSeason;
  /** Spec-003: first-parent graph walk (default false). Additive/optional like
   *  graphStyle; NO metricsVersion bump (geometry is unchanged). */
  graphFirstParent: boolean;
  /** Spec-004: fold linear runs (default false). No metricsVersion bump. */
  graphFoldLinear: boolean;
  /** Spec-005: always-show overview rail (default false). No metricsVersion bump. */
  graphMinimapAlwaysShow: boolean;
  /** Spec-006: graph edge/ring coloring (default 'lane'). Paint-only — no
   *  metricsVersion bump (geometry is unchanged). */
  graphColorMode: GraphColorMode;
  /** Spec-003: persisted solo/hide ref-filter intent (null = none). */
  graphRefFilter: GraphRefFilter | null;
  /** P11d §4.3: bumped on every graph-knob change → GraphCanvas full re-measure. */
  metricsVersion: number;
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  mcpConsented: boolean;
  mcpWriteConsented: boolean;
  autoCheckUpdates: boolean;
  profiles: IdentityProfile[];
  /** P112 §16.4a: the selected terminal / editor — `''` (auto-detect), a catalog
   *  id, or `'custom'`. Read by Settings' detected-tool picker; the launch sites
   *  resolve the id in Rust, so nothing else in the renderer reads them. */
  terminalTool: string;
  editorTool: string;
  /** P91 §10: Dev-mode / observability settings (whole-struct, the autoFetch
   *  idiom). Threaded to the Settings Developer page and the header pill. */
  dev: DevSettings;
  aiDockHeight: number;
  aiDockCollapsed: boolean;
  aiStreamLog: boolean;
  /** P68g §1: the eight AI-run knobs as one read-only struct for the Settings
   *  section (`AiRunPrefs`; the `graph`/`autoFetch` prop idiom). Each field is
   *  still stored and PATCHED independently — this is only the read view. */
  aiRun: AiRunPrefs;
  /** Apply a Settings patch: live preview now, one debounced merged persist.
   *  Referentially stable for as long as `pushToast` is — it is handed to
   *  children as a prop, so it must not churn every render. */
  handleSettingsChange(patch: UiSettingsPatch): void;
  /** P69b: persist-only half of `handleSettingsChange` — merge `patch` into the
   *  same pending write and re-arm the same 300 ms window, touching no state
   *  here. For the settings App owns the state of (`theme`, `listView`,
   *  `paneWidths`, `onboardingSeen`): App updates its own state for the live
   *  preview and calls this so the write still coalesces with everything else.
   *  Stable for as long as `pushToast` is. */
  queueSettingsWrite(patch: UiSettingsPatch): void;
  /** Seed every field from the launch-time `getUiSettings()` read (§6.2). It has
   *  exactly one caller, App's launch effect: a MID-SESSION whole-struct hydrate
   *  reverts every unflushed patch on screen until the next launch (§17.3), so
   *  the external-tool picker takes `adoptToolSelection` instead. */
  hydrateUiSettings(settings: UiSettings): void;
  /** P112 §16.16-5: adopt one or both tool selections read from disk, touching
   *  NO other field, queueing NO write and bumping NO `metricsVersion` — the
   *  Browse flow needs it because `pick_external_tool` persists the selection
   *  itself, so the renderer re-reads rather than patching, or it races that
   *  write (`ipc-api-tools.ts:24-25`). */
  adoptToolSelection(selection: ToolSelection): void;
  /** P113 §17.3: the debounced write is failing. Renders as the Settings card's
   *  save banner; the toast covers the Settings-closed case. */
  settingsSaveFailed: boolean;
  /** Send the pending patch NOW. The backoff stops after three attempts, so
   *  without this the patch sits unsent until the user changes something else. */
  retrySettingsSave(): void;
}

export function useUiSettings(
  pushToast: PushToast,
  settingsOpen: SettingsOpenSignal,
): UiSettingsController {
  // P67 §4: right-panel density. No toolbar button (unlike theme/listView), so
  // it rides the debounced `handleSettingsChange` patch path only.
  const [panelDensity, setPanelDensity] = useState<PanelDensity>('cozy');
  // P80 D1: primary commit action. Default 'commit' (always-safe, non-network).
  const [primaryCommitAction, setPrimaryCommitAction] =
    useState<PrimaryCommitAction>('commit');
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
  // Spec-002: commit-graph visual style + season. Additive/optional settings
  // (absent from the Rust-pinned defaults oracle), so the initial state IS the
  // default; hydration coalesces a missing persisted value to the same default.
  const [graphStyle, setGraphStyle] = useState<GraphStyle>('standard');
  const [graphSeason, setGraphSeason] = useState<GraphSeason>(DEFAULT_SEASON);
  // Spec-003: graph declutter prefs — additive/optional like graphStyle above.
  const [graphFirstParent, setGraphFirstParent] = useState(false);
  const [graphFoldLinear, setGraphFoldLinear] = useState(false);
  // Spec-005: overview-rail always-show — same additive-bool pattern.
  const [graphMinimapAlwaysShow, setGraphMinimapAlwaysShow] = useState(false);
  // Spec-006: author-coloring mode — paint-only pref, same additive pattern.
  const [graphColorMode, setGraphColorMode] = useState<GraphColorMode>('lane');
  const [graphRefFilter, setGraphRefFilter] = useState<GraphRefFilter | null>(null);
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
  // P68 §8.3: the rest of the AI-run knobs, mirroring the Rust field defaults.
  // `aiHardCapSecs: 0` (no deadline — Cancel is the stop) and `aiMaxBudgetUsd: 0`
  // (no spend cap) are LOCKED user decisions, not missing values.
  const [aiConflictTools, setAiConflictTools] = useState<AiConflictTools>('readOnly');
  const [aiIncludePartialMessages, setAiIncludePartialMessages] = useState(false);
  const [aiIdleTimeoutSecs, setAiIdleTimeoutSecs] = useState(300);
  const [aiHardCapSecs, setAiHardCapSecs] = useState(0);
  const [aiMaxTurns, setAiMaxTurns] = useState(6);
  const [aiMaxBudgetUsd, setAiMaxBudgetUsd] = useState(0);
  const [aiBulkMaxBytes, setAiBulkMaxBytes] = useState(400_000);
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
  // P112 §16.4a: sub-increment 4's detected-tool picker is the UI that reads
  // them, so they are held here now — one hook, one context, like every other
  // settings value. Holding them inside `useExternalToolScan` instead was
  // REJECTED: a second source of truth for two settings is how a stale row
  // survives a reset-to-defaults. Both are lookup keys, never program strings
  // (`ipc/types/settings.ts:105-113`), and Rust's `coerce_tool_id` is the gate.
  const [terminalTool, setTerminalTool] = useState('');
  const [editorTool, setEditorTool] = useState('');
  // P91 §10: Dev-mode / observability settings (whole-struct, like autoFetch).
  // Privacy defaults out of the box: OFF and strict redaction (mirrors DEFAULTS.dev).
  const [dev, setDev] = useState<DevSettings>({
    enabled: false,
    level: 'debug',
    captureIpc: true,
    captureReact: true,
    captureFrames: false,
    includeRawNames: false,
  });
  // P11c §3.2 / P69b / P113 §17.3 — the debounced, coalescing, single-writer
  // settings persist, with its retry budget and its failure surface, now in
  // `useSettingsWriteQueue`. It moved out VERBATIM: this file was at 494 of the
  // 500-line ratchet and the write machine is its own concern (the house
  // one-concern-per-file rule), so it is split in the same increment that grew it.
  const { queueSettingsWrite, retrySettingsSave, settingsSaveFailed } = useSettingsWriteQueue(
    pushToast,
    settingsOpen,
  );

  // P91 §10/§11 increment 7d — the one wire that activates the frontend
  // observability pipeline in production. The sink is already attached at module
  // load (`src/ipc/index.ts` → `instrumentIpc` → `attachSink` with the RAW,
  // un-proxied api), so by the time this effect first runs the sink-before-enable
  // ordering holds and the batcher's defensive re-salt path is not relied on.
  //
  // `dev` is a fresh object on every hydrate (`setDev(s.dev)`) and every patch
  // (`setDev(patch.dev)`), so this refires on every change: enabling Dev mode
  // activates capture live with no reload (§10), and disabling it — or any patch
  // with `enabled:false` — deactivates fully (`configureObs` sets `config` → null,
  // so `obsEnabled()` is a single false boolean read again, §11). The initial
  // `dev.enabled:false` state makes the first run a no-op.
  useEffect(() => {
    configureObs(dev);
  }, [dev]);

  // P11c §3.2: apply a Settings patch — update local state immediately (live
  // preview; graph changes bump metricsVersion so the canvas re-measures), then
  // debounce a single merged persist (~300 ms, shared with App's four writers).
  const handleSettingsChange = useCallback(
    (patch: UiSettingsPatch) => {
      if (patch.panelDensity !== undefined) setPanelDensity(patch.panelDensity);
      if (patch.primaryCommitAction !== undefined) {
        setPrimaryCommitAction(patch.primaryCommitAction);
      }
      if (patch.autoFetch !== undefined) setAutoFetch(patch.autoFetch);
      if (patch.healthRefresh !== undefined) setHealthRefresh(patch.healthRefresh);
      if (patch.graph !== undefined) {
        setGraph(patch.graph);
        setMetricsVersion((v) => v + 1);
      }
      // Spec-002: style/season change the palette, not the geometry — no
      // metricsVersion bump (GraphCanvas re-resolves the theme itself, task 3).
      if (patch.graphStyle !== undefined) setGraphStyle(patch.graphStyle);
      if (patch.graphSeason !== undefined) setGraphSeason(patch.graphSeason);
      // Spec-003: no metricsVersion bump — the walk changes, not the geometry.
      // `graphRefFilter: null` is a real value (clear), so `!== undefined` gates.
      if (patch.graphFirstParent !== undefined) setGraphFirstParent(patch.graphFirstParent);
      if (patch.graphFoldLinear !== undefined) setGraphFoldLinear(patch.graphFoldLinear);
      if (patch.graphMinimapAlwaysShow !== undefined)
        setGraphMinimapAlwaysShow(patch.graphMinimapAlwaysShow);
      if (patch.graphColorMode !== undefined) setGraphColorMode(patch.graphColorMode);
      if (patch.graphRefFilter !== undefined) setGraphRefFilter(patch.graphRefFilter);
      if (patch.aiEnabled !== undefined) setAiEnabled(patch.aiEnabled);
      if (patch.aiConflictAutonomy !== undefined) setAiConflictAutonomy(patch.aiConflictAutonomy);
      if (patch.aiConsented !== undefined) setAiConsented(patch.aiConsented);
      if (patch.mcpConsented !== undefined) setMcpConsented(patch.mcpConsented);
      if (patch.mcpWriteConsented !== undefined) setMcpWriteConsented(patch.mcpWriteConsented);
      if (patch.autoCheckUpdates !== undefined) setAutoCheckUpdates(patch.autoCheckUpdates);
      if (patch.profiles !== undefined) setProfiles(patch.profiles);
      if (patch.terminalTool !== undefined) setTerminalTool(patch.terminalTool);
      if (patch.editorTool !== undefined) setEditorTool(patch.editorTool);
      if (patch.dev !== undefined) setDev(patch.dev);
      if (patch.aiDockHeight !== undefined) setAiDockHeight(patch.aiDockHeight);
      if (patch.aiDockCollapsed !== undefined) setAiDockCollapsed(patch.aiDockCollapsed);
      if (patch.aiStreamLog !== undefined) setAiStreamLog(patch.aiStreamLog);
      if (patch.aiConflictTools !== undefined) setAiConflictTools(patch.aiConflictTools);
      if (patch.aiIncludePartialMessages !== undefined) {
        setAiIncludePartialMessages(patch.aiIncludePartialMessages);
      }
      if (patch.aiIdleTimeoutSecs !== undefined) setAiIdleTimeoutSecs(patch.aiIdleTimeoutSecs);
      if (patch.aiHardCapSecs !== undefined) setAiHardCapSecs(patch.aiHardCapSecs);
      if (patch.aiMaxTurns !== undefined) setAiMaxTurns(patch.aiMaxTurns);
      if (patch.aiMaxBudgetUsd !== undefined) setAiMaxBudgetUsd(patch.aiMaxBudgetUsd);
      if (patch.aiBulkMaxBytes !== undefined) setAiBulkMaxBytes(patch.aiBulkMaxBytes);
      queueSettingsWrite(patch);
    },
    [queueSettingsWrite],
  );

  // P112 §16.16-5 — the narrow, non-writing adopt. Two `if`s and nothing else:
  // no `queueSettingsWrite` (the backend already persisted the pick) and no
  // `setMetricsVersion` bump (no geometry changed), which is the pair of costs
  // `hydrateUiSettings` carries and §17.3 priced after the fact.
  const adoptToolSelection = useCallback((selection: ToolSelection) => {
    if (selection.terminalTool !== undefined) setTerminalTool(selection.terminalTool);
    if (selection.editorTool !== undefined) setEditorTool(selection.editorTool);
  }, []);

  // Launch-time hydration (§6.2). Same setter order as the single read it
  // replaces, including the metricsVersion bump that follows setGraph.
  const hydrateUiSettings = useCallback((s: UiSettings) => {
    setPanelDensity(s.panelDensity);
    setPrimaryCommitAction(s.primaryCommitAction);
    setAutoFetch(s.autoFetch);
    setHealthRefresh(s.healthRefresh);
    setGraph(s.graph);
    setMetricsVersion((v) => v + 1);
    // Spec-002 (additive/optional): missing ⇒ default.
    setGraphStyle(s.graphStyle ?? 'standard');
    setGraphSeason(s.graphSeason ?? DEFAULT_SEASON);
    // Spec-003 (additive/optional): missing ⇒ default.
    setGraphFirstParent(s.graphFirstParent ?? false);
    setGraphFoldLinear(s.graphFoldLinear ?? false);
    setGraphMinimapAlwaysShow(s.graphMinimapAlwaysShow ?? false);
    setGraphColorMode(s.graphColorMode ?? 'lane');
    setGraphRefFilter(s.graphRefFilter ?? null);
    setAiEnabled(s.aiEnabled);
    setAiConflictAutonomy(s.aiConflictAutonomy);
    setAiConsented(s.aiConsented);
    setMcpConsented(s.mcpConsented);
    setMcpWriteConsented(s.mcpWriteConsented);
    setAutoCheckUpdates(s.autoCheckUpdates);
    setProfiles(s.profiles);
    setTerminalTool(s.terminalTool);
    setEditorTool(s.editorTool);
    setDev(s.dev);
    setAiDockHeight(s.aiDockHeight);
    setAiDockCollapsed(s.aiDockCollapsed);
    setAiStreamLog(s.aiStreamLog);
    setAiConflictTools(s.aiConflictTools);
    setAiIncludePartialMessages(s.aiIncludePartialMessages);
    setAiIdleTimeoutSecs(s.aiIdleTimeoutSecs);
    setAiHardCapSecs(s.aiHardCapSecs);
    setAiMaxTurns(s.aiMaxTurns);
    setAiMaxBudgetUsd(s.aiMaxBudgetUsd);
    setAiBulkMaxBytes(s.aiBulkMaxBytes);
  }, []);

  return {
    panelDensity,
    primaryCommitAction,
    autoFetch,
    healthRefresh,
    graph,
    graphStyle,
    graphSeason,
    graphFirstParent,
    graphFoldLinear,
    graphMinimapAlwaysShow,
    graphColorMode,
    graphRefFilter,
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
    aiRun: {
      aiConflictTools,
      aiStreamLog,
      aiIncludePartialMessages,
      aiIdleTimeoutSecs,
      aiHardCapSecs,
      aiMaxTurns,
      aiMaxBudgetUsd,
      aiBulkMaxBytes,
    },
    handleSettingsChange,
    queueSettingsWrite,
    hydrateUiSettings,
    adoptToolSelection,
    settingsSaveFailed,
    retrySettingsSave,
  };
}
