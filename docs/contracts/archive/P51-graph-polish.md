# P51 — Commit-graph polish + clutter controls

Add the most-requested per-row graph details, each **individually toggleable** (clutter is the #1
graph complaint worldwide), with sensible defaults and a **compact mode**. New per row: short SHA,
verified/signed **badge stub** (lit later by P58), absolute date **on hover** + an author-vs-committer
date **basis** choice, **ahead/behind** on branch-tip rows. Plus **column show/hide** (author/date/SHA)
and removal of the dead `dotRadius` pref.

References read (current state, verified — not guessed): `crates/bonsai-core/src/graph.rs`
(`GraphNode` populate at L487-495, `commit.author()` only — no committer today),
`src-tauri/src/settings.rs` (`GraphPrefs` L162-213 + clamp + `DOT_RADIUS_*` consts + P49 additive
pattern + back-compat tests), `src-tauri/src/commands/ui_settings.rs` (`UiSettings`/`UiSettingsPatch`
carry `graph: GraphPrefs` whole-struct — no per-field plumbing), `src/ipc/types.ts` (`GraphNode` L71,
`BranchInfo` L200 with `ahead`/`behind`/`tip`, `GraphPrefs` L840, `UiSettings` L937), `src/graph/
metrics.ts` (`METRICS`, `effectiveMetrics`, `MetricKnob`), `src/graph/draw.ts` (pass 4 avatar L721-787,
pass 5 text L793-836, ref-label subsystem `groupRefs`/`entityStyle`/`layoutRefLabels`/`drawRefLabelAt`
L278-540 — all in draw.ts, which is **838 lines, already over the 500 soft limit**), `src/graph/
GraphCanvas.tsx` (`GraphCanvasProps` L42, `TooltipState` L90, `computeHoverTarget` L701, tooltip render
L889), `src/graph/colors.ts` (`Theme`), `src/components/SettingsPanel.tsx` (GRAPH section L363-398,
dead-`dotRadius` comment L367-370), `src/ipc/mock/persistence.ts` (`DEFAULT_UI_SETTINGS.graph` L83,
clamp L151, tolerant parse L198-213), `src/components/WorkspaceGraphPane.tsx` (`<GraphCanvas>` L237),
`src/components/RepoWorkspace.tsx` (`effectiveMetrics(graphPrefs)` L144, `metricsVersion`, `branches`).
House pattern: `docs/contracts/M2-graph.md`, `P49-external-integrations.md`, `P50-search-command-palette.md`.

**No new Tauri command. No new AppError. Command count unchanged (129).** All new prefs ride the
existing `graph: GraphPrefs` patch path. Open decisions in §12.

---

## 0. Key decisions (with rationale)

**D1 — All new render prefs nest inside `GraphPrefs` (serde key `graph`), NOT top-level `UiSettings`.**
`UiSettings`/`UiSettingsPatch`/`apply_patch`/the get+set constructors already carry `graph` as a
whole-struct patch (the frontend sends the entire `graph` object on any change). Nesting means P51 adds
**zero** lines to `ui_settings.rs` and the TS `UiSettings`/`UiSettingsPatch` — only `GraphPrefs` grows.
`GraphPrefs` stays `Copy` (bools + one field-less enum are `Copy`). This is strictly cheaper than P49's
top-level string fields and keeps every graph pref in one section/one patch.

**D2 — Add ONE backend field `committer_ts: i64` to `GraphNode` (recommend ADD, not defer).** The
author-vs-committer date **choice** is an explicit requirement and needs the committer time; it is the
minimal touch (`commit.committer().when().seconds()`, +8 bytes/node ≈ +248 KB JSON at 31k — inside the
M2 wire budget). Short SHA (slice `id`), absolute date (format existing `ts`), and ahead/behind (from
`BranchInfo`) need **no** backend. Committer **name** is NOT added — the choice is about the *date*; the
optional author-name column stays author-only (§12 OQ4).

**D3 — Ahead/behind renders in the LEFT ref band, attached to its local-branch pill** (GitKraken-style,
semantically bound to the branch; requirement #4 lists it separately from the right author/date/SHA
column model, which confirms it is not a right column). Frontend-only: a `branchStats` map
(`name → {ahead,behind}`) derived from the existing `BranchInfo[]`, threaded to the draw layer;
resolved per `localBranch` ref by **name** (the pill's `name` == `BranchInfo.name`). Alternative (a
dedicated right column) is flagged in §12 OQ1.

