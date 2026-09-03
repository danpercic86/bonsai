# P109 — Status-badge semantics (the letter's *meaning*, not its ink)

**Owner:** ui-designer · **Date:** 2026-09-03 · **Branch:** `feat/p91-observability`
**Files this contract governs:** one new file `src/components/FileStatusBadge.tsx`, and the 8 render
sites in `ComposerGroupCard.tsx`, `DiffBrowser.tsx`, `DiffFileTree.tsx`, `DiffOverlay.tsx`,
`prPanel/PrFileRow.tsx`, `StatusConflictsSection.tsx`, `StatusFileRow.tsx` (×2).
**Zero CSS diff. Zero token diff. Zero geometry diff.**
**Inputs:** `docs/contracts/P106-status-badge-ink-ui.md` §3 (the sole-carrier determination this
contract is built on) · `docs/contracts/ui-reference.md` §2 (method + residue rules), §7 (the family
record), §11 (pill-in-accessible-name recipe) · `docs/contracts/P95-a11y-ui.md` §1.2 (why a label
needs an honest role) · `docs/contracts/P108-hue-as-text-on-neutral-ui.md`:638 (defers here).

---

## 0. Result in one screen

- **P106 made the letter legible; P109 is about the letter being insufficient.** P106 proved the
  letter is the *sole* non-colour carrier of file status at all 8 render sites. A sole carrier that
  has no accessible name, and that renders the same glyph for two different states, is not a carrier
  — it is a second colour-only channel wearing a letter costume. One defect class per milestone is
  what has kept this programme reviewable, so P106 deliberately did not roll this in.
- **The render-site list was re-verified and still holds: 8 sites, 7 files.** Two line numbers
  drifted (`DiffOverlay.tsx:295 → :304`, `ComposerGroupCard.tsx:129 → :138`) because P106's own D1
  fix added comments above them — the **count** is the criterion, exactly as `ui-reference.md` §2
  requires.
- **The defect is worse than P106 recorded, and the correction is the headline.** P106 §3 says
  "`untracked` and `added` deliberately share the letter `A` (`StatusFileRow.tsx:15`, P4c)". That is
  true of **3 of the 6** `BADGES` tables. The other **3 already render `U`** (`DiffBrowser.tsx:32`,
  `DiffFileTree.tsx:22`, `prPanel/PrFileRow.tsx:20`). **The same untracked file renders `A` in the
  status panel and `U` in the diff file tree, today, in the shipped app.** This is not a design
  choice to preserve; it is a live inconsistency with a 50/50 split.
- **Decision D-A: `untracked` → `U` everywhere** (§3). It is already half the app, it is already the
  letter in the design system's own family name "A/M/D/**U**/R", `?` is unavailable (§7's house glyph
  vocabulary reserves it for *unknown*), and today's `A` is the mapping that actually contradicts git
  — porcelain `A` means "added to the index", which is precisely Bonsai's `added`.
- **Decision D-B: `role="img"` + `aria-label` on the badge span** (§4). Not bare `aria-label` (invalid
  on `generic`, announced inconsistently — P95 §1.2's exact finding), not `title` (unreliable, and it
  would fight the row's existing path tooltip), not a visually-hidden sibling (stutters "Untracked U",
  adds a node per row across a 200-row list, and breaks `DiffOverlay.test.tsx:61`).
- **Decision D-C: one new component, `FileStatusBadge.tsx`** (§5). The defect *is* drift between six
  duplicated tables; the structural fix is that there stop being six. 6 tables → 1, and the letter and
  its name become physically inseparable.
- **8 statuses, not 7** — `ComposerGroupCard.tsx:49` renders a literal `'?'` for a path with no known
  status. It has never been enumerated in this programme (§2).
- **No e2e spec breaks** (§10). Every Playwright status assertion targets the `row-action` buttons,
  which carry explicit `aria-label`s (`Stage {path}`) that this contract does not touch. Named in §10
  so the implementer is not surprised.

---

## 1. Re-verification of P106's premise (do not re-derive it — confirm it)

P106 §3's determination stands unmodified and is the load-bearing input: at all 8 sites the badge is
the same 12px-wide, `text-align: center`, 11px/600 mono span in the same first slot; shape and
position never vary by status; no word in the row names the status; 11px/600 is far below the
large-text threshold **in both densities** (`font-size: 11px` is literal on `.file-badge`, not
`var(--rp-row-font)`).

**Re-verified this session, by count:**

