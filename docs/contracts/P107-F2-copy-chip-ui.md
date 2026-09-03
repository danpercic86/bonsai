# P107 F2 — the copy-candidate chip: `unknown` verdict gets its own neutral variant

**Status:** spec complete, awaiting implementation (senior-dev).
**Resolves:** `docs/contracts/P107-hue-over-own-tint-ui.md` §10 / §12 **F2** ("still open").
**Touches:** `src/components/WorktreeCopyCandidates.tsx`, `src/styles/dialogs-forms.css`,
`src/ipc/mock/handlers/worktrees.ts` (one harness knob).
**Does NOT touch contrast of the existing chip** — P107 A16 already remediated it (`--text-1`,
11.31 dark / 12.21 light). That decision stands unchanged.

---

## 1. The defect

`WorktreeCopyCandidates.tsx:107` renders one chip for both reasons a checked row needs a decision:

```
<span className="wt-copy-chip">{isConflict ? 'conflict' : 'unchecked'}</span>
```

The chip appears **only on rows whose checkbox IS ticked** (`needsDecision = isChecked &&
(isConflict || previewFailed)`), and the `'unchecked'` branch fires when `previewFailed` — i.e.
Bonsai could not compute a verdict for that path.

Two independent faults:

1. **The word is inverted.** `unchecked` sits ~8px from a real checkbox and reads as "the box is not
   ticked" — the precise opposite of the truth. The intended meaning is "we could not check this
   file".
2. **The tint asserts the wrong severity.** `.wt-copy-chip` (`dialogs-forms.css:133`) is
   danger-tinted: `--danger` 16% fill, `--danger` 35% inset hairline, `--text-1` ink. Danger means
   *this will destroy something*, which is exactly right for `conflict` (the file exists at the
   destination and Overwrite clobbers it) and wrong for *unknown verdict*, which is indeterminate.

Context that matters for the fix: when `previewFailed`, `WorktreeCreateDialog.tsx:345-350` already
renders a `--warning-strong` note **under** the list — *"Couldn't check for conflicts against
`{branch}` — choose Overwrite per file to copy it anyway; unselected files are skipped."* The
warning severity for the whole situation is already stated once, in the right place. Repeating a
warning hue on every row would be the same alarm shouted N times.

---

## 2. Decisions

### (a) The word → `unknown`

Chip text becomes **`unknown`** (rendered `UNKNOWN`; the class sets `text-transform: uppercase`).

- Cannot be misread as checkbox state — `unknown` has no checkbox sense.
- It reads as "we don't know", not "this file is bad".
- It matches the vocabulary already used for this exact state in the code's own comments
  (`WorktreeCopyCandidates.tsx:12`, `WorktreeCreateDialog.tsx:77-79`: "UNKNOWN verdict") and the
  house's neutral-pill family (`.checks-rollup-pill--neutral`).
- One word, so the path column (which truncates) loses nothing versus `conflict`.

Rejected: `not checked` / `unverified` (collide with checkbox or with CI "checks" vocabulary),
`no verdict` (jargon), `can't check` (an apostrophe inside a 10px uppercase chip reads badly).

### (b) Split the style → a `--unknown` modifier, hueless

**Yes, split.** One chip class cannot carry two severities honestly. The split is a *modifier*, not
a new component: identical geometry, one changed fill, one removed hairline.

| | conflict (unchanged) | unknown (new) |
|---|---|---|
| class | `wt-copy-chip` | `wt-copy-chip wt-copy-chip--unknown` |
| fill | `--danger` 16% | `--text-2` 12% |
| hairline | `--danger` 35% inset | none |
| ink | `--text-1` | `--text-1` (inherited) |

The `--text-2` 12% tint is the **house hueless recipe** already in `ui-reference.md` §11 and already
shipping as `.checks-rollup-pill--neutral` (`checks-panel.css:122`), which likewise carries a
neutral background with no border and keeps `--text-1` ink. Nothing new is invented, and **no new
token is introduced.**

**Ink stays `--text-1`, deviating from P107 F2's suggested `--text-2`.** The two chips sit
side-by-side in one list; different ink weights would read as two component families rather than two
states of one. The §11 `--text-2` figure (5.79 / 6.22) governs standalone hueless *pills* in the
sidebar and chrome, not a chip paired with a `--text-1` sibling. `--text-1` is also the safer
number by a wide margin (below).

**Contrast — composited base is `--bg-1`.** Verified, not assumed: this list renders inside
`.dialog-card` (`dialogs.css:17`, `background: var(--bg-1)`), and `.worktree-create-card`
(`dialogs-forms.css:184`) overrides **width only**. So `--bg-1` *is* the dialog surface here; the
two are not different bases. (`dialogs-forms.css:92` says the same thing in a neighbouring comment.)

