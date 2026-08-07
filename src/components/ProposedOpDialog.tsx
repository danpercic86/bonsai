import { ConfirmDialog } from './ConfirmDialog';
import type { DangerLevel, ProposedOperation } from '../ipc';

export interface ProposedOpDialogProps {
  open: boolean;
  /** The resolved proposal to preview; null renders nothing (dialog closed). */
  operation: ProposedOperation | null;
  /** True while the confirmed op is being dispatched (disables Confirm). */
  busy: boolean;
  onConfirm(): void;
  onCancel(): void;
}

const DANGER_LABEL: Record<DangerLevel, string> = {
  safe: 'Safe',
  caution: 'Caution',
  destructive: 'Destructive',
};

/**
 * P55c: preview + confirm gate for an AI-proposed SAFE git operation (contract
 * §9, safety layer L6). PRESENTATIONAL — it touches no IPC; the parent dispatches
 * the resolved op via `safeOpDispatch` only AFTER this dialog's Confirm. Built on
 * the shared `ConfirmDialog` so the invariant holds (initial focus = Cancel; a
 * stray Enter never confirms). The confirm button is `primary` for a Safe op and
 * `danger` otherwise; nothing executes until it is clicked.
 */
export function ProposedOpDialog({
  open,
  operation,
  busy,
  onConfirm,
  onCancel,
}: ProposedOpDialogProps) {
  const preview = operation?.preview ?? null;

  return (
    <ConfirmDialog
      open={open && preview !== null}
      title={preview?.title ?? ''}
      confirmLabel={preview?.confirmLabel ?? 'Confirm'}
      confirmVariant={preview?.danger === 'safe' ? 'primary' : 'danger'}
      busy={busy}
      onConfirm={onConfirm}
      onCancel={onCancel}
    >
      {operation !== null && preview !== null && (
        <div className="proposed-op-dialog">
          <span className={`danger-badge ${preview.danger}`}>{DANGER_LABEL[preview.danger]}</span>

          <p className="proposed-op-summary">{preview.summary}</p>

          {preview.refChanges.length > 0 && (
            <ul className="op-ref-change-list">
              {preview.refChanges.map((rc) => (
                <li key={rc.name} className="op-ref-change">
                  <span className="op-ref-name">{rc.name}</span>
                  <span className="mono">{rc.fromShort}</span>
                  <span className="op-ref-arrow" aria-hidden="true">
                    →
                  </span>
                  <span className="mono">{rc.toShort}</span>
                </li>
              ))}
            </ul>
          )}

          {preview.droppedCommits.length > 0 && (
            <div className="op-dropped">
              <div className="op-section-label">
                {preview.droppedCommits.length === 1
                  ? '1 commit leaves the branch'
                  : `${preview.droppedCommits.length} commits leave the branch`}
              </div>
              <ul className="op-dropped-list">
                {preview.droppedCommits.map((c) => (
                  <li key={c.short}>
                    <span className="mono">{c.short}</span> {c.summary}
                  </li>
                ))}
              </ul>
            </div>
          )}

          {preview.addedCommits > 0 && (
            <p className="op-added">
              Adds {preview.addedCommits} new commit{preview.addedCommits === 1 ? '' : 's'}.
            </p>
          )}

          {preview.worktreeWarning !== null && (
            <p className="op-worktree-warning">{preview.worktreeWarning}</p>
          )}

          {operation.rationale !== '' && <p className="op-rationale">{operation.rationale}</p>}
        </div>
      )}
    </ConfirmDialog>
  );
}
