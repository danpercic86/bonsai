# P98 — `--text-3` read-text sweep (UI contract)

Owner: `ui-designer`. Implementer: `senior-dev`. Read-only inputs: `docs/contracts/ui-reference.md`
§2 (tokens + contrast notes), `docs/contracts/P95-a11y-ui.md` §3 (the enabled-control sweep and its
§3.4 deferral list, which this milestone closes).

**Colour-only milestone. No new tokens. No new components. No new files. No geometry, spacing,
font-size, weight, letter-spacing, radius, hover, motion or copy changes** — with exactly one
sanctioned adjacent exception (§4, the undefined `--border-0`), which is a *border-style* repair,
not a design change.

Because nothing moves and nothing new is drawn, the usual per-surface cozy/compact and dark/light
**geometry** tables do not apply (same disclaimer as P95). The only theme-dependent content here is
the contrast table in §2, given for **both** themes.

`ui-reference.md` §2 was updated in this same design pass to match this contract (§6).
`senior-dev` does not touch `docs/`.

---

## 0. Scope and class boundary

P95 swept the **enabled-interactive-control** class of `--text-3` misuse and wrote the governing
rule into `ui-reference.md` §2. P98 sweeps the **remaining, distinct** class: `--text-3` used for
text the user must actually **read**, governed by the long-standing §2 **4.5:1 AA text** rule — not
by the 3:1 graphics bar, and not by the enabled-control rule.

Seven selectors, two of which carry a state override. Nine declarations total. That is the whole
milestone. Anything not in the §3 table is out of scope, including the ~140 permitted decorative
`--text-3` uses (uppercase section labels that duplicate visible structure, dividers, disabled
glyphs) and including the accent-fill shortfall surfaced in §5-A.

**One eighth candidate exists and is deliberately excluded.** `ui-reference.md` §12.9 specifies
`.pr-merge-method-desc` — the one-line description of the selected merge method in the PR merge
dialog — in `--text-3`. By the rule this milestone makes explicit that is read text (it is the only
sentence explaining what the chosen merge method will do). It is **not** in P98 because the
orchestrator locked this milestone's scope at the seven selectors from P95 §3.4, and because fixing
it requires editing `ui-reference.md` §12.9, which would take this pass's reference diff outside §2.
Filed for the follow-up alongside §5-A — see §5-D.

### 0.1 Why two of these are read-text despite looking decorative

Pre-empting the obvious reviewer objection:

- **`.conflict-editor-split-label`** is 11px, uppercase, letter-spaced — the exact shape §2 sanctions
  as "decorative section label". It is **not** decorative: in the merge editor it is the *sole*
  wayfinder telling the user which split pane is OURS / THEIRS / OUTPUT. Getting it wrong means
  resolving a conflict in the wrong direction. It duplicates no other visible structure.
- **`.wtctx-blocked`** is the explanation of *why* an action is unavailable. Italic + dim, but it is
  the only text that answers the user's question. (Note it is not a `:disabled` control label — the
  P95 disabled exemption does not reach it.)
- **`.combobox-option-hint` / `.command-palette-option-hint`** are the worst offenders: they carry
  the disambiguating text a user reads *in order to choose* (the branch a worktree points at, the
  shortcut a command is bound to, the scope of a duplicate-named option). Read text inside an
  enabled option — they fail both the P98 rule and, arguably, P95's.

---

## 1. Measurement method — what is measured and what is derived

Measured in the mock harness (`pnpm dev:mock`, port 1420, `VITE_MOCK_IPC=1`) on **2026-09-01** via
`javascript_tool`, with `data-theme` toggled between `dark` and `light` and every token resolved
through a real element's computed `color`:

| Measured (harness) | Derived (from source) |
|---|---|
| Every token's computed sRGB value in both themes, and therefore every contrast ratio in §2. | The **identity** of each selector's backdrop — taken from the `background` declaration in the CSS source (§3 names the rule for each). |
| The alpha composite of `rgba(255, 255, 255, 0.8)` over the real `--accent` in both themes, and its ratio against that fill (§2.2). | — |
| That `var(--border-0)` is **undefined**: a probe with `border: 1px solid var(--border-0)` computes to `border-left-style: none; border-left-width: 0px` (§4). | — |

