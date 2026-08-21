# P74 — Accessibility: toast-tone contrast + sidebar hit targets (UI contract)

Author: `ui-designer`. Date: 2026-08-20.
Origin: findings **S-2** and **S-3** of `docs/contracts/design-review-2026-08-19-p73-submodules.md`,
deferred out of P73 by the orchestrator.

> **Status: APPROVED and scheduled as P74** (user, 2026-08-19: "let's go with the followups", in
> answer to the orchestrator offering exactly these two deferred items). This contract WAS
> commissioned — by the P73 orchestrator session, which deferred S-2/S-3 out of P73 to avoid
> widening a submodule bug-fix into an app-wide restyle, then reopened them on the user's request.
>
> A concurrent session working in this repo replaced this banner with "PROPOSED — not approved,
> written unprompted during the P69j-1 design review"; that session had no visibility into the P73
> session's exchange with the user. Corrected here by the commissioning orchestrator. See `TODO.md`
> §P74 for the board entry.
Reference: `docs/contracts/ui-reference.md` §2 (tokens/contrast), §3 (≥24px control floor),
§7 (colour is never the sole carrier), §10.1 (toast dedupe), §11 (pill hue recipe).

**Scope.** CSS-only for the sidebar; CSS + one 3-line markup addition for toasts; one mock seam.
No new theme tokens. No IPC change. No behaviour change to toast queuing, dedupe, the 5-toast cap,
the 5 s auto-dismiss, or `sticky` (§10.1 is untouched — asserted in AC-14).

**Evidence.** All "before" numbers in this contract were measured live on 2026-08-20 in the mock
harness (`pnpm dev:mock`, `http://localhost:1420`, repo open, sidebar 240px) via
`javascript_tool` computed-style reads, in **both** themes. "After" numbers are computed from the
same tokens using the WCAG 2.x relative-luminance formula against the identical `color-mix()`
composites. The Browser pane is not compositing, so there is **no screenshot** and no
motion/frame evidence in this contract; none is needed — P74 adds no motion.

---

## Files touched

| File | Change |
|---|---|
| `src/styles/toasts-and-overlays.css` | Item 1 — rewrite the 4 tone rules + `.toast` + `.toast-dismiss`, add `.toast-glyph` |
| `src/components/Toasts.tsx` | Item 1 — add a module-local glyph map + one `<span className="toast-glyph" aria-hidden>` |
| `src/styles/sidebar.css` | Item 2 — `.sidebar-section-header`, `.sidebar-section-toggle`, `.sidebar-add`, `.list-filter-clear` |
| `src/styles/status-panel.css` | Item 2 — one line on `.tree-dir-toggle` (`align-self: stretch`) |
| `src/styles/empty-and-errors.css` | Item 2 — `.error-dismiss` geometry |
| `src/ipc/mock/handlers/toasts.ts` **(new, ~40 lines)** | harness seam `?toasts=demo` / `?toasts=long` |

`Toasts.tsx` is 55 lines and `sidebar.css` 284 — both well inside the ~500-line limit, so no split
is required. **Do not touch** `src/ipc/mock/persistence.ts`, `src/settings/**`, or any
`NumberSlider*` file (a concurrent session owns them).

---

## Item 1 — Toast tones: hue-as-text → text-1 label + hue edge + hue glyph

### 1.1 The defect

Every tone in `src/styles/toasts-and-overlays.css:28-50` sets `color: var(--<hue>)` on a 14% tint of
that same hue over `--bg-2`. That is the exact anti-pattern `ui-reference.md` §2 already forbids for
new surfaces, and **all four tones fail WCAG 1.4.3 (4.5:1) in at least one theme; `error` and `info`
fail in both.** P73 made this the app's worst-placed defect: the milestone's entire deliverable is
six multi-sentence remediation strings, they render only in `.toast-error`, and error toasts are
sticky with `role="alert"` — the least readable tone carries the most load-bearing text.

Measured live, 2026-08-20 (label vs its own toast background):

