/** P69 §4 — General category rows (UI §1.3 #5–#9, #32–#33, #61, #79).
 *
 *  P112 §8: #32–#33 are back, as the detected-tool PICKER — not the free-text
 *  program fields they were. The value is a lookup key (a catalog id, `''` or
 *  `'custom'`), and `general.rescan-tools` (#79) is the one refresh control for
 *  the scan both rows read.
 *
 *  All three carry NO `help`, deliberately (§8 / ui-reference §12.2's "never
 *  both"): every one of them varies with the scan, so their explanation is a
 *  stateful `.settings-row-note` the section renders. Their vocabulary therefore
 *  has to live in `keywords` — the only place search can see it. */
import type { SettingsIndexEntry } from '../types';
import { resetField, resetKey } from './reset';

export const GENERAL_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'general.auto-fetch',
    category: 'general',
    group: 'Background activity',
    label: 'Auto-fetch from remotes',
    help: 'Fetch from every remote in the background so ahead/behind counts stay honest.',
    keywords: 'background poll origin sync automatic',
    control: 'switch',
    reset: resetField('autoFetch', 'enabled', 'Off'),
  },
  {
    id: 'general.fetch-interval',
    category: 'general',
    group: 'Background activity',
    label: 'Fetch every',
    help: 'How often the background fetch runs.',
    keywords: 'auto-fetch minutes interval schedule',
    control: 'numberSlider',
    reset: resetField('autoFetch', 'intervalMinutes', '5'),
  },
  {
    id: 'general.auto-refresh',
    category: 'general',
    group: 'Background activity',
    label: 'Refresh status automatically',
    help: 'Re-read working-directory status and repository health on a timer.',
    keywords: 'health rescan watcher poll periodic',
    control: 'switch',
    reset: resetField('healthRefresh', 'enabled', 'Off'),
  },
  {
    id: 'general.refresh-interval',
    category: 'general',
    group: 'Background activity',
    label: 'Refresh every',
    help: 'How often the periodic status and health refresh runs.',
    keywords: 'health minutes interval schedule',
    control: 'numberSlider',
    reset: resetField('healthRefresh', 'intervalMinutes', '30'),
  },
  {
    id: 'general.primary-commit-action',
    category: 'general',
    group: 'Committing',
    label: 'Primary commit action',
    help: 'Which button is emphasized at the bottom of the Working tab. The other stays available beside it.',
    keywords: 'commit push button default primary emphasize',
    control: 'segmented',
    reset: resetKey('primaryCommitAction', 'Commit'),
  },
  {
    id: 'general.terminal-tool',
    category: 'general',
    group: 'External tools',
    label: 'Terminal',
    keywords:
      'shell console command external open in terminal program picker browse detect installed',
    control: 'combobox',
    reset: resetKey('terminalTool', 'Auto-detect'),
  },
  {
    id: 'general.editor-tool',
    category: 'general',
    group: 'External tools',
    label: 'Editor',
    keywords: 'ide vscode code editor command external open in program picker browse detect installed',
    control: 'combobox',
    reset: resetKey('editorTool', 'Auto-detect'),
  },
  {
    /** §7: a catalogued BUTTON row — `label` is the button text and its
     *  accessible name, `rowLabel="Detected tools"` is the visible row title.
     *  No `reset`: there is nothing to reset, so the 24px column stays empty. */
    id: 'general.rescan-tools',
    category: 'general',
    group: 'External tools',
    label: 'Rescan',
    keywords: 'detect scan refresh installed terminals editors external tools find again',
    control: 'button',
  },
];
