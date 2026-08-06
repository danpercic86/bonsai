import { useEffect, useRef, useState } from 'react';

export interface CherrypickMessageDialogProps {
  open: boolean;
  /** Short oid of the source commit, shown in the body for context. */
  oid: string;
  /** Prefilled full message of the source commit (editable). */
  initialMessage: string;
  /** Fetching the source commit's message (prefill in flight). */
  loading: boolean;
  /** The cherry-pick invoke is in flight. */
  busy: boolean;
  /** Confirm with the (possibly edited) message. */
  onConfirm(message: string): void;
  onCancel(): void;
}

/**
 * P47d: editable commit-message dialog for cherry-pick. Presentational only —
 * the container (`RepoWorkspace`) fetches the source commit's full message and
 * runs the pick on confirm. Modeled on `TagCreateDialog` styling
 * (`.dialog-overlay` / `.dialog-card`); Esc + overlay-click cancel.
 */
export function CherrypickMessageDialog({
  open,
  oid,
  initialMessage,
  loading,
  busy,
  onConfirm,
  onCancel,
}: CherrypickMessageDialogProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [message, setMessage] = useState('');

  // Sync the editable buffer whenever the prefill arrives (or the dialog opens).
  useEffect(() => {
    if (open) setMessage(initialMessage);
  }, [open, initialMessage]);

  // Focus the textarea once the prefill has loaded.
  useEffect(() => {
    if (open && !loading) textareaRef.current?.focus();
  }, [open, loading]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect must not fire.
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  const canSubmit = !busy && !loading && message.trim() !== '';

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="Cherry-pick"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Cherry-pick</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (canSubmit) onConfirm(message);
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              Commit message
              <textarea
                ref={textareaRef}
                className="dialog-input dialog-textarea"
                placeholder={loading ? 'Loading message…' : 'Commit message'}
                rows={8}
                disabled={loading}
                value={message}
                onChange={(e) => setMessage(e.target.value)}
              />
            </label>
            <p className="dialog-body-note">
              Cherry-picking commit <span className="mono">{oid.slice(0, 7)}</span> onto the current
              branch. The original author is preserved.
            </p>
          </div>
          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={!canSubmit}>
              Cherry-pick
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