| Tone | Pair | Dark | Light | AA 4.5:1 |
|---|---|---|---|---|
| `.toast-error` | `--danger` on 14% `--danger` over `--bg-2` | **3.35:1** | **3.48:1** | ✗ / ✗ |
| `.toast-info` | `--accent` on 14% `--accent` over `--bg-2` | **3.68:1** | **3.38:1** | ✗ / ✗ |
| `.toast-success` | `--success` on 14% `--success` over `--bg-2` | **4.07:1** | **3.66:1** | ✗ / ✗ |
| `.toast-warning` | `--warning` on 14% `--warning` over `--bg-2` | **4.96:1** | **3.53:1** | ✓ / ✗ |

(The dark error/light warning figures reproduce §2's recorded 3.34 / 3.47 to rounding — same pairs,
same tokens, independently re-measured.)

### 1.2 Recommended recipe — **hue-filled left edge + hue glyph + `--text-1` label**

I recommend the **left-edge bar** variant of the §11 pill recipe, not the pill recipe verbatim.
Decisive reasoning:

- The §11 recipe carries hue in a **40% border all the way round**. On a 20px pill that reads as
  tone. On a 360×75px (worst case 360×250px) sticky rectangle a full-perimeter tinted hairline is
  measured at **1.69–2.25:1** against the surrounding chrome — it is invisible at that scale and is
  documented in §11 as decorative-only. Scaling a pill's hue carrier up to a toast is the one place
  the pill recipe genuinely does not transfer.
- A **3px hue bar on the leading edge** concentrates the same hue into a shape with enough area to
  read at a glance, is the established toast idiom on every desktop platform, and measures
  **3.35–4.96:1 dark / 3.38–3.69:1 light** against the toast's own tint — clearing WCAG 1.4.11's
  3:1 non-text bar in every tone × theme.
- The label moves to `--text-1`, which lands at **9.24–10.30:1 dark / 11.68–12.00:1 light**: a
  ~2.8× improvement on the worst case and comfortably past AAA, on the surface that carries P73's
  prose.

**Is the glyph needed?** *Yes — it is not optional.* Two independent reasons:
1. **WCAG 1.4.1.** Once the label is `--text-1`, the *only* remaining tone signal is the tint and
   the bar — both pure colour, both in a fixed position identical across tones. A bar that is always
   in the same place cannot distinguish error from warning for a colour-blind user. The prose does
   technically differ ("Couldn't check out…" vs "Checked out…"), but that requires *reading a
   sentence to learn the severity*, which is not triage. A glyph is shape, and shape is
   colour-independent.
2. **House precedent.** §7 and §11 both mandate hue + letter/word/glyph. Toasts would be the only
   remaining tone surface without one.

**Glyph set — all four already in the codebase's vocabulary, zero new font risk:**

| Tone | Glyph | Existing house use |
|---|---|---|
| error | `⊘` | `AiActivityPanel` blocked/refused chip — and every error toast *is* a refusal |
| warning | `⚠` | `submoduleBadges.ts`, `CommitPanel`, `AiRunQueue` |
| success | `✓` | `submoduleBadges.ts`, `CommitPanel`, `StatusConflictsSection` |
| info | `●` | `CommitPanel` neutral kind, `Sidebar` HEAD glyph |

Four distinct silhouettes (slashed ring / triangle / tick / solid disc). Deliberately **not** `⚠`
for error: `⚠` already means "failed" in the AI dock, and error-vs-warning is precisely the pair
that must stay separable without colour. Deliberately **not** `ℹ` (U+2139) for info: it has an
emoji presentation on Windows and macOS and would render in a fixed vendor colour, defeating `--h`.
Deliberately **not** `✕`, which is the dismiss button's glyph.

### 1.3 Exact CSS

