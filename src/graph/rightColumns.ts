/** Pure right-column layout model for the graph's per-row text pass (P51b §6.2).
 *  Packs the ENABLED author/SHA/date columns against the right edge in a fixed
 *  order (author, SHA, date — date rightmost). A DISABLED column reserves NO
 *  space, so toggling one off reclaims its width and the summary flexes out to
 *  `summaryEndX`. Used by BOTH the draw pass and the hover hit-test, so column
 *  geometry has a single source of truth. No canvas, no React. */

import type { EffectiveMetrics } from './metrics';

/** P51b §6.1: persisted per-row display toggles threaded into the draw + hover
 *  layers. `compact` is NOT here — it is already baked into EffectiveMetrics.
 *  `showAheadBehind`/`branchStats` are consumed by the ref-band chip in P51c;
 *  they ride along here now (inert) so the interface stays stable. */
export interface GraphDisplayOptions {
  showSha: boolean;
  showAuthor: boolean;
  showDate: boolean;
  dateBasis: 'author' | 'committer';
  /** P51c: ahead/behind chip on local-branch-tip pills (inert in P51b). */
  showAheadBehind: boolean;
  /** P51c: name → ahead/behind for local branches (empty map ok in P51b). */
  branchStats: ReadonlyMap<string, { ahead: number | null; behind: number | null }>;
}

/** One packed column, in canvas CSS-px. `rightX` is the right-align anchor the
 *  draw pass and hit-test both use; `leftX` is the column's left edge. */
export interface ColRect {
  leftX: number;
  rightX: number;
  width: number;
}

export interface RightColumns {
  author: ColRect | null;
  /** Includes the verified-badge slot at its LEFT: badgeSlot + gap + SHA text. */
  sha: ColRect | null;
  date: ColRect | null;
  /** Left edge available to the summary (right end of its flex zone). */
  summaryEndX: number;
}

/** Width of the SHA column incl. the leading verified-badge slot + gap. */
function shaWidth(m: EffectiveMetrics): number {
  return m.badgeSlotWidth + m.badgeGap + m.shaColWidth;
}

/** Pack the enabled columns against `effRight` (= vp.width - rightInset) in the
 *  fixed order author, SHA, date (date rightmost). A right→left cursor places
 *  each enabled column and steps one `colGap` past its left edge; disabled
 *  columns are skipped (they consume nothing). `summaryEndX` is where the
 *  cursor lands after the leftmost enabled column — already one `colGap` clear
 *  of it — i.e. the summary's right edge. With only the date column shown this
 *  reproduces the pre-P51 summary/date geometry exactly. */
export function computeRightColumns(
  effRight: number,
  display: GraphDisplayOptions,
  m: EffectiveMetrics,
): RightColumns {
  let cursor = effRight - m.colGap;
  const place = (width: number): ColRect => {
    const rightX = cursor;
    const leftX = rightX - width;
    cursor = leftX - m.colGap;
    return { leftX, rightX, width };
  };
  // Rightmost first so each column's rightX is the right-align anchor.
  const date = display.showDate ? place(m.dateColWidth) : null;
  const sha = display.showSha ? place(shaWidth(m)) : null;
  const author = display.showAuthor ? place(m.authorColWidth) : null;
  return { author, sha, date, summaryEndX: cursor };
}
