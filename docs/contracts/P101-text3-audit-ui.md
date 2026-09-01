# P101 — the full `--text-3` audit (UI contract)

Owner: `ui-designer`. Implementer: `senior-dev`. Read-only inputs:
`docs/contracts/P98-text3-readtext-ui.md` §8.8 (the method — followed literally),
`docs/contracts/ui-reference.md` §2, `docs/contracts/P95-a11y-ui.md` §3.

**Colour-only milestone.** No new tokens, no new components, no new files, no geometry, spacing,
font-size, weight, letter-spacing, text-transform, radius, hover, motion or copy changes. Because
nothing moves and nothing new is drawn, the per-surface cozy/compact geometry tables do not apply
(same disclaimer as P95/P98). The only theme-dependent content is the contrast arithmetic, given for
**both** themes throughout.

> **Status of the "closed" claim.** This pass **completes the audit** — all 124 declarations are
> enumerated and bucketed. It does **not** close the family, because the fixes have not shipped.
> §7's hunks A–F therefore say *audit complete, fixes pending*; **Hunk A′ is the post-ship flip** and
> applies only in the commit that lands the CSS. Do not let the reference's closure wording precede
> the code — that is the P95/P100 failure mode, twice over.

---

## 1. Enumeration — re-pinned at audit time

Fresh grep, `dev` @ `0fe0102`, 2026-09-01:

```
rg -c 'color: var\(--text-3\)' src/styles   →  124 occurrences, 33 files
```

**Count reconciliation (as pre-derived by the orchestrator, confirmed here).** P98 §8.8 pinned
**122**. The delta of 2 is exactly P98's own MUST-FIX-1 disabled-hint overrides:
`src/styles/search.css` 8→9 and `src/styles/dialogs-forms.css` 6→7; **no other per-file count
moved**. Both new declarations are legitimately `disabled` bucket. My per-file distribution matches
the orchestrator's brief exactly (forge-pr 15, settings-legacy-sections 11, search 9, commit-panel 9,
sidebar 7, dialogs-forms 7, blame-history 6, repo-health 5, then 4s/3s/2s/1s), so the enumeration is
**stable at HEAD** — which is what §8.8 step 6's closure claim has to rest on.

**Per-file counts (authoritative, 124 total across 33 files):**

`forge-pr` 15 · `settings-legacy-sections` 11 · `search` 9 · `commit-panel` 9 · `sidebar` 7 ·
`dialogs-forms` 7 · `blame-history` 6 · `repo-health` 5 · `agent-assets` 4 · `commit-box` 4 ·
`controls` 4 · `dialogs` 4 · `diff-content` 4 · `ai-assets` 3 · `app-frame-header` 3 · `composer` 3 ·
`image-diff` 3 · `onboarding` 3 · `context-menu` 2 · `empty-and-errors` 2 · `git-banner` 2 ·
`identity-menu` 2 · `status-panel` 2 · `checks-panel` 1 · `diff` 1 · `git-dock` 1 ·
`right-panel-density` 1 · `settings-primitives` 1 · `settings-shell` 1 · `split-view` 1 · `tabs` 1 ·
`toasts-and-overlays` 1 · `updates` 1.

### 1.1 One grep artefact, recorded so the count stays honest

`src/styles/identity-menu.css:31` matches the grep as a **substring**: it is
`border-color: var(--text-3)`, not a `color` declaration. It is counted in the 124 (it is a real
`--text-3` use and it needs a verdict) but it is audited at the **3:1 graphics bar**, not 4.5:1.
Flagged rather than silently dropped.

### 1.2 Outside `src/styles/`

- `src/components/conflictCmSetup.ts:36` — `.cm-gutters`, already sanctioned decorative in
  `ui-reference.md` §2 (3.68 / 3.17 on `--bg-0`, with a revisit trigger). **Re-evaluated in §5.3 as
  §8.8's revisit trigger required; the sanction stands.**
- `src/graph/colors.ts:131` — reads the token for canvas use, not a CSS declaration.
- `src/components/settings/SettingsEmpty.tsx` — mentions the token in a **comment only**. Not a
  declaration, not an audit item.

### 1.3 One adjacent `--text-3` use my `color:` grep does *not* cover

`src/styles/onboarding.css:31` — `.onboarding-dot.is-done { background: var(--text-3) }`. A
**background**, so outside the 124, but it is a non-text graphic carrying step-completion state:
`--text-3` on the `.dialog-card` `--bg-1` surface = **3.38:1 dark / 2.96:1 light**, i.e. it **fails
the 3:1 graphics bar in the light theme**. Recorded as finding F-1 in §6.2; it is one line and I
fold it into the fix list because it is the same token in the same file as three audited items.

### 1.4 Two "phantom token" seeds — checked, and both are false alarms

The brief flagged `onboarding.css:26` `var(--bg-3, var(--border))` and `:35`
`var(--accent, var(--text-1))` as the same class as `--border-0` (P98 §4) and `--accent-fg` (P100).
**They are not.** Grep of `src/styles/tokens-and-base.css` confirms **both tokens are real and
defined in both themes**: `--bg-3` `#2f343d` dark (:14) / `#e2e5ea` light (:100), `--accent`
`#4f8cff` (:19) / `#2f6fe4` (:105). So the fallbacks are **dead code, never evaluated** — token
hygiene noise, not a rendering defect, and nothing renders differently today.

**Verdict: NIT, not a defect.** Prescribed anyway because the *pattern* is what P98/P100 warned
about and a reader cannot tell a live fallback from a dead one at a glance: drop both fallbacks to
bare `var(--bg-3)` and `var(--accent)`. Two lines, zero visual change, and it removes the smell that
made the brief suspect them. Not an AC — see §6.2 F-2.

---

## 2. Method — §8.8 steps 1–3, and what was measured vs derived

### 2.1 Step 1 — the composited backdrop, per state

Backdrop identity is **source-derived** from a single app-wide sweep of
`background: var(--bg-0|bg-1|bg-2|bg-3|selection)` across `src/styles/`, which resolves every
container and every `:hover` / `--active` / `.is-selected` / `.is-current` / `:focus-visible`
override in one pass. The per-state backdrop columns in §3 come from that sweep, not from guesswork,
and every backdrop found is a **fully opaque token** — no translucent ancestor intervenes anywhere in
the 124 (the same condition P98 §1 stated and its §8.1.1 then confirmed empirically on four chains).

**This is the step that changes verdicts.** 41 of the 124 sit on an element whose row has a
`:hover`, selected or focus backdrop different from its idle one, and P98's finding holds: the worst
cases are not idle. Four `--selection` fills appear in the audit — `.file-chevron` via
`.file-row-expanded:hover` (`diff.css:48`), `.search-result-oid` and `.search-result-date` via
`.search-result.is-current` (`search.css:299`), and `.context-menu-chevron` /
`.context-menu-subnote` via `.context-menu-item:focus-visible` (`context-menu.css:79`) — each at
**2.33:1 dark / 2.55:1 light**, below even the graphics bar. Two elements sit on `--bg-3`
(`.identity-menu-eyebrow`, `.row-action:hover`), a backdrop no previous pass had measured.

### 2.2 Ratio lookup table

Every figure in §3 is a lookup from this table. The `--bg-0/1/2/selection` rows are P98's
harness-measured values (its §2.1, 2026-09-01) and were re-derived here arithmetically from the
shipped hexes in `src/styles/tokens-and-base.css:11-24 / 97-107`. **The `--bg-3` row is new in this
pass**, computed from those same shipped hexes because `--bg-3` is a real backdrop for two audited
elements and no prior contract carries the figure.

| Backdrop | `--text-3` dark | `--text-3` light | `--text-2` dark | `--text-2` light |
|---|---|---|---|---|
| `--bg-0` `#16181d` / `#ffffff` | **3.67:1** ✗ | **3.17:1** ✗ | **7.89:1** ✓ | **7.98:1** ✓ |
| `--bg-1` `#1d2026` / `#f6f7f9` | **3.38:1** ✗ | **2.96:1** ✗✗ | **7.25:1** ✓ | **7.45:1** ✓ |
| `--bg-2` `#262a31` / `#eceef2` | **2.98:1** ✗✗ | **2.73:1** ✗✗ | **6.40:1** ✓ | **6.87:1** ✓ |
| `--bg-3` `#2f343d` / `#e2e5ea` | **2.58:1** ✗✗ | **2.51:1** ✗✗ | **5.56:1** ✓ | **6.32:1** ✓ |
| `--selection` `#2a3b57` / `#dbe7ff` | **2.33:1** ✗✗ | **2.55:1** ✗✗ | **5.01:1** ✓ | **6.42:1** ✓ |

✗ = below 4.5:1 (text). ✗✗ = below 3:1 (graphics). **`--text-3` clears 4.5:1 on nothing, and clears
3:1 only on `--bg-0` (both themes) and `--bg-1` (dark only).** `--text-2` clears 4.5:1 everywhere,
in both themes, with ≥0.5 of margin. `--text-1` for the hierarchy checks: **13.54 / 15.42** on
`--bg-1`, **14.73 / 16.52** on `--bg-0`, **11.95 / 14.23** on `--bg-2`, **10.37 / 13.09** on
`--bg-3`, **9.36 / 13.29** on `--selection`.

**The single most important consequence, stated up front:** `--text-3` is not a viable *text* colour
on any Bonsai surface in either theme. Any declaration that survives this audit does so because it
is **exempt from the text bar** (disabled, placeholder, structural label or coordinate glyph on a
≥3:1 backdrop), never because its ratio is adequate.

### 2.3 Step 2 — the test applied

For each declaration: *what breaks if this string is unreadable?* Not "does it look dim", not "is it
small / uppercase / letter-spaced" (P98's `.conflict-editor-split-label` was all three and was read
text). Where the answer is "the user picks wrong", "the user misidentifies the row", or "the user
cannot tell two similarly-named things apart", it is read text.

The two traps §8.8 named, and where they bit in this pass:

- **The child-rule trap.** Two audited declarations are children/siblings of a row that has its own
  `--text-3` disabled rule, so raising the child re-brightens half a disabled row exactly as P98
  §8.4 did. Both are dispositioned in §4.2: `forge-pr.css:713` `.context-menu-subnote` (child of
  `.context-menu-item:disabled`, `context-menu.css:84`) — **needs a paired override**; and
  `settings-legacy-sections.css:336` `.settings-account-default--static` (**sibling**, not
  descendant, of the two `input:disabled + span` rules) — **needs none**, and adding one would be
  dead CSS. **The first is not optional** — it is the same defect P98 shipped.
