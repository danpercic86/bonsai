import type { RepoOpState } from '../ipc';

// P3c §8.1: operation-state banner at the top of the right panel. Merge mode
// is actionable (Commit merge / Abort); externally-started rebase/cherry-pick/
// revert render an informational strip so the UI never shows a false-clean
// state (P3d makes rebase actionable).

const EXTERNAL_OP_LABEL = {
  rebase: 'rebase',
  cherryPick: 'cherry-pick',
  revert: 'revert',
} as const;

export interface OpBannerProps {
  op: RepoOpState;
  /** Remaining conflicts — drives Commit-merge enablement. */
  conflictCount: number;
  mutating: boolean;
  /** Triggers the merge-commit flow (CommitBox submit path, §8.4). */
  onCommitMerge(): void;
  /** Opens the Abort-merge ConfirmDialog (App owns it). */
  onAbort(): void;
}

export function OpBanner({ op, conflictCount, mutating, onCommitMerge, onAbort }: OpBannerProps) {
  if (op.kind === 'none') return null;

  if (op.kind !== 'merge') {
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
