# Commit graph — design review + resolution plan (2026-08-22)

Owner: ui-designer. Status: **decisions landed (M3/M4 approved 2026-08-22) — ready for senior-dev.**
Scope: the canvas commit graph and everything drawn on/around it — lanes, lane colors, edges,
commit nodes, ref pills, row layout/density, selection/hover, empty & unborn states, keyboard/a11y,
light+dark. Reviewed against `docs/contracts/ui-reference.md` §4–§6/§8 and the P84 reveal contract.

Verification: code (`GraphCanvas.tsx`, `draw.ts`, `metrics.ts`, `colors.ts`,
`useWorkspaceKeyboard.ts`, `WorkspaceGraphPane.tsx`) + browser-harness DOM/token inspection
(`VITE_MOCK_IPC=1`) + computed WCAG ratios. Frame-timing/scroll-feel NOT assessed — the headless
harness pauses rAF; those remain USER CHECKPOINT items.

`ui-reference.md` §4 (avatar node + a11y), §5 (dual lane palettes) and §6 (adaptive pill text +
HEAD pill) were **already updated in this pass** to match the fixes below. Implement to those
sections; the concrete values are duplicated here for convenience.

---

## MUST-FIX

### M1 — Make the graph a focusable, announced composite widget
**Where:** `GraphCanvas.tsx:910-924` (host/scroller markup); nav handlers `useWorkspaceKeyboard.ts:218-335`.
**Confirmed in harness:** `.graph-scroll` has `tabIndex=-1`, `role=null`, `aria-label=null`,
`aria-activedescendant=null`. Canvas content is opaque to SR; the only live region that speaks is
P84's `RevealAnnouncer`, and only on sidebar reveal.
**Fix (spec §4.1):**
- On the scroller: `tabIndex={0}`, `role="grid"`, `aria-label="Commit graph"`,
  `aria-rowcount={totalRows ?? layout.nodes.length}`, and
  `aria-activedescendant={selectedIndex!=null ? \`graph-row-${selectedIndex}\` : undefined}`.
- Add a permanently-mounted polite live region (own the `RevealAnnouncer` pattern in
  `WorkspaceGraphPane.tsx` or a sibling) updated on settled selection with
  `"{summary} — {author}, {relativeDate}. Row {n+1} of {N}. {ref summary}"`, debounced ~150 ms.
- `:focus-visible` ring on `.graph-scroll` — 2px `--accent`, 1px offset, inset (S2). Add to
  `src/styles/graph-canvas.css` (`.graph-scroll:focus-visible`).
**Acceptance:** Tab reaches the graph (announced "Commit graph, grid"); arrow-nav announces each
landing commit with message/author/date/position; `aria-activedescendant` tracks selection; a
`:focus-visible` ring shows on keyboard focus only.

### M2 — Keyboard nav must start with no prior selection
**Where:** `useWorkspaceKeyboard.ts:306-334` — Arrow/Page/Home/End all `return` when
`selectedIndex === null`.
**Fix (spec §4.1):** when the graph is focused and selection is null, the first
ArrowDown/Home/PageDown selects `graph.headIndex` if in range else `0`; the first ArrowUp/End
selects `graph.nodes.length - 1`; PageUp with none → `0`. Then existing ±1 / ±visible-count /
first-last deltas apply. (`headIndex` is on `GraphLayout`; `getVisibleRowCount()` already exposed.)
**Acceptance:** from a fresh load with nothing selected, focus the graph + ArrowDown → a commit is
selected and announced; no mouse required.