| Pair | Composited over | Dark | Light | Bar |
|---|---|---|---|---|
| `--text-1` ink on `--text-2` 12% tint | `--bg-1` (`.dialog-card`) | **10.80:1** | **12.87:1** | 4.5 ✓✓ |
| `--text-1` ink on `--danger` 16% tint (existing, unchanged) | `--bg-1` | 11.31:1 | 12.21:1 | 4.5 ✓ |

(WCAG 2.x relative-luminance computation over the flattened `color-mix` result; `#1d2026`/`#f6f7f9`
base, `#a8adb8`/`#4b515c` tint, `#e8eaed`/`#1c1f24` ink. Margins are >2× the bar, so measurement
noise cannot flip the verdict.)

**Chip edge vs surface is deliberately below 3:1** (~1.2:1 dark). The 3:1 non-text bar applies to
boundaries needed to *identify a control*; this chip is a static label, not a control, and its
meaning is carried by the word at 10.8:1. Adding a neutral hairline would not help — `--text-2` 35%
inset on `--bg-1` computes to **2.06:1**, still under 3:1 — which is another reason the house
`--neutral` recipe drops the border rather than tinting it.

**Colour is not the sole carrier** (§7): the two states are distinguished by *different words*, not
by hue. No glyph is added — a glyph on `unknown` but not on `conflict` would be a new inconsistency,
and the two words are already unambiguous.

### (c) Accessible name → the chip needs a *programmatic association*, not a role

The chip is a bare `<span>` today. It should stay one — it is neither a control nor a landmark, and
inventing `role="status"` would make it announce itself on every render, which is wrong for a static
row annotation.

The real defect is that **the chip is currently invisible to assistive tech**: it lives outside the
row's `<label>`, so a screen-reader user hears the path and the two buttons but never learns *why*
this row grew an Overwrite/Skip choice. Fix by associating it with the row's checkbox:

- chip gets `id={chipId}`;
- the row's `<input type="checkbox">` gets `aria-describedby={needsDecision ? chipId : undefined}`.

Result: `"src/staged-change.ts, checkbox, checked, conflict"`. No role, no live region, no new
visually-hidden text.

Both chips also get a `title` that explains *why* rather than repeating the label (ui-reference §11
title rule). `title` is a sighted-hover affordance here, not the a11y mechanism — the
`aria-describedby` is.

---

## 3. Implementation spec (exact)

### 3.1 CSS — `src/styles/dialogs-forms.css`, immediately after the `.wt-copy-chip` block (ends :143)

```css
/* P107 F2: the `unknown` verdict is NOT a danger state — Bonsai could not compute
   a verdict, it did not find a destructive one. Hueless §11 recipe (--text-2 12%,
   as .checks-rollup-pill--neutral), ink inherited from .wt-copy-chip so the two
   chips in one list read as one family. --text-1 on this tint over the dialog's
   --bg-1 is 10.80:1 dark / 12.87:1 light. No hairline: a neutral one computes to
   2.06:1 and would clear no bar the word does not already clear. */
.wt-copy-chip--unknown {
  background: color-mix(in srgb, var(--text-2) 12%, transparent);
  box-shadow: none;
}
```

Two declarations. No new token, no `[data-theme='light']` block (both values are theme tokens), no
size/padding/font change — so `cozy` and `compact` are untouched and the two chips are pixel-identical
boxes.

### 3.2 Component — `src/components/WorktreeCopyCandidates.tsx`

Add `useId` to the React import and, at the top of the component body:

```tsx
const baseId = useId();
```

In the `rows.map((c) => …)` callback, take the index (`rows.map((c, i) => …)`) and derive:

```tsx
const chipId = `${baseId}chip-${group}-${i}`;
```

(`useId()` output already carries its own delimiters — the `CommandPalette.tsx:56` `baseId` pattern.
Index-based, not path-based: paths contain spaces and `#`, which are not safe in an `id`.)

Then, at :95-107, the two changed elements:

```tsx
<input
  type="checkbox"
  checked={isChecked}
  disabled={disabled}
  aria-describedby={needsDecision ? chipId : undefined}
  onChange={() => onToggle(c.path)}
/>
```

```tsx
<span
  id={chipId}
  className={isConflict ? 'wt-copy-chip' : 'wt-copy-chip wt-copy-chip--unknown'}
  title={
    isConflict
      ? "The branch you're checking out changed this file too. Overwrite replaces it in the new worktree."
      : "Bonsai couldn't check this file for conflicts, so it is skipped unless you choose Overwrite."
  }
>
  {isConflict ? 'conflict' : 'unknown'}
</span>
```

Nothing else in the file changes. The component stays presentational; no new prop, no new state, no
new file (the change is 2 CSS declarations + ~10 lines in a 140-line component — spinning out a
`WorktreeCopyChip.tsx` would add an import and a props interface to save nothing).

