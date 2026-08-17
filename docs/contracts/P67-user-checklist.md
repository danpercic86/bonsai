# P67 — UX polish batch: native USER CHECKPOINT checklist

Milestone P67 (two user-reported items from 2026-08-17): **item 1** the dashed HEAD guideline
vanishing on scroll, **item 2** the right panel's wasted vertical height.
Contract: `docs/contracts/P67-ux-polish-batch.md`.

Sub-increments: **P67a** HEAD guideline (`headGuide()` in `viewport.ts` + `drawHeadGuide` /
`drawHeadEdgeMarker`), **P67b** right-panel structure + tighter cozy default (`RightPanelActionsRow`,
`CommitOptionsRow`, `StashSplitButton` deleted, `--rp-*` variables), **P67c** the
`panelDensity: cozy | compact` setting end to end, **P67d** docs/board, **P67e** `StatusPanel.tsx`
split (pure refactor).

This splits the P67 acceptance into what the orchestrator has already proved by AI gate versus what
only a human at the native window can confirm. **The orchestrator must never self-declare the
NATIVE section** — present the AI-gate evidence, then ask the user to run `pnpm tauri dev`.

## Harness limitation (why these are USER CHECKPOINTs)

The mandatory browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`) runs **headless**: the Browser pane
composites at 0×0, so `document.visibilityState === "hidden"`, the browser **pauses
`requestAnimationFrame`**, and **no canvas pixel is ever produced**. For item 1 that is total: the
guideline's *geometry* is machine-proved through the pure `headGuide()` unit tests and the
`window.__bonsai.p7` JS seam, but nobody can *see* the line, its dash phase, or where it terminates
without a visible window. For item 2 the DOM, computed styles and measured heights ARE machine-
verified (below) — but whether a density *looks* right is human perception, in both themes, at the
user's own display and DPR.

---

## AI GATE — the automated half (listed for context, do NOT re-ask the user)

> **STATUS (updated 2026-08-17 — P67 code-complete).** This section originally claimed every item
> below was "already green" when none of it existed yet; it was corrected to per-increment tracking
> mid-milestone, and every line below has now actually landed. Keep this discipline: only tick an item
> once its sub-increment is committed.
> **P67a ✅ · P67b ✅ · P67c ✅ · P67e ✅ · P67d ✅ (docs).**
> Commits: P67a 0ec69f9 · P67b e607d2c · P67c 5e68db5 · P67e d50361a.
> Final AI gate: vitest **1361/0 across 112 files** (pre-P67 baseline 1331/111), tsc 0, build green,
> cargo test -p bonsai --lib **222/0**, cargo clippy --workspace --tests -D warnings clean, command
> count **157 (+0)**.
>
> **Measured space reclaimed (item 2)** — measurements, not the contract's ~110px estimate:
> the status list went **452.47 → 568.00 px = +115.53 px ≈ 4.8 file rows** in cozy (≈129.5 px / ≈5.4
> rows counting the tighter in-scroller padding), and compact adds **+30 px** more (408 → 438 px,
> measured live in the harness).
>
> **What none of the above proves: any canvas pixel.** The harness pane is headless — rAF is paused and
> it does not composite, so even computer{screenshot} fails outright. The guideline's geometry is
> pinned by unit tests and the window.__bonsai.p7 seam, but its APPEARANCE is entirely unverified.
> That is what the NATIVE section below is for; do not self-declare any of it.

Frontend (vitest):
- **[P67a ✅]** `headGuide` (`src/graph/viewport.test.ts`, **13** cases — 11 as specified plus the two
  added by amendment A5): `headIndex === null` → `null`; WIP anchor at
  `scrollTop 0`; **scrolled far past the WIP row still returns a segment** (the exact reported bug);
  `edge: 'top'` / `'bottom'` when HEAD is off-screen; both ends inside `[-8, height + 8]` at an
  absurd scroll offset (the perf guard — the old code walked ~16 000 dash segments per frame);
  the crawl guard — per amendment **A6.2** this asserts the dash *phase*
  (`mod6(y0 - dashOffset) === mod6(anchor)`) across consecutive 1 px scroll steps, not merely
  6-periodicity, because a periodicity-only check passes with the sign inverted (the A6.1 bug); the avatar-halo
  shortening in both directions; `wipOffset: 0` (clean tree) anchoring at `-8`; sub-1 px collapse
  → `null`.
- **[P67b ✅]** `WorkspaceRightPanel.test.tsx` (new): one merged actions row; the `⋯` menu exposes all three stash
  scopes with the *same* per-scope disabled gating the old `StashSplitButton` had; outside-click and
  Escape close it; the row is hidden while an operation is in progress or HEAD is unborn;
  `data-density` reflects the prop; no `.stash-split` element remains.
- **[P67b ✅]** `CommitBox.test.tsx` passes **unchanged** (role+name queries survive the Sign/Skip row merge),
  plus one new assertion that both checkboxes are siblings in a single `.commit-options-row`.
- **[P67c/P67e ✅]** `StatusPanel.test.tsx` passes **unchanged** in both P67c and P67e — that is the P67e refactor's
  acceptance test.
- **[P67c ✅]** `SettingsPanel.test.tsx` extended: the Appearance section's Cozy/Compact button patches exactly
  `{ panelDensity: 'compact' }`.
- **[P67a ✅]** `src/graph/draw.ts` stays untested **by design** (all guideline arithmetic lives in `viewport.ts`;
  a canvas mock would assert paint calls, not behaviour).

Backend (cargo):
- **[P67c ✅]** `settings.rs`: `panel_density_roundtrips_both_variants` (raw JSON shows
  `"panelDensity": "cozy"` / `"compact"`) and
  `old_settings_file_without_panel_density_loads_default` — the additive-migration guard behind the
  **no version bump** decision.
- **[P67c ✅]** `commands/tests.rs` `set_ui_settings_patch_is_partial` gains a `panel_density` arm proving it
  patches independently of `listView` and `graph`.
- `cargo clippy --workspace --tests -- -D warnings` clean; `clamp_graph_prefs` untouched.

Harness (mock IPC): `data-density` flips and survives a reload; `--rp-*` computed values correct in
both densities; `.status-panel` `getBoundingClientRect().height` measured before/after (the
objective "how much did the changes tree gain" number, quoted in `TODO.md`); `.stash-split` markup
gone; `.file-row` inside the **diff** tree and `.section-label` in the **sidebar** still compute
their pre-P67 values (proving the `var(--x, <today's value>)` fallback keeps them pixel-identical);
`.graph-spacer` scroll extent unchanged; `window.__bonsai.p7SelfTest()` → 0 failures.

**Command count unchanged: 157.** No new command, event, channel or `AppError`.

---

## NATIVE — user must confirm in `pnpm tauri dev` (open a real repo with a few hundred commits)

### Item 1 — the dashed HEAD guideline (P67a)

1. **It survives scrolling.** With uncommitted changes present, scroll the graph well past the top
   (far more than the ~3 rows where it used to vanish) so the checked-out commit sits mid-viewport.
   The dashed line is **still drawn** in the HEAD lane's colour, from the top of the view down to
   the checked-out commit. *This is the reported bug — it must now be continuously visible.*
2. **The dashes do not crawl.** Scroll slowly up and down. The dash pattern must stay locked to the
   content (dashes appear to move *with* the graph), not slide/shimmer along the line as the scroll
   position changes.
3. **It terminates at the avatar ring.** Zoom your attention to the checked-out commit: the dashed
   line stops just short of the HEAD avatar's halo — it does **not** paint over or through the
   avatar disc.
4. **It works on a clean tree.** With **no** uncommitted changes (no WIP row at all), the guideline
   is still drawn from the top edge down to the checked-out commit. (Before P67 nothing was drawn
   in this case.)
5. **Off-screen marker — below.** Scroll so the checked-out commit is *below* the viewport. A small
   triangle in the HEAD lane colour appears at the **bottom** edge, in the same lane column, and the
   dashed line runs down to it. It points the right way and reads as "HEAD is further down".
6. **Off-screen marker — above.** Scroll so the checked-out commit is *above* the viewport (on a
   clean tree, or well past HEAD). The triangle appears at the **top** edge instead. It never shows
   both, and it never shows the wrong direction.
7. **First load of a big repo.** Open a large repo. While the graph is still streaming, before HEAD's
   row has arrived, there is **no** guideline and **no** edge marker (rather than a marker pointing
   nowhere). Once HEAD's row streams in, the guideline appears. A brief absence on first load is
   expected and correct.
8. **The WIP row is unchanged.** At the very top of the graph, the "Uncommitted changes (n)" label,
   its dashed marker circle and its hover highlight look and behave exactly as before, and the
   marker circle still paints **on top of** the dashed line where they overlap.
9. **Both themes.** Repeat items 1, 3 and 5 in the **light** theme: the dash colour and the edge
   marker are legible against the light background.
10. **No scroll regression.** Scrolling a large history still feels smooth (this is one extra stroked
    line per frame — it must not be perceptible). The scrollbar extent and the position of every row
    are unchanged.

### Item 2 — right-panel density (P67b cozy default)

11. **More rows are visible.** With a dirty working tree, compare the changes tree against your
    memory of the previous build: roughly **4–5 more file rows** fit without scrolling. (The
    orchestrator will quote the measured px gain — confirm it feels like a real improvement, not a
    cramped one.)
12. **The merged actions row.** Above the commit box there is now **one** row: the
    "Amend last commit" checkbox on the left and a `⋯` button on the right — not the previous
    two-row Stash-button + Amend stack.
13. **`⋯` is discoverable and complete.** Clicking `⋯` opens a menu with **Stash all**,
    **Stash all + untracked** and **Stash staged only**. Each one stashes the right scope. Items are
    greyed out when their scope has nothing to capture (e.g. "Stash staged only" with nothing
    staged). Clicking elsewhere or pressing **Escape** closes the menu.
14. **The sidebar's one-click stash still works** and still stashes *including untracked* — it was
    deliberately left as the fast path.
15. **Amend still keeps keyboard focus.** Type a partial commit message, then toggle
    "Amend last commit" on and off. The amend checkbox itself must not steal or drop focus oddly,
    and the "already pushed — amending rewrites published history" warning appears only when the
    branch's upstream is up to date.
16. **The merged commit options row.** Below the message box there is **one** row carrying both
    "Sign commit" (when signing is configured) and "Skip hooks". Checking either one still shows its
    hint text, wrapping onto a second line if the panel is narrow. Both still take effect on a real
    commit (a signed commit is signed; skip-hooks really skips `pre-commit`).
17. **The commit message box auto-grows.** Empty, it is shorter than before. Typing multiple lines
    grows it up to a ceiling, after which it scrolls; the drag-to-resize handle still works.
18. **Nothing outside the right panel changed size.** Open a **diff overlay / diff browser** with the
    file tree, and look at the **sidebar**: file rows, directory rows and section labels are exactly
    the size they were before P67 — the density must not leak out of the right panel.
19. **The Pull requests tab still preserves a draft.** Type a partial commit message, switch to
    **Pull requests**, switch back to **Working**: the message is still there.
20. **Merge / rebase banner.** During a real merge with conflicts, the operation banner and the
    conflicts section are tighter but fully legible, and "Commit merge" from the banner still submits
    the commit box.

### Item 2 — the Cozy / Compact toggle (P67c)

21. **The setting exists and reads correctly.** Settings → **Appearance** → **Panel density** shows
    a button labelled with the current value (**Cozy** by default). Clicking it switches to
    **Compact**; the right panel visibly densifies **immediately**, with no reload.
22. **Compact is still legible.** In Compact, file paths, badges, section labels and the commit
    controls are all readable at your display/DPR — nothing clipped, nothing overlapping — in **both
    light and dark** themes. *If it is too tight, say so: every compact value lives in one CSS block
    and is a one-line tune.*
23. **It is independent of the graph's Compact rows.** Graph → **Compact rows** must not change the
    right panel, and Panel density must not change the graph's row height. The Appearance hint
    cross-referencing the two reads clearly.
24. **It persists.** Set **Compact**, quit the app completely, relaunch: the right panel is still
    Compact. Switch back to **Cozy**, relaunch: still Cozy.
25. **An old settings file is not disturbed.** (If you have a pre-P67 `settings.json`.) The first
    launch after updating opens with **Cozy** and every other setting — theme, pane widths, file
    lists, graph prefs, AI settings — exactly as before.

### Regression sweep

26. **Status panel behaviour unchanged (covers P67e).** Stage / unstage / discard individual files
    and whole folders, expand/collapse directories, toggle tree vs flat, open an inline diff from a
    row, and resolve a conflict from the conflicts section. All behave exactly as before the
    refactor.
27. **Commit paths unchanged.** A plain commit, a **Commit & Push**, an **amend**, and a **merge
    commit** all still work from the tightened commit box, including the Ctrl+Enter shortcut and the
    error banner with its "Set identity…" action.
28. **Panel resize.** Drag the right panel narrower and wider in both densities: nothing overlaps,
    the `⋯` menu still opens fully inside the window, and hints wrap rather than clip.
