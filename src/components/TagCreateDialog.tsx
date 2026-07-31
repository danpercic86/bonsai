import { useEffect, useRef, useState } from 'react';

export interface TagCreateDialogProps {
  open: boolean;
  /** Short oid of the target commit, shown in the body for context. */
  targetOid: string;
  busy: boolean;
  /** Existing tag names — used for the client-side duplicate check. */
  existingTags: string[];
  /** message is null for a lightweight tag, non-null (the body) for annotated. */
  onSubmit(name: string, message: string | null): void;
  onCancel(): void;
}

/** Documented simplification of git's ref-name rules (the backend is
 *  authoritative). Rejects blanks, leading `-`, whitespace, `..`, and the
 *  characters git forbids in ref names. */
function validateTagName(name: string, existingTags: string[]): string | null {
  const trimmed = name.trim();
  if (trimmed === '') return 'Enter a tag name';
  if (trimmed.startsWith('-')) return 'Tag name cannot start with "-"';
  if (/\s/.test(trimmed)) return 'Tag name cannot contain spaces';
  if (trimmed.includes('..')) return 'Tag name cannot contain ".."';
  if (/[~^:?*[\\]/.test(trimmed)) return 'Tag name contains an invalid character';
  if (existingTags.includes(trimmed)) return 'A tag with that name already exists';
  return null;
}

/**
 * Create-tag modal (P22 §7.4): tag name + a lightweight/annotated toggle, plus a
 * message textarea shown only when annotated. Modeled on PromptDialog styling
 * (`.dialog-overlay` / `.dialog-card`); Esc + overlay-click cancel.
 */
export function TagCreateDialog({
  open,
  targetOid,
  busy,
  existingTags,
  onSubmit,
  onCancel,
}: TagCreateDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [name, setName] = useState('');
  const [annotated, setAnnotated] = useState(false);
  const [message, setMessage] = useState('');

  // Reset + focus each time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setName('');
    setAnnotated(false);
    setMessage('');
    inputRef.current?.focus();
  }, [open]);

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

  const nameError = validateTagName(name, existingTags);
  const messageError = annotated && message.trim() === '' ? 'Enter a tag message' : null;
  const error = nameError ?? messageError;
  const canSubmit = !busy && error === null;

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="Create tag"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Create tag</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (canSubmit) onSubmit(name.trim(), annotated ? message : null);
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              Tag name
              <input
                ref={inputRef}
                type="text"
                className="dialog-input"
                placeholder="v1.0.0"
                value={name}
                onChange={(e) => setName(e.target.value)}
              />
            </label>
            <label className="dialog-checkbox-label">
              <input
                type="checkbox"
                checked={annotated}
                onChange={(e) => setAnnotated(e.target.checked)}
              />
              Annotated (stores a message + tagger)
            </label>
            {annotated && (
              <label className="dialog-label">
                Message
                <textarea
                  className="dialog-input dialog-textarea"
                  placeholder="Release notes…"
                  rows={3}
                  value={message}
                  onChange={(e) => setMessage(e.target.value)}
                />
              </label>
            )}
            <p className="dialog-body-note">
              Tagging commit <span className="mono">{targetOid.slice(0, 7)}</span>.
            </p>
            {error !== null && <p className="dialog-error">{error}</p>}
          </div>
          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={!canSubmit}>
              Create tag
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
