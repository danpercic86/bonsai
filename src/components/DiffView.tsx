import { Fragment, memo, useEffect, useMemo, useRef, useState } from 'react';
import type { ConflictFile, DiffLine, FileDiff, LineSelection } from '../ipc';
import { detectLanguage } from '../utils/language';
import { useHighlighter } from '../utils/useHighlighter';
import { DiffViewSplit } from './DiffViewSplit';

// Pure unified-diff renderer (M4 contract §4.1). No ipc imports: diffs arrive
// precomputed from Rust (or the mock); this component only lays them out.
//
// P17c adds interactive partial-staging affordances (working-dir diffs only —
// App gates via `stageable`): per-line gutter buttons, a per-hunk "Stage hunk"
// button (Diff View), and a mouse-range → floating action button. The component
// stays presentational: it only COLLECTS the selection and calls back; App owns
// every ipc.stagePartial/unstagePartial mutation.

export interface DiffViewProps {
  diff: FileDiff;
  /** 'diff' (hunks) | 'file' (one continuous full-context listing) | 'split'
   *  (side-by-side old/new). Default 'diff'. */
  viewMode?: 'diff' | 'file' | 'split';
  /** null = read-only (commit/compare/conflict, or binary/tooLarge/renamed). Otherwise the
   *  direction a granular action performs. */
  stageable?: null | 'stage' | 'unstage';
  /** Stage/unstage exactly these changed lines (context already dropped). */
  onStageLines?(selection: LineSelection[]): void;
  /** Stage/unstage every add/del line of hunk `hunkIndex` (Diff View header button). */
  onStageHunk?(hunkIndex: number): void;
  /** P28: discard every add/del line of hunk `hunkIndex` in the WORKTREE. Rendered
   *  only when provided AND stageable === 'stage' (unstaged diffs). */
  onDiscardHunk?(hunkIndex: number): void;
  /** P45: discard exactly these changed lines (context already dropped) in the
   *  WORKTREE. Rendered only when provided AND stageable === 'stage' (unstaged
   *  tracked diffs). Both the gutter control and the drag-range float call this. */
  onDiscardLines?(selection: LineSelection[]): void;
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
  /** P3c: populated instead of `diff` for `conflict:<path>` keys — the
   * read-only marker view (same stale-during-refetch rule as `diff`). */
  conflict?: ConflictFile | null;
}

function Placeholder({ text }: { text: string }) {
  return <div className="diff-placeholder">{text}</div>;
}

export function toSelection(line: DiffLine): LineSelection {
  return { kind: line.kind, oldNo: line.oldNo, newNo: line.newNo };
}

