# P69c follow-up — Draft divergence feedback on numeric settings (UI contract)

> **This file is §13 of `docs/contracts/P69-settings-ui.md`.** It is held in a sibling file rather
> than appended, because the `ui-designer` toolset has only a whole-file `Write` — appending to the
> 1013-line parent would have meant re-emitting it in full, which the task forbade, and
> `ui-reference.md` was under concurrent edit by the P73 pass on the same day. §14 below is a
> ready-to-paste block for `ui-reference.md` §12.3.3; whoever next edits those two files with a
> line-level tool should fold this in verbatim and replace this file with a one-line pointer.
>
> Scope: the missing visual feedback for the P69c draft-display fix. **The input model is settled**
> — draft display + clamped commit per keystroke stays exactly as committed
> (`src/components/NumberSlider.tsx`, P69-settings-shell.md OQ-1). Nothing here reverts to snap-back
> or to commit-on-blur.
>
> Reads: `ui-reference.md` §2 (tokens/contrast), §12.2 (row anatomy), §12.3; `P69-settings-ui.md`
> §5.1, §5.7; `P68g-ui.md` §1.6 (`describedBy`), §7-N1 (the `.settings-number:focus` gap).

---

## 13.1 The problem, stated exactly

Since P69c the number input shows a **draft string** while it is focused, and the setting holds the
**clamped, rounded** commit of that string. Four drafts diverge from the setting:

| Draft class | Example (Row height, 24–48 px, currently 32) | Setting holds | Detect |
|---|---|---|---|
| **above** | `128` | `48` | numeric, `n > max` |
| **below** | `6` | `24` | numeric, `n < min` |
| **rounded** | `24.7` | `25` | numeric, in range, `Math.round(n) !== n` |
| **blank** | `` (cleared) | `32` — unchanged, by design (`P68g §6.1` acceptance 5, pinned by test) | `raw.trim() === ''` |
| **not a number** | `-`, `e`, `.` | `32` — unchanged | `Number.isNaN(Number(raw))` |

Today all five render identically to a valid draft: default border, no text, nothing announced. The
field silently contradicts the setting until blur. That is the gap.

**Not a divergence, and must not warn:** `06` (Number 6 === value 6), `6e1` (=== 60), `32.0`, or any
draft whose numeric value already equals the committed value. Compare **numerically**, never by
string, or leading zeros produce a hint on every normal edit.

**Canonical predicate.** One derived value drives border, hint, `aria-invalid`, and the timer:

```
kind = null | 'above' | 'below' | 'rounded' | 'blank'    // 'blank' also covers not-a-number
```

`null` whenever `draft === null` (not editing), or the draft is numeric and `Number(draft) === value`.

---

## 13.2 The affordance

**Chosen: a `--warning` inner ring on the number input + one line of plain text in the row's help
slot.** The two are a single state — neither ever renders without the other, so colour is never the
carrier (`ui-reference.md` house rule; the A/M/D/U/R badges are the precedent). The sentence is the
non-colour carrier and it is strictly more informative than a glyph would be.

**Rejected alternatives.** A red/`--danger` treatment: nothing is broken and nothing will be lost —
the setting is already at a legal value — so `--danger` overstates it and would collide with the
error language used for failed operations. An icon-only marker (`!` in the field or beside it): a
56px-wide input has no room, and a bare glyph cannot say *what the value will become*, which is the
whole point. A tooltip: invisible to keyboard users mid-typing, and hover is the wrong trigger for a
typing state. No extra glyph is added to the hint line — a warning triangle in front of a 12px
sentence under a 56px input is noise, and the border already carries the "look here".

### 13.2.1 Geometry

Target markup is the P69g row anatomy (`ui-reference.md` §12.2); the interim markup is today's
`.settings-control`. Both place the hint on its own full-width line under the label/control line.

```
P69g target (.settings-row grid, 1fr auto 24px):
┌───────────────────────────────────────────────────────────────────────┐
│ Row height                          [====|=====] [ 128 ] px      ↺    │  ← 44px min
│ Height of one commit row in the graph.                                │  ← help slot, 18px reserved
└───────────────────────────────────────────────────────────────────────┘
                       … while `128` is being typed, the same 18px slot reads:
│ Too high — will be set to 48 px.                                      │
```

