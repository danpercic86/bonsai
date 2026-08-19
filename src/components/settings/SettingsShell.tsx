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
  onClose,
}: {
  initialCategory?: SettingsCategoryId;
  onClose(): void;
}) {
  const { configInitialFocus } = useSettingsValues();

  // Seeded once per MOUNT. `SettingsPanel` returns null while closed, so the shell
  // mounts fresh on every false → true transition of `open` — which covers a
  // second deep link that arrives while Settings is CLOSED, and only that case.
  //
  // ⚠️ A deep link arriving while Settings is ALREADY OPEN will NOT move the
  // category: there is no mount, so nothing re-seeds. Unreachable today (every
  // entry point opens the panel), but P69h's acceptance explicitly requires the
  // already-open case, so P69h must add the `requestSeq` counter its contract
  // lists — do not conclude from this comment that it is unnecessary.
  //
  // `configInitialFocus === 'identity'` IS the `configMissing` deep link (the
  // commit-error "Set identity…" linkage): it must select Git config, or the
  // section's scroll+focus effect would never mount. Deriving it here keeps
  // App.tsx — at its size ratchet — untouched.
  const [selected, setSelected] = useState<SettingsCategoryId>(
    () => initialCategory ?? (configInitialFocus === 'identity' ? 'git-config' : 'general'),
  );
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

  const select = (id: SettingsCategoryId): void => {
    setSelected(id);
    // UI §2.2 scroll reset. Focus stays where the user put it: a mouse click
    // leaves it on the rail item, a keyboard activation on the same item.
    if (paneRef.current !== null) paneRef.current.scrollTop = 0;
  };

  const category = SETTINGS_CATEGORIES.find((c) => c.id === selected) ?? SETTINGS_CATEGORIES[0];
  const Page = CATEGORY_PAGES[category.id];

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
          <SettingsPaneHeader title={category.label} subtitle={category.subtitle} />
          <Page />
        </div>
      </div>
    </div>
  );
}
