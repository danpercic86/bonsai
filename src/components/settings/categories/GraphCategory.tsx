// P69f §1.1 — the "Commit graph" category page.
//
// A thin adapter: the leaf section keeps its existing props (§2.3), so
// `SettingsSections.test.tsx` keeps rendering it directly. P69j re-skinned that
// section onto the canonical row and split it into the catalog's three groups
// (Geometry / Row details / Badges), which is why this file did not need to.

import { SettingsGraphSection } from '../../SettingsGraphSection';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function GraphCategory() {
  const { graph } = useSettingsValues();
  const { change } = useSettingsActions();

  /* --- Graph (geometry sliders + P51 per-row detail toggles) --- */
  return <SettingsGraphSection graph={graph} onChange={change} />;
}