| Property | Value |
|---|---|
| Hint element | `<p class="settings-draft-hint" id="{id}-draft" aria-live="polite">`, **always rendered**, empty when `kind === null` |
| Type | 12px / `line-height: 16px`, `color: var(--text-2)`, `margin: 2px 0 0` — identical to settings help text (`§5.1`), deliberately |
| Slot | The row's help cell (grid row 2, column 1). `min-height: 18px` on that cell for every `NumberSlider` row, so nothing below the row ever moves while typing |
| Help text | `display: none` while `kind !== null`; restored when it clears. One line occupies the slot at a time |
| Interim (`.settings-control`, `styles.css:995` — `display:flex`) | Add modifier `.settings-control--hinted { flex-wrap: wrap; padding-bottom: 18px; position: relative; }` and `.settings-draft-hint { position: absolute; left: 0; bottom: 0; }`. Label + inputs keep their current flex line untouched |
| Input ring | `.settings-number[aria-invalid='true'] { border-color: var(--warning); box-shadow: inset 0 0 0 1px var(--warning); }` — a 2px-reading ring with **no box-model change**, so the 56×28 field does not resize |
| Ordering | That rule must be declared **after** `.settings-number:focus` (equal specificity, source order decides). The field is always focused when a draft exists, so the warning must beat the focus border recolour — see §13.7 N1 |
| Wrapping | Hint strings are bounded (longest = `Too high — will be set to 86400 seconds.`, 40 chars) and always fit one line in the 56ch help slot. No wrap or ellipsis handling needed |

### 13.2.2 Contrast — both themes

| Pair | Dark | Light | Bar |
|---|---|---|---|
| `--warning` ring vs `--bg-2` (input fill) | **6.4:1** | **4.2:1** | ≥3:1 graphics ✔ |
| `--warning` ring vs `--bg-0` (pane) | **7.9:1** | **4.9:1** | ≥3:1 ✔ |
| Hint `--text-2` on `--bg-0` | **7.9:1** | **4.9:1** | ≥4.5:1 text ✔ |

Measured 2026-08-19 for this pass; the first row is new and belongs in `ui-reference.md` §2. Note
the hint is **not** `--warning` text: §2 bars hue-as-text over its own tint, and `--text-2` is what
every other settings hint uses — consistency beats a second signal.

---

## 13.3 Timing — the crux

> **One-sentence rule: an above-maximum draft warns on the keystroke that causes it; every other
> divergence warns only after 600 ms with no further keystroke in the field.**

| Kind | Delay | Why |
|---|---|---|
| `above` | **0 ms — immediate** | For a non-negative integer field, appending a digit is monotonically increasing. Once the draft exceeds `max`, **no continuation of typing can make it valid** — only deleting or replacing can, and both clear the state at once. It is a terminal error, not a transient one, so there is nothing to wait for and waiting only delays the truth. |
| `below` | **600 ms idle** | Every valid entry passes through below-minimum prefixes: with `min = 24`, `30` passes through `3`; with `min = 60`, `600` passes through `6` and `60`. Warning on those punishes ordinary typing — exactly the failure this spec must not create. |
| `rounded` | **600 ms idle** | `24.` is a normal step toward `24.7`, and `24.7` is a normal step toward `24.75`… the user is mid-number. |
| `blank` | **600 ms idle** | Select-all-then-type passes through empty on nearly every re-entry. An instant "Empty" on every edit would be the loudest possible false positive. |

**Why 600 ms.** Comfortable numeric typing runs 150–250 ms per digit; 600 ms is >2× that, so
uninterrupted entry never trips it, while a user who has stopped sees the hint ~0.6 s later — well
inside the ~1 s+ it takes to look away. It is also twice the app's established 300 ms `notify`
debounce, so it reads as the same family of "the user has stopped" delay. No other value is
load-bearing; if a later pass wants to tune it, it is one constant.

**Timer rules.**

1. The timer is **re-armed on every keystroke** in the number input (`onChange`), and only there.
2. It is **cleared** on blur, on `Enter`, on the draft becoming non-divergent, on the range input
   being used, on the external-value resync (`NumberSlider` rule (c)), and on unmount.
3. **Once visible, it stays visible** while any divergence holds, and changes text immediately if
   the kind changes (`below` → `above` when a digit is appended). The delay is armed once per entry
   into the warning state, **never re-armed while already showing** — a hint that flickered off and
   on per keystroke would be worse than no hint.
4. Going from divergent to valid **hides instantly** (no delay, no fade-out). A stale warning is a
   lie; hiding is never the annoying direction.
5. `above` fires immediately even if a `below`/`blank` timer is pending: the pending timer is
   cleared and the hint shows.