| Check | Command | Result |
|---|---|---|
| Render sites | `rg -n "className=\"file-badge mono\"" src/components` | **8**, in **7** files — unchanged |
| Ancestor status classes | `rg -n "file-status-" src/components` (excl. tests) | **8** — P106 D1/AC12 landed, so S7/S8 now carry the class |
| Canvas-drawn status badges | `rg -n "FileStatus\|fileStatus" src/graph` | **0** — still DOM-only (P106's fourth pass) |
| CSS ink rules | `rg -n "file-badge" src/styles` | **8** (1 base + 7 status rules) — this contract adds none |

Two of P106's `file:line` references have drifted by 9 lines because P106's own fix wrote comments
above them. **Per `ui-reference.md` §2, the count is the criterion, not the line** — the count is
identical, so nothing was added or removed. This contract cites `file:line` for navigation only.

**The consequence P106 stopped short of.** A sole carrier that is *not in the accessibility tree as
meaning* is not a carrier for the user who most needs one. Today a screen-reader user gets the
character `M` — jargon they did not choose — or, for `A`, a character that means two different things
depending on which panel they are in. WCAG 1.4.1 (Use of Color) is technically satisfied; 1.3.1
(Info and Relationships) and 4.1.2 (Name, Role, Value) are not served, and the *visual* ambiguity
between `added` and `untracked` is a plain design defect independent of any WCAG clause.

---

## 2. The real status set — verified from code, not from the brief

The brief guessed `A/M/D/U/R/T/?`. The truth is **8 rendered glyphs from 7 `FileStatus` variants plus
one non-`FileStatus` fallback**. `FileStatus` is exactly (`src/ipc/types/status.ts:1-8`):
`added | modified | deleted | renamed | typechange | conflicted | untracked`. The eighth glyph is
`ComposerGroupCard.tsx:49` — `statusByPath.get(path)` returning `undefined` renders `'?'`.

**Recorded verdict, per status. "Current letter" is split where the six tables disagree.**

| Status | Current letter | Current accessible name | Ink (P106, unchanged) | **Proposed letter** | **Proposed accessible name** |
|---|---|---|---|---|---|
| `added` | `A` (6/6 tables) | *none* | `--success-strong` | `A` — **unchanged** | `Added` |
| `modified` | `M` (6/6) | *none* | `--warning-strong` | `M` — unchanged | `Modified` |
| `deleted` | `D` (6/6) | *none* | `--danger-strong` | `D` — unchanged | `Deleted` |
| `renamed` | `R` (6/6) | *none* | `--accent-strong` | `R` — unchanged | `Renamed` |
| `typechange` | `T` (6/6) | *none* | `--warning-strong` | `T` — unchanged | `Type changed` |
| `conflicted` | `C` (6/6) | *none* | `--danger-strong` | `C` — unchanged | `Conflicted` |
| **`untracked`** | **`A` in 3 tables, `U` in 3** | *none* | `--success-strong` | **`U` — CHANGED** | `Untracked` |
| **`(unknown)`** | `?` (composer only) | *none* | inherited `--text-1` | `?` — unchanged | `Status unknown` |

**The 3/3 split, with line numbers, because this is the finding:**

| Table | `untracked` renders | File |
|---|---|---|
| `StatusFileRow.tsx:15` | `'A'` | status panel rows (S1, S2) — and re-exported to `StatusConflictsSection` |
| `DiffOverlay.tsx:25` | `'A'` | full-pane diff header (S7) |
| `ComposerGroupCard.tsx:15` | `'A'` | commit composer (S8) |
| `DiffFileTree.tsx:22` | `'U'` | commit / diff file tree (S4) |
| `DiffBrowser.tsx:32` | `'U'` | all-files diff browser (S5) |
| `prPanel/PrFileRow.tsx:20` | `'U'` | PR changed files (S6) |

`notes/todo.txt` is `untracked` in `src/ipc/fixtures/status.ts:42` **and** in
`src/ipc/fixtures/diffs.ts:126`, so both letters are reachable for the same file in the same session:
`A` in the Changes list, `U` one click away in the diff browser. This is observable today.

**Ink is untouched by this contract.** The letterform changes; the font, size, weight and `color`
declaration do not, so every P106 contrast figure carries over unrecomputed. Do **not** re-measure
the family — `--success-strong` on the six live backdrops is 5.71/5.94 minimum and a `U` is the same
ink as an `A`.

---

## 3. D-A — a different letter, a different name, or both? (recommendation, not a menu)

**Recommendation: both. `untracked` gets the letter `U` *and* the accessible name `Untracked`.**

The brief frames these as competing. They are not, and the trade it describes — "a letter change is
visible to everyone but breaks a convention users may know" — dissolves once the 3/3 split is on the
table: there is no convention to break, because the app does not currently have one.

**Why a name alone is insufficient.** P106's sole-carrier analysis is the premise: the letter is the
only non-colour channel. `added` and `untracked` also share a hue (`--success-strong`, deliberately —
P4c). So under a name-only fix, a sighted user looking at a Changes list sees a green `A` and a green
`A` for two states with different consequences: one has a staged/committed version to restore, the
other does not and can only be **permanently deleted** (`StatusFileRow.tsx:156`, "Delete this new
file (permanently removes it from disk)"). A destructive-affordance difference that is invisible in
the row's own identity column is not acceptable, and fixing it only in the accessibility tree would
mean sighted users are the ones left without the distinction. That is a strange place to land.

**Why `U` and not `?`.** `ui-reference.md` §7's house glyph vocabulary is explicit: **`?` =
unknown/needs-you**, and "a single glyph never means two different things on two surfaces". `?` is
already rendered, in this very family, by `ComposerGroupCard.tsx:49`, for genuinely-unknown status.
Reusing it for `untracked` would collide inside one component. Git's own `??` is two characters and
does not fit a 12px single-glyph slot. **`?` is unavailable on house rules, not on taste.**

**Why `U` and not "keep `A`".**

1. **It is already half the app.** Three tables ship `U` today. Choosing `U` reconciles six tables
   toward the majority; choosing `A` would mean *changing three sites to introduce* the collision.
2. **The design system already says `U`.** `ui-reference.md` §7 names the family "the A/M/D/**U**/R
   letter badge" in three places (`:1107`, `:1123`, and `status-panel.css:207`) while its own table at
   `:1115` records `untracked | A`. The `U` in the canonical family name has had **no referent** under
   the recorded mapping. The doc has been describing the fix for months.
3. **`A` is the mapping that misleads a git-fluent user, not `U`.** In `git status --short`, `A` means
   *added to the index* — exactly Bonsai's `added`. Rendering `A` for an untracked file tells a user
   who knows git something false. Porcelain's `U` means *unmerged*, which Bonsai spells `C`; Bonsai's
   letter set already diverges from porcelain on `C` and always has, so this is a house alphabet, not
   a claim to be porcelain-compatible. Between "diverges from porcelain on one more letter" and
   "actively asserts a false porcelain meaning", the first is better.
4. **P4c's intent survives.** `status-panel.css:217` states the goal: untracked rows "read as normal
   adds — green `A` badge, no muted/italic path treatment". The *mechanisms* carrying that intent are
   the shared `--success-strong` hue, `font-style: normal`, and `--text-1` on the path — all three are
   untouched here. Untracked rows keep reading as first-class Changes rows; they simply stop being
   indistinguishable from staged adds. **Do not delete or weaken the `:217-228` block.**

**Cost, stated honestly.** A user who has learned Bonsai's green `A` for new files sees a green `U`
after this lands. It is a one-time relearn of one glyph, in the same colour, in the same slot, and
half the app was already teaching them `U`. There is no migration copy, no toast, no changelog modal
— that would be marketing chrome for a one-glyph change.

**Rejected alternative — name only, letter stays `A`.** Cheapest, and it does satisfy 4.1.2. Rejected
because it leaves the sighted-user defect that P106 §3 proved is real, and because it would enshrine
the 3/3 split permanently (the three `U` tables would have to be changed *to* `A` for consistency,
making the app worse). Recorded so it is not re-litigated.

---

## 4. D-B — the naming mechanism

**Recommendation: `role="img"` + `aria-label` on the existing badge `<span>`. No new DOM node, no
change to `textContent`, one uniform edit shape at all 8 sites.**

```
<span className="file-badge mono" role="img" aria-label="Untracked">U</span>
```

### 4.1 Why not the three alternatives

| Mechanism | Verdict |
|---|---|
| **Bare `aria-label` on the span** | **Rejected — invalid.** ARIA prohibits `aria-label` on `generic`; browsers expose it inconsistently. This is P95 §1.2's finding verbatim, in a different costume: *"a focusable generic div has no accessible-name computation guaranteed across ATs, so `aria-label` on it is announced inconsistently"* — the house answer there was to give the element an **honest role** so the label computes reliably. For a text character used as an icon, the honest role is `img`. |
| **`title`** | **Rejected.** Not reliably announced, invisible to keyboard and touch, and it would open a *second, competing* tooltip on a 12px target that already sits inside an element carrying `title={path — Double-click to stage}` (`StatusFileRow.tsx:82`). The row's `title` is the path affordance; the badge must not steal it. |
| **Visually-hidden sibling (`.sr-only`)** | **Rejected here, though it is the §11 pill recipe.** Three concrete costs: (a) the letter is *not* replaced, so the reader says **"Untracked U"** — a stutter, 200 times, which is exactly the noise the brief asks to avoid; (b) one extra DOM node per row per site on a list that already virtualizes; (c) it changes `textContent`, breaking `DiffOverlay.test.tsx:61` (`screen.getByText('M')`). §11's own escape hatch applies: *"use an explicit `aria-label` when punctuation matters"* — generalized in AC12 to **when the visible text *is* the token being renamed**. |

`role="img"` reaches the same end-state §11 mandates for pills (glyph out of the tree, word into the
control's name) with one node instead of two, because `aria-label` on `role="img"` **replaces** the
element's text in the accessibility tree rather than prefixing it.

### 4.2 Where the name lives: on the badge, not on the row

**On the badge.** Accessible-name-from-content computation walks children and takes a child's
`aria-label` in place of its text, so the badge's word folds into whatever wraps it, for free:

| Site | Wrapper | Name before | Name after |
|---|---|---|---|
| S1 `StatusFileRow.tsx:100-111` | `<button class="file-row-main" aria-expanded>` | "A notes/todo.txt" | **"Untracked notes/todo.txt"** |
| S3 `StatusConflictsSection.tsx:118-130` | `<button class="file-row-main" aria-expanded>` | "C src/app.rs both modified" | **"Conflicted src/app.rs both modified"** |
| S4 `DiffFileTree.tsx:175-188` | `<button class="file-row diff-tree-file">` | "M src/app.rs +12 −3" | **"Modified src/app.rs +12 −3"** |
| S6 `PrFileRow.tsx:37` | `<button>` (or `<span>` when `binary`) | "U notes/todo.txt +4 −0" | **"Untracked notes/todo.txt +4 −0"** |
| S2, S7, S8 | non-interactive `<span>` / `<div>` / `<li>` | read in browse mode | word instead of letter |

**Not on the row.** Putting an `aria-label` on the row button would *replace* name-from-content, so
the implementer would have to rebuild the whole string — path, rename arrow, `+12 −3` counts,
conflict kind — in four different shapes, and it would silently desync the moment a row gains a
child. It also risks colliding with the `Stage {path}` / `Unstage {path}` labels the e2e suite
depends on (§10). One label on one leaf is the smaller, more durable surface.

### 4.3 Announcement cost on a 200-row list, measured in words

The word replaces the letter; it does not add to it. Per row the reader goes from `"A"` to
`"Untracked"` — one token either way, ~2 syllables longer, and unambiguous. In focus mode (the
primary path: S1/S3/S4/S6 are real `<button>`s in the tab order) only the computed name is spoken.
In browse mode (S2/S7/S8) NVDA prefixes `role="img"` with "graphic", so an unfocusable badge reads
"graphic, Untracked". **That is the one real cost of this pick, and it is accepted**: it affects the
three non-interactive sites, where a single badge is being examined rather than a list being scanned.
The alternative that avoids it (bare `aria-label`) is invalid; the alternative that avoids it validly
(`.sr-only`) costs the "Untracked U" stutter on all 200 rows of the interactive sites. Trading noise
on 3 static sites for silence on the scannable list is the right side of that trade.

### 4.4 Role constraints — the `§4.1` analogue check (and the coordinator's correction)

**Checked, and nothing analogous constrains the status list.** `ui-reference.md` §4.1's forbid-list
(`role="grid"`, `aria-rowcount`, `role="row"`, `aria-rowindex`) is scoped to the **commit-graph
canvas**, where there are no real row elements for those roles to describe. The status list is the
opposite case: a real `<ul>` of real `<li>`s containing real `<button>`s, with native list and button
semantics already carrying structure and operability. **This contract adds no role to the list, the
sections or the rows** — only to the leaf glyph, which genuinely has no native semantics.

The coordinator's mid-task correction is accepted: **`aria-activedescendant` is *not* forbidden**
(`ui-reference.md:847`, valid on `role="group"` per ARIA 1.2, live on this branch at
`GraphCanvas.tsx:745`, absent on `origin/dev`). **Re-evaluated on the merits, and still rejected for
this surface**, for two independent reasons:

1. **It answers a different question.** `aria-activedescendant` conveys *which row is current*. It
   cannot convey *what status a badge carries*. Even under an activedescendant model each row's
   accessible name would still have to contain the word, so the badge fix would be needed anyway.
2. **It would require demolishing the control.** An activedescendant model means one tab stop and a
   roving virtual cursor, which means the per-row `<button class="file-row-main">` and the four
   `.row-action` buttons stop being independently tabbable. That deletes keyboard access to Stage /
   Unstage / Discard / Blame / History and breaks every `getByRole('button', { name: 'Stage …' })`
   assertion in `e2e/04`, `e2e/08`, `e2e/14`, `e2e/15`. The graph earned that pattern because it is a
   20k-row virtualized canvas with no real elements; a file list with ≤ a few thousand real rows and
   five real controls per row has not.

Recorded as a considered-and-rejected alternative, per the brief, not as an untested assumption.

---

## 5. D-C — component decomposition (one new file, six tables deleted)

**Create `src/components/FileStatusBadge.tsx` (~55 lines).** Every one of the 8 sites renders it;
the 5 local `BADGES` tables and `StatusFileRow`'s exported one are deleted.

**Why a new file rather than editing in place** — the reuse rule says extend an existing pattern
first, so this needs a reason. The reason is that **the defect being fixed is drift between six
copies of one table.** Patching eight call sites leaves six copies plus a seventh mapping (letter →
name) to drift against them; the next contributor adding a `FileStatus` variant must find and update
twelve places. A single leaf component makes the letter and its accessible name *physically
inseparable* and makes "add a status" a one-file change. No existing component is a
status-badge-shaped thing to extend: `.file-badge` is a bare span with no owner. Nothing here
approaches the ~500-line limit — the new file is ~55 lines and every touched file **shrinks** (each
loses a 9-line table; net ≈ −25 lines across `src/components`).

**Public shape** (names are load-bearing for §11's residue greps; types only, no bodies):

```
export type BadgeStatus = FileStatus | 'unknown';
const LETTERS: Record<BadgeStatus, string>   // A M D R T C U ?
const LABELS:  Record<BadgeStatus, string>   // §6 strings
export function FileStatusBadge({ status }: { status: BadgeStatus }): JSX.Element
```

Renders exactly: `<span className="file-badge mono" role="img" aria-label={LABELS[status]}>{LETTERS[status]}</span>`.
No state, no effects, no IPC, no props beyond `status`.

**Call-site conversion — all 8, each verified against its current composition:**

| # | Site | Change | Note |
|---|---|---|---|
| S1 | `StatusFileRow.tsx:109` | `<FileStatusBadge status={entry.status} />` | inside the expandable `<button>` |
| S2 | `StatusFileRow.tsx:114` | same | inside the non-expandable `<span>` |
| S3 | `StatusConflictsSection.tsx:124` | `<FileStatusBadge status="conflicted" />` | drop the `BADGES` import from `StatusFileRow` |
| S4 | `DiffFileTree.tsx:181` | `<FileStatusBadge status={file.status} />` | delete local table |
| S5 | `DiffBrowser.tsx:404` | `<FileStatusBadge status={header.status} />` | delete local table |
| S6 | `prPanel/PrFileRow.tsx:37` | `<FileStatusBadge status={header.status} />` | delete local table |
| S7 | `DiffOverlay.tsx:304` | `<FileStatusBadge status={meta.status} />` | **keep the `meta.status !== null &&` guard** — a null status renders *no badge*, unchanged (`DiffOverlay.test.tsx:65` asserts this) |
| S8 | `ComposerGroupCard.tsx:138` | `<FileStatusBadge status={statusByPath.get(path) ?? 'unknown'} />` | replaces `badgeFor()`, which is **deleted**; `rowClassFor()` at `:56` is **kept unchanged** (P106 D1) |

**Do not touch** `entryPaths`, `splitPath`, `RowAction`, or `rowClassFor` — `StatusFileRow.tsx`
remains the owner of the shared row helpers, minus `BADGES`.

**`StatusFileRow.tsx`'s header comment** (`:1-4`) says it owns `BADGES`; update that sentence in the
same edit. A comment that describes a deleted export is the transcription failure mode in miniature.

---

## 6. Microcopy — the exact strings, and the vocabulary audit the brief asked for

| Status | String |
|---|---|
| `added` | `Added` |
| `modified` | `Modified` |
| `deleted` | `Deleted` |
| `renamed` | `Renamed` |
| `typechange` | `Type changed` |
| `conflicted` | `Conflicted` |
| `untracked` | `Untracked` |
| `unknown` | `Status unknown` |

Sentence case, single words where possible, past-participle series so they read as one family in
sequence with the filename: *"Untracked notes/todo.txt"*, *"Type changed scripts/build.sh"*.
`Status unknown` is inverted deliberately — `Unknown notes/todo.txt` would read as *"we don't know
this file"* rather than *"we don't know its status"*.

**Existing vocabulary — reuse, do not invent (the brief's question 4).** Two vocabularies already
exist and they are **not** in conflict; the split is principled and should be preserved:

| Where | Wording | Register |
|---|---|---|
| `DiffOverlay.tsx:34` `KIND_LABEL.untracked` | **`Untracked`** | the state, named as a state |
| `WorktreeCopyCandidates.tsx:28` group label | **`Untracked`** | same |
| `RepoHealthPanel.tsx:266` metric label | **`Untracked`** | same |
| `StatusFileRow.tsx:156` delete tooltip | *"Delete this **new file** (permanently removes it from disk)"* | the **consequence** |
| `DestructiveDialogs.tsx:28` | *"Delete **new file**" / "Delete **new files**"* | the consequence |
| `StatusSection.tsx:145`, `DirRowActions.tsx:32` | *"reverts modified files and deletes **new files**"* | the consequence |

**Rule, and it is worth stating in `ui-reference.md`:** the badge names a **state**, so it takes the
state vocabulary — **`Untracked`**, matching the three places that already name it. "New file" stays
in destructive copy, where the point is not the state but that there is no committed version to
restore. Inventing a third word here, or importing "New file" into the badge, would be the defect.
Also note `Conflicted` (badge, a state) vs the `Conflicts` section header (a bucket) — an existing,
correct distinction; do not harmonize them.

**No other string in the app changes. No new user-visible text is added** — every string in this
contract is invisible to sighted users.

---

## 7. Placement, geometry, themes, densities, states, motion — all unchanged, stated so nobody re-derives

- **Placement.** Unchanged at all 8 sites: first slot of the row, before the path, right panel /
  overlay / dialog as applicable. No new chrome, no header or sidebar cost, nothing displaced.
- **Geometry.** `.file-badge` (`status-panel.css:75`) stays `width: 12px`, `font-size: 11px`,
  `font-weight: 600`, `text-align: center`, `flex: none`. `U` is a single glyph in the same mono face
  as `A`; it fits the 12px box identically. **No CSS diff in this contract.**
- **Both themes.** Ink is unchanged, so P106's whole matrix carries over: `--success-strong` (`A`,
  `U`) MIN **5.71 / 5.94**; `--warning-strong` (`M`, `T`) **5.80 / 5.73**; `--danger-strong` (`D`,
  `C`) **5.27 / 5.18**; `--accent-strong` (`R`) **4.93 / 5.01**; `?` inherits `--text-1`,
  **11.95 / 14.23** on its `--bg-2` backdrop (base stated per §2's BASE rule). Family MIN **5.18**.
  **Do not re-measure** — a letterform swap at identical font, size, weight and colour cannot move a
  contrast ratio.
- **Both densities.** `cozy` rows 24px, `compact` rows 20px; the badge is **11px / 12px wide in
  both** (it does not scale with `--rp-row-font`). Accessible names are density-invariant.
- **States.** Default / hover (`--bg-2`) / selected-expanded (`--selection`) / active
  (`.pr-file-row-active`) — the badge's name and letter are identical in all of them; a status does
  not change because a row is hovered. **Disabled:** rows disable during an in-flight op; the badge
  has no disabled state and must not gain one (P106: the `.55` dim drops `--danger-strong` below the
  bar in light). **Focus:** the badge is never focusable and takes no ring; `role="img"` does not add
  it to the tab order. The row/button keeps its 2px `--accent` `:focus-visible` ring at 1px offset.
  **Loading / empty / error:** the badge does not render; existing `.section-empty` copy unchanged.
  **Overflow:** `flex: none`, one character — never truncates; the path keeps its ellipsis + `title`.
  **Long content:** a 180-char path still ellipsizes beside an intact badge, and the accessible name
  is still the leading token of the row name.
- **Motion.** None added. No transition on any property. `prefers-reduced-motion` unaffected. Nothing
  here can contend with the canvas render budget — it is one attribute per already-rendered span.
- **Command palette.** Nothing to register. This adds no command and no user-invocable action, which
  is the correct outcome for a semantics fix (§restraint: the app already exposes ~150 commands).
- **Destructive-action UX.** Unchanged — but note the fix *serves* it: `U` vs `A` is now the visible
  signal that a row's destructive affordance is 🗑 **Delete** (permanent) rather than ↺ **Discard**
  (restorable). §7.1's `aria-label`s (`Delete {path}` / `Discard changes to {path}`) already spell the
  consequence and are untouched.

---

## 8. Accessibility determination

- **WCAG 4.1.2 (Name, Role, Value)** — the badge gains a role and a name where it had neither.
- **WCAG 1.3.1 (Info and Relationships)** — status now survives conversion to speech as *meaning*,
  not as an uninterpreted glyph.
- **WCAG 1.4.1 (Use of Color)** — was already satisfied by P106 and stays satisfied; `A` → `U`
  strengthens it by making the non-colour channel *distinct per state* rather than merely present.
- **Contrast** — unchanged; §7. No new token, so no new pair to check.
- **Hit targets** — unchanged. The badge is not a target; the row button and the ≥24px `.row-action`
  buttons are (`ui-reference.md` §3.1, §7.1).
- **Colour is never the sole carrier** — reinforced. Post-P109, `added` and `untracked` differ by
  **letter**, and `?` (unknown) differs by both letter and neutral ink.

---

## 9. Harness states (`VITE_MOCK_IPC=1`, `pnpm dev:mock`, port 1420)

All 8 sites are harness-reachable — P106 AC10 proved it per site, with routes, and those routes are
reused here. **No new fixture is required.**

| State | Route / fixture | Status |
|---|---|---|
| `A` added, `U` untracked in the same list | default view; `src/ipc/fixtures/status.ts:41-47` | present — **this is the pair the fix is for** |
| Same untracked file as `U` in the tree/browser | `src/ipc/fixtures/diffs.ts:126,130` (`notes/todo.txt`, `scratch.rs`) | present — **the A/U collision is visible today by switching panels** |
| `M`, `D`, `R` | `src/ipc/fixtures/status.ts` | present |
| `T` typechange | added by P106 AC9 | present |
| `C` conflicted | **`?op=merge`** → `StatusConflictsSection` | present |
| `?` unknown | composer with a path absent from the status snapshot | **verify reachability**; if no fixture produces it, record it as *unverified* verbatim (§2 second failure mode) rather than asserting it renders |
| S5 / S6 / S7 / S8 | diff browser · **`?forge=auth`** · diff overlay · composer | present |
| Light theme, compact density | theme toggle · Settings → right-panel density | present |
| Pathological | the ≥180-char path row added by P106 AC9 | present |

**Verification method for the accessible names is the accessibility tree, not the DOM** — read the
computed name of the row buttons (a browser a11y-tree read or `getByRole('button', { name })`), not
`textContent`, which by design does not change.

---

## 10. Test impact — e2e and unit (the brief's second priority question)

**No e2e spec breaks. Named so the implementer is not surprised.**

- **`rg "file-badge|file-status-|getByText\('[AMDRTCU]'\)" e2e` → 0 matches.** No Playwright spec
  asserts on a badge letter or a badge class.
- **The specs that *do* assert on status rows target the `.row-action` buttons**, whose explicit
  `aria-label`s this contract does not touch:
  `e2e/04-working-dir.spec.ts` (`:30-41`, `:54-59`, `:68-87`, `:115`) ·
  `e2e/08-stash.spec.ts` (`:47-57`) ·
  `e2e/14-destructive-confirms.spec.ts` (`:118-129`) ·
  `e2e/15-error-injection.spec.ts` (`:24-25`) — all `getByRole('button', { name: 'Stage|Unstage|
  Delete|Discard changes to {path}' })`. **Unaffected.**
- **The one spec worth a second look is `e2e/06-merge-conflicts.spec.ts:72,74`** — `Stage resolved`,
  a `ConflictEditor` button, not a badge. Unaffected, listed because it is the only conflict-flow
  assertion and `C` is in scope.
- **`e2e/11-forge.spec.ts` is about canvas-drawn PR/CI badges**, a different family entirely
  (P106's fourth pass: `rg "FileStatus" src/graph` → 0). Unaffected; named because "badge" in a grep
  hits it.

**Unit tests:**

- **`src/components/DiffOverlay.test.tsx:61`** — `screen.getByText('M')`. **Survives**, because
  `role="img"` + `aria-label` leaves `textContent` as `M`. This is a direct argument for D-B over the
  `.sr-only` alternative, which would have broken it.
- **`src/components/DiffOverlay.test.tsx:65`** — "no badge when status is null". **Survives** via the
  preserved `meta.status !== null` guard (S7).
- **`src/components/repoWorkspace/unusableRepoTeardown.wiring.test.tsx:139`** — matches
  `.file-row-main` by `textContent` against `/README\.md/`. **Survives** under either mechanism.
- **`src/components/StatusPanel.test.tsx:204`** — `querySelector('.file-status-conflicted')`.
  **Survives**; row classes are untouched.
- **New coverage owed to `tester`** (not written by this contract): `FileStatusBadge` renders the
  right letter *and* the right name for all 8 `BadgeStatus` values, and `untracked` ≠ `added` in both.

---

## 11. Predicted post-fix grep residue

Per `ui-reference.md` §2: **every baseline below was measured against the real pre-fix tree this
session, not inherited or inferred**, and every row states whether it counts **declarations** or
**raw matches**. Where a row cites `file:line`, the **count is the criterion** — a fix's own comments
shift lines, which is exactly why P106's `DiffOverlay.tsx:295` is now `:304`.

| # | Command | Counts | Now | Predicted after |
|---|---|---|---|---|
| R1 | `rg -n "untracked: 'A'" src/components` | declarations | **3** (`ComposerGroupCard:15`, `DiffOverlay:25`, `StatusFileRow:15`) | **0** |
| R2 | `rg -n "untracked: 'U'" src/components` | declarations | **3** (`DiffBrowser:32`, `DiffFileTree:22`, `PrFileRow:20`) | **1** — the single table in `FileStatusBadge.tsx` |
| R3 | `rg -n "Record<FileStatus, string>" src/components` | declarations | **6** | **0** — the new maps are typed `Record<BadgeStatus, string>` |
| R4 | `rg -n "Record<BadgeStatus, string>" src/components` | declarations | **0** | **2** (`LETTERS`, `LABELS`), both in `FileStatusBadge.tsx` |
| R5 | `rg -n "className=\"file-badge mono\"" src/components` | raw; no comment contains this string today | **8** | **1** — only inside `FileStatusBadge.tsx` |
| R6 | `rg -n "file-badge" src/components` | **raw — inflated by prose** | **10** = 8 JSX + **2 prose comments** (`ComposerGroupCard:54`, `DiffOverlay:296`) | **3** = 1 JSX + the same 2 comments, which stay true (they describe CSS descendant rules that still exist) |
| R7 | `rg -n "<FileStatusBadge" src/components` | raw | **0** | **8** |
| R8 | `rg -n 'role="img"' src` | raw | **0** | **1** |
| R9 | `rg -n "aria-label" src/components/FileStatusBadge.tsx` | declarations | n/a (file absent) | **1** |
| R10 | `rg -n "sr-only" src/components/FileStatusBadge.tsx` | raw | n/a | **0** — records that the `.sr-only` mechanism was rejected (§4.1), so a later pass does not "restore" it |
| R11 | `rg -n "file-badge" src/styles` | declarations | **8** (1 base + 7 status rules) | **8** — **zero CSS diff** |
| R12 | `rg -n "file-status-" src/components` (excl. `*.test.*`) | raw | **8** | **8** — row classes untouched; P106 D1 stays |
| R13 | `rg -n "color:\s*var\(--(danger\|success\|warning)\)" src/styles/status-panel.css` | declarations | **0** (P106 AC2) | **0** — P109 must not reopen P106's file |
| R14 | `rg -n "badgeFor" src/components` | raw | **3** (`ComposerGroupCard:47,49,138`) | **0** |

**Predicted line-count effect:** `FileStatusBadge.tsx` **+~55**; six tables and `badgeFor` removed
**−~60**; net ≈ **−5** across `src/components`, and every touched file gets smaller. No file
approaches the ~500-line limit as a result of this change.

**Trap, called out so review catches it (third failure mode, P106 §10):** the implementer must not
write the literal strings `untracked: 'A'`, `className="file-badge mono"` or `role="img"` inside its
own new comments. Doing so inflates R1, R5 and R8 and makes a correct fix look failed.

---

## 12. Acceptance criteria

**Predicted residue is stated in §11 up front; a mismatch is a finding, not a rounding error.**

1. **AC1 — The component exists and is the only place a letter is chosen.** `src/components/FileStatusBadge.tsx`
   exists, exports `BadgeStatus` and `FileStatusBadge`, and holds exactly two maps. Residue
   **R2 = 1**, **R3 = 0**, **R4 = 2**, **R5 = 1**.
2. **AC2 — All 8 render sites use it.** Residue **R7 = 8**, **R6 = 3** (raw; the 2 prose matches are
   named in §11 and expected). No local `BADGES` table survives anywhere. Residue **R14 = 0**.
3. **AC3 — `untracked` renders `U` at all 8 sites.** Residue **R1 = 0**. Confirmed in the *rendered*
   harness, not by grep: the same fixture file (`notes/todo.txt`) shows `U` in the status panel **and**
   `U` in the diff file tree. A declaration may not be recorded as fixed on grep evidence alone
   (`ui-reference.md` §2, second failure mode).
4. **AC4 — Every status has the §6 accessible name.** Read from the **accessibility tree** for all 8
   `BadgeStatus` values: `Added`, `Modified`, `Deleted`, `Renamed`, `Type changed`, `Conflicted`,
   `Untracked`, `Status unknown`. Residue **R8 = 1**, **R9 = 1**.
5. **AC5 — The name folds into the row's name.** In the harness, the S1 row button for
   `notes/todo.txt` computes the accessible name **`Untracked notes/todo.txt`** (not `A notes/todo.txt`,
   not `Untracked U notes/todo.txt`). Recorded per site for S1, S3, S4, S6 — the four interactive
   wrappers — with the route that reaches each, per P106 AC10's precedent.
6. **AC6 — `added` and `untracked` are distinguishable on both channels.** A staged `A` row and an
   untracked `U` row are visible in one screenshot; their accessible names differ. Neither relies on
   hue — both are `--success-strong`, deliberately (§3, P4c).
7. **AC7 — `textContent` is unchanged.** `DiffOverlay.test.tsx:61` (`getByText('M')`) and `:65`
   (no badge when status is null) pass **unmodified**. If either test had to be edited, D-B was
   implemented wrongly (§4.1).
8. **AC8 — Zero CSS, token and geometry diff.** `src/styles/**` is byte-identical. Residue
   **R11 = 8**, **R13 = 0**, **R12 = 8**. The badge is 12px wide / 11px / 600 in **both** densities;
   rows stay 24px cozy, 20px compact.
9. **AC9 — Contrast is carried over, not re-derived.** State in the implementation report that ink is
   unchanged and P106's family MIN **5.18** stands, with bases (`ui-reference.md` §2 BASE rule). Do
   **not** spend a harness pass re-measuring a letterform swap.
10. **AC10 — `?` unknown is either confirmed rendering or carried forward as `unverified`.** The word
    **`unverified`** appears verbatim in the report and in `ui-reference.md` §7 if the composer's
    unknown-status path is not reachable from a fixture. No aggregate "all statuses render" sentence
    satisfies this AC (§2, second failure mode).
11. **AC11 — No e2e edit.** `e2e/**` is byte-identical, and the specs named in §10 pass unchanged.
    An e2e diff in this increment means the row-action labels were disturbed.
12. **AC12 — `ui-reference.md` is updated in the same pass.** §7's status table gains the **letter**
    and **accessible name** columns with `untracked = U`; the "`added`/`untracked` both render `A`"
    line at `:1197-1199` and the cross-reference at `:330` are replaced by the closed record; §11's
    naming recipe gains the generalization *"prefer a visually-hidden span; use `role="img"` +
    `aria-label` when the visible text **is** the token being renamed — it replaces rather than
    prefixes, avoiding a 'Untracked U' stutter"*; §7 gains the **state-vs-consequence vocabulary rule**
    from §6 (`Untracked` names the state, `new file` names the consequence); and P109 is recorded as
    closed **except** for AC13/AC14. Any `unverified` from AC10 is carried **verbatim**.
13. **AC13 — USER CHECKPOINT (pending).** §13.
14. **AC14 — USER CHECKPOINT (pending).** §13.

AC1–AC12 are AI-gate verifiable in the browser harness plus `rg`.

---

## 13. USER CHECKPOINT items — these stay **pending**; no agent may self-declare one

The user's checkpoint authority does not reach this work, and no agent message closes these.

- **AC13 — screen-reader read-through (native window + a real AT).** `pnpm tauri dev` on a real repo
  with staged, unstaged, untracked and conflicted files: arrow through the Changes and Staged lists
  with NVDA (Windows) or VoiceOver (macOS) and confirm (a) each row announces
  *"{Status} {path}, button"* and never a bare letter, (b) `Untracked` and `Added` are audibly
  distinct, (c) the word does not make a long list feel slower to scan, and (d) the browse-mode
  "graphic" prefix on the three non-interactive sites (§4.3) is acceptable rather than irritating.
  **The headless harness cannot judge this** — it has no AT, and `requestAnimationFrame` does not fire.
- **AC14 — the `U` glyph read at 11px in the native window.** Confirm `U` is instantly
  distinguishable from `A` and from the other six glyphs at arm's length, in **both themes** and
  **both densities**, at real subpixel/gamma rendering; and that a green `U` still reads as
  *new/added-ish* rather than as an unrelated state. Screen rendering of 11px mono is not judgeable
  from the harness, and this is the one perceptual risk the letter change introduces.

---

## 14. Flagged for the orchestrator

1. **D-A is a user-visible change to a glyph users may have learned.** My recommendation is `U`
   (§3), and I hold it: the app already renders `U` at half its sites, the design system already
   calls the family A/M/D/**U**/R, and `A` is the mapping that lies to a git-fluent user. If you
   decline, the *only* coherent alternative is to change the three `U` tables to `A` and take an
   accessible-name-only fix — **do not ship the 3/3 split with names layered on top**, because that
   is two states, one glyph, in the same panel, permanently. AC3 and AC6 are the ACs that move; AC1,
   AC2, AC4, AC5, AC7–AC12 are unaffected either way.
2. **D-C creates one new file and deletes six duplicated tables.** This is slightly more than the
   minimum edit (the minimum is 8 attribute additions + 3 letter changes in place). I recommend the
   component because the defect *is* six-way drift. If you want the truly minimal diff, drop the
   component and patch in place — but then R2/R3/R4/R7/R14 all change and the drift returns the next
   time a `FileStatus` variant is added. My recommendation is the component.
3. **`?` unknown may be unreachable in the harness** (AC10). It is a real code path
   (`ComposerGroupCard.tsx:49`), not dead. If no fixture produces it, the honest outcome is the word
   `unverified` carried into `ui-reference.md`, not a fixture invented solely to prove a fallback —
   though adding one path to a composer fixture is cheap if `senior-dev` finds it trivial. Your call;
   my mild preference is to add it.
4. **`ui-reference.md` §7 is factually wrong today and I corrected the record in this pass** — its
   table said `untracked | A` while its own family name said `U`, and it did not know three tables
   ship `U`. That correction (a record fix, not a proposal) landed with this contract; the *forward*
   changes are AC12 and land with the implementation.
5. **P106's `status-panel.css` must not be reopened** (R13). If any P107/P108 follow-up is in flight
   on that file, P109 is independent of it — this contract touches no stylesheet at all.
