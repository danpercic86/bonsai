# PR-badge placement — UI contract

Scope: relocate the forge PR badge (and the CI status dot) out of the crowded **left ref-column
band** into a dedicated, right-aligned **forge column** in the per-row metadata pack. Fixes the
user complaint that PR pills sit on top of the commit dots / ref pills and make the graph's busy
left edge unreadable. Canvas-drawn; no DOM.

Owner files (all edited by senior-dev — no new files needed):
- `src/graph/metrics.ts` — new column tokens.
- `src/graph/rightColumns.ts` — add the `forge` column to the pack + `RightColumns`.
- `src/graph/forgeBadges.ts` — `rowForgeSignal` row-level helper; PR-state glyph in `drawPrBadge`.
- `src/graph/refLabels.ts` — **remove** the in-band PR/CI signal reservation, layout, and draw.
- `src/graph/drawRowText.ts` — draw the forge cell in the new column.
- `src/graph/hitTest.ts` — replace `prBadgeHitAt`/`signalHitAt` with a column-based `forgeHitAt`.
- `src/graph/GraphCanvas.tsx` — hover tooltip + click-to-open PR now hit the forge column.
- `src/components/RevealAnnouncer` (SR string) — append PR state to the row announcement.

The architect contract here is the existing `GraphDisplayOptions` shape (`prByBranch`,
`ciBySha`, `showPrBadge`, `showCiStatus`) — unchanged. This is a pure placement/draw move.

---

## 1. Why a right-hand column (chosen option)

Options weighed:
1. **Dedicated right-hand forge column in the metadata pack** — CHOSEN. Aligns all PR pills into
   one vertical rail, far from the graph lanes/ref band; reuses the existing right→left pack model
   (author/SHA/date) so geometry, hit-test, and toggle-reclaim all work identically; consistent
   look with the columns already there.
2. Hover/selection-only reveal — rejected: PR status at a glance is a primary value of the feature
   for a pro tool; hiding it behind hover defeats the purpose and hurts scannability.
3. Keep in the ref band but shrink to a dot-only indicator — rejected: still crowds the busiest
   region and loses the `#num`.

The **ahead/behind chip stays in the ref band** — it describes the ref's own local/remote
divergence, not a forge artifact. Only the two forge signals (PR + CI) move.

---

## 2. Placement & geometry

The forge column is the **leftmost** column of the right-aligned metadata pack — placed
immediately right of the flexing commit summary, left of the author/SHA/date columns. It sits
well clear of the graph lanes and the 180px left ref band, and reads adjacent to the summary it
annotates.

```
| refCol band (180px) | graph lanes | summary (flex) → | FORGE | author | sha | date |
  branch pills+icons                  commit message      ● #123
  +ahead/behind chip                                       ○ #98
                                                           (empty rail cell on non-tip rows)
```

Pack order (right→left placement in `computeRightColumns`, so `forge` lands leftmost):
`date` (rightmost) → `sha` → `author` → `forge`. `summaryEndX` = cursor after `forge`.

### 2.1 New METRICS tokens (`src/graph/metrics.ts`)

```
prBadgeMaxWidth: 56,   // was 46 — +10px for the leading PR-state glyph (fits "◆ #12345")
forgeColWidth:   74,   // ciBadgeSize(11) + signalGap(6) + prBadgeMaxWidth(56) + 1px slack
```

`prBadgePadX (5)`, `ciBadgeSize (11)`, `signalGap (6)` are unchanged and reused. No compact
override — the forge column is suppressed in compact (see §5).

### 2.2 Column reservation gate (`computeRightColumns`)

Reserve the column only when a forge signal is both **enabled and present**, so repos with no
forge/PR data give the width back to the summary (identical to how `showDate` etc. reserve):

```
const forgeShown =
  (display.showPrBadge && display.prByBranch.size > 0) ||
  (display.showCiStatus && display.ciBySha.size > 0);
const forge = forgeShown ? place(m.forgeColWidth) : null;
```

Add `forge: ColRect | null` to `RightColumns`. Return `summaryEndX = cursor` after `forge`.

### 2.3 Intra-column layout (left-aligned within the column)

