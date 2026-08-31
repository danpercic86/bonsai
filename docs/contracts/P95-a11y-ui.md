# P95 — a11y: graph scroller semantics, keyboard reachability, toolbar contrast (UI contract)

Owner: `ui-designer`. Implementer: `senior-dev`. Read-only inputs: `docs/contracts/ui-reference.md`
§2 (tokens + contrast notes), §4.1 (graph keyboard & screen-reader access),
`docs/contracts/P92-multi-ref-commit-ui.md`, `docs/contracts/P93-pr-diff-center-overlay-ui.md`.

Three pre-existing accessibility defects, batched. **No new tokens.** **No new components.**
**No layout, geometry, density, motion, or copy changes** except the two microcopy strings in §1.4.
Because nothing moves and nothing new is drawn, the usual per-surface cozy/compact and dark/light
geometry tables do not apply here — the only theme-dependent content in this contract is the
contrast table in §3, which is given for **both** themes.

`ui-reference.md` §2 and §4.1 were **already updated in this same design pass** to match this
contract; senior-dev does not touch `docs/`.

## 0. Scope verdict (asked explicitly)

- **No split needed. All three items belong in one increment.** Item 1 becomes an attribute
  *removal* plus one role attribute (see §1.1 for why the expensive option is rejected); item 2 is
  one new method on an existing imperative handle plus two call sites and one guard; item 3 is a
  bounded list of CSS token swaps. Total surface: 2 `.tsx` files, 1 `.ts` hook, 7 `.css` files.
- **Nothing in this milestone is not-worth-doing.** All three are outright standards violations
  (invalid ARIA; an unreachable keyboard route; sub-AA control labels).
- **One thing IS deliberately deferred** — see §3.4. The broader "`--text-3` used for *read* text
  that is not an interactive control" sweep (7 selectors across the app) is a genuinely separate
  increment and is filed as a follow-up, not smuggled in here. This contract fixes the
  **interactive-control** class completely.

---

## 1. Graph scroller ARIA semantics

### 1.1 The chosen model, and why

**Chosen: drop the composite-widget ARIA entirely. The scroller is a labelled focusable group;
the already-mounted polite live region is the sole announcement channel.**

`src/graph/GraphCanvas.tsx:830-845` currently declares `role="grid"` + `aria-rowcount` +
`aria-activedescendant="graph-row-{i}"`. All three are invalid together: a `grid` with no
`role="row"` descendant is a malformed widget, `aria-rowcount` is only meaningful on a table/grid,
and `graph-row-{i}` **never exists in the DOM** — the rows are canvas pixels (the scroller's only
child is `.graph-spacer`). A dangling IDREF is worse than no IDREF: the AT reports the active
element as empty, so the user hears nothing on selection change from that channel.

Rejected alternatives, with the reason each loses to the canvas/virtualization constraint:

| Option | Verdict |
|---|---|
| Render one visually-hidden `role="row"`/`gridcell` per **visible** row (≈40 nodes, recycled) | **Rejected.** Reintroduces per-row DOM into the one component whose entire design premise is that there is none, adds ID churn on every scroll tick, and forces an answer to "the active row scrolled out of the rendered window" — at which point `aria-activedescendant` points at a removed node and we are back to a dangling IDREF, only intermittently. It also fights the 20k-row render budget. |
| `role="listbox"` with a single visually-hidden `role="option"` for the selected row, carrying `aria-setsize`/`aria-posinset` | **Rejected.** Valid, but it misreports a 20k-row graph as a one-item listbox, and it **double-announces**: the activedescendant change and `GraphSelectionAnnouncer` would both speak on every arrow press. Two channels for one event is a defect, not redundancy. |
| **Live-region-only (chosen)** | Zero DOM per row, zero ID churn, **the "active row scrolled out of the rendered window" case ceases to exist** (there is nothing to point at, ever), one announcement channel, and it is one of the two fixes `ui-reference.md` §4.1 already sanctioned. The scroller stays exactly as focusable and as keyboard-navigable as it is today. |

