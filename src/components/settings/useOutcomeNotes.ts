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
  /** Drop these keys' notes and NOTHING else: the announcer is neither read nor
   *  written here.
   *
   *  For a slot whose HOST row unmounted while the section stayed. The two MCP
   *  register rows render behind `running` (`SettingsMcpSection`), so a stopped
   *  server takes their home away while the note KEYS survive in this map.
   *
   *  This is NOT the withdrawn key-scoped clear (see `begin`). That rule was
   *  wrong because it scoped the ANNOUNCER to a key, which silenced a second row
   *  carrying byte-identical text. This function never touches the announcer, so
   *  it cannot silence anything — and it must not touch it: the announcer belongs
   *  to the still-mounted section and may be carrying the very outcome that
   *  CAUSED the unmount (a failed write-gate bounce emits a stopped status and
   *  rejects `set_mcp_allow_write`, which can land in one React batch), so
   *  clearing it from here would swallow the explanation. */
  discardNotes(keys: readonly string[]): void;
  /** Clear every note, the announcement, and the last-announced record — the
   *  instance goes back to its mount state.
   *
   *  §7, "Also clears on: unmount — leaving the category, or closing Settings.
   *  Reopening Settings is a clean page." `DevCategory` and
   *  `SettingsAccountsSection` get that for free by owning their notes
   *  component-locally; the MCP instance lives in `useMcpControls`, which `App`
   *  mounts for the whole app lifetime, so its host has to say it out loud.
   *
   *  Unlike `begin`, this DOES reset `announcedTextRef`. `begin`'s exception
   *  exists because a caller can issue `begin` and `report` in ONE commit, where
   *  a reset ref would make `report` skip the flush that is then the only thing
   *  forcing the text change. `reset` is a lifecycle call (mount / unmount
   *  cleanup), never coalesced with a `report`, and the announcer ELEMENT is
   *  unmounting with the section — so `''` is exactly what the next mount
   *  displays, which is what the ref is required to record. */
  reset(): void;
  /** Announce WITHOUT writing a note: for an action whose visible result is already
   *  complete in the row's own state (a landed rescan's counts, a confirmed Browse),
   *  where a second visible line would only restate it one step brighter.
   *
   *  P112 §16.4 R2. It obeys `report`'s discipline exactly — same dedup flush,
   *  same ref record — so the AC17 invariant below still holds: the proof is
   *  about WHO may write the announcer and whether they record it, not about how
   *  many such functions there are. */
  announceOnly(text: string): void;
}

export function useOutcomeNotes(): OutcomeNotes {
  const [notes, setNotes] = useState<ReadonlyMap<string, SettingsOutcome>>(NO_NOTES);
  const [announce, setAnnounce] = useState('');
  /** The text of the last utterance written to the announcer — the identity the
   *  AC17 invariant is keyed on, and the only identity any skipped clear may key
   *  on (§8.2:507-509, which requires a skip it can PROVE).
   *
   *  Why this proves it: the announcer goes non-empty ONLY through `report` and
   *  `announceOnly` (P112 §16.4 R2), and BOTH record the text here, so a
   *  non-empty announcer showing `X` implies `ref === X`. Therefore `ref !== next`
   *  proves the next write differs from what is displayed, and `ref === next`
   *  forces the pre-clear. `begin` does not reset it — see the comment there.
   *  `reset` writes the announcer too, but only `''`, and records `''` in the
   *  same call, so it cannot make the announcer non-empty and leaves the claim
   *  above intact. That CLOSES the set: exactly four functions here set
   *  `announce` — `begin`, `report`, `announceOnly`, `reset` — and all four are
   *  accounted for in this paragraph, so the proof stands against the current
   *  surface and not merely against the surface it was first written for.
   *
   *  The scope of that "ONLY" is the invariant, so it is what a new writer has to
   *  join: any future function that sets `announce` must record the text here in
   *  the same call, or the proof silently stops covering the surface.
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
   *  contract forbids. Compared in `report` and in `announceOnly` (P112 §16.4 R2)
   *  — the two places that can see BOTH the displayed text and the new one. */
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

  // Notes only — see the interface doc for why the announcer is off limits here.
  const discardNotes = useCallback((keys: readonly string[]) => {
    setNotes((prev) => {
      if (!keys.some((key) => prev.has(key))) return prev;
      const next = new Map(prev);
      for (const key of keys) next.delete(key);
      return next;
    });
  }, []);

  // Both writes bail out when the instance is already clean, so a host that
  // calls this on MOUNT as well as on unmount costs no extra render.
  const reset = useCallback(() => {
    setNotes((prev) => (prev.size === 0 ? prev : NO_NOTES));
    announcedTextRef.current = '';
    setAnnounce('');
  }, []);

  // P112 §16.4 R2 — `report` minus `setNotes`. The `''`-flush is what makes an
  // IDENTICAL string announce again (§16.8: a warm rescan finds the same tools,
  // so `3 terminals and 4 editors found.` is byte-identical and a live region
  // fires on a text CHANGE — writing it back is silence). Same AC17 shape as
  // `report`: same text, different event, still a mutation.
  const announceOnly = useCallback((text: string) => {
    if (announcedTextRef.current === text) {
      flushSync(() => setAnnounce(''));
    }
    announcedTextRef.current = text;
    setAnnounce(text);
  }, []);

  // Stable identities: `begin`/`report` take the place of the old toast callback
  // inside `useCallback` dependency arrays, so they must not change every render.
  return useMemo(
    () => ({ notes, announce, begin, report, announceOnly, discardNotes, reset }),
    [notes, announce, begin, report, announceOnly, discardNotes, reset],
  );
}
