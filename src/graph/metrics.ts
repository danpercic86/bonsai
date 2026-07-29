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
  /** @deprecated P7 §1.2: author removed from graph rows. Retained (no churn). */
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
  /** P7 §1/§8: fixed LEFT ref-column band width. Holds ~1 medium label + icons
   *  or two short labels before "+n"; bounded so ref-less rows don't shove the
   *  graph too far right (see P7 §12 FLAG-1). */
  refColWidth: 180,
  refColPadLeft: 12, // gutter before the first ref label (matches graph gutter feel)
  refColPadRight: 8, // gap between the ref band and the graph gutter
  /** P7 §2/§8: commit avatar. dia 16 fits row 28 with rings (max ring dia 23). */
  avatarRadius: 8,
  avatarBgRingExtra: 2, // bg0 halo behind the avatar (edge readability)
  avatarRingWidth: 1.5, // lane-color ring
  avatarHeadRingRadius: 10.5,
  avatarSelRingRadius: 11.5,
  avatarFont: '600 9px', // 2 initials inside a dia-16 disc
  /** P7 §3.4/§8: ref-label glyph box + gap (icon-icon and icon-label). */
  iconSize: 11,
  iconGap: 3,
} as const;

/** P7 §2.3: avatar hue-hash HSL constants (theme-invariant, legible both themes). */
export const AVATAR = { sat: 52, light: 42 } as const;

/** Font family appended to the size/weight strings above at draw time. */
export const FONT_UI =
  '"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif';