- **The specificity-tie trap.** `.context-menu-item:disabled .context-menu-subnote` is **0,2,1** and
  beats the plain **0,1,0** subnote rule regardless of source order, and there is **no**
  equal-specificity `--active` override on either child — so unlike P98 §8.4 no ordering constraint
  applies here. Stated explicitly, with the specificity arithmetic, so the next pass does not assume
  the tie is always present *or* always absent.

### 2.4 Step 3 — the five buckets, and the counts

| Bucket | Count | Disposition |
|---|---|---|
| `disabled` | **16** | exempt — dimming *is* the signal, carried also by `disabled` / `aria-disabled` / `cursor: default` |
| `placeholder-empty` | **10** | exempt — the surface itself is the message |
| `label-duplicating-structure` | **3** | exempt — settings group/section titles, §8.8's canonical example |
| `decorative-glyph`, **clears 3:1 on every state** | **2** | exempt |
| `decorative-glyph`, **fails 3:1 on ≥1 state** | **13** | **fix** — §8.8's own rule: a glyph on `--bg-2` / `--bg-3` / `--selection`, or on `--bg-1` in the light theme, is a defect even in this bucket |
| non-text graphic (a `border-color`), **fails 3:1 light** | **1** | **fix** — §1.1 |
| `read-text-violation` | **79** | **fix** to `--text-2` |
| **Total** | **124** | **93 fix · 31 exempt** |

Plus **1** non-`color` adjacent finding folded into the fix list (§1.3, `.onboarding-dot.is-done`),
and **3** of the 93 that are additionally **P95-class enabled-control escapes** (§6.1).

**93 of 124 is the honest number and it is not an over-reach.** It follows mechanically from §2.2:
`--text-3` fails the text bar on every backdrop, and Bonsai uses `--text-3` as its general
"metadata" colour. Every exempt declaration is exempt by *category*, not by ratio. §6.3 flags the
one alternative the orchestrator should rule on before this ships.

### 2.5 Basis of the audit — no new harness measurements were taken

**Stated plainly, because this document family has produced three false measured/closed claims and
I am not adding a fourth.** **I took no browser-harness measurements in this pass.** The audit is
**source-derived throughout**:

| Basis | Count |
|---|---|
| Source-derived — backdrop identity from the §2.1 **complete** app-wide `background:` sweep; ratio from the §2.2 token table (P98-measured, arithmetically re-derived here from the shipped hexes) | **124** |
| Of those, **also** measured mounted + composited — **carried over from P98 §8.1.1**, not re-taken here | **4** — `.reflog-date`, `.reflog-oid-old`, `.reflog-oid-root`, `.reflog-oid-arrow`, at 3.67 / 3.17 on the `.diff-overlay` `--bg-0` host |
| Structurally unverifiable in the harness → **USER CHECKPOINT** | **15** — all of `forge-pr.css`; the PR panel sits behind a forge-token screen and **no token was entered, in this pass or the last** |

**Harness-map facts cited in §8.1 and §9 — port 1420, the `resize_window` 1440×900 requirement (the
hidden pane reports `innerWidth/innerHeight = 0`), the `data-theme`-not-`colorScheme` theming
mechanism, the `?op=merge` route inventory, and the conflicted-`DiffOverlay` hole that keeps
`.conflict-editor-mode-btn` and `.cm-gutters` unmounted — are taken from the orchestrator's brief
(P100-verified), not from this session.** They are recorded because AC7/AC8 need them at
implementation time, not asserted as this pass's findings.

**Why source-derivation is sufficient for the *verdicts*, and where it is not.** Each bucket
decision turns on (a) which opaque token is the backdrop, which the §2.1 sweep establishes
exhaustively rather than by sampling, and (b) the `--text-3` / `--text-2` ratio against that token,
which is measured (P98) and independently re-derived. Neither depends on a live pixel. What
source-derivation cannot give is confirmation that the *shipped* CSS computes what the source says
in a real cascade — that is exactly what **AC7/AC8** are for, per declaration, at implementation
time. §9 lists every remaining gap.

No synthetic DOM was injected anywhere in this pass, and nothing is reported as "confirmed mounted"
that was not.

---

## 3. The audit — all 124 declarations, bucketed

Columns: **file:line** · **selector** · **backdrop per state** (from the §2.1 sweep) · **worst-state
`--text-3` ratio, dark / light** · **bucket** · **verdict**. "**→ 2**" = change to `var(--text-2)`,
colour value only, nothing else on the rule. Ratios are §2.2 lookups.

