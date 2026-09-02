# P92 — Multi-ref commits: actionable "+N" chip + branch-pickable context menu (UI contract)

Owner: ui-designer. Implementer: senior-dev. Status: **implemented, revision 2** (design review
2026-08-31 — see **§8**, which supersedes §1.3, §1.5, §2.2 and §5 where they disagree).
Related: `docs/contracts/ui-reference.md` **§6.2** (the design-system home of the interactive chip,
the ref-picker menu pattern, and the app-wide context-menu height clamp). **§6.2 still carries the
uncorrected clamp rule — it must be updated to match §8.1 in the same pass as the fix.**

## 0. Scope & boundary

**Frontend-only. No backend, no IPC, no Rust change.** Every fact required is already on the wire:
`GraphNode.refs` carries all refs for the row, `groupRefs()` collapses them to display entities, and
`createWorkspaceMenus()` already exposes `branchMenuItems` / `tagMenuItems` / `stashMenuItems` /
`commitMenuItems`. No new tokens; no new CSS variables. One CSS rule is added
(`.context-menu` max-height/scroll) and one new small module file.

Two problems, **one pattern**: a *branch-first* menu level whose rows are branch/tag/stash names and
whose `children` are the existing per-ref menus. The `+N` chip opens that level for the hidden refs;
the commit/row menu embeds that level when the commit carries ≥2 actionable refs.

---

## 1. Problem A — the "+N" chip becomes actionable

### 1.1 Behaviour

| Input | Today | P92 |
|---|---|---|
| Hover chip | tooltip listing hidden ref names | **unchanged** (glance) |
| Left-click chip | nothing | **opens the ref-list menu** anchored to the chip |
| Right-click chip | falls through to `fallbackBranchRef` → first branch's menu | **opens the ref-list menu** |

> Behaviour change to call out explicitly: chip right-click **no longer** falls through to the first
> branch's menu. That fallthrough is the bug — it silently acted on a ref the user did not point at.
> The whole-row fallback for clicks *outside* the ref band is unchanged.

Hover = read, click = act. Keeping the tooltip means no extra latency for the common "what's hidden?"
glance, and no menu opens on mere mouse travel across the band.

### 1.2 The menu

Reuses `ContextMenu` verbatim — it already has a header row (`.context-menu-header`), `children`
flyouts opened by hover-delay / ArrowRight / Enter / click, viewport clamping, and dismissal on
outside pointerdown / Esc / scroll / resize / blur. **No new popover component.**

```
   ┌──────────────────────────────┐
   │ 3 more refs                  │  header (role="presentation")
   ├──────────────────────────────┤
   │ ⑂ chore/dep-refresh-2026-08 ▸│  ─┐ one row per HIDDEN entity
   │ ⑂ main                      ▸│   │ children = that ref's full menu
   │ # v1.5.0                    ▸│  ─┘
   └──────────────────────────────┘
```

- **Anchor:** the chip rect from `chipHitAt`, converted to client coords — menu top-left at
  `(chipLeft, chipBottom + 4)`; the component's existing clamping handles viewport edges.
- **Header:** `1 more ref` / `{n} more refs`. Both forms are specced; `+1` is reachable.
- **Rows:** `entities.slice(shown)` in `groupRefs` order — no re-sorting. Row label = the entity's
  display label: the short branch name **once** (`main`, never `main` + `origin/main`), `# v1.5.0`
  for tags, `stash@{0}` for stashes. **See §8.3 for the single source of that label.**
- **Row icon:** the existing menu-icon slot with the existing chrome icons — `BranchIcon` for
  branches, `TagIcon` for tags, the stash icon for stashes. Local/remote nuance is carried by the
  *submenu contents* (Checkout local vs remote), not by a new glyph.
- **Row children:** `buildContextItems({kind:'ref', ref: targetRefOf(entity), oid: node.id})` — the
  same dispatcher the visible pills use, so overflow refs and pills behave identically. Tag rows get
  `tagMenuItems`; stash rows get `stashMenuItems`; the current HEAD branch gets the existing
  `Rename…` + commit-menu fallback. **No dead rows.**
- **A row whose menu is still empty** (a detached-`head` entity) renders **disabled**, with the label
  and no chevron — never a chevron that opens nothing.