```css
.toast {
  /* unchanged: position in flow, display:flex, align-items:flex-start, gap:8px,
     border-radius:6px, font-size:13px, overflow-wrap:anywhere               */
  --h: var(--accent);                 /* default; each tone overrides. §11 convention */
  padding: 8px 12px 8px 10px;         /* was 8px 12px — see geometry note */
  background: color-mix(in srgb, var(--h) 14%, var(--bg-2));
  border: 1px solid color-mix(in srgb, var(--h) 35%, var(--bg-2));
  border-left: 3px solid var(--h);
  color: var(--text-1);
}

.toast-error   { --h: var(--danger); }
.toast-success { --h: var(--success); }
.toast-warning { --h: var(--warning); }
.toast-info    { --h: var(--accent); }

.toast-glyph {
  flex: none;
  width: 14px;
  font-size: 13px;
  line-height: 1.45;     /* same line box as .toast-text → aligns on line 1 */
  text-align: center;
  color: var(--h);
}

.toast-dismiss {
  flex: none;
  width: 24px;
  height: 24px;
  margin: -3px -4px -3px 0;   /* optically centres the 24px box on line 1 and
                                 reclaims 4px of the text column; the box stays
                                 inside the toast (top edge at 6px)            */
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: inherit;             /* now --text-1, not the hue */
  font-size: 14px;
  line-height: 1;
  cursor: pointer;
}

.toast-dismiss:hover { background: color-mix(in srgb, currentcolor 12%, transparent); }
.toast-dismiss:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
```

The four per-tone `background` / `border` / `color` declarations at lines 28-50 collapse into the
single `--h` line each. **No hardcoded hex, no new token.**

### 1.4 Markup (`Toasts.tsx`)

Module-local (deliberately **not** exported — an exported non-component would trip
`react-refresh/only-export-components`, the same constraint that produced
`sidebar/submoduleBadges.ts` in P73):

```
const TONE_GLYPH: Record<ToastTone, string> = { error: '⊘', warning: '⚠', success: '✓', info: '●' };
```

Inside the `.toast` div, **before** `.toast-text`:

```
<span className="toast-glyph" aria-hidden="true">{TONE_GLYPH[toast.tone]}</span>
```

`aria-hidden` keeps the `role="alert"` announcement byte-identical to today — screen readers must
not hear "circled division slash". Nothing else in the component changes.

### 1.5 After — measured/computed, every tone × theme

Label (`--text-1` on the 14% tint) — **WCAG 1.4.3 AA text, 4.5:1**:

| Tone | Dark before → after | Light before → after |
|---|---|---|
| error | 3.35 → **10.30:1** | 3.48 → **11.68:1** |
| info | 3.68 → **9.81:1** | 3.38 → **12.00:1** |
| success | 4.07 → **9.61:1** | 3.66 → **11.90:1** |
| warning | 4.96 → **9.24:1** | 3.53 → **11.99:1** |

Bar + glyph (`--h` at 100% on the same tint) — **WCAG 1.4.11 non-text, 3:1**:

| Tone | Dark | Light |
|---|---|---|
| error | **3.35:1** | **3.48:1** |
| info | **3.68:1** | **3.38:1** |
| success | **4.07:1** | **3.66:1** |
| warning | **4.96:1** | **3.53:1** |

Every cell clears its bar in both themes. The tightest is light `info` at 3.38:1 (bar/glyph, 3:1
bar) — 13% headroom. Note these are numerically the *old* label ratios: the same pair is now doing
a graphics job it passes instead of a text job it failed.

Dismiss `✕` glyph: `inherit` was the hue (3.35–4.96 / 3.38–3.69, failing as text); now `--text-1`,
**10.30:1 dark / 11.68:1 light** on the error tint.

The 1px 35% border stays at **1.69–2.25:1** vs `--bg-1`: decorative delineation only, exactly as
§11 already rules for the pill border. It carries no meaning and the contract does not rely on it.

### 1.6 Geometry — `.toast-stack` and the text column

`.toast-stack` is **unchanged**: `position: fixed; top: 52px; right: 12px; width: 360px; gap: 8px;
z-index: 90`. `.toast` remains `box-sizing: border-box`, 360px wide.

Horizontal budget, measured before / computed after:

```
BEFORE  |1|      12      |            text 306.56           |8|dm 19.44|   12   |1|
AFTER   |3|    10    |gl 14|8|        text 284.0            |8| dm 24  |  8  |1|
         ^ hue bar                                                        ^ 12−4 margin
```

Text column **306.56px → 284.0px** (−22.6px, −7.4%). Left text inset is unchanged at
3+10+14+8 = 35px vs 1+12 = 13px — the text does move right by 22px, which is what a glyph column
costs. Vertical: a 1-line toast stays at 8+18.85+8+2 = **36.85px**; P73's pathological toast
(91-char path + two URLs, previously 360×225px) grows by roughly one wrapped line to ≈**244px**.
Still non-overflowing, still `overflow-wrap: anywhere`, still no ellipsis.

