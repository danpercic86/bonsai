// P69g — UI §2 / §7: the two-pane Settings shell.
//
// Overlay + 880×min(660, 100vh−64) card laid out as a grid: a 48px header
// spanning both columns, a 200px category rail, and the pane. The card never
// scrolls as a whole — the rail and the pane scroll independently.
//
// P69k adds search (UI §3): every category is catalog-shaped now, so the box can
// find every row rather than three categories' worth. The pane switches between
// the selected category and a cross-category result list; DOM order is
// close ✕ → search → rail → pane, and the grid places each cell explicitly so
// that tab order needs no `tabindex` juggling (UI §7.2).
//
// No focus TRAP (D-4): this codebase has no shared trap and no dialog has one, so
// adding one here would create an inconsistency. Focus RESTORE ships.

import { useEffect, useMemo, useRef, useState } from 'react';

import { CATEGORY_PAGES } from './categories';
import { SETTINGS_CATEGORIES, searchSettings, settingsTabId } from './settingsCatalog';
import { SettingsRail } from './SettingsRail';
import { SettingsPaneHeader } from './SettingsPaneHeader';
import { SettingsResults } from './SettingsResults';
import { SettingsSearchBar } from './SettingsSearchBar';
import { useSettingsValues } from './SettingsContext';
import type { SettingsCategoryId } from './types';

