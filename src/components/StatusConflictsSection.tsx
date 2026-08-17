/** P67 §5.6: the conflicts section + its rows, split out of `StatusPanel.tsx`
 *  verbatim. Renders FLAT and above STAGED (P3c §8.2) — `StatusPanel` owns that
 *  ordering. */
import { useMemo } from 'react';
import type { ConflictEntry, ConflictKind, ConflictResolution, StatusEntry } from '../ipc';
import type { DiffSlot } from './DiffView';
import { BADGES, splitPath } from './StatusFileRow';

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
  aiEligible,
  aiBusy,
  aiDisabled,
  onResolve,
  onToggleView,
  onAiResolve,
}: {
  entry: StatusEntry;
  /** null = kind lookup miss (conflicts list momentarily stale) — no badge. */
  kind: ConflictKind | null;
  disabled: boolean;
  expanded: boolean;
  /** P13 §8.2: AI enabled+consented+CLI installed (button shown but usable). */
  aiEligible: boolean;
  /** This row's AI resolution is in flight. */
  aiBusy: boolean;
  /** Any AI resolution in flight (only one at a time) — disables this button. */
  aiDisabled: boolean;
  onResolve: (r: ConflictResolution) => void;
  onToggleView: () => void;
  onAiResolve: () => void;
}) {
  const { dir, name } = splitPath(entry.path);
  // P13 §8.2: AI only makes sense for the two text-mergeable kinds (matches the
  // ConflictEditor mount guard); hidden for deletion/add/binary kinds.
  const aiShown = kind === 'bothModified' || kind === 'bothAdded';
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
      {aiShown && (
        <button
          type="button"
          className="row-action conflict-action conflict-action-ai"
          title={aiEligible ? 'Resolve with AI' : 'Enable AI features in Settings to use this'}
          aria-label={`Resolve ${entry.path} with AI`}
          disabled={!aiEligible || disabled || aiDisabled}
          onClick={onAiResolve}
        >
          {aiBusy ? '…' : '✨ AI'}
        </button>
      )}
    </li>
  );
}

/** P3c §8.2: conflict rows always render FLAT (no P3b tree grouping) —
 * conflicts are few; keep the section simple. */
export function StatusConflictsSection({
  entries,
  conflicts,
  disabled,
  diffSlot,
  aiEligible,
  aiResolvingPath,
  onResolveConflict,
  onToggleConflictView,
  onAiResolve,
}: {
  entries: StatusEntry[];
  conflicts: ConflictEntry[];
  disabled: boolean;
  diffSlot: DiffSlot | null;
  aiEligible: boolean;
  /** Path whose AI resolution is currently in flight, or null. */
  aiResolvingPath: string | null;
  onResolveConflict: (path: string, r: ConflictResolution) => void;
  onToggleConflictView: (path: string) => void;
  onAiResolve: (path: string) => void;
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
            aiEligible={aiEligible}
            aiBusy={aiResolvingPath === entry.path}
            aiDisabled={aiResolvingPath !== null}
            onResolve={(r) => onResolveConflict(entry.path, r)}
            onToggleView={() => onToggleConflictView(entry.path)}
            onAiResolve={() => onAiResolve(entry.path)}
          />
        ))}
      </ul>
    </section>
  );
}
