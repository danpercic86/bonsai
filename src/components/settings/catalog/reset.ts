/**
 * P69 §4.1 — reset-descriptor factories for the settings catalog.
 *
 * Two shapes only: a top-level `UiSettings` key, and one field of a whole-struct
 * key. The latter MUST merge — patching `{ graph: { rowHeight } }` without
 * spreading the current struct would silently wipe every other graph preference,
 * which is the one way a "reset this row" button can destroy unrelated settings.
 */
import type { UiSettings, UiSettingsPatch } from '../../../ipc/types';
import type { SettingsRowReset } from '../types';

/** The whole-struct `UiSettings` keys a row can reset a single field of. */
export type SettingsStructKey = 'graph' | 'autoFetch' | 'healthRefresh' | 'dev';

/**
 * The `UiSettings` keys holding a primitive.
 *
 * `resetKey` is deliberately restricted to these: an object- or array-valued key
 * (`paneWidths`, `profiles`) would compare by REFERENCE in `isDefault` — always
 * unequal, so the ↺ would never hide — and would alias the shared defaults object
 * straight into a patch. Whole-struct fields go through `resetField` instead.
 */
export type ScalarUiSettingsKey = {
  [K in keyof UiSettings]-?: NonNullable<UiSettings[K]> extends string | number | boolean
    ? K
    : never;
}[keyof UiSettings];

/**
 * The keys whose ↺ may NOT be performed by the generic `resetRow` (P112
 * `useExternalToolScan` rule 6).
 *
 * An explicit selection for one of these has to clear that kind's owed adopt in
 * the same act, and only `useExternalToolScan.changeTool` can do that — a
 * catalog descriptor patching the key itself produces precisely the divergence
 * rule 6 exists to prevent: the browsed value comes back over the reset on the
 * next scan while the reset still reaches disk. `resetKey` therefore EXCLUDES
 * them, so `resetKey('editorTool', …)` is a compile error rather than the
 * fourth review round; `resetRouted` is the only descriptor that may name one.
 */
export type RoutedToolKey = 'terminalTool' | 'editorTool';

/** Reset for a top-level scalar key: compare and patch that key alone. */
export function resetKey<K extends Exclude<ScalarUiSettingsKey, RoutedToolKey>>(
  key: K,
  defaultLabel: string,
): SettingsRowReset {
  return descriptorFor(key, defaultLabel);
}

/**
 * Reset for a row whose ↺ WRITE belongs to its container, not to `resetRow`.
 *
 * The descriptor still DECLARES the key and its default — that is what names
 * the ↺ ("Reset to default (Auto-detect)"), what `settingsCatalog.test.ts`
 * checks the patch of, and what `settingsCatalog.coverage.test.tsx` compares the
 * ↺'s visibility against. `routed` says only who performs it: `resetRow`
 * refuses, `SettingsRow` renders no ↺ from the catalog for it, and the row
 * must pass `SettingsRow`'s `reset` override. A container that forgets is
 * therefore a MISSING ↺ — caught by the coverage guard — never a write that
 * skips the rule.
 */
export function resetRouted(key: RoutedToolKey, defaultLabel: string): SettingsRowReset {
  return { ...descriptorFor(key, defaultLabel), routed: true };
}

/** The comparison and the patch both factories share — one place, so a routed
 *  row's declared default cannot drift from a generic one's. */
function descriptorFor(key: ScalarUiSettingsKey, defaultLabel: string): SettingsRowReset {
  return {
    defaultLabel,
    isDefault: (c, d) => c[key] === d[key],
    patch: (_c, d) => ({ [key]: d[key] }) as UiSettingsPatch,
  };
}

/** Reset for one field of a whole-struct key — MERGES the current struct. */
export function resetField<P extends SettingsStructKey, K extends keyof UiSettings[P]>(
  parent: P,
  field: K,
  defaultLabel: string,
): SettingsRowReset {
  return {
    defaultLabel,
    isDefault: (c, d) => c[parent][field] === d[parent][field],
    patch: (c, d) => ({ [parent]: { ...c[parent], [field]: d[parent][field] } }) as UiSettingsPatch,
  };
}
