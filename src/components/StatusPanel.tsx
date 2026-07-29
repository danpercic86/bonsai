import { useMemo, useState } from 'react';
import type {
  ConflictEntry,
  ConflictKind,
  ConflictResolution,
  FileStatus,
  ListView,
  StatusEntry,
  StatusSnapshot,
} from '../ipc';
import { buildPathTree } from '../utils/pathTree';
import type { DiffSlot } from './DiffView';
import { Tree } from './Tree';

export type { DiffSlot } from './DiffView';

const BADGES: Record<FileStatus, string> = {
  added: 'A',
  modified: 'M',
  deleted: 'D',
  renamed: 'R',
  typechange: 'T',
  untracked: 'A',
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
  /** P3b: the tree supplies directory context — render only the basename
   *  (renames keep the full `orig → path` text; tooltips keep full paths). */
  treeMode?: boolean;
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
  sectionForEntry,
  entries,
  danger = false,
  rowAction,
  actionLabel,
  disabled,
  expandable,
  diffSlot,
  listView,
  onAction,
  onToggleDiff,
}: {
  label: string;
  /** Diff-key prefix; null for the conflicts section (not expandable). */
  section: WorkdirSection | null;
  /** P4c: per-entry origin resolver (Changes section merges unstaged +
   *  untracked). When provided, the row's diff key + toggle use the resolved
   *  origin instead of the representative `section` prop. */
  sectionForEntry?: (e: StatusEntry) => WorkdirSection;
  entries: StatusEntry[];
  danger?: boolean;
  /** Per-row button kind; null = no actions in this section. */
  rowAction: RowAction;
  /** Section-header bulk button label ("Stage all" / "Unstage all"). */
  actionLabel: string | null;
  disabled: boolean;
  expandable: boolean;
  diffSlot: DiffSlot | null;
  listView: ListView;
  onAction: (paths: string[]) => void;
  onToggleDiff: (section: WorkdirSection, entry: StatusEntry) => void;
}) {
  // P3b §5.1: tree placement by NEW path (origPath never affects placement).
  const nodes = useMemo(
    () => (listView === 'tree' ? buildPathTree(entries, (e) => e.path) : null),
    [listView, entries],
  );
  const renderRow = (entry: StatusEntry, treeMode: boolean) => {
    const rowSection = sectionForEntry ? sectionForEntry(entry) : section;
    const key = rowSection !== null ? `${rowSection}:${entry.path}` : null;
    const expanded = key !== null && diffSlot !== null && diffSlot.key === key;
    return (
      <FileRow
        key={`${entry.status}:${entry.path}`}
        entry={entry}
        action={rowAction}
        disabled={disabled}
        expandable={expandable && section !== null}
        expanded={expanded}
        onAction={onAction}
        onToggle={() => {
          if (rowSection !== null) onToggleDiff(rowSection, entry);
        }}
        treeMode={treeMode}
      />
    );
  };
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
      {nodes !== null ? (
        <Tree
          nodes={nodes}
          leafKey={(l) => `${l.item.status}:${l.item.path}`}
          renderLeaf={(l) => renderRow(l.item, true)}
          onActivateDir={(leaves) => onAction(leaves.flatMap((l) => entryPaths(l.item)))}
          dirActionHint={
            rowAction === 'unstage' ? 'Double-click to unstage all' : 'Double-click to stage all'
          }
        />
      ) : (
        <ul className="file-list">{entries.map((entry) => renderRow(entry, false))}</ul>
      )}
    </section>
  );
}

// P3c §8.2: lowercase spaced text of ConflictKind for the per-row badge.
const CONFLICT_KIND_LABELS: Record<ConflictKind, string> = {
  bothModified: 'both modified',
  bothAdded: 'both added',
  deletedByUs: 'deleted by us',
  deletedByThem: 'deleted by them',
  addedByUs: 'added by us',
  addedByThem: 'added by them',
  bothDeleted: 'both deleted',
};

function ConflictRow({
  entry,
  kind,
  disabled,
  expanded,
  onResolve,
  onToggleView,
}: {
  entry: StatusEntry;
  /** null = kind lookup miss (conflicts list momentarily stale) — no badge. */
  kind: ConflictKind | null;
  disabled: boolean;
  expanded: boolean;
  onResolve: (r: ConflictResolution) => void;
  onToggleView: () => void;
}) {
  const { dir, name } = splitPath(entry.path);
  return (
    <li
      className={`file-row file-status-conflicted conflict-row${expanded ? ' file-row-expanded' : ''}`}
      title={entry.path}
    >
      <button
        type="button"
        className="file-row-main"
        aria-expanded={expanded}
        onClick={onToggleView}
      >
        <span className={`file-chevron${expanded ? ' file-chevron-open' : ''}`}>{'›'}</span>
        <span className="file-badge mono">{BADGES.conflicted}</span>
        <span className="file-path">
          {dir !== null && <span className="file-dir">{dir}</span>}
          <span className="file-name">{name}</span>
        </span>
        {kind !== null && <span className="conflict-kind">{CONFLICT_KIND_LABELS[kind]}</span>}
      </button>
      <button
        type="button"
        className="row-action conflict-action"
        title="Take our version"
        aria-label={`Take our version of ${entry.path}`}
        disabled={disabled}
        onClick={() => onResolve('ours')}
      >
        ours
      </button>
      <button
        type="button"
        className="row-action conflict-action"
        title="Take their version"
        aria-label={`Take their version of ${entry.path}`}
        disabled={disabled}
        onClick={() => onResolve('theirs')}
      >
        theirs
      </button>
      <button
        type="button"
        className="row-action conflict-action"
        title="Mark resolved (I edited the file)"
        aria-label={`Mark ${entry.path} resolved`}
        disabled={disabled}
        onClick={() => onResolve('markResolved')}
      >
        resolved
      </button>
    </li>
  );
}

