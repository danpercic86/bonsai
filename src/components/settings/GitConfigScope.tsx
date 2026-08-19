// P69h — UI §1.1 deliverable 1, half two: the pane-level scope treatment.
//
// Git config is the ONE per-repository category in an otherwise global dialog, so
// the answer to "what am I editing?" must be a fact, not an inference. Two
// carriers, neither of them colour: the rail's hueless `repo` pill (folded into
// the tab's accessible NAME by `SettingsRail`), and this header block — a
// segmented `This repository | Global` switch plus a line naming the actual FILE.
//
// The switch is stamped `data-setting-id="git-config.scope"`: it is a catalogued
// settings row that happens to live in the pane header, which is why the coverage
// guard exempts exactly this one row from the "inside a .settings-group" rule.

import { useEffect, useMemo, useState, type ReactNode } from 'react';

import type { ConfigLevelArg } from '../../ipc';
import { GitConfigScopeContext, useGitConfigScope } from './GitConfigScopeContext';
import { useSettingsValues } from './SettingsContext';
import { settingsRowLabelId } from './settingsCatalog';
import { SettingsSegmented } from './SettingsSegmented';

const SCOPE_ROW = 'git-config.scope';

const OPTIONS: readonly { value: ConfigLevelArg; label: string }[] = [
  { value: 'local', label: 'This repository' },
  { value: 'global', label: 'Global' },
];

/** Last path segment of a workdir path, on either separator. */
function folderName(path: string): string {
  const segments = path.split(/[\\/]/).filter((s) => s !== '');
  return segments[segments.length - 1] ?? path;
}

export function GitConfigScopeProvider({ children }: { children: ReactNode }) {
  const { repoPath } = useSettingsValues();
  const [level, setLevel] = useState<ConfigLevelArg>('local');

  // A different repository is a different `.git/config`; landing on Global
  // because the last repo was left there would silently retarget the next write.
  useEffect(() => setLevel('local'), [repoPath]);

  const scope = useMemo(() => ({ level, setLevel }), [level]);
  return <GitConfigScopeContext.Provider value={scope}>{children}</GitConfigScopeContext.Provider>;
}

/** The pane-header trailing block. Renders nothing without a repo — the pane is
 *  showing `SettingsEmpty` then, and a scope switch over no config is a dead
 *  control (it is also why `git-config.scope` carries `requires: 'repo'`). */
export function GitConfigScopeSwitch() {
  const { repoPath } = useSettingsValues();
  const { level, setLevel } = useGitConfigScope();
  if (repoPath === null) return null;

  const labelId = settingsRowLabelId(SCOPE_ROW);
  return (
    <div className="settings-scope" data-setting-id={SCOPE_ROW}>
      <div className="settings-scope-control">
        <span className="settings-scope-label" id={labelId}>
          Scope
        </span>
        <div className="settings-scope-input" data-setting-control="">
          <SettingsSegmented
            name="settings-git-config-scope"
            value={level}
            options={OPTIONS}
            labelledBy={labelId}
            onChange={setLevel}
          />
        </div>
      </div>
      {/* Naming the file is the point: "Local" is libgit2's word for "the one
          inside this repository", and nothing on screen said which repository. */}
      {level === 'local' ? (
        <p className="settings-scope-line" title={repoPath}>
          Editing <span className="mono">.git/config</span> in {folderName(repoPath)}
        </p>
      ) : (
        <p className="settings-scope-line">
          Editing your global Git config (<span className="mono">~/.gitconfig</span>)
        </p>
      )}
    </div>
  );
}