**D4 — Right-side columns (author / SHA / date) use a small pure column-model** (`rightColumns.ts`) so
show/hide reclaims space with no overlap. Fixed left→right order **author, SHA(+badge), date**; **reorder
deferred** (§12 OQ2 — the model already iterates an ordered list, so a future reorder is purely additive:
persist an order array + a settings control). The "author" column is an **optional full-name text
column** (default OFF); the author-initials **avatar is unchanged** (it is the commit node, not a column).

**D5 — Compact mode is a geometry PRESET** applied inside `effectiveMetrics` when `graph.compact` is
true: it overrides row/node/pill/font geometry below the comfortable-mode slider ranges (rowHeight 22,
avatarRadius 8, denser pills/fonts) and ignores those sliders while active; `laneWidth` still honors its
slider (horizontal density is independent). One boolean toggle.

**D6 — Verified badge = reserved slot + faint unlit glyph now, lit by P58.** A fixed `badgeSlot` box
sits immediately left of the SHA text (inside the SHA column); v1 draws a **faint neutral hollow glyph**
(muted `text3`) so the position is visible/harness-testable but carries no meaning. P58 replaces the
glyph with verified/unverified/unknown states — a pure draw swap, no layout change. Slot exists only
when the SHA column is shown (§12 OQ3 for reserve-only vs faint-glyph).

**D7 — Remove the dead `dotRadius` pref end to end.** The slider was already deleted (P11d); the field
is a confirmed no-op (nothing reads `m.dotRadius`; the WIP row uses a literal `4`). serde has no
`deny_unknown_fields`, so old `settings.json` files carrying `dotRadius` still load (key ignored) —
removal is back-compat safe, pinned by a new test.

---

## 1. Module boundaries / files

**New (small, focused)**
- `src/graph/rightColumns.ts` — pure right-column layout model + SHA/badge/author/date geometry consts.
- `src/graph/dates.ts` — pure `shortSha`, `formatAbsolute` (+ `relativeDate` may move here; see §5).
- `src/graph/refLabels.ts` — the ref-label subsystem **extracted** from `draw.ts` (mechanical move,
  §7.1) so `draw.ts` returns under the 500-line limit; the ahead/behind chip is added here in P51c.
- `src/graph/drawRowText.ts` — the per-row text pass (summary + right columns), **extracted** from
  `draw.ts` pass 5 (§6.4), so the new column logic does not inflate `draw.ts`.

**Edited**
- `crates/bonsai-core/src/graph.rs` — `GraphNode.committer_ts`; populate; doc comment.
- `src-tauri/src/settings.rs` — `GraphPrefs` new toggle fields + `GraphDateBasis` enum; remove
  `dot_radius` + `DOT_RADIUS_MIN`/`MAX`; update `Default`/`clamp_graph_prefs`/tests; add back-compat test.
- `src/ipc/types.ts` — `GraphNode.committerTs`; `GraphPrefs` new fields + `GraphDateBasis`; drop `dotRadius`.
- `src/graph/metrics.ts` — `effectiveMetrics` reads `compact`; compact preset consts; `FONT_MONO`; drop
  `dotRadius` from `METRICS`/`MetricKnob`/`EffectiveMetrics`.
- `src/graph/draw.ts` — `drawGraph` gains a `GraphDisplayOptions` param; delegates ref labels →
  `refLabels.ts` and the text pass → `drawRowText.ts`; date-basis feeds the date column.
- `src/graph/GraphCanvas.tsx` — thread `display` opts + `branchStats`; date (+ optional SHA) hover
  tooltip; imports move to `refLabels.ts`.
- `src/graph/colors.ts` — (optional) no new CSS var; ahead/behind + badge reuse `text2`/`text3`.
- `src/components/SettingsPanel.tsx` — GRAPH section toggles + date-basis control; remove dead comment.
- `src/components/WorkspaceGraphPane.tsx`, `src/components/RepoWorkspace.tsx` — thread `display` +
  `branchStats` into `<GraphCanvas>`; `RepoWorkspace` derives both from `graphPrefs`/`branches`.
- `src/App.tsx` — drop `dotRadius: 4` from its graph-prefs fallback (L135).
- `src/ipc/mock/persistence.ts` — `DEFAULT_UI_SETTINGS.graph` new fields, drop `dotRadius`; clamp/parse.
- `src/ipc/mock/handlers/session.ts` — `setUiSettings` graph passthrough (whole-struct already; verify).
- `src/ipc/fixtures/graph.ts` + `src/ipc/fixtures/graph20k.ts` (and any detached/prepend variants) —
  add `committerTs` to every node.
