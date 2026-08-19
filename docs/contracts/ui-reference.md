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

**Contrast notes (measured 2026-08-17, P68e design pass).** Two known AA shortfalls; both are
pre-existing and app-wide, so fixing them is its own milestone — but **new surfaces must not add
to them**:

- `--text-3` is **3.38:1** on `--bg-1` and **3.67:1** on `--bg-0` (dark), **2.96:1** on `--bg-1`
  (light). That is below the 4.5:1 AA bar for text. Treat `--text-3` as **decorative only**
  (uppercase section labels that duplicate visible structure, dividers, disabled glyphs). Any text
  the user must actually read — metadata, timestamps, costs, log lines, hints, **status-pill
  labels** — uses `--text-2` (**7.9:1** dark / **4.9:1** light on `--bg-0`; **7.3:1** / **7.4:1**
  on `--bg-1`).
- `--warning` as *text* over its own 14% tint is **3.47:1** in light theme (`.toast-warning`,
  `styles.css:623`). Use `--warning` for borders, glyphs and fills (≥3:1 graphics bar) and
  `--text-1` for the words beside them. For a filled warning chip, `color: var(--bg-0)` on
  `background: var(--warning)` is safe in both themes (**6.4:1** dark / **4.8:1** light).
- **That rule generalises to every hue-as-text-over-its-own-tint pair** (measured 2026-08-19, P73
  pass): `--danger` on its 14% tint over `--bg-2` (`.toast-error`) is **3.34:1** dark / **3.49:1**
  light; `--success` on the same recipe (`.toast-success`) is **4.07:1** dark; `--success` on a 12%
  tint over `--bg-1` (`.submodule-badge-ok`) is **4.76:1** dark / **4.06:1** light; `--warning` on
  that recipe (`.submodule-badge-warn`) is ≈**5.4:1** / **3.94:1**. All pre-existing and app-wide.
  **New surfaces use the §11 pill recipe instead.** Retro-fitting the three toast tones is a pending
  pass of its own (`P73-submodule-reconnect-ui.md` §7.3 OPT-2).

Additional measured pairs (2026-08-19, P70 pass), all on `--bg-1`: `--text-1` **13.5:1** dark /
**15.4:1** light; `--warning` glyph **7.3:1** / **4.5:1**; `--success` glyph **5.7:1** / **4.7:1**;
`--danger` glyph **4.4:1** / **4.6:1** — all clear the 3:1 graphics bar in both themes. And
(2026-08-19, P73 pass) `--text-2` over its **own** 12% tint on `--bg-1`: **5.79:1** dark /
**6.22:1** light — the safe recipe for a hueless informational pill (§11).

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
- Interactive controls are **≥24px** tall in every density (AA hit target).

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
  failing action N times never stacks N identical alerts (§10.1 mechanism).
- Buttons: primary (accent), secondary (bg-2 + border), icon (transparent, bg-2 on hover); all
  32px tall, 6px radius (dock-density controls may go to 28/24px — §3).
- **Dialog body text (P68g).** Primary sentences: `.dialog-body`, 13px `--text-1`. Must-read
  secondary lines — consent facts, spend and destructive consequences, "written without review"
  caveats — use `.dialog-body-detail`, 12px `--text-2`. `.dialog-body-note` is `--text-3` and is
  for genuinely decorative lines only (`+N more`); never put a consequence on it.

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
  closes P68e §12-F3 for the header sweep. `skeleton-pulse` remains outstanding.

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
  `--text-2`** (§2's `--warning`-as-text rule). One rule set serves both themes; **no new token**.
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

## 11. Status pills (rows and chrome)

The canonical recipe, first shipped as the AI dock's status pills (§9) and applied to the sidebar
row badges (`.submodule-badge-*`, shared by submodule and worktree rows — `Sidebar.tsx`).

- **Shape.** 11px, `padding: 1px 6px`, `border-radius: 8px`, `flex: none`, inherited UI font
  (not mono — mono is for hashes and paths). Never compresses; the row's *name* is what ellipsizes.
- **Label.** A **word**, lowercase inside a row pill (`up to date`, `out of sync`, `modified`,
  `not checked out`) and sentence-case in chrome (`Running`, `Failed`). Colour is never the sole
  carrier (§7).
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
- **Title attribute.** A pill's `title` explains *why* the state holds and what fixes it; it must
  never merely repeat the visible label. Keep the *why* out of the visible row text.
- **Busy pill.** While an op runs on that row, the pill's label becomes the present participle
  (`checking out…`, `updating…`) in the hueless style and the row gets `aria-busy="true"` (§8). The
  pill drops its `title` entirely while busy — the participle is the whole message.
- **Density-invariant** — pills live in the sidebar and chrome, which have one geometry (§3).
