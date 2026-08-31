import { useEffect, useRef } from 'react';
import { useDialogFocus } from '../../hooks/useDialogFocus';

export interface NonFfPullDialogProps {
  open: boolean;
  /** The current (local) branch that diverged. */
  branch: string;
  /** Resolved upstream shorthand ("origin/main") to merge/rebase onto. */
  upstream: string;
  /** Local commits not on upstream / upstream commits not local. */
  ahead: number;
  behind: number;
  /** Drives the action buttons' busy state while a merge/rebase is in flight. */
  busy: boolean;
  onMerge(): void;
  onRebase(): void;
  onCancel(): void;
}

/**
 * P60b: a fast-forward-only pull hit a diverged branch — the backend changed
 * NOTHING (the fetch DID land). This dialog IS the confirm gate: it lets the
 * user reconcile by Merge or Rebase, each routed through the EXISTING
 * `merge_branch` / `rebase_branch` command (with their own autostash, op-state
 * and conflict UX). Cancel is a no-op. Modeled on ConfirmDialog — Esc and
 * overlay-click cancel; purely presentational.
 */
export function NonFfPullDialog({
  open,
  branch,
  upstream,
  ahead,
  behind,
  busy,
  onMerge,
  onRebase,
  onCancel,
}: NonFfPullDialogProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  // Modal focus: move focus into the card on open, trap Tab, restore on close.
  useDialogFocus(open, cardRef, true);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect must not also
      // fire while the dialog is open (matches ConfirmDialog/PromptDialog).
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [open, onCancel]);

  if (!open) return null;

  const plural = (n: number) => (n === 1 ? '' : 's');

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        ref={cardRef}
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label="Fast-forward not possible"
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Fast-forward isn&apos;t possible</h2>
        <div className="dialog-body">
          <div>
            "<span className="mono">{branch}</span>" has diverged from "
            <span className="mono">{upstream}</span>" — {ahead} local commit{plural(ahead)} /{' '}
            {behind} upstream commit{plural(behind)}. Reconcile by:
          </div>
          <div className="dialog-body-note">
            <strong>Merge</strong> — create a merge commit joining both histories.
          </div>
          <div className="dialog-body-note">
            <strong>Rebase</strong> — replay your {ahead} commit{plural(ahead)} on top of{' '}
            {upstream} (rewrites local history).
          </div>
        </div>
        <div className="dialog-buttons">
          <button type="button" className="btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button type="button" className="btn-primary" disabled={busy} onClick={onRebase}>
            Rebase
          </button>
          <button type="button" className="btn-primary" disabled={busy} onClick={onMerge}>
            Merge
          </button>
        </div>
      </div>
    </div>
  );
}
