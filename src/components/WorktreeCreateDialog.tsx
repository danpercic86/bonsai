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
  /** `<mainParent>/.worktrees/<repo-name>` — base of the derived-path preview.
   *  Display only; the backend derives the authoritative path (P27 §2.4 +
   *  P32 Part A). */
  container: string;
  /** Resolves on success (parent closes the dialog + toasts); rejects with
   *  AppError → shown inline, dialog stays open. `name` is the user-editable
   *  on-disk label (defaults to the branch, decoupled from it — P32 Part A). */
  onSubmit(branch: string, name: string): Promise<void>;
  onCancel(): void;
}

/** Display-only mirror of the backend's slug derivation (P27 §2.4): every char
 *  outside [A-Za-z0-9._-] → '-', runs collapsed, leading/trailing '-'/'.'
 *  trimmed. A `..`-containing result is rejected (returns '') to mirror the
 *  backend's `InvalidName`, so the preview never promises a path the backend
 *  refuses. Collision suffixes are NOT mirrored — the backend owns those. */
function slugify(branch: string): string {
  const slug = branch
    .replace(/[^A-Za-z0-9._-]+/g, '-')
    .replace(/-{2,}/g, '-')
    .replace(/^[-.]+|[-.]+$/g, '');
  return slug.includes('..') ? '' : slug;
}

/**
 * New-worktree modal (P27 §6.5 + P32 Part A): pick an existing local branch,
 * optionally give the worktree its own NAME (defaults to the branch, decoupled
 * from it — the leaf/slug is derived from the name, not the branch), preview the
 * derived `.worktrees/<repo>/<name-slug>` path, create. Modeled on
 * TagCreateDialog (`.dialog-overlay` / `.dialog-card`); Esc + overlay-click
 * cancel.
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
  const [name, setName] = useState('');
  // Once the user edits the name field we stop auto-syncing it to the branch.
  const [nameDirty, setNameDirty] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);

  const used = new Set(usedBranches);
  const eligible = localBranches.filter((b) => !used.has(b));

  // Reset + preselect the first eligible branch each time the dialog opens; the
  // name starts synced to that branch and pristine.
  useEffect(() => {
    if (!open) return;
    const first = eligible[0] ?? '';
    setBranch(first);
    setName(first);
    setNameDirty(false);
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
  // A blank name defaults to the branch (backend + mock do the same); the slug
  // is derived from that effective name, NOT the branch (P32 Part A).
  const effectiveName = name.trim() === '' ? branch : name;
  const nameSlug = slugify(effectiveName);
  const canSubmit = !inFlight && branch !== '' && !used.has(branch) && nameSlug !== '';
  // While a create is in flight, cancelling would hide the dialog and swallow
  // the eventual inline error (or fire a toast for a "cancelled" create).
  const cancel = () => {
    if (!inFlight) onCancel();
  };
  const preview =
    branch === '' || nameSlug === '' ? null : `${container}/${nameSlug}`;
  // Non-blocking advisory: a derived name matching a worktree that already
  // exists (approximated by the slugs of already-checked-out branches) will get
  // a `-2` suffix from the backend. Never blocks submit.
  const usedNameSlugs = new Set(usedBranches.map(slugify));
  const nameInUse = nameSlug !== '' && usedNameSlugs.has(nameSlug);

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
            onSubmit(branch, effectiveName).catch((err: unknown) => {
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
                  const next = e.target.value;
                  setBranch(next);
                  // Keep the name synced to the branch until the user edits it.
                  if (!nameDirty) setName(next);
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
            <label className="dialog-label">
              Name
              <input
                type="text"
                className="dialog-input"
                value={name}
                placeholder={branch}
                onChange={(e) => {
                  setName(e.target.value);
                  setNameDirty(true);
                  setError(null);
                }}
              />
            </label>
            {preview !== null ? (
              <p className="dialog-body-note">
                Will be created at <span className="mono">{preview}</span>
                {' '}(the backend derives the final path).
              </p>
            ) : branch === '' ? (
              <p className="dialog-body-note">
                Every local branch is already checked out in a worktree.
              </p>
            ) : (
              <p className="dialog-body-note">
                Enter a name to derive the worktree path.
              </p>
            )}
            {nameInUse && (
              <p className="dialog-body-note">
                Name in use — will create <span className="mono">{nameSlug}-2</span>.
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