- `styles.css` — GRAPH-section toggle rows / segmented control reuse (no canvas colors needed).

---

## 2. Backend — `crates/bonsai-core/src/graph.rs`

Add one field to `GraphNode` (after `ts`):

```rust
    /// Author commit time, seconds since epoch (UTC).
    pub ts: i64,
    /// Committer commit time, seconds since epoch (UTC). P51: powers the
    /// author-vs-committer date basis toggle. Often == `ts` (rebases/amends
    /// differ). Additive; frontend defaults to the author basis.
    pub committer_ts: i64,
```

Populate in the node emit (§graph.rs L486-495), reading the committer alongside the existing author:

```rust
let author = commit.author();
let committer = commit.committer();
nodes.push(GraphNode {
    // ...unchanged fields...
    ts: author.when().seconds(),
    committer_ts: committer.when().seconds(),
});
```

**Test impact:** the E-series tests assert `.lane`/`.parents`/edges/`lane_count` via accessors, not
whole-`GraphNode` literals, so they are unaffected; the `determinism` test (full struct equality across
two runs) still holds. No new graph test required, but the tester should add one assertion that
`committer_ts` is populated (non-zero for the fixture commits). Serialization stays a single command
response (M2 §2.7 unchanged).

---

## 3. Settings model — `src-tauri/src/settings.rs`

### 3.1 New enum

```rust
/// Which timestamp the graph's date column + relative/absolute date use (P51).
/// Pure UI preference; no Git effect. Author is the M2 baseline behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphDateBasis {
    #[default]
    Author,
    Committer,
}
```

### 3.2 `GraphPrefs` — add toggles, remove `dot_radius`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GraphPrefs {
    // dot_radius REMOVED (P51 D7 — dead no-op field).
    pub avatar_radius: u32,
    pub row_height: u32,
    pub lane_width: u32,
    /// P51: show the short-SHA column (+ verified-badge slot). Default true.
    pub show_sha: bool,
    /// P51: show the optional full author-NAME text column. Default false
    /// (the avatar already conveys author; the name is the clutter-iest column).
    pub show_author: bool,
    /// P51: show the date column. Default true (M2 baseline showed it always).
    pub show_date: bool,
    /// P51: which timestamp the date column/tooltip use. Default Author.
    pub date_basis: GraphDateBasis,
    /// P51: ahead/behind chip on local-branch-tip pills. Default true (renders
    /// only on diverged branches — low clutter, high value).
    pub show_ahead_behind: bool,
    /// P51: compact (denser) rows preset. Default false.
    pub compact: bool,
}

impl Default for GraphPrefs {
    fn default() -> Self {
        GraphPrefs {
            avatar_radius: 10,
            row_height: 32,
            lane_width: 16,
            show_sha: true,
            show_author: false,
            show_date: true,
            date_basis: GraphDateBasis::Author,
            show_ahead_behind: true,
            compact: false,
        }
    }
}
```

Remove `DOT_RADIUS_MIN`/`DOT_RADIUS_MAX`. `clamp_graph_prefs` clamps only geometry and **carries the
new fields through with struct-update** (a dev who forgets `..g` silently resets toggles on every
load/save — call this out):

```rust
pub fn clamp_graph_prefs(g: GraphPrefs) -> GraphPrefs {
    GraphPrefs {
        avatar_radius: g.avatar_radius.clamp(AVATAR_RADIUS_MIN, AVATAR_RADIUS_MAX),
        row_height: g.row_height.clamp(ROW_HEIGHT_MIN, ROW_HEIGHT_MAX),
        lane_width: g.lane_width.clamp(LANE_WIDTH_MIN, LANE_WIDTH_MAX),
        ..g // toggles + date_basis pass through unclamped
    }
}
```

### 3.3 Tests
Update the three existing tests that build/assert `GraphPrefs` with `dot_radius`
(`clamp_auto_fetch_and_graph_prefs_clamp_ranges`, `auto_fetch_and_graph_roundtrip`,
`corrupt_auto_fetch_and_graph_on_disk_are_clamped_on_load`) to drop `dot_radius`/`DOT_RADIUS_*`. Add:

- `graph_prefs_toggles_roundtrip` — non-default toggles + `Committer` basis round-trip; raw JSON shows
  `"showSha"`, `"dateBasis": "committer"`, `"compact"`.
- `old_graph_prefs_with_dot_radius_ignored` — a legacy `graph` object containing `"dotRadius": 4` and
  none of the new keys loads with `dot_radius` gone (ignored, no error) and every new toggle at its
  default (`showSha=true`, `showAuthor=false`, `showDate=true`, basis Author, `showAheadBehind=true`,
  `compact=false`) — pins D7 back-compat + the per-field `#[serde(default)]` fallback.

