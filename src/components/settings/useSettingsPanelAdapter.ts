// P69f §2.1–2.2 — the App → Settings adapter.
//
// `SettingsPanelProps` is `SettingsPanel`'s public prop interface (unchanged in
// shape; `SettingsPanel` re-exports it, so no importer moves). It lives here
// with the hook that consumes it because the two are one unit: the ~41 props are
// the App state-ownership boundary, and this hook is the only place that turns
// them into the memoised `SettingsContext` value/action bags the seven category
// pages read. `SettingsPanel.tsx` is then just the overlay shell.
//
// The consent-aware handlers and the MCP in-flight state moved here VERBATIM
// from `SettingsPanel`; only their declaration form changed (plain `const` →
// `useCallback`, so the action bag can be memoised).
//
// IPC at mount, unchanged by P69f and recorded here for P69h: opening Settings
// with a repo open costs THREE `getConfig(repoId, 'local')` round-trips for what
// is one answer —
//   1. `SettingsGitConfigSection.tsx:77`  — the pane's own `ConfigView`;
//   2. `SettingsHooksToggle.tsx:31`       — nested inside that section
//                                           (`SettingsGitConfigSection.tsx:189`),
//                                           reading one key (`bonsai.runHooks`)
//                                           out of `view.advanced`;
//   3. `useEffectiveIdentity.ts:119`      — reached only from
//                                           `SettingsProfilesSection.tsx:33`.
// Calls 1 and 2 read the SAME view; the identity store's in-flight dedupe is not
// what holds the count at three (it has a single consumer today). P69h owns this
// pane and is the place to collapse them.

import { useCallback, useMemo, useRef, useState } from 'react';

import { DEFAULT_UI_SETTINGS } from '../../settings/defaults';
import { findSettingsRow } from './settingsCatalog';
import type { SettingsCategoryId, SettingsRowId } from './types';
import type { UiSettings } from '../../ipc/types';
import type {
  AiAutonomy,
  AiAvailability,
  AutoFetchSettings,
  GraphPrefs,
  HealthRefreshSettings,
  IdentityProfile,
  ListView,
  McpStatus,
  PanelDensity,
  Theme,
  UiSettingsPatch,
} from '../../ipc';
import { type McpScope } from '../../lib/mcpAddCommand';
import type { AiRunPrefs } from '../../settings/aiRunPrefs';
import type { UpdateUiState } from '../../hooks/useUpdateController';
import type { SettingsActions, SettingsValues } from './SettingsContext';

