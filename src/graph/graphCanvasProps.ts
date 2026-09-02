/** `GraphCanvas`'s public prop + imperative-handle surface, extracted verbatim
 *  from `GraphCanvas.tsx` so the container file stays readable (file-size
 *  ratchet). Types only — no runtime code, no React import. `GraphCanvas.tsx`
 *  re-exports both interfaces, so existing import sites keep working. */
import type { GraphLayout, VerifyStatus } from '../ipc';
import type { GraphStyle } from './colors';
import type { GraphContextTarget } from './contextTarget';
import type { WipSummary } from './draw';
import type { GraphFoldView } from './foldView';
import type { IncrementalEdgeIndex } from './incrementalEdgeIndex';
import type { EffectiveMetrics } from './metrics';
import type { GraphSeason } from './palettes';
import type { RailInput } from './rail/OverviewRail';
import type { RevealFlash } from './reveal';
import type { GraphDisplayOptions } from './rightColumns';

export interface GraphCanvasProps {
  layout: GraphLayout;
  selectedIndex: number | null;
  /** Clicking a row toggles it; empty area below the rows selects null. */
  onSelect(index: number | null): void;
  /** P1 §9: non-null when the workdir has changes — renders a frontend-
   *  composited WIP row atop the (unchanged) Rust layout, +1 row offset. */
  wip: WipSummary | null;
  /** P2b §4.4: incremented by App on every theme change — forces a
   *  `resolveTheme` re-run (colors are otherwise cached for the component's
   *  lifetime) followed by a repaint. Lane palette itself is theme-invariant. */
  themeVersion: number;
  /** P3e §5.4: false when the owning tab is display:none (zero-size). Defaults
   *  true. When it flips true the canvas remeasures + repaints from the retained
   *  last-good bitmap (the zero-size guard in resize() kept it intact). */
  active?: boolean;
  /** P5 §4.2: right-click on a ref pill or a commit row. Empty area / WIP row →
   *  not called (the native menu is suppressed regardless). clientX/clientY
   *  anchor the context menu. */
  onContextMenu?(target: GraphContextTarget, clientX: number, clientY: number): void;
  /** P11d §4.3: effective render geometry (METRICS overlaid with the user's
   *  graph knobs). Drives every dot/avatar/row/lane pixel in the draw pass. */
  metrics: EffectiveMetrics;
  /** P11d §4.3: bumped when any graph knob changes → forces a full re-measure +
   *  repaint (analogous to `themeVersion`). */
  metricsVersion: number;
  /** P50b: row indices carrying a commit-search match → an outer match ring on
   *  those dots. Empty/absent when search is closed (no ring pass). */
  matchRows?: readonly number[];
  /** P51b: persisted per-row display toggles (SHA/author/date column + date
   *  basis, ahead/behind data). Fed straight into `drawGraph` and the date-
   *  column hover hit-test; a new object identity triggers a repaint. */
  display: GraphDisplayOptions;
  /** P58c: oid → signature verdict for the LIT badge (visible rows only, cached
   *  by oid in `useCommitVerification`). Absent/missing oid ⇒ the faint P51
   *  stub. A new map identity triggers a repaint so badges light in place. */
  verifyStatus?: ReadonlyMap<string, VerifyStatus>;
  /** P58c: fired once per paint after the visible window is computed (only when
   *  the window changed). Drives the debounced verify request for exactly the
   *  visible (overscanned) rows — the badge is virtualized. Spec-004: with fold
   *  active `first`/`last` are the min/max visible MODEL rows and `modelRows`
   *  lists exactly the visible commit rows (fold rows skipped), so a giant
   *  collapsed run never balloons the verify request. */
  onVisibleRangeChange?(first: number, last: number, modelRows?: readonly number[]): void;
  /** P63: a PR badge on a branch-tip pill was clicked → open that PR in the
   *  right-pane PR panel. When absent, PR-badge clicks fall through to the
   *  normal row-select. */
  onOpenPr?(number: number): void;
  /** P65b (streamed path): the incremental edge index owned by the stream
   *  assembler. When present it REPLACES the internal `buildEdgeIndex(layout)`
   *  memo (which would be O(n) per streamed batch). Absent ⇒ one-shot path,
   *  byte-for-byte unchanged. */
  edgeIndex?: IncrementalEdgeIndex;
  /** P65b (streamed path): total row count for the scroll extent while rows are
   *  still arriving. Absent ⇒ the spacer uses `layout.nodes.length` (one-shot /
   *  grow-as-you-go). */
  totalRows?: number;
  /** P84: nonce-driven reveal flash. A NEW `nonce` (re)starts the row-pulse +
   *  dot-halo highlight on `index`; `null`/absent means no flash. Nonce-driven so
   *  re-revealing the already-selected row re-flashes. */
  revealFlash?: RevealFlash | null;
  /** P84: `prefers-reduced-motion` (read once in the container). When true the
   *  flash is a static hold, not an animated pulse (revealFlash.ts §3.1). */
  reducedMotion?: boolean;
  /** spec 002: Bonsai paint style + season → `resolveTheme` (mirrors
   *  `themeVersion`; a change re-resolves + repaints). Default standard/living. */
  graphStyle?: GraphStyle;
  graphSeason?: GraphSeason;
  /** Spec-004: fold view-model (collapsed-span mapping + expansion callbacks).
   *  Absent ⇒ fold inactive; every path is byte-identical to pre-fold. */
  fold?: GraphFoldView;
  /** Spec-005: overview-rail bundle (RepoWorkspace assembles it). Absent ⇒ no
   *  rail plumbing at all — hover-zone checks and mount both skipped. */
  rail?: RailInput;
}

/** P2c §5.2: imperative escape hatch — App needs the DOM-measured visible row
 *  count for PageUp/PageDown deltas, which App has no other way to learn
 *  without duplicating a ResizeObserver of its own. Pure view-layer index
 *  arithmetic downstream — no lane/edge math involved. */
export interface GraphCanvasHandle {
  getVisibleRowCount(): number;
  /** P95 §2: focus the scroller so the Menu key / Shift+F10 row menu is reachable. */
  focusScroller(): void;
}
