// P69i — UI §4.2/§4.8: the identity state machine and its exact strings.
//
// Pure and React-free (its own file per the fast-refresh rule): both the header
// trigger and Settings → Identities resolve "who commits here" through these,
// so the two surfaces cannot disagree on the match, the source line or the
// fallback name.

import type { IdentityProfile } from '../ipc';
import type { EffectiveIdentity } from '../hooks/useEffectiveIdentity';

/** The visual/semantic state, resolved once and used by both halves. */
export type IdentityState = 'loading' | 'matched' | 'unmatched' | 'unset' | 'unreadable';

export interface IdentityTriggerCopy {
  state: IdentityState;
  ariaLabel: string;
  /** Multi-line (`\n`) — native tooltips honour it; §4.2's three-line block. */
  title: string;
  busy: boolean;
}

/** UI §4.2: first letters of the first two whitespace-separated words of the
 *  effective `user.name`, uppercased, max 2 chars; one word ⇒ one char. Exported
 *  for its unit test (contract §5.2). */
export function identityInitials(name: string | null): string {
  if (name === null) return '';
  const words = name.trim().split(/\s+/).filter((w) => w !== '');
  if (words.length === 0) return '';
  return words
    .slice(0, 2)
    .map((w) => [...w][0] ?? '')
    .join('')
    .toUpperCase();
}

/** An identity is usable only when BOTH halves are set — a name with no email
 *  still fails `git commit`, so a half-set identity is state 3, not state 2. */
export function hasUsableIdentity(identity: EffectiveIdentity): boolean {
  return identity.name !== null && identity.email !== null;
}

/** UI §4.8: `From this repository's config` vs `From your global Git config`.
 *  'system'/'other' fold into the global wording — the distinction Bonsai can act
 *  on is "this repo" vs "everywhere else", and naming a level the user cannot
 *  edit here would be noise. */
export function identitySourceLine(identity: EffectiveIdentity): string {
  if (identity.error !== null) return 'Bonsai couldn’t read this repository’s Git config.';
  return identity.source === 'local'
    ? 'From this repository’s config'
    : 'From your global Git config';
}

export function identityState(
  identity: EffectiveIdentity,
  matchedProfile: IdentityProfile | null,
): IdentityState {
  if (identity.loading) return 'loading';
  if (identity.error !== null) return 'unreadable';
  if (!hasUsableIdentity(identity)) return 'unset';
  return matchedProfile !== null ? 'matched' : 'unmatched';
}

/** The profile whose trimmed name AND email both equal the effective identity.
 *  Shared by the menu's `checked` rows and by Identities' `in use` pill so the
 *  two surfaces can never disagree. */
export function matchProfile(
  identity: EffectiveIdentity,
  profiles: readonly IdentityProfile[],
): IdentityProfile | null {
  const name = (identity.name ?? '').trim();
  const email = (identity.email ?? '').trim();
  if (name === '' || email === '') return null;
  return (
    profiles.find((p) => p.userName.trim() === name && p.userEmail.trim() === email) ?? null
  );
}

/** The display name of a profile: label, else `user.name`, else the §4.8 fallback. */
export function profileDisplayName(profile: IdentityProfile): string {
  const label = profile.label.trim();
  if (label !== '') return label;
  const name = profile.userName.trim();
  return name !== '' ? name : 'Unnamed identity';
}

export function identityTriggerCopy(
  identity: EffectiveIdentity,
  matchedProfile: IdentityProfile | null,
): IdentityTriggerCopy {
  const state = identityState(identity, matchedProfile);
  if (state === 'loading') {
    return { state, ariaLabel: 'Reading commit identity…', title: '', busy: true };
  }
  if (state === 'unreadable') {
    // §4.2: renders as state 3, but says WHY rather than claiming the identity
    // is unset — those are different problems with different fixes.
    return {
      state,
      ariaLabel: 'Commit identity not set',
      title: 'Bonsai couldn’t read this repository’s Git config.',
      busy: false,
    };
  }
  if (state === 'unset') {
    return {
      state,
      ariaLabel: 'Commit identity not set',
      title: 'No name and email are set. Commits will fail until you set one.',
      busy: false,
    };
  }
  const who = `${identity.name ?? ''} <${identity.email ?? ''}>`;
  const source = identitySourceLine(identity);
  if (matchedProfile !== null) {
    const label = profileDisplayName(matchedProfile);
    return {
      state,
      ariaLabel: `Commit identity: ${label}`,
      title: `${label}\n${who}\n${source}`,
      busy: false,
    };
  }
  return {
    state,
    ariaLabel: `Commit identity: ${identity.name ?? ''}`,
    title: `${who}\n${source}`,
    busy: false,
  };
}
