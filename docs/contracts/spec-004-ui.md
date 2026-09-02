# spec-004 — Fold Linear Runs — UI Contract

Companion to `docs/specs/004-graph-fold-linear/spec.md` + `plan.md`. Backend contract (locked):
Rust emits `FoldSpan { start, count, lane }` metadata on `done`/`GraphLayout`; **there is no folded
wire node**. The fold pill is a **frontend display row** produced by `src/graph/foldModel.ts`
(`displayToModel` / `modelToDisplay`); expansion is transient local state in `useGraphFold`.
This contract is written entirely against **display rows** (confirming the plan's FLAG 1).

**No new theme tokens.** Every colour is the per-style lane palette (ui-ref §5 / §5.1) or an
existing `--*` var. Row geometry follows the graph metrics canon (ui-ref §4): the graph pane is
outside the panel-density scopes, but `METRICS`/`COMPACT` cozy/compact numbers apply as stated
below. One canon amendment ships with this spec: ui-ref §4.1 row counts switch to display rows
(§7 here) — updated in `ui-reference.md` in the same pass.

> **Reconciliation note — 2026-09-02.** §3 and §7 originally framed the scroller as a
> treegrid/row-semantics widget (`role="row"` shadow nodes, `aria-rowcount`). P95's accessibility
> rework landed after this contract was written and settled the scroller as
> `role="group"` + `aria-activedescendant`, with `role="grid"`, `aria-rowcount`, `role="row"` and
> `aria-rowindex` all **forbidden** (they require a grid/table role). The passages below were
> revised on 2026-09-02 to match the shipped, user-approved model; everything else spec-004 says
> — keyboard reachability of fold-pill rows, their accessible names, and display-index ordinals —
> is unchanged and still true. **`docs/contracts/ui-reference.md` §4.1 is the canonical source**
> for the scroller's ARIA surface; do not "restore" the earlier grid wording from this file.

Files (per plan): painting in **`src/graph/drawFold.ts`** (draw.ts is at cap — dispatch only);
model in `foldModel.ts`; state in `useGraphFold.ts`; toggle UI lands in the existing spec-003
surfaces (`GraphFilterPopover.tsx`, `SettingsGraphDeclutterSection.tsx`, `useGraphFilter.ts` or
a sibling — no new component files needed for the toggle).

---

## 1. The fold-pill display row

One display row per collapsed span. Row height is **uniform** with commit rows — 32px cozy /
22px compact (`rowHeight`) — so virtualization math is untouched.

```
| ref band (180px, empty) | lanes …  ┆ (dashed)  … | ⋯ 37 commits          |
                                     ┆  ← span.lane x                       |
```

- **Lane connector.** A **dashed vertical segment** on `span.lane`'s x, full row height,
  2px stroke (`edgeWidth`), dash pattern `[3, 4]` CSS px, round caps, colour = the lane's colour
  from the **active palette** (standard `LANE_COLORS_DARK/_LIGHT` or Bonsai
  `LANE_COLORS_BONSAI_*` — `drawFold.ts` receives the resolved palette, never picks its own).
  The dash is the "continuous but elided" cue. Plan rule 4 guarantees **no other lane's edge
  crosses a folded span**, so the fold row paints exactly one lane — nothing else needs bridging.
  In the **Bonsai style** the connector does not taper or sway (it is not a bezier segment;
  sway/seasons never apply — §5.1 locks lane hues, so all contrast claims below hold there too).
- **The pill.** Reuses the §6 local-branch pill recipe verbatim, in the span's lane colour:
  bg = lane colour at 18% alpha over the graph backdrop, text + 1px border = lane colour,
  radius 999px, `pillFont` 11px/600, height 18px cozy / 15px compact, padding 2px 8px.
  Positioned at `lane x + laneWidth` (left edge 8px right of the connector), vertically centred.
  Label: `⋯ {N} commits` — N = `span.count`, locale-grouped (`1,204`), never abbreviated.
  Singular never occurs (`MIN_FOLD_RUN = 5`). Max width 160px (canon) — the count always fits.
  **Contrast:** identical recipe to local-branch pills, already canon-cleared in both themes and
  both styles (§5 lane colours ≥4.6:1 vs `--bg-0`; §5.1 Bonsai lanes ≥5.0:1 dark / ≥5.3:1 light
  vs the paper/soil backdrop — the 18% tint barely shifts the effective background, and the text
  is the lane colour itself, same as every existing branch pill).
- **No avatar, no summary, no meta columns, no ref band content** — the emptiness is the
  distinction from a commit row. The ref band and forge column stay blank for this row, and it
  never collides with the right-column pack (the pill lives in the graph-lane region).
- **States** (painted by `drawFold.ts` from flags passed in, same pattern as `drawWipRow`):
  - Hover (whole row is the hit target, cursor `pointer`): row bg `--bg-2` (the existing
    hover-row fill) + pill bg tint raised 18% → 28%.
  - Active/pressed: pill tint 32% for the pressed frame; no transform.
  - Keyboard-active (active descendant, §3): row bg `--bg-2` plus a 2px `--accent` bar along the
    row's left edge inside the scroller. **This active-row paint is new in this contract** (the
    graph has never had an active-descendant row that is not the selection); it deliberately
    reuses the hover fill + an accent shape rather than the solid `--selection` fill, so it never
    reads as a selected commit.
  - Selection-carrying (§6): the pill additionally gets a 1.5px `--accent` outer ring
    (rounded-rect, 2px outside the pill border) — the selected commit is *inside* this span.
