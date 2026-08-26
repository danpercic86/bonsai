# UI Contract — Bonsai commit-graph theme (spec 002)

**Status:** ready for implementation
**Owner:** ui-designer
**Spec:** `docs/specs/002-bonsai-graph-theme/spec.md`
**Design-system refs:** `docs/contracts/ui-reference.md` §2 (tokens), §4 (graph metrics), §5 (lane
palette), §6 (ref pills), §8 (empty states), §12.3 (settings controls)
**Implementation surfaces:** `src/graph/colors.ts`, `src/graph/draw.ts`, `src/graph/metrics.ts`,
`src/components/settings/categories/AppearanceCategory.tsx`, `src/styles.css`, `src/ipc/mock/*`

This is a **reskin only** (spec §Non-goals). No topology, ordering, lane assignment, row geometry,
ref-pill placement, IPC shape, or virtualization change. Everything below is a paint-layer swap
selected by a new `graphStyle` preference. When `graphStyle === 'standard'` the graph paints exactly
as today (acceptance #2 — no residual styling); Bonsai visuals are additive branches in the draw
layer, never edits to the standard paths.

---

## 0. Decision summary (read first)

- **Two Bonsai lane palettes** (`LANE_COLORS_BONSAI_DARK` / `_LIGHT`), 10 hues each, graph-layer
  constants mirroring the existing `LANE_COLORS_*` precedent — **not** CSS vars (§2).
- **Dark variant = bright earthy foliage on warm dark soil**; **light variant = deep bark/forest ink
  on warm rice-paper**. Chosen so (a) each hue clears **≥3:1 vs the Bonsai backdrop** and (b) the
  current-branch pill's `adaptivePillText` pick clears **≥4.5:1** (Trap 2, §1).
- **Backdrop** is near-flat (≤3% luminance spread) so the flat `theme.bg0` avatar halo is replaced by
  a new `graphBackdrop` base color, avoiding a wrong-colored halo disc (Trap 1, §4.1).
- **Edges taper by stepped `lineWidth`** across the existing three bezier segments — no new geometry,
  no filled quads (Trap 3, §3).
- **Blossom is additive** on top of the existing HEAD (`text1`) / selection (`accent`) / match rings —
  never a replacement. Rings remain the non-color meaning carriers (§2, §7).
- **Sway is a settle-on-scroll flourish, NOT a perpetual loop.** A brief, bounded animation plays for
  a short window after scrolling stops (or on selection change), then the rAF loop stops and the
  canvas returns to fully idle — mirroring the reveal-flash bounded-then-quiet lifecycle. ≤1px, node
  glyphs only, `prefers-reduced-motion` → never starts (§5). Feel is a **USER CHECKPOINT** (headless
  harness pauses rAF).
- **Seasons ship as Living (base) + Spring + Autumn**, implemented as accent+backdrop-tint overlays
  that leave the 10 lane hues fixed (preserves all contrast math). Summer/Winter are named follow-ups.
- Fixed semantic colors (`STASH_COLOR`, `TAG_COLOR`, `DETACHED_HEAD_BG`) **stay fixed** in Bonsai
  (they are semantic, not lane identity) — re-verified vs the new backdrop in §7.

---

## 1. Bonsai lane palettes

Graph-layer constants in `src/graph/colors.ts`, selected in `resolveTheme` when the Bonsai style is
active and the resolved app mode is dark/light. Deterministic `lane % 10`, stable while scrolling by
construction — identical mechanism to today.

Backdrop reference luminances (WCAG-simple sRGB, matching `relLuminance`): **dark backdrop
`#17140f` L≈0.080**, **light backdrop `#f4efe6` L≈0.939**. Ratios below are each lane hue vs its
backdrop (the 3:1 graphics bar, WCAG 1.4.11) and the winning `adaptivePillText` color vs that hue
(the 4.5:1 text bar, current-branch pill).

### 1.1 `LANE_COLORS_BONSAI_DARK` — foliage on soil

| # | Name | Hex | vs backdrop `#17140f` | Pill text | Pill ratio |
|---|------|-----|-----------------------|-----------|------------|
| 0 | sage green   | `#86c5b0` | 5.4:1 | `#16181d` | 12.6:1 |
| 1 | warm sand    | `#e3c07a` | 5.7:1 | `#16181d` | 13.4:1 |
| 2 | lilac bloom  | `#c3a6e0` | 5.2:1 | `#16181d` | 12.2:1 |
| 3 | moss leaf    | `#9cc873` | 5.4:1 | `#16181d` | 12.7:1 |
| 4 | clay         | `#e39b83` | 5.0:1 | `#16181d` | 11.7:1 |
| 5 | jade teal    | `#7fccc4` | 5.5:1 | `#16181d` | 12.9:1 |
| 6 | gold ochre   | `#ddc85f` | 5.7:1 | `#16181d` | 13.6:1 |
| 7 | rose bloom   | `#e6a6bf` | 5.3:1 | `#16181d` | 12.5:1 |
| 8 | wisteria     | `#a3aee6` | 5.2:1 | `#16181d` | 12.2:1 |
| 9 | young lime   | `#bcd17a` | 5.8:1 | `#16181d` | 13.7:1 |

All L≈0.66–0.78 → `isDarkBg` returns **false** → `adaptivePillText` picks near-black `#16181d`,
which clears 4.5:1 with room. Adjacent lanes alternate warm/cool for separability at the 3:1 bar.

### 1.2 `LANE_COLORS_BONSAI_LIGHT` — bark ink on paper

| # | Name | Hex | vs backdrop `#f4efe6` | Pill text | Pill ratio |
|---|------|-----|-----------------------|-----------|------------|
| 0 | deep pine    | `#123330` | 5.4:1 | `#ffffff` | 4.9:1 |
| 1 | bark umber   | `#45280e` | 5.3:1 | `#ffffff` | 4.8:1 |
| 2 | plum bloom   | `#372440` | 5.4:1 | `#ffffff` | 4.9:1 |
| 3 | forest green | `#163419` | 5.4:1 | `#ffffff` | 4.9:1 |
| 4 | deep clay    | `#501e16` | 5.5:1 | `#ffffff` | 5.0:1 |
| 5 | deep teal    | `#0b3038` | 5.5:1 | `#ffffff` | 5.0:1 |
| 6 | dark ochre   | `#332907` | 5.5:1 | `#ffffff` | 5.0:1 |
| 7 | deep rose    | `#4e1e30` | 5.4:1 | `#ffffff` | 4.9:1 |
| 8 | deep indigo  | `#24284e` | 5.4:1 | `#ffffff` | 4.9:1 |
| 9 | dark olive   | `#2c3009` | 5.3:1 | `#ffffff` | 4.8:1 |

All L≈0.16–0.17 → `isDarkBg` returns **true** → `adaptivePillText` picks white `#ffffff`, which
clears 4.5:1. Ratios computed at the pill-text bar's worst case (L≈0.17). Do **not** brighten these
past L≈0.18 or white pill text drops below 4.5:1.

`laneColorsAlpha` (18% pill backgrounds) is derived by `hexToRgba(c, 0.18)` exactly as today; the
local-branch pill (lane color at 18% on the backdrop, text+border = lane color) inherits the same
math and needs no separate table.

### 1.3 Seasonal variants (Living / Spring / Autumn)

Seasons **do not alter the 10 lane hues** — that keeps every contrast/pill-text figure in §1.1–1.2
valid across seasons and avoids quadrupling the verification surface. A season shifts only three
cheap, isolated things:

| Season  | Blossom accent (dark) | Blossom accent (light) | Backdrop tint | Lane saturation bias |
|---------|-----------------------|------------------------|---------------|----------------------|
| Living (base) | `#e6a6bf` (rose) | `#7a2f4d` | none — §4.1 base | none |
| Spring  | `#f2b8d6` (cherry) | `#8f3560` | +2% cooler/lighter (`#181611` dark / `#f6f1ea` light) | none |
| Autumn  | `#e59a5b` (amber)  | `#8a4713` | +2% warmer (`#1a1510` dark / `#f4ece0` light) | none |

The blossom accent is used only by the HEAD/selected blossom (§2.3) — never by lanes, pills, or
rings — so its contrast bar is the decorative-graphics bar against the backdrop, met by all six
values (≥3:1). Backdrop tints stay inside the ≤3% flatness budget (§4.1), so the halo fix holds.
**Follow-up (out of scope here):** Summer (deep-green, warm) and Winter (cool, desaturated) as two
further entries, added the same way.

---

## 2. Node visual spec

Draw order and rings are **unchanged from `draw.ts` Pass 4**; Bonsai adds paint, never removes it.

### 2.1 Ordinary commit node (leaf/bud)

- Keep the **author-initials avatar disc** (`avatarColor`, theme-invariant) — author identity is
  load-bearing and must not be sacrificed to the theme.
- The **bg halo** (currently `theme.bg0`) uses `theme.graphBackdrop` in Bonsai (§4.1) so it matches
  the paper/soil instead of punching a `--bg-0` hole in it. Radius unchanged (`avatarRadius +
  avatarBgRingExtra`).
- The **lane ring** keeps `avatarRingWidth` but is drawn as a **sepal ring**: same circle, same lane
  color, no geometry change. (We deliberately do **not** morph the disc into a leaf polygon — a
  per-node path costs against the 20k-row bar and muddies the author avatar. The organic read comes
  from edges + backdrop + blossom, not node silhouette.)

### 2.2 Rings (HEAD / selection / match) — unchanged carriers

- HEAD ring: `theme.text1`, radius `avatarHeadRingRadius`, 1.5px — **unchanged**.
- Selection ring: `theme.accent`, radius `avatarSelRingRadius`, 1.5px — **unchanged**.
- Match ring: `theme.matchRing`, `avatarSelRingRadius + 1.5`, 1.5px — **unchanged**.
- Reveal flash halo/pulse: unchanged (`theme.accent`, animated alpha).

These stay because color is never the sole carrier of meaning (§ui-reference §2/§7). The blossom is
**decoration layered behind the disc**, additive to the ring that carries the state.

### 2.3 Blossom / accent treatment (HEAD and selected)

Painted in **Pass 4, before the avatar disc**, so it sits behind the node and its rings and never
occludes initials or state rings. Applied to at most the HEAD node and the selected node — never more
than 2 visible nodes, so cost is negligible.

- **Shape:** 5 petals — 5 filled circles of radius `avatarRadius * 0.62`, centers on a ring of radius
  `avatarRadius * 0.95` at 72° spacing, phase `-90°` (top petal up). Fill = season blossom accent
  (§1.3) at **alpha 0.55** (dark) / **0.65** (light). No stroke.
- **HEAD:** full 5-petal blossom.
- **Selected (not HEAD):** a **bud** — a single petal at the top position only, same radius/alpha, so
  selected reads as "about to open" and HEAD reads as "in bloom." Distinct silhouettes, not just
  color.
- **HEAD + selected simultaneously:** full blossom (HEAD wins the silhouette); the accent selection
  ring already differentiates the two states.
- Petal color must not be mistaken for a ring — it is under the disc and softer (alpha), the crisp
  ring on top is the authoritative state marker.

---

## 3. Edge visual spec (tapered branches)

Reskins `drawEdge` (`draw.ts` ~104–143) **without moving any endpoint or control point** — the
three-segment bezier from ui-reference §4 is preserved verbatim. Only `strokeStyle` and per-segment
`lineWidth` change, and only when Bonsai is active.

- **Color:** `theme.laneColors[e.lane % 10]` — identical selection to today, now the Bonsai palette.
- **Taper rule — "older is thicker" (toward the trunk/root, which is downward = higher row index):**
  the graph draws children above parents, so the **bottom** of an edge runs toward the older
  parent/trunk and is thickest; the **top** runs toward the newer tip and is thinnest. Stepped
  `lineWidth` across the three existing segments:

  | Segment (in `drawEdge`) | Role | Width cozy | Width compact |
  |---|---|---|---|
  | top curve `from → e.lane` | tip | 1.5px | 1.0px |
  | middle straight run | branch | 2.0px (= today's `edgeWidth`) | 1.5px |
  | bottom curve `e.lane → to` | trunk | 2.5px | 2.0px |

  For a single-row adjacent edge (`e.to === e.from + 1`, one `segmentTo` call) use the **branch**
  width (2.0 / 1.5) — no visible taper over one row.

- Implementation note: each segment currently accumulates into one `beginPath()`/`stroke()`. To vary
  width per segment, split into up to three `stroke()` calls (one per segment) **only in the Bonsai
  branch**; the standard branch keeps its single-path stroke. `lineCap = 'round'` is retained so
  stepped widths butt cleanly. This is 3 strokes for a small subset of visible multi-row edges — well
  inside budget; do not attempt per-pixel width or filled quads.
- New metrics constants in `metrics.ts`: `edgeTipWidth` / `edgeTrunkWidth` for cozy and compact
  (`edgeWidth` stays the branch width). Standard theme ignores them.

---

## 4. Backdrop spec (paper / pot)

### 4.1 Canvas backdrop

Painted as the first fill of the canvas render (before edges/nodes), replacing the current reliance
on the DOM `--bg-0` behind the canvas. **Near-flat** so the avatar halo (§2.1) and any semantic fill
reads correctly on it.

- **Base color** (also the value of the new `theme.graphBackdrop` field consumed by the halo):
  - Dark: `#17140f` (warm soil-black, L≈0.080).
  - Light: `#f4efe6` (warm rice-paper, L≈0.939).
- **Gradient:** optional single vertical linear gradient, **≤3% luminance spread** (the flatness
  budget the halo fix depends on):
  - Dark: top `#191510` → bottom `#141109` (light pooling near the canopy/top).
  - Light: top `#f7f2ea` → bottom `#f0eadf`.
  - If a gradient adds cost or banding at 20k rows, ship the flat base color — the gradient is
    polish, the flat base is the contract. Repaint only the visible viewport rect (no full-history
    surface).
- Season tints from §1.3 replace the base/gradient endpoints by the stated ±2%.

### 4.2 DOM container behind the canvas

Add token `--graph-canvas-bg` (§ui-reference §2 update) so the graph pane's container div, the load
skeleton, and the empty state sit on the same paper/soil as the canvas — no seam during load or when
the canvas is shorter than the pane. Default aliases `--bg-0`; Bonsai overrides via a
`data-graph-style="bonsai"` attribute on the app root, combined with `[data-theme='light']`.

### 4.3 Empty / unborn-HEAD ("empty pot") state

Reuse the existing `EmptyState` component (§8) and layout — no new component. On the Bonsai backdrop:

- Icon: the app's bonsai/pot glyph (existing icon system §13) at the standard empty-state size,
  `--text-3`.
- Title: `No commits yet` (unchanged copy from the standard empty graph state).
- Body: `This branch is an empty pot — make your first commit to grow it.` (Bonsai-flavored; the
  standard theme keeps its existing line.)
- Loading: existing skeleton, on `--graph-canvas-bg`. Error: existing error state, unchanged copy,
  on `--graph-canvas-bg`.

---

## 5. Sway flourish spec

Sway is a **settle-on-scroll flourish, not a continuous/idle animation.** It is triggered by an
interaction and self-terminates — the canvas spends the vast majority of its time fully idle with no
rAF loop running.

- **Lifecycle (the key rule):** a **brief, bounded settle** plays for a short window **after
  scrolling stops** (scroll/wheel/drag end) or **on selection change**, then the rAF loop **stops**
  and the canvas returns to fully idle. This mirrors the reveal-flash bounded-animation-then-quiet
  lifecycle (`revealFlashRunner.ts`): a run is armed by an event, animates to completion, and the
  loop tears itself down — there is **no perpetual/idle animation**.
  - Recommended window: a single damped settle of ~**600–900ms** total, ease-out to rest (an organic
    "leaves settling after a gust"). Do not re-arm while it is still running; a new scroll-stop or
    selection change restarts it from the current offset.
  - **Not** driven while scrolling is in progress — the settle plays *after* motion stops, so it
    never contends with scroll frames. During active scroll, nodes paint at offset 0.
- **What moves:** node glyphs only (avatar disc + its blossom/bud + rings translate together as a
  unit), by a per-lane phase-offset horizontal displacement that decays to 0 over the settle window.
  **Edges do NOT move** — re-tessellating beziers per frame is the jank path and is explicitly
  forbidden.
- **Amplitude:** **≤1px** peak horizontal offset (`0.8px` recommended), decaying to 0. Below the
  lane-ring width, so lanes never appear to cross.
- **Phase:** seeded by `lane` (e.g. `lane * 0.7 rad`) so adjacent lanes settle out of sync for an
  organic, non-mechanical feel. Vertical position never changes (row hit-testing must stay exact).
- **The offset is applied at paint time only:** it never mutates layout, scrollTop, or hit-test
  coordinates. When the settle completes the runner clears the offset and stops requesting frames.
- **Suspend when `document.hidden`** (tab/window not visible) — a settle armed while hidden does not
  run; it is dropped, not queued.
- **Reduced-motion fallback:** `prefers-reduced-motion: reduce` → **zero motion**, the settle
  **never arms** and the loop never starts; nodes paint at their static positions (offset = 0). This
  mirrors the reveal-flash static two-paint fallback.
- **Verification:** sway smoothness/subtlety is a **USER CHECKPOINT** — the headless browser harness
  pauses `requestAnimationFrame`, so the orchestrator cannot judge frame feel. AI-gate can only
  confirm the static (offset=0) render and that reduced-motion disables the settle.

---

## 6. Settings UI spec (Appearance)

Two new rows in `AppearanceCategory.tsx`, placed **directly after the existing Theme row** (dark/
light) and before File lists — grouping all visual-language controls together.

```
Appearance
  Theme            [ Dark | Light ]            ← existing, unchanged
  Graph style      [ Standard | Bonsai ]       ← NEW (segmented)
  ┌ (fieldset, enabled only when Bonsai) ─────────────────┐
  │ Season          [ Living        ▾ ]        ← NEW (combobox)
  └───────────────────────────────────────────────────────┘
  File lists       [ Tree | Flat ]             ← existing
  Panel density    [ Cozy | Compact ]          ← existing
```

- **Graph style** — `SettingsSegmented` (§12.3.2): 2 exclusive short labels → segmented is correct.
  - Label: `Graph style`. Options: `Standard`, `Bonsai`. Row help:
    `Bonsai restyles the commit graph as a living tree. Topology and behavior are unchanged.`
  - Key `appearance.graph-style`, values `'standard' | 'bonsai'`, default `'standard'`. Coexists with
    Theme: Graph style is orthogonal to app light/dark — Bonsai has its own light/dark palettes, so
    both controls stay independent (mirrors the density note in §12).
- **Season** — `Combobox` (§12.3, ">3 exclusive values" future-proofing; ships with 3 today but
  Summer/Winter are planned, so a segmented control would need replacing).
  - Label: `Season`. Options: `Living`, `Spring`, `Autumn`. Row help:
    `Seasonal accents for blossoms and backdrop. Lane colors are unchanged.`
  - Key `appearance.graph-season`, default `'living'`.
  - **Dependency:** wrapped in the §12.3.3 `<fieldset disabled>` pattern; disabled (and dimmed via
    `.settings-row.is-disabled`, never the fieldset) whenever Graph style is `Standard`. Lead
    sentence on the group: `Seasonal palettes apply to the Bonsai graph style.` The fieldset carries
    `aria-describedby` to that lead only while disabled.
- **Preview:** no inline preview surface — the live commit graph is the preview (changes apply
  immediately, like Theme). Do not add a mini-canvas; it duplicates the real thing and costs a second
  render path.
- **Persistence:** both keys persist like other appearance prefs (acceptance #3). **Flag for
  architect/orchestrator:** the settings store + `Settings` IPC type need two new string fields
  (`graphStyle`, `graphSeason`). This is the architect's data shape, not mine — I assume simple
  enum-valued prefs; confirm before implementation.

---

## 7. Graph states — coverage in both Bonsai variants

| State | Bonsai treatment |
|---|---|
| Normal commit | avatar disc + lane sepal ring, backdrop halo (§2.1) |
| Selected | accent selection ring (unchanged) + top-petal **bud** behind disc (§2.3) |
| HEAD (attached) | `text1` HEAD ring (unchanged) + full 5-petal **blossom** (§2.3) |
| Detached HEAD | same node treatment as HEAD; ref pill = fixed `#b3261e` bg, white text (unchanged) |
| Stash ref | `STASH_COLOR` violet disc+glyph+ring (unchanged) — verified: `#9a7cff` vs `#17140f` = 5.0:1, vs `#f4efe6` = 3.4:1, both ≥3:1 |
| Local-branch pill | lane color @18% bg, lane-color text+border (unchanged math, Bonsai palette) |
| Current-branch pill | solid lane-color bg, `adaptivePillText` (§1: near-black dark / white light, all ≥4.5:1) |
| Remote pill | `--bg-2` bg, `--text-2` text, `--border` — unchanged; sits on backdrop via its own fill |
| Tag pill | `TAG_COLOR` `#d4a72c` @18% (unchanged) — verified: `#d4a72c` vs `#17140f` = 7.2:1, vs `#f4efe6` = 2.9:1 border-only (label is the carrier; matches standard-theme light behavior) |
| Search match | `theme.matchRing` outer ring (unchanged) |
| Empty / unborn repo | "empty pot" EmptyState on backdrop (§4.3) |
| Very wide history | `lane % 10` reuse — identical degradation to standard; Bonsai hues stay ≥3:1 pairwise-adjacent, no new failure mode (spec edge case) |
| WIP row | dashed `--warning` marker + label (unchanged); on backdrop |

Ref pills, summaries, and metadata columns keep their exact placement, fonts, and truncation from
§4/§6 — only the surface behind them (backdrop) changes, and they were already legible on `--bg-0`,
which the near-flat backdrop tracks within its ≤3% budget.

## 8. Accessibility

- **Lane graphics bar (WCAG 1.4.11, ≥3:1):** every Bonsai lane hue vs its backdrop ≥5.0:1 (§1) —
  clears with margin in both modes.
- **Pill/summary text bar (≥4.5:1):** current-branch pill via `adaptivePillText` ≥4.5:1 both modes
  (§1); summary text uses `--text-1` on `--graph-canvas-bg` — verify `#e8eaed` on `#17140f` (12.7:1)
  and `#1c1f24` on `#f4efe6` (14.9:1), both pass. Metadata `--text-2`: `#a8adb8` on `#17140f` = 7.4:1,
  `#4b515c` on `#f4efe6` = 6.9:1.
- **Color never sole carrier:** HEAD/selected differ by silhouette (blossom vs bud) **and** ring;
  detached-HEAD/PR/status carriers unchanged (§6, §7 house rule).
- **Reduced motion:** the sway settle never arms under `prefers-reduced-motion` (§5); no other Bonsai
  motion is introduced.
- **Focus / keyboard / SR:** graph focus, `role="grid"`, activedescendant, and the live-region
  announcement (§4.1) are theme-invariant — Bonsai changes zero interaction, so all §4.1 a11y holds.
- **Both densities:** node radii/ring radii come from `metrics.ts` (cozy/compact) unchanged; new edge
  taper widths specced per density (§3); blossom radii derive from `avatarRadius`, so they scale with
  the density knob automatically.

## 9. Harness fixture states (`src/ipc/mock/`, `VITE_MOCK_IPC=1`)

The Bonsai style is a pure client-side paint switch, so **all** fixtures below already exist for the
standard theme; the only harness need is a way to toggle `graphStyle`/`graphSeason` (via the new
Settings rows, which the mock settings store must serve). Verify each existing fixture under Bonsai
dark and light:

- **Populated multi-lane graph** (existing) — palette, taper, blossom on HEAD, backdrop.
- **Selected commit** — bud silhouette + accent ring.
- **Empty / unborn-HEAD** — "empty pot" state (§4.3).
- **Loading & error** — skeleton/error on `--graph-canvas-bg`.
- **Pathological wide history + long branch names** — palette reuse degradation, pill truncation on
  backdrop.

**AI-gate reachable:** palettes, taper, blossom/bud, backdrop, pills, empty state, contrast (static
screenshot + computed-style read). **USER CHECKPOINT only:** sway settle motion feel and 20k-row
scroll frame timing (harness pauses rAF; frame-timing is never an AI gate).
