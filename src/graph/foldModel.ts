/** Spec-004: pure display↔model row mapping for folded linear runs.
 *
 *  Rust decides WHICH rows are foldable (`FoldSpan` metadata on the stream's
 *  `done` chunk / `GraphLayout.foldSpans`); this module only applies the row
 *  mapping — the same category of work as virtualization. No canvas, no React.
 *
 *  Each COLLAPSED span replaces its `count` hidden model rows with exactly one
 *  fold-pill display row, so the per-span display shrink is `count - 1`.
 *  Expanded spans (their `start` is in the caller's transient expansion set)
 *  contribute nothing to the mapping — display == model over their rows. */

import type { FoldSpan } from '../ipc';

/** Mirror of the Rust `MIN_FOLD_RUN` (fold.rs) — used by the mock only; the
 *  real backend never emits smaller spans. */
export const MIN_FOLD_RUN = 5;

export type DisplayRow =
  | { kind: 'commit'; row: number }
  | { kind: 'fold'; span: FoldSpan };

export interface FoldModel {
  /** Total MODEL rows the mapping was built over. */
  readonly totalRows: number;
  /** Collapsed spans only, sorted by `start` (expanded spans excluded). */
  readonly collapsed: readonly FoldSpan[];
  /** Display index of each collapsed span's fold-pill row (parallel array). */
  readonly pillRows: readonly number[];
  /** Prefix sums: shrink[i] == Σ_{j<=i} (count_j - 1). */
  readonly shrink: readonly number[];
  /** Total DISPLAY rows (== totalRows when nothing is collapsed). */
  readonly displayRowCount: number;
  /** True when the mapping is the identity (no collapsed spans). */
  readonly identity: boolean;
}

/** Sanitize + sort spans and drop the expanded ones. Spans that fall outside
 *  `totalRows` (stale metadata racing a reload) are dropped, never clamped. */
export function buildFoldModel(
  totalRows: number,
  spans: readonly FoldSpan[],
  expanded: ReadonlySet<number>,
): FoldModel {
  const collapsed = spans
    .filter(
      (s) =>
        s.count > 0 &&
        s.start >= 0 &&
        s.start + s.count <= totalRows &&
        !expanded.has(s.start),
    )
    .slice()
    .sort((a, b) => a.start - b.start);
  const pillRows: number[] = [];
  const shrink: number[] = [];
  let acc = 0;
  for (const s of collapsed) {
    pillRows.push(s.start - acc);
    acc += s.count - 1;
    shrink.push(acc);
  }
  return {
    totalRows,
    collapsed,
    pillRows,
    shrink,
    displayRowCount: totalRows - acc,
    identity: collapsed.length === 0,
  };
}

/** Index of the last collapsed span whose pill display row is <= d, or -1. */
function lastPillAtOrBefore(m: FoldModel, d: number): number {
  let lo = 0;
  let hi = m.pillRows.length - 1;
  let ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (m.pillRows[mid] <= d) {
      ans = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return ans;
}

/** Map a display row to its model meaning. Out-of-range display rows map to
 *  out-of-range commit rows (callers bounds-check against the node array). */
export function displayToModel(m: FoldModel, d: number): DisplayRow {
  const i = lastPillAtOrBefore(m, d);
  if (i < 0) return { kind: 'commit', row: d };
  if (m.pillRows[i] === d) return { kind: 'fold', span: m.collapsed[i] };
  return { kind: 'commit', row: d + m.shrink[i] };
}

/** The collapsed span containing model row `r`, or null (visible row). */
export function spanContaining(m: FoldModel, r: number): FoldSpan | null {
  const i = lastStartAtOrBefore(m, r);
  if (i < 0) return null;
  const s = m.collapsed[i];
  return r < s.start + s.count ? s : null;
}

/** Index of the last collapsed span with start <= r, or -1. */
function lastStartAtOrBefore(m: FoldModel, r: number): number {
  let lo = 0;
  let hi = m.collapsed.length - 1;
  let ans = -1;
  while (lo <= hi) {
    const mid = (lo + hi) >> 1;
    if (m.collapsed[mid].start <= r) {
      ans = mid;
      lo = mid + 1;
    } else hi = mid - 1;
  }
  return ans;
}

/** Display index of model row `r`. A HIDDEN row maps to its containing span's
 *  pill row (UI contract §6: nav from a hidden selection anchors at the pill). */
export function modelToDisplay(m: FoldModel, r: number): number {
  const i = lastStartAtOrBefore(m, r);
  if (i < 0) return r;
  const s = m.collapsed[i];
  if (r < s.start + s.count) return m.pillRows[i]; // hidden → its pill
  return r - m.shrink[i];
}

/** Display index of a collapsed span's pill by its `start`, or null when that
 *  span is not currently collapsed. */
export function pillRowOfStart(m: FoldModel, start: number): number | null {
  const i = lastStartAtOrBefore(m, start);
  if (i < 0 || m.collapsed[i].start !== start) return null;
  return m.pillRows[i];
}

/** The (collapsed OR expanded) span of `spans` containing model row `r`, or
 *  null. Used for selection pinning (AC6), reveal auto-expand, and the
 *  ArrowLeft collapse path — those act on the RAW span list, not the model. */
export function spanAt(spans: readonly FoldSpan[], r: number): FoldSpan | null {
  for (const s of spans) if (s.start <= r && r < s.start + s.count) return s;
  return null;
}