**Both densities:** the toast stack is a fixed overlay and, per §3, is outside the `panelDensity`
scope. Identical geometry in `cozy` and `compact`. Both themes: geometry is theme-invariant
(re-measured under `[data-theme='light']`, byte-identical).

### 1.7 States

| State | Spec |
|---|---|
| Default | as §1.3 |
| Hover (toast body) | no change — the toast is not a click target |
| Hover (dismiss) | `background: color-mix(in srgb, currentcolor 12%, transparent)` → a 12% `--text-1` wash, 24×24 |
| Active/pressed (dismiss) | no separate state (matches `.error-dismiss`, `.sidebar-add`) |
| `:focus-visible` (dismiss) | 2px `--accent`, offset 1px — inherits the global rule; restated above only because the 24px box changes the ring's size |
| Disabled | n/a — a toast never has a disabled control |
| Loading | n/a — toasts are terminal, never pending |
| Empty | `Toasts` already returns `null` at `toasts.length === 0`; unchanged |
| Error | the error tone *is* this state |
| Long content | wraps to any height; `overflow-wrap: anywhere`; no `max-height`, no clamp, no ellipsis (P73 §5.3 ruling stands) |
| `prefers-reduced-motion` | no motion added; nothing to honour |

### 1.8 Interaction, keyboard, a11y

Unchanged from today, and asserted so: `.toast-stack` keeps `aria-live="polite"`; `error` toasts
keep `role="alert"` and `sticky: true`; the dismiss button keeps `aria-label="Dismiss"`; toasts are
not in the tab order until reached naturally (they are late in DOM order, after the panes); Esc does
not dismiss a toast (no focus trap — a toast never steals focus). Toasts stay out of the command
palette. No shortcut is added.

### 1.9 Microcopy

**No string changes.** P73's six refusal sentences and every other toast body are unchanged. The
glyph is decorative and unvoiced. `aria-label="Dismiss"` unchanged.

---

## Item 2 — Sidebar hit targets below the 24px floor

`ui-reference.md` §3 states: *interactive controls are ≥24px tall in every density*, and the sidebar
has **one geometry in both densities** — so there is no compact-mode escape hatch and no
density-conditional fix. WCAG 2.2 **2.5.8 Target Size (Minimum)** puts the same floor at 24×24 CSS px.

### 2.1 Full sidebar audit — measured live, 2026-08-20 (240px sidebar, both themes identical)

| # | Control | Selector | Before | Verdict | After |
|---|---|---|---|---|---|
| 1 | Branches toggle | `.sidebar-section-toggle` | 159 × **16** | ✗ | 159 × **24** |
| 2 | Remotes toggle | " | 183 × **16** | ✗ | 183 × **24** |
| 3 | Tags toggle | " | 207 × **16** | ✗ | 207 × **24** |
| 4 | Stashes toggle | " | 183 × **16** | ✗ | 183 × **24** |
| 5 | Submodules toggle | " | 183 × **16** | ✗ | 183 × **24** |
| 6 | Worktrees toggle | " | 183 × **16** | ✗ | 183 × **24** |
| 7 | "Clean up branches…" | `.sidebar-add.sidebar-add-icon` | **20 × 20** | ✗ | **24 × 24** |
| 8 | "Create branch" `+` | `.sidebar-add` | **20 × 20** | ✗ | **24 × 24** |
| 9 | "Add remote" `+` | " | **20 × 20** | ✗ | **24 × 24** |
| 10 | "Stash changes" `+` | " | **20 × 20** | ✗ | **24 × 24** |
| 11 | "Add submodule" `+` | " | **20 × 20** | ✗ | **24 × 24** |
| 12 | "New worktree" `+` | " | **20 × 20** | ✗ | **24 × 24** |
| 13 | Branch-tree folder toggle | `.sidebar .tree-dir-toggle` | 207 × **18.84** | ✗ | 207 × **24** |
| 14 | "Clear filter" `×` | `.list-filter-clear` | **20 × 20** (CSS) | ✗ | **24 × 24** |
| 15 | Sidebar error banner `×` | `.error-dismiss` | ≈22 × **18** (padding `2px 4px`, 14px/1) | ✗ | **24 × 24** |
| — | Section header row | `.sidebar-section-header` | **20** (16 for Tags — inconsistent) | — | **24**, uniform |
| — | Branch / remote / tag / stash / submodule / worktree rows | `.branch-row` | **24** | ✓ | unchanged |
| — | Filter input | `.list-filter-input` | 24 | ✓ | unchanged |
| — | Pane divider | `.pane-divider` | (drag handle, wide enough) | ✓ | unchanged |
| — | Header toolbar icons (precedent) | `.btn-icon` | **32 × 32** | ✓ | unchanged |