- **Parent rows have no default action.** `activate()` fires `onSelect` and closes the menu *if
  `onSelect` is defined*, and only otherwise toggles the flyout. So picker rows must **omit
  `onSelect` entirely** — no no-op, no first-child action. Picking `main` must never mean
  "checkout main" by accident.

### 1.3 Sizing, 2 hidden vs 12 hidden — **SUPERSEDED BY §8.1**

- Width inherits `.context-menu` (`min-width: 200px; max-width: 360px`); long names ellipsize via the
  existing `.context-menu-item` rule and the row carries `title` = the full ref name.
- The app-wide height clamp `max-height: min(60vh, 480px)` stands, **but the two rules that make it
  usable were missing from this section and are now specced in §8.1. Shipping the clamp without them
  is a defect: verified in the harness on 2026-08-31, wheeling a clamped menu closes it, and opening
  a flyout gives the parent a horizontal scrollbar and scrolls its own rows out of view.**
- 2 hidden → a 2-row menu, no scroll, ~72px tall. 12 hidden → scrolls; the header scrolls with the
  list (no sticky-header rule is worth adding).
- 30 refs on one commit still works: the list scrolls and arrow-key focus movement drives
  `scrollIntoView({block:'nearest'})`.

### 1.4 States, themes, densities

| State | Treatment |
|---|---|
| Chip default | unchanged: `--bg-2` fill, `--text-2` label, 1px `--border` (both themes) |
| Chip hover | tooltip only; **no canvas repaint**, no chip restyle (protects the graph render budget) |
| Cursor over chip | `cursor: pointer` on the canvas element while the chip is the hover target — the sole affordance that it is clickable; driven by the already-computed hover target, no extra hit pass |
| Menu open | standard `ContextMenu` surface; rows hover/`:focus-visible` = `--selection`; disabled = `--text-3` + `disabled` |
| Loading / mutating | rows whose actions are gated stay `disabled` exactly as `branchMenuItems` already does (`mutating \|\| opActive`) |
| Empty | unreachable — the chip exists only when `hidden > 0` |
| Error | none: the menu is a pure projection of already-loaded data |
| Long content | ellipsis + `title`; never wraps |
| Densities | menu geometry is density-independent (chrome, not a data row); the chip keeps `pillHeight` 18 cozy / 15 compact |
| Light theme | all colours are existing tokens; no fixed hex is introduced |

### 1.5 Keyboard & accessibility — **CORRECTED, see §8.2**

- **No new canvas focus machinery for the chip.** The keyboard answer to A is B: the selected-row
  context menu (Menu key / Shift+F10 on the focused graph scroller) opens the commit menu, which
  under §2 lists **every** ref on that commit — hidden ones included. A keyboard user never needs to
  reach the chip.
  *Correction: revision 1 cited ui-reference §4.1 as if this key handler already existed. It did
  not — §4.1 specs the scroller's `tabIndex`/`role`/arrow nav only. P92 introduces the
  Menu/Shift+F10 handler; ui-reference §4.1 must be amended to record it (§8.2).*
- The `+N` chip is canvas-drawn and therefore not a tab stop. Accepted limitation, mitigated as
  above; screen-reader parity comes from the row menu.
- Menu a11y is inherited and already correct: `role="menu"`, rows `role="menuitem"`,
  `aria-haspopup="menu"` + `aria-expanded` on picker rows, ArrowUp/Down/Right/Left, Esc closes the
  flyout then the menu, focus restores to the graph scroller on close.
- The header is `role="presentation"`; its information is additionally exposed as `aria-label` on the
  menu root: `"3 more refs on commit 4f2a91c"`.
- Hit target: the chip is ≥18px tall; where the `+N` label makes it narrower than 24px, extend the
  **hit box** (not the paint) to a 24px minimum in `chipHitAt`.

### 1.6 Motion

None beyond the existing flyout behaviour. No new transitions; nothing on the canvas animates;
`prefers-reduced-motion` is unaffected.

---

## 2. Problem B — pick the target branch in the context menu

### 2.1 Decision: branch-first, not action-first

Rejected: action-first (`Merge ▸ [branch list]`, `Rebase onto ▸ [branch list]`). It would require
decomposing `branchMenuItems` into per-action builders, duplicating every gate, and it repeats the
branch list once per action. Branch-first reuses `branchMenuItems` wholesale and is the same widget
as §1 — users learn one pattern.

