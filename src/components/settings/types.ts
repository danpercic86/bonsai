/**
 * P69 §4.1 — the Settings catalog types.
 *
 * Deliberately React-free and IPC-free beyond the `UiSettings` shapes it patches,
 * so `settingsCatalog.ts` stays a pure-data module that unit-tests without a DOM
 * (and so `src/ipc/types.ts`, which sits exactly at its size baseline, does not
 * have to grow).
 */
import type { UiSettings, UiSettingsPatch } from '../../ipc/types';

export type SettingsCategoryId =
  | 'general'
  | 'appearance'
  | 'graph'
  | 'ai'
  | 'identities'
  | 'git-config'
  | 'about';

/**
 * A rail entry.
 *
 * §4.1 lists a `Page: React.ComponentType` field and then resolves it: the page
 * components are zipped in by `SettingsPanel` from a `CATEGORY_PAGES` map, so the
 * catalog itself stays React-free. This type therefore has NO `Page`.
 */
export interface SettingsCategory {
  id: SettingsCategoryId;
  /** Rail label, pane title, and search result-group header. One string, three uses. */
  label: string;
  /** Pane subtitle — states the scope once (UI §1.1). */
  subtitle: string;
  /** Hueless rail pill. Only 'git-config' sets it. */
  pill?: 'repo';
  /** Hairline divider rendered ABOVE this rail item. */
  dividerBefore?: boolean;
}

/** `${category}.${slug}`, kebab-case slug. Enforced by the catalog test. */
export type SettingsRowId = string;

export type SettingsControlKind =
  | 'switch'
  | 'segmented'
  | 'radiogroup'
  | 'numberSlider'
  | 'text'
  /** A row whose value is displayed but not editable (`about.version`, the MCP URL). */
  | 'readonly'
  | 'button'
  /**
   * Amendment A (AM-2): an aggregate row standing for a dynamically-populated
   * block. Stamped on a `role="group"` element named by its heading via
   * `aria-labelledby`. It has NO `[data-setting-control]`; its children are
   * runtime-generated and are not individually catalogued (AM-4b blindness #2).
   */
  | 'group';

/**
 * A row that is not always rendered. Exactly these ids may be missing from a
 * fixture's render, and only when the matching predicate says so (AM-4a).
 */
export type SettingsRowRequirement =
  | 'repo'
  | 'aiActive'
  | 'mcpRunning'
  | 'mcpStopped'
  /**
   * Amendment A (AM-3): folded into the union permanently. The Identities pane
   * renders one card per profile, so with `profiles: []` the four profile fields
   * plus Apply and Delete are absent. `Add identity` is unconditional and
   * correctly carries nothing.
   */
  | 'profile';

/**
 * Amendment A (AM-1): this row is rendered once per item of a runtime
 * collection; the guard dedupes it and checks the instance set against that
 * collection. Instance identity is `(data-setting-id, data-profile-id)` —
 * `SettingsRowId` still identifies the ROW, not the instance.
 */
export type SettingsRowRepeat = 'perProfile';

/**
 * The live values a reset descriptor reads.
 *
 * An alias of `UiSettings` today: every resettable row is backed by a persisted
 * UI setting. It is a distinct name because the rows that are NOT so backed
 * (Git config keys, MCP server state) simply carry no `reset`, and if a future
 * row needs a wider value bag this is the type that widens — not `UiSettings`.
 */
export type PersistedSettingsValues = UiSettings;

export interface SettingsRowReset {
  /** Shown in the ↺ title: 'Reset to default (28)'. Never empty. */
  defaultLabel: string;
  isDefault(current: PersistedSettingsValues, defaults: UiSettings): boolean;
  /** Whole-struct fields MERGE, e.g. `{ graph: { ...current.graph, rowHeight: d.graph.rowHeight } }`. */
  patch(current: PersistedSettingsValues, defaults: UiSettings): UiSettingsPatch;
}

export interface SettingsIndexEntry {
  id: SettingsRowId;
  category: SettingsCategoryId;
  /** Group title as rendered (uppercase styling is CSS; store it in sentence case). */
  group: string;
  /** MUST equal the rendered control's accessible name. Search matches on this. */
  label: string;
  /** The row's help line. Search matches on this. */
  help?: string;
  /** Never displayed. Lowercase, space-separated. UI §3.4 supplies the minimum set. */
  keywords?: string;
  control: SettingsControlKind;
  requires?: SettingsRowRequirement;
  /** Amendment A (AM-1). Absent ⇒ exactly one instance in the DOM. */
  repeats?: SettingsRowRepeat;
  /** Absent ⇒ no ↺ for this row. */
  reset?: SettingsRowReset;
}
