# P67 — UX polish batch: always-visible HEAD guideline · right-panel density

Two user-reported items from real use (board: `TODO.md` § "🐛 USER-REPORTED BATCH (2026-08-17)",
items 1 & 2). Item 1 is a confirmed rendering bug; item 2 is a space-reclaim + a new density
preference. Both are frontend-only except one additive settings field.

**Command count: 157 → 157 (UNCHANGED).** `panelDensity` rides the existing `set_ui_settings`
partial patch — no new command, no new event, no new channel, no new `AppError`.

References read (verified, not guessed):
`src/graph/draw.ts` (`WipSummary` L172-174, `drawWipRow` L179-239 — connector L200-212, line start
`y = RH/2 - vp.scrollTop` L193, `clampedY` L202, dashed marker circle L214-223, label+count
L225-238), `src/graph/GraphCanvas.tsx` (`paintNow` L272-357, `layoutScrollTop` L303, `wipOffset`
L302, the gate `wip !== null && scrollTop < rowHeight + 56` L345, `drawWipRow` call L346-354,
`window.__bonsai` bag L562-572, `hitTestAtMouseY` L575), `src/graph/viewport.ts` (96 lines —
`visibleRowRange` L25-40, `scrollRowIntoView` L46-59, `spacerHeight` L93-95),
`src/graph/metrics.ts` (`METRICS` L3-56 with `rowHeight: 32`, `avatarRadius: 10`,
`avatarBgRingExtra: 2`; `COMPACT` L101-111; `effectiveMetrics` L119-150),
`src/graph/frameStats.ts` (`P7DevHooks` L74-96, `BonsaiDevHooks` L106-117, the `Window` global
L119-123), `src/graph/selfTest.ts` (201 lines, `runP7SelfTest`),
`src/components/WorkspaceRightPanel.tsx` (386 lines — `<aside className="right-panel">` L200, tab
strip L201-220, always-mounted `.right-panel-work` L221, `StashSplitButton` L299-314, inline amend
block L315-335, `<CommitBox key={amend ? …}>` L336-372),
`src/components/CommitBox.tsx` (379 lines — `CommitBoxHandle` L57-59, `showSign` L117,
`signChecked` L118, `signFormatLabel` L120, header L196-233, textarea L234-252 with
`rows={merge ? 5 : 3}`, counter L253-261, sign row L262-291, skip row L294-309, error banner
L310-333, buttons L334-362), `src/components/StashSplitButton.tsx` (111 lines — menu + outside-click
+ Escape, scope enable rules L54-62), `src/components/StatusPanel.tsx` (700 lines — `FileRow` L44,
`Section` L174, `CONFLICT_KIND_LABELS` L329, `ConflictRow` L339, `ConflictsSection` L436),
`src/styles.css` (6811 lines — `.right-panel` L2011, `.right-panel-work` L2022 + `[hidden]` L2030,
`.right-panel-work > .status-panel` L2034 (the ONLY flexible child), `.section-label` L2041,
`.status-panel` L2181 gap 16, `.status-section` L2192 padding `6px 8px 8px`, `.file-list` L2217,
`.file-row` L2223 height 24, `.tree-dir-row` L2270 height 24, `.section-header` L2373,
`.amend-affordance` L2446, `.stash-split` L2480-2547, `.commit-box` L2549 pad 12 gap 8,
`.commit-message` L2559 min 60 / max 120 / `resize: vertical`, `.commit-sign-row` L2595,
`.commit-skip-row` L2633, `.commit-button` L2659, `.commit-box-header` L2679, `.op-banner` L5176
pad `8px 12px`, `.right-pane-tabs` L6297 pad `6px 8px`, `.right-pane-tab` L6305 pad `5px 12px`),
`src/components/SettingsPanel.tsx` (Appearance block L298-317, `SettingsGraphSection` L296),
`src/components/SettingsGraphSection.tsx` (whole-struct patch idiom L28),
`src/ipc/types.ts` (`ListView` L1035, `UiSettings` L1361-1389, `UiSettingsPatch` L1391-1417),
`src/App.tsx` (`listView` state L99, `toggleListView` L364-370, `handleSettingsChange` L375-391,
settings load L730-741, `<RepoWorkspace>` L1003-1006, `<SettingsPanel>` L1056),
`src/components/RepoWorkspace.tsx` (`listView` prop L115, destructure L151,
`<WorkspaceRightPanel>` L2741, `listView={listView}` L2765),
`src/ipc/mock/persistence.ts` (`DEFAULT_UI_SETTINGS` L76-100, tolerant `readUiSettings` L200-338),
`src/ipc/mock/handlers/session.ts` (`setUiSettings` merge L69-99),
`src-tauri/src/settings.rs` (`ListView` L64-72, version-bump bar L276-291, `Settings` L292-368,
`Default` L370-399, `list_view_roundtrips_both_variants` L732-758,
`old_settings_file_without_list_view_loads_default` L842-861),
`src-tauri/src/commands/ui_settings.rs` (`UiSettings` L9-41, `UiSettingsPatch` L43-79,
`apply_patch` L85-134, `get_ui_settings` builder L144-161, `set_ui_settings` builder L182-199),
`src-tauri/src/commands/shared.rs` (settings re-export L96-100),
`src-tauri/src/commands/tests.rs` (`set_ui_settings_patch_is_partial` L1000),
`src/components/CommitBox.test.tsx` (role+name queries L93, L104),
`src/components/SettingsPanel.test.tsx` (appearance test L92-98).
House format: `docs/contracts/{P60-parity-batch,P51-graph-polish,P59-hooks-and-lease-hardening}.md`.

---

## 0. Key decisions (with rationale)

**D1 — The connector is SPLIT OUT of `drawWipRow`, not un-gated.** The gate at
`GraphCanvas.tsx:345` (`wip !== null && scrollTop < rowHeight + 56` ⇒ 88 px with the default
`rowHeight: 32`) asks whether the **WIP row** is near the viewport top; the user's question is
whether **HEAD's row** is reachable. Deleting the gate alone regresses three ways: (a) the line's
start `y = RH/2 - vp.scrollTop` (`draw.ts:193`) is **unclamped** — only the end is clamped
(`:202`) — so at 50 000 px of scroll the dash rasterizer walks ≈16 000 dash segments per frame,
every frame; (b) clamping the start without a compensating `lineDashOffset` makes the dashes
visibly crawl while scrolling, because the pattern is then anchored to a viewport-fixed point
instead of to content; (c) the marker circle, the hover background and the
"Uncommitted changes (n)" label legitimately belong to the WIP row and must keep scrolling away.
So: the connector becomes `drawHeadGuide` + `drawHeadEdgeMarker`, driven by a new pure
`headGuide()`; everything else in `drawWipRow` keeps the existing near-top gate verbatim.

**D2 — ALL guideline arithmetic lives in `src/graph/viewport.ts`; `draw.ts` only strokes.**
`viewport.test.ts` already exists (198 lines) and tests pure scroll math with no canvas mock.
`draw.ts` has **no test file and must not grow one** — a canvas-mock suite there would assert
paint calls, not behaviour, and would rot on every visual tweak. A reviewer must **not** file
"draw.ts is untested" as a MUST-FIX; the testable surface is `headGuide()`.

**D3 — An off-screen edge marker is part of the feature, not garnish.** A dashed line pointing at
something outside the viewport is meaningless. When HEAD's row is above/below the viewport, a small
filled triangle in the HEAD lane colour is drawn at the corresponding edge, in the same lane `x`.
`edge` is derived from the **unclamped** HEAD centre so it can never disagree with reality.

**D4 — Amend stays owned by `WorkspaceRightPanel`.** `WorkspaceRightPanel.tsx:337-343` keys
`<CommitBox>` on `amend`, so toggling amend **remounts CommitBox**. A checkbox living inside that
subtree would lose keyboard focus on every toggle. The merged actions row therefore sits *above*
the commit box (where the two separate rows are today), not inside it.

**D5 — Density is delivered in two stages: the tighter default (P67b) is CSS-only, the toggle
(P67c) is purely additive.** P67b converts every literal in the §3 table to
`var(--rp-*, <today's value>)` **and** moves the cozy values into `.right-panel`'s custom
properties. P67c then adds exactly one `[data-density='compact']` override block plus the settings
plumbing. This means the tighter default can be judged (and reverted) on its own, and the toggle
increment touches no layout rule.

**D6 — `panelDensity: 'cozy' | 'compact'` is a top-level enum on `UiSettings`, not a boolean and
not nested in `GraphPrefs`.** An enum matches the `ListView` precedent (label-bearing
`settings-toggle-btn` in Settings → Appearance), leaves room for a third value, and the distinct
name avoids colliding with the existing `graph.compact`. It is **independent** of `graph.compact`
(a cross-reference hint in Settings only, no master switch) because one is right-panel chrome and
the other is canvas row geometry — a user who wants dense file lists does not necessarily want
22 px graph rows. It does **not** nest in `GraphPrefs` because it has nothing to do with the graph
and `clamp_graph_prefs` must not touch it.

