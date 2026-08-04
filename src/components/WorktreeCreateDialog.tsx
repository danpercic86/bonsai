import { useCallback, useEffect, useRef, useState } from 'react';
import { ipc } from '../ipc';
import type { CopyAction, CopyCandidate, CopySelection, CopyVerdict } from '../ipc';
import { errorMessage } from '../utils/errors';
import { WorktreeCopyCandidates } from './WorktreeCopyCandidates';

export interface WorktreeCreateDialogProps {
  open: boolean;
  busy: boolean;
  /** Repo whose uncommitted/gitignored files can be copied into the worktree. */
  repoId: string;
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
   *  on-disk label (defaults to the branch, decoupled from it — P32 Part A).
   *  `selections` are the `copy` decisions (empty → plain create; P32 Part B). */
  onSubmit(branch: string, name: string, selections: CopySelection[]): Promise<void>;
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
  repoId,
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

  // P32 Part B: copy uncommitted/gitignored files into the new worktree.
  const [candidates, setCandidates] = useState<CopyCandidate[]>([]);
  const [candidatesLoading, setCandidatesLoading] = useState(false);
  const [candidatesError, setCandidatesError] = useState<string | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  // Conflict verdict per checked path (from previewWorktreeCopy).
  const [verdictByPath, setVerdictByPath] = useState<Map<string, CopyVerdict>>(new Map());
  // Overwrite/Skip decision per conflicted path; absent → Skip (safe default).
  const [conflictActions, setConflictActions] = useState<Record<string, CopyAction>>({});
  // True when the last conflict-preview call failed → every checked path has an
  // UNKNOWN verdict and must be treated like a conflict (explicit decision,
  // default Skip) so we never silently overwrite the target branch.
  const [previewFailed, setPreviewFailed] = useState(false);
  // Monotonic id: a stale preview response (older selection/branch) is dropped.
  const previewIdRef = useRef(0);

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
    setChecked(new Set());
    setVerdictByPath(new Map());
    setConflictActions({});
    setPreviewFailed(false);
    selectRef.current?.focus();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  // Fetch copy candidates once per open (they depend on the repo, not the
  // branch). Empty list → the section simply doesn't render.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setCandidatesLoading(true);
    setCandidatesError(null);
    setCandidates([]);
    ipc
      .listCopyCandidates(repoId)
      .then((rows) => {
        if (!cancelled) setCandidates(rows);
      })
      .catch((e: unknown) => {
        if (!cancelled) setCandidatesError(errorMessage(e));
      })
      .finally(() => {
        if (!cancelled) setCandidatesLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [open, repoId]);

  // Re-classify whenever the checked set OR the branch changes. A reqId guard
  // drops stale responses; an empty selection short-circuits to no verdicts.
  useEffect(() => {
    if (!open) return;
    const paths = [...checked];
    const id = (previewIdRef.current += 1);
    if (paths.length === 0 || branch === '') {
      setVerdictByPath(new Map());
      setPreviewFailed(false);
      return;
    }
    ipc
      .previewWorktreeCopy(repoId, branch, paths)
      .then((entries) => {
        if (previewIdRef.current !== id) return;
        setVerdictByPath(new Map(entries.map((e) => [e.path, e.verdict])));
        setPreviewFailed(false);
      })
      .catch(() => {
        // Preview failed → verdicts are UNKNOWN. Rather than silently copy
        // (which could overwrite a divergent target-branch file), mark the whole
        // selection as needing an explicit decision (default Skip). The user can
        // still flip individual files to Overwrite.
        if (previewIdRef.current !== id) return;
        setVerdictByPath(new Map());
        setPreviewFailed(true);
      });
  }, [open, repoId, branch, checked]);

  const toggleChecked = useCallback((path: string) => {
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(path)) {
        next.delete(path);
        // Drop any stale Overwrite/Skip choice so it can't resurface if the file
        // is re-checked later (it would re-default to Skip).
        setConflictActions((actions) => {
          if (!(path in actions)) return actions;
          const { [path]: _drop, ...rest } = actions;
          return rest;
        });
      } else {
        next.add(path);
      }
      return next;
    });
  }, []);

  const setAction = useCallback((path: string, action: CopyAction) => {
    setConflictActions((prev) => ({ ...prev, [path]: action }));
  }, []);

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
            // Build the copy plan. A checked path is "verified clean" only when
            // the preview succeeded AND returned a non-conflict verdict → always
            // copy. A conflict OR an unknown verdict (previewFailed) needs an
            // explicit Overwrite choice; default Skip → omitted (not written).
            // The Set already de-dupes a path appearing in two groups.
            const selections: CopySelection[] = [];
            for (const path of checked) {
              const needsDecision = previewFailed || verdictByPath.get(path) === 'conflict';
              const action = needsDecision ? conflictActions[path] ?? 'skip' : 'copy';
              if (action === 'copy') selections.push({ path, action: 'copy' });
            }
            setSubmitting(true);
            setError(null);
            onSubmit(branch, effectiveName, selections).catch((err: unknown) => {
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
            <WorktreeCopyCandidates
              candidates={candidates}
              loading={candidatesLoading}
              error={candidatesError}
              checked={checked}
              verdictByPath={verdictByPath}
              previewFailed={previewFailed}
              conflictActions={conflictActions}
              disabled={inFlight}
              onToggle={toggleChecked}
              onSetAction={setAction}
            />
            {previewFailed && (
              <p className="dialog-body-note wt-copy-warn">
                Couldn&apos;t check for conflicts against{' '}
                <span className="mono">{branch}</span> — choose Overwrite per file
                to copy it anyway; unselected files are skipped.
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
