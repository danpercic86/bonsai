/** P67 §5.6: one working-dir file row, split out of `StatusPanel.tsx` verbatim.
 *  Also owns the low-level helpers its sibling sections share (`BADGES`,
 *  `entryPaths`, `splitPath`, `RowAction`) — this file imports no sibling, so the
 *  section files can depend on it without a cycle. */
import type { FileStatus, StatusEntry } from '../ipc';

export const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'A',
  conflicted: 'C',
};

/** Rename expansion (M3 contract §2.1): send BOTH sides of a rename. */
export function entryPaths(e: StatusEntry): string[] {
  return e.origPath !== null ? [e.origPath, e.path] : [e.path];
}

export function splitPath(path: string): { dir: string | null; name: string } {
  const idx = path.lastIndexOf('/');
  if (idx === -1) return { dir: null, name: path };
  return { dir: path.slice(0, idx + 1), name: path.slice(idx + 1) };
}

export type RowAction = 'stage' | 'unstage' | null;

export function StatusFileRow({
  entry,
  action,
  disabled,
  expandable,
  expanded,
  onAction,
  onToggle,
  onDiscard,
  onDelete,
  onBlame,
  onFileHistory,
  treeMode = false,
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
  /** P20 §4.3: discard this row's unstaged edits (tracked rows only). Absent
   *  (undefined) → no discard control (untracked rows, staged section). */
  onDiscard?: (paths: string[]) => void;
  /** Permanently delete this row's file from disk — new (untracked) rows only,
   *  which have no staged/committed version to revert to. Mutually exclusive
   *  with `onDiscard`; shares its slot. Absent → no delete control. */
  onDelete?: (paths: string[]) => void;
  /** P23d: open per-line blame for this row's file. Absent → no blame control
   *  (untracked rows — not in HEAD, nothing to blame). */
  onBlame?: (path: string) => void;
  /** P23d: open per-file commit history. Absent → no history control. */
  onFileHistory?: (path: string) => void;
  /** P3b: the tree supplies directory context — render only the basename
   *  (renames keep the full `orig → path` text; tooltips keep full paths). */
  treeMode?: boolean;
}) {
  const isRename = entry.origPath !== null;
  const pathTitle = isRename ? `${entry.origPath} → ${entry.path}` : entry.path;
  // P3f dir-row precedent (Tree.tsx onDoubleClick → onActivateDir): double-click
  // acts; the two single-click toggles cancel out on the diff overlay.
  const actionHint = disabled
    ? null
    : action === 'stage'
      ? 'Double-click to stage'
      : action === 'unstage'
        ? 'Double-click to unstage'
        : null;
  const title = actionHint !== null ? `${pathTitle} — ${actionHint}` : pathTitle;
  const { dir, name } = splitPath(entry.path);
  const pathEl = isRename ? (
    <span className="file-path mono file-rename">
      {entry.origPath} {'→'} {entry.path}
    </span>
  ) : (
    <span className="file-path">
      {!treeMode && dir !== null && <span className="file-dir">{dir}</span>}
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
          onDoubleClick={
            action !== null && !disabled ? () => onAction(entryPaths(entry)) : undefined
          }
        >
          <span className="file-badge mono">{BADGES[entry.status]}</span>
          {pathEl}
        </button>
      ) : (
        <span className="file-row-main">
          <span className="file-badge mono">{BADGES[entry.status]}</span>
          {pathEl}
        </span>
      )}
      {onFileHistory !== undefined && (
        <button
          type="button"
          className="row-action row-action-history"
          title="Show file history"
          aria-label={`Show history of ${entry.path}`}
          onClick={() => onFileHistory(entry.path)}
        >
          {'🕑'}
        </button>
      )}
      {onBlame !== undefined && (
        <button
          type="button"
          className="row-action row-action-blame"
          title="Blame (per-line authorship)"
          aria-label={`Blame ${entry.path}`}
          onClick={() => onBlame(entry.path)}
        >
          {'👁'}
        </button>
      )}
      {onDiscard !== undefined && (
        <button
          type="button"
          className="row-action row-action-discard"
          title="Discard changes (restore to the staged/committed version)"
          aria-label={`Discard changes to ${entry.path}`}
          disabled={disabled}
          onClick={() => onDiscard(entryPaths(entry))}
        >
          {'↺'}
        </button>
      )}
      {onDelete !== undefined && (
        <button
          type="button"
          className="row-action row-action-discard"
          title="Delete this new file (permanently removes it from disk)"
          aria-label={`Delete ${entry.path}`}
          disabled={disabled}
          onClick={() => onDelete(entryPaths(entry))}
        >
          {'🗑'}
        </button>
      )}
      {action !== null && (
        <button
          type="button"
          className="row-action row-action-primary"
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
