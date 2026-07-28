import { Fragment, memo } from 'react';
import type { FileDiff } from '../ipc';

// Pure unified-diff renderer (M4 contract §4.1). No ipc imports: diffs arrive
// precomputed from Rust (or the mock); this component only lays them out.

export interface DiffViewProps {
  diff: FileDiff;
}

/**
 * One expanded diff's fetch state, shared by StatusPanel (mode A) and
 * CommitPanel (mode B). Key convention: `${section}:${path}` for workdir rows,
 * `commit:${path}` for commit files. App owns all fetching.
 *
 * P1 §4.1: `diff` MAY be non-null while `state === 'loading'` — a same-key
 * refetch keeps the stale content visible (dimmed) instead of flashing the
 * skeleton. First-time expansions still load with `diff: null`.
 */
export interface DiffSlot {
  key: string;
  state: 'loading' | 'error' | 'ready';
  diff: FileDiff | null; // when ready, or stale content during a refetch
  error: string | null; // when error
}

function Placeholder({ text }: { text: string }) {
  return <div className="diff-placeholder">{text}</div>;
}

// Memoized (P1 §4.2): with §4.1 keeping the same FileDiff reference for stale
// content, a large diff no longer re-renders while its slot is loading or
// unrelated App state changes.
export const DiffView = memo(function DiffView({ diff }: DiffViewProps) {
  if (diff.binary) return <Placeholder text="Binary file" />;
  if (diff.tooLarge) return <Placeholder text="Diff too large to display (> 5000 lines)" />;
  if (diff.hunks.length === 0) return <Placeholder text="No changes" />;

  return (
    <div className="diff-view">
      {diff.hunks.map((h, hi) => (
        <Fragment key={hi}>
          <div className="diff-hunk-header mono">
            {`@@ -${h.oldStart},${h.oldLines} +${h.newStart},${h.newLines} @@`}
          </div>
          {h.lines.map((line, li) => (
            <Fragment key={li}>
              <div className={`diff-line diff-line-${line.kind}`}>
                <span className="diff-lineno">{line.oldNo ?? ''}</span>
                <span className="diff-lineno">{line.newNo ?? ''}</span>
                <span className="diff-marker">
                  {line.kind === 'add' ? '+' : line.kind === 'del' ? '−' : ' '}
                </span>
                <span className="diff-content">{line.content}</span>
              </div>
              {line.noNewline === true && (
                <div className="diff-line diff-nonewline" aria-hidden="true">
                  <span className="diff-lineno" />
                  <span className="diff-lineno" />
                  <span className="diff-marker" />
                  <span className="diff-content">{'\\ No newline at end of file'}</span>
                </div>
              )}
            </Fragment>
          ))}
        </Fragment>
      ))}
    </div>
  );
});

export interface DiffSlotViewProps {
  slot: DiffSlot;
  /** Dismissing the error banner collapses the expansion (App passes the toggle). */
  onDismissError(): void;
}

/** Loading / error / ready body under an expanded file row (contract §4.2).
 * Skeleton only when there is no content yet; a same-key refetch renders the
 * stale diff dimmed (P1 §4.1). */
export function DiffSlotView({ slot, onDismissError }: DiffSlotViewProps) {
  if (slot.state === 'loading' && slot.diff === null) {
    return (
      <div className="diff-slot-loading skeleton-group" aria-hidden="true">
        {Array.from({ length: 3 }, (_, i) => (
          <div key={i} className="skeleton-row" />
        ))}
      </div>
    );
  }
  if (slot.state === 'error') {
    return (
      <div className="error-banner error-banner-dismissible diff-slot-error" role="alert">
        <span className="error-banner-text">{slot.error}</span>
        <button
          type="button"
          className="error-dismiss"
          aria-label="Dismiss error"
          onClick={onDismissError}
        >
          {'×'}
        </button>
      </div>
    );
  }
  return slot.diff !== null ? (
    <div className={slot.state === 'loading' ? 'diff-scroll diff-stale' : 'diff-scroll'}>
      <DiffView diff={slot.diff} />
    </div>
  ) : null;
}