### 3.4 `ui_settings.rs`
**No change** — `UiSettings`/`UiSettingsPatch` already carry `graph: GraphPrefs` / `Option<GraphPrefs>`,
and `apply_patch` already does `s.graph = clamp_graph_prefs(graph)`. Confirm it still compiles after the
struct grows (it will).

---

## 4. TypeScript types + mock

### 4.1 `src/ipc/types.ts`
- `GraphNode`: add `committerTs: number;` after `ts` (doc: committer time, seconds since epoch).
- New: `export type GraphDateBasis = 'author' | 'committer';`
- `GraphPrefs`: remove `dotRadius`; add:
```ts
export interface GraphPrefs {
  avatarRadius: number;
  rowHeight: number;
  laneWidth: number;
  /** P51: short-SHA column (+ verified-badge slot). Default true. */
  showSha: boolean;
  /** P51: optional full author-name text column. Default false. */
  showAuthor: boolean;
  /** P51: date column. Default true. */
  showDate: boolean;
  /** P51: which timestamp the date column/tooltip use. Default 'author'. */
  dateBasis: GraphDateBasis;
  /** P51: ahead/behind chip on branch-tip pills. Default true. */
  showAheadBehind: boolean;
  /** P51: compact (denser) rows. Default false. */
  compact: boolean;
}
```
`UiSettings`/`UiSettingsPatch` are **unchanged** (they already reference `GraphPrefs`).

### 4.2 Mock (`src/ipc/mock/persistence.ts`)
- `DEFAULT_UI_SETTINGS.graph` (L83): drop `dotRadius`; add `showSha: true, showAuthor: false,
  showDate: true, dateBasis: 'author', showAheadBehind: true, compact: false`.
- Clamp helper (L151): remove the `dotRadius` clamp line + the `DOT_RADIUS_MIN/MAX` import; carry the
  toggles/basis through unchanged (spread).
- Tolerant parse (L198-213): remove the `dotRadius` read; add tolerant reads for each new field
  (`typeof g.showSha === 'boolean' ? g.showSha : DEFAULT…`; basis: `g.dateBasis === 'committer' ?
  'committer' : 'author'`).
- `src/ipc/mock/handlers/session.ts` `setUiSettings`: `graph` is a whole-struct passthrough — verify it
  echoes `patch.graph ?? current.graph` (no per-field mapping needed).

### 4.3 Fixtures
`src/ipc/fixtures/graph.ts` (`buildMockGraph`, and any detached/prepend variants) + `graph20k.ts`
(`generateLayout20k`): add `committerTs` to every node. Set `committerTs = ts` for most rows, but for
**2-3 rows set `committerTs = ts + 3600`** so the author-vs-committer toggle produces a visible
difference in the harness (a rebased/amended commit).

---

## 5. Geometry — `src/graph/metrics.ts`

- Remove `dotRadius: 4` from `METRICS`; drop `'dotRadius'` from `MetricKnob` and from the
  `effectiveMetrics` overlay body + `EffectiveMetrics` widening.
- Add SHA/badge/mono consts to `METRICS`:
```ts
  shaColWidth: 54,        // 7 mono chars right-aligned
  shaFont: '12px',        // sized/weight prefix; FONT_MONO appended at draw time
  badgeSlotWidth: 14,     // verified-badge box, left of the SHA text
  badgeGap: 4,            // gap between badge slot and SHA text
  authorColWidth: 120,    // revived from @deprecated — the optional author-name column
```
- Add `export const FONT_MONO = 'ui-monospace, "Cascadia Code", "Consolas", monospace';`.
- **Compact preset + signature.** `effectiveMetrics` reads `g.compact` (no call-site signature churn —
  `RepoWorkspace` L144 still passes `graphPrefs`; compact lives inside it). Widen the param type to the
  full `GraphPrefs` shape (or `{ avatarRadius; rowHeight; laneWidth; compact }`). When `compact`, overlay:
