# spec-006 UI Contract — Author Coloring + Parent-Highlight on Hover

**Inputs:** `docs/specs/006-graph-author-color-hover/spec.md`, `.../plan.md`,
`docs/contracts/ui-reference.md` §4–§5. Canvas-paint feature + one settings row. No new DOM
surfaces beyond the settings row; no layout changes.

---

## 1. Author coloring mode

### 1.1 Scope of the recolor (hard boundary)

In `graphColorMode: 'author'`, exactly two painted things change:
- **edges** (both styles; Bonsai keeps its stepped taper widths, only `strokeStyle` changes), and
- the **pass-4 avatar lane ring** (the thin ring currently stroked in the lane color).

Everything else is unchanged: avatar disc fill stays `avatarColor` (`AVATAR = { sat: 52,
light: 42 }` in both themes — the disc carries white initials text and must keep its own
contrast), ref pills, backdrop, blossoms, seasons, semantic colors (`STASH_COLOR`, `TAG_COLOR`,
`DETACHED_HEAD_BG`), selection/HEAD/match/flash rings. `'lane'` mode is byte-identical to today.

Edge color = the **child commit's** author (`nodes[e.from].author`), per plan.

### 1.2 Author edge color — per-theme formula (canonical; also ui-reference §5.2)

Hue: `authorHue(name) = FNV-1a(name.trim()) % 360` — the exact hash `avatarColor` uses, so an
author's edges, lane ring, and avatar disc share one hue identity.

| Resolved theme | Formula | Worst-case hue band | Measured worst contrast |
|---|---|---|---|
| Dark (both graph styles) | `hsl(h, 60%, 65%)` | deep blue/violet (h≈240) | ≈4.3:1 vs `#16181d`, ≈4.4:1 vs Bonsai `#17140f` |
| Light (both graph styles) | `hsl(h, 60%, 33%)` | yellow (h≈60) | ≈3.9:1 vs `#ffffff`, ≈3.4:1 vs Bonsai `#f4efe6` |

All 360 hues clear the WCAG 1.4.11 **3:1** graphics bar on every background at these constants.
**The plan's placeholder light values (S 62 / L 40) are rejected:** yellow at that L measures
≈2.6:1 vs white — do not resurrect them. One constant pair per theme serves both graph styles;
no Bonsai-specific set is needed (the 3.4:1 vs paper is the thinnest margin in the system and is
accepted — noted for the USER CHECKPOINT).

These constants intentionally differ from `AVATAR{52,42}` (disc fill needs white-text contrast;
edges need background contrast). Same-author repos rendering single-hue is accepted per spec.

### 1.3 Settings placement & control

**Category: Appearance** (decision — purely visual, zero topology effect; house rule is
topology→Commit graph, visual→Appearance; sits with its siblings Graph style / Season).

Row: segmented control, inserted in `AppearanceCategory.tsx` **after "Season"** (keeps the
Graph style + Season dependency pair adjacent), before "File lists". Reuse the existing
segmented-row component exactly as Graph style does. No `reset` affordance (Appearance §4 rule).

- **Label:** `Graph colors`
- **Options:** `Branch lanes` | `Author` (default `Branch lanes`)
- **Help:** `Color graph edges by branch lane or by commit author. Author colors match the avatars.`

Catalog entry (`src/components/settings/catalog/appearance.ts`, after `appearance.graph-season`):

```
id: 'appearance.graph-colors', category: 'appearance', group: 'Appearance',
label: 'Graph colors', control: 'segmented',
help: 'Color graph edges by branch lane or by commit author. Author colors match the avatars.',
keywords: 'author lane colour color identity graph edges who'
```

States: standard segmented-control states (ui-reference §12) — no bespoke styling. Switching
recolors the canvas on the next paint; no transition/animation on the recolor.

Component file: `src/components/settings/SettingsGraphColorModeSection.tsx` (per plan) — or fold
into the existing Appearance rows if it is a one-liner composition; do **not** grow any file past
the 500-line limit.

---

## 2. Parent highlight

### 2.1 Target & lifecycle

`targetRow = hoverRow ?? selectedRow` (display space, inside `drawGraph`). Pointer-off (existing
mouseleave nulling) restores normal paint; with a selection present, the selected row's parents
stay emphasized (this is the keyboard path — arrow keys move the emphasis). Repaint only on
target change (existing mechanism; no new listeners).

### 2.2 Edge emphasis (pass 3.5)

Re-stroke the target's direct parent edges in the edge's **mode-resolved color** (lane or author
— never a new color) at `width + 1.5`:
- Standard: `edgeWidth (2) → 3.5`.
- Bonsai: each stepped segment width `+ 1.5` via the `emphasis` param — taper preserved.

Same clamped geometry as pass 3 (off-screen parents: visible portion only). Root commits and
fold-pill target rows: no emphasis, no ring — **confirmed, no fold-pill highlight in v1**.

### 2.3 Parent node ring (pass 4) — DEVIATION from plan

Plan said `avatarSelRingRadius + 1.5` — **rejected: that radius is already owned by the search
match ring** (`draw.ts:405`, width 1.5, occupying ~R+0.75..R+2.25). A parent that is also a
match would have one ring silently overwrite the other.

