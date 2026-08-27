# Plan: Replay Mode — Animated History Playback

**Spec:** ./spec.md
**UI contract:** ../../contracts/spec-007-ui.md (ui-designer; transport visuals, entry-point placement, copy)
**Status:** planned

## Approach

Frontend-only presentation mode (brief #5). All data replay needs is already in the assembled
`GraphLayout` (`nodes` with `ts`/`committerTs`, `edges`, refs). **No Rust changes, no new IPC, no
persisted pref** — replay state is transient session state.

**Architecture: overlay, not swap.** A new `ReplayMode` component is absolutely positioned over
the graph pane; the working `GraphCanvas` stays mounted underneath with `active={false}` (its
documented contract: freezes on the retained last-good bitmap, remeasures + repaints when flipped
back true — the P3e tab-hide path, reused verbatim). Exit = unmount the overlay + `active=true`.
Because the working canvas, layout object, fold model, filter state, selection, and its scroller
element are never touched, AC2/AC6 (exact restore, zero mutation) hold by construction — no
snapshot/restore machinery beyond the overlay's own lifecycle.

**Reveal direction (load-bearing — the brief's "top-to-bottom" sketch is inverted).** Layout row 0
is the NEWEST commit (revwalk children-first; edges satisfy `to > from`, `from` = child row). The
spec plays oldest → newest, so the reveal cutoff `c` starts at `n` and animates **down to 0**,
revealing rows `[c, n)` — the graph grows upward from the bottom. The clamp is a **min-row**
clamp. Edge visibility rule: an edge is painted iff `from >= c` (a revealed child's edge to its
already-revealed parent; `to > from >= c` always holds for the parent end). Ref pills render only
on revealed rows (drawGraph does this for free — it paints pills per visible node row).

**Paint seam: zero `draw.ts` edits.** Replay's painter constructs a `Viewport` whose row window is
`[max(scrollFirstRow, c), scrollLastRow]`, pre-filters the edge list to `from >= c`, and calls the
existing `drawGraph` unchanged (`wip: null`, `matchRows: []`, `flash: null`,
`selectedIndex: null`, hover per transport hit-test only). Spec-006's concurrent `colorMode` /
highlight additions to `draw.ts` pass straight through via `GraphDisplayOptions` — no contention;
replay never modifies `draw.ts`/`drawBonsai.ts`.

**Row space: replay runs over the UNFOLDED model-space layout** (narrative clarity — folded runs
are exactly the linear stretches a story should show growing; a fold pill "appearing" reads as a
glitch). The fold model and `expandedSpans` are read-only inputs replay ignores; they are intact
on exit. Active filters are already baked into the loaded layout, so "respects filters" is free;
filter/fold/reload controls are inert while replay is up (UI contract: transport bar owns input).

## Decisions recorded

1. **Cutoff semantics:** integer row-index cutoff `c ∈ [0, n]` in the existing topo display order
   (`n` = nothing revealed, `0` = everything). Timestamps only pace the clock, never reorder.
2. **Clock mapping (pure module `replayModel.ts`):** precompute once per entry a cumulative
   narrative-time array `T[i]` over rows walked oldest→newest, from `committerTs` deltas clamped
   to `[MIN_STEP_MS, MAX_STEP_MS]` narrative units (long real-world gaps compress, bursts stay
   visible). O(n) build at 100k; playhead→cutoff and scrub-position→cutoff are binary searches
   over `T` — no per-commit stalls (spec edge case). Total duration normalized to a target
   wall-clock length (~45 s at 1×, clamped [10 s, 90 s]).
3. **Scrub granularity:** the scrubber maps pointer x → fraction → binary search into `T` → row
   cutoff. At 100k rows one pixel spans many rows; that is fine — the state machine is row-exact,
   the control is continuous. Keyboard: ←/→ step the cutoff by max(1, n/500) rows.
4. **Driver:** one self-terminating rAF loop on the `useSway` pattern (`useReplayDriver.ts`):
   scheduled ONLY while `status === 'playing'`; cancels on pause/finish/exit/unmount and suspends
   on `document.hidden`. Paused/finished/scrubbing states schedule zero frames — a scrub while
   paused paints exactly once per change (AC4). **Entry behavior:** on mount, `ReplayMode` calls
   `play()` when `model.canPlay` (spec AC1 autoplay); under reduced motion / empty layout it stays
   paused at the static scrubber.
5. **Speed:** steps 1× / 2× / 4× (multiplier on narrative-clock advance). Changing speed mid-play
   is seamless (the model advances by `dt * speed`, no re-derivation).
6. **Auto-follow:** while playing, the overlay's own scroller pins the frontier row (`c`) near the
   **top** of the viewport, so the view shows revealed rows `[c .. c+H−1]` filling downward (rows
   above `c` are unrevealed blank space and stay off-screen). A user wheel/drag disengages follow
   (flag) until play is next pressed. The working canvas's scrollTop is untouched throughout.
7. **Reduced motion:** `prefers-reduced-motion` ⇒ the model's `play()` is a structural no-op
   (`canPlay: false`); entry shows the full graph at the scrubbed position with the scrubber as
   the only control — no rAF ever scheduled (mirrors `revealFlash.ts` handling).
8. **Bonsai variant v1:** newly-revealed frontier rows get a revealFlash-style pulse (alpha ring /
   row tint driven by rows-revealed-this-tick, painted by `replayPaint.ts` AFTER `drawGraph`, on
   top). Reuses `flashAlpha`-style easing; works in both styles, reads as "sprouting" under
   Bonsai's leaf/blossom dots. A full bespoke sprout/grow animation (scale-in glyphs) is recorded
   as a follow-up, not v1. **FLAG for orchestrator:** AC5 is satisfied by this pulse
   interpretation; confirm with ui-designer.
9. **Entry points:** a command-palette action ("Replay history…", via `buildPaletteActions` in
   `src/components/paletteActions.ts` — palette exists) + a graph-pane control (placement =
   ui-designer). Disabled/no-op with a notice when the layout is empty (unborn repo). Escape and
   a close control exit.
10. **State preservation:** nothing to restore by design (overlay + frozen canvas). The overlay
    keeps its own scroll element; selection/scrollTop of the working graph are simply never
    written.
11. **Truncated graph:** replay covers the loaded rows; the existing truncated indicator stays
    visible in the transport bar area (UI contract).

## Files touched

- `src/components/WorkspaceGraphPane.tsx` — **exactly at the 500-line cap (FLAG)**: only the
  minimal seam goes here (~8 ln): render `<ReplayMode …/>` from a grouped `replay` prop and pass
  `active={!replayOpen}` to GraphCanvas. All prop assembly lives in the new `replayProps.ts`
  (spec-005 `railProps.ts` precedent). If even that overflows the cap, extract in-increment.
- `src/components/RepoWorkspace.tsx` (>500 container exception; delta ~15 ln): `replayOpen`
  state, palette action registration, Escape handling delegation, empty-layout guard.
- `src/components/paletteActions.ts` — one `PaletteAction` entry (~6 ln).
- `src/components/repoWorkspace/useWorkspaceKeyboard.ts` — Escape-exits-replay precedence + entry
  shortcut if the UI contract assigns one (~6 ln).
- CSS: new `src/styles/graph-replay.css` (imported once) — overlay + transport bar tokens per UI
  contract.

## New files

All replay logic under `src/graph/replay/` (each well under 500 ln):

- `replayModel.ts` (~150 ln) — the pure state machine + clock mapping. No DOM, no React.
- `useReplayDriver.ts` (~120 ln) — rAF driver (useSway pattern), reduced-motion gate,
  auto-follow flag, document.hidden suspend.
- `replayPaint.ts` (~100 ln) — builds the clamped `Viewport` + filtered edge slice, calls
  `drawGraph`, then paints the frontier pulse. Owns NO topology.
- `ReplayMode.tsx` (~200 ln) — overlay container: own canvas + scroller (reusing
  `useCanvasResizeObserver`, `viewport.ts` helpers, theme resolve identical to GraphCanvas's),
  wires model + driver + transport, autoplay-on-mount (decision 4), focus trap, Escape.
- `ReplayTransport.tsx` (~150 ln) — presentational transport bar: play/pause, scrubber, speed,
  close, progress label (visuals per UI contract).
- `src/components/repoWorkspace/replayProps.ts` (~60 ln) — assembles ReplayMode props from
  workspace state (layout, metrics, display, graphStyle/season, themeVersion, reducedMotion).
- Tests: `src/graph/replay/replayModel.test.ts`, `replayPaint.test.ts`;
  `e2e/31-graph-replay.spec.ts`.

## Data model / types

```ts
// src/graph/replay/replayModel.ts (pure)
export type ReplayStatus = 'paused' | 'playing' | 'finished';
export type ReplaySpeed = 1 | 2 | 4;

export interface ReplayModel {
  readonly n: number;                 // total rows
  readonly cumTime: Float64Array;     // T[i], narrative ms, oldest→newest walk
  readonly totalMs: number;           // normalized 1× duration
  readonly canPlay: boolean;          // false under reduced motion or n === 0
}
export interface ReplayState {
  readonly status: ReplayStatus;
  readonly playheadMs: number;        // narrative clock ∈ [0, totalMs]
  readonly cutoff: number;            // derived row cutoff c ∈ [0, n]; reveal = [c, n)
  readonly speed: ReplaySpeed;
  readonly follow: boolean;           // auto-follow engaged
}
export function buildReplayModel(layout: GraphLayout, reducedMotion: boolean): ReplayModel;
export function initialState(m: ReplayModel): ReplayState;      // cutoff = n, status 'paused'
export function tick(m: ReplayModel, s: ReplayState, dtMs: number): ReplayState; // playing only
export function scrubTo(m: ReplayModel, s: ReplayState, fraction: number): ReplayState;
export function play(m: ReplayModel, s: ReplayState): ReplayState;   // no-op if !canPlay
export function pause(s: ReplayState): ReplayState;
export function setSpeed(s: ReplayState, sp: ReplaySpeed): ReplayState;
export function cutoffForPlayhead(m: ReplayModel, ms: number): number; // binary search
```

`ReplayMode` props (assembled by `replayProps.ts`): `{ layout, metrics, display, graphStyle,
graphSeason, themeVersion, reducedMotion, onExit(): void }`. No IPC surface; the mock harness
exercises replay on fixture layouts unchanged.

## Testing

- **vitest `replayModel`:** T[] monotonic + clamped deltas; cutoff monotonically decreases under
  `tick`; scrub↔cutoff inversion (scrubTo(fraction) then position round-trips); speed change
  mid-play continuous; `finished` at playhead=totalMs; edge cases n=0 (canPlay false), n=1;
  reduced motion ⇒ `play` no-op.
- **vitest `replayPaint`:** viewport min-row clamp; edge filter `from >= c`; pills/nodes below
  cutoff absent (assert via draw-call capture on a stub ctx, existing draw-test pattern).
- **e2e (`31-graph-replay.spec.ts`, mock harness):** enter via palette → overlay visible, working
  canvas frozen; exit via Escape → selection + scrollTop identical to before entry, no overlay
  DOM remains; scrub while paused updates the revealed count label; `emulateMedia({
  reducedMotion: 'reduce' })` → play button absent/disabled, scrubber works. **No-rAF assertion
  (headless-viable): wrap `window.requestAnimationFrame` via init script and count scheduling
  calls** — headless pauses rAF *firing*, not scheduling; assert zero new schedules while
  paused/after exit.
- **USER CHECKPOINT:** playback feel/smoothness on a large repo, auto-follow feel, Bonsai pulse
  aesthetics — native `pnpm tauri dev` only (headless pauses rAF).

## Risks / open questions

- **AC5 interpretation** (frontier pulse = "sprout" for v1) — flagged above for ui-designer/
  orchestrator sign-off; bespoke grow animation is a recorded follow-up.
- `WorkspaceGraphPane.tsx` at exactly 500 lines — the seam must stay ≤ ~8 lines or trigger an
  in-increment extraction. `RepoWorkspace.tsx` remains the allowed >500 container.
- Spec-006 lands concurrently in `draw.ts`; replay depends only on `drawGraph`'s public signature
  (+ `GraphDisplayOptions` passthrough). If 006 changes that signature, `replayPaint.ts` adapts —
  single call site.
- Duplicated theme-resolve/canvas-setup between GraphCanvas and ReplayMode: acceptable (reuses the
  shared helpers `useCanvasResizeObserver` / `viewport.ts` / `resolveTheme`); do NOT refactor
  GraphCanvas in this increment.
- 100k-row `Float64Array` build on entry (~O(n)) is one-time and off the paint path; if entry lag
  is perceptible it can move to an idle callback — measure first.
