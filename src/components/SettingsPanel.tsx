// P11c §3.1: full-screen Settings "page" overlay. Mirrors the ShortcutOverlay
// idiom (`.dialog-overlay` backdrop, a `.settings-card` variant, role="dialog",
// backdrop-click + ✕ close; Esc is handled by App's global overlay-Esc effect).
// Every control fires `onChange` with a partial patch — App updates its own
// state immediately (live preview) and debounces the persist.

import { useState } from 'react';

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
} from '../ipc';
import { type McpScope } from '../lib/mcpAddCommand';
import type { AiRunPrefs } from '../settings/aiRunPrefs';
import type { UpdateUiState } from '../hooks/useUpdateController';
import { SettingsExternalToolsSection } from './SettingsExternalToolsSection';
import { SettingsGitConfigSection } from './SettingsGitConfigSection';
import { SettingsProfilesSection } from './SettingsProfilesSection';
import { SettingsMcpSection } from './SettingsMcpSection';
import { SettingsUpdatesSection } from './SettingsUpdatesSection';
import { SettingsAppearanceSection } from './SettingsAppearanceSection';
import { SettingsGraphSection } from './SettingsGraphSection';
import { SettingsAiSection } from './SettingsAiSection';
import { SettingsAiRunSection } from './SettingsAiRunSection';
import { NumberSlider } from './NumberSlider';
import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  HEALTH_REFRESH_INTERVAL_MAX,
  HEALTH_REFRESH_INTERVAL_MIN,
} from '../settings/ranges';

