import { useCallback, useEffect, useState } from 'react';
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
  // Rows start EXPANDED (DiffBrowser-style); an EMPTY set means all expanded.
  const [collapsed, setCollapsed] = useState<Set<string>>(() => new Set());
  const { getEntry, requestFile, retryFile } = fileDiffs;

  const files = stats?.files ?? [];

  // Eagerly fetch every non-binary file's hunks as soon as the list (or the
  // base/head oids behind it) is known — the hook dedupes per cache key and
  // bounds concurrency, so re-runs are cheap no-ops.
  useEffect(() => {
    for (const f of files) {
      if (!f.binary) requestFile(f.path, f.origPath);
    }
  }, [files, requestFile]);

  const toggle = useCallback(
    (path: string) => {
      // No fetch here: the eager effect above already requested every
      // non-binary file, and re-expanding hits the hook's cache.
      setCollapsed((prev) => {
        const next = new Set(prev);
        if (next.has(path)) {
          next.delete(path);
        } else {
          next.add(path);
        }
        return next;
      });
    },
    [],
  );

  const expandable = files.filter((f) => !f.binary);
  const allExpanded = expandable.length > 0 && expandable.every((f) => !collapsed.has(f.path));
  const toggleAll = useCallback(() => {
    if (allExpanded) {
      setCollapsed(new Set(files.map((f) => f.path)));
    } else {
      for (const f of files) {
        if (!f.binary) requestFile(f.path, f.origPath);
      }
      setCollapsed(new Set());
    }
  }, [allExpanded, files, requestFile]);

  const count = stats?.changedFiles ?? files.length;

  // SF2: on a head-advance refetch the hook keeps the prior stats and sets
  // `stale`. Keep rendering the existing rows underneath, dimmed via
  // `.diff-stale`, instead of collapsing to a skeleton — only the FIRST load
  // (no prior rows) shows the bare skeleton.
  const showStaleRows = status === 'loading' && stale && files.length > 0;
  const fileList = (
    <ul className={`pr-changes-list${stale ? ' diff-stale' : ''}`}>
      {files.map((f) => (
        <PrFileRow
          key={f.path}
          header={f}
          entry={f.binary ? undefined : getEntry(f.path)}
          collapsed={collapsed.has(f.path)}
          onToggle={toggle}
          onRetry={retryFile}
        />
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
