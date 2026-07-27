/** Single source for all graph geometry numbers (contract M2-graph.md §3.2,
 * canonical values from ui-reference §4–§6). All values are CSS px. */
export const METRICS = {
  rowHeight: 28,
  laneWidth: 16,
  gutter: 12,
  dotRadius: 4,
  dotRingWidth: 2,
  edgeWidth: 2,
  /** Gap between graph area and pills/summary column. */
  textGap: 12,
  authorColWidth: 120,
  dateColWidth: 72,
  colGap: 12,
  pillHeight: 18,
  pillPadX: 8,
  pillGap: 4,
  pillMaxWidth: 160,
  pillFont: '600 11px',
  summaryFont: '400 13px',
  metaFont: '400 12px',
  /** §6.2 lane clamp — lanes beyond this render at the last lane's x. */
  maxRenderLanes: 24,
} as const;

/** Font family appended to the size/weight strings above at draw time. */
export const FONT_UI =
  '"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif';
