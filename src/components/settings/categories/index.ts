// P69f §4.1 — the category-id → page-component map.
//
// `settingsCatalog.ts` stays React-free, so `SETTINGS_CATEGORIES` carries no
// `Page` field; `SettingsShell` zips the rail order against this map. Pages take
// NO props — they read `SettingsContext`.
//
// P69h adds the optional `HeaderTrailing`: a category may put one control in the
// pane header's trailing slot (UI §1.1 — the Git-config scope switch). Declaring
// it here rather than special-casing `git-config` in the shell keeps the shell
// generic; the state the two halves share travels through their own context.

import type { ComponentType } from 'react';

import type { SettingsCategoryId } from '../types';
import { GitConfigScopeSwitch } from '../GitConfigScope';
import { GeneralCategory } from './GeneralCategory';
import { AppearanceCategory } from './AppearanceCategory';
import { GraphCategory } from './GraphCategory';
import { AiCategory } from './AiCategory';
import { IdentitiesCategory } from './IdentitiesCategory';
import { GitConfigCategory } from './GitConfigCategory';
import { AboutCategory } from './AboutCategory';

export interface SettingsCategoryPage {
  Page: ComponentType;
  /** Rendered in `SettingsPaneHeader`'s trailing slot while this category is
   *  selected. Must render `null` when its own precondition is unmet. */
  HeaderTrailing?: ComponentType;
}

export const CATEGORY_PAGES: Record<SettingsCategoryId, SettingsCategoryPage> = {
  general: { Page: GeneralCategory },
  appearance: { Page: AppearanceCategory },
  graph: { Page: GraphCategory },
  ai: { Page: AiCategory },
  identities: { Page: IdentitiesCategory },
  'git-config': { Page: GitConfigCategory, HeaderTrailing: GitConfigScopeSwitch },
  about: { Page: AboutCategory },
};
