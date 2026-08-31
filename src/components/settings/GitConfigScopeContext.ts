// P69h — UI §1.1: the Git-config SCOPE (which config file is being edited).
//
// It is a context rather than page state because the switch itself lives in the
// pane HEADER (`SettingsPaneHeader`'s trailing slot, rendered by `SettingsShell`)
// while the form it drives lives in the pane BODY. One provider above both is the
// only way to keep them in sync without the shell learning what Git config is.
//
// The context module is separate from the components that use it (the
// `SettingsContext.ts` / `SettingsProvider.tsx` idiom) so this file can export a
// hook without tripping `react-refresh/only-export-components`.

import { createContext, useContext } from 'react';

import type { ConfigLevelArg } from '../../ipc';

export interface GitConfigScope {
  level: ConfigLevelArg;
  setLevel(next: ConfigLevelArg): void;
}

/** No provider ⇒ `local`, read-only. A bare `SettingsGitConfigSection` (its own
 *  suite renders one) then behaves exactly as it did before the switch moved to
 *  the pane header, instead of throwing. */
const LOCAL_ONLY: GitConfigScope = Object.freeze({
  level: 'local',
  setLevel: () => {},
});

export const GitConfigScopeContext = createContext<GitConfigScope>(LOCAL_ONLY);

export function useGitConfigScope(): GitConfigScope {
  return useContext(GitConfigScopeContext);
}
