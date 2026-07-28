// Keyboard-shortcut reference overlay (P1 contract §6.4). Static content;
// closes on Esc (App's global handler), backdrop click, ✕, or `?` again.

export interface ShortcutOverlayProps {
  open: boolean;
  onClose(): void;
}

/** The §6.1 binding table, in display order. */
const SHORTCUTS: { keys: string[]; action: string }[] = [
  { keys: ['Ctrl', 'Enter'], action: 'Commit staged changes' },
  { keys: ['Esc'], action: 'Deselect commit / close dialog' },
  { keys: ['Ctrl', 'R'], action: 'Refresh' },
  { keys: ['F5'], action: 'Refresh' },
  { keys: ['Ctrl', 'O'], action: 'Open repository' },
  { keys: ['Ctrl', 'Shift', 'F'], action: 'Fetch all remotes' },
  { keys: ['Ctrl', 'Shift', 'P'], action: 'Pull (fast-forward only)' },
  { keys: ['Ctrl', 'Shift', 'U'], action: 'Push current branch' },
  { keys: ['↑', '↓'], action: 'Move commit selection' },
  { keys: ['?'], action: 'Toggle this overlay' },
];

export function ShortcutOverlay({ open, onClose }: ShortcutOverlayProps) {
  if (!open) return null;
  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div className="dialog-card shortcut-card" role="dialog" aria-label="Keyboard shortcuts">
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
                    <kbd className="kbd">{k}</kbd>
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
