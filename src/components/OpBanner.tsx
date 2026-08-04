import type { RepoOpState } from '../ipc';

// P3c §8.1 / P3d §8.1: operation-state banner at the top of the right panel.
// Merge mode is actionable (Commit merge / Abort); rebase mode is actionable
// (Continue / Skip / Abort). P20 §8.2: cherry-pick / revert are actionable
// (Continue / Abort — no Skip, single-step ops).

const PICK_REVERT_LABEL = {
  cherryPick: 'Cherry-picking',
  revert: 'Reverting',
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
  /** Cherry-pick / revert mode: finalize the resolved op (P20 §8.2). */
  onOpContinue(): void;
  /** Opens the Abort ConfirmDialog (App owns it) — merge, rebase, pick, revert. */
  onAbort(): void;
}

export function OpBanner({
  op,
  conflictCount,
  mutating,
  onCommitMerge,
  onRebaseContinue,
  onRebaseSkip,
  onOpContinue,
  onAbort,
}: OpBannerProps) {
  if (op.kind === 'none') return null;

  // P39a: the bisect banner arm is implemented in P39b; for now the engine +
  // opstate exist without a dedicated banner (narrows op.kind off 'bisect').
  if (op.kind === 'bisect') return null;

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

  // op.kind is 'cherryPick' | 'revert' here — actionable Continue / Abort.
  return (
    <div className="op-banner" role="status">
      <div className="op-banner-text">
        <span className="op-banner-title">{PICK_REVERT_LABEL[op.kind]}</span>
        <span className="op-banner-sub">
          {conflictCount > 0 ? `${conflictCount} conflict(s) remaining` : 'All conflicts resolved'}
        </span>
      </div>
      <div className="op-banner-actions">
        <button
          type="button"
          className="btn-primary op-banner-btn"
          disabled={conflictCount > 0 || mutating}
          onClick={onOpContinue}
        >
          Continue
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
