// Keyboard-shortcut reference overlay (P1 contract §6.4). Static content;
// closes on Esc (App's global handler), backdrop click, ✕, or `?` again.

import { useRef } from 'react';
import { keyLabel } from '../utils/platform';
import { useDialogFocus } from '../hooks/useDialogFocus';

export interface ShortcutOverlayProps {
  open: boolean;
  onClose(): void;
}

/** The §6.1 binding table, in display order. `Mod` is the primary accelerator —
 *  Ctrl on Windows/Linux, ⌘ on macOS (the handlers bind `ctrlKey || metaKey`).
 *  Each token becomes its own <kbd> cap, so the caps stay '+'-separated on every
 *  platform (inline one-string labels use `shortcutLabel` instead). */
const SHORTCUTS: { keys: string[]; action: string }[] = [
  { keys: ['Mod', 'Enter'], action: 'Commit staged changes' },
  { keys: ['Esc'], action: 'Deselect commit / close dialog' },
  { keys: ['Mod', 'R'], action: 'Refresh' },
  { keys: ['F5'], action: 'Refresh' },
  { keys: ['Mod', 'O'], action: 'Open repository' },
  { keys: ['Mod', 'F'], action: 'Search commits' },
  { keys: ['Mod', 'K'], action: 'Open command palette' },
  { keys: ['Mod', 'Shift', 'F'], action: 'Fetch all remotes' },
  { keys: ['Mod', 'Shift', 'P'], action: 'Pull (fast-forward only)' },
  { keys: ['Mod', 'Shift', 'U'], action: 'Push current branch' },
  { keys: ['Mod', 'Shift', 'A'], action: 'AI activity dock' },
  { keys: ['↑', '↓'], action: 'Move commit selection' },
  { keys: ['Page Up', 'Page Down'], action: 'Move commit selection by one screenful' },
  { keys: ['Home'], action: 'Select the topmost commit' },
  { keys: ['End'], action: 'Select the last commit' },
  { keys: ['?'], action: 'Toggle this overlay' },
];

export function ShortcutOverlay({ open, onClose }: ShortcutOverlayProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  // Modal focus: move focus into the card on open, trap Tab, restore on close.
  useDialogFocus(open, cardRef, true);

  if (!open) return null;
  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={cardRef}
        className="dialog-card shortcut-card"
        role="dialog"
        aria-modal="true"
        aria-label="Keyboard shortcuts"
        tabIndex={-1}
      >
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">Keyboard shortcuts</h2>
          <button
            type="button"
            className="btn-icon shortcut-close"
            aria-label="Close"
            title="Close"
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>
        <ul className="shortcut-list">
          {SHORTCUTS.map((s) => (
            <li key={s.keys.join('+') + s.action} className="shortcut-row">
              <span className="shortcut-keys">
                {s.keys.map((k, i) => (
                  <span key={i}>
                    {i > 0 && <span className="shortcut-plus">{'+'}</span>}
                    <kbd className="kbd">{keyLabel(k)}</kbd>
                  </span>
                ))}
              </span>
              <span className="shortcut-action">{s.action}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
}
