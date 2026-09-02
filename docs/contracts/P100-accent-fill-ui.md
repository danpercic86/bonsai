# P100 — accent-fill row contrast (UI contract)

**Owner:** ui-designer · **Date:** 2026-09-01 · **Class:** hue-as-fill (distinct from P101's
`--text-3` read-text class) · **Predecessor:** `docs/contracts/P98-text3-readtext-ui.md` §5-A,
§8.9 NIT-1, §8.9 NIT-2 · **Successor:** P101 (`--text-3` full audit) — independent, no ordering
dependency on this contract other than both touching `ui-reference.md` §2.

> P98 §5-A named this follow-up "P99". That number shipped as an unrelated fix (`fea4a71`).
> **This milestone is P100** and supersedes the P99 naming everywhere.

---

## 0. The defect, and one correction to my own premise

`--accent-text` / `#ffffff` on `background: var(--accent)` is **3.22:1** dark / **4.65:1** light
(measured P98 §2.2). Dark is the default theme, so **every accent-filled surface in the app carries
its own primary label below the 4.5:1 read-text bar in the default theme** — including
`.btn-primary`, the app's primary action button.

**Correction, and it changes the shape of the fix.** P98 §5-A and `ui-reference.md` §2's KNOWN
SHORTFALL bullet both assert that *"white is the ceiling, so no foreground fixes this; only the fill
can."* That is **false**, and this contract retracts it. White is the ceiling only among *lighter*
foregrounds. Going **darker** than the fill passes comfortably in both themes:

| Foreground on `background: var(--accent)` | Dark | Light | Source |
|---|---|---|---|
| `--accent-text` / `#ffffff` (today) | **3.22:1** ✗ | **4.65:1** ✓ | P98 §2.2, measured |
| `--bg-0` (`#16181d` dark / `#ffffff` light) | **5.52:1** ✓ | **4.65:1** ✓ | `ui-reference.md` §5 lane 0, by symmetry |

The dark figure is not a new measurement: contrast is symmetric, and `ui-reference.md` §5's lane
table already carries `#4f8cff` vs `#16181d` = **5.52:1** and `#2f6fe4` vs `#ffffff` = **4.65:1**.
§2's P69 block records the same pair, rounded, as "`--accent` fill **5.6:1** dark / **4.7:1**
light". No new colour pair is introduced anywhere in this contract; every figure below is an
established measured pair re-cited (P98 §1's method distinction: **derived from established**, not
newly measured).

**House precedent already ships dark ink on a blue fill, twice:** `.diff-stage-float button`
(`src/styles/partial-staging.css:85-86`, `color: var(--bg-0)` on `background: var(--accent)`) and
§6's luminance-adaptive current-branch pill, which picks near-black `#16181d` over white on the
bright dark-theme lanes — lane 0 *is* `--accent`.

So the answer is **two recipes, not one**, because the surfaces are two kinds:

- **Recipe A — selection/segment states.** The fill is what should change. These rows are *states*,
  not actions; the house selected-row language (`--selection`) is what they should have used.
- **Recipe B — `.btn-primary`.** The fill must **not** change: it is the app's single loudest
  affordance and `--selection` would demote every primary action in the product. The ink flips
  instead.

---

## 1. Survey — every `background: var(--accent)` in the app

Method: `Grep` for `background(-color): var(--accent)` and `color-mix(… var(--accent) …)` across
`src/styles/**` plus a `*.tsx` sweep for inline/computed forms (`ackground.*--accent`,
`backgroundColor.*accent`) — **zero matches in TSX**; there are no inline or computed accent fills
in the app. `src/graph/colors.ts:134` reads `--accent-text` into `GraphColors.accentText`, but no
draw path consumes it (dead field; §6-E).

### 1.1 In scope — solid accent fill with text on it

| # | Surface | File:line | Today | Dark | Light | Prescribed fix |
|---|---|---|---|---|---|---|
| 1 | `.combobox-option--active` (label) | `dialogs-forms.css:212-215` | `--accent-text` on `--accent` | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe A** → `--selection` + `--text-1` = **9.36:1** / **13.29:1** ✓ |
| 2 | `.combobox-option--active .combobox-option-hint` | `dialogs-forms.css:241-245` | `--accent-text` on `--accent` | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe A** → `--text-2` on `--selection` = **5.01:1** / **6.42:1** ✓ |
| 3 | `.command-palette-option.is-active` (title) | `search.css:206-209` | `--accent-text` on `--accent` | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe A** → `--selection` + `--text-1` = **9.36:1** / **13.29:1** ✓ |
| 4 | `.command-palette-option.is-active .command-palette-option-hint` | `search.css:233-237` | `--accent-text` on `--accent` | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe A** → `--text-2` on `--selection` = **5.01:1** / **6.42:1** ✓ |
| 5 | `.conflict-editor-mode-btn.is-active` | `conflicts.css:199-202` | `var(--accent-fg, #fff)` on `--accent` — **phantom token** (NIT-1) | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe A-seg** (§12.3.2 house segmented) → `--selection` + `--text-1` + 600 + `border-color: --accent` = **9.36:1** / **13.29:1** ✓. Retires the phantom. |
| 6 | `.wt-copy-toggle-on` | `dialogs-forms.css:162-165` | `#fff !important` on `--accent !important` — hardcoded literal (NIT-2) | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe A-seg**, and both `!important`s dropped via specificity (§3.2) |
| 7 | `.btn-primary` | `controls.css:82-88` | `--accent-text` on `--accent` | **3.22:1** ✗ | **4.65:1** ✓ | **Recipe B** → redefine `--accent-text` **dark** to `#16181d` = **5.52:1** / **4.65:1** ✓. Light renders byte-identically. |