**None of the seven selectors could be reached mounted in the harness.** The default mock state opens
with no repo tab, so the command palette, combobox lists, diff overlay, diff file tree, conflict
editor and worktree-context dialog are all unmounted; `document.querySelectorAll` returned 0 for all
eight probes. The ratios in §2 are therefore **computed from measured token values against the
CSS-declared backdrop**, not read off a composited pixel.

This is safe here in a way it was not in P95, and the reason must be checked by the reviewer rather
than assumed: **every backdrop in §3 is a fully opaque token** (`--bg-0`, `--bg-1`, `--bg-2`,
`--selection`, `--accent`) declared directly on an ancestor with no intervening translucency. The
P95 failure mode — estimating against a transparent backdrop when the real one was opaque — is the
inverse case and does not arise. The one genuinely translucent value in scope,
`rgba(255, 255, 255, 0.8)`, is composited explicitly in §2.2.

**AC12 requires senior-dev to confirm the composited backdrop on a mounted instance** of each
selector, which the harness *can* do once a repo fixture is open. Do not skip it.

---

## 2. Measured token contrast

### 2.1 Tokens and ratios (measured 2026-09-01)

Computed sRGB: `--text-3` `#6b7280` dark / `#8a919e` light · `--text-2` `#a8adb8` / `#4b515c` ·
`--text-1` `#e8eaed` / `#1c1f24` · `--bg-0` `#16181d` / `#ffffff` · `--bg-1` `#1d2026` / `#f6f7f9` ·
`--bg-2` `#262a31` / `#eceef2` · `--selection` `#2a3b57` / `#dbe7ff` · `--accent` `#4f8cff` /
`#2f6fe4` · `--accent-text` `#ffffff` / `#ffffff`.

| Backdrop | `--text-3` dark | `--text-3` light | `--text-2` dark | `--text-2` light |
|---|---|---|---|---|
| `--bg-0` | **3.67:1** ✗ | **3.17:1** ✗ | **7.89:1** ✓ | **7.98:1** ✓ |
| `--bg-1` | **3.38:1** ✗ | **2.96:1** ✗ | **7.25:1** ✓ | **7.45:1** ✓ |
| `--bg-2` | **2.98:1** ✗✗ | **2.73:1** ✗✗ | **6.40:1** ✓ | **6.87:1** ✓ |
| `--selection` | **2.33:1** ✗✗ | **2.55:1** ✗✗ | **5.01:1** ✓ | **6.42:1** ✓ |

✗ = below the 4.5:1 text bar. ✗✗ = below even the 3:1 graphics bar. `--text-2` clears 4.5:1 on
every backdrop in scope, in both themes, with ≥0.5 of margin. The `--selection` row is **new**
information relative to `ui-reference.md` §2 and is added there in this pass.

`--text-1` for reference: **9.36:1** dark / **13.29:1** light on `--selection`; **13.54** / **15.42**
on `--bg-1`. The `--text-2` ↔ `--text-1` gap is what carries the hierarchy claim in §3.3.

*Note on the last two decimals:* this pass measured `--text-2` on `--bg-0` as **7.89 / 7.98**;
`ui-reference.md` §2 carries the established **7.90 / 7.99** from the P95 pass. That is rounding
noise in the luminance arithmetic, not a discrepancy — the reference figures were deliberately left
alone rather than churned by ±0.01.

### 2.2 The active-option fill — the honest numbers

Both `*-hint` active overrides paint `rgba(255, 255, 255, 0.8)` on a `background: var(--accent)`
row. Composited (measured): `rgb(220,232,255)` dark, `rgb(213,226,250)` light.

