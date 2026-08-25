/** Single source for all graph geometry numbers (contract M2-graph.md §3.2,
 * canonical values from ui-reference §4–§6). All values are CSS px. */
export const METRICS = {
  rowHeight: 32,
  laneWidth: 16,
  gutter: 12,
  dotRingWidth: 2,
  edgeWidth: 2,
  /** Gap between graph area and pills/summary column. */
  textGap: 12,
  /** P51: optional full author-name text column (revived — was @deprecated). */
  authorColWidth: 120,
  dateColWidth: 72,
  /** P51 §5: short-SHA right column geometry. */
  shaColWidth: 54, // ~7 mono chars, right-aligned
  shaFont: '12px', // size/weight prefix; FONT_MONO appended at draw time
  badgeSlotWidth: 14, // verified-badge box, left of the SHA text
  badgeGap: 4, // gap between the badge slot and the SHA text
  colGap: 12,
  pillHeight: 18,
  pillPadX: 8,
  pillGap: 4,
  /** P51c: gap between a local-branch pill and its trailing ahead/behind chip
   *  (the chip has no pill background, so it sits a touch clearer than pillGap). */
  chipGap: 6,
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
  /** P7 §2/§8 (P7e §13.3): commit avatar. dia 20 fits row 32 with rings
   *  (max ring — selection — dia 27 → ~2.5px top/bottom margin). */
  avatarRadius: 10,
  avatarBgRingExtra: 2, // bg0 halo behind the avatar (edge readability); r=12, inside the head ring
  avatarRingWidth: 1.5, // lane-color ring
  avatarHeadRingRadius: 12.5,
  avatarSelRingRadius: 13.5,
  avatarFont: '600 11px', // 2 initials inside a dia-20 disc
  /** P7 §3.4/§8: ref-label glyph box + gap (icon-icon and icon-label). */
  iconSize: 11,
  iconGap: 3,
  /** PR-badge-placement §2.1: forge-signal badges now live in a dedicated
   *  right-hand FORGE column (leftmost of the metadata pack), not the ref band.
   *  The PR pill's max width fits the leading PR-state glyph + "#12345"; the CI
   *  dot box == ciBadgeSize; `signalGap` separates the CI dot from the PR pill. */
  prBadgeMaxWidth: 56, // was 46 — +10px for the leading PR-state glyph
  prBadgePadX: 5,
  ciBadgeSize: 11,
  signalGap: 6,
  /** Fixed width of the forge column = ciBadgeSize(11) + signalGap(6) +
   *  prBadgeMaxWidth(56) + 1px slack. Reserved only when forge data is present. */
  forgeColWidth: 74,
} as const;

/** The three user-tunable geometry knobs (P11 §2.3) — the METRICS fields the
 *  sliders vary at runtime. Everything else stays at its `as const` baseline. */
type MetricKnob = 'avatarRadius' | 'rowHeight' | 'laneWidth';

/** P11d: ring radii derived from `avatarRadius` at runtime (see
 *  `effectiveMetrics`); they widen to `number` alongside the user knobs. */
type DerivedMetric = 'avatarHeadRingRadius' | 'avatarSelRingRadius';

/** P51 §5: METRICS fields the compact preset overrides at runtime — widened
 *  from their `as const` literal types so the denser preset values type-check.
 *  (`rowHeight`/`avatarRadius` are compact-overridden too but already widen via
 *  `MetricKnob`.) */
type CompactMetric =
  | 'avatarBgRingExtra'
  | 'pillHeight'
  | 'textGap'
  | 'avatarFont'
  | 'summaryFont'
  | 'metaFont'
  | 'shaFont';

/** P11d §4.1 / P51 §5: the effective render-geometry object threaded through
 *  the draw pass — the METRICS baseline with the user knobs (or the compact
 *  preset) overlaid. Same shape as METRICS; the knobs + derived rings + compact
 *  fields widen to `number`/`string` (they carry runtime values). */