6. Blur/Enter drop the draft (existing P69c behaviour) and therefore the hint, in the same render.
   **The affordance can never outlive the draft it describes** — this is why no "stranded warning"
   state exists and why nothing needs to be reconciled on close.

### 13.3.1 Motion

`transition: opacity 120ms ease-out` on the hint and `border-color 120ms ease-out` on the input, on
appear only; hide is instant. Opacity + a 1px border colour on a 56px box — no layout, no transform,
nothing that can contend with the canvas render budget. Both go to `none` inside the existing
`prefers-reduced-motion` block (`ui-reference.md` §9); the hint then appears instantly, which is
correct, not degraded.

---

## 13.4 Microcopy — implement verbatim

`unitSuffix = unit ? ' ' + unit : ''`. `value` is the prop, i.e. exactly what the field will show
when the user leaves it.

| Kind | String | Example (Row height, 24–48 px, at 32) |
|---|---|---|
| `above` | `Too high — will be set to {max}{unitSuffix}.` | `Too high — will be set to 48 px.` |
| `below` | `Too low — will be set to {min}{unitSuffix}.` | `Too low — will be set to 24 px.` |
| `rounded` | `Whole numbers only — will be set to {value}{unitSuffix}.` | `Whole numbers only — will be set to 25 px.` |
| `blank` (empty **or** not a number) | `No value — stays at {value}{unitSuffix}.` | `No value — stays at 32 px.` |

Each string says what is wrong *and* what the value will be, in that order, in ≤40 characters.
Sentence case, em-dash, no jargon, no libgit2 or DOM vocabulary, and no subject — naming the setting
would produce `Fetch every will be 300 seconds.`, which is worse than saying nothing. The row label
sits 16px above the sentence and is the control's accessible name, so context is never missing.

Empty and not-a-number share one string deliberately: to the user both are "I have not typed a
number yet", and `type="number"` blanks most non-numeric input anyway, so the distinction is
invisible far more often than it is real.

---

## 13.5 States — both themes, both densities

The Settings surface has **one geometry in `cozy` and `compact`** (`ui-reference.md` §3, D10). Every
row below is identical in both densities: hint 12px/16px, slot 18px, input 56×28, ring 1px inset.

| State | Number input | Hint slot | `aria-invalid` |
|---|---|---|---|
| Default (no draft) | `--border` 1px, `--bg-2` fill | help text, `--text-2` | absent |
| Focused, draft valid | current focus treatment (see N1) | help text | absent |
| Focused, draft divergent, timer pending | unchanged from focused | help text (unchanged) | absent |
| Focused, draft divergent, showing | `--warning` border + inset ring; **focus outline unchanged** | hint sentence, help hidden | `true` |
| Hover | hover border, if any | unchanged | — |
| Hover **+** showing | warning ring **wins** over hover and over the focus border recolour | hint | `true` |
| Disabled | wrapper `.is-disabled` `opacity: .5`, inputs disabled | help text | absent |
| Blank draft, showing | warning ring; the field itself is visibly empty — the empty field is its own second carrier | `No value — stays at 32 px.` | `true` |
| Long draft (`999999999999`) | value scrolls inside the 56px field (native `type=number`); ring unchanged | `Too high — will be set to 48 px.` | `true` |

**Disabled transition.** If `disabled` flips true while a hint is showing (the AI fieldset gate,
`P69 §5.4`), the draft, the timer, and the hint are all dropped in that render. A disabled control
must never display a warning the user cannot act on.

**Theme parity.** Identical treatment in dark and light; only the two token values change. The ring
clears 3:1 in both (§13.2.2) and the hint clears 4.5:1 in both, so neither theme relies on the other
being the "real" one.

---

## 13.6 Accessibility