`GraphSelectionAnnouncer` is real and **already mounted** — `src/components/WorkspaceGraphPane.tsx:18,228`
renders it with `graph`/`selectedIndex`/`display`, and it announces
`"{summary} — {author}, {date}. Row {n+1} of {N}. {refs}"` through `RevealAnnouncer`
(`role="status" aria-live="polite"`). The row count that `aria-rowcount` was trying to convey is
therefore **already spoken**, in a place the user actually hears it. No change to either component.

### 1.2 Exact markup — `src/graph/GraphCanvas.tsx`, the `.graph-scroll` div (lines ~830-845)

Attributes on the scroller div, after the change — this is the complete, authoritative set:

```
ref, className="graph-scroll", data-testid="graph-scroller"
tabIndex={0}
role="group"
aria-label="Commit graph"
aria-describedby={<id of the hint span, §1.4>}
onScroll onMouseMove onMouseLeave onClick onKeyDown onContextMenu   (all unchanged)
```

**Removed:** `role="grid"`, `aria-rowcount`, `aria-activedescendant`.
**Added:** `role="group"`, `aria-describedby`.
**Unchanged:** `tabIndex={0}`, `aria-label="Commit graph"`, every handler, the `.graph-spacer`
child, and `.graph-scroll:focus-visible` in `src/styles/graph-canvas.css:35`.

Why `role="group"` rather than no role at all: a *focusable generic div* has no accessible-name
computation guaranteed across ATs, so `aria-label` on it is announced inconsistently.
`role="group"` is honest (it is a labelled container, not a widget), gives the label a reliable
home, and — unlike `grid`/`listbox` — imposes **no** required child roles, so it cannot become
invalid again as the graph grows.

No `aria-activedescendant` is set under **any** state, including while a row is selected.
There are no virtualized row IDs; the `graph-row-{i}` ID scheme is deleted, not redefined.

### 1.3 State table for the scroller

| State | Behaviour |
|---|---|
| Default (no focus, no selection) | `role="group"`, label "Commit graph". No ring. Nothing announced. |
| Focused, no selection | `.graph-scroll:focus-visible` ring (2px `--accent`, 1px offset, inset — unchanged). AT reads "Commit graph, group" + the §1.4 hint. |
| Focused, row selected | Same ring. Live region announces the settled selection (existing ~150 ms debounce). |
| Selection changed while scroller **not** focused | Cannot happen after §2 for keyboard nav; a programmatic reveal (e.g. Search → reveal commit) still announces via the same live region. |
| Held arrow key | Live region debounce absorbs it (existing behaviour, unchanged). |
| Empty graph (0 nodes) | `role="group"` + label still present; no selection, nothing announced. Existing empty state in the graph pane is unchanged. |
| Loading / streaming (`totalRows` > loaded rows) | Unchanged; the row count in the announcement comes from the announcer, which already handles the streamed path. |
| `prefers-reduced-motion` | No motion added or removed by this milestone. |

### 1.4 The hint span (new, tiny, no new file)

Screen-reader users get no discoverability for the keyboard model once the widget is a plain group,
so add one visually-hidden description **inside `GraphCanvas.tsx`**, as a sibling of the scroller
inside `.graph-canvas-host`:

```
<span id={hintId} className="sr-only">{HINT}</span>
```

- `hintId`: from React `useId()` — do **not** hand-roll a global constant string; two graph panes
  can be mounted across tabs.
- `HINT` (exact string, sentence case, no jargon):
  **"Use the arrow keys to move between commits. Press the Menu key or Shift+F10 for actions on the selected commit."**
- `.sr-only` already exists in the stylesheet (used by `AiActivityPanel`, `ChecksAnnouncer`,
  `SettingsSearchBar`) — reuse it, do not add a class.
- The second microcopy string in this milestone is unchanged from today: `aria-label="Commit graph"`.

### 1.5 Test fallout senior-dev must expect