### 3.3 Strings — the complete set

| Where | String |
|---|---|
| chip, conflict verdict | `conflict` |
| chip, unknown verdict | `unknown` |
| chip `title`, conflict | `The branch you're checking out changed this file too. Overwrite replaces it in the new worktree.` |
| chip `title`, unknown | `Bonsai couldn't check this file for conflicts, so it is skipped unless you choose Overwrite.` |
| list note (existing, unchanged) | `Couldn't check for conflicts against {branch} — choose Overwrite per file to copy it anyway; unselected files are skipped.` |

---

## 4. States, themes, densities

- **Default / hover / pressed:** the chip is not interactive — no hover, active or pressed state,
  in either variant. The row around it has none either (`.wt-copy-row` sets no hover). Unchanged.
- **`:focus-visible`:** the chip is not focusable and gains no `tabIndex`. The focus ring belongs to
  the checkbox and the Overwrite/Skip buttons, unchanged (global 2px `--accent`, 1px offset).
- **Disabled** (`disabled` prop, i.e. the dialog is submitting): the chip keeps full opacity — it is
  a statement of fact, not a control, and the `.6`/`.55` dim is spent on the actual controls. Matches
  today's behaviour.
- **Loading / error / empty:** early returns at :52-58 — the list is replaced wholesale by
  `Loading uncommitted files…`, `.dialog-error`, or nothing. No chip exists in those states.
- **Long content:** both words are single short tokens; the chip is `flex: 0 0 auto` inside
  `.wt-copy-conflict` and the path (`.wt-copy-path`) is the flexible element that ellipsises with a
  `title`. A 240-char path shrinks the path, never the chip, and never wraps the row.
- **Mixed rows:** a `conflict` chip and an `unknown` chip never co-occur — `previewFailed` clears
  `verdictByPath` (`WorktreeCreateDialog.tsx:153`), so a failed preview makes every checked row
  `unknown`. The two variants nonetheless share geometry so a future mixed state would still align.
- **Both themes:** every value is a theme token; measured above for dark and light.
- **Both densities:** `panelDensity` does not reach dialogs — `dialogs-forms.css` contains no
  `data-density` rule, and `.wt-copy-row` is a fixed `min-height: 26px` in both. Nothing added here
  is a size, so `cozy` and `compact` are identical, as they are today.
- **Motion:** none added. No transition on `background` (an instantaneous change when the preview
  resolves is correct). `prefers-reduced-motion` unaffected.
- **Command palette:** nothing to register — this is a row annotation inside a modal dialog.

---

## 5. Harness states (`VITE_MOCK_IPC=1`)

The `unknown` chip is **currently unreachable in the browser harness**: mock `previewWorktreeCopy`
(`src/ipc/mock/handlers/worktrees.ts:169-186`) only rejects on an empty branch name, which the
branch combobox prevents. Add one knob, matching the house `query()` pattern
(`obsDeleteFail`, `wtPreviewFail` naming):

| Fixture | Behaviour | Verifies |
|---|---|---|
| default (existing) | `src/staged-change.ts` → `conflict`, others `clean` | the danger chip, unchanged |
| **`?wtCopyPreviewFail=1`** (new) | `previewWorktreeCopy` rejects with `{ kind:'git', message:'could not read the target tree' }` after the existing 120 ms delay | `previewFailed` → the `unknown` chip on **every** checked row, plus the existing `--warning-strong` note under the list |
| `?wtCopyPreviewFail=1` + check all four candidates, incl. `.env.local` and the 240-char path case | as above | chip/​toggle alignment with a truncating path; both themes; both densities |

Everything in §4 is AI-gate verifiable in the harness. No USER CHECKPOINT item.

---

## 6. Acceptance criteria

1. `grep -n "unchecked" src/components/WorktreeCopyCandidates.tsx` returns nothing.
2. `.wt-copy-chip--unknown` exists with exactly the two declarations in §3.1; `.wt-copy-chip` is
   otherwise byte-identical to today (P107 A16's ink and hairline untouched).
3. No hardcoded colour value is added anywhere — both new values are `var(--…)` inside `color-mix`.
4. With `?wtCopyPreviewFail=1`, every checked row shows a neutral `UNKNOWN` chip; without it, the
   conflicting row shows the danger `CONFLICT` chip. Both readable in dark and light.
5. The checkbox of a `needsDecision` row exposes the chip text as its accessible description
   (`aria-describedby` resolves to the chip's `id`); rows without a chip carry no
   `aria-describedby`.
6. `ui-reference.md` §11's hueless-pill bullet lists `.wt-copy-chip--unknown` as an instance
   (done in the same pass as this contract).
