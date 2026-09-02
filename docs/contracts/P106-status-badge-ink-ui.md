# P106 — Status-badge ink (the A/M/D/U/R/T/C letter family)

**Owner:** ui-designer · **Date:** 2026-09-03 · **Branch:** `feat/p91-observability`
**Files this contract governs:** `src/styles/status-panel.css` (whole file — 230 lines, read in full),
`src/styles/tokens-and-base.css` (3 token pairs), `src/ipc/fixtures/status.ts` (1 fixture row),
and — only if decision **D1** is taken — `src/components/DiffOverlay.tsx`,
`src/components/ComposerGroupCard.tsx`.
**Inputs:** `docs/contracts/ui-reference.md` §2 / §7 · `docs/contracts/P102-P105-hue-audit-ui.md`
(two-recipe rule) · `docs/contracts/P107-hue-over-own-tint-ui.md` §1 (three-pass search), §4
(per-state resolution), §6 (`--*-text` trap), §8 (the hand-over that created this contract).

P107 does not touch `status-panel.css`. This contract is the only diff on that file.

---

## 0. Result in one screen

- **The search found 8 render sites** (not the 2 the inherited measurements name) across 7 files,
  **7 status classes**, **5 hue ink declarations** and **6 distinct composited backdrops**. Two render
  sites carry **no hue at all** — a class the `.file-status-*` grep structurally cannot see.
- **The letter is the sole non-colour carrier at every one of the 8 sites.** No variant is exempt;
  the 4.5:1 read-text bar applies to all of them (§3).
- **Bucket A (FIX, hue letter below 4.5:1 in at least one live state): 5 declarations** — `--success`
  ×2, `--warning` ×1, `--danger` ×2. Worst measured value **3.05** (`D` on `--selection`, dark).
- **Bucket B (KEEP, already clears the bar in every state): 1** — `.file-status-renamed`
  (`--accent-strong`, min **4.93 / 5.01**), shipped by P105.
- **Bucket C (no hue, compliant by inheritance, inconsistent): 2** — `DiffOverlay.tsx:295`,
  `ComposerGroupCard.tsx:129`. Verified live in the DOM: the overlay header `R` computes
  `rgb(232,234,237)` = `--text-1` on `--bg-0`.
- **Fix: three new ink tokens** — `--danger-strong`, `--success-strong`, `--warning-strong` — exactly
  mirroring `--accent-strong`'s role (read-text form of a hue; never a fill, border or focus ring).
  Post-fix minimum across all statuses, all six live backdrops, both themes: **5.18**.
- **`--warning-text`: NOT needed, and it would be actively wrong here.** Measured, both ways, in §5.

---

## 1. The search that was run (this is a deliverable — restate it in review)

Two prior counts in this programme were wrong because a whole class of instance was invisible to the
search that was run. P107's three passes were run again, scoped to the status-badge family, plus a
fourth pass this surface needs.

### (i) Same rule block — hue `color` and the tint `background` in one selector

```
rg -n "var\(--(danger|success|warning|accent|merged|badge-good|badge-warn|h)\b" src/styles/status-panel.css
```
**Yield: 7 declarations** — `:28` (the 6% `--success` section tint, a *background*), `:193`, `:198`,
`:203`, `:210`, `:216` (badge inks), `:227` (`.section-label-danger`). No selector holds both ink and
tint, so pass (i) alone resolves **no** backdrop. This is why the inherited numbers were partial.

### (ii) Descendant combinator — ink and tint in *different* rules

```
rg -nU --multiline-dotall "\.file-status-[\w-]+\s+\.file-badge\s*\{[^{}]*color:" src/styles
rg -n "file-badge" src/styles src/components
```
**Yield: 5 ink rules** covering **7 status classes** (`added`, `modified`, `typechange`, `deleted`,
`conflicted`, `renamed`, `untracked`), and **8 JSX render sites**. `.file-badge` is styled in exactly
one file (`status-panel.css:75`); no other stylesheet restates its colour. Cross-referencing against
the tint inventory produces the composite P107 handed over: `.file-status-added .file-badge` inside
`.status-section--staged`'s 6% `--success` tint = success ink on a success tint.

### (iii-a) Unqualified child whose parent chain decides everything — **this pass found the new class**

`.file-badge` renders in **8** places; only **6** have a `file-status-*` ancestor. The exhaustive
check is a grep of the *ancestor* class, not the badge class:

```
rg -n "file-status-" src/components   # 6 occurrences (excl. tests), in 5 files
rg -n "className=\"file-badge mono\"" src/components   # 8 occurrences, in 7 files
```
The difference is the finding: **`DiffOverlay.tsx:295` and `ComposerGroupCard.tsx:129` render the
badge with no status class anywhere above them**, so the letter inherits ambient `--text-1`. No
selector-shaped grep over `src/styles` can see this — the CSS for those two instances does not exist.
Confirmed in the running harness, not asserted: with the diff overlay open, the header badge computes
`color: rgb(232,234,237)` (`--text-1`) over `.diff-overlay`'s `rgb(22,24,29)` (`--bg-0`).

### (iii-b) Custom-property indirection (`--h`, `--badge-good`, `--badge-warn`)

