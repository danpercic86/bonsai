import { useCallback, useState } from 'react';
import type { PrDiffStats } from '../../ipc';
import { SkeletonRows } from '../CommitPanel';
import { PrFileRow } from './PrFileRow';
import type { PrDiffErrorCause, PrDiffStatus } from './usePrDiff';
import type { UsePrFileDiffs } from './usePrFileDiffs';

// P89: the PR detail's changed-files section (contract §1/§4). A small state
// machine over the forgePrDiff result: loading skeleton, empty, error+retry, or
// the <ul> of expandable file rows. Presentational — no IPC of its own; the
// per-file fetch orchestration is passed in via `fileDiffs`.

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
  fileDiffs: UsePrFileDiffs;
}

export function PrChangesSection({
  status,
  stats,
  stale,
  errorCause,
  onRetry,
  fileDiffs,
}: PrChangesSectionProps) {
  // Rows start collapsed (lazy per-file fetch); an EMPTY set means all collapsed.
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const { getEntry, requestFile, retryFile } = fileDiffs;

  const files = stats?.files ?? [];

  const toggle = useCallback(
    (path: string) => {
      setExpanded((prev) => {
        const next = new Set(prev);
        if (next.has(path)) {
          next.delete(path);
        } else {
          next.add(path);
          const header = files.find((f) => f.path === path);
          if (header !== undefined && !header.binary) requestFile(path, header.origPath);
        }
        return next;
      });
    },
    [files, requestFile],
  );

  const expandable = files.filter((f) => !f.binary);
  const allExpanded = expandable.length > 0 && expandable.every((f) => expanded.has(f.path));
  const toggleAll = useCallback(() => {
    if (allExpanded) {
      setExpanded(new Set());
    } else {
      const next = new Set<string>();
      for (const f of files) {
        next.add(f.path);
        if (!f.binary) requestFile(f.path, f.origPath);
      }
      setExpanded(next);
    }
  }, [allExpanded, files, requestFile]);

  const count = stats?.changedFiles ?? files.length;

  return (
    <section className="pr-changes" role="region" aria-label="Changed files">
      <div className="pr-changes-head">
        <span className="pr-changes-label">Changed files</span>
        {(status === 'ready' || status === 'empty') && (
          <span className="pr-changes-count">
            {`${count} file${count === 1 ? '' : 's'}`}
          </span>
        )}
        {status === 'ready' && expandable.length > 0 && (
          <button
            type="button"
            className="section-action pr-changes-collapse-all"
            title={allExpanded ? 'Collapse all' : 'Expand all'}
            onClick={toggleAll}
          >
            {allExpanded ? 'Collapse all' : 'Expand all'}
          </button>
        )}
        {status === 'error' && (
          <button type="button" className="section-action pr-changes-retry" onClick={onRetry}>
            Retry
          </button>
        )}
      </div>

      {status === 'loading' && (
        <>
          <p className="pane-empty pr-changes-loading">Computing diff…</p>
          <div className={`skeleton-group${stale ? ' diff-stale' : ''}`} aria-hidden="true">
            <SkeletonRows />
          </div>
        </>
      )}

      {status === 'error' && (
        <div className="error-banner" role="alert">
          <span className="error-banner-text">{ERROR_COPY[errorCause]}</span>
        </div>
      )}

      {status === 'empty' && (
        <p className="pane-empty">No changes between base and head.</p>
      )}

      {status === 'ready' && (
        <ul className={`pr-changes-list${stale ? ' diff-stale' : ''}`}>
          {files.map((f) => (
            <PrFileRow
              key={f.path}
              header={f}
              entry={f.binary ? undefined : getEntry(f.path)}
              collapsed={!expanded.has(f.path)}
              onToggle={toggle}
              onRetry={retryFile}
            />
          ))}
        </ul>
      )}
    </section>
  );
}