Draw the row's forge cell anchored at `cols.forge.leftX`, so pills line up on their left edges:
- CI dot centered at `leftX + ciBadgeSize/2`, row-center `cy`.
- PR pill left edge at `leftX + (ciPresent ? ciBadgeSize + signalGap : 0)`, width =
  `prBadgeWidth(pr)` (content-measured, capped at `prBadgeMaxWidth`).
- If only a PR: pill hugs `leftX`. If only CI: dot at `leftX`, no pill.

### 2.4 Row-level signal selection (`rowForgeSignal` — new pure fn in `forgeBadges.ts`)

A row may carry several branch entities. The forge cell shows the signals of the **first branch
entity** (in `groupRefs` order: detached-head, then branches local-first) that has any:

```
rowForgeSignal(refs, node, display): { pr: PrBadge|null; ci: CiBadge|null } | null
```

Returns `null` when the row has no branch entity with a signal. This is the single source of
truth for both `drawRowText` and `forgeHitAt`. (Replaces the per-entity `branchSignals` call
inside `layoutRefLabels`.)

---

## 3. Visual treatment & PR-state glyph (a11y MUST-FIX)

Pill colours are unchanged from today: open `--badge-good`, merged `#8957e5`, closed
`--badge-warn`, draft grey outline (`bg-2` fill, `text-3` border, `text-2` label).

**Colour is currently the ONLY carrier distinguishing open / merged / closed** (all three read
`#num`) — a house-rule violation (§7 of ui-reference). Fix by drawing a leading **PR-state glyph**
inside the pill, before `#num`, in the pill font/label colour:

| State  | Pill                     | Leading glyph | Meaning carrier |
|--------|--------------------------|---------------|-----------------|
| open   | filled `--badge-good`    | `○`           | hollow ring = active/open |
| merged | filled `#8957e5`         | `◆`           | filled diamond = merged |
| closed | filled `--badge-warn`    | `✕`           | house "dismiss/close" glyph |
| draft  | grey **outline** pill    | `○`           | outline shape (a draft is an open-but-unready PR; same family as open, distinguished by fill-vs-outline) |

Glyph sits at `x + prBadgePadX`; `#num` follows after a 3px gap; truncate `#num` to the remaining
`prBadgeMaxWidth - prBadgePadX*2 - glyphW - 3`. Document this glyph set in ui-reference §6 as the
"PR-state glyph" extension (not a synonym of the CI vocabulary).

CI dot glyphs are unchanged (`✓`/`✕`/pending dot/neutral dash from `ciBadgeVisual`).

---

## 4. States

- **No signal on row** — blank rail cell (column reserved globally, cell empty). This is the
  common case; the aligned empty rail is intentional, like empty SHA on a merge row.
- **One PR, no CI** — pill at `leftX`.
- **PR + CI** — dot, gap, pill.
- **CI only** — dot at `leftX`.
- **Many PRs (across rows)** — bounded to branch-tip rows; the vertical rail keeps them aligned
  and legible. No extra de-emphasis needed — distance from the graph focal point already de-emphasises
  them versus the old in-band placement. Do NOT desaturate; state hue is the signal.
- **Selected row** — the `--accent` row/selection treatment spans the full width including the
  forge column; the PR pill keeps its own fill (opaque, so contrast is unaffected). No change.
- **Hover (row)** — row hover tint spans the column; tooltip on the cell (§6).
- **Long content** — PR number capped at `prBadgeMaxWidth`; `#12345`+ ellipsised by
  `truncateToWidth`. Column is fixed-width so it never pushes into the summary (summary's right
  edge is `summaryEndX`, already left of the column).
- **Loading / stale** — no forge data cached ⇒ `forgeShown` false ⇒ column absent; when data
  arrives the column appears on next paint. No skeleton on canvas (consistent with SHA/date).

---

## 5. Density (compact) & themes

- **Compact:** `showPrBadge`/`showCiStatus` already arrive `false` in compact (AND-ed with
  `!compact` by the caller), so `forgeShown` is false and the column is not reserved — the summary
  reclaims the width. No compact geometry needed. Unchanged behaviour.
