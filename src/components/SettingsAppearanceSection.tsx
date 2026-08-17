// P67c §5.3: Settings "Appearance" section (own file, mirrors the other
// extracted settings sections — SettingsGraphSection / SettingsExternalTools).
// Theme and File-lists were lifted verbatim out of SettingsPanel.tsx; the third
// row is the new P67 right-panel density preference.
//
// Theme and File lists have their own toolbar buttons, so App owns dedicated
// toggle callbacks for them. Panel density has NO toolbar button (P67 §4.3), so
// it rides the generic debounced `onChange` patch path instead.

import type { ListView, PanelDensity, Theme, UiSettingsPatch } from '../ipc';

export interface SettingsAppearanceSectionProps {
  theme: Theme;
  onToggleTheme(): void;
  listView: ListView;
  onToggleListView(): void;
  /** P67: right-panel density (rides the debounced settings patch). */
  panelDensity: PanelDensity;
  onChange(patch: UiSettingsPatch): void;
}

export function SettingsAppearanceSection({
  theme,
  onToggleTheme,
  listView,
  onToggleListView,
  panelDensity,
  onChange,
}: SettingsAppearanceSectionProps) {
  return (
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
      {/* P67 §4: one toggling button showing the CURRENT value and flipping on
          click — the same idiom as Theme / File lists above. Deliberately not a
          segmented control (D6: room for a third value later, but the label is
          the affordance today). */}
      <div className="settings-row">
        <span className="settings-control-label">Panel density</span>
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          onClick={() => onChange({ panelDensity: panelDensity === 'cozy' ? 'compact' : 'cozy' })}
        >
          {panelDensity === 'cozy' ? 'Cozy' : 'Compact'}
        </button>
      </div>
      {/* D6: the two densities are INDEPENDENT knobs (right-panel chrome vs
          canvas row geometry) — cross-reference only, no master switch. */}
      <p className="settings-section-desc">
        Affects the right panel only. Graph row density is a separate setting under Graph →
        Compact rows.
      </p>
    </section>
  );
}
