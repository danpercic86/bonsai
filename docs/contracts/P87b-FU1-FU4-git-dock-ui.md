# P87b FU-1 + FU-4 — the git-activity dock: run targets, and one toggle rule for both docks

**Owner:** ui-designer · **Written:** 2026-09-03 · **Status:** implemented + design-reviewed 2026-09-10
**Review corrections applied 2026-09-10** (this file is the spec of record; the review is in the
orchestrator transcript): §3.4 noun weight, §3.7 preposition-map wording + the running-hook
de-duplication rule + the three rulings the implementer had to invent, §3.10 long-target literal,
§4 file table (the §3.5 block now lives in `git-dock-log.css`), §5 two new flags.
**Parent:** `docs/contracts/archive/P87-ui.md` (§3.3, §3.4, §5, §6, §8) ·
`docs/contracts/ui-reference.md` §9, §12.10
**Follow-ups closed:** FU-1 (`docs/history/todo-archive-2026-09.md:613`), FU-4 (`:618`)
**Depends on:** `docs/contracts/P111-pill-truncation-ui.md` §3 — `RefLabel` is the renderer used here.
**Needs the architect:** §3.6 lists exactly what `GitActivityRun.target` must carry. Nothing else in
this contract is blocked on it; §2 (FU-4) can ship alone.

---

## 0. Premise check (read both docks before designing to the brief)

| Claim in the brief | Verdict |
|---|---|
| FU-4: "the two docks disagree" | **DOES NOT HOLD.** They agree exactly. `GitActivityHeader.tsx:40-50` and `AiActivityHeader.tsx:81-91` are the same nine lines: a `<button class="*-dock-toggle">` holding one `aria-hidden` chevron, with `aria-expanded`, `aria-controls`, `aria-label` and a `title`; identical CSS (`git-dock.css:146-163`, `ai-dock.css:125-142`). Both toggle on the glyph only. **The disagreement is contract-vs-code, in one dock's contract.** |
| FU-4: "…or the contract's §5-1 wording disagrees with what shipped" | **HOLDS.** `archive/P87-ui.md:319-320` reads "click anywhere on it (or its chevron) to expand". Nothing implements that. The AI dock's own contract never claimed it (`P68e-ai-activity-dock.md:133` and `ui-reference.md` §9/§14 line 247 describe the chevron button only), so only P87-ui is wrong. |
| FU-1: "`GitActivityRun` has no `target`/ref field" | **HOLDS.** `src/components/repoWorkspace/gitActivityState.ts:26-46` — no target. `GitActivityEvent` (`src/ipc/types/activity.ts:49-68`) has no target either. Both headers and rows render `categoryMeta().noun` alone (`GitActivityHeader.tsx:57`, `GitActivityRow.tsx:103`), so every push in a session is an indistinguishable row reading "Push". |
| FU-1: "render per §3.4" | §3.4 (`archive/P87-ui.md:221-223`) specced `→ <upstream>`. §3.2 below **revises** it — the arrow is dropped. Reason stated there. |

---

## 1. FU-4 — decision

### 1.1 Decision: **both docks keep the chevron-only toggle. The §5-1 wording changes.**

The brief offered two options and asked me not to leave the docks inconsistent. They are already
consistent; the reconciliation is therefore a documentation fix, and whole-bar is the option to
reject. Three reasons, in order of weight:

1. **A stretched hit target would kill six live tooltips.** The AI dock's collapsed bar is a dense
   readout in which almost every element owns a `title` that is the *only* way to recover truncated
   or unexplained information: `.ai-dock-subject` (`AiActivityHeader.tsx:100`, the full path behind an
   ellipsis — the exact "truncated name must stay recoverable" case P111 exists to protect),
   `.ai-dock-activity` (:112), `.ai-dock-turn` (:118), `.ai-dock-elapsed` (:125),
   `.ai-dock-thinking` (:133), `.ai-dock-cost` (:139). Any overlay that makes the bar clickable sits
   above those spans and suppresses their native tooltips. Raising each span back above the overlay
   removes the overlay's reason to exist. The git dock has the same shape at `.git-dock-detail`
   (`GitActivityHeader.tsx:65`).
