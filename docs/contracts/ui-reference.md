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
  left to right: theme · list view · AI assets · health · settings · **identity** (§12.4). The
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

- `--text-3` is **3.38:1** on `--bg-1` and **3.67:1** on `--bg-0` (dark), **2.96:1** on `--bg-1`
  (light). That is below the 4.5:1 AA bar for text. Treat `--text-3` as **decorative only**
  (uppercase section labels that duplicate visible structure, dividers, disabled glyphs). Any text
  the user must actually read — metadata, timestamps, costs, log lines, hints, **status-pill
  labels**, **settings help text** — uses `--text-2` (**7.9:1** dark / **4.9:1** light on `--bg-0`;
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

Additional measured pairs (2026-08-19, P70 pass), all on `--bg-1`: `--text-1` **13.5:1** dark /
**15.4:1** light; `--warning` glyph **7.3:1** / **4.5:1**; `--success` glyph **5.7:1** / **4.7:1**;
`--danger` glyph **4.4:1** / **4.6:1** — all clear the 3:1 graphics bar in both themes. And
(2026-08-19, P73 pass) `--text-2` over its **own** 12% tint on `--bg-1`: **5.79:1** dark /
**6.22:1** light — the safe recipe for a hueless informational pill (§11).

Measured pairs added 2026-08-19 (P69 Settings pass), on `--bg-0`: `--accent` fill **5.6:1** dark /
**4.7:1** light; `--text-2` fill **7.9:1** / **4.9:1**; `--text-1` on `--selection` **9.4:1** /
**13.3:1**; `--accent` 1px border on `--bg-2` **4.4:1** / **4.1:1**. `--accent` on `--selection` is
**2.6:1** / **3.6:1** — decorative delineation only, never a meaning carrier.

Measured pairs added 2026-08-20 (P74 pass), `--text-1` over a hue's own 14% tint on `--bg-2` — the
canonical "hue surface, readable words" pair: **9.24–10.30:1** dark, **11.68–12.00:1** light,
across all four of `--danger` / `--success` / `--warning` / `--accent`. Use these numbers for any
new tinted surface that must carry prose.

**Dimming budget (added 2026-08-20, P69j pass).** `opacity: .55` on `--text-1` over `--bg-0` lands
at ≈**4.2:1** dark / **3.6:1** light — acceptable *only* on genuinely inert controls (WCAG 1.4.3
exempts inactive components). It is a budget, not a free knob: **it may be spent once per subtree**.
Two nested `.55` layers compound to **.30**, which is ≈2.5:1 dark / ≈1.8:1 light and unreadable in
both themes. See §12.3.3.

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

- Row height: **28px**. Lane width (x-spacing between lane centers): **16px**. Left graph gutter:
  12px before lane 0.
- Commit dot: radius **4px**, filled with the lane color, 2px ring of `--bg-0` behind it (so edges
  passing under read cleanly). Selected commit: radius 5px + 1.5px `--accent` outer ring. HEAD
  commit dot: 1.5px `--text-1` outer ring.
- Edge stroke: **2px**, round caps, color = lane color of the edge's lane.
- Fork/merge curve: cubic bézier between (x1, y1) and (x2, y2) of adjacent rows with control
  points `(x1, y1 + rowHeight/2)` and `(x2, y2 − rowHeight/2)` — vertical tangents at both ends,
  GitKraken-style S-curve. Straight vertical segments elsewhere.
- Commit text column starts after `gutter + laneCount*laneWidth + 12px`: message (text-1,
  truncated), then author + relative date (text-3, 12px) as space allows.
- HiDPI: canvas backing store scaled by `devicePixelRatio`; all metrics above are CSS px.

## 5. Lane color palette (deterministic, both themes)

Assigned by `lane % 10`, computed in Rust with the layout; stable while scrolling by construction.

| # | Hex | | # | Hex |
|---|---|---|---|---|
| 0 | `#4f8cff` blue | | 5 | `#3ec6c0` teal |
| 1 | `#f2994a` orange | | 6 | `#e8c341` yellow |
| 2 | `#9b6dff` purple | | 7 | `#f26d9c` pink |
| 3 | `#43b97f` green | | 8 | `#7a86ff` indigo |
| 4 | `#e5534b` red | | 9 | `#8fbf4d` lime |

These hold ≥ 3:1 contrast against both `#16181d` and `#ffffff` at 2px strokes; do not lighten or
darken per theme.

## 6. Ref pills (beside commit message)

Shape: 999px radius, 11px text, 600 weight, padding 2px 8px, max-width 160px with ellipsis,
4px gap between pills. Rendered on the canvas in graph rows.

| Kind | Style |
|---|---|
| Local branch | bg = lane color at 18% alpha; text + 1px border = lane color |
| Current branch (HEAD attached) | solid lane-color bg, `--accent-text` text, prefix `⌂ ` |
| Remote branch | bg `--bg-2`, text `--text-2`, 1px `--border`; label `origin/name` |
| Tag | bg `#d4a72c` at 18% alpha, text + border `#d4a72c`, prefix `# ` |
| HEAD (detached) | solid `--danger` bg, white text, label `HEAD` |

## 7. File status colors (right panel, M1+)

Added/staged `--success`, modified `--warning`, deleted `--danger`, untracked `--text-3` italic,
renamed `--accent`. Letter badge (A/M/D/U/R) in mono 11px before the path.

**Never let color be the only carrier of meaning** — the A/M/D/U/R letter badge is the house
precedent. Every new status indicator pairs its hue with a letter, word, or glyph.

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
  interactive row (e.g. the Settings rail's `repo` scope pill, §12.1), fold its text into the
  control's accessible name via a visually-hidden span — a purely visual pill leaves AT users
  without the qualifier.

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
│  ▌selected       │ pane  role=tabpanel  --bg-0               │
│ ──── divider ─── │  title / subtitle / groups of rows        │
│  Git config[repo]│                                           │
└──────────────────┴───────────────────────────────────────────┘
                          880px
```

- Card: `.dialog-card.settings-card`, `width: 880px; max-width: calc(100vw - 48px);
  height: min(660px, calc(100vh - 64px)); display: grid;
  grid-template-columns: 200px 1fr; grid-template-rows: 48px 1fr; overflow: hidden;`
  `--bg-1`, 1px `--border`, radius 6, over the existing `.dialog-overlay` scrim
  (backdrop-mousedown closes).
- **The card never scrolls as a whole.** The rail and the pane scroll independently; the header and
  the search bar are fixed.
- Header 48px, `padding: 0 8px 0 16px`, title 15px/600 `--text-1`, bottom 1px `--border`.
- Rail `--bg-1`, `padding: 8px`, right 1px `--border`; items 32px tall, `padding: 0 8px`, radius 6,
  13px `--text-2`, 2px gap. Selected: `--selection` bg + `--text-1` + 600 +
  `box-shadow: inset 2px 0 0 var(--accent)` (the accent bar is the shape carrier — `--selection` vs
  `--bg-1` is only ~1.3:1). Hover `--bg-2`; pressed `--bg-3`.
- Rail grouping is done with `aria-hidden` 1px `--border` dividers and a hueless scope pill
  (§11) — **never** with heading elements inside the `role="tablist"`.
- Pane `--bg-0`, `padding: 16px 24px 24px`, own scrollbar; `scrollTop` resets to 0 on category
  change. Pane header: title 15px/600 `--text-1` + subtitle 12px `--text-2`, block margin-bottom 16.
- Group: `margin-bottom: 24px`; group title 11px uppercase, `letter-spacing: .08em`, `--text-3`
  (decorative — it duplicates visible structure).
- **Density-invariant** (§3): one geometry in both `cozy` and `compact`.
- Below 720px wide the rail collapses to a 40px horizontally-scrolling strip above the search bar;
  roles and tab order are unchanged.

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

### 12.3 Control kinds

| The setting is… | Control |
|---|---|
| An independent on/off | **Switch** (§12.3.1) |
| One of 2–3 exclusive values, short self-explanatory labels | **Segmented** (§12.3.2) |
| One of 2+ exclusive values where each needs a sentence | Radio group, stacked, hint under each |
| A bounded number tuned by feel | `NumberSlider` (slider + number + unit) |
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

### 12.4 Keyboard, roles, focus

- `Ctrl/Cmd + ,` opens Settings, registered **above** App's `typing` guard so it fires from the
  commit box. No-op when already open — a shortcut must not toggle a modal.
- Card: `role="dialog" aria-modal="true" aria-labelledby="settings-title"`. Rail:
  `role="tablist" aria-orientation="vertical"`, items `role="tab" aria-selected aria-controls`.
  Pane: `role="tabpanel" tabindex="-1" aria-labelledby="{selected tab}"`.
- Tab order is pure DOM order (close ✕ → search → rail → pane), trapped in the card. The rail is
  **one** tab stop with a roving tabindex (the `AiRunStrip.tsx:54-75` precedent); `↑`/`↓` move and
  activate, `Home`/`End` jump, `→` moves focus into the pane.
- Initial focus: the search input — a text field, so no accidental activation. When opened via a
  deep link (`initialCategory` set) initial focus goes to the **pane** so the deep-linked section's
  own focus effect is not fighting the search box.
- Focus restore: the element active before opening, falling back to the ⚙ trigger.
- `Esc` is layered: it first clears a non-empty search (the `ListFilterInput` capture-phase idiom),
  then closes the dialog. `Enter` never closes — settings apply live and there is no "OK".
- Every hit target ≥24px: rail 32, switch 36×24, segment ≥24, reset 24, close 32 (§3.1).

### 12.5 Settings search

- One model: the query produces a **cross-category result list** of live, editable rows grouped by
  category — not a rail filter and not a within-page filter. The failure mode being solved is "I
  know the setting's name, not its category".
- Matching: synchronous, case-insensitive substring, all whitespace-separated terms required, over
  `label` + `help` + a never-displayed `keywords` string (the `PaletteAction.keywords` idiom). Not
  fuzzy. A row that carries a stateful `.settings-row-note` instead of catalog `help` (§12.2) must
  compensate in `keywords` — that is the only place its vocabulary can live.
- The searchable index is **pure data in its own module** (`settings/settingsIndex.ts`), never
  inlined in a component — the same rule as any large static table.
- Matched substrings in the label are wrapped in `<mark>`: `background: var(--selection);
  color: var(--text-1);` (**9.4:1** dark / **13.3:1** light). Help text is not highlighted.
- The rail shows per-category match counts while searching; zero-count items dim to `opacity: .5`
  but stay clickable (clicking clears the query).
- Result count changes are announced by a visually-hidden `role="status" aria-live="polite"`.

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
- `ContextMenuItem` gained `checked` and `detail`, and `ContextMenuProps` gained `header`, in P69.
  All three are additive: absent ⇒ byte-identical rendering to before.
