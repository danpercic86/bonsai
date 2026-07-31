import { useEffect, useRef, useState } from 'react';
import type { CloneProgress } from '../ipc';

/**
 * Derive the destination folder name from a clone URL: the last path segment
 * with a trailing `.git` stripped. Handles https (`.../foo/bar.git`), scp-style
 * ssh (`git@host:foo/bar.git`), and trailing slashes. Falls back to a sanitized
 * name — or `'repository'` — when parsing yields nothing usable.
 */
export function deriveRepoName(url: string): string {
  const trimmed = url.trim().replace(/[\\/]+$/, '');
  // Split on path, scp-`:` and backslash separators; the tail is the repo name.
  const last = trimmed.split(/[\\/:]/).pop() ?? '';
  const name = last.replace(/\.git$/i, '');
  const sanitized = name.replace(/[^A-Za-z0-9._-]/g, '');
  // Reject empty or all-dots names ('.', '..', '...') — joinRepoPath would
  // otherwise resolve them to the parent/grandparent folder.
  return /[^.]/.test(sanitized) ? sanitized : 'repository';
}

/**
 * Join a parent folder and a repo name using the separator the parent already
 * uses (backslash on Windows paths, else forward slash — matching how mock and
 * real paths are represented). Strips a trailing separator off the parent.
 */
export function joinRepoPath(parent: string, name: string): string {
  const sep = parent.includes('\\') && !parent.includes('/') ? '\\' : '/';
  const trimmed = parent.replace(/[\\/]+$/, '');
  return `${trimmed}${sep}${name}`;
}

export interface CloneDialogProps {
  open: boolean;
  /** true while a clone is in flight. */
  busy: boolean;
  /** latest tick (null before the first). */
  progress: CloneProgress | null;
  /** inline error (authFailed/networkError/io/git). */
  error: string | null;
  /** App wires this to ipc.pickFolder(). */
  onPickDest(): void;
  /** chosen parent folder (App-owned); null until picked. */
  dest: string | null;
  /** App runs the clone with the entered url. */
  onSubmit(url: string): void;
  onCancel(): void;
}

/**
 * Clone-a-repository modal (P21 §6.1). Modeled on PromptDialog (same
 * `.dialog-*` shell, focus/Esc-capture/overlay-click discipline). The user
 * enters a URL and picks a PARENT folder; the dialog derives the repo name from
 * the URL and shows the computed final path. While cloning it renders a
 * determinate progress bar; errors show inline (the dialog stays open to retry).
 */
export function CloneDialog({
  open,
  busy,
  progress,
  error,
  onPickDest,
  dest,
  onSubmit,
  onCancel,
}: CloneDialogProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const [url, setUrl] = useState('');

  // Reset + focus the URL input each time the dialog opens.
  useEffect(() => {
    if (!open) return;
    setUrl('');
    inputRef.current?.focus();
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect must not also
      // fire while the dialog is open.
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  const trimmedUrl = url.trim();
  const name = trimmedUrl.length > 0 ? deriveRepoName(trimmedUrl) : null;
  const finalPath = dest !== null && name !== null ? joinRepoPath(dest, name) : null;
  const canSubmit = !busy && trimmedUrl.length > 0 && dest !== null;

  // Deterministic fraction (§4.2): resolving-deltas phase once totalDeltas>0.
  const fraction =
    progress === null
      ? 0
      : progress.totalDeltas > 0
        ? progress.indexedDeltas / progress.totalDeltas
        : progress.totalObjects > 0
          ? progress.receivedObjects / progress.totalObjects
          : 0;
  const phaseLabel =
    progress !== null && progress.totalDeltas > 0 ? 'Resolving deltas…' : 'Receiving objects…';
  const percent = Math.round(fraction * 100);

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="Clone repository"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Clone repository</h2>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            if (canSubmit) onSubmit(trimmedUrl);
          }}
        >
          <div className="dialog-body">
            <label className="dialog-label">
              Repository URL
              <input
                ref={inputRef}
                type="text"
                className="dialog-input"
                placeholder="https://github.com/owner/repo.git"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                disabled={busy}
              />
            </label>

            <div className="clone-dest-row">
              <span className="clone-dest-path" title={dest ?? undefined}>
                {dest ?? 'No parent folder chosen'}
              </span>
              <button
                type="button"
                className="btn-secondary clone-dest-btn"
                onClick={onPickDest}
                disabled={busy}
              >
                {'Choose…'}
              </button>
            </div>

            {finalPath !== null && (
              <p className="dialog-body-note">
                {'Will clone into '}
                <strong>{finalPath}</strong>
              </p>
            )}

            {busy && (
              <div className="clone-progress" aria-live="polite">
                <div className="clone-progress-head">
                  <span>{phaseLabel}</span>
                  <span>{`${percent}%`}</span>
                </div>
                <progress className="clone-progress-bar" max={1} value={fraction} />
                {progress !== null && (
                  <p className="clone-progress-detail">
                    {`${formatBytes(progress.receivedBytes)} received`}
                  </p>
                )}
              </div>
            )}

            {error !== null && <p className="dialog-error">{error}</p>}
          </div>
          <div className="dialog-buttons">
            <button type="button" className="btn-secondary" onClick={onCancel}>
              Cancel
            </button>
            <button type="submit" className="btn-primary" disabled={!canSubmit}>
              {busy ? 'Cloning…' : 'Clone'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const kib = bytes / 1024;
  if (kib < 1024) return `${kib.toFixed(1)} KiB`;
  return `${(kib / 1024).toFixed(1)} MiB`;
}
