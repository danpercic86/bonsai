/**
 * P69 §4 — About category rows (UI §1.3 #1–#4).
 *
 * Only the launch-time update check is a persisted knob with a default; the
 * version line, the manual check and the tour are read-only/button rows (UI §5.7).
 */
import type { SettingsIndexEntry } from '../types';
import { resetKey } from './reset';

export const ABOUT_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'about.version',
    category: 'about',
    group: 'Version',
    label: 'Current version',
    help: 'The Bonsai build you are running.',
    keywords: 'build number release',
    control: 'readonly',
  },
  {
    id: 'about.check-updates',
    category: 'about',
    group: 'Version',
    label: 'Check for updates',
    help: 'Ask the update server whether a newer build exists.',
    keywords: 'updater download release',
    control: 'button',
  },
  {
    id: 'about.auto-check-updates',
    category: 'about',
    group: 'Version',
    label: 'Automatically check for updates on launch',
    help: 'Check once at startup instead of only when you ask.',
    keywords: 'updater version release auto',
    control: 'switch',
    reset: resetKey('autoCheckUpdates', 'Off'),
  },
  {
    // UI §1.3 #1 calls the ROW "Welcome tour" and the button "Show tour"; `label`
    // must equal the control's accessible name, so it is the button text, and
    // "welcome" rides along in the keywords.
    id: 'about.welcome-tour',
    category: 'about',
    group: 'Help',
    label: 'Show tour',
    help: 'Re-open the first-run walkthrough.',
    keywords: 'welcome onboarding getting started intro help tour',
    control: 'button',
  },
];
