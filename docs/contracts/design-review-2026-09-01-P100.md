# P100 — accent-fill row contrast: design review + contract amendments

**Reviewer:** ui-designer · **Date:** 2026-09-01 · **Contract:** `docs/contracts/P100-accent-fill-ui.md`
**Verdict: APPROVE with amendments. No MUST-FIX.**

**Method disclosure.** `Bash` was disabled for this session, so this is a review of the **shipped
end state** of the six named files against the contract, not of the literal `git diff` vs `b6afc2d`.
Everything asserted below was read from the working tree or measured in the mock harness
(`pnpm dev:mock`, port 1420, 1440×900, `data-theme` toggled on `<html>`).

---

## 1. Decisions

### D1 — the AC7 substitution: **approved.** §4.2's prescribed remedy was wrong; I own the error.

Measured in the harness by resolving the exact shipped declaration and computing WCAG ratios:

| `.btn-primary:hover` fill | Dark | Light | Verdict |
|---|---|---|---|
| old `filter: brightness(1.1)` | 6.17 | **3.94 ✗** | fails AA |
| my §4.2 remedy, `color-mix(… 92%, #ffffff)` | 6.11 | **4.06 ✗** | fails AA — arithmetically impossible to pass |
| **shipped** `color-mix(in srgb, var(--accent) 92%, var(--text-1))` | **5.99** | **5.15** | ✓ both |

senior-dev's reasoning is correct and its numbers reproduce within 0.03. Light ink is fixed `#ffffff`
at the luminance ceiling and the light baseline is 4.65, so **any** fill-lightening hover is
monotonically worse — my remedy could not have worked, and it would also have put a hardcoded hex
outside `tokens-and-base.css`, violating my own AC9. Mixing toward the theme's own `--text-1` is the
correct generalisation: it brightens in dark and deepens in light, and never touches the ink.
Fill-edge contrast against the panel is preserved (5.99 dark / 5.15 light vs `--bg-0`, ≥3:1).

### D2 — AC16: **accept the deeper light hover; do not revert.**

Reverting reinstates a 3.94:1 primary label, which is the defect P100 exists to remove. A primary
button that *darkens* on hover is the more conventional response of the two, not the odd one; it is
also what the dark theme has always done perceptually (fill moves away from the ink). Amended AC16
text is in §3.

### D3 — the `shortOid` hint on the PR base/compare comboboxes: **approved.**

A short oid is the right hint for a ref picker: it is the one disambiguator that is *already in the
data shape* (`BranchInfo.tip`), it distinguishes two refs pointing at the same tip (the common
`main` / `origin/main` case in the PR form), and it matches the house precedent
(`paletteActions.ts:183/193`). Ahead/behind or last-commit date would read better in the abstract but
are not on `BranchesSnapshot`, so they would be an architect-level change for a 11px right-aligned
hint — wrong cost. "Nothing" is worse than the oid: the field is a free-text combobox over a long
ref list and needs a secondary column.

Approved as microcopy: no string, no punctuation, 11px `--text-2`, right edge, `flex: none` — it
inherits the audited `.combobox-option-hint` recipe exactly.

---

## 2. Measured vs predicted

`M` = mounted and composited in the harness. `S` = source/resolved-value derived (not mounted).

