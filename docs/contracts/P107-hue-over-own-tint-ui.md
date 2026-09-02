# P107 — Hue-over-own-tint remediation (UI contract)

**Owner:** ui-designer · **Written:** 2026-09-02 · **Supersedes** the 16-row table in
`docs/contracts/ui-reference.md` §2 (kept there as the inherited evidence; this file is the
enumeration of record).
**Related:** `P102-P105-hue-audit-ui.md` (the shipped accent/fill halves), `P101-text3-audit-ui.md`
(the §3 bucket+verdict format this file follows), **P106** (A/M/D/U/R status badges — see §8, it owns
one call site this search surfaced).

---

## 0. Result in one screen

| | Count |
|---|---|
| Hue-ink-over-its-own-tint instances found **in total** | **38** |
| — Bucket A: **read text**, judged 4.5:1 → **FIX** | **17** |
| — Bucket B: **glyph/bar**, judged 3:1, measured ≥3:1 in both themes → **KEEP** | **19** |
| — Bucket C: **glyph**, judged 3:1, **fails** in one theme → **FIX** | **1** |
| — Bucket D: owned by another milestone (P106) → **not touched here** | **1** |
| Declarations P107 changes | **18** (17 A + 1 C) |
| CSS files touched | **9**, all under `src/styles/` |
| Component / `.tsx` files touched | **0** |
| New tokens | **0** (see §6, including the `--warning-text` verdict: **not needed**) |

**The count moved off 16.** The predecessor list was 16 because it was built from a search that could
only see *same-rule-block* pairs. The descendant- and inheritance-aware search in §1 found **one more
failing text site** (`.error-boundary-title`), **one failing glyph state**
(`.error-dismiss:hover`, light **2.96:1**), **19 passing glyph instances** that a blanket sweep would
have wrecked, and **one instance that belongs to P106**. It also found a **false claim in
`ui-reference.md` §2** — see §7. That is the search working.

---

## 1. The search that was run (state this in the review; it is the deliverable)

The recipe is *any hue ink composited over a tint of that same hue*. There are **three structurally
different ways** that can be written in CSS, and each needs its own pass. Prior passes ran only (i),
which is why every prior count was low.

### (i) Same rule block — `color` and `background` in one selector

```
rg -n "color:\s*(var\(--(danger|success|warning|merged|accent)\)|color-mix\([^;]*--(danger|success|warning|merged|accent))" src/styles
rg -n "background(-color)?:\s*[^;]*color-mix\([^;]*var\(--(danger|success|warning|merged|accent|h,)" src/styles
```
Intersect by file+adjacent line. **Yield: 16 (the inherited list) + 0 new.**

### (ii) Descendant combinator — ink and tint in *different* rules

```
rg -nU --multiline-dotall "^[^\n{}/]*\.[\w-]+[^\n{},]*\s+[.:\[&>][^\n{}]*\{[^{}]*color:\s*var\(--(danger|success|warning|merged|accent|h)\)" src/styles
```
13 descendant hue-ink rules exist app-wide. Cross-referenced against the tint inventory from (i):
5 sit on a same-hue tint (`.checks-rollup-pill--{good,warn,pending} .checks-rollup-glyph`,
`.graph-filter-chip-stale .graph-filter-chip-glyph`), the rest sit on neutral backdrops.
**Yield: 5 instances, all glyphs.** This is the pattern that hid `.pr-state-open` from P102.

### (iii) The two patterns (ii) *still* misses — found here, record them

