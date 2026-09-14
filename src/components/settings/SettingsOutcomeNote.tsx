// P113 §5 — the ONE element every Settings action outcome renders into.
//
// Why a component and not an inline `<p>` at each call site: two invariants are
// invisible at a call site and were already broken once on this surface.
//
//   1. It is PERMANENTLY MOUNTED with empty text when idle. An element that
//      appears in the same React commit as its text is not reliably announced,
//      and `:empty` (settings-outcome-note.css) is what collapses its chrome to
//      zero height — so `display: none` must never be reached for.
//   2. It carries NO `aria-live` and NO `role`. It is description-only: the
//      section owns exactly one live region and `useOutcomeNotes` writes it, so
//      no outcome can be uttered twice (ui-reference §12.14).
//
// Not a toast: no ✕, no timer, no motion. It reads "this is the result of the
// last time you pressed this", and it clears when that control is pressed again.

/** The result of one Settings action. `tone` is often computed at RUNTIME (the
 *  log-delete outcome is `success` or `error` for the same key press), which is
 *  why tone selects the RECIPE and never the LOCATION. */
export interface SettingsOutcome {
  tone: 'success' | 'error';
  text: string;
}

export function SettingsOutcomeNote({
  slot,
  id,
  outcome,
}: {
  /** Stable slot key — the row id (`dev.logs`) or the host (`github.com`).
   *  Emitted as `data-outcome-note` so a harness can hit-test it by slot. */
  slot: string;
  /** DOM id, composed into the acting control's `aria-describedby`. */
  id: string;
  outcome: SettingsOutcome | null;
}) {
  // The idle element still carries a modifier: the `:empty` collapse is written
  // against the modifiers, so a bare `.settings-row-note` would keep its 2px
  // top margin and idle row geometry would change (§10.1, AC11).
  const modifier = outcome?.tone === 'error' ? 'warn' : 'result';
  return (
    <p
      id={id}
      data-outcome-note={slot}
      className={`settings-row-note settings-row-note--${modifier}`}
      /* The expression is the element's ONLY child and there is no JSX
         whitespace around it on purpose: a newline inside the tags is a text
         node, `:empty` stops matching, and the idle chrome silently stops
         collapsing. Do not reformat this line. */
    >{outcome?.text ?? ''}</p>
  );
}