**D7 — `data-density` is a PROP on the `<aside>`, not `documentElement.dataset`.** A prop is
unit-testable by `render()`, keeps the CSS cascade scoped to the panel, and cannot leak into the
diff overlay or sidebar.

**D8 — Every converted CSS declaration uses the fallback form `var(--x, <today's value>)`.** Hard
rule, and the reason is concrete: `.file-row`, `.file-list` and `.tree-dir-row` are **also**
rendered outside `.right-panel` by `src/components/DiffFileTree.tsx:71,121` (inside
DiffBrowser/DiffOverlay), and `.section-label` by `Sidebar.tsx:114`, `EmptyState.tsx:73`,
`OnboardingSteps.tsx:87`. Those elements never inherit a custom property scoped to
`.right-panel`, so the fallback is what keeps them **pixel-identical** to today. Conversely
`.status-section` rendered from `CommitPanel.tsx:214` / `ComparePanel.tsx:95` **is** inside
`.right-panel`, so commit-details and compare correctly inherit the density — that is intended.

**D9 — Auto-growing textarea is progressive enhancement, zero JS.** `field-sizing: content` plus
`rows={1}` plus `min-height`/`max-height` vars, keeping `resize: vertical`. WebView2 is evergreen
Chromium so the native app gets it; a browser without support falls back to the `rows` attribute
and looks like today. No `scrollHeight` measuring effect, no ResizeObserver.

---

## 1. Item 1 — the dashed HEAD guideline (P67a)

### 1.1 `src/graph/viewport.ts` — the new pure function (ALL the arithmetic)

```ts
/** P67 §1: geometry for the dashed HEAD guideline + its off-screen marker.
 *  All values are viewport CSS px (y grows downward, 0 = top of the canvas). */
export interface HeadGuide {
  /** Echo of the resolved (non-null) HEAD row index — lets the draw layer index
   *  `layout.nodes` for the lane without a non-null assertion. */
  headIndex: number;
  /** Anchor end of the segment (WIP dot centre, or just above the top edge on a
   *  clean tree), clamped to [-PAD, viewportHeight + PAD]. */
  y0: number;
  /** Target end, stopped short of the HEAD avatar's halo, same clamp. */
  y1: number;
  /** `lineDashOffset` that keeps the 3/3 dash phase anchored to CONTENT, so the
   *  dashes do not crawl while scrolling. */
  dashOffset: number;
  /** Which viewport edge HEAD is beyond, from the UNCLAMPED centre. null when
   *  HEAD's centre is on screen. */
  edge: 'top' | 'bottom' | null;
  /** A5: false when the segment collapsed below 1 px (both ends clamped to the
   *  same edge). The caller then draws ONLY the edge marker. Never both-false
   *  with `edge === null` — that returns `null` instead. */
  segment: boolean;
}

/** P67 §1: pure geometry for the always-visible HEAD guideline. Returns `null`
 *  when there is nothing meaningful to draw:
 *   - `headIndex === null` (unknown HEAD — notably the streamed-graph window
 *     before HEAD's chunk arrives; this also suppresses the edge marker, so the
 *     UI never claims "HEAD is below" while it does not know);
 *   - the segment collapses below 1 px AND HEAD's centre is on screen (its halo
 *     already covers the anchor, so there is nothing to point at). A collapsed
 *     segment with HEAD off-screen still returns a value — see A5 / §1.1a.
 *
 *  `layoutScrollTop` is the WIP-shifted scroll position (`visibleRowRange`'s
 *  third return value) — the same value passed to `drawGraph` as
 *  `Viewport.scrollTop`, so row centres agree by construction.
 *
 *  Anchor: the WIP dot centre when `wipOffset === 1`, else `-PAD`. The `-PAD`
 *  fallback is what makes the guide work on a CLEAN tree (`wip === null`), where
 *  today nothing at all is drawn even though the user still wants to see where
 *  HEAD is.
 *
 *  Both ends are clamped (today only the target is — see contract D1) so the
 *  stroked path length is bounded by `viewportHeight + 2*PAD` no matter how far
 *  the user has scrolled; `dashOffset` compensates the clamp so the dash phase
 *  stays content-stable. */
export function headGuide(a: {
  headIndex: number | null;
  layoutScrollTop: number;
  /** 1 when a WIP row is synthesised, else 0. */
  wipOffset: number;
  rowHeight: number;
  /** `EffectiveMetrics.avatarRadius`. */
  avatarRadius: number;
  /** `EffectiveMetrics.avatarBgRingExtra` — the bg0 halo behind the avatar. */
  ringExtra: number;
  viewportHeight: number;
}): HeadGuide | null;

/** P67 §1: breathing room kept beyond each viewport edge when clamping the
 *  guideline. Replaces the old ad-hoc 56 (which only existed to accommodate the
 *  UNCLAMPED start). Exported so the tests and the self-test assert the bound. */
export const HEAD_GUIDE_PAD = 8;
```

Algorithm (normative — implement verbatim):

```
PAD  = HEAD_GUIDE_PAD                       // 8
DASH = 6                                    // period of the [3,3] dash pattern

if headIndex === null            -> return null

rawScrollTop = layoutScrollTop + wipOffset * rowHeight
anchor       = wipOffset === 1 ? rowHeight / 2 - rawScrollTop : -PAD
headCenter   = headIndex * rowHeight + rowHeight / 2 - layoutScrollTop
halo         = avatarRadius + ringExtra

dir = Math.sign(headCenter - anchor)
if dir === 0                     -> return null          // degenerate
target = headCenter - dir * halo                         // stop AT the halo, not over it

LO = -PAD ; HI = viewportHeight + PAD
y0 = clamp(anchor, LO, HI)
y1 = clamp(target, LO, HI)
dashOffset = ((y0 - anchor) % DASH + DASH) % DASH         // true modulo, never negative
                                                          // NOTE the sign — see A6
edge = headCenter < 0 ? 'top' : headCenter > viewportHeight ? 'bottom' : null

// AMENDMENT A5 (orchestrator, 2026-08-17): the collapse check suppresses only the
// SEGMENT, never the edge marker. Ordering matters — `edge` must be computed BEFORE
// this test. See §1.1a for why.
collapsed = Math.abs(y1 - y0) < 1
if collapsed && edge === null     -> return null           // nothing meaningful to draw

return { headIndex, y0, y1, dashOffset, edge, segment: !collapsed }
```

Notes the implementer must not "simplify" away:
- `rawScrollTop` reconstruction is exact: `visibleRowRange` defines
  `layoutScrollTop = scrollTop - wipOffset * rowHeight`, so at raw `scrollTop = 0` with a WIP row,
  `anchor = rowHeight/2` — **identical** to today's `RH/2 - vp.scrollTop` (`draw.ts:193`).
- `dir` generalises both directions: HEAD below the anchor (the normal case) and HEAD above it
  (possible on a clean tree scrolled past HEAD, where `anchor = -PAD`).
- `Math.sign` returns `-0` for `-0`; the `=== 0` check catches it (`-0 === 0` is true).
- The modulo must be the wrapped form; `%` alone yields negatives for downward scroll and
  `lineDashOffset` would jitter.

### 1.1a AMENDMENT A5 — the collapse check must not swallow the edge marker

**Author:** orchestrator, 2026-08-17, after the P67a implementation pass surfaced it.
**Status:** binding; supersedes the original §1.1 ordering.

The first-cut algorithm ran `if |y1-y0| < 1 -> return null` *before* computing `edge`. That is a
hole, and precisely the one the user reported, in its mirror image:

Because a WIP row always sits **above** HEAD, once HEAD scrolls off the **top** of the viewport the
anchor is above the top edge too. Both ends then clamp to `-PAD`, the segment collapses, and the
function returned `null` — so with uncommitted changes present (`wipOffset === 1`), `edge: 'top'`
was **unreachable** and Bonsai drew *neither* the line *nor* the up-marker. The guideline would have
worked while scrolling toward HEAD and then silently vanished the moment you scrolled past it, which
fails the locked user decision ("always visible while scrolling"). It also made `edge: 'top'`
reachable only on a clean tree with HEAD's centre inside a ~11 px band — a tell-tale sign the
branch was vestigial rather than intended.

**Rule:** compute `edge` first; let the collapse test suppress only the *segment*. A collapsed
segment with `edge !== null` is the informative case — the whole point is the marker saying
"HEAD is that way". Return `null` only when the segment collapsed *and* HEAD is on screen (its halo
genuinely covers the anchor, so there is nothing left to point at).

**Consequences for the draw layer:** `drawHeadGuide` must no-op when `segment === false` and
`drawHeadEdgeMarker` must still run whenever `edge !== null`. This keeps D2 intact — the decision is
a boolean computed in `viewport.ts`, not arithmetic in `draw.ts`.

