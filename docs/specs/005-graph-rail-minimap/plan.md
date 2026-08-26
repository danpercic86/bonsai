# Plan: Graph Overview Rail — Search Match Ticks & On-Demand Minimap

**Spec:** ./spec.md
**UI contract:** ../../contracts/spec-005-ui.md (pending — ui-designer; final tokens/geometry there)
**Status:** draft

## Approach

Briefs #2 + #3 built together as **one component with two paint layers on one canvas**:
`OverviewRail` renders the minimap (density strip + ref/HEAD pips + viewport thumb) and, when a
rings channel is live, the match-tick layer on top. Frontend-only: no Rust logic, no IPC changes;
the single persisted pref (`graphMinimapAlwaysShow: boolean`) follows the plain-bool
`graphFirstParent` settings chain (spec-003 verified: `settings.rs` + `commands/ui_settings.rs` —
NOT a `prefs.rs`; the brief is stale there).

**Decisions recorded:**

- **One component, not two.** `MatchRail` and `Minimap` share geometry, visibility policy, hit
  zone, scroll subscription, and click→row mapping; two components would duplicate all of it.
  Layers are independent paint passes inside one `drawRail()` call.
- **Canvas painter, not SVG.** Matches every graph paint convention (draw.ts siblings), no
  per-tick DOM for thousands of matches, one HiDPI-scaled `<canvas>` ~14×railHeight px — trivially
  cheap to repaint per scroll frame.
- **Mounted inside `GraphCanvas`** (it owns the scroller, right inset, metrics, theme). GraphCanvas
  gains ONE bundle prop `rail?: RailProps` and renders `<OverviewRail scrollerRef={scrollerRef} …>`
  when the rail should exist. **GraphCanvas delta ≤ ~25 ln** (standing >500 exception — 910 ln;
  keep the delta to: hover-zone check in the existing `handleMouseMove`, reveal state, one render
  line). ALL rail logic lives in new `src/graph/rail/*` files.
- **Visibility / idle-cost guarantee (literal): hidden ⇒ nothing mounted.**
  `visible = searchRingsLive || alwaysShow || hoverRevealed || dragging`. When false, `OverviewRail`
  is not rendered at all — zero DOM, zero listeners, zero canvas, zero memos.
  - **No always-mounted hover strip.** An overlay div would sit outside the scroller and eat wheel
    events (dead-scroll zone). Instead the existing `handleMouseMove`/`handleMouseLeave` in
    GraphCanvas (already tracking `mouseXRef`) set `hoverRevealed` when
    `x > scroller.clientWidth + rightInset − HOVER_ZONE_PX` (i.e. within 20 px of the scroller's
    outer right edge, scrollbar included — hovering the scrollbar also reveals). Hover-out hides
    after a **300 ms linger** (timer in `OverviewRail`), never mid-drag.
- **Geometry** (defaults; UI contract pins final tokens): rail content width **14 px**, full pane
  height, absolutely positioned at `right: rightInset` (the live
  `offsetWidth − clientWidth` scrollbar inset GraphCanvas already computes) so it never overlaps
  the native scrollbar (AC7). Hover-reveal zone `HOVER_ZONE_PX = 20`. Ticks span the full 14 px;
  pips are 3 px dots on a left/right column split (branch left, tag right, HEAD full-width);
  thumb is a rounded translucent overlay, **min height 24 px**.
- **Display-row seam (spec-004 composition).** The rail never touches model rows directly; it
  takes a `RailRowModel { displayRowCount; toDisplayRow(modelRow): number; displayRowOid(d) }`.
  Now: identity over `layout.nodes` (`toDisplayRow = r => r`, count = `totalRows ?? nodes.length`).
  When spec-004 lands, `foldModel` supplies it: `toDisplayRow` returns the **clamped** display row
  (a match/ref inside a collapsed span maps to the span's pill row — tick still visible; clicking
  goes through `goToMatch → revealCommitByOid`, which auto-expands per the 004 plan). Never null.