```
rg -n -- "--h\b|--badge-good|--badge-warn" src/styles/status-panel.css
```
**Yield: 0.** Recorded so the next pass does not re-run it. The status-badge family does not use hue
indirection; every ink is a literal `var(--hue)`.

### (iii-c) Imperative canvas — **added by P106; run this pass on any visual family from now on**

The commit graph draws badges from Rust layout with no CSS rule behind them, so a CSS-shaped search is
blind to them by construction — the same failure shape as (iii-a), one layer further out.

```
rg -n "FileStatus|fileStatus" src/graph   # 0 matches
```
**Yield: 0.** `src/graph/{drawRowText,forgeBadges,verifyBadge}.ts` draw ref pills, PR pills, CI dots
and verification badges — never an A/M/D/U/R letter. The status-badge family is **DOM-only**, and this
is now evidenced rather than assumed.

### Population

| Pass | Yield |
|---|---|
| (i) declarations in `status-panel.css` | 7 (5 badge inks + 1 section label + 1 tint background) |
| (ii) render sites with a hue | 6 |
| (iii-a) render sites with **no** hue | 2 |
| (iii-b) hue via custom property | 0 |
| (iii-c) canvas-drawn status badges | 0 |
| **Total render sites** | **8** (7 files) |
| **Total ink declarations under verdict** | **6** (5 badge + `.section-label-danger`) |

---

## 2. Method and calibration (reproduce before trusting anything below)

Measured in the running harness (`http://localhost:1420`, `VITE_MOCK_IPC=1`, viewport **1440×900** —
set explicitly, because a hidden Browser pane reports `innerWidth/innerHeight = 0`), both themes, on
**2026-09-03**. An off-screen probe resolves each `color-mix()` through the browser's own mixing
model; the result is source-over composited onto the resolved backdrop token in sRGB; WCAG 2.x
relative luminance and contrast follow. Themes are read by toggling `data-theme` on
`documentElement` and restoring it.

**P107's serialization trap was handled explicitly:** Chrome serialises the 6% `--success` tint as
`color(srgb 0.341176 0.670588 0.352941 / 0.06)` (0–1 components) and other mixes as `rgba(…)` (0–255).
The `color(` form is parsed by its own branch. A single numeric regex over both inflates every
dark-theme figure by ≈0.8 and invalidates the run.

**Calibration — nine independently recorded historical values, re-derived:**

| Check | Recorded (source) | This run | |
|---|---|---|---|
| `--danger` on `--bg-1` | 4.41 / 4.60 (ui-ref §7, P107 §8) | **4.41 / 4.60** | ✓ |
| `--danger` on `--bg-2` | 3.89 / 4.24 (P107 §8) | **3.89 / 4.24** | ✓ |
| `--danger` on `--selection` | 3.05 / 3.96 (P107 §8) | **3.05 / 3.96** | ✓ |
| `--success` on `--bg-1` | 5.73 / 4.74 (ui-ref §7) | **5.73 / 4.74** | ✓ |
| `--warning` on `--bg-1` | 7.28 / 4.54 (ui-ref §7) | **7.28 / 4.54** | ✓ |
| `--accent-strong` on `--bg-1` | 7.13 / 5.81 (ui-ref §7) | **7.13 / 5.81** | ✓ |
| `--accent-strong` on `--selection` | 4.93 / 5.01 (ui-ref §7) | **4.93 / 5.01** | ✓ |
| `--success` on the staged 6% tint | 5.26 / 4.38 (P107 §3 D) | **5.26 / 4.38** | ✓ |
| `--danger`/`--success`/`--warning` on own 14% tint / `--bg-2` | 3.35/3.48, 4.07/3.66, 4.96/3.53 (ui-ref §2) | **identical** | ✓ |

> **Carry this hedge forward.** One calibration item did **not** reproduce: P107 §2 records
> `--accent-strong` on a 14% accent tint as 5.85 / 4.87; this run gets **5.16 / 4.52** with `--bg-2` as
> the base. The gap is a **base ambiguity** — P107's row does not state which surface the tint sits on
> — not a method disagreement, and it touches nothing in P106 (no accent tint exists in this family).
> It is recorded here rather than silently dropped, per §2's "evidence lost in transcription" rule.
> **Do not** transcribe it into `ui-reference.md` as a discrepancy; transcribe it as *base
> unspecified*.

---

## 3. Is the letter ever the sole carrier? (required determination)

**Yes — at all 8 render sites the letter is the only non-colour carrier of the status, so no variant
is exempt from 4.5:1.** The evidence, per candidate exemption:

