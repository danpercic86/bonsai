# P98 — `--text-3` read-text sweep (UI contract)

Owner: `ui-designer`. Implementer: `senior-dev`. Read-only inputs: `docs/contracts/ui-reference.md`
§2 (tokens + contrast notes), `docs/contracts/P95-a11y-ui.md` §3 (the enabled-control sweep and its
§3.4 deferral list, which this milestone closes).

> **READ §8 FIRST.** §8 is the 2026-09-01 amendment record. It supersedes §0's scope statement,
> §5-D's exclusion decision, and ACs 5 / 10 / 12 below, and it adds the P101 hand-off. The original
> §0–§7 text is left intact as the design reasoning trail; where §8 and §0–§7 disagree, §8 wins.
> **§8.4 is the only work item still outstanding.**

**Colour-only milestone. No new tokens. No new components. No new files. No geometry, spacing,
font-size, weight, letter-spacing, radius, hover, motion or copy changes** — with exactly one
sanctioned adjacent exception (§4, the undefined `--border-0`), which is a *border-style* repair,
not a design change.

Because nothing moves and nothing new is drawn, the usual per-surface cozy/compact and dark/light
**geometry** tables do not apply (same disclaimer as P95). The only theme-dependent content here is
the contrast table in §2, given for **both** themes.

`ui-reference.md` §2 was updated in this same design pass to match this contract (§6), and amended
again at review time (§8.6).

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
*(Amended by §8.2 — eight selectors, ten declarations. And the "~140 permitted decorative uses"
phrasing was itself an overclaim: they are **unaudited**, not permitted — §8.5.)*

**One eighth candidate exists and is deliberately excluded.** `ui-reference.md` §12.9 specifies
`.pr-merge-method-desc` — the one-line description of the selected merge method in the PR merge
dialog — in `--text-3`. By the rule this milestone makes explicit that is read text (it is the only
sentence explaining what the chosen merge method will do). It is **not** in P98 because the
orchestrator locked this milestone's scope at the seven selectors from P95 §3.4, and because fixing
it requires editing `ui-reference.md` §12.9, which would take this pass's reference diff outside §2.
Filed for the follow-up alongside §5-A — see §5-D.
*(Superseded by §8.2 — the orchestrator folded it in; it shipped.)*

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

**None of the seven selectors could be reached mounted in the harness** *at contract time.* The
default mock state opens with no repo tab, so the command palette, combobox lists, diff overlay,
diff file tree, conflict editor and worktree-context dialog are all unmounted;
`document.querySelectorAll` returned 0 for all eight probes. The ratios in §2 were therefore
**computed from measured token values against the CSS-declared backdrop**, not read off a composited
pixel.

*This gap was closed at review time — §8.1 carries the mounted, composited measurements taken with
a repo tab open, plus an explicit not-reachable list.*

**Theme-toggle mechanism, stated because it matters for reproduction.** Both passes set the
`data-theme` attribute on `<html>` directly. That is the correct mechanism here and not a shortcut:
Bonsai resolves its theme from that attribute, and the browser-level `prefers-color-scheme`
emulation the harness also offers is **not** read by the app for theming, so emulating it would have
measured the dark theme twice.

This is safe here in a way it was not in P95, and the reason must be checked by the reviewer rather
than assumed: **every backdrop in §3 is a fully opaque token** (`--bg-0`, `--bg-1`, `--bg-2`,
`--selection`, `--accent`) declared directly on an ancestor with no intervening translucency. The
P95 failure mode — estimating against a transparent backdrop when the real one was opaque — is the
inverse case and does not arise. The one genuinely translucent value in scope,
`rgba(255, 255, 255, 0.8)`, is composited explicitly in §2.2.

**AC12 requires the composited backdrop to be confirmed on a mounted instance** of each selector,
which the harness *can* do once a repo fixture is open. Do not skip it. *(Done — §8.1.)*

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
**optional**, not an AC.) *(Taken — §8.3.)*

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

> **This paragraph was wrong, and the harness proved it.** It reasoned over the *option* rules only
> and missed that the **child hint** rule also matches inside a disabled option, where it now wins
> over the ancestor's inherited `--text-3`. The measured consequence and the required fix are in
> **§8.4 (MUST-FIX-1)**.

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

*Points 1–4 are now backed by mounted measurements of the real primary/secondary pairs — §8.1.3.*

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

