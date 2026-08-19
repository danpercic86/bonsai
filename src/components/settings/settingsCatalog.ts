/**
 * P69 §4 — the Settings catalog: the seven rail categories, and one entry per
 * settings ROW in UI §1.3 order.
 *
 * PURE DATA (the `workspaceMenus.ts` precedent): no React, no IPC calls, no DOM.
 * It is the single source of truth for what Settings contains, and
 * `settingsCatalog.test.ts` — plus the per-category DOM guard that goes live as
 * each category is re-skinned — keeps it honest: a control that exists on a page
 * but not here is unsearchable, and an entry whose row was deleted or renamed
 * offers the user a dead search result.
 *
 * `label` MUST equal the rendered control's accessible name. Where UI §1.3
 * relabels a control ("Interval" → "Fetch every"), the NEW label is stored here:
 * the DOM half of the guard only attaches to a category once it has been
 * re-skinned, so the catalog leads and the pages follow.
 *
 * The rows themselves live in `catalog/*.ts`, one module per category group, so
 * no single file grows past the size limit and a category can be edited without
 * reading the other six.
 */
import { ABOUT_ENTRIES } from './catalog/about';
import { AI_ENTRIES } from './catalog/ai';
import { APPEARANCE_ENTRIES } from './catalog/appearance';
import { GENERAL_ENTRIES } from './catalog/general';
import { GRAPH_ENTRIES } from './catalog/graph';
import { GIT_CONFIG_ENTRIES, IDENTITY_ENTRIES } from './catalog/repo';
import type {
  SettingsCategory,
  SettingsCategoryId,
  SettingsIndexEntry,
  SettingsRowId,
} from './types';

/** Rail order (UI §1.1). `git-config` is fenced by hairlines and carries the only pill. */
export const SETTINGS_CATEGORIES: readonly SettingsCategory[] = [
  {
    id: 'general',
    label: 'General',
    subtitle:
      'Background activity and the external tools Bonsai launches. Applies to every repository.',
  },
  {
    id: 'appearance',
    label: 'Appearance',
    subtitle: 'How Bonsai itself looks. Applies to every repository.',
  },
  {
    id: 'graph',
    label: 'Commit graph',
    subtitle: 'How the history canvas is drawn. Applies to every repository.',
  },
  {
    id: 'ai',
    label: 'AI',
    subtitle: 'AI assistance, run limits, and the local MCP server. Applies to every repository.',
  },
  {
    id: 'identities',
    label: 'Identities',
    subtitle: 'Saved name/email pairs. Switching happens in the header; edit the list here.',
  },
  {
    id: 'git-config',
    label: 'Git config',
    subtitle: 'Raw Git configuration for the open repository, or your global file.',
    pill: 'repo',
    dividerBefore: true,
  },
  {
    id: 'about',
    label: 'About',
    subtitle: 'Version, updates, and the welcome tour.',
    dividerBefore: true,
  },
];

/** Every row, in UI §1.3 rail order. */
export const SETTINGS_INDEX: readonly SettingsIndexEntry[] = [
  ...GENERAL_ENTRIES,
  ...APPEARANCE_ENTRIES,
  ...GRAPH_ENTRIES,
  ...AI_ENTRIES,
  ...IDENTITY_ENTRIES,
  ...GIT_CONFIG_ENTRIES,
  ...ABOUT_ENTRIES,
];

const BY_ID = new Map<SettingsRowId, SettingsIndexEntry>(SETTINGS_INDEX.map((e) => [e.id, e]));

export function findSettingsRow(id: SettingsRowId): SettingsIndexEntry | undefined {
  return BY_ID.get(id);
}

/** AND over whitespace-split terms, case-insensitive substring over label+help+keywords. */
export function searchSettings(query: string): readonly SettingsIndexEntry[] {
  const terms = query
    .toLowerCase()
    .split(/\s+/)
    .filter((t) => t !== '');
  if (terms.length === 0) return [];
  return SETTINGS_INDEX.filter((e) => {
    const hay = `${e.label} ${e.help ?? ''} ${e.keywords ?? ''}`.toLowerCase();
    return terms.every((t) => hay.includes(t));
  });
}

/** DOM id of a rail tab — the `aria-labelledby` target the pane points at. */
export function settingsTabId(id: SettingsCategoryId): string {
  return `settings-tab-${id}`;
}

/** Id of the row's visible label element — the `aria-labelledby` target for a
 *  control that cannot be wired with `<label for>` (radiogroups, segmented). */
export function settingsRowLabelId(id: SettingsRowId): string {
  return `${id}-label`;
}

/** Id of the row's help paragraph — the `aria-describedby` target (UI §5.1). */
export function settingsRowHelpId(id: SettingsRowId): string {
  return `${id}-help`;
}

/** '28' | 'On' | 'Off' | 'auto-detect' | 'Author' — the value named in the ↺ title. */
export function formatDefaultLabel(entry: SettingsIndexEntry): string {
  return entry.reset?.defaultLabel ?? '';
}
