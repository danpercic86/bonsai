import { useMemo, useState } from 'react';
import type {
  ConflictEntry,
  ConflictResolution,
  ListView,
  StatusEntry,
  StatusSnapshot,
} from '../ipc';
import type { DiffSlot } from './DiffView';
import type { AiRowState } from './repoWorkspace/useAiRuns';
import type { BulkAiControl } from './repoWorkspace/useBulkAiResolve';
import { StatusConflictsSection } from './StatusConflictsSection';
import { StatusSection } from './StatusSection';
import type { WorkdirSection } from './StatusSection';

export type { DiffSlot } from './DiffView';
// P67 §5.6: the type moved down to the section file that consumes it; re-exported
// here so callers keep importing it from `./StatusPanel`.
export type { WorkdirSection } from './StatusSection';

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
  /** P13 §8.2: aiEnabled && aiConsented && aiAvailability?.installed. */
  aiEligible: boolean;
  /** P68d §5.4: per-path AI run state for the conflict rows' affordance. Replaces
   *  the old `aiResolvingPath` scalar, which disabled every row during any run. */
  aiRows: Record<string, AiRowState>;
  /** P68d/OQ1: at the AI concurrency cap — no NEW run may start. */
  aiAtCapacity: boolean;
  /** P68f: the conflicts-header "Resolve all with AI" control (§6.4). */
  aiBulk?: BulkAiControl;
  onStage(paths: string[]): void;
  onUnstage(paths: string[]): void;
  /** P20 §4.3: discard unstaged edits to tracked Changes rows (App confirms). */
  onDiscard(paths: string[]): void;
  /** Bulk force-discard for the Changes section: reverts modified tracked files
   *  AND deletes new/untracked files. Drives the "Discard all" header button and
   *  folder-level discard hover buttons (App confirms before the IPC call). */
  onDiscardForce(paths: string[]): void;
  /** Toggle a row's diff in the center-pane overlay (App owns the fetch). */
  onToggleDiff(section: WorkdirSection, entry: StatusEntry): void;
  /** P3c §8.2: resolve one conflicted path (no confirm — re-doable). */
  onResolveConflict(path: string, r: ConflictResolution): void;
  /** Toggle the read-only marker view (diffSlot key `conflict:<path>`). */
  onToggleConflictView(path: string): void;
  /** P13 §8.3: request an AI resolution for one conflicted path. */
  onAiResolve(path: string): void;
  /** P68d: re-open an already-computed proposal (never re-runs the CLI). */
  onAiReview(path: string): void;
  /** P68e: reveal the AI activity dock for a live run. */
  onAiReveal?(path: string): void;
  /** P23d: open per-line blame for a tracked file (staged/unstaged rows). */
  onBlame(path: string): void;
  /** P23d: open per-file commit history for a tracked file. */
  onFileHistory(path: string): void;
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
  aiEligible,
  aiRows,
  aiAtCapacity,
  aiBulk,
  onStage,
  onUnstage,
  onDiscard,
  onDiscardForce,
  onToggleDiff,
  onResolveConflict,
  onToggleConflictView,
  onAiResolve,
  onAiReview,
  onAiReveal,
  onBlame,
  onFileHistory,
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
    <div
      className={isEmpty ? 'status-panel status-panel-empty' : 'status-panel'}
      data-testid="status-panel"
    >
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
          {snapshot.conflicted.length > 0 && (
            <StatusConflictsSection
              entries={snapshot.conflicted}
              conflicts={conflicts}
              disabled={disabled}
              diffSlot={diffSlot}
              aiEligible={aiEligible}
              aiRows={aiRows}
              aiAtCapacity={aiAtCapacity}
              aiBulk={aiBulk}
              onResolveConflict={onResolveConflict}
              onToggleConflictView={onToggleConflictView}
              onAiResolve={onAiResolve}
              onAiReview={onAiReview}
              onAiReveal={onAiReveal}
            />
          )}
          <StatusSection
            label="Staged"
            variant="staged"
            section="staged"
            entries={snapshot.staged}
            rowAction="unstage"
            actionLabel="Unstage all"
            emptyText="Stage files to include them in your commit."
            disabled={disabled}
            expandable
            diffSlot={diffSlot}
            listView={listView}
            onAction={onUnstage}
            onToggleDiff={onToggleDiff}
            onBlame={onBlame}
            onFileHistory={onFileHistory}
          />
          <StatusSection
            label="Changes"
            variant="changes"
            section="unstaged"
            sectionForEntry={(e) => originByPath.get(e.path) ?? 'unstaged'}
            entries={changes}
            rowAction="stage"
            actionLabel="Stage all"
            emptyText="Nothing to commit — your working tree is clean."
            disabled={disabled}
            expandable
            diffSlot={diffSlot}
            listView={listView}
            onAction={onStage}
            onToggleDiff={onToggleDiff}
            onDiscard={onDiscard}
            onDiscardForce={onDiscardForce}
            onBlame={onBlame}
            onFileHistory={onFileHistory}
          />
        </>
      )}
    </div>
  );
}
