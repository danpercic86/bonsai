# Bonsai — UI Reference Spec (all milestones)

Feel: GitButler-clean minimalism; GitKraken-style commit graph as the centerpiece. Dark theme is
the default. All values below are canonical — implement as CSS custom properties in
`src/styles.css` and reuse everywhere.

## 1. Layout geometry

```
+--------------------------------------------------------------+
| Header bar (40px): "Bonsai" · repo name + path · ⟳ refresh    |
+--------------------------------------------------------------+
| App notice bar (P70) — absent unless a global fault — §10     |
+--------------------------------------------------------------+
| Workspace toolbar (40px): remote ops · refresh               |
+-----------+---------------------------------+----------------+
| Sidebar   | Commit graph (canvas)           | Right panel    |
| 240px     | flex 1, min 480px               | 380px          |
| branches  |                                 | status / diff /|
| remotes   |                                 | commit details |
| tags      |                                 |                |
+-----------+---------------------------------+----------------+
| AI activity dock (30px collapsed / 120-600px open) — §9      |
+--------------------------------------------------------------+
```

- Header: height 40px, `--bg-1` background, 1px bottom border `--border`. Left: app name (600
  weight). Center-left: repo folder name (text-1) + full path (text-3, 12px, truncated). Right:
  refresh icon button (32×32 hit area).
- App notice bar (P70): second child of `.app`, between the header and the tab workspace hosts.
  App-global, in-flow, `flex: none`; absent from the DOM's visible output unless a process-wide
  fault is present. See §10.
- Left sidebar: fixed 240px, `--bg-1`, 1px right border. Collapsible sections "Branches",
  "Remotes", "Tags" (section headers: 11px uppercase, letter-spacing 0.08em, text-3).
- Center: `--bg-0`, hosts the `<canvas>` graph; fills remaining width, min 480px.
- Right panel: fixed 380px, `--bg-1`, 1px left border. Content: commit details when a graph node
  is selected; working-dir status + staging otherwise.
- Pane resizing: sidebar and right panel are drag-resized and persisted (`PaneDivider`).
- Bottom dock (P68e): full-width third child of `.workspace-host`, `flex: none`, absent from the
  DOM until an AI run exists. Never overlaps the panes — it takes height from them. See §9.
- Header toolbar (P69): `.header-toolbar` lives in `HeaderToolbar.tsx`, not in `App.tsx`. Order,
  left to right: theme · list view · AI assets · health · **dev (P91, conditional)** · settings ·
  **identity** (§12.6). The identity control is the far-right "account slot"; repo-scoped controls
  in the toolbar render only when a repo is open, and the **dev indicator renders only while Dev
  mode is on** (§12.11).

## 2. Theme tokens

CSS custom properties on `:root` (dark, default) and `[data-theme="light"]`.

| Token | Dark | Light | Use |
|---|---|---|---|
| `--bg-0` | `#16181d` | `#ffffff` | app/canvas background, content surfaces |
| `--bg-1` | `#1d2026` | `#f6f7f9` | panels, header, chrome |
| `--bg-2` | `#262a31` | `#eceef2` | hover, inputs, pill bg base |
| `--bg-3` | `#2f343d` | `#e2e5ea` | active/pressed |
| `--text-1` | `#e8eaed` | `#1c1f24` | primary text |
| `--text-2` | `#a8adb8` | `#4b515c` | secondary text |
| `--text-3` | `#6b7280` | `#8a919e` | **four exempt roles only** — never read text; see SANCTIONED ROLES below |
| `--border` | `#2c313a` | `#dcdfe5` | 1px pane/row borders |
| `--accent` | `#4f8cff` | `#2f6fe4` | primary buttons, links, focus ring |
| `--accent-text` | `#16181d` | `#ffffff` | text on a solid `--accent` fill (**5.52:1** dark / **4.65:1** light) — the two themes differ deliberately, P100 |
| `--accent-strong` | `#7fabff` | `#2a5cbe` | **the accent used as read text** (P105 — **shipped 2026-09-02**, commit `0e5dcab`). Clears 4.5:1 on `--bg-0`/`--bg-1`/`--bg-2`/`--bg-3`/`--selection` in both themes: **7.76/7.13/6.29/5.46/4.93** dark, **6.23/5.81/5.36/4.93/5.01** light; and on a **14% accent tint over `--bg-1`** **5.85 / 4.87** (over `--bg-2` it is 5.16 / 4.52 — see BASE, below). Never a fill, never a border, never the focus ring — those stay `--accent` |
| `--selection` | `#2a3b57` | `#dbe7ff` | selected row background |
| `--danger` | `#e5534b` | `#d13438` | errors, destructive |
| `--danger-text` | `#16181d` | `#ffffff` | ink on a solid `--danger` fill (**4.80:1** dark / **4.93:1** light) — P102, **shipped 2026-09-02**. **Fill-ink only — see the `--*-text` trap below** |
| `--danger-strong` | `#ff938c` | `#b3282e` | **the danger hue used as read text** (P106 — **shipped 2026-09-03**, commit `10ce967`). **Ink only**: never a fill, never a border, never the focus ring — those stay `--danger`. Measured live over the six composited backdrops the status-badge family reaches: `--bg-0` 8.29/6.44 · `--bg-1` 7.62/6.01 · `--bg-2` 6.72/5.55 · `--selection` **5.27/5.18** · staged 6% `--success` tint over `--bg-1` 6.99/5.56 · changes 5% `--text-3` tint over `--bg-1` 7.25/5.74 · **`--bg-3` 5.84/5.10** (P108, **shipped 2026-09-03** `42206fd` — the first `-strong` ink to reach `--bg-3`). **MIN 5.10 (light, on `--bg-3`) — the lowest figure anywhere in the `-strong` family** |
| `--success` | `#57ab5a` | `#1a7f37` | staged/added |
| `--success-text` | `#16181d` | `#ffffff` | ink on a solid `--success` fill (**6.24:1** dark / **5.08:1** light) — P102, **shipped 2026-09-02**. **Fill-ink only** |
| `--success-strong` | `#7fc98a` | `#116329` | **the success hue used as read text** (P106 — **shipped 2026-09-03**, `10ce967`). Same ink-only restriction. `--bg-0` 8.98/7.39 · `--bg-1` 8.26/6.90 · `--bg-2` 7.29/6.36 · `--selection` **5.71/5.94** · staged tint 7.57/6.38 · changes tint 7.86/6.59. **MIN over its shipped backdrops 5.71 / 5.94**; `--bg-3` is measured at **6.33 / 5.85** (P108) and takes the light minimum to 5.85 the day this ink ships there |
| `--merged` | `#a371f7` | `#8250df` | PR "merged" hue — **Option A was chosen** (orchestrator call, 2026-09-02) and **shipped**: it replaces the theme-invariant `#8957e5` literal, makes the "no colour literal outside `tokens-and-base.css`" grep a clean **zero**, and gives the light theme a purple tuned for a white page instead of the dark value reused |
| `--merged-text` | `#16181d` | `#ffffff` | ink on `--merged` (**5.30:1** dark / **5.05:1** light) — P102, **shipped 2026-09-02**. **Fill-ink only** |
| `--warning` | `#d4a72c` | `#9a6700` | modified/dirty. **As a letterform it is dark-only** — see the letterform note below |
| `--warning-strong` | `#e3b341` | `#7a4f01` | **the warning hue used as read text** (P106 — **shipped 2026-09-03**, `10ce967`). Same ink-only restriction. `--bg-0` 9.13/7.13 · `--bg-1` 8.39/6.65 · `--bg-2` 7.40/6.14 · `--selection` **5.80/5.73** · staged tint 7.69/6.15 · changes tint 7.98/6.36. **MIN over its shipped backdrops 5.80 / 5.73**; `--bg-3` is measured at **6.43 / 5.64** (P108) and takes the light minimum to 5.64 the day this ink ships there. There is deliberately **no `--warning-text`** — and, separately, **no `--warn`**; see the trap below |
| `--graph-canvas-bg` | `#16181d` | `#ffffff` | graph-pane surface behind the canvas (container div, load skeleton, empty state) — keeps the DOM behind the canvas seamless with the canvas fill. Aliases `--bg-0` by default; the **Bonsai graph style** (spec 002) overrides it via `[data-graph-style='bonsai']` to a warm backdrop: **`#17140f`** dark / **`#f4efe6`** light. See §5.1. |

Focus: 2px `--accent` outline, offset 1px, keyboard only (`:focus-visible`).

**THE `--*-strong` FAMILY — four tokens, one rule (completed by P106, shipped 2026-09-03 `10ce967`).**
`--accent-strong` (P105) · `--danger-strong` · `--success-strong` · `--warning-strong` (P106).
Each is its hue **lifted in dark, deepened in light**, and each exists for exactly one job: **the hue
used as read text**. They are **ink only** — never a fill, never a border, never a bar or glyph edge,
never the focus ring; the fill/bar/border keeps the base hue (`--danger`/`--success`/`--warning`/
`--accent`), whose ≥3:1 graphics figures are untouched by this family. **Fixed-family minimum across
every backdrop any of the four reaches: 5.10** (`--danger-strong`, light, `--bg-3`) — 0.60 above the
text bar. **This number moved from 5.18 to 5.10 when P108 shipped** (`42206fd`, 2026-09-03) and
`.btn-secondary-danger:hover` put a `-strong` ink on `--bg-3` for the first time. The P108 contract
recorded 5.10 forward-lookingly; it is now shipped fact, and 5.18 is history.