- **Bucket precompute — once per layout generation, not per paint.** Fixed-resolution
  `RAIL_BUCKETS = 1024` typed-array model (`RailBuckets`), resolution-independent (resampled
  nearest-bucket at paint for any rail height). Built by pure `buildRailBuckets(layout, rowModel)`
  — O(n + refs), a few ms at 100k.
  **Invalidation:** memo keyed on `[railGeneration, rowModel]` where `railGeneration` is a number
  RepoWorkspace bumps **when the graph stream reaches `done`** (graphStreamApply already knows;
  filter changes and refreshes re-stream, so they're covered by the same key) — NOT on `layout`
  identity, which bumps per streamed batch. Fold expand/collapse is frontend-only, hence the
  explicit `rowModel` identity key. While streaming with the rail visible, the previous
  generation's buckets keep painting (positions proportional to the growing `displayRowCount`);
  stale marks clear at `done` (spec edge case).
- **Match ticks are a separate cheap memo** (`[matchRows, currentMatchRow, rowModel,
  displayRowCount]`) — O(matches) remap per search keystroke / fold toggle, independent of the
  heavy buckets.
  - **currentMatch trap:** `deriveMatchRows` can drop oids absent from the layout, so
    `matchRows[currentMatch]` is NOT valid. The rail takes `currentMatchRow: number | null`
    derived separately in RepoWorkspace from `search.results.matches[search.currentMatch]?.oid`
    → node index (same lookup `revealCommitByOid` uses).
  - **Coalescing:** ticks are bucketed to rail pixels; one drawn tick per occupied pixel, carrying
    a **representative matchIndex** (the first match landing in that pixel) so click →
    `goToMatch(index)` is exact. Density is NOT encoded beyond presence (browser-find precedent);
    the current-match tick paints last in a distinct color/width.
- **Rings-channel parity (FLAG for orchestrator, recommended + assumed below):** the graph's rings
  show *either* commit-search or Ask-history matches (WorkspaceGraphPane picks the channel). The
  rail mirrors **whichever channel is live** — same rows in, only the click resolver differs
  (`goToMatch(i)` for search; `revealCommitByOid(oid)` for historySearch). Spec only mandates
  commit search; parity is ~free and avoids rings-without-ticks inconsistency.
- **Thumb / jump mapping — scroller truth, not spacer recompute.** All mapping uses the live
  `scroller.scrollHeight − scroller.clientHeight` (same source as `scrollSweep`). Thumb rect from
  `scrollTop` fraction with min-height track-range mapping (short histories: thumb spans most of
  the rail — falls out of the clamp). Drag: pointer capture on the canvas, preserve the grab
  offset within the thumb, `scrollTop = frac(y) * maxScroll`; click outside the thumb (minimap
  zone) centers the viewport there; click within ±3 px of a tick prefers the tick's
  `goToMatch`/reveal. Rail stays visible while dragging even if the pointer leaves the zone.
  **Wheel over the rail:** forward `deltaY` to the scroller (`scroller.scrollTop += e.deltaY`) —
  no dead zone.
- **Scroll sync:** while mounted, `OverviewRail` attaches its own `scroll` listener to the
  scroller (via `scrollerRef`) and schedules a single-rAF repaint of the thumb layer — no React
  state per scroll frame, mirroring GraphCanvas's own pattern. Unmount removes the listener.
- **Theme:** colors resolved from the same CSS-variable approach (`resolveTheme` output passed in
  or re-read); ticks use the match-ring color, current match the selection/accent color; works in
  both app themes and both graph styles (density strip uses lane-palette-neutral fills).
- **Persistence:** `graphMinimapAlwaysShow: bool` (default false) — exact `graphFirstParent`
  precedent: `settings.rs` snapshot/patch + `commands/ui_settings.rs`, `useUiSettings`,
  `uiSettingsDefaults.json` + `defaults.ts`, mock `persistence.ts` validation (malformed → false).
  Settings row in the Commit-graph category per the UI contract.
- **A11y:** the rail is a redundant pointer affordance — every jump has an existing keyboard path
  (Enter/F3 next, Shift+F3 prev, arrows/PageUp/PageDown/Home/End scroll, results-list is
  focusable). The rail canvas is `aria-hidden="true"` and not in the tab order; the settings
  toggle is a standard keyboard-accessible row. No new keyboard surface required — record this in
  the UI contract.

## Rust/TS boundary

No Rust graph/IPC changes. Rust's only touch is the opaque `graph_minimap_always_show` settings
bool (snapshot/patch). Everything else is React/canvas over data the frontend already holds
(`GraphLayout` nodes/refs/headIndex, `matchRows`, scroller metrics). This does not violate
"Rust owns layout math": the rail is a *view* of the already-computed layout (same category as
virtualization), makes no topology decisions, and composes with fold via the display-row seam.

## Files touched

- `src/graph/GraphCanvas.tsx` (910 ln, standing exception — **delta ≤ ~25 ln**): new optional
  `rail?: RailProps` prop; `hoverRevealed` state set from the existing `handleMouseMove` right-edge
  check + cleared in `handleMouseLeave`; render
  `{railVisible && <OverviewRail scrollerRef={scrollerRef} rightInset… />}` inside the host. No
  other logic here.
- `src/components/WorkspaceGraphPane.tsx` (**~470 ln — FLAG: near the 500 cap**; the rail bundle
  adds ~10 ln of prop threading. If it crosses 500, extract the rail-props assembly into
  `src/components/repoWorkspace/railProps.ts` in the same increment): thread `rail` bundle from
  RepoWorkspace to GraphCanvas; pick the live rings channel + click resolver (search vs
  historySearch), mirroring the existing `matchRows` pick at ~line 352.
- `src/components/RepoWorkspace.tsx` (>500 exception; delta ~15 ln): `railGeneration` bump at
  stream `done` (from the graphStreamApply setter bundle), derive `currentMatchRow`, read
  `graphMinimapAlwaysShow` from `useUiSettings`, assemble the `rail` bundle.
- `src/components/repoWorkspace/graphStreamApply.ts` — surface a `onDone`/generation signal if not
  already exposed (~5 ln).
- `src/hooks/useUiSettings.ts`, `src/settings/uiSettingsDefaults.json`, `src/settings/defaults.ts`
  — `graphMinimapAlwaysShow: false` (~10 ln total).
- `src-tauri/src/settings.rs` + `src-tauri/src/commands/ui_settings.rs` —
  `graph_minimap_always_show: bool` snapshot/patch (~10 ln).
- `src/ipc/mock/persistence.ts` — validate + round-trip the bool (~5 ln).
- Commit-graph settings category (`src/components/settings/categories/...` per spec-003's
  `SettingsGraphDeclutterSection` placement) — one SettingsRow toggle (~8 ln, or in the UI
  contract's chosen section file).
- `src/styles` (wherever `.graph-canvas-host` styles live) — `.graph-rail` positioning classes.

## New files

- `src/graph/rail/railMath.ts` (~160 ln) — ALL pure math, zero DOM/React:
  `buildRailBuckets`, `coalesceTicks`, `thumbRect`, `railYForDisplayRow`, `displayRowForRailY`,
  `scrollTopForRailY`, `hitTick`. Unit-tested.
- `src/graph/rail/OverviewRail.tsx` (~180 ln) — the component: canvas mount + HiDPI resize,
  scroll-listener + single-rAF repaint, hover-linger timer, pointer handlers (click/drag with
  capture, wheel forward), memos for buckets and ticks, visibility lifecycle. `aria-hidden`.
- `src/graph/rail/drawRail.ts` (~130 ln) — painter: density strip → ref/HEAD pips → viewport
  thumb → match ticks → current-match tick (paint order = z-order). Sibling of draw.ts (at cap —
  nothing added there).
- `src/graph/rail/railMath.test.ts` (vitest) — see Testing.
- e2e spec `e2e/graph-rail.spec.ts` (or the existing graph e2e file if small).

## Data model / types

```ts
// src/graph/rail/railMath.ts
export const RAIL_BUCKETS = 1024;
export const RAIL_WIDTH_PX = 14;
export const HOVER_ZONE_PX = 20;
export const RAIL_LINGER_MS = 300;
export const THUMB_MIN_PX = 24;
export const TICK_HIT_SLOP_PX = 3;

/** Display-row seam — identity now; foldModel-backed under spec-004. */
export interface RailRowModel {
  displayRowCount: number;
  /** Clamped: a row inside a collapsed span maps to the span's pill row. Never null. */
  toDisplayRow(modelRow: number): number;
}

/** Fixed-resolution downsample of the whole (display-row) history. Typed arrays,
 *  built once per railGeneration — resampled at paint for any rail height. */
export interface RailBuckets {
  rows: number;                 // displayRowCount at build time
  density: Uint16Array;         // commits per bucket (RAIL_BUCKETS)
  laneMax: Uint8Array;          // max active lane count per bucket (strip darkness/width)
  flags: Uint8Array;            // bit 0 = branch pip, 1 = tag pip, 2 = HEAD
}

export interface RailTick {
  y: number;                    // rail px (coalesced: unique per px)
  matchIndex: number;           // representative index → goToMatch / list index
  displayRow: number;
  current: boolean;
}

export interface ThumbRect { top: number; height: number }

export function buildRailBuckets(layout: GraphLayout, rowModel: RailRowModel): RailBuckets;
export function coalesceTicks(
  matchDisplayRows: readonly number[],   // already mapped via rowModel
  currentMatchDisplayRow: number | null,
  displayRowCount: number,
  railHeight: number,
): RailTick[];
export function thumbRect(
  scrollTop: number, scrollHeight: number, clientHeight: number, railHeight: number,
): ThumbRect;                            // min-height + track-range mapping
export function scrollTopForRailY(
  y: number, grabOffsetPx: number, scrollHeight: number, clientHeight: number, railHeight: number,
): number;                               // inverse of thumbRect's track mapping, clamped
export function hitTick(ticks: readonly RailTick[], y: number): RailTick | null; // ±TICK_HIT_SLOP_PX

// src/graph/rail/OverviewRail.tsx
export interface RailProps {
  layout: GraphLayout;
  totalRows?: number;
  rowModel: RailRowModel;                // identity impl until spec-004
  railGeneration: number;                // bumped at stream done
  /** Live rings channel (search OR historySearch), display-row-mapped upstream. */
  matchRows: readonly number[];
  currentMatchRow: number | null;        // model row; null when none / historySearch
  onJumpToMatch(matchIndex: number): void;   // goToMatch | reveal-by-row wrapper
  ringsLive: boolean;                    // search or historySearch open with matches
  alwaysShow: boolean;                   // graphMinimapAlwaysShow
  hoverRevealed: boolean;                // from GraphCanvas mousemove zone check
  onHoverExpired(): void;                // linger timeout → GraphCanvas clears hoverRevealed
  scrollerRef: RefObject<HTMLDivElement | null>;
  rightInset: number;
  rowHeight: number;
  theme: Theme | null;                   // resolved theme (or re-resolve internally)
}
```

Settings: `graphMinimapAlwaysShow: boolean` (TS) / `graph_minimap_always_show: bool` (Rust
snapshot+patch, serde camelCase) — default `false`.

## Algorithm pseudocode

```
buildRailBuckets(layout, rowModel):                       // O(n + refs), once per generation
  rows = rowModel.displayRowCount (>=1)
  for i in 0..layout.nodes.len:
    d = rowModel.toDisplayRow(i)
    b = floor(d * RAIL_BUCKETS / rows)                    // clamp to RAIL_BUCKETS-1
    density[b] += 1 (saturating)
    laneMax[b] = max(laneMax[b], node.lane + 1)
    if node.refs has branch/remote → flags[b] |= BRANCH
    if node.refs has tag           → flags[b] |= TAG
  if headIndex != null → flags[bucket(headIndex)] |= HEAD

coalesceTicks(matchDisplayRows, currentRow, rows, railHeight):
  seen = Map<pixelY, matchIndex>                          // first match wins = representative
  for (idx, d) in matchDisplayRows:
    y = round(d / max(rows-1, 1) * (railHeight-1))
    if !seen.has(y): seen.set(y, idx)
  ticks = [{y, matchIndex, current:false} for seen]       // sorted by y
  if currentRow != null: mark/insert its pixel as current (drawn last)

paint (drawRail):                                         // per rAF while visible
  1 density strip: for each rail px → nearest bucket → fill alpha ~ density, width ~ laneMax
  2 pips: buckets with flags → 3px dots (branch left col, tag right col, HEAD accent full-width)
  3 thumb: thumbRect(scrollTop, scrollHeight, clientHeight, railHeight) → translucent rounded rect
  4 ticks: 14px-wide 2px-tall bars in match-ring color; current tick last, accent + 3px

pointer:
  down on thumb → capture, grabOffset = y - thumb.top; move → scroller.scrollTop =
    scrollTopForRailY(y, grabOffset, …)
  click: hitTick(y) → onJumpToMatch(tick.matchIndex)
         else → scroller.scrollTop = scrollTopForRailY(y, thumb.height/2, …)   // center viewport
  wheel → scroller.scrollTop += deltaY
```

## Testing

- **vitest (`railMath.test.ts`):** bucket build — density/laneMax/flags on a synthetic layout;
  rows < RAIL_BUCKETS (short history) → sensible bucket spread, no NaN; identity vs fold-like
  clamping rowModel (rows inside a "span" all land on the pill row's bucket/tick — round-trip
  under a fold model). Coalescing — 10k matches → ≤ railHeight ticks, deterministic (same input ⇒
  same tick set), representative matchIndex = first per pixel, current flag correct. Thumb —
  min-height clamp, track-range mapping round-trips with `scrollTopForRailY` (incl. grab offset),
  short history thumb spans most of the rail, clamped at both ends. `hitTick` slop boundaries.
- **vitest (settings):** `graphMinimapAlwaysShow` default false, round-trip, malformed persisted
  value → false (mock persistence).
- **e2e (harness):** hidden by default — **no `.graph-rail` element in the DOM** (idle-cost AC3);
  open search with matches → rail appears with tick elements/canvas present; click a tick region →
  selection jumps (assert selected row changed); close search → rail gone; enable always-show in
  Settings → rail persists across reload (mock persistence); hover-reveal: dispatch mousemove near
  the right edge → rail mounts, mouseleave + linger → unmounts. Thumb drag → `scrollTop` changes
  proportionally (synthetic pointer events; rAF-paint pixels are not asserted headlessly).
- **Perf:** timed test — `buildRailBuckets` on the 20k mock fixture ≤ a few ms; no scroll-path
  regression with the rail hidden (nothing mounted). Reveal/scroll *feel* on 20k+ = USER
  CHECKPOINT (AC6; headless harness pauses rAF).

## Risks / open questions

- **FLAG (orchestrator): rings-channel parity** — plan assumes ticks mirror whichever rings
  channel is live (commit search OR Ask-history); spec only mandates commit search. Confirm or
  scope to search-only (then `ringsLive = search.open && matches > 0` and historySearch never
  shows ticks while its rings do — inconsistent, hence the recommendation).
- **FLAG: WorkspaceGraphPane.tsx near the 500 cap** (~470 ln) — if the rail threading crosses it,
  extract `railProps.ts` in the same increment.
- **Streaming while visible:** buckets rebuild only at `done`; during a long stream the rail shows
  the previous generation (or nothing on first load). Accepted — the rail is an orientation aid,
  not a live progress bar; ticks (cheap memo) still update live.
- **Spec-004 sequencing:** the identity `RailRowModel` ships first; when fold lands, RepoWorkspace
  swaps in a foldModel-backed impl (`toDisplayRow` = 004's `modelToDisplay` with span-pill clamp)
  and adds `foldModel` identity to the memo keys. No rail-file changes needed if the seam is
  respected — verify at 004/005 integration whichever lands second.
- **Hover reveal shares GraphCanvas's mousemove** — the zone check must not regress the hover
  tooltip path (it's a one-comparison addition; reviewer should confirm no extra `setState` per
  move: `hoverRevealed` only transitions on zone enter/exit).
- Final visuals (colors, pip layout, exact widths, motion on reveal) are the UI contract's call;
  the constants above are implementable defaults.