export interface SettingsPanelProps {
  open: boolean;
  onClose(): void;
  /** P69g: rail category to select on open. Omitted ⇒ `general`, except that a
   *  `configInitialFocus` deep link selects `git-config` (SettingsShell). */
  initialCategory?: SettingsCategoryId;
  theme: Theme;
  listView: ListView;
  /** P67 §4: right-panel density; patched via `onChange` (no toolbar toggle). */
  panelDensity: PanelDensity;
  autoFetch: AutoFetchSettings;
  /** P30: periodic read-only refresh signal (backend scheduler). */
  healthRefresh: HealthRefreshSettings;
  graph: GraphPrefs;
  /** Fires on ANY change with a partial patch; App debounces the persist +
   *  updates its own state so consumers re-render live. */
  onChange(patch: UiSettingsPatch): void;
  /** Reuse App's existing toggles for the Appearance section. */
  onToggleTheme(): void;
  onToggleListView(): void;
  // AI assistance (P13 §8.1).
  aiEnabled: boolean;
  aiConflictAutonomy: AiAutonomy;
  aiConsented: boolean;
  /** CLI health status; `null` while App is probing (never a dead control). */
  aiAvailability: AiAvailability | null;
  /** Enabling AI when consent has not yet been given: App shows the consent
   *  ConfirmDialog and only patches `{ aiEnabled, aiConsented }` on confirm. */
  onRequestEnableAi(): void;
  /** P68g §1: the eight AI-run knobs, threaded straight through to
   *  SettingsAiRunSection as one read-only struct (the `graph`/`autoFetch` prop
   *  idiom). Each still PATCHES independently via `onChange`. */
  aiRun: AiRunPrefs;
  // Embedded MCP server (P16). Live runtime status (null until first loaded);
  // consent gate + start/stop are owned by App, like the AI section.
  mcpStatus: McpStatus | null;
  mcpConsented: boolean;
  /** Start/stop the embedded MCP server (read-only in P16b). */
  onSetMcpEnabled(enabled: boolean): void;
  /** Enabling without prior consent: App shows the MCP consent dialog and only
   *  starts the server (+ records consent) on confirm. */
  onRequestEnableMcp(): void;
  /** One-time consent for the stronger write grant (P16c), distinct from the
   *  read `mcpConsented`. */
  mcpWriteConsented: boolean;
  /** Flip the write-gate (P16c). Bounces the running server (stop+restart on the
   *  same token/port) so the 20 mutation tools (de)register. */
  onSetMcpAllowWrite(allowWrite: boolean): void;
  /** Turning write ON without prior write-consent: App shows the write-consent
   *  dialog and only flips the gate (+ records consent) on confirm. */
  onRequestEnableMcpWrite(): void;
  /** Path of the currently-open repo, or `null` when none is open. Gates the
   *  "This repository" (`local`-scope) registration row. */
  repoPath: string | null;
  /** P40b: when 'identity', the Git-config section scrolls/focuses its Identity
   *  sub-section on open (commit-error "Set identity…" linkage). */
  configInitialFocus?: 'identity' | null;
  /** P44: named identity profiles (global app setting). CRUD persists via
   *  `onChange({ profiles })`; Apply is owned by the section's own IPC. */
  profiles: IdentityProfile[];
  /** P49b: external-tool command templates ('' ⇒ auto-detect). Threaded from
   *  App's UiSettings state; persisted via `onChange` like every other setting. */
  terminalCommand: string;
  editorCommand: string;
  /** Run `claude mcp add` for the running server at the given scope (P16).
   *  `'user'` = global, `'local'` = the open repo (private). Resolves when the
   *  run settles so the panel can clear its in-flight state (App still owns the
   *  success/error toast). */
  onRegisterMcp(scope: McpScope): Promise<void>;
  /** P43a: re-open the first-run onboarding overlay ("Show welcome tour").
   *  Does not reset the seen flag. */
  onShowOnboarding(): void;
  // Software updates (P42b). State + IPC owned by App/useUpdateController.
  /** App version from the last check; `null` until one resolves. */
  updateCurrentVersion: string | null;
  /** Auto-check-for-updates-on-launch preference (persists via `onChange`). */
  autoCheckUpdates: boolean;
  /** Shared update state — drives the inline result line + dialog affordance. */
  updateState: UpdateUiState;
  /** Run a manual (non-silent) update check. */
  onCheckUpdate(): void;
  /** Open the UpdateDialog (release notes + download flow). */
  onOpenUpdateDialog(): void;
}

/**
 * Builds the two memoised context bags (§2.2 rules 1–2). Every hook here runs
 * unconditionally, before `SettingsPanel`'s `if (!open) return null`.
 */