**15 violations, 3 distinct root causes.** Notably every section header is 16px tall — the audit
also exposes that the **Tags** header row is 16px while the five with an action button are 20px, so
the section labels do not sit on a uniform rhythm today. Normalising all six to 24px fixes the
hit-target failure *and* that inconsistency in one move.

### 2.2 Exact CSS — `src/styles/sidebar.css`

```css
.sidebar-section-header {
  display: flex;
  align-items: center;
  gap: 4px;
  min-height: 24px;        /* NEW — the row that sets the floor */
}

.sidebar-section-toggle {
  flex: 1;
  min-width: 0;
  align-self: stretch;     /* NEW — fills the 24px header, no magic number */
  display: flex;
  align-items: center;     /* keeps the label optically centred */
  gap: 4px;
  border: none;
  background: transparent;
  padding: 0;              /* WAS 2px 0 — the stretch supplies the height now */
  cursor: pointer;
  text-align: left;
}

.sidebar-add {
  flex: none;
  width: 24px;             /* WAS 20px */
  height: 24px;            /* WAS 20px */
  /* everything else unchanged: border-radius 4px, transparent bg, --text-2,
     font-size 14px, line-height 1, padding 0, cursor pointer               */
}

.list-filter-clear {
  flex: none;
  width: 24px;             /* WAS 20px */
  height: 24px;            /* WAS 20px */
  /* rest unchanged */
}
```

`.sidebar-add-icon svg` stays **14 × 14** and the text `+` / `×` stay at 14px — **the painted glyph
does not change size**; only the transparent box around it grows. That is exactly the
"transparent hit area larger than the painted glyph" technique the task asks about, and the house
precedent already exists: `.btn-icon` in the header toolbar is a 32 × 32 box around a 14–16px glyph.
A 24 × 24 `.sidebar-add` sitting in a 24px header row is the same relationship one step down.

### 2.3 Exact CSS — `src/styles/status-panel.css` (shared `Tree`)

```css
.tree-dir-toggle {
  align-self: stretch;     /* NEW — single added line */
  /* rest unchanged */
}
```

`.tree-dir-row` is `height: var(--rp-row-h, 24px)`. The sidebar never defines `--rp-row-h`, so the
fallback applies and a stretched toggle is **24px** there. In the right panel the toggle tracks
whatever density has set — which is the correct behaviour and strictly better than today's 18.84px
in both places. **Do not hardcode 24px here**; that would break `compact`.

### 2.4 Exact CSS — `src/styles/empty-and-errors.css`

```css
.error-dismiss {
  flex: none;
  width: 24px;             /* NEW */
  height: 24px;            /* NEW */
  display: inline-flex;    /* NEW */
  align-items: center;     /* NEW */
  justify-content: center; /* NEW */
  padding: 0;              /* WAS 2px 4px */
  /* border:none, background:transparent, color:var(--danger),
     font-size:14px, line-height:1, border-radius:4px, cursor:pointer — unchanged */
}
```

### 2.5 Density and rhythm cost — accepted, and stated

`.sidebar-section` keeps `margin-bottom: 16px`; `.branch-list` keeps `margin: 4px 0 0`;
`.branch-row` stays exactly **24px**. The only vertical change is the six header rows:
16/20px → 24px, i.e. **+28px total** across a scrolling pane (`.sidebar` is `overflow-y: auto`).
That is a real but small cost, and it buys a sidebar where every row — header or item — is on a
single 24px module. **No list row, indent, gap, font size, or colour changes.** Information density
per row is untouched; nothing truncates differently.

