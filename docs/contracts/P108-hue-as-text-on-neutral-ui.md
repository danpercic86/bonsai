# P108 — Hue as text over a NEUTRAL surface (UI contract)

**Status:** contract, not yet implemented. **Date:** 2026-09-03. **Owner:** ui-designer.
**Predecessors:** `P102-P105-hue-audit-ui.md` (accent-as-text), `P106-status-badge-ink-ui.md`
(A/M/D/U/R letter family, `-strong` tokens), `P107-hue-over-own-tint-ui.md` (hue over its own tint).
**Canonical design system:** `docs/contracts/ui-reference.md` §2 (tokens, BASE rule, the three
failure modes), §7 (status colours), §11 (pill recipe).

---

## 0. Result in one screen

- **The search found 62 call sites** (61 declarations; one declaration carries two verdicts).
  **Verdict: FIX 28, KEEP 34.**
- **The recorded inventory of 48 was wrong again — this is the fifth time a count in this programme
  has moved.** The same-shaped grep measured against *this* tree returns **54 raw matches**, not 48.
  The gap is not mysterious: 48 was measured before the `settings-dev.css` (P91) surface existed and
  before three `border-color: var(--hue)` declarations were counted by the same pattern. **48 was a
  starting figure to verify and it did not survive verification.** Every number below was measured in
  this tree on 2026-09-03; none is inherited.
- **Two whole classes were invisible to the pass-1 grep and are the reason the count is 62, not 54:**
  (a) **token aliasing** — `--badge-warn` is byte-identical to `--danger`, and
  `.commit-signature-warn .commit-signature-text` is hue-as-read-text under that alias, failing at
  **4.41 dark**; (b) **indirection through an undefined custom property** —
  `.settings-config-warn` is `color: var(--warn, #b8860b)` and **`--warn` is not defined anywhere**,
  so that rule paints a hardcoded, theme-invariant gold measuring **3.25:1 in light**.
- **A third find outside the hue alphabet, kept because it sits in the same rule block:**
  `--badge-unknown` is byte-identical to `--text-3`, so `.commit-signature-unknown
  .commit-signature-icon` is a `--text-3` glyph at **2.96 light** — below even the 3:1 graphics bar.
  It was invisible to P101's exhaustive `color: var(--text-3)` audit **because of the alias**. The
  `--text-3` family is recorded as CLOSED in `ui-reference.md` §2; this is an escape from it.
- **No new token.** All 28 fixes are `--danger-strong` / `--success-strong` / `--warning-strong` /
  `--text-2`, all existing. Post-fix worst figure anywhere: **5.10** (`--danger-strong`, light, on
  `--bg-3`) — see §6, which revises the family minimum recorded in `ui-reference.md` §2.
- **19 hue glyphs, 3 hue borders and 6 compliant hue texts are KEPT untouched.** A blanket sweep
  would have deleted four working scanning aids and three deliberate redundant-cue recipes.

---

## 1. Scope — and the boundary against P105 and P107, stated so it is unambiguous

This programme has now split the "hue used as ink" defect into three disjoint milestones. The
partition is by **what the ink sits on**, and by **which hue token**:

| Milestone | Ink | Backdrop | Status |
|---|---|---|---|
| **P105** | `color: var(--accent)` **specifically** | any | shipped `0e5dcab`; residue is 7 glyph keeps (its §3.3 A1–A7) |
| **P107** | any hue | **a tint of that same hue** | shipped `2168057`; 38 sites, 17 fixed / 19 glyph keeps / 1 glyph fix / 1 handed to P106 |
| **P108 (this)** | `--danger` / `--success` / `--warning` / `--merged` **and their aliases** | a **NEUTRAL** surface — `--bg-0/1/2/3`, `--selection`, the graph canvas, or a tint of a *different* hue | this contract |

**Explicitly OUT of P108 scope, and why:**

- **`color: var(--accent)` — 7 live declarations** (`sidebar.css:152`, `graph-filter.css:62/188/240`,
  `graph-replay.css:27`, `partial-staging.css:24`, `commit-box.css:225`). This is P105's class by the
  table above. Six are recorded with verdicts in `P102-P105-hue-audit-ui.md` §3.3 as A1/A2/A3/A4/A6/A7
  glyph keeps; **`commit-box.css:225` was not found in that table by this search** — see §10, flagged
  to the orchestrator as a possible P105 residue item, **not claimed here**.
- **Hue over its own tint — 5 of the 54 pass-1 matches** are P107's class and already carry a P107
  verdict: `checks-panel.css:119` (`.checks-rollup-pill--pending .checks-rollup-glyph`),
  `controls.css:218` (`.dev-mode-pill-glyph`), `ai-dock-log.css:273` (`.ai-dock-ask-guard-glyph`),
  `forge-pr.css:806` (`.forge-reauth-icon`), `graph-filter.css:93`
  (`.graph-filter-chip-stale .graph-filter-chip-glyph`). Plus the four `--h` indirection sites
  (`toasts-and-overlays.css:61`, `sidebar.css:221`, `git-dock.css:270`, `ai-dock.css:186`), all of
  which resolve to a hue on a 14% tint of that same hue. **Not re-litigated here.**
- **`--*-text` on a solid hue fill — 6 declarations** (`controls.css:73`, `forge-pr.css:186/194/200`,
  `updates.css:119`, `partial-staging.css:115`). P102's class, shipped.
