# P111 — A pill is one line: ref/path truncation inside stadium chips (UI contract)

**Owner:** ui-designer · **Written:** 2026-09-03 · **Status:** spec complete, awaiting implementation
**Trigger:** `TODO.md:571` / `docs/history/todo-archive-2026-09.md:1598` — "the 90-char branch-name chip
becomes a 50 px two-line stadium at `border-radius: 999px`", surfaced by the P102/P105 long-name fixture.
**Changes a shared recipe** → `docs/contracts/ui-reference.md` §3.2 (new) and §11 are updated in the
same pass.
**Supplies** the ref renderer that `docs/contracts/P87b-FU1-FU4-git-dock-ui.md` §3 depends on.

---

## 0. Premise check (what I verified before designing)

| Claim in the brief | Verdict |
|---|---|
| The chip is at `border-radius: 999px` and wraps to two lines | **HOLDS.** `src/styles/dialogs.css:286-298` `.branch-name-chip` has `border-radius: 999px` and declares no `white-space`, no `max-width`, no `overflow`. Its container `.branch-name-suggest-chips` (`:280`) is `display: flex; flex-wrap: wrap`, so a chip wider than the container wraps *inside itself*. |
| "It appears in the graph ref pills" | **DOES NOT HOLD.** The graph ref pills are drawn imperatively on `<canvas>` from Rust layout (`ui-reference.md` §6: 999px, `max-width 160px with ellipsis`) and already truncate. They share no code and no CSS with this chip. Canvas is out of scope here and needs no change. |
| "and possibly elsewhere" | **HOLDS, and it is worse than one chip.** See §1 — three DOM instances of the same defect, one of which (`.pr-label`) carries fully arbitrary forge text. |

Only one call site renders `.branch-name-chip`: `src/components/BranchNameSuggest.tsx:83-92`. The
fixture that reaches it is `src/ipc/mock/handlers/ai.ts:283`.

---

## 1. The defect class, enumerated

A `border-radius: 999px` box is a *stadium*: the radius is clamped to half the shorter side, so at one
line of text the ends are semicircles and the shape reads as a pill. At two lines the same radius is
still half the (now ~2.4×) height, the ends bow outward, and the box reads as a lozenge — a shape the
app uses nowhere else. **The radius is not the bug; the second line is.**

Every `border-radius: 999px` rule in `src/styles/` was inspected. Boxes with a fixed height, a
closed-set label, or an explicit `white-space: nowrap` are safe. Three hold variable-length,
externally-supplied text with no line guard:

| # | Rule | Text it holds | Call site |
|---|---|---|---|
| **P1** | `dialogs.css:286` `.branch-name-chip` | an AI-suggested branch name — model output, unbounded | `BranchNameSuggest.tsx:86` |
| **P2** | `forge-pr-detail.css:119` `.pr-label` | a forge PR label name — arbitrary, and **has no `title`** | `PrDetailView.tsx:150` |
| **P3** | `ai-assets.css:129` `.asset-chip` | closed-set words at 6 of 7 call sites, but `ProfileManager.tsx:254` renders `{profile.model}` (a user-typed model id) | `ProfileManager.tsx:254` |

The house already has the correct recipe in three places — `controls.css:57` `.pill`
(`max-width: 160px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap`),
`graph-filter.css:54` `.graph-filter-chip-label` (same, at 260px), and `git-dock.css:18-22`
`.toolbar-phase` (same, at `22ch`). P1–P3 are the three that never got it.

---

## 2. Decision — threshold behaviour, not a magic number

**A pill is one line. Always.** Three rules, in force order:

1. **R1 — line guard.** Any box with `border-radius: 999px` that can contain variable-length text
   declares `white-space: nowrap`. Content that may legitimately occupy two lines does not get a
   999px radius; it gets the §11 `8px`, or `6px`/`4px`. *This is the rule that makes the class of bug
   impossible, not just this instance of it.*
2. **R2 — width threshold, expressed relative to the surface, never as a tuned constant.**
   - A chip that is **the row's own content** (P1, P2, P3 — chips in a wrapping chip-strip) gets
     `max-width: 100%`. It is as wide as its text needs, up to the width of its container; past that
     it truncates. The chip strip already wraps, so a long chip simply takes a whole row of the strip.
     No px number is involved, so it is correct at every window width and in a resized dialog.
   - A ref that is **one field of a dense metadata row** (the dock target, §3 of the FU contract) gets
     a `Nch` cap in the app's existing 20–26ch band (`.toolbar-phase` 22ch, `.forge-account` scope pill
     20ch), because there it must yield space to its siblings rather than take the row.