**(iii-a) Unqualified child class whose only parent is tinted.** The child rule names no ancestor at
all, so no selector-shaped grep can find it; only the DOM nesting reveals the backdrop.
Found: `.dev-mode-pill-glyph` (`controls.css:217`, only ever rendered inside
`.dev-mode-pill.is-write-failed`'s 14%/22% `--danger` tint), `.forge-reauth-icon`
(`forge-pr.css:789`, inside `.forge-reauth-banner`'s 12% `--warning` tint),
`.ai-dock-ask-guard-glyph` (`ai-dock-log.css:271`, inside `.ai-dock-ask`'s 14% `--warning` tint),
`.error-dismiss` (`empty-and-errors.css:113`, inside `.error-banner`'s 12% `--danger` tint),
`.error-boundary-title` (`empty-and-errors.css:75`, inside `.error-boundary`'s 8% `--danger` panel).
**Procedure:** take the tint inventory from (i) — 25 tinted containers — and for each, read the
component that renders it and check every child's ink. There is no grep substitute.

**(iii-b) Custom-property indirection.** `--h` carries the hue, so neither `color: var(--danger)` nor
`background: …var(--danger)…` appears anywhere in the rule:

```
rg -n -- "--h\b" src/styles
```
Four families do this — `.toast` / `.toast-glyph`, `.submodule-badge-{ok,warn}` /
`.submodule-badge-glyph`, `.ai-dock-status[data-status] / .ai-dock-status-glyph`,
`.git-{run,run-hook}-pill[data-status] / .git-run-pill-glyph` — **11 instances**, every one a
glyph-over-its-own-tint. A fifth alias family hides behind *token* indirection:
`--badge-good` is byte-identical to `--success` and `--badge-warn` to `--danger` in **both** themes
(`tokens-and-base.css:59-60,151-152`), so `.checks-rollup-pill--good/--warn` are success- and
danger-over-own-tint under another name. **Any future search must run the hue alphabet
`danger|success|warning|merged|accent|badge-good|badge-warn|h`, not just the first five.**

**Total population: 16 + 5 + 5 + 11 + 1 (P106's staged-section composite, §8) = 38.**

---

## 2. How the ratios were measured (and the calibration that proves the method)

Measured in the running harness (`http://localhost:1420`, viewport 1440×900), both themes, on
2026-09-02. Method: an off-screen probe resolves each `color-mix()` through the **browser's** mixing
model; `color-mix(<hue> N%, transparent)` resolves to the hue at alpha N/100, which is then
source-over composited onto the resolved backdrop token; WCAG 2.x relative luminance and contrast are
computed from the composited sRGB. Both themes are read by toggling `data-theme` on
`documentElement` and restoring it.

**A parsing trap that cost the first run, recorded so the next pass avoids it:** Chrome serialises
some `color-mix()` results as `color(srgb 0.898 0.325 0.294 / 0.14)` — 0–1 components — and others as
`rgba(229, 83, 75, 0.14)` — 0–255. A single numeric regex over both silently produces near-black
tints and inflates every dark-theme ratio by ~0.8. Parse the `color(` form separately.

**Calibration — six values re-derived, against six independently recorded historical numbers:**

| Check | ui-reference §2 says | This run | |
|---|---|---|---|
| `--accent-strong` on 14% accent tint | 5.86 / 4.87 | **5.85 / 4.87** | ✓ |
| `--danger` on 14% danger tint / `--bg-2` | 3.35 / 3.48 | **3.35 / 3.48** | ✓ |
| `--warning` on 14% warning tint / `--bg-2` | 4.96 / 3.53 | **4.96 / 3.53** | ✓ |
| `--success` on 14% success tint / `--bg-2` | 4.07 / 3.66 | **4.07 / 3.66** | ✓ |
| `--accent` on 14% accent tint / `--bg-2` | 3.68 / 3.38 | **3.68 / 3.38** | ✓ |
| `--text-2` on its own 12% tint / `--bg-1` | 5.79 / 6.22 | **5.80 / 6.22** | ✓ |

Every number below is from the same run. Format is **dark / light**.

---

## 3. Enumeration — bucket + verdict per call site

### Bucket A — read text over its own tint. Bar 4.5:1. **FIX all 17.**

Backdrop column is the composited tint over the stated base. Every row is the label the user reads to
act (a verdict word, a banner sentence, a badge word), so none is exempt under §2's "must read it to
act" test.

| # | File:line | Selector | Hue / tint | Backdrop base | **Now** | `--text-1` after | Verdict |
|---|---|---|---|---|---|---|---|
| A1 | `forge-pr.css:299` | `.pr-mergeable-clean` | success 14% | `--bg-1` | **4.61 / 3.94** ✗L | 10.90 / 12.82 | FIX |
| A2 | `forge-pr.css:304` | `.pr-mergeable-conflict` | danger 14% | `--bg-1` | **3.78 / 3.76** ✗✗ | 11.61 / 12.59 | FIX |
| A3 | `forge-pr.css:309` | `.pr-mergeable-pending` | warning 14% | `--bg-1` | **5.64 / 3.80** ✗L | 10.49 / 12.91 | FIX |
| A4 | `ai-assets.css:35` | `.asset-badge-ok` | success 15% | `--bg-2` | **4.00 / 3.61** ✗✗ | 9.45 / 11.75 | FIX |
| A5 | `ai-assets.css:40` | `.asset-badge-warn` | warning 15% | `--bg-2` | **4.87 / 3.49** ✗L | 9.06 / 11.84 | FIX |
| A6 | `dialogs.css:165` | `.danger-badge.safe` | success 14% | `--bg-2` (explicit) | **4.07 / 3.66** ✗✗ | 9.61 / 11.90 | FIX |
| A7 | `dialogs.css:171` | `.danger-badge.caution` | warning 14% | `--bg-2` (explicit) | **4.96 / 3.53** ✗L | 9.24 / 11.99 | FIX |
| A8 | `dialogs.css:177` | `.danger-badge.destructive` | danger 14% | `--bg-2` (explicit) | **3.35 / 3.48** ✗✗ | 10.30 / 11.68 | FIX |
| A9 | `agent-assets.css:29` | `.asset-readonly-banner` | warning 12% | `--bg-1` | **5.87 / 3.90** ✗L | 10.92 / 13.25 | FIX |
| A10 | `agent-assets.css:52` | `.asset-issue-error` | danger 12% | `--bg-1` | **3.87 / 3.87** ✗✗ | 11.90 / 12.96 | FIX |
| A11 | `agent-assets.css:57` | `.asset-issue-warning` | warning 12% | `--bg-1` | **5.87 / 3.90** ✗L | 10.92 / 13.25 | FIX |
| A12 | `conflicts.css:67` | `.conflict-kind` | danger 12% | `--bg-1` | **3.87 / 3.87** ✗✗ | 11.90 / 12.96 | FIX |
| A13 | `empty-and-errors.css:92` | `.error-banner` | danger 12% | `--bg-1` | **3.87 / 3.87** ✗✗ | 11.90 / 12.96 | FIX |
| A14 | `graph-banners.css:14` | `.graph-truncated-banner` | warning 12% | `--bg-0` | **6.46 / 4.17** ✗L | 12.02 / 14.14 | FIX |
| A15 | `settings-legacy-sections.css:495` | `.settings-ai-status-warn` | warning 12% | `--bg-0` | **6.46 / 4.17** ✗L | 12.02 / 14.14 | FIX |
| A16 | `dialogs-forms.css:136` | `.wt-copy-chip` | danger 16% | `--bg-1` | **3.68 / 3.64** ✗✗ | 11.31 / 12.21 | FIX |
| A17 | `empty-and-errors.css:77` | `.error-boundary-title` | danger 8% | `--bg-1` (explicit) | **4.06 / 4.10** ✗✗ | 12.47 / 13.75 | **FIX — NEW, found by §1(iii-a)** |

✗L = fails 4.5:1 in light only · ✗✗ = fails in both themes. **All 17 fail in light; 8 fail in both.**
Worst: A8 at **3.35** dark. **Note the line-number correction:** §2's list said
`settings-legacy-sections.css:487` for A15; the declaration is at **495** (rule starts 491).

**Base sensitivity:** the verdict does not depend on resolving each site's exact ancestor. The same
ink/tint pair measures 4.61 / 3.94 over `--bg-1`, 4.07 / 3.66 over `--bg-2` and 4.9 / 4.0 over
`--bg-0`; **every combination fails 4.5:1 in light**, and the `--text-1` replacement clears 9:1 in the
worst case. The base column is recorded for the record, not because the verdict turns on it.

### Bucket B — glyph or bar over its own tint. Bar 3:1. **KEEP all 19, untouched.**

Each has a `--text-1` word beside it carrying the meaning (WCAG 1.4.1 satisfied by the word, not the
hue), so the ink is a graphic and 3:1 is the correct bar. A blanket sweep would have destroyed the
§11 pill recipe in four families.

| # | File:line | Selector | Hue / tint | **Measured** | Verdict |
|---|---|---|---|---|---|
| B1 | `checks-panel.css:103` | `.checks-rollup-pill--good .checks-rollup-glyph` | badge-good(=success) 14% | 4.61 / 3.94 | KEEP |
| B2 | `checks-panel.css:111` | `…--warn .checks-rollup-glyph` | badge-warn(=danger) 14% | 3.78 / 3.76 | KEEP |
| B3 | `checks-panel.css:119` | `…--pending .checks-rollup-glyph` | warning 14% | 5.64 / 3.80 | KEEP |
| B4 | `graph-filter.css:93` | `.graph-filter-chip-stale .graph-filter-chip-glyph` | warning 14% / `--bg-2` | 4.96 / 3.53 | KEEP |
| B5–B8 | `toasts-and-overlays.css:61` | `.toast-glyph` × 4 tones (`--h`) | 14% / `--bg-2` | danger 3.35/3.48 · success 4.07/3.66 · warning 4.96/3.53 · accent 3.68/3.38 | KEEP (P74's own fix) |
| B9–B10 | `sidebar.css:221` | `.submodule-badge-glyph` × 2 (`--h`) | 14% / `--bg-1` | ok 4.61/3.94 · warn 5.64/3.80 | KEEP (P73) |
| B11–B14 | `ai-dock.css:186` | `.ai-dock-status-glyph` × 4 (`--h`) | 14% / `--bg-1` | accent 4.16/3.63 · danger 3.78/3.76 · warning 5.64/3.80 · success 4.61/3.94 | KEEP |
| B15–B17 | `git-dock.css:270` | `.git-run-pill-glyph` × 3 (`--h`) | 14% / `--bg-1` | accent 4.16/3.63 · success 4.61/3.94 · danger 3.78/3.76 | KEEP |
| B18 | `controls.css:218` | `.dev-mode-pill-glyph` (rest **and** hover) | danger 14% → 22% | rest 3.78/3.76 · **hover 3.39/3.33** | KEEP — hover is the margin case, still ≥3:1 |
| B19 | `forge-pr.css:791` | `.forge-reauth-icon` | warning 12% | 5.87 / 3.90 | KEEP |
| — | `ai-dock-log.css:273` | `.ai-dock-ask-guard-glyph` | warning 14% | 5.64 / **3.80** | KEEP — **but the code comment beside it claims "5.4:1 dark / 3.6:1 light", which is the ratio against `--bg-1`, not against the tint the glyph actually sits on. Correct the comment; the verdict is unchanged.** |
| — | `conflicts.css:99` | `.conflict-action-ai:hover` (`--accent-strong` on 14% accent) | 5.85 / 4.87 | KEEP — the one sanctioned hue-**text** exception (§2) |

### Bucket C — glyph over its own tint that **fails**. **FIX 1.**

| # | File:line | Selector | State | **Measured** | Verdict |
|---|---|---|---|---|---|
| C1 | `empty-and-errors.css:122` + `:130` | `.error-dismiss` (× glyph on `.error-banner`) | **rest** — danger ink on the banner's 12% danger tint | 3.87 / 3.87 | passes 3:1 at rest |
| C1 | ″ | ″ | **hover** — danger ink on 20% danger *stacked on* the banner's 12% | **3.03 / 2.96** | **FAILS 3:1 in light. FIX.** |

This is the entry that only a **per-state** backdrop resolution can find: the control is compliant at
rest and non-compliant the moment the pointer touches it, because its hover fill is a *deeper tint of
its own ink's hue*. Nominal-parent analysis reports it as fine.

### Bucket D — same pattern, **owned by P106**, not touched here. See §8.

| File:line | Selector | Composited backdrop | Measured | Owner |
|---|---|---|---|---|
| `status-panel.css:193` / `:216` | `.file-status-added .file-badge`, `.file-status-untracked .file-badge` | `--success` ink over `.status-section--staged`'s **6% `--success` tint** (`status-panel.css:28`) over `--bg-1` | **5.26 / 4.38** — fails 4.5:1 in **light** | **P106** |

---

## 4. Per-state backdrop resolution (required by the contract, and it changes answers)

Measured for the four sites that have a state-dependent backdrop. Rows marked → are the reason a
"measure the nominal parent" audit is not sufficient.

| Site | default | hover | selected | disabled |
|---|---|---|---|---|
| `.pr-mergeable-conflict` | 3.78 / 3.76 | on `.pr-row:hover` (`--bg-2`): **3.35 / 3.48** → worse | inside `.pr-row.selected` (12% accent): **3.25 / 3.26** → **worst** | n/a |
| `.pr-mergeable-clean` | 4.61 / 3.94 | 4.07 / 3.66 | **3.91 / 3.43** → worst | n/a |
| `.error-dismiss` glyph | 3.87 / 3.87 | **3.03 / 2.96** → **fails** | n/a | n/a |
| `.dev-mode-pill-glyph` | 3.78 / 3.76 | 3.39 / 3.33 → still passes | n/a | n/a |
| `.file-status-added .file-badge` (P106) | 5.26 / 4.38 (staged tint) | `--bg-2`: 5.06 / 4.37 | `--selection`: — (P106 to measure) | n/a |

After the Bucket A fix the same states measure, with `--text-1`: `.pr-mergeable-conflict` 11.61 /
12.59 default, 10.30 / 11.68 hover, **9.99 / 10.92** selected; `.pr-mergeable-clean` 9.24 / 11.16
selected. **Every state of every fixed site clears 4.5:1 in both themes with headroom ≥5.4.**

**Disabled note:** if any of these chips ever gains a disabled state, the §2 `.55` dim applied to
`--text-1` over a 12% danger tint measures **4.69 dark / 3.50 light** — below the text bar in light.
Disabled text is exempt (§2), but do not introduce a *dimmed enabled* variant of these chips.

---

## 5. The fix — three recipes, CSS only, zero DOM change

All 18 changed declarations live in `src/styles/`. **No `.tsx` file is touched**: every site already
renders a word that carries the meaning (`Clean` / `Conflicts` / `Checking…`, `Safe` / `Caution` /
`Destructive`, `In sync`, `conflict`, the banner sentence, `Something went wrong`), so no glyph or
label needs to be added to satisfy WCAG 1.4.1.

### R1 — Pills and badges (A1–A8, A12, A16): ink to the base rule, hue to a hairline edge

```
/* base rule */            color: var(--text-1);
/* each variant, keep */   background: color-mix(in srgb, var(--<hue>) N%, <base>);
/* each variant, add  */   box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--<hue>) 35%, transparent);
```
- Put `color: var(--text-1)` on the **base** rule (`.pr-mergeable`, `.asset-badge`, `.danger-badge`,
  `.asset-issue`) and **delete** the per-variant `color` declaration. None of those four base rules
  currently declares a `color` (verified), so this is one added declaration each.
- **`box-shadow: inset`, not `border`** — `.pr-mergeable`, `.asset-badge`, `.wt-copy-chip` and
  `.conflict-kind` have no border today, and `border-width: 1px` would add 2px to every pill's box
  (§2's box-model rule). `.danger-badge` already carries `border: 1px solid transparent`
  (`dialogs.css:161`) and its variants already set `border-color`, so **A6–A8 keep their existing
  border and add nothing**.
- **Deliberately NOT using the `--h` custom-property device** here, although four families in the app
  do. `--h` is inherited, so a component that reads `var(--h)` without setting it silently adopts an
  ancestor's hue; introducing it into dialog-scoped badges widens that surface for no visual gain.
  Filed as a NIT, not adopted.

### R2 — Banners (A9–A11, A13–A15, A17): ink to `--text-1`, hue to a 3px leading bar

```
color: var(--text-1);
box-shadow: inset 3px 0 0 var(--<hue>);   /* omit for .error-boundary-title — see below */
```
- Exact device and geometry already shipped as `.dev-warning-bar` (`settings-dev.css:16-19`) and as
  P74's toast leading edge. **Reuse it; do not invent a variant.**
- **Zero layout change:** the bar is painted inside the banner's existing `padding: 8px 12px`, so text
  keeps its current position with a 9px gap to the bar. Do **not** add `padding-left`.
- The bar is a solid hue and clears the graphics bar against both the banner tint and the page:
  warning bar **6.46 / 4.17** vs its own 12% tint and **7.92 / 4.87** vs `--bg-0`; danger bar
  **3.87 / 3.87** vs its own 12% tint and **4.41 / 4.60** vs `--bg-1`.
- **A17 `.error-boundary-title` takes the ink change only.** `.error-boundary`
  (`empty-and-errors.css:70-72`) already draws a 40% danger border on all four sides; a leading bar
  inside it would read as a double rule.
- **Blast radius, state it in the review:** `.error-banner` is consumed by **12+ components**
  (`AiOutputPanel`, `CommitBox`, `ConflictEditor`, `ComparePanel`, `CommitPanel`, `ComposerDialog`,
  `DiffOverlay`, `DiffBrowser`, `DiffImageCard`, `AiAssetsPanel`, `AgentAssetEditor`,
  `ChecksEmptyState`, `AiActivityPanel`). One rule change fixes all of them; verify at least three in
  the harness.

### R3 — `.error-dismiss` (C1): ink to `--text-1`, hover wash to `currentcolor`

```
.error-dismiss        { color: var(--text-1); }
.error-dismiss:hover  { background: color-mix(in srgb, currentcolor 12%, transparent); }
```
- Measured after: ink on the hovered wash **8.48 / 10.30**. (Keeping `--danger` ink on that same wash
  measures **2.76 / 3.07** — the ink must move, the wash alone is not enough.)
- `color-mix(in srgb, currentcolor 12%, transparent)` is the existing house hover for a borderless
  glyph button — `.graph-filter-clear` (`graph-filter.css:84`). Reuse, don't invent.
- The 24×24 hit target, `border-radius: 4px` and the global `:focus-visible` ring
  (`tokens-and-base.css:205-208`, 2px `--accent`, 1px offset) are unchanged and already correct.

### What must **not** change

`.toast-glyph`, `.submodule-badge-glyph`, `.ai-dock-status-glyph`, `.git-run-pill-glyph`,
`.checks-rollup-glyph` (3 variants), `.graph-filter-chip-glyph` under `-stale`,
`.dev-mode-pill-glyph`, `.forge-reauth-icon`, `.ai-dock-ask-guard-glyph`,
`.conflict-action-ai:hover`. Nineteen keeps plus the sanctioned accent exception — **touching any of
them is a defect in this increment.**

---

## 6. Token decisions

**Zero new tokens.** `tokens-and-base.css` is not edited by P107.

- **`--text-1` is the ink for every fixed site.** 9.06–12.47 dark / 11.68–14.26 light across all 17,
  in every state (§4). It is also what P73/P74/P102 already used for exactly this recipe, so the fix
  is consistency, not invention.
- **`--danger-text` / `--success-text` / `--merged-text` do NOT fit and must not be used here.** They
  are inks for a **solid** hue fill (near-black in dark, white in light). Measured on a 14% tint of
  their own hue: `--danger-text` **1.27 / 1.31**, `--success-text` **1.35 / 1.29** — effectively
  invisible. Reaching for them because the name matches the hue is the single most likely
  implementation error in this increment; it is called out here so review catches it.
- **`--accent-strong` does not apply.** None of the 17 sites is accent-hued. On the other hues' tints
  it measures 4.40–5.42 (dark) / 3.81–5.33 (light) — it would sometimes pass, but a blue label on a
  red "Destructive" badge is semantic nonsense. `--accent-strong` stays what §2 says it is: the read-
  text form of the accent hue only.
- **Is a `--warning-text` needed? NO.** Reasons, in order:
  1. P107 contains **zero solid-warning-fill sites**. Every warning site here is a *tint*, and a
     fill-ink token is the wrong tool for a tint (see the 1.27–1.35 measurements above).
  2. The app's only solid `--warning` fill — `.ai-dock-ask-glyph` (`ai-dock-log.css:229-230`),
     `background: var(--warning); color: var(--bg-0)` — already measures **7.92 dark / 4.87 light**
     and clears AA in both themes. There is no shortfall to fix.
  3. A `--warning-text` would be a **pure alias**: `--bg-0` is `#16181d` dark / `#ffffff` light, which
     is exactly what `--danger-text` / `--success-text` / `--merged-text` already resolve to. It would
     add a token and change no pixel.
  4. This answers §2's open note that "`--warning` has not been measured against a letterform
     anywhere": it now has. `--warning` **as a letterform on its own tint** is 3.49–4.17 in light —
     that is the A3/A5/A7/A9/A11/A14/A15 failure. `--bg-0` **as a letterform on a solid `--warning`
     fill** is 4.87 light / 7.92 dark — compliant. Both numbers go into `ui-reference.md` §2.
  - **Recommendation for a later pass (NIT, not P107):** if the `--*-text` family is ever completed
    for device consistency, define `--warning-text` with exactly the `--bg-0` values and switch
    `.ai-dock-ask-glyph` to it. Cosmetic; no a11y effect.

---

## 7. Correction required in `ui-reference.md` §2 — a claim that does not hold

§2 currently prescribes: *"keep the tint, label to `--text-1`, demote the hue to a 35% border or a
leading bar/glyph **at the 3:1 graphics bar**."* The bar/glyph half is true. **The 35% border half is
false.** Measured:

| Edge | vs the surface outside it | vs the tint inside it |
|---|---|---|
| `--danger` 35% over `--bg-1` | **1.58 / 1.69** | 1.39 / 1.42 |
| `--warning` 35% over `--bg-0` | **2.05 / 1.61** | — |
| `--danger` 40% over `--bg-1` (`.error-boundary`, shipped) | **1.98 / 2.16** | — |

A 35–40% hue edge is **decorative delineation**, nowhere near 3:1. That is acceptable *only* because
the word carries the meaning (WCAG 1.4.11 applies to graphics "required to understand the content"),
and it must be described that way. **It is not an "identity carrier" and it must never be the sole
carrier of a state.** This does not change any P107 fix — R1 still adds the hairline edge, as
decoration — but it retires a sentence that would otherwise license shipping a hue-only state with a
35% border and calling it compliant. `ui-reference.md` §2 is updated in this same pass.

---

## 8. Interaction with P106 (which is higher priority — read this before either lands)

**Exactly one call site is claimed by both searches, and P106 owns it.**

- P106's finding is `.file-status-deleted .file-badge` at **4.41 dark on `--bg-1`** — failing **at
  rest**. P107's search independently reproduces that (4.41 dark / 4.60 light) and adds the backdrops
  P106 has not yet measured: **`--bg-2` hover 3.89 / 4.24**, **`--selection` 3.05 / 3.96**, and — the
  one that matters here — **`.status-section--staged`'s 6% `--success` tint**, where the *added*
  badge is success-ink-on-success-tint at **5.26 dark / 4.38 light**, failing light at rest.
- **P107 does not touch `src/styles/status-panel.css`.** The whole A/M/D/U/R family — `.file-badge`
  letters, `.section-label-danger`, the staged-section tint — is P106's, including the composite this
  search surfaced. Handing P106 the extra backdrop is the useful output; claiming the selector is not.
- **Sequencing:** P106 first. It sets the house answer for "a hue letter that IS the whole label"
  (there is no word beside an `A`, so 4.5:1 is unavoidable and the fix is the ink, per P105 B16's
  `.file-status-renamed` precedent). P107 is independent of that answer and can land in either order,
  but if both are in flight, **`status-panel.css` must appear in exactly one diff.**
- P106 should also inherit §7: whatever it does, a 35% tint edge will not carry the badge.

---

## 9. Found by this search, **out of P107 scope** — propose as P108

The §1 search also crossed a different defect class: **hue ink as read text over a *neutral*
backdrop**, where §2's own measured pairs already show `--danger` at **4.41 dark on `--bg-1`** — below
4.5:1. This is not enumerated here and **no app-wide claim is made about it**; three instances were
noted incidentally:

- `settings-dev.css:81-83` `.dev-status-write-failed .dev-status-state` — 12px/600 danger text on
  `--bg-1` (`.dev-status-card`), **4.41 dark**. This is P91 §8.4's "Not writing" state — the string
  whose whole job is to report a failure.
- `ai-dock-log.css:192` `.ai-run-queue-reason` — 11px danger text on `--bg-1`/`--bg-2`.
- `forge-pr.css:278/283` `.pr-stat-add` / `.pr-stat-del` — success/danger `+n`/`−n` counts.

Recommend filing **P108 — hue-as-text over neutral surfaces** with the same enumerate/bucket/predict
discipline. Do **not** fold it into P107: mixing two defect classes in one diff is how the residue
prediction stops being checkable.

---

## 10. Themes, densities, states, motion, copy

- **Both themes:** every ratio in §3–§5 is stated dark / light and was read from both. No token is
  theme-conditional; no `[data-theme='light']` block is added.
- **Both densities:** none of the 18 declarations is a size, padding, height or font. `cozy` and
  `compact` row metrics (`--rp-row-h`, `--rp-section-pad`, `--rp-list-margin`) are untouched, and the
  `box-shadow: inset` device is chosen specifically so no box grows in either density.
- **States:** default / hover / selected measured in §4; `:focus-visible` unchanged (global ring);
  disabled unaffected (§4 note); loading, empty and error states of the host components are
  unaffected — only ink and an inset shadow change.
- **Long content:** `.error-banner` already sets `overflow-wrap: anywhere`; `.wt-copy-chip` and
  `.conflict-kind` are single short words. The 3px inset bar does not interact with wrapping. A
  long-path fixture is still required (§11) to confirm the banner bar renders down the full height of
  a wrapped 3-line message.
- **Motion:** none added, none removed. No transition on `color` (an instantaneous ink change on
  hover is correct for a 24px glyph button). `prefers-reduced-motion` unaffected.
- **Microcopy:** no string changes. One copy NIT recorded, not fixed here:
  `WorktreeCopyCandidates.tsx:107` renders **`unchecked`** through `.wt-copy-chip`, the *danger*-tinted
  chip — a neutral, not-yet-known state wearing the alarm styling. Recommend a
  `.wt-copy-chip--unknown` neutral variant (`--text-2` on a 12% `--text-2` tint, the §11 hueless
  recipe, measured 5.80 / 6.22). Component change; file as a follow-up.

---

## 11. Harness states (`VITE_MOCK_IPC=1`, `pnpm dev:mock`, port 1420)

Fixtures needed to see all 18 changed declarations in a plain browser. Each must be checkable in
**both** themes.

| Fixture state | Reaches |
|---|---|
| PR detail open, `mergeable` = clean / conflict / pending, one row **selected** and one **hovered** | A1–A3 + §4's selected/hover composites |
| AI assets panel with one in-sync and one drifted asset | A4, A5 |
| `ProposedOpDialog` for a safe, a caution and a destructive operation | A6–A8 |
| Agent asset editor: read-only asset **and** an asset with one error + one warning issue | A9–A11 |
| Status panel with a conflicted file (`both modified`) | A12 |
| Any IPC error injected into a diff/commit surface → `.error-banner`, **with** the dismissible variant so `.error-dismiss` is present; hover it | A13, C1 |
| Graph truncated at the row cap | A14 |
| Settings → AI with the CLI missing; Settings → Git config with a load error | A15 |
| Worktree copy-candidates dialog with a conflicting path | A16 |
| A component forced to throw → `ErrorBoundary` | A17 |
| **Pathological:** a 3-line wrapped error message; a 120-char branch/PR title beside a mergeable pill; a deep path in the worktree chip | banner bar over wrapped text, pill truncation |
| **Regression:** a toast of each tone, a submodule ok/warn badge, the AI-dock and git-dock status pills, a pending checks rollup, the stale graph-filter chip, the dev-mode pill in `is-write-failed` | the 19 Bucket B keeps must look **identical** before/after |

**Harness gaps — USER CHECKPOINT items.** If any of A6–A8 (`ProposedOpDialog`), A16 (worktree copy
dialog) or A17 (`ErrorBoundary`) has no mock path today, it cannot be verified in the browser and
must be confirmed in `pnpm tauri dev`. Also note the harness is headless: `requestAnimationFrame` does
not fire, so hover-state *feel* is a user judgement; the hover *ratios* in §4 are computed, not felt.

---

## 12. Acceptance criteria

Mechanically checkable unless marked. `[UC]` = **USER CHECKPOINT — stays pending; the orchestrator
must not self-declare it.**

**AC1** — The 17 Bucket A declarations resolve to `color: var(--text-1)`; four of them via a new
`color` on the base rules `.pr-mergeable`, `.asset-badge`, `.danger-badge`, `.asset-issue` with the
per-variant `color` deleted.

**AC2** — Measured in the harness with the §2 method, both themes: every Bucket A site ≥ **9.0:1**
dark and ≥ **11.6:1** light on its own tint, in default, hover and selected states.

**AC3** — `.error-dismiss` is `color: var(--text-1)`; its `:hover` background is
`color-mix(in srgb, currentcolor 12%, transparent)`; the `color-mix(… var(--danger) 20% …)` hover fill
is gone. Measured ≥ **8.4:1** dark / ≥ **10.3:1** light on the hovered wash.

**AC4 — predicted post-fix grep residue** (baselines measured 2026-09-02, before the fix):

| Grep (over `src/styles`) | Before | **Predicted after** |
|---|---|---|
| `color:\s*var\(--danger\);` | 35 | **27** |
| `color:\s*var\(--success\);` | 12 | **9** |
| `color:\s*var\(--warning\);` | 24 | **17** |
| `color:\s*var\(--text-1\);` | 159 | **171** |
| `--h:\s*var\(` | 15 | **15** (unchanged — R1 deliberately does not adopt `--h`) |
| `color:\s*(#\|white)` (case-insens.) | 0 | **0** |
| files changed under `src/` | — | **9**, all `src/styles/*.css` |
| files changed outside `src/styles/` | — | **0** |

A deviation in any row means the implementation diverged from §5 — investigate before accepting, do
not adjust the number.

**AC5** — The 19 Bucket B keeps are byte-identical: `.toast-glyph`, `.submodule-badge-glyph`,
`.ai-dock-status-glyph`, `.git-run-pill-glyph`, `.checks-rollup-pill--{good,warn,pending}
.checks-rollup-glyph`, `.graph-filter-chip-stale .graph-filter-chip-glyph`, `.dev-mode-pill-glyph`,
`.forge-reauth-icon`, `.ai-dock-ask-guard-glyph`, `.conflict-action-ai:hover`. `checks-panel.css`,
`toasts-and-overlays.css`, `git-dock.css`, `ai-dock.css`, `sidebar.css`, `graph-filter.css`,
`controls.css` do not appear in the diff at all.

**AC6** — `src/styles/tokens-and-base.css` is not in the diff. Zero new tokens.

**AC7** — No layout shift: no `border`, `border-width` or `padding` declaration is added or changed by
this increment; every new hue edge is `box-shadow: inset`. `.danger-badge` variants keep their
existing `border-color` and gain no `box-shadow`.

**AC8** — No `.tsx` file is in the diff.

**AC9** — `src/styles/status-panel.css` is not in the diff (P106 owns it, §8).

**AC10** — `ui-reference.md` §2 is updated in the same pass: the 16-row table is annotated as
superseded by this file, the "35% border … at the 3:1 graphics bar" sentence is corrected per §7, and
the `--warning` letterform measurements from §6 are recorded.

**AC11** `[UC]` — Native window (`pnpm tauri dev`), both themes: the fixed banners, badges and pills
read as the same components, not as a new visual family; the leading bars line up with the toast and
`.dev-warning-bar` precedent; nothing looks washed out at 100% and 150% display scaling.

**AC12** `[UC]` — The surfaces with no confirmed mock path (`ProposedOpDialog` A6–A8, worktree copy
dialog A16, `ErrorBoundary` A17) are visually confirmed in the native window, both themes.

**AC13** `[UC]` — Hover feel on `.error-dismiss` and the PR rows is unchanged (the harness is headless;
`requestAnimationFrame` does not fire, so this cannot be judged from the browser).

---

## 13. Flagged for the orchestrator

1. **Sequencing / file conflict.** P107's implementation edits 9 files under `src/styles/`, which
   another agent is currently working in. Land this only when that work is committed, and keep
   `status-panel.css` out of the diff (P106, §8).
2. **P106 first.** It fails at rest and its family has no word beside the letter; P107's sites all
   fail in light and all have a word. Recommendation: P106, then P107. They are independent.
3. **`--warning-text`: my answer is NO** (§6, with measurements). If the orchestrator wants the
   `--*-text` family completed for symmetry, that is a separate cosmetic NIT with no pixel change.
4. **§7 is a defect in `ui-reference.md`, not in the code.** No shipped surface relies on a 35% border
   for meaning today — the correction is preventative. I am making the §2 edit in this pass; flagging
   it because it retracts guidance a prior contract may have been written against.
5. **P108 proposed** (§9) — hue-as-text over neutral surfaces, including P91 §8.4's own
   `.dev-status-write-failed .dev-status-state` at 4.41 dark. Not enumerated; not claimed closed.