```ts
const COMPACT = {
  rowHeight: 22, avatarRadius: 8, avatarBgRingExtra: 1, pillHeight: 15,
  textGap: 8, avatarFont: '600 10px', summaryFont: '400 12px', metaFont: '400 11px',
  shaFont: '11px',
} as const;
```
  Compact **overrides** rowHeight/avatarRadius (+ the two avatarRadius-derived ring radii, which already
  derive from `avatarRadius`)/pill/fonts/textGap and **ignores** the geometry sliders for those; `laneWidth`
  still honors the slider. Comfortable mode is unchanged. `metricsVersion` already bumps when `graphPrefs`
  changes (compact is inside it) — confirm the container's bump dep covers it.

---

## 6. Draw layer

### 6.1 `GraphDisplayOptions` (new, threaded into `drawGraph`)
`drawGraph` gains a param `display: GraphDisplayOptions` (do NOT overload `Interaction` — keep hover/
selection separate from persisted prefs):
```ts
export interface GraphDisplayOptions {
  showSha: boolean;
  showAuthor: boolean;
  showDate: boolean;
  dateBasis: 'author' | 'committer';
  showAheadBehind: boolean;
  /** name → ahead/behind for local branches (from BranchInfo). Empty map ok. */
  branchStats: ReadonlyMap<string, { ahead: number | null; behind: number | null }>;
}
```
Passes 1-4 (clear, row bg, edges, avatar/rings/match-ring) are **unchanged**. Only pass 5 (text) and the
ref-band label pass consume `display`. `compact` is NOT here — it is already baked into `EffectiveMetrics`.

### 6.2 `src/graph/rightColumns.ts` (pure column model)
```ts
export interface ColRect { leftX: number; rightX: number; width: number; }
export interface RightColumns {
  author: ColRect | null;
  sha: ColRect | null;      // includes the badge slot at its LEFT
  date: ColRect | null;
  /** left edge available to the summary (right end of the summary flex zone). */
  summaryEndX: number;
}
/** Pack enabled columns against the right edge in fixed order (author, sha, date;
 *  date rightmost). `effRight` = vp.width - (rightInset ?? 0). Pure; used by BOTH
 *  the draw pass and the hover hit-test (single source of truth). */
export function computeRightColumns(
  effRight: number,
  display: GraphDisplayOptions,
  m: EffectiveMetrics,
): RightColumns;
```
Algorithm (right→left): `cursor = effRight - m.colGap`; for each enabled column in order **date, sha,
author** (rightmost first): `rightX = cursor; width = colWidth(col); leftX = rightX - width; cursor =
leftX - m.colGap`. `colWidth`: date → `m.dateColWidth`; sha → `m.badgeSlotWidth + m.badgeGap +
m.shaColWidth`; author → `m.authorColWidth`. `summaryEndX = cursor`. Disabled → `null` and no space
reserved (that is how show/hide reclaims width with no overlap).

### 6.3 `src/graph/dates.ts` (pure)
```ts
export function shortSha(id: string, len = 7): string;          // id.slice(0, len)
/** Locale absolute timestamp for the hover tooltip, e.g. "2026-08-07 14:32".
 *  Deterministic fixed format (NOT toLocaleString) so it is unit-testable. */
export function formatAbsolute(tsSeconds: number): string;      // "YYYY-MM-DD HH:mm"
```
`relativeDate` MAY move here from `draw.ts` (re-export from `draw.ts` to avoid breaking its test import),
or stay — implementer's choice; keep its unit test green either way.

### 6.4 `src/graph/drawRowText.ts` (extracted pass 5)
Move the per-row text rendering out of `draw.ts` into `drawRowText(ctx, row, node, y, cols, display,
theme, m)` where `cols = computeRightColumns(...)` is computed **once** before the row loop. Per row:
- **summary** — `summaryFont`/`text1`, left at `summaryStartX(laneCount)`, truncated to
  `cols.summaryEndX - m.colGap - summaryStartX`.
- **author** (if `cols.author`) — `metaFont`/`text3`, right-aligned at `cols.author.rightX`,
  `truncateToWidth(node.author, m.authorColWidth)`.
- **sha** (if `cols.sha`) — `FONT_MONO`/`text2`, right-aligned at `cols.sha.rightX`,
  `shortSha(node.id)`; **verified-badge stub** (§6.5) drawn in the slot at the column's left.
- **date** (if `cols.date`) — `metaFont`/`text3`, right-aligned at `cols.date.rightX`,
  `relativeDate(display.dateBasis === 'committer' ? node.committerTs : node.ts, now)`.

