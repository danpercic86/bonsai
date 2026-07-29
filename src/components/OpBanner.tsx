import type { RepoOpState } from '../ipc';

// P3c §8.1 / P3d §8.1: operation-state banner at the top of the right panel.
// Merge mode is actionable (Commit merge / Abort); rebase mode is actionable
// (Continue / Skip / Abort). Externally-started cherry-pick / revert render an
// informational strip so the UI never shows a false-clean state.

const EXTERNAL_OP_LABEL = {
  cherryPick: 'cherry-pick',
  revert: 'revert',
} as const;

export interface OpBannerProps {
  op: RepoOpState;
  /** Remaining conflicts — drives Commit-merge / Continue enablement. */
  conflictCount: number;
  mutating: boolean;
  /** Triggers the merge-commit flow (CommitBox submit path, §8.4). */
  onCommitMerge(): void;
  /** Rebase mode: commit the resolved op and replay on. */
  onRebaseContinue(): void;
  /** Rebase mode: drop the current op and replay on. */
  onRebaseSkip(): void;
  /** Opens the Abort ConfirmDialog (App owns it) — merge & rebase. */
  onAbort(): void;
}

export function OpBanner({
  op,
  conflictCount,
  mutating,
  onCommitMerge,
  onRebaseContinue,
  onRebaseSkip,
  onAbort,
}: OpBannerProps) {
  if (op.kind === 'none') return null;

  if (op.kind === 'merge') {
    return (
      <div className="op-banner" role="status">
        <div className="op-banner-text">
          <span className="op-banner-title">Merging {op.incoming}</span>
          <span className="op-banner-sub">
            {conflictCount > 0 ? `${conflictCount} conflict(s) remaining` : 'All conflicts resolved'}
          </span>
        </div>
        <div className="op-banner-actions">
          <button
            type="button"
            className="btn-primary op-banner-btn"
            disabled={conflictCount > 0 || mutating}
            onClick={onCommitMerge}
          >
            Commit merge
          </button>
          <button
            type="button"
            className="btn-danger op-banner-btn"
            disabled={mutating}
            onClick={onAbort}
          >
            Abort
          </button>
        </div>
      </div>
    );
  }

  if (op.kind === 'rebase') {
    const stepFragment = op.totalSteps === 0 ? '' : `step ${op.currentStep}/${op.totalSteps} — `;
    const conflictFragment =
      conflictCount > 0 ? `${conflictCount} conflict(s) remaining` : 'all conflicts resolved';
    return (
      <div className="op-banner" role="status">
        <div className="op-banner-text">
          <span className="op-banner-title">Rebasing {op.headName ?? 'HEAD'}</span>
          <span className="op-banner-sub">
            {stepFragment}
            {conflictFragment}
          </span>
        </div>
        <div className="op-banner-actions">
          <button
            type="button"
            className="btn-primary op-banner-btn"
            disabled={conflictCount > 0 || mutating}
            onClick={onRebaseContinue}
          >
            Continue
          </button>
          <button
            type="button"
            className="btn-secondary op-banner-btn"
            disabled={mutating}
            onClick={onRebaseSkip}
          >
            Skip
          </button>
          <button
            type="button"
            className="btn-danger op-banner-btn"
            disabled={mutating}
            onClick={onAbort}
          >
            Abort
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="op-banner" role="status">
      <div className="op-banner-text">
        <span className="op-banner-title">
          A {EXTERNAL_OP_LABEL[op.kind]} is in progress — finish or abort it in your terminal.
        </span>
      </div>
    </div>
  );
}