- **The 5 `--*-strong` declarations in `status-panel.css`** — P106's shipped fix.
- **`forgeBadges.ts:51/58`** — `text: '#ffffff'` on a `badgeGood`/`badgeWarn` **fill**. Ink-on-fill
  (P102's class) *and* a hex literal in TS. Recorded in §9 as a seed; not fixed here.

---

## 2. The search that was run — this is the deliverable as much as the fix list

All four established passes were run over this tree on 2026-09-03. **What each pass found is
recorded, including the passes that found nothing**, so the next milestone inherits evidence rather
than a number.

### Pass (i) — same rule block

```
rg -n "color:\s*var\(--(danger|success|warning)[,)]" src/styles      # 54 matches, 22 files
rg -n "color:\s*var\(--(accent|merged)\)"            src/            # 7 accent (P105), 0 merged
```

**Yield: 54.** Note the pattern deliberately ends in `[,)]` so it does **not** swallow `--danger-text`
or `--danger-strong`; the looser `\b` form returns 64 and is the wrong number to quote. Also note it
**does** match `border-color: var(--hue)` (3 of the 54: `identity-menu.css:39`,
`diff-content.css:132`, `forge-pr.css:561`) — those are graphics, judged at 3:1, and they are part of
why the recorded 48 did not reproduce. **`--merged` is never used as ink anywhere** — 0 matches,
recorded so it is not re-run.

### Pass (ii) — multiline descendant combinator (ink and tint declared in different rules)

```
rg -nU --multiline-dotall "\.[\w-]+\s+\.[\w-]+\s*\{[^{}]*color:\s*var\(--(danger|success|warning)"
```

**Yield: 6 within the 54, none new** — `ai-dock-log.css:165/169`, `git-banner.css:181/185`,
`commit-box.css:45`, `graph-filter.css:93`, `checks-panel.css:119`, `settings-dev.css:82`. Pass (ii)
adds no site here, but it is what **resolved the backdrops**: for these six the tint/fill lives in the
parent rule, so pass (i) alone cannot rule on them. Recorded as run.

### Pass (iii-a) — unqualified child whose only parent decides the backdrop (found by READING, not grepping)

Every container in §4 was opened and each child's ink checked. **Yield: 1 new site with a distinct
verdict** — `commit-panel.css:98-100` is a **single declaration over two selectors**
(`.commit-signature-warn .commit-signature-icon, .commit-signature-warn .commit-signature-text`). The
icon half is a compliant glyph; the **text half is read text at 4.41 dark**. No selector-shaped grep
distinguishes them, because they share one declaration. This is the pattern P106 named and it bit
again.

Also confirmed by reading: `.right-panel` is `--bg-1` (`graph-canvas.css:66`), the settings body is
`--bg-0` (`settings-shell.css:290`), `.dialog-card` is `--bg-1` (`dialogs.css:20`) and
`.onboarding-card`/`.stale-card` extend it, and the AI log surface is `--ai-dock-log-bg` = `--bg-0`
(`ai-dock.css:32`). Four Bucket-A verdicts depend on those four facts.

### Pass (iii-b) — custom-property indirection

```
rg -n -- "--h\b|--badge-good|--badge-warn|--badge-unknown|--warn[,)]" src/
```

**Yield: 11 sites, 6 of them new to this milestone and 2 of them defects.**
- `--h` (4 sites) → all resolve to hue-over-own-tint → **P107's**, excluded.
- `--badge-good` / `--badge-warn` (6 sites) → **byte-identical to `--success` / `--danger` in both
  themes** (`tokens-and-base.css:72-73`, `171-172`), confirmed numerically: the measured rows for
  `--badge-good` and `--success` are identical to the last digit (§3). Two are own-tint (P107's), four
  are in P108 scope, and **one of those four is the `.commit-signature-warn` read text that fails**.
- `--badge-unknown` (1 site) → **byte-identical to `--text-3`** (`#6b7280` / `#8a919e`). Not a hue, but
  it is an alias-hidden `--text-3` at **2.96 light**, i.e. an escape from a family recorded as CLOSED.
- **`--warn` (1 site) → the token does not exist.** `rg -n -- "--warn:" src/styles` returns **0**
  (`--badge-warn:` does not match — one hyphen, not two). So `settings-legacy-sections.css:134`
  `color: var(--warn, #b8860b)` has *always* painted the literal `#b8860b`, in both themes, since it
  was written. P106 catalogued this line as one of five known `var(--x, #hex)` fallbacks but nobody
  checked whether the property resolved. **A `var()` fallback is not a fallback if the property is
  undefined — it is the value.**

**The hue alphabet for any future search is therefore
`danger|success|warning|merged|accent|badge-good|badge-warn|badge-unknown|h` — plus a check that
every custom property named in a `var()` is actually defined.**

### Pass (iv) — imperative canvas rendering

```
rg -n "badgeGood|badgeWarn|badgeUnknown|theme\.(danger|success|warning)" src/graph --glob '!*.test.ts'
```

**Yield: 7 draw sites, all glyph/graphic, all compliant, none fixed** — `forgeBadges.ts:70` (CI check
glyph), `:73` (CI x glyph), `:75` (pending dot), `drawRowText.ts:139` (verification check), `:158`
(verification triangle), `drawWip.ts:54` (WIP ring stroke), plus `colors.ts:198`'s `badgeUnknown`
read. Measured against both canvas backdrops (default `--graph-canvas-bg` = `--bg-0`, and the Bonsai
graph style's `#17140f` / `#f4efe6`) in §3. **The canvas contributes 0 fixes — but only because it was
measured.** Pass (iv) exists because "the search could not see a whole class" is how three prior
counts went wrong; recording a zero yield is the point.

### Population

| Pass | In-scope yield |
|---|---|
| (i) same rule block | 54 raw → **49** after removing 5 P107-owned own-tint sites |
| (ii) descendant combinator | 0 new sites (6 backdrops resolved) |
| (iii-a) tinted/neutral parent, read from components | **+1 verdict** (the split `.commit-signature-warn` declaration) |
| (iii-b) alias + undefined-property indirection | **+5** (`--badge-*` ×4 in scope, `--warn` ×1) |
| (iii-c) `--badge-unknown` / `--text-3` alias | **+1** (Bucket C, adjacent) |
| (iv) imperative canvas | **+6** (all keeps) |
| **Total call sites under verdict** | **62** (61 declarations) |

---

## 3. Method, and the calibration that licenses every number below

Measured in the running harness (`http://localhost:1420`, `VITE_MOCK_IPC=1`), **viewport set
explicitly to 1440×900** (a hidden Browser pane reports `innerWidth/innerHeight = 0`), both themes,
2026-09-03. An off-screen probe resolves each token and each `color-mix()` through the browser's own
mixing model; translucent results are source-over composited onto the resolved backdrop in sRGB; WCAG
2.x relative luminance and contrast follow. Themes are read by toggling `data-theme` on
`documentElement` and restoring it; the Bonsai canvas backdrop by toggling `data-graph-style`.

**P107's serialization trap was handled explicitly.** Chrome serialises some `color-mix()` results as
`color(srgb 0.898 0.325 0.294 / 0.14)` (0–1 components) and others as `rgba(229, 83, 75, 0.14)`
(0–255). The `color(` form is parsed in its own branch. A single numeric regex over both produces
near-black tints and inflates every dark figure by ≈0.8, which invalidated an entire earlier run.

**Calibration — 25 independently recorded historical values re-derived before any new measurement.
25/25 reproduce exactly.**

| Check | Recorded (source) | This run |
|---|---|---|
| `--danger` on `--bg-0` / `--bg-1` / `--bg-2` / `--selection` | 4.80/4.93 · 4.41/4.60 · 3.89/4.24 · 3.05/3.96 (ui-ref §7) | identical ✓ |
| `--success` on the same four | 6.24/5.08 · 5.73/4.74 · 5.06/4.37 · 3.96/4.08 (ui-ref §7) | identical ✓ |
| `--warning` on the same four | 7.92/4.87 · 7.28/4.54 · 6.42/4.19 · 5.03/3.91 (ui-ref §7) | identical ✓ |
| `--danger-strong` / `--success-strong` / `--warning-strong` on the same four | ui-ref §2 (12 figures) | identical ✓ |
| `--accent-strong` on `--bg-1` / `--selection` / **14% accent tint over `--bg-1`** | 7.13/5.81 · 4.93/5.01 · 5.85/4.87 | identical ✓ — **including the BASE-sensitive one** |
| `--danger`/`--success`/`--warning` on own 14% tint over `--bg-2` | 3.35/3.48 · 4.07/3.66 · 4.96/3.53 (ui-ref §2) | identical ✓ |
| `--text-2` on `--bg-1` / `--bg-2` / `--bg-3` / `--selection` | 7.25/7.45 · 6.40/6.87 · 5.56/6.32 · 5.01/6.42 (ui-ref §2) | identical ✓ |
| `--text-1` on `--selection` | 9.36/13.29 | identical ✓ |
| pre-fix hex-literal count outside `tokens-and-base.css` | 12 (P106, corrected) | **12** ✓ re-measured, not inherited |

**Every ratio in this contract names its composited base**, per `ui-reference.md` §2's BASE rule.
Format is **dark / light**.

### 3.1 The matrix (all figures new-measured; the `--bg-3` column and the canvas columns are new to the design system)

| Ink | `--bg-0` | `--bg-1` | `--bg-2` | `--bg-3` | `--selection` | canvas default | canvas bonsai |
|---|---|---|---|---|---|---|---|
| `--danger` ≡ `--badge-warn` | 4.80 / 4.93 | **4.41** / 4.60 | **3.89 / 4.24** | **3.38 / 3.90** | **3.05 / 3.96** | 4.80 / 4.93 | 4.96 / 4.30 |
| `--success` ≡ `--badge-good` | 6.24 / 5.08 | 5.73 / 4.74 | 5.06 / **4.37** | **4.39 / 4.02** | **3.96 / 4.08** | 6.24 / 5.08 | 6.45 / 4.43 |
| `--warning` | 7.92 / 4.87 | 7.28 / **4.54** | 6.42 / **4.19** | 5.58 / **3.85** | 5.03 / **3.91** | 7.92 / 4.87 | 8.19 / 4.25 |
| `--merged` | 5.30 / 5.05 | 4.87 / 4.71 | **4.30 / 4.34** | **3.73 / 4.00** | **3.36 / 4.06** | — | — |
| `#b8860b` (the undefined-`--warn` literal) | 5.46 / **3.25** | 5.01 / **3.04** | **4.43 / 2.80** | — | **3.47 / 2.62** | — | — |
| `--badge-unknown` ≡ `--text-3` | 3.68 / 3.17 | **3.38 / 2.96** | **2.98 / 2.73** | — | — | 3.68 / 3.17 | — |
| **`--danger-strong`** | 8.29 / 6.44 | 7.62 / 6.01 | 6.72 / 5.55 | **5.84 / 5.10** | 5.27 / 5.18 | 8.29 / 6.44 | 8.57 / 5.62 |
| **`--success-strong`** | 8.98 / 7.39 | 8.26 / 6.90 | 7.29 / 6.36 | **6.33 / 5.85** | 5.71 / 5.94 | — | — |
| **`--warning-strong`** | 9.13 / 7.13 | 8.39 / 6.65 | 7.40 / 6.14 | **6.43 / 5.64** | 5.80 / 5.73 | 9.13 / 7.13 | 9.44 / 6.22 |
| `--text-2` | 7.89 / 7.98 | 7.25 / 7.45 | 6.40 / 6.87 | 5.56 / 6.32 | 5.01 / 6.42 | — | — |

Bold = below **4.5:1**. Two diff-line tint backdrops are also used by one Bucket-B row:
`--danger` on a **12% `--danger` tint over `--bg-0`** = 4.23 / 4.13, on a **12% `--success` tint over
`--bg-0`** = 4.03 / 4.19.

**Read across the `--warning` row.** Dark `--warning` is a fine letterform on every neutral; light
`--warning` clears 4.5:1 on **`--bg-0` only** (4.87), and its 4.54 on `--bg-1` has 0.04 of headroom,
which `ui-reference.md` §2 already instructs us to treat as a fail. That single row decides eight
Bucket-A verdicts.

---

## 4. Enumeration — bucket, verdict and per-state backdrop for every call site

**Backdrops are resolved per state** (rest / hover / active / selected / disabled). P106 is the
argument: its `D` badge measured 4.41 at rest and **3.05 selected** — the worst figure was the state
the user is actually in. Five rows below fail *only* in a non-rest state, and a rest-only
measurement would have passed all five.

**Sole-carrier column:** does the hue alone carry information the user needs? If a word, glyph shape,
`aria-*` attribute or adjacent label carries it independently, the hue is decoration and the site is
judged at the 3:1 graphics bar.

### Bucket A — hue as READ TEXT over a neutral surface, failing 4.5:1 on at least one live state. **FIX all 27.**

| # | File:line | Selector | Ink now | Backdrops by state (base stated) | Measured worst | Sole carrier? | Fix |
|---|---|---|---|---|---|---|---|
| A1 | `dialogs.css:107` | `.dialog-error` | `--danger` | `.dialog-card` `--bg-1` (rest only) | **4.41** / 4.60 | yes — it is the error sentence | `--danger-strong` → 7.62 / 6.01 |
| A2 | `dialogs.css:120` | `.hook-output-heading` | `--danger` | `.dialog-card` `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` |
| A3 | `dialogs.css:238` | `.op-worktree-warning` | `--danger` | `.dialog-card` `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` (see §8 on the tone mismatch) |
| A4 | `dialogs.css:276` | `.branch-name-suggest-error` | `--danger` | `.dialog-card` `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` |
| A5 | `sidebar.css:259` | `.branch-create-error` | `--danger` | sidebar `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` |
| A6 | `updates.css:78` | `.update-error` | `--danger` | `.update-*` panel `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` |
| A7 | `onboarding.css:139` | `.onboarding-error` | `--danger` | `.dialog-card.onboarding-card` `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` |
| A8 | `search.css:133` | `.commit-search-error` | `--danger` | search bar / history panel `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong` |
| A9 | `search.css:126` | `.commit-search-truncated` | `--warning` | search bar `--bg-1` | 7.28 / **4.54** | yes — "Showing first N" | `--warning-strong` → 8.39 / 6.65 |
| A10 | `composer.css:144` | `.composer-message-hint` | `--warning` | composer body `--bg-1` | 7.28 / **4.54** | yes | `--warning-strong` |
| A11 | `commit-box.css:358` | `.commit-counter-over` | `--warning` | `.commit-box` `--bg-1` | 7.28 / **4.54** | yes — the count *is* the message | `--warning-strong` |
| A12 | `commit-panel.css:225` | `.file-count-del` | `--danger` | row `--bg-1` → hover `--bg-2` → **`.diff-tree-selected` `--selection`** | **3.05 / 3.96** selected | yes — a numeral | `--danger-strong` → worst 5.27 / 5.18 |
| A13 | `commit-panel.css:221` | `.file-count-add` | `--success` | same three | **3.96 / 4.08** selected | yes | `--success-strong` → worst 5.71 / 5.94 |
| A14 | `commit-panel.css:98-100` | `.commit-signature-warn .commit-signature-text` | `--badge-warn` ≡ `--danger` | `.commit-panel` in `.right-panel` `--bg-1` | **4.41** / 4.60 | yes — signature verdict prose | split the declaration; text → `--danger-strong` |
| A15 | `forge-pr.css:283` | `.pr-stat-del` | `--danger` | `.pr-detail` in `.right-panel` `--bg-1` | **4.41** / 4.60 | yes — `−N` | `--danger-strong` |
| A16 | `forge-pr.css:560` | `.btn-secondary-danger:hover`, `:focus-visible` | `--danger` | `.btn-secondary` `--bg-2` → **hover `--bg-3`** (the only state where the ink applies) | **3.38 / 3.90** | yes — a button label | `--danger-strong` → **5.84 / 5.10** |
| A17 | `ai-dock-log.css:192` | `.ai-run-queue-reason` | `--danger` | log `--bg-0` rest (4.80 ✓) → **row hover `--bg-2`** | **3.89 / 4.24** hover | yes — why the run was blocked | `--danger-strong` |
| A18 | `ai-assets.css:255` | `.stale-force-hint` | `--warning` | `.dialog-card.stale-card` `--bg-1` → row hover `--bg-2` | 6.42 / **4.19** hover | yes | `--warning-strong` |
| A19 | `ai-assets.css:265` | `.stale-outcome-warn` | `--warning` | same | 6.42 / **4.19** hover | yes | `--warning-strong` |
| A20 | `ai-assets.css:269` | `.stale-outcome-error` | `--danger` | same | **3.89 / 4.24** hover | yes | `--danger-strong` |
| A21 | `settings-dev.css:82` | `.dev-status-write-failed .dev-status-state` | `--danger` | `.dev-status-card` `--bg-1` | **4.41** / 4.60 | yes — **this is P91 §8.4's "▲ Not writing"** | `--danger-strong` → 7.62 / 6.01 |
| A22 | `dialogs-forms.css:179` | `.wt-copy-warn` | `var(--warning, var(--danger))` | `.dialog-card` `--bg-1` | 7.28 / **4.54** | yes | `--warning-strong`, and **drop the dead fallback** |
| A23 | `settings-legacy-sections.css:134` | `.settings-config-warn` | **`var(--warn, #b8860b)` — `--warn` undefined, so `#b8860b`** | settings body `--bg-0`; also `--bg-2` inside inputs | **5.46 / 3.25** | yes | `--warning-strong` → 9.13 / 7.13 |
| A24 | `context-menu.css:100` | `.context-menu-item[data-tone='danger']` | `--danger` | menu `--bg-2` rest → **hover/`:focus-visible` `--selection`** | **3.05 / 3.96** hover | yes — the only pre-click signal that "Delete branch…" is destructive | `--danger-strong` → 6.72 / 5.55 rest, 5.27 / 5.18 hover |
| A25 | `diff-content.css:133` | `.diff-hunk-discard-btn:hover` | `--danger` | button's own `--bg-1` (unchanged on hover) | **4.41** / 4.60 | yes — a destructive button label | `--danger-strong` |
| A26 | `commit-box.css:49` | `.section-action-discard:hover:not(:disabled)` | `var(--danger, #e5534b)` | section header `--bg-1` | **4.41** / 4.60 | yes | `--danger-strong`, no fallback |
| A27 | `commit-box.css:68` | `.section-action-ai-bulk[data-state='cancel']` | `var(--danger, #e5534b)` | section header `--bg-1` | **4.41** / 4.60 | yes — "Cancel" | `--danger-strong`, no fallback |

**A26/A27 are P105's own neighbours.** `.section-action-ai-bulk:hover` at `commit-box.css:63-64` was
moved to `--accent-strong` by P105 (B13) for the identical reason, with a comment three lines above
A27 explaining why. The danger siblings in the same rule block were not touched, because P105's grep
was `color: var(--accent)`. **This is the strongest single piece of evidence that the three-way split
in §1 is a partition of one defect, not three defects.**

### Bucket B — hue as GLYPH, BORDER or BAR over a neutral surface, ≥3:1 on every live state. **KEEP all 28, untouched.**

| # | File:line | Selector | Ink | Backdrops by state | Worst measured | Non-colour carrier |
|---|---|---|---|---|---|---|
| B1 | `ai-dock-log.css:165` | `.ai-run-queue-row[data-status='ready'] .ai-run-queue-glyph` | `--success` | `--bg-0` → hover `--bg-2` | 5.06 / 4.37 | glyph shape + status text |
| B2 | `ai-dock-log.css:169` | `…[data-status='failed'] .ai-run-queue-glyph` | `--danger` | `--bg-0` → hover `--bg-2` | 3.89 / 4.24 | glyph shape + `.ai-run-queue-reason` |
| B3 | `git-banner.css:46` | `.git-banner-icon` | `--warning` | `.git-banner` `--bg-1` | 7.28 / 4.54 | the banner sentence |
| B4 | `git-banner.css:181` | `.git-banner-cap-works .git-banner-cap-glyph` | `--success` | `.git-banner-cap*` `--bg-0` | 6.24 / 5.08 | `.git-banner-cap-leader` word (its own comment says so) |
| B5 | `git-banner.css:185` | `.git-banner-cap-broken .git-banner-cap-glyph` | `--danger` | `--bg-0` | 4.80 / 4.93 | same leader word |
| B6 | `commit-box.css:45` | `.section-action-discard .section-action-glyph` | `--danger` | section header `--bg-1` | 4.41 / 4.60 | label stays `--text-2` (comment at `:38`) |
| B7 | `commit-box.css:138` | `.row-action-discard:hover:not(:disabled)` | `--danger` | `.row-action:hover` **`--bg-3`** | **3.38 / 3.90** | `aria-label`; thinnest keep in the app |
| B8 | `commit-box.css:258` | `.commit-note-glyph` | `--warning` | `.commit-box` `--bg-1` | 7.28 / 4.54 | `.commit-note` text is `--text-2` |
| B9 | `commit-box.css:262` | `.commit-note-glyph-ok` | `--success` | `--bg-1` | 5.73 / 4.74 | same |
| B10 | `partial-staging.css:54` | `.diff-line:hover .diff-gutter-discard-btn`, `:hover`, `:focus-visible` | `--danger` | `--bg-0`; **12% `--danger` tint / `--bg-0`** on deleted lines; 12% `--success` tint / `--bg-0` on added lines | 4.03 / 4.13 | `aria-label`. **Note:** the deleted-line state is technically hue-over-own-tint (P107's shape) but is a glyph and clears 3:1 — recorded, not re-opened |
| B11 | `checks-panel.css:156` | `.checks-glyph--good` | `--badge-good` | `.checks-panel` `--bg-1` → `.checks-row:hover` `--bg-2` | 5.06 / 4.37 | `.checks-row-name` + `.checks-row-desc` |
| B12 | `checks-panel.css:159` | `.checks-glyph--warn` | `--badge-warn` | same | 3.89 / 4.24 | same |
| B13 | `checks-panel.css:162` | `.checks-glyph--pending` | `--warning` | same | 6.42 / 4.19 | same |
| B14 | `commit-panel.css:96` | `.commit-signature-good .commit-signature-icon` | `--badge-good` | `.right-panel` `--bg-1` | 5.73 / 4.74 | `.commit-signature-text` beside it |
| B15 | `commit-panel.css:98-100` | `.commit-signature-warn .commit-signature-icon` (the icon half of A14) | `--badge-warn` | `--bg-1` | 4.41 / 4.60 | keeps `--badge-warn` after the split |
| B16 | `settings-dev.css:25` | `.dev-warning-glyph` | `--warning` | settings body `--bg-0`; `.dev-status-card` `--bg-1` | 7.28 / 4.54 | 3px `--warning` bar + the prose |
| B17 | `settings-dev.css:79` | `.dev-status-glyph-danger` | `--danger` | `.dev-status-card` `--bg-1` | 4.41 / 4.60 | the ▲ shape (its own comment at `:72`) |
| B18 | `settings-legacy-sections.css:216` | `.settings-account-state.is-connected .settings-account-dot` | `--success` | settings body `--bg-0` | 6.24 / 5.08 | the state word beside it |
| B19 | `graph-filter.css:98` | `.graph-filter-chip-glyph-warning` | `--warning` | `.graph-filter-chip` `--bg-2` → hover/open `--bg-3` | 5.58 / **3.85** | ⚠ shape + chip label |
| B20 | `identity-menu.css:39` | `.identity-avatar[data-identity-state='unset'\|'unreadable']` **border** | `--warning` | avatar `--bg-2` inside header `--bg-1` | 4.19 / 4.54 | `?` glyph + accessible name (its own comment) |
| B21 | `diff-content.css:132` | `.diff-hunk-discard-btn:hover` **border** | `--danger` | `--bg-1` | 4.41 / 4.60 | pairs with A25's label |
| B22 | `forge-pr.css:561` | `.btn-secondary-danger:hover` **border** | `--danger` | `--bg-3` | 3.38 / 3.90 | pairs with A16's label |
| B23 | `forgeBadges.ts:70` | CI check glyph | `theme.badgeGood` | canvas `--bg-0` / bonsai `#17140f`,`#f4efe6`; selected-row fill ≈ `--selection` | 3.96 / 4.08 | `✓` shape (§6.1 glyph vocabulary) |
| B24 | `forgeBadges.ts:73` | CI failure glyph | `theme.badgeWarn` | same | **3.05 / 3.96** | `✕` shape |
| B25 | `forgeBadges.ts:75` | CI pending dot | `theme.warning` | same | 5.03 / 3.91 | `•` shape + PR pill label |
| B26 | `drawRowText.ts:139` | verification check badge | `theme.badgeGood` | canvas | 6.24 / 4.43 | badge shape + tooltip |
| B27 | `drawRowText.ts:158` | verification warning triangle | `theme.badgeWarn` | canvas | 4.80 / 4.30 | triangle shape + white `!` |
| B28 | `drawWip.ts:54` | WIP row ring stroke | `theme.warning` | canvas | 7.92 / 4.25 | the WIP row's own label |

### Bucket C — not a hue, found by the alias pass, adjacent to a Bucket-A fix. **FIX 1.**

| # | File:line | Selector | Ink | Backdrop | Measured | Verdict |
|---|---|---|---|---|---|---|
| C1 | `commit-panel.css:103` | `.commit-signature-unknown .commit-signature-icon` | `--badge-unknown` **≡ `--text-3`** | `.right-panel` `--bg-1` | **3.38 / 2.96** | **FIX → `--text-2`** (7.25 / 7.45). Below the **3:1 graphics** bar in light, so it fails even as a pure glyph. Invisible to P101's exhaustive `color: var(--text-3)` audit purely because of the alias; the `--text-3` family is recorded CLOSED in `ui-reference.md` §2 and this is an escape from it. It sits between B14 and A14 in the same rule block — the P105 precedent (`.asset-chip-sync`/`-drifted`) is to fix same-block neighbours in the same diff rather than file a milestone for one line. **Decision D2 in §10 if the orchestrator disagrees.** |

`--badge-unknown` is also read by the canvas (`colors.ts:198` → `theme.badgeUnknown`), where it draws
on `--bg-0` at 3.68 / 3.17 and clears 3:1. **The token stays; only the CSS use moves.**

### Bucket D — hue as read text over a neutral surface that PASSES on every live state. **KEEP 6, with the margin recorded.**

| # | File:line | Selector | Ink / base | Measured | Margin over 4.5 |
|---|---|---|---|---|---|
| D-a | `ai-dock-log.css:44` | `.ai-log-line[data-kind='stderr']` | `--danger` / log `--bg-0` (no hover rule on log lines — verified) | 4.80 / 4.93 | +0.30 / +0.43 |
| D-b | `settings-legacy-sections.css:140` | `.settings-config-error` | `--danger` / settings body `--bg-0` | 4.80 / 4.93 | +0.30 / +0.43 |
| D-c | `identity-menu.css:124` | `.settings-profile-danger` | `--danger` / settings body `--bg-0` | 4.80 / 4.93 | +0.30 / +0.43 |
| D-d | `settings-legacy-sections.css:465` | `.settings-ai-status-ok` | `--success` / `--bg-0` | 6.24 / 5.08 | +1.74 / +0.58 |
| D-e | `forge-pr.css:278` | `.pr-stat-add` | `--success` / `.right-panel` `--bg-1` | 5.73 / 4.74 | +1.23 / +0.24 |
| D-f | `composer.css:75` | `.composer-note-uncommitted` | `--warning` / `.composer-note*` `--bg-0` | 7.92 / 4.87 | +3.42 / +0.37 |

**These six are compliant and are NOT defects.** They are listed because a bare count would otherwise
imply they were missed, and because three of them (D-b, D-c, D-e) sit beside a Bucket-A sibling that
*is* changing — see decision **D1** in §10, where I recommend moving them too, for cohesion rather
than compliance.

---

## 5. Live-match confirmation — a grep proves a declaration exists, not that it renders

`ui-reference.md` §2's second failure mode: `.settings-toggle-btn.is-active` was credited as a
shipped P105 fix on a rule that had matched nothing since `7354aca`. **Every Bucket-A and Bucket-C
selector below has a confirmed live render site in `src/**/*.tsx`.**

| Site | Rendered by |
|---|---|
| A1/A2/A4 `.dialog-error`, `.hook-output-heading`, `.branch-name-suggest-error` | `CloneDialog.tsx`, `ChangelogDialog.tsx`, `PromptDialog.tsx`, `RemoteEditDialog.tsx`, `TagCreateDialog.tsx`, `RebasePlanEditor.tsx`, `ProfileManager.tsx`, `WhatChangedDialog.tsx`, `WorktreeCreateDialog.tsx`, `WorktreeCopyCandidates.tsx`, `SubmoduleDialogs.tsx`, `AgentAssetEditor.tsx`; `HookOutputDialog.tsx`; `BranchNameSuggest.tsx` |
| A3 `.op-worktree-warning` | `ProposedOpDialog.tsx` |
| A5 / A6 / A7 | `sidebar/BranchesSection.tsx` · `UpdateDialog.tsx` · `OnboardingSteps.tsx` |
| A8 / A9 | `CommitSearchBar.tsx`, `HistorySearchPanel.tsx` |
| A10 / A11 | `Composer*` message pane · `CommitBox.tsx` (covered by `CommitBox.test.tsx`) |
| A12 / A13 | `DiffFileTree.tsx:189`, `DiffBrowser.tsx:412`, `prPanel/PrFileRow.tsx:45` (three render sites; the `--selection` state is `.diff-tree-selected`, `diff-browser.css:130`) |
| A14 / B14 / B15 / C1 | `.commit-signature*` — commit-details signature line (P58c) |
| A15 | `PrDetailView.tsx` |
| A16 | `PrActionsBar.tsx` (`btn-secondary-danger`) |
| A17 | `AiRunQueue.tsx` |
| A18/A19/A20 | `StaleBranchesDialog.tsx` |
| A21 | `settings/DevSessionStatus.tsx` (covered by `settings/dev.test.tsx`) |
| A22 | `WorktreeCreateDialog.tsx` |
| A23 | `settings/CuratedConfigControl.tsx`, `settings/IdentityProfileCard.tsx` |
| A24 | `workspaceMenusBranch.ts`, `workspaceMenusCommit.ts`, `workspaceMenusRows.ts`, `workspaceMenusTag.ts`, `settings/SettingsAccountCard.tsx` (`tone: 'danger'`) |
| A25 | `DiffView.tsx`, `DiffViewSplit.tsx` |
| A26 / A27 | `StatusSection.tsx` · `BulkAiResolveButton.tsx` |

**No fix in this contract is applied to a rule without a live match.** `.ai-log-line[data-kind=…]` is
rendered dynamically (`AiActivityLog.tsx:119`, `data-kind={line.kind}` with `'stderr'` in the
`AiRunLogKind` union) and `.checks-glyph--<tone>` from a template string
(`checksPanel/CheckRow.tsx:19`) — both confirmed by reading the components, not by a class-literal
grep, which would have reported them dead.

---

## 6. Token decisions

**No new token.** The `--*-strong` family covers all 27 hue fixes and `--text-2` covers C1.

- `--danger-strong` — 18 new declarations (A1–A8, A12, A14–A17, A20, A21, A24–A27).
- `--warning-strong` — 7 new declarations (A9, A10, A11, A18, A19, A22, A23).
- `--success-strong` — 1 new declaration (A13).
- `--text-2` — 1 (C1).

**Rejected, recorded so they are not re-derived:**
- **A new "muted danger" token** for A16's `--bg-3` hover. Unnecessary: `--danger-strong` measures
  **5.84 / 5.10** there.
- **`--text-1` for everything.** Rejected for the same reason P106 rejected it: `.file-count-add` /
  `-del` and the signature verdict are *scanned by colour* before they are read. Only states demote to
  neutral; an identity keeps its hue and flips the ink.
- **`--*-text` for anything here.** The trap in `ui-reference.md` §2: those are inks for solid fills.
  A hypothetical `--warning-text` resolves to `--bg-0` and measures **1.09 / 1.07** on `--bg-1`.
- **A hue border as the carrier.** 35% hue edges measure 1.58 / 1.69. B20/B21/B22 keep their borders
  as **decoration paired with a word or a shape**, never as the identity.

### 6.1 One revision to `ui-reference.md` §2 that this milestone forces

§2 currently records the `--*-strong` family's **fixed-family minimum as 5.18** (`--danger-strong`,
light, on `--selection`). That minimum was computed over the backdrops P105/P106 reached, which did
**not include `--bg-3`**. A16 puts `--danger-strong` on `--bg-3` for the first time, where it measures
**5.84 / 5.10**.

**On landing, the family minimum becomes 5.10** (`--danger-strong`, light, `--bg-3`) — still 0.60
above the text bar, still inside the band that makes the four tokens read as one system. The `--bg-3`
row is measured for all four `-strong` tokens in §3.1 and has been added to `ui-reference.md` §2 now,
qualified as forward-looking, so the measurement is not lost between contract and landing (that is
exactly the transcription failure mode §2 records). **AC6 is what closes it.**

---

## 7. The fix — CSS only, one selector split, zero DOM change

**Files touched (10 CSS files + 0 TS/TSX):** `dialogs.css`, `dialogs-forms.css`, `sidebar.css`,
`updates.css`, `onboarding.css`, `search.css`, `composer.css`, `commit-box.css`, `commit-panel.css`,
`forge-pr.css`, `ai-dock-log.css`, `ai-assets.css`, `settings-dev.css`,
`settings-legacy-sections.css`, `context-menu.css`, `diff-content.css`. (16 — no component file, no
`src/graph/**`, no `src-tauri/**`, no `crates/**`, no `e2e/**`.)

**Recipe — one line each.** Replace the ink token; change nothing else. No background, no size, no
padding, no font-weight, no border, no new class, no markup.

```
color: var(--danger)  ->  color: var(--danger-strong)      (A1..A8, A12, A15..A17, A20, A21, A24..A27)
color: var(--warning) ->  color: var(--warning-strong)     (A9, A10, A11, A18, A19)
color: var(--success) ->  color: var(--success-strong)     (A13)
```

**Three sites need more than a token swap:**

1. **A14 + B15 — split one declaration into two rules.** Today
   `.commit-signature-warn .commit-signature-icon, .commit-signature-warn .commit-signature-text`
   share `color: var(--badge-warn)`. Split so the **icon keeps `--badge-warn`** (compliant glyph,
   4.41 / 4.60) and the **text takes `--danger-strong`** (7.62 / 6.01). Rule order: icon rule first,
   text rule second, matching the `-good` / `-unknown` blocks above them.
2. **A22, A23, A26, A27 — drop dead `var()` fallbacks while replacing the ink.**
   `var(--warning, var(--danger))` → `var(--warning-strong)`; `var(--warn, #b8860b)` →
   `var(--warning-strong)`; `var(--danger, #e5534b)` → `var(--danger-strong)`. **Also drop the two
   surviving dead fallbacks in Bucket B/D rows** — `commit-box.css:45` and `:138`
   (`var(--danger, #e5534b)` → `var(--danger)`, zero visual change) and
   `settings-legacy-sections.css:140` (`var(--danger, #d9534f)` → `var(--danger)`). Six hex literals
   leave `src/styles` for free.
3. **A24 — replace a comment that is now known to be false.** `context-menu.css:96-98` currently
   asserts that "`--danger` text stays legible on \[`--selection`\] in both themes". It measures
   **3.05 / 3.96**. Replace with the measured figures and the base, per the BASE rule.

**What must NOT change:** the 28 Bucket-B rows (including all of `src/graph/**`), the 6 Bucket-D rows
unless decision **D1** says otherwise, every `background`/`border`/`box-shadow` in the touched files,
the five P107-owned own-tint rows, and the seven P105-owned `color: var(--accent)` rows.

**Comment discipline (P106's third failure mode).** The fix's own comments must **not** contain the
literal strings `var(--danger)`, `var(--success)`, `var(--warning)` or any `#rrggbb`, or they will
inflate AC2 and AC5. Write "the base danger hue" in prose. Where a comment records a ratio it names
**ink, tint-and-percentage if any, and base**.

---

## 8. Themes, densities, states, motion, copy, a11y, destructive UX

- **Both themes.** Every figure in §3–§4 is dark / light and was read from both. No token is
  theme-conditional and **no `[data-theme='light']` block is added** — the `-strong` tokens already
  carry the per-theme value (lifted in dark, deepened in light).
- **Both densities.** Nothing here is a size, padding, height, font-size or line-height. `cozy` and
  `compact` (`--rp-row-h` 24 → 20px, `--rp-row-font` inherit → 12px) are untouched. A12/A13/A14/B11–B15
  live inside `--rp-*` scopes; at **both** densities the text stays below the WCAG large-text
  threshold, so the **4.5:1** bar applies in both and no density-specific figure exists.
- **All states, and this is where five of the fixes come from.** Rest, hover, `:focus-visible`,
  active/pressed, selected and disabled backdrops were resolved per row in §4. Focus rings are
  untouched (2px `--accent`, 1px offset, `:focus-visible` only). **Disabled:** A16, A25, A26, B7 and
  B10 are all `…:not(:disabled)` or hover-only rules — the disabled path continues to fall through to
  `--text-3`, which stays sanctioned (`ui-reference.md` §2, disabled role). **Long content:**
  A1/A3/A4/A5/A6 already carry `overflow-wrap: anywhere`; A17 and A9 truncate with ellipsis; the ink
  change cannot affect wrapping.
- **Motion:** none added, none removed. No transition is declared on `color` in any touched rule, so
  `prefers-reduced-motion` is unaffected.
- **Microcopy: no string changes.** Not one user-visible word moves. Two *code* comments change
  (§7.3, and A23's, which must record that `--warn` never existed).
- **Accessibility.** Every fixed row lands ≥4.5:1 on **every** live state in **both** themes; every
  kept row is ≥3:1 on every live state with an independent non-colour carrier named in §4.
  Colour is never the sole carrier anywhere in either bucket. Hit targets, roles and accessible names
  are untouched — this diff changes no DOM.
- **Destructive-action UX.** A24 is the only place in the app where hue is the *pre-click* signal that
  a menu item ("Delete branch…", "Hard reset…", "Remove account") is destructive, and it is currently
  **3.05:1 on the hovered item — i.e. weakest exactly while the pointer is on it.** `--danger-strong`
  takes that to 5.27 / 5.18 while keeping the red identity. A25/A26/B6/B7/B10 are discard controls;
  their ConfirmDialog guard is unchanged and remains the real protection. **No confirmation flow, verb
  or consequence sentence is altered by this milestone.**
- **Tone note, flagged not fixed.** A3 `.op-worktree-warning` is a *warning* rendered in the danger
  hue. Repainting it `--warning-strong` would change its meaning, so it keeps the danger family here.
  If the string is genuinely advisory, that is a separate copy/tone decision — **D3 in §10**.

---

## 9. Harness states (`VITE_MOCK_IPC=1`, `pnpm dev:mock`, port 1420)

The measurement in §3 needs no fixtures — it resolves tokens through an off-screen probe. **Visual
confirmation does**, and most of these surfaces are behind a route or a failure state.

**Reachable today, no new fixture:** A12/A13 (select a commit → file tree; hover and select a row to
reach the `--bg-2` and `--selection` states), A15/A16 (`?forge=auth`), A24 (right-click a branch row),
A25 (open a diff, hover a hunk header), A26/A27 (status section headers), B3–B5, B11–B13, B19,
B23–B28 (graph canvas, both `data-graph-style` values).

**Fixture states the increment needs** (add to `src/ipc/mock/`, no production code):
1. **Error** — a mock rejection that surfaces `.dialog-error`, `.branch-create-error`,
   `.commit-search-error` and `.update-error` with a **long** message (≥300 chars, no spaces in one
   token) to exercise `overflow-wrap: anywhere` at the new ink.
2. **Empty / pathological** — a commit with a `--badge-warn` signature verdict (A14/B15) and one with
   `unknown` (C1); a PR with `−0`/`+0` and one with 5-digit stats (A12/A13/A15).
3. **Loading / blocked** — an AI run queue row in `failed` with a long reason string (A17, hovered).
4. **`.dev-status-write-failed`** (A21) — the P91 status card in its write-failed state.

**Cannot be seen in the harness — USER CHECKPOINT items:**
- A21 under a *real* disk-write failure (the fixture proves the rule, the native app proves the
  pipeline).
- A9's truncation banner requires a >10k-commit search.
- Any judgement about how 11–12px text at these luminances reads at arm's length, in either density.
  The harness is headless; frame timing and perceptual judgements are not mine to declare.

---

## 10. Acceptance criteria, with the predicted post-fix residue stated up front

**Predicted residue, before implementation.** After P108, `src/styles` should hold **29 raw matches**
of `color:\s*var\(--(danger|success|warning)[,)]`, composed of **18 Bucket-B glyph/border keeps + 6
Bucket-D compliant-text keeps + 5 P107-owned own-tint keeps**. Pre-fix baseline **54**, measured in
this tree on 2026-09-03 — **not inherited from any prior contract**.

**Two rules that apply to every row below.** (1) Each row states whether it counts **declarations** or
**raw matches**; a raw-match count includes prose comments and `var()` fallbacks. (2) **Where a
prediction names `file:line`, the count is the criterion and the line numbers are not** — the fix's
own comments shift every line beneath them. P106's R4 predicted 5 declarations at
`193,198,203,216,227` and shipped them at `193,198,203,220,231`: the count was exact, the lines
drifted, and a reviewer checking lines would have failed a correct fix.

| # | Criterion | Counts | Pre-fix | Predicted post-fix |
|---|---|---|---|---|
| **AC1** | §4 has a bucket, a verdict, a per-state backdrop **with its base**, and a sole-carrier determination for all **62** call sites; no blank cell | — | — | 62 rows, 0 blanks |
| **AC2** | `rg -c "color:\s*var\(--(danger\|success\|warning)[,)]" src/styles` summed | **raw matches** | **54** | **29** |
| **AC3** | `rg -c -- "var\(--(danger\|success\|warning)-strong\)" src/styles` summed | **raw matches** | **5** (all `status-panel.css`) | **32** (5 + 27) |
| **AC4** | `rg -n -- "var\(--warn[,)]" src/styles` | raw matches | 1 | **0** |
| **AC5** | `rg -c "#[0-9a-fA-F]{6}" src/styles -g '!tokens-and-base.css'` summed | **raw matches, comments included** | **12** (re-measured, §3) | **6** = 5 prose-comment hex (`updates.css`, `forge-pr.css` ×2, `controls.css` ×2) + 1 live fallback (`ai-dock.css:65` `--ai-dock-attention`) |
| **AC6** | Every Bucket-A row measures **≥4.5:1 on every live state, both themes**, verified in the harness by §3's method. Worst post-fix figures recorded: **5.84 / 5.10** (A16, `--bg-3`) and **5.27 / 5.18** (A12/A24, `--selection`) | measured ratios | — | min **5.10** |
| **AC7** | Bucket B is untouched: `git diff --stat` shows **zero** changes under `src/graph/**` and none of the 22 DOM Bucket-B lines changed except the two dead-fallback cleanups explicitly listed in §7.2 | diff | — | 0 |
| **AC8** | C1 lands: `commit-panel.css` `.commit-signature-unknown .commit-signature-icon` is `var(--text-2)`; `rg -n -- "--badge-unknown" src/styles` = **2** (both in `tokens-and-base.css`); `src/graph/colors.ts:198` unchanged | declarations | 3 | **2** |
| **AC9** | Every Bucket-A/C selector has a named live render site (§5). **No fix is applied to a rule with no live DOM match**, and any selector that cannot be confirmed is recorded as unconfirmed **in this file and carried verbatim into `ui-reference.md`** | — | — | 28/28 confirmed |
| **AC10** | Zero DOM change: no element, class, attribute, role or accessible name added, removed or renamed. The only structural change is the A14/B15 selector split | diff | — | 1 split, 0 DOM |
| **AC11** | Harness screenshot in **both** themes of the fixture states in §9, showing A12/A13 in the `--selection` state and A24 in the hovered state — the two worst pre-fix figures | — | — | 2 screenshots |
| **AC12** | **USER CHECKPOINT.** Native window (`pnpm tauri dev`): the error and warning strings read as *errors* and *warnings* at arm's length in both themes and both densities, and the brighter dark inks do not read as "selected" or "highlighted" | — | — | **pending** |
| **AC13** | **USER CHECKPOINT.** Hue identity survives: `--danger-strong` still reads red-for-error, `--warning-strong` yellow-for-caution; the signature verdict line, the `+N/−N` counts and the destructive menu items stay mutually distinguishable, including under deuteranopia | — | — | **pending** |
| **AC14** | **USER CHECKPOINT.** A21 verified against a **real** write failure (read-only log directory), not the fixture — this string's whole job is to stay legible while the disk is failing | — | — | **pending** |

**AC12, AC13 and AC14 are USER CHECKPOINT items. They stay pending. No agent may self-declare them** —
the user's checkpoint authority does not reach this work, and no agent message closes them.

---

## 11. Flagged for the orchestrator — decisions I am not making silently

**D1 — the six compliant Bucket-D texts: keep, or move for cohesion? (my recommendation: move three.)**
D-b (`.settings-config-error`), D-c (`.settings-profile-danger`) and D-e (`.pr-stat-add`) each sit
**beside** a sibling that is changing: D-b's own file gains A23 two rules above it; D-c is the twin of
A24's account-removal tone; **D-e is literally the `+N` next to A15's `−N`, and shipping
`--success` beside `--danger-strong` in one 10px-gap flexbox will read as a colour bug.** All three
pass, so this is cohesion, not compliance. **Recommendation: move D-b, D-c and D-e to the `-strong`
form (making AC3 = 35 and AC2 = 26), and leave D-a, D-d and D-f alone.** If the orchestrator prefers
the minimal diff, keep all six and the numbers stand as written. **Either way, D-e must not be split
from A15.**

**D2 — Bucket C in this diff, or its own item?** C1 is one line, in a rule block two lines from A14,
and it is a **sub-3:1 glyph** — worse than anything in Bucket A relative to its own bar.
**Recommendation: fix it here** (P105's same-block precedent). The cost of not doing so is that a
closed-and-audited family (`--text-3`) keeps a known escape for another milestone.

**D3 — `.op-worktree-warning` (A3) is a warning painted in the danger hue.** I have kept the hue and
only fixed the contrast, because changing it changes meaning. Whether the string should be
`--warning-strong` is a copy/tone call, not a contrast one. **Recommendation: leave as danger now;
file the tone question as a NIT.**

**D4 — `commit-box.css:225` `color: var(--accent)` was not found in P105's §3.3 verdict table.** It is
`--accent`, therefore **P105's class and explicitly outside P108's boundary**, and I am **not**
claiming it. If it is genuinely unclassified it is a P105 residue item and should be routed to
whoever owns that residue — but *this* contract does not enumerate it, measure it or fix it.

**D5 — `forgeBadges.ts:51/58` hardcode `text: '#ffffff'` on the PR-state pill fills.** Ink-on-fill is
P102's class and canvas hex is not covered by any existing grep AC (all of them scope `src/styles`).
Recorded as a seed. **Not P108 scope; not claimed.**

**Overlaps I left with their owners, explicitly:** the five own-tint rows and the four `--h` rows are
**P107's** and carry P107 verdicts; the seven `color: var(--accent)` rows are **P105's**; the five
`status-panel.css` `-strong` rows and the two pending status-badge checkpoints are **P106's**; the
badge's missing accessible name and the `added`/`untracked` `A` collision are **P109's** — C1 touches
the same rule block as the signature icons but does **not** touch any status badge or its name.
