// P69g — UI §2 / §7: the two-pane Settings shell.
//
// Overlay + 880×min(660, 100vh−64) card laid out as a grid: a 48px header
// spanning both columns, a 200px category rail, and the pane. The card never
// scrolls as a whole — the rail and the pane scroll independently.
//
// No search bar yet: shell contract D-3 ships search in P69k, because a box that
// can only find three of seven categories' rows is a control that lies.
//
// No focus TRAP (D-4): this codebase has no shared trap and no dialog has one, so
// adding one here would create an inconsistency. Focus RESTORE ships.

import { useEffect, useRef, useState } from 'react';

import { CATEGORY_PAGES } from './categories';
import { SETTINGS_CATEGORIES, settingsTabId } from './settingsCatalog';
import { SettingsRail } from './SettingsRail';
import { SettingsPaneHeader } from './SettingsPaneHeader';
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
  const { configInitialFocus } = useSettingsValues();

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

  // Focus restore (D-4): remember what was focused when the shell mounted and put
  // it back on close, falling back to the ⚙ trigger and then to <body>.
  useEffect(() => {
    const previous = document.activeElement;
    // Deep-linked opens leave initial focus to the pane's own section effect
    // (`configInitialFocus`), which focuses `user.name`. Otherwise land on the
    // rail so the dialog is immediately keyboard-navigable.
    if (!deepLinked.current) {
      document.getElementById(settingsTabId('general'))?.focus();
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
    if (paneRef.current !== null) paneRef.current.scrollTop = 0;
  }, [requestSeq, requested]);

  const select = (id: SettingsCategoryId): void => {
    setSelected(id);
    // UI §2.2 scroll reset. Focus stays where the user put it: a mouse click
    // leaves it on the rail item, a keyboard activation on the same item.
    if (paneRef.current !== null) paneRef.current.scrollTop = 0;
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

        <SettingsRail
          selected={selected}
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
          <SettingsPaneHeader
            title={category.label}
            subtitle={category.subtitle}
            trailing={HeaderTrailing === undefined ? undefined : <HeaderTrailing />}
          />
          <Page key={pageKey} />
        </div>
      </div>
    </div>
  );
}
