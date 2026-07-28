import { Fragment, useState } from 'react';
import type { FileStatus, StatusEntry, StatusSnapshot } from '../ipc';
import { DiffSlotView } from './DiffView';
import type { DiffSlot } from './DiffView';

export type { DiffSlot } from './DiffView';

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

export type WorkdirSection = 'staged' | 'unstaged' | 'untracked';

function FileRow({
  entry,
  action,
  disabled,
  expandable,
  expanded,
  onAction,
  onToggle,
}: {
  entry: StatusEntry;
  /** Which button the row shows; null = no button (conflicted rows). */
  action: RowAction;
  disabled: boolean;
  /** Conflicted rows are not expandable (no diff kind for conflicts in v1). */
  expandable: boolean;
  expanded: boolean;
  onAction: (paths: string[]) => void;
  onToggle: () => void;
}) {
  const isRename = entry.origPath !== null;
  const title = isRename ? `${entry.origPath} → ${entry.path}` : entry.path;
  const { dir, name } = splitPath(entry.path);
  const pathEl = isRename ? (
    <span className="file-path mono file-rename">
      {entry.origPath} {'→'} {entry.path}
    </span>
  ) : (
    <span className="file-path">
      {dir !== null && <span className="file-dir">{dir}</span>}
      <span className="file-name">{name}</span>
    </span>
  );
  return (
    <li
      className={`file-row file-status-${entry.status}${expanded ? ' file-row-expanded' : ''}`}
      title={title}
    >
      {expandable ? (
        <button
          type="button"
          className="file-row-main"
          aria-expanded={expanded}
          onClick={onToggle}
        >
          <span className={`file-chevron${expanded ? ' file-chevron-open' : ''}`}>{'›'}</span>
          <span className="file-badge mono">{BADGES[entry.status]}</span>
          {pathEl}
        </button>
      ) : (
        <span className="file-row-main">
          <span className="file-badge mono">{BADGES[entry.status]}</span>
          {pathEl}
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
  section,
  entries,
  danger = false,
  rowAction,
  actionLabel,
  disabled,
  expandable,
  diffSlot,
  onAction,
  onToggleDiff,
}: {
  label: string;
  /** Diff-key prefix; null for the conflicts section (not expandable). */
  section: WorkdirSection | null;
  entries: StatusEntry[];
  danger?: boolean;
  /** Per-row button kind; null = no actions in this section. */
  rowAction: RowAction;
  /** Section-header bulk button label ("Stage all" / "Unstage all"). */
  actionLabel: string | null;
  disabled: boolean;
  expandable: boolean;
  diffSlot: DiffSlot | null;
  onAction: (paths: string[]) => void;
  onToggleDiff: (section: WorkdirSection, entry: StatusEntry) => void;
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
        {entries.map((entry) => {
          const key = section !== null ? `${section}:${entry.path}` : null;
          const expanded = key !== null && diffSlot !== null && diffSlot.key === key;
          return (
            <Fragment key={`${entry.status}:${entry.path}`}>
              <FileRow
                entry={entry}
                action={rowAction}
                disabled={disabled}
                expandable={expandable && section !== null}
                expanded={expanded}
                onAction={onAction}
                onToggle={() => {
                  if (section !== null) onToggleDiff(section, entry);
                }}
              />
              {expanded && diffSlot !== null && (
                <li className="diff-expansion">
                  <DiffSlotView
                    slot={diffSlot}
                    onDismissError={() => {
                      if (section !== null) onToggleDiff(section, entry);
                    }}
                  />
                </li>
              )}
            </Fragment>
          );
        })}
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
  /** Currently expanded diff (null = none). Single expansion across ALL sections. */
  diffSlot: DiffSlot | null;
  onStage(paths: string[]): void;
  onUnstage(paths: string[]): void;
  /** Toggle inline diff expansion for a row (App owns the fetch). */
  onToggleDiff(section: WorkdirSection, entry: StatusEntry): void;
}

/** Pure presentational right-panel status view; all fetching lives in App. */
export function StatusPanel({
  snapshot,
  loading,
  error,
  busy,
  diffSlot,
  onStage,
  onUnstage,
  onToggleDiff,
}: StatusPanelProps) {
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
            section="staged"
            entries={snapshot.staged}
            rowAction="unstage"
            actionLabel="Unstage all"
            disabled={disabled}
            expandable
            diffSlot={diffSlot}
            onAction={onUnstage}
            onToggleDiff={onToggleDiff}
          />
          <Section
            label="Unstaged"
            section="unstaged"
            entries={snapshot.unstaged}
            rowAction="stage"
            actionLabel="Stage all"
            disabled={disabled}
            expandable
            diffSlot={diffSlot}
            onAction={onStage}
            onToggleDiff={onToggleDiff}
          />
          <Section
            label="Untracked"
            section="untracked"
            entries={snapshot.untracked}
            rowAction="stage"
            actionLabel="Stage all"
            disabled={disabled}
            expandable
            diffSlot={diffSlot}
            onAction={onStage}
            onToggleDiff={onToggleDiff}
          />
          {snapshot.conflicted.length > 0 && (
            <Section
              label="Conflicts"
              section={null}
              entries={snapshot.conflicted}
              danger
              rowAction={null}
              actionLabel={null}
              disabled={disabled}
              expandable={false}
              diffSlot={null}
              onAction={() => {}}
              onToggleDiff={() => {}}
            />
          )}
        </>
      )}
    </div>
  );
}