2. **The bar carries primary and near-destructive actions.** `Review`, `Answer`, `Cancel` and
   `Dismiss` (`AiActivityHeader.tsx:145+`) and `Clear` (`GitActivityHeader.tsx:75-83`) live on these
   bars. Putting a full-width toggle surface under a `Cancel` button means a 3px miss cancels
   nothing and instead resizes the workspace.
3. **The chevron is not the discovery path and never was.** §5-2 (command palette: `Git activity` /
   the AI dock's twin) and §5-3 (the clickable toolbar phase readout) are, and both shipped. The
   chevron is already **28×28 cozy / 24×24 compact** — at or above the §3.1 floor — and carries a
   `title` naming the action.

**If the orchestrator overrules this**, whole-bar is still implementable without nested interactive
elements; the complete markup is in Appendix A so the call can be made without another round-trip.
It is a real spec, not a strawman — but it costs the six tooltips.

### 1.2 What changes

**Nothing in `.tsx`. Nothing in `.css`.** Two documentation edits, both applied in this pass:

- `docs/contracts/archive/P87-ui.md` §5, entry 1 — replaced (see §1.3).
- `docs/contracts/ui-reference.md` §9 — one new bullet making the rule canonical for both docks, so
  the next dock inherits it.

### 1.3 The exact replacement wording for `archive/P87-ui.md:319-320`

> 1. **The collapse chevron on the dock bar** — the bar itself is always visible once the first op
>    ran; its leading `⌃`/`⌄` button expands and collapses the log. The button is the *only* toggle
>    target: the rest of the bar is a readout whose spans own `title` tooltips, and it carries
>    `Clear` (and, in the AI dock, `Review`/`Answer`/`Cancel`/`Dismiss`), so a bar-wide click surface
>    would suppress those tooltips and sit under those actions. Discovery is carried by entry points
>    2 and 3 below, not by the bar. *(Revised 2026-09-03, FU-4: the original text read "click
>    anywhere on it (or its chevron) to expand" and never shipped in either dock — see
>    `docs/contracts/P87b-FU1-FU4-git-dock-ui.md` §1.)*

### 1.4 The rule, stated once for both docks

Recorded in `ui-reference.md` §9:

> **A collapsed dock bar toggles from its chevron button, not from the bar.** Both docks (`.ai-dock`,
> `.git-activity-dock`) put exactly one `<button>` in the a11y tree for collapse — the leading
> `.*-dock-toggle`, `aria-expanded` + `aria-controls` on the button, glyph `aria-hidden`, box at
> `--*-dock-ctl-h` (28 cozy / 24 compact). The bar's remaining spans stay non-interactive so their
> `title` tooltips survive, and the bar's other buttons stay siblings, never descendants. Discovery
> of the dock is the command palette's job, plus the clickable toolbar phase readout.

---

## 2. FU-4 — states, keyboard, a11y (unchanged, restated so the rule is checkable)

| State | `.git-dock-toggle` / `.ai-dock-toggle` |
|---|---|
| Default | 28×28 (cozy) / 24×24 (compact), transparent, glyph `--*-dock-meta` (= `--text-2`), radius 6px |
| Hover | `background: var(--bg-2)` — unchanged, and it now correctly marks the whole clickable area |
| Active | none; the collapse is instantaneous |
| `:focus-visible` | global 2px `--accent`, 1px offset (`tokens-and-base.css:225-228`) around the 28/24px box |
| Disabled | never |
| Collapsed | glyph `⌃`, `aria-expanded="false"`, `title` "Show git activity" / "Show AI output" |
| Expanded | glyph `⌄`, `aria-expanded="true"`, `title` "Hide git activity" / "Hide AI output" |

Keyboard: `Tab` reaches the toggle first in each bar (DOM order = focus order); `Enter`/`Space` are
the native button activation; `Esc` inside the expanded git dock collapses it
(`GitActivityDock.tsx:136-141`) and `Ctrl/Cmd+Shift+L` / `Ctrl/Cmd+Shift+A` toggle from anywhere.
No change to any of it.

---

## 3. FU-1 — the run target

### 3.1 Placement

Immediately after the category noun, in both surfaces. Nothing else moves; the status pill keeps its
`margin-left: auto` right-hug.

```
collapsed dock bar (GitActivityHeader), running
┌──────────────────────────────────────────────────────────────────────────────────┐
│ ⌃  ⤴ Push  origin/main   ● Running  · Sending objects…        2.4s      Clear     │
└──────────────────────────────────────────────────────────────────────────────────┘
      ^glyph ^noun ^target  ^pill      ^detail                  ^elapsed

run row (GitActivityRow), terminal
  ▸  ⤴ Push  origin/main                       ⋯ trimmed   ✓ Success   1.2s   14:32
  ▸  ⤵ Fetch  all remotes                                  ✓ Success   0.8s   14:31
  ▸  ● Commit  feature/api/…/retry-budget                  ✓ Success   0.2s   14:30
                       ^ leading segments ellipsize, the leaf never does (P111 R3)
```

### 3.2 Treatment — no arrow (revises `archive/P87-ui.md` §3.4)

§3.4 specced `→ origin/main`. Dropped, for three reasons: the noun already carries direction (`Push`
vs `Fetch`), so the arrow is a redundant second carrier; `→` before `←` for fetch/pull would make the
direction a *glyph* distinction where a *word* distinction already exists, which is the inverse of the
§11 rule; and screen readers verbalize `→` inconsistently. The target instead takes the app's ref
treatment — **mono, `--text-2`** — against the noun's **UI font, 600 weight, `--text-1`**. That is
exactly the AI dock's `pill · .ai-dock-subject.mono` shape, so the two bars read the same.

### 3.3 The string table (these are the literal strings)

A single pure formatter in `src/components/gitActivityFormat.ts` — the file whose header already
states that every human string is derived in the frontend:

```
/** §3.3 — the row/bar target text, or null for "this run has no target worth showing". */
export function runTarget(run: GitActivityRun): string | null
```

| Category | `run.target` from the backend | `runTarget()` returns | The row reads |
|---|---|---|---|
| `push` | `"origin/main"` | `origin/main` | `Push origin/main` |
| `push` (new branch, no upstream yet) | `"origin/feature/x"` | `origin/feature/x` | `Push origin/feature/x` |
| `push` (remote known, branch not) | `"origin"` | `origin` | `Push origin` |
| `forcePush` | `"origin/main"` | `origin/main` | `Force-push origin/main` |
| `pull` | `"origin/main"` | `origin/main` | `Pull origin/main` |
| `fetch` (one remote) | `"origin"` | `origin` | `Fetch origin` |
| `fetch` (all remotes) | `null` | `all remotes` | `Fetch all remotes` |
| `commit` | `"main"` | `main` | `Commit main` |
| `commit` (detached / unborn HEAD) | `null` | `null` | `Commit` |
| `amend` | `"main"` | `main` | `Amend main` |
| `mergeCommit` | `"main"` | `main` | `Merge commit main` |
| any | field absent (old event, mock without the knob) | `null` | the noun alone |

Rules the table encodes:

- **`all remotes` is derived in the frontend**, from `category === 'fetch' && target == null`. The
  backend must not send a human string. This is why `target?: string | null` is sufficient and no
  discriminant field is needed (§3.6).
- **No placeholder.** A missing target renders *nothing* — never `(none)`, `—`, `unknown`, or an
  empty pill. The row is still meaningful; a placeholder would claim a fact.
- **No quoting, no arrow, no prefix.** The bare ref.
- Identical in the collapsed bar and the expanded row. **The two surfaces do not differ** — the same
  formatter, the same class, the same truncation. (`GitActivityHeader` and `GitActivityRow` are
  separate files; both call `runTarget`.)

### 3.4 Rendering + CSS

Both files render the target with the P111 component, so a long ref keeps its leaf:

```
{target !== null && <RefLabel value={target} className="git-run-target" withTitle />}
```
```
{target !== null && <RefLabel value={target} className="git-dock-target" withTitle />}
```

New CSS in `src/styles/git-dock.css`, beside the existing `.git-run-subphase` block:

```css
/* FU-1 — the run's target ref. Identity, so it outranks the transient phase text
   when the dock is narrow (subphase shrinks 3× faster). 22ch matches .toolbar-phase;
   `ch` resolves against this rule's own 11px mono, so one value serves both densities. */
.git-run-target,
.git-dock-target {
  flex: 0 1 auto;
  min-width: 0;
  max-width: 22ch;
  font-family: var(--font-mono);
  font-size: 11px;
  color: var(--git-dock-meta, var(--text-2));
}
```

and one line added to the existing `.git-run-subphase` and `.git-dock-detail` rules:

```css
  flex-shrink: 3;   /* NEW — the phase yields width before the target does */
```

and — **added 2026-09-10 by review** — one line on the run row's noun, so the two surfaces treat it
identically:

```css
.git-run-noun {
  font-weight: 600;   /* NEW — .git-dock-noun already has it; §3.2 assumes both do */
}
```

This is a deliberate revision of `archive/P87-ui.md` §3.4, which specced only "**noun** in
`--text-1`" and left the weight unstated, so the row shipped at regular. Three reasons: §3.2's
noun-vs-target contrast (UI 600 `--text-1` against mono `--text-2`) is the whole mechanism that keeps
the target reading as metadata, and it was specced for both surfaces; §3.3 requires the bar and the
row to be identical, and a 600 bar noun beside a regular row noun is the one place they differ; and
without it the row's hierarchy is **inverted** — `.git-run-pill` is `font-weight: 600` at 11px while
the noun is regular, so the status chip is the boldest text in a row whose subject is the noun. In
**compact** both noun and target are 11px, which leaves weight as the only remaining carrier.

`.ref-label`'s own `overflow: hidden; white-space: nowrap` (P111 §3.3) does the truncating;
`withTitle` puts the whole ref on the span's `title`.

**Densities.** Cozy: header 30px, row `--git-dock-row-h` 28px, gap 8px, pad-x 12px. Compact: 28px,
26px, 6px, 8px. The target adds **no height in either** — 11px mono on a ≥26px row — and takes one
`--git-dock-gap` of horizontal space. `--git-dock-label-font` (12/11px) is deliberately *not* used:
the target is metadata, sized with `.git-run-subphase`/`.git-run-duration` at a flat 11px, so the
noun stays the loudest thing in the row in both densities.

**Themes.** `--git-dock-meta` is an alias of `--text-2`, the same pair `.git-run-subphase`,
`.git-run-duration` and `.git-run-time` already use on `--git-dock-bg` (= `--bg-1`). **No new token,
no new colour pair, so no new contrast measurement is owed** — this reuses a pair already recorded in
`ui-reference.md` §2 and shipped in this exact rule block.

### 3.5 States

| State | Collapsed bar | Run row |
|---|---|---|
| Default (target present) | noun + target | noun + target |
| Default (no target) | noun alone, nothing shifts | noun alone |
| Running | target renders **before** the phase detail and does not change for the life of the run (§3.6-2) | same; the subphase yields width first |
| Success / failed | unchanged; the target is status-independent | unchanged |
| Hover | `.git-run-summary:hover` wash unchanged; the target's `title` shows | same |
| `:focus-visible` | n/a (non-interactive span) | the ring stays on `.git-run-summary` |
| Loading / empty dock | the empty state (`No git activity yet.`) is unchanged — it has no run and so no target | — |
| Error | a failed run still shows its target; the failure is carried by the pill and the log | — |
| Long content | leading segments ellipsize at 22ch, leaf intact, full ref on `title` and in the row's accessible name |

### 3.6 What the architect must supply (commission this half without a round-trip)

`target?: string | null` on **both** `GitActivityEvent` and `GitActivityRun` **is the right type and
is sufficient.** It is not sufficient *unqualified* — the rendering above depends on four content
guarantees, and each one is a rendering bug if it is not held:

1. **It is a raw git identifier, never a formatted phrase.** No arrow, no quotes, no preposition, no
   `"all remotes"`. Exactly one of: `"<remote>/<branch>"` for push/forcePush/pull; `"<remote>"` for a
   single-remote fetch, or a push whose branch could not be resolved; the branch **short name**
   (`main`, `feature/x` — not `refs/heads/main`) for commit/amend/mergeCommit; `null` for fetch-all,
   detached HEAD, and unborn HEAD. If the backend sends a sentence, §3.3's `all remotes` derivation
   and the `splitPath` truncation both break.
2. **It is set on the `started` event and is immutable for the run.** A target that appears or
   changes mid-run reflows the collapsed bar while the user is reading it and makes the announcer
   (§3.7) contradict itself. Set once, at `started`.
3. **It is sanitized like output lines.** Branch names are repo-controlled and can carry bidi
   overrides and zero-width characters; a bidi override in a ref visually reorders the whole dock row
   around it. Run it through the same funnel as `activity_line` (C0/C1 + bidi + `U+200B–200D/FEFF`,
   the P87a M1/L1 stripper). It already renders as a React text node, so this is a spoofing concern,
   not an injection one — but it is a real one.
4. **It is length-capped at the boundary — 255 characters.** The frontend truncates *visually*; the
   store must not be able to hold a megabyte ref across a 200-run ring. Git's own practical ref-name
   bound is far under this, so 255 never truncates a real name; if it ever does, truncate at the
   backend and the UI's `title` will simply show the capped value.

Nothing else is needed. In particular **do not** add a `targetKind` discriminant, a `remote`/`branch`
pair, or a `direction` field — the category already discriminates, and splitting the ref would force
the frontend to reassemble a string the backend already had.

### 3.7 Accessibility

- **Row accessible name.** FU-3 is already queued to move `role="button"` + `aria-expanded` off the
  role-less `<div>` onto `.git-run-summary`. **Do FU-1 and FU-3 in the same increment**, because the
  row's name must be built explicitly once the target is two spans — otherwise name computation joins
  siblings and can emit `origin/feature/ retry-budget` (the `Git config , repository` failure recorded
  in `ui-reference.md` §11). Add a second pure formatter:

  ```
  /** The run row's accessible name — the visible row, in words, in reading order. */
  export function runRowName(run: GitActivityRun, now: number): string
  ```

  producing, for the rows drawn in §3.1:

  | Run | `aria-label` |
  |---|---|
  | push, success | `Push to origin/main — success, 1.2 seconds, 14:32` |
  | fetch-all, success | `Fetch from all remotes — success, 0.8 seconds, 14:31` |
  | commit on a branch | `Commit on feature/api/retry-budget — success, 0.2 seconds, 14:30` |
  | push, running | `Push to origin/main — running, sending objects, 2.4 seconds` |
  | commit, detached HEAD | `Commit — success, 0.2 seconds, 14:30` |
  | failed push with a blocking hook | `Push to origin/main — failed, 0.9 seconds, 14:29` |
  | push, running a hook | `Push to origin/main — running, pre-push hook, 0.4 seconds` |
  | push, running, `preparing` | `Push to origin/main — running, preparing, 0.1 seconds` |
  | push, running, unknown phase | `Push to origin/main — running, working, 0.1 seconds` |
  | push, success, over a minute | `Push to origin/main — success, 2 minutes 5 seconds, 14:32` |

  Prepositions come from **one exhaustive `Record<GitActivityCategory, 'to' | 'from' | 'on'>` — 7
  keys, 3 values** (the 2026-09-03 wording said "3-entry map", which was wrong: there are seven
  categories and three prepositions, and the map must be keyed by category so a new category cannot
  silently miss one). push/forcePush → `to`, fetch/pull → `from`, commit/amend/mergeCommit → `on`.
  Used by this function **and** by the announcer, nowhere else. They exist only in the accessible
  name; the visible row stays arrow-free and preposition-free (§3.2).

  Four rules the six-row table above does not state on its own — **all four resolved 2026-09-10**:

  1. **De-duplicate `running`.** The status word for a running row is the pill's (`running`), and
     three phase labels also begin with it, which produced `— running, running pre-push hook,`.
     The phase clause therefore **drops a leading `running `** after the `…` is stripped and the
     label lowercased: `Running pre-push hook…` → `pre-push hook`. Sentence-initial otherwise
     unchanged (`Sending objects…` → `sending objects`). Only the accessible name is affected: the
     **visible** bar reads `● Running · Running pre-push hook…` and gets away with it because the
     pill is a chip and the phase is muted text two type steps apart — a linearized name has no such
     chunking, so it must not repeat the word.
  2. **A running row names its phase, never its progress.** The name carries
     `phaseLabel()`, not `objectsReadout()` — the visible sub-line's `12,340 / 50,000 objects` moves
     on every `progress` event (many per second), and an `aria-label` that rewrites that fast is
     unusable. The live elapsed **is** included, and is safe: `tick` is a **1 s** interval that only
     runs while something is running (`useGitActivity.ts:229`).
  3. **The `⋯ trimmed` chip is not in the name.** The row is `role="button"` with an explicit
     `aria-label`, so the chip and its `title` are already outside the a11y tree — repeating the
     chip in the name would be the only way AT ever heard it, and the fact is recoverable in the
     right place: expanding the row exposes `↑ N earlier lines trimmed` as the log's first line.
     §3.1's wireframe keeps the chip visible; the name omits it, by design.
  4. **Elapsed is spelled out in words** — `1.2 seconds`, `2 minutes 5 seconds`, singular at 1 —
     because `durationLabel`'s `2:05` reads as a clock time. Under a minute keeps one decimal
     (`1.0 seconds` is accepted: it is a measurement, not a count).
- **`⋯ trimmed` and the pills** keep their §11 treatment; the pill word is already in the row name
  via `— success`, so the pill glyph stays `aria-hidden`.
- **Announcer.** `sentenceFor()` (`gitActivityFormat.ts:211-218`) gains the target for terminal
  results only: `Push to origin/main finished — success`, `Push to origin/main failed`. Phase
  transitions stay bare (`Sending objects`) — repeating the target on every phase change is the
  hostile-verbosity failure §6 of the parent contract already forbids. When `runTarget()` is `null`
  the sentence is unchanged from today.
- **Hit targets, focus, colour.** Untouched. The target adds no interactive element and no colour
  carrier.

### 3.8 Motion

None. The target must not fade or slide in — it is present from `started` and the dock's no-animation
rule (`ui-reference.md` §9: only opacity/colour, ≤150ms, and never height) already covers it.
`prefers-reduced-motion` needs no new entry.

### 3.9 Microcopy

Every visible string is in §3.3; every accessible string is in §3.7. Two more:

- The target's `title` is **the bare full ref** — not `Target: origin/main`. It exists to recover
  truncated text, and §11's "a `title` must not merely repeat the visible label" does not apply when
  the visible label is elided.
- No error string is added. A run that fails still shows its target; the failure copy is the pill and
  the existing `This hook blocked the <op>.` note.

### 3.10 Harness states (`VITE_MOCK_IPC=1`)

`src/ipc/mock/gitActivity.ts` drives runs through `runMockActivity(category, fn)`; it must gain the
target (signature is the architect's call — `runMockActivity(category, target, fn)` is the obvious
shape). New query seams, in the file's existing `?flag` idiom:

| Seam | Fixture | Proves |
|---|---|---|
| default push / commit | `target: 'origin/main'` / `'main'` | the common case in bar + row |
| `?fetchAll` | fetch with `target: null` | `Fetch all remotes` — the derived string. **Named no-op:** fetch-all is the only fetch entry point, so the default already passes `null` and the derived phrase is on screen without the flag. The seam exists so the case has a name; a future per-remote fetch is what makes it load-bearing. |
| `?gitNoTarget` | commit with `target: null` | the no-placeholder rule; nothing shifts |
| `?gitLongTarget` | push with `origin/feature/very-long-experimental-branch/with-many-nested-path-segments/retry-budget-tuning` (**95 chars**, leaf `retry-budget-tuning`) | 22ch ellipsis, leaf intact, `title` recovers, row height unchanged, and the running row's subphase yields first |
| `?gitBidiTarget` | a ref containing `U+202E` | §3.6-3 — **must render inert**; if it reorders the row, the sanitizer is missing |
| existing `?pushSlow` | + a target | the target does not change mid-run (§3.6-2) |

All six are visible in the browser harness in **both themes** and **both densities** (toggle
`panelDensity`). **No USER CHECKPOINT item** in this contract.

> **Corrected 2026-09-10.** The literal written here on 2026-09-03 was an elided illustration
> (`…/retry-budget-with-a-90-character-name`) that measures **83 characters** — it fails this
> table's own "≥90 chars" requirement. The 95-char string above is the fixture that shipped
> (`MOCK_LONG_TARGET`), and it is the correct one: its leaf is 19 chars, so the leaf fits inside the
> 22ch box while the head still needs ~4× the available width — exactly the P111 R3 case. The
> architect's `P87b-FU1-run-target.md` §8 still carries the 83-char literal; that is not my file, so
> it is flagged in §5 (F-G) rather than edited.

---

## 4. Component decomposition

No new component file for FU-1. Both consumers are already small and presentational, and both stay
well under the limit:

| File | Change | Size after |
|---|---|---|
| `src/components/gitActivityFormat.ts` | `+ runTarget()`, `+ runRowName()`, `+` the preposition map, announcer edit | ~218 → ~275 |
| `src/components/GitActivityHeader.tsx` | one `<RefLabel>` after the noun | 86 → ~90 |
| `src/components/GitActivityRow.tsx` | one `<RefLabel>` after the noun; `aria-label={runRowName(...)}` on `.git-run-summary` (with FU-3) | 205 → ~212 |
| `src/components/repoWorkspace/gitActivityState.ts` | `target` field (architect) | +2 |
| `src/styles/git-dock.css` | one new rule + two `flex-shrink` lines + the noun weight | 493 → **429** (see the split below) |
| `src/styles/git-dock-log.css` | **new** — the §3.5 expanded-detail block, moved verbatim | ~95 |
| `src/components/RefLabel.tsx` | **new**, owned by P111 §3.2 | ~45 |

`GitActivityDock.tsx` is untouched. No addition goes into a large file.

**The CSS split (approved 2026-09-10, not in the original spec).** `git-dock.css` was at 493 lines,
so the `+14` above would have pushed it to 507 and tripped the size ratchet. The implementer moved
the `/* expanded detail (§3.5) */` block — `.git-run-detail`, `.git-run-hooks`, `.git-run-hook*`,
`.git-run-output`, `.git-run-copy`, `.git-run-log`, `.git-log-empty`, `.git-log-line`, the `stderr`
variant and `.git-run-note` — into `src/styles/git-dock-log.css`, imported immediately after
`git-dock.css` (`styles.css:58-59`). **This is the right seam:** it is the same cut the AI dock
already makes (`ai-dock.css` + `ai-dock-log.css`, `styles.css:56-57`), it is a *log-surface* seam
rather than an arbitrary halving, both log files stay adjacent and after their bar file so
`.git-run-log`'s override of `.ai-log` is unaffected, and the shared status-pill rules (which name
`.git-run-hook-pill`) correctly stayed with the other pills. **Cascade:** safe by property, not only
by order — no moved rule declares `outline`, and the one rule now sequenced before them,
`.git-activity-dock :focus-visible`, wins on specificity (0,2,0 vs 0,1,0) regardless. Keep the two
imports adjacent and in this order.