**Survey seeds for that follow-up** — accent-filled surfaces already found, so P99 does not start
from zero: `.combobox-option--active` and `.command-palette-option.is-active` (both now
`var(--accent-text)`, §8.3); `.conflict-editor-mode-btn.is-active` (`conflicts.css:200-201`, which
also carries a **phantom token** — see §8.9 NIT-1); `.wt-copy-toggle-on`
(`dialogs-forms.css:163-164`, `background: var(--accent) !important; color: #fff !important` — a
hardcoded literal on an accent fill, pre-existing and outside AC3's scope, §8.9 NIT-2).

**If the orchestrator would rather fix it now**, the change is 4 declarations in 2 files and I would
fold it into P98 as an explicitly-labelled second class, with rows 6a/7a becoming `--text-2` at
5.01 / 6.42 instead of `--accent-text` at 3.22 / 4.65. Say the word. **My recommendation stands:
defer.**

### B. `--border-0` (§4) — confirm the exception

I have specced the fix. If the orchestrator prefers P98 to be literally colour-only, drop §4 and
file it as a follow-up; the seven colour changes are independent of it. **Recommendation: keep §4
in** — it is 5 lines, same file, same defect. *(Kept; verified §8.1.4.)*

### C. Harness reachability

None of the seven selectors mount in the default mock state (no repo open). AC12's composited-
backdrop verification therefore requires opening a repo fixture and reaching each surface.
If any surface has no reachable fixture at all, that selector's verification degrades to a computed-
style check on a mounted-in-isolation instance, and the *backdrop* half must be asserted from the
CSS source — say so in the implementation report rather than reporting it as measured. No new
fixtures are requested by this contract. *(Resolved: §8.1 reports 4 of 10 declarations measured
mounted and names those that are not reachable. No synthetic DOM was injected — "not reachable" is
reported as not reachable.)*

### D. `.pr-merge-method-desc` — the eighth read-text candidate, out of scope by decision

`ui-reference.md` §12.9 specifies the PR merge dialog's per-method description line in `--text-3`
on the `.dialog-card` `--bg-1` surface — **3.38:1** dark / **2.96:1** light, and it is read text by
the rule §6 makes explicit (it is the only sentence telling the user what the selected merge method
does). It is excluded from P98 because the seven-selector scope was fixed by the orchestrator and
because fixing it means editing `ui-reference.md` §12.9, which would push this pass's reference diff
outside §2. **Recommendation:** bundle it with the §5-A follow-up as a one-line `--text-2` swap in
`src/styles/forge-pr.css` plus the matching §12.9 sentence. Do **not** let senior-dev fix it inside
P98; it would break AC10.

> **OVERRIDDEN by the orchestrator, 2026-09-01, and I agree with the override.** It shipped as
> `--text-2` and §12.9 has been amended. See §8.2. The stated reason for the override — that it was
> the *last* gap in a "closed" claim — turned out to be false (§8.5), but the change itself is
> correct on its own merits and stands.

---

## 6. `ui-reference.md` edits — applied in this pass

Applied by `ui-designer` at contract time. **All five edits were confined to §2** (roughly lines
69–145). A second, review-time pass amended §2 again and §12.9 — see §8.6.

1. **The stale header claim** — previously "One known AA shortfall remains (`--text-3`)" — is
   replaced. It could not become "no shortfalls remain", because §5-A is a *measured, shipping*
   3.22:1. ~~The `--text-3` family is now stated as **closed** (P95 + P98)~~ — **this claim was
   false and has been retracted; see §8.5/§8.6** — and the "one known AA shortfall remains" sentence
   now names **the `--accent` fill** instead.
2. **§2 `--text-3` bullet** — added the measured `--selection` row (**2.33:1** dark / **2.55:1**
   light for `--text-3`; **5.01** / **6.42** for `--text-2`), and replaced the parenthetical listing
   the seven residual read-text selectors with an explicit "swept by P98; a new occurrence is a
   defect" statement in the new read-text bullet.
3. **New §2 read-text rule bullet** — parallel to P95's enabled-control sentence: read text is
   judged at **4.5:1 against its actual composited backdrop in both themes**, "small, uppercase and
   letter-spaced" does not by itself make text decorative (the test is whether the user must read it
   to act), the swept selectors are named, and each hover/selected/active state's backdrop is
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
| AC4 | For each declaration, `getComputedStyle` on a **mounted** instance returns a `color` equal to the computed `--text-2` (or `--accent-text`) value, in **both** `data-theme` states. | Harness — **partially met; §8.1** |
| AC5 | `--text-3` no longer appears as the `color` of any **read text** in `src/styles/`, with the single sanctioned exception of `.pr-merge-method-desc`, plus the §3.1 disabled set, the P95 §3.3 exempt set, and decorative uses. Each remaining occurrence is classifiable into one of those buckets. | Harness (grep) — **SUPERSEDED by AC5′, §8.7** |
| AC6 | The §3.1 disabled selectors — `.combobox-option--disabled`, `.combobox-option--disabled.combobox-option--active`, `.command-palette-option.is-disabled`, `.command-palette-option.is-disabled.is-active` — **still** declare `var(--text-3)`, unchanged. | Harness (grep) |
| AC7 | In the combobox and the command palette, a **disabled** option's text is visibly dimmer than an **enabled** option's hint (`--text-3` vs `--text-2`), and both keep `cursor: default` / the `disabled` attribute respectively. | Harness + UC — **FAILS as shipped; see §8.4** |
| AC8 | All five `var(--border-0)` occurrences in `src/styles/conflicts.css` are now `var(--border)`; `--border-0` appears nowhere in `src/`. A probe of `.conflict-editor-split-label + .conflict-editor-split-label` computes `border-left-width: 1px` and `border-left-style: solid` (was `0px` / `none`). | Harness |
| AC9 | No `--border-0` token is defined in `src/styles/tokens-and-base.css` or anywhere else. | Harness (grep) |
| AC10 | **senior-dev's implementation diff** touches **only** `src/styles/diff.css`, `diff-browser.css`, `conflicts.css`, `worktrees.css`, `dialogs-forms.css`, `search.css` — six CSS files, no `.tsx`, no `.ts`, no fixtures, and nothing under `docs/`. | Harness (git diff --stat) — **AMENDED to seven files by §8.2** |
| AC11 | Within those files, every changed line is a `color:` value, a `color:` value on an active-state override, or a `var(--border-0)` → `var(--border)` substitution (5 lines). **No** size, weight, style, spacing, padding, margin, radius, background, transition, hover or focus declaration changed. | Harness (diff read) |
| AC12 | For each declaration, the **actual composited backdrop** of a mounted instance is walked to the first opaque ancestor background and matches §3's declared backdrop; the recomputed ratio matches §3's "After" figure within ±0.05. Any selector whose surface has no reachable fixture is reported as such, not as measured. | Harness — **partially met; evidence and the not-reachable list in §8.1** |
| AC13 | Each swapped element still reads as clearly **subordinate** to its primary label in both themes. In particular the active palette/combobox row's hint does not read as a second title. | **UC** (perceptual) |
| AC14 | The merge editor's split-label bar now shows a bottom border and a divider between panes, matching every other 1px divider in the app; nothing else in the conflict editor shifted position. | **UC** |
| AC15 | `.diff-tree-count` remains legible in all three of its row states — idle, hover (`--bg-2`) and selected (`--selection`) — in both themes. | Harness (AC4 per state) + **UC** — **not harness-verifiable; §8.1.2** |
| AC16 | Long-content behaviour unchanged: a 60-char branch name in `.wtctx-branch`, a deep path in `.diff-overlay-kind`'s row, and a long shortcut hint in a narrow palette still truncate/clip exactly as they do today (no wrapping change — no layout property moved). | Harness |
| AC17 | `ui-reference.md` is consistent with the shipped code. | Harness (file read) — **restated as AC17′, §8.7** |
| AC18 | `pnpm gate` tiers green: no snapshot/visual test asserts a `--text-3` computed colour on any of the changed declarations. Any test that does is updated in the same increment. | Harness |
| **AC19** | **New, from §8.4.** A **disabled** combobox option and a **disabled** command-palette option each render **both** their label and their hint at the computed `--text-3` value (3.38:1 dark / 2.96:1 light on `--bg-1`), in both themes and in **both** the plain-disabled and the disabled+active states. | Harness |

