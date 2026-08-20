// P69k — UI §3.1/§3.3: the cross-category result list.
//
// The pane's content is REPLACED while the query is non-empty: every matching
// row from every category, rendered with its real control, live and editable in
// place, grouped under its category name (UI D3). Rejected alternatives — filter
// the rail, filter the current page, jump-and-highlight — are in §3.1.
//
// The mechanism is deliberately not a second renderer for the ~60 rows: each
// category's own page is mounted inside a `SettingsSearchContext`, and the rows
// that did not match remove themselves. A result therefore cannot drift from the
// pane, and there is nothing to keep in sync when a page changes.
//
// `HeaderTrailing` is rendered too, because exactly one catalogued row lives in
// the pane header rather than in a group (`git-config.scope`). It self-filters
// through the same context, so a category whose only hit is elsewhere shows no
// scope switch.

import { useMemo } from 'react';

import { CATEGORY_PAGES } from './categories';
import { SETTINGS_CATEGORIES } from './settingsCatalog';
import { SettingsEmpty } from './SettingsEmpty';
import { SettingsSearchContext } from './SettingsSearchContext';
import type { SettingsCategoryId, SettingsIndexEntry } from './types';

export function SettingsResults({
  query,
  terms,
  matches,
  onGoToCategory,
  onClear,
}: {
  /** The raw query — the zero-match copy quotes it verbatim (§3.3). */
  query: string;
  /** Lowercased split terms, for `<mark>` highlighting in labels. */
  terms: readonly string[];
  matches: readonly SettingsIndexEntry[];
  /** Clears the query and selects the category (§3.2). */
  onGoToCategory(id: SettingsCategoryId): void;
  onClear(): void;
}) {
  const visible = useMemo(() => new Set(matches.map((m) => m.id)), [matches]);

  if (matches.length === 0) {
    return (
      <SettingsEmpty
        title={`No settings match “${query.trim()}”.`}
        body="Try a shorter word — for example graph, fetch, identity, or spend."
        actionLabel="Clear search"
        onAction={onClear}
      />
    );
  }

  const hit = SETTINGS_CATEGORIES.filter((c) => matches.some((m) => m.category === c.id));

  return (
    <div className="settings-results">
      {hit.map((category) => {
        const { Page, HeaderTrailing } = CATEGORY_PAGES[category.id];
        const titleId = `settings-results-${category.id}`;
        return (
          <section className="settings-results-group" key={category.id} aria-labelledby={titleId}>
            <div className="settings-results-header">
              <h3 className="settings-results-title" id={titleId}>
                {category.label}
              </h3>
              <button
                type="button"
                className="settings-results-goto"
                onClick={() => onGoToCategory(category.id)}
              >
                {`Go to ${category.label}`}
              </button>
            </div>
            <SettingsSearchContext.Provider value={{ terms, visible, category: category.id }}>
              {HeaderTrailing !== undefined && (
                <div className="settings-results-trailing">
                  <HeaderTrailing />
                </div>
              )}
              <Page />
            </SettingsSearchContext.Provider>
          </section>
        );
      })}
    </div>
  );
}