| Foreground on the `--accent` fill | Dark | Light |
|---|---|---|
| `rgba(255,255,255,0.8)` (today's hint) | **2.61:1** ✗ | **3.56:1** ✗ |
| `--accent-text` / `#fff` (today's *label*, and P98's prescribed hint) | **3.22:1** ✗ | **4.65:1** ✓ |

**The row's own primary label already fails 4.5:1 in dark at 3.22:1.** No hint colour can pass on
this fill in the dark theme, because white is the ceiling and white is 3.22. That is an
**accent-as-fill** defect, a different class from `--text-3` read-text, and it is deliberately **not**
fixed here — see §5-A for the worked, measured alternative and my recommendation.

---

## 3. The mandatory change list

Locate by **selector**, not by line number (lines drift). In every row the change is
`color: var(--text-3)` → `color: var(--text-2)` and **nothing else**.

| # | Selector | File | Backdrop (CSS-declared) | Now (dark/light) | → Token | After (dark/light) | ≥4.5:1? |
|---|---|---|---|---|---|---|---|
| 1 | `.diff-overlay-kind` | `src/styles/diff.css` | `.diff-overlay { background: var(--bg-0) }` (opaque) | 3.67 / 3.17 ✗ | `--text-2` | **7.89 / 7.98** | ✓ both |
| 2 | `.diff-tree-count` | `src/styles/diff-browser.css` | `.diff-tree-root` — idle inherits the host pane (`--bg-1` in Commit/Compare panel, `--bg-0` in `.diff-browser`); **hover `--bg-2`**; `.diff-tree-selected` **`--selection`** | 3.38 / 2.96, hover 2.98 / 2.73, selected **2.33 / 2.55** ✗✗ | `--text-2` | idle **7.25 / 7.45**, hover **6.40 / 6.87**, selected **5.01 / 6.42** | ✓ both, all three states |
| 3 | `.conflict-editor-split-label` | `src/styles/conflicts.css` | `.conflict-editor-split-labels { background: var(--bg-1) }` | 3.38 / 2.96 ✗ | `--text-2` | **7.25 / 7.45** | ✓ both |
| 4 | `.wtctx-branch` | `src/styles/worktrees.css` | `.dialog-card { background: var(--bg-1) }` | 3.38 / 2.96 ✗ | `--text-2` | **7.25 / 7.45** | ✓ both |
| 5 | `.wtctx-blocked` | `src/styles/worktrees.css` | `.dialog-card` → `--bg-1` | 3.38 / 2.96 ✗ | `--text-2` | **7.25 / 7.45** | ✓ both |
| 6 | `.combobox-option-hint` (idle) | `src/styles/dialogs-forms.css` | `.combobox-list { background: var(--bg-1) }`; there is **no** `.combobox-option:hover` background, so idle is the only non-active state | 3.38 / 2.96 ✗ | `--text-2` | **7.25 / 7.45** | ✓ both |
| 6a | `.combobox-option--active .combobox-option-hint` | `src/styles/dialogs-forms.css` | `.combobox-option--active { background: var(--accent) }` | `rgba(255,255,255,.8)`: 2.61 / 3.56 ✗ | **`--accent-text`** | **3.22 / 4.65** | light ✓ · **dark ✗ — fill ceiling, see §5-A** |
| 7 | `.command-palette-option-hint` (idle) | `src/styles/search.css` | `.command-palette { background: var(--bg-1) }`; no option `:hover` background | 3.38 / 2.96 ✗ | `--text-2` | **7.25 / 7.45** | ✓ both |
| 7a | `.command-palette-option.is-active .command-palette-option-hint` | `src/styles/search.css` | `.is-active { background: var(--accent) }` | `rgba(255,255,255,.8)`: 2.61 / 3.56 ✗ | **`--accent-text`** | **3.22 / 4.65** | light ✓ · **dark ✗ — fill ceiling, see §5-A** |

Rows 6a and 7a additionally **remove two hardcoded `rgba(255, 255, 255, 0.8)` literals** — hardcoded
colour in a stylesheet is a defect per §2 — and make the hint's colour identical to the row's own
`color: #fff`. (That `#fff` on the two `--active` rules is itself a hardcoded literal that should be
`var(--accent-text)`; it is part of the §5-A follow-up, not P98, because touching it invites the
whole fill question. If senior-dev wants to swap those two `#fff` → `var(--accent-text)` in passing
it is a zero-risk token hygiene win — it is the same computed value in both themes — but it is
**optional**, not an AC.)

### 3.1 Explicitly NOT changed — the disabled exemption still holds

Leave `var(--text-3)` exactly as it is at:

- `.combobox-option--disabled` and `.combobox-option--disabled.combobox-option--active`
  (`dialogs-forms.css`)
- `.command-palette-option.is-disabled` and `.command-palette-option.is-disabled.is-active`
  (`search.css`)
- every selector in P95 §3.3's exempt set.

These are disabled states: dimming *is* the signal, and it is carried independently by the
`aria-disabled`/`disabled` attribute and `cursor: default`, so colour is not the sole carrier. Note
that after P98 the **enabled** hint is `--text-2` and the **disabled** option stays `--text-3`, a
measured **2.15:1 dark / 2.52:1 light** apparent-luminance step (P95 §finding) — the
enabled/disabled distinction survives, and in fact becomes *clearer* than it is today, since today
both are `--text-3`.

### 3.2 Colour-only confirmation (explicit, per the requirement)

Every one of the nine declarations above changes **only** the `color` property's value. Confirmed
against the source for each: no `font-size`, `font-style`, `font-weight`, `letter-spacing`,
`text-transform`, `flex`, `padding`, `margin`, `border`, `border-radius`, `background`,
`font-variant-numeric`, `opacity` or `transition` declaration is added, removed or altered. In
particular `.wtctx-blocked` keeps `font-style: italic`, `.conflict-editor-split-label` keeps its
`text-transform: uppercase` + `letter-spacing: 0.04em`, and
`.command-palette-option-hint` keeps `font-variant-numeric: tabular-nums`. No focus, hover or active
rule is added; all of these are non-interactive text or live inside a row whose own states are
unchanged.

The **one** non-colour change in this milestone is §4, and it is called out separately.

### 3.3 What must NOT change: the subordinate reading

All seven selectors are **secondary/supporting** text and must still read as subordinate to their
primary label after the swap. How I verified that this holds:

1. **The hierarchy is not carried by colour alone in any of these cases, and never was.** Each pairs
   a `--text-1` primary at 12–13px with an 11–12px secondary. Sizes are untouched, so the size step
   survives by construction.
2. **The colour step survives and stays large.** On `--bg-1`, primary `--text-1` measures
   **13.54:1** dark / **15.42:1** light against **7.25 / 7.45** for the new `--text-2` — a ratio-of-
   ratios of ~1.87× dark / ~2.07× light. The old `--text-3` step was ~4.0× / ~5.2×. Less emphatic,
   still unmistakable: `--text-2` is the token the app already uses for exactly this role in
   ~40 places (`.diff-browser-title`, commit metadata, timestamps, settings help text), always
   beside a `--text-1` primary, and that pairing is the house convention. Consistency argues *for*
   the swap, not against it.
3. **Nothing gains emphasis it should not have.** `--text-2` is never the primary colour anywhere in
   Bonsai, so no swapped element can be mistaken for a primary label. No element becomes brighter
   than its own row's primary.
4. **Rows 6a/7a get *dimmer* relative to nothing** — they go from 80% white to 100% white on the
   accent fill, i.e. they become equal in colour to the row's label. Here, and only here, the colour
   step is eliminated. Subordination is then carried by **11px vs 13px**, by `flex: none` right-edge
   placement vs the flexible left-aligned title, and (in the palette) by tabular-nums. That is
   sufficient: the two are unambiguously different roles by position and size, and this is the same
   arrangement the *selected graph row* and *selected sidebar row* already use. This is a deliberate
   judgement call, recorded in §5-A.
5. **AC13 makes it checkable perceptually** (USER CHECKPOINT), because "still reads as subordinate"
   is a perceptual claim and I will not assert it from numbers alone.

---

## 4. Sanctioned adjacent exception — the undefined `--border-0`

**Measured**: `var(--border-0)` is not defined on `:root` or `[data-theme]`. A probe with
`border: 1px solid var(--border-0)` computes to `border-left-style: none; border-left-width: 0px`.
The token appears **5 times, in `src/styles/conflicts.css` only** — including on the two rules that
frame the selector this milestone is fixing:

- `.conflict-editor-split-labels { border-bottom: 1px solid var(--border-0) }`
- `.conflict-editor-split-label + .conflict-editor-split-label { border-left: 1px solid var(--border-0) }`

So the merge editor's split-label bar has **no bottom border and no divider between OURS and
THEIRS** today. That is not a colour bug, but it is load-bearing for this contract: the split labels
are the sole pane wayfinder (§0.1), and the divider is the other half of that wayfinding. Shipping
the colour fix while the divider stays invisible fixes half the defect.

**Prescribed, and this is the one non-colour change in P98:** replace all **5** occurrences of
`var(--border-0)` in `src/styles/conflicts.css` with `var(--border)`. Do not define a `--border-0`
token — there is no design need for a second border token, and the existing `--border` (`#2c313a`
dark / `#dcdfe5` light) is the app-wide 1px pane/row border. Ratio against `--bg-1`: **1.25:1** dark
/ **1.14:1** light — below 3:1, which is **correct and intentional**: this is a decorative
separator, not a UI-component boundary that conveys state, so the WCAG 1.4.11 3:1 bar does not
apply to it, and it matches every other divider in the app exactly.

This is called out as an exception rather than smuggled in: it changes rendered geometry (borders
appear where none render today), so a reviewer diffing screenshots will see it and must know it is
sanctioned. Justification: it is a five-line dead-token repair confined to the same file and the
same feature as change #3, and it completes the same defect.

---

## 5. Flagged for the orchestrator

### A. The `--accent` fill fails AA in dark — recommend a P99 follow-up, not a P98 expansion

**Finding (measured).** `#fff` on `--accent` is **3.22:1** dark / 4.65:1 light. Every
accent-filled active/selected row in the app — combobox options and command-palette options at
minimum — has a **primary label below 4.5:1 in the dark theme**, which is the default theme. P98
raises its two hints to that same 3.22 ceiling; it cannot go higher.

**My recommendation: fix it in its own increment, not here.** Reasons: (a) it is a different defect
class (hue-as-fill vs `--text-3` read-text), and the orchestrator split P95/P98 precisely to keep
one class per milestone; (b) it changes the app's most-used selection affordance's visual identity,
which is a product call, not a sweep; (c) it needs a survey of *all* accent-filled rows, not just
these two.

**The worked alternative, measured, so the follow-up starts with numbers.** Switch the two active
rules from `background: var(--accent); color: #fff` to the house selected-row recipe —
`background: var(--selection)` with label `var(--text-1)` and hint `var(--text-2)`:

| | Dark | Light |
|---|---|---|
| `--text-1` on `--selection` (label) | **9.36:1** ✓ | **13.29:1** ✓ |
| `--text-2` on `--selection` (hint) | **5.01:1** ✓ | **6.42:1** ✓ |

Both pass, the colour-based label/hint hierarchy is *restored*, it aligns the palette with the
graph/sidebar selected-row language, and §2's "`--accent` as text never sits on a `--selection`
fill" rule is not engaged (no accent text is involved). Cost: the accent-filled active row is a
strong, well-liked affordance and this makes it quieter; keeping the `inset 2px 0 0 var(--accent)`
leading bar already used by `.diff-tree-selected` would preserve the punch. Suggested title:
**"P99 — accent-fill row contrast"**.

**If the orchestrator would rather fix it now**, the change is 4 declarations in 2 files and I would
fold it into P98 as an explicitly-labelled second class, with rows 6a/7a becoming `--text-2` at
5.01 / 6.42 instead of `--accent-text` at 3.22 / 4.65. Say the word. **My recommendation stands:
defer.**

### B. `--border-0` (§4) — confirm the exception

I have specced the fix. If the orchestrator prefers P98 to be literally colour-only, drop §4 and
file it as a follow-up; the seven colour changes are independent of it. **Recommendation: keep §4
in** — it is 5 lines, same file, same defect.

### C. Harness reachability

None of the seven selectors mount in the default mock state (no repo open). AC12's composited-
backdrop verification therefore requires senior-dev to open a repo fixture and reach each surface.
If any surface has no reachable fixture at all, that selector's verification degrades to a computed-
style check on a mounted-in-isolation instance, and the *backdrop* half must be asserted from the
CSS source — say so in the implementation report rather than reporting it as measured. No new
fixtures are requested by this contract.

### D. `.pr-merge-method-desc` — the eighth read-text candidate, out of scope by decision

`ui-reference.md` §12.9 specifies the PR merge dialog's per-method description line in `--text-3`
on the `.dialog-card` `--bg-1` surface — **3.38:1** dark / **2.96:1** light, and it is read text by
the rule §6 makes explicit (it is the only sentence telling the user what the selected merge method
does). It is excluded from P98 because the seven-selector scope was fixed by the orchestrator and
because fixing it means editing `ui-reference.md` §12.9, which would push this pass's reference diff
outside §2. **Recommendation:** bundle it with the §5-A follow-up as a one-line `--text-2` swap in
`src/styles/forge-pr.css` (or wherever `.pr-merge-method-desc` lives — locate by selector) plus the
matching §12.9 sentence. Do **not** let senior-dev fix it inside P98; it would break AC10.

---

## 6. `ui-reference.md` edits — already applied in this pass

Applied by `ui-designer` at contract time, so the reference never contradicts the contract. **All
five edits are confined to §2** (roughly lines 69–145); nothing outside §2 was touched.

1. **The stale header claim** — previously "One known AA shortfall remains (`--text-3`)" — is
   replaced. It could not become "no shortfalls remain", because §5-A is a *measured, shipping*
   3.22:1. The `--text-3` family is now stated as **closed** (P95 + P98), and the "one known AA
   shortfall remains" sentence now names **the `--accent` fill** instead.
2. **§2 `--text-3` bullet** — added the measured `--selection` row (**2.33:1** dark / **2.55:1**
   light for `--text-3`; **5.01** / **6.42** for `--text-2`), and replaced the parenthetical listing
   the seven residual read-text selectors with an explicit "swept by P98; a new occurrence is a
   defect" statement in the new read-text bullet.
3. **New §2 read-text rule bullet** — parallel to P95's enabled-control sentence: read text is
   judged at **4.5:1 against its actual composited backdrop in both themes**, "small, uppercase and
   letter-spaced" does not by itself make text decorative (the test is whether the user must read it
   to act), the seven swept selectors are named, and each hover/selected/active state's backdrop is
   measured separately.
