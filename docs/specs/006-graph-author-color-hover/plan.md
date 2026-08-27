# Plan: Author Coloring & Parent-Highlight on Hover

**Spec:** ./spec.md
**UI contract:** ../../contracts/spec-006-ui.md (to be written — step 2b REQUIRED, user-visible)
**Status:** planned

## Approach

Frontend-only except the pref plumbing (exact `graphFirstParent`/`graphStyle` precedent). Two
independent features sharing one increment:

1. **Author coloring mode** — a persisted pref `graphColorMode: 'lane' | 'author'` threaded into
   the paint layer via `GraphDisplayOptions`. In author mode, each edge and each node's lane ring
   take the **child commit's author hue** (`nodes[e.from].author` for edges; `node.author` for the
   ring), derived with the same FNV-1a hue the avatars use, but with **per-theme S/L constants**
   (like the lane palettes) so edges meet contrast on light+dark × standard+Bonsai. Avatars,
   pills, backdrop, blossoms: unchanged.
2. **Parent highlight** — an emphasis re-stroke of the hovered/selected row's direct parent edges
   plus an outer ring on the parent nodes. **All in display space:** targets are computed by
   filtering the already-projected `visibleEdges` for `e.from === targetRow` — fold projection
   (spec-004) has already dropped edges into folded spans and remapped indices, so `node.parents`
   (MODEL indices) is never consulted and AC5 falls out for free. `targetRow = hoverRow ??
   selectedIndex` (both already display-space inside `drawGraph`).

**Decisions recorded:**

- **Pref shape:** `graphColorMode: 'lane' | 'author'` (default `'lane'`), global persistence,
  two-value segmented control. **Placement:** spec says Appearance; spec-003 precedent used the
  Commit-graph category — defer to the spec-006 UI contract. FLAG for ui-designer.
- **Author edge color = same hue as `avatarColor`, per-theme S/L.** Export the hue from
  `geometry.ts` (new `authorHue(name): number` wrapping the private `hashString`; `avatarColor`
  refactors onto it — byte-identical output). New `authorColor.ts` composes
  `hsl(hue, S, L)` with constants per (dark|light) — placeholder starting points, final values
  owned by ui-designer in the UI contract: dark `S 60% / L 62%`, light `S 62% / L 40%` (lane-palette
  ballpark). One set serves both graph styles (Bonsai palettes differ only in curation, not
  formula); UI contract may add Bonsai-specific constants if contrast review demands. Cache:
  **hue by name** (Map, unbounded within a mount — author cardinality is small), HSL string
  composed per resolved theme; never a name→string cache (stale on theme flip).
- **Unknown/empty author:** `authorHue('')` is well-defined (same trim+hash path the avatar
  fallback uses) — no special case.
- **Highlight paint order — inside `drawGraph`, not after it.** A post-`drawGraph` pass would
  stroke over avatar halos/discs. Instead: **pass 3.5** (right after the edge loop) re-strokes the
  target's parent edges with emphasis; **pass 4** adds a parent-node outer ring in the existing
  `matchRows` pattern. Logic lives in a new file (draw.ts is at ~449 ln, near cap); draw.ts gains
  only the two call sites (~15 ln).
- **Emphasis mechanics (contract-level tokens, final values = ui-designer):** edge re-stroke in
  the edge's own (mode-resolved) color at `edgeWidth + 1.5` (Bonsai: stepped widths + 1.5 via a
  new optional `emphasis?: number` param on `drawEdge`/`drawBonsaiEdge` — no mutated theme copy);
  parent ring at `avatarSelRingRadius + 1.5`, width 1.5, same mode-resolved color. Composes with
  selection/HEAD/match rings (distinct radius/color).
- **No global dim in v1** (spec: "optionally"). Skipped for visual churn on every hover;
  recorded as a ui-designer option (a `highlightDimAlpha` token) — mechanically trivial to add
  later (globalAlpha around passes 3–4 for non-target rows) but off by default.
