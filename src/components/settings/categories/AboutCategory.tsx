// P69g — the "About" category page: the version/updates group (P42b) and the
// welcome-tour row (P43a).
//
// The tour row is the one place where the catalog `label` is the BUTTON text
// (`Show tour`) while the row reads `Welcome tour`: `label` must equal the
// control's accessible name, and a button row's control names itself.

import { SettingsUpdatesSection } from '../../SettingsUpdatesSection';
import { SettingsGroup } from '../SettingsGroup';
import { SettingsRow } from '../SettingsRow';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function AboutCategory() {
  const { updateCurrentVersion, autoCheckUpdates, updateState } = useSettingsValues();
  const { change, showOnboarding, checkUpdate, openUpdateDialog } = useSettingsActions();

  return (
    <>
      <SettingsUpdatesSection
        currentVersion={updateCurrentVersion}
        autoCheckUpdates={autoCheckUpdates}
        onToggleAutoCheck={(v) => change({ autoCheckUpdates: v })}
        checkState={updateState}
        onCheck={checkUpdate}
        onOpenDialog={openUpdateDialog}
      />

      <SettingsGroup id="about-help" title="Help">
        <SettingsRow id="about.welcome-tour" rowLabel="Welcome tour">
          <button type="button" className="btn-secondary" onClick={showOnboarding}>
            {'Show tour'}
          </button>
        </SettingsRow>
      </SettingsGroup>
    </>
  );
}
