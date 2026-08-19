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
export type SettingsStructKey = 'graph' | 'autoFetch' | 'healthRefresh';

/**
 * The `UiSettings` keys holding a primitive.
 *
 * `resetKey` is deliberately restricted to these: an object- or array-valued key
 * (`paneWidths`, `profiles`) would compare by REFERENCE in `isDefault` — always
 * unequal, so the ↺ would never hide — and would alias the shared defaults object
 * straight into a patch. Whole-struct fields go through `resetField` instead.
 */
export type ScalarUiSettingsKey = {
  [K in keyof UiSettings]: UiSettings[K] extends string | number | boolean ? K : never;
}[keyof UiSettings];

/** Reset for a top-level scalar key: compare and patch that key alone. */
export function resetKey<K extends ScalarUiSettingsKey>(
  key: K,
  defaultLabel: string,
): SettingsRowReset {
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
