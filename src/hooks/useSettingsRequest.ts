// P69h §5.3/§5.4 — the Settings open state and its deep-link request.
//
// Extracted from `App.tsx` (which sits at its size ratchet) because the three
// pieces belong together: whether the panel is open, which category it was asked
// for, and a monotonic sequence number.
//
// The sequence number is the whole point. `SettingsPanel` unmounts the shell
// while closed, so a close→open cycle re-seeds the category by construction — but
// a deep link that arrives while Settings is ALREADY open produces no mount and
// would otherwise leave the user staring at the wrong pane. Every open path bumps
// `seq`, including the plain ⚙ click (which names no category and therefore only
// clears state), so the shell can key its re-seed on the request rather than on
// an `open` transition.

import { useCallback, useMemo, useState } from 'react';

import type { SettingsCategoryId } from '../components/settings/types';

export interface SettingsRequest {
  /** `null` ⇒ "wherever it was" (the plain ⚙ / palette open). */
  category: SettingsCategoryId | null;
  /** P40b: 'identity' scrolls + focuses the Git-config Identity sub-section. */
  focus: 'identity' | null;
  seq: number;
}

export interface SettingsRequestState {
  open: boolean;
  request: SettingsRequest;
  /** Open (or re-target) Settings. Always bumps `seq`. */
  openAt(category: SettingsCategoryId | null, focus?: 'identity' | null): void;
  /** The `configMissing` commit-error linkage (App.tsx's old
   *  `openIdentitySettings`), verbatim in behaviour. */
  openIdentity(): void;
  /** Close and drop the request, KEEPING `seq` — the counter is monotonic. */
  close(): void;
}

const CLOSED: SettingsRequest = { category: null, focus: null, seq: 0 };

export function useSettingsRequest(): SettingsRequestState {
  const [open, setOpen] = useState(false);
  const [request, setRequest] = useState<SettingsRequest>(CLOSED);

  const openAt = useCallback(
    (category: SettingsCategoryId | null, focus: 'identity' | null = null): void => {
      setRequest((r) => ({ category, focus, seq: r.seq + 1 }));
      setOpen(true);
    },
    [],
  );

  const openIdentity = useCallback(() => openAt('git-config', 'identity'), [openAt]);

  const close = useCallback((): void => {
    setOpen(false);
    setRequest((r) => ({ category: null, focus: null, seq: r.seq }));
  }, []);

  return useMemo(
    () => ({ open, request, openAt, openIdentity, close }),
    [open, request, openAt, openIdentity, close],
  );
}
