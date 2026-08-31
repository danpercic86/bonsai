import { useEffect } from 'react';
import type { RefObject } from 'react';

// Elements that can receive keyboard focus inside a dialog. `:not([disabled])`
// drops disabled controls; the `[tabindex="-1"]` exclusion drops the dialog
// container itself, which takes `tabIndex={-1}` only so it can hold INITIAL
// focus without joining the Tab cycle.
const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

function focusableWithin(container: HTMLElement): HTMLElement[] {
  return Array.from(container.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
}

/**
 * Shared modal-dialog focus management (a11y batch). Call once with the dialog's
 * `.dialog-card` ref:
 *
 *  - captures the element focused when the dialog opened and restores focus to it
 *    on close/unmount, guarding a removed/detached node (mirrors PrMergeDialog);
 *  - with `autoFocus`, moves initial focus to the container on open — give the
 *    container `tabIndex={-1}`. Callers with a bespoke initial focus
 *    (ConfirmDialog → Cancel, PromptDialog → input) omit `autoFocus` and focus
 *    their own element;
 *  - traps Tab / Shift+Tab within the container so focus can't reach the inert
 *    background. The trap no-ops whenever focus is NOT within this container
 *    (i.e. a nested dialog rendered as a sibling holds it), so stacked dialogs
 *    each manage only their own focus.
 */
export function useDialogFocus<T extends HTMLElement>(
  open: boolean,
  containerRef: RefObject<T | null>,
  autoFocus = false,
): void {
  // Capture the trigger + (optionally) move focus in on open; restore on close.
  useEffect(() => {
    if (!open) return;
    const trigger = document.activeElement as HTMLElement | null;
    if (autoFocus) containerRef.current?.focus();
    return () => {
      if (trigger !== null && trigger.isConnected && typeof trigger.focus === 'function') {
        trigger.focus();
      }
    };
  }, [open, autoFocus, containerRef]);

  // Focus trap: keep Tab / Shift+Tab inside the dialog.
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Tab') return;
      const container = containerRef.current;
      if (container === null) return;
      const active = document.activeElement;
      // Focus lives in a nested/sibling dialog → let it trap its own Tab.
      if (active !== null && !container.contains(active)) return;
      const focusable = focusableWithin(container);
      if (focusable.length === 0) {
        e.preventDefault();
        return;
      }
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (e.shiftKey) {
        if (active === first || active === container) {
          e.preventDefault();
          last.focus();
        }
      } else if (active === last) {
        e.preventDefault();
        first.focus();
      }
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, containerRef]);
}