- **Hover tracking already exists — spec-006 adds ZERO mousemove code.** `Interaction.hoverRow`,
  `hoverRowRef`, repaint-only-on-change (`GraphCanvas.tsx:718–720`) and mouseleave nulling
  (`:751–753`) all shipped earlier. That is the spec-005 composition answer: no touch of
  `handleMouseMove`/`handleMouseLeave`, so concurrent rail-hover/fold-cursor work cannot conflict.
  AC3 (no idle repaints) is satisfied by the existing mechanism.
- **Fold pill rows:** conservative — hovering a pill row shows **no** highlight (pill nodes have
  `parents: []` and projection drops their edges, so the display-space filter yields nothing by
  construction; additionally guard `foldRows.has(targetRow)` → skip, so no ring pass runs). No
  span-boundary highlighting in v1.
- **Keyboard path:** selection-driven — `targetRow = hoverRow ?? dSel.row`; arrow-key selection
  moves the emphasis with it. Parent-of-selected emphasis composes with the selection ring (spec).
- **Root commits / merges / off-screen parents:** root → filter yields no edges; merge → one
  emphasis per parent edge; off-screen parent → the edge re-stroke uses the same clamped geometry
  as pass 3 (visible portion emphasized), ring skipped when the parent row is outside
  first/lastRow — all fall out of reusing the pass-3 painters.

## Rust/TS boundary

No Git/graph logic changes. Rust touch is ONLY the opaque UI pref (`graph_color_mode`) in the
settings snapshot/patch — graphStyle precedent. All coloring + highlight math is paint-time in
`src/graph/` (React renders; but note this is canvas paint, not layout — the layout invariant is
untouched).

## Files touched

- `src/graph/geometry.ts` (~109 ln): export `authorHue(name: string): number`; `avatarColor`
  delegates to it (output byte-identical; existing avatar vitest guards this) (~8 ln).
- `src/graph/draw.ts` (~449 ln, NEAR CAP — call sites only): `drawEdge` gains
  `color: string` + `emphasis?: number` params (strokeStyle/lineWidth resolved by caller);
  `drawGraph` resolves per-edge color (`display.colorMode === 'author' ? authorEdgeColor(...) :
  theme.laneColors[e.lane % 10]`), calls `strokeHighlight` after the edge loop (pass 3.5), uses the
  mode-resolved color for the avatar lane ring in pass 4, and adds the parent-ring branch beside
  the `matchRows` ring (~30 ln net; if this pushes past 500, extract per the risk note below).
- `src/graph/drawBonsai.ts`: `drawBonsaiEdge` gains `color: string` + `emphasis?: number`
  (widths become `theme.edge*Width + (emphasis ?? 0)`) (~10 ln).
- `src/graph/rightColumns.ts`: `GraphDisplayOptions` gains `colorMode: 'lane' | 'author'` (~3 ln).
- `src/graph/GraphCanvas.tsx` (~900 ln container, known exception — delta tiny): accept
  `colorMode` prop, include in the `display` object + paint deps (~6 ln).
- `src/components/repoWorkspace/WorkspaceGraphPane.tsx` (or wherever `display` is assembled —
  follow the `graphStyle` prop chain): thread `colorMode` from `useUiSettings` (~6 ln).
- `src/hooks/useUiSettings.ts`: `graphColorMode` state + patch + hydrate (graphStyle precedent)
  (~10 ln).
- `src/settings/uiSettingsDefaults.json` + `src/settings/defaults.ts`: `graphColorMode: 'lane'`
  (~4 ln).
- `src-tauri/src/settings.rs` + `src-tauri/src/commands/ui_settings.rs`: `graph_color_mode:
  String` (opaque, default "lane") — snapshot + patch (~15 ln).
- `src/ipc/mock/persistence.ts`: validate + round-trip (`'lane'|'author'`, malformed → `'lane'`)
  (~8 ln).
- Settings category file (Appearance or Commit-graph per UI contract): compose the new segmented
  row component (~5 ln).