---

## 5. Flags for the orchestrator

**Counterpart list:** the architect's flags for this same increment are in
`docs/contracts/P87b-FU1-run-target.md` §10 (F-1…F-5). The two lists cover one increment from two
sides and must be read together. **When they disagree, check the tree and correct the wrong one in
place** — do not leave both standing (that is exactly how F-E below survived long enough to mislead
two agents).

- **F-C (sequencing).** FU-1 needs the architect's `target` before it can render anything real, but
  **FU-3 must land with it, not after** (§3.7) — the row's accessible name has to be built explicitly
  the moment the target becomes two spans. Recommend commissioning FU-1 + FU-3 as one senior-dev pass,
  and FU-4 (two doc edits, already applied here) as nothing at all.
- **F-D (P111 dependency).** `RefLabel` comes from `P111-pill-truncation-ui.md`. If P111 is deferred,
  FU-1 can ship with plain `white-space: nowrap; overflow: hidden; text-overflow: ellipsis` on
  `.git-run-target` and a `title` — correct, just tail-clipping. Say so rather than inlining a second
  copy of the split.
- **F-F (mock fidelity, follow-up, low).** Two gaps found in review, neither blocking:
  (a) the mock call sites pass literal targets (`'main'`, `'origin/main'`) regardless of the mock
  repo's HEAD — §8 of the run-target contract specified literals, so this is per spec, but a
  `detached`/`unborn` fixture would render `Commit main`, which claims a fact. Fix when a
  detached/unborn mock fixture exists: derive from the fixture HEAD and pass `null` when there is no
  branch. (b) **WITHDRAWN 2026-09-10 — this half was false.** It claimed the mock wrapped
  `commitAmend` while the backend did not, and recommended "fix the backend (FU-2)". It rested
  entirely on F-E, which was wrong (see below). Both halves wrap amend, so the harness's
  `Amend main` row is exactly what the app produces. **Do not unwrap the mock and do not open a
  FU-2 backend task** — acting on the original text would have removed working coverage.