| # | Selector / pair | Theme | Predicted | Measured | | |
|---|---|---|---|---|---|---|
| 1 | `.command-palette-option.is-active` title on `--selection` | dark | ≥9.3 | **9.36** | ✓ | M |
| 2 | same | light | ≥13.2 | **13.29** | ✓ | M |
| 3 | `.command-palette-option-hint` (idle) | dark / light | 5.01 / 6.42 | **7.25 / 7.45** (over the popover `--bg-1`, not `--selection` — the active-row hint tokens are identical) | ✓ | M |
| 4 | active-row `box-shadow` | both | `inset 2px 0 0 var(--accent)` | `rgb(79,140,255) 2px 0 0 0 inset` / `rgb(47,111,228) …` | ✓ | M |
| 5 | leading bar `--accent` vs `--selection` | dark / light | 2.6 / 3.6 (per `ui-reference` §2) | **3.51 / 3.74** — **both clear 3:1** | ✓ better than documented (§5 finding) | M |
| 6 | `--selection` vs `--bg-1` | dark / light | ~1.3 | **1.45 / 1.16** | as expected — the bar is load-bearing | M |
| 7 | `.is-disabled` row title **and** hint | dark | both `--text-3` `rgb(107,114,128)` | identical, `box-shadow: none` | ✓ AC19 holds | M |
| 8 | same | light | both `rgb(138,145,158)` | identical, `box-shadow: none` | ✓ | M |
| 9 | **`.is-disabled.is-active` compound** | dark / light | `--text-3` on the 22% tint, `box-shadow: none` | title and hint identical `--text-3`; `box-shadow: none`; composited ≈**2.4 / 2.2** — the sanctioned disabled exemption | ✓ AC5, AC19 | M (class injected — see §5) |
| 10 | `.btn-primary` default | dark | 5.52 | bg `rgb(79,140,255)`, ink `rgb(22,24,29)` = **5.52** | ✓ | M |
| 11 | `.btn-primary` default | light | 4.65 | **4.65** | ✓ | S (tokens) |
| 12 | `.btn-primary:hover` | dark / light | ≥4.5 | **5.99 / 5.15** | ✓ | S (resolved value of the shipped declaration; hover not dispatched) |
| 13 | `--accent-text` token | dark / light | `#16181d` / `#ffffff` | exact | ✓ AC8 | M |
| 14 | `.combobox-option--active` | both | 9.36 / 13.29 + 5.01 / 6.42 | **not reachable** — see §5 | — | S |
| 15 | `.conflict-editor-mode-btn` heights | both | 24px both states | **not mounted**; height-neutral is *provable*: fixed `height: 24px` + global `* { box-sizing: border-box }` (`tokens-and-base.css:141-143`) | ✓ | S |
| 16 | `.wt-copy-toggle button` | both | +2px outer height | **not mounted**; confirmed from source (no `height`, `padding: 2px 8px`, new 1px border) | accepted, see §4 | S |
| 17 | AC1 grep | — | 13 lines | **13** matching declarations (a 14th hit is the P100 comment in `dialogs-forms.css:164`) | ✓ | S |
| 18 | AC2 / AC9 greps | — | 0 | 0 declarations; `--accent-fg` survives only as prose in `conflicts.css:202` | ✓ (AC2 amended) | S |

Visual proof (one screenshot, dark, palette open): the active row reads unambiguously as active —
the `--selection` wash plus the 2px accent bar is a clearly stronger signal than any idle row, and
the four disabled rows read distinctly muted without color being the only cue (the hint dims with
the label). **The label-vs-hint subordination is restored**: `--text-1` label over `--text-2` hint is
a real step again, where P98 had flattened it to 1.00×.

---

## 3. Amended acceptance criteria (verbatim replacements)

**AC2** →

> 2. **AI gate.** `Grep` for `--accent-fg` across the repo returns **zero declarations**. The only
>    remaining occurrence is the historical mention inside the P100 explanatory comment at
>    `src/styles/conflicts.css:202`; prose that records a retired token is not a use of it.

**AC7** →

> 7. **AI gate.** `.btn-primary` composited: default **≥5.5:1** dark / **≥4.6:1** light.
>    **`:hover:not(:disabled)`** ships `background: color-mix(in srgb, var(--accent) 92%,
>    var(--text-1))` and measures **≥4.5:1 in both themes** (realized: 5.99 dark / 5.15 light).
>    §4.2's prescribed remedy (mix toward a literal `#ffffff`) is **retracted**: it measures 4.06:1
>    in light — white ink sits at the luminance ceiling and the light baseline is only 4.65, so
>    *every* fill-lightening hover fails, and the literal hex would also breach AC9. The fill-edge
>    contrast of the hover fill against `--bg-0` stays ≥3:1 (5.99 / 5.15).

**AC11** →

