// P113 §10.3 / §17.1 ruling R1 — the one motion this contract allows, and only
// under four conditions, all of which are required.
//
// The defect it fixes is NOT the occlusion P113 exists to remove. `dev.delete-logs`
// is the last row on the Dev page: its note grows below the `Delete all…` button,
// and `ConfirmDialog`'s focus restore scrolls the BUTTON into view, which is
// satisfied while the note's tail is still past `.settings-pane`'s clip. Measured
// at `scrollTop 1207/1310`, ~20 of the note's 64px were visible and
// `elementFromPoint` at its centre fell through to the overlay — because nothing
// below an `overflow-y: auto` clip is PAINTED, so it could not return anything
// else. That is AC2b (clipping), a different failure with a different fix from
// AC2a (occlusion, scroll-independent).
//
// The four conditions:
//   1. MEASURED, not assumed — adjust only when the note's rect is not fully
//      inside the scroll container's CLIENT rect.
//   2. `block: 'nearest'`, `behavior: 'auto'` — instant. An instant scroll
//      correction is the same class of thing as a caret-following scroll, so
//      there is nothing to gate behind `prefers-reduced-motion` and no media
//      query branch. Never `'smooth'`.
//   3. Only while focus is inside the row that OWNS the slot. The user standing
//      on the control gets the correction; a user who navigated elsewhere during
//      the async operation is never yanked.
//   4. Named callers only — `DevCategory` and, since P112 §16.12,
//      `GeneralCategory`. The permission was extended rather than opened up: the
//      external-tools group is the LAST group on General and `general.rescan-tools`
//      is the last row on the page, structurally identical to `dev.delete-logs`,
//      which is where the clipping failure was found; and focus is still on the
//      acting control there, because Browse and Rescan are `aria-disabled`, not
//      `disabled`. The Accounts host slots keep the blanket ban: they sit at the
//      TOP of their group (rarely clipped) and their commit can REMOVE a card, so
//      scrolling there is a jump under the pointer.

import { useEffect, useRef } from 'react';

import type { SettingsOutcome } from './SettingsOutcomeNote';

/** True when `rect` lies entirely inside the scrollport of `container` — its
 *  CLIENT box, so a vertical scrollbar does not count as visible width. */
function clipRect(container: HTMLElement): { top: number; left: number; bottom: number; right: number } {
  const box = container.getBoundingClientRect();
  const top = box.top + container.clientTop;
  const left = box.left + container.clientLeft;
  return { top, left, bottom: top + container.clientHeight, right: left + container.clientWidth };
}

function fullyInside(rect: DOMRect, clip: ReturnType<typeof clipRect>): boolean {
  return (
    rect.top >= clip.top &&
    rect.left >= clip.left &&
    rect.bottom <= clip.bottom &&
    rect.right <= clip.right
  );
}

function correct(slot: string): void {
  // The slot key is inside a quoted attribute value, so a `.` needs no escaping.
  const el = document.querySelector<HTMLElement>(`[data-outcome-note="${slot}"]`);
  if (el === null) return;
  // Condition 3.
  const row = el.closest('.settings-row');
  const active = document.activeElement;
  if (row === null || active === null || !row.contains(active)) return;
  const pane = el.closest<HTMLElement>('.settings-pane');
  // jsdom does not implement `scrollIntoView`; guard rather than stub it in
  // every suite that renders a Dev page.
  if (pane === null || typeof el.scrollIntoView !== 'function') return;
  // Condition 1.
  if (fullyInside(el.getBoundingClientRect(), clipRect(pane))) return;
  // Condition 2.
  el.scrollIntoView({ block: 'nearest', behavior: 'auto' });

  // MEASURED RESIDUAL, not a belt-and-braces retry — and the measurement is the
  // point, because `scrollIntoView` alone does NOT satisfy AC2b here.
  //
  // Harness run 2026-09-14, `dev.delete-logs` with the pathological 7-line
  // outcome: `scrollIntoView` settled at `scrollTop 1269` and left the note's
  // bottom at 687.171875 against a clip bottom of exactly 687 — the pane's rect
  // is 137→687 with zero borders, so this is real clipping, not `clientHeight`
  // rounding. Probed directly in the page: setting `scrollTop = 0` and calling
  // `scrollIntoView({block:'nearest'})` returns to 1269 every time, and
  // `scrollTop += 0.171875` reads back as 1269 — at devicePixelRatio 1 the
  // scroll offset snaps to whole pixels, so the last 0.17px of a fractionally
  // tall row is unreachable from below. `Math.ceil` takes the ONE extra pixel
  // that does fit it, which is what makes the four-edge test pass honestly
  // instead of with a tolerance. Still instant; still gated by condition 1.
  const clip = clipRect(pane);
  const after = el.getBoundingClientRect();
  const overflow = after.bottom - clip.bottom;
  // Only the bottom edge can overshoot here: `nearest` never scrolls further
  // than it must, and the note grows downward from a row already in view. The
  // height guard keeps a note TALLER than the scrollport where `nearest` put it
  // (top aligned) rather than scrolling its head out of sight to chase its tail.
  if (overflow > 0 && after.height <= clip.bottom - clip.top) {
    pane.scrollTop = Math.ceil(pane.scrollTop + overflow);
  }
}

/**
 * Watches `notes` and, after the commit that changed one, brings that slot back
 * inside the pane if the growth pushed it past the clip.
 *
 * Keyed on OBJECT IDENTITY, not text: `report()` mints a fresh `{tone, text}`
 * every time, so repeating the same outcome still registers as a change — the
 * same property `begin()` cannot supply for the announcer.
 *
 * A passive effect, so it runs after `ConfirmDialog`'s unmount cleanup has
 * restored focus to `Delete all…` in the same flush. If focus has not landed
 * (or landed elsewhere) the correction simply does not happen — condition 3 is
 * a gate, never a retry.
 */
export function useOutcomeScrollCorrection(notes: ReadonlyMap<string, SettingsOutcome>): void {
  const previous = useRef(notes);
  useEffect(() => {
    const before = previous.current;
    previous.current = notes;
    for (const [slot, outcome] of notes) {
      if (before.get(slot) === outcome) continue;
      // At most one slot changes per commit: every Dev action is `anyBusy`-gated,
      // and on General a scan and a Browse each report into one slot only.
      correct(slot);
      return;
    }
  }, [notes]);
}
