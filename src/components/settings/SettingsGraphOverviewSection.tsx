// Spec-005 (UI contract §6): the Commit-graph "Overview" group — one switch
// binding `graphMinimapAlwaysShow` (the on-demand overview rail's persistent
// mode). Rendered after the Declutter group. Leaf section (§2.3): its props are
// its only value source, so the bare-render test idiom keeps working.

import type { UiSettingsPatch } from '../../ipc';
import { SettingsGroup } from './SettingsGroup';
import { SettingsSwitchRow } from './SettingsSwitchRow';

const MINIMAP_ALWAYS_SHOW = 'graph.minimap-always-show';

export interface SettingsGraphOverviewSectionProps {
  graphMinimapAlwaysShow: boolean;
  /** The shared debounced settings patch channel (App owns the persist). */
  onChange(patch: UiSettingsPatch): void;
}

export function SettingsGraphOverviewSection({
  graphMinimapAlwaysShow,
  onChange,
}: SettingsGraphOverviewSectionProps) {
  return (
    <SettingsGroup id="graph-overview" title="Overview">
      <SettingsSwitchRow
        id={MINIMAP_ALWAYS_SHOW}
        checked={graphMinimapAlwaysShow}
        onChange={(next) => onChange({ graphMinimapAlwaysShow: next })}
      />
    </SettingsGroup>
  );
}
