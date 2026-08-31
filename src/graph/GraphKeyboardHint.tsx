/** P95 §1.4 — the graph scroller's visually-hidden description.
 *
 *  The scroller is a labelled focusable group (the rows are canvas pixels, so
 *  there is no per-row DOM an `aria-activedescendant` IDREF could point at).
 *  That leaves screen-reader users with no discoverability for the keyboard
 *  model, so the scroller's `aria-describedby` points here.
 *
 *  Its own file so `GraphCanvas.tsx` does not grow. */

/** Exact microcopy from P95 §1.4 — sentence case, no jargon. */
export const GRAPH_KEYBOARD_HINT =
  'Use the arrow keys to move between commits. Press the Menu key or Shift+F10 for actions on the selected commit.';

export function GraphKeyboardHint({ id }: { id: string }) {
  return (
    <span id={id} className="sr-only">
      {GRAPH_KEYBOARD_HINT}
    </span>
  );
}