/** P3c §8.2: conflict rows always render FLAT (no P3b tree grouping) —
 * conflicts are few; keep the section simple. */
function ConflictsSection({
  entries,
  conflicts,
  disabled,
  diffSlot,
  onResolveConflict,
  onToggleConflictView,
}: {
  entries: StatusEntry[];
  conflicts: ConflictEntry[];
  disabled: boolean;
  diffSlot: DiffSlot | null;
  onResolveConflict: (path: string, r: ConflictResolution) => void;
  onToggleConflictView: (path: string) => void;
}) {
  const kindByPath = useMemo(
    () => new Map(conflicts.map((c) => [c.path, c.kind] as const)),
    [conflicts],
  );
  return (
    <section className="status-section">
      <div className="section-header section-label section-label-danger">
        <span>Conflicts ({entries.length})</span>
      </div>
      <ul className="file-list">
        {entries.map((entry) => (
          <ConflictRow
            key={entry.path}
            entry={entry}
            kind={kindByPath.get(entry.path) ?? null}
            disabled={disabled}
            expanded={diffSlot !== null && diffSlot.key === `conflict:${entry.path}`}
            onResolve={(r) => onResolveConflict(entry.path, r)}
            onToggleView={() => onToggleConflictView(entry.path)}
          />
        ))}
      </ul>
    </section>
  );
}

function SkeletonRows() {
  return (
    <div className="skeleton-group" aria-hidden="true">
      {Array.from({ length: 6 }, (_, i) => (
        <div key={i} className="skeleton-row" />
      ))}
    </div>
  );
}

export interface StatusPanelProps {
  snapshot: StatusSnapshot | null;
  loading: boolean;
  /** Id-wrapped (P1 §4.5): identical messages from distinct operations get
   * distinct ids, so the banner re-surfaces after dismissal. */
  error: { id: number; message: string } | null;
  /** True while any stage/unstage/commit is in flight — disables all action buttons. */
  busy: boolean;
  /** Currently open diff (null = none) — drives row expanded/highlight state;
   * the diff itself renders in App's center-pane DiffOverlay (P3a). */
  diffSlot: DiffSlot | null;
  /** P3b: flat vs directory-tree file lists (display-only). */
  listView: ListView;
  /** P3c: authoritative kind per conflicted path (from listConflicts). */
  conflicts: ConflictEntry[];
  onStage(paths: string[]): void;
  onUnstage(paths: string[]): void;
  /** Toggle a row's diff in the center-pane overlay (App owns the fetch). */
  onToggleDiff(section: WorkdirSection, entry: StatusEntry): void;
  /** P3c §8.2: resolve one conflicted path (no confirm — re-doable). */
  onResolveConflict(path: string, r: ConflictResolution): void;
  /** Toggle the read-only marker view (diffSlot key `conflict:<path>`). */
  onToggleConflictView(path: string): void;
}

/** Pure presentational right-panel status view; all fetching lives in App. */
export function StatusPanel({
  snapshot,
  loading,
  error,
  busy,
  diffSlot,
  listView,
  conflicts,
  onStage,
  onUnstage,
  onToggleDiff,
  onResolveConflict,
  onToggleConflictView,
}: StatusPanelProps) {
  const [dismissedErrorId, setDismissedErrorId] = useState<number | null>(null);
  const visibleError = error !== null && error.id !== dismissedErrorId ? error : null;

  const disabled = busy || loading;

  // P4c: merge unstaged + untracked into one presentation-only "Changes" list.
  // Each row keeps its ORIGIN section (via originByPath) so diff keys, refetch,
  // and overlayMeta still resolve entries in the correct snapshot array.
  const changes = useMemo(
    () => [...(snapshot?.unstaged ?? []), ...(snapshot?.untracked ?? [])],
    [snapshot?.unstaged, snapshot?.untracked],
  );
  const originByPath = useMemo(() => {
    const m = new Map<string, WorkdirSection>();
    for (const e of snapshot?.unstaged ?? []) m.set(e.path, 'unstaged');
    for (const e of snapshot?.untracked ?? []) m.set(e.path, 'untracked');
    return m;
  }, [snapshot?.unstaged, snapshot?.untracked]);

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
          <span className="error-banner-text">{visibleError.message}</span>
          <button
            type="button"
            className="error-dismiss"
            aria-label="Dismiss error"
            onClick={() => setDismissedErrorId(visibleError.id)}
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
            listView={listView}
            onAction={onUnstage}
            onToggleDiff={onToggleDiff}
          />
          <Section
            label="Changes"
            section="unstaged"
            sectionForEntry={(e) => originByPath.get(e.path) ?? 'unstaged'}
            entries={changes}
            rowAction="stage"
            actionLabel="Stage all"
            disabled={disabled}
            expandable
            diffSlot={diffSlot}
            listView={listView}
            onAction={onStage}
            onToggleDiff={onToggleDiff}
          />
          {snapshot.conflicted.length > 0 && (
            <ConflictsSection
              entries={snapshot.conflicted}
              conflicts={conflicts}
              disabled={disabled}
              diffSlot={diffSlot}
              onResolveConflict={onResolveConflict}
              onToggleConflictView={onToggleConflictView}
            />
          )}
        </>
      )}
    </div>
  );
}