export type EffectiveMetrics = Omit<
  typeof METRICS,
  MetricKnob | DerivedMetric | CompactMetric
> &
  Record<MetricKnob | DerivedMetric, number> & {
    avatarBgRingExtra: number;
    pillHeight: number;
    textGap: number;
    avatarFont: string;
    summaryFont: string;
    metaFont: string;
    shaFont: string;
  };

/** P51 §5 (D5): compact-mode geometry preset. Overrides row/node/pill/font
 *  geometry below the comfortable-mode slider ranges; `laneWidth` still honors
 *  its slider (horizontal density is independent). Default `compact:false` ⇒
 *  this is inert (the comfortable branch equals the pre-P51 baseline exactly). */
const COMPACT = {
  rowHeight: 22,
  avatarRadius: 8,
  avatarBgRingExtra: 1,
  pillHeight: 15,
  textGap: 8,
  avatarFont: '600 10px',
  summaryFont: '400 12px',
  metaFont: '400 11px',
  shaFont: '11px',
} as const;

/** P11d §4.1 / P51 §5: overlay the user knobs — or the compact preset when
 *  `g.compact` — onto the METRICS baseline. In comfortable mode the row/node
 *  sliders apply and every other field equals METRICS.* (no visual change from
 *  the pre-P51 baseline). In compact mode row/node/pill/font geometry comes
 *  from COMPACT and those sliders are ignored; `laneWidth` always honors its
 *  slider. Call-site passes the full `GraphPrefs` (compact lives inside it). */
export function effectiveMetrics(g: {
  avatarRadius: number;
  rowHeight: number;
  laneWidth: number;
  compact: boolean;
}): EffectiveMetrics {
  const preset = g.compact ? COMPACT : METRICS;
  const avatarRadius = g.compact ? COMPACT.avatarRadius : g.avatarRadius;
  const rowHeight = g.compact ? COMPACT.rowHeight : g.rowHeight;
  return {
    ...METRICS,
    // P51 compact overrides (each equals METRICS.* in comfortable mode).
    avatarBgRingExtra: preset.avatarBgRingExtra,
    pillHeight: preset.pillHeight,
    textGap: preset.textGap,
    avatarFont: preset.avatarFont,
    summaryFont: preset.summaryFont,
    metaFont: preset.metaFont,
    shaFont: preset.shaFont,
    // User knobs (row/node ignored while compact; laneWidth always honored).
    avatarRadius,
    rowHeight,
    laneWidth: g.laneWidth,
    // P11d: derive the HEAD/selection rings from the chosen avatarRadius so they
    // stay outside the disc at large sizes (preserving the baseline deltas of
    // +2.5 / +3.5 → 12.5 / 13.5 at the default avatarRadius 10).
    avatarHeadRingRadius:
      avatarRadius + (METRICS.avatarHeadRingRadius - METRICS.avatarRadius),
    avatarSelRingRadius:
      avatarRadius + (METRICS.avatarSelRingRadius - METRICS.avatarRadius),
  };
}

/** P7 §2.3: avatar hue-hash HSL constants (theme-invariant, legible both themes). */
export const AVATAR = { sat: 52, light: 42 } as const;

/** Font family appended to the size/weight strings above at draw time.
 *  INVARIANT: FONT_UI / FONT_MONO must stay token-for-token identical to
 *  `--font-ui` / `--font-mono` in src/styles.css — canvas rows and DOM rows show
 *  the same text, so a divergent stack renders two typefaces side by side.
 *  `metrics.test.ts` pins both token lists — update all three together. */
export const FONT_UI =
  '"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif';

/** P51 §5: monospace family for the short-SHA column (appended to `shaFont`
 *  at draw time, like FONT_UI). Consumed by the draw layer in P51b. */
export const FONT_MONO =
  'ui-monospace, "SF Mono", Menlo, "Cascadia Code", "Cascadia Mono", Consolas, "JetBrains Mono", monospace';
