// P69f §4.1 — the category-id → page-component map.
//
// `settingsCatalog.ts` stays React-free, so `SETTINGS_CATEGORIES` carries no
// `Page` field; `SettingsPanel` zips the rail order against this map. Pages take
// NO props — they read `SettingsContext`.

import type { ComponentType } from 'react';

import type { SettingsCategoryId } from '../types';
import { GeneralCategory } from './GeneralCategory';
import { AppearanceCategory } from './AppearanceCategory';
import { GraphCategory } from './GraphCategory';
import { AiCategory } from './AiCategory';
import { IdentitiesCategory } from './IdentitiesCategory';
import { GitConfigCategory } from './GitConfigCategory';
import { AboutCategory } from './AboutCategory';

export const CATEGORY_PAGES: Record<SettingsCategoryId, ComponentType> = {
  general: GeneralCategory,
  appearance: AppearanceCategory,
  graph: GraphCategory,
  ai: AiCategory,
  identities: IdentitiesCategory,
  'git-config': GitConfigCategory,
  about: AboutCategory,
};