### 2.2 The rule (no regression for the common case)

Let `candidates` = the row's entities, in `groupRefs` order, whose `buildContextItems` result is
non-empty (every actionable branch/tag/stash on the commit).

- **`candidates.length ≤ 1` → today's menu, byte-identical.** Flat items, labels unchanged
  (`Merge chore/x into dev`, `Rebase dev onto chore/x`). No `▸`, no extra level. This is the
  overwhelmingly common case and must not regress.
- **`candidates.length ≥ 2` → the picker level is prepended to the commit menu:**

```
   ┌───────────────────────────────────┐
   │ ⑂ dev                            ▸│   ← per-ref submenus, groupRefs order
   │ ⑂ main                           ▸│
   │ ⑂ chore/dep-refresh-2026-08      ▸│
   │ # v1.5.0                         ▸│
   ├───────────────────────────────────┤   ← separator (see below)
   │ Create branch here…               │   ← commitMenuItems, unchanged
   │ Compare with HEAD                 │
   │ Cherry-pick…                      │
   │ Revert…                           │
   └───────────────────────────────────┘
```

- **Separator.** Revision 1 said "the existing separator idiom"; **there was none** — `ContextMenu`
  had no way to draw a rule. P92 adds an additive `separator?: true` entry rendering a
  non-interactive `role="separator"` div (`.context-menu-sep`, 1px `--border`, `4px 6px` margin),
  skipped by keyboard nav because the focus queries scope to the row buttons. **This is now the
  canonical separator primitive for every menu in the app** — no other mechanism may be invented.
- Right-clicking a **specific visible pill** keeps today's precise behaviour: that pill's own menu,
  flat, no picker (orchestrator ruling on §7: **no** picker level on a visible pill). The picker
  appears only when the target is genuinely ambiguous — a right-click on the row/commit, or on the
  `+N` chip.
- **Ordering:** `groupRefs` order — detached HEAD, then branches (HEAD-local first, then other
  locals, then remote-only, in insertion order), then tags, then stashes. The menu order and the pill
  order match, which is the point.
- **The HEAD branch is not excluded.** It appears as a row whose submenu is the existing HEAD
  fallback (`Rename…` + commit actions). Hiding it would make the menu disagree with the pills.
- **Labels:** row label = the entity display label only (`main`). All verb phrasing stays inside the
  submenu (`Merge main into dev`), so the user reads the full sentence before acting.
- **Icons:** as §1.2.
- **Disabled/gated:** unchanged — each leaf keeps its own `disabled: mutating || opActive`. A picker
  row is disabled only when its submenu is empty (and then shows no chevron).
- **Keyboard:** inherited. Down/Up across rows, Right/Enter opens a submenu and focuses its first
  enabled item, Left/Esc returns. Picker rows come first, so the fastest path to a branch action is
  `Menu → Down×k → Right`.
- **Command palette:** no new entries. Branch actions are already palette-exposed per branch; this
  contract only changes graph-local targeting.

---

## 3. Component decomposition (file paths)

Nothing is appended to `RepoWorkspace.tsx`; `workspaceMenus.ts` gains only thin wiring.

