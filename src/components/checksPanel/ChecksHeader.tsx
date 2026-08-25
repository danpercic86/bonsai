// P90: sticky header block (§2.2) — branch + refresh, tip sha + summary, rollup
// pill + freshness. Presentational: requests refresh/reveal, owns no state.
import type { CommitStatus } from '../../ipc';
import type { ChecksTarget } from './checksTarget';
import { ChecksRollupPill } from './ChecksRollupPill';

export interface ChecksHeaderProps {
  target: ChecksTarget;
  /** Non-null only in the loaded state; drives the rollup pill. */
  status: CommitStatus | null;
  refreshing: boolean;
  lastUpdated: number | null;
  /** Epoch-ms of a failed refetch over stale data (§4.10); drives error copy. */
  failedRefreshAt?: number | null;
  onRefresh(): void;
  /** Reveal the tip commit in the graph (reuses the reveal affordance). */
  onRevealCommit?(oid: string): void;
}

function pad2(n: number): string {
  return n < 10 ? `0${n}` : String(n);
}

function freshness(
  refreshing: boolean,
  lastUpdated: number | null,
  failedRefreshAt: number | null,
): string {
  if (refreshing) return 'Checking…';
  if (failedRefreshAt !== null) {
    const d = new Date(failedRefreshAt);
    return `Couldn't refresh — tried ${pad2(d.getHours())}:${pad2(d.getMinutes())}.`;
  }
  if (lastUpdated === null) return 'Never checked';
  const secs = Math.round((Date.now() - lastUpdated) / 1000);
  if (secs < 45) return 'Updated just now';
  const mins = Math.round(secs / 60);
  if (mins < 60) return `Updated ${mins} min ago`;
  const hrs = Math.round(mins / 60);
  return `Updated ${hrs} hr ago`;
}

export function ChecksHeader({
  target,
  status,
  refreshing,
  lastUpdated,
  failedRefreshAt = null,
  onRefresh,
  onRevealCommit,
}: ChecksHeaderProps) {
  const shortSha = target.tip.slice(0, 7);
  return (
    <div className="checks-header">
      <div className="checks-header-row1">
        <span className="checks-branch" title={target.name}>
          {target.name}
        </span>
        <button
          type="button"
          className="btn-icon checks-refresh"
          aria-label="Refresh checks"
          aria-busy={refreshing}
          disabled={refreshing}
          onClick={onRefresh}
        >
          ⟳
        </button>
      </div>
      <div className="checks-header-row2">
        {onRevealCommit !== undefined ? (
          <button
            type="button"
            className="checks-tip-sha link-button"
            onClick={() => onRevealCommit(target.tip)}
            title="Reveal tip commit in the graph"
          >
            {shortSha}
          </button>
        ) : (
          <span className="checks-tip-sha">{shortSha}</span>
        )}
      </div>
      <div className="checks-header-row3">
        {status !== null && <ChecksRollupPill status={status} />}
        <span className="checks-freshness">
          {freshness(refreshing, lastUpdated, failedRefreshAt)}
        </span>
      </div>
    </div>
  );
}