- **Hit target.** The full row width × row height (≥24px cozy; 22px compact matches commit-row
  precedent) toggles the span. No sub-targets on the collapsed row.
- **Motion.** None. Expand/collapse is an instant re-layout (spec-003's "no filtered/unfiltered
  transitions" lock applies — same category of change). The one-time collapse when spans arrive
  at stream `done` is likewise instant.

## 2. Re-collapse affordance (expanded run)

**Decision: a persistent boundary pill, not a hover gutter affordance** (canvas hover-only
affordances are undiscoverable and keyboard-invisible).

While a span is in the expanded set, the **first revealed row of the run** (model row
`span.start`) renders a compact collapse pill **in the left ref-column band** (the 180px band,
§6), right-aligned where that row's ref pills would end — guaranteed vacant, because every row
inside a span is refs-empty by fold rule 1. Placing it in the ref band (not at `lane x +
laneWidth`) avoids any collision with the row's summary/meta columns, which are packed once per
frame by `computeRightColumns` and cannot shift for one row; pills already live in that band
idiomatically. Same recipe as §1, same lane colour, label **`Collapse {N}`** (e.g.
`Collapse 37`; a bare glyph like `⌃ {N}` was rejected as cryptic). It is a distinct hit target
within the row: clicking it re-collapses; clicking anywhere else on the row selects the commit
normally. Its hit box is the pill rect expanded to ≥24×24. Hover: tint 18%→28%, cursor
`pointer`; the label is self-describing, no tooltip required.

Rationale: the boundary row is where the user's eye already is after expanding in place, and it
pairs 1:1 with the ArrowLeft keyboard path (§3).

## 3. Keyboard & screen-reader semantics (plan FLAG 3 — decided: **land, don't skip**)

Skipping would make expansion keyboard-unreachable. The scroller stays the ui-ref §4.1 composite
widget exactly as P95 specced it — `.graph-scroll` with `tabIndex={0}`, `role="group"`,
`aria-label="Commit graph"`, `aria-describedby` → the sr-only keyboard hint, and
`aria-activedescendant`. No grid/treegrid role, no `aria-rowcount`, no `role="row"` or
`aria-rowindex`: those are only meaningful under a grid/table role. `aria-activedescendant` is
supported on `role="group"` (ARIA 1.2) and its IDREF resolves to a real element, because this
spec adds a single sr-only `<div id="graph-row-{i}">` that is re-rendered per active display row
and carries that row's accessible name (plus `aria-expanded` / `aria-selected`). The selection
model stays exactly as today for commit rows; the pill row is the only row type where "active"
and "selected" diverge:

- **ArrowUp/Down onto a commit row selects it** (current behaviour, unchanged — active ==
  selected on commit rows). ArrowUp/Down onto a **fold-pill row** lands there: it updates
  `aria-activedescendant` (and paints the §1 keyboard-active state) but does **not** change the
  commit selection, which stays on its oid.
- **Enter / Space / ArrowRight** on a pill row: expand. The next ArrowDown/ArrowUp then selects
  the adjacent commit row as normal — there is never an active-but-unselected *commit* row;
  after expand the active position simply resumes ordinary selection-on-arrival semantics from
  the pill's display index.
- **ArrowLeft** anywhere on a row inside an expanded run (including its boundary row): collapse
  the run; the active row moves to the restored pill row. On rows not inside an expanded run,
  ArrowLeft is a no-op (reserved).
- **Enter on the boundary collapse pill** is not separately reachable — ArrowLeft is the
  keyboard collapse path; the pill is the mouse path.
- PageUp/Down / Home/End operate on display rows (they already will, via the mapping); if the
  landing display row is a pill, the land-don't-select rule above applies.
- **Accessible name** of a pill row (the sr-only active-descendant target element, no role):
  `"{N} commits folded. Press Enter to expand."`, with `aria-expanded="false"`
  (`true` never occurs — an expanded span has no pill row; the *boundary* row appends
  `", start of an expanded run of {N} commits. Press Left Arrow to collapse."` to its normal
  commit announcement). The §4.1 live announcer reads the pill row as:
  `"{N} commits folded. Row {d+1} of {D}."` (display indices, §7).
- **Command palette:** one action, group `action` — `Toggle fold linear runs`
  (keywords `fold collapse linear runs condense declutter graph`), enabled while a repo is open.
  No global shortcut (map is crowded — same call as spec-003; flag if the user wants one).

## 4. Toggle — joining the spec-003 declutter system

Fold is **frontend truth** (plan: `Meta` untouched) — the chip ORs in the local
`graphFoldLinear` setting. This is honest by the first-parent precedent: the chip reflects the
*requested* mode, not per-repo yield (first-parent on a merge-free repo already reads the same
way). "Zero spans found" is not a stale state and never triggers §2.2 warning styling.

1. **GraphFilterPopover** gains a third switch row, inserted **after** the First-parent row,
   before the divider (declutter switches group together; ref filters stay below):
   §12.3.1 Switch, label 13px `--text-1` `Fold linear runs`, help 12px `--text-2`:
   `Collapse stretches of plain commits — no branches, merges, or tags — into a single
   expandable row.` `aria-describedby` wired. Applies immediately (toggle-on re-requests per
   plan; toggle-off is instant and local).
2. **Chip label** (`activeSummary` grammar extends spec-003 §2): segment word **`Folded`**,
   joined with `·` after First-parent, before the ref summary —
   `Folded` / `First-parent · Folded` / `First-parent · Folded · Solo: main`. Existing 260px
   ellipsis + `title` rules absorb the longer strings. The chip's explanatory `title` sentence
   is unchanged (it already says commits are hidden).
   **Stale-fallback interaction:** the §2.2 warning chip concerns only the ref filter, and fold
   is frontend truth, so it must survive the stale swap. When stale is detected **while** fold
   and/or first-parent are on, the chip keeps the normal active recipe and shows the honest
   segments (`Folded`, `First-parent`); the leading glyph swaps to `⚠` in `--warning` and the
   stale sentence lives in the popover's section 3 (words remain the meaning carrier). Only when
   stale is the *sole* state does the full §2.2 warning chip render as specced. (Appending
   `Filter not applied` as a fourth label segment was rejected as overloaded.)
3. **Settings** — `SettingsGraphDeclutterSection.tsx` gains row 3, `graph.fold-linear`:
   Switch, label `Fold linear runs`, help:
   `Collapse long runs of plain commits into "⋯ N commits" rows you can expand in place.`
   Keywords: `fold collapse linear runs condense declutter graph`. Reset `↺` when on.
   Same value as the popover switch — one setting, two controls. Persisted globally
   (`graphFoldLinear`, default false), per plan.

## 5. Reveal auto-expand

`revealCommitByOid` landing inside a collapsed span: expand first (synchronous remap), then the
existing P84 reveal flash runs unchanged on the target row — **no new cue**. The expansion
itself is instant (§1 motion rule); the flash is the "here it is" signal and already honours
`prefers-reduced-motion` via its existing path.

## 6. Selection inside a run (AC6 + manual collapse)

- **Enabling fold with a selection inside a would-be span** (AC6): that span enters the expanded
  set before first folded paint (plan mechanism). No copy, no toast — the user sees their
  selection undisturbed.
- **Manually collapsing the run containing the selection: allowed.** Selection persists (keyed
  by oid/model row); the fold pill renders the selection-carrying ring (§1) so the hidden
  selection stays visible; the right panel keeps showing the selected commit (its model row
  never left the store). Expanding restores the normal selected-row paint. Arrow navigation
  from a hidden selection anchors at the pill row (nearest display row).

## 7. Canon amendment — ui-reference §4.1 (shipped with this contract)

With fold active, model-row counts lie to assistive tech. Active-descendant ids
(`graph-row-{d}`) and the announcer's `"Row {d+1} of {D}"` clause switch to **display rows**
(`displayRowCount`, display indices). With fold off, display == model, so nothing changes for
existing behaviour.

The row ordinal reaches the user **only** through `GraphSelectionAnnouncer`'s live-region
`"Row {d+1} of {D}"` clause — never through `aria-rowcount`, which is forbidden here (it needs a
grid/table role the scroller does not have; see the 2026-09-02 note at the top of this file and
`ui-reference.md` §4.1). What spec-004 *adds* to §4.1 is the per-active-row sr-only DOM node
that makes `aria-activedescendant` resolve to a real element. Fold-row visuals stay specced here
(a §4 pointer line added).

## 8. States matrix

| State | Rendering |
|---|---|
| Collapsed span | dashed connector + `⋯ N commits` pill (§1) |
| Hover / pressed / kbd-active | §1 tints; row bg `--bg-2` |
| Expanded run | commits in place; boundary row `Collapse N` pill in the ref band (§2) |
| Selection inside collapsed span | pill + 1.5px `--accent` ring (§6) |
| Streaming | unfolded during stream; one instant collapse at `done` (accepted, plan flag) |
| Fold on, zero spans | graph unchanged; chip still says `Folded` (§4 honesty rule) |
| Fold off | byte-identical pre-fold view (AC4); no fold UI anywhere |
| Filter change / reload / repo switch | expansion set resets; runs return folded |
| Loading / error / empty-unborn | existing graph states own the pane; no fold-specific UI |
| Pathological N (5+ digits) | `⋯ 12,048 commits` — fits 160px pill max at 11px/600 |

Both themes × both graph styles: all colours are palette- or var-driven; no per-theme branches
in `drawFold.ts` beyond the palette it is handed.

## 9. Mock-IPC harness states (`VITE_MOCK_IPC=1`)

Via `src/ipc/mock/handlers/graphFold.ts` (plan) against the fixture layout:

1. Fold off → fixture graph byte-identical (AC4 baseline).
2. Fold on → row count shrinks; `⋯ N commits` pill visible; N correct (AC1).
3. A 4-commit linear run present → does **not** fold.
4. Click pill → rows appear in place, `Collapse N` on the boundary row; click it → pill restored
   (AC2 round-trip).
5. Keyboard: arrow onto pill → announcer string; Enter expands; ArrowLeft collapses (§3).
6. Reveal (mock jump) to a hidden commit → auto-expand + flash (§5).
7. Select a mid-run commit, enable fold → run stays expanded, selection intact (AC6); then
   collapse it → selection ring on the pill (§6).
8. Pathological: a span with count ≥ 10,000 → grouped N renders inside the pill.
9. Both styles (standard/Bonsai) × both themes via the existing style/theme switches.
10. Persistence: toggle on, reload → still on (AC7; mock persistence validation).

Scroll feel on 20k+ with fold on is a **USER CHECKPOINT** (AC8 — headless harness has no rAF).

## 10. Out of scope / locked

No expand/collapse animation. No per-span "expand all" chrome. No change to `MIN_FOLD_RUN`
exposure (design-fixed at 5). No search-in-folded-runs features (spec non-goal). Second-parent
subtree folding is future work.