**Parent ring: radius `avatarSelRingRadius + 3.5`, width `1.5`,** stroke = the parent node's
mode-resolved color. Derived offset, so it scales with the avatar-size knob and density
(`avatarSelRingRadius` per metrics.test.ts: 13.5 default, 11.5 compact, 17.5 knob-large, 3.5
knob-min → parent ring 17 / 15 / 21 / 7). Skipped when the parent row is outside the viewport.

Ring stacking canon (inner → outer), all composable on one node:

| Ring | Radius | Width | Color |
|---|---|---|---|
| HEAD | `avatarHeadRingRadius` | 1.5 | `theme.text1` |
| Selection | `avatarSelRingRadius` | 1.5 | `theme.accent` |
| Search match | `+1.5` | 1.5 | `theme.matchRing` |
| **Parent highlight** | **`+3.5`** | **1.5** | **mode-resolved edge color** |
| Reveal flash | animated, outermost | 2 | `theme.accent`, alpha-animated |

### 2.4 No global dim (v1 decision)

Confirmed: **no dimming of non-highlighted content.** Width+ring emphasis is sufficient and
avoids whole-canvas churn on every hover. Recorded escape hatch: a `highlightDimAlpha` constant
(suggested 0.45 on passes 3–4 for non-target rows) — add only if the USER CHECKPOINT finds the
emphasis too subtle. Not implemented now.

---

## 3. States matrix

- **Lane mode** (default): pixel-identical to current rendering when nothing hovered/selected.
- **Author mode:** §1.2 colors on edges + lane rings; everything else per §1.1.
- **Hover / selection highlight:** works identically in both modes and both styles; emphasis
  color always matches the edge's current mode.
- **Themes × styles:** all four combos covered by the two constant pairs in §1.2.
- **Root / merge / off-screen parent / fold pill / unknown author:** per §2.2 and spec edge
  cases; unknown author falls through `authorHue('')` — no special casing.
- **Motion:** none. This feature introduces zero animation; `prefers-reduced-motion` is
  trivially satisfied. State transitions are instant repaints.

## 4. Accessibility

- **Color-only author identity is acceptable:** color is redundant encoding — author identity is
  already carried non-chromatically by the avatar initials on every node (`draw.ts:371` text
  pass) and by the author column/tooltip. Author mode adds a chromatic thread; it removes no
  information from lane mode users can't get elsewhere (lane structure itself is still encoded
  by geometry).
- **Keyboard parity:** selection-driven target gives full keyboard access to the parent
  highlight (arrow keys). Hover is a pointer-only progressive enhancement — stated in spec.
- Contrast: §1.2 table (≥3:1 graphics on all backgrounds). Settings row inherits the segmented
  control's existing focus-visible ring and roles.

## 5. Harness states (mock IPC)

Author mode is invisible on a single-author fixture — the mock graph must include:
1. **Multi-author history** (≥4 distinct authors, interleaved) — primary author-mode fixture.
2. **Single-author repo** — expected single-hue render, no errors.
3. **Merge commit** (2+ parents) — two parent rings + two emphasized edges on hover.
4. **Root commit** — hover yields no emphasis.
5. **Fold-pill rows** (spec-004 fixture) — hover yields nothing.
6. Long/odd author names: empty string, single char, CJK, 60-char name (hash path only — no
   layout impact expected, verify no errors).

**USER CHECKPOINT:** legibility of author hues + highlight in all 4 theme×style combos, and
hover feel / idle CPU in the native window (headless harness pauses rAF).

---

## Appendix A — ui-reference.md §5.2 addendum (READY TO PASTE)

**Orchestrator action:** insert the block below into `docs/contracts/ui-reference.md`
immediately **after §5.1** (i.e. just before the `## 6. Ref pills` heading, currently line 330).
Flagged for manual insertion because this agent run had only whole-file Write available and a
full rewrite of the 1200-line canon file was judged riskier than a paste.

```markdown
### 5.2 Author color mode (spec 006)

A persisted alternative to lane coloring (`graphColorMode: 'lane' | 'author'`, Settings →
Appearance → "Graph colors"). In author mode, edges and the avatar lane ring take the **child
commit's author hue**: `authorHue(name) = FNV-1a(name.trim()) % 360` — the identical hash behind
`avatarColor`, so edges and avatar discs share one hue identity per author. Per-theme S/L
constants (graph-layer constants in `src/graph/authorColor.ts`, not CSS vars; one pair serves
both graph styles):

| Resolved theme | Formula | Worst-case hue | Measured worst contrast |
|---|---|---|---|
| Dark | `hsl(h, 60%, 65%)` | h≈240 (blue/violet) | ≈4.3:1 vs `#16181d`, ≈4.4:1 vs `#17140f` |
| Light | `hsl(h, 60%, 33%)` | h≈60 (yellow) | ≈3.9:1 vs `#ffffff`, ≈3.4:1 vs `#f4efe6` |

All hues clear the 3:1 graphics bar on every background. Do not raise light L past 33 (yellow
falls under 3:1 vs the Bonsai paper backdrop) and do not reuse `AVATAR{52,42}` for edges (those
constants are tuned for white initials on the disc, not for background contrast). Avatar discs,
pills, backdrops, and semantic colors are unchanged in author mode. Ring stacking + parent
highlight: `spec-006-ui.md` §2.3.
```
