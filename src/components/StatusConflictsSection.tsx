/** P67 §5.6: the conflicts section + its rows, split out of `StatusPanel.tsx`
 *  verbatim. Renders FLAT and above STAGED (P3c §8.2) — `StatusPanel` owns that
 *  ordering.
 *
 *  P68d §5.4: the single `aiResolvingPath` scalar became a per-path `aiRows` map.
 *  That scalar's `aiDisabled={aiResolvingPath !== null}` disabled EVERY row's ✨AI
 *  button during any single run — the reported item-5 bug, part (a). Now the ONLY
 *  thing that disables an idle row is the concurrency cap. */
import { useMemo } from 'react';
import type { ConflictEntry, ConflictKind, ConflictResolution, StatusEntry } from '../ipc';
import { BulkAiResolveButton } from './BulkAiResolveButton';
import { SummarizeIcon } from './menuIcons';
import type { DiffSlot } from './DiffView';
import type { AiRowState } from './repoWorkspace/useAiRuns';
import { isAiResolvableKind, type BulkAiControl } from './repoWorkspace/useBulkAiResolve';
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

/** The ✨AI button's label / tooltip / `data-state` per store status (§5.4). */
function aiButtonView(
  path: string,
  row: AiRowState | undefined,
  aiEligible: boolean,
): { label: string; title: string; state: string } {
  if (!aiEligible) {
    return { label: 'AI', title: 'Enable AI features in Settings to use this', state: 'idle' };
  }
  switch (row?.status) {
    case 'running':
      return {
        label: `…${row.elapsedSecs}s`,
        title: `Resolving ${path} with AI — ${row.elapsedSecs}s elapsed`,
        state: 'running',
      };
    case 'awaitingInput':
      return { label: '?', title: 'Claude asked a question — answer it to continue', state: 'ask' };
    case 'ready':
      return { label: '✓ review', title: `Review the AI proposal for ${path}`, state: 'ready' };
    case 'failed':
      return {
        label: '⚠',
        title: `${row.error ?? 'AI resolution failed'} — click to retry`,
        state: 'failed',
      };
    // A cancelled run leaves nothing to review: offer a plain retry.
    case 'cancelled':
    default:
      return { label: 'AI', title: 'Resolve with AI', state: 'idle' };
  }
}

function ConflictRow({
  entry,
  kind,
  disabled,
  expanded,
  aiEligible,
  aiRow,
  aiAtCapacity,
  onResolve,
  onToggleView,
  onAiResolve,
  onAiReview,
  onAiReveal,
}: {
  entry: StatusEntry;
  /** null = kind lookup miss (conflicts list momentarily stale) — no badge. */
  kind: ConflictKind | null;
  disabled: boolean;
  expanded: boolean;
  /** P13 §8.2: AI enabled+consented+CLI installed (button shown but usable). */
  aiEligible: boolean;
  /** P68d: THIS path's run state, or undefined when it has none. A run on another
   *  path is invisible here by design — that is the item-5 fix. */
  aiRow: AiRowState | undefined;
  /** P68d/OQ1: the concurrency cap is reached, so a NEW run cannot start. */
  aiAtCapacity: boolean;
  onResolve: (r: ConflictResolution) => void;
  onToggleView: () => void;
  onAiResolve: () => void;
  /** Re-open the finished proposal (`✓ review`) — the store still has it. */
  onAiReview: () => void;
  /** P68e: reveal/expand the activity dock for a live run. Absent until the dock
   *  exists, in which case a live row's button is a read-only status badge. */
  onAiReveal?: () => void;
}) {
  const { dir, name } = splitPath(entry.path);
  // P13 §8.2: AI only makes sense for the two text-mergeable kinds (matches the
  // ConflictEditor mount guard); hidden for deletion/add/binary kinds. The predicate is
  // SHARED with the bulk button (P68f) — a row and "Resolve all with AI" must offer AI
  // for exactly the same set of files, so there is only one place to change it.
  const aiShown = isAiResolvableKind(kind);
  const status = aiRow?.status;
  const live = status === 'running' || status === 'awaitingInput';
  const view = aiButtonView(entry.path, aiRow, aiEligible);
  // A run elsewhere never disables this row (item-5 part a); only the cap does, and
  // only for a row that would START something.
  const aiDisabled =
    !aiEligible ||
    disabled ||
    (live ? onAiReveal === undefined : status !== 'ready' && aiAtCapacity);
  const onAiClick = live ? onAiReveal : status === 'ready' ? onAiReview : onAiResolve;
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
          data-state={view.state}
          title={view.title}
          aria-label={`Resolve ${entry.path} with AI`}
          disabled={aiDisabled}
          onClick={onAiClick}
        >
          {view.state === 'idle' && <SummarizeIcon />}
          {view.label}
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
  aiRows,
  aiAtCapacity,
  aiBulk,
  onResolveConflict,
  onToggleConflictView,
  onAiResolve,
  onAiReview,
  onAiReveal,
}: {
  entries: StatusEntry[];
  conflicts: ConflictEntry[];
  disabled: boolean;
  diffSlot: DiffSlot | null;
  aiEligible: boolean;
  /** P68d: per-path AI run state, keyed by conflicted path. Replaces the single
   *  `aiResolvingPath` scalar. */
  aiRows: Record<string, AiRowState>;
  /** P68d/OQ1: at the concurrency cap — no NEW run may start. */
  aiAtCapacity: boolean;
  /** P68f: the "Resolve all with AI" / "Cancel all" control for the section header.
   *  Absent (or `shown: false`) ⇒ no header button, which is the case for fewer than
   *  two AI-eligible conflicts. */
  aiBulk?: BulkAiControl;
  onResolveConflict: (path: string, r: ConflictResolution) => void;
  onToggleConflictView: (path: string) => void;
  onAiResolve: (path: string) => void;
  /** Re-open a finished proposal from the store (never re-runs the CLI). */
  onAiReview: (path: string) => void;
  /** P68e: reveal the AI activity dock for a live run. */
  onAiReveal?: (path: string) => void;
}) {
  const kindByPath = useMemo(
    () => new Map(conflicts.map((c) => [c.path, c.kind] as const)),
    [conflicts],
  );
  return (
    <section className="status-section">
      <div className="section-header section-label section-label-danger">
        <span>Conflicts ({entries.length})</span>
        {/* P68f: one run for ALL eligible conflicts. Only rendered with ≥2 of them —
            a single one already has its row button. */}
        {aiBulk !== undefined && (
          <BulkAiResolveButton control={aiBulk} variant="section" busy={disabled} />
        )}
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
            aiRow={aiRows[entry.path]}
            aiAtCapacity={aiAtCapacity}
            onResolve={(r) => onResolveConflict(entry.path, r)}
            onToggleView={() => onToggleConflictView(entry.path)}
            onAiResolve={() => onAiResolve(entry.path)}
            onAiReview={() => onAiReview(entry.path)}
            onAiReveal={onAiReveal === undefined ? undefined : () => onAiReveal(entry.path)}
          />
        ))}
      </ul>
    </section>
  );
}