## New files

- `src/graph/authorColor.ts` (~50 ln): per-theme S/L constants; `authorEdgeColor(name, dark:
  boolean): string`; hue memo Map; `resetAuthorColorCache()` for tests.
- `src/graph/highlight.ts` (~60 ln): pure
  `highlightTargets(visibleEdges, targetRow, foldRows): { edges: GraphEdge[]; parentRows: number[] }`
  (display-space filter + pill guard) — unit-testable, no canvas; plus
  `strokeHighlight(ctx, targets, nodes, vp, theme, display, m)` which re-invokes the pass-3 edge
  painters with `emphasis` (pass 3.5) and exposes `parentRows` for the pass-4 ring branch.
- `src/components/settings/SettingsGraphColorModeSection.tsx` (~50 ln, per UI contract):
  segmented control, registered in the settings catalog.

## Data model / types

```ts
// settings / display
export type GraphColorMode = 'lane' | 'author';
// UiSettings: graphColorMode: GraphColorMode (default 'lane')
// GraphDisplayOptions gains: colorMode: GraphColorMode

// src/graph/highlight.ts
export interface HighlightTargets {
  edges: GraphEdge[];      // display-space parent edges of targetRow
  parentRows: number[];    // display rows to ring (may be off-viewport → skipped at paint)
}
```

```rust
// src-tauri/src/settings.rs — opaque pref, graphStyle precedent
// graph_color_mode: String (serde default "lane")
```

No IPC/graph wire changes; mock graph handlers untouched.

## Testing

- **Vitest (pure):**
  - `authorHue`: deterministic, trim/fallback parity with `avatarColor` (same name ⇒ avatar hue ==
    edge hue); `avatarColor` output unchanged (regression).
  - `authorColor`: per-theme constants applied; cache keyed by name only for hue (theme flip
    yields new string).
  - `highlightTargets`: root → empty; merge → 2 edges + 2 parentRows; fold-pill target → empty
    (guard) ; parent inside a folded span → that edge absent (projection input fixture); identity
    projection == unfolded behavior; targetRow null → empty.
  - Settings round-trip + hydration default + malformed persisted value → 'lane' (mock
    persistence).
- **E2e smoke (mock harness):** toggle author mode in Settings → canvas repaints without console
  errors, pref survives reload; mousemove across rows + mouseleave → no errors. Canvas pixel
  assertions are unreliable headless — limit to error-free repaint + persistence.
- **USER CHECKPOINT:** visual legibility of author colors and the highlight in all 4
  theme×style combos; hover feel/idle CPU in the native window (headless harness pauses rAF).
- AC map: AC1 = pref + mode-resolved colors (vitest + checkpoint); AC2 = highlightTargets +
  checkpoint; AC3 = existing repaint-on-change (no new code; e2e no-error); AC4 = checkpoint +
  ui-designer contrast review; AC5 = display-space filter (vitest fold fixtures).

## Risks / open questions

- **draw.ts is ~449 ln** — the ~30-ln delta keeps it under 500, but it has no headroom left; if
  review lands it over, extract the edge loop (pass 3) into `drawEdges.ts` in the same increment.
- **GraphCanvas.tsx ~900 ln** (known exception): delta is ~6 ln; spec-005 is concurrently editing
  it — spec-006 touches only the `display` object assembly + props, not mousemove/paint plumbing,
  so a trivial merge at worst. Land whichever is second as a rebase over the first.
- **Contrast of hashed hues is not guaranteeable per-name** (360 free hues vs a curated 10-color
  palette): the per-theme S/L constants bound worst-case contrast, but ui-designer must sign off
  the constants (FLAG). Yellows/cyans at fixed S/L are the risk band.
- **Settings category placement** (Appearance vs Commit-graph) — FLAG, ui-designer decides.
- Same-author repos render single-hue in author mode — accepted per spec.
- No dim in v1 — recorded above; revisit only if the checkpoint finds the emphasis too subtle.