### 3.1 `forge-pr.css` — 15 (source-derived + USER CHECKPOINT)

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 139 | `.pr-row-num` | `.pr-row` idle `--bg-1`; **hover `--bg-2`** (:113) | 2.98 / 2.73 | read-text-violation | **→ 2** — `#1234` is how the user names the PR |
| 147 | `.pr-row-meta` | same | 2.98 / 2.73 | read-text-violation | **→ 2** — author + date + branch |
| 240 | `.pr-detail-num` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** |
| 272 | `.pr-stat-files` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — a count the user acts on |
| 366 | `.pr-comment-kind` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — only wayfinder for review vs issue comment |
| 375 | `.pr-comment-date` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — timestamp, §2's canonical example |
| 381 | `.pr-comment-loc` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — `path:line`; unreadable ⇒ the user cannot find the comment's target |
| 436 | `.pr-create-arrow` | `--bg-1` (:431) | 3.38 / **2.96** | decorative-glyph, **fails 3:1 light** | **→ 2** — defect in bucket, **plus** cohesion: `base → head` is one string |
| 456 | `.pr-field-label` | `.pr-create` `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — a form field label is read text |
| 564 | `.forge-connect-note` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — the instructions for connecting a forge |
| 628 | `.forge-account-host` | `.forge-account-trigger` idle `--bg-1`; **hover `--bg-2`** (:658) | 2.98 / 2.73 | read-text-violation | **→ 2** — the host disambiguates two accounts |
| 679 | `.forge-account-caret` | same | 2.98 / 2.73 | decorative-glyph, **fails 3:1 both** | **→ 2** — also a glyph inside an enabled trigger (P95 class) |
| 686 | `.forge-account-source` | same | 2.98 / 2.73 | read-text-violation | **→ 2** — "keychain" vs "env" is security-relevant |
| 713 | `.context-menu-subnote` | context menu `--bg-2` (`context-menu.css:12`); **`:focus-visible` `--selection`** (:79) | **2.33 / 2.55** | read-text-violation | **→ 2** + **paired disabled override, §4.2** (child-rule trap) |
| 805 | `.pr-changes-count` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** |

### 3.2 `settings-legacy-sections.css` — 11

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 27 | `.settings-section-title` | settings body `--bg-0` (`settings-shell.css:290`) | 3.67 / 3.17 | **label-duplicating-structure** | **keep** — uppercase heading over a visibly bounded group; §8.8's named example; clears 3:1 in both themes |
| 33 | `.settings-section-desc` | `--bg-0` | 3.67 / 3.17 | read-text-violation | **→ 2** — §2 already names "settings help text" as read text |
| 90 | `.settings-unit` | `--bg-0` | 3.67 / 3.17 | read-text-violation | **→ 2** — `ms` / `MB` is the *only* carrier of the unit |
| 124 | `.settings-config-subtitle` | `--bg-0` | 3.67 / 3.17 | **label-duplicating-structure** | **keep** |
| 135 | `.settings-config-hint` | `--bg-0` | 3.67 / 3.17 | read-text-violation | **→ 2** |
| 199 | `.settings-account-host` | `.settings-account-row` **`--bg-2`** (:71) | 2.98 / 2.73 | read-text-violation | **→ 2** |
| 225 | `.settings-account-state.is-disconnected .settings-account-dot` | `--bg-2` | **2.98 / 2.73** | decorative-glyph, **fails 3:1 both** | **→ 2** — and see §6.2 F-3: a bare dot is colour-only, so the adjacent state word must remain the real carrier |
| 275 | `.settings-account-kind input:disabled + span` | `--bg-2` | — | **disabled** | **keep** |
| 332 | `.settings-account-default input:disabled + span` | `--bg-2` | — | **disabled** | **keep** |
| 336 | `.settings-account-default--static` | `--bg-2` | 2.98 / 2.73 | read-text-violation | **→ 2** — inert *text*, not a disabled control; **paired disabled check run, §4.2 — no override needed** |
| 399 | `.settings-config-advanced-summary` | `--bg-0` | 3.67 / 3.17 | read-text-violation (**P95-class**) | **→ 2** — a `<summary>` with `cursor: pointer` is an **enabled interactive control**; P95 escape |

### 3.3 `search.css` — 9

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 28 | `.commit-search-icon` | `.commit-search` `--bg-0` (:47) | 3.67 / 3.17 | **decorative-glyph, clears 3:1 both** | **keep** — duplicates the visible input and its placeholder |
| 183 | `.command-palette-empty` | `.command-palette` `--bg-1` (:149) | 3.38 / 2.96 | **placeholder-empty** | **keep** |
| 192 | `.command-palette-group` | `--bg-1` | 3.38 / **2.96** | read-text-violation | **→ 2** — §8.8 is explicit that a **result-group header** is not `label-duplicating-structure`. At **10px** this is the smallest text in the whole audit and the group name is the user's only wayfinder in a ~150-command palette |
| 213 | `.command-palette-option.is-disabled` | `--bg-1` | — | **disabled** | **keep** |
| 218 | `.command-palette-option.is-disabled.is-active` | `--selection` (P100 recipe, :208) | — | **disabled** | **keep** |
| 244 | `.command-palette-option.is-disabled .command-palette-option-hint` | `--bg-1` / `--selection` | — | **disabled** | **keep** — one of the two 122→124 additions (P98 MUST-FIX-1) |
| 278 | `.search-results-empty` | `.search-results` `--bg-0` (:273) | 3.67 / 3.17 | **placeholder-empty** | **keep** |
| 304 | `.search-result-oid` | idle `--bg-0`; **hover `--bg-1`** (:296); **`.is-current` `--selection`** (:299) | **2.33 / 2.55** | read-text-violation | **→ 2** — same string class as `.reflog-oid-old`; the SHA is what the user reads to pick |
| 332 | `.search-result-date` | same three states | **2.33 / 2.55** | read-text-violation | **→ 2** — the new seed; a date, identical class to the three `blame-history.css` timestamps, and its **worst state is worse than theirs** |

### 3.4 `commit-panel.css` — 9 (right panel `--bg-1`)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 71 | `.commit-author-email` | 3.38 / 2.96 | read-text-violation | **→ 2** |
| 76 | `.commit-date` | 3.38 / 2.96 | read-text-violation | **→ 2** — timestamp |
| 106 | `.commit-signature-signer` | 3.38 / 2.96 | read-text-violation | **→ 2** — *who signed* is the whole point of the signature block |
| 110 | `.commit-signature-key` | 3.38 / 2.96 | read-text-violation | **→ 2** — the key id the user compares against a known key |
| 139 | `.compare-endpoint-summary` | 3.38 / 2.96 | read-text-violation | **→ 2** |
| 143 | `.compare-arrow` | 3.38 / **2.96** | decorative-glyph, **fails 3:1 light** | **→ 2** — defect in bucket, plus cohesion with the two endpoints |
| 160 | `.commit-parents-label` | 3.38 / 2.96 | read-text-violation | **→ 2** — the only thing identifying those oids as parents |
| 182 | `.commit-merge-note` | 3.38 / 2.96 | read-text-violation | **→ 2** — explains that a merge diff is vs the first parent; without it the diff is misleading |
| 228 | `.file-count-bin` | 3.38 / 2.96 | read-text-violation | **→ 2** — sits beside the `--success`/`--danger` add/del counts; read text and consistency |

### 3.5 `sidebar.css` — 7 (sidebar `--bg-1`, `.branch-row:hover` `--bg-2`)

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 82 | `.sidebar-add:disabled` | `--bg-1` | — | **disabled** | **keep** |
| 113 | `.branch-glyph` | idle `--bg-1`; **hover `--bg-2`** (:105) | 2.98 / 2.73 | decorative-glyph, **fails 3:1 both on hover** | **→ 2** |
| 165 | `.branch-badge` | idle `--bg-1`; hover `--bg-2` | 2.98 / 2.73 | read-text-violation | **→ 2** — the ahead/behind counts are the sidebar's reason to exist |
| 225 | `.branch-muted` | idle `--bg-1`; hover `--bg-2` | 2.98 / 2.73 | read-text-violation | **→ 2** |
| 251 | `.branch-create-input::placeholder` | `--bg-2` (:243) | — | **placeholder-empty** | **keep** — see §6.4 |
| 288 | `.list-filter-input::placeholder` | `--bg-2` (:280) | — | **placeholder-empty** | **keep** — see §6.4 |
| 294 | `.list-filter-count` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — "12 of 40" is what tells the user the filter is hiding things |

### 3.6 `dialogs-forms.css` — 7 (`.dialog-card` `--bg-1`)

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 78 | `.wt-copy-group-header` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — a group header over a checkbox list; the group name is the only wayfinder (§8.8's carve-out), not duplicated structure |
| 228 | `.combobox-option--disabled` | `--bg-1` | — | **disabled** | **keep** |
| 234 | `.combobox-option--disabled.combobox-option--active` | `--selection` (P100 recipe, :222) | — | **disabled** | **keep** |
| 262 | `.combobox-option--disabled .combobox-option-hint` | `--bg-1` / `--selection` | — | **disabled** | **keep** — the second 122→124 addition |
| 273 | `.rebase-plan-hint` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** |
| 345 | `.rebase-plan-commit.dropped` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — **safe**: `text-decoration: line-through` (:344) is a non-colour carrier of "dropped", so the state survives the swap. The user must still read *which* commit they dropped before rebasing |
| 426 | `.clone-progress-detail` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — live progress text |

### 3.7 `blame-history.css` — 6 (pre-seeded verdicts; `.diff-overlay` `--bg-0` host)

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 93 | `.blame-date` | idle `--bg-0`; **`.blame-gutter:hover` `--bg-2`** (:49) | **2.98 / 2.73** | read-text-violation | **→ 2** — seed. Note the hover state is **worse** than the 3.67 / 3.17 P98 recorded; recorded here for the first time |
| 112 | `.blame-lineno` | idle `--bg-0` (3.67 / 3.17); **hover `--bg-2`** | **2.98 / 2.73** | decorative-glyph (coordinate), **fails 3:1 on hover** | **→ 2** — the one place I depart from the `.cm-gutters` sanction, and the reason is narrow: `.cm-gutters` never leaves `--bg-0`, where it clears 3:1; this gutter does. The sanction is about a *convention on a `--bg-0` surface*, not a blanket exemption for line numbers |
| 167 | `.file-history-date` | idle `--bg-0`; **`.file-history-main:hover` `--bg-1`** (:144) | 3.38 / 2.96 | read-text-violation | **→ 2** — seed |
| 214 | `.reflog-oid-old, .reflog-oid-arrow` (**one grouped declaration**) | idle `--bg-0`; **`.reflog-row:hover` `--bg-1`** (:186) | 3.38 / 2.96 | read-text-violation (oid) + decorative-glyph (arrow) | **→ 2** — seed; **measured mounted** by P98. See the cohesion note below |
| 218 | `.reflog-oid-root` | same | 3.38 / 2.96 | read-text-violation | **→ 2** — seed; **measured mounted** |
| 241 | `.reflog-date` | same | 3.38 / 2.96 | read-text-violation | **→ 2** — seed; **measured mounted** |

> **The `.reflog-oid-arrow` cohesion note — stated as §8.8 requires, so the precedent is not
> misread.** `.reflog-oid-old` is read text: the `abc1234 → def5678` pair is what the user reads to
> pick a reset target, which is the entire purpose of the reflog view. `.reflog-oid-arrow` is a
> **genuinely decorative separator that already clears the 3:1 graphics bar** on `--bg-0`
> (3.67 / 3.17) in both themes. It moves **for cohesion only** — one visual string rendered half
> `--text-2` and half `--text-3` reads as a rendering bug. **This is explicitly not a contrast
> justification, and it does not establish that decorative glyphs need 4.5:1.** It is also
> physically the same declaration as `.reflog-oid-old` (they share the rule at `:212-215`), so
> leaving it behind would mean splitting a rule — a structural change this colour-only milestone does
> not permit. (Its `--bg-1` hover state at 2.96 light is separately marginal, but that is a second
> reason, not the stated one.)

### 3.8 `repo-health.css` — 5 (`--bg-1`)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 16 | `.health-elapsed` | 3.38 / 2.96 | read-text-violation | **→ 2** — an elapsed-time metric |
| 21 | `.health-caption` | 3.38 / 2.96 | read-text-violation | **→ 2** — the sentence explaining a metric |
| 36 | `.health-metric-label` | 3.38 / 2.96 | read-text-violation | **→ 2** — the only thing naming the number beside it |
| 80 | `.health-stat-size` | 3.38 / 2.96 | read-text-violation | **→ 2** |
| 86 | `.health-stale-base` | 3.38 / 2.96 | read-text-violation | **→ 2** — which base a branch is stale against |

### 3.9 Four-declaration files

**`agent-assets.css`** (panel `--bg-1`; `.asset-compare` `--bg-2` at :150)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 13 | `.agent-group-empty` | 3.38 / 2.96 | **placeholder-empty** | **keep** — a one-line "nothing here" under a visible group title |
| 64 | `.asset-field-hint` | 3.38 / 2.96 | read-text-violation | **→ 2** — §2 names "hints" as read text |
| 139 | `.asset-compare-head` | **2.98 / 2.73** | read-text-violation | **→ 2** — the only wayfinder for which compare column is which; the `.conflict-editor-split-label` precedent exactly |
| 162 | `.asset-compare-empty` | 2.98 / 2.73 | **placeholder-empty** | **keep** |

**`commit-box.css`**

| line | selector | backdrop / states | bucket | verdict |
|---|---|---|---|---|
| 112 | `.row-action:disabled` | `--bg-1`; hover **`--bg-3`** (:106) | **disabled** | **keep** |
| 318 | `.commit-message::placeholder` | `--bg-2` (:286) | **placeholder-empty** | **keep** — see §6.4 |
| 352 | `.commit-counter` | `--bg-1` (:267) → 3.38 / 2.96 | read-text-violation | **→ 2** — the 50/72-char counter is read *in order to edit* |
| 421 | `.commit-msg-tool:disabled` | `--bg-2` (:417) | **disabled** | **keep** |

**`controls.css`**

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 49 | `.recents-item-path` | `--bg-1`; **`.recents-item:hover` `--bg-2`** (:34) | 2.98 / 2.73 | read-text-violation | **→ 2** — the path is the only way to tell two repos called `app` apart |
| 131 | `.btn-icon:disabled` | varies | — | **disabled** | **keep** |
| 195 | `.toolbar-btn:disabled` | `--bg-1` | — | **disabled** | **keep** |
| 236 | `.toolbar-job-status` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — "Fetching…", "Push failed" |

**`dialogs.css`** (`.dialog-card` `--bg-1`)

| line | selector | backdrop | d/l | bucket | verdict |
|---|---|---|---|---|---|
| 54 | `.dialog-body-note` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — often the consequence line above a destructive confirm |
| 143 | `.hook-output-note` | `.hook-output` **`--bg-2`** (:132) | 2.98 / 2.73 | read-text-violation | **→ 2** |
| 210 | `.op-ref-arrow` | `--bg-1` | 3.38 / **2.96** | decorative-glyph, **fails 3:1 light** | **→ 2** — defect in bucket + cohesion with the ref pair |
| 241 | `.op-rationale` | `--bg-1` | 3.38 / 2.96 | read-text-violation | **→ 2** — **highest-stakes item in the audit**: the "what this operation will do" sentence in an operation-confirm dialog. Destructive-action UX requires the consequence to be legible |

**`diff-content.css`**

| line | selector | backdrop | d/l | bucket | verdict |
|---|---|---|---|---|---|
| 83 | `.diff-hunk-header` | **its own `background: var(--bg-2)`** (:84) | **2.98 / 2.73** | read-text-violation | **→ 2** — `@@ -1,7 +1,9 @@` plus the enclosing function name *is* how a user navigates a diff |
| 144 | `.diff-lineno` | `--bg-0` / `--bg-1` gutter | 3.67 / 3.17 | **decorative-glyph, clears 3:1 both** | **keep** — the `.cm-gutters` precedent applied consistently: a coordinate that duplicates visible structure, and it never sits on `--bg-2`. **Added to §2's sanctioned-decorative list in §7, with the same revisit trigger** |
| 218 | `.diff-nonewline .diff-content` | `--bg-0` (:226) | 3.67 / 3.17 | read-text-violation | **→ 2** — "\ No newline at end of file" is unique diff information the user may need to act on, not a coordinate |
| 225 | `.diff-placeholder` | `--bg-0` | 3.67 / 3.17 | **placeholder-empty** | **keep** |

### 3.10 Three-declaration files

**`ai-assets.css`**

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 104 | `.asset-row-path` | `.asset-row` **`--bg-2`** (:68) | 2.98 / 2.73 | read-text-violation | **→ 2** — the path identifies the asset |
| 224 | `.stale-summary` | `--bg-1`; **`.stale-row:hover` `--bg-2`** (:200) | 2.98 / 2.73 | read-text-violation | **→ 2** — what is stale and why |
| 233 | `.stale-time` | same | 2.98 / 2.73 | read-text-violation | **→ 2** — timestamp |

**`app-frame-header.css`** (header `--bg-1`)

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 42 | `.repo-path` | `--bg-1`; **`.header-repo-btn:hover` `--bg-2`** (:86) | 2.98 / 2.73 | read-text-violation | **→ 2** — the open repo's path; disambiguates same-named repos across tabs |
| 95 | `.repo-switcher-caret` | same | 2.98 / 2.73 | decorative-glyph, **fails 3:1 both** | **→ 2** — also a glyph inside an enabled control (P95 class) |
| 141 | `.repo-switcher-item-path` | dropdown `--bg-1` (:107); **`.repo-switcher-item:hover` `--bg-2`** (:131) | 2.98 / 2.73 | read-text-violation | **→ 2** |

**`composer.css`**

| line | selector | backdrop | d/l | bucket | verdict |
|---|---|---|---|---|---|
| 65 | `.composer-note` | **its own `background: var(--bg-0)`** (:67) | 3.67 / 3.17 | read-text-violation | **→ 2** — a note bar is prose |
| 113 | `.composer-card-count` | `.composer-card` **`--bg-2`** (:90); `--unassigned` `--bg-0` (:95) | 2.98 / 2.73 | read-text-violation | **→ 2** |
| 151 | `.composer-empty` | `--bg-1` (:24) | 3.38 / 2.96 | **placeholder-empty** | **keep** |

**`image-diff.css`** (diff surface `--bg-0`)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 52 | `.img-diff-size` | 3.67 / 3.17 | read-text-violation | **→ 2** — dimensions + byte size are the *only* diff information a binary image diff carries |
| 76 | `.img-diff-missing` | 3.67 / 3.17 | **placeholder-empty** | **keep** — the surface's whole message is "not present in this revision" |
| 145 | `.img-diff-hint` | 3.67 / 3.17 | read-text-violation | **→ 2** |

**`onboarding.css`** (`.dialog-card.onboarding-card` `--bg-1`)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 98 | `.onboarding-field-label` | 3.38 / 2.96 | read-text-violation | **→ 2** — a form field label |
| 129 | `.onboarding-identity-row dt` | 3.38 / 2.96 | read-text-violation | **→ 2** — the `<dt>` is the only wayfinder for its `<dd>` value |
| 208 | `.onboarding-skip` | 3.38 / 2.96 | read-text-violation (**P95-class**) | **→ 2** — "Skip" is an **enabled control label** on the first screen the user ever sees; P95 escape |
| *(31)* | *`.onboarding-dot.is-done` — a `background`, §1.3* | 3.38 / **2.96** | *non-text graphic, fails 3:1 light* | **→ 2** (folded in as F-1) |

### 3.11 Two-declaration files

**`context-menu.css`** (menu `--bg-2`; `.context-menu-item:focus-visible` **`--selection`** at :79)

| line | selector | worst d/l | bucket | verdict |
|---|---|---|---|---|
| 84 | `.context-menu-item:disabled` | — | **disabled** | **keep** — but its `.context-menu-subnote` child needs §4.2 |
| 115 | `.context-menu-chevron` | **2.33 / 2.55** on the `:focus-visible` fill | decorative-glyph, **fails 3:1 on every state** | **→ 2** — the submenu chevron is also the only indicator that an item *has* a submenu |

**`empty-and-errors.css`** (`--bg-0`)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 40 | `.empty-tagline` | 3.67 / 3.17 | read-text-violation | **→ 2** — see the note |
| 46 | `.empty-subhead` | 3.67 / 3.17 | read-text-violation | **→ 2** |

> **Why both, and not `placeholder-empty`.** §8.8's exemption is for copy "where the surface itself
> is the message" and explicitly withholds it from "an empty state that **names the fix**". This is
> the no-repo screen — the first screen the app ever shows — and between them these two lines carry
> the tagline and the "what to do next" sentence, which `ui-reference.md` §8 already requires at
> `--text-2`. Rather than guess which of the two carries the instruction, both move. **If senior-dev
> finds `:40` is genuinely a decorative flourish carrying no instruction, leaving it at `--text-3`
> is acceptable — but report it, do not silently skip it.**

**`git-banner.css`** (banner `--bg-1` at :21)

| line | selector | d/l | bucket | verdict |
|---|---|---|---|---|
| 150 | `.git-banner-label` | 3.38 / 2.96 | read-text-violation | **→ 2** — "MERGING" / "REBASING" is the only wayfinder for repo state |
| 165 | `.git-banner-bullet` | 3.38 / **2.96** | decorative-glyph, **fails 3:1 light** | **→ 2** — defect in bucket + cohesion with the text either side |

**`identity-menu.css`**

| line | selector | backdrop | d/l | bucket | verdict |
|---|---|---|---|---|---|
| 31 | `.identity-avatar[data-identity-state='loading']` — **`border-color`**, §1.1 | header `--bg-1` | 3.38 / **2.96** | non-text graphic, **fails 3:1 light** | **→ 2** — judged at 3:1, not 4.5:1. It is a state ring, so it must clear the graphics bar in both themes |
| 70 | `.identity-menu-eyebrow` | menu `--bg-2` / **`--bg-3`** (:22) | **2.58 / 2.51** | read-text-violation | **→ 2** — an eyebrow label is the only wayfinder for the block under it; the worst `--bg-3` case in the audit |

**`status-panel.css`** (right panel `--bg-1`)

| line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| 92 | `.file-dir` | `--bg-1`; **`.file-row:hover` `--bg-2`** (:72) | 2.98 / 2.73 | read-text-violation | **→ 2** — *which* `index.ts` am I staging. This is the app's core surface |
| 141 | `.tree-dir-name` | `--bg-1`; **`.tree-dir-row:hover` `--bg-2`** (:119) | 2.98 / 2.73 | read-text-violation | **→ 2** |

### 3.12 One-declaration files

| file:line | selector | backdrop / states | worst d/l | bucket | verdict |
|---|---|---|---|---|---|
| `checks-panel.css:165` | `.checks-glyph--neutral` | `--bg-1` (:21); **`.checks-row:hover` `--bg-2`** (:147) | **2.98 / 2.73** | decorative-glyph, **fails 3:1 both on hover** | **→ 2** — and it is not really decorative: it is a **status** carrier beside `--pending` / `--success` / `--danger` siblings. Its glyph *shape* is the non-colour carrier and is unchanged |
| `diff.css:31` | `.file-chevron` | `--bg-1`; **`.file-row-expanded:hover` `--selection`** (:48) | **2.33 / 2.55** | decorative-glyph, **fails 3:1 on every state** | **→ 2** — also the affordance of an enabled expand/collapse row |
| `git-dock.css:230` | `.git-dock-clear:disabled` | `--bg-1` | — | **disabled** | **keep** |
| `right-panel-density.css:100` | `.section-label` | right panel `--bg-1` (`graph-canvas.css:67`) | 3.38 / **2.96** | read-text-violation | **→ 2** — "STAGED" / "UNSTAGED". The *grouping* is visible; **which group is which is carried only by this string**, which is §8.8's split-pane-label carve-out. Highest-traffic fix in the audit |
| `settings-primitives.css:145` | `.settings-reset` | settings body `--bg-0` | 3.67 / 3.17 | read-text-violation (**P95-class**) | **→ 2** — an **enabled icon-only button** that hovers to `--text-1` (:150). §2's P95 rule names "hover-revealed gutter controls" and "icon-only glyphs" explicitly; P95 escape. Its `:disabled` companion (the `opacity: .55` block at :153) is untouched |
| `settings-shell.css:412` | `.settings-group-title` | `--bg-0` | 3.67 / 3.17 | **label-duplicating-structure** | **keep** — §8.8's canonical example; clears 3:1 in both themes |
| `split-view.css:53` | `.diff-split-nonewline` | `--bg-0`; `.diff-split-filler` `--bg-2` (:48) | 2.98 / 2.73 | read-text-violation | **→ 2** — same string and same reasoning as `diff-content.css:218`; the two **must move together** |
| `tabs.css:117` | `.tab-add:disabled` | tab strip `--bg-1` (:140) | — | **disabled** | **keep** |
| `toasts-and-overlays.css:145` | `.shortcut-plus` | overlay `--bg-1`; the keys either side are `--bg-2` chips (:154) | 3.38 / **2.96** | decorative-glyph, **fails 3:1 light** | **→ 2** — defect in bucket + cohesion: `Ctrl + K` is one string and both keys are brighter |
| `updates.css:52` | `.update-version-from` | `.update-card` `--bg-1` (:19) | 3.38 / 2.96 | read-text-violation | **→ 2** — the version you are updating *from*; the user reads both halves of `1.2.3 → 1.3.0` |

---

## 4. The fix list — mechanically implementable

### 4.1 The 93 swaps

**Every one is the same edit: `color: var(--text-3)` → `color: var(--text-2)` on the named line,
and nothing else on that rule.** No size, weight, style, spacing, padding, margin, radius, border,
background, transition, hover or focus declaration is added, removed or altered anywhere.
`.rebase-plan-commit.dropped` keeps `line-through`; `.settings-config-advanced-summary` keeps
`text-transform: uppercase` + `letter-spacing` + `cursor: pointer`; `.reflog-oid-root` keeps
`font-style: italic`; the labels that move keep their `text-transform` and `letter-spacing`.
**Locate by selector, not by line number** — the swaps are one-for-one value replacements so lines
do not shift, but verify against §3's selector before editing.

Two non-`color` edits: **F-1** `src/styles/onboarding.css:31` `background: var(--text-3)` →
`var(--text-2)`, and `identity-menu.css:31` `border-color: var(--text-3)` → `var(--text-2)` (the
latter is already inside the 93, per §1.1).

**Line-by-line, by file (93 declarations + F-1):**

| File | Lines to swap | n |
|---|---|---|
| `forge-pr.css` | 139, 147, 240, 272, 366, 375, 381, 436, 456, 564, 628, 679, 686, 713, 805 | 15 |
| `commit-panel.css` | 71, 76, 106, 110, 139, 143, 160, 182, 228 | 9 |
| `settings-legacy-sections.css` | 33, 90, 135, 199, 225, 336, 399 | 7 |
| `blame-history.css` | 93, 112, 167, 214, 218, 241 | 6 |
| `repo-health.css` | 16, 21, 36, 80, 86 | 5 |
| `sidebar.css` | 113, 165, 225, 294 | 4 |
| `dialogs-forms.css` | 78, 273, 345, 426 | 4 |
| `dialogs.css` | 54, 143, 210, 241 | 4 |
| `ai-assets.css` | 104, 224, 233 | 3 |
| `app-frame-header.css` | 42, 95, 141 | 3 |
| `onboarding.css` | 98, 129, 208 **+ 31 (`background`, F-1)** | 3 (+1) |
| `search.css` | 192, 304, 332 | 3 |
| `agent-assets.css` | 64, 139 | 2 |
| `controls.css` | 49, 236 | 2 |
| `composer.css` | 65, 113 | 2 |
| `diff-content.css` | 83, 218 | 2 |
| `empty-and-errors.css` | 40, 46 | 2 |
| `git-banner.css` | 150, 165 | 2 |
| `identity-menu.css` | 31 (**`border-color`**), 70 | 2 |
| `image-diff.css` | 52, 145 | 2 |
| `status-panel.css` | 92, 141 | 2 |
| `checks-panel.css` | 165 | 1 |
| `commit-box.css` | 352 | 1 |
| `context-menu.css` | 115 | 1 |
| `diff.css` | 31 | 1 |
| `right-panel-density.css` | 100 | 1 |
| `settings-primitives.css` | 145 | 1 |
| `split-view.css` | 53 | 1 |
| `toasts-and-overlays.css` | 145 | 1 |
| `updates.css` | 52 | 1 |
| **30 files** | | **93 + 1** |

Three of the 33 files are untouched: `git-dock.css`, `settings-shell.css`, `tabs.css` (a single
`disabled` / `label-duplicating-structure` declaration each).

**Post-fix grep arithmetic — get this right, AC1/AC2 depend on it.** 124 − 93 = **31 surviving
originals**, in **14 files**: `search` 6, `settings-legacy-sections` 4, `dialogs-forms` 3,
`sidebar` 3, `commit-box` 3, `controls` 2, `agent-assets` 2, `diff-content` 2, `context-menu` 1,
`composer` 1, `image-diff` 1, `git-dock` 1, `settings-shell` 1, `tabs` 1. **The §4.2 rule *adds* one
`color: var(--text-3)` declaration**, so the grep after implementation returns **32 matches across
14 files** — 31 originals + 1 addition. A result of 31 means the §4.2 rule is missing.

### 4.2 The disabled-sibling checks (§8.8 step 5, first check)

Raising a child inside a row that carries a `--text-3` disabled rule re-brightens half of every
disabled row. **This is P98 MUST-FIX-1 recurring.** Both candidates were checked; one needs a rule
and one does not.

1. **`src/styles/context-menu.css` — ADD** this rule, after the `.context-menu-item:disabled` block:

   ```
   /* P101 §4.2: the disabled exemption applies to the subnote too — the child rule
      (forge-pr.css .context-menu-subnote) would otherwise re-brighten half a disabled
      menu item. Specificity 0,2,1 beats the plain 0,1,0 subnote rule regardless of
      source order; there is no equal-specificity --active override to tie against. */
   .context-menu-item:disabled .context-menu-subnote {
     color: var(--text-3);
   }
   ```
   Placement: **this file, not `forge-pr.css`** — the disabled rule it defends lives here.
   `context-menu.css` imports *before* `forge-pr.css` in `src/styles.css`, which is precisely why the
   specificity margin (not source order) has to carry it; confirmed above that it does.

2. **`src/styles/settings-legacy-sections.css` — ADD NOTHING.**
   `.settings-account-default--static` (:336) is a **sibling** of the two `input:disabled + span`
   rules (:275, :332), not a descendant, so nothing inherits through it and a defensive rule would
   be dead CSS. Recorded because the check was run and came back negative — an unrecorded negative
   is indistinguishable from a skipped check.

**Re-measurement after the swap (AC6):** a disabled context-menu item must render label **and**
subnote at the computed `--text-3` value — **2.98:1 dark / 2.73:1 light** on the menu's `--bg-2`, and
**2.33 / 2.55** on the `:focus-visible` `--selection` fill. The six P98 disabled option/hint
declarations (`dialogs-forms.css:228/234/262`, `search.css:213/218/244`) are **unchanged by P101** and
must still measure `--text-3`.

### 4.3 Subordination proof (§8.8 step 5, second check) — the ratio-of-ratios, derived once

§8.8 requires the swapped element to be proven still subordinate to its primary by measuring both.
Because every one of the 93 pairs a `--text-1` primary with the newly-`--text-2` secondary **on the
same backdrop**, the ratio-of-ratios is a constant, and here it is with the derivation:

| Backdrop | `--text-1` (primary) d/l | `--text-2` (new secondary) d/l | **ratio-of-ratios** | old `--text-3` step |
|---|---|---|---|---|
| `--bg-0` | 14.73 / 16.52 | 7.89 / 7.98 | **1.87× / 2.07×** | 4.01× / 5.21× |
| `--bg-1` | 13.54 / 15.42 | 7.25 / 7.45 | **1.87× / 2.07×** | 4.01× / 5.21× |
| `--bg-2` | 11.95 / 14.23 | 6.40 / 6.87 | **1.87× / 2.07×** | 4.01× / 5.21× |
| `--bg-3` | 10.37 / 13.09 | 5.56 / 6.32 | **1.87× / 2.07×** | 4.02× / 5.21× |
| `--selection` | 9.36 / 13.29 | 5.01 / 6.42 | **1.87× / 2.07×** | 4.02× / 5.21× |

**The step is backdrop-independent at ~1.87× dark / ~2.07× light** — it is a property of the token
pair, not of the surface, so one derivation covers all 93 rather than 93 assertions. It is the same
step the app already uses ~40 times for `--text-1` primary + `--text-2` secondary, so the swapped
surfaces now **match** the house convention instead of diverging from it. The step shrinks from
~4–5× to ~1.9–2.1×: less emphatic, still unmistakable, and in every case reinforced by an untouched
**size step** (10–12px secondary vs 12–14px primary) and, in list rows, by right-edge `flex: none`
placement.

**Three cases where the colour step is not the carrier and must be looked at perceptually** — these
are AC12/AC15 USER CHECKPOINT items, not claims:
- `.settings-account-dot` and `.checks-glyph--neutral` — a dot/glyph whose *sibling states* are hues
  (`--success`, `--danger`, `--warning`). After the swap the neutral state is a neutral grey against
  coloured siblings, which is the intended "no signal" reading, but it is the one place a reviewer
  could reasonably see it as gaining emphasis.
- `.branch-glyph` and `.file-chevron` — glyphs that now sit at the same token as the row label they
  precede. Subordination is carried by size and by the label's `--text-1`.
- `.command-palette-group` at 10px vs 13px option labels — the smallest size step in the audit.

### 4.4 Sequencing — this is a 3-increment job, not one

94 edits across 30 files is too large for one review diff. **Recommended split**, each independently
shippable and each a clean grep target:

| Increment | Files | Edits | Why grouped |
|---|---|---|---|
| **P101a — core workflow surfaces** | `right-panel-density`, `status-panel`, `commit-panel`, `commit-box`, `sidebar`, `diff`, `diff-content`, `split-view`, `blame-history`, `search`, `context-menu` (+ the §4.2 rule) | 36 | The daily-driver panes and the four `--selection` / `--bg-2` worst cases. Highest user value, and it retires the child-rule risk first |
| **P101b — dialogs, chrome, onboarding** | `dialogs`, `dialogs-forms`, `controls`, `app-frame-header`, `identity-menu`, `empty-and-errors`, `onboarding` (+ F-1), `git-banner`, `toasts-and-overlays`, `updates`, `image-diff`, `composer` | 27 | Overlay/chrome family; includes the destructive-confirm `.op-rationale` |
| **P101c — settings, AI, forge, health** | `settings-legacy-sections`, `settings-primitives`, `repo-health`, `ai-assets`, `agent-assets`, `checks-panel`, `forge-pr` | 31 | Isolates the 15 unverifiable `forge-pr` declarations in the last increment, so the USER CHECKPOINT is one review, not three |

Hunks A–F (§7) apply with **P101a**; **Hunk A′** applies with **P101c**, the increment that completes
the fix list.

---

## 5. Cross-cutting decisions

### 5.1 What `--text-3` becomes after P101

The audit's real output is a **redefinition of the token's role**, and this is what §2 should carry:

> `--text-3` has exactly **four** sanctioned roles and is never used for anything else:
> **(1)** the text/glyph of a **disabled** control or row (16 declarations); **(2)** `::placeholder`
> and empty-state copy where the surface itself is the message (10); **(3)** an **uppercase settings
> group/section title** over a visibly bounded group (3); **(4)** a **coordinate or
> duplicate-structure glyph on a `--bg-0` surface**, where it clears the 3:1 graphics bar in both
> themes (2 + `.cm-gutters`). It is **never** read text, never an enabled control's label or glyph,
> and never used on `--bg-2`, `--bg-3` or `--selection` in any role.

That last clause is the cheap, checkable invariant a future pass can grep for: `--text-3` may not
appear on a `--bg-2` / `--bg-3` / `--selection` surface at all, because it fails even the graphics
bar there in both themes.

### 5.2 The `--bg-3` figures are new and belong in the reference

No prior contract measured `--text-3` or `--text-2` on `--bg-3`. Both rows are added to §2 in §7's
Hunk C. `--text-3` on `--bg-3` (**2.58 / 2.51**) is the second-worst pair in the app after
`--selection`.

### 5.3 `.cm-gutters` re-evaluated, as §8.8's revisit trigger required

The §2 trigger is: *"if a line number ever becomes actionable or is named in copy — go-to-line,
line-range selection, any message citing a line — it becomes read text."* Checked at HEAD: no
go-to-line control, no line-range selection UI, and no user-facing string in `src/` that cites a
line number in the conflict editor. **The sanction stands, unchanged.** `.cm-gutters` also stays on
`--bg-0` (`conflicts.css:105`), where 3.68 / 3.17 clears the graphics bar in both themes, so §5.1's
new "never on `--bg-2` / `--bg-3` / `--selection`" clause does not touch it either. **I could not
reach it mounted** (§2.5) and am not claiming to have.

`diff-content.css:144` `.diff-lineno` is ruled the **same way for the same reasons** and joins the
sanctioned list. `blame-history.css:112` `.blame-lineno` is ruled the **other** way, and the
distinction is purely the backdrop: its gutter goes to `--bg-2` on hover, where 2.98 / 2.73 fails
the graphics bar. The three rulings are consistent under §5.1's clause; they are not three different
opinions about line numbers.

---

## 6. Flagged for the orchestrator

### 6.1 Three P95-class escapes, found by this audit and folded in

P95 claimed the enabled-interactive-control class was swept app-wide, and `ui-reference.md` §2 says
"a new occurrence is a defect". These three are **not new** — they predate P95 and it missed them:

| Declaration | Why it is an enabled control |
|---|---|
| `settings-primitives.css:145` `.settings-reset` | icon-only ↺ button; hovers to `--text-1` (:150). §2 names "hover-revealed gutter controls" and "icon-only glyphs" explicitly |
| `settings-legacy-sections.css:399` `.settings-config-advanced-summary` | a `<summary>` with `cursor: pointer` and `user-select: none` — a disclosure control |
| `onboarding.css:208` `.onboarding-skip` | the "Skip" action on the first screen the app ever shows |

Plus two glyphs *inside* enabled controls, which P95's icon-only clause also covers:
`app-frame-header.css:95` `.repo-switcher-caret` and `forge-pr.css:679` `.forge-account-caret`.

**No action requested beyond the swaps already in §4.1** — they are the same one-line edit. Flagged
because it means **P95's app-wide claim was as unevidenced as P98's**, and §7's Hunk A says so.

### 6.2 Three findings adjacent to the audit

- **F-1 — `.onboarding-dot.is-done` (`onboarding.css:31`), fixed here.** A `background`, so outside
  the 124; **2.96:1 in light**, below the 3:1 graphics bar for a state-carrying dot. One line, same
  file, same token — folded into §4.1 / P101b.
- **F-2 — the two dead fallbacks (`onboarding.css:26`, `:35`), NIT, not folded in.** Both tokens
  resolve (§1.4), so nothing renders wrong. Prescribed: drop the fallbacks. **Not an AC** — it is
  the only non-`--text-3` hygiene item I would let out of scope, because unlike `--border-0` it has
  zero rendered consequence. **Recommend a TODO.md line, not a rework round.**
- **F-3 — `.settings-account-dot` is colour-only, and that outlives P101.** After the swap the
  connected/disconnected states are `--success` vs `--text-2`, i.e. still **colour as the sole
  carrier** on the dot itself. Per the house rule (the A/M/D/U/R badges) it needs a letter/shape or
  an adjacent word. The adjacent `.settings-account-state` text almost certainly already provides it
  — **I could not reach the surface to confirm** (the accounts row needs a configured forge). Filed
  as a **follow-up milestone**, not a P101 fix: it is a component/markup question, not a token swap.

### 6.3 The one call I want the orchestrator to make: swap 93, or retune the token?

**The alternative, worked with numbers so the decision is informed.** Instead of 93 CSS edits, retune
the `--text-3` token so it clears 4.5:1 on the surfaces that matter. To clear 4.5:1 on `--bg-2` (the
hover backdrop of most list rows) the token would have to be ≈`#909aa6` dark / ≈`#6c727c` light.