**Required test coverage** (replaces the P67a test that pinned the old behaviour):
- WIP row present, scrolled *below* HEAD → non-null, `segment === false`, `edge === 'top'`.
- WIP row present, HEAD on screen with its halo over the anchor → `null`.
- The clean-tree `edge: 'top'` case keeps its existing assertion **plus `segment === true`.**
  *(Corrected 2026-08-17 after implementation measured it: at `wipOffset: 0, headIndex: 0,
  layoutScrollTop: 26, rowHeight: 32` HEAD's centre is `-10`, so `dir = -1` puts the target one halo
  BELOW the centre at `y1 = +2` while the anchor clamps to `y0 = -8` — a real 10 px run, not a
  collapse. The `segment === false` path is pinned by the two WIP-row cases above instead.)*

### 1.1b AMENDMENT A6 — `dashOffset` sign, and why `dir === 0` must go

**Author:** orchestrator, 2026-08-17, from the P67a review. **Status:** binding.

**A6.1 — the sign was inverted in the original §1.1 block.** Canvas semantics: with
`lineDashOffset = off`, the pattern at path distance `s` behaves as if at `s + off`, so dash-on runs
begin where `s ≡ -off (mod DASH)`. The path is stroked from `y0`, so on screen the dash grid sits at
`y ≡ y0 - off`. Content-anchoring requires that grid at `y ≡ anchor` (which is what the pre-P67 code
got for free, because its start was unclamped and `y0 === anchor`). Therefore:

```
off ≡ y0 - anchor   (mod DASH)        ✔ correct
off ≡ anchor - y0   (mod DASH)        ✘ original spec — inverted
```

The error is invisible whenever `(y0 - anchor) ≡ 0 or 3 (mod 6)` and otherwise wrong by
`-2·(y0 - anchor)`, which **varies with scroll** — i.e. it produces exactly the ~1 px-per-px dash
crawl that D1(b) exists to prevent, and it is a *regression* against pre-P67 behaviour on that axis.
Worked instance (the §6 acceptance-(3) tuple, `headIndex 2000, layoutScrollTop 50_000,
rowHeight 32, h 640`): `anchor = -50016 ≡ 0 (mod 6)`, `y0 = -8`; correct `off = 4` puts the grid at
`≡ 0` = the content grid, while the inverted `off = 2` puts it at `≡ 2`.

Reachability note (why no existing test caught it): with `wipOffset === 0` the anchor is `-PAD` and
never clamps, so `dashOffset` is always 0; with `wipOffset === 1`, `dir` is always `+1` because
`headCenter - anchor = (headIndex + 1) · rowHeight > 0`. So **every reachable non-zero `dashOffset`
had the wrong sign.**

**A6.2 — the crawl guard must assert phase, not just periodicity.** A guard that only checks
6-periodicity and non-negativity is satisfied by either sign. It must pin the phase, e.g.
`mod6(y0 - dashOffset) === mod6(anchor)` swept across consecutive 1 px scroll positions.

**A6.3 — delete the `if (dir === 0) return null` early return.** It is A5's hole in miniature: it
runs before `edge`, so on a clean tree at the exact scroll where `headCenter === -PAD` it suppresses
a marker that should be drawn (`edge === 'top'`). It is also redundant — `dir === 0` implies
`target === headCenter === anchor` implies `y1 === y0` implies `collapsed`, which the post-A5 test
already handles correctly as marker-only. Removing it **leaves `dir === 0` reachable** — on a clean
tree at `layoutScrollTop === rowHeight/2 + PAD`, where `headCenter` lands exactly on the `-PAD`
anchor — and that is the point: the collapse test then yields the correct marker-only result instead
of suppressing the marker. Add a test for `dir === 0`; nothing currently covers it.

> ⚠️ Do **not** "simplify" the collapse handling on the assumption that `dir` is always `±1`. It is
> not, and an earlier draft of this amendment wrongly said so. `dir === 0` is a live, tested path;
> collapsing it back into an early return reintroduces the suppressed-marker bug.

### 1.2 `src/graph/draw.ts` — two stroke-only functions

```ts
/** P67 §1: the dashed guideline from the WIP dot (or the top edge on a clean
 *  tree) to the checked-out commit. MOVED OUT of `drawWipRow` (P1 §9.3, now
 *  superseded) so it paints at EVERY scroll position. Contains no scroll, row,
 *  clamp or dash-phase arithmetic — `headGuide()` in viewport.ts owns all of it
 *  (contract D2); the only expressions here are the existing `laneX(lane, m)`
 *  helper and the `lane % 10` palette index, both moved verbatim from
 *  `drawWipRow`. Call AFTER `drawGraph` and BEFORE `drawWipRow`. */
export function drawHeadGuide(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  guide: HeadGuide,
  theme: Theme,
  m: EffectiveMetrics,
): void;

/** P67 §1 (D3): small filled triangle in the HEAD lane colour at the top or
 *  bottom viewport edge, pointing the way to an off-screen HEAD row. No-op when
 *  `guide.edge === null`. Same lane `x` as the guideline, so the two read as one
 *  pointer. */
export function drawHeadEdgeMarker(
  ctx: CanvasRenderingContext2D,
  layout: GraphLayout,
  guide: HeadGuide,
  viewportHeight: number,
  theme: Theme,
  m: EffectiveMetrics,
): void;

/** P67 §1: edge-marker geometry (CSS px). Local paint constants only. */
const HEAD_EDGE_MARKER = { halfWidth: 5, height: 6, inset: 2 } as const;
```

`drawHeadGuide` body shape (no other logic):
`lane = layout.nodes[guide.headIndex]?.lane ?? 0` → `x = laneX(lane, m)` → `ctx.save()`,
`setLineDash([3, 3])`, `lineDashOffset = guide.dashOffset`, `lineWidth = 2`,
`strokeStyle = theme.laneColors[lane % 10]`, `beginPath()`, `moveTo(x, guide.y0)`,
`lineTo(x, guide.y1)`, `stroke()`, `ctx.restore()`.

`drawHeadEdgeMarker` body shape: `edge === null` → return; same lane/x/colour resolution; tip y is
`HEAD_EDGE_MARKER.inset` for `'top'` or `viewportHeight - HEAD_EDGE_MARKER.inset` for `'bottom'`;
fill a 3-point path (tip + two base corners offset by `halfWidth`/`height` in the pointing
direction) with the lane colour; `setLineDash([])` is never needed (fill only).

**Deletions in `drawWipRow`:** remove exactly `draw.ts:200-212` (the `if (headIndex !== null) { … }`
connector block). `headIndex`/`headLane`/`x` at `:190-192` stay (the marker circle and hover
background still need `x`); the local `layoutScrollTop` at `:189` becomes unused **only if** nothing
else reads it — check and delete if so, otherwise leave. Everything from `:214` down is untouched,
including the near-top gate at the call site.

### 1.3 `GraphCanvas.tsx` — call order (replaces L345-355)

```
drawGraph(…)                                     // unchanged, L335-344
const guide = headGuide({
  headIndex: lay.headIndex,
  layoutScrollTop,                               // already in scope, L303
  wipOffset,                                     // already in scope, L302
  rowHeight,                                     // already in scope, L286
  avatarRadius: m.avatarRadius,
  ringExtra: m.avatarBgRingExtra,
  viewportHeight: h,
});
if (guide !== null) {
  drawHeadGuide(ctx, lay, guide, themeRef.current, m);
  if (guide.edge !== null) drawHeadEdgeMarker(ctx, lay, guide, h, themeRef.current, m);
}
if (wip !== null && scrollTop < rowHeight + 56) { drawWipRow(…) }   // UNCHANGED gate
```

