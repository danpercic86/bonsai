import { useEffect, useRef } from 'react';
import type { ReactNode } from 'react';

export interface ConfirmDialogProps {
  open: boolean;
  title: string;
  /** Body content; branch names rendered in <span className="mono">. */
  children: ReactNode;
  confirmLabel: string;
  /** Visual style of the confirm button. Defaults to 'danger' so existing
   * (destructive) call sites are unchanged; use 'primary' for non-destructive
   * confirmations (e.g. set-upstream & push). */
  confirmVariant?: 'danger' | 'primary';
  /** P68g OQ-1: extra class on `.dialog-card`, for a body that needs more than the
   *  default 360px (e.g. `ai-consent-card` = 420px). Width only — the focus/Esc
   *  behaviour is unchanged. */
  cardClass?: string;
  busy: boolean;
  onConfirm(): void;
  onCancel(): void;
}

/**
 * Small reusable confirmation modal (M5 contract §4.3). Initial focus lands on
 * Cancel and Enter only activates the focused button, so a stray Enter never
 * confirms a destructive action. Esc and overlay-click cancel.
 */
export function ConfirmDialog({
  open,
  title,
  children,
  confirmLabel,
  confirmVariant = 'danger',
  cardClass,
  busy,
  onConfirm,
  onCancel,
}: ConfirmDialogProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    if (open) cancelRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect (a bubble
      // listener on window) must not also fire while the dialog is open.
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className={cardClass === undefined ? 'dialog-card' : `dialog-card ${cardClass}`}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">{title}</h2>
        <div className="dialog-body">{children}</div>
        <div className="dialog-buttons">
          <button type="button" className="btn-secondary" ref={cancelRef} onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className={confirmVariant === 'primary' ? 'btn-primary' : 'btn-danger'}
            disabled={busy}
            onClick={onConfirm}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
