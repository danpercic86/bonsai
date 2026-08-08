import { useEffect, useRef, useState } from 'react';
import { ConfirmDialog } from '../ConfirmDialog';

export interface SubmoduleDialogsProps {
  mutating: boolean;

  addOpen: boolean;
  setAddOpen: (v: boolean) => void;
  handleAddSubmodule(url: string, path: string): void;

  pendingDeinit: string | null;
  setPendingDeinit: (v: string | null) => void;
  handleDeinitSubmodule(name: string): void;

  pendingRemove: string | null;
  setPendingRemove: (v: string | null) => void;
  handleRemoveSubmodule(name: string): void;
}

/** P60d submodule dialogs: Add (url + path prompt), Deinit (confirm), and
 *  Remove (destructive confirm). Purely presentational — RepoWorkspace owns the
 *  open/pending flags + handlers. */
export function SubmoduleDialogs({
  mutating,
  addOpen,
  setAddOpen,
  handleAddSubmodule,
  pendingDeinit,
  setPendingDeinit,
  handleDeinitSubmodule,
  pendingRemove,
  setPendingRemove,
  handleRemoveSubmodule,
}: SubmoduleDialogsProps) {
  return (
    <>
      <AddSubmoduleDialog
        open={addOpen}
        busy={mutating}
        onSubmit={(url, path) => {
          setAddOpen(false);
          handleAddSubmodule(url, path);
        }}
        onCancel={() => setAddOpen(false)}
      />

      {/* Deinit: clears config + empties the worktree; .gitmodules is kept, so
          this is reversible via Init/Update — a primary (non-danger) confirm. */}
      <ConfirmDialog
        open={pendingDeinit !== null}
        title="Deinitialize submodule"
        confirmLabel="Deinitialize"
        confirmVariant="primary"
        busy={mutating}
        onConfirm={() => {
          const name = pendingDeinit;
          setPendingDeinit(null);
          if (name !== null) handleDeinitSubmodule(name);
        }}
        onCancel={() => setPendingDeinit(null)}
      >
        <div>
          Deinitialize "<span className="mono">{pendingDeinit ?? ''}</span>"?
        </div>
        <div className="dialog-body-note">
          Clears its local config and empties the working tree. The{' '}
          <span className="mono">.gitmodules</span> entry is kept, so you can re-initialize it
          later.
        </div>
      </ConfirmDialog>

      {/* Remove: deinit + git rm + drops .git/modules — destructive. */}
      <ConfirmDialog
        open={pendingRemove !== null}
        title="Remove submodule"
        confirmLabel="Remove submodule"
        busy={mutating}
        onConfirm={() => {
          const name = pendingRemove;
          setPendingRemove(null);
          if (name !== null) handleRemoveSubmodule(name);
        }}
        onCancel={() => setPendingRemove(null)}
      >
        <div>
          Remove "<span className="mono">{pendingRemove ?? ''}</span>" entirely?
        </div>
        <div className="dialog-body-note">
          Drops the <span className="mono">.gitmodules</span> entry and the gitlink (staged for the
          next commit) and deletes the submodule's working tree from disk. This cannot be undone
          from Bonsai.
        </div>
      </ConfirmDialog>
    </>
  );
}

/** Two-field prompt (url + path) for adding a submodule. Enter submits; Esc /
 *  overlay cancel. Path defaults from the URL's basename once the user has typed
 *  a URL but not yet touched the path field. */
function AddSubmoduleDialog({
  open,
  busy,
  onSubmit,
  onCancel,
}: {
  open: boolean;
  busy: boolean;
  onSubmit(url: string, path: string): void;
  onCancel(): void;
}) {
  const urlRef = useRef<HTMLInputElement>(null);
  const [url, setUrl] = useState('');
  const [path, setPath] = useState('');
  const [pathEdited, setPathEdited] = useState(false);

  useEffect(() => {
    if (!open) return;
    setUrl('');
    setPath('');
    setPathEdited(false);
    urlRef.current?.focus();
  }, [open]);

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

  // Suggested path = the repo name from the URL, until the user edits the field.
  const effectivePath = pathEdited ? path : path === '' ? derivePath(url) : path;
  const error = validate(url, effectivePath);
  const canSubmit = !busy && error === null;

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="Add submodule"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Add submodule</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (canSubmit) onSubmit(url.trim(), effectivePath.trim());
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              Repository URL
              <input
                ref={urlRef}
                type="text"
                className="dialog-input"
                placeholder="https://example.com/lib.git"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
            </label>
            <label className="dialog-label">
              Path (relative to the repository root)
              <input
                type="text"
                className="dialog-input"
                placeholder="vendor/lib"
                value={effectivePath}
                onChange={(e) => {
                  setPathEdited(true);
                  setPath(e.target.value);
                }}
              />
            </label>
            {error !== null && <p className="dialog-error">{error}</p>}
          </div>
          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={!canSubmit}>
              Add submodule
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

/** Last path segment of a git URL, stripped of a trailing `.git` — a friendly
 *  default submodule path. */
function derivePath(url: string): string {
  const trimmed = url.trim().replace(/[/\\]+$/, '');
  if (trimmed === '') return '';
  const last = trimmed.split(/[/\\]/).pop() ?? '';
  return last.replace(/\.git$/i, '');
}

/** Mirror the backend guards: non-blank url + a relative, non-traversing path
 *  (no absolute, no `..`, no backslash). Returns an error string or null. */
function validate(url: string, path: string): string | null {
  if (url.trim() === '') return 'Enter a repository URL';
  const p = path.trim();
  if (p === '') return 'Enter a path for the submodule';
  if (p.startsWith('/') || /^[A-Za-z]:/.test(p)) return 'Path must be relative to the repository';
  if (p.includes('\\')) return 'Use forward slashes in the path';
  if (p.split('/').includes('..')) return 'Path cannot contain ".."';
  return null;
}