**I recommend the 93 swaps (option A), and I recommend against the retune.** Reasons, in order:

1. **The retune does not actually solve it.** Clearing 4.5:1 on a `--selection` fill requires
   luminance ≈ `--text-2`'s. The four `--selection` cases (`.search-result-*`, `.file-chevron`,
   `.context-menu-*`) would still need `--text-2`. A retune leaves the worst cases unfixed.
2. **It destroys the hierarchy it is meant to save.** At `#909aa6` the `--text-2` ↔ `--text-3` step
   collapses from 2.15× to ~1.30× — the app would have a three-token scale with two
   indistinguishable steps, which is worse than a clean two-token scale.
3. **Blast radius is larger, not smaller.** A token retune changes all 124 declarations *plus* the
   31 deliberately-exempt ones (every disabled control and placeholder in the app gets brighter,
   destroying the disabled affordance), *plus* the non-`color` uses in `onboarding.css` and
   `src/graph/colors.ts` (canvas). 2 lines of CSS with an app-wide visual diff is harder to review
   than 94 lines with a mechanical one.
4. **`--text-2` is already the house token for exactly this role**, used ~40× beside `--text-1`.
   Option A makes 93 surfaces *consistent*; the retune makes all of them slightly different from
   both.

**The honest cost of option A, stated rather than hidden:** the app will read a little flatter,
because a lot of metadata brightens at once. The compensating hierarchy is already in place — the
size step (10–12px vs 12–14px) is untouched everywhere, `--text-1` remains exclusively the primary,
and `--text-2` is never a primary anywhere in Bonsai. **AC13 makes that a USER CHECKPOINT and I am
not asserting it from numbers.** If after seeing it the user finds specific surfaces too flat, the
correct remedy is a *weight* or *size* adjustment on the primary, not a return to sub-AA colour.