The guide is drawn **before** `drawWipRow` so the dashed WIP marker circle paints on top of the
dash (today's z-order for that overlap is preserved). `handleScroll` (`:678-693`) already repaints
on every scroll — nothing else changes. Cost per frame: one stroke of at most
`viewportHeight + 2*PAD` px (≤ 16 dash segments per 100 px of height), plus at most one 3-point
fill.

### 1.4 Streamed-graph transient (documented, bounded)

`layout.headIndex` originates in Rust (`crates/bonsai-core/src/graph.rs:103`, computed `:457`,
streamed `graph/stream.rs:99,:203`) and is re-resolved per chunk on the streamed path by
`resolveHead()` (`src/graph/streamAssembler.ts:44-55`), starting at `null`. Until HEAD's row
arrives, `headGuide` returns `null`: **no guideline and no edge marker**. This is deliberate —
suppressing the marker is what prevents the UI from asserting "HEAD is below" before it knows. It
is a first-load-only transient, bounded by the first chunk containing HEAD. Do not add a
placeholder marker.

### 1.5 `selfTest.ts` + the dev-hook bag

- `src/graph/frameStats.ts`: add to `P7DevHooks` (L74-96)
  `headGuide: typeof import('./viewport').headGuide;` — no other change to `BonsaiDevHooks`.
- `src/graph/GraphCanvas.tsx:565`: extend the bag →
  `const p7 = { initials, avatarColor, groupRefs, layoutRefLabels, refColArea, avatarHit, relativeDate, headGuide };`
- `src/graph/selfTest.ts`: add a `// P67 §1: head guideline` block of `check(...)` assertions with
  known answers (§7.1 lists them). This JS seam is the **only** way to assert the geometry from a
  headless browser pane, since no canvas pixel is ever produced there.

### 1.6 Contract amendment — `docs/contracts/P1-polish.md` §9.3

House convention: in-place `> **SUPERSEDED by …**` block quotes, **nothing deleted**
(`M6-remotes.md:155`, `P27-worktrees.md:50`). Insert **two** markers.

(a) Immediately after the "WIP row paint" bullet's gate clause (`P1-polish.md:517-518`):

```
> **SUPERSEDED by P67 §1** — the `scrollTop < RH + 56` gate now applies ONLY to the WIP marker
> circle, the hover background and the "Uncommitted changes (n)" label. The dashed connector moved
> out of `drawWipRow` into `drawHeadGuide`, which paints at every scroll position.
```

(b) Immediately after the "Dashed connector" bullet (`P1-polish.md:528-531`):

```
  > **SUPERSEDED by P67 §1** — the connector is now `drawHeadGuide(ctx, layout, guide, theme, m)`
  > fed by the pure `headGuide()` in `src/graph/viewport.ts`: BOTH ends are clamped to
  > `[-8, height + 8]` (not just the target), a `lineDashOffset` keeps the dash phase anchored to
  > content, the target stops at the HEAD avatar halo instead of painting over it, the anchor falls
  > back to `-8` on a clean tree (`wip === null`), and an off-screen HEAD adds
  > `drawHeadEdgeMarker`. Scroll-independent.
```

### 1.7 Explicitly out of scope (deferred, not forgotten)

**Click-the-edge-marker-to-scroll-to-HEAD.** `scrollRowIntoView` already exists
(`viewport.ts:46-59`) but this needs a hit-test box in `hitTest.ts` plus a click route in
`GraphCanvas`. Not in P67; record as a follow-up TODO.

**Rejected alternative (do not resurrect):** a persistent full-height dashed rule in the HEAD lane.
It sits at the same `x` as the solid lane edges `drawGraph` already paints, reads as graph noise,
crosses unrelated avatars, and stops *pointing at* anything — which is the whole point of the
affordance.

---

## 2. Item 2 — right-panel chrome inventory (before / after)

`.right-panel` is a flex column in which **only `.status-panel` flexes**
(`styles.css:2034-2039`); every other child is `flex: none` at natural height with its own
`border-top` and padding. Every px removed from those children goes straight to the changes tree.

| # | Block | Today (est.) | Cozy (est.) | How |
|---|---|---|---|---|
| 1 | `.right-pane-tabs` L6297 + `.right-pane-tab` L6305 | ~39 px | ~30 px | container pad `6px 8px` → `var(--rp-tab-pad, 6px 8px)` = `4px 8px`; tab pad `5px 12px` → `var(--rp-tab-btn-pad, 5px 12px)` = `3px 10px`. **Both tabs stay** — the always-mounted `.right-panel-work` wrapper (L2022-2032) is what preserves a half-typed commit message across tab toggles (§5, do not touch). |
| 2 | `.stash-split` L2480 + `.amend-affordance` L2446 | ~62 px, 2 rows | **~29 px, 1 row** | merged into `RightPanelActionsRow` (§5.1): Amend checkbox left, `⋯` overflow menu right (all three stash scopes). Deletes one `border-top` + one 8 px top pad + one full 28 px button row. |
| 3 | `.commit-box` L2549 padding 12 + gap 8 (×≤5) | 24 + ~40 px | 20 + ~30 px | `--rp-box-pad` 10, `--rp-box-gap` 6 |
| 4 | `.commit-message` L2559 | fixed `min-height: 60` | auto-grow 48 → 160 | D9: `field-sizing: content` + `rows={1}` + `min-height: var(--rp-msg-min, 60px)` / `max-height: var(--rp-msg-max, 120px)`; `resize: vertical` retained |
| 5 | `.commit-sign-row` L2595 + `.commit-skip-row` L2633 | 2 rows, ~46 px | **1 row, ~24 px** | merged into `CommitOptionsRow` (§5.2). Sign is `showSign`-gated, Skip hooks unconditional; both hints become wrapping flex items rendered only when their box is checked. |
| 6 | `.op-banner` L5176 pad `8px 12px` | ~37 px (when shown) | ~33 px | `--rp-banner-pad` = `6px 10px` |
| 7 | `.right-panel-work > .status-panel` L2037 pad 12 | 24 px (top+bottom) | 20 px | `--rp-pad-outer` 10 |
| 8 | `.status-panel` L2184 `gap: 16` + `.status-section` L2194 pad `6px 8px 8px` | — | gap 10, pad `5px 6px 6px` | inside the scroller ⇒ buys rows *within* the tree rather than growing it (~21 px over 3 sections) |

**Reclaimed above the scroller (items 1-7): ≈ 93 px. Reclaimed inside the scroller (item 8):
≈ 21 px. Total ≈ 114 px ≈ 4-5 more `.file-row`s at 24 px.** Compact mode reclaims a further
~2 rows (row height 24→20, gaps 10→6, message min 48→36).

**These numbers are estimates from the CSS, not measurements.** The implementer MUST re-measure in
the browser harness — `document.querySelector('.status-panel').getBoundingClientRect().height`
before/after, in the same viewport, same fixture — and report the real delta. The user-facing
checklist quotes the measured number, never these estimates.

---

## 3. The `--rp-*` custom-property table (P67b introduces, P67c overrides)

Declared once on `.right-panel` (cozy = the new tighter default), overridden by exactly one
`.right-panel[data-density='compact']` block in P67c.

**HARD RULE (D8): every converted declaration uses `var(--rp-x, <today's value>)`.** The fallback
is not defensive style — it is the mechanism that keeps `.file-row`, `.file-list`, `.tree-dir-row`
(also rendered by `DiffFileTree.tsx:71,121` outside the panel) and `.section-label` (also rendered
by `Sidebar.tsx:114`, `EmptyState.tsx:73`, `OnboardingSteps.tsx:87`) **pixel-identical to today**.
A converted rule without a fallback silently resizes the diff tree and the sidebar.

| Custom property | Cozy | Compact | Declaration(s) it replaces (with fallback) |
|---|---|---|---|
| `--rp-pad-outer` | `10px` | `8px` | `.right-panel-work > .status-panel { padding: var(--rp-pad-outer, 12px) }` |
| `--rp-gap` | `10px` | `6px` | `.status-panel { gap: var(--rp-gap, 16px) }` |
| `--rp-section-pad` | `5px 6px 6px` | `3px 6px 4px` | `.status-section { padding: var(--rp-section-pad, 6px 8px 8px) }` |
| `--rp-row-h` | `24px` | `20px` | `.file-row { height: var(--rp-row-h, 24px) }`, `.tree-dir-row { height: var(--rp-row-h, 24px) }` |
| `--rp-row-font` | `inherit` | `12px` | `.file-row { font-size: var(--rp-row-font, inherit) }`, `.tree-dir-row { font-size: var(--rp-row-font, inherit) }` — neither rule declares `font-size` today, and `inherit` reproduces the current computed value exactly, so outside-panel instances are byte-identical |
| `--rp-list-margin` | `3px` | `2px` | `.file-list { margin: var(--rp-list-margin, 4px) 0 0 }`, `.tree { margin: var(--rp-list-margin, 4px) 0 0 }` |
| `--rp-label-font` | `11px` | `10px` | `.section-label { font-size: var(--rp-label-font, 11px) }` |
| `--rp-tab-pad` | `4px 8px` | `3px 6px` | `.right-pane-tabs { padding: var(--rp-tab-pad, 6px 8px) }` |
| `--rp-tab-btn-pad` | `3px 10px` | `2px 8px` | `.right-pane-tab { padding: var(--rp-tab-btn-pad, 5px 12px) }` |
| `--rp-banner-pad` | `6px 10px` | `5px 8px` | `.op-banner { padding: var(--rp-banner-pad, 8px 12px) }` |
| `--rp-box-pad` | `10px` | `8px` | `.commit-box { padding: var(--rp-box-pad, 12px) }` |
| `--rp-box-gap` | `6px` | `4px` | `.commit-box { gap: var(--rp-box-gap, 8px) }` |
| `--rp-msg-min` | `48px` | `36px` | `.commit-message { min-height: var(--rp-msg-min, 60px) }` |
| `--rp-msg-max` | `160px` | `120px` | `.commit-message { max-height: var(--rp-msg-max, 120px) }` |
| `--rp-ctl-h` | `22px` | `20px` | new `.rp-actions-row { min-height: var(--rp-ctl-h, 22px) }`, `.commit-options-row { min-height: var(--rp-ctl-h, 22px) }` |
| `--rp-ctl-gap` | `8px` | `6px` | `.rp-actions-row { gap: var(--rp-ctl-gap, 8px) }`, `.commit-options-row { gap: var(--rp-ctl-gap, 8px) }` |
| `--rp-ctl-font` | `12px` | `11px` | `.rp-actions-row { font-size: var(--rp-ctl-font, 12px) }`, `.commit-options-row { font-size: var(--rp-ctl-font, 12px) }` (12px is today's `.commit-sign-row`/`.commit-skip-row` value) |

Declaration block (P67b):

```css
/* ---------- P67 §3: right-panel density variables ---------- */
.right-panel {
  --rp-pad-outer: 10px;   --rp-gap: 10px;        --rp-section-pad: 5px 6px 6px;
  --rp-row-h: 24px;       --rp-row-font: inherit; --rp-list-margin: 3px;
  --rp-label-font: 11px;  --rp-tab-pad: 4px 8px; --rp-tab-btn-pad: 3px 10px;
  --rp-banner-pad: 6px 10px;
  --rp-box-pad: 10px;     --rp-box-gap: 6px;
  --rp-msg-min: 48px;     --rp-msg-max: 160px;
  --rp-ctl-h: 22px;       --rp-ctl-gap: 8px;     --rp-ctl-font: 12px;
}
```

Override block (P67c — the ONLY thing P67c adds to `styles.css`):

```css
.right-panel[data-density='compact'] {
  --rp-pad-outer: 8px;    --rp-gap: 6px;         --rp-section-pad: 3px 6px 4px;
  --rp-row-h: 20px;       --rp-row-font: 12px;   --rp-list-margin: 2px;
  --rp-label-font: 10px;  --rp-tab-pad: 3px 6px; --rp-tab-btn-pad: 2px 8px;
  --rp-banner-pad: 5px 8px;
  --rp-box-pad: 8px;      --rp-box-gap: 4px;
  --rp-msg-min: 36px;     --rp-msg-max: 120px;
  --rp-ctl-h: 20px;       --rp-ctl-gap: 6px;     --rp-ctl-font: 11px;
}
```

Deleted CSS: `.stash-split*` (`styles.css:2480-2547`, 68 lines) and the adjacency rules
`.stash-split + .amend-affordance` (`:2488`) — replaced by `.rp-actions*` rules. `.amend-toggle`,
`.amend-push-warning` and `.amend-affordance + .commit-box { border-top: none }` are **kept**
(reused by the new row; the last one becomes `.rp-actions + .commit-box`).

---

## 4. `panelDensity` — wire format + the full layer table (P67c)

### 4.1 Wire format

- serde/TS value strings: `"cozy"` | `"compact"` (lowercase, like `ListView`'s `"tree"`/`"flat"`).
- JSON key: `panelDensity` (camelCase, `Settings` has container-level `#[serde(rename_all =
  "camelCase")]`).
- On-disk shape addition: `{ …, "listView": "tree", "panelDensity": "cozy", … }`.
- Patch semantics: `Option<PanelDensity>` / `panelDensity?: PanelDensity` — patches independently
  of `listView` and `graph` (pinned by a test, §7.2).

### 4.2 Migration justification (no version bump)

`settings.rs:282-291` documents the bar: `SETTINGS_VERSION` stays `1` while every added field is an
additive `#[serde(default)]` field with a safe type default, and a bump is required only for a
genuine breaking change (renaming/removing a field with no safe default). `panel_density` is a
new field whose `Default` is `Cozy` and whose absence is indistinguishable from an explicit
`"cozy"`, so a pre-P67 `settings.json` deserialises unchanged. **No version bump.** Extend the
field list in that doc comment to mention `panel_density`. `clamp_graph_prefs` is **NOT** touched —
`panel_density` is not a graph pref and has no numeric range to clamp.

### 4.3 Layer table

| Layer | File (anchor) | Change |
|---|---|---|
| TS type | `src/ipc/types.ts` (beside `ListView`, ~L1035) | `export type PanelDensity = 'cozy' \| 'compact';` |
| TS settings | `src/ipc/types.ts` L1364 / L1394 | `UiSettings.panelDensity: PanelDensity;` after `listView`; `UiSettingsPatch.panelDensity?: PanelDensity;` after `listView` |
| App state | `src/App.tsx` L99 | `const [panelDensity, setPanelDensity] = useState<PanelDensity>('cozy');` |
| App load | `src/App.tsx` L737 | `setPanelDensity(s.panelDensity);` after `setListView(s.listView);` |
| App patch | `src/App.tsx` L375-391 | one line in `handleSettingsChange`: `if (patch.panelDensity !== undefined) setPanelDensity(patch.panelDensity);`. **No dedicated `onTogglePanelDensity`** — unlike theme/listView there is no toolbar button, so it rides the existing debounced patch path |
| App threading | `src/App.tsx` L1006, L1056 | `panelDensity={panelDensity}` → `<RepoWorkspace>`; `panelDensity={panelDensity}` → `<SettingsPanel>` |
| Workspace | `src/components/RepoWorkspace.tsx` L115 / L151 / L2741+ | prop `panelDensity: PanelDensity;`, destructure, forward to `<WorkspaceRightPanel panelDensity={panelDensity} />` |
| Panel | `src/components/WorkspaceRightPanel.tsx` L200 | `<aside className="right-panel" data-density={panelDensity} style={{ width: rightPanelWidth }}>` — always present (also `"cozy"`), so the harness reads one attribute unconditionally |
| Settings UI | new `src/components/SettingsAppearanceSection.tsx`, used from `SettingsPanel.tsx` L298-317 | extract the whole Appearance `<section>` (Theme + File lists) into the new file and add the third row (§5.3) |
| Mock default | `src/ipc/mock/persistence.ts` L79 | `panelDensity: 'cozy',` after `listView: 'tree',` |
| Mock parse | `src/ipc/mock/persistence.ts` L216 + L320 | `const panelDensity: PanelDensity = parsed.panelDensity === 'compact' ? 'compact' : 'cozy';` beside the `listView` line, and `panelDensity,` in the returned object |
| Mock patch | `src/ipc/mock/handlers/session.ts` L76 | `panelDensity: patch.panelDensity ?? current.panelDensity,` |
| Rust enum | `src-tauri/src/settings.rs` beside `ListView` L64-72 | see below |
| Rust settings | `src-tauri/src/settings.rs` L299 / L377 / doc L282-291 | field after `list_view`, `Default` arm, doc-comment field list |
| Rust re-export | `src-tauri/src/commands/shared.rs` L96-100 | add `PanelDensity` to the `crate::settings::{…}` re-export (`ui_settings.rs` refers to these unqualified) |
| Rust command | `src-tauri/src/commands/ui_settings.rs` L14 / L53 / L92-94 / L147 / L185 | field in `UiSettings`, `Option<PanelDensity>` in `UiSettingsPatch`, one `apply_patch` arm, and **both** builder literals (`get_ui_settings` L144-161 **and** `set_ui_settings` L182-199 — forgetting the second is a compile error, good) |

```rust
/// Right-panel vertical density (P67). Pure UI preference; display-only, no Git
/// effect. `Cozy` is the P67b tightened default; `Compact` squeezes rows,
/// paddings and fonts further. INDEPENDENT of `GraphPrefs::compact` (which is
/// canvas row geometry) — Settings only cross-references the two.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PanelDensity {
    #[default]
    Cozy,
    Compact,
}
```

```rust
// settings.rs — Settings, after `list_view`:
    /// P67: right-panel vertical density. Additive `#[serde(default)]` (via the
    /// container-level `default`); a pre-P67 settings.json without this key loads
    /// `PanelDensity::default()` (Cozy). No version bump — meets the documented
    /// bar above. NOT clamped (no numeric range; `clamp_graph_prefs` untouched).
    pub panel_density: PanelDensity,
```

```rust
// ui_settings.rs — apply_patch, mirroring the list_view arm at L92-94:
    if let Some(panel_density) = patch.panel_density {
        s.panel_density = panel_density;
    }
```

```ts
// types.ts
/** P67 §4: right-panel vertical density. Independent of `GraphPrefs.compact`
 *  (graph row geometry). 'cozy' is the P67b tightened default. */
export type PanelDensity = 'cozy' | 'compact';
```

---

## 5. New / deleted / edited files (with line budgets)

Soft limit ~500 lines per file (CLAUDE.md). Every number below is a target, not a hard gate;
report actuals.

### 5.1 NEW `src/components/RightPanelActionsRow.tsx` (~110)

```tsx
export interface RightPanelActionsRowProps {
  /** Amend state — OWNED BY WorkspaceRightPanel (D4: CommitBox is keyed on it). */
  amend: boolean;
  onToggleAmend(next: boolean): void;
  /** App-wide mutation in flight → every control disabled. */
  busy: boolean;
  /** Amend would rewrite already-pushed history (upstream set && ahead === 0). */
  showAmendPushWarning: boolean;
  /** `busy || nothing to stash at all` → the ⋯ button is disabled. */
  stashDisabled: boolean;
  /** Per-scope enablement, identical rules to the deleted StashSplitButton. */
  stagedCount: number;
  hasTrackedChanges: boolean;
  hasUntracked: boolean;
  onStash(scope: StashScope): void;
}
```

Markup contract (class names and visible strings are part of the contract — tests and muscle memory
depend on them):

```
<div className="rp-actions">
  <div className="rp-actions-row">
    <label className="amend-toggle">
      <input type="checkbox" checked={amend} disabled={busy} onChange=… />
      <span>Amend last commit</span>
    </label>
    <div className="rp-overflow" ref={rootRef}>
      <button type="button" className="rp-overflow-btn"
              aria-haspopup="menu" aria-expanded={open}
              aria-label="More actions" disabled={stashDisabled}>⋯</button>
      {open && (
        <div className="rp-overflow-menu" role="menu">
          <button role="menuitem" className="rp-overflow-item" disabled={!hasTrackedChanges}>Stash all</button>
          <button role="menuitem" className="rp-overflow-item" disabled={!(hasTrackedChanges || hasUntracked)}>Stash all + untracked</button>
          <button role="menuitem" className="rp-overflow-item" disabled={stagedCount === 0}>Stash staged only</button>
        </div>
      )}
    </div>
  </div>
  {showAmendPushWarning && (
    <div className="amend-push-warning" role="note">
      This commit is already pushed — amending rewrites published history.
    </div>
  )}
</div>
```

- The `open` state, the document `mousedown` outside-click close and the `Escape` handler are
  **moved verbatim** from `StashSplitButton.tsx:33-52`; `choose(scope)` closes then calls
  `onStash(scope)` (`:64-67`).
- Scope labels and enable rules are **unchanged** from `StashSplitButton.tsx:54-62`. The sidebar's
  one-click stash (`Sidebar.tsx:819`, `allWithUntracked`) is untouched and remains the fast path.
- Menu positioning: reuse the `.stash-split-menu` geometry (absolute, `bottom: calc(100% - 4px)`,
  `right: 0`) under the new `.rp-overflow-menu` name.

### 5.2 NEW `src/components/CommitOptionsRow.tsx` (~90)

```tsx
export interface CommitOptionsRowProps {
  /** P58c: render the Sign checkbox at all (false in merge mode / unknown status). */
  showSign: boolean;
  signChecked: boolean;
  onChangeSign(next: boolean): void;
  signingStatus: SigningStatus | null | undefined;
  /** 'SSH' | 'GPG' — precomputed by CommitBox (L120). */
  signFormatLabel: string;
  onOpenIdentitySettings?: () => void;
  skipHooks: boolean;
  onChangeSkipHooks(next: boolean): void;
  /** `submitting !== null || blocked` — both checkboxes. */
  disabled: boolean;
}
```

One `<div className="commit-options-row">` (flex, `flex-wrap: wrap`) containing, in order: the Sign
`<label className="commit-sign-toggle">` (only when `showSign`), the Skip `<label
className="commit-skip-toggle">`, then the hints as wrapping flex items — `.commit-sign-hint` /
`.commit-sign-warn` (+ the `.commit-sign-fix` "Set key…" button) when `signChecked`, and
`.commit-skip-hint` when `skipHooks`. **Existing class names, `<span>` label texts
("Sign commit", "Skip hooks") and hint strings are preserved verbatim** so
`CommitBox.test.tsx:93,104` (`getByRole('checkbox', { name: /Sign commit/ })`,
`/Skip hooks/`) keep passing untouched.

### 5.3 NEW `src/components/SettingsAppearanceSection.tsx` (~70)

```tsx
export interface SettingsAppearanceSectionProps {
  theme: Theme;
  onToggleTheme(): void;
  listView: ListView;
  onToggleListView(): void;
  /** P67: right-panel density (rides the debounced settings patch). */
  panelDensity: PanelDensity;
  onChange(patch: UiSettingsPatch): void;
}
```

Extract `SettingsPanel.tsx:298-317` verbatim (Theme row + File-lists row), then add a third
`.settings-row`:

- label `<span className="settings-control-label">Panel density</span>`
- one `btn-secondary settings-toggle-btn` showing the **current** value
  (`panelDensity === 'cozy' ? 'Cozy' : 'Compact'`) and flipping on click via
  `onChange({ panelDensity: panelDensity === 'cozy' ? 'compact' : 'cozy' })` — exactly the
  Theme/File-lists idiom, so `SettingsPanel.test.tsx`'s
  `getByRole('button', { name: 'Cozy' })` style works.
- one `<p className="settings-section-desc">` cross-reference hint, e.g. *"Affects the right panel
  only. Graph row density is a separate setting under Graph → Compact rows."*

`SettingsPanel.tsx` swaps the inline block for `<SettingsAppearanceSection … />` in the same
position (between `SettingsGraphSection` and `SettingsGitConfigSection`) and gains one
`panelDensity` prop.

### 5.4 DELETED

- `src/components/StashSplitButton.tsx` (111 lines) — no test file exists for it; its logic moves
  into §5.1.
- `src/styles.css:2480-2547` (`.stash-split*`, 68 lines).
- The `StashSplitButton` import at `WorkspaceRightPanel.tsx:9`.

### 5.5 EDITED (with target sizes)

| File | Now | Target | Nature of the change |
|---|---|---|---|
| `src/graph/viewport.ts` | 96 | ~155 | `+HeadGuide`, `+headGuide`, `+HEAD_GUIDE_PAD` |
| `src/graph/draw.ts` | — | −13/+45 | delete `:200-212`; add `drawHeadGuide`, `drawHeadEdgeMarker`, `HEAD_EDGE_MARKER` |
| `src/graph/GraphCanvas.tsx` | — | +12 | the §1.3 call block; `headGuide` in the `p7` bag |
| `src/graph/frameStats.ts` | — | +2 | `P7DevHooks.headGuide` |
| `src/graph/selfTest.ts` | 201 | ~245 | the P67 known-answer block |
| `src/components/WorkspaceRightPanel.tsx` | 386 | ~360 | two blocks (L299-335) → one `<RightPanelActionsRow>`; `+panelDensity` prop + `data-density` |
| `src/components/CommitBox.tsx` | 379 | ~300 | sign+skip blocks (L262-309) → one `<CommitOptionsRow>`; textarea `rows={merge ? 5 : 1}`; state stays here |
| `src/components/SettingsPanel.tsx` | ~400 | ~380 | Appearance block extracted; `+panelDensity` prop |
| `src/components/RepoWorkspace.tsx` | 3049 | +3 | prop, destructure, forward |
| `src/App.tsx` | — | +5 | state, load, patch line, two prop passes |
| `src/styles.css` | 6811 | ≈ +55 / −70 | §3 var blocks + `.rp-actions*` / `.commit-options-row`; delete `.stash-split*` |
| `src/ipc/types.ts` | — | +6 | `PanelDensity` + two fields |
| `src/ipc/mock/persistence.ts` | — | +3 | default, parse, return |
| `src/ipc/mock/handlers/session.ts` | — | +1 | patch merge |
| `src-tauri/src/settings.rs` | — | +18 | enum, field, default, doc, 2 tests |
| `src-tauri/src/commands/ui_settings.rs` | — | +7 | field, patch field, arm, 2 builders |
| `src-tauri/src/commands/shared.rs` | — | +1 | re-export |
| `src-tauri/src/commands/tests.rs` | — | +10 | `panel_density` arm in `set_ui_settings_patch_is_partial` |
| `docs/contracts/P1-polish.md` | — | +12 | the two §1.6 SUPERSEDED markers (nothing deleted) |

### 5.6 P67e — `StatusPanel.tsx` split (pure refactor, droppable)

700 → ~250 by moving three self-contained blocks into their own files. **Behaviour-free**: no prop
renames, no logic edits, no CSS changes.

| New file | Target | Moved from |
|---|---|---|
| `src/components/StatusFileRow.tsx` | ~140 | `FileRow` + its props type, `StatusPanel.tsx:44-172` |
| `src/components/StatusSection.tsx` | ~165 | `Section` + its props type, `:174-326` |
| `src/components/StatusConflictsSection.tsx` | ~180 | `CONFLICT_KIND_LABELS`, `ConflictRow`, `ConflictsSection`, `:329-486` |

Shared helpers `entryPaths` (`:30`), `splitPath` (`:34`), `BADGES` (`:19`) and the exported
`WorkdirSection` type (`:42`) move to whichever file needs them and are imported back — or into a
small `statusPanelUtils.ts` if more than one file needs them. `export type { DiffSlot } from
'./DiffView'` (`:17`) stays in `StatusPanel.tsx` (public re-export). Components are exported by
name (`export function StatusFileRow`, `StatusSection`, `StatusConflictsSection`) with JSX call
sites renamed accordingly.

**`src/components/StatusPanel.test.tsx` (271 lines) must pass UNTOUCHED — that is the refactor's
acceptance test.** If a test needs editing, the refactor changed behaviour and is wrong. The
density work needs zero `StatusPanel.tsx` change (pure CSS), which is exactly why this goes last
and can be dropped without blocking P67.

---

## 6. Sub-increments

Each is one fresh-context senior-dev pass. Commit after each reviewer approval.

### P67a — HEAD guideline
Scope: `viewport.ts` (`headGuide`, `HeadGuide`, `HEAD_GUIDE_PAD`) + `viewport.test.ts` cases;
`draw.ts` (delete `:200-212`, add `drawHeadGuide` / `drawHeadEdgeMarker`); `GraphCanvas.tsx:345-355`
call order + the `p7` bag; `frameStats.ts` hook type; `selfTest.ts` known answers; the two
`P1-polish.md` §9.3 SUPERSEDED markers.
**Acceptance:** (1) `pnpm build` + `tsc` clean; the **fourteen** `describe('headGuide')` cases (11 as originally specified, +2 from A5, +1 from A6.3) green
(§7.1). (2) `window.__bonsai.p7SelfTest()` reports the new head-guide checks passing, 0 failures.
(3) Harness: `window.__bonsai.p7.headGuide({...})` returns a non-null segment at
`layoutScrollTop: 50_000` with `headIndex: 2000` and both ends inside `[-8, h+8]`; identical
`dashOffset` at `+6` px. **Corrected 2026-08-17** — the original `headIndex: 1500` is
arithmetically impossible: at `rowHeight: 32` it puts HEAD's centre at content y 48 016, ~2 000 px
*above* a viewport at `layoutScrollTop: 50_000`, so both ends clamp to `-8` and (post-A5) the result
is marker-only, not a segment. `headIndex: 2000` puts HEAD below the viewport as intended →
`{ y0: -8, y1: 648, dashOffset: 2, edge: 'bottom', segment: true }` at `viewportHeight: 640`.
**Further corrected by A6.1 (§1.1b):** that `dashOffset` is **4**, not 2 — `2` came from the
inverted `anchor - y0` form; with the correct `y0 - anchor` it is `(-8 + 50_016) mod 6 = 4`. (4) `.graph-spacer` scroll extent unchanged (`spacerHeight` untouched).
(5) `draw.ts` still has **no** test file (D2) and `drawHeadGuide`/`drawHeadEdgeMarker` contain no
scroll/row/clamp/dash arithmetic (reviewer greps for `scrollTop`, `rowHeight`, `Math.max`,
`Math.min` inside them).

### P67b — right-panel structure + tighter default (no settings field)
Scope: new `RightPanelActionsRow.tsx`, `CommitOptionsRow.tsx`; delete `StashSplitButton.tsx` +
CSS `:2480-2547`; edit `WorkspaceRightPanel.tsx` (one row replaces two blocks) and `CommitBox.tsx`
(one options row; `rows={merge ? 5 : 1}`); in `styles.css` add the `.right-panel` `--rp-*` block
with **cozy = the new tighter values** and convert every §3 declaration to
`var(--rp-*, <today's value>)`; add `.rp-actions*` / `.commit-options-row` rules; new
`WorkspaceRightPanel.test.tsx`.
**Acceptance:** (1) `tsc` + `pnpm build` clean; `CommitBox.test.tsx` and `StatusPanel.test.tsx`
pass **unchanged**. (2) `WorkspaceRightPanel.test.tsx` green (§7.1). (3) Harness: no `.stash-split`
element anywhere in the DOM; the `⋯` menu lists all three scopes with the same per-scope disabled
states; `.status-panel` `getBoundingClientRect().height` grew — **report the measured px and the
row equivalent at 24 px**. (4) Computed styles outside the panel are unchanged: `.file-row` inside
`DiffFileTree` still computes `height: 24px`, `.section-label` in the sidebar still `11px`
(the D8 fallback proof). (5) No settings field exists yet; `getUiSettings()` shape unchanged.
(6) New files at/below their §5 budgets; `CommitBox.tsx` **≤ ~350**.
**Corrected 2026-08-17:** the original "≤ ~310" was unreachable from §5.5's own change list —
removing the 46-line sign/skip JSX and adding ~13 lines of `<CommitOptionsRow>` gives 379 − 33 =
**349**. Reaching 310 would require extracting a *second*, unspecified block (the `.commit-box-header`
Generate/Compose pair, or the error banner). **Decision: land 349 and do not extract further** —
`CommitBox.tsx` is comfortably inside the ~500-line soft limit, and a speculative split would add a
file boundary that no size pressure justifies. A `CommitBoxHeader.tsx` extraction stays available if
`CommitBox.tsx` later grows toward the limit.

### P67c — density setting end-to-end
Scope: the §4.3 layer table (16 rows) + exactly one `.right-panel[data-density='compact']` block +
`SettingsAppearanceSection.tsx` extraction + the two `settings.rs` tests + the `tests.rs:1000` arm
+ `SettingsPanel.test.tsx` extension.
**Acceptance:** (1) `cargo test -p bonsai --lib` (or the workspace suite) green incl.
`panel_density_roundtrips_both_variants` and
`old_settings_file_without_panel_density_loads_default`; `cargo clippy --workspace --tests --
-D warnings` clean; `generate_handler!` still lists **157**. (2) `tsc` + `pnpm build` clean;
`SettingsPanel.test.tsx` green. (3) Harness: `await ipc.getUiSettings()` returns
`panelDensity: 'cozy'`; `await ipc.setUiSettings({ panelDensity: 'compact' })` round-trips and
leaves `listView`/`graph` untouched; the Settings → Appearance button flips Cozy ↔ Compact;
`.right-panel[data-density]` flips and `getComputedStyle(el).getPropertyValue('--rp-row-h')` reads
`24px` / `20px`. (4) `clamp_graph_prefs` untouched (reviewer greps the diff).

### P67d — docs + board
Scope: this contract file (already on disk) + `docs/contracts/P67-user-checklist.md`; the `TODO.md`
P67 entry after the P66 block (`TODO.md:231`) with the measured space-gained number; a CHANGELOG
line. No code.
**Acceptance:** contract + checklist + `TODO.md` current-step line consistent with the shipped
increments; `git status --porcelain -uall` shows the docs (a `--name-only` diff hides untracked
files — this has stranded docs twice before).

### P67e — `StatusPanel.tsx` split (droppable)
Scope: §5.6 only.
**Acceptance:** (1) `StatusPanel.test.tsx` passes **with zero edits**. (2) `tsc` + `pnpm build`
clean; `StatusPanel.tsx` ≤ ~260 and each new file ≤ ~200. (3) Harness screenshot of the status
panel is visually identical to the P67c screenshot (same fixture, same viewport). (4) Reviewer
confirms the diff is a pure move: no changed conditionals, no renamed props, no CSS touched.

---

## 7. Acceptance criteria

### 7.1 AI gate — vitest

`src/graph/viewport.test.ts`, new `describe('headGuide')` (11 cases):
1. `headIndex: null` → `null`.
2. WIP present (`wipOffset: 1`) at raw `scrollTop 0` → `y0 === rowHeight / 2`,
   `y1 === headCenter - (avatarRadius + ringExtra)`, `edge === null`, `dashOffset === 0`.
3. **Regression guard (the user's bug):** scrolled well past the WIP row — e.g. `wipOffset: 1`,
   `rowHeight: 32`, raw `scrollTop 5000` (`layoutScrollTop: 4968`), `headIndex: 200` → a **non-null
   segment** is returned. (The old gate drew nothing beyond 88 px.)
4. HEAD above the viewport → `edge === 'top'`.
5. HEAD below the viewport → `edge === 'bottom'`.
6. **Perf guard:** absurd `layoutScrollTop` (1e6) with `headIndex: 0` → both `y0` and `y1` inside
   `[-HEAD_GUIDE_PAD, viewportHeight + HEAD_GUIDE_PAD]`.
7. **Crawl guard:** `dashOffset` is identical at `layoutScrollTop` and `layoutScrollTop + 6` at a
   deep (clamped) scroll position, and always in `[0, 6)`.
8. Avatar shortening: `y1` stops `avatarRadius + ringExtra` short of the HEAD centre — on the
   near side, for HEAD **below** the anchor and for HEAD **above** it (sign flip).
9. Clean tree (`wipOffset: 0`) → `y0 === -HEAD_GUIDE_PAD` and a segment is still returned.
10. Collapse: HEAD centre within `halo + 1` of the anchor → `null`.
11. `headIndex` is echoed on the result (so the draw layer needs no non-null assertion).

New `src/components/WorkspaceRightPanel.test.tsx`:
- renders one actions row: the "Amend last commit" checkbox and a `⋯` button named "More actions".
- `⋯` opens `role="menu"` with exactly three `role="menuitem"`s — "Stash all",
  "Stash all + untracked", "Stash staged only" — each firing `onCreateStash` with `'all'` /
  `'allWithUntracked'` / `'staged'`.
- per-scope disabled gating identical to the deleted `StashSplitButton`: `staged` disabled when
  `stagedCount === 0`; `all` disabled without tracked changes; `allWithUntracked` enabled by
  untracked alone; the `⋯` button disabled when `mutating` or nothing to stash at all.
- outside `mousedown` and `Escape` close the menu.
- the whole row is absent when `opState.kind !== 'none'` or `head.unborn`.
- the amend push warning renders only when `amend && headBranch.upstream !== null &&
  headBranch.ahead === 0`.
- `.right-panel` carries `data-density="cozy"` / `"compact"` from the prop.
- **no `.stash-split` element exists** (regression guard for the deletion).

Unchanged-and-must-stay-green: `src/components/CommitBox.test.tsx` (its role+name queries at
`:93,104` survive the row merge) **plus one new assertion** that the Sign and Skip checkboxes are
siblings inside a single `.commit-options-row`. `src/components/StatusPanel.test.tsx` unchanged in
both P67b/P67c **and** P67e.

`src/components/SettingsPanel.test.tsx:92-98` extended: the Appearance section shows a
`Cozy` button; clicking it calls `onChange` with exactly `{ panelDensity: 'compact' }` (and the
existing Theme/File-lists assertions still pass).

**`src/graph/draw.ts` stays untested by design (D2).** Stated here so a reviewer does not file a
canvas-mock suite as a MUST-FIX.

### 7.2 AI gate — cargo

- `src-tauri/src/settings.rs`, new (clone the `ListView` precedents):
  - `panel_density_roundtrips_both_variants` (model: `list_view_roundtrips_both_variants`
    L732-758) — both variants survive `save_to`/`load_from`, and the raw JSON contains
    `"panelDensity": "cozy"` / `"panelDensity": "compact"`.
  - `old_settings_file_without_panel_density_loads_default` (model:
    `old_settings_file_without_list_view_loads_default` L842-861) — a pre-P67 JSON with
    `version`/`recentRepos`/`theme`/`paneWidths` loads `PanelDensity::Cozy` with the other fields
    untouched. **This is the migration guard for the no-version-bump claim (§4.2).**
- `src-tauri/src/commands/tests.rs:1000` `set_ui_settings_patch_is_partial` gains a `panel_density`
  arm proving it patches **independently** of `list_view` and `graph`: patch only
  `panel_density: Some(PanelDensity::Compact)` → `s.panel_density == Compact`,
  `s.list_view == Tree`, `s.graph == GraphPrefs::default()`, `s.theme` untouched.
- `cargo clippy --workspace --tests -- -D warnings` clean. **Never run `cargo test` and `clippy`
  concurrently** (target-dir race); set `TMP`/`TEMP` to `D:\Temp`.
- `generate_handler!` count **157, unchanged** (grep the macro list and count).

### 7.3 AI gate — browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`)

Frugal, batched checks (no screenshot per step; one final visual):
1. `document.querySelector('.right-panel').dataset.density` flips `'cozy'` → `'compact'` through
   the Settings button, and survives a reload (localStorage-backed mock persistence).
2. `getComputedStyle(document.querySelector('.right-panel'))` reports the §3 values for
   `--rp-row-h`, `--rp-gap`, `--rp-msg-min`, `--rp-ctl-h` in **both** densities.
3. **The objective space-gained number:** `.status-panel`
   `getBoundingClientRect().height` before P67b vs after, and cozy vs compact, in the same viewport
   with the same fixture. Divide by 24 px for the row equivalent. This number goes in `TODO.md` and
   the user checklist — the §2 estimates never do.
4. `document.querySelectorAll('.stash-split').length === 0`; the `⋯` menu opens and lists the three
   scopes.
5. Outside-panel invariance (D8): a `.file-row` inside the diff overlay's `DiffFileTree` still
   computes `height: 24px`; a sidebar `.section-label` still computes `font-size: 11px`, in both
   densities.
6. `.graph-spacer` scroll extent unchanged before/after P67a.
7. `window.__bonsai.p7SelfTest()` → `fail: 0`, and `window.__bonsai.p7.headGuide` is callable with
   the §7.1 case-3 and case-7 arguments.
8. Console clean (no new warnings/errors); `tsc` + `pnpm build` clean.

### 7.4 USER CHECKPOINT — native only (`pnpm tauri dev`)

**The browser pane is headless.** It composites at 0×0, so `requestAnimationFrame` never fires and
**no canvas pixel is ever produced there** — the AI gate can prove the *geometry* via
`headGuide()`/`p7SelfTest()` but can never see the line. Likewise, whether a density "looks good"
is human perception, not a measurement. The following are therefore native-only and the
orchestrator must **never** self-declare them:

- The dashed guideline is **visibly present while scrolling** (including far past the WIP row, and
  on a clean tree with no uncommitted changes).
- The dashes **do not crawl** during a scroll (the `dashOffset` phase check).
- The line **terminates cleanly at the HEAD avatar's ring**, not over it.
- The off-screen edge marker **reads correctly** — points the right way, correct lane colour,
  appears/disappears at the right moment.
- The **cozy default** looks right (not cramped) and the **compact** mode is still legible at the
  user's display/DPR, in **both** light and dark themes.
- The `⋯` overflow menu is **discoverable** and all three stash scopes behave.
- The auto-growing commit message feels right in the native WebView2 (`field-sizing: content`).

Full numbered list: `docs/contracts/P67-user-checklist.md`.

---

## 8. Ambiguities resolved / flagged to the orchestrator

Resolved while writing (non-blocking, recorded for the reviewer so these are not read as drift):

- **A1 — `headGuide` takes 7 fields, not 6.** The task sketch listed `avatarRadius`; the halo also
  needs `avatarBgRingExtra`. Summing them at the call site would put untested arithmetic in
  `GraphCanvas`, defeating D2, so the param object carries `avatarRadius` **and** `ringExtra` and
  `viewport.ts` sums them.
- **A2 — `HeadGuide` also carries `headIndex`.** A four-field return would force a non-null
  assertion (`layout.nodes[layout.headIndex!]`) in `draw.ts`. Echoing the narrowed index is free
  and keeps the draw layer assertion-free.
- **A3 — `drawHeadEdgeMarker` takes `viewportHeight` and does one subtraction** for the bottom
  edge, plus local triangle-vertex offsets. The D2 invariant is therefore stated precisely as
  "no scroll / row / clamp / dash-phase arithmetic in `draw.ts`" rather than "zero arithmetic".
  The alternative (returning `edgeY` from `headGuide`) would push a paint constant into
  `viewport.ts` — worse separation.
- **A4 — `--rp-row-font` fallback is `inherit`.** `.file-row` / `.tree-dir-row` declare no
  `font-size` today, so there is no literal to fall back to; `inherit` reproduces today's computed
  value exactly and keeps the outside-panel instances byte-identical.

**Needs the orchestrator's (or the user's) confirmation:**

- **OQ1 — Density control shape.** This contract specifies **one toggling
  `settings-toggle-btn` showing the current value** (`Cozy` ⇄ `Compact`), matching Theme and
  File lists exactly. The approved plan's wording ("→ Cozy/Compact") could also mean a two-button
  segmented control. **Recommend the single toggle** (consistency, one fewer control style, and it
  is what the extended `SettingsPanel.test.tsx` asserts). Confirm before P67c.
- **OQ2 — Compact values are a first cut.** The compact column in §3 (row 20 px, font 12 px,
  gap 6 px, message min 36 px) is a judgement call that cannot be validated headlessly. Expect one
  native tuning pass after the USER CHECKPOINT; the values live in **one** CSS block, so tuning is
  a one-line-per-value edit with no code change.
- **OQ3 — Edge-marker shape/size.** A 10×6 px filled triangle in the lane colour, 2 px inset. Not
  AI-verifiable. Flagging so the user is asked about it at the checkpoint rather than after.
- **OQ4 — Merge-mode textarea.** `rows={merge ? 5 : 1}` keeps the taller fallback for merge
  messages, but with `field-sizing: content` the supporting browser sizes to content and only
  `--rp-msg-min` (48 px) floors it. If the user prefers merge messages to *open* tall,
  `--rp-msg-min` needs a merge-mode variant. **Recommend shipping as specified** and revisiting
  only if the checkpoint flags it.
- **OQ5 — P67e ordering.** `StatusPanel.tsx` at 700 lines is over the ~500 soft limit today, so the
  split is overdue; it is scheduled last because it is behaviour-free and droppable if the batch
  needs to land sooner. Confirm it should still ship inside P67 rather than as its own hygiene
  commit.