| Concern | Spec |
|---|---|
| Announcement | The hint `<p>` is **permanently in the DOM** with `aria-live="polite"` and empty text when idle, so the live region is registered before content ever arrives. Never `display:none` the hint element — hide the **help** paragraph instead. |
| Why a live region is safe here | It only ever fires when the user has stopped typing for 600 ms, or on the single terminal above-max keystroke. Rule 3 (no re-arm while showing) means the text does not change per keystroke, so there is no repeat chatter. |
| `aria-invalid` | `true` exactly when the hint is showing; **removed**, not `"false"`, otherwise. It is the single flag driving CSS, so the visual and the semantic can never disagree. |
| `aria-describedby` | **Composes, never clobbers**: `[describedBy, `${id}-draft`].filter(Boolean).join(' ')`, help id first so the explanation is announced before the warning. The existing `describedBy` prop (`P68g §1.6`) keeps working unchanged, including when it carries several ids. When the help paragraph is hidden it drops out of the accessible description on its own — no conditional id juggling. |
| Frozen surface | Input `id` (`#settings-graph-row`) and the range's `aria-label={label}` (`Row height`) are **untouched**. No role changes. `getByLabelText('Row height')` and every existing query still resolve. |
| Hit targets | Nothing new is interactive. The hint is text; it is not focusable, not clickable, and adds no tab stop. |
| Focus ring | The warning uses `border-color` + `inset box-shadow` only. The `:focus-visible` **outline** (2px `--accent`, 1px offset) is never overridden, so a keyboard user keeps the ring at all times. |
| Colour alone | Never. Ring and sentence are one state; the blank case additionally shows a visibly empty field. |
| Command palette | No entry. This is feedback, not an action. |

---

## 13.7 Component decomposition

| File | New/changed | Contents | Est. |
|---|---|---|---|
| `src/settings/draftHint.ts` | **new** | Pure, no React: `classifyDraft({draft, value, min, max, integer})` → `kind`, and `draftHintText(kind, {min, max, value, unit})` → the §13.4 string. Directly unit-testable, and the strings live in one place. | ~55 |
| `src/settings/useDraftHint.ts` | **new** | The hook: owns the 600 ms timer, the "already showing" latch, and cleanup. Returns `{ kind, text, ariaInvalid, describedBy, onDraftInput(raw), clear() }`. | ~70 |
| `src/components/NumberSlider.tsx` | changed | Calls the hook from the existing `onChange`/`onBlur`/`onKeyDown`/resync paths, spreads `aria-invalid` + composed `aria-describedby`, renders the hint `<p>`. **+~20 lines → ~150**, well under the limit; no split needed. | 126 → ~150 |
| `src/components/SettingsAiLimits.tsx` | changed | The hand-rolled USD field calls the **same** hook — see §13.8. | +~12 |
| `src/styles.css` | changed | `.settings-draft-hint`, `.settings-number[aria-invalid='true']`, `.settings-control--hinted`, the reduced-motion additions. **P73 currently holds this file — sequence behind it.** | +~25 |