### 6.4 The four `::placeholder` declarations — bucketed exempt, flagged as arguable

`commit-box.css:318`, `sidebar.css:251`, `sidebar.css:288` are bucketed `placeholder-empty` per §8.8
and **kept**. The counter-argument, which I am recording rather than burying: WCAG 1.4.3 has no
placeholder exemption, and `.commit-message::placeholder` on `--bg-2` at **2.98 / 2.73** is the
*only* prompt in an empty commit box. **My recommendation: keep them at `--text-3` in P101** —
raising a placeholder toward `--text-2` makes an empty field look filled, which is a worse usability
outcome than the contrast gain — and if the user wants them addressed, the right fix is a **visible
label**, not a brighter placeholder: a markup change, not a token swap. **Flagged for the
orchestrator's call.**

### 6.5 Scope note on the "cohesion" justifications

Six of the moved glyphs (`.compare-arrow`, `.op-ref-arrow`, `.pr-create-arrow`,
`.git-banner-bullet`, `.shortcut-plus`, `.context-menu-chevron`) *also* independently fail the 3:1
bar in at least one state, so cohesion is their second reason, not their only one.
**`.reflog-oid-arrow` is the sole case where cohesion is the only reason** on its idle backdrop, and
that distinction is written into §3.7 and into Hunk D so the precedent cannot be misread as
"decorative glyphs need 4.5:1".

