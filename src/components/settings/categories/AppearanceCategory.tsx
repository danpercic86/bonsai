// P69g — the "Appearance" category page.
//
// The three self-labelling `btn-secondary` toggles are gone (UI §5.3): a button
// reading `Dark` says "make it dark", the opposite of what it does. Each is now a
// segmented control over native radios, so the CURRENT value is visible and the
// OTHER value is the affordance. Selecting the already-selected segment does
// nothing, which the old always-flipping button could not express.
//
// Theme and File lists have their own toolbar buttons, so App owns dedicated
// toggle callbacks for them (two values ⇒ selecting the other one IS the toggle).
// Panel density has no toolbar button (P67 §4.3), so it rides the generic patch.

import { settingsRowHelpId, settingsRowLabelId } from '../settingsCatalog';
import { SettingsGroup } from '../SettingsGroup';
import { SettingsRow } from '../SettingsRow';
import { SettingsSegmented } from '../SettingsSegmented';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';
import type { ListView, PanelDensity, Theme } from '../../../ipc';

const THEME = 'appearance.theme';
const FILE_LISTS = 'appearance.file-lists';
const DENSITY = 'appearance.panel-density';

export function AppearanceCategory() {
  const { theme, listView, panelDensity } = useSettingsValues();
  const { change, toggleTheme, toggleListView } = useSettingsActions();

  return (
    <SettingsGroup id="appearance-appearance" title="Appearance">
      <SettingsRow id={THEME}>
        <SettingsSegmented<Theme>
          name={THEME}
          value={theme}
          labelledBy={settingsRowLabelId(THEME)}
          describedBy={settingsRowHelpId(THEME)}
          options={[
            { value: 'dark', label: 'Dark' },
            { value: 'light', label: 'Light' },
          ]}
          onChange={toggleTheme}
        />
      </SettingsRow>

      <SettingsRow id={FILE_LISTS}>
        <SettingsSegmented<ListView>
          name={FILE_LISTS}
          value={listView}
          labelledBy={settingsRowLabelId(FILE_LISTS)}
          describedBy={settingsRowHelpId(FILE_LISTS)}
          options={[
            { value: 'tree', label: 'Tree' },
            { value: 'flat', label: 'Flat' },
          ]}
          onChange={toggleListView}
        />
      </SettingsRow>

      {/* D6: the two densities are INDEPENDENT knobs (right-panel chrome vs canvas
          row geometry). The cross-reference is row help, not a master switch. */}
      <SettingsRow id={DENSITY}>
        <SettingsSegmented<PanelDensity>
          name={DENSITY}
          value={panelDensity}
          labelledBy={settingsRowLabelId(DENSITY)}
          describedBy={settingsRowHelpId(DENSITY)}
          options={[
            { value: 'cozy', label: 'Cozy' },
            { value: 'compact', label: 'Compact' },
          ]}
          onChange={(next) => change({ panelDensity: next })}
        />
      </SettingsRow>
    </SettingsGroup>
  );
}
