/** P69 §4 — General category rows (UI §1.3 #5–#9, #61).
 *
 *  P112 §5.1: UI §1.3 #32–#33 ("Terminal command" / "Editor command") are NOT
 *  here any more. They were free-text program fields; their replacements are
 *  catalog ids, so the row comes back as a detected-tool picker in sub-increment
 *  4 rather than as a text box that would accept a value the backend discards.
 *  `settingsCatalogRows.test.ts` carries them as RETIRED_ROWS until then. */
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
];
