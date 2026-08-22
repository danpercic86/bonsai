import { useEffect, useRef } from 'react';
import type { UndoKind, UndoPlan } from '../../ipc';
import { useDialogFocus } from '../../hooks/useDialogFocus';

export interface UndoDialogProps {
  /** The plan to reverse the last op, or null when the dialog is closed. */
  plan: UndoPlan | null;
  /** Drives the confirm button's busy state while the reset is in flight. */
  busy: boolean;
  onConfirm(): void;
  onCancel(): void;
}

/** Human label for the undo verb in the title. */
const KIND_LABEL: Record<UndoKind, string> = {
  commit: 'commit',
  amend: 'amend',
  merge: 'merge',
  rebase: 'rebase',
  fastForward: 'fast-forward',
  cherryPick: 'cherry-pick',
  revert: 'revert',
  reset: 'reset',
  branchSwitch: 'branch switch',
  unknown: 'last operation',
};

/**
 * P60c: confirm dialog for one-click undo. Presentational — the plan is
 * computed by the READ-ONLY `describeLastUndo`; confirming dispatches the
 * shipped `resetBranch(targetOid, resetMode)`. The Undo button is DISABLED when
 * the op isn't undoable (shows `reason`) or when a hard-reset undo would clobber
 * a dirty worktree (`requiresCleanWorktree && worktreeDirty` → "stash first").
 * Hard classes carry the destructive (danger) styling. Modeled on
 * ConfirmDialog/NonFfPullDialog — Esc and overlay-click cancel.
 */
export function UndoDialog({ plan, busy, onConfirm, onCancel }: UndoDialogProps) {
  const cardRef = useRef<HTMLDivElement>(null);
  // Modal focus: move focus into the card on open, trap Tab, restore on close.
  useDialogFocus(plan !== null, cardRef, true);

  useEffect(() => {
    if (plan === null) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      // Capture phase + stopPropagation: App's global Esc-deselect must not also
      // fire while the dialog is open (matches ConfirmDialog/NonFfPullDialog).
      e.stopPropagation();
      onCancel();
    };
    window.addEventListener('keydown', onKeyDown, true);
    return () => window.removeEventListener('keydown', onKeyDown, true);
  }, [plan, onCancel]);

  if (plan === null) return null;

  const isHard = plan.resetMode === 'hard';
  const dirtyBlocked = plan.requiresCleanWorktree && plan.worktreeDirty;
  const blocked = !plan.undoable || dirtyBlocked;
  // The disabled-button explanation: the plan's own reason when not undoable,
  // else the stash-first note for a hard-reset undo on a dirty worktree.
  const blockReason = !plan.undoable
    ? (plan.reason ?? 'This operation can’t be undone.')
    : dirtyBlocked
      ? 'Commit or stash your changes first.'
      : null;

  return (
    <div className="dialog-overlay" onClick={onCancel}>
      <div
        ref={cardRef}
        className="dialog-card"
        role="dialog"
        aria-modal="true"
        aria-label={`Undo ${KIND_LABEL[plan.kind]}`}
        tabIndex={-1}
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="dialog-title">Undo {KIND_LABEL[plan.kind]}?</h2>
        <div className="dialog-body">
          {plan.undoable ? (
            <>
              <div>
                {plan.summary !== '' && (
                  <>
                    &ldquo;<span className="mono">{plan.summary}</span>&rdquo;.{' '}
                  </>
                )}
                This will{' '}
                {isHard
                  ? 'reset your branch and working tree to'
                  : 'move your branch back to'}{' '}
                <span className="mono">{plan.targetShort}</span>.
              </div>
              {isHard && (
                <div className="dialog-body-note">
                  A hard reset permanently discards uncommitted changes to tracked files.
                </div>
              )}
              {plan.kind === 'amend' && (
                <div className="dialog-body-note">
                  The amended commit message is discarded; its changes return to your working tree.
                </div>
              )}
              {dirtyBlocked && <div className="dialog-body-note">{blockReason}</div>}
            </>
          ) : (
            <div>{blockReason}</div>
          )}
        </div>
        <div className="dialog-buttons">
          <button type="button" className="btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className={isHard ? 'btn-danger' : 'btn-primary'}
            disabled={busy || blocked}
            title={blocked ? (blockReason ?? undefined) : undefined}
            onClick={onConfirm}
          >
            Undo
          </button>
        </div>
      </div>
    </div>
  );
}
