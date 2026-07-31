// P11c §3.1: full-screen Settings "page" overlay. Mirrors the ShortcutOverlay
// idiom (`.dialog-overlay` backdrop, a `.settings-card` variant, role="dialog",
// backdrop-click + ✕ close; Esc is handled by App's global overlay-Esc effect).
// Every control fires `onChange` with a partial patch — App updates its own
// state immediately (live preview) and debounces the persist.

import type {
  AiAutonomy,
  AiAvailability,
  AutoFetchSettings,
  GraphPrefs,
  ListView,
  McpStatus,
  Theme,
  UiSettingsPatch,
} from '../ipc';
import {
  AUTO_FETCH_INTERVAL_MAX,
  AUTO_FETCH_INTERVAL_MIN,
  AVATAR_RADIUS_MAX,
  AVATAR_RADIUS_MIN,
  LANE_WIDTH_MAX,
  LANE_WIDTH_MIN,
  ROW_HEIGHT_MAX,
  ROW_HEIGHT_MIN,
} from '../settings/ranges';

export interface SettingsPanelProps {
  open: boolean;
  onClose(): void;
  theme: Theme;
  listView: ListView;
  autoFetch: AutoFetchSettings;
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
  // Embedded MCP server (P16). Live runtime status (null until first loaded);
  // consent gate + start/stop are owned by App, like the AI section.
  mcpStatus: McpStatus | null;
  mcpConsented: boolean;
  /** Start/stop the embedded MCP server (read-only in P16b). */
  onSetMcpEnabled(enabled: boolean): void;
  /** Enabling without prior consent: App shows the MCP consent dialog and only
   *  starts the server (+ records consent) on confirm. */
  onRequestEnableMcp(): void;
}

/** Best-effort clipboard copy (harness + native). Silent on failure — the
 *  values are also visible for manual selection. */
function copyText(text: string): void {
  void navigator.clipboard?.writeText(text).catch(() => {});
}

function clamp(v: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, v));
}

/** A labeled number input + range slider bound to the same value. Clamps and
 *  ignores non-numeric input (empty field) before calling `onChange`. */
function NumberSlider({
  id,
  label,
  value,
  min,
  max,
  unit,
  disabled,
  onChange,
}: {
  id: string;
  label: string;
  value: number;
  min: number;
  max: number;
  unit?: string;
  disabled?: boolean;
  onChange(next: number): void;
}) {
  const commit = (raw: string): void => {
    const n = Number(raw);
    if (Number.isNaN(n)) return;
    onChange(clamp(Math.round(n), min, max));
  };
  return (
    <div className={`settings-control${disabled === true ? ' is-disabled' : ''}`}>
      <label className="settings-control-label" htmlFor={id}>
        {label}
      </label>
      <div className="settings-control-inputs">
        <input
          className="settings-range"
          type="range"
          min={min}
          max={max}
          step={1}
          value={value}
          disabled={disabled}
          onChange={(e) => commit(e.target.value)}
          aria-label={label}
        />
        <input
          id={id}
          className="settings-number"
          type="number"
          min={min}
          max={max}
          step={1}
          value={value}
          disabled={disabled}
          onChange={(e) => commit(e.target.value)}
        />
        {unit !== undefined && <span className="settings-unit">{unit}</span>}
      </div>
    </div>
  );
}