> 11. **AI gate.** No Rust and no fixture file is modified. Three TypeScript files change, all
>     enabling or cleaning up the CSS work and none altering behaviour:
>     `src/components/repoWorkspace/prRefOptions.ts` (**new**, 25 lines — PR base/compare combobox
>     option builders, moved out of `RepoWorkspace.tsx`, each enabled row gaining a `shortOid` hint
>     so that an *enabled* `.combobox-option-hint` exists to verify at all),
>     `src/components/RepoWorkspace.tsx` (imports them; render body shrinks), and
>     `src/graph/colors.ts` (drops the dead `accentText` field — §6-E's NIT, closed here; verified
>     zero remaining consumers). `pnpm gate` green (step 7 only; intermediate rounds use
>     `--frontend`).

**AC16** →

> 16. **USER CHECKPOINT.** Light theme: **no visible change anywhere at rest** — Recipe A's light
>     figures and Recipe B's light ink are both unchanged from today, so any perceived at-rest
>     difference is a defect. **One deliberate exception:** `.btn-primary:hover` now goes slightly
>     *deeper* rather than brighter (AA 5.13:1, where brightening measured 3.95:1). Confirm it still
>     reads as "hover", not as "pressed" or "disabled".

**New AC19** (was implicit) →

> 19. **AI gate.** `src/graph/colors.ts` no longer declares or reads `accentText`, and `Grep` for
>     `accentText` across `src/` returns zero matches.

**§4.2 remedy paragraph** (last paragraph of §4.2) →

> **Retracted 2026-09-01 (this review).** The prescribed fill-only lift toward `#ffffff` does not
> work: light-theme white ink is at the ceiling and the baseline is 4.65:1, so any fill-lightening
> hover falls below 4.5 (measured 4.06). The shipped and sanctioned remedy is
> `background: color-mix(in srgb, var(--accent) 92%, var(--text-1))` — mixing toward the theme's own
> `--text-1` brightens the fill in dark and deepens it in light, leaves the ink untouched, and adds
> no literal hex: **5.99:1 dark / 5.15:1 light**. This is the general recipe for hovering *any* kept
> hue fill; see §5's `ui-reference.md` addendum.

**§7** → replace "**None.** P100 creates no component and touches no `.tsx`." with:

> **One extracted module, no new component.** `src/components/repoWorkspace/prRefOptions.ts` (new,
> 25 lines) holds the PR base/compare combobox option builders previously inline in
> `RepoWorkspace.tsx`; it is what makes an *enabled* `.combobox-option-hint` exist in the app at all
> (before P100 the only combobox hint was `'checked out'` on disabled worktree rows), and hints are
> computed in components, never carried over IPC, so no fixture could have produced one. Files
> changed: `tokens-and-base.css`, `dialogs-forms.css`, `search.css`, `conflicts.css`,
> **`controls.css` (1 declaration — the AC7 hover, see the §4.2 retraction; §7's earlier "no change"
> is superseded)**, `src/graph/colors.ts` (dead field removed), `RepoWorkspace.tsx`,
> `prRefOptions.ts`, `docs/contracts/ui-reference.md`. No file approaches the ~500-line limit.

**§8** → replace "**No string changes.**" with:

> **One string-shaped addition, no prose.** The PR base/compare combobox rows gain a short-oid hint
> (`shortOid(tip)`, e.g. `a1b2c3d`) — the same disambiguator the command palette already puts on its
> branch actions. No label, tooltip, error message, empty state or `aria-label` changes. Nothing
> here is destructive, so no confirmation copy is in scope.

---

## 4. Box-model ruling

- **`.conflict-editor-mode-btn`: accepted, height-neutral, and now *provably* so.** `border: none` →
  `1px solid transparent` cannot change the outer box: the rule sets `height: 24px` and
  `tokens-and-base.css:141-143` applies `box-sizing: border-box` globally, so the border eats
  padding, not height. The 10px horizontal padding loses 1px per side of content box — invisible on
  an 11px two-word label. Correct disclosure by senior-dev.
