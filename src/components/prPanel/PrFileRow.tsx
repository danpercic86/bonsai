import { useId } from 'react';
import type { FileDiffHeader, FileStatus } from '../../ipc';
import { SkeletonRows } from '../CommitPanel';
import { DiffView } from '../DiffView';
import type { PrFileState } from './usePrFileDiffs';

// P89: one changed-file row in the PR detail's changed-files section. A
// collapsible header (chevron + status badge + path + ±counts) that expands in
// place to render the existing DiffView — mirroring DiffBrowser's DiffCard, but
// fed by forgePrFileDiff. Presentational: all fetch state is passed in.
//
// NOTE: we compose DiffView DIRECTLY here rather than extracting DiffBrowser's
// DiffCard/DiffCardBody to a shared file. Those are tightly coupled to
// DiffBrowserSource, the File/Diff view toggle, and DiffImageCard's per-mode
// fetch; extracting them cleanly is a larger refactor than P89 warrants and
// risked growing DiffBrowser.tsx. Composing DiffView reuses the same classes
// (.diff-card / .file-* / .error-banner) with no duplicated fetch logic.

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

export interface PrFileRowProps {
  header: FileDiffHeader;
  /** undefined for binary headers (never fetched) or a not-yet-requested file. */
  entry: PrFileState | undefined;
  collapsed: boolean;
  onToggle(path: string): void;
  onRetry(path: string, origPath: string | null): void;
}

export function PrFileRow({ header, entry, collapsed, onToggle, onRetry }: PrFileRowProps) {
  const bodyId = useId();
  const isRename = header.origPath !== null;
  const title = isRename ? `${header.origPath} → ${header.path}` : header.path;

  return (
    <li className={`diff-card pr-file-row${collapsed ? ' diff-card-collapsed' : ''}`}>
      <button
        type="button"
        className={`diff-card-header file-status-${header.status}`}
        title={title}
        aria-expanded={!collapsed}
        aria-controls={bodyId}
        onClick={() => onToggle(header.path)}
      >
        <span
          className={`file-chevron${collapsed ? '' : ' file-chevron-open'}`}
          aria-hidden="true"
        >
          {'›'}
        </span>
        <span className="file-badge mono">{BADGES[header.status]}</span>
        {isRename ? (
          <span className="diff-card-path mono file-rename">
            {header.origPath} {'→'} {header.path}
          </span>
        ) : (
          <span className="diff-card-path mono">{header.path}</span>
        )}
        <span className="file-counts mono">
          {header.binary ? (
            <span className="file-count-bin">bin</span>
          ) : (
            <>
              <span className="file-count-add">+{header.additions}</span>
              <span className="file-count-del">−{header.deletions}</span>
            </>
          )}
        </span>
      </button>
      {/* Collapsing UNMOUNTS the body (not display:none) so a giant DiffView
          leaves the DOM entirely — same as DiffBrowser's DiffCard. */}
      {!collapsed && (
        <div className="diff-card-body pr-file-body" id={bodyId}>
          <PrFileBody header={header} entry={entry} onRetry={onRetry} />
        </div>
      )}
    </li>
  );
}

function PrFileBody({
  header,
  entry,
  onRetry,
}: {
  header: FileDiffHeader;
  entry: PrFileState | undefined;
  onRetry(path: string, origPath: string | null): void;
}) {
  // Binary files are not fetched; DiffView renders its own binary placeholder.
  if (header.binary) return <div className="diff-placeholder">Binary file</div>;
  if (entry === undefined || entry.state === 'idle' || entry.state === 'loading') {
    return (
      <div className="diff-card-loading skeleton-group" aria-hidden="true">
        <SkeletonRows />
      </div>
    );
  }
  if (entry.state === 'error') {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{"Couldn't load this file's diff."}</span>
        <button
          type="button"
          className="section-action"
          onClick={() => onRetry(header.path, header.origPath)}
        >
          Retry
        </button>
      </div>
    );
  }
  return <DiffView diff={entry.diff} viewMode="diff" />;
}