Harness-visible: AC1–AC3, AC6, AC8–AC12, AC16–AC19. Partially harness-visible: AC4, AC12.
USER CHECKPOINT: AC7 (perceptual half), AC13, AC14, AC15.

### 7.1 Harness fixture states needed

**No new fixtures** were requested by the original contract. §8.1.2 records which of those existing
states actually exist and which do not — and §8.8 turns the gaps into a concrete, optional fixture
ask.

---

## 8. Amendments and design review — 2026-09-01

This section is written by `ui-designer` at review time, after driving the mock harness with a repo
tab open. It supersedes §0, §5-D, AC5, AC10, AC12 and AC17 where they conflict.

### 8.1 Mounted, composited measurement — closing the AC4 / AC12 gap

Method: `pnpm dev:mock` on port 1420, viewport 1440×900, repo fixture `bonsai-fixture` opened
through the empty-state `Open repository` button. For each selector a helper walked the element's
ancestor chain to the **first fully opaque** `background-color`, alpha-composited any translucent
layers above it, alpha-composited the element's own `color` onto that result, and computed the WCAG
ratio. Themes toggled by setting `data-theme` on `<html>` (see §1 for why that, and not
`prefers-color-scheme` emulation, is the correct mechanism). No DOM was injected; nothing was faked
as `:hover`.

#### 8.1.1 Measured