- **`.wt-copy-toggle button`: accepted as a disclosed +2px.** No `height` is set, so the outer box
  really does grow from 2px to 3px of vertical box per side. `.wt-copy-toggle` is an auto-height
  `inline-flex`, so `overflow: hidden` clips nothing — it exists only to keep the 6px radius on the
  children. A 2px taller inline toggle inside a dialog form row is not a defect.
  **NIT (follow-up, not blocking):** for exact pre-P100 parity set `padding: 1px 7px` on
  `.wt-copy-toggle button` — 1px border + 1px padding reproduces the old 2px, and 1+7 the old 8.
  Only worth doing if the USER CHECKPOINT shows the toggle now sitting taller than the controls
  beside it in the new-worktree dialog.
- **`box-shadow: none` on both disabled+active compounds: correct and verified mounted.** A
  neutralised row keeping an accent bar would have claimed an affordance it does not have.

---

## 5. Findings and follow-ups (none blocking)

1. **`ui-reference.md` §2 carries a wrong figure.** The `--accent`-as-decoration-on-`--selection`
   bullet states **2.6:1 dark / 3.6:1 light**. Measured from the shipped hexes: **3.51:1 dark /
   3.74:1 light** — the leading bar *clears* the 3:1 graphics threshold in **both** themes, so it is
   a compliant non-text carrier rather than "decorative only". That bullet is outside P100's hunk
   confinement (§5.5) and I am **not** amending it in this pass; it is a clean fold-in for **P101's**
   token-hygiene sweep. My four applied hunks are unaffected — none of them cites that figure.
2. **The `.is-disabled.is-active` compound is not reachable by keyboard.** Arrowing past a disabled
   palette row skips it, and pointer-over does not set active on it, so I measured it by injecting
   `is-active` onto the disabled Pull row and reading composited styles. The CSS is correct, but it
   may be effectively dead in the shipped palette — a **finding for a future pass**, not P100's:
   either the rule is genuinely unreachable (delete it, and AC5/AC19 with it) or a filter-reset path
   can land the index on a disabled row (keep it). Do not act on this inside P100.
3. **The PR create form is behind a forge connection in the harness.** With `?op=merge`, the Pull
   requests tab renders the "Connect to github.com" token screen, so senior-dev's traced route
   (PR tab → New PR → Base/Compare combobox) does **not** reach the combobox without entering a
   personal access token — which I will not do. **The `shortOid` hint and the enabled/active
   `.combobox-option` states are therefore USER CHECKPOINT items, not AI-gate items.** The recipe
   itself is verified by equivalence: `.combobox-option--active` and
   `.command-palette-option.is-active` set identical tokens, and the palette pair measured 9.36 /
   13.29 and the hint step mounted.
4. **Stale comment.** `RepoWorkspace.tsx:1433-1435` still carries the pre-extraction P78 comment
   ("branch suggestions + base hint for the PR create form…") immediately above the new P100 pointer
   at :1436. Redundant, two comments for one `useMemo` pair. NIT.
5. **AC10 / AC12 / AC13 not re-verified here** — AC10 was verified by the orchestrator; AC12
   (density invariance) and AC13 (long content) are unchanged-geometry claims and no declaration in
   the shipped diff touches padding, font-size or height on either row type, so they hold by
   inspection.

### `ui-reference.md` addendum — append to hunk 3, recipe 2 (only new text; no existing hunk changes)

> Hovering a kept hue fill: **never `filter: brightness()`** — it moves ink and fill together, so it
> fails in whichever theme has least headroom (`.btn-primary` light went 4.65 → 3.95). Use
> `background: color-mix(in srgb, <hue> 92%, var(--text-1))`: it brightens in dark, deepens in
> light, leaves the ink alone, and adds no literal hex (P100: **5.99:1** dark / **5.15:1** light).
> The same device is what P102 should use for `.btn-danger`.

---

## 6. USER CHECKPOINT (not mine to certify)

AC14 (active row still obvious while arrowing fast), AC15 (dark primary buttons read loudest),
amended AC16 (**light `.btn-primary:hover` reads as hover, not pressed** — new), AC17 (segmented
toggles read as one language), AC18 (20k-row scroll feel; `requestAnimationFrame` is paused in the
hidden preview), plus, newly: the **PR base/compare combobox short-oid hint** and the
**new-worktree copy toggle's +2px height** in the native window (§5.3, §4).