`draw.ts` pass 5 becomes: compute `cols` once, then loop calling `drawRowText`. This extraction is what
keeps `draw.ts` under the 500-line limit after the ref-label extraction (§7.1).

### 6.5 Verified-badge stub (D6)
In `drawRowText`, when `cols.sha` exists, draw a faint hollow glyph centered in the badge slot
(`cx = cols.sha.leftX + m.badgeSlotWidth/2`, `cy = rowY`): a small circle/shield outline, `strokeStyle =
theme.text3`, ~1px, radius ~4. No fill, no meaning. Add a one-line comment: `// P58 lights this
(verified/unverified/unknown); v1 is an unlit placeholder`. No `GraphNode` verification field in P51.

---

## 7. Ahead/behind on branch-tip rows (P51c)

### 7.1 Extract `refLabels.ts` first (mechanical, no behavior change)
Move `RefEntity`, `PillStyle`, `groupRefs`, `entityStyle`, `layoutRefLabels`, `drawRefLabelAt` from
`draw.ts` into `src/graph/refLabels.ts`; update imports in `draw.ts` and `GraphCanvas.tsx`. This is a
pure move — build green, zero visual change — and is the primary reason `draw.ts` drops back under 500.

### 7.2 Chip data (frontend only)
`RepoWorkspace` derives `branchStats: Map<string, {ahead, behind}>` from its existing `branches`
(`BranchInfo[]`), keyed by `BranchInfo.name`, memoized on `branches`. Threaded via `WorkspaceGraphPane`
→ `GraphCanvas` → `GraphDisplayOptions.branchStats`.

### 7.3 Rendering (in `refLabels.ts`)
`layoutRefLabels` gains awareness of `display.showAheadBehind` + `branchStats` so it can **reserve**
chip width during layout (otherwise later pills overlap the chip). For a `localBranch` entity whose
`branchStats[name]` has non-null `ahead`/`behind` with `ahead+behind > 0`, reserve `chipWidth`
(measured: `"↑{a} ↓{b}"` in `metaFont`) after the pill; the paint pass draws the chip immediately right
of that pill in `theme.text2` (ahead) / `theme.text3` (behind) or a single muted string. The chip counts
toward the fixed-band budget → it can be pushed into the `+n` overflow when the band is full (acceptable
clutter fallback). When the toggle is off, no reservation, no draw (space reclaimed). Render for **each**
qualifying local-branch entity on the row (usually one); §12 OQ1 flags limiting to the HEAD branch if
noisy.

---

## 8. Hover / tooltip — `GraphCanvas.tsx`

Add a `TooltipState` kind for the date (and optionally SHA) column:
```ts
  | { kind: 'date'; lines: string[]; anchor: Rect }   // absolute authored + committed
```
Extend `computeHoverTarget` (after the avatar + ref-band hit tests, before returning null): recompute
`cols = computeRightColumns(effRight, display, m)` (same pure helper — cheap) and, if `cols.date` exists
and `x` is within `[cols.date.leftX, cols.date.rightX]`, return:
```
lines = [ `Authored  ${formatAbsolute(node.ts)}`, `Committed ${formatAbsolute(node.committerTs)}` ]
anchor = { left: cols.date.leftX, top: cy - m.pillHeight/2, width: cols.date.width, height: m.pillHeight }
```
Render reuses the existing multi-line branch (`tooltip.kind === 'overflow' ? lines.map(...)` — generalize
that check to `'overflow' || 'date'`). `sameTarget` gains a `date` arm (compare `lines.join('␟')`).
The inline date stays **relative** — only the hover shows absolute (requirement). Threading: `display`
enters `GraphCanvas` as a prop → `propsRef`; `computeHoverTarget` reads it from `propsRef.current`.
Optional SHA-hover → full 40-char oid tooltip is a cheap nice-to-have (§12 OQ5); not required for v1.

`GraphCanvasProps` gains `display: GraphDisplayOptions;` (passed straight into `drawGraph` and
`computeHoverTarget`). Bump the existing repaint on `display` change (add `display` to the paint effect
deps, or a `displayVersion` like `metricsVersion` — recommend deriving from `graphPrefs` identity, which
already changes by reference on any pref edit).

---

## 9. SettingsPanel — GRAPH section (`SettingsPanel.tsx`)