`src/settings/` already exists (`ranges.ts`), so no new directory. The classifier is deliberately
outside `src/components/` so a non-component (`SettingsAiLimits`'s USD input) can import it without
depending on a component module.

**N1 — fix while you are here.** `.settings-number:focus` currently only recolours its border
(`P68g §7-N1`, `styles.css:1030-1033`). Since the field is *always* focused when a draft exists, that
focus border and this warning border compete for the same 1px and the order of two equal-specificity
rules becomes load-bearing. Replace it with a real `:focus-visible` outline (2px `--accent`,
offset 1px, per §2) and the conflict disappears structurally. This is a P69g item, not a blocker.

---

## 13.8 Scope

Applies to **every settings slider**, with no per-call-site opt-out: Commit graph → Geometry (commit
node size, row height, lane width), General → Background activity (`Fetch every`, `Refresh every`),
AI → Runs → Limits (idle timeout, hard cap, replies per run), AI → Runs → Bulk resolve (batch size).
Eight controls, one behaviour.

**`SettingsAiLimits`'s USD field: yes, the same treatment.** It hand-rolls its own draft today
(`P68g §1.3` control 7 — `NumberSlider` rounds to integers, so USD could not use it) and is
scheduled to fold onto `NumberSlider` later. It has the same divergence, so it gets the same hook
now; folding it later then removes code instead of needing a second pass. Two adjustments:

- Its bounds are 0.5–100 USD, so `above`/`below`/`blank` apply verbatim
  (`Too high — will be set to 100 USD.`).
- The `rounded` kind is **not** enabled for it: it is a `step 0.5` decimal field, not an integer
  field, so `integer: false` is passed and fractional drafts are legal. Do **not** invent a
  step-rounding message — if the control ever starts snapping to the step, that is a separate spec.

**Not in scope:** the range inputs (they cannot produce an invalid value), text fields, path fields,
and the Git-config identity fields. Free-text validation is a different problem with a different
answer, and P69 has no requirement for it.

---

## 13.9 Harness verification

`pnpm dev` with `VITE_MOCK_IPC=1`. **No new fixtures are required** — the affordance is produced by
typing, so the default fixture reaches it. The harness is headless: `requestAnimationFrame` is
paused and `computer{screenshot}` **fails outright**, but `setTimeout` runs normally, so the 600 ms
timer is genuinely exercisable.

| Check | How | Verdict |
|---|---|---|
| `above` is immediate | Settings → Commit graph, click `#settings-graph-row`, type `128`, read `#settings-graph-row-draft` at once | AI-verifiable |
| `below` waits | Clear the field, type `6`, read immediately (expect empty), `computer{wait 1}`, read again (expect `Too low — will be set to 24 px.`) | AI-verifiable |
| Normal typing never warns | Select-all, type `30` at speed, read immediately — hint must be empty and `aria-invalid` absent | AI-verifiable |
| `blank` | Select-all, Delete, wait 1s → `No value — stays at 32 px.`, and the setting is unchanged | AI-verifiable |
| Blur clears everything | Tab away → hint empty, `aria-invalid` gone, field shows the clamped value | AI-verifiable |
| `aria-describedby` composes | Read the attribute on a slider that already has help text: expect `"{help-id} {id}-draft"`, both ids resolving | AI-verifiable |
| AI limit sliders | Seed `localStorage bonsai.mockUiSettings` with `aiConsented: true` and reload, otherwise the ten run knobs are disabled and cannot hold a draft | AI-verifiable |
| Light theme | `resize_window` `colorScheme: 'light'`, batched `javascript_tool` read of the computed `border-color` on the invalid input | AI-verifiable |

**USER CHECKPOINT (not AI-verifiable).**

1. **Whether 600 ms feels right** — the only judgement that matters here cannot be made from a DOM
   read. Type a two-digit value normally in `pnpm tauri dev` and confirm the hint never appears
   mid-entry; then stop mid-number and confirm it arrives before you would have looked away.
2. Any visual proof at all — ring weight, whether `--warning` reads as "check this" rather than
   "error", and both themes. Screenshots fail in this harness.
3. The 120 ms fade (rAF paused ⇒ not observable headlessly).
4. Native IME / numeric-keypad entry on macOS and Windows: confirm composition does not fire a
   spurious blank draft.

---

## 14. Paste block for `ui-reference.md` §12.3.3

> Insert immediately after §12.3.2 Segmented, before §12.4. Also add to §2's measured-pairs list:
> `--warning` 1px ring on `--bg-2`: **6.4:1** dark / **4.2:1** light (2026-08-19, P69c pass).

```markdown
#### 12.3.3 NumberSlider — draft divergence feedback

The number input shows a **draft** while focused and commits the clamped value on every keystroke
(P69c), so the field can legitimately show text the setting does not hold. Whenever it does, say so.

- **Divergent** = editing, and the draft is blank/non-numeric, or `Number(draft) !== value`. Compare
  numerically — `06` and `6e1` are not divergences and must never warn.
- **Treatment:** `--warning` 1px border + `inset 0 0 0 1px var(--warning)` on the input (no box-model
  change, focus **outline** untouched), plus one 12px `--text-2` sentence in the row's help slot,
  which hides the help text while it shows. Ring and sentence are **one state** — colour never
  travels alone. Never `--danger`: the setting is already at a legal value, nothing is lost.
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

Full spec: `docs/contracts/P69c-draft-feedback-ui.md`.
```

---

## 15. Flagged for the orchestrator

**B1 — File placement.** This should be §13 of `P69-settings-ui.md` and §12.3.3 of
`ui-reference.md`. I could not append with a whole-file-only `Write` without re-emitting 1013 lines
(and `ui-reference.md` was being edited by the P73 pass the same day). **Recommendation: fold §13
and §14 in with a line-level edit at the start of P69g, then replace this file with a pointer.**
Nothing here depends on where it lives.

**B2 — 600 ms.** Reasoned from typing cadence, not measured on users. **Recommendation: ship 600 ms**
as a single named constant (`DRAFT_HINT_DELAY_MS`) and let the native checkpoint (§13.9 item 1) be
the one chance to change it. Anything under ~400 ms will start catching ordinary two-digit entry.

**B3 — Sequencing vs P73.** §13.7 needs ~25 lines of `src/styles.css`, which P73 holds.
**Recommendation: land the TS (hook + classifier + `NumberSlider`) first and the CSS in P69g**, which
is already the CSS gate. The intermediate state is harmless: `aria-invalid` and the live hint work
with no ring.