export function SettingsPanel({
  open,
  onClose,
  theme,
  listView,
  autoFetch,
  graph,
  onChange,
  onToggleTheme,
  onToggleListView,
  aiEnabled,
  aiConflictAutonomy,
  aiConsented,
  aiAvailability,
  onRequestEnableAi,
  mcpStatus,
  mcpConsented,
  onSetMcpEnabled,
  onRequestEnableMcp,
}: SettingsPanelProps) {
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

        {/* --- Auto-fetch --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">Auto-fetch</h3>
          <p className="settings-section-desc">Fetch the active repository automatically.</p>
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
            label="Interval"
            value={autoFetch.intervalMinutes}
            min={AUTO_FETCH_INTERVAL_MIN}
            max={AUTO_FETCH_INTERVAL_MAX}
            unit="minutes"
            disabled={!autoFetch.enabled}
            onChange={(v) => onChange({ autoFetch: { ...autoFetch, intervalMinutes: v } })}
          />
        </section>

        {/* --- Graph --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">Graph</h3>
          <p className="settings-section-desc">Tune the commit-graph geometry. Changes preview live.</p>
          {/* P11d: single node-size knob == avatarRadius (post-P7 the graph has
              no commit dot — each commit is an avatar disc). The old dotRadius
              slider was a dead no-op; the field is kept in the model, only the
              UI control is gone. */}
          <NumberSlider
            id="settings-graph-avatar"
            label="Commit node size"
            value={graph.avatarRadius}
            min={AVATAR_RADIUS_MIN}
            max={AVATAR_RADIUS_MAX}
            unit="px"
            onChange={(v) => onChange({ graph: { ...graph, avatarRadius: v } })}
          />
          <NumberSlider
            id="settings-graph-row"
            label="Row height"
            value={graph.rowHeight}
            min={ROW_HEIGHT_MIN}
            max={ROW_HEIGHT_MAX}
            unit="px"
            onChange={(v) => onChange({ graph: { ...graph, rowHeight: v } })}
          />
          <NumberSlider
            id="settings-graph-lane"
            label="Lane width"
            value={graph.laneWidth}
            min={LANE_WIDTH_MIN}
            max={LANE_WIDTH_MAX}
            unit="px"
            onChange={(v) => onChange({ graph: { ...graph, laneWidth: v } })}
          />
        </section>

        {/* --- Appearance --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">Appearance</h3>
          <div className="settings-row">
            <span className="settings-control-label">Theme</span>
            <button type="button" className="btn-secondary settings-toggle-btn" onClick={onToggleTheme}>
              {theme === 'dark' ? 'Dark' : 'Light'}
            </button>
          </div>
          <div className="settings-row">
            <span className="settings-control-label">File lists</span>
            <button
              type="button"
              className="btn-secondary settings-toggle-btn"
              onClick={onToggleListView}
            >
              {listView === 'tree' ? 'Tree' : 'Flat'}
            </button>
          </div>
        </section>

        {/* --- AI assistance (P13 §8.1) --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">AI assistance</h3>
          <p className="settings-section-desc">
            Resolve merge conflicts with the local Claude Code CLI, under your Claude subscription.
          </p>
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={aiEnabled}
              onChange={(e) => handleEnableToggle(e.target.checked)}
            />
            <span>Enable AI features</span>
          </label>

          <fieldset className="settings-radio-group" disabled={!aiActive}>
            <legend className="settings-radio-legend">Conflict resolution</legend>
            <label className="settings-radio">
              <input
                type="radio"
                name="ai-autonomy"
                checked={aiConflictAutonomy === 'proposeReview'}
                disabled={!aiActive}
                onChange={() => onChange({ aiConflictAutonomy: 'proposeReview' })}
              />
              <span>Propose &amp; review</span>
            </label>
            <label className="settings-radio">
              <input
                type="radio"
                name="ai-autonomy"
                checked={aiConflictAutonomy === 'autoResolve'}
                disabled={!aiActive}
                onChange={() => onChange({ aiConflictAutonomy: 'autoResolve' })}
              />
              <span>Auto-resolve, then review</span>
            </label>
          </fieldset>

          {aiAvailability === null ? (
            <p className="settings-ai-status">Checking for the Claude Code CLI…</p>
          ) : aiAvailability.installed ? (
            <p className="settings-ai-status settings-ai-status-ok">{aiAvailability.detail}</p>
          ) : (
            <p className="settings-ai-status settings-ai-status-warn" role="note">
              Claude Code CLI not found on PATH — install it and log in to use AI features
            </p>
          )}
        </section>

        {/* --- AI access (MCP server) (P16 §10.5) --- */}
        <section className="settings-section">
          <h3 className="settings-section-title">AI access (MCP server)</h3>
          <p className="settings-section-desc">
            Run a local MCP server on 127.0.0.1 so an external AI client (e.g. Claude Code) can read
            the repositories you have open in Bonsai. Access requires the token below; the server is
            read-only.
          </p>
          <label className="settings-checkbox">
            <input
              type="checkbox"
              checked={mcpEnabled}
              onChange={(e) => handleMcpEnableToggle(e.target.checked)}
            />
            <span>Enable MCP server</span>
          </label>

          {mcpEnabled && mcpStatus !== null ? (
            <p className="settings-ai-status settings-ai-status-ok">
              Running on port {mcpStatus.port} · {mcpStatus.toolCount} tools (read-only)
            </p>
          ) : (
            <p className="settings-ai-status">Stopped.</p>
          )}

          {mcpEnabled && mcpStatus !== null && (
            <>
              <div className="settings-control">
                <label className="settings-control-label" htmlFor="settings-mcp-url">
                  Server URL
                </label>
                <div className="settings-control-inputs">
                  <input
                    id="settings-mcp-url"
                    className="settings-number settings-mcp-field"
                    type="text"
                    readOnly
                    value={mcpStatus.url ?? ''}
                    onFocus={(e) => e.target.select()}
                  />
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    onClick={() => mcpStatus.url !== null && copyText(mcpStatus.url)}
                  >
                    Copy
                  </button>
                </div>
              </div>

              <div className="settings-control">
                <label className="settings-control-label" htmlFor="settings-mcp-token">
                  Bearer token
                </label>
                <div className="settings-control-inputs">
                  <input
                    id="settings-mcp-token"
                    className="settings-number settings-mcp-field"
                    type="text"
                    readOnly
                    value={mcpStatus.token ?? ''}
                    onFocus={(e) => e.target.select()}
                  />
                  <button
                    type="button"
                    className="btn-secondary settings-toggle-btn"
                    onClick={() => mcpStatus.token !== null && copyText(mcpStatus.token)}
                  >
                    Copy
                  </button>
                </div>
              </div>

              <div className="settings-row">
                <span className="settings-control-label">Register with Claude Code</span>
                <button
                  type="button"
                  className="btn-secondary settings-toggle-btn"
                  disabled={mcpStatus.claudeAddCommand === null}
                  onClick={() =>
                    mcpStatus.claudeAddCommand !== null && copyText(mcpStatus.claudeAddCommand)
                  }
                >
                  Copy command
                </button>
              </div>
            </>
          )}
        </section>
      </div>
    </div>
  );
}
