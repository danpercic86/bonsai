import type { RepoOpState } from '../ipc';
import { BulkAiResolveButton } from './BulkAiResolveButton';
import type { BulkAiControl } from './repoWorkspace/useBulkAiResolve';
import { shortOid } from './workspaceUtils';

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
  /** Opens the Abort ConfirmDialog (App owns it) — merge, rebase, pick, revert.
   *  P39b: also the bisect Reset confirm. */
  onAbort(): void;
  /** Bisect mode (P39b): mark the current midpoint good (`true`) or bad. */
  onBisectMark(isGood: boolean): void;
  /** Bisect mode (P39b): skip the current (untestable) midpoint. */
  onBisectSkip(): void;
  /** Bisect mode (P39b): oid → commit summary, for the first-bad / current
   *  rows (resolved from the loaded graph; missing oids fall back to shortOid). */
  bisectSummaries?: Record<string, string>;
  /** P68f/OQ4: "Resolve all with AI" — the MERGE arm only, because the banner is
   *  where the user looks while a merge is paused. Rendered only with ≥2 AI-eligible
   *  conflicts, and it becomes `Cancel all` (ONE `ai_cancel_run`) while the run is
   *  live. Absent ⇒ nothing added to the actions row. */
  aiBulk?: BulkAiControl;
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
  onBisectMark,
  onBisectSkip,
  bisectSummaries,
  aiBulk,
}: OpBannerProps) {
  if (op.kind === 'none') return null;

  // P39b: git-bisect banner. Not conflict-driven → ignores conflictCount. Three
  // phases: in-progress (current set, no first-bad) → Good/Bad/Skip/Reset;
  // found (first-bad set) → the culprit + Reset; cannot-determine (current set,
  // firstBad null, no testable revisions) surfaces the same in-progress controls
  // (Reset only path — the toast from the command result explains it).
  if (op.kind === 'bisect') {
    if (op.firstBad !== null) {
      const sum = bisectSummaries?.[op.firstBad];
      return (
        <div className="op-banner" role="status">
          <div className="op-banner-text">
            <span className="op-banner-title">Bisect found first bad commit</span>
            <span className="op-banner-sub">
              {shortOid(op.firstBad)}
              {sum ? ` — ${sum}` : ''}
            </span>
          </div>
          <div className="op-banner-actions">
            <button
              type="button"
              className="btn-danger op-banner-btn"
              disabled={mutating}
              onClick={onAbort}
            >
              Reset
            </button>
          </div>
        </div>
      );
    }
    return (
      <div className="op-banner" role="status">
        <div className="op-banner-text">
          <span className="op-banner-title">Bisecting</span>
          <span className="op-banner-sub">
            {op.revisionsRemaining} revision{op.revisionsRemaining === 1 ? '' : 's'} left, ~
            {op.estimatedSteps} step{op.estimatedSteps === 1 ? '' : 's'}
          </span>
        </div>
        <div className="op-banner-actions">
          <button
            type="button"
            className="btn-primary op-banner-btn"
            disabled={mutating}
            onClick={() => onBisectMark(true)}
          >
            Good
          </button>
          <button
            type="button"
            className="btn-primary op-banner-btn"
            disabled={mutating}
            onClick={() => onBisectMark(false)}
          >
            Bad
          </button>
          <button
            type="button"
            className="btn-secondary op-banner-btn"
            disabled={mutating}
            onClick={onBisectSkip}
          >
            Skip
          </button>
          <button
            type="button"
            className="btn-danger op-banner-btn"
            disabled={mutating}
            onClick={onAbort}
          >
            Reset
          </button>
        </div>
      </div>
    );
  }

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
          {/* P68f: leftmost, because it is the thing that MAKES the merge
              committable; Commit merge stays the primary action beside it. */}
          {aiBulk !== undefined && (
            <BulkAiResolveButton control={aiBulk} variant="banner" busy={mutating} />
          )}
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
