import type { JSX } from 'react';

/** Folder-level bulk action buttons rendered inside a `.tree-dir-row` (via
 *  Tree's `renderDirActions` render prop). Revealed on row hover / focus-within
 *  (see `.tree-dir-actions` in styles.css). Each handler already closes over the
 *  folder's flattened leaf paths — this component just renders the affordances. */
export function DirRowActions({
  disabled,
  onStage,
  onUnstage,
  onDiscard,
}: {
  disabled: boolean;
  /** Changes section: stage every file under this folder. */
  onStage?: () => void;
  /** Staged section: unstage every file under this folder. */
  onUnstage?: () => void;
  /** Changes section: discard (revert tracked + delete new) under this folder. */
  onDiscard?: () => void;
}): JSX.Element {
  return (
    <span className="tree-dir-actions">
      {onStage !== undefined && (
        <button
          type="button"
          className="row-action"
          title="Stage all files in this folder"
          aria-label="Stage all files in this folder"
          disabled={disabled}
          onClick={onStage}
        >
          {'+'}
        </button>
      )}
      {onUnstage !== undefined && (
        <button
          type="button"
          className="row-action"
          title="Unstage all files in this folder"
          aria-label="Unstage all files in this folder"
          disabled={disabled}
          onClick={onUnstage}
        >
          {'−'}
        </button>
      )}
      {onDiscard !== undefined && (
        <button
          type="button"
          className="row-action row-action-discard"
          title="Discard all changes in this folder (reverts modified files and deletes new files)"
          aria-label="Discard all changes in this folder"
          disabled={disabled}
          onClick={onDiscard}
        >
          {'↺'}
        </button>
      )}
    </span>
  );
}