---

## 7. `ui-reference.md` — verbatim, line-anchored hunks

**Not applied by me.** My `Write` is whole-file only and this file is ~1322 lines / ≈40k tokens; a
whole-file rewrite truncates mid-file, which is the structural cause of the P95 silent failure.
**The orchestrator applies these with `Edit`.**

**Current state:** 1322 lines, 13 `##` top-level sections, tail sentinel = the icon-system
canvas-glyph sentence.

| Hunk | Anchor | Find | Replace | Net |
|---|---|---|---|---|
| A | lines 72–86 | 15 | 15 | **0** |
| B | line 58 | 1 | 1 | **0** |
| C | lines 88–98 | 11 | 32 | **+21** |
| D | lines 118–122 | 5 | 10 | **+5** |
| E | lines 191–198 | 8 | 12 | **+4** |
| F | lines 216–217 | 2 | 3 | **+1** |
| | | | **total** | **+31** |

**Expected after A–F: 1353 lines, still 13 `##` sections, unchanged tail sentinel.** All six hunks
are inside §2 (lines 46–233); no other section is touched. **Hunk A′ (post-ship) is net 0**, so the
final state after P101c is also **1353**.

Robust invariants to check alongside the line count, in case an editor normalises wrapping:
13 `##` sections · tail sentinel unchanged · `"2.6:1"` and `"3.6:1"` appear **nowhere** in §2 ·
the string `--bg-3` appears in the §2 contrast matrix · `"122"` appears nowhere in §2.

> **Apply-order note:** apply the hunks **bottom-up (F → E → D → C → B → A)** so earlier line
> numbers stay valid.

---

### Hunk A — §2 scope paragraph. Replace **lines 72–86**.

**Find (verbatim, 15 lines):**

```
**Scope of the `--text-3` work so far — stated honestly, because an overclaim here stops the next
sweep from looking.** Two *enumerated* classes have been swept: P95 took the
**enabled-interactive-control** class, P98 took an **eight-selector read-text set** (named in the
read-text bullet). The hue-as-text family was retro-fitted by P74. **The `--text-3` family is NOT
closed.** As of 2026-09-01 there are **122 `color: var(--text-3)` declarations across 33 files in
`src/styles/`**, and apart from the eight P98 swept and the sanctioned-decorative list below,
**none of them has ever been individually classified**. Known violations already sitting inside that
unaudited remainder: `.blame-date`, `.file-history-date` and `.reflog-date`
(`src/styles/blame-history.css`) are **timestamps on the `--bg-0` `.diff-overlay` surface at 3.67:1
dark / 3.17:1 light** — read text by this section's own rule, in a file neither sweep opened. The
full audit is **P101**; its classification method is specced in
`docs/contracts/P98-text3-readtext-ui.md` §8.8. **One known AA shortfall remains** — the unaudited
`--text-3` remainder. The accent-fill shortfall is **closed** (P100, 2026-09-01): the full survey
found seven text-bearing accent fills and fixed all seven; see the ACCENT FILL bullet for the two
recipes that replaced it. **New surfaces must not add to it**:
```

**Replace with (15 lines):**

```
**Scope of the `--text-3` work — the audit is complete, the fixes are not yet shipped.** Three
*enumerated* passes: P95 took the **enabled-interactive-control** class, P98 took an
**eight-selector read-text set**, and **P101 (2026-09-01) audited the family exhaustively** — all
**124 `color: var(--text-3)` declarations across 33 files in `src/styles/`** (re-pinned by grep at
audit time: 122 + the two disabled-hint overrides P98 itself added), plus the one declaration
outside `src/styles/`. Every declaration now carries a recorded bucket and verdict in
`docs/contracts/P101-text3-audit-ui.md` §3 — **93 fixes, 31 sanctioned exemptions** — and the
token's role is redefined by the SANCTIONED ROLES bullet below. P101 also found **three pre-existing
P95-class enabled-control escapes** (`.settings-reset`, `.settings-config-advanced-summary`,
`.onboarding-skip`), so P95's app-wide claim was as unevidenced as P98's: **an "app-wide swept"
claim without an enumeration behind it has now failed twice — do not make a third.** The
hue-as-text family was retro-fitted by P74; the accent-fill shortfall is **closed** (P100). **One
known AA shortfall remains:** the 93 P101 fixes, **enumerated but not yet implemented**. This
section may call the `--text-3` family **closed** only in the increment that lands that CSS — not
before. **New surfaces must not add to it**:
```

---

### Hunk A′ — the post-ship flip. Apply **only** in the commit that lands the last P101 CSS increment (P101c). Not before.