- **Cozy:** as specced above; forge cell is vertically centred on the 32px row, pill height 18px.
- **Both themes:** pill fills are theme tokens (`--badge-good`/`--badge-warn`) plus the fixed
  merged violet `#8957e5` and draft grey — all inherited unchanged and already shipping. White
  label on `--badge-good`/`--badge-warn`/violet and `text-2` on the draft grey all currently meet
  ≥4.5:1 in both themes (unchanged pairs; no new token, so no new ratio to clear). The new glyph
  uses the same label colour as `#num`, so its contrast equals the label's.

---

## 6. Interaction, hit-test & keyboard

- **Click a PR pill** → `onOpenPr(number)`, does NOT select the row (unchanged behaviour, new
  location). Move the click hit-test from the `x < refColWidth` ref-band branch to a forge-column
  branch: if `cols.forge !== null` and `x ∈ [forge.leftX, forge.rightX]`, compute
  `rowForgeSignal` and, if the x falls within the PR pill sub-rect, open it. Rebuild `forgeHitAt`
  in `hitTest.ts` from `cols.forge` + `rowForgeSignal` (retire `prBadgeHitAt`/`signalHitAt`).
- **Hover tooltip** (unchanged copy, re-anchored to the forge cell):
  - PR: line 1 `PR #{n} ({state})`, line 2 `{title}`. State word = `open`/`merged`/`closed`/`draft`.
  - CI: `Checks: {p} passed, {f} failed, {q} pending`.
- **Hit target:** the PR pill is 18px tall × up to 56px wide (≥24px in its long axis; row is the
  pointer target for selection, the pill is an inner action target — matches the ref-pill
  precedent). The CI dot's tooltip box is `ciBadgeSize` (11px) but it is informational only (no
  click), consistent with today.
- **Keyboard / SR:** the canvas is opaque, so the forge signal joins the row live-region
  announcement (§4.1 of ui-reference). Append, when present:
  `" PR #{n} {state}."` and `" Checks {rollup}."` to the existing settled-selection string. This is
  the only way an AT user gets the signal the pill colour gives a sighted one.

---

## 7. Motion

None. The column appears/disappears with data availability on the normal paint; no transition on
canvas (consistent with every other graph element and the render budget).

---

## 8. Harness states (`src/ipc/mock/`, `VITE_MOCK_IPC=1`)

Verifiable in the browser harness. Ensure the mock fixtures cover (extend, do not add new files):
- **empty:** a repo whose branch tips have no PRs → column absent, summary full-width.
- **one PR:** a single open PR on a branch tip → single pill in the rail.
- **all states:** open + merged + closed + draft PRs on different tips → glyph set + colours.
- **PR + CI together:** a tip with both a PR and a CI rollup → dot + pill in one cell.
- **pathological:** a 5-digit PR number (`#98765`) and a very long PR title (tooltip line 2) → pill
  truncation + tooltip overflow.

Frame-timing/scroll-feel remain USER CHECKPOINT (headless harness pauses rAF).

---

## 9. ui-reference.md changes (same pass)

- §4 "Right of the graph": note the metadata pack now includes a leftmost **forge column**
  (`forgeColWidth` 74px) reserved only when forge data is present; PR/CI no longer render in the
  ref band.
- §6: remove the PR/CI-in-band description; add the **forge column** + the **PR-state glyph** table
  (○ open / ◆ merged / ✕ closed / ○-on-outline draft) as the non-colour carrier for PR lifecycle.
- Metrics mirror: `prBadgeMaxWidth 46→56`, add `forgeColWidth 74`.

---

## 10. Flagged ambiguities (orchestrator decision)

- **F1 — Move the CI dot too, or PR only?** Recommendation: move BOTH (specced above) so the ref
  band is fully clear of forge signals and all forge status forms one coherent rail. Lower-effort
  alternative: move only the PR pill and leave the tiny CI dot trailing the ref pill. I recommend
  moving both for consistency; the extra churn is `drawRowText`/`hitTest` only.
- **F2 — Column position:** leftmost of the pack (adjacent to summary) vs far-right rail. I chose
  adjacent-to-summary so the badge reads with the commit it annotates and to keep `date` at its
  conventional far-right slot. If the user prefers a fixed far-right rail, swap the pack order to
  place `forge` first (rightmost); geometry is otherwise identical.
