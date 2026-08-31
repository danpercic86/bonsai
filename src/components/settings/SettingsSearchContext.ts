// P69k — UI §3.1: the seam that turns the seven category pages into a
// cross-category result list.
//
// The results list does NOT re-implement any row. It renders the REAL page of
// every category that has a hit, wrapped in this context, and each stamped row
// asks whether it is one of the matches. That is why a search result is live and
// editable in place (§3.1) and why it can never drift from the pane: it IS the
// pane, minus the rows that did not match.
//
// FOUR components stamp `data-setting-id` and therefore consume this:
// `SettingsRow`, `IdentityProfileCard`'s action cell, `GitConfigScopeSwitch`
// (the one catalogued row that lives in the pane header) and
// `GitConfigAdvanced`, whose two aggregate blocks (`git-config.behaviour`,
// `git-config.custom-keys`) are stamped on `role="group"` elements. Adding a
// fifth stamper without wiring it here silently renders a non-matching block
// inside a result list — keep this count honest.
//
// Absent provider ⇒ no search is running ⇒ everything renders, unchanged.

import { createContext, useContext } from 'react';

import { SETTINGS_INDEX } from './settingsCatalog';
import type { SettingsCategoryId, SettingsRowId } from './types';

export interface SettingsSearchState {
  /** Lowercased whitespace-split query terms. Never empty while a search runs. */
  terms: readonly string[];
  /** Ids of the rows that matched, across every category. */
  visible: ReadonlySet<SettingsRowId>;
  /** The category whose page this provider wraps — groups need it to decide
   *  whether any of THEIR rows survived (group titles repeat across categories). */
  category: SettingsCategoryId;
}

export const SettingsSearchContext = createContext<SettingsSearchState | null>(null);

/** The active search, or `null` when the pane is showing a plain category. */
export function useSettingsSearch(): SettingsSearchState | null {
  return useContext(SettingsSearchContext);
}

/** False only when a search is running and this row is not one of its hits. */
export function useSettingsRowVisible(id: SettingsRowId): boolean {
  const search = useContext(SettingsSearchContext);
  return search === null || search.visible.has(id);
}

/**
 * False only when a search is running and none of the group's rows survived.
 *
 * Matched on (category, group title) because group titles repeat across
 * categories ("Identity" exists in two). The coverage guard is what keeps a
 * `<SettingsGroup title>` equal to the catalog `group` of every row inside it,
 * so this lookup cannot silently answer for the wrong group.
 *
 * Sections also need it directly: a note that leads a group carries an id some
 * OTHER element points at with `aria-describedby`, and a dangling idref is worse
 * than none (ui-reference §12.3.3).
 */
export function useSettingsGroupVisible(title: string): boolean {
  const search = useContext(SettingsSearchContext);
  if (search === null) return true;
  return SETTINGS_INDEX.some(
    (e) => e.category === search.category && e.group === title && search.visible.has(e.id),
  );
}
