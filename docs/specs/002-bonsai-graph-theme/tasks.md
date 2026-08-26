# Tasks: Bonsai Graph Theme (organic commit-graph reskin)

**Plan:** ./plan.md
**Spec:** ./spec.md
**UI contract:** ../../contracts/002-bonsai-graph-theme-ui.md
**Status:** ready

- [x] 1. Palettes + Theme extension: create `src/graph/palettes.ts` (move existing standard
  light/dark tables out of `colors.ts`; add `LANE_COLORS_BONSAI_LIGHT` / `LANE_COLORS_BONSAI_DARK`
  and the seasonal accent/backdrop variants — Living/Spring/Autumn — with hex values from the UI
  contract §5.1). Extend the `Theme` interface in `src/graph/colors.ts` with the Bonsai styling
  fields (graphStyle, node/blossom style, edge-taper widths tip/branch/trunk, backdrop colors) and
  make `resolveTheme()` accept the active `graphStyle` + `graphSeason` and return the right palette
  + flags. Keep `colors.ts` logic-only and under the ~500-line limit. — `src/graph/palettes.ts`,
  `src/graph/colors.ts` — owner: senior-dev
- [x] 2. Settings wiring + persistence: add `graphStyle: 'standard' | 'bonsai'` (default
  `'standard'`) and `graphSeason` (default per contract, e.g. `'living'`) to the UI-settings type,
  the settings store/catalog + `useSettingsValues`/`useSettingsActions`, the `AppearanceCategory`
  control (Segmented **Graph style** Standard/Bonsai + a **Season** Combobox in a
  `<fieldset disabled>` when Standard is active, per contract §settings), and the mock IPC store so
  the browser harness persists them. Fields additive/optional for back-compat with saved settings.
  — `src/ipc/types/*` (UI settings), `src/components/settings/**` (catalog + hooks +
  `categories/AppearanceCategory.tsx`), `src/ipc/mock.ts` — owner: senior-dev
- [x] 2b. Rust settings persistence: add `graph_style` / `graph_season` serde fields to the
  native UI-settings struct in `src-tauri/**/settings/prefs.rs` (+ the `ui_settings_of` mapping and
  the defaults oracle / parity test), so `setUiSettings` persists them across a native app restart
  (acceptance #3). Additive, defaulted for back-compat with existing on-disk settings. This is
  settings storage only — NOT git or layout logic, so the Rust/React invariant is preserved.
  — `src-tauri/**/settings/prefs.rs` (+ oracle) — owner: senior-dev
- [x] 3. Bonsai painters: branch node + edge + backdrop drawing on the Theme flags. Nodes keep the
  avatar + rings (rings remain the state carriers) and add the **additive blossom** — full 5-petal
  for HEAD, single top-bud for selected — with distinct silhouettes; edges use stepped `lineWidth`
  across the existing three bezier segments (endpoints untouched); paint the near-flat backdrop
  before the graph and switch the avatar halo base to the backdrop color; empty/unborn-HEAD reuses
  the empty-state as an "empty pot". Extract Bonsai painters into `src/graph/drawBonsai.ts` if
  `draw.ts` would cross the ~500-line limit. Wire `GraphCanvas.tsx` to pass `graphStyle`/`season`
  into `resolveTheme()` and re-resolve on change (mirror the existing `themeVersion` re-resolve).
  — `src/graph/draw.ts`, `src/graph/drawBonsai.ts` (if split), `src/graph/GraphCanvas.tsx` —
  owner: senior-dev
- [x] 4. Settle-on-scroll sway: `src/graph/sway.ts` (bounded settle offset, ≤1px, node glyphs
  only — edges never move) triggered after scroll stops / on selection change, self-terminating
  like the revealFlash lifecycle (rAF stops when the settle completes; nothing scheduled at idle or
  when graphStyle is standard). Zero motion under `prefers-reduced-motion`. Wire its lifecycle in
  `GraphCanvas.tsx`. — `src/graph/sway.ts`, `src/graph/GraphCanvas.tsx`, `src/graph/drawBonsai.ts`
  — owner: senior-dev
- [x] 3c. [P] DOM backdrop token: implement UI contract §4.2 — a `--graph-canvas-bg` token and a
  `data-graph-style="bonsai"` root attribute so the graph-pane container, load skeleton, and error
  state sit on the Bonsai backdrop color (matching the canvas). CSS/DOM only, no canvas painter.
  — `src/styles.css` (+ the root attribute setter), owner: senior-dev
- [x] 5. Review changes from tasks 1–4 — correctness, Rust/React boundary (must stay React-only),
  the ~500-line splits, and **verify the idle path stays quiet**: no rAF scheduled once the settle
  completes and none when graphStyle is standard. Check against spec acceptance criteria + UI
  contract. — owner: reviewer
- [x] 6. Tests: unit (`resolveTheme()` returns correct palette+flags for each
  graphStyle × app-light/dark × season; Bonsai palettes length 10 and distinct; missing-field
  settings default to `standard`); settings round-trip through `setUiSettings`/reload; frontend
  smoke via mock harness (Bonsai in both app modes renders same topology/node-count as standard,
  ref pills legible, HEAD/selected/detached/stash/match/empty states visible — screenshots for the
  visual gate); reduced-motion assert no continuous rAF. — owner: tester

## Notes
- **Naming:** use `graphStyle` (values `standard` | `bonsai`) and `graphSeason` — matches the UI
  contract and the "Graph style" settings label. Do not reintroduce the earlier `graphTheme` name.
- **Invariant:** rendering is React-only and no graph-layout/topology math or git logic moves to
  TS; no new IPC command — settings ride the existing `setUiSettings`/`getUiSettings` surface. The
  ONE Rust change (task 2b) is additive settings STORAGE (graph_style/graph_season serde fields) so
  the pref survives a native restart — not git or layout logic. A reviewer finding of any
  graph-layout/topology/git change in Rust or TS is a MUST-FIX.
- **Sway is settle-on-scroll only** (bounded, self-terminating) — NOT perpetual/idle. This overrides
  any "idle-only rAF loop" wording; the UI contract is being corrected to match. Preserve the
  graph's on-demand repaint model.
- **Palette values are canonical in the UI contract §5.1** — pass its hex tables to senior-dev
  verbatim; don't re-derive hues (they were chosen for ≥5:1 backdrop contrast and ≥4.5:1
  `adaptivePillText`).
- **USER CHECKPOINTs** (cannot be AI-gated per the headless-harness rAF limitation): sway motion
  feel and 20k-row scroll frame timing with Bonsai active. Present AI-gate evidence (screenshots +
  correctness), then ask the user to confirm in `pnpm tauri dev`.
- Delegate per `CLAUDE.md`: pass this file + plan.md + spec.md + the UI contract **paths** and the
  target **file paths**; commit `wip(spec-002): ...` after reviewer approves each increment.
