# Bonsai — UI Reference Spec (all milestones)

Feel: GitButler-clean minimalism; GitKraken-style commit graph as the centerpiece. Dark theme is
the default. All values below are canonical — implement as CSS custom properties in
`src/styles.css` and reuse everywhere.

## 1. Layout geometry

```
+--------------------------------------------------------------+
| Header bar (40px): "Bonsai" · repo name + path · ⟳ refresh    |
+--------------------------------------------------------------+
| App notice bar (P70) — absent unless a global fault — §10     |
+--------------------------------------------------------------+
| Workspace toolbar (40px): remote ops · refresh               |
+-----------+---------------------------------+----------------+
| Sidebar   | Commit graph (canvas)           | Right panel    |
| 240px     | flex 1, min 480px               | 380px          |
| branches  |                                 | status / diff /|
| remotes   |                                 | commit details |
| tags      |                                 |                |
+-----------+---------------------------------+----------------+
| AI activity dock (30px collapsed / 120-600px open) — §9      |
+--------------------------------------------------------------+
```

- Header: height 40px, `--bg-1` background, 1px bottom border `--border`. Left: app name (600
  weight). Center-left: repo folder name (text-1) + full path (text-3, 12px, truncated). Right:
  refresh icon button (32×32 hit area).
- App notice bar (P70): second child of `.app`, between the header and the tab workspace hosts.
  App-global, in-flow, `flex: none`; absent from the DOM's visible output unless a process-wide
  fault is present. See §10.
- Left sidebar: fixed 240px, `--bg-1`, 1px right border. Collapsible sections "Branches",
  "Remotes", "Tags" (section headers: 11px uppercase, letter-spacing 0.08em, text-3).
- Center: `--bg-0`, hosts the `<canvas>` graph; fills remaining width, min 480px.
- Right panel: fixed 380px, `--bg-1`, 1px left border. Content: commit details when a graph node
  is selected; working-dir status + staging otherwise.
- Pane resizing: sidebar and right panel are drag-resized and persisted (`PaneDivider`).
- Bottom dock (P68e): full-width third child of `.workspace-host`, `flex: none`, absent from the
  DOM until an AI run exists. Never overlaps the panes — it takes height from them. See §9.
- Header toolbar (P69): `.header-toolbar` lives in `HeaderToolbar.tsx`, not in `App.tsx`. Order,
  left to right: theme · list view · AI assets · health · settings · **identity** (§12.6). The
  identity control is the far-right "account slot"; repo-scoped controls in the toolbar render only
  when a repo is open.

## 2. Theme tokens

CSS custom properties on `:root` (dark, default) and `[data-theme="light"]`.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg-0` | `#16181d` | `#ffffff` | app/canvas background, content surfaces |
| `--bg-1` | `#1d2026` | `#f6f7f9` | panels, header, chrome |
| `--bg-2` | `#262a31` | `#eceef2` | hover, inputs, pill bg base |
| `--bg-3` | `#2f343d` | `#e2e5ea` | active/pressed |
| `--text-1` | `#e8eaed` | `#1c1f24` | primary text |
| `--text-2` | `#a8adb8` | `#4b515c` | secondary text |
| `--text-3` | `#6b7280` | `#8a919e` | muted/labels — **see the contrast note below** |
| `--border` | `#2c313a` | `#dcdfe5` | 1px pane/row borders |
| `--accent` | `#4f8cff` | `#2f6fe4` | primary buttons, links, focus ring |
| `--accent-text` | `#ffffff` | `#ffffff` | text on accent |
| `--selection` | `#2a3b57` | `#dbe7ff` | selected row background |
| `--danger` | `#e5534b` | `#d13438` | errors, destructive |
| `--success` | `#57ab5a` | `#1a7f37` | staged/added |
| `--warning` | `#d4a72c` | `#9a6700` | modified/dirty |

Focus: 2px `--accent` outline, offset 1px, keyboard only (`:focus-visible`).

**Contrast notes (measured 2026-08-17, P68e design pass; toast rows updated 2026-08-20, P74).**
One known AA shortfall remains (`--text-3`); the hue-as-text family was retro-fitted by P74.
**New surfaces must not add to either**:

- `--text-3` is **3.38:1** on `--bg-1` and **3.67:1** on `--bg-0` (dark), **2.96:1** on `--bg-1` and
  **≈3.2:1** on `--bg-0` (light). That is below the 4.5:1 AA bar for text. Treat `--text-3` as
  **decorative only** (uppercase section labels that duplicate visible structure, dividers, disabled
  glyphs). Any text the user must actually read — metadata, timestamps, costs, log lines, hints,
  **status-pill labels**, **settings help text**, **any heading that is the user's only wayfinder**
  (§12.5's result-group headers) — uses `--text-2` (**7.9:1** dark / **4.9:1** light on `--bg-0`;
  **7.3:1** / **7.4:1** on `--bg-1`).
- **A hue is never a label colour over its own tint.** Use the hue for borders, glyphs, bars and
  fills (≥3:1 graphics bar) and `--text-1` for the words beside them. For a filled warning chip,
  `color: var(--bg-0)` on `background: var(--warning)` is safe in both themes (**6.4:1** dark /
  **4.8:1** light). Measured failures of the forbidden recipe, kept as the evidence trail — hue on
  its own 14% tint over `--bg-2`: `--danger` **3.35:1** dark / **3.48:1** light; `--accent`
  **3.68** / **3.38**; `--success` **4.07** / **3.66**; `--warning` **4.96** / **3.53**. All four
  were the four toast tones; **P74 fixed them** (§10.2 — label `--text-1` at **9.24–10.30:1** dark /
  **11.68–12.00:1** light, hue demoted to a 3px leading bar + glyph at **3.35–4.96** / **3.38–3.69**,
  clearing the 3:1 graphics bar). The same recipe on a 12% tint over `--bg-1`
  (`.submodule-badge-ok` **4.76** / **4.06**, `.submodule-badge-warn` ≈**5.4** / **3.94**) was
  already fixed in P73 by §11's pill recipe. **There is no remaining sanctioned use of
  hue-as-text-over-its-own-tint anywhere in the app; a new one is a defect.**