3. **R3 — truncate the *leading* segments, never the last one.** Refs and paths are distinguished by
   their tail (`feature/api/retry-budget` vs `feature/api/retry-window`; three AI suggestions that
   share `feature/add-`). Plain `text-overflow: ellipsis` clips exactly the part that identifies the
   thing. Use the app's existing leaf-preserving split instead (§3).

**Rejected alternatives, and why.** *Keep wrapping at a smaller radius* — a two-line chip in a
three-suggestion strip destroys the strip's rhythm and still reads as a paragraph in a box; and it
leaves R1 unwritten, so the next 999px chip repeats this. *Cap the width and let it clip* — a hard
clip with no ellipsis gives the user no signal that text was removed, which is the one thing
truncation must always signal.

---

## 3. `RefLabel` — the shared leaf-preserving label

### 3.1 Why a new file

The **splitter already exists and is already shared**: `splitPath()` at
`src/components/StatusFileRow.tsx:16-20` (`lastIndexOf('/')` → `{ dir, name }`), already imported
across components by `AiActivityHeader.tsx:18`. What does not exist is the shared *presentational*
pair — `AiActivityHeader.tsx:100-109` hand-rolls it with `.ai-dock-subject` / `.ai-dock-dir` /
`.ai-dock-name`, which are component-scoped names a dialog must not borrow. Extracting the 10-line
render into one file is cheaper than a fourth copy, and it gives the truncation policy exactly one
home. No existing component fits: `EmptyState`, `Combobox` and the pill helpers all solve other
problems.

### 3.2 New file — `src/components/RefLabel.tsx` (~45 lines, presentational, no state)

```
export interface RefLabelProps {
  /** The complete ref or path. Rendered whole into the DOM; CSS does the truncating. */
  value: string;
  /** Class on the wrapper, so each surface keeps its own font/colour. */
  className?: string;
  /** Omit `title` where an ancestor already owns the tooltip (the dock row does). */
  withTitle?: boolean;
}
```

Renders exactly:

```
<span className={`ref-label${className ? ` ${className}` : ''}`} title={withTitle ? value : undefined}>
  {dir !== null && <span className="ref-label-head">{dir}</span>}
  <span className="ref-label-leaf">{name}</span>
</span>
```

with `const { dir, name } = splitPath(value)` — the existing helper, imported, not reimplemented.

**Hard rule for the implementer: the truncation is CSS-only.** `value` is never `slice`d, never
shortened with `…` in JS, never measured. The DOM always holds the whole string. This is what keeps
the accessible name correct (§5).

### 3.3 New CSS — `src/styles/controls.css`, appended beside `.pill` (13 lines)

```css
/* P111 R3 — the leaf-preserving ref/path label. The LEADING segments ellipsize;
   the last segment never truncates, because that is the part that identifies the
   ref. Same idiom as .ai-dock-subject/.ai-dock-dir/.ai-dock-name, extracted so
   chips and dense rows share one policy. */
.ref-label {
  display: inline-flex;
  align-items: baseline;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  white-space: nowrap;
}

.ref-label-head {
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.ref-label-leaf {
  flex: none;
}
```

`.ref-label` sets no colour and no font — it inherits from whatever chip or row hosts it, so it adds
no token and cannot drift from its surface. (`.ai-dock-dir` dims the head to `--text-2`; that is an
AI-dock choice and stays there. A chip whose whole text is one ref keeps one colour — dimming half a
branch name inside a *choice* control would imply the dim half is less real.)

**Degenerate cases.** No `/` in the value → `dir === null`, one leaf span, `flex: none`, so the label
overflows `.ref-label`'s `overflow: hidden` and hard-clips. That is correct for a single unbroken
90-character token: there is no leading segment to sacrifice, and the `title` carries the whole
string. A value *ending* in `/` → `name === ''`, head takes the whole width and ellipsizes normally.
Empty string → renders one empty span; callers must not pass `''` (all four do not).

---

## 4. Per-surface specs

### 4.1 P1 — `.branch-name-chip` (the trigger)

`BranchNameSuggest.tsx:83-92` becomes:

