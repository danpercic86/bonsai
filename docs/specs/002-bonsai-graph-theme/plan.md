# Plan: Bonsai Graph Theme (organic commit-graph reskin)

**Spec:** ./spec.md
**UI contract:** ../../contracts/002-bonsai-graph-theme-ui.md (owned by `ui-designer`)
**Status:** draft

## Approach
Implement the Bonsai theme as a **frontend-only graph styling variant** layered onto the existing
render pipeline — no changes to Rust, the graph-layout math, or the IPC surface. The graph already
separates *layout* (Rust ships `lane` indices + parent links) from *pixels* (React computes
coordinates in `src/graph/geometry.ts` and paints in `src/graph/draw.ts`, reading a `Theme` object
built by `resolveTheme()` in `src/graph/colors.ts`). We extend that seam: add a **graph-theme**
setting (`standard` | `bonsai`) orthogonal to the existing app light/dark mode, teach
`resolveTheme()` to produce a Bonsai `Theme` (its own light + dark lane palettes plus styling
flags), and branch the node/edge/backdrop drawing on those flags. Topology, ordering, lane
stability, ref pills, selection, and virtualized scrolling are untouched by construction.

Rejected alternative: overloading the existing `theme` dark/light setting with `bonsai-light` /
`bonsai-dark` values. Rejected because app light/dark drives many CSS tokens app-wide via
`data-theme`; the graph theme must be an *independent* axis so Bonsai still follows the app's
light/dark. We therefore add a **separate** `graphTheme` setting.

## Rust/TS boundary
- **Rust:** no change. No git2, no layout math, no new command/event/channel. This is the
  invariant-preserving heart of Version A.
- **TS (render only):** all work is in the frontend graph render layer + settings UI + settings
  persistence (which already round-trips through the existing `setUiSettings` IPC / mock).
- **IPC:** one additive field on the existing UI-settings payload — `graphTheme: 'standard' |
  'bonsai'` — carried by the **existing** `setUiSettings`/`getUiSettings` surface (same mechanism
  as `theme`, `listView`, `panelDensity`). No new IPC command. The mock IPC layer
  (`src/ipc/mock.ts`) must persist it alongside the other UI settings.

## Files touched
- `src/graph/colors.ts` (~140 lines now) — extend `Theme` with Bonsai styling fields (node style,
  edge taper params, backdrop colors, blossom/HEAD accent, seasonal palette selection); add
  `LANE_COLORS_BONSAI_LIGHT` / `LANE_COLORS_BONSAI_DARK` (+ seasonal variants); make
  `resolveTheme()` take the active `graphTheme` (and season) and return the right palette + flags.
  **Watch the ~500-line limit:** the palette tables (standard + Bonsai + seasonal variants) should
  move into a `src/graph/palettes.ts` (or `src/graph/fixtures/`-style data module) so `colors.ts`
  stays logic-only and under the limit — do the split in this increment.
- `src/graph/draw.ts` — branch node drawing (leaf/bud vs avatar disc; blossom treatment for
  HEAD/selected) and `drawEdge` (tapered branch stroke) on the Bonsai flags; add backdrop fill
  before the graph paints. Keep endpoints/coordinates identical. If this pushes `draw.ts` over the
  limit, extract Bonsai node/edge/backdrop painters into a sibling module (e.g.
  `src/graph/drawBonsai.ts`) called from `draw.ts`.
- `src/graph/GraphCanvas.tsx` — pass the active `graphTheme`/season into `resolveTheme()` and
  re-resolve when it changes (mirror the existing `themeVersion` re-resolve at ~line 526); if sway
  is enabled, own its animation-frame lifecycle here (see Risks).
- `src/components/settings/categories/AppearanceCategory.tsx` — add the graph-theme control
  (segmented/dropdown per UI contract) next to the existing dark/light control, plus the seasonal
  selector when Bonsai is active.
- `src/components/settings/**` settings state/actions hook (`useSettingsValues` /
  `useSettingsActions` and the settings catalog that defines `THEME`) — add `graphTheme` (+ season)
  value, action, and default.
- `src/ipc/mock.ts` — persist `graphTheme` (+ season) in the mock UI-settings store so the browser
  harness reflects the setting.
- `src/ipc/types/*` UI-settings type — add the `graphTheme` (+ season) field (additive, optional
  with a `standard` default for back-compat with saved settings).