| File | Change |
|---|---|
| `src/components/workspaceMenusRefPicker.ts` | **new**. `refPickerItems` / `actionableEntities` / `pickerLabel` / `pickerTitle` / `overflowHeaderText` / `overflowMenuLabel` / `graphMenuState`. Pure item-building. |
| `src/components/workspaceMenusGraphTarget.ts` | **new** (as built — the dispatcher was extracted here rather than left in `workspaceMenus.ts`, which keeps that file under the size limit. **Accepted; better than the contract's plan.**) |
| `src/graph/contextTarget.ts` | **new** (as built — `GraphContextTarget` + pure right-click/keyboard target resolution moved out of `GraphCanvas.tsx`. **Accepted.**) |
| `src/graph/hitTest.ts` | `chipHitAt` 24px minimum hit box; new `hiddenEntities(entities, laid)` shared by the hover tooltip and the chip menu. |
| `src/graph/GraphCanvas.tsx` | left- and right-click on the chip → `refPicker` target; chip hover → `cursor: pointer`; Menu/Shift+F10 on the scroller → the selected row's menu. |
| `src/components/ContextMenu.tsx` | `separator?: true`, `title?`, `ariaLabel`, `ContextMenuState`; focus scroll-into-view. |
| `src/styles/context-menu.css` | the height clamp (**as corrected by §8.1**) and `.context-menu-sep`. |
| `src/ipc/fixtures/multiRefRow.ts` | **new** fixture table (§5). |
| `src/graph/refLabels.ts` | unchanged today; **§8.3 asks for one shared `entityLabel(e)` export.** |

---

## 4. Microcopy (exact strings)

- Overflow menu header: `1 more ref` / `{n} more refs`
- Menu root accessible name: `{n} more refs on commit {shortOid}`
- Overflow/picker row `title`: the full ref name (`origin/topic/remote-only` for a remote-only
  branch)
- Nothing else changes. All branch/tag/stash verbs come from the existing builders.
- No destructive action is introduced. `Delete` / `Reset` keep their confirm dialogs and
  `tone: 'danger'`; they are simply now reachable for **every** ref on the commit rather than the
  arbitrary first one.

---

## 5. Harness fixtures (`src/ipc/fixtures/`, `VITE_MOCK_IPC=1`)

1. **Two-hidden case** — an existing row with 4 refs (2 local, 1 remote-only, 1 tag) → `+2`. ✅
2. **Twelve-hidden case** — `core work 1` carries 14 refs. **As built the chip reads `+14`, not
   `+12`: the pathological long branch name is first in the list, so it alone overflows the 180px
   band and NO pill is shown.** That loses the mixed shown/hidden case the contract was after (a
   band with visible pills *and* a chip). **Fix: move `LONG_BRANCH` out of first position** (e.g.
   after `release/2026-01`) so one or two short pills render and the chip reads `+12`/`+13`, while
   the long name still proves ellipsis in the picker row. See §8.4.
3. **Pathological length** — `feature/very-long-topic-branch-name-that-definitely-overflows-the-pill`
   proves ellipsis + `title` in the menu row. ✅
4. **Single-ref control row** — a commit with exactly one branch, verifying the flat menu. ✅
5. Matching `branches` snapshot entries for **every** fixture ref. ✅ (verified: all 14 picker rows
   render enabled with chevrons.)

Flyout hover *timing* and scroll feel remain **USER CHECKPOINT** items (the headless harness has no
rAF, and its pane reports a 0×0 viewport unless a size is emulated — emulate 1440×900 before
measuring anything that depends on `vh`).

---

## 6. Acceptance criteria

1. A commit with exactly one actionable ref produces a menu **identical** to today. ✅ verified
2. Left-click and right-click on the `+N` chip both open a menu listing every hidden entity. ✅
3. Every hidden ref is actionable; no dead rows, no empty-chevron rows. ✅
4. Right-clicking the row of a multi-ref commit shows one row per ref, in `groupRefs` order, above
   the unchanged commit actions, separated by a rule. ✅
5. `main` + `origin/main` on the same commit produce **one** row. ✅
6. A long picker menu stays inside the viewport **and scrolls**. ❌ — clamped correctly, but see
   §8.1: it cannot actually be scrolled.
7. Keyboard: Menu/Shift+F10 on a selected multi-ref row reaches every ref's actions. ✅ with the
   §8.2 caveat.
8. Roles/names present. ✅
9. No hardcoded hex in any changed file; both themes and both densities. ✅
10. Graph performance unchanged: no extra canvas repaint on chip hover. ✅

## 7. Open question — RESOLVED

Should the picker level also appear on a right-click of a *visible pill*? **Orchestrator ruling: no.**
Implemented as ruled: `resolveContextTarget` returns `kind:'ref'` directly for a pill hit.

---

## 8. Design-review corrections (2026-08-31)

### 8.1 The height clamp needs two companion rules (MUST-FIX)

Measured in the harness at 1440×900 with the 14-ref picker open:

- **Wheeling the menu closes it.** `ContextMenu`'s dismiss effect registers
  `window.addEventListener('scroll', () => onClose(), true)`. `scroll` does not bubble, but the
  capture listener still receives it from the menu's own scroll box. Empirically: setting
  `root.scrollTop` and firing `scroll` took the open menus from 2 → 0. A clamped-but-undismissable
  menu is the whole point of the clamp, so:
  **the scroll dismiss handler must ignore scroll events whose `target` is inside the menu root**
  (`rootRef.current.contains(e.target as Node)` → return), exactly as the pointerdown handler
  already does.
- **Flyouts are clipped and hijack the parent's scroll.** `overflow-y: auto` with an unset
  `overflow-x` computes `overflow-x: auto`, and a scroll container clips absolutely-positioned
  descendants. `.context-menu--sub` is `position: absolute` inside `.context-menu-row`, inside the
  now-scrollable menu. Measured with a flyout open: parent `scrollWidth` 617 vs `clientWidth` 343,
  and the browser auto-scrolled the parent to `scrollLeft: 269` to reveal the focused submenu —
  i.e. the parent's own rows slide out of view and a horizontal scrollbar appears.
  **Fix: the flyout must escape the scroll container.** Render `.context-menu--sub` as
  `position: fixed`, with `left`/`top` computed from the row's `getBoundingClientRect()` (the
  existing `useLayoutEffect` already measures `rowRect`; keep the same right-flip and bottom-raise
  clamping, now in client coords). Additionally set `overflow-x: hidden` on the clamped surface so a
  stray wide child can never produce a horizontal scrollbar.