| Candidate exemption | Holds? | Why |
|---|---|---|
| Shape / position varies by status | **No** | Every badge is the same 12px-wide, `text-align:center`, 11px/600 mono span in the same first slot of the row. `A`, `M`, `D`, `R`, `T`, `C` differ only as letterforms. |
| A word elsewhere in the row names the status | **No** | The row shows the path and nothing else. `.diff-overlay-kind` names the diff *kind* (staged/unstaged/PR), not the file status. |
| The section header names it | **Partly, and it does not exempt** | Conflicted rows sit under a `Conflicts` header, and staged rows under `Staged`. WCAG 1.4.3 applies to all text that is not incidental or decorative; a letter that *names* the status is neither. Redundancy elsewhere does not lower the bar on the glyph the user reads. |
| It is "large text" (3:1 allowed) | **No** | 11px at weight 600 is far below the 18.66px-bold large-text threshold. The badge does **not** scale with `--rp-row-font`, so it is 11px in **both** densities. |
| It is a graphic, judged at 3:1 | **No** | It is a rendered character in the accessibility tree, selectable and searchable. P105 already ruled this for `.file-status-renamed`; P106 does not reopen it. |

**Corollary, and a genuine gap this contract records but does not fix.** The badge is a bare `<span>`
with no accessible name, so a screen reader announces the bare character `M` — present, but jargon the
user did not choose. WCAG 1.4.1 is satisfied (the letter is a non-colour carrier); the quality of the
accessible name is not. **Follow-up, not an AC here** (it is 8 JSX edits and a new string set, i.e. its own
increment): give the span `role="img"` + `aria-label` of `Added` / `Modified` / `Deleted` / `Renamed` /
`Type changed` / `Untracked` / `Conflicted`. Note `untracked` and `added` deliberately share the
letter `A` (`StatusFileRow.tsx:15`, P4c), so the accessible name is the only thing that could ever
tell them apart — file it as **P109**, do not silently roll it in here.

---

## 4. Enumeration — bucket + verdict per call site, backdrop resolved per state

### 4.1 The 8 render sites and the backdrops they actually composite over

`.file-row:hover { background: var(--bg-2) }` and `.file-row-expanded { background: var(--selection) }`
are **opaque**, so they replace the section tint entirely — the hover and selected backdrops are the
same regardless of which section the row lives in. That is what makes the state matrix small.

| # | Render site | Container | rest | hover | selected/active | Hue? |
|---|---|---|---|---|---|---|
| S1 | `StatusFileRow.tsx:109` (expandable row) | `.status-section--staged` / `--changes` in `.right-panel` | staged: 6% `--success` / `--bg-1`; changes: 5% `--text-3` / `--bg-1` | `--bg-2` | `--selection` (`.file-row-expanded`) | yes |
| S2 | `StatusFileRow.tsx:114` (non-expandable row) | same | same | `--bg-2` | `--selection` | yes |
| S3 | `StatusConflictsSection.tsx:124` | plain `.status-section` (**no tint**) in `.right-panel` | `--bg-1` | `--bg-2` | `--selection` | yes |
| S4 | `DiffFileTree.tsx:181` | `.commit-files .diff-tree` in `.right-panel` | `--bg-1` | `--bg-2` | `--selection` (`.diff-tree-selected`) | yes |
| S5 | `DiffBrowser.tsx:404` | `.diff-card-header` (own `--bg-1`) inside `.diff-card` on `--bg-0` | `--bg-1` | `--bg-2` | — | yes |
| S6 | `prPanel/PrFileRow.tsx:37` | `.diff-card-header`; binary rows restate `--bg-1` | `--bg-1` | `--bg-2` | `--selection` (`.pr-file-row-active`) | yes |
| S7 | `DiffOverlay.tsx:295` | `.diff-overlay-header` on `.diff-overlay`'s `--bg-0` | `--bg-0` | — | — | **no** |
| S8 | `ComposerGroupCard.tsx:129` | `.composer-file-row` (no own background) | inherited card surface | — | — | **no** |

**Live backdrop set = 6:** `--bg-0`, `--bg-1`, `--bg-2`, `--selection`, staged 6% `--success` / `--bg-1`,
changes 5% `--text-3` / `--bg-1`.

### 4.2 Measured — current ink × live backdrop, **dark / light**

Bold = **below 4.5:1**.

| Ink (statuses) | `--bg-0` | `--bg-1` | `--bg-2` hover | `--selection` | staged tint | changes tint | **MIN** |
|---|---|---|---|---|---|---|---|
| `--success` (`A` added, `A` untracked) | 6.24 / 5.08 | 5.73 / 4.74 | 5.06 / **4.37** | **3.96** / **4.08** | 5.26 / **4.38** | 5.46 / 4.53 | **3.96 / 4.08** |
| `--warning` (`M` modified, `T` typechange) | 7.92 / 4.87 | 7.28 / 4.54 | 6.42 / **4.19** | 5.03 / **3.91** | 6.67 / **4.20** | 6.93 / **4.34** | **5.03 / 3.91** |
| `--danger` (`D` deleted, `C` conflicted) | 4.80 / 4.93 | **4.41** / 4.60 | **3.89** / **4.24** | **3.05** / **3.96** | **4.04** / **4.26** | **4.19** / **4.40** | **3.05 / 3.96** |
| `--accent-strong` (`R` renamed) | 7.76 / 6.23 | 7.13 / 5.81 | 6.29 / 5.36 | 4.93 / 5.01 | 6.54 / 5.37 | 6.78 / 5.55 | 4.93 / 5.01 ✓ |
| `--text-1` (S7/S8, no hue) | 14.73 / 16.52 | 13.54 / 15.42 | 11.95 / 14.23 | 9.36 / 13.29 | 12.42 / 14.26 | 12.89 / 14.74 | 9.36 / 13.29 ✓ |