> **THE `--bg-3` ROW — measured for the P108 contract, shipped 2026-09-03 in `42206fd`.** No
> `-strong` ink had reached `--bg-3` before P108, so the earlier 5.18 minimum was computed over
> backdrops that did not include it. Measured on `--bg-3`: `--danger-strong` **5.84 / 5.10** ·
> `--success-strong` **6.33 / 5.85** · `--warning-strong` **6.43 / 5.64** · `--accent-strong`
> **5.46 / 4.93**. The post-fix minimum anywhere in the app after P108 is therefore **5.10** —
> `--danger-strong` as ink on `--bg-3`, light — matching the family minimum predicted by the
> contract exactly. For the base hues on `--bg-3`, which is where the P108 fixes came from:
> `--danger` **3.38 / 3.90**, `--success` **4.39 / 4.02**, `--warning` **5.58 / 3.85**. The four minima sit in a **4.93–5.85** band, counting every
backdrop measured for the family including `--bg-3` (`--accent-strong` 4.93/4.93,
`--danger-strong` 5.27/**5.10**, `--warning-strong` 5.80/5.64, `--success-strong` 5.71/5.85), so a surface
mixing them reads as one system rather than four unrelated colours. **Reach for a `-strong` token
before inventing a hex or demoting a hue to `--text-1`.** Candidates measured and rejected during
P106, recorded so they are not re-derived: `#ef7c74` (danger dark, MIN 4.19 — fails), `#f5877f`
(4.65 — passes on 0.15 of headroom, too thin for a reused token), `#c0272d` (danger light, 4.73 —
same objection), `#855800` (warning light, 4.98 — compliant but out of band with its siblings), and
`#6e4600` (warning light, floor 6.65 — **not** the shipped value; if a future pass sees 6.65
attributed to `#7a4f01`, that is P106's caught first-draft error, not a measurement).

**THE `--*-text` TRAP — the single most likely misuse of the hue tokens, and it is now measured.**
`--danger-text` / `--success-text` / `--merged-text` are **inks for solid fills only**. They are
near-black in dark and white in light, i.e. exactly inverted from what a letterform on a *tint or a
neutral panel* needs. **The `--*-text` family must never sit on a tint.** Measured: `--danger-text`
on a 14% `--danger` tint is **1.27 / 1.31** (P107) and **1.43 / 1.41** on the status-badge backdrops
(P106); a hypothetical `--warning-text` (which by construction resolves to `--bg-0`) measures
**1.19 / 1.16** on the staged tint, **1.09 / 1.07** on `--bg-1`, **1.57 / 1.24** on `--selection` —
invisible. **Never reach for a `--*-text` token because the hue name matches**; on anything that is
not a solid fill of that hue, the answer is the `-strong` token. This is why **no `--warning-text`
token exists and none should be added**: the only compliant solid-`--warning` ink in the app is
`--bg-0` (7.92 dark / 4.87 light, `.ai-dock-ask-glyph`), and a `--warning-text` would immediately be
mistaken for the tint answer.

**THE UNDEFINED-TOKEN TRAP — two live cases, both proven 2026-09-03 (P108, shipped `42206fd`). A
`var()` fallback can mask a token that does not exist, and a naive probe of a non-existent token
returns a plausible, wrong, *passing* number.**

1. **`--warn` is defined nowhere in the app.** `settings-legacy-sections.css:134` was
   `color: var(--warn, #b8860b)`, so it had **always** painted the theme-invariant literal
   `#b8860b` — **5.46 dark / 3.25 light on `--bg-0`**, i.e. sub-AA in light since the day it was
   written. Confirmed twice: by grep (no definition in `tokens-and-base.css` or anywhere else) and
   live (`getComputedStyle(document.documentElement).getPropertyValue('--warn')` returns `""` in
   **both** themes). **No hue-name grep can see this, because the hue name appears only inside the
   fallback** — `--warn` is not in the hue alphabet, and `#b8860b` is not in any token grep. P108
   fixed it by pointing at a real token **with no fallback at all**.
2. **`--warning-text` is also undefined** — only `--accent-text`, `--danger-text` and
   `--success-text` (and `--merged-text`) exist. Same trap class, nastier symptom: **probing an
   undefined custom property on an element yields the element's *inherited* colour**, so a naive
   measurement of "`--warning-text` on `--bg-1`" returns **13.54 dark / 15.42 light** — a
   comfortable pass — instead of the **1.09 / 1.07** this file records for the value it would
   resolve to by construction (`--bg-0`). **Anyone re-measuring that row without checking that the
   token exists will get a plausible, wrong, passing number and will "correct" a correct figure.**

**The rules, both grep-checkable:** (a) **a `var(--x, …)` fallback is not a fallback when `--x` is
undefined — it is the value**; any audit that touches `var(--x, …)` must first confirm `--x` is
defined in `tokens-and-base.css`, for both themes. (b) **Never record a ratio for a token you have
not confirmed exists**; an undefined property measures as inheritance, not as itself. Ship real
tokens with **no** fallback — a fallback on a token that exists is dead bytes, and on a token that
does not exist it is an untracked hardcoded literal.

**Contrast notes (measured 2026-08-17, P68e design pass; toast rows updated 2026-08-20, P74;
`--text-3` swept in two enumerated passes, 2026-08-31 P95 and 2026-09-01 P98).**

**Scope of the `--text-3` work — audit complete, all 93 fixes SHIPPED (P101a–c); closed by token
name only, see the qualifier below.** Three
*enumerated* passes: P95 took the **enabled-interactive-control** class, P98 took an
**eight-selector read-text set**, and **P101 (2026-09-01) audited the family exhaustively** — all
**124 `color: var(--text-3)` declarations across 33 files in `src/styles/`** (re-pinned by grep at
audit time: 122 + the two disabled-hint overrides P98 itself added), plus the one declaration
outside `src/styles/`. Every declaration now carries a recorded bucket and verdict in
`docs/contracts/P101-text3-audit-ui.md` §3 — **93 fixes, 31 sanctioned exemptions** — and the
token's role is redefined by the SANCTIONED ROLES bullet below. P101 also found **three pre-existing
P95-class enabled-control escapes** (`.settings-reset`, `.settings-config-advanced-summary`,
`.onboarding-skip`), so P95's app-wide claim was as unevidenced as P98's: **an "app-wide swept"
claim has now failed SIX times — do not make a seventh.** The tally:
(1) P95's enabled-control class — 3 escapes found by P101; (2) P98's "`--text-3` family closed" —
122 declarations never classified; (3) P74's hue-as-text sweep — the "`--accent` is fine on
`--bg-0`/`--bg-1`/`--bg-2`" sentence was false, 21 sub-AA call sites (P105, below); (4) P74's "no
remaining hue-as-text-over-its-own-tint anywhere" — 6 instances found by P105, and that 6 was
itself under-counted; (5) **this file's own "6 live instances" figure (2026-09-02, P105)** — a full
scan run during P102/P105 implementation found **16 further live instances** outside that
milestone's scope, plus **2 inside its own rule block** that the P105 enumeration missed
(`.asset-chip-sync`, `.asset-chip-drifted`). The count is replaced below by an enumerated list with
`file:line`, so the next pass inherits evidence instead of a number. **A claim of app-wide closure
is worthless without a per-declaration table behind it — and a bare count with no list behind it is
the same failure in miniature.**
**(6) P101's own "`--text-3` family CLOSED, exhaustively" — added 2026-09-03 on the P108 landing,
and it is the most instructive of the six**, because P101 is the one that *did* have the
per-declaration table. It enumerated all 124 `color: var(--text-3)` declarations and still missed
`.commit-signature-unknown .commit-signature-icon`, a **2.96-in-light** glyph — below even the 3:1
graphics bar — because that rule spells the same value `--badge-unknown`. The first five failed for
want of an enumeration; **the sixth failed because the enumeration was scoped by token NAME.**

> **THE ALIASING RULE (P108, 2026-09-03 — token aliasing has now hidden instances THREE times):**
> **an audit scoped by token NAME cannot see an alias — scope by resolved VALUE.**
> The three: `--badge-good` ≡ `--success` and `--badge-warn` ≡ `--danger` (found in P106/P107, which
> hid a real 4.41-dark read-text failure), and `--badge-unknown` ≡ `--text-3` (found in P108, which
> hid the sub-3:1 glyph above from a family recorded as CLOSED). Before claiming any colour family
> closed, resolve every token in `tokens-and-base.css` to its literal in **both** themes, group the
> tokens by identical value, and search on the whole group. An alias is invisible to every
> name-shaped grep, and aliases are exactly where the escapes have been.

**A SECOND FAILURE MODE, recorded 2026-09-03 (P107 landing). It does not increment the tally above —
that tally counts false app-wide *closure claims*, while this one is a false *individual fix
credit*, and diluting it would blunt what it teaches.** This one is the opposite shape: an item that *was* enumerated with
`file:line`, and was still wrong. **`.settings-toggle-btn.is-active`
(`settings-legacy-sections.css:101`) was credited below as a fixed-and-shipped P105 site, but the
rule never matched anything** — the class had been orphaned in `7354aca` when `SettingsSegmented`
replaced the one real toggle — and it was deleted outright in `2168057`. A grep enumeration proves a
**declaration exists**; it does not prove the declaration **renders**.

**The aggravating detail: this was known and then lost.** P105's own contract recorded the deadness
three times — `docs/contracts/P102-P105-hue-audit-ui.md` **:543-544** ("matches nothing in the app
today"), **:572-573** ("dead rule — probed, not rendered") and **:591** ("could not be verified in the
harness"). The caveat was dropped when the result was summarised into *this* file, where it became an
unqualified shipped-fix credit. So the root cause is not only "asserted without checking" but
**evidence lost in transcription**: a hedge in a source contract must survive into the canonical doc,
because the canonical doc is what the next pass actually reads.

**The discipline is therefore three-part: (1) enumerate with `file:line`; (2) confirm each enumerated
selector has a live DOM match** — grep the class in `src/**/*.tsx`, or see it in the harness —
**before recording it as fixed; (3) carry every "unverified" / "dead rule" / "could not confirm"
qualifier verbatim into this file.** An unverified fix on dead CSS is worse than no fix: it consumes
the audit budget and leaves a false green in the record.

**A THIRD FAILURE MODE, recorded 2026-09-03 (P106 landing) — the grep counts TEXT, not declarations,
and this has now bitten three times.** An acceptance-criterion grep is a string search over source
bytes: **prose comments and `var(--x, #hex)` fallbacks inflate it**, and a stale baseline invalidates
the delta it was supposed to prove. Both halves went wrong in P106 and were caught only by measuring
the real pre-fix state instead of inferring it: its `R2` baseline was recorded as **50 and was
actually 53** (post-fix 48, so the predicted delta of −5 was exactly right and P106's own contribution
was 0 — the prediction was fine, the baseline was not), and its `R10` — `#[0-9a-fA-F]{6}` outside
`tokens-and-base.css` — was recorded as **0 and was actually 12**, unchanged by P106 and entirely in
files it never touched: **7 hex literals quoted inside prose CSS comments** (`updates.css`,
`forge-pr.css`, `controls.css`) and **5 `var(--x, #hex)` fallbacks**
(`settings-legacy-sections.css:134,140`, `commit-box.css:45,49,68,138`, `ai-dock.css:65`). The
implementer had to deliberately avoid literal token strings in its own new comments; without that
care `R8` would have read 11 instead of 8 and `R9` 2 instead of 1. Earlier passes hit the identical
thing — the `--h:` grep counted a comment at `graph-filter.css:87`, and two `color: #` hits were
comments. **The rules: (1) a residue prediction must state whether it counts *declarations* or *raw
matches*, and if declarations, the grep must exclude comments and `var()` fallbacks or the criterion
must say what the expected non-declaration matches are; (2) a baseline is measured against the real
pre-fix tree, never inherited from a prior contract or inferred; (3) when a grep and a prediction
disagree, re-measure the baseline before touching the code — the next pass will otherwise "fail" a
correct fix; (4) when a residue prediction names `file:line`, it must state that **the count is the
criterion and the line numbers are not** — any comment the fix itself adds shifts every line below
it. P106's R4 predicted its five new declarations at `193, 198, 203, 216, 227` and shipped them at
`193, 198, 203, 220, 231`: the count was exact, the lines drifted by the explanatory comments the
same fix added. A reviewer checking lines rather than counts would have called a correct fix a
miss.** The accent-fill shortfall is **closed** (P100); the accent-as-text
(P105) and hardcoded-ink-on-hue-fill (P102) shortfalls **shipped 2026-09-02** in commit `0e5dcab`,
enumerated in `docs/contracts/P102-P105-hue-audit-ui.md` §2–§3 and verified in the browser harness
in both themes. All 93
P101 fixes **shipped** (P101a–c), so the **`--text-3` family is CLOSED *by token name*** — the
enumeration is behind it, declaration by declaration, in `docs/contracts/P101-text3-audit-ui.md` §3,
unlike the retracted P95/P98 claims — **but "by name" is the whole qualifier, and it failed once
already**: P108 found the `--badge-unknown` alias escape (2.96 in light) and fixed it (Bucket C,
shipped `42206fd`). **The family has never been audited by resolved VALUE.** Treat the closure as
holding for `color: var(--text-3)` and for the one alias now known; a third spelling of `#6b7280` /
`#8a919e` would be a new escape. **A new
`--text-3` use outside the four SANCTIONED ROLES below is a defect.** **Do not write "no AA colour
shortfall remains in §2" here again.** Two shortfalls are open and enumerated: the
hue-over-own-tint residue (the list below was **16**; a descendant- and inheritance-aware re-search
raised the population to **38 instances — 17 failing text, 1 failing glyph state, 19 compliant glyph
keeps, 1 owned by P106** — enumerated with per-site measured ratios in
`docs/contracts/P107-hue-over-own-tint-ui.md`, which is now the record for this class) and the
A/M/D/U/R status-badge hue family (**P106**, §7 — **this one shipped 2026-09-03 in `10ce967`**;
its two USER CHECKPOINTs remain **pending**, see §7).

**P108 — hue-as-text over NEUTRAL surfaces. SHIPPED 2026-09-03, commit `42206fd`** (contract
`8027cef`, `docs/contracts/P108-hue-as-text-on-neutral-ui.md`; that contract's §2 and §4 remain the
per-declaration record). **17 CSS files, no `.ts`/`.tsx`, zero DOM change, no new token** — the fixes
are the three `-strong` tokens plus `--text-2`. Population **62 call sites / 61 declarations**:
**fixed 27 Bucket-A hue-texts + 1 Bucket-C alias-hidden glyph**; **kept 28 Bucket-B glyph/border
(6 of them canvas) + 6 compliant Bucket-D texts**, three of which (`.settings-config-error`,
`.settings-profile-danger`, `.pr-stat-add`) were additionally moved to `-strong` for cohesion —
decision **D1 accepted**, which is why the `-strong` count lands at 35 rather than 32.

**Every residue row matched its prediction, and every pre-fix baseline was re-measured in-tree
rather than inherited** — the discipline from the third failure mode, applied and vindicated:
raw hue-as-text **54 → 26** · `var(--…-strong)` **5 → 35** · `var(--warn` **1 → 0** · hex outside
`tokens-and-base.css` **12 → 6** · `--badge-unknown` **3 → 2**. **Post-fix minimum anywhere in the
app: 5.10** (`--danger-strong` as ink on `--bg-3`, light), exactly the family minimum the contract
predicted.

**Four things P108 established that outlive it — read these before running any colour audit:**
1. **A `var()` fallback can mask a token that does not exist**, and an undefined token probes as
   inheritance. Both cases (`--warn`, `--warning-text`) are written up in the UNDEFINED-TOKEN TRAP
   block under the token table.
2. **An audit scoped by token NAME cannot see an alias — scope by resolved VALUE** (the ALIASING
   RULE block above; three escapes to date).
3. **One declaration can serve two selectors with different verdicts, so bucket per *selector*, not
   per *declaration*.** `commit-panel.css:98-100` carried a compliant glyph half and a text half
   failing at **4.41 dark**; bucketing the declaration as a unit would have got **both** wrong —
   either shipping a needless glyph change or leaving sub-AA text. The fix splits the rule. This is
   the only structural change in the whole milestone.
4. **A search pass that finds nothing is still evidence, and must still be run.** P108's imperative-
   canvas pass covered **7 draw sites under `src/graph/`** and produced **0 fixes** — all glyph, all
   ≥3:1 on both canvas backdrops. That is a *result*, not a wasted pass: assuming a pass would find
   nothing is precisely how the earlier counts went wrong. Keep the canvas pass in the standard
   method, and record its null result with the ratios that justify it.

**A FIFTH count moved, and the mechanism is new: TOKEN ALIASING and UNDEFINED custom properties.**
The inventory recorded here as **48** did not reproduce — the real population is **62 call sites**,
and the same-shaped raw grep measures **54** matches against the real tree (the 48 predates
`settings-dev.css` and did not account for the three `border-color: var(--hue)` declarations the same
pattern matches). **Quote 62, never 48.** Two whole classes were invisible
to it, and both are now permanent search requirements:
- **`--badge-good` ≡ `--success` and `--badge-warn` ≡ `--danger`, byte-identical in both themes**
  (already noted in §2's hiding-patterns block) — this hid a real read-text failure,
  `.commit-signature-warn .commit-signature-text` at **4.41 dark on `--bg-1`**.
- **`--badge-unknown` ≡ `--text-3`, byte-identical in both themes** (`#6b7280` / `#8a919e`). So
  `.commit-signature-unknown .commit-signature-icon` is a `--text-3` glyph measuring **3.38 / 2.96**
  — **below even the 3:1 graphics bar in light**, and it was invisible to P101's *exhaustive*
  `color: var(--text-3)` audit purely because of the alias. **The `--text-3` family is recorded as
  CLOSED above; this is a known escape from it**, fixed by P108 Bucket C.
- **`settings-legacy-sections.css:134` was `color: var(--warn, #b8860b)` and `--warn` is not defined
  anywhere in the app** — confirmed by grep *and* live (`getComputedStyle` returns `""` in both
  themes). The rule had always painted the literal `#b8860b` — **5.46 dark / 3.25 light on `--bg-0`**
  — in both themes. **Fixed and shipped in `42206fd`**, by pointing at a real token with **no**
  fallback. **A `var()` fallback is not a fallback when the property is undefined; it is the value**,
  and **no hue-name grep can see it, because the hue name lives only inside the fallback.** Any grep
  over `var(--x, …)` must check that `--x` exists. Full write-up: the UNDEFINED-TOKEN TRAP above.

**OWED — P108's AC11 is UNVERIFIED, and must never be recorded as closed.** Two states could not be
reached in the harness, so their post-fix figures are **source-derived, not measured**:
`.file-count-del` in its **selected** state (**3.05**) and `.context-menu-item[data-tone='danger']`
in its **hovered** state (**3.05**) — both pre-fix worst cases. **Both the implementing agent and the
orchestrator tried and failed:** React's context menu needs a **trusted** input event, and the diff
panes never mount via keyboard navigation. The qualifier travels with the numbers, verbatim, per the
transcription rule above — and this one is **the orchestrator's own unverified claim, not an
agent's**, which is exactly why it is written down here rather than quietly rounded to green.
**AC12, AC13 and AC14 remain USER CHECKPOINTs and stay pending; no agent may self-declare them.**

**One OPEN TONE question, deliberately unresolved and filed for the user:** `.op-worktree-warning`
received the **contrast** fix and **kept its danger hue**. Whether a *warning* should be painted in
the danger hue at all is a **tone** decision, not a contrast one, and P108 did not make it (contract
§11, D3). Do not "fix" it as a contrast defect — it is compliant.

**The hue alphabet for any future search is
`danger|success|warning|merged|accent|badge-good|badge-warn|badge-unknown|h`** — three tokens longer
than the list P107 recorded. Also open: **P109** — the status badge has no accessible name, and
`added`/`untracked` collide on `A` in 3 of the 6 badge tables while the other 3 already render `U`
(§7, record correction 2026-09-03).

- **The full `--text-3` / `--text-2` matrix (P98 measured; the `--bg-3` row added by P101).** Read
  this before choosing either token on any surface.

  | Backdrop | `--text-3` d / l | `--text-2` d / l |
  |---|---|---|
  | `--bg-0` | **3.68** / **3.17** ✗ | **7.90** / **7.99** ✓ |
  | `--bg-1` | **3.38** / **2.96** ✗ (light also ✗✗) | **7.25** / **7.45** ✓ |
  | `--bg-2` | **2.98** / **2.73** ✗✗ | **6.40** / **6.87** ✓ |
  | `--bg-3` | **2.58** / **2.51** ✗✗ | **5.56** / **6.32** ✓ |
  | `--selection` | **2.33** / **2.55** ✗✗ | **5.01** / **6.42** ✓ |
  | own 18% tint | **2.78** / **2.51** ✗✗ | — |

  ✗ = below 4.5:1 (text). ✗✗ = below **3:1** (graphics). **`--text-3` clears 4.5:1 on nothing, in
  either theme, and clears 3:1 only on `--bg-0` (both) and `--bg-1` (dark only).** So it is never
  *adequate* for text: a `--text-3` declaration is legitimate only when it is **exempt** from the
  text bar, never because its ratio is good enough. **`--text-3` must not appear on a `--bg-2`,
  `--bg-3` or `--selection` surface in any role** — it fails even the graphics bar there, in both
  themes. That is a grep-checkable invariant; use it.

  Any text the user must actually read — metadata, timestamps, costs, log lines, hints,
  **status-pill labels**, **settings help text**, **form field labels**, **units**, **oids and
  dates in a picker**, **any heading that is the user's only wayfinder** (§12.5's result-group
  headers, the command palette's group headers, "STAGED" / "UNSTAGED") — uses `--text-2`.
- **SANCTIONED ROLES for `--text-3` (P101, exhaustive).** Exactly four, and nothing else:
  **(1) disabled** — the text or glyph of a disabled control or row (16 declarations);
  **(2) placeholder / empty** — `::placeholder` and empty-state copy where the surface itself is the
  message and no fix is named (10); **(3) settings group / section titles** — an uppercase heading
  over a *visibly bounded* group, on `--bg-0` (3); **(4) coordinate glyphs on `--bg-0`** — line
  numbers and duplicate-structure glyphs, where 3.68 / 3.17 clears the graphics bar
  (`.diff-lineno`, `.commit-search-icon`, `.cm-gutters`). A `--text-3` use outside these four is a
  defect. The per-declaration enumeration behind this list is
  `docs/contracts/P101-text3-audit-ui.md` §3.
- **Read text is judged at 4.5:1 against its *actual composited* backdrop, in both themes
  (rule made explicit 2026-09-01, P98).** The test for "decorative" is **not** how the text looks —
  small, uppercase and letter-spaced does not make text decorative. The test is whether the user
  **must read it to act**. A 11px uppercase pane label is read text when it is the only thing
  telling the user which merge pane is OURS vs THEIRS; an option's trailing hint is read text
  because it is what the user reads *in order to choose*. Where the same element has
  hover/selected/active states, **each state's backdrop is measured separately** — `--text-3` on a
  `--selection` row fill was the worst case found (2.33:1).
  - **BASE — always record the composited base with the number (rule added 2026-09-03, P106 landing).
    A contrast figure is meaningless without its composited base.** A tint is translucent, so "on a
    14% accent tint" names only half of the stack; the ratio moves with whatever the tint sits on.
    Worked example, and the reason the rule exists: `--accent-strong` on a 14% accent tint measures
    **6.42 / 5.19** over `--bg-0`, **5.85 / 4.87** over `--bg-1`, **5.16 / 4.52** over `--bg-2` — a
    spread of 1.26 in dark from the base alone. P106 and P107 recorded **5.16/4.52** and
    **5.85/4.87** for what read as the same measurement and were carried in this file and in
    `TODO.md` as two contracts disagreeing on a measured value. They never disagreed: **neither
    stated its base**, P106 had used `--bg-2` and P107 `--bg-1`, and both reproduce exactly under one
    method. Resolved 2026-09-03; both contracts corrected. **This is the same class as the
    transcription failure mode above** — a number survives into the canonical doc without the
    qualifier that makes it interpretable, and the next pass then spends a milestone re-deriving it
    or, worse, "corrects" a correct figure. So: **every ratio recorded anywhere in this file, in a
    contract, or in a CSS comment names the ink, the tint and percentage if any, AND the base**
    (`ink on N% hue over --bg-X`). A figure that names only the tint is incomplete evidence and may
    not be used to close an AC. The existing per-site tables in `P107-hue-over-own-tint-ui.md` §3 and
    `P106-status-badge-ink-ui.md` §4/§6.1 carry a base column; copy that shape.
  - **The eight selectors P98 swept to `--text-2`** — a new `--text-3` on any of these is a defect:
    `.diff-overlay-kind`, `.diff-tree-count`, `.conflict-editor-split-label`, `.wtctx-branch`,
    `.wtctx-blocked`, `.combobox-option-hint`, `.command-palette-option-hint`,
    `.pr-merge-method-desc` (§12.9). Plus the two active-state hint overrides, which moved from a
    hardcoded `rgba(255,255,255,.8)` to `var(--accent-text)` in P98, and then to **`--text-2`** in
    P100 when the `--accent` fill under them was retired for `--selection` (see the ACCENT FILL
    bullet).
  - **Sanctioned decorative — audited and deliberately left at `--text-3`.** This list is
    exhaustive; a `--text-3` use *not* on it and *not* among the eight above is **unclassified**,
    not approved.

    | Use | Where | Measured | Why decorative |
    |---|---|---|---|
    | Editor line-number gutter | `src/components/conflictCmSetup.ts:36`, `.cm-gutters` | 3.68 / 3.17 on `--bg-0` | a coordinate that duplicates visible structure; universal editor convention (a `--text-2` gutter would be louder than any editor the user knows). The act-carrying text in that pane — `.conflict-region-caption`, `.conflict-editor-split-label`, the accept/reject buttons — is already `--text-2` or brighter. **Revisit trigger:** if a line number ever becomes actionable or is named in copy (go-to-line, line-range selection, any message citing a line), it becomes read text and moves to `--text-2`. Re-checked at P101 — no go-to-line control, no line-range UI, no copy citing a line: **sanction stands**. |
    | Diff gutter line numbers | `diff-content.css:144`, `.diff-lineno` | 3.68 / 3.17 on `--bg-0` | same ruling and same revisit trigger as `.cm-gutters` (P101). **Contrast the blame gutter:** `blame-history.css:112` `.blame-lineno` was ruled the *other* way and moved to `--text-2`, purely because its gutter goes to `--bg-2` on hover (2.98 / 2.73). This is a coordinate-glyph-**on-`--bg-0`** exemption, not a blanket line-number exemption. |
    | Search input glyph | `search.css:28`, `.commit-search-icon` | 3.68 / 3.17 on `--bg-0` | duplicates the visible input and its placeholder (P101) |
    | The disabled set (16 declarations — this list is the whole of them, P101) | `dialogs-forms.css` `.combobox-option--disabled`(`.combobox-option--active`)(` .combobox-option-hint`); `search.css` `.command-palette-option.is-disabled`(`.is-active`)(` .command-palette-option-hint`); `.btn-icon:disabled`, `.toolbar-btn:disabled` (`controls.css`); `.context-menu-item:disabled`(` .context-menu-subnote`) (`context-menu.css`); `.row-action:disabled`, `.commit-msg-tool:disabled` (`commit-box.css`); `.git-dock-clear:disabled`; `.sidebar-add:disabled`; `.tab-add:disabled`; `.settings-account-{kind,default} input:disabled + span` | 3.38 / 2.96 on `--bg-1`; 2.98 / 2.73 on `--bg-2` | dimming *is* the disabled signal, carried independently by `disabled` / `aria-disabled` + `cursor: default`. **Note the child-rule pairs** — a disabled row's hint/subnote needs its own restated dim (see the child-rule trap below); P98 and P101 each had to add one |
    | `::placeholder` and empty-state copy (10) | `commit-box.css:318`, `sidebar.css:251/288`, `search.css:183/278`, `composer.css:151`, `agent-assets.css:13/162`, `image-diff.css:76`, `diff-content.css:225` | 2.98–3.68 / 2.73–3.17 | the surface itself is the message. **Does not extend to an empty state that names the fix** — that sentence is `--text-2` (§8). P101 recorded the placeholder half as arguable under WCAG 1.4.3 and recommended a **visible label** over a brighter placeholder if it is ever revisited |
    | Settings group / section titles (3) | `settings-shell.css:412`, `settings-legacy-sections.css:27/124` | 3.68 / 3.17 on `--bg-0` | an uppercase heading over a *visibly bounded* group. **Does not extend to a header that is the only wayfinder** — a result-group header, a split-pane label, a command-palette group header, a checkbox-group header: all `--text-2`. A decorative separator inside a string whose other half moved to `--text-2` also moves, **for cohesion, not contrast** (`.reflog-oid-arrow`, P101 §6.5) — that is not a precedent that glyphs need 4.5:1 |
    | The P95 §3.3 exempt set | `docs/contracts/P95-a11y-ui.md` §3.3 | — | as recorded there, **minus** the three escapes P101 found (`.settings-reset`, `.settings-config-advanced-summary`, `.onboarding-skip`), which are enabled controls and move to `--text-2` |
  - **The child-rule trap (found 2026-09-01, P98 §8.4 — check this after every swap).** A `color`
    rule on a **child** of a disabled row beats the `--text-3` that child would otherwise inherit
    from the row, so brightening a hint app-wide silently re-brightens **half of every disabled
    row**. P98 shipped exactly this bug: a disabled combobox option's label measured `--text-3`
    (3.38 / 2.96) while its own hint measured `--text-2` (7.25 / 7.45) — the qualifier twice as
    bright as the thing it qualified, and the disabled affordance only half-applied. **Whenever a
    child element is raised to `--text-2`, restate the dim on the disabled compound selector**
    (`.combobox-option--disabled .combobox-option-hint { color: var(--text-3) }`). The disabled
    exemption is not inherited for you.
- **`--text-3` is never the label or glyph colour of an *enabled* interactive control
  (added 2026-08-31, P95).** Button labels, segmented-control segments, tab labels, close
  buttons, hover-revealed gutter controls, chip labels and pill glyphs use `--text-2` or brighter.
  **Disabled states are exempt** — dimming *is* the disabled signal, and the `disabled` attribute
  plus `cursor: default` carry the meaning independently of colour. **Icon-only glyphs** are judged
  at the 3:1 graphics bar against their *actual composited* backdrop in **both** themes; `--text-3`
  clears 3:1 on none of `--bg-0`/`--bg-1`/`--bg-2` in the light theme, so in practice an
  icon-only control also uses `--text-2`. P95 swept the enabled-control class app-wide; a new
  occurrence is a defect. The enabled/disabled distinction survives the swap: `--text-2` vs
  `--text-3` is a **2.15:1** dark / **2.52:1** light apparent step, so an enabled `--text-2` hint
  beside a disabled `--text-3` option still reads as two states.
- **A hue is never a label colour over its own tint.** Use the hue for borders, glyphs, bars and
  fills (≥3:1 graphics bar) and `--text-1` for the words beside them. For a filled warning chip,
  `color: var(--bg-0)` on `background: var(--warning)` is safe in both themes (**6.4:1** dark /
  **4.8:1** light). Measured failures of the forbidden recipe, kept as the evidence trail — hue on
  its own 14% tint over `--bg-2`: `--danger` **3.35:1** dark / **3.48:1** light; `--accent`
  **3.68** / **3.38**; `--success` **4.07** / **3.66**; `--warning` **4.96** / **3.53**. All four
  were the four toast tones; **P74 fixed them** (§10.2 — label `--text-1` at **9.24–10.30:1** dark /
  **11.68–12.00:1** light, hue demoted to a 3px leading bar + glyph at **3.35–4.96** / **3.38–3.69**,
  clearing the 3:1 graphics bar). The same recipe on a 12% tint over `--bg-1`
  (`.submodule-badge-ok` **4.76** / **4.06**, `.submodule-badge-warn` ≈**5.4** / **3.94**) was
  already fixed in P73 by §11's pill recipe.

  **TWO retractions live here. Read both before adding anything to this bullet.**

  **(1) RETRACTED 2026-09-02 (P105): "there is no remaining sanctioned use of
  hue-as-text-over-its-own-tint anywhere in the app."** P74 fixed the four toast tones and stopped;
  it never enumerated.

  **(2) RETRACTED 2026-09-02 (P102/P105 implementation review): the replacement claim that "six
  instances survived P74."** Six was what P105's `color: var(--accent)` grep happened to surface,
  not what a scan of the recipe found. The recipe is *any* hue ink over a tint of that same hue, so
  the correct grep is over `--danger` / `--success` / `--warning` / `--merged` / `--accent`
  together. Running it found **8 in P102/P105's own scope and 16 outside it**. This is why the list
  below exists instead of a number.

  **Fixed and shipped in `0e5dcab` (8).** `.branch-name-chip` (`dialogs.css:282`, was **4.29 / 3.74**
  → `--text-1` at **11.46 / 13.25**); `.asset-chip-canonical`, `-new`, `-active`, and — found during
  implementation, in the same rule block and rendering in the same row — `-sync` (**4.02 / 3.61**)
  and `-drifted` (**4.86 / 3.50**), all six now `--text-1` on the tint (**8.99–9.67** dark /
  **11.29–11.85** light), the word carrying the meaning and a 35% hue border as **decorative
  delineation only** (`ai-assets.css:122-162`) — see the CORRECTION below; the sentence that stood
  here called that border "the identity carrier", which is retracted and was never true (35%
  measures **1.58 / 1.69**);
  `.right-pane-tab.active` (`forge-pr.css:37`, was **4.17 / 3.64** → P100 recipe 1: `--selection`
  fill, `--text-1` at **9.36 / 13.29**, `inset 0 -2px 0 var(--accent)` bar).

  **CORRECTION 2026-09-03 (P107 landing) — this list was 8, and is now 7.** The eighth entry,
  `.settings-toggle-btn.is-active` (`settings-legacy-sections.css:101`), was credited above as a
  fixed and shipped P105 site. **That rule never matched anything.** The class was orphaned in
  `7354aca`, when `SettingsSegmented` replaced the one real toggle that used it; the "fix" was applied
  to dead CSS and never rendered a pixel. The rule was **deleted** in `2168057`. It is struck from the
  shipped-fix list. See the second failure mode recorded in §2's tally — this is its first instance.

  **P107 SHIPPED 2026-09-02 in `2168057` — this section is now history, not a work queue.** The
  16-site list below is what P107's *first* pass surfaced; **the real population, once the full
  three-pass search of `docs/contracts/P107-hue-over-own-tint-ui.md` §1 was run, was 38** — 17 fixed
  (Bucket A), 19 judged compliant and kept untouched (Bucket B, glyph/bar at the 3:1 graphics bar),
  1 fixed glyph (Bucket C, `.error-dismiss`), 1 handed to P106. **Quote 38, never 16.** The 16 below
  is preserved only as the evidence trail showing how the count moved; the authoritative
  per-declaration table with bucket, verdict and measured ratio is P107 §3.

  **Was: "STILL LIVE — 16 sites, filed as P107."** Every one is `color: var(--hue)` over a 12–16%
  tint of that same hue; all are now fixed or explicitly sanctioned in P107 §3:

  | # | File:line | Selector | Hue | Tint |
  |---|---|---|---|---|
  | 1 | `forge-pr.css:298` | `.pr-mergeable-clean` | `--success` | 14% |
  | 2 | `forge-pr.css:303` | `.pr-mergeable-conflict` | `--danger` | 14% |
  | 3 | `forge-pr.css:308` | `.pr-mergeable-pending` | `--warning` | 14% |
  | 4 | `ai-assets.css:33` | `.asset-badge-ok` | `--success` | 15% |
  | 5 | `ai-assets.css:38` | `.asset-badge-warn` | `--warning` | 15% |
  | 6 | `dialogs.css:164` | `.danger-badge.safe` | `--success` | 14% over `--bg-2` |
  | 7 | `dialogs.css:170` | `.danger-badge.caution` | `--warning` | 14% over `--bg-2` |
  | 8 | `dialogs.css:176` | `.danger-badge.destructive` | `--danger` | 14% over `--bg-2` |
  | 9 | `agent-assets.css:24` | `.asset-readonly-banner` | `--warning` | 12% |
  | 10 | `agent-assets.css:50` | `.asset-issue-error` | `--danger` | 12% |
  | 11 | `agent-assets.css:55` | `.asset-issue-warning` | `--warning` | 12% |
  | 12 | `conflicts.css:61` | `.conflict-kind` | `--danger` | 12% |
  | 13 | `empty-and-errors.css:90` | `.error-banner` | `--danger` | 12% |
  | 14 | `graph-banners.css:7` | `.graph-truncated-banner` | `--warning` | 12% |
  | 15 | `settings-legacy-sections.css:487` | `.settings-ai-status-warn` | `--warning` | 12% |
  | 16 | `dialogs-forms.css:131` | `.wt-copy-chip` | `--danger` | 16% |

  **Per-site ratios were measured for all 16 (plus a 17th, `.error-boundary-title`, that the
  same-rule-block search structurally could not see) on 2026-09-02 — see
  `docs/contracts/P107-hue-over-own-tint-ui.md` §3, which supersedes the estimates that stood here.**
  Range: **3.35–6.46** dark / **3.48–4.17** light. **Every one fails 4.5:1 in the light theme**; 8
  fail in both. **The fix shipped in `2168057`** — the §11 pill recipe already applied to the
  `.asset-chip` family: keep the tint, label to `--text-1` (**9.06–12.47** dark / **11.68–14.26**
  light on those same tints), and carry the hue in a leading bar or glyph. This remains the recipe for
  any new hue-tinted chip; reach for it before inventing anything.

  **CORRECTION 2026-09-02 (P107 §7) — a 35% hue border does NOT clear the 3:1 graphics bar.** The
  sentence that stood here offered "a 35% border **or** a leading bar/glyph at the 3:1 graphics bar",
  which reads as though both options clear 3:1. Measured: `--danger` 35% over `--bg-1` is **1.58**
  dark / **1.69** light against the surrounding surface (and 1.39 / 1.42 against the tint it
  encloses); `--warning` 35% over `--bg-0` is **2.05** / **1.61**; the shipped 40% border on
  `.error-boundary` is **1.98** / **2.16**. A 35–40% hue edge is **decorative delineation** and is
  acceptable only where the *word* carries the meaning. **It is never an identity carrier and never
  the sole carrier of a state.** What does clear 3:1 is the **solid-hue** leading bar or glyph:
  `--warning` bar **6.46 / 4.17** on its own 12% tint and **7.92 / 4.87** on `--bg-0`; `--danger` bar
  **3.87 / 3.87** on its own 12% tint and **4.41 / 4.60** on `--bg-1`.

  **`--warning` as a letterform — measured in BOTH roles, and the note is now closed** (P107 §6 took
  the tint, P106 §5 took neutral chrome — **the first time in this design system that `--warning` was
  measured as a letterform on a neutral surface**). Net: **dark `--warning` is an adequate letterform
  on neutral surfaces; light `--warning` is not.**
  - **On neutral chrome** (P106): `--bg-0` **7.92 / 4.87** · `--bg-1` **7.28 / 4.54** (light passes by
    0.04 — no headroom, treat as a fail for any new use) · `--bg-2` **6.42 / 4.19** ✗ · `--selection`
    **5.03 / 3.91** ✗ · staged 6% `--success` tint over `--bg-1` **6.67 / 4.20** ✗ · changes 5%
    `--text-3` tint over `--bg-1` **6.93 / 4.34** ✗. Range **5.03–7.92** dark, **3.91–4.87** light.
  - **On its own 12–15% tint** (P107, A3/A5/A7/A9/A11/A14/A15): **4.87–6.46** dark / **3.49–4.17**
    light → fails 4.5:1 in light.
  - **As a solid fill's ink**, `color: var(--bg-0)` on solid `--warning` is **7.92 / 4.87** →
    compliant, and it is what `.ai-dock-ask-glyph` already ships.

  **So `--warning` as read text is `--warning-strong` everywhere** (MIN 5.80 / 5.73, §2 table). The
  `--*-text` trap that used to be recorded here has moved up to its own block under the token table —
  read it before choosing any ink for a hue surface.

  **Same shape, glyph bar, NOT on the P107 list — recorded so a future sweep does not re-litigate
  them.** `.checks-rollup-pill--pending .checks-rollup-glyph` (`checks-panel.css:119`, on the
  pill's own 14% `--warning` tint) and `.graph-filter-chip-stale .graph-filter-chip-glyph`
  (`graph-filter.css:93`, on a 14% `--warning` tint over `--bg-2`) are **glyphs judged at 3:1**, and
  each has a text label beside it as the carrier. Note the shape: the ink and the tint live in
  **different rules joined by a descendant selector**. That is exactly the pattern that hid
  `.pr-state-open` from P102's seed list — **a grep for `color:` and `background:` in the same rule
  block will not find these.**

  **THREE hiding patterns, not one (P107 §1 — run all three or the count will be low again).**
  (i) *same rule block* — what every prior pass ran; (ii) *descendant combinator* — the two chips
  above, found by a multiline selector grep; (iii) two patterns even (ii) misses:
  **(iii-a) an unqualified child class whose only parent is tinted** (`.dev-mode-pill-glyph`,
  `.forge-reauth-icon`, `.ai-dock-ask-guard-glyph`, `.error-dismiss`, `.error-boundary-title`) — no
  selector-shaped grep can find these, you must read the component that renders the tinted container
  and check every child's ink; and **(iii-b) custom-property indirection** — `--h` carries the hue, so
  neither `color: var(--danger)` nor a `--danger` background appears in the rule at all
  (`.toast-glyph`, `.submodule-badge-glyph`, `.ai-dock-status-glyph`, `.git-run-pill-glyph`:
  11 instances). **Token aliasing hides a fifth family:** `--badge-good` is byte-identical to
  `--success` and `--badge-warn` to `--danger` in both themes, so
  `.checks-rollup-pill--good/--warn` are this recipe under another name; **`--badge-unknown` ≡
  `--text-3` is the third alias escape (P108)** — see the ALIASING RULE above, and scope by resolved
  value, not by name. **A sixth pattern, P108: (iii-c) an *undefined* custom property with a hex
  fallback** (`var(--warn, #b8860b)`) — the hue name appears only in the fallback, so no alphabet of
  real token names can match it; grep `var\(--[a-z-]+,` and check each property is defined.
  **The hue alphabet for any
  future search is `danger|success|warning|merged|accent|badge-good|badge-warn|badge-unknown|h` —
  not the first five.** And resolve the backdrop **per state**: `.error-dismiss` passes 3:1 at rest (3.87 / 3.87)
  and **fails at 2.96 in light on hover**, because its hover fill is a deeper tint of its own ink's
  hue.

  **The rule, after P105: the only hue ink permitted over its own tint is `--accent-strong`**
  (`.conflict-action-ai:hover`, `conflicts.css:99`, measured **5.85 / 4.87** on a **14% accent tint
  over `--bg-1`** — base stated per the BASE rule above; the figure is 5.16 / 4.52 over `--bg-2`, so
  the sanction is base-specific, not blanket), and only because it is measured at ≥4.5:1 *on that
  tint over that base*. Any other hue-as-text-over-its-own-tint is a defect. **The three P106
  `-strong` tokens do not extend this sanction** — they were measured on neutral and near-neutral
  backdrops (§2 table), not on their own 12–16% tints, so putting `--danger-strong` on a `--danger`
  tint is unmeasured and therefore not permitted until someone measures it and records the base.
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
     Hovering a kept hue fill: **never `filter: brightness()`** — it moves ink and fill together, so
     it fails in whichever theme has least headroom (`.btn-primary` light went 4.65 -> 3.95). Use
     `background: color-mix(in srgb, <hue> 92%, var(--text-1))`: it brightens in dark, deepens in
     light, leaves the ink alone, and adds no literal hex (P100: **5.99:1** dark / **5.15:1** light).
     The same device is what P102 should use for `.btn-danger`.
  3. **Never dim text on a hue fill with a hardcoded `rgba(255,255,255,α)`.** P98 removed the two
     instances (**2.61:1** dark / **3.56:1** light). Subordination on a filled row is carried by
     size (11px vs 13px) and right-edge placement, never by opacity — and on a `--selection` fill
     it is carried by the real `--text-1` ↔ `--text-2` colour step.
  4. **The same defect existed for `--danger` and `--success` fills — enumerated and CLOSED
     2026-09-02 (P102, commit `0e5dcab`).** A whole-frontend sweep found **3** hardcoded-ink
     declarations in `src/styles/`, not the 2 the seed list named:
     `.pill-detached` (**`controls.css:71`** — note the correction: `controls.css:70` is the
     *detached-HEAD pill*, **not** `.btn-danger`), `.btn-danger` (**`updates.css:119`**, not
     `controls.css`), and `.pr-state-pill` (`forge-pr.css:169-183`). `#ffffff` on `--danger` is
     **3.73:1** dark; the ink flip `#16181d` is **4.80:1** — measured in the running app, both
     themes, 2026-09-02. (The "4.76" that briefly stood here was my own arithmetic slip; 4.80 is
     correct and is what the shipped comments say.) **The worst case in the whole audit is
     `.pr-state-open`: white on the dark `--success` fill at 2.85:1** — below even the graphics bar,
     and invisible to the seed list because one shared `color` on `.pr-state-pill` serves three
     different fills. **A shared ink declaration over multiple hue fills is itself the defect
     pattern** — set `background` and `color` together, per state. Shipped fixes: `--danger-text` /
     `--success-text` / `--merged-text` (§2 table). **Verified in the harness 2026-09-02, both
     themes:** OPEN **6.24** dark / **5.08** light, MERGED **5.30** / **5.05**, CLOSED **4.80** /
     **4.93**, `.pill-detached` and `.btn-danger` **4.80** / **4.93**.
     `rg -i "\bcolor:\s*(#[0-9a-fA-F]{3,8}|white)\b" src/styles` now returns **0**. A new
     hardcoded-ink-on-hue-fill anywhere is a defect; enumeration in
     `docs/contracts/P102-P105-hue-audit-ui.md` §2.
  5. **Hovering a kept hue fill uses `background: color-mix(…)`, never `filter` — shipped (P102).**
     `rg "filter:\s*brightness" src/styles` is down to **1**: `settings-primitives.css:278`
     (`.settings-switch:hover .settings-switch-track`, a fill with no ink on it, compliant at the
     3:1 graphics bar — filed as a device-consistency NIT, not a defect). The three converted sites
     measure, ink on the hovered fill: `.btn-danger` **5.18** dark / **5.49** light,
     `.diff-float-discard` **5.18** / **5.49**, `.diff-stage-float button` **5.99** / **5.15**.
- **`--accent` IS NOT A TEXT COLOUR (added 2026-08-20 P69l as a `--selection`-only rule; the
  app-wide claim RETRACTED and the full matrix measured 2026-09-02, P105).** The sentence that
  stood here — *"`color: var(--accent)` … is fine on `--bg-0` / `--bg-1` / `--bg-2`"* — was
  **false**, and it licensed **21 measurably sub-AA call sites** (of 30 total). The matrix, from the shipped hexes
  (`--accent` `#4f8cff` / `#2f6fe4`):

  | Backdrop | `--accent` d / l | `--accent-strong` d / l |
  |---|---|---|
  | `--bg-0` | **5.52** / **4.65** ✓ | 7.76 / 6.23 ✓ |
  | `--bg-1` | **5.07** ✓ / **4.34** ✗ | 7.13 / 5.81 ✓ |
  | `--bg-2` | **4.48** / **4.00** ✗ | 6.29 / 5.36 ✓ |
  | `--bg-3` | **3.89** / **3.68** ✗ | 5.46 / 4.93 ✓ |
  | `--selection` | **3.51** / **3.74** ✗ | 4.93 / 5.01 ✓ |
  | own 12–15% tint **over `--bg-1`** | **4.29–3.68** / **3.74–3.38** ✗ | 5.85 / 4.87 ✓ |
  | own 14% tint **over `--bg-2`** | — | 5.16 / 4.52 ✓ |

  **`--accent` clears the 4.5:1 text bar on `--bg-0` only.** `--bg-1` passes in dark and fails in
  light, which is not a shippable rule. It clears the **3:1 graphics bar everywhere**, so `--accent`
  as a **border, bar, glyph or fill** is compliant on every surface — the settings rail's inset bar
  (§12.1), the `inset 2px 0 0 var(--accent)` leading bar on every selected list row, the focus ring,
  the sidebar's checked-out `.branch-glyph`. Those are load-bearing and they pass; they are not
  "decorative delineation carrying no meaning". **Read text in the accent hue uses `--accent-strong`,
  which clears 4.5:1 on every surface the app has, in both themes** — that surface-independence is
  deliberate, so a text verdict never depends on resolving an ancestor chain. The focus ring must
  **not** move to `--accent-strong` (it would stop matching the brand hue and the canvas selection
  ring). The per-declaration enumeration — **30 call sites: 7 glyph keeps, 18 to `--accent-strong`,
  5 recipe changes** — is `docs/contracts/P102-P105-hue-audit-ui.md` §3, **shipped 2026-09-02** in
  `0e5dcab`. Post-fix invariants, grep-checkable and verified: `color: var(--accent)` in
  `src/styles/` is **exactly 7** (the glyph keeps), `color: var(--accent-strong)` is **exactly 18**.
  The older "**2.6:1** / **3.6:1**" `--selection` reading that stood here and below was wrong and is
  retired.
- **A hue link sitting INLINE in body text needs a resting non-colour carrier — hover-only underline
  is not enough (added 2026-09-02, found reviewing P105).** WCAG technique G183 wants **≥3:1 between
  the link text and the surrounding prose** whenever the link has no other resting distinction.
  Because `--accent-strong` is tuned to sit near `--text-2`'s luminance, moving an inline link from
  `--accent` to `--accent-strong` *reduces* that separation: `.forge-connect-link`
  (`forge-pr.css:592`, "Create a token", inline in a `--text-2` `<p>`) went from **1.43 dark /
  **1.72** light to **1.02 / 1.28** — the link and the sentence around it are now the same
  luminance in dark. The fix is `text-decoration: underline` at rest, not a colour change.
  **Rule: `--accent-strong` on a *standalone* control or a block of its own is fine; on an inline
  link inside a sentence it must carry a resting underline.** The standalone cases in the app
  (`.settings-update-link`, `.commit-parent-link`, `.blame-why`) are unaffected —
  `.settings-update-link` already underlines at rest.
- **The specificity trap (added 2026-09-02, found implementing P105 C5 — sibling to the child-rule
  trap above).** When a state rule changes from setting only `color` to setting `background`, it
  starts competing with the base component's hover rule, which is usually **more specific**.
  `.settings-toggle-btn.is-active` is (0,2,0); `.btn-secondary:hover:not(:disabled)`
  (`updates.css:107`) is (0,3,0) and wins, so the selected fill would vanish on hover and the state
  would hang on the border alone. **Whenever a selected/active rule gains a `background`, add the
  matching `…:hover:not(:disabled)` rule at equal-or-higher specificity**, lifted with
  `color-mix(in srgb, var(--selection) 92%, var(--text-1))` so hover still gives feedback
  (**7.55:1** dark / **11.42:1** light for a `--text-1` label). Check the base component's hover,
  focus, active and disabled rules — not just its default.
  **The lesson is live; the example selector is not.** `.settings-toggle-btn.is-active` was deleted in
  `2168057` (it had matched nothing since `7354aca`) — do not grep for it. The rule generalises to any
  selected/active state rule, e.g. `.right-pane-tab.active` over `.right-pane-tab:hover`.
- **A `var(--x)` that is not defined silently deletes its declaration.** `var(--border-0)` — never a
  real token — appeared 5× in `src/styles/conflicts.css` and computed to
  `border-style: none; border-width: 0px`, so the merge editor's split-label dividers simply did not
  render (found and fixed 2026-09-01, P98 §4). There is **one** border token: `--border`. Before
  shipping a new stylesheet, confirm every `var(--…)` it names resolves. Note the box-model
  consequence when repairing one: `border-width: 1px` with `border-style: none` has a **used width
  of 0px**, so restoring the style adds 1px of real space — small, but state it rather than claiming
  a zero-layout change.

Additional measured pairs (2026-08-19, P70 pass), all on `--bg-1`: `--text-1` **13.5:1** dark /
**15.4:1** light; `--warning` glyph **7.3:1** / **4.5:1**; `--success` glyph **5.7:1** / **4.7:1**;
`--danger` glyph **4.4:1** / **4.6:1** — all clear the 3:1 graphics bar in both themes. And
(2026-08-19, P73 pass) `--text-2` over its **own** 12% tint on `--bg-1`: **5.79:1** dark /
**6.22:1** light — the safe recipe for a hueless informational pill (§11).

Measured pairs added 2026-08-19 (P69 Settings pass), on `--bg-0`: `--accent` fill **5.6:1** dark /
**4.7:1** light; `--text-2` fill **7.9:1** / **4.9:1**; `--text-1` on `--selection` **9.4:1** /
**13.3:1**; `--accent` 1px border on `--bg-2` **4.4:1** / **4.1:1**. `--accent` on `--selection` is
**3.51:1** / **3.74:1** (corrected P101 — the "2.6 / 3.6" that stood here was wrong): it clears the
3:1 graphics bar in both themes, so it is a valid non-text carrier, but never text. And (P69c pass)
`--warning` as a 1px ring on `--bg-2`, the input fill: **6.4:1** dark / **4.2:1** light — clears the
3:1 graphics bar (§12.3.4).

Measured pairs added 2026-08-20 (P74 pass), `--text-1` over a hue's own 14% tint on `--bg-2` — the
canonical "hue surface, readable words" pair: **9.24–10.30:1** dark, **11.68–12.00:1** light,
across all four of `--danger` / `--success` / `--warning` / `--accent`. Use these numbers for any
new tinted surface that must carry prose.

**Dimming budget (added 2026-08-20, P69j pass).** `opacity: .55` on `--text-1` over `--bg-0` lands
at ≈**4.2:1** dark / **3.6:1** light — acceptable *only* on genuinely inert controls (WCAG 1.4.3
exempts inactive components). It is a budget, not a free knob: **it may be spent once per subtree**.
Two nested `.55` layers compound to **.30**, which is ≈2.5:1 dark / ≈1.8:1 light and unreadable in
both themes. See §12.3.3. **And it may only be spent on something the user cannot act on:** dimming
a control that is still clickable buys the contrast loss with none of the exemption — the P69k rail
counts started as `opacity: .5` on clickable zero-count tabs (**2.87:1** dark / **2.36:1** light) and
had to be replaced by inverted emphasis (§12.5).

## 3. Typography & spacing

- UI font: `"Segoe UI Variable", "Segoe UI", system-ui, -apple-system, sans-serif`.
- Mono (hashes, paths, diffs): `"Cascadia Code", "Cascadia Mono", Consolas, "JetBrains Mono", monospace`.
- Sizes: base 13px / line-height 1.45; secondary 12px; section labels 11px; header app name 14px;
  diff/mono 12px. Weights: 400 normal, 600 emphasis; never bolder.
- Spacing scale (margins/padding/gaps): 4 / 8 / 12 / 16 / 24 px only.
- Border radius: 6px (buttons, panels, inputs), 999px (pills).
- Density: the `panelDensity` setting (`cozy` | `compact`) is applied as `data-density` on a
  container which redefines a `--<scope>-*` custom-property block; every consumer reads
  `var(--x, <pre-density fallback>)`. Precedents: `--rp-*` on `.right-panel` (P67b),
  `--ai-dock-*` on `.ai-dock` (P68e). **Scope:** the right panel and the dock only — the sidebar,
  the Settings overlay, dialogs, and app chrome (header, workspace toolbar, the §10 notice bar) have
  one geometry in both densities.

### 3.1 Hit-target floor (WCAG 2.2 · 2.5.8)

- Every interactive control is **≥24 × 24 CSS px**, in every density and both themes. There is no
  compact-mode escape hatch outside the two density scopes named above.
- **The box grows, the glyph does not.** Enlarge the transparent/hover box around an icon and leave
  the painted glyph at its designed size. Canonical sizes: `.btn-icon` **32×32** around a 14–16px
  glyph (header toolbar); `.sidebar-add` **24×24** around a 14px `+` and a 14×14 SVG;
  `.settings-switch` **36×24** where the invisible `<input>` *is* the target. Never inflate the
  glyph to reach the floor — that changes visual weight and density.
- **A text button reaches the floor with padding plus a negative margin, not with height.** When a
  small text link must stay optically flush with a container edge (`.settings-results-goto`, §12.5:
  `min-height: 24px; padding: 0 6px; margin-right: -6px; display: inline-flex; align-items: center`),
  the padding grows the hit box and the equal negative margin gives the alignment back. This is the
  one sanctioned negative margin — it pays for a hit target, it does not claw back gutter (contrast
  the P74 SF-1 rule below).
- **Prefer `align-self: stretch` over a hardcoded height** when the control sits in a row that
  already owns a height (`.sidebar-section-toggle` in a 24px `.sidebar-section-header`,
  `.tree-dir-toggle` in a `.tree-dir-row`). The control then tracks the row and stays correct when
  a density block changes the row height.
- **A stretched toggle's hover wash must share its list rows' left edge.** When a full-width control
  gains a hover background, that rectangle becomes a visible alignment edge: keep it flush with the
  sibling rows below it (`.branch-row` at the pane's 12px gutter) and never let it bleed into the pane
  gutter. Pad the control for breathing room; do not claw the padding back with a negative margin
  (P74 SF-1).
- **Sidebar geometry (P74).** One 24px module for everything: `.sidebar-section-header` 24,
  `.sidebar-section-toggle` 24 (stretched), `.sidebar-add` 24×24, `.list-filter-clear` 24×24,
  `.list-filter-input` 24, `.branch-row` 24, `.sidebar .tree-dir-toggle` 24, `.error-dismiss`
  24×24, `.toast-dismiss` 24×24. `.sidebar-section` keeps `margin-bottom: 16px` and `.branch-list`
  `margin-top: 4px`. Before P74 the toggles were 16px, the action buttons 20×20, and the Tags
  header 16px against the others' 20px — the fix also removed that rhythm inconsistency.
- **A newly enlarged target must announce itself.** `.sidebar-section-toggle:hover` paints
  `background: var(--bg-2)` with `border-radius: 4px` — the same wash `.branch-row:hover` and
  `.sidebar-add:hover` already use, so no new idiom. *Decided by the orchestrator (P74 OPEN-1,
  2026-08-20) and recorded here because it is a visible change the user did not ask for:* a
  full-width 24px control with no hover feedback is a worse affordance than the 16px one it
  replaced, since nothing tells the user the whole header row is clickable. Enlarging a hit target
  without a matching hover state is half a fix.
- **Known exemption:** `.right-panel[data-density='compact']` sets `--rp-row-h: 20px`, so
  `.tree-dir-row`/`.tree-dir-toggle` are 20px there. That is a deliberate user opt-in inside a
  density scope; raising it would delete the point of `compact`. Do not "fix" it silently, and do
  not copy it to any surface outside that scope.
- Row hover-action buttons (`.row-action`, §7.1) are 20×20 and are the one other standing
  exception: they live inside a 24px row, are revealed on hover, and sit in a 2px-gap cluster where
  24px boxes would collide. Treat the **row** as the target for pointer purposes.

## 4. Commit graph metrics (canvas)

Canonical numbers live in `src/graph/metrics.ts` (`METRICS` = cozy baseline, `COMPACT` = compact
preset); the three user knobs `avatarRadius` / `rowHeight` / `laneWidth` vary at runtime. This
section mirrors them — update both together.

- **Row height:** **32px** (cozy) / **22px** (compact). Lane width (x-spacing between lane centers):
  **16px**. Left graph gutter: 12px before lane 0. A fixed **180px** left ref-column band
  (`refColWidth`) precedes the graph and carries the ref pills (§6).
- **Forge column** (§6.1): fixed **74px** (`forgeColWidth` = `ciBadgeSize` 11 + `signalGap` 6 +
  `prBadgeMaxWidth` 56 + 1px slack); PR pill max width **56px** (`prBadgeMaxWidth`, was 46 — +10px
  for the leading PR-state glyph). Reserved only when forge data is present; suppressed in compact.
- **Commit node = author avatar, not a bare dot.** A filled disc of radius **10px** (cozy) / **8px**
  (compact) at the commit's lane x, hue-hashed from the author (HSL sat 52 / light 42 —
  theme-invariant, legible on both backgrounds) with the author's 1–2 initials in 600 11px. Behind
  it a **2px** (`avatarBgRingExtra`; 1px compact) `--bg-0` halo so edges passing under read cleanly;
  over it a **1.5px** lane-color ring tying the node to its lane.
- **Selected commit:** an extra outer ring at radius `avatarRadius + 3.5` (→ 13.5px cozy) in
  **`--accent`**. **HEAD commit:** an outer ring at radius `avatarRadius + 2.5` (→ 12.5px cozy) in
  `--text-1`. Both radii derive from `avatarRadius`, so they stay outside the disc at any node-size
  knob value.
- **Search-match:** an outer ring in `--match-ring` at a radius distinct from the selection/HEAD
  rings, so a match stays spottable while scrolling.
- **Edge stroke:** **2px**, round caps, color = the edge lane's color from the **per-theme** palette
  (§5). The **Bonsai graph style** (§5.1) steps edge width per bezier segment (thinner toward the
  tip, thicker toward the trunk) without moving any endpoint — see §5.1 / `002-bonsai-graph-theme-ui.md` §3.
- **Fork/merge curve:** cubic bézier between (x1, y1) and (x2, y2) of adjacent rows with control
  points `(x1, y1 + rowHeight/2)` and `(x2, y2 − rowHeight/2)` — vertical tangents at both ends,
  GitKraken-style S-curve. Straight vertical segments elsewhere.
- **Fold-pill display rows (spec 004):** with "Fold linear runs" on, a collapsed linear run renders
  as one uniform-height display row — dashed lane connector + a "⋯ N commits" pill in the §6
  local-branch recipe on the run's lane colour, painted by `src/graph/drawFold.ts`. Full spec
  (geometry, states, keyboard, re-collapse): `docs/contracts/spec-004-ui.md`.
- **Right of the graph:** ref pills (§6), then the commit summary (`--text-1`, `summaryFont` 13px
  cozy / 12px compact, truncated), then the metadata pack — a leftmost **forge column**
  (`forgeColWidth` 74px, adjacent to the summary) followed by the optional author / relative-date /
  short-SHA columns (`--text-2`, `metaFont` 12px cozy / 11px compact) as space and the per-row
  display toggles allow. The forge column holds the row's CI dot + PR pill (§6); it is reserved
  only when forge data is present (else the summary reclaims its width) and is suppressed in
  compact mode. PR/CI signals no longer render in the left ref band — only the ahead/behind chip
  stays there.
- **HiDPI:** canvas backing store scaled by `devicePixelRatio`; all metrics above are CSS px.

### 4.1 Keyboard & screen-reader access (added 2026-08-22, graph review; ARIA model revised 2026-08-31, P95; reconciled with spec-004 on 2026-09-02)

The `<canvas>` is opaque to assistive tech and the graph is virtualized to visible rows, so the rows
themselves are pixels, not DOM. The graph is **not** a grid or table: it is a single labelled,
focusable container whose selection is announced through a live region, plus **one** visually-hidden
element describing the currently active row.

- The scroller (`.graph-scroll`) is the single tab stop and carries **exactly**: `tabIndex={0}`,
  `role="group"`, `aria-label="Commit graph"`, `aria-describedby` pointing at the keyboard hint
  below, and `aria-activedescendant` pointing at the active-row element below (omitted when there is
  no active row). It shows a `:focus-visible` ring (2px `--accent`, 1px offset, inset so the canvas
  does not clip it), distinct from the per-row `--accent` selection ring above.
- **`role="grid"`, `aria-rowcount`, `role="row"` and `aria-rowindex` are forbidden here** (settled by
  P95, reaffirmed 2026-09-02). A `grid` with no `role="row"` children is malformed, and the row
  attributes are only meaningful under a grid/table role. The rejected alternatives — a
  visually-hidden row per *visible* row, and a one-option `listbox` — are recorded in
  `docs/contracts/P95-a11y-ui.md` §1.1.
- **`aria-activedescendant` is kept and is valid** (amended 2026-09-02, spec-004 merge). ARIA 1.2
  lists it as supported on `role="group"`, and the IDREF resolves to a real element: spec-004 renders
  exactly **one** `.sr-only` `<div id="graph-row-{d}">` inside the scroller, re-rendered for the
  current active **display** row, carrying that row's accessible name plus `aria-selected` and
  `aria-expanded`. One node, not one per visible row — it moves with the active row instead of
  churning on scroll, so it neither dangles nor costs render budget. It carries **no role**.
- A visually-hidden `.sr-only` span (id from `useId`, so multiple repo tabs do not collide) is the
  described-by target and reads: **"Use the arrow keys to move between commits. Press the Menu key
  or Shift+F10 for actions on the selected commit."**
- A permanently-mounted polite live region is the **sole** announcement channel — one utterance per
  settled selection, never two. `GraphSelectionAnnouncer` (mounted by `WorkspaceGraphPane`, wrapping
  the P84 `RevealAnnouncer`) announces
  `"{summary} — {author}, {relative date}. Row {n+1} of {N}. {ref summary}"`, debounced ~150 ms so a
  held arrow key does not flood the reader. The `Row {n+1} of {N}` clause is what `aria-rowcount`
  used to attempt, delivered where the user actually hears it.
- **The row counts the announcer speaks are display rows (amended 2026-08-26, spec 004).** With
  "Fold linear runs" on, model row indices lie to assistive tech, so the `Row {n+1} of {N}` clause
  is computed from **display** indices and `foldModel.displayRowCount` (display == model when fold
  is off, so unfolded behaviour is unchanged). A fold-pill row is a normal selectable row for
  keyboard nav and announces its own accessible name instead of a commit summary — see
  `spec-004-ui.md` §3.
- **Keyboard nav must be able to START with no prior selection.** The nav keys live on a
  **window-level** handler (`useWorkspaceKeyboard`), deliberately, so arrows work without first
  tabbing to the graph. When nothing is selected, the first
  ArrowDown/ArrowUp/Home/End/PageUp/PageDown selects an anchor — `headIndex` if it is in loaded
  history, else `0` (Down/Home) or the last row (Up/End). Thereafter Arrow = ±1 row, PageUp/Down =
  ±visible-row-count (`getVisibleRowCount`), Home/End = first/last. Selection scroll-into-view
  already exists (`viewport.ts:scrollRowIntoView`).
- **Focus follows consumption (P95).** Whenever that window-level handler *consumes* a nav key to
  change the graph selection — precisely the branches that call `e.preventDefault()` — it must also
  call `GraphCanvasHandle.focusScroller()` (`scroller.focus({ preventScroll: true })`;
  `preventScroll` is required so the browser does not fight `scrollRowIntoView`). Without this, a
  user can select a row while focus sits on `<body>`, and every key the scroller owns — the row menu
  below — is unreachable. The handler's existing `typing` guard and its
  dialog/palette/search/composer bail-out run first, so the graph never steals focus from an open
  overlay.
- **Menu key / Shift+F10** on the focused graph scroller opens the **selected** row's context menu,
  anchored at the ref band's left edge just under that row (clamped to the scroller's box). This is
  the keyboard route to the P92 ref picker, and therefore to every ref on a multi-ref commit. Esc
  restores focus to the scroller.
- Clicking a commit in the graph leaves focus in the scroller, including while a centre overlay is
  open (P93) — the click path is unchanged by P95.

### 4.2 Graph right-edge overlay stack (added 2026-08-26, spec 005)

Native scrollbar (never covered; overlays sit at `right: rightInset`) → `.graph-rail` overview
rail `z-index: 4` → search/filter fabs and filter chip `z-index: 5`. Fabs win pointer events in
the top ~40px overlap band. New right-edge overlays slot into this stack rather than inventing
z values. The replay fab (spec 007) is the third member of the fab cluster, leftmost; the replay
overlay itself sits above everything in the pane at `z-index: 6`. Replay's frontier motion
reuses the revealFlash pattern (600ms, +5px halo) — no new motion vocabulary.

## 5. Lane color palette (deterministic, per theme)

Assigned by `lane % 10`, computed in Rust with the layout; stable while scrolling by construction.
**Revised 2026-08-22 (graph review):** the palette is now **theme-specific**. The previous
single-palette rule left six of ten lanes below the 3:1 graphics bar against the light `#ffffff`
background (measured: orange 2.23, green 2.48, teal 2.09, yellow 1.71, pink 2.83, lime 2.16). The
dark palette is unchanged; the light palette darkens each hue to clear 3:1 vs white while preserving
hue order and identity. The draw layer selects the palette by resolved theme.

| # | Hue | Dark hex (vs `#16181d`) | Light hex (vs `#ffffff`) |
|---|---|---|---|
| 0 | blue   | `#4f8cff` (5.52:1) | `#2f6fe4` (4.65:1) |
| 1 | orange | `#f2994a` (7.98:1) | `#b0530f` (5.13:1) |
| 2 | purple | `#9b6dff` (5.10:1) | `#7b46d6` (5.69:1) |
| 3 | green  | `#43b97f` (7.18:1) | `#1b7d4c` (5.14:1) |
| 4 | red    | `#e5534b` (4.80:1) | `#c62f33` (5.45:1) |
| 5 | teal   | `#3ec6c0` (8.49:1) | `#0c7d78` (4.98:1) |
| 6 | yellow | `#e8c341` (10.41:1) | `#8a6f08` (4.82:1) |
| 7 | pink   | `#f26d9c` (6.29:1) | `#c8437a` (4.63:1) |
| 8 | indigo | `#7a86ff` (5.65:1) | `#5560e0` (5.08:1) |
| 9 | lime   | `#8fbf4d` (8.23:1) | `#517c20` (4.94:1) |

Ratios are the lane color against that theme's `--bg-0` (2px stroke / dot fill) — all **≥4.6:1**,
comfortably clearing the 3:1 graphics bar (WCAG 1.4.11). Do **not** reuse the dark hex in light mode.
The light values double as the current-branch pill background (§6), so each also carries white pill
text at ≥4.5:1.

### 5.1 Bonsai graph style — palettes & backdrop (spec 002)

The **Bonsai graph style** is a selectable third look (Settings → Appearance → Graph style), a
**reskin only** — identical topology, ordering, stable lanes, ref pills, and virtualized scroll. It
has its **own light and dark lane palettes** and a warm paper/soil backdrop. Full visual spec (node
blossom, edge taper, sway, seasons, settings control, all states): `docs/contracts/002-bonsai-graph-theme-ui.md`.
Only the token/palette **canon** lives here.

Backdrop base colors (also the halo fill; `--graph-canvas-bg` override, §2): dark **`#17140f`**
(L≈0.080), light **`#f4efe6`** (L≈0.939). Palettes are graph-layer constants
(`LANE_COLORS_BONSAI_DARK` / `_LIGHT`) mirroring the `LANE_COLORS_*` precedent — **not** CSS vars.
Ratios use the codebase's non-gamma `relLuminance` (the exact function `adaptivePillText` branches
on); true-WCAG margins are wider, so these are conservative.

**`LANE_COLORS_BONSAI_DARK`** — bright foliage on warm soil. All L≈0.66–0.78, so `adaptivePillText`
picks near-black `#16181d` (pill ratios ≥11.7:1); vs backdrop `#17140f` all ≥5.0:1.

| # | Name | Hex |  | # | Name | Hex |
|---|------|-----|--|---|------|-----|
| 0 | sage green | `#86c5b0` |  | 5 | jade teal  | `#7fccc4` |
| 1 | warm sand  | `#e3c07a` |  | 6 | gold ochre | `#ddc85f` |
| 2 | lilac bloom| `#c3a6e0` |  | 7 | rose bloom | `#e6a6bf` |
| 3 | moss leaf  | `#9cc873` |  | 8 | wisteria   | `#a3aee6` |
| 4 | clay       | `#e39b83` |  | 9 | young lime | `#bcd17a` |

**`LANE_COLORS_BONSAI_LIGHT`** — deep bark/forest ink on paper. All L≈0.16–0.17, so
`adaptivePillText` picks white `#ffffff` (pill ratios ≥4.8:1); vs backdrop `#f4efe6` all ≥5.3:1. Do
**not** brighten past L≈0.18 or white pill text drops below 4.5:1.

| # | Name | Hex |  | # | Name | Hex |
|---|------|-----|--|---|------|-----|
| 0 | deep pine  | `#123330` |  | 5 | deep teal   | `#0b3038` |
| 1 | bark umber | `#45280e` |  | 6 | dark ochre  | `#332907` |
| 2 | plum bloom | `#372440` |  | 7 | deep rose   | `#4e1e30` |
| 3 | forest grn | `#163419` |  | 8 | deep indigo | `#24284e` |
| 4 | deep clay  | `#501e16` |  | 9 | dark olive  | `#2c3009` |

Seasons (Living/Spring/Autumn) shift only the blossom accent + backdrop tint, **never the 10 lane
hues**, so every ratio above holds across seasons. Fixed semantic colors (`STASH_COLOR`,
`TAG_COLOR`, `DETACHED_HEAD_BG`) stay fixed in Bonsai. Full tables + rationale in the 002 contract.

### 5.2 Author color mode (spec 006)

A persisted alternative to lane coloring (`graphColorMode: 'lane' | 'author'`, Settings →
Appearance → "Graph colors"). In author mode, edges and the avatar lane ring take the **child
commit's author hue**: `authorHue(name) = FNV-1a(name.trim()) % 360` — the identical hash behind
`avatarColor`, so edges and avatar discs share one hue identity per author. Per-theme S/L
constants (graph-layer constants in `src/graph/authorColor.ts`, not CSS vars; one pair serves
both graph styles):

| Resolved theme | Formula | Worst-case hue | Measured worst contrast |
|---|---|---|---|
| Dark | `hsl(h, 60%, 65%)` | h≈240 (blue/violet) | ≈4.3:1 vs `#16181d`, ≈4.4:1 vs `#17140f` |
| Light | `hsl(h, 60%, 33%)` | h≈60 (yellow) | ≈3.9:1 vs `#ffffff`, ≈3.4:1 vs `#f4efe6` |

All hues clear the 3:1 graphics bar on every background. Do not raise light L past 33 (yellow
falls under 3:1 vs the Bonsai paper backdrop) and do not reuse `AVATAR{52,42}` for edges (those
constants are tuned for white initials on the disc, not for background contrast). Avatar discs,
pills, backdrops, and semantic colors are unchanged in author mode. Ring stacking + parent
highlight: `spec-006-ui.md` §2.3.

## 6. Ref pills (beside commit message)

Shape: 999px radius, `pillFont` 11px / 600 weight, padding 2px 8px, height 18px cozy / 15px compact,
max-width 160px with ellipsis, 4px gap between pills. Rendered on the canvas in graph rows, in the
left ref-column band.

| Kind | Style |
|---|---|
| Local branch | bg = lane color at 18% alpha; text + 1px border = lane color |
| Current branch (HEAD attached) | solid lane-color bg (per-theme, §5), **luminance-adaptive text** (below), prefix `⌂ ` |
| Remote branch | bg `--bg-2`, text `--text-2`, 1px `--border`; label `origin/name` |
| Tag | bg `#d4a72c` at 18% alpha, text + border `#d4a72c`, prefix `# ` |
| HEAD (detached) | solid **`#b3261e`** bg (fixed, both themes), white text, label `HEAD` |

**Luminance-adaptive pill text (added 2026-08-22, graph review).** The current-branch pill sits on a
lane color, so a fixed white label failed catastrophically on the bright lanes (white on dark-mode
yellow = 1.71:1). The label color is chosen per-pill as whichever of near-black `#16181d` or white
`#ffffff` has the higher contrast with the lane background (`isDarkBg()` already exists in
`GraphCanvas.tsx:66`). Result: near-black on the dark-mode (bright) lanes = **4.8–10.4:1**; white on
the light-mode (darkened) lanes = **4.6–5.7:1** — all clear the 4.5:1 text bar in both themes.

**Detached HEAD** uses a fixed dark red `#b3261e` background (not `--danger`, which gave white text
only 3.70:1 in dark) — white on `#b3261e` is **6.54:1** in both themes. The `HEAD` label word
carries the meaning; the color is secondary.

### 6.1 Forge column — PR pill + CI dot (PR-badge-placement)

The forge PR pill and CI status dot render in the dedicated **forge column** (§4, leftmost of the
metadata pack) — **not** in the left ref band. Intra-column layout is left-aligned at the column's
left edge: CI dot centered at `leftX + ciBadgeSize/2`, then the PR pill (`signalGap` after the dot,
or hugging `leftX` when there is no CI). The pill is `pillHeight` tall, `prBadgeMaxWidth` 56px max.
The row shows the signals of its first branch entity that carries any.

PR pill fills: open `--badge-good`, merged `#8957e5` (fixed violet, both themes), closed
`--badge-warn`, draft grey **outline** (`--bg-2` fill, `--text-3` border, `--text-2` label).

**PR-state glyph (non-colour carrier).** Colour alone must never carry PR lifecycle (§7 house rule).
A leading glyph sits inside the pill, before `#num`, in the label colour. This is a distinct
extension of the house glyph vocabulary (§7), not a synonym of the CI dot glyphs:

| State  | Pill                  | Glyph | Meaning carrier |
|--------|-----------------------|-------|-----------------|
| open   | filled `--badge-good` | `○`   | hollow ring = active/open |
| merged | filled `#8957e5`      | `◆`   | filled diamond = merged |
| closed | filled `--badge-warn` | `✕`   | house dismiss/close glyph |
| draft  | grey **outline**      | `○`   | open family, distinguished by outline fill |

CI dot glyphs are unchanged: `✓` success · `✕` failure/error · pending dot · neutral dash.

**A11y:** the canvas is opaque, so the settled row live-region announcement (§4.1) appends the
forge signal when present: `" PR #{n} {state}."` and `" Checks {rollup}."`.

### 6.2 Overflow "+N" chip and the ref-picker menu (P92)

Full contract: `docs/contracts/P92-multi-ref-commit-ui.md`. When a commit carries more refs than the
180px band fits, the trailing `+N` chip is **interactive**, not a dead label.

- **Chip styling is unchanged** — `--bg-2` fill, `--text-2` label, 1px `--border`, `pillHeight` tall.
  No new token. Hovering keeps the existing tooltip (the hidden ref names, one per line).
- **Hover reads, click acts.** Left-click and right-click both open a `ContextMenu` anchored to the
  chip, headed `{n} more refs`, with one row per hidden ref entity; each row's flyout is that ref's
  own menu (branch / tag / stash), identical to what its pill would open. Chip right-click no longer
  falls through to the first branch on the row.
- **Same pattern in the commit menu.** When a commit carries ≥2 actionable refs, the row's context
  menu prepends the same one-row-per-ref picker above the commit actions, so a branch-scoped verb
  (Merge, Rebase, Reset, Delete…) always names the branch the user chose. With ≤1 candidate the menu
  stays flat and byte-identical to before — the picker level never appears for the common case.
  Ordering is `groupRefs` order (HEAD, locals, remote-only, tags, stashes), so the menu and the pills
  always agree; a local branch and its same-commit remote are one row, not two.
- **Picker rows have no default action** — they omit `onSelect` (`ContextMenu.activate()` fires
  `onSelect` and closes when one is present, and only otherwise toggles the flyout), so clicking one
  opens its flyout and never mutates.
- **Context menus are height-clamped, app-wide** — and the clamp only works together with its
  companion rules below. The first shipped version had the clamp alone and was broken (P92 review
  2026-08-31); this is what ships now, in `src/styles/context-menu.css`:
  ```css
  .context-menu, .context-menu--sub {
    max-height: min(60vh, 480px);
    overflow-y: auto;
    overflow-x: hidden;          /* `overflow-y: auto` alone computes overflow-x to `auto` */
    overscroll-behavior: contain;
  }
  .context-menu--sub { position: fixed; }
  ```
  Previously an unclamped menu (12+ refs, or any long list) could run off-screen. The clamp requires
  all four of:
  1. **`overflow-x: hidden` is explicit, not incidental.** `overflow-y: auto` computes `overflow-x`
     to `auto`, which gave every clamped menu a spurious horizontal scrollbar.
  2. **The scroll-dismiss handler must ignore scrolls originating inside the menu root.**
     `window.addEventListener('scroll', …, true)` receives the menu's own scroll box even though
     `scroll` does not bubble, so an unguarded handler closed the menu the instant the user wheeled
     it — and also on `focusRow`'s `scrollIntoView` during arrow-key navigation. Guard with
     `rootRef.current.contains(e.target as Node)`, as the pointerdown handler already does.
  3. **Flyouts escape the scroll box via `position: fixed`.** A scroll container clips
     absolutely-positioned descendants, so an `absolute` `.context-menu--sub` inside a
     `.context-menu-row` was clipped and made the browser scroll the parent sideways to reveal it.
     The flyout is positioned from the anchor row's `getBoundingClientRect()`: `left = rowRect.right`,
     right-flipping to `max(4, rowRect.left - width)` when `rowRect.right + width > innerWidth - 4`,
     and `top = clamp(rowRect.top, 4, innerHeight - 4 - height)`. It is rendered
     `visibility: hidden` until measured, so it never flashes at the wrong spot.
  4. **A fixed flyout no longer tracks its row, so the parent closes its own open flyout when the
     parent's scroll box scrolls** (hover or ArrowRight reopens it). Scrolling a *submenu* never
     closes it — the guard is scoped to the list's own box.

  Two further invariants of the component, recorded here because they are design contract, not
  implementation detail:
  - **Focus restores to the opener on close.** The element focused at first render is captured and
    refocused on unmount, but only if focus is still inside the menu (never steal focus back from
    somewhere the user has since moved).
  - **`separator?: true` is the canonical separator primitive.** A menu item with `separator: true`
    renders a non-interactive `role="separator"` divider; it is skipped by arrow-key navigation and
    by index-based row resolution. Do not spec ad-hoc divider rows or blank labels.
- **Keyboard parity.** The canvas chip is not a tab stop; the keyboard path to every hidden ref is the
  selected-row context menu (§4.1), which contains the picker. Menu roles/keys are the component's
  existing `role="menu"` / `aria-haspopup` / Arrow-key behaviour.
- **Density/theme:** menu chrome is density-invariant (§3); the chip follows the §6 pill metrics. All
  colours are existing tokens in both themes.

## 7. File status colors (right panel, M1+)

**Current, as shipped (P106, `10ce967`, 2026-09-03).** The A/M/D/U/R letter badge is mono 11px / 600,
12px wide, before the path, in **both** densities (it does not scale with `--rp-row-font`). Its ink is
a `--*-strong` token in every case — the base hues remain correct for fills, bars and glyphs, but
**not for this letter**:

| Status | Letter | Ink | Declaration |
|---|---|---|---|
| added | `A` | `--success-strong` | `status-panel.css:193` |
| untracked | `A` **in 3 of 6 tables, `U` in the other 3 — see the correction below** | `--success-strong` (italic path) | `status-panel.css:220` |
| modified | `M` | `--warning-strong` | `status-panel.css:198` |
| typechange | `T` | `--warning-strong` | same rule |
| deleted | `D` | `--danger-strong` | `status-panel.css:203` |
| conflicted | `C` | `--danger-strong` | same rule |
| renamed | `R` | `--accent-strong` (P105, `0e5dcab` — unchanged by P106) | `status-panel.css:214` |
| — | `Conflicts` section label | `--danger-strong` (**7.62 / 6.01** on `--bg-1`) | `status-panel.css:231` |

**Never let color be the only carrier of meaning** — the A/M/D/U/R letter badge is the house
precedent. Every new status indicator pairs its hue with a letter, word, or glyph. A **digit** counts
as a carrier too (§12.5's rail counts: `0` vs `N` is what makes the colour lift optional).

**But the letter badge is TEXT, so it is judged at 4.5:1, not 3:1** (P105, 2026-09-02; the whole
family closed by P106, 2026-09-03). The letter carrying the meaning does not exempt the colour from
the text bar. **P106 determined that at all 8 render sites the letter is the *sole* non-colour carrier
of the status** — shape and position are identical across statuses, no word in the row names the
status, and 11px/600 is far below the large-text threshold — so **nothing in this family is exempt**,
and a `Conflicts` section header does *not* excuse the `C` letter.

**The pre-fix state, kept as the evidence trail** (dark / light, per composited backdrop). Bold = below
4.5:1:

| Pre-fix ink | `--bg-0` | `--bg-1` | `--bg-2` hover | `--selection` | staged tint | changes tint | MIN |
|---|---|---|---|---|---|---|---|
| `--success` (`A`, `A`) | 6.24 / 5.08 | 5.73 / 4.74 | 5.06 / **4.37** | **3.96 / 4.08** | 5.26 / **4.38** | 5.46 / 4.53 | **3.96 / 4.08** |
| `--warning` (`M`, `T`) | 7.92 / 4.87 | 7.28 / 4.54 | 6.42 / **4.19** | 5.03 / **3.91** | 6.67 / **4.20** | 6.93 / **4.34** | **5.03 / 3.91** |
| `--danger` (`D`, `C`) | 4.80 / 4.93 | **4.41** / 4.60 | **3.89 / 4.24** | **3.05 / 3.96** | **4.04 / 4.26** | **4.19 / 4.40** | **3.05 / 3.96** |
| `--accent-strong` (`R`) | 7.76 / 6.23 | 7.13 / 5.81 | 6.29 / 5.36 | 4.93 / 5.01 | 6.54 / 5.37 | 6.78 / 5.55 | 4.93 / 5.01 ✓ |

**Resolve the backdrop PER STATE — this family is the strongest evidence for that rule anywhere in the
app.** `D` measured **4.41 at rest, 3.89 hovered, 3.05 selected**: the worst figure in the app was the
state the user is in *while reading the diff*. `M` was compliant in dark everywhere and failed in
**4 of 6** backdrops in light. The six live backdrops are `--bg-0`, `--bg-1`, `--bg-2`, `--selection`,
the staged section's 6% `--success` tint over `--bg-1`, and the changes section's 5% `--text-3` tint
over `--bg-1` — `.file-row:hover` and `.file-row-expanded` are **opaque**, so they replace the section
tint entirely, which is what keeps the matrix small.

**Post-fix: MIN 5.18 across the whole family** (§2's `-strong` block). Measured live in a real browser
across all **8** render sites, both themes, compositing the full ancestor stack rather than assuming
the nearest declared background. `--text-1`-for-everything was measured (MIN 9.36 / 13.29) and
**rejected**: only *states* demote to neutral, an identity the user *scans by* keeps its hue and flips
the ink — five grey letters would delete a working scanning aid (find the deletions by colour in a
200-file list, then read the letter to confirm).

**A FOURTH search pass was added by P106 over P107's three — imperative canvas rendering**
(`rg "FileStatus" src/graph` → 0). Run it on any visual family from now on; "the search could not see a
whole class" is how both prior counts in this programme went wrong.

**Live-match confirmation — all 8 render sites reached, per site, with the route that reaches each**
(P106 AC10, resolved 2026-09-03). A grep proves a declaration *exists*; only this proves it
*renders* (§2, second failure mode). Four sites were unconfirmed at contract time and are now
individually recorded — **no "unverified" qualifier survives on this family**:

| Site | File | Route in the harness | Confirmed |
|---|---|---|---|
| S1 / S2 | `StatusFileRow.tsx:109` / `:114` | status rows, default view | rendered |
| S3 | `StatusConflictsSection.tsx:124` | **`?op=merge`** | rendered — this is the route that reaches the **`C` conflicted** letter |
| S4 | `DiffFileTree.tsx:181` | commit selected | rendered |
| S5 | `DiffBrowser.tsx:404` | diff browser | rendered — **`M` measured 8.39 / 6.65** (`--warning-strong` on `--bg-1`), i.e. the declaration itself, not just the site |
| S6 | `prPanel/PrFileRow.tsx:37` | **`?forge=auth`** | rendered — **4 statuses** exercised; its backdrops are `--bg-1` / `--bg-2` / `--selection` per the matrix above |
| S7 | `DiffOverlay.tsx:295` | diff overlay header | rendered — computed `rgb(232,234,237)` = `--text-1` on `--bg-0` (pre-D1) |
| S8 | `ComposerGroupCard.tsx:129` | composer | rendered — **6 statuses**, backdrop **`--bg-2`** |

**The `C` letter is corroborated twice over**, which matters because it is the worst figure in the
family: `.file-status-conflicted .file-badge` → `--danger-strong` measures **7.62 / 6.01** at rest,
**6.72 / 5.55** hovered and **5.27 / 5.18** selected — and that **5.18** (light, `--selection`) *is*
the conflicted letter on a selected row, the worst live badge anywhere in the app. Separately, AC11
measures the `Conflicts (3)` section label at **7.62 / 6.01** on `--bg-1`.

**Still open on this family — it is NOT fully closed:**
- **AC14 (USER CHECKPOINT, pending).** Native-window read: `pnpm tauri dev` on a real repo with staged
  and unstaged changes — the badge column legible at arm's length in both themes and both densities,
  and the brighter dark letters must not read as "selected"/"highlighted" rows. 11px mono at these
  luminances is not judgeable from the headless harness.
- **AC15 (USER CHECKPOINT, pending).** Hue identity: `--danger-strong` still reads as
  *red-for-deleted* and `--warning-strong` as *yellow-for-modified* rather than drifting to
  pink/olive, and the five badge colours stay mutually distinguishable — including under deuteranopia,
  where `--success-strong` vs `--warning-strong` is the risky pair (the letters carry the meaning
  regardless, so this is a quality judgement, not a compliance one).
- **AC9's real-repo half (USER CHECKPOINT, pending).** A genuine `typechange` (symlink → regular file)
  cannot be produced in the mock harness; the fixture proves the *rule*, the native app proves the
  *pipeline*.
- **P109 (open).** The badge has **no accessible name**, and `added` and `untracked` render the same
  letter — indistinguishable to a screen reader and ambiguous visually. P106 made the letter
  *legible*; P109 is about the letter being *insufficient*.
  > **RECORD CORRECTION 2026-09-03 (P109 §2), and it makes the defect worse than recorded above.**
  > P106 wrote that `added` and `untracked` "deliberately share the letter `A` (`StatusFileRow.tsx:15`,
  > P4c)". That is true of **3 of the 6 `BADGES` tables**. The other **3 already render `U`**:
  > `DiffBrowser.tsx:32`, `DiffFileTree.tsx:22`, `prPanel/PrFileRow.tsx:20` (vs `A` in
  > `StatusFileRow.tsx:15`, `DiffOverlay.tsx:25`, `ComposerGroupCard.tsx:15`). **The same untracked
  > file renders `A` in the status panel and `U` in the diff file tree, in the shipped app** —
  > `notes/todo.txt` is `untracked` in both `src/ipc/fixtures/status.ts:42` and
  > `src/ipc/fixtures/diffs.ts:126`, so the collision is observable by switching panels. The `U` in
  > this section's own family name "A/M/D/**U**/R" has had **no referent** under the mapping recorded
  > in the table above. This is a record correction, not a proposal; the forward fix is
  > `docs/contracts/P109-status-badge-semantics-ui.md` (recommendation: `U` everywhere, plus
  > `role="img"` + `aria-label`), which lands under its own AC12.

**No agent may self-declare the three checkpoint items** — the user's checkpoint authority did not
reach this work, and no agent message closes them.

**House glyph vocabulary** (use these, do not invent synonyms): `✓` good/ready/checked ·
`⚠` warning/failed · `⊘` blocked/refused/cancelled · `●` neutral/informational ·
`✕` dismiss/close · `?` unknown/needs-you. A single glyph never means two different things on two
surfaces — that is why toast `error` is `⊘` and not `⚠` (§10.2).

### 7.1 Row hover actions (Changes / Staged rows and folder rows)

`.row-action` is 20×20, `opacity: 0` until the row is hovered or the button takes focus. **Order is
fixed: secondary and destructive controls first, the primary stage/unstage toggle last (rightmost).**

| Slot | Glyph | Where | `aria-label` |
|---|---|---|---|
| history | `🕑` | tracked rows | `Show history of {path}` |
| blame | `👁` | tracked rows | `Blame {path}` |
| destructive | `↺` / `🗑` | see below | `Discard changes to {path}` / `Delete {path}` |
| primary | `+` / `−` | any actionable row | `Stage {path}` / `Unstage {path}` |

The destructive slot holds **exactly one** control, chosen by whether the file exists in git:

- **`↺` "Discard changes"** — tracked rows with unstaged edits. Reverts to the staged/committed
  version. Never shown on new or staged rows.
- **`🗑` "Delete"** — new (untracked) rows only. There is no version to revert to, so the outcome
  is permanent removal from disk; a different glyph keeps that from reading as a revert.

Folder rows (`.tree-dir-actions`, same 2px gap) use the same order — a folder and its children
share the column, so `↺` sits above `↺`/`🗑` and `+` above `+`.

Both destructive controls are confirm-gated and tint `var(--danger)` on hover via
`.row-action-discard`. Confirm chrome follows the set's composition, never the entry point: a
new-files-only set is titled "Delete new file(s)" and confirms with "Delete"; any set containing a
modified file is "Discard all changes" / "Discard all". Permanently deleted paths are always
listed by name in the dialog body (first 10, then "+N more").

### 7.2 Naming an icon-only or repeated-label button (P69l)

When several controls on one surface share the same visible verb — four `Copy` buttons in
Settings → AI access — each needs an `aria-label` that **names the object**, and the label:

- **starts with the visible word** (`Copy server URL`, not `Server URL`), so a speech-input user can
  say what they see and still hit the right control;
- is **never derived from a sibling node**. A name computed from the neighbouring row label breaks
  the moment the row is reordered, the sibling is truncated, or the row moves into a search-result
  block (§12.5).
- leaves the **visible** text alone. Adding an `aria-label` to a button inside a copy-frozen section
  is not a string change.

## 8. Empty / loading / error states

- Empty (no repo): centered column — "Bonsai" 20px/600, tagline "A tidy Git client" (text-3),
  primary button "Open repository" (accent bg, 8px 16px padding).
- Empty panes (repo open, nothing selected): centered text-3 message, 13px (e.g. "Select a commit
  to see details").
- Unborn repo: empty graph pane message "No commits yet"; status panel remains usable.
- Loading: skeleton rows (bg-2 rounded bars, 1.2s pulse) for lists; graph shows nothing until
  layout arrives (no spinners over the canvas). Any operation > 300ms shows an indeterminate 2px
  accent bar under the header (`.header-progress`, `styles.css:1320`).
- **In-flight, non-blocking operations are announced with words, never spinners.** The house pattern
  is a present-participle label on the control that started the op — `Fetching…` / `Pulling…` /
  `Pushing…` (`WorkspaceToolbar.tsx:142-166`), `Checking…` (`GitMissingBanner`,
  `SettingsUpdatesSection`), `Committing & Pushing…` (`CommitBox`) — plus `.header-progress`.
  Where the trigger is a context-menu item with no persistent control, the participle goes on the
  affected **row's status pill** instead and the row carries `aria-busy="true"`
  (P73 §6.1, submodule rows). Lowercase inside a pill, sentence-case on a button; always a trailing
  `…` (U+2026).
- Errors: inline banner at top of the affected pane — `--danger` at 12% alpha bg, `--danger` text,
  6px radius, dismissible. No modal error dialogs. **Process-global** faults (not pane-scoped) use
  the app notice bar instead — §10.
- **Transient operation failures are toasts** (`Toasts.tsx`): `error` tone, sticky, `role="alert"`.
  Copy shape is `Couldn't <verb> <target>. <what to do next>` — the frontend supplies the prefix
  naming the action and the exact target, the backend's `AppError.message` supplies the remedy
  sentence and is surfaced **verbatim** (so `authFailed` / `networkError` copy is not duplicated in
  the UI). Backend messages reaching a toast must therefore be complete, capitalised,
  period-terminated, user-ready sentences — **never raw libgit2 prose and never internal paths like
  `.git/modules/…`**. Repeatable failures pass a dedupe `key` (`<domain>:<target>`) so pressing a
  failing action N times never stacks N identical alerts (§10.1 mechanism). Visual recipe: §10.2.
- Buttons: primary (accent), secondary (bg-2 + border), icon (transparent, bg-2 on hover); all
  32px tall, 6px radius (dock-density controls may go to 28/24px — §3).
- **Dialog body text (P68g).** Primary sentences: `.dialog-body`, 13px `--text-1`. Must-read
  secondary lines — consent facts, spend and destructive consequences, "written without review"
  caveats — use `.dialog-body-detail`, 12px `--text-2`. `.dialog-body-note` is `--text-3` and is
  for genuinely decorative lines only (`+N more`); never put a consequence on it.
- **An empty state inside a pane names the fix.** A bare declarative sentence ("Open a repository
  to view its Git config.") is incomplete: the block is title (13px/600 `--text-1`) + one-line
  reason (12px `--text-2`) + exactly one action button. `EmptyState.tsx` is the *app-level* no-repo
  screen (hero mark, tagline, recents, three CTAs) and must not be embedded in a panel or dialog.

## 9. Bottom AI activity dock (P68e)

Full contract: `docs/contracts/P68e-ai-activity-dock.md`. Canonical geometry:

- Placement: third child of `.workspace-host`, after `.panes`. `flex: none`, `overflow: hidden`,
  full width. Renders `null` when no AI run exists — zero layout cost by default.
- Collapsed = one status bar, `30px` cozy / `28px` compact. Expanded = that bar + a body of
  persisted height, **120–600px** (default `180`), also capped at 60% of window height.
- Resizer: 4px strip on the **top** edge (8px pointer band), `role="separator"
  aria-orientation="horizontal"`, keyboard ±8px, double-click resets to 180.
- Tokens: `--ai-dock-*` declared on `.ai-dock`, swapped by `.ai-dock[data-density='compact']`.
  Every colour token is an alias of an existing theme token (`--ai-dock-bg: var(--bg-1)`,
  `--ai-dock-log-bg: var(--bg-0)`, `--ai-dock-meta: var(--text-2)`,
  `--ai-dock-tool: var(--accent)`, `--ai-dock-attention: var(--warning)`), so one rule set serves
  both themes and **no new `:root` / `[data-theme='light']` token is introduced**.
- Status pills: word + glyph, never colour alone — `✨ Running`, `✨ Stopping…`, `? Needs you`,
  `✓ Ready`, `⚠ Failed`, `⊘ Cancelled`. Label is always `--text-1`; the hue lives in the 40%
  border and the 100% glyph over a 14% tint. **This is the canonical pill recipe — §11.**
- Log surface: `--bg-0`, mono 12px/18px cozy · 11px/16px compact, `white-space: pre-wrap`,
  stick-to-bottom with a 24/4px hysteresis band and a `↓ Jump to latest` escape button.
- **The mid-run question is untrusted model output** (security audit M3). The ask block carries an
  attribution line (`Claude wrote this — Bonsai did not:`) and a fixed, non-model-controlled line
  (`Bonsai never asks for passwords or tokens. Don’t paste secrets here.`), both `--text-1` with
  hue only as an `aria-hidden` `⚠` glyph, and the reply box's `aria-describedby` names the guard
  line **first**. See `P68e-ai-activity-dock.md` §4.1/§4.5 and `P68g-ui.md` §3.
- Streaming output is **not** an `aria-live` region. A separate visually-hidden
  `role="status" aria-live="polite"` element announces status transitions only.
- Motion: the dock never animates its height (a height transition would force repeated
  20k-row canvas relayouts). Only opacity/colour, ≤150ms, ease-out; the app's first
  `prefers-reduced-motion` block lives in this section. P70 adds `.file-chevron` (the app-wide
  120ms disclosure-caret transform) to that same block; P73 adds `.header-progress::after`
  (`animation: none; width: 100%; opacity: 0.6` — the same treatment as `.ai-dock-progress`), which
  closes P68e §12-F3 for the header sweep. P69 adds `.settings-switch-*` and `.settings-segment`.
  `skeleton-pulse` remains outstanding.

## 10. App notice bar (P70)

Full contract: `docs/contracts/P70-ui.md`. The canonical pattern for a **process-global, persistent,
non-dismissable** fault — as distinct from a pane-scoped error banner (§8) or a transient toast.
First and currently only instance: `GitMissingBanner` ("Git is not available").

- **Placement.** Direct child of `.app`, immediately after `</header>`, before the
  `.workspace-host` tab hosts. App-level, not per-tab (tab hosts stay mounted at `display:none`, so
  a per-tab banner would render N times) and visible on the no-repo empty state.
- **In-flow, never overlay.** `flex: none`, full width, no scrim, no focus trap. The rest of the
  app stays fully usable and keyboard-reachable — that is what keeps a non-dismissable surface from
  being a trap. `UpdateNotification` (fixed, bottom-right, `z-index: 90`) is a different layer;
  both may show at once.
- **Zero-cost when healthy.** The component is always mounted but returns only a visually-hidden
  live region until the fault is known, so the healthy path reserves no space and produces **no
  layout shift** when the probe resolves.
- **Geometry.** `padding: 8px 12px`, `gap: 8px 12px`, `flex-wrap: wrap`; text column
  `flex: 1 1 320px; min-width: 240px; max-width: 72ch`; actions `flex: none; margin-left: auto`;
  controls `padding: 4px 10px`, 12px, `min-height: 24px` (matching `.op-banner` / `.op-banner-btn`).
  Density-invariant (§3).
- **Severity.** Degraded-but-usable ⇒ **warning**, not danger: `background: var(--bg-1)`,
  `border-bottom: 1px solid var(--border)`, `box-shadow: inset 3px 0 0 var(--warning)` as the left
  severity rail, plus an `aria-hidden` `⚠` glyph in `--warning`. **All words are `--text-1` /
  `--text-2`** (§2's hue-as-text rule). One rule set serves both themes; **no new token**. The 3px
  left severity rail is the same device toasts use (§10.2) — it is the house severity idiom for a
  wide surface.
- **Content shape.** Title (13px/600 `--text-1`) → one-line explanation (12px `--text-2`) → the
  single best remedy (13px `--text-1` — it is the thing the user came for). Secondary remedies,
  the degraded-capability list, and the paste-into-a-bug-report technical block live behind a
  `Details` disclosure (`aria-expanded` + `aria-controls`, `.file-chevron`), default closed,
  `max-height: 220px; overflow-y: auto`.
- **Recovery action.** Exactly one primary button that re-runs the check. Pending state is
  **text-only** (`Checking…` + `disabled` + `aria-busy`) — no spinner, so it is reduced-motion-safe
  and costs the canvas nothing. A retry that fails again updates an in-bar 11px `--text-2` readout
  (`Still not found — checked HH:MM.`); it never toasts and never re-animates.
- **No dismiss control at all** while the fault holds — not a disabled `✕`.
- **A11y.** `role="region"` + `aria-labelledby` on the title; **not** `role="alert"` (an assertive
  region would re-announce the whole bar on every retry). Announcements go through a separate
  always-mounted visually-hidden `role="status" aria-live="polite"` span — the same split the AI
  dock uses (§9). Focus is **never** moved to the bar. Tab order is pure DOM order, no `tabindex`.
- **Motion.** The bar never animates its appearance, its height, or the disclosure.
- **Copy rules.** Name the real fault in the title; deny the wrong reading explicitly when a wrong
  reading is likely ("This is not a sign-in problem — your saved credentials were never used.");
  lead with the remedy that fixes the reported case; state honestly what still works and what does
  not. Never surface raw backend error prose as the bar's own text — it belongs in the technical
  block.

### 10.1 Toast dedupe key

`Toast.key` (`Toasts.tsx:18`) — an optional stable id. A `pushToast` with an existing key
**replaces** that toast in place (a no-op when the text is identical) rather than stacking. The
rule lives in App's `pushToast`; the presentational component ignores it. Use it for any action a
user will plausibly retry: convention `<domain>:<target>` (e.g. `submodule:vendor/libcore`).

### 10.2 Toast tone recipe (P74 — canonical)

Full contract: `docs/contracts/P74-a11y-toasts-hit-targets.md`. Toasts are the one hue surface
large enough that §11's all-round 40% border cannot carry the tone, so the hue moves to a
**leading edge bar**. This is the recipe for any wide tinted surface that carries prose.

- **Geometry.** `.toast-stack`: `position: fixed; top: 52px; right: 12px; width: 360px; gap: 8px;
  z-index: 90` (above panes, below `.dialog-overlay` at 100), newest on top, `aria-live="polite"`.
  `.toast`: `display: flex; align-items: flex-start; gap: 8px; padding: 8px 12px 8px 10px;
  border-radius: 6px; font-size: 13px; overflow-wrap: anywhere; box-sizing: border-box`. Text
  column **284px**. Density-invariant — a fixed overlay is outside the §3 density scopes.
- **Colour.** One local `--h` per tone (the §11 convention): `--danger` / `--success` /
  `--warning` / `--accent`. `background: color-mix(in srgb, var(--h) 14%, var(--bg-2))`;
  `border: 1px solid color-mix(in srgb, var(--h) 35%, var(--bg-2))`;
  `border-left: 3px solid var(--h)`; `color: var(--text-1)`. **No new theme token, no hex.**
- **Contrast.** Label **9.24–10.30:1** dark / **11.68–12.00:1** light (AA text ✓ in all four tones,
  both themes). Bar + glyph **3.35–4.96** / **3.38–3.69** (1.4.11 graphics ✓). The 1px 35% border is
  **1.69–2.25:1** and is decorative delineation only, exactly as in §11 — never a meaning carrier.
- **The glyph is mandatory, not decoration.** With the label at `--text-1`, the tint and the bar are
  pure colour in a fixed position, so they cannot satisfy WCAG 1.4.1 on their own. `.toast-glyph` is
  `flex: none; width: 14px; font-size: 13px; line-height: 1.45; text-align: center;
  color: var(--h)`, carries `aria-hidden="true"` (the `role="alert"` announcement must stay the
  prose only), and uses the §7 vocabulary: **error `⊘`** (every error toast is a refusal),
  **warning `⚠`**, **success `✓`**, **info `●`**. Error is deliberately not `⚠` — `⚠` already means
  "failed" in the AI dock, and error-vs-warning is precisely the pair that must separate without
  colour. Info is deliberately not `ℹ` — it has an emoji presentation on Windows and macOS and would
  ignore `--h`.
- **Dismiss.** `.toast-dismiss` is **24×24** (§3.1), `margin: -3px -4px -3px 0` so the box is
  optically centred on line 1 and reclaims 4px of text column, `color: inherit` (now `--text-1`,
  **10.30:1** dark), `aria-label="Dismiss"`, hover
  `background: color-mix(in srgb, currentcolor 12%, transparent)`.
- **Behaviour, unchanged and load-bearing.** `error` ⇒ `role="alert"` + `sticky: true`; every other
  tone auto-dismisses at 5 s; the stack caps at 5; §10.1 dedupe applies. Toasts never take focus,
  Esc does not dismiss them, they are not in the command palette, and the component returns `null`
  at zero toasts.
- **Long content.** No `max-height`, no clamp, no ellipsis — a toast wraps to whatever height it
  needs (`overflow-wrap: anywhere`). The worst observed real case (a submodule URL-mismatch refusal
  with a 91-char path and two URLs) is ≈360×244px and stays readable and dismissible.
- **Motion.** None. Toasts appear and disappear without transition, so there is nothing for
  `prefers-reduced-motion` to honour and nothing contending with the canvas render budget.

## 11. Status pills (rows and chrome)

The canonical recipe, first shipped as the AI dock's status pills (§9) and applied to the sidebar
row badges (`.submodule-badge-*`, shared by submodule and worktree rows — `Sidebar.tsx`).

- **Shape.** 11px, `padding: 1px 6px`, `border-radius: 8px`, `flex: none`, inherited UI font
  (not mono — mono is for hashes and paths). Never compresses; the row's *name* is what ellipsizes.
- **Label.** A **word**, lowercase inside a row pill (`up to date`, `out of sync`, `modified`,
  `not checked out`, `repo`, `in use`) and sentence-case in chrome (`Running`, `Failed`). Colour is
  never the sole carrier (§7).
- **Hueless / informational pills** — no verdict, just a fact or an in-flight state: label
  `--text-2` over its own 12% tint (**5.79:1** dark / **6.22:1** light, §2). No glyph, no hue — but
  keep `border: 1px solid transparent` so hueless and verdict pills are the same **19.94 px** height
  in a shared list.
- **Verdict pills** — good/warning/bad: label `--text-1`, hue in the 40% border and in a 100%
  `aria-hidden` glyph over a 14% tint (`✓`, `⚠`, `⊘`). The glyph is the accessible hue carrier
  (measured 2026-08-19: `✓` **4.61:1** dark / **3.94:1** light, `⚠` **5.64:1** / **3.80:1** — both
  clear the 3:1 graphics bar). The 40% border is decorative delineation only and measures
  **1.7–2.3:1** against the row background; never rely on it to carry meaning. Set the hue through
  the local `--h` custom property, the AI-dock convention. **Do not use the hue as the label colour**
  over its own tint — that recipe misses AA (§2).
- **Solid-hue pills take their hue's `-text` ink, never a literal (added 2026-09-02, P102).** A pill
  that must stay loud rather than tinted — the PR state pills (`.pr-state-open` / `-merged` /
  `-closed`, `forge-pr.css:169-201`) and the detached-HEAD pill (`.pill-detached`,
  `controls.css:69`) — keeps the fill and flips the ink (P100 recipe 2): `--danger-text` /
  `--success-text` / `--merged-text`, all `#16181d` dark / `#ffffff` light. **One `color`
  declaration may never serve several different fills** — set `background` and `color` together on
  each state rule. That pattern is exactly what hid `.pr-state-open`'s 2.85:1 white-on-green for a
  whole release. Shipped and harness-verified 2026-09-02 (`0e5dcab`), both themes: OPEN **6.24 /
  5.08**, MERGED **5.30 / 5.05**, CLOSED **4.80 / 4.93**, detached HEAD **4.80 / 4.93**.
  A solid-hue pill's label is a closed set (OPEN / MERGED / CLOSED / HEAD) and `flex: none`, so it
  never shrinks beside a long neighbour — the PR row *title* is what ellipsizes.
- **This recipe is size-bounded (added 2026-08-20, P74).** The 40% perimeter border reads as tone
  only at pill scale (≈20px tall, ≈60–110px wide). On a wide surface — a toast, a notice bar, a
  banner — the same hairline measures 1.7–2.3:1 across a 360px edge and disappears; there, move the
  hue into a **3px leading edge bar** and keep the `--text-1` label + 100% `aria-hidden` glyph
  unchanged (§10.2 toasts, §10 notice bar). The label rule and the glyph rule never change with
  size; only the hue's *shape* does.
- **Title attribute.** A pill's `title` explains *why* the state holds and what fixes it; it must
  never merely repeat the visible label. Keep the *why* out of the visible row text.
- **Busy pill.** While an op runs on that row, the pill's label becomes the present participle
  (`checking out…`, `updating…`) in the hueless style and the row gets `aria-busy="true"` (§8). The
  pill drops its `title` entirely while busy — the participle is the whole message.
- **Density-invariant** — pills live in the sidebar and chrome, which have one geometry (§3).
- **A pill that carries meaning must also be in the accessible name.** When a pill qualifies an
  interactive row (e.g. the Settings rail's `repo` scope pill, §12.1), the pill itself is
  `aria-hidden` and its text is folded into the control's accessible name. Prefer a visually-hidden
  span; use an explicit **`aria-label`** when punctuation matters, because name computation joins
  sibling nodes with a space and would produce `Git config , repository` (the shipped rail does
  exactly this). A purely visual pill leaves AT users without the qualifier.

## 12. Settings surface (P69)

Full contract: `docs/contracts/P69-settings-ui.md`. The canonical pattern for a **categorised
preference surface**, and the home of the app's switch / segmented / settings-row specs. Any future
milestone adding a setting adds a row here, not a new section on a scrolling column.

### 12.1 Two-pane shell

```
┌──────────────────────────────────────────────────────────────┐
│ Settings                                                 ✕   │ 48px, spans both cols
├──────────────────┬───────────────────────────────────────────┤
│ rail 200px       │ 🔍 Search settings                        │ 56px
│  role=tablist    ├───────────────────────────────────────────┤
│  (spans rows 2-3)│ pane  role=tabpanel  --bg-0               │
│  ▌selected       │  title / subtitle / groups of rows        │
│  Git config[repo]│                                           │
└──────────────────┴───────────────────────────────────────────┘
                          880px
```

- Card: `.dialog-card.settings-card`, `width: 880px; max-width: calc(100vw - 48px);
  height: min(660px, calc(100vh - 64px)); display: grid;
  grid-template-columns: 200px 1fr; grid-template-rows: 48px 56px 1fr; overflow: hidden;`
  `--bg-1`, 1px `--border`, radius 6, over the existing `.dialog-overlay` scrim
  (backdrop-mousedown closes).
- **One grid, three rows** — header / search bar / pane — with **every cell placed explicitly** and
  the rail at `grid-row: 2 / span 2`. That is what lets the search bar **precede the rail in DOM
  order** (giving the ✕ → search → rail → pane tab order for free) while sitting beside it on
  screen. Do not nest a second grid for the content column.
- **The card never scrolls as a whole.** The rail and the pane scroll independently; the header and
  the search bar are fixed.
- Header 48px, `padding: 0 8px 0 16px`, title 15px/600 `--text-1`, bottom 1px `--border`.
- Rail `--bg-1`, `padding: 8px`, right 1px `--border`; items 32px tall, `padding: 0 8px`, radius 6,
  13px `--text-2`, 2px gap. Selected: `--selection` bg + `--text-1` + 600 +
  `box-shadow: inset 2px 0 0 var(--accent)` (the accent bar is the shape carrier — `--selection` vs
  `--bg-1` is only ~1.3:1).  Hover `--bg-2`; pressed `--bg-3`.
- Rail grouping is done with `aria-hidden` 1px `--border` dividers and a hueless scope pill
  (§11) — **never** with heading elements inside the `role="tablist"`.
- Search bar `--bg-0`, `padding: 12px 16px`, bottom 1px `--border`; the shared `ListFilterInput`
  overridden to a full-width 32px field (compound selectors, so the override wins on specificity
  whatever the import order).
- Pane `--bg-0`, `padding: 16px 24px 24px`, own scrollbar; `scrollTop` resets to 0 on category
  change. Pane header: title 15px/600 `--text-1` + subtitle 12px `--text-2`, block margin-bottom 16.
- Group: `margin-bottom: 24px`; group title 11px uppercase, `letter-spacing: .08em`, `--text-3`
  (decorative — it duplicates visible structure).
- **Density-invariant** (§3): one geometry in both `cozy` and `compact`.
- Below 720px wide the rail collapses to a 40px horizontally-scrolling strip above the search bar
  (`grid-template-rows: 48px 40px 56px 1fr`); roles and tab order are unchanged. Below 560px tall the
  card takes `calc(100vh - 32px)`.
- CSS lives in `src/styles/settings-shell.css` (shell) and `src/styles/settings-primitives.css`
  (row, switch, segmented, text, reset, hint) — not in `styles.css`. Cascade order is fixed by the
  import list; do not reorder.

### 12.2 Settings row anatomy

```
grid-template-columns: 1fr auto 24px;  column-gap: 12px;  align-items: center;
padding: 10px 0;  min-height: 44px;   sibling rows separated by 1px --border
row 1: [ label 13px --text-1        ] [ control ] [ ↺ ]
row 2: [ help 12px --text-2, 56ch   ]  control spans both rows
```

- Help text is **`--text-2`, never `--text-3`** (§2), carries `id="{rowId}-help"`, and is wired to
  the control with `aria-describedby`. A setting whose effect is not obvious from its label gets
  help text; a section-level paragraph explaining three different rows is a smell — split it.
- **Exactly one help line per row.** `.settings-row-help` is the static sentence, owned by the
  catalog. A row whose explanation must track the live value uses `.settings-row-note` instead —
  identical typography (12px `--text-2`, `margin-top: 2px`, `max-width: 56ch`), same help cell,
  emitted by the call site because the catalog is static data — and then carries **no** catalog
  `help` at all. **Never both.** A static line sitting above a stateful line that restates it is the
  section-paragraph smell moved down one level, and it inflates the row by ~45px.
- **Everything visible in the help cell is in the control's `aria-describedby`.** Where a row
  legitimately shows two paragraphs (a note plus a conditional caveat), the control names both ids,
  space-separated. A visible sentence the screen reader never hears is the same defect as a hidden
  one, and it is the failure mode of a note that lives on a *neighbouring* row.
- The help cell (`.settings-row-help-slot`) is also the slot for the §12.3.4 draft hint. Slider rows
  reserve `min-height: 18px` on it so nothing below moves when the hint replaces the help text.
- `.settings-group-lead` — 12px `--text-2`, `margin: 0 0 8px`, 56ch — is the **group-level**
  paragraph: a frozen section description, or the §12.3.3 gate note. It sits directly under the
  group title and outside every row. `.settings-group-note` is the same type at the **foot** of a
  group, for a caveat qualifying several rows at once ("PR and CI badges need a connected forge…").
  Neither is a substitute for per-row help; use them only when the sentence genuinely has no single
  owning row.
- Reset `↺`: 24×24 `.btn-icon`, `aria-label="Reset {label} to default"`,
  `title="Reset to default ({value})"`. **Conditionally rendered** when value ≠ default; the 24px
  grid column is always reserved so the row never shifts. Reset lives **per row only** — no
  per-category and no global reset.
- `.settings-row--stacked`: label / help / control each on its own grid row, control at
  `width: 100%`. Use for text fields, paths, and read-only value + Copy pairs.
- Two rows in one dialog must never share an accessible name (`Fetch every` / `Refresh every`, not
  `Interval` / `Interval`; `Time limit` / `Spend limit`, not `Limit` / `Limit`). A `NumberSlider`
  produces **two** named controls — the range twin is `aria-label`led from the same string — so one
  duplicated label is four ambiguous nodes. Proximity to a distinguishing switch above does not
  count: the accessible name must stand alone.
- **The row is the search unit.** Every row is stamped with its catalog id and removes itself when a
  query does not hit it (§12.5). A control stamped **outside** `SettingsRow` must self-filter with
  the same hook, or a single hit in its category renders the whole block as a "result".

### 12.3 Control kinds

| The setting is… | Control |
|---|---|
| An independent on/off | **Switch** (§12.3.1) |
| One of 2–3 exclusive values, short self-explanatory labels | **Segmented** (§12.3.2) |
| One of 2+ exclusive values where each needs a sentence | Radio group, stacked, hint under each |
| A bounded number tuned by feel | `NumberSlider` (slider + number + unit) — §12.3.4 |
| Free text, a path, an unbounded value | Text field, stacked row |
| A one-shot action | Button |
| More than 3 exclusive values | `Combobox` |

**A button labelled with its own current value is not a control** — `[ Dark ]` reads as "make it
dark". Never use one for state.

#### 12.3.1 Switch

CSS over a **native `<input type="checkbox">`** — the implicit `checkbox` role, native Space
toggling, and every `getByRole('checkbox', {name})` query survive. `role="switch"` is deliberately
**not** used.

- Wrapper `<label>`: `position: relative; display: inline-flex; align-items: center;
  min-height: 24px; width: 36px; flex: none;`
- The input is `position: absolute; inset: 0; width: 100%; height: 100%; opacity: 0; margin: 0;` —
  it **is** the 36×24 hit target.
- Track 36×20, radius 999: off `background: var(--text-2)`; on `background: var(--accent)`.
- Knob 14×14, radius 999, `background: var(--bg-0)`; `translateX(3px)` off → `translateX(19px)` on.
- **The knob's position is the non-colour meaning carrier.** Never rely on the track hue.
- Motion: `transform`/`background-color` 120ms ease-out, in the §9 reduced-motion block.
  Hover: track `filter: brightness(1.08)`. Active: knob `width: 17px`.
- Focus: `.settings-switch:has(> input:focus-visible) .settings-switch-track` → 2px `--accent`,
  offset 2px. Disabled: wrapper `opacity: .55; cursor: not-allowed`.
- Contrast, all four boundary pairs on `--bg-0`: **5.6–7.9:1** dark, **4.7–4.9:1** light (§2).

#### 12.3.2 Segmented

CSS over **native `<input type="radio">`** inside a `role="radiogroup"` labelled by the row label —
arrow-key navigation and `getByRole('radio', {name})` come free. **Never** `role="tablist"`.

- Container `display: inline-flex; background: var(--bg-2); border: 1px solid var(--border);
  border-radius: 6px; padding: 2px; gap: 2px;`
- Segment `min-height: 24px; padding: 0 10px; border-radius: 4px; font-size: 12px;
  color: var(--text-2); border: 1px solid transparent;` hover `background: var(--bg-3)`.
- Selected `background: var(--selection); color: var(--text-1); font-weight: 600;
  border-color: var(--accent);` — the 600 weight and the accent border are the non-colour carriers
  (`--accent` on `--bg-2` = **4.4:1** dark / **4.1:1** light).
- Focus: `.settings-segment:has(> input:focus-visible)` → 2px `--accent`, offset 1px.
- Max 3 segments.
- A radio group that stays a **radio group** (each option needs a sentence) is a
  `role="radiogroup"` **div** named by the row label via `aria-labelledby` — not a
  `<fieldset>`/`<legend>`, which reports `group` and hides the row label from the control's
  accessible name. Each option's sentence is `--text-2` at the **same 12px as row help**; an 11px
  hint next to 12px help in the same dialog is an inconsistency, not a hierarchy.

#### 12.3.3 Disabling a whole group

- One `<fieldset disabled>` around the dependent rows. It is the only mechanism that removes every
  descendant from the tab order in one place, and it maps exactly to the real dependency.
- The reason **leads** the group as a `.settings-group-lead` carrying an id, and the `<fieldset>`
  carries `aria-describedby` pointing at it (only while the note exists — a dangling idref is worse
  than none), so the reason is announced on entry rather than discovered afterwards.
- The `.55` dim lives on `.settings-row.is-disabled`, **never on the `<fieldset>`**: `opacity` is a
  group property, so dimming the fieldset would dim the very sentence that explains the dim.
- **`opacity` never nests** (§2, dimming budget). A dimmed row's descendants must not dim
  themselves — `.55 × .55 = .30`. Any control with its own `disabled` opacity rule is reset to `1`
  inside `.settings-row.is-disabled`, and the override must win on **specificity**, not source
  order: `:has()` takes the specificity of its most specific argument, so the obvious override ties
  and the *later* rule silently wins. Qualify the override so it cannot tie, and keep it after the
  rule it defeats.
- Hue never carries disablement, and the switch knob's **position** still reads on-vs-off through
  the dim.

#### 12.3.4 NumberSlider — draft divergence feedback

Full spec: `P69-settings-ui.md` §13. **Specced, not yet implemented** — the CSS hook, the reserved
help slot and `SettingsRow`'s `hint` prop shipped in P69; the classifier, the timer and the
`aria-invalid` wiring have not.

The number input shows a **draft** while focused and commits the clamped value on every keystroke
(P69c), so the field can legitimately show text the setting does not hold. Whenever it does, say so.

- **Divergent** = editing, and the draft is blank/non-numeric, or `Number(draft) !== value`. Compare
  numerically — `06` and `6e1` are not divergences and must never warn.
- **Treatment:** `--warning` 1px border + `inset 0 0 0 1px var(--warning)` on the input (no box-model
  change, focus **outline** untouched), plus one 12px `--text-2` sentence in the row's help slot,
  which hides the help text while it shows. Ring and sentence are **one state** — colour never
  travels alone. Never `--danger`: the setting is already at a legal value, nothing is lost. Ring
  contrast on `--bg-2`: **6.4:1** dark / **4.2:1** light (§2).
- **Timing (the rule):** an **above-maximum** draft warns on the keystroke that causes it — appending
  digits only increases, so it is terminal and no further typing can fix it. **Below-minimum, blank,
  and non-integer** drafts warn only after **600 ms with no keystroke**, because every valid entry
  passes through them (`30` passes through `3`; re-typing passes through empty). Once showing it
  stays showing and switches text without re-arming; it hides instantly when the draft becomes
  valid, and always dies with the draft on blur/Enter.
- **Copy:** `Too high — will be set to {max} {unit}.` / `Too low — will be set to {min} {unit}.` /
  `Whole numbers only — will be set to {value} {unit}.` / `No value — stays at {value} {unit}.`
  Subject-free (naming the label yields `Fetch every will be 300 seconds.`), ≤40 chars, says the
  fault and the outcome in that order.
- **A11y:** `aria-invalid="true"` exactly while showing (removed otherwise); the hint `<p>` is
  permanently present with `aria-live="polite"` and empty text when idle; `aria-describedby`
  **composes** `{help-id} {id}-draft` and never replaces an existing description.
- **Layout:** the help slot reserves `min-height: 18px` on slider rows so nothing moves while typing.
  Every slider row should therefore carry real help text, or that line sits empty.
- Applies to all eight sliders and to the hand-rolled USD field (with `integer: false`, so the
  whole-numbers case is off).

### 12.4 Keyboard, roles, focus

- `Ctrl/Cmd + ,` opens Settings, registered **above** App's `typing` guard so it fires from the
  commit box. No-op when already open — a shortcut must not toggle a modal.
- Card: `role="dialog" aria-modal="true" aria-labelledby="settings-title"`. Rail:
  `role="tablist" aria-orientation="vertical"`, items `role="tab" aria-selected aria-controls`.
  Pane: `role="tabpanel" tabindex="-1" aria-labelledby="{selected tab}"`.
- Tab order is pure DOM order (close ✕ → search → rail → pane). The rail is **one** tab stop with a
  roving tabindex that follows **focus**, not selection (the `AiRunStrip.tsx:54-75` precedent).
- **Manual activation** (P69 D-5, corrected here 2026-08-20): `↑`/`↓` **move focus only**;
  `Enter`/`Space` selects. `Home`/`End` move focus to first/last; `→` moves focus into the pane.
  Automatic activation is wrong for this rail because it would fire a `getConfig` IPC round-trip
  every time focus passed *over* `Git config` — arrow-browsing a rail must not perform I/O. For the
  same reason, selecting a category does **not** move focus into the pane.
- **There is no focus trap** (P69 D-4). This codebase has no shared trap utility and no dialog has
  one, so adding one to Settings alone would be the inconsistency; a half-implemented trap is worse
  than none. Anyone introducing one introduces it for every dialog in the same pass.
- Focus **restore** does ship: the element active when the dialog mounted, falling back to the ⚙
  trigger and then `<body>`.
- Initial focus: the search input — a text field, so no accidental activation. A **deep-linked**
  open (`initialCategory` given, or `configInitialFocus` set) deliberately sets **no** initial focus
  of its own and leaves placement to the target section's own effect (which focuses `user.name`); a
  search box grabbing focus there would defeat the commit-error linkage.
- `Esc` is layered: it first clears a non-empty search (the `ListFilterInput` capture-phase idiom),
  then closes the dialog. `Enter` never closes — settings apply live and there is no "OK".
- Every hit target ≥24px: rail 32, switch 36×24, segment ≥24, reset 24, close 32, `Go to {Category}`
  24 (§3.1).

### 12.5 Settings search

- One model: the query produces a **cross-category result list** of live, editable rows grouped by
  category — not a rail filter and not a within-page filter. The failure mode being solved is "I
  know the setting's name, not its category".
- **No second renderer.** Each hit category's own page is mounted inside a search context and every
  row that did not match removes itself, so a result can never drift from the pane. Consequences:
  a control stamped outside `SettingsRow` must self-filter (§12.2), a group hides itself when no
  child survived, and a `<details>` disclosure containing hits is **forced open while a query is
  running** — a hit inside a collapsed disclosure is an invisible result.
- Matching: synchronous, case-insensitive substring, all whitespace-separated terms required, over
  `label` + `help` + a never-displayed `keywords` string (the `PaletteAction.keywords` idiom). Not
  fuzzy. A row that carries a stateful `.settings-row-note` instead of catalog `help` (§12.2) must
  compensate in `keywords` — that is the only place its vocabulary can live.
- **Only renderable rows can match.** The matcher filters through the availability gate first: a row
  whose precondition fails is not in the DOM, so matching it would report a count nobody can see and
  hand the result list a category whose block renders empty. One list feeds the status line, the rail
  counts and the results, so the three cannot disagree.
- The searchable index is **pure data in its own modules** (`settings/settingsCatalog.ts` +
  `settings/catalog/*.ts`), never inlined in a component — the same rule as any large static table.
  It is the same catalog that supplies every row's label, help and reset descriptor.
- Matched substrings in the label are wrapped in `<mark>`: `background: var(--selection);
  color: var(--text-1);` (**9.4:1** dark / **13.3:1** light). Overlapping ranges from different terms
  are merged before wrapping, never nested. Help text is not highlighted today; the specced fallback
  (`P69-settings-ui.md` §3.2.1, **not implemented**) highlights the **help** line only for rows whose
  label produced no ranges, so a `keywords`-driven query stops pointing at nothing.
- Result group header: category name, 11px uppercase 600, **`--text-2`** — not `--text-3`. In a
  result list this heading is the user's only wayfinder to the row's category, so it is text the user
  must read (§2). Trailing `Go to {Category}` text button, 12px `--text-2` → `--text-1` on hover,
  24px min hit box (§3.1).
- **Rail counts: emphasise the hits, never dim the misses.** Each item shows a right-aligned
  `aria-hidden` count (11px, tabular numerals). An item with matches lifts label **and** count to
  `--text-1`; a zero-count item keeps the resting `--text-2` and shows `0`. Every item stays
  clickable — clicking any of them clears the query — so the earlier `opacity: .5` on zero-count
  items was inadmissible (§2 dimming budget: 2.87:1 dark / 2.36:1 light on a control the user is
  meant to click). The count is **`--text-1`, not `--accent`**: accent-as-text fails AA on the
  `--selection` fill of a selected rail item in both themes (§2). The non-colour carrier is the
  **digit** (`0` vs `N`); the colour lift is secondary.
- Counts are per catalog **entry**, not per rendered instance — a `repeats: 'perProfile'` row counts
  once while rendering three rows. State it wherever it is consumed; do not change it in one place.
- Result count changes are announced by a visually-hidden `role="status" aria-live="polite"`:
  `{n} settings match` / `1 setting matches` / `No settings match`. One sentence shape for all three
  counts.
- **While a query is active, a rail tab's accessible name gains a count suffix** —
  `{label}[, repository][, {n} match|matches]` — because the visible count is `aria-hidden` and this
  is the only way an AT user gets the per-category signal the colour lift gives a sighted one. It is
  the one sanctioned exception to the frozen-name rule, and it means
  `getByRole('tab', { name: 'Commit graph' })` **does not match mid-search**: query with a prefix
  regex in any test that spans both states.

### 12.6 Identity in the header

- The identity control is the far-right item of `.header-toolbar` (§1) and reads the **effective**
  Git identity — `local` if set, otherwise `global` — and **names its source** in the menu. A
  local-only read is wrong: Git resolves local-over-global, so a repo with no local identity still
  commits fine, and a control that showed nothing there would be lying.
- Trigger: 32×32 button containing a 22px circle of **initials** (first letters of the first two
  name words, max 2). No identity → glyph `?` with a 1px `--warning` ring (**7.3:1** / **4.5:1**);
  the glyph and the accessible name, not the hue, carry the state. Loading → `·` + `aria-busy`.
  `aria-haspopup="menu"` + `aria-expanded`.
- Menu: a `ContextMenu` anchored with the house idiom (`rect.right`, `rect.bottom + 2`), a
  non-interactive `header` block stating name / email / source, then one row per saved identity with
  `checked` (⇒ `role="menuitemradio"`) and a `detail` second line, then `Manage identities…`.
- The menu owns its open state and lifts it via `onMenuOpenChange` (the `TabStrip.tsx:35-37`
  precedent) because App early-returns global shortcuts while a menu is open.
- **Writing an identity into a repo confirms only when it would overwrite a differing *local*
  value** — writing into an empty slot destroys nothing. The confirm names both identities and the
  consequence, uses `confirmVariant='primary'` (recoverable), and says
  `Commits you have already made are not changed.`
- P69 added **four** additive fields: `ContextMenuItem` gained `checked`, `detail` and `busy`, and
  `ContextMenuProps` gained `header` and `busy` (`ContextMenu.tsx:50` / `:72`). All are additive:
  absent ⇒ byte-identical rendering to before. The **check column belongs to the list, not the
  row** — it is reserved whenever any item declares `checked`, so plain rows in the same menu stay
  aligned with the labelled ones.

### 12.7 Forge accounts (P79/P80)

- **Provider display without color as sole carrier:** `ForgeProviderBadge` (2-letter monogram, GH /
  GL / BB / AZ / ??) + `ForgeAvatar` (image or login-initial monogram, 22px cozy / 20px compact).
  Both reuse `.identity-avatar` / `.pr-draft-tag` geometry; no hue carries meaning.
- **PR-panel account switcher (P80)** reuses the §12.6 identity-menu idiom exactly: a `ContextMenu`
  anchored `rect.right` / `rect.bottom + 2`, a non-interactive `header` block (`Accounts on {host}`,
  plus the no-default nudge line when applicable), one `checked` (`role="menuitemradio"`) row per
  account with a `detail` second line, then `Use host default` + `Add another account…`. The
  left group (avatar + login + host) is the trigger, shown as a button **only when the host has ≥2
  accounts** (no switcher chrome for a single account). Writes are optimistic + `busy`.
- **`AccountSource` label vocabulary** (canonical microcopy — do not reword per surface;
  `src/components/forgeAccountSource.ts`):

  | `accountSource` | header caption | tooltip |
  |---|---|---|
  | `override`    | `Pinned to this repo` | `Pinned to this repository. Other repositories on this host use the default.` |
  | `ownerMatch`  | `Matched by owner`    | `Chosen because its username matches this repository's owner.` |
  | `hostDefault` | `Host default`        | `The default account for this host.` |
  | `single`      | *(none)*              | *(none)* |
  | `none`        | *(none)*              | *(n/a — connect view shows)* |

- **Per-repo vs global semantics (P80):** the PR panel is **per-repo** — it pins/unpins the repo's
  account override (`Reset to host default` is nondestructive, no confirm) and never signs out.
  **Full sign-out (keychain token deletion via `forgeRemoveAccount`) lives only in Settings →
  Accounts**, always behind a danger `ConfirmDialog` that names the account and states pinned repos
  fall back. Never label a per-repo unpin "Disconnect" (it reads as sign-out).
- Connected-state chip: `● Connected` (`--success` dot, `--text-2` word) / `○ Token missing`
  (`--text-3` dot) — the word carries meaning, never the dot color alone.
- No new tokens were introduced for any forge-account surface; all classes are built from existing
  `--bg/-text/-border/-accent/-success/-warning/-danger` tokens.

### 12.8 Identity profile colors (P82)

- **Purpose:** an at-a-glance answer to "which identity is this repo on?", robust to duplicate
  labels. A curated 9-value palette (`ProfileColor`: `neutral` + 8 hues), never free-form hex.
  Deliberately separate from the semantic (`--success/--warning/--danger/--accent`) and graph
  (`--lane-*`) sets — an identity swatch must not read as a status or a branch lane.
- **Tokens (new, both themes).** Swatch/ring fills only — never used as text, never the sole carrier
  of meaning (always paired with the profile label or the avatar initials + accessible name):

  | `ProfileColor` | token | dark | light |
  |---|---|---|---|
  | `neutral` | `--profile-neutral` | `#6b7280` | `#8a919e` |
  | `slate`   | `--profile-slate`   | `#7d8aa3` | `#5d6b85` |
  | `blue`    | `--profile-blue`    | `#4f8cff` | `#2f6fe4` |
  | `teal`    | `--profile-teal`    | `#3ec6c0` | `#0f8f89` |
  | `green`   | `--profile-green`   | `#57ab5a` | `#1a7f37` |
  | `amber`   | `--profile-amber`   | `#e8c341` | `#9a6700` |
  | `orange`  | `--profile-orange`  | `#f2994a` | `#c2410c` |
  | `purple`  | `--profile-purple`  | `#9b6dff` | `#7c3aed` |
  | `pink`    | `--profile-pink`    | `#f26d9c` | `#c2266f` |

  **Contrast:** all nine ≥3:1 (non-text/graphics) against their panel background in both themes —
  dark vs `--bg-1 #1d2026` (min 3.4:1, neutral), light vs `#ffffff`/`--bg-1` (min 3.1:1, neutral).
  Each swatch also carries a 1px `--border` outline so its edge survives when a hue is close to the
  row background. No `-text` variants exist: profile text is always `--text-1`/`--text-2`.
- **Swatch primitive** `IdentityColorSwatch` (`src/components/IdentityColorSwatch.tsx`): a 10px
  (`size="sm"` 8px) circle, fill chosen by `.identity-swatch[data-profile-color='<c>']` CSS
  attribute selector — **no inline color, no hex in TSX**. `aria-hidden` everywhere except the
  picker (adjacent text is the accessible name).
- **Appears in:** the header avatar (2px hue ring when a non-neutral profile is matched; unset
  `?`+`--warning` ring keeps priority), identity-menu rows (reuses the existing `ContextMenuItem.icon`
  slot — no `ContextMenu` change), the menu header block (when the effective identity matches a
  profile), and the Settings profile card head (beside the title; the `in use` badge stays the
  textual "active" carrier).
- **Picker** `IdentityColorPicker` (`src/components/settings/IdentityColorPicker.tsx`): a
  `role="radiogroup"` of native `<input type="radio">` (the `SettingsSegmented` idiom, but a swatch
  grid — segmented is text-only and caps at 3). Nine ≥24px swatch cells; selected = 2px `--accent`
  ring + full-size dot; each radio's accessible name is the color name (`Neutral`…`Pink`). Duplicate
  hues across profiles are **allowed** (labels disambiguate). New catalog control type `'color'` on
  `SettingsIndexEntry`; catalog row `identities.profile-color` (`requires:'profile'`,
  `repeats:'perProfile'`).
- **Auto-distinct (UI layer, no persistence rewrite):** create-flow and the header save-as draft use
  `nextFreeHue(profiles)` (first unused hue in table order, wrap to least-used). Pre-P82 profiles
  (`color` absent) render a distinct **display-fallback** hue by array index (`ASSIGNABLE_COLORS[i%8]`);
  an explicit `neutral` is honoured as grey. The concrete color is written through the whole-array
  patch the moment the user touches the picker.
- **Motion:** only the ≤120ms selected-swatch grow/ring on the picker; collapses under
  `prefers-reduced-motion`.

### 12.9 PR actions — merge & close/decline (P83)

- **Footer action bar** `.pr-actions-bar`: the canonical pattern for a panel's terminal, commit-point
  actions. Pinned under the scrollable content region (e.g. `PrDetailView`), not in the header
  (header = navigation + metadata; actions read as a distinct commit-point). Full panel width, top
  `1px solid var(--border)`, `display:flex; justify-content:space-between; gap:8px`; padding
  `12px 16px` cozy / `8px 12px` compact. Buttons at the standard height (32px cozy / 28px compact;
  hit target ≥24px met on both densities). Rendered only while the item is actionable (PRs:
  `summary.state === 'open'`; a merged/closed PR shows its state pill and no bar).
- **Hierarchy:** exactly one primary + one quieter danger-secondary — no third button. Affirmative
  action on the right (`btn-primary`, label ends in `…` when a dialog follows, e.g. `Merge…`);
  the destructive/abandoning action on the far left (`.btn-secondary-danger`). This mirrors dialog
  button order (destructive-left, affirmative-right) so muscle memory transfers. The left label is
  per-context/per-forge (`Close` / `Decline` / `Abandon`); meaning is carried by the verb, never hue.
- **`.btn-secondary-danger`** recipe: a `btn-secondary` modifier whose base state is quiet
  (`--text-2` text, `--border`) so it does not compete with the primary, tinting text/border to
  `--danger` on `:hover`/`:focus-visible`. Built from `--danger` on `--bg-1` — the same pair as
  `btn-danger` text, which already clears ≥4.5:1 in both themes. Color is never the sole signal: the
  verb and the follow-up confirm dialog restate the consequence.
- **Form dialog pattern** `.pr-merge-card`: when a confirmation needs form fields (a picker + optional
  text + a checkbox) that the shared `ConfirmDialog` cannot host, build a dedicated dialog on the
  existing `.dialog-card` chrome — and that dialog *is* the confirmation (no second modal). Width
  420px (matches `.ai-consent-card`, the form-bearing-dialog precedent; the 360px default is too
  tight for a picker + fields). Same overlay, Esc, and overlay-click-cancels behaviour as
  `ConfirmDialog`. Structure top→bottom: `dialog-title` → lead summary paragraph (names consequence +
  irreversibility) → a method **radiogroup** (`SettingsSegmented`, `role="radiogroup"`, options
  filtered to `SUPPORTED_MERGE_METHODS[kind]`, with a one-line `.pr-merge-method-desc` in
  **`--text-2`** updated per selection — **amended by P98**: it was `--text-3` at 3.38:1 dark /
  2.96:1 light on the `.dialog-card` `--bg-1` surface, which fails the §2 text bar; it is the only
  sentence telling the user what the selected method will do to their branch, immediately above an
  irreversible action, so it is read text and now measures **7.25:1** dark / **7.45:1** light) →
  optional commit title/message fields (`.pr-input` / `.pr-textarea`, shown
  only for methods that consume them) → a delete-source-branch checkbox (`.pr-draft-toggle` idiom,
  default OFF, **hidden when `kind === 'gitHub'`** since GitHub ignores it on merge) → `.dialog-buttons`
  Cancel + confirm. The merge confirm is `btn-primary` (constructive happy path; irreversibility is
  carried by the copy per house destructive-copy rules), busy label `Merging…` with `disabled` in
  flight.
- **Confirm-dialog reuse for close/decline:** the form-less destructive path reuses `ConfirmDialog`
  verbatim (`confirmVariant='danger'`) with per-forge title/label/body (Close / Decline / Abandon).
  No new dialog for it.
- **A11y (hard rules, consistent with the §12.x dialogs):** initial focus lands on **Cancel** in both
  the merge and close dialogs (a stray Enter never fires an irreversible action); Esc and
  overlay-click cancel; focus returns to the invoking bar button on close (focus restore). The merge
  dialog is `role="dialog" aria-modal="true"` labelled by its `dialog-title`; the method group is
  `role="radiogroup"` with an id-wired label; the checkbox is a real `<input type="checkbox">` +
  `<label>`. `aria-busy="true"` on the panel (`.pr-detail`) while an action is in flight, with the
  confirm button's busy label as the visible cue (no spinner-only state). Focus ring 2px `--accent`,
  1px offset, `:focus-visible` only. No new tokens; the only motion is the dialog's existing ≤150ms
  fade/scale-in, which already honours `prefers-reduced-motion`.

### 12.10 Git activity: in-flight phase readout + session log dock (P87)

Full contract: `docs/contracts/P87-ui.md`. The git analogue of the AI stream (§9). Two surfaces off
**one** event stream: an in-flight **toolbar phase readout** (View C) and a bottom **git activity
dock** (View D, a twin of the §9 AI dock). Reuses §9 geometry, §11 pills, the §9 log surface, and the
§9 live-region rule. **No new theme token, no hex** — every colour aliases a §2 token.

- **Phase copy is UI-derived.** The backend emits structured `category × phase{kind,hook}` only; the
  human strings live in `gitActivityFormat.ts` (`phaseLabel`). Locked table in `P87-ui.md` §1
  (`Running pre-push hook…` → `Sending objects…`/`Fetching…`, `Writing commit…`, generic `Working…`).
  The RunningHook → Network transition is the canonical "it's not hung" fix.
- **View C — two-part in-flight display.** The op button keeps a **stable short participle**
  (`Pushing…`/`Fetching…`/`Committing…`, from `categoryMeta().participle`) so the toolbar never
  reflows mid-op; the granular phase/progress rides an adjacent `.toolbar-phase` span reusing the
  `.toolbar-job-status` treatment (11px `--text-2`). Applies on the workspace toolbar remote buttons
  and the `CommitBox` commit button. The phase string is NOT put in the button label.
- **Progress bar — indeterminate ↔ determinate.** `.header-progress` stays the indeterminate 2px
  `header-progress-sweep` by default (preparing, hooks, push network, refresh). During a **fetch/pull
  `network`** phase *with a derivable fraction* it gains `data-determinate` + `--progress: <0..1>`;
  the `::after` fill scales by `transform: scaleX(var(--progress))` (transform, not width — no layout),
  `transition: transform 150ms ease-out`. Determinacy + an object/byte count readout both require the
  backend to surface `transfer_progress` as structured counts (flagged to the architect); text-only →
  best-effort parse, else stays indeterminate. Never colour-only: the bar is backed by the readout.
- **View D dock geometry** = §9 exactly: child of `.workspace-host` **after `.ai-dock`** (git dock is
  the outermost/bottom; both collapse independently); `flex: none; overflow: hidden`; collapsed bar
  30px cozy / 28px compact; expanded 120–600px (default 180, capped 60% viewport); `PaneDivider`
  top-edge resizer (±8px, dbl-click → 180). **Divergence from §9:** the git dock returns `null` only
  before the first op of the session, then **stays mounted** for the session (it is an always-on
  record and its live region must persist) — and it **never auto-expands** (git ops are frequent;
  contrast the AI dock's auto-expand on `awaitingInput`). Geometry may be session-only (recommended,
  no settings change) or persisted like the AI dock (2 new keys — architect call).
- **Tokens:** component-scoped `--git-dock-*` alias block on `.git-activity-dock`
  (`--git-dock-bg: var(--bg-1)`, `--git-dock-log-bg: var(--bg-0)`, `--git-dock-meta: var(--text-2)`,
  `--git-dock-border: var(--border)`), swapped by `.git-activity-dock[data-density='compact']`.
  Status hue via local `--h`. Same alias discipline as `--ai-dock-*`.
- **Run row anatomy.** Summary line: `.file-chevron` disclosure → category glyph (`aria-hidden`,
  reuse `PushIcon`/`FetchIcon`/`PullIcon`/`MergeIcon`/`RefDotIcon`) → noun `--text-1` → target
  (`→ origin/main`, `--text-2`, ellipsis + `title`) → optional `⋯ trimmed` hueless chip → status pill
  → duration + `HH:MM` timestamp (both `--text-2`, never `--text-3` for read text; timestamp `title` =
  full date-time). Expanded: per-hook sub-rows (`✓ exit 0` / `⚠ exit N` verdict pills — exit code
  inside the label, never colour-only) → the output log (reuse `.ai-log`: mono, `--bg-0`, `pre-wrap`,
  `.ai-log-dropped`/`.ai-log-trunc` chips; **stderr** lines get a 2px `--warning` left-border shape
  cue + `--text-1` + a visually-hidden `stderr:` prefix, not colour) → a Copy button (reuse
  `AiOutputPanel`'s Copy→Copied) → for a blocking-hook failure, a `--text-2` note tying to
  `HookOutputDialog`.
- **Status vocabulary** (§11 recipe, word + glyph): run `● Running` (`--h: --accent`) / `✓ Success` /
  `⚠ Failed`; hook `✓ exit 0` / `⚠ exit N` / `⊘ killed`. `●` (not `✨`, which is AI; not `⚠`, which is
  "failed") matches the toast info glyph (§10.2).
- **Relationship to `HookOutputDialog` (unchanged).** The dialog stays the point-in-time **blocking**
  modal (verbatim output + skip-hooks retry); the dock is the always-on record that *also* keeps
  passing/successful runs whose output was previously discarded. A failed blocking-hook run appears in
  **both**; the dock row is read-only and never re-offers "skip hooks".
- **Clear** — header text button, `aria-label="Clear git activity log"`, ≥24px, disabled (`--text-3`)
  when no terminal runs; clears terminal runs only (never a running one). **No confirm** — it discards
  only ephemeral, session-scoped observability data (already gone on restart); nothing recoverable is
  lost, so a confirm would be friction, not safety. This is the sanctioned no-confirm exception;
  real destructive-git actions keep their confirmation.
- **A11y.** One always-mounted visually-hidden `role="status" aria-live="polite"` announcer for the
  **active run's phase transitions + terminal result only** (never output lines — the §9 rule); it is
  the single announcer for both C and D (the op button is `disabled` mid-op). The log `<ol>` is
  focusable (`tabIndex=0`, `aria-label="Git activity log"`) but **not** a live region. Keyboard:
  Arrow up/down move row focus, Enter/Space toggle disclosure, Esc collapses; focus restores to the
  invoker on collapse. Optional open shortcut `Ctrl/Cmd+Shift+L` (free in the map; pairs with
  `+Shift+A`). Focus ring 2px `--accent`, 1px offset, `:focus-visible`.
- **Entry points.** No new toolbar button: (1) the always-visible collapsed dock bar, (2) a
  `Git activity` command-palette row (enabled once `runs.length > 0`, mirroring the AI dock gate),
  (3) the clickable in-flight `.toolbar-phase` readout → expand + reveal the active run.
- **Motion.** No dock height animation (§9 canvas-relayout prohibition — snap). `.file-chevron` 120ms;
  determinate fill 150ms `scaleX`. Add the git-dock/bar selectors to the §9 `prefers-reduced-motion`
  block (sweep → static 100% @ .6 opacity; chevron/fill snap).

### 12.11 Developer settings, the sensitive row, and the logging indicator (P91)

Full contract: `docs/contracts/P91-observability-ui.md`. **No new tokens.**

**Rail.** `'dev'` / label `Developer`, last, `dividerBefore: true`. Discoverable and searchable like
any other category — a power-user diagnostic surface is still a settings surface, and hiding it would
break both the entry point of the debugging workflow and `settingsCatalog.coverage.test.tsx`.
A future `'statistics'` category sits between `About` and `Developer` and inherits the divider.

**Master-gate pattern (canonical).** A page whose rows configure a capture/recording gate splits into
gated and ungated groups, and the split is by *workflow*, not by topic: rows that configure the gate
are wrapped in one `<fieldset disabled>` (§12.3.3); rows the user needs **after turning the gate off**
(here: the privacy statement and the reveal/export actions) are never gated. Disabled, never hidden —
hiding five rows moves the content below them on the exact click the user is reading it, and needs a
new `SettingsRowRequirement` plus a coverage-test exception.

**Sensitive-row treatment — `.settings-row--sensitive`.** For a benign-looking switch with a real
privacy consequence sitting among ordinary switches:
`padding-left: 12px; box-shadow: inset 3px 0 0 var(--warning);` plus a 12px `--warning` glyph before
the label. Label stays `--text-1`, help stays `--text-2` — **no tinted background and never a
coloured label** (§2). It is the §10.2 leading-bar recipe reused at row scale; `--warning` bar/glyph
on `--bg-0` measures **7.3:1** dark / **4.5:1** light (≥3:1 graphics bar, both themes). The row's
live-value line uses `.settings-row-note` and therefore carries **no** catalog `help` (§12.2), and
while the switch is on a matching leading-bar `.settings-group-note` closes the group.

**Privacy-sensitive confirmation uses `confirmVariant: 'primary'`, not `'danger'`.** `danger` is
reserved for data loss. A reversible setting that only affects files created afterwards carries its
weight in the copy (name the exact consequence and what is *still* excluded) and in the row
treatment above. Default focus is `Cancel`; the switch must not flip optimistically.

**App-level activity indicator (the pattern).** A background process the user opted into and could
forget gets a **conditional header-toolbar pill**, not a §10 notice-bar row — §10 is a *global fault*
channel and a user-chosen state is not a fault. The pill is **absent from the DOM** when inactive, so
its chrome cost in the default state is zero. Recipe: 24px pill in a 32×32 button box,
`background: color-mix(in srgb, var(--warning) 14%, transparent)`, 1px 45%-`--warning` border, 12px
`--text-1` label (**9.24–10.30:1** dark / **11.68–12.00:1** light — the §2 canonical readable pair on
a hue tint), leading 8px `--warning` dot, `border-radius: 999px`. **The label is a stable word, not a
live one** — activity detail belongs in a readable panel, not in glanceable chrome. Fault state swaps
the dot for a `--danger` triangle glyph and the label for the negated word (`Not logging`); the pill
must never claim an activity it is not performing. Click opens the owning settings category. Motion:
opacity-only dot fade, 1200ms ease-out, removed entirely under `prefers-reduced-motion`. A
permanently-mounted, idle-empty visually-hidden `aria-live="polite"` region announces start and stop
**once each** — never per record.

**Density.** The whole surface is density-invariant (§3): Settings overlay and header chrome have one
geometry in `cozy` and `compact`.

**Instrumentation-only increments.** Adding observability hooks to existing components must produce
**zero** change to DOM structure, class names, computed styles, layout, scroll offset, focus or ARIA
— with the gate on or off. Design-side verification is a before/after signature capture
(`nodes`/`classList`/`getBoundingClientRect`/`scrollTop`) per instrumented root, in **one batched**
`javascript_tool` call. For any surface that re-renders on external churn — the **sidebar** re-renders
on every ref change — the comparison must be a *sequence* of signatures across a scripted churn run
(rapid checkouts, a fetch adding many refs, a deletion), not an idle end-state snapshot: a flicker is
an intermediate state and would hide in an end-state-only diff. Frame-timing parity is always a
USER CHECKPOINT (headless harness, no `requestAnimationFrame`).

**Sensitive is not destructive.** A reversible setting with a privacy consequence gets
`.settings-row--sensitive` (`--warning` leading bar + glyph) and a `primary` confirm; an action that
loses data gets `.btn-danger`, a `danger` `ConfirmDialog` naming the count and consequence, and
**its own row**, separated by the standard hairline from any benign action — never a second filled
button in the same flex line. A destructive row is **last in tab order** on its page,
**disabled-not-hidden** when it has nothing to act on, and uses `aria-disabled` rather than
`disabled` so focus survives its own success (a control that becomes `disabled` on completion drops
focus to `<body>` and strands the tab ring inside a modal).

**CSS location.** `src/styles/settings-dev.css`, imported after `settings-primitives.css`; the pill
rule lives in the existing header-toolbar stylesheet. Do not reorder the settings import list.

## 13. Icon system (SVG chrome)

Full contract: `docs/contracts/lucide-icons-ui.md`. As of the Lucide migration, all SVG chrome icons
come from **`lucide-react`** (imported per-icon, never the barrel). The prior hand-drawn glyphs used a
16×16 grid with fractional coordinates and blurred on macOS/WebKit; Lucide's 24×24 integer grid fixes
this.

- **Wrappers, not call-site changes.** `src/components/appIcons.tsx` and `src/components/menuIcons.tsx`
  keep their existing exported names (`SunIcon`, `GearIcon`, `CheckoutIcon`, …); each returns its
  mapped Lucide component with the shared `ICON_PROPS`. Call sites (~26) are untouched.
- **Render spec.** `size={16}`, `strokeWidth={2}` (Lucide default → 1.33px effective at 16px, matching
  the old ~1.4 weight). Do **not** set `absoluteStrokeWidth`. No `color` prop — glyphs inherit
  `currentColor` so state (hover/disabled/`--text-*`) and both themes flow from the enclosing button.
  No `shape-rendering`/`vector-effect` override (Lucide `geometricPrecision` is correct; `crispEdges`
  would harm curves). `strokeWidth={2.1}` is the exact-1.4 option if ever needed, but 2 is the spec.
- **A11y.** Every chrome glyph is decorative: `aria-hidden` + `focusable={false}` on the SVG; the
  accessible name lives on the button/menuitem. Hit-target geometry (§3.1) is unchanged — the box
  grows, the 16px glyph does not.
- **Bespoke exceptions** (no adequate Lucide match — kept hand-drawn on the existing `svgProps`
  recipe): `RefDotIcon` (solid filled dot), `RebaseIcon`, `RebaseInteractiveIcon`, `BisectIcon`.
- **Canvas glyphs are separate.** The commit-graph glyphs in `src/graph/draw.ts` are canvas-drawn (not
  SVG DOM), keep their 1.4 stroke, and are out of scope for this migration.
