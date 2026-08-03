// P24d §8.3: the activation SAFETY GATE. Opening this dialog previews (writes
// nothing) the per-target current-vs-proposed content; the "Activate & write
// files" button stays disabled until the preview loads. Confirming is the ONLY
// path that writes instruction files; Cancel writes nothing.

import { useEffect, useState } from 'react';
import { ipc } from '../ipc';
import type { ProfileActivation, ProfilePreviewEntry } from '../ipc';
import { usePushToast } from '../ToastContext';
import { errorMessage } from '../utils/errors';

/** Shared read-only two-pane text compare (contract §8.1 permits a simple
 *  two-column current-vs-proposed view for v1 instead of the git-coupled
 *  DiffView). `left`/`right` null => the file is absent (new / missing). */
export function TextComparePane({
  leftLabel,
  rightLabel,
  left,
  right,
}: {
  leftLabel: string;
  rightLabel: string;
  left: string | null;
  right: string | null;
}) {
  return (
    <div className="asset-compare">
      <div className="asset-compare-col">
        <div className="asset-compare-head">{leftLabel}</div>
        {left === null ? (
          <div className="asset-compare-empty">No file — will be created</div>
        ) : (
          <pre className="asset-compare-body mono">{left}</pre>
        )}
      </div>
      <div className="asset-compare-col">
        <div className="asset-compare-head">{rightLabel}</div>
        {right === null ? (
          <div className="asset-compare-empty">No content</div>
        ) : (
          <pre className="asset-compare-body mono">{right}</pre>
        )}
      </div>
    </div>
  );
}

export interface ProfileActivateDialogProps {
  open: boolean;
  repoId: string;
  /** Profile to activate; null when the dialog is closed. */
  name: string | null;
  /** P31 §7: target worktree KEY (`"@main"` | linked name). When set, preview
   *  and activation route to the per-worktree commands and the header names the
   *  worktree; when absent the P24 open-tab path is unchanged. */
  worktreeName?: string | null;
  onClose(): void;
  /** Fired after a successful activation with the backend result (its `store`
   *  carries the updated `activeProfile`; the parent also refetches inventory). */
  onActivated(activation: ProfileActivation): void;
}

export function ProfileActivateDialog({
  open,
  repoId,
  name,
  worktreeName = null,
  onClose,
  onActivated,
}: ProfileActivateDialogProps) {
  const pushToast = usePushToast();
  const [preview, setPreview] = useState<ProfilePreviewEntry[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // P31: eligibility / dirty-target failures on confirm stay IN the dialog
  // (nothing was written; the preview is still valid context for the message).
  const [activateError, setActivateError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // Bumped after a confirm failure to force a fresh preview — the user must
  // re-confirm against the CURRENT diff, never the pre-failure one.
  const [previewNonce, setPreviewNonce] = useState(0);

  // Load the preview each time the dialog opens for a profile. previewProfile
  // writes nothing; the Activate button is gated on `preview !== null`.
  useEffect(() => {
    if (!open || name === null) return;
    let cancelled = false;
    setPreview(null);
    setLoadError(null);
    (async () => {
      try {
        const entries =
          worktreeName !== null
            ? await ipc.previewWorktreeProfile(repoId, worktreeName, name)
            : await ipc.previewProfile(repoId, name);
        if (!cancelled) setPreview(entries);
      } catch (e) {
        if (!cancelled) setLoadError(errorMessage(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, repoId, name, worktreeName, previewNonce]);

  // Clear the confirm-failure banner only on open/profile change, not on the
  // nonce-driven re-preview (the banner explains WHY the preview reloaded).
  useEffect(() => {
    if (open) setActivateError(null);
  }, [open, repoId, name, worktreeName]);

  // Esc closes (capture phase + stopPropagation, mirroring ConfirmDialog).
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopPropagation();
      if (!busy) onClose();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, busy, onClose]);

  if (!open || name === null) return null;

  const activate = async (): Promise<void> => {
    setBusy(true);
    setActivateError(null);
    try {
      const activation =
        worktreeName !== null
          ? await ipc.activateWorktreeProfile(repoId, worktreeName, name)
          : await ipc.activateProfile(repoId, name);
      const written = activation.results.filter((r) => r.action !== 'unchanged').length;
      const where = worktreeName !== null ? ` in ${worktreeName}` : '';
      if (written === 0) {
        pushToast('info', 'No changes — files already match the profile');
      } else {
        pushToast(
          'success',
          `Activated '${name}'${where} — wrote ${written} file${written === 1 ? '' : 's'}`,
        );
      }
      onActivated(activation);
      onClose();
    } catch (e) {
      // P31 §7: dirty-target / eligibility refusals surface in-dialog for the
      // worktree path (nothing written); the legacy tab path keeps its toast.
      if (worktreeName !== null) {
        setActivateError(errorMessage(e));
        // Force a re-preview: confirm stays disabled (preview === null) until
        // the user sees the diff that reflects the post-failure state.
        setPreviewNonce((n) => n + 1);
      } else {
        pushToast('error', errorMessage(e));
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      className="dialog-overlay"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget && !busy) onClose();
      }}
    >
      <div
        className="dialog-card ai-assets-card"
        role="dialog"
        aria-modal="true"
        aria-label={
          worktreeName !== null ? `Activate ${name} in ${worktreeName}` : `Activate ${name}`
        }
      >
        <div className="shortcut-header">
          <h2 className="dialog-title shortcut-title">
            {worktreeName !== null ? (
              <>
                Activate “{name}” in <span className="mono">{worktreeName}</span>
              </>
            ) : (
              <>Activate “{name}”</>
            )}
          </h2>
          <button
            type="button"
            className="btn-icon shortcut-close"
            aria-label="Close"
            title="Close"
            disabled={busy}
            onClick={onClose}
          >
            {'×'}
          </button>
        </div>

        <p className="settings-section-desc">
          {worktreeName !== null
            ? `Review each target below — files are written inside worktree ${worktreeName}. Confirming writes these files; Cancel writes nothing.`
            : 'Review each target below. Confirming writes these files; Cancel writes nothing.'}
        </p>

        {activateError !== null && (
          <div className="error-banner" role="alert">
            {activateError}
          </div>
        )}

        {loadError !== null ? (
          <div className="error-banner" role="alert">
            {loadError}
          </div>
        ) : preview === null ? (
          <p className="settings-ai-status">Loading preview…</p>
        ) : preview.length === 0 ? (
          <p className="settings-ai-status">This profile has no targets.</p>
        ) : (
          <div className="asset-preview-list">
            {preview.map((entry) => (
              <section className="asset-preview-target" key={entry.assetId}>
                <div className="asset-row-head">
                  <span className="asset-row-path mono">{entry.path}</span>
                  {entry.current === null ? (
                    <span className="asset-chip asset-chip-new">new file</span>
                  ) : entry.changed ? (
                    <span className="asset-chip asset-chip-drifted">changed</span>
                  ) : (
                    <span className="asset-chip asset-chip-sync">unchanged</span>
                  )}
                </div>
                <TextComparePane
                  leftLabel="Current"
                  rightLabel="Proposed"
                  left={entry.current}
                  right={entry.proposed}
                />
              </section>
            ))}
          </div>
        )}

        <div className="dialog-buttons">
          <button type="button" className="btn-secondary" disabled={busy} onClick={onClose}>
            Cancel
          </button>
          <button
            type="button"
            className="btn-primary"
            disabled={busy || preview === null}
            onClick={() => void activate()}
          >
            {busy ? 'Activating…' : 'Activate & write files'}
          </button>
        </div>
      </div>
    </div>
  );
}
