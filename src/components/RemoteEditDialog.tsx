import { useEffect, useRef, useState } from 'react';

export interface RemoteEditDialogProps {
  open: boolean;
  title: string;
  confirmLabel: string;
  busy: boolean;
  /** When set (Edit URL), the name field is read-only and pre-seeded. */
  nameReadOnly?: boolean;
  initialName?: string;
  initialUrl?: string;
  /** Existing remote names — used for the client-side duplicate check when
   *  adding (skipped when `nameReadOnly`). */
  existingNames: string[];
  onSubmit(name: string, url: string): void;
  onCancel(): void;
}

/** Client mirror of git remote-name rules (the backend is authoritative):
 *  non-empty, no whitespace. */
function validateName(name: string, existingNames: string[], skipDup: boolean): string | null {
  const trimmed = name.trim();
  if (trimmed === '') return 'Enter a remote name';
  if (/\s/.test(trimmed)) return 'Remote name cannot contain whitespace';
  if (!skipDup && existingNames.includes(trimmed)) return 'A remote with that name already exists';
  return null;
}

/**
 * Two-field (name + url) modal for adding or editing a remote (P22 §7.4).
 * Modeled on PromptDialog styling (`.dialog-overlay` / `.dialog-card`); Esc +
 * overlay-click cancel. Used for "Add remote" (both fields editable) and
 * "Edit URL" (`nameReadOnly`, name pre-seeded).
 */
export function RemoteEditDialog({
  open,
  title,
  confirmLabel,
  busy,
  nameReadOnly = false,
  initialName,
  initialUrl,
  existingNames,
  onSubmit,
  onCancel,
}: RemoteEditDialogProps) {
  const firstRef = useRef<HTMLInputElement>(null);
  const [name, setName] = useState(initialName ?? '');
  const [url, setUrl] = useState(initialUrl ?? '');

  useEffect(() => {
    if (!open) return;
    setName(initialName ?? '');
    setUrl(initialUrl ?? '');
    firstRef.current?.focus();
    firstRef.current?.select();
  }, [open, initialName, initialUrl]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  const nameError = nameReadOnly ? null : validateName(name, existingNames, false);
  const urlError = url.trim() === '' ? 'Enter a URL' : null;
  const error = nameError ?? urlError;
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
            if (canSubmit) onSubmit(name.trim(), url.trim());
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              Name
              <input
                ref={nameReadOnly ? undefined : firstRef}
                type="text"
                className="dialog-input"
                placeholder="origin"
                value={name}
                readOnly={nameReadOnly}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="dialog-label">
              Fetch URL
              <input
                ref={nameReadOnly ? firstRef : undefined}
                type="text"
                className="dialog-input"
                placeholder="https://example.com/repo.git"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
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
