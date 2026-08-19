/**
 * P69 §4 — Appearance category rows (UI §1.3 #22–#25).
 *
 * No `reset` anywhere: a segmented control's "default" is one of two visible
 * choices, so a ↺ next to it says nothing the user cannot already see (§3.4).
 */
import type { SettingsIndexEntry } from '../types';

export const APPEARANCE_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'appearance.theme',
    category: 'appearance',
    group: 'Appearance',
    label: 'Theme',
    help: 'Dark or light chrome.',
    keywords: 'dark light appearance colour color',
    control: 'segmented',
  },
  {
    id: 'appearance.file-lists',
    category: 'appearance',
    group: 'Appearance',
    label: 'File lists',
    help: 'Show changed files as a folder tree or as one flat list.',
    keywords: 'tree flat folders nesting',
    control: 'segmented',
  },
  {
    id: 'appearance.panel-density',
    category: 'appearance',
    group: 'Appearance',
    label: 'Panel density',
    help: 'Row spacing in the right panel. Separate from the graph setting of the same name.',
    keywords: 'cozy compact spacing right panel',
    control: 'segmented',
  },
];