**Reading of the state axis, which is the point of measuring it:** the badge fails *worse* in the
states a user actually drives it through. `D` is 4.41 at rest in dark (already failing), 3.89 the
moment the pointer is on the row, and **3.05** once the row is selected and its diff is open — i.e. the
state the user is in while reading the diff is the worst state in the app. `M` is compliant in dark
everywhere and fails in light in 4 of its 6 backdrops. `A` fails on selection in both themes.

### 4.3 Buckets and verdicts

**Bucket A — hue letter as sole label, below 4.5:1 in ≥1 live state. FIX all 5.**

| # | `file:line` | Selector | Statuses | Worst live state | Verdict |
|---|---|---|---|---|---|
| A1 | `status-panel.css:193` | `.file-status-added .file-badge` | `A` | `--selection` **3.96 / 4.08** | → `--success-strong` |
| A2 | `status-panel.css:198` | `.file-status-modified .file-badge`, `.file-status-typechange .file-badge` | `M`, `T` | `--selection` 5.03 / **3.91** | → `--warning-strong` |
| A3 | `status-panel.css:203` | `.file-status-deleted .file-badge`, `.file-status-conflicted .file-badge` | `D`, `C` | `--selection` **3.05 / 3.96** | → `--danger-strong` |
| A4 | `status-panel.css:216` | `.file-status-untracked .file-badge` | `A` | `--selection` **3.96 / 4.08** | → `--success-strong` |
| A5 | `status-panel.css:227` | `.section-label-danger` | the word `Conflicts` | `--bg-1` **4.41** / 4.60 | → `--danger-strong` |

