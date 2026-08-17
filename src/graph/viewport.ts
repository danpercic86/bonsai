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

// ---------- P67 §1: the always-visible HEAD guideline ----------

/** P67 §1: geometry for the dashed HEAD guideline + its off-screen marker.
 *  All values are viewport CSS px (y grows downward, 0 = top of the canvas). */
export interface HeadGuide {
  /** Echo of the resolved (non-null) HEAD row index — lets the draw layer index
   *  `layout.nodes` for the lane without a non-null assertion. */
  headIndex: number;
  /** Anchor end of the segment (WIP dot centre, or just above the top edge on a
   *  clean tree), clamped to [-PAD, viewportHeight + PAD]. */
  y0: number;
  /** Target end, stopped short of the HEAD avatar's halo, same clamp. */
  y1: number;
  /** `lineDashOffset` that keeps the 3/3 dash phase anchored to CONTENT, so the
   *  dashes do not crawl while scrolling. */
  dashOffset: number;
  /** Which viewport edge HEAD is beyond, from the UNCLAMPED centre. null when
   *  HEAD's centre is on screen. */
  edge: 'top' | 'bottom' | null;
  /** A5: false when the segment collapsed below 1 px (both ends clamped to the
   *  same edge). The caller then draws ONLY the edge marker. Never both-false
   *  with `edge === null` — that returns `null` instead. */
  segment: boolean;
}

/** P67 §1: breathing room kept beyond each viewport edge when clamping the
 *  guideline. Replaces the old ad-hoc 56 (which only existed to accommodate the
 *  UNCLAMPED start). Exported so the tests and the self-test assert the bound. */
export const HEAD_GUIDE_PAD = 8;

/** Period of the `[3, 3]` dash pattern the guideline strokes with. */
const HEAD_GUIDE_DASH = 6;

/** P67 §1: pure geometry for the always-visible HEAD guideline. Returns `null`
 *  when there is nothing meaningful to draw:
 *   - `headIndex === null` (unknown HEAD — notably the streamed-graph window
 *     before HEAD's chunk arrives; this also suppresses the edge marker, so the
 *     UI never claims "HEAD is below" while it does not know);
 *   - the segment collapses below 1 px AND HEAD's centre is on screen (its halo
 *     already covers the anchor, so there is nothing to point at). A collapsed
 *     segment with HEAD off-screen still returns a value (`segment: false`,
 *     `edge !== null`) so the caller draws the marker alone — see A5 / §1.1a.
 *
 *  `layoutScrollTop` is the WIP-shifted scroll position (`visibleRowRange`'s
 *  third return value) — the same value passed to `drawGraph` as
 *  `Viewport.scrollTop`, so row centres agree by construction.
 *
 *  Anchor: the WIP dot centre when `wipOffset === 1`, else `-PAD`. The `-PAD`
 *  fallback is what makes the guide work on a CLEAN tree (`wip === null`), where
 *  today nothing at all is drawn even though the user still wants to see where
 *  HEAD is.
 *
 *  Both ends are clamped (today only the target is — see contract D1) so the
 *  stroked path length is bounded by `viewportHeight + 2*PAD` no matter how far
 *  the user has scrolled; `dashOffset` compensates the clamp so the dash phase
 *  stays content-stable. */
export function headGuide(a: {
  headIndex: number | null;
  layoutScrollTop: number;
  /** 1 when a WIP row is synthesised, else 0. */
  wipOffset: number;
  rowHeight: number;
  /** `EffectiveMetrics.avatarRadius`. */
  avatarRadius: number;
  /** `EffectiveMetrics.avatarBgRingExtra` — the bg0 halo behind the avatar. */
  ringExtra: number;
  viewportHeight: number;
}): HeadGuide | null {
  const { headIndex, layoutScrollTop, wipOffset, rowHeight, viewportHeight } = a;
  if (headIndex === null) return null;

  const PAD = HEAD_GUIDE_PAD;
  // `visibleRowRange` defines layoutScrollTop = scrollTop - wipOffset*rowHeight,
  // so this reconstruction is exact: at raw scrollTop 0 with a WIP row the
  // anchor is rowHeight/2 — identical to the old `RH/2 - vp.scrollTop`.
  const rawScrollTop = layoutScrollTop + wipOffset * rowHeight;
  const anchor = wipOffset === 1 ? rowHeight / 2 - rawScrollTop : -PAD;
  const headCenter = headIndex * rowHeight + rowHeight / 2 - layoutScrollTop;
  const halo = a.avatarRadius + a.ringExtra;

  // Generalises both directions: HEAD below the anchor (the normal case) and
  // HEAD above it (a clean tree scrolled past HEAD, where anchor = -PAD).
  // A6.3 (§1.1b): no `dir === 0` early return — it ran before `edge` and so
  // suppressed a legitimate marker at the exact scroll where the centre meets
  // the anchor. It was also redundant: dir === 0 => target === anchor => the
  // segment collapses, which the A5 test below already handles as marker-only.
  // `Math.sign` may yield -0 here; -0 * halo is -0 and target === headCenter,
  // so the collapsed path is taken either way.
  const dir = Math.sign(headCenter - anchor);
  const target = headCenter - dir * halo; // stop AT the halo, not over it

  const LO = -PAD;
  const HI = viewportHeight + PAD;
  const y0 = Math.max(LO, Math.min(HI, anchor));
  const y1 = Math.max(LO, Math.min(HI, target));

  // A6.1 (§1.1b): canvas strokes the path from `y0`, and `lineDashOffset = off`
  // shifts the pattern so dash-on runs begin where path distance s ≡ -off; on
  // screen the dash grid therefore sits at y ≡ y0 - off. Content-anchoring wants
  // that grid at y ≡ anchor, hence off ≡ y0 - anchor (NOT anchor - y0 — the
  // inverted form is wrong by -2*(y0 - anchor), which varies with scroll and so
  // makes the dashes crawl against the content at ~1 px per px).
  // True modulo: plain `%` yields negatives and `lineDashOffset` would jitter.
  const dashOffset = (((y0 - anchor) % HEAD_GUIDE_DASH) + HEAD_GUIDE_DASH) % HEAD_GUIDE_DASH;
  const edge = headCenter < 0 ? 'top' : headCenter > viewportHeight ? 'bottom' : null;

  // A5 (§1.1a): the collapse test suppresses only the SEGMENT, never the edge
  // marker — `edge` is therefore computed ABOVE this test. With a WIP row the
  // anchor sits above HEAD, so scrolling past HEAD clamps both ends to -PAD;
  // returning null there would make `edge: 'top'` unreachable and the guide
  // would vanish exactly when the user needs it.
  const collapsed = Math.abs(y1 - y0) < 1;
  if (collapsed && edge === null) return null; // nothing meaningful to draw

  return { headIndex, y0, y1, dashOffset, edge, segment: !collapsed };
}