### 2.6 States (both themes, both densities — sidebar geometry is density-invariant per §3)

| State | `.sidebar-section-toggle` | `.sidebar-add` / `.list-filter-clear` |
|---|---|---|
| Default | transparent, label `--text-3` 11px uppercase (`.section-label`, unchanged) | transparent, `--text-2` |
| Hover | **OPT-1, see OPEN-1** | `background: var(--bg-2); color: var(--text-1)` — unchanged rule, now painting 24×24 |
| Active/pressed | none (no existing rule; unchanged) | none (unchanged) |
| `:focus-visible` | 2px `--accent`, offset 1px — global rule; the ring now traces a 24px box instead of a 16px one, so it no longer clips the chevron | same, 24px box |
| Disabled | n/a | `color: var(--text-3)`, `cursor: default` — unchanged. `--text-3` on `--bg-1` is 3.38:1 dark / 2.96:1 light, below AA — **permitted**, WCAG 1.4.3 exempts inactive components (§2 dimming note) |
| Loading | `SkeletonRows` replaces the whole list; headers unaffected | `disabled={actionsDisabled}` — as today |
| Empty | `No submodules` / `No tags` / etc. `.branch-muted` — unchanged | unchanged |
| Error | `.sidebar-error` banner above the sections — unchanged except its `×` (2.4) | — |
| Long content | `.sidebar-section-toggle` keeps `flex: 1; min-width: 0`; labels are fixed short words and never overflow. Item rows keep their existing ellipsis + `title` | — |

Colour: **no colour changes anywhere in Item 2.** Nothing to re-measure; the P73 pill ratios (5.80 /
6.22 / 10.90 / 12.82) and §2's `--text-3` rulings are unaffected.

### 2.7 Interaction and keyboard

Unchanged: tab order is `toggle → action button(s)` per section, in DOM order top to bottom;
`aria-expanded` on the toggle; `aria-label` + `title` on every icon-only button (already present on
all seven); Enter/Space activate; right-click on item rows still opens the context menu.
No new shortcut, no command-palette entry — this milestone adds no commands.

### 2.8 Microcopy

**No string changes.** All seven `aria-label`/`title` pairs stay as they are
("Create branch", "Clean up branches…", "Add remote", "Stash changes", "Add submodule",
"New worktree", "Clear filter", "Dismiss").

---

## Destructive-action UX

P74 introduces no operation that can lose work. The one destructive control in scope —
`.sidebar-add.sidebar-add-icon` "Clean up branches…" — keeps its ellipsis, its existing
`StaleBranchesDialog` confirmation, and its current styling; only its box grows 20→24px. No
confirmation copy changes.

---

## Harness states (`src/ipc/mock/`)

Item 2 is fully visible today: open the mock repo and all 15 controls are in the DOM. No fixture
needed.

Item 1 currently has **no way to see three of the four tones in one place** — `error` is reachable
via `?submodule=notEmpty|urlMismatch|fail`, `success` via a successful submodule op, but `warning`
and `info` require driving unrelated flows. Add one seam so the increment is AI-gate verifiable:

**New file `src/ipc/mock/handlers/toasts.ts`** using the existing `query()` helper
(same pattern as `handlers/submodules.ts:11-27`):

| Seam | Fixture state |
|---|---|
| `?toasts=demo` | On repo open, push one of each tone, newest last: `info` "Already up to date" · `success` "Checked out vendor/libcore" · `warning` "Pre-commit hook printed a warning" · `error` "Couldn't check out vendor/libcore. The folder already has files in it. Move or delete everything inside 'vendor/libcore', then try again." — all four tones visible simultaneously in the stack, for the contrast and shape check |
| `?toasts=long` | One `error` toast carrying P73's pathological string (91-char path + both URLs) — the overflow/wrapping regression check |
| `?toasts=cap` | Six `info` toasts pushed in sequence — proves the §10.1 5-toast cap still holds after the CSS change |
| `?submodule=notEmpty` (existing) | unchanged — the real-flow error toast |

Empty state (`toasts.length === 0` → `null`) and the loading state (n/a) need no fixture.

