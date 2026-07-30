import { useEffect, useRef, useState } from 'react';

export interface PromptDialogProps {
  open: boolean;
  title: string;
  /** Label above the text input. */
  label: string;
  placeholder?: string;
  initialValue?: string;
  confirmLabel: string;
  busy: boolean;
  /** Return an error string to block submit, or null when valid. Re-run on every
   *  keystroke; the error renders under the input and disables the confirm button. */
  validate?(value: string): string | null;
  onSubmit(value: string): void;
  onCancel(): void;
}

/**
 * Small reusable single-input prompt modal (P11 §1.4). Modeled on ConfirmDialog,
 * but Enter submits (a prompt exists to submit text) and the confirm button is
 * `.btn-primary` (a create action, not destructive). Initial focus lands on the
 * input; Esc + overlay-click cancel.
 */
export function PromptDialog({
  open,
  title,
  label,
  placeholder,
  initialValue,
  confirmLabel,
  busy,
  validate,
  onSubmit,
  onCancel,
}: PromptDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [value, setValue] = useState(initialValue ?? '');

  // Seed the value + focus/select the input each time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setValue(initialValue ?? '');
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [open, initialValue]);

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

  const error = validate ? validate(value) : null;
  const canSubmit = !busy && error === null;

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">{title}</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (canSubmit) onSubmit(value);
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              {label}
              <input
                ref={inputRef}
                type="text"
                className="dialog-input"
                placeholder={placeholder}
                value={value}
                onChange={(e) => setValue(e.target.value)}
              />
            </label>
            {error !== null && <p className="dialog-error">{error}</p>}
          </div>
          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={!canSubmit}>
              {confirmLabel}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
