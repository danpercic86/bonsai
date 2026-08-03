import { useEffect, useRef, useState } from 'react';
import { errorMessage } from '../utils/errors';

export interface WorktreeCreateDialogProps {
  open: boolean;
  busy: boolean;
  /** Local branch names (backend order). */
  localBranches: string[];
  /** Branches already checked out in some worktree — rendered disabled (a
   *  branch can be checked out in only one worktree). */
  usedBranches: string[];
  /** `<mainParent>/.worktrees` — base of the derived-path preview. Display
   *  only; the backend derives the authoritative path (P27 §2.4). */
  container: string;
  /** Resolves on success (parent closes the dialog + toasts); rejects with
   *  AppError → shown inline, dialog stays open. */
  onSubmit(branch: string): Promise<void>;
  onCancel(): void;
}

/** Display-only mirror of the backend's slug derivation (P27 §2.4): every char
 *  outside [A-Za-z0-9._-] → '-', runs collapsed, leading/trailing '-'/'.'
 *  trimmed. Collision suffixes are NOT mirrored — the backend owns those. */
function slugify(branch: string): string {
  return branch
    .replace(/[^A-Za-z0-9._-]+/g, '-')
    .replace(/-{2,}/g, '-')
    .replace(/^[-.]+|[-.]+$/g, '');
}

/**
 * New-worktree modal (P27 §6.5): pick an existing local branch, preview the
 * derived `.worktrees/<slug>` path, create. Modeled on TagCreateDialog
 * (`.dialog-overlay` / `.dialog-card`); Esc + overlay-click cancel.
 */
export function WorktreeCreateDialog({
  open,
  busy,
  localBranches,
  usedBranches,
  container,
  onSubmit,
  onCancel,
}: WorktreeCreateDialogProps) {
  const selectRef = useRef<HTMLSelectElement>(null);
  const [branch, setBranch] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const used = new Set(usedBranches);
  const eligible = localBranches.filter((b) => !used.has(b));

  // Reset + preselect the first eligible branch each time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setBranch(eligible[0] ?? '');
    setError(null);
    setSubmitting(false);
    selectRef.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect must not fire.
      e.stopPropagation();
      if (!busy && !submitting) onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel, busy, submitting]);

  if (!open) return null;

  const inFlight = busy || submitting;
  const canSubmit = !inFlight && branch !== '' && !used.has(branch);
  // While a create is in flight, cancelling would hide the dialog and swallow
  // the eventual inline error (or fire a toast for a "cancelled" create).
  const cancel = () => {
    if (!inFlight) onCancel();
  };
  const preview = branch === '' ? null : `${container}/${slugify(branch)}`;

  return (
    <div className="dialog-overlay" onClick={cancel}>
      <div
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="New worktree"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">New worktree</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (!canSubmit) return;
            setSubmitting(true);
            setError(null);
            onSubmit(branch).catch((err: unknown) => {
              setError(errorMessage(err));
              setSubmitting(false);
            });
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              Branch
              <select
                ref={selectRef}
                className="dialog-input"
                value={branch}
                onChange={(e) => {
                  setBranch(e.target.value);
                  setError(null);
                }}
              >
                {localBranches.length === 0 && (
                  <option value="" disabled>
                    No local branches
                  </option>
                )}
                {localBranches.map((b) => (
                  <option key={b} value={b} disabled={used.has(b)}>
                    {used.has(b) ? `${b} (checked out)` : b}
                  </option>
                ))}
              </select>
            </label>
            {preview !== null ? (
              <p className="dialog-body-note">
                Will be created at <span className="mono">{preview}</span>
                {' '}(the backend derives the final path).
              </p>
            ) : (
              <p className="dialog-body-note">
                Every local branch is already checked out in a worktree.
              </p>
            )}
            {error !== null && <p className="dialog-error">{error}</p>}
          </div>
          <div className="dialog-buttons">
            <button
              type="button"
              className="btn-secondary"
              onClick={cancel}
              disabled={inFlight}
            >
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={!canSubmit}>
              Create worktree
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