- **`--accent` as *text* never sits on a `--selection` fill (added 2026-08-20, P69l).**
  `color: var(--accent)` is a house-wide pattern (~30 call sites) and is fine on `--bg-0` / `--bg-1`
  / `--bg-2`, but over `--selection` it measures **2.6:1** dark / **3.6:1** light (independently
  re-measured at **3.51** / **3.74** in the P69k pass — both readings fail the 4.5:1 text bar in both
  themes). So: accent-coloured *text* inside a row that can become selected is a latent defect, and
  `--accent` may never be chosen as the "emphasised" colour for a value inside a selected row — use
  `--text-1`. `--accent` as a **border, bar or glyph** on `--selection` remains fine as decorative
  delineation that carries no meaning (the settings rail's inset bar, §12.1).

Additional measured pairs (2026-08-19, P70 pass), all on `--bg-1`: `--text-1` **13.5:1** dark /
**15.4:1** light; `--warning` glyph **7.3:1** / **4.5:1**; `--success` glyph **5.7:1** / **4.7:1**;
`--danger` glyph **4.4:1** / **4.6:1** — all clear the 3:1 graphics bar in both themes. And
(2026-08-19, P73 pass) `--text-2` over its **own** 12% tint on `--bg-1`: **5.79:1** dark /
**6.22:1** light — the safe recipe for a hueless informational pill (§11).

Measured pairs added 2026-08-19 (P69 Settings pass), on `--bg-0`: `--accent` fill **5.6:1** dark /
**4.7:1** light; `--text-2` fill **7.9:1** / **4.9:1**; `--text-1` on `--selection` **9.4:1** /
**13.3:1**; `--accent` 1px border on `--bg-2` **4.4:1** / **4.1:1**. `--accent` on `--selection` is
**2.6:1** / **3.6:1** — decorative delineation only, never a meaning carrier. And (P69c pass)
`--warning` as a 1px ring on `--bg-2`, the input fill: **6.4:1** dark / **4.2:1** light — clears the
3:1 graphics bar (§12.3.4).

Measured pairs added 2026-08-20 (P74 pass), `--text-1` over a hue's own 14% tint on `--bg-2` — the
canonical "hue surface, readable words" pair: **9.24–10.30:1** dark, **11.68–12.00:1** light,
across all four of `--danger` / `--success` / `--warning` / `--accent`. Use these numbers for any
new tinted surface that must carry prose.

**Dimming budget (added 2026-08-20, P69j pass).** `opacity: .55` on `--text-1` over `--bg-0` lands
at ≈**4.2:1** dark / **3.6:1** light — acceptable *only* on genuinely inert controls (WCAG 1.4.3
exempts inactive components). It is a budget, not a free knob: **it may be spent once per subtree**.
Two nested `.55` layers compound to **.30**, which is ≈2.5:1 dark / ≈1.8:1 light and unreadable in
both themes. See §12.3.3. **And it may only be spent on something the user cannot act on:** dimming
a control that is still clickable buys the contrast loss with none of the exemption — the P69k rail
counts started as `opacity: .5` on clickable zero-count tabs (**2.87:1** dark / **2.36:1** light) and
had to be replaced by inverted emphasis (§12.5).

## 3. Typography & spacing

- UI font: `"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif`.
- Mono (hashes, paths, diffs): `"Cascadia Code", "Cascadia Mono", Consolas, "JetBrains Mono", monospace`.
- Sizes: base 13px / line-height 1.45; secondary 12px; section labels 11px; header app name 14px;
  diff/mono 12px. Weights: 400 normal, 600 emphasis; never bolder.
- Spacing scale (margins/padding/gaps): 4 / 8 / 12 / 16 / 24 px only.
- Border radius: 6px (buttons, panels, inputs), 999px (pills).
- Density: the `panelDensity` setting (`cozy` | `compact`) is applied as `data-density` on a
  container which redefines a `--<scope>-*` custom-property block; every consumer reads
  `var(--x, <pre-density fallback>)`. Precedents: `--rp-*` on `.right-panel` (P67b),
  `--ai-dock-*` on `.ai-dock` (P68e). **Scope:** the right panel and the dock only — the sidebar,
  the Settings overlay, dialogs, and app chrome (header, workspace toolbar, the §10 notice bar) have
  one geometry in both densities.

### 3.1 Hit-target floor (WCAG 2.2 · 2.5.8)

- Every interactive control is **≥24 × 24 CSS px**, in every density and both themes. There is no
  compact-mode escape hatch outside the two density scopes named above.
- **The box grows, the glyph does not.** Enlarge the transparent/hover box around an icon and leave
  the painted glyph at its designed size. Canonical sizes: `.btn-icon` **32×32** around a 14–16px
  glyph (header toolbar); `.sidebar-add` **24×24** around a 14px `+` and a 14×14 SVG;
  `.settings-switch` **36×24** where the invisible `<input>` *is* the target. Never inflate the
  glyph to reach the floor — that changes visual weight and density.
- **A text button reaches the floor with padding plus a negative margin, not with height.** When a
  small text link must stay optically flush with a container edge (`.settings-results-goto`, §12.5:
  `min-height: 24px; padding: 0 6px; margin-right: -6px; display: inline-flex; align-items: center`),
  the padding grows the hit box and the equal negative margin gives the alignment back. This is the
  one sanctioned negative margin — it pays for a hit target, it does not claw back gutter (contrast
  the P74 SF-1 rule below).
- **Prefer `align-self: stretch` over a hardcoded height** when the control sits in a row that
  already owns a height (`.sidebar-section-toggle` in a 24px `.sidebar-section-header`,
  `.tree-dir-toggle` in a `.tree-dir-row`). The control then tracks the row and stays correct when
  a density block changes the row height.
- **A stretched toggle's hover wash must share its list rows' left edge.** When a full-width control
  gains a hover background, that rectangle becomes a visible alignment edge: keep it flush with the
  sibling rows below it (`.branch-row` at the pane's 12px gutter) and never let it bleed into the pane
  gutter. Pad the control for breathing room; do not claw the padding back with a negative margin
  (P74 SF-1).
- **Sidebar geometry (P74).** One 24px module for everything: `.sidebar-section-header` 24,
  `.sidebar-section-toggle` 24 (stretched), `.sidebar-add` 24×24, `.list-filter-clear` 24×24,
  `.list-filter-input` 24, `.branch-row` 24, `.sidebar .tree-dir-toggle` 24, `.error-dismiss`
  24×24, `.toast-dismiss` 24×24. `.sidebar-section` keeps `margin-bottom: 16px` and `.branch-list`
  `margin-top: 4px`. Before P74 the toggles were 16px, the action buttons 20×20, and the Tags
  header 16px against the others' 20px — the fix also removed that rhythm inconsistency.
- **A newly enlarged target must announce itself.** `.sidebar-section-toggle:hover` paints
  `background: var(--bg-2)` with `border-radius: 4px` — the same wash `.branch-row:hover` and
  `.sidebar-add:hover` already use, so no new idiom. *Decided by the orchestrator (P74 OPEN-1,
  2026-08-20) and recorded here because it is a visible change the user did not ask for:* a
  full-width 24px control with no hover feedback is a worse affordance than the 16px one it
  replaced, since nothing tells the user the whole header row is clickable. Enlarging a hit target
  without a matching hover state is half a fix.
- **Known exemption:** `.right-panel[data-density='compact']` sets `--rp-row-h: 20px`, so
  `.tree-dir-row`/`.tree-dir-toggle` are 20px there. That is a deliberate user opt-in inside a
  density scope; raising it would delete the point of `compact`. Do not "fix" it silently, and do
  not copy it to any surface outside that scope.
- Row hover-action buttons (`.row-action`, §7.1) are 20×20 and are the one other standing
  exception: they live inside a 24px row, are revealed on hover, and sit in a 2px-gap cluster where
  24px boxes would collide. Treat the **row** as the target for pointer purposes.

## 4. Commit graph metrics (canvas)

Canonical numbers live in `src/graph/metrics.ts` (`METRICS` = cozy baseline, `COMPACT` = compact
preset); the three user knobs `avatarRadius` / `rowHeight` / `laneWidth` vary at runtime. This
section mirrors them — update both together.

- **Row height:** **32px** (cozy) / **22px** (compact). Lane width (x-spacing between lane centers):
  **16px**. Left graph gutter: 12px before lane 0. A fixed **180px** left ref-column band
  (`refColWidth`) precedes the graph and carries the ref pills (§6).
- **Commit node = author avatar, not a bare dot.** A filled disc of radius **10px** (cozy) / **8px**
  (compact) at the commit's lane x, hue-hashed from the author (HSL sat 52 / light 42 —
  theme-invariant, legible on both backgrounds) with the author's 1–2 initials in 600 11px. Behind
  it a **2px** (`avatarBgRingExtra`; 1px compact) `--bg-0` halo so edges passing under read cleanly;
  over it a **1.5px** lane-color ring tying the node to its lane.
- **Selected commit:** an extra outer ring at radius `avatarRadius + 3.5` (→ 13.5px cozy) in
  **`--accent`**. **HEAD commit:** an outer ring at radius `avatarRadius + 2.5` (→ 12.5px cozy) in
  `--text-1`. Both radii derive from `avatarRadius`, so they stay outside the disc at any node-size
  knob value.
- **Search-match:** an outer ring in `--match-ring` at a radius distinct from the selection/HEAD
  rings, so a match stays spottable while scrolling.
- **Edge stroke:** **2px**, round caps, color = the edge lane's color from the **per-theme** palette
  (§5).
- **Fork/merge curve:** cubic bézier between (x1, y1) and (x2, y2) of adjacent rows with control
  points `(x1, y1 + rowHeight/2)` and `(x2, y2 − rowHeight/2)` — vertical tangents at both ends,
  GitKraken-style S-curve. Straight vertical segments elsewhere.
- **Right of the graph:** ref pills (§6), then the commit summary (`--text-1`, `summaryFont` 13px
  cozy / 12px compact, truncated), then the optional author / relative-date / short-SHA columns
  (`--text-2`, `metaFont` 12px cozy / 11px compact) as space and the per-row display toggles allow.
- **HiDPI:** canvas backing store scaled by `devicePixelRatio`; all metrics above are CSS px.

### 4.1 Keyboard & screen-reader access (added 2026-08-22, graph review)

The `<canvas>` is opaque to assistive tech, so the graph MUST be a focusable composite widget:

- The scroller (`.graph-scroll`) is the single tab stop: `tabIndex={0}`, `role="grid"`,
  `aria-label="Commit graph"`, `aria-rowcount={total}`, and
  `aria-activedescendant="graph-row-{selectedIndex}"` while a row is selected. It shows a
  `:focus-visible` ring (2px `--accent`, 1px offset, inset so the canvas does not clip it), distinct
  from the per-row `--accent` selection ring above.
- **Keyboard nav must be able to START with no prior selection.** When the graph has focus and
  nothing is selected, the first ArrowDown/ArrowUp/Home/End/PageUp/PageDown selects an anchor —
  `headIndex` if it is in loaded history, else `0` (Down/Home) or the last row (Up/End). Thereafter
  Arrow = ±1 row, PageUp/Down = ±visible-row-count (`getVisibleRowCount`), Home/End = first/last.
  Selection scroll-into-view already exists (`viewport.ts:scrollRowIntoView`).
- A permanently-mounted polite live region (the RevealAnnouncer / §9–§10 split) announces the
  settled selection: `"{summary} — {author}, {relative date}. Row {n+1} of {N}. {ref summary}"`,
  debounced ~150 ms so a held arrow key does not flood the reader.

## 5. Lane color palette (deterministic, per theme)

Assigned by `lane % 10`, computed in Rust with the layout; stable while scrolling by construction.
**Revised 2026-08-22 (graph review):** the palette is now **theme-specific**. The previous
single-palette rule left six of ten lanes below the 3:1 graphics bar against the light `#ffffff`
background (measured: orange 2.23, green 2.48, teal 2.09, yellow 1.71, pink 2.83, lime 2.16). The
dark palette is unchanged; the light palette darkens each hue to clear 3:1 vs white while preserving
hue order and identity. The draw layer selects the palette by resolved theme.

| # | Hue | Dark hex (vs `#16181d`) | Light hex (vs `#ffffff`) |
|---|---|---|---|
| 0 | blue   | `#4f8cff` (5.52:1) | `#2f6fe4` (4.65:1) |
| 1 | orange | `#f2994a` (7.98:1) | `#b0530f` (5.13:1) |
| 2 | purple | `#9b6dff` (5.10:1) | `#7b46d6` (5.69:1) |
| 3 | green  | `#43b97f` (7.18:1) | `#1b7d4c` (5.14:1) |
| 4 | red    | `#e5534b` (4.80:1) | `#c62f33` (5.45:1) |
| 5 | teal   | `#3ec6c0` (8.49:1) | `#0c7d78` (4.98:1) |
| 6 | yellow | `#e8c341` (10.41:1) | `#8a6f08` (4.82:1) |
| 7 | pink   | `#f26d9c` (6.29:1) | `#c8437a` (4.63:1) |
| 8 | indigo | `#7a86ff` (5.65:1) | `#5560e0` (5.08:1) |
| 9 | lime   | `#8fbf4d` (8.23:1) | `#517c20` (4.94:1) |

Ratios are the lane color against that theme's `--bg-0` (2px stroke / dot fill) — all **≥4.6:1**,
comfortably clearing the 3:1 graphics bar (WCAG 1.4.11). Do **not** reuse the dark hex in light mode.
The light values double as the current-branch pill background (§6), so each also carries white pill
text at ≥4.5:1.

## 6. Ref pills (beside commit message)

Shape: 999px radius, `pillFont` 11px / 600 weight, padding 2px 8px, height 18px cozy / 15px compact,
max-width 160px with ellipsis, 4px gap between pills. Rendered on the canvas in graph rows, in the
left ref-column band.

| Kind | Style |
|---|---|
| Local branch | bg = lane color at 18% alpha; text + 1px border = lane color |
| Current branch (HEAD attached) | solid lane-color bg (per-theme, §5), **luminance-adaptive text** (below), prefix `⌂ ` |
| Remote branch | bg `--bg-2`, text `--text-2`, 1px `--border`; label `origin/name` |
| Tag | bg `#d4a72c` at 18% alpha, text + border `#d4a72c`, prefix `# ` |
| HEAD (detached) | solid **`#b3261e`** bg (fixed, both themes), white text, label `HEAD` |

**Luminance-adaptive pill text (added 2026-08-22, graph review).** The current-branch pill sits on a
lane color, so a fixed white label failed catastrophically on the bright lanes (white on dark-mode
yellow = 1.71:1). The label color is chosen per-pill as whichever of near-black `#16181d` or white
`#ffffff` has the higher contrast with the lane background (`isDarkBg()` already exists in
`GraphCanvas.tsx:66`). Result: near-black on the dark-mode (bright) lanes = **4.8–10.4:1**; white on
the light-mode (darkened) lanes = **4.6–5.7:1** — all clear the 4.5:1 text bar in both themes.

**Detached HEAD** uses a fixed dark red `#b3261e` background (not `--danger`, which gave white text
only 3.70:1 in dark) — white on `#b3261e` is **6.54:1** in both themes. The `HEAD` label word
carries the meaning; the color is secondary.

## 7. File status colors (right panel, M1+)

Added/staged `--success`, modified `--warning`, deleted `--danger`, untracked `--text-3` italic,
renamed `--accent`. Letter badge (A/M/D/U/R) in mono 11px before the path.

**Never let color be the only carrier of meaning** — the A/M/D/U/R letter badge is the house
precedent. Every new status indicator pairs its hue with a letter, word, or glyph. A **digit** counts
as a carrier too (§12.5's rail counts: `0` vs `N` is what makes the colour lift optional).

**House glyph vocabulary** (use these, do not invent synonyms): `✓` good/ready/checked ·
`⚠` warning/failed · `⊘` blocked/refused/cancelled · `●` neutral/informational ·
`✕` dismiss/close · `?` unknown/needs-you. A single glyph never means two different things on two
surfaces — that is why toast `error` is `⊘` and not `⚠` (§10.2).

### 7.1 Row hover actions (Changes / Staged rows and folder rows)

`.row-action` is 20×20, `opacity: 0` until the row is hovered or the button takes focus. **Order is
fixed: secondary and destructive controls first, the primary stage/unstage toggle last (rightmost).**

| Slot | Glyph | Where | `aria-label` |
|---|---|---|---|
| history | `🕑` | tracked rows | `Show history of {path}` |
| blame | `👁` | tracked rows | `Blame {path}` |
| destructive | `↺` / `🗑` | see below | `Discard changes to {path}` / `Delete {path}` |
| primary | `+` / `−` | any actionable row | `Stage {path}` / `Unstage {path}` |

The destructive slot holds **exactly one** control, chosen by whether the file exists in git:

- **`↺` "Discard changes"** — tracked rows with unstaged edits. Reverts to the staged/committed
  version. Never shown on new or staged rows.
- **`🗑` "Delete"** — new (untracked) rows only. There is no version to revert to, so the outcome
  is permanent removal from disk; a different glyph keeps that from reading as a revert.

Folder rows (`.tree-dir-actions`, same 2px gap) use the same order — a folder and its children
share the column, so `↺` sits above `↺`/`🗑` and `+` above `+`.

Both destructive controls are confirm-gated and tint `var(--danger)` on hover via
`.row-action-discard`. Confirm chrome follows the set's composition, never the entry point: a
new-files-only set is titled "Delete new file(s)" and confirms with "Delete"; any set containing a
modified file is "Discard all changes" / "Discard all". Permanently deleted paths are always
listed by name in the dialog body (first 10, then "+N more").

### 7.2 Naming an icon-only or repeated-label button (P69l)

When several controls on one surface share the same visible verb — four `Copy` buttons in
Settings → AI access — each needs an `aria-label` that **names the object**, and the label:

- **starts with the visible word** (`Copy server URL`, not `Server URL`), so a speech-input user can
  say what they see and still hit the right control;
- is **never derived from a sibling node**. A name computed from the neighbouring row label breaks
  the moment the row is reordered, the sibling is truncated, or the row moves into a search-result
  block (§12.5).
- leaves the **visible** text alone. Adding an `aria-label` to a button inside a copy-frozen section
  is not a string change.

## 8. Empty / loading / error states

- Empty (no repo): centered column — "Bonsai" 20px/600, tagline "A tidy Git client" (text-3),
  primary button "Open repository" (accent bg, 8px 16px padding).
- Empty panes (repo open, nothing selected): centered text-3 message, 13px (e.g. "Select a commit
  to see details").
- Unborn repo: empty graph pane message "No commits yet"; status panel remains usable.
- Loading: skeleton rows (bg-2 rounded bars, 1.2s pulse) for lists; graph shows nothing until
  layout arrives (no spinners over the canvas). Any operation > 300ms shows an indeterminate 2px
  accent bar under the header (`.header-progress`, `styles.css:1320`).
- **In-flight, non-blocking operations are announced with words, never spinners.** The house pattern
  is a present-participle label on the control that started the op — `Fetching…` / `Pulling…` /
  `Pushing…` (`WorkspaceToolbar.tsx:142-166`), `Checking…` (`GitMissingBanner`,
  `SettingsUpdatesSection`), `Committing & Pushing…` (`CommitBox`) — plus `.header-progress`.
  Where the trigger is a context-menu item with no persistent control, the participle goes on the
  affected **row's status pill** instead and the row carries `aria-busy="true"`
  (P73 §6.1, submodule rows). Lowercase inside a pill, sentence-case on a button; always a trailing
  `…` (U+2026).
- Errors: inline banner at top of the affected pane — `--danger` at 12% alpha bg, `--danger` text,
  6px radius, dismissible. No modal error dialogs. **Process-global** faults (not pane-scoped) use
  the app notice bar instead — §10.
- **Transient operation failures are toasts** (`Toasts.tsx`): `error` tone, sticky, `role="alert"`.
  Copy shape is `Couldn't <verb> <target>. <what to do next>` — the frontend supplies the prefix
  naming the action and the exact target, the backend's `AppError.message` supplies the remedy
  sentence and is surfaced **verbatim** (so `authFailed` / `networkError` copy is not duplicated in
  the UI). Backend messages reaching a toast must therefore be complete, capitalised,
  period-terminated, user-ready sentences — **never raw libgit2 prose and never internal paths like
  `.git/modules/…`**. Repeatable failures pass a dedupe `key` (`<domain>:<target>`) so pressing a
  failing action N times never stacks N identical alerts (§10.1 mechanism). Visual recipe: §10.2.
- Buttons: primary (accent), secondary (bg-2 + border), icon (transparent, bg-2 on hover); all
  32px tall, 6px radius (dock-density controls may go to 28/24px — §3).
- **Dialog body text (P68g).** Primary sentences: `.dialog-body`, 13px `--text-1`. Must-read
  secondary lines — consent facts, spend and destructive consequences, "written without review"
  caveats — use `.dialog-body-detail`, 12px `--text-2`. `.dialog-body-note` is `--text-3` and is
  for genuinely decorative lines only (`+N more`); never put a consequence on it.
- **An empty state inside a pane names the fix.** A bare declarative sentence ("Open a repository
  to view its Git config.") is incomplete: the block is title (13px/600 `--text-1`) + one-line
  reason (12px `--text-2`) + exactly one action button. `EmptyState.tsx` is the *app-level* no-repo
  screen (hero mark, tagline, recents, three CTAs) and must not be embedded in a panel or dialog.

## 9. Bottom AI activity dock (P68e)

Full contract: `docs/contracts/P68e-ai-activity-dock.md`. Canonical geometry:

- Placement: third child of `.workspace-host`, after `.panes`. `flex: none`, `overflow: hidden`,
  full width. Renders `null` when no AI run exists — zero layout cost by default.
- Collapsed = one status bar, `30px` cozy / `28px` compact. Expanded = that bar + a body of
  persisted height, **120–600px** (default `180`), also capped at 60% of window height.
- Resizer: 4px strip on the **top** edge (8px pointer band), `role="separator"
  aria-orientation="horizontal"`, keyboard ±8px, double-click resets to 180.
- Tokens: `--ai-dock-*` declared on `.ai-dock`, swapped by `.ai-dock[data-density='compact']`.
  Every colour token is an alias of an existing theme token (`--ai-dock-bg: var(--bg-1)`,
  `--ai-dock-log-bg: var(--bg-0)`, `--ai-dock-meta: var(--text-2)`,
  `--ai-dock-tool: var(--accent)`, `--ai-dock-attention: var(--warning)`), so one rule set serves
  both themes and **no new `:root` / `[data-theme='light']` token is introduced**.
- Status pills: word + glyph, never colour alone — `✨ Running`, `✨ Stopping…`, `? Needs you`,
  `✓ Ready`, `⚠ Failed`, `⊘ Cancelled`. Label is always `--text-1`; the hue lives in the 40%
  border and the 100% glyph over a 14% tint. **This is the canonical pill recipe — §11.**
- Log surface: `--bg-0`, mono 12px/18px cozy · 11px/16px compact, `white-space: pre-wrap`,
  stick-to-bottom with a 24/4px hysteresis band and a `↓ Jump to latest` escape button.
- **The mid-run question is untrusted model output** (security audit M3). The ask block carries an
  attribution line (`Claude wrote this — Bonsai did not:`) and a fixed, non-model-controlled line
  (`Bonsai never asks for passwords or tokens. Don’t paste secrets here.`), both `--text-1` with
  hue only as an `aria-hidden` `⚠` glyph, and the reply box's `aria-describedby` names the guard
  line **first**. See `P68e-ai-activity-dock.md` §4.1/§4.5 and `P68g-ui.md` §3.
- Streaming output is **not** an `aria-live` region. A separate visually-hidden
  `role="status" aria-live="polite"` element announces status transitions only.
- Motion: the dock never animates its height (a height transition would force repeated
  20k-row canvas relayouts). Only opacity/colour, ≤150ms, ease-out; the app's first
  `prefers-reduced-motion` block lives in this section. P70 adds `.file-chevron` (the app-wide
  120ms disclosure-caret transform) to that same block; P73 adds `.header-progress::after`
  (`animation: none; width: 100%; opacity: 0.6` — the same treatment as `.ai-dock-progress`), which
  closes P68e §12-F3 for the header sweep. P69 adds `.settings-switch-*` and `.settings-segment`.
  `skeleton-pulse` remains outstanding.

## 10. App notice bar (P70)

Full contract: `docs/contracts/P70-ui.md`. The canonical pattern for a **process-global, persistent,
non-dismissable** fault — as distinct from a pane-scoped error banner (§8) or a transient toast.
First and currently only instance: `GitMissingBanner` ("Git is not available").

- **Placement.** Direct child of `.app`, immediately after `</header>`, before the
  `.workspace-host` tab hosts. App-level, not per-tab (tab hosts stay mounted at `display:none`, so
  a per-tab banner would render N times) and visible on the no-repo empty state.
- **In-flow, never overlay.** `flex: none`, full width, no scrim, no focus trap. The rest of the
  app stays fully usable and keyboard-reachable — that is what keeps a non-dismissable surface from
  being a trap. `UpdateNotification` (fixed, bottom-right, `z-index: 90`) is a different layer;
  both may show at once.
- **Zero-cost when healthy.** The component is always mounted but returns only a visually-hidden
  live region until the fault is known, so the healthy path reserves no space and produces **no
  layout shift** when the probe resolves.
- **Geometry.** `padding: 8px 12px`, `gap: 8px 12px`, `flex-wrap: wrap`; text column
  `flex: 1 1 320px; min-width: 240px; max-width: 72ch`; actions `flex: none; margin-left: auto`;
  controls `padding: 4px 10px`, 12px, `min-height: 24px` (matching `.op-banner` / `.op-banner-btn`).
  Density-invariant (§3).
- **Severity.** Degraded-but-usable ⇒ **warning**, not danger: `background: var(--bg-1)`,
  `border-bottom: 1px solid var(--border)`, `box-shadow: inset 3px 0 0 var(--warning)` as the left
  severity rail, plus an `aria-hidden` `⚠` glyph in `--warning`. **All words are `--text-1` /
  `--text-2`** (§2's hue-as-text rule). One rule set serves both themes; **no new token**. The 3px
  left severity rail is the same device toasts use (§10.2) — it is the house severity idiom for a
  wide surface.
- **Content shape.** Title (13px/600 `--text-1`) → one-line explanation (12px `--text-2`) → the
  single best remedy (13px `--text-1` — it is the thing the user came for). Secondary remedies,
  the degraded-capability list, and the paste-into-a-bug-report technical block live behind a
  `Details` disclosure (`aria-expanded` + `aria-controls`, `.file-chevron`), default closed,
  `max-height: 220px; overflow-y: auto`.
- **Recovery action.** Exactly one primary button that re-runs the check. Pending state is
  **text-only** (`Checking…` + `disabled` + `aria-busy`) — no spinner, so it is reduced-motion-safe
  and costs the canvas nothing. A retry that fails again updates an in-bar 11px `--text-2` readout
  (`Still not found — checked HH:MM.`); it never toasts and never re-animates.
- **No dismiss control at all** while the fault holds — not a disabled `✕`.
- **A11y.** `role="region"` + `aria-labelledby` on the title; **not** `role="alert"` (an assertive
  region would re-announce the whole bar on every retry). Announcements go through a separate
  always-mounted visually-hidden `role="status" aria-live="polite"` span — the same split the AI
  dock uses (§9). Focus is **never** moved to the bar. Tab order is pure DOM order, no `tabindex`.
- **Motion.** The bar never animates its appearance, its height, or the disclosure.
- **Copy rules.** Name the real fault in the title; deny the wrong reading explicitly when a wrong
  reading is likely ("This is not a sign-in problem — your saved credentials were never used.");
  lead with the remedy that fixes the reported case; state honestly what still works and what does
  not. Never surface raw backend error prose as the bar's own text — it belongs in the technical
  block.

### 10.1 Toast dedupe key

`Toast.key` (`Toasts.tsx:18`) — an optional stable id. A `pushToast` with an existing key
**replaces** that toast in place (a no-op when the text is identical) rather than stacking. The
rule lives in App's `pushToast`; the presentational component ignores it. Use it for any action a
user will plausibly retry: convention `<domain>:<target>` (e.g. `submodule:vendor/libcore`).

### 10.2 Toast tone recipe (P74 — canonical)

Full contract: `docs/contracts/P74-a11y-toasts-hit-targets.md`. Toasts are the one hue surface
large enough that §11's all-round 40% border cannot carry the tone, so the hue moves to a
**leading edge bar**. This is the recipe for any wide tinted surface that carries prose.

- **Geometry.** `.toast-stack`: `position: fixed; top: 52px; right: 12px; width: 360px; gap: 8px;
  z-index: 90` (above panes, below `.dialog-overlay` at 100), newest on top, `aria-live="polite"`.
  `.toast`: `display: flex; align-items: flex-start; gap: 8px; padding: 8px 12px 8px 10px;
  border-radius: 6px; font-size: 13px; overflow-wrap: anywhere; box-sizing: border-box`. Text
  column **284px**. Density-invariant — a fixed overlay is outside the §3 density scopes.
- **Colour.** One local `--h` per tone (the §11 convention): `--danger` / `--success` /
  `--warning` / `--accent`. `background: color-mix(in srgb, var(--h) 14%, var(--bg-2))`;
  `border: 1px solid color-mix(in srgb, var(--h) 35%, var(--bg-2))`;
  `border-left: 3px solid var(--h)`; `color: var(--text-1)`. **No new theme token, no hex.**
- **Contrast.** Label **9.24–10.30:1** dark / **11.68–12.00:1** light (AA text ✓ in all four tones,
  both themes). Bar + glyph **3.35–4.96** / **3.38–3.69** (1.4.11 graphics ✓). The 1px 35% border is
  **1.69–2.25:1** and is decorative delineation only, exactly as in §11 — never a meaning carrier.
- **The glyph is mandatory, not decoration.** With the label at `--text-1`, the tint and the bar are
  pure colour in a fixed position, so they cannot satisfy WCAG 1.4.1 on their own. `.toast-glyph` is
  `flex: none; width: 14px; font-size: 13px; line-height: 1.45; text-align: center;
  color: var(--h)`, carries `aria-hidden="true"` (the `role="alert"` announcement must stay the
  prose only), and uses the §7 vocabulary: **error `⊘`** (every error toast is a refusal),
  **warning `⚠`**, **success `✓`**, **info `●`**. Error is deliberately not `⚠` — `⚠` already means
  "failed" in the AI dock, and error-vs-warning is precisely the pair that must separate without
  colour. Info is deliberately not `ℹ` — it has an emoji presentation on Windows and macOS and would
  ignore `--h`.
- **Dismiss.** `.toast-dismiss` is **24×24** (§3.1), `margin: -3px -4px -3px 0` so the box is
  optically centred on line 1 and reclaims 4px of text column, `color: inherit` (now `--text-1`,
  **10.30:1** dark), `aria-label="Dismiss"`, hover
  `background: color-mix(in srgb, currentcolor 12%, transparent)`.
- **Behaviour, unchanged and load-bearing.** `error` ⇒ `role="alert"` + `sticky: true`; every other
  tone auto-dismisses at 5 s; the stack caps at 5; §10.1 dedupe applies. Toasts never take focus,
  Esc does not dismiss them, they are not in the command palette, and the component returns `null`
  at zero toasts.
- **Long content.** No `max-height`, no clamp, no ellipsis — a toast wraps to whatever height it
  needs (`overflow-wrap: anywhere`). The worst observed real case (a submodule URL-mismatch refusal
  with a 91-char path and two URLs) is ≈360×244px and stays readable and dismissible.
- **Motion.** None. Toasts appear and disappear without transition, so there is nothing for
  `prefers-reduced-motion` to honour and nothing contending with the canvas render budget.

## 11. Status pills (rows and chrome)

The canonical recipe, first shipped as the AI dock's status pills (§9) and applied to the sidebar
row badges (`.submodule-badge-*`, shared by submodule and worktree rows — `Sidebar.tsx`).

- **Shape.** 11px, `padding: 1px 6px`, `border-radius: 8px`, `flex: none`, inherited UI font
  (not mono — mono is for hashes and paths). Never compresses; the row's *name* is what ellipsizes.
- **Label.** A **word**, lowercase inside a row pill (`up to date`, `out of sync`, `modified`,
  `not checked out`, `repo`, `in use`) and sentence-case in chrome (`Running`, `Failed`). Colour is
  never the sole carrier (§7).
- **Hueless / informational pills** — no verdict, just a fact or an in-flight state: label
  `--text-2` over its own 12% tint (**5.79:1** dark / **6.22:1** light, §2). No glyph, no hue — but
  keep `border: 1px solid transparent` so hueless and verdict pills are the same **19.94 px** height
  in a shared list.
- **Verdict pills** — good/warning/bad: label `--text-1`, hue in the 40% border and in a 100%
  `aria-hidden` glyph over a 14% tint (`✓`, `⚠`, `⊘`). The glyph is the accessible hue carrier
  (measured 2026-08-19: `✓` **4.61:1** dark / **3.94:1** light, `⚠` **5.64:1** / **3.80:1** — both
  clear the 3:1 graphics bar). The 40% border is decorative delineation only and measures
  **1.7–2.3:1** against the row background; never rely on it to carry meaning. Set the hue through
  the local `--h` custom property, the AI-dock convention. **Do not use the hue as the label colour**
  over its own tint — that recipe misses AA (§2).
- **This recipe is size-bounded (added 2026-08-20, P74).** The 40% perimeter border reads as tone
  only at pill scale (≈20px tall, ≈60–110px wide). On a wide surface — a toast, a notice bar, a
  banner — the same hairline measures 1.7–2.3:1 across a 360px edge and disappears; there, move the
  hue into a **3px leading edge bar** and keep the `--text-1` label + 100% `aria-hidden` glyph
  unchanged (§10.2 toasts, §10 notice bar). The label rule and the glyph rule never change with
  size; only the hue's *shape* does.
- **Title attribute.** A pill's `title` explains *why* the state holds and what fixes it; it must
  never merely repeat the visible label. Keep the *why* out of the visible row text.
- **Busy pill.** While an op runs on that row, the pill's label becomes the present participle
  (`checking out…`, `updating…`) in the hueless style and the row gets `aria-busy="true"` (§8). The
  pill drops its `title` entirely while busy — the participle is the whole message.
- **Density-invariant** — pills live in the sidebar and chrome, which have one geometry (§3).
- **A pill that carries meaning must also be in the accessible name.** When a pill qualifies an
  interactive row (e.g. the Settings rail's `repo` scope pill, §12.1), the pill itself is
  `aria-hidden` and its text is folded into the control's accessible name. Prefer a visually-hidden
  span; use an explicit **`aria-label`** when punctuation matters, because name computation joins
  sibling nodes with a space and would produce `Git config , repository` (the shipped rail does
  exactly this). A purely visual pill leaves AT users without the qualifier.

## 12. Settings surface (P69)

Full contract: `docs/contracts/P69-settings-ui.md`. The canonical pattern for a **categorised
preference surface**, and the home of the app's switch / segmented / settings-row specs. Any future
milestone adding a setting adds a row here, not a new section on a scrolling column.

### 12.1 Two-pane shell

```
┌──────────────────────────────────────────────────────────────┐
│ Settings                                                 ✕   │ 48px, spans both cols
├──────────────────┬───────────────────────────────────────────┤
│ rail 200px       │ 🔍 Search settings                        │ 56px
│  role=tablist    ├───────────────────────────────────────────┤
│  (spans rows 2-3)│ pane  role=tabpanel  --bg-0               │
│  ▌selected       │  title / subtitle / groups of rows        │
│  Git config[repo]│                                           │
└──────────────────┴───────────────────────────────────────────┘
                          880px
```

- Card: `.dialog-card.settings-card`, `width: 880px; max-width: calc(100vw - 48px);
  height: min(660px, calc(100vh - 64px)); display: grid;
  grid-template-columns: 200px 1fr; grid-template-rows: 48px 56px 1fr; overflow: hidden;`
  `--bg-1`, 1px `--border`, radius 6, over the existing `.dialog-overlay` scrim
  (backdrop-mousedown closes).
- **One grid, three rows** — header / search bar / pane — with **every cell placed explicitly** and
  the rail at `grid-row: 2 / span 2`. That is what lets the search bar **precede the rail in DOM
  order** (giving the ✕ → search → rail → pane tab order for free) while sitting beside it on
  screen. Do not nest a second grid for the content column.
- **The card never scrolls as a whole.** The rail and the pane scroll independently; the header and
  the search bar are fixed.
- Header 48px, `padding: 0 8px 0 16px`, title 15px/600 `--text-1`, bottom 1px `--border`.
- Rail `--bg-1`, `padding: 8px`, right 1px `--border`; items 32px tall, `padding: 0 8px`, radius 6,
  13px `--text-2`, 2px gap. Selected: `--selection` bg + `--text-1` + 600 +
  `box-shadow: inset 2px 0 0 var(--accent)` (the accent bar is the shape carrier — `--selection` vs
  `--bg-1` is only ~1.3:1).  Hover `--bg-2`; pressed `--bg-3`.
- Rail grouping is done with `aria-hidden` 1px `--border` dividers and a hueless scope pill
  (§11) — **never** with heading elements inside the `role="tablist"`.
- Search bar `--bg-0`, `padding: 12px 16px`, bottom 1px `--border`; the shared `ListFilterInput`
  overridden to a full-width 32px field (compound selectors, so the override wins on specificity
  whatever the import order).
- Pane `--bg-0`, `padding: 16px 24px 24px`, own scrollbar; `scrollTop` resets to 0 on category
  change. Pane header: title 15px/600 `--text-1` + subtitle 12px `--text-2`, block margin-bottom 16.
- Group: `margin-bottom: 24px`; group title 11px uppercase, `letter-spacing: .08em`, `--text-3`
  (decorative — it duplicates visible structure).
- **Density-invariant** (§3): one geometry in both `cozy` and `compact`.
- Below 720px wide the rail collapses to a 40px horizontally-scrolling strip above the search bar
  (`grid-template-rows: 48px 40px 56px 1fr`); roles and tab order are unchanged. Below 560px tall the
  card takes `calc(100vh - 32px)`.
- CSS lives in `src/styles/settings-shell.css` (shell) and `src/styles/settings-primitives.css`
  (row, switch, segmented, text, reset, hint) — not in `styles.css`. Cascade order is fixed by the
  import list; do not reorder.

### 12.2 Settings row anatomy

```
grid-template-columns: 1fr auto 24px;  column-gap: 12px;  align-items: center;
padding: 10px 0;  min-height: 44px;   sibling rows separated by 1px --border
row 1: [ label 13px --text-1        ] [ control ] [ ↺ ]
row 2: [ help 12px --text-2, 56ch   ]  control spans both rows
```

- Help text is **`--text-2`, never `--text-3`** (§2), carries `id="{rowId}-help"`, and is wired to
  the control with `aria-describedby`. A setting whose effect is not obvious from its label gets
  help text; a section-level paragraph explaining three different rows is a smell — split it.
- **Exactly one help line per row.** `.settings-row-help` is the static sentence, owned by the
  catalog. A row whose explanation must track the live value uses `.settings-row-note` instead —
  identical typography (12px `--text-2`, `margin-top: 2px`, `max-width: 56ch`), same help cell,
  emitted by the call site because the catalog is static data — and then carries **no** catalog
  `help` at all. **Never both.** A static line sitting above a stateful line that restates it is the
  section-paragraph smell moved down one level, and it inflates the row by ~45px.
- **Everything visible in the help cell is in the control's `aria-describedby`.** Where a row
  legitimately shows two paragraphs (a note plus a conditional caveat), the control names both ids,
  space-separated. A visible sentence the screen reader never hears is the same defect as a hidden
  one, and it is the failure mode of a note that lives on a *neighbouring* row.
- The help cell (`.settings-row-help-slot`) is also the slot for the §12.3.4 draft hint. Slider rows
  reserve `min-height: 18px` on it so nothing below moves when the hint replaces the help text.
- `.settings-group-lead` — 12px `--text-2`, `margin: 0 0 8px`, 56ch — is the **group-level**
  paragraph: a frozen section description, or the §12.3.3 gate note. It sits directly under the
  group title and outside every row. `.settings-group-note` is the same type at the **foot** of a
  group, for a caveat qualifying several rows at once ("PR and CI badges need a connected forge…").
  Neither is a substitute for per-row help; use them only when the sentence genuinely has no single
  owning row.
- Reset `↺`: 24×24 `.btn-icon`, `aria-label="Reset {label} to default"`,
  `title="Reset to default ({value})"`. **Conditionally rendered** when value ≠ default; the 24px
  grid column is always reserved so the row never shifts. Reset lives **per row only** — no
  per-category and no global reset.
- `.settings-row--stacked`: label / help / control each on its own grid row, control at
  `width: 100%`. Use for text fields, paths, and read-only value + Copy pairs.
- Two rows in one dialog must never share an accessible name (`Fetch every` / `Refresh every`, not
  `Interval` / `Interval`; `Time limit` / `Spend limit`, not `Limit` / `Limit`). A `NumberSlider`
  produces **two** named controls — the range twin is `aria-label`led from the same string — so one
  duplicated label is four ambiguous nodes. Proximity to a distinguishing switch above does not
  count: the accessible name must stand alone.
- **The row is the search unit.** Every row is stamped with its catalog id and removes itself when a
  query does not hit it (§12.5). A control stamped **outside** `SettingsRow` must self-filter with
  the same hook, or a single hit in its category renders the whole block as a "result".

### 12.3 Control kinds

| The setting is… | Control |
|---|---|
| An independent on/off | **Switch** (§12.3.1) |
| One of 2–3 exclusive values, short self-explanatory labels | **Segmented** (§12.3.2) |
| One of 2+ exclusive values where each needs a sentence | Radio group, stacked, hint under each |
| A bounded number tuned by feel | `NumberSlider` (slider + number + unit) — §12.3.4 |
| Free text, a path, an unbounded value | Text field, stacked row |
| A one-shot action | Button |
| More than 3 exclusive values | `Combobox` |

**A button labelled with its own current value is not a control** — `[ Dark ]` reads as "make it
dark". Never use one for state.

#### 12.3.1 Switch

CSS over a **native `<input type="checkbox">`** — the implicit `checkbox` role, native Space
toggling, and every `getByRole('checkbox', {name})` query survive. `role="switch"` is deliberately
**not** used.

- Wrapper `<label>`: `position: relative; display: inline-flex; align-items: center;
  min-height: 24px; width: 36px; flex: none;`
- The input is `position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; margin: 0;` —
  it **is** the 36×24 hit target.
- Track 36×20, radius 999: off `background: var(--text-2)`; on `background: var(--accent)`.
- Knob 14×14, radius 999, `background: var(--bg-0)`; `translateX(3px)` off → `translateX(19px)` on.
- **The knob's position is the non-colour meaning carrier.** Never rely on the track hue.
- Motion: `transform`/`background-color` 120ms ease-out, in the §9 reduced-motion block.
  Hover: track `filter: brightness(1.08)`. Active: knob `width: 17px`.
- Focus: `.settings-switch:has(> input:focus-visible) .settings-switch-track` → 2px `--accent`,
  offset 2px. Disabled: wrapper `opacity: .55; cursor: not-allowed`.
- Contrast, all four boundary pairs on `--bg-0`: **5.6–7.9:1** dark, **4.7–4.9:1** light (§2).

#### 12.3.2 Segmented

CSS over **native `<input type="radio">`** inside a `role="radiogroup"` labelled by the row label —
arrow-key navigation and `getByRole('radio', {name})` come free. **Never** `role="tablist"`.

- Container `display: inline-flex; background: var(--bg-2); border: 1px solid var(--border);
  border-radius: 6px; padding: 2px; gap: 2px;`
- Segment `min-height: 24px; padding: 0 10px; border-radius: 4px; font-size: 12px;
  color: var(--text-2); border: 1px solid transparent;` hover `background: var(--bg-3)`.
- Selected `background: var(--selection); color: var(--text-1); font-weight: 600;
  border-color: var(--accent);` — the 600 weight and the accent border are the non-colour carriers
  (`--accent` on `--bg-2` = **4.4:1** dark / **4.1:1** light).
- Focus: `.settings-segment:has(> input:focus-visible)` → 2px `--accent`, offset 1px.
- Max 3 segments.
- A radio group that stays a **radio group** (each option needs a sentence) is a
  `role="radiogroup"` **div** named by the row label via `aria-labelledby` — not a
  `<fieldset>`/`<legend>`, which reports `group` and hides the row label from the control's
  accessible name. Each option's sentence is `--text-2` at the **same 12px as row help**; an 11px
  hint next to 12px help in the same dialog is an inconsistency, not a hierarchy.

#### 12.3.3 Disabling a whole group

- One `<fieldset disabled>` around the dependent rows. It is the only mechanism that removes every
  descendant from the tab order in one place, and it maps exactly to the real dependency.
- The reason **leads** the group as a `.settings-group-lead` carrying an id, and the `<fieldset>`
  carries `aria-describedby` pointing at it (only while the note exists — a dangling idref is worse
  than none), so the reason is announced on entry rather than discovered afterwards.
- The `.55` dim lives on `.settings-row.is-disabled`, **never on the `<fieldset>`**: `opacity` is a
  group property, so dimming the fieldset would dim the very sentence that explains the dim.
- **`opacity` never nests** (§2, dimming budget). A dimmed row's descendants must not dim
  themselves — `.55 × .55 = .30`. Any control with its own `disabled` opacity rule is reset to `1`
  inside `.settings-row.is-disabled`, and the override must win on **specificity**, not source
  order: `:has()` takes the specificity of its most specific argument, so the obvious override ties
  and the *later* rule silently wins. Qualify the override so it cannot tie, and keep it after the
  rule it defeats.
- Hue never carries disablement, and the switch knob's **position** still reads on-vs-off through
  the dim.

#### 12.3.4 NumberSlider — draft divergence feedback

Full spec: `P69-settings-ui.md` §13. **Specced, not yet implemented** — the CSS hook, the reserved
help slot and `SettingsRow`'s `hint` prop shipped in P69; the classifier, the timer and the
`aria-invalid` wiring have not.

The number input shows a **draft** while focused and commits the clamped value on every keystroke
(P69c), so the field can legitimately show text the setting does not hold. Whenever it does, say so.

- **Divergent** = editing, and the draft is blank/non-numeric, or `Number(draft) !== value`. Compare
  numerically — `06` and `6e1` are not divergences and must never warn.
- **Treatment:** `--warning` 1px border + `inset 0 0 0 1px var(--warning)` on the input (no box-model
  change, focus **outline** untouched), plus one 12px `--text-2` sentence in the row's help slot,
  which hides the help text while it shows. Ring and sentence are **one state** — colour never
  travels alone. Never `--danger`: the setting is already at a legal value, nothing is lost. Ring
  contrast on `--bg-2`: **6.4:1** dark / **4.2:1** light (§2).
- **Timing (the rule):** an **above-maximum** draft warns on the keystroke that causes it — appending
  digits only increases, so it is terminal and no further typing can fix it. **Below-minimum, blank,
  and non-integer** drafts warn only after **600 ms with no keystroke**, because every valid entry
  passes through them (`30` passes through `3`; re-typing passes through empty). Once showing it
  stays showing and switches text without re-arming; it hides instantly when the draft becomes
  valid, and always dies with the draft on blur/Enter.
- **Copy:** `Too high — will be set to {max} {unit}.` / `Too low — will be set to {min} {unit}.` /
  `Whole numbers only — will be set to {value} {unit}.` / `No value — stays at {value} {unit}.`
  Subject-free (naming the label yields `Fetch every will be 300 seconds.`), ≤40 chars, says the
  fault and the outcome in that order.
- **A11y:** `aria-invalid="true"` exactly while showing (removed otherwise); the hint `<p>` is
  permanently present with `aria-live="polite"` and empty text when idle; `aria-describedby`
  **composes** `{help-id} {id}-draft` and never replaces an existing description.
- **Layout:** the help slot reserves `min-height: 18px` on slider rows so nothing moves while typing.
  Every slider row should therefore carry real help text, or that line sits empty.
- Applies to all eight sliders and to the hand-rolled USD field (with `integer: false`, so the
  whole-numbers case is off).

### 12.4 Keyboard, roles, focus

- `Ctrl/Cmd + ,` opens Settings, registered **above** App's `typing` guard so it fires from the
  commit box. No-op when already open — a shortcut must not toggle a modal.
- Card: `role="dialog" aria-modal="true" aria-labelledby="settings-title"`. Rail:
  `role="tablist" aria-orientation="vertical"`, items `role="tab" aria-selected aria-controls`.
  Pane: `role="tabpanel" tabindex="-1" aria-labelledby="{selected tab}"`.
- Tab order is pure DOM order (close ✕ → search → rail → pane). The rail is **one** tab stop with a
  roving tabindex that follows **focus**, not selection (the `AiRunStrip.tsx:54-75` precedent).
- **Manual activation** (P69 D-5, corrected here 2026-08-20): `↑`/`↓` **move focus only**;
  `Enter`/`Space` selects. `Home`/`End` move focus to first/last; `→` moves focus into the pane.
  Automatic activation is wrong for this rail because it would fire a `getConfig` IPC round-trip
  every time focus passed *over* `Git config` — arrow-browsing a rail must not perform I/O. For the
  same reason, selecting a category does **not** move focus into the pane.
- **There is no focus trap** (P69 D-4). This codebase has no shared trap utility and no dialog has
  one, so adding one to Settings alone would be the inconsistency; a half-implemented trap is worse
  than none. Anyone introducing one introduces it for every dialog in the same pass.
- Focus **restore** does ship: the element active when the dialog mounted, falling back to the ⚙
  trigger and then `<body>`.
- Initial focus: the search input — a text field, so no accidental activation. A **deep-linked**
  open (`initialCategory` given, or `configInitialFocus` set) deliberately sets **no** initial focus
  of its own and leaves placement to the target section's own effect (which focuses `user.name`); a
  search box grabbing focus there would defeat the commit-error linkage.
- `Esc` is layered: it first clears a non-empty search (the `ListFilterInput` capture-phase idiom),
  then closes the dialog. `Enter` never closes — settings apply live and there is no "OK".
- Every hit target ≥24px: rail 32, switch 36×24, segment ≥24, reset 24, close 32, `Go to {Category}`
  24 (§3.1).

### 12.5 Settings search

- One model: the query produces a **cross-category result list** of live, editable rows grouped by
  category — not a rail filter and not a within-page filter. The failure mode being solved is "I
  know the setting's name, not its category".
- **No second renderer.** Each hit category's own page is mounted inside a search context and every
  row that did not match removes itself, so a result can never drift from the pane. Consequences:
  a control stamped outside `SettingsRow` must self-filter (§12.2), a group hides itself when no
  child survived, and a `<details>` disclosure containing hits is **forced open while a query is
  running** — a hit inside a collapsed disclosure is an invisible result.
- Matching: synchronous, case-insensitive substring, all whitespace-separated terms required, over
  `label` + `help` + a never-displayed `keywords` string (the `PaletteAction.keywords` idiom). Not
  fuzzy. A row that carries a stateful `.settings-row-note` instead of catalog `help` (§12.2) must
  compensate in `keywords` — that is the only place its vocabulary can live.
- **Only renderable rows can match.** The matcher filters through the availability gate first: a row
  whose precondition fails is not in the DOM, so matching it would report a count nobody can see and
  hand the result list a category whose block renders empty. One list feeds the status line, the rail
  counts and the results, so the three cannot disagree.
- The searchable index is **pure data in its own modules** (`settings/settingsCatalog.ts` +
  `settings/catalog/*.ts`), never inlined in a component — the same rule as any large static table.
  It is the same catalog that supplies every row's label, help and reset descriptor.
- Matched substrings in the label are wrapped in `<mark>`: `background: var(--selection);
  color: var(--text-1);` (**9.4:1** dark / **13.3:1** light). Overlapping ranges from different terms
  are merged before wrapping, never nested. Help text is not highlighted today; the specced fallback
  (`P69-settings-ui.md` §3.2.1, **not implemented**) highlights the **help** line only for rows whose
  label produced no ranges, so a `keywords`-driven query stops pointing at nothing.
- Result group header: category name, 11px uppercase 600, **`--text-2`** — not `--text-3`. In a
  result list this heading is the user's only wayfinder to the row's category, so it is text the user
  must read (§2). Trailing `Go to {Category}` text button, 12px `--text-2` → `--text-1` on hover,
  24px min hit box (§3.1).
- **Rail counts: emphasise the hits, never dim the misses.** Each item shows a right-aligned
  `aria-hidden` count (11px, tabular numerals). An item with matches lifts label **and** count to
  `--text-1`; a zero-count item keeps the resting `--text-2` and shows `0`. Every item stays
  clickable — clicking any of them clears the query — so the earlier `opacity: .5` on zero-count
  items was inadmissible (§2 dimming budget: 2.87:1 dark / 2.36:1 light on a control the user is
  meant to click). The count is **`--text-1`, not `--accent`**: accent-as-text fails AA on the
  `--selection` fill of a selected rail item in both themes (§2). The non-colour carrier is the
  **digit** (`0` vs `N`); the colour lift is secondary.
- Counts are per catalog **entry**, not per rendered instance — a `repeats: 'perProfile'` row counts
  once while rendering three rows. State it wherever it is consumed; do not change it in one place.
- Result count changes are announced by a visually-hidden `role="status" aria-live="polite"`:
  `{n} settings match` / `1 setting matches` / `No settings match`. One sentence shape for all three
  counts.
- **While a query is active, a rail tab's accessible name gains a count suffix** —
  `{label}[, repository][, {n} match|matches]` — because the visible count is `aria-hidden` and this
  is the only way an AT user gets the per-category signal the colour lift gives a sighted one. It is
  the one sanctioned exception to the frozen-name rule, and it means
  `getByRole('tab', { name: 'Commit graph' })` **does not match mid-search**: query with a prefix
  regex in any test that spans both states.

### 12.6 Identity in the header

- The identity control is the far-right item of `.header-toolbar` (§1) and reads the **effective**
  Git identity — `local` if set, otherwise `global` — and **names its source** in the menu. A
  local-only read is wrong: Git resolves local-over-global, so a repo with no local identity still
  commits fine, and a control that showed nothing there would be lying.
- Trigger: 32×32 button containing a 22px circle of **initials** (first letters of the first two
  name words, max 2). No identity → glyph `?` with a 1px `--warning` ring (**7.3:1** / **4.5:1**);
  the glyph and the accessible name, not the hue, carry the state. Loading → `·` + `aria-busy`.
  `aria-haspopup="menu"` + `aria-expanded`.
- Menu: a `ContextMenu` anchored with the house idiom (`rect.right`, `rect.bottom + 2`), a
  non-interactive `header` block stating name / email / source, then one row per saved identity with
  `checked` (⇒ `role="menuitemradio"`) and a `detail` second line, then `Manage identities…`.
- The menu owns its open state and lifts it via `onMenuOpenChange` (the `TabStrip.tsx:35-37`
  precedent) because App early-returns global shortcuts while a menu is open.
- **Writing an identity into a repo confirms only when it would overwrite a differing *local*
  value** — writing into an empty slot destroys nothing. The confirm names both identities and the
  consequence, uses `confirmVariant='primary'` (recoverable), and says
  `Commits you have already made are not changed.`
- P69 added **four** additive fields: `ContextMenuItem` gained `checked`, `detail` and `busy`, and
  `ContextMenuProps` gained `header` and `busy` (`ContextMenu.tsx:50` / `:72`). All are additive:
  absent ⇒ byte-identical rendering to before. The **check column belongs to the list, not the
  row** — it is reserved whenever any item declares `checked`, so plain rows in the same menu stay
  aligned with the labelled ones.

### 12.7 Forge accounts (P79/P80)

- **Provider display without color as sole carrier:** `ForgeProviderBadge` (2-letter monogram, GH /
  GL / BB / AZ / ??) + `ForgeAvatar` (image or login-initial monogram, 22px cozy / 20px compact).
  Both reuse `.identity-avatar` / `.pr-draft-tag` geometry; no hue carries meaning.
- **PR-panel account switcher (P80)** reuses the §12.6 identity-menu idiom exactly: a `ContextMenu`
  anchored `rect.right` / `rect.bottom + 2`, a non-interactive `header` block (`Accounts on {host}`,
  plus the no-default nudge line when applicable), one `checked` (`role="menuitemradio"`) row per
  account with a `detail` second line, then `Use host default` + `Add another account…`. The
  left group (avatar + login + host) is the trigger, shown as a button **only when the host has ≥2
  accounts** (no switcher chrome for a single account). Writes are optimistic + `busy`.
- **`AccountSource` label vocabulary** (canonical microcopy — do not reword per surface;
  `src/components/forgeAccountSource.ts`):

  | `accountSource` | header caption | tooltip |
  |---|---|---|
  | `override`    | `Pinned to this repo` | `Pinned to this repository. Other repositories on this host use the default.` |
  | `ownerMatch`  | `Matched by owner`    | `Chosen because its username matches this repository's owner.` |
  | `hostDefault` | `Host default`        | `The default account for this host.` |
  | `single`      | *(none)*              | *(none)* |
  | `none`        | *(none)*              | *(n/a — connect view shows)* |

- **Per-repo vs global semantics (P80):** the PR panel is **per-repo** — it pins/unpins the repo's
  account override (`Reset to host default` is nondestructive, no confirm) and never signs out.
  **Full sign-out (keychain token deletion via `forgeRemoveAccount`) lives only in Settings →
  Accounts**, always behind a danger `ConfirmDialog` that names the account and states pinned repos
  fall back. Never label a per-repo unpin "Disconnect" (it reads as sign-out).
- Connected-state chip: `● Connected` (`--success` dot, `--text-2` word) / `○ Token missing`
  (`--text-3` dot) — the word carries meaning, never the dot color alone.
- No new tokens were introduced for any forge-account surface; all classes are built from existing
  `--bg/-text/-border/-accent/-success/-warning/-danger` tokens.

### 12.8 Identity profile colors (P82)

- **Purpose:** an at-a-glance answer to "which identity is this repo on?", robust to duplicate
  labels. A curated 9-value palette (`ProfileColor`: `neutral` + 8 hues), never free-form hex.
  Deliberately separate from the semantic (`--success/--warning/--danger/--accent`) and graph
  (`--lane-*`) sets — an identity swatch must not read as a status or a branch lane.
- **Tokens (new, both themes).** Swatch/ring fills only — never used as text, never the sole carrier
  of meaning (always paired with the profile label or the avatar initials + accessible name):

  | `ProfileColor` | token | dark | light |
  |---|---|---|---|
  | `neutral` | `--profile-neutral` | `#6b7280` | `#8a919e` |
  | `slate`   | `--profile-slate`   | `#7d8aa3` | `#5d6b85` |
  | `blue`    | `--profile-blue`    | `#4f8cff` | `#2f6fe4` |
  | `teal`    | `--profile-teal`    | `#3ec6c0` | `#0f8f89` |
  | `green`   | `--profile-green`   | `#57ab5a` | `#1a7f37` |
  | `amber`   | `--profile-amber`   | `#e8c341` | `#9a6700` |
  | `orange`  | `--profile-orange`  | `#f2994a` | `#c2410c` |
  | `purple`  | `--profile-purple`  | `#9b6dff` | `#7c3aed` |
  | `pink`    | `--profile-pink`    | `#f26d9c` | `#c2266f` |

  **Contrast:** all nine ≥3:1 (non-text/graphics) against their panel background in both themes —
  dark vs `--bg-1 #1d2026` (min 3.4:1, neutral), light vs `#ffffff`/`--bg-1` (min 3.1:1, neutral).
  Each swatch also carries a 1px `--border` outline so its edge survives when a hue is close to the
  row background. No `-text` variants exist: profile text is always `--text-1`/`--text-2`.
- **Swatch primitive** `IdentityColorSwatch` (`src/components/IdentityColorSwatch.tsx`): a 10px
  (`size="sm"` 8px) circle, fill chosen by `.identity-swatch[data-profile-color='<c>']` CSS
  attribute selector — **no inline color, no hex in TSX**. `aria-hidden` everywhere except the
  picker (adjacent text is the accessible name).
- **Appears in:** the header avatar (2px hue ring when a non-neutral profile is matched; unset
  `?`+`--warning` ring keeps priority), identity-menu rows (reuses the existing `ContextMenuItem.icon`
  slot — no `ContextMenu` change), the menu header block (when the effective identity matches a
  profile), and the Settings profile card head (beside the title; the `in use` badge stays the
  textual "active" carrier).
- **Picker** `IdentityColorPicker` (`src/components/settings/IdentityColorPicker.tsx`): a
  `role="radiogroup"` of native `<input type="radio">` (the `SettingsSegmented` idiom, but a swatch
  grid — segmented is text-only and caps at 3). Nine ≥24px swatch cells; selected = 2px `--accent`
  ring + full-size dot; each radio's accessible name is the color name (`Neutral`…`Pink`). Duplicate
  hues across profiles are **allowed** (labels disambiguate). New catalog control type `'color'` on
  `SettingsIndexEntry`; catalog row `identities.profile-color` (`requires:'profile'`,
  `repeats:'perProfile'`).
- **Auto-distinct (UI layer, no persistence rewrite):** create-flow and the header save-as draft use
  `nextFreeHue(profiles)` (first unused hue in table order, wrap to least-used). Pre-P82 profiles
  (`color` absent) render a distinct **display-fallback** hue by array index (`ASSIGNABLE_COLORS[i%8]`);
  an explicit `neutral` is honoured as grey. The concrete color is written through the whole-array
  patch the moment the user touches the picker.
- **Motion:** only the ≤120ms selected-swatch grow/ring on the picker; collapses under
  `prefers-reduced-motion`.

### 12.9 PR actions — merge & close/decline (P83)

- **Footer action bar** `.pr-actions-bar`: the canonical pattern for a panel's terminal, commit-point
  actions. Pinned under the scrollable content region (e.g. `PrDetailView`), not in the header
  (header = navigation + metadata; actions read as a distinct commit-point). Full panel width, top
  `1px solid var(--border)`, `display:flex; justify-content:space-between; gap:8px`; padding
  `12px 16px` cozy / `8px 12px` compact. Buttons at the standard height (32px cozy / 28px compact;
  hit target ≥24px met on both densities). Rendered only while the item is actionable (PRs:
  `summary.state === 'open'`; a merged/closed PR shows its state pill and no bar).
- **Hierarchy:** exactly one primary + one quieter danger-secondary — no third button. Affirmative
  action on the right (`btn-primary`, label ends in `…` when a dialog follows, e.g. `Merge…`);
  the destructive/abandoning action on the far left (`.btn-secondary-danger`). This mirrors dialog
  button order (destructive-left, affirmative-right) so muscle memory transfers. The left label is
  per-context/per-forge (`Close` / `Decline` / `Abandon`); meaning is carried by the verb, never hue.
- **`.btn-secondary-danger`** recipe: a `btn-secondary` modifier whose base state is quiet
  (`--text-2` text, `--border`) so it does not compete with the primary, tinting text/border to
  `--danger` on `:hover`/`:focus-visible`. Built from `--danger` on `--bg-1` — the same pair as
  `btn-danger` text, which already clears ≥4.5:1 in both themes. Color is never the sole signal: the
  verb and the follow-up confirm dialog restate the consequence.
- **Form dialog pattern** `.pr-merge-card`: when a confirmation needs form fields (a picker + optional
  text + a checkbox) that the shared `ConfirmDialog` cannot host, build a dedicated dialog on the
  existing `.dialog-card` chrome — and that dialog *is* the confirmation (no second modal). Width
  420px (matches `.ai-consent-card`, the form-bearing-dialog precedent; the 360px default is too
  tight for a picker + fields). Same overlay, Esc, and overlay-click-cancels behaviour as
  `ConfirmDialog`. Structure top→bottom: `dialog-title` → lead summary paragraph (names consequence +
  irreversibility) → a method **radiogroup** (`SettingsSegmented`, `role="radiogroup"`, options
  filtered to `SUPPORTED_MERGE_METHODS[kind]`, with a one-line `.pr-merge-method-desc` in `--text-3`
  updated per selection) → optional commit title/message fields (`.pr-input` / `.pr-textarea`, shown
  only for methods that consume them) → a delete-source-branch checkbox (`.pr-draft-toggle` idiom,
  default OFF, **hidden when `kind === 'gitHub'`** since GitHub ignores it on merge) → `.dialog-buttons`
  Cancel + confirm. The merge confirm is `btn-primary` (constructive happy path; irreversibility is
  carried by the copy per house destructive-copy rules), busy label `Merging…` with `disabled` in
  flight.
- **Confirm-dialog reuse for close/decline:** the form-less destructive path reuses `ConfirmDialog`
  verbatim (`confirmVariant='danger'`) with per-forge title/label/body (Close / Decline / Abandon).
  No new dialog for it.
- **A11y (hard rules, consistent with the §12.x dialogs):** initial focus lands on **Cancel** in both
  the merge and close dialogs (a stray Enter never fires an irreversible action); Esc and
  overlay-click cancel; focus returns to the invoking bar button on close (focus restore). The merge
  dialog is `role="dialog" aria-modal="true"` labelled by its `dialog-title`; the method group is
  `role="radiogroup"` with an id-wired label; the checkbox is a real `<input type="checkbox">` +
  `<label>`. `aria-busy="true"` on the panel (`.pr-detail`) while an action is in flight, with the
  confirm button's busy label as the visible cue (no spinner-only state). Focus ring 2px `--accent`,
  1px offset, `:focus-visible` only. No new tokens; the only motion is the dialog's existing ≤150ms
  fade/scale-in, which already honours `prefers-reduced-motion`.
