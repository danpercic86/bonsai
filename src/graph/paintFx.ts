/** Per-frame transient paint effects — extracted from GraphCanvas.paintNow
 *  (spec-004 size split, moved verbatim): the P84 reveal flash resolution and
 *  the spec-002 sway resolution, plus the P58c visible-range report guard. */

import type { Theme } from './colors';
import type { GraphStyle } from './colors';
import { isDarkBg } from './colors';
import { flashAlpha, flashRingRadius } from './revealFlash';
import { visibleModelRows, displayRowFor } from './foldView';
import type { FoldSpan } from '../ipc';
import type { FoldModel } from './foldModel';

/** P84: resolve the reveal flash (row-bg pulse + dot halo) for this frame.
 *  `row` is a MODEL row; it is mapped to display space at paint time so a
 *  mid-flash expand/collapse remap never flashes the wrong row. */
export function resolveFlash(
  fs: { row: number; start: number } | null,
  theme: Theme,
  avatarSelRingRadius: number,
  reducedMotion: boolean,
  foldModel: FoldModel | null,
): { row: number; alpha: number; ringRadius: number } | null {
  if (fs === null) return null;
  const elapsed = performance.now() - fs.start;
  const alpha = flashAlpha(elapsed, isDarkBg(theme.bg0), reducedMotion);
  if (alpha <= 0) return null;
  return {
    row: displayRowFor(foldModel, fs.row),
    alpha,
    ringRadius: flashRingRadius(elapsed, avatarSelRingRadius, reducedMotion),
  };
}

/** spec 002 §5: resolve the active settle for this frame (Bonsai only; never
 *  under reduced motion — arming is gated). */
export function resolveSway(
  sw: { start: number } | null,
  graphStyle: GraphStyle,
  reducedMotion: boolean,
): { elapsedMs: number } | null {
  if (sw === null || graphStyle !== 'bonsai' || reducedMotion) return null;
  return { elapsedMs: performance.now() - sw.start };
}

/** P58c/spec-004: fire `onVisibleRangeChange` once per window change. With fold
 *  active the payload is the visible COMMIT model rows (min/max + list) so the
 *  verify request never spans a collapsed run's hidden commits. Returns the new
 *  guard value when a report fired, else null. */
export function reportVisibleRange(
  onVisibleRangeChange: (first: number, last: number, modelRows?: readonly number[]) => void,
  prev: { first: number; last: number; count: number } | null,
  firstRow: number,
  lastRow: number,
  n: number,
  foldModel: FoldModel | null,
  foldRows: ReadonlyMap<number, FoldSpan> | null,
): { first: number; last: number; count: number } | null {
  const modelRows =
    foldModel !== null && foldRows !== null
      ? visibleModelRows(foldModel, firstRow, lastRow, foldRows, n)
      : null;
  const first = modelRows !== null && modelRows.length > 0 ? modelRows[0] : firstRow;
  const last =
    modelRows !== null && modelRows.length > 0 ? modelRows[modelRows.length - 1] : lastRow;
  const count = modelRows !== null ? modelRows.length : -1;
  if (prev !== null && prev.first === first && prev.last === last && prev.count === count) {
    return null;
  }
  onVisibleRangeChange(first, last, modelRows ?? undefined);
  return { first, last, count };
}