export interface SettingsPanelProps {
  open: boolean;
  onClose(): void;
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

export function SettingsPanel({
  open,
  onClose,
  theme,
  listView,
  panelDensity,
  autoFetch,
  healthRefresh,
  graph,
  onChange,
  onToggleTheme,
  onToggleListView,
  aiEnabled,
  aiConflictAutonomy,
  aiConsented,
  aiAvailability,
  onRequestEnableAi,
  aiRun,
  mcpStatus,
  mcpConsented,
  onSetMcpEnabled,
  onRequestEnableMcp,
  mcpWriteConsented,
  onSetMcpAllowWrite,
  onRequestEnableMcpWrite,
  repoPath,
  configInitialFocus,
  profiles,
  terminalCommand,
  editorCommand,
  onRegisterMcp,
  onShowOnboarding,
  updateCurrentVersion,
  autoCheckUpdates,
  updateState,
  onCheckUpdate,
  onOpenUpdateDialog,
}: SettingsPanelProps) {
  // In-flight scope for the "Add" registration buttons — disables a button while
  // its `claude mcp add` run is pending. Hooks run unconditionally (before the
  // `open` early-return below).
  const [mcpRegistering, setMcpRegistering] = useState<McpScope | null>(null);

  if (!open) return null;

  // Enabling requires one-time consent (§8.1): turning ON without consent defers
  // to App's consent dialog; turning OFF patches immediately (consent is kept).
  const handleEnableToggle = (checked: boolean): void => {
    if (!checked) {
      onChange({ aiEnabled: false });
      return;
    }
    if (aiConsented) onChange({ aiEnabled: true });
    else onRequestEnableAi();
  };
  const aiActive = aiEnabled && aiConsented;

  // MCP enable toggle (P16): enabling without consent defers to App's consent
  // dialog; disabling stops immediately.
  const mcpEnabled = mcpStatus?.enabled ?? false;
  const handleMcpEnableToggle = (checked: boolean): void => {
    if (!checked) {
      onSetMcpEnabled(false);
      return;
    }
    if (mcpConsented) onSetMcpEnabled(true);
    else onRequestEnableMcp();
  };

  // MCP write-gate (P16c): only meaningful while the server runs. Turning ON
  // without the stronger write consent defers to App's write-consent dialog;
  // turning OFF flips immediately. Either direction bounces the server.
  const mcpAllowWrite = mcpStatus?.allowWrite ?? false;
  const handleMcpWriteToggle = (checked: boolean): void => {
    if (!checked) {
      onSetMcpAllowWrite(false);
      return;
    }
    if (mcpWriteConsented) onSetMcpAllowWrite(true);
    else onRequestEnableMcpWrite();
  };

  // Run `claude mcp add` for one scope, holding the in-flight scope so its "Add"
  // button disables until the run settles (App owns the toast).
  const handleRegister = (scope: McpScope): void => {
    setMcpRegistering(scope);
    void onRegisterMcp(scope).finally(() => setMcpRegistering(null));
  };

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card settings-card" role="dialog" aria-label="Settings">
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">Settings</h2>
          <button
            type="button"
            className="btn-icon shortcut-close"
            aria-label="Close"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>

        {/* --- Getting started (P43a) --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">Getting started</h3>
          <div className="settings-row">
            <span className="settings-control-label">First-run tour</span>
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              onClick={onShowOnboarding}
            >
              {'Show welcome tour'}
            </button>
          </div>
        </section>

        {/* --- Updates (P42b) --- */}
        <SettingsUpdatesSection
          currentVersion={updateCurrentVersion}
          autoCheckUpdates={autoCheckUpdates}
          onToggleAutoCheck={(v) => onChange({ autoCheckUpdates: v })}
          checkState={updateState}
          onCheck={onCheckUpdate}
          onOpenDialog={onOpenUpdateDialog}
        />

        {/* --- Background jobs (P30 §6) --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">Background jobs</h3>
          <p className="settings-section-desc">
            Runs in the background for all open repositories. Auto-fetch never pulls, pushes, or
            prompts for credentials.
          </p>
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={autoFetch.enabled}
              onChange={(e) => onChange({ autoFetch: { ...autoFetch, enabled: e.target.checked } })}
            />
            <span>Enable auto-fetch</span>
          </label>
          <NumberSlider
            id="settings-auto-fetch-interval"
            /* P69d / UI §5.3.7: two rows both labelled "Interval" gave two controls in
               one dialog the SAME accessible name. Ids are unchanged. */
            label="Fetch every"
            value={autoFetch.intervalMinutes}
            min={AUTO_FETCH_INTERVAL_MIN}
            max={AUTO_FETCH_INTERVAL_MAX}
            unit="minutes"
            disabled={!autoFetch.enabled}
            onChange={(v) => onChange({ autoFetch: { ...autoFetch, intervalMinutes: v } })}
          />
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={healthRefresh.enabled}
              onChange={(e) =>
                onChange({ healthRefresh: { ...healthRefresh, enabled: e.target.checked } })
              }
            />
            <span>Refresh status &amp; health periodically</span>
          </label>
          <NumberSlider
            id="settings-health-refresh-interval"
            label="Refresh every"
            value={healthRefresh.intervalMinutes}
            min={HEALTH_REFRESH_INTERVAL_MIN}
            max={HEALTH_REFRESH_INTERVAL_MAX}
            unit="minutes"
            disabled={!healthRefresh.enabled}
            onChange={(v) =>
              onChange({ healthRefresh: { ...healthRefresh, intervalMinutes: v } })
            }
          />
        </section>

        {/* --- Graph (geometry sliders + P51 per-row detail toggles) --- */}
        <SettingsGraphSection graph={graph} onChange={onChange} />

        {/* --- Appearance (theme / file lists / P67 panel density) --- */}
        <SettingsAppearanceSection
          theme={theme}
          onToggleTheme={onToggleTheme}
          listView={listView}
          onToggleListView={onToggleListView}
          panelDensity={panelDensity}
          onChange={onChange}
        />

        {/* --- Git config (P40b) --- */}
        <SettingsGitConfigSection repoId={repoPath} initialFocus={configInitialFocus} />

        {/* --- External tools (P49b) --- */}
        <SettingsExternalToolsSection
          terminalCommand={terminalCommand}
          editorCommand={editorCommand}
          onChange={onChange}
        />

        {/* --- Identity profiles (P44) --- */}
        <SettingsProfilesSection
          repoId={repoPath}
          profiles={profiles}
          onProfilesChange={(next) => onChange({ profiles: next })}
        />

        {/* --- AI assistance (P13 §8.1, P68g §2.3) --- */}
        <SettingsAiSection
          aiEnabled={aiEnabled}
          aiConflictAutonomy={aiConflictAutonomy}
          aiActive={aiActive}
          aiAvailability={aiAvailability}
          onToggleEnabled={handleEnableToggle}
          onChange={onChange}
        />

        {/* --- AI runs (P68g §1): the eight knobs that had no UI at all --- */}
        <SettingsAiRunSection aiRun={aiRun} aiActive={aiActive} onChange={onChange} />

        {/* --- AI access (MCP server) (P16 §10.5) --- */}
        <SettingsMcpSection
          mcpStatus={mcpStatus}
          mcpEnabled={mcpEnabled}
          mcpAllowWrite={mcpAllowWrite}
          repoPath={repoPath}
          mcpRegistering={mcpRegistering}
          onToggleEnabled={handleMcpEnableToggle}
          onToggleAllowWrite={handleMcpWriteToggle}
          onRegister={handleRegister}
        />
      </div>
    </div>
  );
}