Nothing here is a USER CHECKPOINT: every claim in this contract is a computed style or a
`getBoundingClientRect`, both readable in the headless harness. The only judgement I cannot make is
whether the 22px rightward shift of toast text *feels* right, which is a look-at-it call, not a
measurement — flag it for the user at the milestone gate, not as a blocker.

---

## Acceptance criteria (browser harness, exact expected values)

Contrast ratios are computed with the WCAG relative-luminance formula on the resolved
`getComputedStyle` colours; tolerance ±0.05.

**Item 1 — dark theme, `?toasts=demo`**

1. `.toast-error` label ratio (`color` vs `background-color`) = **10.30:1**; `.toast-info` **9.81:1**;
   `.toast-success` **9.61:1**; `.toast-warning` **9.24:1**. All ≥ 4.5:1.
2. All four `.toast-*` have `color` resolving to `--text-1` (`rgb(232, 234, 237)`), not a hue.
3. `border-left-width` = `3px` and `border-top-width` = `1px` on all four; `border-left-color`
   equals the tone hue at 100% (`--danger` = `rgb(229, 83, 75)` for error).
4. `.toast-glyph` textContent is `⊘` / `⚠` / `✓` / `●` for error / warning / success / info; each
   carries `aria-hidden="true"`; `color` = the tone hue; glyph-vs-toast-background ratio =
   **3.35 / 4.96 / 4.07 / 3.68** respectively, all ≥ 3:1.
5. Removing colour entirely (compare the four `.toast-glyph` textContents) yields **4 distinct**
   strings — WCAG 1.4.1 satisfied by shape.

**Item 1 — light theme (`[data-theme='light']`)**

6. Label ratios = error **11.68**, info **12.00**, success **11.90**, warning **11.99** — all ≥ 4.5:1.
7. Glyph/bar ratios = error **3.48**, info **3.38**, success **3.66**, warning **3.53** — all ≥ 3:1.

**Item 1 — geometry**

8. `.toast-stack` is exactly `360 × auto`, `top: 52px`, `right: 12px`, `gap: 8px` — **unchanged**.
9. `.toast` `padding` = `8px 12px 8px 10px`; `box-sizing` = `border-box`; width **360px**.
10. `.toast-dismiss` `getBoundingClientRect()` = **24 × 24**; its top edge is ≥ 0px inside the
    toast's border box (no overflow from the negative margin).
11. `.toast-text` width = **284 ± 1px** on a wrapping toast.
12. `?toasts=long`: the toast's `scrollWidth === clientWidth` (no horizontal overflow),
    `text-overflow` is not `ellipsis`, `-webkit-line-clamp` is `none`, `overflow-wrap` is `anywhere`,
    no `max-height`, and the height grows freely with the line count. **No pixel height is
    asserted** (REVISED 2026-08-20): the seam's `LONG_TEXT` is a re-creation of P73's pathological
    string rather than the byte-identical one, so a pinned height would assert something about a
    fixture rather than about the layout.

**Item 1 — behaviour unchanged**

13. An `error` toast still has `role="alert"`; the stack still has `aria-live="polite"`; the dismiss
    button still has `aria-label="Dismiss"`.
14. §10.1 intact: `?toasts=cap` yields exactly **5** `.toast` nodes; repeating a keyed push replaces
    rather than stacks (existing `toastQueue.test.ts` passes unmodified); non-`error` toasts still
    auto-dismiss and `error` toasts are still sticky.

**Item 2 — both themes, repo open, 240px sidebar**

15. All **6** `.sidebar-section-toggle` elements measure height **24** (was 16).
16. All **6** `.sidebar-add` elements (including `.sidebar-add-icon`) measure **24 × 24** (was 20 × 20).
17. All **6** `.sidebar-section-header` elements measure height **24** — including **Tags**, which
    was 16 while the others were 20.
18. `.sidebar-add-icon svg` still measures **14 × 14**; the `+` and `×` `font-size` is still `14px` —
    no painted glyph grew.
19. With a filter active, `.list-filter-clear` measures **24 × 24**; `.list-filter-input` is still **24**.
20. In branch tree mode, `.sidebar .tree-dir-toggle` measures height **24**, and
    `.sidebar .tree-dir-row` is still **24** (row rhythm unchanged).