### M3 — Theme-specific lane palettes (light palette darkened to ≥3:1 vs #ffffff)
**Where:** lane colors resolved for the draw pass — `colors.ts` / `draw.ts` (currently a single
theme-invariant palette). ui-reference §5 rewritten.
**Decision (approved):** add a **separate light-mode lane palette**; keep the dark palette exactly
as-is. Select by resolved theme (the draw pass already has `themeRef`/`resolveTheme`).
**Concrete values** (ratio = lane vs that theme's `--bg-0`; all ≥4.6:1):

| # | Hue | Dark (unchanged) | Light (new) |
|---|---|---|---|
| 0 | blue | `#4f8cff` | `#2f6fe4` |
| 1 | orange | `#f2994a` | `#b0530f` |
| 2 | purple | `#9b6dff` | `#7b46d6` |
| 3 | green | `#43b97f` | `#1b7d4c` |
| 4 | red | `#e5534b` | `#c62f33` |
| 5 | teal | `#3ec6c0` | `#0c7d78` |
| 6 | yellow | `#e8c341` | `#8a6f08` |
| 7 | pink | `#f26d9c` | `#c8437a` |
| 8 | indigo | `#7a86ff` | `#5560e0` |
| 9 | lime | `#8fbf4d` | `#517c20` |

**Acceptance:** in light theme every lane stroke/dot measures ≥3:1 vs `#ffffff` (measured 4.63–5.69);
dark palette byte-identical; hue order preserved; scrolling stability unchanged.

### M4 — Ref-pill text contrast (adaptive current-branch text + fixed detached-HEAD bg)
**Where:** pill draw in `draw.ts` / `refLabels.ts` (`entityStyle`). ui-reference §6 rewritten.
**Decisions (approved):**
- **Current-branch pill** (solid lane-color bg): label color = whichever of near-black `#16181d`
  or white `#ffffff` has the higher contrast with the lane bg. Reuse the existing `isDarkBg()`
  helper (`GraphCanvas.tsx:66`) — lift it to the pill draw. Result: near-black on dark-mode bright
  lanes = 4.8–10.4:1; white on light-mode darkened lanes = 4.6–5.7:1 (all ≥4.5).
- **Detached-HEAD pill**: fixed dark-red background **`#b3261e`** (both themes, replacing `--danger`
  which gave 3.70:1 in dark) + white text = **6.54:1** both themes.
**Acceptance:** current-branch and detached-HEAD pill labels measure ≥4.5:1 in both themes across
all ten lanes.

---

## SHOULD-FIX

### S1 — ui-reference §4 accuracy (DONE in this pass)
§4 now documents the author-avatar node (disc r=10 cozy / 8 compact, bg-0 halo, 1.5px lane ring,
initials), the HEAD (r=avatarRadius+2.5, `--text-1`) and selection (r=avatarRadius+3.5, `--accent`)
rings, the match ring, and both densities (row 32 cozy / 22 compact). No code change — spec-accuracy
only. Kept here so senior-dev knows the "4px lane dot" language is retired.

### S2 — Focus indicator distinct from selection
Folded into M1 (the `:focus-visible` container ring). Listed separately so it is not lost: the
per-row `--accent` selection ring and the container focus ring are two different affordances.
**Acceptance:** keyboard focus paints the container ring; mouse click does not.

### S3 — Match ring vs pink lane (verify only)
`--match-ring` (#ff4dd2 dark / #c026a3 light) vs lane-7 pink. Confirm the match-ring radius delta
(`draw.ts:425`) keeps a match on a pink-lane commit distinct; nudge `--match-ring` bluer only if it
does not. No change unless the visual check fails.

---

## NIT

### N1 — Empty-state 🌱 emoji
`WorkspaceGraphPane.tsx:274`. Outside the §7 glyph vocabulary; per-OS rendering varies. Acceptable
as brand flavor on the unborn-repo card; swap to a monochrome mark only if cross-platform rendering
looks off. No action required now.

### N2 — Tooltip long-content clipping
`graph-canvas.css:31-44`: `white-space: nowrap` + `max-width: 260px` lets a very long branch name
overflow rather than ellipsize. Add `overflow: hidden; text-overflow: ellipsis;` (single-line
tooltips) or allow wrap for the ref/overflow variants.

---

## Files senior-dev will touch
- `src/graph/GraphCanvas.tsx` (~910-924) — scroller a11y attrs, activedescendant, live-region wiring.
- `src/graph/draw.ts` / `src/graph/refLabels.ts` — adaptive pill text, `#b3261e` HEAD bg, per-theme
  lane palette consumption.
- `src/graph/colors.ts` — light-mode lane palette + theme selection.
- `src/components/repoWorkspace/useWorkspaceKeyboard.ts` (306-334) — start-from-null nav.
- `src/components/WorkspaceGraphPane.tsx` — permanently-mounted selection announcer.
- `src/styles/graph-canvas.css` — `.graph-scroll:focus-visible` ring; tooltip overflow (N2).

No new CSS theme tokens are required (lane palettes live in the graph color layer, not `:root`;
`#b3261e` and `#16181d`/`#ffffff` pill text are graph-internal constants). If the team prefers the
lane palettes as `--lane-N` / `--lane-N-light` CSS vars, define both `:root` and
`[data-theme='light']` blocks and note it back to ui-designer for a §5 token table.
