// P69g — UI §1.1 / §2.2 / §6.1 / §7: the category rail.
//
// `role="tablist"` with `role="tab"` items and a roving tabindex: the rail is ONE
// tab stop, arrows move focus inside it, `→` hands focus to the pane.
//
// **Manual activation** (shell contract D-5, superseding ui-reference §12.4's
// "move and activate"): arrows move focus only, Enter/Space select. Automatic
// activation would fire a `getConfig` round-trip every time focus passed over
// `Git config`.
//
// Grouping is two `aria-hidden` hairlines plus one hueless `repo` pill (UI D2) —
// never heading elements, which are an ARIA hazard inside a tablist. The pill's
// meaning is folded into the tab's accessible NAME (`Git config, repository`), so
// the scope is never carried by a colour AT cannot see.

import { Fragment, useState } from 'react';

import { SETTINGS_CATEGORIES, settingsTabId } from './settingsCatalog';
import type { SettingsCategoryId } from './types';

const IDS: readonly SettingsCategoryId[] = SETTINGS_CATEGORIES.map((c) => c.id);

export function SettingsRail({
  selected,
  counts,
  onSelect,
  onFocusPane,
}: {
  selected: SettingsCategoryId;
  /** P69k / UI §3.2: per-category match counts while a search is running; `null`
   *  otherwise. Items with matches are EMPHASISED rather than the empty ones
   *  dimmed, and every item stays clickable — clicking any item clears the query
   *  (the shell's `onSelect` does that), so a zero-count tab is still the way
   *  back out of a search. */
  counts: ReadonlyMap<SettingsCategoryId, number> | null;
  onSelect(id: SettingsCategoryId): void;
  /** `→` from the rail moves focus into the pane (the tablist convention). */
  onFocusPane(): void;
}) {
  /* APG roving tabindex: the single rail tab stop follows FOCUS, not selection.
     Binding it to `selected` instead would throw the arrow position away every
     time the user tabbed out and back — they would land on the selected category
     again and have to re-arrow. `onFocus` covers arrows, clicks and Tab alike. */
  const [focusedId, setFocusedId] = useState<SettingsCategoryId>(selected);

  const move = (delta: number, from: SettingsCategoryId): void => {
    const at = IDS.indexOf(from);
    const next = IDS[(at + delta + IDS.length) % IDS.length];
    document.getElementById(settingsTabId(next))?.focus();
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLButtonElement>, id: SettingsCategoryId): void => {
    switch (e.key) {
      case 'ArrowDown':
        move(1, id);
        break;
      case 'ArrowUp':
        move(-1, id);
        break;
      case 'Home':
        document.getElementById(settingsTabId(IDS[0]))?.focus();
        break;
      case 'End':
        document.getElementById(settingsTabId(IDS[IDS.length - 1]))?.focus();
        break;
      case 'ArrowRight':
        onFocusPane();
        break;
      default:
        return;
    }
    e.preventDefault();
  };

  return (
    <div
      className="settings-rail"
      role="tablist"
      aria-orientation="vertical"
      aria-label="Settings categories"
    >
      {SETTINGS_CATEGORIES.map((category) => {
        const count = counts?.get(category.id) ?? 0;
        // The base name is the one UI §7.1 froze; the count is appended only
        // while searching, so an AT user gets the same per-category signal the
        // emphasis gives a sighted one instead of a bare "12 settings match".
        const baseName = category.pill === 'repo' ? `${category.label}, repository` : undefined;
        const name =
          counts === null
            ? baseName
            : `${baseName ?? category.label}, ${String(count)} ${count === 1 ? 'match' : 'matches'}`;
        return (
        <Fragment key={category.id}>
          {category.dividerBefore === true && (
            <div className="settings-rail-divider" aria-hidden="true" />
          )}
          <button
            type="button"
            role="tab"
            id={settingsTabId(category.id)}
            className={[
              'settings-rail-item',
              category.id === selected ? 'is-selected' : '',
              /* Emphasis is inverted, not dimmed: the item WITH matches is
                 lifted, a zero-count one keeps the resting colour. A dimmed
                 clickable item fell under 4.5:1 in both themes. */
              counts !== null && count > 0 ? 'is-hit' : '',
            ]
              .filter((c) => c !== '')
              .join(' ')}
            aria-selected={category.id === selected}
            aria-controls="settings-pane"
            /* Exactly one rail stop; arrows focus siblings programmatically (a
               tabindex=-1 button still takes focus, which then moves the stop). */
            tabIndex={category.id === focusedId ? 0 : -1}
            onFocus={() => setFocusedId(category.id)}
            title={category.label}
            /* The pill is decorative, so its meaning is folded into the NAME
               rather than left as a colour AT cannot see: `Git config,
               repository`. An explicit label, not a visually-hidden span —
               name computation joins sibling nodes with a space, which would
               give `Git config , repository`. */
            aria-label={name}
            onClick={() => onSelect(category.id)}
            onKeyDown={(e) => onKeyDown(e, category.id)}
          >
            <span className="settings-rail-label">{category.label}</span>
            {category.pill === 'repo' && (
              <span className="settings-rail-pill" aria-hidden="true">
                repo
              </span>
            )}
            {counts !== null && (
              <span className="settings-rail-count" aria-hidden="true">
                {count}
              </span>
            )}
          </button>
        </Fragment>
        );
      })}
    </div>
  );
}
