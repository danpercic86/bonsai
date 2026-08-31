import { useRef, useState } from 'react';
import type { BranchNameProposal } from '../ipc';
import { SummarizeIcon } from './menuIcons';
import { errorMessage } from '../utils/errors';

export interface BranchNameSuggestProps {
  /** AI installed && enabled && consented (UX gate; the backend re-checks). */
  aiEligible: boolean;
  /** Worktree has staged/unstaged/untracked changes — the grounding to name
   *  from. A clean tree has nothing to describe, so the button is disabled (OQ6). */
  workingDirty: boolean;
  /** Fill the branch-name input with the chosen candidate. */
  onPick(name: string): void;
  /** Fetch ranked candidates (read-only; WRITES NOTHING). */
  suggest(): Promise<BranchNameProposal>;
}

/**
 * "Suggest name ✨" affordance for the branch-create dialog (P53c §5.3). Renders
 * a button (enabled only when `aiEligible && workingDirty`); on click it calls
 * `suggest()`, shows an inline spinner, then a row of candidate chips. Clicking a
 * chip calls `onPick(name)` to fill the dialog's name input — the actual branch
 * is still created by the existing confirmed create path. A last-wins req-id
 * guard drops a stale/superseded response. Purely presentational: it owns no IPC
 * (the container binds `suggest`).
 */
export function BranchNameSuggest({ aiEligible, workingDirty, onPick, suggest }: BranchNameSuggestProps) {
  const [loading, setLoading] = useState(false);
  const [names, setNames] = useState<string[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  // Monotonic id: only the most recent request's result is applied.
  const reqIdRef = useRef(0);

  const enabled = aiEligible && workingDirty;

  const onClick = () => {
    const reqId = ++reqIdRef.current;
    setLoading(true);
    setError(null);
    setNames(null);
    suggest().then(
      (proposal) => {
        if (reqId !== reqIdRef.current) return; // superseded
        setLoading(false);
        setNames(proposal.names);
      },
      (err: unknown) => {
        if (reqId !== reqIdRef.current) return; // superseded
        setLoading(false);
        setError(errorMessage(err));
      },
    );
  };

  const title = !aiEligible
    ? 'Enable AI in Settings to suggest branch names'
    : !workingDirty
      ? 'Make some changes first — there is nothing to name a branch from'
      : 'Suggest branch names from your working changes';

  return (
    <div className="branch-name-suggest">
      <button
        type="button"
        className="btn-secondary branch-name-suggest-btn"
        disabled={!enabled || loading}
        title={title}
        onClick={onClick}
      >
        {loading ? (
          'Suggesting…'
        ) : (
          <>
            <span>Suggest name</span>
            <SummarizeIcon />
          </>
        )}
      </button>
      {error !== null && <p className="branch-name-suggest-error">{error}</p>}
      {names !== null && names.length > 0 && (
        <div className="branch-name-suggest-chips">
          {names.map((name) => (
            <button
              key={name}
              type="button"
              className="branch-name-chip"
              title={`Use "${name}"`}
              onClick={() => onPick(name)}
            >
              {name}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