21. `.right-panel[data-density='compact'] .tree-dir-toggle` measures **20** — i.e. the shared
    `align-self: stretch` tracked density rather than hardcoding 24 (see OPEN-2).
22. `.error-dismiss` measures **24 × 24**.
23. `.branch-row` is still exactly **24**; `.sidebar-section` `margin-bottom` still `16px`;
    `.branch-list` `margin-top` still `4px` — no list geometry moved.
24. `document.querySelectorAll('.sidebar button')` yields **zero** elements with
    `getBoundingClientRect().height < 24` or `.width < 24`.

**Global**

25. No new CSS custom property on `:root` or `[data-theme='light']`; no hex literal added to any
    touched file (grep the diff for `#[0-9a-fA-F]{3,8}`).
26. `pnpm build` (`tsc`) clean; `vitest` green with no test edited except additions for the new seam.

---

## OPEN items

**OPEN-1 — hover feedback on the section header toggle.**
The toggle becomes a full-width 24px target with **no hover affordance at all** today. A large
invisible click target is worse for discoverability than a small one. Recommended default:
**add it** —

```css
.sidebar-section-toggle { padding: 0 4px; margin-left: -4px; }
.sidebar-section-toggle:hover { background: var(--bg-2); border-radius: 4px; }
```

The negative margin keeps the label's optical left edge exactly where it is today (aligned with the
`.branch-row` glyph column), and `--bg-2` on hover is the same wash `.branch-row:hover` and
`.sidebar-add:hover` already use, so no new visual idiom. **Cost:** a visible change to a surface
the user did not ask about. If the orchestrator wants a zero-visual-delta increment, drop OPT-1 and
keep `padding: 0` — the hit-target fix stands on its own. **My recommendation: include it.**

> **REVISED 2026-08-20, post-implementation design review — `margin-left: -4px` is withdrawn.**
> Measured live: `.sidebar-section-toggle` rect starts at x = 8, while `.branch-row` and its hover
> wash start at x = 12 and `.sidebar`'s own gutter is 12px — the new wash is the only element in the
> pane that intrudes into the gutter. The rationale above was factually wrong: the header chevron
> sits at x = 12 and `.branch-glyph` at x = 16, so the two were never aligned and the negative margin
> preserves a pre-existing misalignment while buying a new one. Keep `padding: 0 4px`, drop
> `margin-left: -4px`: the wash then aligns with `.branch-row:hover` and the chevron column joins the
> 16px content column shared by `.branch-glyph` and `.sidebar .tree-dir-toggle`. Cost: six section
> labels shift right 4px. (P74 SF-1.)

**OPEN-2 — `.tree-dir-row` is 20px in right-panel `compact`, and it is interactive.**
Found while specifying 2.3: `--rp-row-h: 20px` in compact makes the *right panel's* folder toggle a
20px target, violating §3's own ≥24px rule. That is a genuine collision between §3's two clauses
(the 24px floor vs. `compact` existing at all) and it is **outside P74's sidebar scope**.
Recommended default: **leave it, and record the exemption in §3** — a 20px row in a
deliberately-chosen compact density is a defensible user opt-in, and raising it to 24 would delete
the point of `compact`. AC-21 pins the current behaviour so a future pass cannot regress it
silently. Flagging for your decision rather than folding it in.

**OPEN-3 — `.toast-stack` width.**
The glyph + compliant dismiss cost 22.6px of text column (306.6 → 284). Widening the stack to
**380px** would restore ~304px. Recommended default: **keep 360px.** 284px at 13px still holds
~55 characters per line, the stack width is asserted in existing e2e specs and in P73's wrapping
evidence, and 360 is the restrained choice. Raise to 380 only if the user finds the taller error
toasts objectionable at the gate.

---

## What I would push back on

Nothing in this request. Both items are things I flagged; the only judgement call I would defend is
using a **left edge bar rather than the §11 all-round border** for toasts — §11's recipe is
correct for pills and does not scale to a 360px surface, and I would rather amend §11 with a
size-dependent clause (done, §11 note in `ui-reference.md`) than apply a recipe outside the size
range where it was measured.
