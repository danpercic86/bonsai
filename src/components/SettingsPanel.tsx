// P11c §3.1: full-screen Settings "page" overlay. Mirrors the ShortcutOverlay
// idiom (`.dialog-overlay` backdrop, a `.settings-card` variant, role="dialog",
// backdrop-click + ✕ close; Esc is handled by App's global overlay-Esc effect).
// Every control fires `onChange` with a partial patch — App updates its own
// state immediately (live preview) and debounces the persist.

import type {
  AutoFetchSettings,
  GraphPrefs,
  ListView,
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
}: SettingsPanelProps) {
  if (!open) return null;

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
      </div>
    </div>
  );
}
