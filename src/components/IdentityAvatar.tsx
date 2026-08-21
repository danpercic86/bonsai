// P69i — UI §4.2 / ui-reference §12.6: the 22px identity circle.
//
// The circle shows INITIALS, not a person glyph: the whole point of the control
// is *which* identity commits here, and a generic pictograph answers a different
// question (and the toolbar already carries 🤖 and 📊 — a third would read as
// decoration). The unset state is carried by the `?` GLYPH and by the accessible
// name; the `--warning` ring is a second, redundant signal, never the only one.
//
// The state machine and every string live in `identityCopy.ts` — this file is a
// component and nothing else.

import type { IdentityProfile } from '../ipc';
import type { EffectiveIdentity } from '../hooks/useEffectiveIdentity';
import { identityInitials, identityState } from './identityCopy';
import { resolveProfileColor } from './identityProfileColor';

export function IdentityAvatar({
  identity,
  matchedProfile,
}: {
  identity: EffectiveIdentity;
  /** The saved profile matching the EFFECTIVE identity, if any (§5.2). */
  matchedProfile: IdentityProfile | null;
}) {
  const state = identityState(identity, matchedProfile);
  const content =
    state === 'loading'
      ? '·'
      : state === 'unset' || state === 'unreadable'
        ? '?'
        : identityInitials(identity.name);
  // P82 (UI §3.1): a 2px hue ring when a non-neutral profile is matched. The
  // unset `?`+--warning ring keeps priority — an unset identity has no profile,
  // so `matchedProfile` is null there and no hue ring is emitted.
  const hue =
    matchedProfile !== null ? resolveProfileColor(matchedProfile) : 'neutral';
  const profileColor = hue !== 'neutral' ? hue : undefined;
  return (
    <span
      className="identity-avatar"
      data-identity-state={state}
      data-profile-color={profileColor}
      aria-hidden="true"
    >
      {content}
    </span>
  );
}
