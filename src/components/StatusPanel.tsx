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

/** Rename expansion (M3 contract §2.1): send BOTH sides of a rename. */
function entryPaths(e: StatusEntry): string[] {
  return e.origPath !== null ? [e.origPath, e.path] : [e.path];
}

function splitPath(path: string): { dir: string | null; name: string } {
  const idx = path.lastIndexOf('/');
  if (idx === -1) return { dir: null, name: path };
  return { dir: path.slice(0, idx + 1), name: path.slice(idx + 1) };
}

type RowAction = 'stage' | 'unstage' | null;

function FileRow({
  entry,
  action,
  disabled,
  onAction,
}: {
  entry: StatusEntry;
  /** Which button the row shows; null = no button (conflicted rows). */
  action: RowAction;
  disabled: boolean;
  onAction: (paths: string[]) => void;
}) {
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
      {action !== null && (
        <button
          type="button"
          className="row-action"
          aria-label={`${action === 'stage' ? 'Stage' : 'Unstage'} ${entry.path}`}
          disabled={disabled}
          onClick={() => onAction(entryPaths(entry))}
        >
          {action === 'stage' ? '+' : '−'}
        </button>
      )}
    </li>
  );
}

function Section({
  label,
  entries,
  danger = false,
  rowAction,
  actionLabel,
  disabled,
  onAction,
}: {
  label: string;
  entries: StatusEntry[];
  danger?: boolean;
  /** Per-row button kind; null = no actions in this section. */
  rowAction: RowAction;
  /** Section-header bulk button label ("Stage all" / "Unstage all"). */
  actionLabel: string | null;
  disabled: boolean;
  onAction: (paths: string[]) => void;
}) {
  return (
    <section className="status-section">
      <div
        className={
          danger ? 'section-header section-label section-label-danger' : 'section-header section-label'
        }
      >
        <span>
          {label} ({entries.length})
        </span>
        {actionLabel !== null && entries.length > 0 && (
          <button
            type="button"
            className="section-action"
            disabled={disabled}
            onClick={() => onAction(entries.flatMap(entryPaths))}
          >
            {actionLabel}
          </button>
        )}
      </div>
      <ul className="file-list">
        {entries.map((entry) => (
          <FileRow
            key={`${entry.status}:${entry.path}`}
            entry={entry}
            action={rowAction}
            disabled={disabled}
            onAction={onAction}
          />
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
  /** True while any stage/unstage/commit is in flight — disables all action buttons. */
  busy: boolean;
  onStage(paths: string[]): void;
  onUnstage(paths: string[]): void;
}

/** Pure presentational right-panel status view; all fetching lives in App. */
export function StatusPanel({ snapshot, loading, error, busy, onStage, onUnstage }: StatusPanelProps) {
  const [dismissedError, setDismissedError] = useState<string | null>(null);
  const visibleError = error !== null && error !== dismissedError ? error : null;

  const disabled = busy || loading;

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
          <Section
            label="Staged"
            entries={snapshot.staged}
            rowAction="unstage"
            actionLabel="Unstage all"
            disabled={disabled}
            onAction={onUnstage}
          />
          <Section
            label="Unstaged"
            entries={snapshot.unstaged}
            rowAction="stage"
            actionLabel="Stage all"
            disabled={disabled}
            onAction={onStage}
          />
          <Section
            label="Untracked"
            entries={snapshot.untracked}
            rowAction="stage"
            actionLabel="Stage all"
            disabled={disabled}
            onAction={onStage}
          />
          {snapshot.conflicted.length > 0 && (
            <Section
              label="Conflicts"
              entries={snapshot.conflicted}
              danger
              rowAction={null}
              actionLabel={null}
              disabled={disabled}
              onAction={() => {}}
            />
          )}
        </>
      )}
    </div>
  );
}
