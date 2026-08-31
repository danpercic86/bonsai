/**
 * P69 §4 — the two repository-facing categories: Identities (UI §1.3 #34–#40)
 * and Git config (#26–#31).
 *
 * Nothing here has a `reset`: a person's name and a repository's config keys have
 * no meaningful default (UI §5.7).
 */
import type { SettingsIndexEntry } from '../types';

export const IDENTITY_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'identities.profile-label',
    category: 'identities',
    group: 'Identities',
    label: 'Label',
    help: 'What you call this identity in the list.',
    keywords: 'profile nickname title',
    control: 'text',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.profile-name',
    category: 'identities',
    group: 'Identities',
    label: 'user.name',
    help: 'The name this identity writes into commits.',
    keywords: 'identity author committer whoami',
    control: 'text',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.profile-email',
    category: 'identities',
    group: 'Identities',
    label: 'user.email',
    help: 'The email this identity writes into commits.',
    keywords: 'identity author committer whoami',
    control: 'text',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.profile-signing-key',
    category: 'identities',
    group: 'Identities',
    label: 'signing key',
    help: 'Optional key id used when signing commits with this identity.',
    keywords: 'gpg ssh sign key',
    control: 'text',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.profile-color',
    category: 'identities',
    group: 'Identities',
    label: 'Color',
    help: 'Shown as a dot on this identity everywhere it appears, so profiles with the same name stay easy to tell apart.',
    keywords: 'colour hue swatch dot distinguish',
    control: 'color',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.apply',
    category: 'identities',
    group: 'Identities',
    label: 'Use in this repository',
    /* P69k: no `help` — the two action buttons share one cell with no help slot
       (UI §4.6), so a help string here would be text search could match and the
       user could never see. Its vocabulary moved into `keywords`, the same trade
       P69j made for the four AI rows whose note is stateful. */
    keywords: 'apply switch active repo write local git config identity use',
    control: 'button',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.delete',
    category: 'identities',
    group: 'Identities',
    label: 'Delete',
    /* P69k: no `help`, same reason as `identities.apply` above. */
    keywords: 'remove profile identity delete saved confirm',
    control: 'button',
    requires: 'profile',
    repeats: 'perProfile',
  },
  {
    id: 'identities.add',
    category: 'identities',
    group: 'Identities',
    label: 'Add identity',
    help: 'Create a new saved name/email pair.',
    keywords: 'new profile create identity',
    control: 'button',
  },
];

export const GIT_CONFIG_ENTRIES: readonly SettingsIndexEntry[] = [
  {
    id: 'git-config.run-hooks',
    category: 'git-config',
    group: 'Hooks',
    label: 'Run git hooks for this repository',
    help: 'Run pre-commit and friends when Bonsai commits.',
    keywords: 'husky pre-commit hooks',
    control: 'switch',
    requires: 'repo',
  },
  {
    // UI §8: "Level" is libgit2's word, not the user's. The row lives in the
    // pane HEADER (UI §1.1), which is why it is the one catalogued row that is
    // not inside a `.settings-group` — the coverage guard exempts exactly it.
    id: 'git-config.scope',
    category: 'git-config',
    group: 'Scope',
    label: 'Scope',
    help: 'Whether you are editing this repository’s config or your global one.',
    keywords: 'local global scope repository gitconfig',
    control: 'segmented',
    requires: 'repo',
  },
  {
    id: 'git-config.user-name',
    category: 'git-config',
    group: 'Identity',
    label: 'user.name',
    help: 'The name written into commits made in this repository.',
    keywords: 'identity author committer whoami',
    control: 'text',
    requires: 'repo',
  },
  {
    id: 'git-config.user-email',
    category: 'git-config',
    group: 'Identity',
    label: 'user.email',
    help: 'The email written into commits made in this repository.',
    keywords: 'identity author committer whoami',
    control: 'text',
    requires: 'repo',
  },
  {
    // Aggregate row: the curated Behaviour keys are fetched per repository, so the
    // catalog names the block, not each key. Amendment A (AM-2): `group`, stamped
    // on a `role="group"` element whose <h4> must equal `label` byte-for-byte.
    id: 'git-config.behaviour',
    category: 'git-config',
    group: 'Advanced',
    label: 'Behaviour',
    help: 'Curated Git keys such as pull.rebase and core.autocrlf.',
    keywords: 'pull rebase autocrlf curated preset',
    control: 'group',
    requires: 'repo',
  },
  {
    // Aggregate row, like `git-config.behaviour` — the entries are user-created.
    id: 'git-config.custom-keys',
    category: 'git-config',
    group: 'Advanced',
    label: 'Custom keys',
    help: 'Any other section.key = value entry, added or removed by hand.',
    keywords: 'custom entry add remove raw',
    control: 'group',
    requires: 'repo',
  },
];