- Corrected rule for `src/styles/context-menu.css` and **ui-reference §6.2**:

```css
.context-menu {
  max-height: min(60vh, 480px);
  overflow-y: auto;
  overflow-x: hidden;
  overscroll-behavior: contain;
}
.context-menu--sub { position: fixed; }   /* escapes the parent's scroll box */
```

  Note the clamp should apply to the **root and each flyout independently** — with the flyout
  fixed-positioned it is no longer nested, so listing both selectors is still correct, but the
  `--sub` rule must keep its own `max-height`.

### 8.2 ui-reference §4.1 must record the Menu/Shift+F10 handler

§4.1 (2026-08-22) specs the scroller's `tabIndex={0}`, `role="grid"`, `aria-label`, `aria-rowcount`,
`aria-activedescendant` and arrow-key nav. It never specced a context-menu key; P92 §1.5's citation
was wrong. Add to §4.1:

> **Menu key / Shift+F10** on the focused scroller opens the **selected** row's context menu,
> anchored at the ref band's left edge just under that row (clamped to the scroller's box). This is
> the keyboard route to the ref picker, and therefore to every ref on a multi-ref commit.
> *Known gap:* arrow-key row selection is a **window-level** keydown, so a user can select a row
> without the scroller holding focus, and the Menu key then does nothing. Acceptable for now (a
> click or Tab to the graph fixes it); the durable fix is to move the graph's arrow-key nav onto the
> scroller's own handler, or to have the window-level handler focus the scroller when it changes
> the graph selection. Tracked as a follow-up, not a P92 blocker.

**Separate pre-existing design-system defect (not P92's):** §4.1 mandates
`aria-activedescendant="graph-row-{selectedIndex}"`, but no element with that id exists — the rows
are canvas pixels. A dangling IDREF is worse than none: `role="grid"` with zero `role="row"`
children is an invalid structure to a screen reader. Either render one visually-hidden
`role="row"`/`role="gridcell"` element per *visible* row, or drop `role="grid"` +
`aria-activedescendant` and keep the live-region announcement as the sole channel. Needs its own
increment.

### 8.3 One source for the entity label (SHOULD-FIX)

`pickerLabel()` in `workspaceMenusRefPicker.ts` re-implements the label arm of
`entityStyle()` in `refLabels.ts` (`# ${name}` for tags, `name` otherwise). They agree today for all
four entity kinds — verified — but the graph tooltip renders `entityStyle(...).label` while the menu
renders `pickerLabel(...)`, so a future change to either drifts them apart silently.
**Fix: export `entityLabel(e: RefEntity): string` from `refLabels.ts`, have `entityStyle` return
`label: entityLabel(e)`, and have `pickerLabel` be a re-export.** No behaviour change; a one-line
unit test asserting the two agree for each kind is enough.

### 8.4 Fixture ordering (NIT) — see §5.2 above.