Any test asserting the grid semantics will fail and must be updated in the same increment:
`getByRole('grid')` → `getByRole('group', { name: 'Commit graph' })`, and assertions on
`aria-activedescendant` / `aria-rowcount` become assertions that those attributes are **absent**.
Keep `data-testid="graph-scroller"` as the query of choice for non-a11y tests.

---

## 2. Keyboard reachability of the row menu

### 2.1 The intended focus model (the contract between the two handlers)

> **Whenever the window-level keyboard handler consumes a navigation key to change the graph
> selection, it must also move DOM focus to the graph scroller.**

Rationale: the key acted on the graph, so focus belongs on the graph. This preserves the deliberate
M2 behaviour that arrow keys work *without first tabbing to the graph* (moving nav onto the
scroller's own `onKeyDown` would regress that and is therefore rejected), while making
`GraphCanvas`'s `handleKeyDown` — which only ever fires on the focused scroller — actually
reachable. Menu key / Shift+F10 then works after any arrow-key navigation.

The mirror-image rule, and the reason for the new guard in §2.2 item 3:

> **The graph never consumes a navigation key that another widget has already consumed.**

### 2.2 Implementation surface

1. **`src/graph/GraphCanvas.tsx:126-128`** — extend the handle:
   ```
   export interface GraphCanvasHandle {
     getVisibleRowCount(): number;
     /** P95 §2: focus the scroller so the Menu key / Shift+F10 row menu is reachable. */
     focusScroller(): void;
   }
   ```
   Implementation in the existing `useImperativeHandle` (lines ~217-221):
   `focusScroller: () => scrollerRef.current?.focus({ preventScroll: true })`.
   `preventScroll: true` is **required** — without it the browser may scroll the scroller to its
   own idea of the focus target and fight `scrollRowIntoView`.

2. **`src/components/repoWorkspace/useWorkspaceKeyboard.ts`** — call
   `graphRef.current?.focusScroller()` in **exactly** the branches that already call
   `e.preventDefault()` for graph navigation, immediately after the `setSelectedIndex(...)` call:
   - the seed branch, line ~334-336 (first Arrow/Page/Home/End with no prior selection);
   - ArrowDown/ArrowUp, line ~342-347;
   - PageDown/PageUp, line ~353-359;
   - Home/End, line ~365-366.

   Do **not** call it anywhere else. In particular: not on the fetch/pull/push shortcuts
   (lines ~301-317), not on the Escape/clear-selection path (line ~201), and not in any branch that
   returns early without consuming the key. `preventDefault()` is the precise marker of "this key
   was consumed by graph nav" — keep the two in lockstep.

3. **New guard, same file — bail when another widget already consumed the key.** Immediately before
   the seed branch (i.e. before line ~323, after the fetch/pull/push shortcuts), add:
   ```
   // P95 §2.1: another focused widget with its own arrow handling already consumed
   // this key (it called preventDefault without stopPropagation). Do not move the
   // graph selection, and above all do not yank focus out of that widget.
   if (e.defaultPrevented) return;
   ```
   This is **not** hypothetical: `src/components/GitActivityDock.tsx:116-133` moves row focus on
   ArrowUp/ArrowDown with `e.preventDefault()` and **no** `stopPropagation()`, so today the
   window-level handler also fires and silently moves the graph selection in the background. Without
   this guard, P95 would upgrade that latent quirk into a visible regression — arrowing through the
   dock would rip focus back to the graph on every keypress. The same protection covers
   `CommitOptionsMenu` and any future arrow-handling list.

4. The hook's existing guards are the correct gate and must not be relaxed: `typing` (input /
   textarea / select / contentEditable) and the `dialogOpen || abortConfirmOpen || searchOpen ||
   paletteOpen || composerOpen || historySearchOpen` bail-out both run *before* these branches, so
   the graph never steals focus from a dialog, the command palette, the search bar, or the commit
   composer.

### 2.3 Non-regression: the P93 click rule

