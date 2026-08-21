/**
 * P79 §3.1 — the Accounts category catalog.
 *
 * The connected-account cards are runtime-generated from `forgeListAccounts` (the
 * `SettingsProfilesSection` precedent: the card is a `role="group"` carrying no
 * `data-setting-id`, so it is not individually catalogued). The one catalogued,
 * unconditional row is the "Add a token for a host" button — mirroring
 * `identities.add`.
 */
import type { SettingsIndexEntry } from '../types';

export const ACCOUNTS_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'accounts.add',
    category: 'accounts',
    group: 'Connected accounts',
    label: 'Add a token for a host',
    help: 'Store a personal access token for a forge host so Bonsai can view its pull requests.',
    keywords: 'forge github gitlab bitbucket sign in connect token account',
    control: 'button',
  },
];