### 1.2 In scope, already compliant — no change

| Surface | File:line | Ink | Dark | Light |
|---|---|---|---|---|
| `.diff-stage-float button` | `partial-staging.css:85-86` | `--bg-0` on `--accent` | **5.52:1** ✓ | **4.65:1** ✓ |

This is the precedent Recipe B generalises. Do not touch it.

### 1.3 Surveyed, decorative — graphics bar (≥3:1) only, no text, **no change**

Completeness block, so "every accent-filled surface" is provable rather than asserted. Each is a
bar, dot, divider, tick or track with no text on it; all are `--accent` on `--bg-0`/`--bg-1`/`--bg-2`
at **5.52:1** dark / **4.65:1** light against `--bg-0` (§5 lane 0), clearing 3:1 in both themes.
**Eleven surfaces:**

`graph-banners.css:32` `.header-progress::after` · `ai-dock.css:87` `.ai-dock-progress::after` ·
`git-dock.css:108` `.git-dock-progress::after` · `search.css:429` `.history-score-fill` ·
`search.css:454` `.index-progress-fill` · `dialogs-forms.css:403` and `:408`
`.clone-progress-bar::-webkit-progress-value` / `::-moz-progress-bar` · `image-diff.css:127`
`.img-swipe-divider` · `conflicts.css:310` `.conflict-overview-tick` (hover `:316` is a 70% mix) ·
`settings-primitives.css:270` `.settings-switch-input:checked ~ .settings-switch-track` (knob
*position* is the carrier, §12.3.1) · `onboarding.css:35` `.onboarding-dot.is-current`
(**note the form**: `var(--accent, var(--text-1))`, so it matches only the widened grep of AC1).

### 1.4 Surveyed, classified out — tints, not fills