The P93 rule — *clicking a commit in the graph while a PR overlay is open leaves focus in the graph
scroller* — is **untouched**. That path goes through the scroller's own `onClick`, where the browser
has already put focus on (or inside) the scroller; §2 adds no click-path code. The single risk to
watch is the reverse: §2 must not move focus *out of* an open overlay/dialog or an arrow-handling
widget. It cannot, because of items 3 and 4 above. Both directions are stated as ACs
(AC7, AC8, AC17).

### 2.4 States

| State | Behaviour |
|---|---|
| Arrow key, focus on `<body>` | Selection moves **and** the scroller takes focus; `:focus-visible` ring appears (the interaction was keyboard-driven). |
| Arrow key, focus already on the scroller | Unchanged; `focusScroller()` is a no-op. |
| Arrow key, focus in an input/textarea | `typing` guard returns first: no selection change, no focus move. |
| Arrow key while a dialog / palette / search bar / composer is open | Guard returns first: no focus move. |
| Arrow key while focus is in the git activity dock (or any widget that `preventDefault`s arrows) | New `defaultPrevented` guard returns first: that widget navigates, the graph does not, focus stays put. |
| Menu key / Shift+F10 after arrow nav | Opens the selected row's menu at the §P92 anchor. Previously did nothing. |
| Menu key with no selection | Unchanged: `handleKeyDown` returns (`index === null`). |
| Menu key on a stale index (selection beyond loaded rows) | Unchanged bounds check at `GraphCanvas.tsx:808`. |
| Escape from the opened row menu | Unchanged: `CommitOptionsMenu` restores focus to its trigger; with the keyboard route the trigger is the scroller, so focus returns to the graph. |

---

## 3. `--text-3` on enabled interactive controls

### 3.1 Measured contrast (computed 2026-08-31 from the actual token hex values in `src/styles/tokens-and-base.css`, WCAG 2.x relative-luminance formula)

The reported ≈4.0:1 for `.diff-intra-toggle` is **too optimistic**. The real backdrop is
`.diff-overlay { background: var(--bg-0) }` (`src/styles/diff.css:65-73`) — the overlay is opaque,
not a translucent scrim, so the button's `background: transparent` composites straight onto
`--bg-0`.

Token luminances used throughout: `--text-3` 0.1673 dark / 0.2813 light · `--text-2` 0.4167 dark /
0.0815 light · `--bg-0` 0.00911 dark / 1.0 light · `--bg-1` 0.01435 dark / 0.9297 light ·
`--bg-2` 0.02292 dark / 0.8539 light.

| Effective backdrop | `--text-3` dark | `--text-3` light | `--text-2` dark | `--text-2` light |
|---|---|---|---|---|
| `--bg-0` | **3.68:1** ✗ | **3.17:1** ✗ | **7.90:1** ✓ | **7.99:1** ✓ |
| `--bg-1` | **3.38:1** ✗ | **2.96:1** ✗ | **7.25:1** ✓ | **7.45:1** ✓ |
| `--bg-2` | **2.98:1** ✗✗ | **2.73:1** ✗✗ | **6.40:1** ✓ | **6.87:1** ✓ |
| `--text-3` 18% tint over `--bg-1` (≈`#2b2f36` / `#e3e5e9`) | **2.78:1** ✗✗ | **2.51:1** ✗✗ | **5.97:1** ✓ | **6.32:1** ✓ |

✗ = below the 4.5:1 text floor. ✗✗ = below even the 3:1 non-text/graphics floor, so it fails as an
icon glyph too. `--text-2` clears 4.5:1 on every backdrop in both themes with margin.

The `--bg-0`/`--bg-1`/`--bg-2` `--text-3` rows agree with the figures already in `ui-reference.md`
§2. One figure there was **wrong and has been corrected in this pass**: `--text-2` on light `--bg-0`
is **7.99:1**, not the "4.9:1" previously written (4.9 is what the *dark* `--text-3` hex gives on
white — the wrong hex was used).

### 3.2 The mandatory change list (selector → token)

