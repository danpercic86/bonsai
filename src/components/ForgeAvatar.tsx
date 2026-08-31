// P79 (UI §0): a forge account avatar with a monogram fallback. Reuses the
// `.identity-avatar` visual under `.forge-avatar` (identical rules, separate
// class). Always decorative — the login text beside it is the accessible name,
// so the avatar is `aria-hidden` / `alt=""` and never the sole carrier.
import { useState } from 'react';

export interface ForgeAvatarProps {
  /** May be null (never validated / not cache-warm) → monogram. */
  avatarUrl: string | null;
  /** Drives the monogram initial; null → `?`. */
  login: string | null;
}

/** First Unicode code point of `login`, uppercased; `?` when login is null/empty. */
function monogram(login: string | null): string {
  if (login === null) return '?';
  const cp = [...login][0];
  return cp === undefined ? '?' : cp.toUpperCase();
}

export function ForgeAvatar({ avatarUrl, login }: ForgeAvatarProps) {
  // Swap to the monogram ONCE on image error; never loop back to <img>.
  const [failed, setFailed] = useState(false);

  if (avatarUrl !== null && !failed) {
    return (
      <img
        className="forge-avatar"
        src={avatarUrl}
        alt=""
        width={22}
        height={22}
        onError={() => setFailed(true)}
      />
    );
  }
  return (
    <span className="forge-avatar" aria-hidden="true">
      {monogram(login)}
    </span>
  );
}
