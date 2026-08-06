import type { CopyAction, CopyCandidate, CopyGroup, CopyVerdict } from '../ipc';

export interface WorktreeCopyCandidatesProps {
  candidates: CopyCandidate[];
  loading: boolean;
  error: string | null;
  /** Paths currently checked to copy (keyed by path → auto-dedupes a file that
   *  appears in two groups). */
  checked: Set<string>;
  /** Conflict verdict per checked path, from `previewWorktreeCopy`. */
  verdictByPath: Map<string, CopyVerdict>;
  /** The last conflict preview failed → every checked path has an UNKNOWN
   *  verdict and is treated like a conflict (explicit decision, default Skip). */
  previewFailed: boolean;
  /** Overwrite/Skip decision per path needing a decision (default Skip). */
  conflictActions: Record<string, CopyAction>;
  disabled: boolean;
  onToggle(path: string): void;
  /** Bulk check/uncheck every path in a group (Check all ⇄ Uncheck all). */
  onToggleGroup(paths: string[], check: boolean): void;
  onSetAction(path: string, action: CopyAction): void;
}

/** Display order + human labels for the four candidate groups. */
const GROUPS: { group: CopyGroup; label: string }[] = [
  { group: 'staged', label: 'Staged' },
  { group: 'unstaged', label: 'Unstaged' },
  { group: 'untracked', label: 'Untracked' },
  { group: 'ignored', label: 'Gitignored' },
];

/**
 * Presentational candidate list for the new-worktree dialog (P32 Part B).
 * Renders the copy-eligible files grouped Staged / Unstaged / Untracked /
 * Gitignored, each a checkbox (default unchecked). A checked file the target
 * branch also changed gets a `conflict` chip + an Overwrite/Skip toggle. All
 * state lives in the parent dialog; this component only renders + reports.
 */
export function WorktreeCopyCandidates({
  candidates,
  loading,
  error,
  checked,
  verdictByPath,
  previewFailed,
  conflictActions,
  disabled,
  onToggle,
  onToggleGroup,
  onSetAction,
}: WorktreeCopyCandidatesProps) {
  if (loading) {
    return <p className="dialog-body-note">Loading uncommitted files…</p>;
  }
  if (error !== null) {
    return <p className="dialog-error">{error}</p>;
  }
  if (candidates.length === 0) return null;

  return (
    <div className="wt-copy-list">
      <p className="dialog-body-note">Copy uncommitted files into the new worktree:</p>
      {GROUPS.map(({ group, label }) => {
        const rows = candidates.filter((c) => c.group === group);
        if (rows.length === 0) return null;
        // Gitignored is intentionally excluded from bulk-select (copying every
        // ignored file — build output, node_modules — is rarely intended).
        const groupPaths = rows.map((r) => r.path);
        const allChecked = groupPaths.every((p) => checked.has(p));
        return (
          <div key={group} className="wt-copy-group">
            <div className="wt-copy-group-header">
              <span>{label}</span>
              {group !== 'ignored' && (
                <button
                  type="button"
                  className="wt-copy-selectall"
                  disabled={disabled}
                  onClick={() => onToggleGroup(groupPaths, !allChecked)}
                >
                  {allChecked ? 'Uncheck all' : 'Check all'}
                </button>
              )}
            </div>
            {rows.map((c) => {
              const isChecked = checked.has(c.path);
              const isConflict = verdictByPath.get(c.path) === 'conflict';
              // A checked path needs an explicit decision when it conflicts OR
              // its verdict is unknown (preview failed) — both default to Skip.
              const needsDecision = isChecked && (isConflict || previewFailed);
              const action = conflictActions[c.path] ?? 'skip';
              return (
                <div key={`${group}:${c.path}`} className="wt-copy-row">
                  <label className="dialog-checkbox-label wt-copy-check">
                    <input
                      type="checkbox"
                      checked={isChecked}
                      disabled={disabled}
                      onChange={() => onToggle(c.path)}
                    />
                    <span className="mono wt-copy-path" title={c.path}>
                      {c.path}
                    </span>
                  </label>
                  {needsDecision && (
                    <div className="wt-copy-conflict">
                      <span className="wt-copy-chip">{isConflict ? 'conflict' : 'unchecked'}</span>
                      <div className="wt-copy-toggle">
                        <button
                          type="button"
                          className={
                            action === 'copy' ? 'wt-copy-toggle-on' : 'wt-copy-toggle-off'
                          }
                          disabled={disabled}
                          onClick={() => onSetAction(c.path, 'copy')}
                        >
                          Overwrite
                        </button>
                        <button
                          type="button"
                          className={
                            action === 'skip' ? 'wt-copy-toggle-on' : 'wt-copy-toggle-off'
                          }
                          disabled={disabled}
                          onClick={() => onSetAction(c.path, 'skip')}
                        >
                          Skip
                        </button>
                      </div>
                    </div>
                  )}
                </div>
              );
            })}
          </div>
        );
      })}
    </div>
  );
}