Every one of these is an **enabled** interactive control (or the sole glyph inside one), so
`--text-3` is disallowed. Change `color: var(--text-3)` → `color: var(--text-2)`; change **nothing
else** — no size, padding, weight, radius, border or hover/active rule moves. All of these already
inherit the global `:focus-visible` ring from `src/styles/tokens-and-base.css:156`, so no
focus styling is added.

| # | File:line | Selector | Backdrop | Before (dark/light) | After |
|---|---|---|---|---|---|
| 1 | `src/styles/split-view.css:104` | `.diff-intra-toggle` (off state) | `--bg-0` | 3.68 / 3.17 | 7.90 / 7.99 |
| 2 | `src/styles/split-view.css:81` | `.diff-view-toggle button` (inactive segment) | `--bg-0` | 3.68 / 3.17 | 7.90 / 7.99 |
| 3 | `src/styles/forge-pr.css:23` | `.right-pane-tab` (inactive tab) | `--bg-1` | 3.38 / 2.96 | 7.25 / 7.45 |
| 4 | `src/styles/diff-content.css:124` | `.diff-hunk-discard-btn` | `--bg-1` (declared on the rule) | 3.38 / 2.96 | 7.25 / 7.45 |
| 5 | `src/styles/tabs.css:75` | `.tab-close` (icon-only) | `--bg-2` on the active tab (worst case) | 2.98 / 2.73 — fails even 3:1 | 6.40 / 6.87 |
| 6 | `src/styles/partial-staging.css:16` | `.diff-gutter-btn` (idle) | tinted `--bg-1` | 3.38 / 2.96 | 7.25 / 7.45 |
| 7 | `src/styles/partial-staging.css:47` | `.diff-gutter-discard-btn` (idle) | tinted `--bg-1` | 3.38 / 2.96 | 7.25 / 7.45 |
| 8 | `src/styles/ai-assets.css:141` | `.asset-chip-missing`, `.asset-chip-muted` label | own 18% `--text-3` tint | 2.78 / 2.51 | 5.97 / 6.32 |
| 9 | `src/styles/checks-panel.css:126` | `.checks-rollup-pill--neutral .checks-rollup-glyph` | `--bg-2` | 2.98 / 2.73 — fails 3:1 | 6.40 / 6.87 |
| 10 | `src/styles/settings-primitives.css:446` | `.identity-swatch-option:hover:not(.is-selected)` — `border-color` | `--bg-1` | 3.38 / 2.96 (graphics bar; light fails) | 7.25 / 7.45 |

Notes on the judgement calls:
- **#6/#7 keep their design intent.** The gutter controls are meant to read as *dim* at idle and
  brighten to `--accent` / `--danger` on row-hover, self-hover and `:focus-visible`. `--text-2` is
  still clearly dimmer than `--text-1` and than the hover colours, so the reveal gradient survives.
  Dark `--text-3` (3.38) technically cleared the 3:1 glyph bar; **light (2.96) did not**, and a
  theme-conditional token for one glyph is not worth a fork — one token, both themes.
- **#8 keeps the tint fill.** Only the label colour changes. The 18% `--text-3` tint stays as the
  chip background; this is the §2 "a hue is never a label colour over its own tint" rule applied to
  the neutral hue.
- **#9 and #10 are graphics, judged at 3:1, and still fail** in at least one theme. They are in the
  mandatory list for that reason, not for the 4.5:1 rule.
- **#5 `.tab-close` is icon-only** and must additionally have an accessible name; if
  `src/components/TabStrip.tsx` does not already give it `aria-label="Close tab"` (or a
  repo-name-qualified equivalent) plus a ≥24px hit target, add both. Verify, don't assume.

### 3.3 Explicitly exempt — do not change these

Disabled-state rules are exempt from WCAG contrast, and dimming *is* the disabled signal. Leave
`--text-3` in place at: `src/styles/controls.css:125` (`.btn-icon:disabled`),
`controls.css:189` (`.toolbar-btn:disabled`), `tabs.css:110` (`.tab-add:disabled`),
`context-menu.css:84` (`.context-menu-item:disabled`), `commit-box.css:112` (`.row-action:disabled`),
`dialogs-forms.css:218,224` (`.combobox-option--disabled`), `search.css:211,216`
(`.command-palette-option.is-disabled`). In every one of these the disabled meaning is also carried
by the `disabled` attribute and `cursor: default` — colour is not the sole carrier.