| # | Selector / state | Composited backdrop (measured) | Dark | Light | §3 predicted | Match |
|---|---|---|---|---|---|---|
| 1 | `.diff-overlay-kind` (in `ReflogView`, `.diff-overlay`) | `rgb(22,24,29)` = `--bg-0` ✓ | **7.89** | **7.98** | 7.89 / 7.98 | exact |
| 7 | `.command-palette-option-hint`, idle | `rgb(29,32,38)` = `--bg-1` ✓ | **7.25** | **7.45** | 7.25 / 7.45 | exact |
| 7a | `.command-palette-option.is-active .command-palette-option-hint` | `rgb(79,140,255)` / `rgb(47,111,228)` = `--accent` ✓ | **3.22** | **4.65** | 3.22 / 4.65 | exact |
| — | `.command-palette-option.is-active` (the row's own **label**, for the hierarchy check) | `--accent` | **3.22** | **4.65** | 3.22 / 4.65 | exact |
| 8 | `.pr-merge-method-desc` (§8.2's added selector) | CSSOM-confirmed `--text-2`; dialog not reachable, so the `--bg-1` backdrop half is source-derived | 7.25 | 7.45 | — | see §8.2 |

Every measured composited backdrop is the **opaque token §3 predicted from source**. The
§1 "safe here" argument is therefore confirmed empirically, not just asserted: no translucent
ancestor intervened anywhere in the four measured chains.

`.combobox-option-hint`'s **disabled** instance was also measured (7.25 / 7.45) — that is the
MUST-FIX in §8.4, not a pass.

#### 8.1.2 Not reachable mounted — reported as such, not as measured

| # | Selector | Why not reachable in the mock harness |
|---|---|---|
| 2 | `.diff-tree-count` (idle / hover / selected) | `DiffFileTree` mounts only inside `CommitPanel` / `ComparePanel` with a commit selected. Selection is driven by the `<canvas>` graph; synthetic clicks do not land on canvas hit-testing, and no palette command selects a commit. `.diff-tree-root` count was 0 throughout. **All three states unverified mounted**; `hover` additionally cannot be verified without a real pointer even if the tree mounted. This is why **AC15 is not harness-verifiable** and is a pure USER CHECKPOINT. |
| 3 | `.conflict-editor-split-label` | Requires a conflicted file opened in the merge editor. No mock state in the fixture reaches `ConflictEditor` (it is `React.lazy`-mounted from `DiffOverlay` on a conflicted scope). |
| 4 | `.wtctx-branch` | `WorktreeContextDialog` is reached from `AiAssetsPanel` / `WorktreeDialogs`; no route found from the sidebar or the palette in this fixture. |
| 5 | `.wtctx-blocked` | Same dialog; additionally needs a row with `blockedReason` set. |
| 6 | `.combobox-option-hint`, **enabled** idle state | The combobox *does* mount (New worktree → Branch field, 23 options), but in this fixture **exactly one option carries a hint** and it is the **disabled** one (`main` / `checked out`). The enabled and active-enabled hint states have no fixture data. |
| 6a | `.combobox-option--active .combobox-option-hint` | Same cause: no active option carries a hint. The rule is CSSOM-confirmed (`color: var(--accent-text)`); its *composited* figure is inferred from 7a, which shares the identical backdrop and value. |
| 8 | `.pr-merge-method-desc` | The PR merge dialog was not reachable; rule confirmed in CSSOM only. |

So: **4 of 10 declarations measured mounted with a walked composited backdrop; 6 not reachable.** For
all 6, the CSS rule text was confirmed present and correct in the live CSSOM (§8.1.4), which
verifies the *declaration* but not the *backdrop*. Their §3 ratios remain source-derived. I am not
reporting them as measured.

#### 8.1.3 Visual hierarchy — how I checked it, and the verdict

The contract's §3.3 claim was numeric. Here it is measured on the real mounted pairs:

| Surface | Primary (measured) | Secondary after P98 (measured) | Ratio-of-ratios | Size step |
|---|---|---|---|---|
| Diff overlay header | `.diff-overlay-title` `--text-1` **14.73** dark / **16.52** light | `.diff-overlay-kind` **7.89 / 7.98** | 1.87× / 2.07× | both 12px — but the kind is a *chip*, right of the title |
| Command palette, idle row | label `--text-1` 13px | hint **7.25 / 7.45**, 11px | ~1.87× / ~2.07× | 13 → 11px |
| Command palette, active row | label **3.22 / 4.65**, 13px | hint **3.22 / 4.65**, 11px | **1.00×** | 13 → 11px + `flex:none` right edge |

**Verdict: the hierarchy holds, with one honest caveat.** On every non-active surface the colour
step survives at ~1.9–2.1×, which is the same step the app already uses ~40 times for
`--text-1` primary + `--text-2` secondary — so these surfaces now match the house convention rather
than diverging from it. On the **active** palette/combobox row the colour step is now exactly zero
by design (§3.3 point 4): subordination there rests entirely on 11px-vs-13px, right-edge
`flex: none` placement, and tabular numerals. I consider that sufficient and consistent with the
selected graph/sidebar rows, but it is the one place where a perceptual check can legitimately
disagree, so **AC13 stays a USER CHECKPOINT** and I am not asserting it passed.

One thing I did **not** check and cannot: `.diff-overlay-kind` is the same 12px as its title, so its
subordination is carried by chip shape and position rather than size. That reads correctly in the
CSSOM geometry, but "reads as a chip, not a second title" is perceptual — folded into AC13.

#### 8.1.4 The §4 border repair — verified, and the appearance question

Live CSSOM, dark theme:

```
.conflict-editor-split-labels { display: flex; flex: 0 0 auto;
  border-bottom: 1px solid var(--border); background: var(--bg-1); }
.conflict-editor-split-label + .conflict-editor-split-label {
  border-left: 1px solid var(--border); }
```

All five `var(--border-0)` uses are gone; `border-0` survives in `src/` **only inside the P98
explanatory comment** in `conflicts.css` (grep, src-wide: 1 hit, line 129, a comment). No
`--border-0` token was defined (AC9 ✓). A live probe still confirms the token is undefined
(`1px solid var(--border-0)` → `0px / none`), while `1px solid var(--border)` resolves to
`rgb(44,49,58)` at `1px` — so the repair is real and the diagnosis was correct.

**Does it look intentional, and does it shift layout?**

- **Layout: a real but negligible 1px growth, stated rather than denied.** With
  `border-style: none` the used border width is `0px`, so restoring the style adds 1px of genuine
  box-model space: the split-label bar grows **1px** in height, and each label after the first gains
  **1px** on its left. Against a `padding: 3px 12px` label in a `flex: none` bar, 1px is below the
  4px spacing grain and cannot reflow the panes (the editor body takes the remainder).
  **Not a layout defect — but not zero either.**
- **Appearance: intentional.** `--border` at 1px is the app's single divider token and this is
  exactly the treatment every other pane-header/column divider uses. The bar goes from a floating
  pair of labels to a bounded two-column header, which is what the labels were always drawn to be.
- **Crowding: no.** The divider lands in the 24px gap between two 12px paddings, so it sits with
  ≥11px of air on each side.
- I could **not** mount the conflict editor (§8.1.2), so the above is CSSOM + box-model reasoning,
  not a screenshot. **AC14 stays a USER CHECKPOINT** — and the 1px growth is the specific thing to
  look at.

#### 8.1.5 Not a defect — one thing I checked and cleared

An initial grep appeared to show malformed comment openers (`\*` instead of `/*`) on six of the new
P98 comments, which would have silently dropped the following declaration. **It is a false alarm:**
fetching the raw served CSS shows correct `/*` on every one, and the live CSSOM shows every affected
rule present with its intended value. The `\*` was a rendering artifact of the grep tool's
context-line output. Recorded here so a future pass does not re-raise it.

### 8.2 Amendment — the eighth selector shipped (`.pr-merge-method-desc`)

The orchestrator overrode §5-D and folded `.pr-merge-method-desc` (`src/styles/forge-pr.css`) into
P98 as `--text-2`. **I endorse the change and reject the reason given for it** (that it was the last
gap in a "closed" claim — §8.5).

On the merits: it is the only sentence telling the user what the selected merge method will do to
their branch, immediately above an irreversible `Merge` button. That is read text by the §2 test
with no ambiguity. Was **3.38:1** dark / **2.96:1** light on the `.dialog-card` `--bg-1` surface;
now **7.25:1** / **7.45:1** — CSSOM-confirmed as `color: var(--text-2)` on the real rule; the
dialog itself was not reachable in the harness, so the backdrop half is source-derived.

Consequential amendments:

- **AC10 → seven CSS files.** `src/styles/forge-pr.css` joins `diff.css`, `diff-browser.css`,
  `conflicts.css`, `worktrees.css`, `dialogs-forms.css`, `search.css`. Still no `.tsx`, no `.ts`, no
  fixtures.
- **AC11** extends by one `color:` value.
- **AC5 → AC5′** (§8.7): the `.pr-merge-method-desc` carve-out is deleted, because it no longer
  exists.
- **`ui-reference.md` §12.9 amended** to state `--text-2`, the ratio, and P98 as the change (§8.6).
- §0's "seven selectors / nine declarations" reads **eight selectors / ten declarations**.

### 8.3 Amendment — the optional `#fff` freebie was taken

`.combobox-option--active` and `.command-palette-option.is-active` now declare
`color: var(--accent-text)` instead of the `#fff` literal. Same computed value in both themes
(`#ffffff`), CSSOM-confirmed, measured 3.22 / 4.65 — i.e. **no visual change at all**, purely token
hygiene, and it removes two hardcoded literals from `src/styles/`. Correct call; §3's "optional"
framing stands as satisfied.

`rgba(255,255,255,…)` is now **absent from `src/styles/` entirely** (grep: 0 hits) — AC3 ✓.

### 8.4 MUST-FIX-1 — the disabled option's hint is now brighter than its own label

**This is the one defect in the increment, it is a real regression, and it was measured.**

`.combobox-option-hint` and `.command-palette-option-hint` are *child* rules. They match inside a
**disabled** option too, where the element's own `color` declaration beats the `--text-3` it would
otherwise inherit from `.combobox-option--disabled` / `.command-palette-option.is-disabled`. §3.1
reasoned only about the option rules and missed this.

Measured, mounted, on the real disabled combobox option (`main` / `checked out`), backdrop
composited to `--bg-1`:

| Element | Dark | Light |
|---|---|---|
| `.combobox-option--disabled .combobox-option-label` ("main") | `--text-3`, **3.38:1** | **2.96:1** |
| `.combobox-option--disabled .combobox-option-hint` ("checked out") | `--text-2`, **7.25:1** | **7.45:1** |

Three things are wrong with that:

1. **Inverted hierarchy inside the row.** The qualifier is now ~2× brighter than the thing it
   qualifies. Before P98 both were `--text-3` and the row was internally consistent.
2. **The disabled affordance is degraded.** Half the row now renders at enabled brightness, so a
   disabled option reads as partly available. Dimming is the disabled signal (§3.1) and it is now
   only half-applied.
3. **AC7 fails as written.** AC7 requires a disabled option's text to be visibly dimmer than an
   enabled option's hint. The disabled option's hint is now **identical** to the enabled hint —
   7.25 / 7.45 in both cases. This is not a perceptual judgement; it is the same computed colour.

#### The fix — hand this to `senior-dev`; it is the whole of the rework

Add **two** rules. `color`-only, no new token, both files already in the AC10 set.

- `src/styles/dialogs-forms.css`:
  `.combobox-option--disabled .combobox-option-hint { color: var(--text-3); }`
- `src/styles/search.css`:
  `.command-palette-option.is-disabled .command-palette-option-hint { color: var(--text-3); }`

**Placement is load-bearing — read this before inserting.** My earlier draft claimed these "win on
specificity regardless of source order". **That is wrong for the state that matters**, and the
correction is the point of this paragraph:

- Against the plain hint rule (`.combobox-option-hint`, specificity **0,1,0**) the new rule
  (**0,2,0**) does win regardless of order. ✓
- Against the **active** hint override (`.combobox-option--active .combobox-option-hint`, also
  **0,2,0**) it is an exact **tie**, so **source order decides**. The same tie exists in
  `search.css` between `.is-disabled .command-palette-option-hint` and
  `.is-active .command-palette-option-hint`.
- The **disabled + active** state is real and reachable — `.command-palette-option.is-disabled.is-active`
  and `.combobox-option--disabled.combobox-option--active` both already exist as rules. A user
  arrow-keying onto a disabled row lands exactly there.

Therefore, choose **one** of these two, not "somewhere after the hint rules":

1. **Ordering (preferred, smaller):** place each new rule **after** the corresponding
   `--active` / `.is-active` hint override, i.e. last among the hint rules in that file. Then the
   disabled dim wins the tie in every state. Add a comment saying the position is required so a
   future reorder or a lint autofix does not silently break it.
2. **Explicit triple compound (belt-and-braces, order-independent):** additionally declare
   `.combobox-option--disabled.combobox-option--active .combobox-option-hint` and
   `.command-palette-option.is-disabled.is-active .command-palette-option-hint` at `var(--text-3)`
   (**0,3,0**), which beats the active override outright. Use this if the file's rule order is
   volatile.

**My recommendation: option 1**, matching how `.combobox-option--disabled.combobox-option--active`
already sits after `.combobox-option--active` in the same file — it is the file's existing idiom, and
consistency beats defensiveness here.

Comment both as: "P98 §8.4: the disabled exemption applies to the hint too — the child rule would
otherwise re-brighten half a disabled row. Must stay after the --active hint override (equal
specificity)."

This is **within** the P98 disabled exemption, not an expansion of scope: §3.1 already declared the
intent, and this makes the CSS match the declared intent.

**Verification (this is AC19).** Re-measure the disabled combobox option and the disabled palette
option, in both themes, in **both** the plain-disabled and the disabled+active states: label **and**
hint must both compute the `--text-3` value (3.38:1 dark / 2.96:1 light on `--bg-1`). Reaching the
disabled+active state in the harness: the palette's disabled rows are the ones to arrow onto; if the
fixture has no disabled palette row (it had none at review time — §8.1.2), verify that state by
adding the class in the CSSOM-inspection sense only and report it as rule-level, not composited.

**AC6 is unaffected** — the four option-level rules still declare `var(--text-3)` unchanged
(CSSOM-confirmed).

### 8.5 The "`--text-3` family is closed" claim was false — retracted

§6 item 1 and the `ui-reference.md` §2 header both asserted the `--text-3` family **closed** after
P95 + P98. **That claim is withdrawn.** It was not evidenced, and it is not true.

Pinned by grep, 2026-09-01: **`color: var(--text-3)` appears 122 times across 33 files in
`src/styles/`.** None of those 122 has ever been individually classified. P95's AC10 grep hit ~140
and that AC was reworded precisely because enumerating them was unsatisfiable; P98 then swept an
enumerated 8-selector set and inherited the unenumerated remainder without auditing it. §0's
"~140 permitted decorative uses" was the same overclaim in the other direction — they are
**unaudited**, not permitted.

Three of the 122 are unambiguous read-text violations by §2's own test, and I have now **measured**
one of them mounted:

| Selector | File:line | Composited backdrop | Dark | Light | Basis |
|---|---|---|---|---|---|
| `.reflog-date` | `src/styles/blame-history.css:241` | `rgb(22,24,29)` / `rgb(255,255,255)` = `--bg-0` (host is `.diff-overlay.reflog-view`) | **3.67:1** ✗ | **3.17:1** ✗ | **measured mounted** (Reflog opens from the workspace toolbar) |
| `.blame-date` | `src/styles/blame-history.css:93` | `--bg-0` (same `.diff-overlay` host, `BlameView`) | 3.67 ✗ | 3.17 ✗ | source-confirmed; not mounted |
| `.file-history-date` | `src/styles/blame-history.css:167` | `--bg-0` (same host, `FileHistoryView`) | 3.67 ✗ | 3.17 ✗ | source-confirmed; not mounted |

A timestamp is the canonical example `ui-reference.md` §2 already gives of read text ("metadata,
timestamps"). These are dates the user reads *in order to pick which reflog entry to reset to* —
read text under any reading of the test. The reference text and the shipped CSS have contradicted
each other here the whole time, in a file neither P95 nor P98 opened.

Two conclusions:

1. **A "closed" claim on an unenumerated set is worse than no claim**, because it tells the next
   sweep there is nothing to look for. §2 now states only what is evidenced (§8.6).
2. **The pattern predicts more.** Three files were inspected casually and yielded five findings
   (`.pr-merge-method-desc`, `.cm-gutters`, and the three dates). The remaining ~119 need a method,
   not another spot-check — §8.8.

### 8.6 `ui-reference.md` — review-time amendments (second pass)

Two sections edited; nothing else in the file touched.

1. **§2, header paragraph + the read-text bullet.** The word "closed" is gone. §2 now says: P95
   swept the enabled-interactive-control class; P98 swept an **enumerated** read-text set of
   **eight** selectors (named); **122 further `color: var(--text-3)` declarations across 33 files in
   `src/styles/` are unaudited and unclassified**, pending **P101**; known violations already found
   inside that remainder are named (`.blame-date`, `.file-history-date`, `.reflog-date`, with the
   measured 3.67 / 3.17). The read-text bullet also now carries the **sanctioned-decorative
   list** (§8.7's `.cm-gutters` decision) so that call does not get re-litigated, and the §8.4
   **child-rule trap** as a general rule.
2. **§12.9.** `.pr-merge-method-desc` now reads `--text-2` (**7.25:1** dark / **7.45:1** light on the
   `.dialog-card` `--bg-1` surface) instead of `--text-3`, annotated "amended by P98". The
   surrounding merge-dialog structure sentence is otherwise unchanged.

The §2 undefined-token bullet also gained the box-model note from §8.1.4 (a repaired
`border-style: none` adds 1px of real space).

### 8.7 Decision — `.cm-gutters` is sanctioned decorative

`src/components/conflictCmSetup.ts:36` hardcodes `color: 'var(--text-3)'` for `.cm-gutters`, the
conflict editor's line-number gutter, on `--bg-0` → **3.68:1** dark / **3.17:1** light. It is the
only `--text-3` outside `src/styles/` that is a real colour declaration (`src/graph/colors.ts:131`
reads the token for canvas use; `SettingsEmpty.tsx:21` is a comment).

**Ruling: decorative. Leave it at `--text-3`. No AC, no change for `senior-dev`.** Reasoning:

- **Line numbers are editor chrome by universal convention.** Every editor the user already knows —
  VS Code, JetBrains, Vim, GitHub's blob view — renders the gutter markedly dimmer than the code. A
  `--text-2` gutter would be *louder* than any of them and would read as a defect, not as a fix.
- **The gutter is not what carries the act in this pane.** In the conflict editor the user acts on
  regions, not on absolute line numbers: `.conflict-region-caption`, the OURS/THEIRS split labels,
  and the accept/reject buttons are the decision surface, and all three are already `--text-2` or
  brighter (CSSOM-confirmed: `.conflict-region-caption` and `.conflict-editor-split-label` both
  `--text-2`). A user reconciling by line reads the *code*, which is `--text-1`.
- **It is a coordinate, not information.** The number restates a position the user can see; it
  duplicates visible structure, which is exactly §2's decorative test.

I am recording the counter-argument rather than burying it: in a *conflict* editor specifically, a
user may well be talking about line numbers ("keep 41–58"), and 3.17:1 in light mode is a poor read
for that. So this ruling is **conditional**, and the condition is written into §2:

> **Revisit trigger.** If any future feature makes the line number itself actionable or referenced in
> copy — a "go to line" control, a line-range selection UI, or any message that names a line number —
> `.cm-gutters` becomes read text and must move to `--text-2`. Re-evaluate in P101.

**Sanctioned-decorative list (as recorded in `ui-reference.md` §2).** These `--text-3` uses are
audited and deliberately kept:

| Use | Where | Measured | Why decorative |
|---|---|---|---|
| Editor line-number gutter | `conflictCmSetup.ts:36` `.cm-gutters` | 3.68 / 3.17 on `--bg-0` | coordinate, duplicates visible structure, universal editor convention |
| The §3.1 disabled set | `dialogs-forms.css`, `search.css` (4 rules) | 3.38 / 2.96 on `--bg-1` | dimming *is* the disabled signal; carried also by `disabled` / `cursor: default` |
| The P95 §3.3 exempt set | see P95 | — | as recorded there |

Everything else among the 122 is **unclassified**, not sanctioned.

**Restated ACs:**

- **AC5′** — `--text-3` is not the `color` of any read text among the **eight selectors P98
  enumerated** plus the sanctioned-decorative list above. The remaining **122** `color:
  var(--text-3)` declarations in `src/styles/` are explicitly **out of P98's scope and unaudited**;
  P98 makes no claim about them, and `ui-reference.md` §2 says so. (The old AC5's app-wide claim was
  unsatisfiable and is deleted, not weakened.)
- **AC17′** — `ui-reference.md` is consistent with the shipped code: §2 makes **no** "closed" claim,
  names the eight swept selectors, states the 122-declaration unaudited remainder and the three
  known `blame-history.css` violations with their ratios, carries the sanctioned-decorative list
  including `.cm-gutters`, and records the §8.4 child-rule trap; §12.9 says `--text-2` for
  `.pr-merge-method-desc`. The `--accent`-fill KNOWN SHORTFALL bullet and the undefined-token bullet
  are unchanged from the first pass apart from the added box-model note.

### 8.8 Hand-off — P101, the full `--text-3` audit

Scope: the **122** `color: var(--text-3)` declarations across **33** files in `src/styles/`
(count pinned by grep 2026-09-01; the per-file distribution is reproducible with
`rg -c 'color: var\(--text-3\)' src/styles`). Highest counts: `forge-pr.css` 15,
`settings-legacy-sections.css` 11, `commit-panel.css` 9, `search.css` 8, `sidebar.css` 7,
`blame-history.css` 6, `dialogs-forms.css` 6, `repo-health.css` 5.

**Seed evidence (already found, do not re-derive):**

| Selector | File:line | Verdict | Measured |
|---|---|---|---|
| `.reflog-date` | `blame-history.css:241` | **read text — MUST fix** | 3.67 dark / 3.17 light on `--bg-0`, **measured mounted** |
| `.blame-date` | `blame-history.css:93` | **read text — MUST fix** | 3.67 / 3.17, source-derived |
| `.file-history-date` | `blame-history.css:167` | **read text — MUST fix** | 3.67 / 3.17, source-derived |
| `.reflog-oid-old`, `.reflog-oid-root` | `blame-history.css:214/218` | **read text — fix** (my call, below) | **3.67 / 3.17 measured mounted** |
| `.reflog-oid-arrow` | `blame-history.css:~216` | decorative glyph, but **fix for cohesion** | 3.67 / 3.17 measured; clears the 3:1 graphics bar in both themes |

**My call on the three borderline reflog oids, since it was asked for.** `.reflog-oid-old` and
`.reflog-oid-root` are the abbreviated SHAs in the `abc1234 → def5678` pair on each reflog row.
That pair *is* what the user reads to decide which entry to reset to — the whole point of the reflog
view — so they are read text and go to `--text-2`. `.reflog-oid-arrow` is a `→` separator and is
genuinely decorative; at 3.67 / 3.17 it already clears the 3:1 graphics bar, so contrast does not
force it. **Move it anyway**, because a single visual string rendered half at `--text-2` and half at
`--text-3` reads as a rendering bug. That is a *cohesion* justification, explicitly not a contrast
one — state it that way in the P101 contract so the precedent is not misread as "decorative glyphs
need 4.5:1".

**The classification method — this is the durable deliverable.** For each of the 122, in this order:

1. **Find the composited backdrop, per state.** Walk to the first opaque ancestor `background-color`
   and composite any translucent layers above it. Do this for **every** state the element has —
   idle, `:hover`, selected/`--active`, and disabled — because P98 found `--text-3` on a
   `--selection` fill at **2.33:1**, worse than any idle case. A single idle measurement is not an
   audit.
2. **Apply the one test: must the user read this to act?** Not "does it look dim". Not "is it small,
   uppercase and letter-spaced" — P98's `.conflict-editor-split-label` had all three and was read
   text. Ask what breaks if the string is unreadable. If the answer is "the user picks wrong",
   it is read text.
3. **Bucket it into exactly one of five**, and record which:
   - **`disabled`** — the element is inside a disabled control/row. Exempt; dimming is the signal.
     **Check the child-rule trap (§8.4):** if a *child* of the disabled row has its own `color` rule,
     that child is not dimmed, and the exemption is silently half-applied. Check the specificity
     **tie** against any `--active` override on the same child, too.
   - **`label-duplicating-structure`** — an uppercase section/group heading whose text restates a
     visible grouping the user can already see (settings group titles, sidebar section headers).
     Exempt. If it is the user's *only* wayfinder — a result-group header, a split-pane label — it is
     **not** in this bucket.
   - **`placeholder-empty`** — empty-state and placeholder copy where the surface itself is the
     message. Exempt only if the fix is stated elsewhere; an empty state that names the fix
     (§8 of the reference) puts that sentence at `--text-2`.
   - **`decorative-glyph`** — a separator, caret, dot or coordinate carrying no unique information.
     Judged at the **3:1** graphics bar, both themes. `--text-3` fails 3:1 on `--bg-2` (2.98 / 2.73)
     and on `--selection` (2.33 / 2.55), so a glyph on either of those backdrops is a defect even in
     this bucket.
   - **`read-text-violation`** — everything else. Fix to `--text-2`.
4. **Record the verdict per declaration in the P101 contract**, with the measured figure and the
   bucket. A declaration with no recorded bucket is an unfinished audit item, not a pass — this is
   what P95's and P98's unsatisfiable app-wide grep ACs got wrong twice.
5. **Two cross-cutting checks P98 learned the hard way:**
   - After any swap, re-measure the **disabled** sibling of the swapped element (§8.4).
   - Confirm the swapped element is still subordinate to its own primary by measuring **both** and
     reporting the ratio-of-ratios (§8.1.3), not by asserting it.
6. **Then, and only then, §2 may state the family closed** — with the enumeration behind it.

**Fixture gaps P101 will hit, and the optional ask.** P98 could not mount 6 of its 10 declarations
(§8.1.2). If the orchestrator wants P101's audit to be *measurable* rather than source-derived,
three small mock-fixture additions would close most of it, and they are the only new fixtures I would
ask for in the whole `--text-3` programme:

| Fixture | Unlocks |
|---|---|
| A mock state that opens `DiffOverlay` on a **conflicted** scope | `.conflict-editor-split-label`, `.cm-gutters`, the §4 border repair, `.conflict-region-caption` — and makes AC14 an AI gate instead of a UC |
| A **hint on at least one enabled and one active** combobox option, and **one disabled palette option** (today only the disabled *combobox* option has a hint) | `.combobox-option-hint` idle + active, and AC19's disabled+active state |
| A route to `WorktreeContextDialog` with **one blocked row** | `.wtctx-branch`, `.wtctx-blocked` |

`.diff-tree-count` stays unreachable regardless, because selection is canvas-driven — that one is
structurally a USER CHECKPOINT (AC15), and I would stop trying to automate it.

### 8.9 Verdict

**Approve with one MUST-FIX.** The ten shipped declarations are correct, token-only, and every
composited figure I could measure matched the prediction exactly. The two additions the orchestrator
made on top are both right. The single defect is §8.4 (two missing disabled-child overrides), it is
two lines in two files already in scope, and it is a straight consequence of a wrong sentence in my
own §3.1 — not of the implementation.

- **MUST-FIX-1** — §8.4, the disabled option hint. Two `color` declarations, **placement after the
  `--active` hint override is required** (equal specificity — read §8.4's placement paragraph).
  Verified by the new **AC19**.
- **SHOULD-FIX** — none in the code.
- **NIT-1 (pre-existing, out of P98 scope; file to P99).** `conflicts.css:201` —
  `color: var(--accent-fg, #fff)` on `.conflict-editor-mode-btn.is-active`. **`--accent-fg` is not a
  token** (it is absent from `ui-reference.md` §2's table), so this rule survives only on its
  hardcoded fallback. It is a *second* phantom token in the very file §4 just repaired, and exactly
  the class §2's new undefined-token bullet warns about. Prescribed: `var(--accent-text)`, same
  computed value, one line. Not folded into P98 because it sits on an accent-filled active control
  and therefore belongs with the §5-A survey.
- **NIT-2 (pre-existing, out of AC3's scope; file to P99).** `dialogs-forms.css:163-164` —
  `.wt-copy-toggle-on { background: var(--accent) !important; color: #fff !important }`. A hardcoded
  literal on an accent fill; add it to the §5-A accent-fill survey list rather than fixing it in
  isolation.
- **Documentation MUST-FIX, done in this pass** — the retracted "closed" claim (§8.5/§8.6) and the
  §12.9 amendment.
- **Still USER CHECKPOINT** — AC13 (subordinate reading, especially the zero-colour-step active
  row), AC14 (the split-label borders, incl. the 1px growth in §8.1.4), AC7's perceptual half, and
  **AC15 in full** (`.diff-tree-count` never mounts in the harness).