```
<button
  key={name}
  type="button"
  className="branch-name-chip"
  aria-label={name}
  title={`Use "${name}"`}
  onClick={() => onPick(name)}
>
  <RefLabel value={name} />
</button>
```

CSS — replace `dialogs.css:286-298` with (changed lines marked; the colour comment block at :291-293
is preserved verbatim, it records a measured P105 decision):

```css
.branch-name-chip {
  display: inline-flex;        /* NEW — needed for the min-width:0 truncation chain */
  align-items: center;         /* NEW */
  min-width: 0;                /* NEW */
  max-width: 100%;             /* NEW — R2: as wide as the strip, never wider */
  min-height: 24px;            /* NEW — §3.1 hit floor, stated not inherited */
  padding: 3px 10px;
  border: 1px solid color-mix(in srgb, var(--accent) 40%, var(--border));
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  /* P105 C1 comment block — keep verbatim */
  color: var(--text-1);
  font-family: var(--font-mono);
  font-size: 12px;
  white-space: nowrap;         /* NEW — R1 */
  cursor: pointer;
}
```

`.branch-name-suggest-chips` (`:280-284`) is unchanged: `flex-wrap: wrap` + `gap: 6px` still puts a
too-wide chip on its own row of the strip, which is exactly R2's threshold behaviour.

**Geometry.** Chip height is now a stated **24 px** in both themes and both densities — dialogs are
density-invariant (`dialogs.css` contains no `[data-density]` rule; `panelDensity` scopes
`.right-panel`, `.ai-dock`, `.git-activity-dock` only), so one geometry serves `cozy` and `compact`.
Strip gap stays 6px; the dialog's own padding is untouched.

**Colour.** Nothing changes. Ink stays `--text-1` on the 12% accent tint — **11.46:1 dark / 13.27:1
light** (9.73 / 11.60 on the 22% hover tint), the P105 C1 measurement recorded in the rule's own
comment. No new token, no new pair, so no new measurement is owed.

**States.**

| State | Treatment |
|---|---|
| Default | as above; one line; ellipsis only when the name exceeds the strip width |
| Hover | `background: color-mix(in srgb, var(--accent) 22%, transparent)` — unchanged (`:300-302`) |
| Active/pressed | no dedicated rule; the click resolves in <100 ms into the branch-name field. Do not add one. |
| `:focus-visible` | global ring, `tokens-and-base.css:225-228` — 2px `--accent`, 1px offset. Now drawn around a stadium of stated 24px height, so it no longer wraps a lozenge. |
| Disabled | does not exist — chips render only after a successful suggestion (`names !== null && names.length > 0`, `BranchNameSuggest.tsx:80`) |
| Loading | the strip is absent; the button reads `Suggesting…` (`:71`) — unchanged |
| Empty | the strip is absent — unchanged |
| Error | `.branch-name-suggest-error` paragraph, `--danger-strong`, `overflow-wrap: anywhere` — unchanged (a prose error *should* wrap; it is not a pill) |
| Long content | one line, leading segments ellipsized, leaf intact, whole name on `title` and in `aria-label` |

### 4.2 P2 — `.pr-label`

`PrDetailView.tsx:150-152` gains a `title` (it has none today, so a truncated forge label would be
unrecoverable) and the same chip treatment. Labels are flat words, not refs — **no `RefLabel`**, plain
tail ellipsis is right here:

```
<span key={label} className="pr-label" title={label}>{label}</span>
```

```css
.pr-label {
  display: inline-block;
  max-width: 100%;             /* NEW — R2 */
  overflow: hidden;            /* NEW */
  text-overflow: ellipsis;     /* NEW */
  white-space: nowrap;         /* NEW — R1 */
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 1px 8px;
  font-size: 11px;
  color: var(--text-2);
  background: var(--bg-2);
}
```

Non-interactive, so no hit-target floor and no focus state. `--text-2` on `--bg-2` is unchanged.

### 4.3 P3 — `.asset-chip`

One line added to `ai-assets.css:129-135`:

```css
  white-space: nowrap;         /* NEW — R1 */
```

and `ProfileManager.tsx:254` gains `title={profile.model}` (the only variable-length call site; the
other six render closed-set words and need nothing). No `max-width`: these chips sit at the end of a
non-wrapping row and are already bounded by it. **`.asset-chip`'s measured 19.9375 px height
(P102/P105 AC12) must not change** — `white-space` does not affect height. Any diff that moves that
number is a defect.