### 3.4 Deferred to a follow-up (state this in TODO.md; do NOT implement here)

`--text-3` is also used for **read text that is not an interactive control**, which §2 of
`ui-reference.md` already forbids but which predates it. This is a separate, larger sweep and is
out of scope for P95: `diff.css:94` `.diff-overlay-kind`, `diff-browser.css:74` `.diff-tree-count`,
`conflicts.css:211` `.conflict-editor-split-label`, `worktrees.css:13,22` `.wtctx-branch` /
`.wtctx-blocked`, `dialogs-forms.css:236` `.combobox-option-hint`, `search.css:228`
`.command-palette-option-hint`. Recommended follow-up title: **"`--text-3` read-text sweep"**.
The two `*-hint` selectors are the strongest of these (read text inside an *enabled* option) — if
the orchestrator wants one of them pulled forward, take those two and leave the rest.

---

## 4. Acceptance criteria

Harness = verifiable in the mock browser harness (`pnpm dev:mock`, `VITE_MOCK_IPC=1`) via
`javascript_tool` / `read_page`. UC = **USER CHECKPOINT**, native window and/or human perception.

| # | Criterion | Where |
|---|---|---|
| AC1 | `document.querySelector('[data-testid="graph-scroller"]')` has `role="group"`, `aria-label="Commit graph"`, a non-empty `aria-describedby`, `tabindex="0"`, and **no** `role="grid"`, `aria-rowcount` or `aria-activedescendant` attribute. | Harness |
| AC2 | The element referenced by that `aria-describedby` exists in the DOM, is `.sr-only`, and its text equals the §1.4 HINT string verbatim. | Harness |
| AC3 | With two repo tabs open, the two graph scrollers' `aria-describedby` values differ (ids come from `useId`, not a constant). | Harness |
| AC4 | `GraphCanvasHandle` exports `focusScroller(): void`; `tsc` passes; the mock/real IPC boundary is untouched. | Harness (`pnpm tsc`) |
| AC5 | From `document.body`, dispatching ArrowDown at the window changes the selection **and** leaves `document.activeElement` equal to the graph scroller. Same for ArrowUp, PageUp, PageDown, Home, End, and for the first-key seed case with no prior selection. | Harness |
| AC6 | With the scroller focused and a row selected, `ContextMenu` and `Shift+F10` each open the row context menu; its anchor is inside the scroller's bounding box. With no selection, neither key opens anything. | Harness |
| AC7 | While a dialog, the command palette, the search bar, the commit composer, or the Ask-history overlay is open, an arrow key changes neither the selection nor `document.activeElement`. Same while focus is in an `input`/`textarea`. | Harness |
| AC8 | **P93 non-regression:** with a PR overlay open, clicking a commit in the graph leaves `document.activeElement` on the graph scroller. | UC (real canvas click) |
| AC9 | Every selector in the §3.2 table resolves to `color`/`border-color` = the computed `--text-2` value in **both** `data-theme` states; `getComputedStyle` on a rendered instance of each confirms it. No hardcoded hex is introduced. | Harness |
| AC10 | No selector listed in §3.3 changed; a grep for `var(--text-3)` in `src/styles/` returns exactly the §3.3 exempt set plus the §3.4 deferred set plus non-colour uses. | Harness (grep) |
| AC11 | `.tab-close` has an accessible name and a ≥24px hit target. | Harness |
| AC12 | Nothing in §3 changed any size, padding, weight, radius, border-width, hover or active rule; the visual diff is colour-only. | Harness (one screenshot pair, dark + light) |
| AC13 | `ui-reference.md` (already updated by `ui-designer` in the design pass) is consistent with the shipped code: no `role="grid"` mandate remains in §4.1, the old "known gap"/"known defect" paragraphs are gone, and §2 reads **7.99:1** for `--text-2` on light `--bg-0`. Verification only — senior-dev does not edit `docs/`. | Harness (file read) |
| AC14 | A screen reader (NVDA on Windows / VoiceOver on macOS) announces "Commit graph, group" + the hint on focusing the graph, and announces exactly **one** utterance per arrow press — no dangling/empty active-element announcement, no double-speak. | **UC** |
| AC15 | The canvas selection ring visibly follows keyboard navigation and the focus ring does not fight `scrollRowIntoView` (no scroll jump on `focusScroller`). | **UC** (canvas repaint needs rAF) |
| AC16 | The gutter controls (#6/#7) still read as dim-at-idle and clearly brighten on hover — the reveal affordance survives `--text-2`. | **UC** (perceptual) |
| AC17 | With focus on a git-activity-dock run row, ArrowUp/ArrowDown move focus **within the dock only**: the graph `selectedIndex` is unchanged and `document.activeElement` stays inside the dock. (This is a *new* behaviour — today the graph selection also moves silently.) | Harness |

Harness-visible: AC1-AC7, AC9-AC13, AC17. USER CHECKPOINT: AC8, AC14, AC15, AC16.

### 4.1 Harness fixture states needed

No new fixtures. The existing mock states already cover everything: a loaded graph (AC1-AC7),
a diff overlay with the intraline toggle (AC9 #1/#2), the right-pane PR tabs (#3), an unstaged
tracked diff with the discardable gutter (#4/#6/#7), two repo tabs (AC3), a neutral CI rollup (#9),
a muted AI asset chip (#8), and a git activity dock with ≥2 runs (AC17). If any of #8/#9/#10 has no
reachable mock state, note it and verify that selector by injecting the class onto a probe element
via `javascript_tool` — a computed-style check does not need the real feature mounted.

---

## 5. `ui-reference.md` edits — already applied in this pass

Applied by `ui-designer` at contract time, so the reference never contradicts the contract:

1. **§2 contrast notes** — corrected `--text-2` on light `--bg-0` to **7.99:1**; extended the
   `--text-3` figures with the `--bg-2` and own-tint rows; added the governing rule:
   > `--text-3` is **never** the label or glyph colour of an **enabled** interactive control.
   > Disabled states are exempt (dimming is the disabled signal, and `disabled` carries the meaning
   > independently). Icon-only glyphs must clear **3:1** against their *actual composited* backdrop
   > in **both** themes — `--text-3` clears it on none of `--bg-0`/`--bg-1`/`--bg-2` in light, so in
   > practice use `--text-2`.
2. **§4.1** — replaced the `role="grid"` / `aria-rowcount` / `aria-activedescendant` mandate with the
   §1.2 attribute set, documented the hint span, stated the focus-follows-consumption rule and the
   `defaultPrevented` guard from §2, and **deleted** both the "Known gap" and "Known defect"
   paragraphs, which this milestone closes.

## 6. Flagged ambiguities (orchestrator decision)

- **A. Scope of §3.4.** Recommendation: defer the whole read-text sweep as its own increment.
  Alternative: pull `.combobox-option-hint` + `.command-palette-option-hint` into P95 (2 extra lines,
  they are the worst of the set). My recommendation is to defer all six and keep P95 clean.
- **B. `role="group"` vs no role** on the scroller. Recommendation: `role="group"` (§1.2 reasoning).
  If a reviewer objects that a focusable `group` is unusual, the fallback is `role="region"` +
  `aria-label` — also valid, but it adds a landmark to the landmarks list, which is noisier. I do
  not recommend it.
- **C. AC17 changes existing behaviour.** The `defaultPrevented` guard also fixes the pre-existing
  silent double-move in the git activity dock. I consider that a strict improvement and have specced
  it; if the orchestrator wants dock-arrow-also-moves-the-graph preserved, say so and I will scope
  the guard to focus-containment instead. Recommendation: keep the guard as specced.