A5 is in this contract because P107 §8 assigned it here explicitly ("the whole A/M/D/U/R family —
`.file-badge` letters, `.section-label-danger`, the staged-section tint — is P106's"). It is a
**read label on a neutral backdrop**, the P108 class, but it lives in P106's file and `status-panel.css`
must appear in exactly one diff.

**Bucket B — clears 4.5:1 in every live state. KEEP, untouched: 1.**

| `file:line` | Selector | MIN | Verdict |
|---|---|---|---|
| `status-panel.css:210` | `.file-status-renamed .file-badge` (`--accent-strong`, P105 `0e5dcab`) | 4.93 / 5.01 | KEEP — do not restyle, do not "harmonise" it with the new tokens |

**Bucket C — no hue; compliant by inheritance, inconsistent with the family: 2.**

| `file:line` | Ink | MIN | Verdict |
|---|---|---|---|
| `DiffOverlay.tsx:295` | inherited `--text-1` on `--bg-0` | 14.73 / 16.52 | **No a11y defect.** Consistency fix is decision **D1** |
| `ComposerGroupCard.tsx:129` | inherited `--text-1` | ≥12 on any app surface | same |

**Live-DOM confirmation (the §2 three-part discipline, part 2).** Confirmed rendering in the harness
this session, with computed colours read from the DOM: `file-status-added` (1), `file-status-untracked`
(4), `file-status-modified` (9), `file-status-deleted` (2), `file-status-renamed` + `file-row-expanded`
(1, on `--selection`), and the hue-less `.diff-overlay-header` badge (1) — **18 live instances**.
**Not confirmed in the harness, hedge carried forward:**

- **`.file-status-typechange` (`T`) — no fixture produces it.** It is **not** a dead rule: the real
  backend emits it (`crates/bonsai-core/src/git/diff/collect.rs:40`,
  `git2::Delta::Typechange => FileStatus::Typechange`, and the porcelain map `'T' => typechange`), and
  all six `BADGES` tables map it. It is *unreachable in the mock harness*, which is a verification gap,
  not a dead-CSS gap. **AC9 closes the gap by adding the fixture** rather than by asserting.
- **`.file-status-conflicted` (`C`)** renders only in the merge-conflict mock state
  (`src/ipc/fixtures/conflicts.ts`); not exercised in this session's harness pass. Verify under AC10.
- **S5, S6, S8 (`DiffBrowser`, `PrFileRow`, `ComposerGroupCard`)** were confirmed by source, not by
  DOM, this session. Their backdrops are restated `--bg-1`/`--bg-2`/`--selection` — already in the
  matrix — so no number depends on them, but the *rendering* confirmation is owed under AC10.

---

## 5. The `--warning-text` question — decided, with measurements both ways

**Decision: NO. `--warning-text` is not needed, and using it here would be a defect.**

P107 answered this for *solid fills* ("`--bg-0` already resolves to the same value"). That reasoning
does not transfer, so it was re-derived for a letterform on a tint:

| What | Would-be value | On the badge's worst live backdrops (dark / light) | |
|---|---|---|---|
| `--warning-text` (= `--bg-0`, by construction) | `#16181d` / `#ffffff` | staged tint **1.19 / 1.16**, `--bg-1` **1.09 / 1.07**, `--selection` **1.57 / 1.24** | catastrophic |
| `--danger-text` (the same family, measured for the trap) | `#16181d` / `#ffffff` | 14% danger tint **1.43 / 1.41** | reproduces P107's 1.27-class trap |
| `--warning-strong` (this contract) | `#e3b341` / `#7a4f01` | **5.80 / 5.73** minimum across all six | correct tool |

**The `--*-text` tokens are fill-inks. They are near-black in dark and white in light, which is exactly
inverted from what a letter on a *tinted or neutral panel* needs.** Reaching for `--warning-text`
because the hue name matches is the single most likely implementation error in this increment — the
same error P107 called out — and it is called out here so review catches it.

**This also closes §2's open note that `--warning` had never been measured against a letterform.**
It now has, in both roles:

- `--warning` **as a letterform on neutral chrome**: `--bg-0` 7.92 / 4.87 · `--bg-1` 7.28 / **4.54**
  (light passes by 0.04) · `--bg-2` 6.42 / **4.19** ✗ · `--selection` 5.03 / **3.91** ✗ ·
  staged tint 6.67 / **4.20** ✗ · changes tint 6.93 / **4.34** ✗.
- `--warning` **as a letterform on its own tint** (P107 A3/A5/A7/A9/A11/A14/A15): 3.49–4.17 light ✗.
- `--bg-0` **as ink on a solid `--warning` fill** (`.ai-dock-ask-glyph`): 7.92 / 4.87 ✓ — unchanged,
  not P106's business.

Net: **dark `--warning` is an adequate letterform on neutral surfaces; light `--warning` is not.**
Both numbers belong in `ui-reference.md` §2.

---

## 6. Token decisions

### 6.1 Three new tokens — the read-text form of the three remaining hues

Added to `src/styles/tokens-and-base.css` beside `--accent-strong` (`:root` line ~35,
`[data-theme='light']` line ~131). **No other value in the file changes.**

| Token | Dark | Light | Role |
|---|---|---|---|
| `--danger-strong` | `#ff938c` | `#b3282e` | **the danger hue used as read text.** Never a fill, never a border, never the focus ring — those stay `--danger` |
| `--success-strong` | `#7fc98a` | `#116329` | **the success hue used as read text.** Same restriction |
| `--warning-strong` | `#e3b341` | `#7a4f01` | **the warning hue used as read text.** Same restriction |

**Measured on all six live backdrops (dark / light):**

| Token | `--bg-0` | `--bg-1` | `--bg-2` | `--selection` | staged tint | changes tint | **MIN** |
|---|---|---|---|---|---|---|---|
| `--danger-strong` | 8.29 / 6.44 | 7.62 / 6.01 | 6.72 / 5.55 | **5.27 / 5.18** | 6.99 / 5.56 | 7.25 / 5.74 | **5.27 / 5.18** |
| `--success-strong` | 8.98 / 7.39 | 8.26 / 6.90 | 7.29 / 6.36 | **5.71 / 5.94** | 7.57 / 6.38 | 7.86 / 6.59 | **5.71 / 5.94** |
| `--warning-strong` | 9.13 / 7.13 | 8.39 / 6.65 | 7.40 / 6.14 | **5.80 / 5.73** | 7.69 / 6.15 | 7.98 / 6.36 | **5.80 / 5.73** |
| `--accent-strong` (existing, for comparison) | 7.76 / 6.23 | 7.13 / 5.81 | 6.29 / 5.36 | 4.93 / 5.01 | 6.54 / 5.37 | 6.78 / 5.55 | 4.93 / 5.01 |

**Worst value anywhere in the fixed family: 5.18** (`--danger-strong`, light, `--selection`) — 0.68
above the bar. The four minima now sit in a **4.93–5.94** band (`--accent-strong` 4.93/5.01,
`--danger-strong` 5.27/5.18, `--warning-strong` 5.80/5.73, `--success-strong` 5.71/5.94), so the five
badges read as one system rather than four unrelated colours.

> **Transcription check (do not skip).** The light column of `--warning-strong` above is `#7a4f01`.
> A darker candidate, `#6e4600`, measures 8.27 / 7.72 / 7.12 / **6.65** / 7.14 / 7.38 and was **not**
> chosen — its floor is out of band with its siblings. If a future pass sees 6.65 attributed to
> `#7a4f01`, that is this contract's own first-draft error, caught and corrected here.

**Why these hexes.** Each is the same hue, lifted in dark / deepened in light, exactly the rule
`--accent-strong` established (`#4f8cff` → `#7fabff` dark, `#2f6fe4` → `#2a5cbe` light). Candidates
measured and rejected: `#ef7c74` (danger dark, MIN 4.19 — still fails), `#f5877f` (4.65 — passes with
0.15 of headroom, too thin for a token that will be reused), `#c0272d` (danger light, 4.73 — same
objection), `#855800` (warning light, 4.98 — acceptable but out of band with its siblings).

**Graphics bar sanity check:** these tokens are *ink only*, so the 3:1 graphics bar is not the relevant
test for them; the hue used as a fill/bar/border stays `--danger`/`--success`/`--warning`, whose ≥3:1
figures are unchanged by this contract.

### 6.2 Tokens explicitly rejected here

- **`--text-1` for all badges (the "demote to letter + shape" option P105 floated).** MIN 9.36 / 13.29
  — trivially compliant, and **rejected**. §7 makes the hue+letter pairing the house precedent for
  file status; five grey letters would delete a scanning aid that works (a user finds the deletions in
  a 200-file list by colour, then reads the letter to confirm). This is the two-recipe rule from
  P102/P105 applied correctly: only **states** demote to neutral; an identity that the user scans by
  keeps its hue and flips the ink. Recorded as the rejected alternative so it is not re-litigated.
- **`--danger-text` / `--success-text` / a new `--warning-text`.** §5. 1.07–1.57 here.
- **`--accent-strong` for anything but `renamed`.** A blue `D` is semantic nonsense.
- **A 35% hue border or tint behind the badge as the carrier.** `ui-reference.md` §2 now records that a
  35% hue edge is **1.58 / 1.69** — decoration, never an identity carrier (P107 §7). The badge is
  12×~16px; there is no room for a bar, and a bar would not carry the meaning anyway.

---

## 7. The fix

**CSS only** for the a11y defect: 5 declarations in `src/styles/status-panel.css` and 6 token lines in
`src/styles/tokens-and-base.css`. **No DOM change, no geometry change, no new component file.** Nothing
in this fix approaches the ~500-line file limit; both files are edited in place and neither grows by
more than 8 lines.

```
status-panel.css:193   color: var(--success-strong);   /* was --success  (A added)              */
status-panel.css:198   color: var(--warning-strong);   /* was --warning  (M modified, T typechange) */
status-panel.css:203   color: var(--danger-strong);    /* was --danger   (D deleted, C conflicted)  */
status-panel.css:210   color: var(--accent-strong);    /* UNCHANGED      (R renamed, P105)       */
status-panel.css:216   color: var(--success-strong);   /* was --success  (A untracked)          */
status-panel.css:227   color: var(--danger-strong);    /* was --danger   (.section-label-danger) */
```

Update the P105 comment block at `status-panel.css:207-209` so it no longer says "The other three badge
hues are filed as P106" — replace with a one-line pointer to this contract and the MIN figures.

**What must NOT change:**

- `.status-section--staged`'s 6% `--success` tint (`:28`) and `.status-section--changes`'s 5%
  `--text-3` tint (`:38`). They are **backgrounds**, not text; the ink fix is what closes the
  composite, and removing the tint would delete the staged/changes non-verbal distinction.
- `.file-badge` geometry (`:75-81`): `width: 12px`, `font-size: 11px`, `font-weight: 600`,
  `text-align: center`, `flex: none`.
- `.file-status-untracked .file-path/.file-name` (`:220-224`) — already `--text-1`.
- The `--danger`/`--success`/`--warning` tokens themselves. Changing them would ripple into every
  fill, bar and border in the app and would invalidate the `--danger-text`/`--success-text` pairs
  measured in P102.

### D1 (decision required) — the two hue-less sites

`DiffOverlay.tsx:295` and `ComposerGroupCard.tsx:129` show a grey letter where every other surface
shows a coloured one. **Recommendation: fix it**, by adding the existing class to the element that
already wraps the badge — `className={\`diff-overlay-header file-status-${meta.status}\`}` and
`className={\`composer-file-row file-status-${...}\`}`. Verified safe: the only other
`.file-status-*` descendant rules target `.file-path` / `.file-name`, and neither surface uses those
class names (`.diff-overlay-path`, `.composer-file-path`), so there is no bleed. Two one-line edits, no
new strings, and it makes the family uniform.
**This is a visual change, not an a11y fix** — AC12 is written so it can be dropped without touching
AC1–AC11. Orchestrator's call.

---

## 8. Themes, densities, states, motion, copy

- **Dark and light** — both specced above; every figure in §6.1 is a dark/light pair. Light is the
  theme that fails hardest today (`M` fails 4 of 6 backdrops), so light is the theme to screenshot.
- **Densities** — `cozy`: row 24px, path font inherited; `compact`: row 20px, path font 12px. The badge
  is **11px in both** (`font-size: 11px` is literal on `.file-badge`, not `var(--rp-row-font)`) and
  12px wide in both. P106 changes neither; the 4.5:1 bar applies identically at both densities. Do not
  "fix" the badge to scale with density in this increment — that is a separate geometry decision.
- **States** — rest / hover (`--bg-2`) / selected-expanded (`--selection`) / active
  (`.pr-file-row-active`, `--selection`) all measured in §4.2 and §6.1. **Disabled:** the status badge
  has no disabled state and must not gain one — the §2 `.55` dim over these backdrops drops
  `--danger-strong` below the bar in light. **Focus:** the badge is never focusable; the row or its
  button owns the 2px `--accent` `:focus-visible` ring, unchanged. **Empty/loading/error:** the badge
  does not render; the section's existing `.section-empty` copy is unchanged. **Overflow:** the badge
  is `flex: none` and one character — it never truncates; the path beside it keeps its
  ellipsis + `title`.
- **Motion** — none. No transition is added to `color`; a colour transition on a virtualized 20k-row
  list is exactly the kind of work that contends with the graph render budget.
  `prefers-reduced-motion` is unaffected.
- **Microcopy** — **zero new strings.** No label, tooltip or error text changes. (The accessible-name
  work is P109, §3.)

---

## 9. Harness states (`VITE_MOCK_IPC=1`, `pnpm dev:mock`, port 1420)

| State | Fixture | Status today |
|---|---|---|
| `A` added (staged tint), `A` untracked (changes tint), `M`, `D`, `R` | `src/ipc/fixtures/status.ts` | **present**, 18 live badges confirmed this session |
| `R` on `--selection` | click a renamed row → `.file-row-expanded` | **present**, confirmed |
| hover `--bg-2` | pointer over any row | present |
| `C` conflicted | merge-conflict mock state, `src/ipc/fixtures/conflicts.ts` | present, **not exercised this session** — verify under AC10 |
| **`T` typechange** | **none — must be added** | **MISSING.** Add one row, e.g. `{ path: 'scripts/build.sh', origPath: null, status: 'typechange' }`, to the unstaged list so `status-panel.css:198` has a live harness match |
| Pathological long content | a ≥180-char nested path in the same fixture list | add alongside the `T` row; confirms the badge stays `flex: none` while the path ellipsises |
| Light theme | theme toggle / `data-theme='light'` | present |
| Compact density | Settings → right-panel density | present |

`DiffBrowser` (S5) and `PrFileRow` (S6) are reachable in the harness (all-files diff; PR changed-files
panel). `ComposerGroupCard` (S8) is reachable via the commit composer. **All 8 render sites are
harness-visible** — nothing in this contract is invisible to browser verification, which is why the
only USER CHECKPOINT items below are perceptual.

---

## 10. Predicted post-fix grep residue (state this **before** implementing; verify after)

P102/P105 and P107 hit their predictions exactly on every metric. This is the standard; a mismatch is a
finding, not a rounding error.

| # | Command | Now | Predicted after |
|---|---|---|---|
| R1 | `rg -n "color:\s*var\(--(danger\|success\|warning)\)" src/styles/status-panel.css` | **5** | **0** |
| R2 | `rg -n "color:\s*var\(--(danger\|success\|warning)\)" src/styles` | **50** | **45** — all outside `status-panel.css`; this is the P108 population and P106 must not shrink it further |
| R3 | `rg -n -- "--(danger\|success\|warning)-strong:" src/styles/tokens-and-base.css` | **0** | **6** (3 in `:root`, 3 in `[data-theme='light']`) |
| R4 | `rg -n -- "var\(--(danger\|success\|warning)-strong\)" src/styles` | **0** | **5**, all in `status-panel.css` (`193`, `198`, `203`, `216`, `227`) |
| R5 | `rg -n -- "(background\|border\|box-shadow\|outline\|fill)[^;]*--(danger\|success\|warning\|accent)-strong" src/styles` | **0** | **0** — the `-strong` family is ink-only |
| R6 | `rg -n -- "--(danger\|success\|warning)-text" src/styles/status-panel.css` | **0** | **0** — the §5 trap |
| R7 | `rg -n "className=\"file-badge mono\"" src/components` | **8** | **8** — no render site added or removed |
| R8 | `rg -n "file-status-" src/components` (excl. tests) | **6** | **6**, or **8** if D1 is taken |
| R9 | `rg -n "typechange" src/ipc/fixtures` | **0** | **1** |
| R10 | `rg -n "#[0-9a-fA-F]{6}" src/components src/styles --glob '!tokens-and-base.css'` | **0** | **0** |

---

## 11. Acceptance criteria

Numbered; each is checkable against §10's residue table or a measured ratio.

1. **AC1 — Tokens exist, both themes.** `--danger-strong`, `--success-strong`, `--warning-strong` are
   defined in `src/styles/tokens-and-base.css` only, with the exact §6.1 hexes, in both `:root` and
   `[data-theme='light']`. Residue **R3 = 6**.
2. **AC2 — The five declarations are swapped, and nothing else in the file is.** `status-panel.css`
   diff touches lines `193`, `198`, `203`, `216`, `227` and the `207-209` comment only. The tints at
   `:28`/`:38` and the geometry at `:75-81` are byte-identical. Residue **R1 = 0**, **R4 = 5**.
3. **AC3 — Every status clears 4.5:1 in every live state, both themes.** Re-measured in the harness
   with §2's method; the §6.1 matrix reproduces within ±0.02 and the minimum over the whole family is
   **≥ 5.18**.
4. **AC4 — `renamed` is untouched.** `status-panel.css:210` still reads `var(--accent-strong)`, and
   re-measures 7.13 / 5.81 on `--bg-1` and 4.93 / 5.01 on `--selection`.
5. **AC5 — The `-strong` family is ink-only.** No `-strong` token appears in a `background`, `border`,
   `box-shadow`, `outline` or `fill` anywhere. Residue **R5 = 0**.
6. **AC6 — No `--*-text` token in `status-panel.css`.** Residue **R6 = 0** (the 1.07–1.57 trap).
7. **AC7 — No geometry, DOM or copy change.** Badge is 12px wide / 11px / 600 in **both** densities;
   rows remain 24px cozy and 20px compact; zero string changes. Residue **R7 = 8**.
8. **AC8 — The P108 population is unchanged.** Residue **R2 = 45**, all outside `status-panel.css`;
   P106 does not opportunistically fix sites it did not enumerate.
9. **AC9 — The `T` badge becomes harness-verifiable.** A `typechange` row and a ≥180-char path row are
   added to `src/ipc/fixtures/status.ts`. Residue **R9 = 1**. (`typechange` is emitted by
   `crates/bonsai-core/src/git/diff/collect.rs:40` — this closes a *verification* gap, not a dead rule.)
10. **AC10 — Live-match confirmation table, per declaration.** For each of the 6 declarations under
    verdict, the implementation records: rendered / not rendered, the surface it was seen on, and the
    computed colour read from the DOM. The four sites not confirmed this session — `C` conflicted, S5
    `DiffBrowser`, S6 `PrFileRow`, S8 `ComposerGroupCard` — are each either confirmed or carried
    forward with the word **"unverified"** verbatim into `ui-reference.md`. **A declaration may not be
    recorded as fixed on grep evidence alone** (§2, second failure mode).
11. **AC11 — `.section-label-danger` clears the bar.** The `Conflicts` header measures **7.62 dark /
    6.01 light** on `--bg-1`.
12. **AC12 — [D1, droppable]** `DiffOverlay.tsx:295` and `ComposerGroupCard.tsx:129` are hue-coded via
    an added `file-status-${status}` class on their existing wrapper; no other class or string changes.
    Residue **R8 = 8**. Dropping AC12 invalidates no other AC.
13. **AC13 — `ui-reference.md` is updated in the same pass.** §2 gains the three tokens with their
    dark/light ratios and the ink-only restriction; §2's `--warning`-letterform note is replaced by
    §5's measurements; §7's "filed as P106 … do not close it by assertion" paragraph is replaced by the
    closed record with the per-state matrix and the §3 sole-carrier determination; the §2 calibration
    hedge from §2 of this contract and every "unverified" from AC10 are carried **verbatim**; P109
    (accessible names) is named as the open follow-up so the family is not recorded as fully closed.
14. **AC14 — USER CHECKPOINT (perceptual, native window).** See §12.
15. **AC15 — USER CHECKPOINT (hue-identity judgement).** See §12.

---

## 12. USER CHECKPOINT items — these stay **pending**; do not self-declare

The user's checkpoint authority this session was scoped to other milestones and does **not** reach this
work. No agent message closes these.

- **AC14 — native-window read.** `pnpm tauri dev`, real repo with staged + unstaged changes: confirm
  the badge column is legible at arm's length in **both themes** and **both densities**, and that the
  brighter dark-theme letters do not read as "selected" or "highlighted" rows. Screen rendering of
  11px mono at these luminances is not judgeable from the headless harness (no `requestAnimationFrame`,
  no subpixel/gamma parity).
- **AC15 — hue identity.** Confirm `--danger-strong` still reads as *red-for-deleted* and
  `--warning-strong` as *yellow-for-modified* rather than drifting to pink/olive, and that the five
  badge colours remain mutually distinguishable — including for a user with deuteranopia, where
  `--success-strong` vs `--warning-strong` is the risky pair (the letters `A` vs `M` carry the meaning
  regardless, per §3, so this is a quality judgement, not a compliance one).
- **AC9's real-world half.** A genuine `typechange` (symlink → regular file) cannot be produced in the
  mock harness; the fixture proves the *rule*, the native app proves the *pipeline*. Optional.

Everything else — AC1–AC13 — is AI-gate verifiable in the browser harness plus `rg`.

---

## 13. Flagged for the orchestrator

1. **D1 (AC12) — hue-code the two colourless badges?** My recommendation: **yes**, two one-line class
   additions, verified bleed-free, and it is the difference between "the badge is coloured" and "the
   badge is coloured except in two places". Counter-argument if you decline: P106 is an a11y fix and
   D1 is a visual change with no a11y content. Either way, AC12 is isolated.
2. **`--warning-strong`'s dark value.** I specify `#e3b341` (MIN **5.80**) so all three tokens follow
   the same "lifted in dark, deepened in light" rule. The alternative is `#d4a72c` — byte-identical to
   `--warning`, MIN **5.03**, i.e. compliant with 0.53 of headroom and **zero** visual change in dark.
   If minimising dark-theme churn matters more than family coherence, take the alternative; it still
   satisfies AC3 with a lower floor. My recommendation is `#e3b341`.
3. **P109 is created by this contract** (§3): the badge has no accessible name, and `added` and
   `untracked` share the letter `A`, so a screen-reader user cannot distinguish them at all. Eight JSX
   edits + a seven-string set. Not rolled in here on purpose.
4. **P108's population is unchanged at 45** (R2). P106 hands it the three new tokens as its tool — most
   of those 45 are hue-as-read-text on neutral backdrops, which is precisely what `--*-strong` exists
   for. Do not let P108 re-derive different hexes.
5. **`status-panel.css` must appear in exactly one diff** (P107 §8). If P107 follow-ups are in flight,
   land P106 first.
