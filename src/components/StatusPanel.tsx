import { useState } from 'react';
import type { FileStatus, StatusEntry, StatusSnapshot } from '../ipc';

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'U',
  conflicted: 'C',
};

function splitPath(path: string): { dir: string | null; name: string } {
  const idx = path.lastIndexOf('/');
  if (idx === -1) return { dir: null, name: path };
  return { dir: path.slice(0, idx + 1), name: path.slice(idx + 1) };
}

function FileRow({ entry }: { entry: StatusEntry }) {
  const isRename = entry.origPath !== null;
  const title = isRename ? `${entry.origPath} → ${entry.path}` : entry.path;
  const { dir, name } = splitPath(entry.path);
  return (
    <li className={`file-row file-status-${entry.status}`} title={title}>
      <span className="file-badge mono">{BADGES[entry.status]}</span>
      {isRename ? (
        <span className="file-path mono file-rename">
          {entry.origPath} {'→'} {entry.path}
        </span>
      ) : (
        <span className="file-path">
          {dir !== null && <span className="file-dir">{dir}</span>}
          <span className="file-name">{name}</span>
        </span>
      )}
    </li>
  );
}

function Section({
  label,
  entries,
  danger = false,
}: {
  label: string;
  entries: StatusEntry[];
  danger?: boolean;
}) {
  return (
    <section className="status-section">
      <div className={danger ? 'section-label section-label-danger' : 'section-label'}>
        {label} ({entries.length})
      </div>
      <ul className="file-list">
        {entries.map((entry) => (
          <FileRow key={`${entry.status}:${entry.path}`} entry={entry} />
        ))}
      </ul>
    </section>
  );
}

function SkeletonRows() {
  return (
    <div aria-hidden="true">
      {Array.from({ length: 6 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

export interface StatusPanelProps {
  snapshot: StatusSnapshot | null;
  loading: boolean;
  error: string | null;
}

/** Pure presentational right-panel status view; all fetching lives in App. */
export function StatusPanel({ snapshot, loading, error }: StatusPanelProps) {
  const [dismissedError, setDismissedError] = useState<string | null>(null);
  const visibleError = error !== null && error !== dismissedError ? error : null;

  const isEmpty =
    snapshot !== null &&
    snapshot.staged.length === 0 &&
    snapshot.unstaged.length === 0 &&
    snapshot.untracked.length === 0 &&
    snapshot.conflicted.length === 0;

  return (
    <div className={isEmpty ? 'status-panel status-panel-empty' : 'status-panel'}>
      {visibleError !== null && (
        <div className="error-banner error-banner-dismissible" role="alert">
          <span className="error-banner-text">{visibleError}</span>
          <button
            type="button"
            className="error-dismiss"
            aria-label="Dismiss error"
            onClick={() => setDismissedError(visibleError)}
          >
            {'×'}
          </button>
        </div>
      )}
      {snapshot === null ? (
        // Skeletons only before the first snapshot; refreshes keep showing the
        // previous snapshot (no flicker).
        loading && <SkeletonRows />
      ) : isEmpty ? (
        <p className="pane-empty">No changes</p>
      ) : (
        <>
          <Section label="Staged" entries={snapshot.staged} />
          <Section label="Unstaged" entries={snapshot.unstaged} />
          <Section label="Untracked" entries={snapshot.untracked} />
          {snapshot.conflicted.length > 0 && (
            <Section label="Conflicts" entries={snapshot.conflicted} danger />
          )}
        </>
      )}
    </div>
  );
}
