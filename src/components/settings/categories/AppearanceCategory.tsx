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

import { useId } from 'react';

import { settingsRowHelpId, settingsRowLabelId } from '../settingsCatalog';
import { SettingsGroup } from '../SettingsGroup';
import { SettingsRow } from '../SettingsRow';
import { SettingsSegmented } from '../SettingsSegmented';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';
import { Combobox } from '../../Combobox';
import type {
  GraphColorMode,
  GraphSeason,
  GraphStyle,
  ListView,
  PanelDensity,
  Theme,
} from '../../../ipc';

const THEME = 'appearance.theme';
const GRAPH_STYLE = 'appearance.graph-style';
const GRAPH_SEASON = 'appearance.graph-season';
const GRAPH_COLORS = 'appearance.graph-colors';
const FILE_LISTS = 'appearance.file-lists';
const DENSITY = 'appearance.panel-density';

const SEASON_OPTIONS = [
  { value: 'living', label: 'Living' },
  { value: 'spring', label: 'Spring' },
  { value: 'autumn', label: 'Autumn' },
];

export function AppearanceCategory() {
  const { theme, listView, panelDensity, graphStyle, graphSeason, graphColorMode } =
    useSettingsValues();
  const { change, toggleTheme, toggleListView } = useSettingsActions();

  // §12.3.3: Season depends on graphStyle. When Standard, the whole group is
  // disabled via `<fieldset disabled>` (removes the control from the tab order)
  // and the lead sentence explains the dim; the `.55` dim rides the row.
  const seasonDisabled = graphStyle !== 'bonsai';
  const seasonControlId = useId();
  const seasonLeadId = useId();

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

      <SettingsRow id={GRAPH_STYLE}>
        <SettingsSegmented<GraphStyle>
          name={GRAPH_STYLE}
          value={graphStyle}
          labelledBy={settingsRowLabelId(GRAPH_STYLE)}
          describedBy={settingsRowHelpId(GRAPH_STYLE)}
          options={[
            { value: 'standard', label: 'Standard' },
            { value: 'bonsai', label: 'Bonsai' },
          ]}
          onChange={(next) => change({ graphStyle: next })}
        />
      </SettingsRow>

      {/* §12.3.3: one `<fieldset disabled>` around the dependent Season row. The
          reason LEADS the group and the fieldset points `aria-describedby` at it
          only while disabled (a dangling idref is worse than none). The dim lives
          on `.settings-row.is-disabled`, never the fieldset. */}
      <fieldset
        className="settings-fieldset"
        disabled={seasonDisabled}
        aria-describedby={seasonDisabled ? seasonLeadId : undefined}
      >
        <p className="settings-group-lead" id={seasonLeadId}>
          Seasonal palettes apply to the Bonsai graph style.
        </p>
        <SettingsRow id={GRAPH_SEASON} controlId={seasonControlId} disabled={seasonDisabled}>
          <Combobox
            id={seasonControlId}
            options={SEASON_OPTIONS}
            value={graphSeason}
            disabled={seasonDisabled}
            /* P112 §1: pre-existing gap, fixed here because the prop now exists.
               This row's help line was unannounced — `Combobox` had no way to
               describe its input. The catalog gives this row a `help`, so
               `settingsRowHelpId` resolves to a real element (unlike the picker
               rows, whose explanation is stateful and hand-written). */
            describedBy={settingsRowHelpId(GRAPH_SEASON)}
            onChange={(next) => change({ graphSeason: next as GraphSeason })}
          />
        </SettingsRow>
      </fieldset>

      {/* Spec-006: edge/lane-ring coloring — after Season (keeps the Graph
          style + Season dependency pair adjacent), per spec-006-ui.md §1.3. */}
      <SettingsRow id={GRAPH_COLORS}>
        <SettingsSegmented<GraphColorMode>
          name={GRAPH_COLORS}
          value={graphColorMode}
          labelledBy={settingsRowLabelId(GRAPH_COLORS)}
          describedBy={settingsRowHelpId(GRAPH_COLORS)}
          options={[
            { value: 'lane', label: 'Branch lanes' },
            { value: 'author', label: 'Author' },
          ]}
          onChange={(next) => change({ graphColorMode: next })}
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
