// P69f §1.1 — the "Appearance" category page.
//
// Replaces `SettingsAppearanceSection.tsx` (deleted). The markup below was moved
// VERBATIM out of that file, comments included; only the value/callback sources
// changed (props → SettingsContext).
//
// Theme and File lists have their own toolbar buttons, so App owns dedicated
// toggle callbacks for them. Panel density has NO toolbar button (P67 §4.3), so
// it rides the generic debounced `change` patch path instead.

import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function AppearanceCategory() {
  const { theme, listView, panelDensity } = useSettingsValues();
  const { change, toggleTheme, toggleListView } = useSettingsActions();

  return (
    <section className="settings-section">
      <h3 className="settings-section-title">Appearance</h3>
      <div className="settings-row">
        <span className="settings-control-label">Theme</span>
        <button type="button" className="btn-secondary settings-toggle-btn" onClick={toggleTheme}>
          {theme === 'dark' ? 'Dark' : 'Light'}
        </button>
      </div>
      <div className="settings-row">
        <span className="settings-control-label">File lists</span>
        <button
          type="button"
          className="btn-secondary settings-toggle-btn"
          onClick={toggleListView}
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
          onClick={() => change({ panelDensity: panelDensity === 'cozy' ? 'compact' : 'cozy' })}
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
