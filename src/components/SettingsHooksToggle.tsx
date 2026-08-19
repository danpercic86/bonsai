// P59a: repo-scoped "Run git hooks" row, bound to `bonsai.runHooks` in the
// repo's LOCAL git config. Independent of the Git-config scope switch — hooks are
// always a per-repo (Local) concern.
//
// P69h made it PRESENTATIONAL. It used to run its own `getConfig(repoId,'local')`
// nested inside the Git-config section's own read of the same view — one of the
// three duplicate mount reads the increment collapsed. `useGitConfigEditor` now
// owns the read and the write; this file owns the row.

import { SettingsGroup } from './settings/SettingsGroup';
import { SettingsRow } from './settings/SettingsRow';
import { SettingsSwitch } from './settings/SettingsSwitch';
import { settingsRowHelpId } from './settings/settingsCatalog';

const ROW = 'git-config.run-hooks';
const CONTROL_ID = 'settings-run-hooks';

export interface SettingsHooksToggleProps {
  /** Unset ⇒ ON (git's default); only an explicit `false` disables. */
  enabled: boolean;
  /** The local config has not been read yet (or could not be), so `enabled` is
   *  git's default rather than this repo's answer. The switch shows it but must
   *  not accept a click — flipping a value the user has not been shown yet is
   *  how a repo silently loses its `bonsai.runHooks=false`. */
  loading: boolean;
  /** A write is in flight (the switch holds its optimistic value meanwhile). */
  busy: boolean;
  error: string | null;
  onToggle(next: boolean): void;
}

export function SettingsHooksToggle({
  enabled,
  loading,
  busy,
  error,
  onToggle,
}: SettingsHooksToggleProps) {
  return (
    <SettingsGroup id="git-config-hooks" title="Hooks">
      <SettingsRow id={ROW} controlId={CONTROL_ID} disabled={loading || busy}>
        <SettingsSwitch
          id={CONTROL_ID}
          checked={enabled}
          disabled={loading || busy}
          describedBy={settingsRowHelpId(ROW)}
          onChange={onToggle}
        />
      </SettingsRow>
      <p className="settings-config-hint">
        When off, commits run with <span className="mono">--no-verify</span> and{' '}
        <span className="mono">bonsai.runHooks=false</span> is written to this repo. Unset means
        hooks run (git’s default).
      </p>
      {error !== null && <p className="settings-config-error">{error}</p>}
    </SettingsGroup>
  );
}
