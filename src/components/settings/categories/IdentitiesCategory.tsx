// P69f §1.1 — the "Identities" category page (P44 named identity profiles).
// The leaf section keeps its existing props (§2.3).

import { SettingsProfilesSection } from '../../SettingsProfilesSection';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';

export function IdentitiesCategory() {
  const { repoPath, profiles } = useSettingsValues();
  const { change } = useSettingsActions();

  /* --- Identity profiles (P44) --- */
  return (
    <SettingsProfilesSection
      repoId={repoPath}
      profiles={profiles}
      onProfilesChange={(next) => change({ profiles: next })}
    />
  );
}
