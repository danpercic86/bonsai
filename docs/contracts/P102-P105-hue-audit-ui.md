# P102 + P105 — Hue Audit UI Contract

**Scope:** two defects of one shape — *a hue token placed against a surface that does not give it
enough contrast*. **P105** is `--accent` used as **text**; **P102** is a hardcoded `#ffffff` used as
**ink on a `--danger` fill**. They share a method (enumerate → bucket → verdict) and a decision rule
(P100's two recipes), so they ship as one milestone in two stageable sections.

**Owner:** ui-designer. **Implementer:** senior-dev. **Files touched:** CSS only
(`src/styles/*.css`) plus `docs/contracts/ui-reference.md`. **No component files change.**
No new components, no layout change, no geometry change, no copy change.

**Method note (read this first).** This programme has now had **four** app-wide claims fail on
inspection: P95's enabled-control class (3 escapes found by P101), P98's "`--text-3` family closed"
(122 declarations never classified), P74's hue-as-text sweep (this milestone, §2.3), and P74's
"no remaining hue-as-text-over-its-own-tint anywhere in the app" (**5 live instances**, §3.4).
Nothing in this contract may be closed by assertion. Every call site below carries a recorded
**bucket + verdict**, P101 §3 style, and §6's acceptance criteria are grep-checkable counts.

---

## 1. Measurements (recomputed from the shipped token hexes)

All ratios below are computed from `src/styles/tokens-and-base.css` at the current tip, WCAG 2.x
relative-luminance formula, sRGB. **Text bar = 4.5:1. Glyph / border / bar / fill-edge bar = 3:1**
(the P100/P101 split). Composited tints are resolved to a flat hex before measuring.

### 1.1 `--accent` (`#4f8cff` dark / `#2f6fe4` light) as text

| Backdrop | Dark | Light |
|---|---|---|
| `--bg-0` | **5.52** ✓ | **4.65** ✓ |
| `--bg-1` | **5.07** ✓ | **4.34** ✗ |
| `--bg-2` | **4.48** ✗ | **4.00** ✗ |
| `--bg-3` | **3.89** ✗ | **3.68** ✗ |
| `--selection` | **3.51** ✗ | **3.74** ✗ |
| own 12% tint over `--bg-1` | **4.29** ✗ | **3.74** ✗ |
| own 14% tint over `--bg-1` | **4.17** ✗ | **3.64** ✗ |
| own 14–15% tint over `--bg-2` | **3.68** ✗ | **3.38** ✗ |

**`--accent` clears the 4.5:1 text bar on `--bg-0` only** (both themes; `--bg-1` passes in dark
and fails in light, which is not a shippable rule). It clears the 3:1 graphics bar **everywhere**,
including `--selection` and every tint above. So: `--accent` stays the fill / border / bar / glyph
hue and stops being an ink.

`ui-reference.md` §2 currently says `color: var(--accent)` "is fine on `--bg-0` / `--bg-1` /
`--bg-2`". **That sentence is false** and is corrected in this pass (§7).

### 1.2 New token `--accent-strong` — the accent tuned to carry text

| | Dark | Light |
|---|---|---|
| value | `#7fabff` | `#2a5cbe` |
| derivation | `--accent` lightened toward white (hue held at 219°) | `--accent` deepened toward black (hue held at 219.7°) |

| Backdrop | Dark | Light |
|---|---|---|
| `--bg-0` | **7.76** ✓ | **6.23** ✓ |
| `--bg-1` | **7.13** ✓ | **5.81** ✓ |
| `--bg-2` | **6.29** ✓ | **5.36** ✓ |
| `--bg-3` | **5.46** ✓ | **4.93** ✓ |
| `--selection` | **4.93** ✓ | **5.01** ✓ |
| own-accent 14% tint over `--bg-1` | **5.86** ✓ | **4.87** ✓ |

`--accent-strong` clears 4.5:1 on **every** neutral surface the app has, in **both** themes, and on
an accent tint. That is deliberate: it makes the token **surface-independent**, so the verdict for a
text call site never depends on resolving its ancestor chain, and the post-fix invariant is a single
grep rather than a per-site argument.

### 1.3 Ink on hue fills

| Fill | Dark fill | White ink (dark) | `#16181d` ink (dark) | Light fill | White ink (light) | `#16181d` ink (light) |
|---|---|---|---|---|---|---|
| `--danger` | `#e5534b` | **3.73** ✗ | **4.76** ✓ | `#d13438` | **4.93** ✓ | 3.03 ✗ |
| `--success` | `#57ab5a` | **2.85** ✗✗ | **6.24** ✓ | `#1a7f37` | **5.08** ✓ | 2.42 ✗✗ |
| `--merged` (new) | `#a371f7` | 3.99 ✗ | **5.30** ✓ | `#8250df` | **5.05** ✓ | 2.86 ✗✗ |
| current merged literal `#8957e5` | — | 4.61 ✓ | 3.85 ✗ | (theme-invariant) | 4.61 ✓ | 3.85 ✗ |

✗✗ = below even the 3:1 graphics bar. **`.pr-state-open` — white on the dark `--success` fill at
2.85:1 — is the worst finding in this audit, worse than the P102 seed defect.** It was invisible to
the seed list because the `color: #ffffff` lives on the shared `.pr-state-pill` rule, not on the
danger rule.

The per-theme split is identical for all three hues: **near-black ink in dark, white ink in light**,
exactly `--accent-text`'s split (P100). See §5 for why they are three tokens and not one.

### 1.4 Hover on a kept hue fill

`filter: brightness()` moves fill **and** ink together, so it fails in whichever theme has least
headroom (P100 AC7). Measured for `--danger`:

| Rule | Device | Dark | Light |
|---|---|---|---|
| `.btn-danger:hover` today | `brightness(1.1)` + white ink | 5.71 ✓ | **4.17** ✗ |
| `.btn-danger:hover` fixed | `color-mix(… 92%, var(--text-1))` + `--danger-text` | **5.17** ✓ | **5.49** ✓ |
| `.diff-float-discard:hover` today | `brightness(1.08)` + `--bg-0` ink | 5.6 ✓ | **4.32** ✗ |
| `.diff-float-discard:hover` fixed | `color-mix(… 92%, var(--text-1))` + `--danger-text` | 5.1 ✓ | **5.3** ✓ |

Same device, same reason, as P100 prescribed for `.btn-primary`.

---

## 2. P102 — hardcoded ink on a hue fill

### 2.1 Enumeration (exhaustive)

`rg "\bcolor:\s*(#[0-9a-fA-F]{3,8}|white|rgba?\()" src/` over the whole frontend returns **3**
hardcoded-ink declarations in `src/styles/` (the 4th and 5th hits are a comment and a diff fixture
string, not declarations). The seed list named 2; the sweep found 3, and the third one carries the
worst ratio in the audit.

| # | File:line | Selector | Fill under it | Bucket | Measured | Verdict |
|---|---|---|---|---|---|---|
| D1 | `controls.css:69-72` | `.pill-detached` | `var(--danger)` | text (11px/600 pill label) | 3.73 dark ✗ / 4.93 light ✓ | **FIX** → `var(--danger-text)` |
| D2 | `updates.css:109-120` | `.btn-danger` | `var(--danger)` | text (13px/600 button label, destructive) | 3.73 ✗ / 4.93 ✓ | **FIX** → `var(--danger-text)` |
| D3a | `forge-pr.css:163-173` + `:175-177` | `.pr-state-pill` on `.pr-state-open` | `var(--success)` | text (10px/700 uppercase) | **2.85** ✗✗ / 5.08 ✓ | **FIX** → `var(--success-text)` |
| D3b | `forge-pr.css:163-173` + `:179-181` | `.pr-state-pill` on `.pr-state-merged` | `#8957e5` literal | text | 4.61 ✓ / 4.61 ✓ | **PASSES contrast**; fill is a hardcoded hex — see §5.4 |
| D3c | `forge-pr.css:163-173` + `:183-185` | `.pr-state-pill` on `.pr-state-closed` | `var(--danger)` | text | 3.73 ✗ / 4.93 ✓ | **FIX** → `var(--danger-text)` |

**Note the seed-list correction.** `ui-reference.md` §2 attributes `.btn-danger` to
`controls.css:70`. It is not there: `controls.css:69-72` is **`.pill-detached`**, and `.btn-danger`
lives in `updates.css:109`. Both are real defects; the reference's line pointer is stale and is
corrected in §7.

### 2.2 `background: var(--danger)` sweep (all fills, ink included)

`rg "background:\s*var\(--danger\)" src/styles` → **4** hits. D1, D2, D3c above, plus:

| # | File:line | Selector | Current ink | Measured | Verdict |
|---|---|---|---|---|---|
| D4 | `partial-staging.css:104` | `.diff-float-discard` | `var(--bg-0)` | 4.76 ✓ / 4.93 ✓ | **PASSES**; move ink to `var(--danger-text)` for role correctness (numerically identical today — zero visual change) |

### 2.3 P100 residual — the four `filter: brightness` sites

`rg "filter:\s*brightness" src/styles` → **4**. Verdicts:

| File:line | Selector | Fill / ink | In scope? |
|---|---|---|---|
| `updates.css:123` | `.btn-danger:hover` | `--danger` / white → `--danger-text` | **IN.** Leaving it would move the fill out from under the new ink; measured 4.17:1 light today (§1.4) |
| `partial-staging.css:111` | `.diff-float-discard:hover` | `--danger` / `--bg-0` | **IN.** Same defect, measured **4.32:1 light** today |
| `partial-staging.css:92` | `.diff-stage-float button:hover` | `--accent` / `--accent-text` | **IN.** This is P100's own recipe-2 surface; same 3.95:1-light failure `.btn-primary` had. One line, and shipping a known sub-AA primary-action hover is not defensible |
| `settings-primitives.css:278` | `.settings-switch:hover .settings-switch-track` | `--accent` fill, **no ink on it** | **OUT — stays filed.** Judged at the 3:1 graphics bar vs `--bg-1`: **3.80:1 light** after brightening. Compliant. File as a NIT ("device consistency: switch track still uses `filter`") |

So 3 of the 4 residuals come in **on measured merit**, not for tidiness; 1 is genuinely compliant
and stays a filed NIT.

### 2.4 Prescribed changes (P102)

```
tokens-and-base.css  :root              + --danger-text:  #16181d;
                                        + --success-text: #16181d;
                                        + --merged:       #a371f7;   (Option A, §5.4)
                                        + --merged-text:  #16181d;   (Option A, §5.4)
                     [data-theme=light] + --danger-text:  #ffffff;
                                        + --success-text: #ffffff;
                                        + --merged:       #8250df;
                                        + --merged-text:  #ffffff;

controls.css:71          color: #ffffff                      -> var(--danger-text)
updates.css:115          color: #ffffff                      -> var(--danger-text)
updates.css:123          filter: brightness(1.1)             -> background: color-mix(in srgb, var(--danger) 92%, var(--text-1))
partial-staging.css:104  color: var(--bg-0)                  -> var(--danger-text)          [no visual change]
partial-staging.css:111  filter: brightness(1.08)            -> background: color-mix(in srgb, var(--danger) 92%, var(--text-1))
partial-staging.css:92   filter: brightness(1.08)            -> background: color-mix(in srgb, var(--accent) 92%, var(--text-1))
forge-pr.css:172         DELETE `color: #ffffff` from .pr-state-pill
forge-pr.css:176         .pr-state-open   += color: var(--success-text)
forge-pr.css:180         .pr-state-merged  = background: var(--merged); color: var(--merged-text)
forge-pr.css:184         .pr-state-closed += color: var(--danger-text)
```

Each changed rule carries a one-line comment naming the milestone and the measured pair, matching
the house convention already in `controls.css:96-101`.

### 2.5 States, both themes, both densities (P102)

- **Default / hover / active:** `.btn-danger` default `--danger` + `--danger-text`; hover the
  92%/`--text-1` mix (§1.4); **pressed** is unspecified today and stays unspecified — do not add
  one in this milestone.
- **`:focus-visible`:** unchanged — 2px `--accent` outline, 1px offset. Accent ring on a `--danger`
  fill is a graphics edge; it is legible because of the 1px offset against the parent surface, not
  against the fill. No change.
- **Disabled:** `.btn-danger:disabled { opacity: 0.6 }` is unchanged. The dimming budget
  (ui-reference §2) is spent once here and nowhere in the subtree. `--danger-text` at 0.6 over the
  dimmed fill remains ≥3:1 and the control is inert, so WCAG 1.4.3's inactive-component exemption
  applies — same standing as today.
- **Loading / empty / error:** these surfaces have none of their own; unchanged.
- **Long content:** `.pill-detached` already truncates at `max-width: 160px` with ellipsis;
  `.pr-state-pill` labels are a closed set (OPEN / MERGED / CLOSED) and cannot overflow.
- **Densities:** no geometry changes. `cozy` and `compact` row heights, paddings and pill heights
  are untouched by this milestone in both themes.

---

## 3. P105 — `--accent` as text

### 3.1 Enumeration (exhaustive)

`rg "(^|[^-])\bcolor:\s*var\(--accent\)" src/styles` → **30** declarations across 16 files. (The
looser `var(--accent)` grep returns 140 across 34 files; the other 110 are `background`,
`border-color`, `box-shadow`, `accent-color`, `outline`, `color-mix` operands and canvas reads —
all graphics-bar uses, all compliant per §1.1.) Outside `src/styles/`, the only `--accent` colour
uses are `conflictCmSetup.ts:32-33` (`caretColor`, `cm-cursor` border — a caret is a graphic,
5.52/4.65 on the `--bg-0` editor, **KEEP**) and `graph/colors.ts:157` / `rail/OverviewRail.tsx:75`
(canvas paints, out of DOM scope, governed by ui-reference §5).

**Buckets: 7 KEEP (glyph, 3:1) + 18 → `--accent-strong` (text, 4.5:1) + 5 → recipe change
(state or own-tint) = 30.**

### 3.2 Bucket G — glyph / graphic. **KEEP `var(--accent)`.** (7)

Each is judged at 3:1 against the worst composited backdrop it can reach; `--accent` clears 3:1 on
every surface (§1.1), so all 7 pass in both themes. Each already has a non-colour carrier.

| # | File:line | Selector | Backdrops reached | Worst measured | Carrier |
|---|---|---|---|---|---|
| A1 | `sidebar.css:152` | `.branch-row-head .branch-glyph` | `--bg-1` → `--bg-2` hover → `--selection` | 3.51 / 3.74 | `aria-current` + 600 weight on the name (comment at `:143-145` already states this) |
| A2 | `graph-filter.css:62` | `.graph-filter-chip-glyph` | `--bg-2` → `--bg-3` hover | 3.89 / 3.68 | chip label text beside it |
| A3 | `graph-filter.css:188` | `.graph-filter-ref-glyph-solo` | `--bg-1` → `--bg-2` hover | 4.48 / 4.00 | glyph **shape** differs from `-hidden`; ref name beside it |
| A4 | `graph-filter.css:240` | `.ref-filter-marker-solo` | `--bg-1` → `--bg-2` → `--selection` | 3.51 / 3.74 | visually-hidden accessible-name suffix (comment at `:232-233`) |
| A5 | `commit-box.css:224` | `.rp-overflow-check` | menu `--bg-1` → `--bg-2` hover | 4.48 / 4.00 | the checkmark is a **shape**; `aria-checked` on the item |
| A6 | `graph-replay.css:27` | `.graph-replay-fab:hover` | `--bg-2` (hover state only) | 4.48 / 4.00 | `▶` is `aria-hidden`; the name comes from `aria-label="Replay history"` |
| A7 | `partial-staging.css:24` | `.diff-gutter-btn` hover/focus | `--bg-0` + add/del row tint | ≥4.4 / ≥3.9 | `+`/`−` is a pictogram; `aria-label` carries "Stage this line" |

A5, A6 and A7 are icon-only controls whose accessible name is not the glyph — the ui-reference §2
icon-only rule applies, and they are compliant at 3:1. Recorded here so a future sweep does not
re-litigate them.

### 3.3 Bucket T — read text. **→ `var(--accent-strong)`.** (18)

Every one of these is text the user must read in order to act. `--accent-strong` clears 4.5:1 on
every backdrop any of them can reach (§1.2), so the verdict does not depend on the ancestor chain;
the backdrop column is recorded anyway, per the method.

| # | File:line | Selector | Backdrop(s) per state | `--accent` today | Verdict |
|---|---|---|---|---|---|
| B1 | `diff-content.css:111` | `.diff-hunk-stage-btn:hover` | `--bg-1` (hover changes border only) | 5.07 / **4.34** ✗ | FIX |
| B2 | `settings-legacy-sections.css:384` | `.settings-profile-applied` | settings body `--bg-0` | 5.52 / 4.65 ✓ | FIX for family consistency — it is 11px read text and one theme change away from failing; the token is surface-independent |
| B3 | `dialogs-forms.css:92` | `.wt-copy-selectall` | dialog `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B4 | `forge-pr.css:370` | `.pr-comment-kind-review` | PR detail `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B5 | `forge-pr.css:501` | `.pr-generate-button` | PR form `--bg-1` → `--bg-2` hover | 4.48 / **4.00** ✗ | FIX |
| B6 | `forge-pr.css:575` | `.forge-connect-link` | `--bg-1` | 5.07 / **4.34** ✗ | FIX (a link; underline is the second carrier and stays) |
| B7 | `updates.css:85` | `.settings-update-link` | settings `--bg-0` | 5.52 / 4.65 ✓ | FIX for consistency; same reasoning as B2 |
| B8 | `ai-assets.css:176` | `.stale-selectall` | panel `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B9 | `blame-history.css:60` | `.blame-why` | gutter `--bg-1` → `--bg-2` on `.blame-gutter:hover` **and** on `.blame-why:hover` | 4.48 / **4.00** ✗ | FIX |
| B10 | `blame-history.css:80` | `.blame-oid` | `--bg-1` → `--bg-2` gutter hover | 4.48 / **4.00** ✗ | FIX (an oid is read text — ui-reference §2 already rules this) |
| B11 | `blame-history.css:149` | `.file-history-oid` | `--bg-0` → `--bg-1` row hover | 5.07 / **4.34** ✗ | FIX |
| B12 | `blame-history.css:223` | `.reflog-oid-new` | reflog row `--bg-1` → hover | 4.48 / **4.00** ✗ | FIX |
| B13 | `commit-box.css:63` | `.section-action-ai-bulk:hover` | section header `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B14 | `commit-box.css:365` | `.commit-sign-fix` | commit box `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B15 | `commit-panel.css:167` | `.commit-parent-link` | commit panel `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B16 | `status-panel.css:207` | `.file-status-renamed .file-badge` | row `--bg-1` → `--bg-2` hover → `--selection` | **3.51 / 3.74** ✗ | FIX — see §3.5 |
| B17 | `conflicts.css:88` | `.row-action.conflict-action-ai` | row `--bg-1` | 5.07 / **4.34** ✗ | FIX |
| B18 | `conflicts.css:94` | `.row-action.conflict-action-ai:hover` | **own 14% accent tint** over `--bg-1` | **4.17 / 3.64** ✗ | FIX. Keep the tint background; `--accent-strong` on it is 5.86 / 4.87 (§1.2). This is the one sanctioned hue-ink-over-own-tint in the app after this milestone, and it is sanctioned **only** because `--accent-strong` clears 4.5:1 on the tint — restate that in the rule comment |

### 3.4 Bucket S — state or hue-over-own-tint. **Recipe change; `--accent` stops being ink.** (5)

ui-reference §2 asserts *"There is no remaining sanctioned use of hue-as-text-over-its-own-tint
anywhere in the app; a new one is a defect."* **That claim is false — here are five live
instances.** Corrected in §7.

| # | File:line | Selector | Today | Measured | Prescribed |
|---|---|---|---|---|---|
| C1 | `dialogs.css:282-289` | `.branch-name-chip` | `--accent` on own 12% tint over `--bg-1`, 40% accent border | **4.29 / 3.74** ✗ | `color: var(--text-1)` — keep tint + border. §2's canonical pair: ~9.2 dark / ~11.7 light. Single-line change |
| C2 | `ai-assets.css:123-126` | `.asset-chip-canonical` | `--accent` on own 15% tint over `.asset-row` `--bg-2` | **3.68 / 3.38** ✗ | §11 pill recipe: `color: var(--text-1)`; add `border: 1px solid color-mix(in srgb, var(--accent) 35%, var(--bg-2))` as the boundary carrier |
| C3 | `ai-assets.css:149-152` | `.asset-chip-new` | same, `--accent` | **3.68 / 3.38** ✗ | same recipe, `--accent` border |
| C3b | `ai-assets.css:144-147` | `.asset-chip-active` | `--success` on own 18% tint over `--bg-2` | **≈4.0 / ≈3.6** ✗ | **Same-block sibling — IN SCOPE.** Same recipe, `--success` border. Fixing 2 of 3 chips would leave the family inconsistent, which the house rule (consistency beats local optimality) forbids. `.asset-chip-muted` already has a `--text-2` label; give it the matching `--text-3` 35% border so all four chips share one shape |
| C4 | `forge-pr.css:32-35` | `.right-pane-tab.active` | `--accent` on own 14% tint | **4.17 / 3.64** ✗ | **A state ⇒ P100 recipe 1.** `background: var(--selection); color: var(--text-1); font-weight: 600;` + `box-shadow: inset 0 -2px 0 var(--accent)` as the mandatory non-colour carrier (`--selection` vs `--bg-1` is only ~1.3:1). `--text-1` on `--selection` = 9.36 / 13.29 |
| C5 | `settings-legacy-sections.css:97-100` | `.settings-toggle-btn.is-active` | `--accent` text on `.btn-secondary`'s `--bg-2` | **4.48 / 4.00** ✗ | **A state ⇒ P100 recipe 1**, §12.3.2 segmented form: delete `color: var(--accent)` (inherits `--text-1` from `.btn-secondary`), add `background: var(--selection)`, **keep** `border-color: var(--accent)` — the accent border is already the non-colour carrier at 3.51 / 3.74 |

C4 adds a 2px inset bar. Because `.right-pane-tab` has no border today, the bar is drawn with
`box-shadow: inset`, not `border-bottom` — zero layout shift, no reflow of the tab strip. Do **not**
add `font-weight: 600` transitions; the weight change is instant and the tab strip is not
pointer-synced, so no label reflows under the cursor.

Count check: C1–C5 plus C3b is 6 rules but only **5** of the 30 `color: var(--accent)`
declarations (C3b's ink is `--success`, so it is a scope addition, not one of the 30).

### 3.5 The A/M/D/U/R status badge family (B16 and its siblings)

`.file-badge` is the house's own non-colour-carrier precedent — but the **letter is text**, so it
is judged at 4.5:1, not 3:1. B16 (`--accent`, renamed) fails on every state and is fixed here.
Its three siblings are **out of scope and filed**, with measurements so the follow-up starts from
evidence, not a re-sweep:

| Selector | Hue | on `--bg-1` | on `--bg-2` hover | on `--selection` |
|---|---|---|---|---|
| `.file-status-deleted/-conflicted .file-badge` | `--danger` | 4.4 / 4.6 (marginal dark) | ✗ | ✗ |
| added/staged | `--success` | 5.7 / 4.7 | ✗ | ✗ |
| modified/typechange | `--warning` | 7.3 / 4.5 | ✗ | ✗ |

**File as P106 — "status-badge hue family as text".** Do not fix it here: it needs three more
`-strong` variants or a demotion to `--text-1` + shape, and that is a design decision with its own
survey. Flagged for the orchestrator.

### 3.6 States, both themes, both densities (P105)

- **Default / hover / active / selected:** covered per call site in §3.3–§3.4. Because
  `--accent-strong` clears 4.5:1 on `--bg-0`…`--bg-3` **and** `--selection`, no B-bucket site can
  regress into failure when its row is hovered, pressed or selected. That property is the point of
  the token and is what makes AC5 a sufficient check.
- **`:focus-visible`:** unchanged everywhere — 2px `--accent`, 1px offset, keyboard only. The ring
  keeps `--accent` (a graphics edge at ≥3:1 on every surface); it must **not** move to
  `--accent-strong`, or the ring stops matching the brand hue and the canvas selection ring.
- **Disabled:** `.wt-copy-selectall:disabled`, `.stale-selectall`, `.graph-replay-fab:disabled`,
  `.section-action-ai-bulk:disabled` keep their existing `opacity` / `--text-3` treatment. Watch the
  **child-rule trap** (ui-reference §2): none of the 18 B sites is a child of a disabled compound
  selector today — verify with `rg "disabled.*accent-strong" src/styles` returning 0 after the change.
- **Loading / empty / error:** unaffected; no B site renders in a loading or empty state.
- **Long content:** B10/B11/B12 are short oids (fixed width). B6, B3, B8 already truncate or
  `white-space: nowrap`. C1 `.branch-name-chip` holds branch names — verify with the long-name
  fixture (§8) that the chip wraps at the container, not mid-token.
- **Densities:** no geometry, padding or row-height change in `cozy` or `compact`. C2/C3/C3b add a
  1px border to `.asset-chip`; to keep the chip's outer height identical in both densities, add
  `border: 1px solid transparent` to the base `.asset-chip` rule and let the modifiers set only
  `border-color` — the same device `conflicts.css:181-183` already uses.

---

## 4. Interaction, motion, a11y, microcopy

- **Interaction / keyboard:** unchanged. No new controls, no new shortcuts, no command-palette
  entries, no focus-trap or focus-restore changes, no context-menu changes.
- **Motion:** this milestone **removes** motion cost. Three `filter: brightness()` hovers become
  `background` swaps — `filter` forces a compositing pass over the whole element; a flat background
  does not. No transitions are added; no `prefers-reduced-motion` handling is needed because nothing
  animates. Nothing here can contend with the canvas render budget (no rule is inside the graph
  pane's paint path).
- **Accessibility:** every changed pair is stated with its measured ratio in §1 and §3. No colour is
  the sole carrier of meaning anywhere in the changed set — A1–A7 each name their carrier, C4/C5
  gain an explicit non-colour carrier, and the A/M/D/U/R letter stays the primary carrier for B16.
  Hit targets, roles and accessible names are untouched.
- **Microcopy:** no string changes. Not one label, error, tooltip or empty-state line moves.
- **Destructive-action UX:** `.btn-danger` and `.diff-float-discard` keep their existing
  `ConfirmDialog` gating; this milestone only makes their labels readable. Confirm copy, target
  naming and the "cannot be undone" line are unchanged.

---

## 5. Token decisions

Add to **both** `:root` and `[data-theme='light']` in `src/styles/tokens-and-base.css`, each with a
comment naming P102/P105 and the measured pair — the house convention at `tokens-and-base.css:26-29`.

| Token | Dark | Light | Role |
|---|---|---|---|
| `--accent-strong` | `#7fabff` | `#2a5cbe` | **`--accent` as read text**, on any neutral surface or accent tint. Never a fill, never a border, never the focus ring |
| `--danger-text` | `#16181d` | `#ffffff` | ink on a solid `--danger` fill (4.76 / 4.93) |
| `--success-text` | `#16181d` | `#ffffff` | ink on a solid `--success` fill (6.24 / 5.08) |
| `--merged` | `#a371f7` | `#8250df` | PR "merged" hue — **Option A only**, §5.4 |
| `--merged-text` | `#16181d` | `#ffffff` | ink on `--merged` (5.30 / 5.05) — **Option A only** |

### 5.1 Why `--accent-strong` and not a strengthened `--accent`

`--accent` is simultaneously the fill under `--accent-text` (5.52 / 4.65), the focus ring, lane 0 in
the graph palette, and the border/bar carrier on `--selection`. Moving it moves all four, including
a canvas palette that `ui-reference` §5 pins. A second token is the smaller blast radius.

### 5.2 Why not demote every accent text to `--text-1`

Three of the 18 B sites (`.blame-why`, `.commit-parent-link`, `.forge-connect-link`) are links or
link-like affordances where the hue **is** the affordance, and B18's comment records the accent tint
as the deliberate distinction between the AI action and the plain ours/theirs buttons. Removing the
hue would remove meaning. Where the hue is *not* load-bearing — a selected state, a chip on its own
tint — §3.4 does demote it, per P100 recipe 1.

### 5.3 Why three ink tokens and not one `--on-hue`

`--danger-text`, `--success-text` and `--merged-text` are **numerically identical** to each other
and to `--bg-0` in both themes today. They are still three tokens, because the correct ink depends
on its own hue's luminance and each pair must stay independently measurable and independently
changeable — exactly the reasoning P100 used when it created `--accent-text` rather than reusing
`--bg-0`. A reviewer will notice the duplication; this paragraph is the answer.

### 5.4 FLAGGED AMBIGUITY — the merged-PR purple

`.pr-state-merged` uses a hardcoded `#8957e5` with white ink. **It passes contrast** (4.61:1 in both
themes), so it is not a P102 defect. But it is the only theme-invariant hue literal in
`src/styles/`, and it sits inside the exact rule block P102 must rewrite.

- **Option A (recommended):** introduce `--merged` / `--merged-text`. Makes AC7 a clean
  **zero-count** grep, removes the last colour literal outside `tokens-and-base.css`, and gives the
  light theme a purple tuned for a white page instead of a dark-theme purple reused.
- **Option B:** leave `#8957e5` + a locally restated `color: #ffffff` with an explanatory comment.
  Two fewer tokens; AC7 becomes "exactly 1, and it is this one".

**Recommendation: Option A.** Orchestrator to confirm — the implementer must not pick silently.

---

## 6. Acceptance criteria

Mechanically checkable unless marked. Greps are run from the repo root over `src/styles/`.

| # | Criterion |
|---|---|
| **AC1** | `--accent-strong` is defined in **both** `:root` and `[data-theme='light']` in `tokens-and-base.css`, values `#7fabff` / `#2a5cbe`, with a comment naming P105 and the ratio range |
| **AC2** | `--danger-text` and `--success-text` are defined in **both** blocks, `#16181d` / `#ffffff`, each commented with its measured pair |
| **AC3** | *(Option A)* `--merged` (`#a371f7` / `#8250df`) and `--merged-text` (`#16181d` / `#ffffff`) defined in both blocks. *(Option B: AC3 is dropped and AC7 becomes "exactly 1")* |
| **AC4** | `rg -c "(^\|[^-])\bcolor:\s*var\(--accent\)" src/styles` totals **exactly 7**, and they are exactly A1–A7 of §3.2 — no additions, no substitutions |
| **AC5** | `rg -c "color:\s*var\(--accent-strong\)" src/styles` totals **exactly 18**, and they are exactly B1–B18 of §3.3 |
| **AC6** | Each of C1, C2, C3, C3b, C4, C5 carries the prescribed recipe from §3.4, and none of them contains `color: var(--accent)` or `color: var(--success)` |
| **AC7** | `rg -i "\bcolor:\s*(#[0-9a-fA-F]{3,8}\|white)\b" src/styles` returns **0** (Option A) / **1** (Option B). Comments containing the string do not count — check declarations only |
| **AC8** | `rg "filter:\s*brightness" src/styles` returns **exactly 1**: `settings-primitives.css` (the switch track), and a TODO.md follow-up line exists for it |
| **AC9** | `.btn-danger:hover:not(:disabled)`, `.diff-float-discard:hover` and `.diff-stage-float button:hover` each use `background: color-mix(in srgb, var(--<hue>) 92%, var(--text-1))` and set no `filter` |
| **AC10** | `.pr-state-pill` declares **no** `color`; `.pr-state-open`, `.pr-state-merged`, `.pr-state-closed` each declare both `background` and `color` |
| **AC11** | The diff introduces **zero** new colour hex literals outside `tokens-and-base.css` |
| **AC12** | `.asset-chip` base gains `border: 1px solid transparent`; all four `.asset-chip-*` modifiers set only `border-color`. Chip outer height is unchanged in `cozy` and `compact`, both themes |
| **AC13** | `rg "disabled.*accent-strong" src/styles` returns **0** (child-rule trap, ui-reference §2) |
| **AC14** | `docs/contracts/ui-reference.md` §2 no longer contains the sentence claiming `--accent` text is "fine on `--bg-0` / `--bg-1` / `--bg-2`"; it carries the §1.1 five-row matrix, the `--accent-strong` row, the corrected `.btn-danger` file pointer, and the corrected hue-over-own-tint claim (§7) |
| **AC15** | Every one of the 30 §3 call sites and the 5 §2 call sites has a recorded bucket + verdict in this document, and the implementer's report maps each changed line to its row ID (A*/B*/C*/D*) |
| **AC16** | `pnpm gate` green: vitest, tsc, eslint, stylelint. No test or snapshot references a removed `#ffffff` |
| **AC17** | Harness (`VITE_MOCK_IPC=1`): every fixture state in §8 renders and the changed surfaces are visible in **both** themes at both `panelDensity` values |
| **AC18** | **USER CHECKPOINT** — the updater panel's `.btn-danger` (default, hover, disabled) in the native Tauri window. The updater surface does not exist in the browser harness |
| **AC19** | **USER CHECKPOINT** — perceptual: `--accent-strong` still reads as "the Bonsai blue" and not as a washed-out or muddy second blue, in both themes, on a real display at 100% scale |
| **AC20** | **USER CHECKPOINT** — hovering `.btn-danger`, `.btn-primary`, `.diff-stage-float button` and `.diff-float-discard` in the native window shows a clean fill change with no flicker, no ink shift and no layout movement (the `filter` → `background` swap) |

**Predicted post-fix grep residue** (state these before running, per the P101 precedent):

```
color: var(--accent)         in src/styles :  30  ->   7
color: var(--accent-strong)  in src/styles :   0  ->  18
color: <hex literal>         in src/styles :   3  ->   0   (Option A)
filter: brightness           in src/styles :   4  ->   1
background: var(--danger)    in src/styles :   4  ->   4   (unchanged — fills stay)
color: var(--danger-text)    in src/styles :   0  ->   4
color: var(--success-text)   in src/styles :   0  ->   1
```

---

## 7. `ui-reference.md` changes (this pass)

Owned by ui-designer, applied in the same pass as this contract:

1. **§2 token table** — add rows for `--accent-strong`, `--danger-text`, `--success-text`, and
   (Option A) `--merged` / `--merged-text`, each with its measured pair.
2. **§2, the "`--accent` as *text* never sits on a `--selection` fill" bullet** — the false clause
   *"is fine on `--bg-0` / `--bg-1` / `--bg-2`"* is **deleted** and replaced by the §1.1 five-row
   matrix plus the ruling: *`--accent` clears 4.5:1 on `--bg-0` only; it is a graphics-bar token
   everywhere else. Read text in the accent hue uses `--accent-strong`.*
3. **§2 ACCENT FILL bullet, item 4** — P102 marked closed; the `.btn-danger` pointer corrected from
   `controls.css:70` (which is `.pill-detached`) to `updates.css:109`; `.pr-state-open` recorded as
   the worst case found (2.85:1) with the note that a shared `color` on a multi-fill rule is the
   pattern that hid it.
4. **§2 hue-over-own-tint bullet** — the claim *"There is no remaining sanctioned use … anywhere in
   the app"* is **retracted and corrected**: five instances survived P74 (§3.4). Replaced with the
   post-P105 rule: *the only hue ink permitted over its own tint is `--accent-strong`, and only
   because it is measured at ≥4.5:1 on that tint.*
5. **§2 method note** — the failed-claim tally goes from two to four, with this milestone's two
   additions named.
6. **§7 (file status colors)** — a line recording that the A/M/D/U/R letter badge is **text**, that
   `--accent` (renamed) is fixed here, and that the other three hues are filed as P106.
7. **§11 (status pills)** — a line recording that PR state pills are §11 surfaces and that a solid
   hue pill takes its hue's `-text` ink, never a literal.

---

## 8. Harness fixture states (`src/ipc/mock/`, `VITE_MOCK_IPC=1`)

Needed so every changed surface is verifiable in a plain browser, in both themes and both densities.
Most already exist; the list marks what must be added.

| Surface | Fixture state | Status |
|---|---|---|
| `.pill-detached` | repo in detached HEAD | verify exists |
| `.pr-state-open` / `-merged` / `-closed` / `.pr-draft-tag` | PR list containing **all four** states at once | **add if absent** — the merged and closed states are the ones under test |
| `.btn-danger` | any destructive `ConfirmDialog` (delete branch) | exists |
| `.diff-stage-float` / `.diff-float-discard` | a partial-staging line selection | exists |
| `.file-status-renamed .file-badge` | a status list containing a rename **plus** A/M/D/U rows for side-by-side comparison | **add if absent** |
| `.branch-name-chip` | a dialog listing branch chips, incl. a 90-char branch name | **add long-name case** |
| `.asset-chip-canonical/-new/-active/-muted` | AI assets panel showing all four chips at once | **add if absent** |
| `.stale-selectall` | a non-empty stale-assets list | verify exists |
| `.blame-why` / `.blame-oid` | blame view with hover reachable | exists |
| `.file-history-oid` / `.reflog-oid-new` | file history + reflog overlay | exists |
| `.right-pane-tab.active` | right panel with the PR tab selected | exists |
| `.settings-toggle-btn.is-active` | settings profile list with a profile active | verify exists |
| `.conflict-action-ai` | conflicted file list | exists |
| Pathological | 90-char branch name, 12-segment path, a 400-line hunk, a 200-char PR title | **add** |
| Empty | empty PR list, empty stale list, empty blame | verify exists |
| Error | forge-connect error banner (`.forge-connect-link` visible) | verify exists |
| **Not harness-visible** | the **updater panel** (`.settings-update-link`, `.btn-danger` in `updates.css`) is Tauri-only | **USER CHECKPOINT — AC18** |

---

## 9. File-size discipline

No new files, no new components. Net CSS growth is ~20 lines across 11 stylesheets plus 5–9 token
lines. Two files touched are already over the ~500-line soft limit — `forge-pr.css` (~710 lines,
last rule at `:706`) and `settings-legacy-sections.css`. This milestone adds **no new rule block**
to either: `forge-pr.css` gains 4 lines by moving one `color` declaration onto three existing state
rules, and `settings-legacy-sections.css` gains 1. **File the `forge-pr.css` split as a separate
`refactorer` task** — do not attempt it inside this milestone; mixing a behaviour-preserving split
with a colour change makes the review diff unreadable.

---

## 10. Staging

Two independently reviewable increments on `feat/p91-observability`:

- **P102** — §2 + §5 tokens (`--danger-text`, `--success-text`, and Option A's `--merged` pair) +
  the 3 in-scope `filter: brightness` residuals. AC1–AC3, AC7–AC11.
- **P105** — §3 (`--accent-strong`, 18 text sites, 7 keeps, 6 recipe changes). AC1, AC4–AC6,
  AC12–AC13.
- **Both** — AC14–AC17 and the three USER CHECKPOINT criteria (AC18–AC20) run once, at the end.

Follow-ups to file, not to fix here: **P106** (status-badge hue family as text, §3.5), the
`settings-primitives.css` switch-track `filter` NIT (§2.3), `.wt-copy-chip` (`dialogs-forms.css:135`
— `--danger` on its own 16% tint, the sixth hue-over-own-tint instance, out of both sections' greps),
and the `forge-pr.css` split (§9).