`color-mix(in srgb, var(--accent) N%, …)` surfaces are **tints**, governed by §2's already-measured
tint numbers (`--text-1` over a hue's own 14% tint = 9.24–10.30:1 dark / 11.68–12.00:1 light) and by
the P74 toast recipe. None carries hue-as-text. No change, listed for completeness:
`forge-pr.css:34,117` · `dialogs.css:286,294` · `search.css:104,216` · `conflicts.css:93,116,258,288`
· `ai-assets.css:124,150` · `ai-dock.css:105,157` · `git-dock.css:126,248`.

Note `search.css:215-217` (`.command-palette-option.is-disabled.is-active`) is a 22% accent tint
with `--text-3` ink — that is the **sanctioned disabled exemption** recorded in `ui-reference.md`
§2's decorative table, not a P100 target. It does, however, need one declaration added — §2.4.

### 1.5 Out of scope — sibling finding, do **not** fix here

The identical hue-as-fill defect exists for `--danger` fills and is **not** P100's:

| Surface | File:line | Today | Note |
|---|---|---|---|
| `.btn-danger` | `controls.css:70-71` | `color: #ffffff` on `background: var(--danger)` | `ui-reference.md` §6 already records white on `--danger` as **3.70:1** in dark — a hardcoded literal *and* an AA failure |
| `.update-btn-danger`(-family) | `updates.css:114-116` | `color: #ffffff` on `var(--danger)` | same pair, same literal |

`partial-staging.css:104-105` already uses `--bg-0` on `--danger` (§5 lane 4 red vs `--bg-0` dark =
**4.80:1** ✓) — the same ink-flip remedy, already shipped once. **Recommendation:** file as
**P102 — danger-fill contrast**, one increment, same shape as this one; the light-theme
`#d13438`-vs-`#ffffff` pair is the only figure that still needs measuring. Expanding P100 to cover
it would repeat exactly the scope creep P98 §5-A refused.

Also spotted and **not** P100's: `onboarding.css:26` `var(--bg-3, var(--border))` — `--bg-3` **is** a
real token (§2), so the fallback is inert, but the pattern is the phantom-token smell §2 warns
about. `onboarding.css:35` `var(--accent, var(--text-1))` likewise. Both are cosmetic; fold into
P101's token-hygiene sweep if it wants them.

---

## 2. Recipe A — the selection-row recipe (surfaces 1–4)

### 2.1 Visual decision, including the leading bar

**Decision: keep the leading bar.** `box-shadow: inset 2px 0 0 var(--accent)` ships on the active
row. Reasons:

1. It is the reason the change is affordable. `--selection` vs the popover's `--bg-1` is only
   **~1.3:1** (§12.1's own note), so without the bar the active row is a barely-visible wash and
   arrow-key navigation loses its anchor.
2. It is not a new idiom: two shipped precedents — `.diff-tree-selected` and the Settings rail
   (§12.1, `inset 2px 0 0 var(--accent)` on a `--selection` fill).
3. **It is the non-colour carrier**, which the change otherwise loses. A `--selection` fill is
   colour; a 2px bar at a fixed edge is *shape and position*, which §2 explicitly sanctions as
   decorative delineation for `--accent` on `--selection` (2.6:1 dark / 3.6:1 light — below 3:1, so
   the bar may never be the *only* signal; it is paired with the fill, and with
   `aria-selected`/`aria-activedescendant` programmatically).

**Deliberately not added: `font-weight: 600` on the active row.** It would be a second non-colour
carrier, but both lists are pointer-and-arrow-synced with the active row changing on every mouse
move and every ↓ — a weight change reflows the label and jitters the right-aligned hint under the
cursor. The bar carries the shape signal without touching metrics. (Contrast surfaces 5–6, which are
static segmented controls where 600 is the house recipe and costs nothing.)

### 2.2 Geometry — unchanged

No box-model change on any of surfaces 1–6. `box-shadow` does not participate in layout, so the
active row's height, padding (`5px 8px` combobox / `6px 8px` palette), radius (4px / 5px), gap and
font sizes (13px label / 11px hint) are byte-identical to today. **Both densities:** these are
popover/overlay surfaces and are density-invariant per §3 — `cozy` and `compact` render identically,
and this contract adds no density block.

### 2.3 Exact rule sets — `src/styles/dialogs-forms.css`

Replace the two rules at `:212-215` and `:241-245`. **Source order below is normative** —
see §2.5.

```css
.combobox-option--active {
  /* P100: the --accent fill put a 3.22:1 dark label on this row (P98 §2.2). House
     selected-row recipe instead; the inset bar is the shape carrier (ui-reference §2). */
  background: var(--selection);
  color: var(--text-1);
  box-shadow: inset 2px 0 0 var(--accent);
}

.combobox-option--disabled {
  color: var(--text-3);
  cursor: default;
}

.combobox-option--disabled.combobox-option--active {
  background: transparent;
  color: var(--text-3);
  box-shadow: none;          /* P100: a neutralised row must not keep the accent bar */
}
```

…and the hint block, whose **three rules keep their current order**:

```css
.combobox-option-hint {              /* (0,1,0) */
  flex: none;
  font-size: 11px;
  /* P98 §3: disambiguating text read in order to choose — read text, needs AA. */
  color: var(--text-2);
}

.combobox-option--active .combobox-option-hint {   /* (0,2,0) */
  /* P100: was var(--accent-text) at the 3.22:1 dark ceiling of the old --accent fill.
     On --selection, --text-2 measures 5.01:1 dark / 6.42:1 light and the
     label/hint colour step is restored (P98 §3.3 flattened it to 1.00x). */
  color: var(--text-2);
}

/* P98 AC19 (§8.4): the disabled exemption applies to the hint too — the child rule would
   otherwise re-brighten half a disabled row. MUST STAY LAST — equal specificity (0,2,0)
   to the --active override above, so source order alone resolves a disabled+active
   option. Do not reorder, do not merge, do not "tidy" this block. */
.combobox-option--disabled .combobox-option-hint {   /* (0,2,0) */
  color: var(--text-3);
}
```

### 2.4 Exact rule sets — `src/styles/search.css`

```css
.command-palette-option.is-active {
  /* P100: see dialogs-forms.css — same recipe, same reason. */
  background: var(--selection);
  color: var(--text-1);
  box-shadow: inset 2px 0 0 var(--accent);
}
.command-palette-option.is-disabled {
  color: var(--text-3);
  cursor: default;
}
.command-palette-option.is-disabled.is-active {
  background: color-mix(in srgb, var(--accent) 22%, transparent);
  color: var(--text-3);
  box-shadow: none;          /* P100 */
}
```

```css
.command-palette-option-hint {                                    /* (0,1,0) */
  flex: none;
  font-size: 11px;
  /* P98 §3: the bound shortcut is read in order to choose — read text, needs AA. */
  color: var(--text-2);
  font-variant-numeric: tabular-nums;
}
.command-palette-option.is-active .command-palette-option-hint {   /* (0,3,0) */
  /* P100: was var(--accent-text); --text-2 on --selection = 5.01:1 / 6.42:1. */
  color: var(--text-2);
}

/* P98 AC19 (§8.4). MUST STAY LAST — (0,3,0), equal to the .is-active override above;
   source order alone resolves a disabled+active option. */
.command-palette-option.is-disabled .command-palette-option-hint { /* (0,3,0) */
  color: var(--text-3);
}
```

### 2.5 The AC19 ordering constraint, restated because it has regressed once

Specificities **measured, not assumed** (counted from the shipped selectors):

| File | Active-hint override | Disabled-hint override | Tie? |
|---|---|---|---|
| `dialogs-forms.css` | `.combobox-option--active .combobox-option-hint` — **(0,2,0)** | `.combobox-option--disabled .combobox-option-hint` — **(0,2,0)** | **yes** |
| `search.css` | `.command-palette-option.is-active .command-palette-option-hint` — **(0,3,0)** | `.command-palette-option.is-disabled .command-palette-option-hint` — **(0,3,0)** | **yes** |

Because they tie, **source order is the only thing that resolves a row that is both disabled and
active**, and the disabled rule must remain last in both files. P98 shipped without the disabled
rules and produced a disabled row whose hint was twice as bright as its own label. P100 rewrites the
*middle* rule of each triple and must not disturb the order.

Note the middle rule now sets the same value as the base rule and is therefore redundant *as CSS*.
**Keep it anyway, with its comment.** It is the documented seam where the active-state ink is
specified; deleting it makes the next pass believe no active override ever existed and invites the
regression back.

---

## 3. Recipe A-seg — the segmented recipe (surfaces 5–6)

Both are two-button toggle groups, which is exactly `ui-reference.md` §12.3.2's control kind. Use
that recipe verbatim rather than the §2.1 inset bar, so the merge-editor mode toggle and the
worktree copy toggle look like every other segmented control in the app.

### 3.1 `.conflict-editor-mode-btn.is-active` — `src/styles/conflicts.css:199-202`

```css
.conflict-editor-mode-btn.is-active {
  /* P100 + P98 §8.9 NIT-1: `--accent-fg` was never a token, so this rule survived only on
     its hardcoded #fff fallback — and #fff on --accent is 3.22:1 dark. House segmented
     recipe (ui-reference §12.3.2): --text-1 on --selection = 9.36:1 / 13.29:1; the 600
     weight and the accent border are the non-colour carriers. */
  background: var(--selection);
  color: var(--text-1);
  font-weight: 600;
  border-color: var(--accent);
}
```

- **NIT-1 supersession, stated explicitly.** P98 §8.9 prescribed `var(--accent-text)` for this
  line. That prescription is **superseded, not ignored**: the whole rule is rewritten, the phantom
  `--accent-fg` is gone, and no `--accent-text` is needed here. The defect NIT-1 named (a second
  phantom token in the file P98 §4 just repaired) is closed by this rule.
- The button already has a `1px solid var(--border)` context (`:190-192` sets the divider between
  siblings). If the base `.conflict-editor-mode-btn` rule at `:179` has no `border`, add
  `border: 1px solid transparent` there so `border-color` in the active rule has something to
  colour and the two states are the same height — **state the 0-or-1px box change in the
  implementation report**, per §2's `--border-0` box-model lesson. Do not silently claim zero
  layout change.
- **Contrast of the carriers:** `--accent` border on `--selection` is **2.6:1** dark / **3.6:1**
  light — decorative delineation only (§2), which is why the **600 weight** is the carrier that
  must ship alongside it.
- `:hover` at `:194-196` (`--bg-2` + `--text-1`) is unchanged and, being (0,1,1)+pseudo-class =
  (0,2,0), it must stay **before** the `.is-active` rule (0,2,0) so an active button keeps its
  selected look under the cursor. Verify after editing.

### 3.2 `.wt-copy-toggle-on` — `src/styles/dialogs-forms.css:162-165`

The two `!important`s exist only because `.wt-copy-toggle-on` is **(0,1,0)** and loses to
`.wt-copy-toggle button` at **(0,1,1)**. Fix the specificity, drop both:

```css
/* P100 + P98 §8.9 NIT-2: was `background: var(--accent) !important; color: #fff !important`
   — a hardcoded literal on an accent fill at 3.22:1 dark. Selector qualified to (0,2,1) so
   it beats `.wt-copy-toggle button` (0,1,1) on specificity; both !important are dropped. */
.wt-copy-toggle button.wt-copy-toggle-on {
  background: var(--selection);
  color: var(--text-1);
  font-weight: 600;
  border-color: var(--accent);
}
```

- Specificity **(0,2,1)** — two classes + one type — beats `.wt-copy-toggle button` **(0,1,1)**
  regardless of source order. `!important` in this codebase is a smell; removing two is part of the
  fix, not a bonus.
- Same `border: 1px solid transparent` proviso as §3.1 if the base `.wt-copy-toggle button` rule at
  `:148-155` carries no border.
- `:disabled` at `:157-160` (`opacity: .6`) is unchanged and still applies on top; per §2's dimming
  budget, `.6` is spent once here and nothing nested may dim again.

---

## 4. Recipe B — `.btn-primary` (surface 7)

### 4.1 The change

**One token value, in `src/styles/tokens-and-base.css`:**

```css
:root {
  /* … */
  --accent-text: #16181d;   /* P100: was #ffffff — 3.22:1 on the --accent fill in dark.
                               Near-black ink on the dark theme's light-blue accent is
                               5.52:1 (ui-reference §5 lane 0, symmetric). Same device as
                               §6's luminance-adaptive branch pill and
                               .diff-stage-float button. */
}

[data-theme='light'] {
  /* … */
  --accent-text: #ffffff;   /* unchanged — 4.65:1 on the light theme's darker accent */
}
```

- **Light theme renders byte-identically.** `--accent-text` light stays `#ffffff`, so every
  light-theme accent fill is pixel-for-pixel what it is today.
- **Dark theme:** every remaining accent fill flips to near-black ink. After §2–§3 land, the only
  consumer left is `.btn-primary` (plus the dead `GraphColors.accentText` field). `--accent-text` is
  therefore no longer a trap token — it now means what its name says on the only fill it serves.
- **The stale comment must go.** `tokens-and-base.css:105` currently reads *"`--accent-text` is
  intentionally identical to the dark palette's value…"*. That becomes false. Replace with: `/* The
  two themes differ deliberately: the dark accent is a light blue and needs dark ink (P100). */`
- **No new token.** Redefining an existing one is strictly better than adding `--accent-fill` or
  `--accent-ink`: no `[data-theme]` duplication, no third blue in the palette, and the token's name
  already carries the semantics.

### 4.2 `.btn-primary` states, all of them, both themes

`src/styles/controls.css:82-102`. No declaration changes beyond what `--accent-text` gives for free;
this is the audit that says so.

| State | Rule today | Effect under the new ink | Verdict |
|---|---|---|---|
| default | `background: var(--accent); color: var(--accent-text)` | **5.52:1** dark / **4.65:1** light | ✓ AA |
| `:hover:not(:disabled)` | `filter: brightness(1.1)` (`:95-97`) | the filter brightens fill **and** ink together; the ink is near-black so it gains almost nothing while the fill gains luminance — the pair goes to ≈**6.1:1** dark. Light theme: white ink is already clipped at 1.0, fill brightens → ratio *drops* slightly from 4.65 toward ≈4.5 | ✓ dark improves; **light is the tight one — AC7 measures it** |
| active/pressed | none today | inherits hover | ✓ no change |
| `:focus-visible` | global 2px `--accent`, 1px offset (§2) | `--accent` ring against the surrounding `--bg-0`/`--bg-1` = **5.52:1** / **4.65:1** | ✓ ≥3:1 |
| `:disabled` | `opacity: .6` (`:99-101`) | inert control, §2 dimming budget, spent once | ✓ exempt (WCAG 1.4.3) |
| loading/busy | house pattern: participle label + `disabled` + `aria-busy` (§8) | no colour change | ✓ |
| long content | unchanged: `padding: 0 16px`, height 32px, no wrap | ✓ | ✓ |

If AC7 finds the light-theme hover below 4.5:1, the sanctioned remedy is to replace the
whole-element `filter: brightness(1.1)` with a fill-only lift —
`background: color-mix(in srgb, var(--accent) 92%, #ffffff)` — which brightens the fill without
touching the ink. **Do not ship that speculatively**; ship it only if AC7 fails.

### 4.3 Perceptual cost — the honest statement

Every primary button in the dark theme goes from *white on blue* to *near-black on blue*. That is a
visible identity change the user did not ask for, and it is the single riskiest thing in this
contract. It is also the only way to keep the loud accent fill on the app's primary action while
passing AA in the default theme. Alternatives considered and rejected:

- **Recipe A on `.btn-primary`** (`--selection` fill): demotes every primary action in the product
  to the visual weight of a hovered list row. Rejected — this is a "quiet the whole app" change
  masquerading as an a11y fix.
- **B2 — a new darker `--accent-fill` token + keep white ink.** A dark-theme fill around `#2a63cc`
  gives white **5.60:1**, but: it adds a token that must be defined in both themes, introduces a
  third blue beside `--accent` and lane 0, and its own edge against the `--bg-0` panel falls to
  ≈**3.17:1** — barely over the graphics bar, where today's accent fill sits at 5.52. It also
  leaves `--accent-text` a sub-AA trap token for anyone who uses it on `--accent`. Rejected;
  recorded here so it is not re-litigated.

**Recommendation: ship Recipe B inside P100.** It is one token value, the light theme does not move,
and the USER CHECKPOINT already has to sign off on the perceptual result of §2–§3 — folding
`.btn-primary` in costs the user one look, not two. **Flagged for the orchestrator (§6-A):** if the
call is to defer it, §2's KNOWN SHORTFALL bullet is **rewritten to name `.btn-primary` as the sole
remaining case**, not retired (see §5.2).

---

## 5. `ui-reference.md` edits

### 5.0 Process flag — read this first, it is why the last two passes silently failed

`docs/contracts/ui-reference.md` is **1300 lines / ≈40k tokens**. My `Write` tool is whole-file
only, and 40k tokens exceeds a single response's output budget — a whole-file rewrite **truncates
mid-file**, which is precisely the failure mode observed in P95 (a patch silently unapplied) and
guarded against since. I am therefore **not** rewriting it in this pass. The edits below are
specified as **verbatim, line-anchored hunks** to be applied with `Edit`.

**This is a deviation from the briefed process and from CLAUDE.md's "no other agent edits
`ui-reference.md`".** I retain ownership of the *content* — every hunk below is final, verbatim
text, not a suggestion. Who applies the keystrokes (senior-dev inside this increment, or the
orchestrator) is the orchestrator's call; see §6-D.

**Counts for the orchestrator's hunk-confinement check:**

| | Lines | Top-level `##` sections | Tail sentinel |
|---|---|---|---|
| Before (verified this pass) | **1300** | **13** | line 1299: `…are canvas-drawn (not SVG DOM), keep their 1.4 stroke, and are out of scope for this migration.` |
| After (predicted) | **≈1315** (+15: hunk 1 ±0, hunk 2 +1, hunk 3 +14, hunk 4 +0/+1) | **13** (unchanged) | unchanged |

The +15 is a prediction, not a measurement — AC10 verifies the **structural** invariants and asks
for the realized count to be reported, rather than pinning an exact number a correct
implementation might miss by one wrapped line.

Hunks touch **§2 only** — lines 61, 83–85, 106–110, and 152–167. Every other line of the file,
including **§2's `--text-3` / "122 declarations unaudited" P101 pointer (lines 72–82), must remain
byte-identical.** That pointer is P101's to retire, not P100's.

**On the brief's "update the relevant §12.x rows for the active-row recipe":** I looked, and the
relevant rows are **not** in §12. §12.1 (rail: `--selection` + `inset 2px 0 0 var(--accent)`) and
§12.3.2 (segmented: `--selection` + `--text-1` + 600 + accent border) already specify exactly the
two recipes P100 adopts — they are this contract's *precedents* and need no edit. The only stale
active-row sentence in the whole document lives in §2, at lines 106–110, and hunk 4 repairs it. That
is a finding, not a skipped instruction.

### 5.1 Hunk 1 — the token table, line 61

```diff
-| `--accent-text` | `#ffffff` | `#ffffff` | text on accent |
+| `--accent-text` | `#16181d` | `#ffffff` | text on a solid `--accent` fill (**5.52:1** dark / **4.65:1** light) — the two themes differ deliberately, P100 |
```

*(Apply only if Recipe B ships. If deferred, leave line 61 alone.)*

### 5.2 Hunk 2 — the shortfall count, lines 83–85

**If Recipe B ships (recommended):**

```diff
-`docs/contracts/P98-text3-readtext-ui.md` §8.8. **Two known AA shortfalls remain** — white text on
-the `--accent` fill in the dark theme (see the KNOWN SHORTFALL bullet), and the unaudited `--text-3`
-remainder. **New surfaces must not add to any of these**:
+`docs/contracts/P98-text3-readtext-ui.md` §8.8. **One known AA shortfall remains** — the unaudited
+`--text-3` remainder. The accent-fill shortfall is **closed** (P100, 2026-09-01): the full survey
+found seven text-bearing accent fills and fixed all seven; see the ACCENT FILL bullet for the two
+recipes that replaced it. **New surfaces must not add to it**:
```

**If Recipe B is deferred:** keep "Two known AA shortfalls remain" and change the first clause to
`--accent-text on the --accent fill of .btn-primary in the dark theme (3.22:1 — the sole remaining
case after P100; every list/segment surface was fixed)`. **Do not claim closure while any surveyed
surface is unfixed.**

### 5.3 Hunk 3 — replace the KNOWN SHORTFALL bullet, lines 152–167

Delete lines 152–167 in full and insert:

```markdown
- **ACCENT FILL — two recipes, and the retracted "white is the ceiling" claim (P98, revised and
  closed 2026-09-01, P100).** `--accent-text` on `background: var(--accent)` was `#ffffff` in both
  themes and measured **3.22:1** dark / **4.65:1** light, putting a sub-AA primary label on every
  accent-filled surface in the default theme. P98's conclusion that *"white is the ceiling, so only
  the fill can fix this"* was **wrong**: white is the ceiling only among *lighter* inks. Going
  darker passes — `#16181d` on the dark accent is **5.52:1** (§5 lane 0, symmetric), and
  `.diff-stage-float button` and §6's luminance-adaptive branch pill already shipped that device.
  P100 surveyed all 7 text-bearing accent fills (plus 11 decorative ones, which are fine at the 3:1
  graphics bar) and closed the class with two recipes. **Pick by what the surface is:**
  1. **A state (selected row, active option, segment) ⇒ change the fill.** House selected-row
     recipe: `background: var(--selection)` with `--text-1` label (**9.36:1** dark / **13.29:1**
     light) and `--text-2` secondary (**5.01** / **6.42**). Because `--selection` vs `--bg-1` is
     only ~1.3:1, a **non-colour carrier is mandatory**: an `inset 2px 0 0 var(--accent)` leading
     bar for list rows (`.diff-tree-selected`, the §12.1 rail, `.combobox-option--active`,
     `.command-palette-option.is-active`), or §12.3.2's `font-weight: 600` +
     `border-color: var(--accent)` for segmented controls (`.conflict-editor-mode-btn.is-active`,
     `.wt-copy-toggle-on`). Never `font-weight` on a pointer-synced list row — it reflows the label
     under the cursor. A neutralised disabled+active row must also reset `box-shadow: none`.
  2. **An action (primary button) ⇒ keep the fill, flip the ink.** `--accent-text` is `#16181d`
     dark / `#ffffff` light — deliberately different per theme, because the dark accent is a light
     blue. `.btn-primary` is the only consumer. A hue fill that must stay loud keeps its hue and
     takes dark ink; it does **not** get demoted to `--selection`.
  3. **Never dim text on a hue fill with a hardcoded `rgba(255,255,255,α)`.** P98 removed the two
     instances (**2.61:1** dark / **3.56:1** light). Subordination on a filled row is carried by
     size (11px vs 13px) and right-edge placement, never by opacity — and on a `--selection` fill
     it is carried by the real `--text-1` ↔ `--text-2` colour step.
  4. **The same defect exists for `--danger` fills and is not yet fixed** — `.btn-danger`
     (`controls.css:70`) and `updates.css:114` put a hardcoded `#ffffff` on `var(--danger)`, which
     §6 measures at **3.70:1** in dark. `partial-staging.css:104` already ships the ink flip
     (`--bg-0` on `--danger`, **4.80:1** dark). Tracked as **P102**; a new `#fff`-on-hue fill
     anywhere is a defect.
```

### 5.4 Hunk 4 — the now-false active-hint sentence, lines 106–110

The tail of §2's "eight selectors P98 swept" bullet currently ends:

> Plus the two active-state hint overrides, which moved from a hardcoded `rgba(255,255,255,.8)` to
> `var(--accent-text)`.

After P100 those overrides are `--text-2` on a `--selection` fill, so the sentence is false. Replace
that sentence (and only that sentence) with:

```markdown
    hardcoded `rgba(255,255,255,.8)` to `var(--accent-text)` in P98, and then to **`--text-2`** in
    P100 when the `--accent` fill under them was retired for `--selection` (see the ACCENT FILL
    bullet).
```

### 5.5 What must **not** change

Lines 1–60, 62–82, 86–105, 111–151, 168–1300. In particular: §2's P101 pointer (72–82), the
`--accent`-as-*text*-on-`--selection` bullet (168–175), the `--border-0` phantom-token bullet
(176–183), and **every §12.x section** — P100 edits none of them, for the reason given in §5.0.

---

## 6. Flagged for the orchestrator

**A. Does `.btn-primary` ship in P100?** It is the largest perceptual change here (every primary
button in the dark theme flips to dark ink) but the smallest diff (one token value; light theme
unmoved). **My recommendation: ship it in P100.** It is the same defect class, the survey found it,
and deferring means asking the user to sign off on accent-fill perception twice. If deferred, apply
§5.2's "Two known shortfalls" variant and skip §5.1 — do **not** let the reference claim closure.

**B. Recipe B vs B2 (new `--accent-fill` token + white ink).** Recommendation: **B**, per §4.3.
Recorded so it is not re-opened.

**C. The `--danger` sibling (§1.5).** Recommendation: **file as P102, do not expand P100.**

**D. The `ui-reference.md` whole-file-Write problem (§5.0).** The reference doc has outgrown my
tool, and this pass therefore delivers hunks rather than a rewritten file — a deviation from both
the brief and CLAUDE.md's single-editor rule. Recommendation: from P100 on, ui-designer supplies
verbatim hunks and they are applied with `Edit`; the orchestrator's existing
line/section/sentinel check then verifies a real diff rather than a 40k-token retype. This is the
structural cause of the P95 silent failure and it will recur on every future pass otherwise.

**E. `GraphColors.accentText` (`src/graph/colors.ts:18,134`) is read but never drawn.** Dead field.
Not a P100 change; file as a NIT.

---

## 7. Component decomposition

**None.** P100 creates no component and touches no `.tsx`. Files changed:
`src/styles/tokens-and-base.css` (1 value + 1 comment), `src/styles/dialogs-forms.css` (2 rules
rewritten, 1 selector requalified, 2 declarations added), `src/styles/search.css` (2 rules
rewritten, 1 declaration added), `src/styles/conflicts.css` (1 rule rewritten, possibly 1
`border: 1px solid transparent` added), `src/styles/controls.css` (**no change** — §4.2 is an audit,
not an edit), `docs/contracts/ui-reference.md` (§5 hunks). No file approaches the ~500-line limit as
a result.

## 8. Microcopy

**No string changes.** P100 is colour and specificity only. No label, tooltip, error message, empty
state or `aria-label` is added, removed or reworded. Nothing here is destructive, so no confirmation
copy is in scope.

## 9. Motion

**No motion added or removed.** `.btn-primary`'s `filter: brightness(1.1)` hover is instantaneous
today and stays so. The segmented `border-color` change is untransitioned. No
`prefers-reduced-motion` block changes. Nothing touches the canvas render path — the commit graph
reads `--accent` and `--selection` through `src/graph/colors.ts`, and **neither token's value
changes**, so 20k-row layout and paint are bit-identical.

## 10. Harness states

`VITE_MOCK_IPC=1`, port 1420. Existing fixtures suffice — **no new fixture is requested.**

| Surface | Reachable in the harness? |
|---|---|
| `.command-palette-option.is-active` | **Yes** — `Ctrl/Cmd+K`, arrow down. Default (no-repo) state reaches it. |
| `.command-palette-option.is-disabled.is-active` | **Yes** — a repo-gated command with no repo open; arrow onto it. |
| `.btn-primary` | **Yes** — the no-repo `EmptyState` "Open repository" button. |
| `.combobox-option--active` | **Yes, with a repo fixture open** — any Combobox row in Settings (`Ctrl/Cmd+,`). |
| `.conflict-editor-mode-btn.is-active` | **Only with the conflict fixture** — needs a conflicted-merge mock state. If unreachable, the check degrades to a computed-style read on the rule and the backdrop half is asserted from CSS source; **say "not reachable", do not report it as measured** (P98 §5-C precedent). |
| `.wt-copy-toggle-on` | **Unlikely** — the new-worktree dialog's copy toggle needs a worktree-capable fixture. Same degradation rule. |
| Pathological long content | Palette: a long command label + a long shortcut hint (`Ctrl/Cmd+Shift+Alt+K`) on one active row; Combobox: a 120-char branch name with a 40-char hint. Assert the label ellipsizes, the hint stays flush right, and the 2px bar is not clipped by the 4/5px radius. |

**Harness gotchas** (these have cost sessions): the pane reports `innerWidth/innerHeight = 0`, so
call `resize_window(1440×900)` **before** any measurement or every `vh`/`vw` rule evaluates to 0;
`resize_window`'s `colorScheme` does **not** switch this app's theme — it reads a `data-theme`
attribute on `<html>` and never `prefers-color-scheme`, so set the attribute directly or you will
measure dark twice and report it as both themes; `setTimeout` is throttled to ~1s in a hidden page,
so batch `javascript_tool` dispatches.

---

## 11. Acceptance criteria

Every contrast claim is stated for **both themes**. "Composited" means read from
`getComputedStyle` on a mounted element and its actual painted backdrop, not from the CSS source
(P98 §8.1 method).

1. **AI gate.** `Grep` for `background(-color)?:\s*var\(--accent[,)]` across `src/styles/**` returns
   exactly: the 11 decorative surfaces of §1.3 (12 matching lines — `dialogs-forms.css` contributes
   two), `.diff-stage-float button` (§1.2), and `.btn-primary` (§4) — **13 lines total**. No surface
   listed in §1.1 rows 1–6 still carries a solid `--accent` background. *(The `[,)]` class is
   required: `onboarding.css:35` is `var(--accent, var(--text-1))` and does not match a
   `var\(--accent\)`-only pattern.)*
2. **AI gate.** `Grep` for `--accent-fg` across the repo returns **zero** matches. The phantom token
   is gone (NIT-1 closed).
3. **AI gate.** `Grep` for `#fff` / `#ffffff` in `src/styles/dialogs-forms.css` returns zero matches
   in `.wt-copy-toggle*` rules, and `Grep` for `!important` in that file returns zero matches inside
   `.wt-copy-toggle*` (NIT-2 closed, both `!important`s dropped).
4. **AI gate.** Composited, mounted, **dark**: `.command-palette-option.is-active` label measures
   **≥9.3:1** and its `.command-palette-option-hint` **≥5.0:1**. **Light**: **≥13.2:1** and
   **≥6.4:1**. Same four figures for `.combobox-option--active` where the fixture reaches it; where
   it does not, report "not reachable" and verify from source.
5. **AI gate.** A row that is **both disabled and active** —
   `.command-palette-option.is-disabled.is-active` — measures `--text-3` on **both** its title and
   its hint (i.e. the AC19 ordering survives), and its computed `box-shadow` is `none`. Both themes.
6. **AI gate.** Source-order assertion, both files: in `dialogs-forms.css` the three
   `.combobox-option*hint` `color` rules appear in the order base → `--active` → `--disabled`, and
   in `search.css` the three `.command-palette-option*hint` rules likewise. **AC19 still passes** —
   this criterion is the explicit restatement of it.
7. **AI gate.** `.btn-primary` composited: default **≥5.5:1** dark / **≥4.6:1** light;
   **`:hover`** (with `filter: brightness(1.1)` applied) **≥4.5:1 in both themes**. If the light
   hover lands below 4.5, apply §4.2's fill-only remedy and re-measure. *(Skip if Recipe B is
   deferred per §6-A.)*
8. **AI gate.** `--accent-text` computes to `#16181d` under `:root` and `#ffffff` under
   `[data-theme='light']`, and `tokens-and-base.css`'s "intentionally identical" comment is gone.
   *(Skip if Recipe B is deferred.)*
9. **AI gate.** No new CSS custom property is introduced. `Grep` for `--accent-fill` / `--accent-ink`
   / `--on-accent` returns zero. No hardcoded hex is added to any component or stylesheet outside
   `tokens-and-base.css`.
10. **AI gate.** `ui-reference.md` after the §5 hunks: **13** top-level `##` sections, tail sentinel
    at the last line unchanged, lines 72–82 (§2's P101 `--text-3` pointer) **byte-identical** to
    before, and `git diff` confined to §2 (no hunk header outside lines 46–211). The realized line
    count is **reported** in the implementation report and compared against §5.0's ≈1315 prediction;
    a mismatch of ±2 from line wrapping is fine, a mismatch of tens means a truncated write.
11. **AI gate.** No `.tsx`, no Rust, no fixture file is modified. `pnpm gate` green (step 7 only;
    intermediate rounds use `--frontend`).
12. **AI gate.** Both densities: `.command-palette-option` and `.combobox-option` computed height,
    padding and font-size are identical under `data-density="cozy"` and `"compact"` and identical to
    the pre-P100 values (these surfaces are density-invariant, §2.2).
13. **AI gate.** Long-content case (§10): with a 120-char label and a 40-char hint on the active
    row, the label ellipsizes, the hint is not clipped, the row does not wrap, and the 2px inset bar
    renders at the row's left edge.
14. **USER CHECKPOINT.** In `pnpm tauri dev`, dark theme: the command palette's active row is still
    obviously the active row at a glance while arrowing quickly through 20+ commands. This is the
    perceptual cost of Recipe A and only a human can sign it off.
15. **USER CHECKPOINT.** Dark theme: every primary button (`Open repository`, `Commit`, `Merge…`)
    with near-black ink on the blue fill reads as the loudest thing on its surface and does not look
    like a disabled or "washed" button. *(Skip if Recipe B is deferred.)*
16. **USER CHECKPOINT.** Light theme: no visible change anywhere. Recipe A's light figures and
    Recipe B's light ink are both unchanged from today, so any perceived difference is a defect.
17. **USER CHECKPOINT.** The merge-editor mode toggle and the new-worktree copy toggle read as the
    same kind of control as Settings' segmented controls (§12.3.2) — one visual language for
    segmented state across the app.
18. **USER CHECKPOINT.** Scroll a 20k-row graph after the change and confirm no regression in feel.
    Nothing in this contract touches the canvas, but the harness is headless
    (`requestAnimationFrame` does not fire), so frame-feel is not mine to certify.
