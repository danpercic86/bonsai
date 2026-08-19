// P11c §3.1: full-screen Settings "page" overlay. Mirrors the ShortcutOverlay
// idiom (`.dialog-overlay` backdrop, a `.settings-card` variant, role="dialog",
// backdrop-click + ✕ close; Esc is handled by App's global overlay-Esc effect).
// Every control fires `onChange` with a partial patch — App updates its own
// state immediately (live preview) and debounces the persist.
//
// P69f §2.1–2.3: the panel keeps its full prop interface (it is App's
// state-ownership boundary — `SettingsPanelProps` is declared alongside the
// adapter hook that consumes it and re-exported here, so no importer moves). It
// is now the shell: overlay + header + provider, with the seven category pages
// rendered as fragments, one after another, in the rail order of
// `SETTINGS_CATEGORIES`. The pages read `SettingsContext`; the existing leaf
// sections keep their own props and are handed them by their page.

import { CATEGORY_PAGES } from './settings/categories';
import { SETTINGS_CATEGORIES } from './settings/settingsCatalog';
import { SettingsProvider } from './settings/SettingsProvider';
import {
  useSettingsPanelAdapter,
  type SettingsPanelProps,
} from './settings/useSettingsPanelAdapter';

export type { SettingsPanelProps };

export function SettingsPanel(props: SettingsPanelProps) {
  // Hooks run unconditionally (before the `open` early-return below).
  const { values, actions } = useSettingsPanelAdapter(props);
  const { open, onClose } = props;

  if (!open) return null;

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card settings-card" role="dialog" aria-label="Settings">
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">Settings</h2>
          <button
            type="button"
            className="btn-icon shortcut-close"
            aria-label="Close"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>

        <SettingsProvider values={values} actions={actions}>
          {SETTINGS_CATEGORIES.map(({ id }) => {
            const Page = CATEGORY_PAGES[id];
            return <Page key={id} />;
          })}
        </SettingsProvider>
      </div>
    </div>
  );
}
