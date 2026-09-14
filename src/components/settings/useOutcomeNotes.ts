// P113 §5/§8 — the slot bookkeeping both Settings surfaces need.
//
// Centralising it is what makes "one live region per section" structural rather
// than per-call-site discipline: `report()` writes the note AND the section's
// single announcement from one source, so the two cannot drift and one event
// cannot be uttered twice.

import { useCallback, useMemo, useState } from 'react';

import type { SettingsOutcome } from './SettingsOutcomeNote';

const NO_NOTES: ReadonlyMap<string, SettingsOutcome> = new Map();

export interface OutcomeNotes {
  /** Slot key → its newest outcome. Keys are row ids (`dev.logs`) or hosts. */
  notes: ReadonlyMap<string, SettingsOutcome>;
  /** The section's ONE announcement. Render it in a single always-mounted
   *  `<p className="sr-only" role="status" aria-live="polite">`. */
  announce: string;
  /** Call at the START of an operation that reports into `key` — not when a
   *  confirm dialog opens, so cancelling leaves the previous outcome intact.
   *
   *  It also clears the announcer, and that is not cosmetic: a live region only
   *  fires on a text CHANGE. Export twice and the announcer was set to the same
   *  string twice, React skipped the identical state, and the second outcome was
   *  SILENT. `begin` and `report` are separated by the operation's `await`, so
   *  they land in different commits and the transition is always `'' → text`. */
  begin(key: string): void;
  /** Set `key`'s note and the section's announcement. `announceText` defaults to
   *  `text`; pass it only for a sanctioned "not actionable by voice" trim. */
  report(key: string, tone: SettingsOutcome['tone'], text: string, announceText?: string): void;
}

export function useOutcomeNotes(): OutcomeNotes {
  const [notes, setNotes] = useState<ReadonlyMap<string, SettingsOutcome>>(NO_NOTES);
  const [announce, setAnnounce] = useState('');

  const begin = useCallback((key: string) => {
    setNotes((prev) => {
      if (!prev.has(key)) return prev;
      const next = new Map(prev);
      next.delete(key);
      return next;
    });
    setAnnounce('');
  }, []);

  const report = useCallback(
    (key: string, tone: SettingsOutcome['tone'], text: string, announceText?: string) => {
      setNotes((prev) => new Map(prev).set(key, { tone, text }));
      setAnnounce(announceText ?? text);
    },
    [],
  );

  // Stable identities: `begin`/`report` take the place of the old toast callback
  // inside `useCallback` dependency arrays, so they must not change every render.
  return useMemo(() => ({ notes, announce, begin, report }), [notes, announce, begin, report]);
}