4. **New §2 KNOWN-SHORTFALL bullet** — the accent-fill figures (`#fff` on `--accent` = 3.22 dark /
   4.65 light; `rgba(255,255,255,.8)` = 2.61 / 3.56), the rule that a hardcoded `rgba(255,…)` over
   a hue fill is never the way to dim text on a fill, and the measured `--selection` recipe as the
   sanctioned pattern for a new selected row carrying read text.
5. **New §2 undefined-token bullet** — `var(--border-0)` was never a real token, appeared 5× in
   `conflicts.css`, and computed to `border-style: none; border-width: 0px`; there is **one** border
   token, `--border`; confirm every `var(--…)` a new stylesheet names actually resolves (§4).

*The P95 enabled-control bullet also gained one sentence* recording that the enabled/disabled
distinction survives the swap (`--text-2` vs `--text-3` = 2.15:1 dark / 2.52:1 light) — still inside
§2.

---

## 7. Acceptance criteria

Harness = verifiable in the mock browser harness (`pnpm dev:mock`, `VITE_MOCK_IPC=1`) via
`javascript_tool` / `read_page` / grep. UC = **USER CHECKPOINT** (human perception).

| # | Criterion | Where |
|---|---|---|
| AC1 | `.diff-overlay-kind`, `.diff-tree-count`, `.conflict-editor-split-label`, `.wtctx-branch`, `.wtctx-blocked`, `.combobox-option-hint`, `.command-palette-option-hint` each declare `color: var(--text-2)`. Exactly these seven rules changed. | Harness (grep) |
| AC2 | `.combobox-option--active .combobox-option-hint` and `.command-palette-option.is-active .command-palette-option-hint` each declare `color: var(--accent-text)`. Neither contains `rgba(` any more. | Harness (grep) |
| AC3 | `src/styles/` contains **zero** `rgba(255, 255, 255, …)` literals introduced or retained by these two rules, and P98 introduces **no** hardcoded hex or rgb colour anywhere. | Harness (grep) |
| AC4 | For each of the nine declarations, `getComputedStyle` on a **mounted** instance returns a `color` equal to the computed `--text-2` (or `--accent-text`) value, in **both** `data-theme` states. | Harness |
| AC5 | `--text-3` no longer appears as the `color` of any **read text** in `src/styles/`, with the single sanctioned exception of `.pr-merge-method-desc` (§5-D, out of scope by decision and to be left untouched). Remaining `var(--text-3)` occurrences are limited to: that exception, the §3.1 disabled set, the P95 §3.3 exempt set, and decorative uses (uppercase section labels duplicating visible structure, dividers, placeholder/empty-state copy). Each remaining occurrence is classifiable into one of those buckets; the implementation report lists any that is not. | Harness (grep) |
| AC6 | The §3.1 disabled selectors — `.combobox-option--disabled`, `.combobox-option--disabled.combobox-option--active`, `.command-palette-option.is-disabled`, `.command-palette-option.is-disabled.is-active` — **still** declare `var(--text-3)`, unchanged. | Harness (grep) |
| AC7 | In the combobox and the command palette, a **disabled** option's text is visibly dimmer than an **enabled** option's hint (`--text-3` vs `--text-2`), and both keep `cursor: default` / the `disabled` attribute respectively. | Harness + UC |
| AC8 | All five `var(--border-0)` occurrences in `src/styles/conflicts.css` are now `var(--border)`; `--border-0` appears nowhere in `src/`. A probe of `.conflict-editor-split-label + .conflict-editor-split-label` computes `border-left-width: 1px` and `border-left-style: solid` (was `0px` / `none`). | Harness |
| AC9 | No `--border-0` token is defined in `src/styles/tokens-and-base.css` or anywhere else. | Harness (grep) |
| AC10 | **senior-dev's implementation diff** touches **only** `src/styles/diff.css`, `diff-browser.css`, `conflicts.css`, `worktrees.css`, `dialogs-forms.css`, `search.css` — six CSS files, no `.tsx`, no `.ts`, no fixtures, and nothing under `docs/`. (The two `docs/contracts/` files in the working tree are `ui-designer`'s and are not part of the implementation diff.) | Harness (git diff --stat) |
| AC11 | Within those files, every changed line is a `color:` value (7 rules), a `color:` value on an active-state override (2 rules), or a `var(--border-0)` → `var(--border)` substitution (5 lines). **No** size, weight, style, spacing, padding, margin, radius, background, transition, hover or focus declaration changed. | Harness (diff read) |
| AC12 | For each of the nine declarations, the **actual composited backdrop** of a mounted instance is walked to the first opaque ancestor background and matches §3's declared backdrop; the recomputed ratio matches §3's "After" figure within ±0.05. Any selector whose surface has no reachable fixture is reported as such, not as measured. | Harness |
| AC13 | Each swapped element still reads as clearly **subordinate** to its primary label in both themes: the diff overlay's kind chip beside the file path, the file-tree count beside "All files", the split labels above the merge panes, the worktree branch/blocked lines beside the worktree name, and the two option hints beside their titles. In particular the active palette/combobox row's hint does not read as a second title. | **UC** (perceptual) |
| AC14 | The merge editor's split-label bar now shows a bottom border and a divider between panes, matching every other 1px divider in the app; nothing else in the conflict editor shifted position. | **UC** |
| AC15 | `.diff-tree-count` remains legible in all three of its row states — idle, hover (`--bg-2`) and selected (`--selection`) — in both themes. | Harness (AC4 per state) + **UC** |
| AC16 | Long-content behaviour unchanged: a 60-char branch name in `.wtctx-branch`, a deep path in `.diff-overlay-kind`'s row, and a long shortcut hint in a narrow palette still truncate/clip exactly as they do today (no wrapping change — no layout property moved). | Harness |
| AC17 | `ui-reference.md` (already updated by `ui-designer`) is consistent with the shipped code: **(a)** the stale claim that `--text-3` is the remaining AA shortfall is gone — §2 now states the `--text-3` family closed by P95 + P98; **(b)** the "one known AA shortfall remains" sentence now names the **`--accent` fill**, with the measured 3.22 / 4.65 figures in their own §2 bullet; **(c)** §2 lists the `--selection` figures (2.33 / 2.55 for `--text-3`, 5.01 / 6.42 for `--text-2`); **(d)** the seven-selector residual list is marked swept by P98; **(e)** the undefined-`--border-0` finding is recorded. Verification only — senior-dev does not edit `docs/`. | Harness (file read) |
| AC18 | `pnpm gate` tiers green: no snapshot/visual test asserts a `--text-3` computed colour on any of the nine declarations. Any test that does is updated in the same increment. | Harness |

Harness-visible: AC1–AC6, AC8–AC12, AC16–AC18. USER CHECKPOINT: AC7 (perceptual half), AC13, AC14,
AC15 (perceptual half).

### 7.1 Harness fixture states needed

**No new fixtures.** The existing mock states cover every surface once a repo tab is open: a single-
file diff overlay (#1), a Commit/Compare panel file tree with idle + hovered + selected rows (#2), a
conflicted file opened in the merge editor (#3), the worktree-context dialog with at least one
blocked row (#4/#5), any combobox with hinted options (#6/#6a), and the command palette with a
bound-shortcut option and a disabled option (#7/#7a, AC7). If the conflict editor or the worktree
dialog has no reachable mock state, note it per §5-C — a computed-style check does not need the real
feature mounted, but the *backdrop* half of AC12 does.