Keep the three geometry sliders (avatar/row/lane). Remove the dead-`dotRadius` comment (L367-370). Add,
below the sliders, a "Row details" group of toggles + one segmented control, each wired
`onChange({ graph: { ...graph, <field>: v } })` (whole-struct patch, matching the existing sliders):
- Checkbox **Short SHA** → `showSha`.
- Checkbox **Author name** → `showAuthor`.
- Checkbox **Date** → `showDate`.
- Segmented **Date basis: Author | Committer** → `dateBasis` (disabled/greyed when `showDate` is false —
  it only affects the shown date; still applies to the hover tooltip, so keep enabled — implementer note:
  keep enabled).
- Checkbox **Ahead/behind on branches** → `showAheadBehind`.
- Checkbox **Compact rows** → `compact`.
Reuse existing settings-row/checkbox/segmented classes (mirror the Appearance section's controls). This
is ~40 lines added to an existing ~400-line section — under the limit; if it pushes over, extract a
`SettingsGraphSection.tsx` (P49 precedent) — recommend extracting for cleanliness.

---

## 10. Threading (containers)

- `RepoWorkspace`: build `display: GraphDisplayOptions` from `graphPrefs` (`showSha`, `showAuthor`,
  `showDate`, `dateBasis`, `showAheadBehind`) + `branchStats` from `branches` (both `useMemo`). Pass
  `display` to `WorkspaceGraphPane` (new prop) → `GraphCanvas`. `metrics` already flows via
  `effectiveMetrics(graphPrefs)` (now compact-aware).
- `WorkspaceGraphPane`: add `display: GraphCanvasProps['display']` to its props; forward to `<GraphCanvas
  display={display} … />`.
- `App`: `graphPrefs`/`UiSettings` fallback — drop `dotRadius: 4` (L135); ensure new toggle defaults are
  present wherever a `GraphPrefs` literal is constructed (else `tsc` fails once the field is required).

---

## 11. Sub-increment split + acceptance

### P51a — Data model, settings, geometry plumbing, mock (no visual change)
Scope: `graph.rs` `committer_ts` + populate; `settings.rs` `GraphPrefs` toggles + `GraphDateBasis` +
`dotRadius` removal + clamp/Default/tests; `types.ts` (`committerTs`, `GraphPrefs`, `GraphDateBasis`,
drop `dotRadius`); `metrics.ts` (`effectiveMetrics` compact + consts + `FONT_MONO` + drop `dotRadius`);
mock `persistence.ts`/`session.ts`; fixtures `committerTs`. `App.tsx` L135 fallback.
**Acceptance:** (1) `cargo test -p bonsai-core` + settings tests green incl. `graph_prefs_toggles_roundtrip`
and `old_graph_prefs_with_dot_radius_ignored`; `cargo clippy -- -D warnings` clean; `generate_handler!`
still lists 129. (2) `tsc`/`pnpm build` clean; no `dotRadius` symbol remains (grep). (3) Harness console:
`await ipc.getUiSettings()` returns `graph` with the six new fields at defaults and no `dotRadius`;
`await ipc.setUiSettings({ graph: { ...g, compact: true, dateBasis: 'committer', showSha: false } })`
round-trips. (4) **No visual change** (compact off, draw does not yet consume toggles): a before/after
graph screenshot is identical.

### P51b — Right-column system + SHA + verified-badge stub + date basis + absolute-date hover + compact
Scope: `rightColumns.ts`, `dates.ts`, `drawRowText.ts` (extract pass 5); `refLabels.ts` (extract §7.1 —
do it here so `draw.ts` lands under 500); `draw.ts` `GraphDisplayOptions` + pass-5 delegation + badge
stub; `GraphCanvas.tsx` thread `display` + date hover; `WorkspaceGraphPane`/`RepoWorkspace` thread
`display` (branchStats may be an empty map until P51c); SettingsPanel toggles for SHA/author/date/basis/
compact (ahead/behind toggle may ship here too, inert until P51c).
**Acceptance:** (1) `pnpm build` clean; `draw.ts`, `GraphCanvas.tsx` under the ~500-line soft limit
(report line counts). (2) Harness screenshot: SHA column visible with a faint badge glyph to its left;
date column relative inline; hovering the date shows a two-line absolute tooltip (Authored/Committed);
toggling `dateBasis` to Committer changes the relative date + tooltip on the fixture rows where
`committerTs != ts`. (3) Toggling `showSha`/`showAuthor`/`showDate` off via settings **removes** the
element AND reclaims its width (summary widens; no overlap) — verify via screenshot + a
`javascript_tool` read of the canvas width math or a DOM settings round-trip. (4) `compact: true` yields
visibly denser rows (rowHeight 22) with no clipping; passes 1-4 (edges/avatars/rings/match-ring) still
correct while scrolling; frame timing unregressed. (5) `relativeDate`/`formatAbsolute`/`shortSha`/
`computeRightColumns` unit-tested (pure).

### P51c — Ahead/behind on branch-tip rows
Scope: `RepoWorkspace` `branchStats` derivation; `refLabels.ts` chip reservation + paint; wire the
`showAheadBehind` toggle end to end (if not already in P51b).
**Acceptance:** (1) `pnpm build` clean. (2) Harness (fixture branch tips carry ahead/behind in the mock
`BranchInfo`): a diverged branch pill shows `↑a ↓b`; a non-diverged / no-upstream branch shows none;
toggling `showAheadBehind` off removes the chip and reclaims band budget (no `+n` regression on
unrelated rows). (3) Ref-band overflow (`+n`) still correct at every scroll position with the chip
present. (4) No file over the limit (`refLabels.ts` split keeps `draw.ts` small).

(P51b is the largest; if its diff is too big for one review, cut at
**P51b-1** = extractions + `rightColumns` + SHA + badge + compact, **P51b-2** = date basis + absolute
hover + author column. Flagged, not mandated.)

---

## 12. Acceptance criteria + user-checklist

**AI gate (orchestrator verifies):** all three increments' acceptance above — cargo/clippy/tsc/build
green; the pure helpers unit-tested; harness screenshots proving each element draws, each toggle hides/
shows AND reclaims space, compact densifies, the date tooltip shows absolute times, ahead/behind renders
on diverged tips; `draw.ts`/`GraphCanvas.tsx` under the size limit; no `dotRadius` remains.

**USER CHECKPOINT (native, cannot be AI-judged — perception of density/legibility):** see
`docs/contracts/P51-user-checklist.md`. In `pnpm tauri dev` against a real repo: (a) the row detail set
feels *decluttered*, not busy, at defaults; (b) compact mode is legible (text not cramped) on the user's
display/DPR; (c) the SHA is readable and the badge slot is unobtrusive; (d) the date tooltip appears at a
comfortable hover and reads correctly; (e) ahead/behind on the checked-out branch matches the sidebar;
(f) toggling each element on/off feels instant with no layout jank on a large history.

---

## 13. Ambiguities / decisions flagged (recommendation in bold)

- **OQ1 — Ahead/behind placement.** **Recommend LEFT ref band, attached to the branch pill** (D3;
  semantically correct, matches every Git GUI). Alternative: a dedicated right column (cleaner layout but
  detached when >1 branch shares a row). If the reviewer finds the `layoutRefLabels` chip-reservation
  change too invasive, fall back to a right column via `rightColumns.ts` (additive). Also: render for
  **every** qualifying local branch on a row vs **HEAD branch only** — recommend every-qualifying (rare
  to have >1), flag if noisy.
- **OQ2 — Column reorder.** **Recommend defer (show/hide only for v1).** Fixed order author→SHA→date. The
  model iterates an ordered list, so reorder later = persist an order array + a settings control (no draw
  change). Reorder now costs a drag/up-down UI + persistence for 3 columns — not justified.
- **OQ3 — Verified-badge stub visibility.** **Recommend a faint unlit hollow glyph** (D6 — visible/
  testable, obviously a slot). Alternative: reserve-only (draw nothing) for maximum declutter until P58.
  Confirm.
- **OQ4 — Committer NAME.** **Recommend NOT adding it** — the toggle is author-vs-committer *date*; the
  optional author-name column stays author-only. Adding committer name is +bytes and a second name
  concept for little value. Confirm, or ask for a name-basis toggle too.
- **OQ5 — SHA default + SHA-hover.** **Recommend `showSha` default TRUE** (the single most-requested
  missing element, compact ~72px incl. badge). If the clutter principle should win, default FALSE —
  confirm. SHA-hover → full-oid tooltip is a cheap add; recommend include, low priority.
- **OQ6 — `committer_ts` add vs defer** (D2). **Recommend ADD** (one `i64`, satisfies the required
  choice). Defer alternative = ship the toggle author-only now, which does not meet the requirement.
  Confirm.
- **OQ7 — Compact as preset vs scale** (D5). **Recommend preset override** (predictable, fits dense rows).
  Alternative: multiply the user sliders by ~0.7 (keeps relative sizing) — more complex, less
  predictable. Confirm.