export function useSettingsPanelAdapter(props: SettingsPanelProps): {
  values: SettingsValues;
  actions: SettingsActions;
} {
  const {
    onChange,
    onToggleTheme,
    onToggleListView,
    aiEnabled,
    aiConsented,
    onRequestEnableAi,
    mcpStatus,
    mcpConsented,
    onSetMcpEnabled,
    onRequestEnableMcp,
    mcpWriteConsented,
    onSetMcpAllowWrite,
    onRequestEnableMcpWrite,
    onRegisterMcp,
    onShowOnboarding,
    onCheckUpdate,
    onOpenUpdateDialog,
  } = props;

  // In-flight scope for the "Add" registration buttons — disables a button while
  // its `claude mcp add` run is pending.
  const [mcpRegistering, setMcpRegistering] = useState<McpScope | null>(null);

  // Enabling requires one-time consent (§8.1): turning ON without consent defers
  // to App's consent dialog; turning OFF patches immediately (consent is kept).
  const setAiEnabled = useCallback(
    (checked: boolean): void => {
      if (!checked) {
        onChange({ aiEnabled: false });
        return;
      }
      if (aiConsented) onChange({ aiEnabled: true });
      else onRequestEnableAi();
    },
    [onChange, aiConsented, onRequestEnableAi],
  );

  // MCP enable toggle (P16): enabling without consent defers to App's consent
  // dialog; disabling stops immediately.
  const setMcpEnabled = useCallback(
    (checked: boolean): void => {
      if (!checked) {
        onSetMcpEnabled(false);
        return;
      }
      if (mcpConsented) onSetMcpEnabled(true);
      else onRequestEnableMcp();
    },
    [onSetMcpEnabled, mcpConsented, onRequestEnableMcp],
  );

  // MCP write-gate (P16c): only meaningful while the server runs. Turning ON
  // without the stronger write consent defers to App's write-consent dialog;
  // turning OFF flips immediately. Either direction bounces the server.
  const setMcpAllowWrite = useCallback(
    (checked: boolean): void => {
      if (!checked) {
        onSetMcpAllowWrite(false);
        return;
      }
      if (mcpWriteConsented) onSetMcpAllowWrite(true);
      else onRequestEnableMcpWrite();
    },
    [onSetMcpAllowWrite, mcpWriteConsented, onRequestEnableMcpWrite],
  );

  // Run `claude mcp add` for one scope, holding the in-flight scope so its "Add"
  // button disables until the run settles (App owns the toast).
  const registerMcp = useCallback(
    (scope: McpScope): void => {
      setMcpRegistering(scope);
      void onRegisterMcp(scope).finally(() => setMcpRegistering(null));
    },
    [onRegisterMcp],
  );

  const {
    theme,
    listView,
    panelDensity,
    autoFetch,
    healthRefresh,
    graph,
    aiConflictAutonomy,
    aiAvailability,
    aiRun,
    autoCheckUpdates,
    profiles,
    terminalCommand,
    editorCommand,
    repoPath,
    configInitialFocus,
    updateCurrentVersion,
    updateState,
  } = props;

  /**
   * The whole-`UiSettings` view the catalog's reset descriptors read (P69 §4.1).
   *
   * `DEFAULT_UI_SETTINGS` supplies the four keys Settings does not own —
   * `paneWidths`, `onboardingSeen`, `aiDockHeight`, `aiDockCollapsed`. None of
   * them is a settings ROW, so no descriptor can read them; `resetKey` is typed
   * to scalar `UiSettings` keys and `settingsCatalog.test.ts` pins which rows
   * carry a `reset` at all. If a future row ever resets one of those four, it
   * must be threaded as a real prop first.
   */
  const snapshot = useMemo<UiSettings>(
    () => ({
      ...DEFAULT_UI_SETTINGS,
      theme,
      listView,
      panelDensity,
      autoFetch,
      healthRefresh,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      mcpConsented,
      mcpWriteConsented,
      autoCheckUpdates,
      profiles,
      terminalCommand,
      editorCommand,
      ...aiRun,
    }),
    [
      theme,
      listView,
      panelDensity,
      autoFetch,
      healthRefresh,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      mcpConsented,
      mcpWriteConsented,
      autoCheckUpdates,
      profiles,
      terminalCommand,
      editorCommand,
      aiRun,
    ],
  );

  // Read through a ref so the ACTION bag stays stable across value changes
  // (§2.2 rule 1) — a row that only dispatches must not re-render on every patch.
  const snapshotRef = useRef(snapshot);
  snapshotRef.current = snapshot;

  const resetRow = useCallback(
    (id: SettingsRowId): void => {
      const reset = findSettingsRow(id)?.reset;
      if (reset === undefined) return;
      onChange(reset.patch(snapshotRef.current, DEFAULT_UI_SETTINGS));
    },
    [onChange],
  );

  const values = useMemo<SettingsValues>(
    () => ({
      theme,
      listView,
      panelDensity,
      autoFetch,
      healthRefresh,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      aiRun,
      mcpConsented,
      mcpWriteConsented,
      autoCheckUpdates,
      profiles,
      terminalCommand,
      editorCommand,
      repoPath,
      aiAvailability,
      aiActive: aiEnabled && aiConsented,
      mcpStatus,
      mcpEnabled: mcpStatus?.enabled ?? false,
      mcpAllowWrite: mcpStatus?.allowWrite ?? false,
      mcpRegistering,
      updateCurrentVersion,
      updateState,
      configInitialFocus,
      snapshot,
    }),
    [
      theme,
      listView,
      panelDensity,
      autoFetch,
      healthRefresh,
      graph,
      aiEnabled,
      aiConflictAutonomy,
      aiConsented,
      aiRun,
      mcpConsented,
      mcpWriteConsented,
      autoCheckUpdates,
      profiles,
      terminalCommand,
      editorCommand,
      repoPath,
      aiAvailability,
      mcpStatus,
      mcpRegistering,
      updateCurrentVersion,
      updateState,
      configInitialFocus,
      snapshot,
    ],
  );

  const actions = useMemo<SettingsActions>(
    () => ({
      change: onChange,
      toggleTheme: onToggleTheme,
      toggleListView: onToggleListView,
      setAiEnabled,
      setMcpEnabled,
      setMcpAllowWrite,
      registerMcp,
      showOnboarding: onShowOnboarding,
      checkUpdate: onCheckUpdate,
      openUpdateDialog: onOpenUpdateDialog,
      resetRow,
    }),
    [
      onChange,
      onToggleTheme,
      onToggleListView,
      setAiEnabled,
      setMcpEnabled,
      setMcpAllowWrite,
      registerMcp,
      onShowOnboarding,
      onCheckUpdate,
      onOpenUpdateDialog,
      resetRow,
    ],
  );

  return { values, actions };
}
