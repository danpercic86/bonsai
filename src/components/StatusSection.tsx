/** P67 §5.6: one working-dir section (Staged / Changes), split out of
 *  `StatusPanel.tsx` verbatim. Owns the `WorkdirSection` origin type (its props
 *  and `StatusPanel`'s both need it; `StatusPanel` re-exports it for callers). */
import { useMemo } from 'react';
import type { ReactNode } from 'react';
import type { ListView, StatusEntry } from '../ipc';
import { buildPathTree } from '../utils/pathTree';
import type { DiffSlot } from './DiffView';
import { DirRowActions } from './DirRowActions';
import { entryPaths, StatusFileRow } from './StatusFileRow';
import type { RowAction } from './StatusFileRow';
import { Tree } from './Tree';

export type WorkdirSection = 'staged' | 'unstaged' | 'untracked';

export function StatusSection({
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
  extraAction,
  emptyText,
  variant,
  onAction,
  onToggleDiff,
  onDiscard,
  onDiscardForce,
  onBlame,
  onFileHistory,
}: {
  label: string;
  /** Visual modifier: tints the section so Staged vs Changes read differently. */
  variant: 'staged' | 'changes';
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
  /** P15b: optional extra header control (e.g. the staged-section "✨ Review"). */
  extraAction?: ReactNode;
  /** A3: one-line placeholder shown in place of the list when there are no rows
   *  (expandable Staged/Changes sections only). */
  emptyText?: string;
  onAction: (paths: string[]) => void;
  onToggleDiff: (section: WorkdirSection, entry: StatusEntry) => void;
  /** P20 §4.3: discard a tracked row's unstaged edits. When provided, rows whose
   *  resolved origin is `unstaged` get a discard control; untracked rows do not. */
  onDiscard?: (paths: string[]) => void;
  /** Bulk force-discard: reverts modified tracked files AND deletes new/untracked
   *  files. Changes section only — drives the "Discard all" header button, the
   *  folder-level discard hover button, and the per-row delete button on new
   *  (untracked) rows. App confirms before the IPC call. */
  onDiscardForce?: (paths: string[]) => void;
  /** P23d: open blame for a row's file (tracked rows only). */
  onBlame?: (path: string) => void;
  /** P23d: open file history for a row's file (tracked rows only). */
  onFileHistory?: (path: string) => void;
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
    // P20 §4.3: offer discard only on tracked (unstaged-origin) rows.
    const rowDiscard =
      onDiscard !== undefined && rowSection === 'unstaged' ? onDiscard : undefined;
    // New (untracked) rows have no staged/committed version to revert to — offer
    // permanent deletion instead (the force variant deletes untracked paths).
    const rowDelete =
      onDiscardForce !== undefined && rowSection === 'untracked' ? onDiscardForce : undefined;
    // P23d: blame/history need a committed version — offer them on tracked rows
    // only (untracked files are not in HEAD).
    const tracked = rowSection !== 'untracked';
    return (
      <StatusFileRow
        key={`${entry.status}:${entry.path}`}
        entry={entry}
        action={rowAction}
        disabled={disabled}
        expandable={expandable && section !== null}
        expanded={expanded}
        onAction={onAction}
        onDiscard={rowDiscard}
        onDelete={rowDelete}
        onBlame={tracked ? onBlame : undefined}
        onFileHistory={tracked ? onFileHistory : undefined}
        onToggle={() => {
          if (rowSection !== null) onToggleDiff(rowSection, entry);
        }}
        treeMode={treeMode}
      />
    );
  };
  return (
    <section
      className={`status-section status-section--${variant}`}
      aria-labelledby={`section-${variant}-label`}
    >
      <div
        id={`section-${variant}-label`}
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
        {variant === 'changes' && onDiscardForce !== undefined && entries.length > 0 && (
          <button
            type="button"
            className="section-action section-action-discard"
            disabled={disabled}
            title="Discard all changes (reverts modified files and deletes new files)"
            onClick={() => onDiscardForce(entries.flatMap(entryPaths))}
          >
            <span className="section-action-glyph" aria-hidden="true">
              {'↺'}
            </span>{' '}
            Discard all
          </button>
        )}
        {extraAction}
      </div>
      {entries.length === 0 && expandable && emptyText !== undefined ? (
        <p className="section-empty">{emptyText}</p>
      ) : nodes !== null ? (
        <Tree
          nodes={nodes}
          leafKey={(l) => `${l.item.status}:${l.item.path}`}
          renderLeaf={(l) => renderRow(l.item, true)}
          onActivateDir={(leaves) => onAction(leaves.flatMap((l) => entryPaths(l.item)))}
          dirActionHint={
            rowAction === 'unstage' ? 'Double-click to unstage all' : 'Double-click to stage all'
          }
          renderDirActions={(leaves) => {
            const paths = leaves.flatMap((l) => entryPaths(l.item));
            return variant === 'changes' ? (
              <DirRowActions
                disabled={disabled}
                onStage={() => onAction(paths)}
                onDiscard={
                  onDiscardForce !== undefined ? () => onDiscardForce(paths) : undefined
                }
              />
            ) : (
              <DirRowActions disabled={disabled} onUnstage={() => onAction(paths)} />
            );
          }}
        />
      ) : (
        <ul className="file-list">{entries.map((entry) => renderRow(entry, false))}</ul>
      )}
    </section>
  );
}