## New files (if any)
- `src/graph/palettes.ts` — all lane-color tables (standard light/dark already in colors.ts move
  here, plus Bonsai light/dark and seasonal variants). Pure data, keeps `colors.ts` under limit.
- `src/graph/drawBonsai.ts` (only if `draw.ts` would exceed the ~500-line limit) — Bonsai node,
  edge, and backdrop painters, called from `draw.ts`.
- `src/graph/sway.ts` (only if sway is kept) — the sway offset function + reduced-motion gate,
  reused by the painters.

## Data model / types
- **UI settings (additive):** `graphTheme: 'standard' | 'bonsai'` (default `'standard'`), and — if
  seasonal is kept — `graphSeason?: 'spring' | 'summer' | 'autumn' | 'winter'` (default per UI
  contract). Optional/defaulted so existing persisted settings load unchanged.
- **`Theme` (frontend render struct in colors.ts):** add fields per the UI contract, e.g.
  `graphStyle: 'standard' | 'bonsai'`, `nodeStyle`, `edgeTaper` params, `backdrop` colors,
  `blossom`/HEAD accent color. `resolveTheme()` returns these so `draw.ts` branches on a single
  object, not on scattered globals.
- **No Rust/serde changes.** No new wire types for graph data.

## Testing
- **Unit (vitest):** `resolveTheme()` returns the correct palette + flags for each
  (graphTheme × app-light/dark × season) combination; Bonsai palettes have the expected length (10)
  and are distinct from standard; back-compat — settings payload with no `graphTheme` defaults to
  `standard`.
- **Settings round-trip:** toggling graph theme persists through `setUiSettings` and reloads
  (mirror the existing `theme` e2e/localStorage test).
- **Frontend smoke (mock IPC harness, `pnpm dev` + `VITE_MOCK_IPC=1`):** switch to Bonsai in both
  app light and dark; verify graph renders with same node count/topology as standard (compare
  fixture `GraphLayout`), ref pills legible, HEAD/selected/detached/stash/match states visible,
  empty-repo state styled. Screenshot each variant for the visual gate.
- **Reduced-motion:** with `prefers-reduced-motion`, sway is fully static (assert no continuous rAF
  scheduled) — reuse the revealFlash static-fallback pattern.
- **Perf (USER CHECKPOINT-adjacent):** frame-timing on a 20k fixture with Bonsai active vs standard
  — no measurable regression. Note: continuous frame-timing capture is a native/USER-CHECKPOINT
  concern per memory (headless harness pauses rAF); the AI gate covers correctness + static
  screenshots, the user confirms scroll feel.

## Risks / open questions
- **Sway vs the on-demand repaint model (load-bearing).** The graph today only repaints on demand
  (scroll/selection/flash) — there is no continuous animation loop. A perpetual "sway" would force
  a persistent `requestAnimationFrame` loop repainting the whole virtualized canvas every frame,
  even when idle — a real CPU/battery cost, and the exact thing the on-demand model was built to
  avoid. **DECIDED (user): settle-on-scroll only — NO perpetual sway.** The sway is a brief,
  subtle settle that plays for a short bounded window after scrolling stops (or on select), then
  the rAF loop stops and the canvas returns to fully idle — exactly the revealFlash lifecycle
  pattern (bounded animation, then quiet). Strictly gated behind (reduced-motion off) *and* the
  Bonsai theme. `reviewer` must verify no rAF is scheduled once the settle completes and when the
  theme is standard. *(ui-designer's contract pins amplitude/what-moves; implement it as
  bounded-on-scroll, never perpetual.)*
- **File-size limits.** `colors.ts` + new palettes and `draw.ts` + Bonsai painters will approach the
  ~500-line soft limit; the splits above (`palettes.ts`, optional `drawBonsai.ts`) are mandatory in
  this increment, not follow-ups.
- **Palette reuse on wide histories.** With 10 hues, distant lanes reuse colors exactly as today;
  the Bonsai palette must stay distinguishable under reuse in both variants (ui-designer's job).
- **No architecture-invariant conflicts** otherwise: Rust untouched, no blocking calls, no god-file,
  topology/ordering/product decisions all preserved. This stays within the `/specify`→`/plan`→
  `/tasks` lane and does **not** need escalation to a full milestone — with the caveat that it's on
  the *larger* end of a spec-flow item (several files, 2 subagents), so `/tasks` should split it
  into a few coherent senior-dev increments (palettes+resolveTheme; node/edge/backdrop painters;
  settings wiring+persistence; flourishes/sway last).
