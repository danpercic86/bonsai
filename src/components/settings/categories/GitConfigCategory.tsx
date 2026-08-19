// P69f §1.1 — the "Git config" category page (P40b).
//
// A pass-through today; P69h fills it with the scope switch, the no-repo empty
// block, and the Level row lifted out of the leaf section. `configInitialFocus`
// is forwarded verbatim so the section's existing scroll+focus effect (and its
// `focusedOnce` guard) is untouched.

import { SettingsGitConfigSection } from '../../SettingsGitConfigSection';
import { useSettingsValues } from '../SettingsContext';

export function GitConfigCategory() {
  const { repoPath, configInitialFocus } = useSettingsValues();

  /* --- Git config (P40b) --- */
  return <SettingsGitConfigSection repoId={repoPath} initialFocus={configInitialFocus} />;
}
