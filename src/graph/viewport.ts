/** Pure scroll/virtualization math for the graph canvas (T3.6 split from
 *  GraphCanvas.tsx). Plain data in → plain data out; zero canvas/DOM imports.
 *  All coordinates are CSS px. The formulas are moved VERBATIM from the
 *  component — behavior-preserving. */

/** Rectangle in host CSS coords (P7 §6.1) — anchors the hover tooltip. */
export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

/** P2c §5.2: DOM-measured visible row count for PageUp/PageDown deltas.
 *  Never below 1 (a zero-height host must still page by one row). */
export function visibleRowCount(cssHeight: number, rowHeight: number): number {
  return Math.max(1, Math.floor(cssHeight / rowHeight));
}

/** The visible (overscanned) layout-row window painted by one frame (§4.2).
 *  `layoutScrollTop` is the raw scroller position shifted by the synthetic WIP
 *  row (P1 §9.3) — the Rust layout knows nothing about the WIP row. `lastRow`
 *  may be < `firstRow` (e.g. an empty graph: n=0 ⇒ lastRow=-1); callers'
 *  `for (row = first; row <= last)` loops then simply do nothing. */
export function visibleRowRange(
  scrollTop: number,
  wipOffset: number,
  rowHeight: number,
  viewportHeight: number,
  nodeCount: number,
  overscan: number,
): { firstRow: number; lastRow: number; layoutScrollTop: number } {
  const layoutScrollTop = scrollTop - wipOffset * rowHeight;
  const firstRow = Math.max(0, Math.floor(layoutScrollTop / rowHeight) - overscan);
  const lastRow = Math.min(
    nodeCount - 1,
    Math.ceil((layoutScrollTop + viewportHeight) / rowHeight) + overscan,
  );
  return { firstRow, lastRow, layoutScrollTop };
}

/** P1 §6.3/§9.3: scroll adjustment that brings `row` into view, or `null` when
 *  it is already fully visible. Row position accounts for the WIP row offset:
 *  target y = (row + wipOffset) * rowHeight. One row of breathing room is kept
 *  above/below (the ± rowHeight), clamped at 0 on top. */
export function scrollRowIntoView(
  row: number,
  wipOffset: number,
  rowHeight: number,
  viewTop: number,
  viewHeight: number,
): number | null {
  const rowTop = (row + wipOffset) * rowHeight;
  const rowBottom = rowTop + rowHeight;
  const viewBottom = viewTop + viewHeight;
  if (rowTop < viewTop) return Math.max(0, rowTop - rowHeight);
  if (rowBottom > viewBottom) return rowBottom - viewHeight + rowHeight;
  return null;
}

/** P7 §6.2: clamp the tooltip inside the host. Default below the anchor;
 *  flip above / pull left when it would overflow the host edges. */
export function clampTooltipPos(
  anchor: Rect,
  tipWidth: number,
  tipHeight: number,
  hostWidth: number,
  hostHeight: number,
): { left: number; top: number } {
  let left = anchor.left;
  let top = anchor.top + anchor.height + 4;
  if (left + tipWidth > hostWidth) left = hostWidth - tipWidth - 4;
  left = Math.max(4, left);
  if (top + tipHeight > hostHeight) top = anchor.top - tipHeight - 4;
  return { left, top };
}

/** §4.3: HiDPI backing-store dimensions for a CSS-px canvas size. Never 0
 *  (canvas dimensions of 0 throw / blank the bitmap). */
export function backingStoreSize(
  cssWidth: number,
  cssHeight: number,
  dpr: number,
): { width: number; height: number } {
  return {
    width: Math.max(1, Math.round(cssWidth * dpr)),
    height: Math.max(1, Math.round(cssHeight * dpr)),
  };
}

/** P11d §4.3: total scroll extent — every row (plus the synthetic WIP row)
 *  at the live rowHeight knob, plus 8px bottom breathing room. */
export function spacerHeight(nodeCount: number, wipOffset: number, rowHeight: number): number {
  return (nodeCount + wipOffset) * rowHeight + 8;
}
