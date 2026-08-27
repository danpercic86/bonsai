import type { PrDiffStats } from '../../ipc';
import { SkeletonRows } from '../CommitPanel';
import { PrFileRow } from './PrFileRow';
import type { PrDiffErrorCause, PrDiffStatus } from './usePrDiff';

// P89 (revised): the PR detail's changed-files section — now a COMPACT list
// only (status badge, path, ±counts). The diff bodies live in the center-pane
// DiffBrowser (pr mode), auto-opened by PrDetailContainer once the local diff
// resolves; clicking any row (or "View diffs") re-opens that browser.
// Presentational — no IPC of its own.

const ERROR_COPY: Record<PrDiffErrorCause, string> = {
  network:
    "Couldn't reach the remote to fetch this pull request. Check your connection and retry.",
  auth: 'Sign-in required to fetch this pull request.',
  unresolved: "Couldn't resolve this pull request's base or head commit.",
  rateLimited: 'Rate limited by the forge. Try again in a moment.',
  generic: "Couldn't compute this pull request's diff.",
};

export interface PrChangesSectionProps {
  status: PrDiffStatus;
  stats: PrDiffStats | null;
  stale: boolean;
  errorCause: PrDiffErrorCause;
  /** Re-run forgePrDiff for this PR. */
  onRetry(): void;
  /** (Re)open the center-pane DiffBrowser on this PR's diff. Undefined while
   *  the local diff has not resolved (rows render non-interactive). */
  onOpenBrowser?(): void;
}

export function PrChangesSection({
  status,
  stats,
  stale,
  errorCause,
  onRetry,
  onOpenBrowser,
}: PrChangesSectionProps) {
  const files = stats?.files ?? [];
  const count = stats?.changedFiles ?? files.length;

  // SF2: on a head-advance refetch the hook keeps the prior stats and sets
  // `stale`. Keep rendering the existing rows underneath, dimmed via
  // `.diff-stale`, instead of collapsing to a skeleton — only the FIRST load
  // (no prior rows) shows the bare skeleton.
  const showStaleRows = status === 'loading' && stale && files.length > 0;
  const fileList = (
    <ul className={`pr-changes-list${stale ? ' diff-stale' : ''}`}>
      {files.map((f) => (
        <PrFileRow key={f.path} header={f} onOpen={onOpenBrowser} />
      ))}
    </ul>
  );

  return (
    <section className="pr-changes" role="region" aria-label="Changed files">
      <div className="pr-changes-head">
        <span className="pr-changes-label">Changed files</span>
        {(status === 'ready' || status === 'empty') && (
          <span className="pr-changes-count">
            {`${count} file${count === 1 ? '' : 's'}`}
          </span>
        )}
        {status === 'ready' && onOpenBrowser !== undefined && (
          <button
            type="button"
            className="section-action pr-changes-view-diffs"
            title="Open all file diffs in the center pane"
            onClick={onOpenBrowser}
          >
            View diffs
          </button>
        )}
        {status === 'error' && (
          <button type="button" className="section-action pr-changes-retry" onClick={onRetry}>
            Retry
          </button>
        )}
      </div>

      {status === 'loading' &&
        (showStaleRows ? (
          <>
            <p className="pane-empty pr-changes-loading">Computing diff…</p>
            {fileList}
          </>
        ) : (
          <>
            <p className="pane-empty pr-changes-loading">Computing diff…</p>
            <div className="skeleton-group" aria-hidden="true">
              <SkeletonRows />
            </div>
          </>
        ))}

      {status === 'error' && (
        <div className="error-banner" role="alert">
          <span className="error-banner-text">{ERROR_COPY[errorCause]}</span>
        </div>
      )}

      {status === 'empty' && (
        <p className="pane-empty">No changes between base and head.</p>
      )}

      {status === 'ready' && fileList}
    </section>
  );
}