- **F-G (RESOLVED 2026-09-10 by the orchestrator — the flag was addressed to it, and is answered).**
  It read: `P87b-FU1-run-target.md` §8 still carries the 83-char `?gitLongTarget` literal that §3.10
  corrects to the 95-char shipped fixture. The architect has since corrected §8 to the shipped
  95-char string, with a dated note quoting the prior literal. **Both contracts now agree with the
  code.** Nothing to do; kept as a record of the disagreement and its resolution.
- **F-E (CORRECTED 2026-09-10 — the original claim was false; kept as a record, not as a task).**
  The original text asserted that `commitAmend` was not activity-wrapped, so an amend produced no
  dock row and therefore no target, and filed that as an out-of-scope backend gap "FU-2".
  **That was wrong at the time it was written and is wrong now.** Verified at HEAD:
  - `src-tauri/src/commands/staging.rs:171-191` — `commit_amend_inner` resolves
    `activity_target(state, repo_id, GitActivityCategory::Amend)` (l.178) and runs the whole
    operation inside `with_activity(state.git_activity_hub(), GitActivityCategory::Amend, target,
    …)` (l.179).
  - The mock mirrors it: `src/ipc/mock/handlers/stash.ts:148` —
    `runMockActivity('amend', 'main', …)`.
  - `src/ipc/mock/handlers/amendActivity.test.tsx` covers the amend row.
  - Since FU-1 (`1d8c6f9`) amend resolves a target like every other category, which is why §3.4's
    table (l.160) lists `amend → main → "Amend main"` with no exception.

  The architect refuted this as **F-4** in `docs/contracts/P87b-FU1-run-target.md` §10 ("ui-designer's
  F-E is stale"), but the refutation was never carried back here, so both statements stood and
  readers picked up whichever they saw first — it was restated as a real gap twice, including by
  F-F(b) above. **There is no FU-2 backend wrapping gap.** Nothing to implement from this flag.

---

## Appendix A — the whole-bar option, fully specced (REJECTED; implement only if overruled)

Whole-bar clickability **without** nested interactive elements uses the stretched-target pattern: the
existing toggle button stays the only `<button>`, and a transparent pseudo-element of that button
covers the bar. The other controls are *siblings*, not descendants, so there is no nested-interactive
problem in the a11y tree — they only need to be raised above the overlay.

```css
/* Container is the positioning context; the toggle stays position: static so its
   ::after resolves against the bar, not against the 28px button box. */
.git-dock-header { position: relative; }

/* Hit surface: transparent, covers the whole bar, belongs to the toggle button. */
.git-dock-toggle::after {
  content: '';
  position: absolute;
  inset: 0;
}

/* Hover wash for the enlarged target (§3.1: an enlarged target must announce
   itself). z-index:-1 paints it above the bar's background and below its text. */
.git-dock-toggle::before {
  content: '';
  position: absolute;
  inset: 0;
  z-index: -1;
}
.git-dock-toggle:hover::before { background: var(--bg-2); }
.git-dock-toggle:hover { background: transparent; }  /* the 28px-only wash is replaced */

/* The bar's real controls stay on top and keep their own click + tooltip. */
.git-dock-clear { position: relative; z-index: 1; }
```

The AI dock takes the identical block with `.ai-dock-header` / `.ai-dock-toggle` and
`.ai-dock-review, .ai-dock-answer, .ai-dock-cancel, .ai-dock-dismiss { position: relative; z-index: 1; }`.

Markup: **unchanged in both docks.** No `onClick` on a `<div>`, no `role="button"` on a container, no
element nested inside another interactive element, one `aria-expanded` in the tree. Focus stays on the
28/24px chevron box (the outline follows the button's border box, not the pseudo-element), which is
correct — a bar-wide focus ring would be a worse affordance.

**The price, stated so the trade is explicit:** every non-raised span on the bar loses its native
tooltip, because the overlay takes the hover — `.ai-dock-subject`, `.ai-dock-activity`,
`.ai-dock-turn`, `.ai-dock-elapsed`, `.ai-dock-thinking`, `.ai-dock-cost`, `.git-dock-detail`, and the
new `.git-run-target`/`.git-dock-target` `title` from §3.4. Raising them back with
`position: relative; z-index: 1` restores the tooltips and re-opens holes in the click surface, so the
bar becomes clickable only in its gaps — worse than either endpoint. Text selection on the bar is also
lost. That is why §1.1 rejects this.