// Memoized (P1 §4.2): with §4.1 keeping the same FileDiff reference for stale
// content, a large diff no longer re-renders while its slot is loading or
// unrelated App state changes. Read-only usages (DiffBrowser) pass no callbacks,
// so memo still shields them; interactive overlay usages pass App useCallbacks.
export const DiffView = memo(function DiffView({
  diff,
  viewMode = 'diff',
  stageable = null,
  onStageLines,
  onStageHunk,
  onDiscardHunk,
  onDiscardLines,
}: DiffViewProps) {
  // P4e Step 2: detect the file language and lazily load its grammar. Hooks
  // run unconditionally at the top; the binary/too-large/empty short-circuits
  // below never highlight, so this work is harmless for those states.
  const lang = useMemo(() => detectLanguage(diff.path), [diff.path]);
  const highlight = useHighlighter(lang?.id ?? null);

  const interactive = stageable !== null;
  // P45: a per-line discard control is offered only for unstaged tracked diffs
  // (same gate as the hunk-header "Discard hunk") AND when App wires the callback.
  const discardable = stageable === 'stage' && onDiscardLines !== undefined;

  // Flat, in-render-order list of every diff line across all hunks. A stable
  // GLOBAL index (position in this array) identifies a row for mouse-range
  // selection; `${hi}:${li}` maps the per-hunk render coordinates onto it.
  const rows = useMemo(() => {
    const out: { hi: number; li: number; line: DiffLine }[] = [];
    diff.hunks.forEach((h, hi) => h.lines.forEach((line, li) => out.push({ hi, li, line })));
    return out;
  }, [diff]);
  const globalIndexOf = useMemo(() => {
    const m = new Map<string, number>();
    rows.forEach((r, g) => m.set(`${r.hi}:${r.li}`, g));
    return m;
  }, [rows]);
  // P46 WS1: DiffLine → global index by object identity. Split cells reference the
  // exact `hunk.lines[*]` objects (via pairSplitRows), so this resolves each cell's
  // global index for the `data-g` attribute and the selection test. Context lines
  // appear once in `rows` → once here.
  const globalIndexByLine = useMemo(() => {
    const m = new Map<DiffLine, number>();
    rows.forEach((r, g) => m.set(r.line, g));
    return m;
  }, [rows]);

  const containerRef = useRef<HTMLDivElement | null>(null);
  // Active contiguous mouse-range [min(anchor,focus)..max]; null = none.
  const [range, setRange] = useState<{ anchor: number; focus: number } | null>(null);
  const [floatTop, setFloatTop] = useState(0);
  const draggingRef = useRef(false);
  const rangeRef = useRef(range);
  rangeRef.current = range;

  // A fresh diff / view-mode / read-only flip drops any pending range so a stale
  // selection can never be actioned against different content.
  useEffect(() => {
    setRange(null);
    draggingRef.current = false;
  }, [diff, viewMode, interactive]);

  // Range lifecycle: pointerup ends the drag (keeps the selection); Escape or a
  // click outside the diff clears it. Escape runs in the CAPTURE phase and stops
  // propagation when a range is live, so it clears the range BEFORE the overlay's
  // bubble-phase Esc handler would close the whole diff.
  useEffect(() => {
    if (!interactive) return;
    const onUp = () => {
      draggingRef.current = false;
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && rangeRef.current !== null) {
        e.stopImmediatePropagation();
        e.preventDefault();
        setRange(null);
      }
    };
    const onDocDown = (e: PointerEvent) => {
      const t = e.target as Node | null;
      if (t !== null && containerRef.current !== null && !containerRef.current.contains(t)) {
        setRange(null);
      }
    };
    window.addEventListener('pointerup', onUp);
    window.addEventListener('keydown', onKey, true);
    window.addEventListener('pointerdown', onDocDown);
    return () => {
      window.removeEventListener('pointerup', onUp);
      window.removeEventListener('keydown', onKey, true);
      window.removeEventListener('pointerdown', onDocDown);
    };
  }, [interactive]);

  const selectedBounds = useMemo(() => {
    if (range === null) return null;
    return { lo: Math.min(range.anchor, range.focus), hi: Math.max(range.anchor, range.focus) };
  }, [range]);

  const changedInRange = useMemo(() => {
    if (selectedBounds === null) return [];
    const out: DiffLine[] = [];
    for (let g = selectedBounds.lo; g <= selectedBounds.hi; g++) {
      const r = rows[g];
      if (r !== undefined && (r.line.kind === 'add' || r.line.kind === 'del')) out.push(r.line);
    }
    return out;
  }, [selectedBounds, rows]);

  if (diff.binary) return <Placeholder text="Binary file" />;
  if (diff.tooLarge) return <Placeholder text="Diff too large to display (> 5000 lines)" />;
  if (diff.hunks.length === 0) return <Placeholder text="No changes" />;

  const onRowPointerDown = (e: React.PointerEvent<HTMLElement>, g: number) => {
    if (!interactive || e.button !== 0) return;
    // Own the drag from the line-number gutter only: preventDefault here keeps
    // the drag range clean without suppressing native text selection over the
    // (.diff-content) code, which carries no pointerdown handler.
    e.preventDefault();
    draggingRef.current = true;
    setRange({ anchor: g, focus: g });
    // currentTarget is a .diff-lineno span, not the row: resolve the row so the
    // "Stage N lines" float lines up with the row's offsetTop. Split rows are
    // `.diff-split-row` (a child of the position:relative container), so widen the
    // lookup to resolve `offsetTop` in either view (P46 WS1).
    const row = (e.currentTarget as HTMLElement).closest(
      '.diff-line, .diff-split-row',
    ) as HTMLElement | null;
    setFloatTop(row?.offsetTop ?? 0);
  };
  const onRowPointerEnter = (e: React.PointerEvent<HTMLElement>, g: number) => {
    if (!draggingRef.current) return;
    setRange((prev) => (prev === null ? { anchor: g, focus: g } : { anchor: prev.anchor, focus: g }));
    const row = (e.currentTarget as HTMLElement).closest(
      '.diff-line, .diff-split-row',
    ) as HTMLElement | null;
    setFloatTop(row?.offsetTop ?? 0);
  };

  const commitRange = () => {
    if (changedInRange.length === 0) return;
    onStageLines?.(changedInRange.map(toSelection));
    setRange(null);
  };

  // P45: discard the drag-range's changed lines. App captures the selection into
  // pendingLineDiscard and arms the ConfirmDialog; nothing reverts until confirm.
  const commitDiscardRange = () => {
    if (changedInRange.length === 0) return;
    onDiscardLines?.(changedInRange.map(toSelection));
    setRange(null);
  };

  const lineRow = (hi: number, li: number, line: DiffLine) => {
    const g = globalIndexOf.get(`${hi}:${li}`) ?? 0;
    const selected =
      selectedBounds !== null && g >= selectedBounds.lo && g <= selectedBounds.hi;
    const isChanged = line.kind === 'add' || line.kind === 'del';
    const html = highlight ? highlight(line.content) : null;
    return (
      <Fragment key={`${hi}:${li}`}>
        <div
          className={`diff-line diff-line-${line.kind}${selected ? ' diff-line-selected' : ''}`}
          data-hunk={hi}
          data-line={li}
          // Range-drag is armed only from the line-number gutters below; the row
          // keeps pointerenter so a gutter-initiated drag extends across rows,
          // while pointerdown over .diff-content stays native (text-selectable).
          onPointerEnter={interactive ? (e) => onRowPointerEnter(e, g) : undefined}
        >
          <span
            className="diff-lineno"
            onPointerDown={interactive ? (e) => onRowPointerDown(e, g) : undefined}
          >
            {line.oldNo ?? ''}
          </span>
          <span
            className="diff-lineno"
            onPointerDown={interactive ? (e) => onRowPointerDown(e, g) : undefined}
          >
            {line.newNo ?? ''}
          </span>
          <span className="diff-marker">
            {interactive && isChanged ? (
              <button
                type="button"
                className="diff-gutter-btn"
                title={stageable === 'stage' ? 'Stage this line' : 'Unstage this line'}
                aria-label={stageable === 'stage' ? 'Stage this line' : 'Unstage this line'}
                // Stop the row-drag from starting so a plain click stages one line.
                onPointerDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation();
                  onStageLines?.([toSelection(line)]);
                }}
              >
                {stageable === 'stage' ? '+' : '−'}
              </button>
            ) : line.kind === 'add' ? (
              '+'
            ) : line.kind === 'del' ? (
              '−'
            ) : (
              ' '
            )}
            {discardable && isChanged && (
              <button
                type="button"
                className="diff-gutter-discard-btn"
                title="Discard this line"
                aria-label="Discard this line"
                // Stop the row-drag from starting so a plain click discards one line.
                onPointerDown={(e) => e.stopPropagation()}
                onClick={(e) => {
                  e.stopPropagation();
                  onDiscardLines?.([toSelection(line)]);
                }}
              >
                {'×'}
              </button>
            )}
          </span>
          {html !== null ? (
            <span className="diff-content" dangerouslySetInnerHTML={{ __html: html }} />
          ) : (
            <span className="diff-content">{line.content}</span>
          )}
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
    );
  };

  const floatButton =
    interactive && changedInRange.length > 0 ? (
      <div className="diff-stage-float" style={{ top: floatTop }}>
        <button
          type="button"
          onPointerDown={(e) => e.stopPropagation()}
          onClick={commitRange}
        >
          {`${stageable === 'stage' ? 'Stage' : 'Unstage'} ${changedInRange.length} line${
            changedInRange.length === 1 ? '' : 's'
          }`}
        </button>
        {discardable && (
          <button
            type="button"
            className="diff-float-discard"
            onPointerDown={(e) => e.stopPropagation()}
            onClick={commitDiscardRange}
          >
            {`Discard ${changedInRange.length} line${changedInRange.length === 1 ? '' : 's'}`}
          </button>
        )}
      </div>
    ) : null;

  // File View: one continuous listing with no @@ headers (concatenate hunks
  // defensively — File View should already be a single full-context hunk).
  if (viewMode === 'file') {
    return (
      <div
        className={`diff-view diff-view-file${discardable ? ' diff-view--discardable' : ''}`}
        ref={containerRef}
      >
        {rows.map(({ hi, li, line }) => lineRow(hi, li, line))}
        {floatButton}
      </div>
    );
  }

  // Split View: side-by-side old/new columns. DiffViewSplit is pure presentation —
  // it reuses DiffView's selection domain (globalIndexByLine + selectedBounds) and
  // the same handlers, so the float button and staging path are shared unchanged.
  if (viewMode === 'split') {
    return (
      <div
        className={`diff-view diff-view-split${discardable ? ' diff-view--discardable' : ''}`}
        ref={containerRef}
      >
        <DiffViewSplit
          hunks={diff.hunks}
          globalIndexByLine={globalIndexByLine}
          selectedBounds={selectedBounds}
          interactive={interactive}
          stageable={stageable}
          discardable={discardable}
          highlight={highlight}
          onGutterPointerDown={onRowPointerDown}
          onRowPointerEnter={onRowPointerEnter}
          onStageLines={(s) => onStageLines?.(s)}
          onDiscardLines={onDiscardLines}
          onStageHunk={(i) => onStageHunk?.(i)}
          onDiscardHunk={onDiscardHunk}
        />
        {floatButton}
      </div>
    );
  }

  return (
    <div
      className={`diff-view${discardable ? ' diff-view--discardable' : ''}`}
      ref={containerRef}
    >
      {diff.hunks.map((h, hi) => (
        <Fragment key={hi}>
          <div className="diff-hunk-header mono">
            <span className="diff-hunk-header-text">
              {`@@ -${h.oldStart},${h.oldLines} +${h.newStart},${h.newLines} @@`}
            </span>
            {interactive && (
              <button
                type="button"
                className="diff-hunk-stage-btn"
                onClick={() => onStageHunk?.(hi)}
              >
                {stageable === 'stage' ? 'Stage hunk' : 'Unstage hunk'}
              </button>
            )}
            {stageable === 'stage' && onDiscardHunk !== undefined && (
              <button
                type="button"
                className="diff-hunk-discard-btn"
                onClick={() => onDiscardHunk(hi)}
              >
                {'Discard hunk'}
              </button>
            )}
          </div>
          {h.lines.map((line, li) => lineRow(hi, li, line))}
        </Fragment>
      ))}
      {floatButton}
    </div>
  );
});

