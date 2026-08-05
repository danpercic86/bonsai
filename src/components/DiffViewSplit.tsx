import { Fragment } from 'react';
import type React from 'react';
import type { DiffLine, Hunk, LineSelection } from '../ipc';
import { toSelection } from './DiffView';
import { pairSplitRows } from '../utils/splitRows';

// P46 Workstream 1: presentational side-by-side (split) renderer. Owns NO state —
// DiffView keeps ALL selection/drag state and hands this file the selection domain
// (globalIndexByLine + selectedBounds) plus the same callbacks the unified renderer
// uses. Per-cell gutters are the drag handles; per-cell +/−/× and per-hunk header
// buttons call straight back into DiffView's handlers, so staging/discarding logic
// is shared with the unified view with zero divergence.

export interface DiffViewSplitProps {
  /** The diff's hunks (same objects DiffView's flat `rows` were built from). */
  hunks: Hunk[];
  /** DiffLine → global index in DiffView's flat unified `rows`, by object identity.
   *  Used for BOTH the `data-g` attribute and the per-cell selection test. */
  globalIndexByLine: Map<DiffLine, number>;
  /** Current unified-order selection bounds [lo..hi] inclusive, or null. */
  selectedBounds: { lo: number; hi: number } | null;
  /** stageable !== null (App gates working-dir diffs). Read-only when false. */
  interactive: boolean;
  /** Direction of a granular action; drives marker glyphs + button labels. */
  stageable: null | 'stage' | 'unstage';
  /** stageable === 'stage' && onDiscardLines wired (widens the marker column, adds ×). */
  discardable: boolean;
  /** highlight.js per-line renderer (HTML string) or null; DiffView's `highlight`. */
  highlight: ((text: string) => string | null) | null;
  /** Gutter (`.diff-lineno`) pointerdown → anchor the drag at global index g.
   *  DiffView passes its existing `onRowPointerDown` unchanged. */
  onGutterPointerDown(e: React.PointerEvent<HTMLElement>, g: number): void;
  /** Cell pointerenter (while dragging) → extend the range to global index g.
   *  DiffView passes its existing `onRowPointerEnter` unchanged. */
  onRowPointerEnter(e: React.PointerEvent<HTMLElement>, g: number): void;
  /** Per-cell `+`/`−` stages/unstages exactly one line (DiffView.onStageLines). */
  onStageLines(selection: LineSelection[]): void;
  /** Per-cell `×` discards one line (DiffView.onDiscardLines); only when discardable. */
  onDiscardLines?(selection: LineSelection[]): void;
  /** Per-hunk header Stage/Unstage button. */
  onStageHunk(hunkIndex: number): void;
  /** Per-hunk header Discard button (unstaged tracked only). */
  onDiscardHunk?(hunkIndex: number): void;
}

export function DiffViewSplit({
  hunks,
  globalIndexByLine,
  selectedBounds,
  interactive,
  stageable,
  discardable,
  highlight,
  onGutterPointerDown,
  onRowPointerEnter,
  onStageLines,
  onDiscardLines,
  onStageHunk,
  onDiscardHunk,
}: DiffViewSplitProps) {
  const cell = (side: 'left' | 'right', line: DiffLine | null) => {
    if (line === null) {
      return <div className="diff-split-cell diff-split-filler" />;
    }
    const g = globalIndexByLine.get(line);
    const selected =
      selectedBounds !== null &&
      g !== undefined &&
      g >= selectedBounds.lo &&
      g <= selectedBounds.hi;
    const isChanged = line.kind === 'add' || line.kind === 'del';
    const tint =
      side === 'left' && line.kind === 'del'
        ? ' diff-line-del'
        : side === 'right' && line.kind === 'add'
          ? ' diff-line-add'
          : '';
    const html = highlight ? highlight(line.content) : null;
    return (
      <div
        className={`diff-split-cell diff-split-cell-${side}${tint}${
          selected ? ' diff-line-selected' : ''
        }`}
        data-g={g}
        onPointerEnter={
          interactive && g !== undefined ? (e) => onRowPointerEnter(e, g) : undefined
        }
      >
        <span
          className="diff-lineno"
          onPointerDown={
            interactive && g !== undefined ? (e) => onGutterPointerDown(e, g) : undefined
          }
        >
          {side === 'left' ? (line.oldNo ?? '') : (line.newNo ?? '')}
        </span>
        <span className="diff-marker">
          {interactive && isChanged ? (
            <button
              type="button"
              className="diff-gutter-btn"
              title={stageable === 'stage' ? 'Stage this line' : 'Unstage this line'}
              aria-label={stageable === 'stage' ? 'Stage this line' : 'Unstage this line'}
              onPointerDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                onStageLines([toSelection(line)]);
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
        <span className="diff-content diff-split-content">
          {html !== null ? (
            <span dangerouslySetInnerHTML={{ __html: html }} />
          ) : (
            line.content
          )}
          {line.noNewline === true && (
            <span className="diff-split-nonewline">{' \\ No newline at end of file'}</span>
          )}
        </span>
      </div>
    );
  };

  return (
    <>
      {hunks.map((h, hi) => (
        <Fragment key={hi}>
          <div className="diff-hunk-header mono">
            <span className="diff-hunk-header-text">
              {`@@ -${h.oldStart},${h.oldLines} +${h.newStart},${h.newLines} @@`}
            </span>
            {interactive && (
              <button
                type="button"
                className="diff-hunk-stage-btn"
                onClick={() => onStageHunk(hi)}
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
          {pairSplitRows(h).map((row, ri) => (
            <div className="diff-split-row" key={ri}>
              {cell('left', row.left)}
              {cell('right', row.right)}
            </div>
          ))}
        </Fragment>
      ))}
    </>
  );
}
