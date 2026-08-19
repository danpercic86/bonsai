// P69h §1.1/§1.2 — the "Git config" category page (P40b).
//
// The ONE per-repository category. Its scope switch lives in the pane header
// (`GitConfigScopeSwitch`, registered as this category's `HeaderTrailing`); this
// page owns the two states the pane can be in — a repo, or none.
//
// With no repo open the pane is NOT a disabled form and not a bare sentence: it
// is an in-pane empty block that names the reason and offers the fix (UI §1.2).
// The rail item stays enabled, because a dead tab explains nothing.

import { SettingsGitConfigSection } from '../../SettingsGitConfigSection';
import { useSettingsActions, useSettingsValues } from '../SettingsContext';
import { SettingsEmpty } from '../SettingsEmpty';

export function GitConfigCategory() {
  const { repoPath, configInitialFocus } = useSettingsValues();
  const { openRepository } = useSettingsActions();

  if (repoPath === null) {
    return (
      <SettingsEmpty
        title="No repository open"
        body="Git config is stored per repository. Open one to view and edit it."
        actionLabel="Open repository…"
        onAction={openRepository}
      />
    );
  }

  return <SettingsGitConfigSection repoId={repoPath} initialFocus={configInitialFocus} />;
}
