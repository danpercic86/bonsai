// P11c §3.1: the Settings overlay. `.dialog-overlay` backdrop, a `.settings-card`
// variant, role="dialog", backdrop-mousedown + ✕ close; Esc is handled by App's
// global overlay-Esc effect. Every control fires `onChange` with a partial patch —
// App updates its own state immediately (live preview) and debounces the persist.
//
// P69f §2.1–2.3: the panel keeps its full prop interface (it is App's
// state-ownership boundary — `SettingsPanelProps` is declared alongside the
// adapter hook that consumes it and re-exported here, so no importer moves).
//
// P69g: it is now the props façade + provider only. `SettingsShell` owns the
// two-pane layout, the category rail and the selected-category state, and renders
// exactly ONE category page at a time. The `if (!open) return null` seeds the
// category on a close→open cycle; `requestSeq` (P69h §5.4) covers the case it
// cannot — a deep link that arrives while Settings is already open.

import { GitConfigScopeProvider } from './settings/GitConfigScope';
import { SettingsProvider } from './settings/SettingsProvider';
import { SettingsShell } from './settings/SettingsShell';
import {
  useSettingsPanelAdapter,
  type SettingsPanelProps,
} from './settings/useSettingsPanelAdapter';

export type { SettingsPanelProps };

export function SettingsPanel(props: SettingsPanelProps) {
  // Hooks run unconditionally (before the `open` early-return below).
  const { values, actions } = useSettingsPanelAdapter(props);
  const { open, onClose, initialCategory, requestSeq } = props;

  if (!open) return null;

  return (
    <SettingsProvider values={values} actions={actions}>
      {/* The Git-config scope is shared by the pane HEADER (the switch) and the
          pane BODY (the form), so its provider sits above both. */}
      <GitConfigScopeProvider>
        <SettingsShell
          initialCategory={initialCategory}
          requestSeq={requestSeq}
          onClose={onClose}
        />
      </GitConfigScopeProvider>
    </SettingsProvider>
  );
}