**Find (5 lines — the tail of Hunk A's replacement):**

```
hue-as-text family was retro-fitted by P74; the accent-fill shortfall is **closed** (P100). **One
known AA shortfall remains:** the 93 P101 fixes, **enumerated but not yet implemented**. This
section may call the `--text-3` family **closed** only in the increment that lands that CSS — not
before. **New surfaces must not add to it**:
```

**Replace with (5 lines):**

```
hue-as-text family was retro-fitted by P74; the accent-fill shortfall is **closed** (P100). All 93
P101 fixes **shipped** (P101a–c), so the **`--text-3` family is now CLOSED**, and — unlike the
retracted P95/P98 claims — the enumeration is behind it, declaration by declaration, in
`docs/contracts/P101-text3-audit-ui.md` §3. **No AA colour shortfall remains in §2. A new
`--text-3` use outside the four SANCTIONED ROLES below is a defect**:
```

*(Find is 4 lines + the trailing `:` line; match on the two sentences, not on the wrap.)*

---

### Hunk B — §2 token table, `--text-3` row. Replace **line 58**.

**Find:**

```
| `--text-3` | `#6b7280` | `#8a919e` | muted/labels — **see the contrast note below** |
```

**Replace with:**

```
| `--text-3` | `#6b7280` | `#8a919e` | **four exempt roles only** — never read text; see SANCTIONED ROLES below |
```

---

### Hunk C — §2 `--text-3` bullet. Replace **lines 88–98**.

**Find (verbatim, 11 lines):**

```
- `--text-3` is **3.68:1** on `--bg-0`, **3.38:1** on `--bg-1` and **2.98:1** on `--bg-2` (dark);
  **3.17:1**, **2.96:1** and **2.73:1** respectively (light). On `--selection` it is **2.33:1** /
  **2.55:1** (measured 2026-09-01, P98). Over its own 18% tint it is **2.78:1** / **2.51:1**. Every
  one of those is below the 4.5:1 text bar, and the `--bg-2`, `--selection` and own-tint cases are
  below even the **3:1** graphics bar. Treat `--text-3` as
  **decorative only** (uppercase section labels that duplicate visible structure, dividers, disabled
  glyphs). Any text the user must actually read — metadata, timestamps, costs, log lines, hints,
  **status-pill labels**, **settings help text**, **any heading that is the user's only wayfinder**
  (§12.5's result-group headers) — uses `--text-2` (**7.90:1** dark / **7.99:1** light on `--bg-0`;
  **7.25:1** / **7.45:1** on `--bg-1`; **6.40:1** / **6.87:1** on `--bg-2`; **5.01:1** / **6.42:1**
  on `--selection`).
```

**Replace with (32 lines):**

```
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
```

---

### Hunk D — §2 sanctioned-decorative table. Replace **lines 118–122**.

**Find (verbatim, 5 lines):**

```
    | Use | Where | Measured | Why decorative |
    |---|---|---|---|
    | Editor line-number gutter | `src/components/conflictCmSetup.ts:36`, `.cm-gutters` | 3.68 / 3.17 on `--bg-0` | a coordinate that duplicates visible structure; universal editor convention (a `--text-2` gutter would be louder than any editor the user knows). The act-carrying text in that pane — `.conflict-region-caption`, `.conflict-editor-split-label`, the accept/reject buttons — is already `--text-2` or brighter. **Revisit trigger:** if a line number ever becomes actionable or is named in copy (go-to-line, line-range selection, any message citing a line), it becomes read text and moves to `--text-2`. |
    | The disabled option set | `dialogs-forms.css` / `search.css`: `.combobox-option--disabled`(`.combobox-option--active`), `.command-palette-option.is-disabled`(`.is-active`) | 3.38 / 2.96 on `--bg-1` | dimming *is* the disabled signal, carried independently by `disabled` / `aria-disabled` + `cursor: default` |
    | The P95 §3.3 exempt set | `docs/contracts/P95-a11y-ui.md` §3.3 | — | as recorded there |
```

**Replace with (10 lines):**

```
    | Use | Where | Measured | Why decorative |
    |---|---|---|---|
    | Editor line-number gutter | `src/components/conflictCmSetup.ts:36`, `.cm-gutters` | 3.68 / 3.17 on `--bg-0` | a coordinate that duplicates visible structure; universal editor convention (a `--text-2` gutter would be louder than any editor the user knows). The act-carrying text in that pane — `.conflict-region-caption`, `.conflict-editor-split-label`, the accept/reject buttons — is already `--text-2` or brighter. **Revisit trigger:** if a line number ever becomes actionable or is named in copy (go-to-line, line-range selection, any message citing a line), it becomes read text and moves to `--text-2`. Re-checked at P101 — no go-to-line control, no line-range UI, no copy citing a line: **sanction stands**. |
    | Diff gutter line numbers | `diff-content.css:144`, `.diff-lineno` | 3.68 / 3.17 on `--bg-0` | same ruling and same revisit trigger as `.cm-gutters` (P101). **Contrast the blame gutter:** `blame-history.css:112` `.blame-lineno` was ruled the *other* way and moved to `--text-2`, purely because its gutter goes to `--bg-2` on hover (2.98 / 2.73). This is a coordinate-glyph-**on-`--bg-0`** exemption, not a blanket line-number exemption. |
    | Search input glyph | `search.css:28`, `.commit-search-icon` | 3.68 / 3.17 on `--bg-0` | duplicates the visible input and its placeholder (P101) |
    | The disabled set (16 declarations — this list is the whole of them, P101) | `dialogs-forms.css` `.combobox-option--disabled`(`.combobox-option--active`)(` .combobox-option-hint`); `search.css` `.command-palette-option.is-disabled`(`.is-active`)(` .command-palette-option-hint`); `.btn-icon:disabled`, `.toolbar-btn:disabled` (`controls.css`); `.context-menu-item:disabled`(` .context-menu-subnote`) (`context-menu.css`); `.row-action:disabled`, `.commit-msg-tool:disabled` (`commit-box.css`); `.git-dock-clear:disabled`; `.sidebar-add:disabled`; `.tab-add:disabled`; `.settings-account-{kind,default} input:disabled + span` | 3.38 / 2.96 on `--bg-1`; 2.98 / 2.73 on `--bg-2` | dimming *is* the disabled signal, carried independently by `disabled` / `aria-disabled` + `cursor: default`. **Note the child-rule pairs** — a disabled row's hint/subnote needs its own restated dim (see the child-rule trap below); P98 and P101 each had to add one |
    | `::placeholder` and empty-state copy (10) | `commit-box.css:318`, `sidebar.css:251/288`, `search.css:183/278`, `composer.css:151`, `agent-assets.css:13/162`, `image-diff.css:76`, `diff-content.css:225` | 2.98–3.68 / 2.73–3.17 | the surface itself is the message. **Does not extend to an empty state that names the fix** — that sentence is `--text-2` (§8). P101 recorded the placeholder half as arguable under WCAG 1.4.3 and recommended a **visible label** over a brighter placeholder if it is ever revisited |
    | Settings group / section titles (3) | `settings-shell.css:412`, `settings-legacy-sections.css:27/124` | 3.68 / 3.17 on `--bg-0` | an uppercase heading over a *visibly bounded* group. **Does not extend to a header that is the only wayfinder** — a result-group header, a split-pane label, a command-palette group header, a checkbox-group header: all `--text-2`. A decorative separator inside a string whose other half moved to `--text-2` also moves, **for cohesion, not contrast** (`.reflog-oid-arrow`, P101 §6.5) — that is not a precedent that glyphs need 4.5:1 |
    | The P95 §3.3 exempt set | `docs/contracts/P95-a11y-ui.md` §3.3 | — | as recorded there, **minus** the three escapes P101 found (`.settings-reset`, `.settings-config-advanced-summary`, `.onboarding-skip`), which are enabled controls and move to `--text-2` |
```

---

### Hunk E — §2 accent-on-`--selection` bullet. Replace **lines 191–198**. **This is the wrong-figure correction handed over by the P100 review.**

**Find (verbatim, 8 lines):**

```
- **`--accent` as *text* never sits on a `--selection` fill (added 2026-08-20, P69l).**
  `color: var(--accent)` is a house-wide pattern (~30 call sites) and is fine on `--bg-0` / `--bg-1`
  / `--bg-2`, but over `--selection` it measures **2.6:1** dark / **3.6:1** light (independently
  re-measured at **3.51** / **3.74** in the P69k pass — both readings fail the 4.5:1 text bar in both
  themes). So: accent-coloured *text* inside a row that can become selected is a latent defect, and
  `--accent` may never be chosen as the "emphasised" colour for a value inside a selected row — use
  `--text-1`. `--accent` as a **border, bar or glyph** on `--selection` remains fine as decorative
  delineation that carries no meaning (the settings rail's inset bar, §12.1).
```

**Replace with (12 lines):**

```
- **`--accent` as *text* never sits on a `--selection` fill (added 2026-08-20, P69l; figures
  corrected 2026-09-01, P101).** `color: var(--accent)` is a house-wide pattern (~30 call sites) and
  is fine on `--bg-0` / `--bg-1` / `--bg-2`, but over `--selection` it measures **3.51:1** dark /
  **3.74:1** light. **These are the authoritative figures**, re-derived by P101 from the shipped
  hexes (`--accent` `#4f8cff` / `#2f6fe4` over `--selection` `#2a3b57` / `#dbe7ff`); the older
  "**2.6:1** / **3.6:1**" reading that stood here and below was wrong and is retired. Both fail the
  4.5:1 **text** bar, so accent-coloured *text* inside a row that can become selected is a latent
  defect and `--accent` may never be the "emphasised" colour for a value inside a selected row — use
  `--text-1`. But 3.51 / 3.74 **clears the 3:1 graphics bar in both themes**, so `--accent` as a
  **border, bar or glyph** on `--selection` is a **compliant non-text carrier** — the settings
  rail's inset bar (§12.1) and the `inset 2px 0 0 var(--accent)` leading bar on every selected list
  row. It is load-bearing and it passes; it is not "decorative delineation carrying no meaning".
```

---

### Hunk F — the duplicate wrong figure. Replace **lines 216–217**.

**Find (verbatim, 2 lines):**

```
**13.3:1**; `--accent` 1px border on `--bg-2` **4.4:1** / **4.1:1**. `--accent` on `--selection` is
**2.6:1** / **3.6:1** — decorative delineation only, never a meaning carrier. And (P69c pass)
```

**Replace with (3 lines):**

```
**13.3:1**; `--accent` 1px border on `--bg-2` **4.4:1** / **4.1:1**. `--accent` on `--selection` is
**3.51:1** / **3.74:1** (corrected P101 — the "2.6 / 3.6" that stood here was wrong): it clears the
3:1 graphics bar in both themes, so it is a valid non-text carrier, but never text. And (P69c pass)
```

---

## 8. Acceptance criteria

**AI gate** = verifiable by the orchestrator (grep, `javascript_tool` computed-style probe in the
mock harness, `pnpm gate`). **USER CHECKPOINT** = human perception, or a surface the harness cannot
reach.

| # | Criterion | Gate |
|---|---|---|
| **AC1** | The 93 `color:` swaps in §4.1 are applied — exactly those lines, exactly `var(--text-3)` → `var(--text-2)`, across 30 files — plus F-1. Post-implementation, `rg -c 'color: var\(--text-3\)' src/styles` returns **32 matches across 14 files**: the **31** surviving originals enumerated in §4.1 **plus the one declaration the §4.2 rule adds**. A result of **31** means §4.2 is missing; **>32** means a swap was skipped. | **AI gate** (grep) |
| **AC2** | Those 31 surviving originals are *exactly* the §2.4 exempt set — 16 `disabled` + 10 `placeholder-empty` + 3 `label-duplicating-structure` + 2 `decorative-glyph` — verified as an enumerated diff against §3, file:line by file:line, **not** as a bare count. | **AI gate** (grep) |
| **AC3** | `src/styles/onboarding.css:31` declares `background: var(--text-2)` (F-1), and `identity-menu.css:31` declares `border-color: var(--text-2)`. | **AI gate** (grep) |
| **AC4** | Within the changed files, **every** changed line is a `color:` / `background:` / `border-color:` **value**, plus the one new rule in §4.2. No size, weight, style, `text-transform`, `letter-spacing`, spacing, padding, margin, border-width, radius, other-`background`, transition, hover or focus declaration changed. No hardcoded hex, `rgb()` or `rgba()` introduced anywhere. | **AI gate** (diff read + grep) |
| **AC5** | `src/styles/context-menu.css` contains the §4.2 rule `.context-menu-item:disabled .context-menu-subnote { color: var(--text-3) }` with its comment, placed after the `.context-menu-item:disabled` block. **No** override was added to `settings-legacy-sections.css`. | **AI gate** (grep) |
| **AC6** | **Child-rule regression check.** A **disabled** context-menu item renders label *and* subnote at the computed `--text-3` value, in both themes, in both the plain-disabled and `:focus-visible` states; and the six P98 disabled option/hint declarations still compute `--text-3`. **If no fixture route mounts a disabled context-menu item carrying a subnote**, verify at **rule level in the live CSSOM** and report it as rule-level, not composited — the P98 AC19 precedent. Do **not** inject synthetic DOM. | **AI gate** (harness, degrading to CSSOM) |
| **AC7** | For every swapped declaration reachable in the harness, `getComputedStyle` on a **mounted** instance returns a `color` equal to the computed `--text-2` value in **both** `data-theme` states. Every selector not reachable is listed as **not reachable**, not reported as measured. | **AI gate** (harness) |
| **AC8** | **The `--selection` worst cases are re-measured composited**: `.search-result-oid` and `.search-result-date` on `.search-result.is-current`, and `.context-menu-chevron` / `.context-menu-subnote` on `.context-menu-item:focus-visible`. Each ≥ **5.01:1** dark / **6.42:1** light (±0.05). `.file-chevron`'s `.file-row-expanded:hover` half needs a real pointer → **USER CHECKPOINT**. | **AI gate** (+ UC for the hover half) |
| **AC9** | `ui-reference.md` carries hunks **A–F**: **1353 ±3 lines**, 13 `##` sections, unchanged tail sentinel; the `--bg-3` row present in the §2 matrix; `--accent` on `--selection` reads **3.51 / 3.74** in **both** places, with `"2.6:1"`, `"3.6:1"` and `"122"` appearing **nowhere** in §2; §2 states the audit **complete** and the fixes **enumerated but not yet implemented**. | **AI gate** (file read + grep) |
| **AC9′** | **Only in the increment that lands the last CSS (P101c):** Hunk **A′** applied; §2 states the `--text-3` family **CLOSED** with the enumeration cited; line count still **1353 ±3**. AC9′ **must not** be met before AC1. | **AI gate** (file read) |
| **AC10** | `pnpm gate` tiers green. No snapshot or visual test asserts a `--text-3` computed colour on any swapped declaration; any that does is updated in the same increment. | **AI gate** |
| **AC11** | Long-content behaviour unchanged: a 60-char branch name in `.branch-muted`, a deep path in `.file-dir`, a long PR title beside `.pr-row-meta`, and a 12-word `.op-rationale` still truncate / clip / wrap exactly as today. No layout property moved, so this is a regression check, not a change. | **AI gate** (harness, pathological fixture) |
| **AC12** | **Hierarchy.** Every swapped element still reads as clearly subordinate to its primary, in both themes and at both `panelDensity` settings. Specifically: the right panel's `.section-label`, `.file-dir` under `.file-name`, `.commit-date` under the commit summary, `.branch-badge` beside a branch name, `.command-palette-group` above its options, and the three §4.3 no-colour-step cases. | **USER CHECKPOINT** |
| **AC13** | **Density.** The app does not read as flatter or noisier after 93 metadata strings brighten — the §6.3 stated cost. Both themes, `cozy` and `compact`. | **USER CHECKPOINT** |
| **AC14** | The 15 `forge-pr.css` declarations render correctly on a **real forge connection** (PR list, PR detail, comments, the create form, the account menu) in both themes. Not harness-reachable; **no token was entered during the audit.** | **USER CHECKPOINT** |
| **AC15** | `.settings-account-dot`, `.checks-glyph--neutral` and the `.identity-avatar` loading ring remain distinguishable from their hue siblings **without relying on colour** after the swap (F-3). | **USER CHECKPOINT** |
| **AC16** | `.rebase-plan-commit.dropped` still reads unambiguously as dropped (the `line-through` is now the sole carrier), and `.blame-lineno` at `--text-2` does not read as louder than the code beside it. | **USER CHECKPOINT** |

**AI gate:** AC1–AC11, AC9′. **USER CHECKPOINT:** AC12–AC16, plus AC8's hover half.

### 8.1 Harness fixture states needed

**No new fixtures are requested.** The states P101 verification uses, all already shipped by P100
(route details per the orchestrator's brief, not re-verified in this pass — §2.5):

| Route | Unlocks |
|---|---|
| `?op=merge` → right panel → context menu on a file row | AC5 / AC6 — disabled menu item + `.context-menu-subnote` |
| `?op=merge` → command palette (`Ctrl/Cmd+K`) — note the brief's warning that the palette input can freeze after synthetic mouse events; reload rather than fight it | `.command-palette-group`, and the six exempt palette declarations |
| default mock state → sidebar, right panel, header | `.branch-*`, `.file-dir`, `.tree-dir-name`, `.section-label`, `.repo-path`, `.repo-switcher-*` |
| Reflog / Blame / File history from the workspace toolbar | all six `blame-history.css` declarations (P98 measured four mounted here) |
| Commit search (`Ctrl/Cmd+F`) with a matching query | `.search-result-oid`, `.search-result-date`, and their `.is-current` `--selection` state — **the AC8 case** |
| Settings (`Ctrl/Cmd+,`) | `.settings-*`, `.settings-reset`, `.settings-config-advanced-summary` |
| Header robot icon → AI Assets → Worktrees | `.asset-row-path`, `.stale-*`, `.asset-compare-*` |

**Before any measurement:** `resize_window` 1440×900 (the hidden pane reports
`innerWidth/innerHeight = 0`, so every `vh`/`vw` rule evaluates to 0), and toggle themes by setting
`data-theme` on `<html>` — `resize_window`'s `colorScheme` does **not** theme this app.

**Pathological long-content case for AC11:** the existing fixture's long branch names and deep paths
should suffice; if none exceeds 60 chars, extend an existing fixture rather than adding a new one.

---

## 9. What remains unverified, and why

Stated plainly, because §8.8 step 4's whole point is that an unrecorded item is not a pass.

1. **No composited pixel measurement was taken in this pass.** Every verdict is source-derived per
   §2.5; the only mounted figures cited are the four `blame-history.css` reflog items **carried over
   from P98 §8.1.1**. **AC7 / AC8 close this at implementation time**, per declaration. The
   verdicts do not depend on it (backdrop identity comes from an exhaustive source sweep; ratios come
   from measured tokens), but the *shipped cascade* does need checking, and that check is deferred,
   not done.
2. **`forge-pr.css`'s 15 declarations (12% of the audit) are structurally unverifiable** in the
   harness: the PR panel is behind a forge-token screen and no token was entered. Their buckets are
   confident — they are dates, oids, counts, locations, field labels, notes and a caret, with no
   borderline calls — but AC14 is a USER CHECKPOINT and cannot be discharged by me.
3. **`.cm-gutters` was not reachable.** Per the brief, the `?op=merge` conflicted route mounts a
   read-only view whose only control is the close `×`, so neither `.conflict-editor-mode-btn` nor the
   CodeMirror gutter mounts. §5.3's re-evaluation of the sanction is source + convention reasoning.
   Reported as a **harness map hole**, not worked around with synthetic DOM.
4. **Canvas-driven selection remains unreachable** (P98 §8.1.2). No P101 declaration depends on it,
   but `.file-chevron`'s `--selection` state is reached only via a real hover on an expanded row —
   hence AC8's split.
5. **Hover states cannot be verified in the harness at all** without a real pointer. 41 of the 124
   have a distinct hover backdrop; for those, the *idle* figure is verifiable and the *hover* figure
   is arithmetic from the same token table. This is the single largest structural verification gap in
   the `--text-3` programme and no fixture can close it.
6. **`requestAnimationFrame` is paused and `setTimeout` throttled to ~1s in the hidden preview**, so
   nothing about frame timing or scroll feel is claimed. P101 changes no motion and no geometry, so
   there is nothing here to time — recorded so the absence is deliberate rather than overlooked.
7. **F-3 (`.settings-account-dot`'s colour-only state)** could not be confirmed: the accounts row
   needs a configured forge. Filed as a follow-up rather than guessed at.
8. **§6.3 (swap vs retune) and §6.4 (the four placeholders) are open calls for the orchestrator.**
   My recommendations are recorded with the numbers behind them; neither is decided here.
9. **`ui-reference.md`'s line counts in §7 are my own count of the blocks I wrote**, not a read of
   the file after editing. AC9's `1353 ±3` plus the four robust invariants (13 `##` sections, tail
   sentinel, zero `"2.6:1"` / `"122"` in §2, `--bg-3` present) are what should actually be checked;
   the number alone is the weaker signal.
