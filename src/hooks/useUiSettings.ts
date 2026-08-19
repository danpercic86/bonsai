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
// NOT owned here: the STATE for `theme`, `listView`, `paneWidths` and
// `onboardingSeen`. Those live in App (toolbar toggles, the resize drag, the
// first-run overlay) and are hydrated alongside this hook, not by it — but
// P69b routed their PERSIST calls through `queueSettingsWrite` so every field
// on disk goes out through this one coalescing window. Nothing else may call
// `ipc.setUiSettings` directly.
//
// Adding a setting is therefore two edits — a `useState` and a patch arm — plus
// whatever prop threads it to a child.

import { useCallback, useEffect, useRef, useState } from 'react';

import type { ToastTone } from '../components/Toasts';
import { ipc } from '../ipc';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import type {
  AiAutonomy,
  AiConflictTools,
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

/** Coalescing window for the settings write (§3.2; mirrors the session write). */
const SETTINGS_SAVE_DEBOUNCE_MS = 300;
/** P69b: automatic attempts after a failed write, then the patch waits for the
 *  next user change or teardown. Backoff 300 / 600 / 1200 ms — long enough to
 *  ride out a transient lock, bounded so a dead disk cannot spin or toast-storm. */
const SETTINGS_SAVE_MAX_RETRIES = 3;

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
  // P49b: external-tool command templates ('' ⇒ backend auto-detects per-OS).
  // Threaded into the Settings section; persisted via handleSettingsChange.
  const [terminalCommand, setTerminalCommand] = useState('');
  const [editorCommand, setEditorCommand] = useState('');
  // P11c §3.2: debounced settings persist — accumulates partial patches so a
  // burst of knob changes within the window all reach disk in one write.
  const settingsSaveTimerRef = useRef<number | null>(null);
  const pendingSettingsPatchRef = useRef<UiSettingsPatch>({});

  // P69b: at most ONE write may be outstanding. A second concurrent write makes
  // the failure merge-back below unsound: the newer values would already have
  // left `pendingSettingsPatchRef` inside that other write, so restoring the
  // failed patch could resurrect a value the UI has moved past. With a single
  // writer, everything newer is provably still pending.
  const settingsWriteInFlightRef = useRef(false);
  // Consecutive failed writes — bounds the automatic retry and keeps a dead disk
  // to ONE toast. Reset by a success and by any new user change.
  const settingsFailureStreakRef = useRef(0);
  // Set by the effect cleanup: after teardown nothing may arm a new timer. A
  // forced (teardown) flush that then REJECTS would otherwise leave a retry
  // timer outliving the component — harmless in production, but in tests it can
  // fire into a later test's spy.
  const disposedRef = useRef(false);
  // Late-bound so `armSettingsSave` can schedule the flush that is defined after
  // it (the timer only ever fires once the ref holds the real function).
  const flushRef = useRef<(force?: boolean) => void>(() => {});

  const armSettingsSave = useCallback((delayMs: number) => {
    if (disposedRef.current) return;
    if (settingsSaveTimerRef.current !== null) {
      window.clearTimeout(settingsSaveTimerRef.current);
    }
    settingsSaveTimerRef.current = window.setTimeout(() => {
      settingsSaveTimerRef.current = null;
      flushRef.current();
    }, delayMs);
  }, []);

  // P69b: send the accumulated patch now. Called by the debounce timer, by the
  // bounded retry, and by teardown (`force`) — unmount, `pagehide`,
  // `beforeunload` — where a patch still inside the window would otherwise die
  // with the JS context.
  const flushSettingsWrite = useCallback(
    (force = false) => {
      if (settingsSaveTimerRef.current !== null) {
        window.clearTimeout(settingsSaveTimerRef.current);
        settingsSaveTimerRef.current = null;
      }
      // A write is already out: leave the patch pending and let that write's
      // settle handler pump it, so only one write is ever in flight. Teardown
      // forces the send anyway — a possible reorder beats losing the patch.
      if (settingsWriteInFlightRef.current && !force) return;
      const merged = pendingSettingsPatchRef.current;
      // Nothing pending — also the StrictMode double-mount case, where the first
      // cleanup must not fire a write.
      if (Object.keys(merged).length === 0) return;
      pendingSettingsPatchRef.current = {};
      settingsWriteInFlightRef.current = true;
      void ipc.setUiSettings(merged).then(
        () => {
          settingsWriteInFlightRef.current = false;
          settingsFailureStreakRef.current = 0;
          // A change made while this write was out is still unsent. Re-arm the
          // FULL window rather than writing immediately: the burst it belongs to
          // may still be in progress, and coalescing it is the whole point.
          if (Object.keys(pendingSettingsPatchRef.current).length > 0) {
            armSettingsSave(SETTINGS_SAVE_DEBOUNCE_MS);
          }
        },
        (e: unknown) => {
          settingsWriteInFlightRef.current = false;
          // P69b defect 2: the write failed, so put the patch back rather than
          // drop it. Spread `merged` FIRST so anything changed since (which,
          // single-writer, is still pending) wins — a retry must never resurrect
          // a value the UI has moved past.
          pendingSettingsPatchRef.current = { ...merged, ...pendingSettingsPatchRef.current };
          const streak = settingsFailureStreakRef.current;
          // One toast per failure streak, not one per retry.
          if (streak === 0) pushToast('error', `Could not save settings: ${errorMessage(e)}`);
          settingsFailureStreakRef.current = streak + 1;
          // Bounded backoff (300 / 600 / 1200 ms), then wait for the next change
          // or teardown: a permanently failing disk must not spin forever.
          if (streak < SETTINGS_SAVE_MAX_RETRIES) {
            armSettingsSave(SETTINGS_SAVE_DEBOUNCE_MS * 2 ** streak);
          }
        },
      );
    },
    [armSettingsSave, pushToast],
  );
  // Render-time ref mutation, deliberately — NOT the bug deleted from App.tsx's
  // `paneWidthsRef`. Both deps (`armSettingsSave`, `pushToast`) are stable, so
  // every candidate closure here is behaviourally identical and a re-assignment
  // from a discarded render cannot install a stale one. Do not "fix" by symmetry.
  flushRef.current = flushSettingsWrite;

  // P11c §3.2 / P69b: merge into the pending patch and re-arm the single 300 ms
  // window. Every persisted setting rides this — the ones this hook owns state
  // for (via `handleSettingsChange`) and App's four (theme, listView,
  // paneWidths, onboardingSeen) — so one burst is one write, whatever moved.
  const queueSettingsWrite = useCallback(
    (patch: UiSettingsPatch) => {
      pendingSettingsPatchRef.current = { ...pendingSettingsPatchRef.current, ...patch };
      // A fresh user action earns a fresh retry budget (and, if it fails again, a
      // fresh toast); the streak only silences the automatic retries.
      settingsFailureStreakRef.current = 0;
      armSettingsSave(SETTINGS_SAVE_DEBOUNCE_MS);
    },
    [armSettingsSave],
  );

  // P69b defect 3: flush a patch that is still inside the debounce window when
  // the page goes away. React cleanup does NOT run on window close, app quit or
  // reload, and `App` is the root (`src/main.tsx`) so it never unmounts in
  // production — `pagehide`/`beforeunload` are what actually cover quit and
  // reload; the cleanup covers HMR and tests. Synchronous fire-and-forget: the
  // IPC call is dispatched, never awaited (nothing may await during teardown).
  useEffect(() => {
    // Cleared on every (re)mount: StrictMode's dev double-mount runs the cleanup
    // once on the SAME instance, and a permanently-disposed hook would then
    // never persist another setting.
    disposedRef.current = false;
    const flushNow = () => flushRef.current(true);
    window.addEventListener('pagehide', flushNow);
    window.addEventListener('beforeunload', flushNow);
    return () => {
      window.removeEventListener('pagehide', flushNow);
      window.removeEventListener('beforeunload', flushNow);
      flushNow();
      // After this point a rejection may still land, but it must not schedule.
      disposedRef.current = true;
    };
  }, []);

  // P11c §3.2: apply a Settings patch — update local state immediately (live
  // preview; graph changes bump metricsVersion so the canvas re-measures), then
  // debounce a single merged persist (~300 ms, shared with App's four writers).
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
  };
}
