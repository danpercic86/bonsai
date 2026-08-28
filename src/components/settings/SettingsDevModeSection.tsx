// P91 §2 — group 1 "Dev mode": the master switch + the live status card.
//
// The status card is NOT gated by the switch (it is the readout of the gate
// itself). The switch starts/stops WRITING and deletes nothing (§10).

import type { DevSettings, LogSessionInfo } from '../../ipc';
import { SettingsGroup } from './SettingsGroup';
import { SettingsSwitchRow } from './SettingsSwitchRow';
import { DevSessionStatus } from './DevSessionStatus';

export function SettingsDevModeSection({
  dev,
  info,
  onToggleEnabled,
}: {
  dev: DevSettings;
  info: LogSessionInfo | null;
  onToggleEnabled(next: boolean): void;
}) {
  return (
    <SettingsGroup id="dev-mode" title="Dev mode">
      <SettingsSwitchRow id="dev.enabled" checked={dev.enabled} onChange={onToggleEnabled} />
      <DevSessionStatus info={info} enabled={dev.enabled} />
    </SettingsGroup>
  );
}
