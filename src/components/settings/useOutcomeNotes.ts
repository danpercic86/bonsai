// P113 §5/§8 — the slot bookkeeping both Settings surfaces need.
//
// Centralising it is what makes "one live region per section" structural rather
// than per-call-site discipline: `report()` writes the note AND the section's
// single announcement from one source, so the two cannot drift and one event
// cannot be uttered twice.

import { useCallback, useMemo, useRef, useState } from 'react';
import { flushSync } from 'react-dom';

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
   *  It clears `key`'s note AND, **unconditionally**, the section's one
   *  announcement (§8.2). A section has exactly one announcement, so "clear the
   *  announcement" is the only thing `begin` can mean — and an operation that
   *  succeeds without reporting therefore leaves the announcer EMPTY, not still
   *  voicing some other key's outcome.
   *
   *  The clear is not cosmetic: a live region fires on a text CHANGE, so writing
   *  the string already in it is silence. Usually `begin` and `report` are
   *  separated by the operation's `await` and land in different commits, which
   *  is what makes the transition `'' → text`. Callers that issue both in ONE
   *  commit are covered by `report`'s flush, not by call-site discipline.
   *
   *  The ONLY permitted skip of a clear keys on **text identity** and lives in
   *  `report` (AC17). `begin` cannot skip: the outcome text is not known at
   *  operation start. Never on **key** identity — two keys routinely carry
   *  byte-identical text, so key identity is no proxy for text identity.
   *
   *  Accepted cost, deliberately (§8.2): Accounts has no global busy gate (Dev's
   *  `anyBusy` means two Dev actions cannot overlap), so an unconditional clear
   *  can truncate an in-flight utterance about another host. That window is
   *  roughly one utterance, while silence is unbounded and permanent — §8.1's
   *  MUST wins. */
  begin(key: string): void;
  /** Set `key`'s note and the section's announcement. `announceText` defaults to
   *  `text`; pass it only for a sanctioned "not actionable by voice" trim.
   *
   *  AC17 invariant: **every reported outcome produces a text change in the
   *  announcer** — whatever its key, and whether or not the previous
   *  announcement carried the same string. */
  report(key: string, tone: SettingsOutcome['tone'], text: string, announceText?: string): void;
}

export function useOutcomeNotes(): OutcomeNotes {
  const [notes, setNotes] = useState<ReadonlyMap<string, SettingsOutcome>>(NO_NOTES);
  const [announce, setAnnounce] = useState('');
  /** The text of the last utterance `report` wrote — the identity the AC17
   *  invariant is keyed on, and the only identity any skipped clear may key on
   *  (§8.2:507-509, which requires a skip it can PROVE).
   *
   *  Why this proves it: the announcer goes non-empty ONLY through `report`,
   *  which records the text here, so a non-empty announcer showing `X` implies
   *  `ref === X`. Therefore `ref !== next` proves the next write differs from
   *  what is displayed, and `ref === next` forces the pre-clear. `begin` does
   *  not reset it — see the comment there.
   *
   *  Key identity is no proxy for text identity, which is why clearing by key
   *  was WRONG: two different keys routinely carry byte-identical text (no
   *  `claude` on PATH fails both MCP register rows with the same sentence),
   *  React bails on the identical state, and the second outcome is SILENT — the
   *  §8.1 defect, one axis over and unbounded, since a last-announced-key ref
   *  never expires.
   *
   *  A ref, not state: read and written inside these callbacks only, and state
   *  here would churn `begin`/`report` identity, which the hook's `useCallback`
   *  contract forbids. Compared in `report`, the only place that can see BOTH
   *  the displayed text and the new one. */
  const announcedTextRef = useRef('');

  const begin = useCallback((key: string) => {
    setNotes((prev) => {
      if (!prev.has(key)) return prev;
      const next = new Map(prev);
      next.delete(key);
      return next;
    });
    // §8.2: the section has ONE announcement, so clearing it is unconditional.
    // Scoping this to `key` is the WITHDRAWN rule — it silenced a second row
    // carrying identical text, forever. Do not "fix" by re-scoping to `key`.
    //
    // `announcedTextRef` is deliberately NOT reset here, and that is measured,
    // not taste: this clear is not guaranteed to reach the DOM in an earlier
    // commit than `report`'s write. `SettingsAccountsSection.tsx:226-229`
    // issues `begin` and `report` in ONE synchronous block, so React coalesces
    // `''` and the text into a single render and the announcer never blanks —
    // measured as ZERO announcer mutations for a repeated add. Resetting the ref
    // would tell `report` the display is `''` while it still shows the text, so
    // `report` would skip the flush that is then the only thing left to force
    // the change. Leaving it alone costs one no-op `flushSync` when this clear
    // DID land, and does the real work when it did not.
    setAnnounce('');
  }, []);

  const report = useCallback(
    (key: string, tone: SettingsOutcome['tone'], text: string, announceText?: string) => {
      const next = announceText ?? text;
      // AC17 — a live region fires on a text CHANGE, so re-announcing the string
      // that is already in it is silence. `flushSync` commits the `''` before
      // this render, making `setAnnounce(next)` below a real change: the observed
      // sequence is `'' → T → '' → T`, not `'' → T`. It is a no-op when `begin`'s
      // clear already committed, and the whole transition when it did not (see
      // `begin`). Every caller runs in a
      // promise continuation (never render, never a lifecycle), which is what
      // makes a synchronous flush legal here.
      if (announcedTextRef.current === next) {
        flushSync(() => setAnnounce(''));
      }
      setNotes((prev) => new Map(prev).set(key, { tone, text }));
      announcedTextRef.current = next;
      setAnnounce(next);
    },
    [],
  );

  // Stable identities: `begin`/`report` take the place of the old toast callback
  // inside `useCallback` dependency arrays, so they must not change every render.
  return useMemo(() => ({ notes, announce, begin, report }), [notes, announce, begin, report]);
}