---

## 5. Accessibility — the truncated-name rule

**A truncated branch name is a wrong branch name, so truncation may never reach the accessible name.**

- **Truncation is CSS-only.** The complete string is always in the DOM. A JS `slice` + `…` would put
  a wrong ref into the accessible name and is prohibited in every one of these surfaces.
- **`.branch-name-chip` gets an explicit `aria-label={name}`.** With `RefLabel`'s two spans, name
  computation would concatenate sibling text nodes and can insert a separating space
  (`origin/feature/ x`) — the failure `ui-reference.md` §11 already records for the Settings rail
  (`Git config , repository`). An explicit `aria-label` holding the exact ref removes the ambiguity.
  WCAG 2.5.3 (Label in Name) is satisfied: the visible text, concatenated, *is* the accessible name.
- **`title` stays a second channel, not the only one.** `title={`Use "${name}"`}` is the sighted
  mouse user's recovery path and reads the action; `aria-label` is the AT path and reads the ref.
  Neither is load-bearing alone.
- **`.pr-label` / `.asset-chip`** are non-interactive spans with a single text node; CSS truncation
  leaves their text intact, so the surrounding region's name is already complete. `title` is added
  purely for the sighted user.
- **Hit target.** `.branch-name-chip` states `min-height: 24px` (§3.1 floor) rather than inheriting it
  from line-height, which is what made the current height depend on `body`'s 1.45 line-height.
- **Focus ring** is the global `:focus-visible` only; no `:focus` ring, no hover-as-focus.
- **Colour is not a carrier** anywhere in this contract — every change is shape and flow.
- **Reduced motion:** nothing here animates, so nothing to honour.

---

## 6. Motion

None. No transition is added; the chip's existing hover is an instantaneous `background` swap and
stays that way. A width or `max-width` transition on a chip strip would reflow the dialog and is
prohibited.

---

## 7. Microcopy

No new strings. `title={`Use "${name}"`}` on the chip and `title={label}` / `title={profile.model}`
on P2/P3 are the only text touched, and the first is unchanged from today.

---

## 8. Harness states (`VITE_MOCK_IPC=1`)

| Fixture | State | Proves |
|---|---|---|
| `src/ipc/mock/handlers/ai.ts:283` (**exists**) | the 90-char branch name, alongside two normal-length siblings | one line, 24px, leading segments ellipsized, leaf intact, no lozenge |
| **add** to the same fixture | a 60-char name with **no `/` at all** | the degenerate branch of §3.3 — hard clip, `title` recoverable |
| **add** to the same fixture | a name ending in `/` | head-only truncation does not blank the chip |
| existing suggest error path | error string | the `.branch-name-suggest-error` paragraph still wraps (a pill rule must not leak into prose) |
| PR detail with a **long forge label** — `src/ipc/fixtures/forge.ts` | P2 | one line + `title` |
| `ProfileManager` with a 40-char `profile.model` | P3 | one line, height still 19.94 px |

Verify in **both themes** (`data-theme`) and, for the dock consumer, both densities. The chip
surfaces are density-invariant, so one pass covers both.

**Not a USER CHECKPOINT** — every surface here is reachable in the browser harness.

---

## 9. Flags for the orchestrator

- **F-A (scope).** P2 and P3 are the same defect as P1 but were not in the brief. They are two CSS
  lines and one `title` each. **Recommendation: fix all three in one increment** — fixing only the
  chip that a fixture happened to reach is how this defect survived to begin with. If the
  orchestrator wants a narrower diff, P1 alone is coherent and P2/P3 become TODO lines.
- **F-B (ellipsis direction).** R3 (leaf-preserving) is a small behaviour change to a shipped surface
  and costs a new 45-line file. The cheaper option is plain tail ellipsis on the chip, which is one
  CSS line and no new component but clips the distinguishing half of a ref. **Recommendation: R3**,
  because the AI suggests names that share prefixes by construction. Flagged because it is a
  judgement, not a defect fix.

---

## 10. `ui-reference.md` changes made in this pass

- **§3.2 (new) — "A pill is one line".** R1/R2/R3 above, the three-instance inventory, and the
  `.pill` / `.graph-filter-chip-label` / `.toolbar-phase` precedents.
- **§11** — one bullet cross-referencing §3.2, so the pill recipe itself carries the line guard.