export interface DiffSlotViewProps {
  slot: DiffSlot;
  /** Dismissing the error banner collapses the expansion (App passes the toggle). */
  onDismissError(): void;
  /** P17c: forwarded to DiffView (File/Diff/Split toggle + partial-staging affordances). */
  viewMode?: 'diff' | 'file' | 'split';
  stageable?: null | 'stage' | 'unstage';
  onStageLines?(selection: LineSelection[]): void;
  onStageHunk?(hunkIndex: number): void;
  /** P28: forwarded to DiffView (danger hunk-header discard button). */
  onDiscardHunk?(hunkIndex: number): void;
  /** P45: forwarded to DiffView (per-line gutter + drag-range discard controls). */
  onDiscardLines?(selection: LineSelection[]): void;
}

/** Loading / error / ready body under an expanded file row (contract §4.2).
 * Skeleton only when there is no content yet; a same-key refetch renders the
 * stale diff dimmed (P1 §4.1). */
export function DiffSlotView({
  slot,
  onDismissError,
  viewMode,
  stageable,
  onStageLines,
  onStageHunk,
  onDiscardHunk,
  onDiscardLines,
}: DiffSlotViewProps) {
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
      <DiffView
        diff={slot.diff}
        viewMode={viewMode}
        stageable={stageable}
        onStageLines={onStageLines}
        onStageHunk={onStageHunk}
        onDiscardHunk={onDiscardHunk}
        onDiscardLines={onDiscardLines}
      />
    </div>
  ) : null;
}