export function SettingsShell({
  initialCategory,
  requestSeq,
  onClose,
}: {
  initialCategory?: SettingsCategoryId;
  /** Monotonic per OPEN request (§5.4). Every open path bumps it — including the
   *  plain ⚙ click, which passes no category and therefore only clears state. */
  requestSeq: number;
  onClose(): void;
}) {
  const { configInitialFocus, repoPath, aiEnabled, aiConsented, mcpStatus, profiles } =
    useSettingsValues();

  // Seeded on MOUNT (`SettingsPanel` returns null while closed, so the shell
  // mounts fresh on every false → true transition of `open`) and RE-SEEDED on
  // every later `requestSeq` change — that second half is what makes a deep link
  // land while Settings is already open, which no mount would cover (§5.4.3).
  //
  // `configInitialFocus === 'identity'` IS the `configMissing` deep link (the
  // commit-error "Set identity…" linkage): it must select Git config, or the
  // section's scroll+focus effect would never mount.
  const requested = initialCategory ?? (configInitialFocus === 'identity' ? 'git-config' : null);
  const [selected, setSelected] = useState<SettingsCategoryId>(() => requested ?? 'general');
  const deepLinked = useRef(initialCategory !== undefined || configInitialFocus === 'identity');
  const paneRef = useRef<HTMLDivElement | null>(null);

  // UI §3.2: no debounce — matching is a synchronous pass over ~60 static entries,
  // and a delay on a list this size only makes the box feel broken.
  const [query, setQuery] = useState('');
  const searching = query.trim() !== '';
  // A row whose `requires` fails is not in the DOM, so it must not be matched:
  // otherwise the status line and the rail count a row whose result block would
  // render empty (P69k review A3). One object, one source, every consumer.
  const availability = useMemo(
    () => ({ repoPath, aiEnabled, aiConsented, mcpStatus, profiles }),
    [repoPath, aiEnabled, aiConsented, mcpStatus, profiles],
  );
  const matches = useMemo(() => searchSettings(query, availability), [query, availability]);
  const terms = useMemo(
    () =>
      query
        .toLowerCase()
        .split(/\s+/)
        .filter((t) => t !== ''),
    [query],
  );
  const counts = useMemo(() => {
    const byCategory = new Map<SettingsCategoryId, number>();
    for (const entry of matches) {
      byCategory.set(entry.category, (byCategory.get(entry.category) ?? 0) + 1);
    }
    return byCategory;
  }, [matches]);

  const scrollPaneToTop = (): void => {
    if (paneRef.current !== null) paneRef.current.scrollTop = 0;
  };

  const changeQuery = (next: string): void => {
    setQuery(next);
    // The result list is rebuilt on every keystroke, so a retained scroll offset
    // would land the user in the middle of a list they have not seen.
    scrollPaneToTop();
  };

  // Focus restore (D-4): remember what was focused when the shell mounted and put
  // it back on close, falling back to the ⚙ trigger and then to <body>.
  useEffect(() => {
    const previous = document.activeElement;
    // UI §7.2: initial focus is the SEARCH input — a text field, so no keystroke
    // can activate anything, and it is the fastest route for a user who knows the
    // setting's name but not its category.
    //
    // Deep-linked opens are the exception and must stay one: `configInitialFocus`
    // makes the Git-config section focus `user.name`, and a search box that
    // grabbed focus here would silently defeat the commit-error linkage.
    if (!deepLinked.current) {
      document.querySelector<HTMLElement>('.settings-search .list-filter-input')?.focus();
    }
    return () => {
      const fallback = document.querySelector<HTMLElement>('.settings-toggle');
      const target = previous instanceof HTMLElement && previous.isConnected ? previous : fallback;
      target?.focus();
    };
  }, []);

  // §5.4.3: keyed on the monotonic seq, not on an `open` transition, so a second
  // deep link in the same session still lands. A request that names no category
  // (the plain ⚙ click) only clears state — it must never yank the user off the
  // category they are reading.
  const seenSeq = useRef(requestSeq);
  useEffect(() => {
    if (requestSeq === seenSeq.current) return;
    seenSeq.current = requestSeq;
    if (requested === null) return;
    setSelected(requested);
    // A deep link names a category, so a live search must not keep the result
    // list on screen in front of it.
    setQuery('');
    scrollPaneToTop();
  }, [requestSeq, requested]);

  const select = (id: SettingsCategoryId): void => {
    setSelected(id);
    // UI §3.2: clicking ANY rail item clears the query, including a zero-count
    // one — the rail is the way out of a search, not a second filtered view.
    setQuery('');
    // UI §2.2 scroll reset. Focus stays where the user put it: a mouse click
    // leaves it on the rail item, a keyboard activation on the same item.
    scrollPaneToTop();
  };

  const category = SETTINGS_CATEGORIES.find((c) => c.id === selected) ?? SETTINGS_CATEGORIES[0];
  const { Page, HeaderTrailing } = CATEGORY_PAGES[category.id];
  // A deep link that asks for a FOCUS target must re-run the target page's focus
  // effect even when that page is already mounted (Settings open, already on Git
  // config, second failed commit). Remounting the page is the honest way to say
  // "this is a new request"; ordinary category switches keep a stable key.
  const pageKey =
    category.id === 'git-config' && configInitialFocus === 'identity'
      ? `${category.id}:${requestSeq}`
      : category.id;

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        className="dialog-card settings-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
      >
        <div className="settings-header">
          <h2 className="dialog-title settings-title" id="settings-title">
            Settings
          </h2>
          <button
            type="button"
            className="btn-icon settings-close"
            aria-label="Close"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>

        {/* DOM order: the search bar precedes the rail so Tab runs
            ✕ → search → rail → pane (UI §7.2). The grid puts it back in column 2
            beside the rail, so nothing moves visually. */}
        <SettingsSearchBar query={query} matchCount={matches.length} onChange={changeQuery} />

        <SettingsRail
          selected={selected}
          counts={searching ? counts : null}
          onSelect={select}
          onFocusPane={() => paneRef.current?.focus()}
        />

        <div
          className="settings-pane"
          id="settings-pane"
          role="tabpanel"
          tabIndex={-1}
          aria-labelledby={settingsTabId(category.id)}
          ref={paneRef}
        >
          {searching ? (
            <SettingsResults
              query={query}
              terms={terms}
              matches={matches}
              onGoToCategory={select}
              onClear={() => changeQuery('')}
            />
          ) : (
            <>
              <SettingsPaneHeader
                title={category.label}
                subtitle={category.subtitle}
                trailing={HeaderTrailing === undefined ? undefined : <HeaderTrailing />}
              />
              <Page key={pageKey} />
            </>
          )}
        </div>
      </div>
    </div>
  );
}
