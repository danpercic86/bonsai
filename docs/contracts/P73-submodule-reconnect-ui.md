# P73 — Submodule init/update fixes — UI contract

Owner: `ui-designer`. Scope: the submodule row surface only (sidebar row + badge, row context menu,
toast copy, pending state). No new panel, no new dialog, no new top-level chrome.

**Input contracts:** `docs/contracts/P19-submodules.md` (§6.1–§6.5 — the surface being amended),
`docs/contracts/ui-reference.md` (§2 tokens + contrast rules, §3 spacing/density, §7 "never colour
alone", §8 loading/error, §9 pill recipe), `TODO.md` §P73 (confirmed diagnosis).

**Sources read (verified, not guessed):** `src/components/Sidebar.tsx` (`SUBMODULE_BADGE` 302-308,
`SubmoduleRow` 310-335, Submodules section 845-878, `actionsDisabled` 446, props 73-76),
`src/components/workspaceMenus.ts` (`submoduleMenuItems` 407-457, `worktreeMenuItems` 462-498 —
the Lock…/Unlock precedent), `src/components/repoWorkspace/useSubmoduleActions.ts` (all 6
handlers), `src/components/Toasts.tsx`, `src/components/ContextMenu.tsx` (menu roles 202-224),
`src/components/OpBanner.tsx`, `src/components/WorkspaceToolbar.tsx:142-166,242`
(`Fetching…`/`.header-progress` precedent), `src/utils/errors.ts`,
`src/ipc/mock/handlers/submodules.ts`, `src/ipc/fixtures/submodules.ts`, `src/styles.css`
(`.branch-row` 4328-4342, `.branch-badge` 4381-4386, `.submodule-badge-*` 4388-4411, `.toast*`
596-654, `.header-progress` 1320-1338, reduced-motion block 7678-7683),
`scripts/file-size-baseline.json` (Sidebar.tsx 918, workspaceMenus.ts 803).

**Locked upstream decision (user, 2026-08-19):** the Init menu action now invokes
`ipc.updateSubmodule` (= `sm.update(init: true, …)`), i.e. register config **then** fetch + check
out. No fifth wire status is added. Everything below follows from that.

---

## 1. The defect in UI terms

The old Init wrote `.git/config` and nothing else, then toasted `Initialized <name>` while the row
badge still read `not initialized`. Two lies in one interaction: a success toast for an operation
the user could not see the result of, and a verb ("Init") whose everyday reading ("make this
submodule usable") did not match its effect. P73 makes the action match the verb, and this contract
makes the words match the action.

---

## 2. Component decomposition + file paths

`Sidebar.tsx` is **918 lines** and `workspaceMenus.ts` is **803** — both over the ~500-line soft
limit. Nothing may be *appended* to either.

| File | Action | Why |
|---|---|---|
| `src/components/sidebar/SubmoduleRow.tsx` | **CREATE** (~70 lines) | Move `SUBMODULE_BADGE` (Sidebar.tsx:302-308) and `SubmoduleRow` (310-335) here verbatim, then apply §4/§5. Net **removes ~34 lines** from `Sidebar.tsx`. First file in a new `src/components/sidebar/` folder — the natural home for the remaining row components when someone next splits the sidebar. |
| `src/components/sidebar/SubmoduleRow.test.tsx` | **CREATE** | Badge label/title/busy assertions (§9). |
| `src/components/Sidebar.tsx` | **EDIT, net shrink** | Delete the moved code, import the new file, add the one `submoduleBusy` prop (§6) and pass it through. No new render markup. |
| `src/components/workspaceMenus.ts` | **EDIT, net shrink** | `submoduleMenuItems` only: relabel one item, change two `disabled` expressions, shorten the comment block. Line count goes **down**. No extraction needed — this is a modification, not growth. |
| `src/components/repoWorkspace/useSubmoduleActions.ts` | **EDIT** (111 lines, well under limit) | `handleInitSubmodule` calls `ipc.updateSubmodule`; all six handlers gain the shared `submoduleBusy` set/clear and the §5 error wrapper. |
| `src/styles.css` | **EDIT** | One new class `.submodule-badge-busy`, one colour change on `.submodule-badge-muted`, one line in the existing `prefers-reduced-motion` block. |
| `src/ipc/fixtures/submodules.ts` | **EDIT** | One pathological row (§8). |
| `src/ipc/mock/handlers/submodules.ts` | **EDIT** | Error + slow seams (§8). |

**No new component is invented.** Everything reuses `.branch-row`, `.branch-badge`, the
`submodule-badge-*` intents, `ContextMenu`, `Toasts`, and `.header-progress`.

---

## 3. Context menu — labels and enablement

### 3.1 Judgment call: keep two items, rename Init

**Recommendation: keep Init and Update as two separate items; rename Init to
`Initialize and check out`; make Update *disabled* while the row is uninitialized** (the exact
inverse of Init's gate).

Reasoning:

1. **The house precedent is a mutually-exclusive pair, not a merge.** `worktreeMenuItems`
   (`workspaceMenus.ts:479-487`) ships `Lock…` and `Unlock` as two items where exactly one is live
   per row state. Submodules get the same shape: on an uninitialized row only Init is live; on any
   other row only Update is live. That removes the actual source of confusion — the old menu offered
   *two enabled items that did the same thing* on an initialized row and two items with different
   promises on an uninitialized one.
2. **Discoverability beats brevity here.** The badge the user is staring at says "not checked
   out" and the git verb they know is "init". A single item labelled only `Update` would leave a
   user with an empty submodule folder hunting for the word they came for. Merging also breaks the
   1:1 map between menu items and the two things the docs, tests, and CLI call init and update.
3. **The label must now name both effects.** "Init" reads as a cheap local config write; this
   action fetches over the network and writes files to disk. `Initialize and check out` states both
   halves in five words and is the honest fix for the reported complaint. Rejected
   `Initialize` alone (same under-promise as "Init") and `Init and update` (jargon-on-jargon).
4. **No ellipsis.** House rule, visible in this very menu: `…` means a dialog follows
   (`Deinitialize…`, `Remove…`, `Lock…`). Init and Update act immediately, so no ellipsis.

Rejected alternative: **one item with a status-dependent label** (`Initialize and check out` when
uninitialized, `Update` otherwise). It is defensible and shorter, but a menu whose item *identity*
changes between rows is harder to document, harder to test by name, and makes the item's position
in the list carry meaning the user cannot see. Not worth it against the Lock/Unlock precedent.

### 3.2 Exact spec — every item in `submoduleMenuItems(sub)`

`const gate = mutating || opActive;` is unchanged. `const uninit = sub.status === 'uninitialized';`

| # | `label` | `icon` | `disabled` | `tone` | Change |
|---|---|---|---|---|---|
| 1 | `'Initialize and check out'` | `BranchIcon` | `gate \|\| !uninit` | — | **label changed** (was `'Init'`); gate unchanged |
| 2 | `'Update'` | `StashApplyIcon` | `gate \|\| uninit` | — | **gate changed** (was `gate`) |
| 3 | `'Sync'` | `RebaseIcon` | `gate` | — | untouched |
| 4 | `'Deinitialize…'` | `ResetIcon` | `gate \|\| uninit` | — | untouched (same expression, via `uninit`) |
| 5 | `'Remove…'` | `DeleteIcon` | `gate` | `'danger'` | untouched |
| 6 | `'Open in new tab'` | `CompareIcon` | `uninit` | — | untouched |
| 7-9 | `...externalToolsItems(sub.absPath, extHandlers)` | — | never gated | — | untouched |

Order is unchanged: the two mutually-exclusive worktree-populating actions first, then Sync, then
the two destructive `…` actions, then navigation, then the external-tools trio. Item 5 stays the
only `tone: 'danger'` entry — one destructive emphasis per menu.

Replace the stale comment at `workspaceMenus.ts:407-410` with:

```
// P19 §6.4 + P73 §3.2: submodule row menu. "Initialize and check out" and
// "Update" are the same backend call (sm.update(init:true)) presented as a
// mutually-exclusive pair — exactly one is live per row state, mirroring the
// Lock…/Unlock pair below. Deinit is a no-op once uninitialized; open-in-tab
// needs files on disk.
```

### 3.3 Command palette

Submodule actions stay **out** of the command palette (they are row-scoped and need a target;
`paletteActions.ts` has no submodule entries today). Unchanged by P73 — noted so nobody adds them
as a "completeness" gesture. The row context menu is the single entry point.

---

## 4. Judgment call: badge wording

**Recommendation: `uninitialized → 'not checked out'`, with an explanatory `title`.**

```
uninitialized:   label 'not checked out'   intent 'submodule-badge-muted'
                 title 'No files on disk yet. Right-click the row → Initialize and check out.'
upToDate:        label 'up to date'        intent 'submodule-badge-ok'
                 title 'Files on disk match the commit the superproject pins.'
outOfSync:       label 'out of sync'       intent 'submodule-badge-warn'
                 title 'Checked out at a different commit than the superproject pins. Update to fix.'
modifiedWorkdir: label 'modified'          intent 'submodule-badge-warn'
                 title 'Uncommitted changes inside this submodule.'
```

Why `not checked out` over `not initialized`:

- It names **the observable fact** (no files in the folder) rather than an internal git bookkeeping
  step. It is true for both sub-cases the status now covers — never registered, and registered in
  `.git/config` with an empty worktree — which is exactly the ambiguity that made the old pairing
  of "Initialized …" + "not initialized" read as a bug.
- It matches the menu item that fixes it (`Initialize and check out`), so badge and remedy share a
  word.
- It is the same length class as the old string (15 vs 14 chars) — no sidebar layout risk at 240px,
  and it still ellipsizes correctly under `.branch-name`'s `min-width: 0` when the name is long.

The current `title={badge.label}` (`Sidebar.tsx:330`) duplicates the visible text and is dead
weight for both sighted and screen-reader users; the table above replaces it. Titles are the only
place the *why* lives — do not put them in the row's visible text.

Wire status names (`uninitialized` etc.) are unchanged; this is display copy only.

---

## 5. Toast copy

All toasts go through the existing `pushToast(tone, text, key?)` / `Toasts.tsx` — success
auto-dismisses at 5 s, `error` is sticky with `role="alert"`. No new toast tone, no new component.

`name` below is `sub.name` verbatim (in practice the submodule path).

### 5.1 Success

| Action | Tone | Text |
|---|---|---|
| Initialize and check out | `success` | `` `Checked out ${name}` `` |
| Update | `success` | `` `Updated ${name}` `` |
| Sync | `success` | `` `Synced URL for ${name}` `` (unchanged) |
| Deinitialize | `success` | `` `Deinitialized ${name}` `` (unchanged) |
| Remove | `success` | `` `Removed ${name}` `` (unchanged) |

`Checked out ${name}` replaces `Initialized ${name}`. It names the effect the user can now verify
in the folder and in the badge, which is the whole point of the milestone — and it can no longer be
true while the badge says otherwise, because the same call produced both.

### 5.2 Failure — one shape, two prefixes

```
Initialize:  pushToast('error', `Couldn't check out ${name}. ${errorMessage(e)}`, `submodule:${name}`)
Update:      pushToast('error', `Couldn't update ${name}. ${errorMessage(e)}`,   `submodule:${name}`)
Sync:        pushToast('error', `Couldn't sync ${name}. ${errorMessage(e)}`,     `submodule:${name}`)
Deinit:      pushToast('error', `Couldn't deinitialize ${name}. ${errorMessage(e)}`, `submodule:${name}`)
Remove:      pushToast('error', `Couldn't remove ${name}. ${errorMessage(e)}`,   `submodule:${name}`)
```

Rules:

- **Prefix names the action and the target; the backend sentence says what to do next.** The
  frontend does **not** switch on `AppError.kind`. `errorMessage(e)` is appended verbatim — that is
  how `authFailed` / `networkError` keep their existing copy without the UI duplicating remote-error
  vocabulary (the same division `useRemoteActions` and every other hook already uses).
- **Dedupe key `submodule:${name}`** (P70 §10.1 mechanism, `Toast.key`). Repeatedly pressing a
  failing action on one row replaces its sticky toast in place instead of stacking five identical
  alerts. Two different submodules still get two toasts.
- **Requirement on the backend contract (cross-contract, flag to architect):** the two new
  refusals must arrive as `AppError { kind: 'git' }` with a **complete, capitalised, period-
  terminated, user-ready sentence** — no libgit2 prose, no absolute `.git/modules` paths, no
  "attempt to reinitialize". The UI will surface them verbatim, so they are UI copy that happens to
  live in Rust. Required strings:

  | Refusal | Required `AppError.message` |
  |---|---|
  | Worktree not empty | `The folder already has files in it. Move or delete everything inside <relative path>, then try again.` |
  | Cached gitdir belongs to another URL | `Bonsai has cached data for a different remote URL. Run Sync on this submodule, then try again.` |

  Resulting toasts (the reported case):

  ```
  Couldn't check out src/Hamilton.Voyager.Protocol/protocol. The folder already has
  files in it. Move or delete everything inside src/Hamilton.Voyager.Protocol/protocol,
  then try again.

  Couldn't check out src/Hamilton.Voyager.Protocol/protocol. Bonsai has cached data for
  a different remote URL. Run Sync on this submodule, then try again.
  ```

  Both name the exact target and the exact next step. Neither asks the user to understand
  `.git/modules`. If the architect's message wording differs, **the architect's file wins for the
  wire and this table is amended** — but the four properties (complete sentence, capitalised,
  period-terminated, no raw libgit2 text) are non-negotiable UI requirements.
- Neither refusal is destructive and neither is recoverable by pressing again, so **no confirm
  dialog and no undo affordance** is warranted — the toast's "then try again" is the whole recovery
  path. (Deinit and Remove keep their existing `ConfirmDialog`s untouched.)

### 5.3 Path length — no truncation

**Do not truncate the path in toast copy.** `.toast` is a 360px column with
`overflow-wrap: anywhere` (`styles.css:604,614`) and no line clamp, so a long path wraps and stays
fully readable; the toast grows downward inside a `flex-column` stack that is not height-capped.
The user needs the complete path to know *which* submodule and *which* folder to empty — a middle
ellipsis would destroy exactly the segment they must act on. The real-world path
(`src/Hamilton.Voyager.Protocol/protocol`, 38 chars) fits on two lines; the §8 pathological
fixture (91 chars, as implemented) wraps and is the harness check that this holds.

Corollary: no path-truncation helper is introduced (none exists for DOM today —
`graph/textMeasure.ts:truncateToWidth` is canvas-only and must not be pulled into React).

---

## 6. Pending / busy state

Init is now a network fetch plus a worktree checkout — potentially tens of seconds on a large
submodule. Today the only feedback is the global `setMutating(true)`, which greys distant controls
and says nothing at the place the user clicked. Three additions, all reusing existing patterns.

**Pattern being reused:** `WorkspaceToolbar.tsx:142-166,242` — a long remote op swaps its control's
label to a present participle (`Fetching…`, `Pulling…`, `Pushing…`) and renders the existing 2px
`.header-progress` sweep. Text-first, no spinner, reduced-motion-safe by construction.

### 6.1 Row-local busy badge (primary affordance)

New optional `Sidebar` prop, threaded to `SubmoduleRow`:

```ts
/** P73 §6: the submodule with an op in flight, and the present-participle label
 *  its badge shows meanwhile. null ⇒ no submodule op running. */
submoduleBusy: { name: string; label: string } | null;
```

`useSubmoduleActions` owns it: every handler sets `{ name, label }` beside its existing
`setMutating(true)` and clears it to `null` in the same `finally`.

| Handler | `label` |
|---|---|
| `handleInitSubmodule` | `'checking out…'` |
| `handleUpdateSubmodule` | `'updating…'` |
| `handleSyncSubmodule` | `'syncing…'` |
| `handleDeinitSubmodule` | `'deinitializing…'` |
| `handleRemoveSubmodule` | `'removing…'` |
| `handleAddSubmodule` | *(none — there is no row yet; the section's `+` button already reads disabled)* |

`SubmoduleRow` when `submoduleBusy?.name === sub.name`:

- The badge `<span>` renders `submoduleBusy.label` with class `submodule-badge-busy` and
  `title={undefined}` (the label is already the whole message).
- The `<li>` gets `aria-busy="true"`.
- The `<li>` keeps its context-menu handler (the menu opens; items 1-5 are already disabled by
  `gate`, so right-clicking during an op shows why nothing is actionable rather than swallowing the
  gesture). Items 6-9 stay live — opening a tab or a terminal during a fetch is harmless.
- Lowercase, present participle, trailing `…` (U+2026) — matches the existing badge casing
  (`up to date`, `out of sync`) and the toolbar's participle convention.

No layout shift: the badge is `flex: none` inside a fixed **24px** `.branch-row`, and the longest
label (`deinitializing…`) is shorter than the longest steady-state label the row already survives.

### 6.2 Global sweep

`WorkspaceToolbar` gains one optional prop:

```ts
/** P73 §6.2: a non-remote background op (submodule init/update/sync) is running —
 *  drives the 2px sweep only; the remote buttons keep their own `remoteOp` labels. */
netBusy?: boolean;
```

Line 242 becomes `{(remoteOp !== null || refreshing || netBusy === true) && <div className="header-progress" aria-hidden="true" />}`.
`RepoWorkspace` passes `netBusy={submoduleBusy !== null}`. No button label changes.

### 6.3 Reduced motion (in scope, one line)

`.header-progress::after` animates unconditionally today — the known gap recorded as P68e §12-F3.
Since P73 is the first milestone to point that sweep at a multi-second operation, close it by adding
to the **existing** block at `styles.css:7678`:

```css
  .header-progress::after {
    animation: none;
    width: 100%;
    opacity: 0.6;
  }
```

Identical treatment to `.ai-dock-progress` (7679-7683). No new keyframes, no new token.

### 6.4 No announcement, no banner

- The busy state is **not** an `aria-live` region — a per-keystroke-free 30-second op produces one
  start and one end, and the completion toast (`aria-live="polite"` on `.toast-stack`) is the
  announcement. `aria-busy` on the row is the semantic marker. This mirrors §9 of `ui-reference.md`
  (streaming surfaces are not live regions; status transitions announce separately).
- **No `OpBanner` arm.** `OpBanner` is for *paused repository states the user must resolve*
  (merge/rebase/pick/bisect), not for in-flight operations. A submodule fetch is a transient async
  call like fetch/pull, and those use the sweep. Adding a banner arm would also mean a new
  `RepoOpState` variant — a wire change for a transient state, which the architect's contract does
  not carry. Do not.
- **No cancel affordance.** No submodule IPC command is cancellable today; a Cancel button that
  cannot cancel is worse than none. Flagged as a possible follow-up, not P73.

---

## 7. Tokens, themes, densities, a11y

### 7.1 New CSS class (no new token)

```css
/* P73 §6.1: in-flight submodule op. Word-only — no hue, because "busy" is not a
   verdict; the verdict arrives as the toast + the settled badge. */
.submodule-badge-busy {
  font-family: inherit;
  padding: 1px 6px;
  border-radius: 8px;
  color: var(--text-2);
  background: color-mix(in srgb, var(--text-2) 12%, transparent);
}
```

Add `.submodule-badge-busy` to the shared selector list at `styles.css:4390-4396` instead of
repeating `font-family`/`padding`/`border-radius`.

**Zero new custom properties.** `--text-2` only. One rule set serves both themes.

### 7.2 Required contrast fix on the muted badge

`.submodule-badge-muted` uses `--text-3`, which `ui-reference.md` §2 restricts to **decorative**
use. "not checked out" is load-bearing text — the user reads it to decide whether to act. Change:

```css
.submodule-badge-muted {
  color: var(--text-2);
  background: color-mix(in srgb, var(--text-2) 12%, transparent);
}
```

Measured (this pass, 2026-08-19), `--text-2` on its own 12% tint over `--bg-1`:
**5.79:1 dark / 6.22:1 light** — clears AA. Previously `--text-3` was ~3.4:1 / ~3.0:1.
This also lifts the worktree `main` badge (`Sidebar.tsx:342`), which shares the class — an
improvement, and the only collateral effect.

### 7.3 Measured pairs for every badge state (both themes)

| Class | Text/bg | Dark | Light | Verdict |
|---|---|---|---|---|
| `.submodule-badge-busy` (new) | `--text-2` on 12% self-tint over `--bg-1` | **5.79:1** | **6.22:1** | AA text ✓ |
| `.submodule-badge-muted` (fixed) | same | **5.79:1** | **6.22:1** | AA text ✓ |
| `.submodule-badge-ok` | `--success` on 12% self-tint over `--bg-1` | **4.76:1** | **4.06:1** | dark ✓ / **light 0.44 short** |
| `.submodule-badge-warn` | `--warning` on 12% self-tint over `--bg-1` | ~5.4:1 | **3.94:1** | dark ✓ / **light 0.56 short** |
| `.toast-error` text | `--danger` on 14% self-tint over `--bg-2` | **3.34:1** | **3.49:1** | **both short** |
| `.toast-success` text | `--success` on 14% self-tint over `--bg-2` | **4.07:1** | *not measured* | **dark short** |

The last four are **pre-existing, app-wide, and not caused by P73** — the same class of shortfall
already recorded in `ui-reference.md` §2 for `.toast-warning`. P73 must not *add* to them (the two
badges it introduces/repairs clear AA), and it records them. Two optional fixes below; both are the
orchestrator's call, not silently in scope.

**OPT-1 (recommended) — adopt the §9 pill recipe for the `ok`/`warn` badges.** Label goes to
`--text-1`, hue moves to a 40% border plus an `aria-hidden` glyph over the existing 14% tint —
exactly what the AI-dock status pills do and what §2's `--warning`-as-text rule already mandates.
Visible result: `✓ up to date`, `⚠ out of sync`, `⚠ modified`. Cost: ~10 lines of CSS + one glyph
field in the badge map. Benefit: AA in both themes, and colour stops being even a partial carrier
(§7 of `ui-reference.md`). Also touches the worktree badges, which share the classes.

**OPT-2 (defer) — `.toast-error`/`.toast-success`/`.toast-warning` text to `--text-1`,** keeping the
hue in the border and adding a leading `⚠`/`✓` glyph. Correct, but it restyles every toast in the
app and belongs in its own pass — P73's error copy is readable, just under-contrasted. Recorded in
`ui-reference.md` §2 so it is not re-discovered a third time.

### 7.4 Density

`ui-reference.md` §3: `panelDensity` scope is the right panel and the AI dock only. The sidebar has
**one geometry in both densities** — `.branch-row` `height: 24px`, `gap: 6px`, `padding: 0 4px`,
`border-radius: 4px`; badge `padding: 1px 6px`, `border-radius: 8px`, 11px. Identical in `cozy` and
`compact`. The 24px row is exactly the AA hit-target floor; the busy badge must not add vertical
padding.

### 7.5 States matrix (`SubmoduleRow`)

| State | Presentation |
|---|---|
| default | `.branch-row`, `⊡` glyph `--text-3`, name `--text-1`, badge per §4 |
| hover | `background: var(--bg-2)` (existing `.branch-row:hover`) |
| active/pressed | none — the row is not a button; right-click opens the menu |
| `:focus-visible` | rows are not tab stops today (unchanged by P73); focus lives in the `ContextMenu`, which already ships 2px `--accent` rings |
| busy | badge → `.submodule-badge-busy` + label; `aria-busy="true"`; hover unchanged |
| disabled | n/a for the row; menu items carry `aria-disabled` (`ContextMenu.tsx:221`) |
| loading (list) | unchanged — the section renders after `listSubmodules` resolves |
| empty | unchanged — `<p class="branch-muted">No submodules</p>` |
| error | unchanged — failures toast; the section keeps its last good list |
| long content | name truncates via `.branch-name` ellipsis with `title={sub.path}`; badge is `flex: none` and never compresses |

### 7.6 Keyboard & screen reader

- Menu: unchanged `role="menu"` / `role="menuitem"` / `aria-disabled` / `tabIndex={-1}` roving
  focus from `ContextMenu.tsx`. Item 1's accessible name becomes "Initialize and check out" — the
  only a11y-visible change, and an improvement (the old "Init" was an abbreviation announced as a
  word).
- Because Init and Update are now mutually exclusive, arrow-key traversal always lands on exactly
  one enabled worktree-populating item. Disabled items remain focusable-and-announced (correct —
  discoverable, not silently absent).
- Toasts: unchanged. Success announces politely via `.toast-stack[aria-live="polite"]`; errors are
  `role="alert"` and sticky, so the two new refusal sentences are read out in full including the
  "then try again" remedy.
- No colour-only meaning anywhere in §4-§6: every badge state is a word, and the busy state is a
  word plus `aria-busy`.

### 7.7 Motion

Nothing new. The only animation in scope is the pre-existing `.header-progress` sweep (transform
only, 1.1s, `aria-hidden`, absolutely positioned so it triggers no reflow of `.panes` and cannot
contend with the canvas). §6.3 makes it reduced-motion-safe. Badge label swaps are instant — a
cross-fade on a 24px row would read as a glitch.

---

## 8. Harness states (`pnpm dev:mock`)

The mock is where the whole of P73's UI is verifiable — no native window needed for §3-§7.

### 8.1 The mock's init behaviour is now CORRECT — do not "fix" it

`src/ipc/mock/handlers/submodules.ts:27-36` flips `uninitialized → upToDate` on `initSubmodule`.
That was **wrong before P73** (it hid Bug 1 — the real backend left the row uninitialized) and is
**right after P73**, because Init means init + checkout. Additionally, `handleInitSubmodule` now
calls `ipc.updateSubmodule`, so the UI exercises `updateSubmodule` (lines 38-47, which already
flips to `upToDate`) and never reaches `initSubmodule` at all. Keep the `initSubmodule` handler as
is for IPC-surface completeness and add this comment above it:

```ts
// P73: init means init + CHECKOUT, so flipping to upToDate is the intended
// semantics — this is no longer a mock/backend divergence. Note the UI no
// longer calls this command; handleInitSubmodule invokes updateSubmodule.
```

### 8.2 Fixture — one pathological row

Append to `seedSubmodules` (`src/ipc/fixtures/submodules.ts`) a fifth row reproducing the reported
case at worst-case length, so overflow is verifiable in every surface at once (row ellipsis, badge
position, toast wrapping):

```ts
{
  name: 'src/Hamilton.Voyager.Protocol/protocol/vendor/third-party/generated-openapi-client-bindings',
  path: <same>,
  absPath: `/mock/repo/${<same>}`,
  url: 'https://dev.azure.com/example/_git/Hamilton.Voyager.Protocol.Generated.Client.Bindings',
  headOid: fixtureOid(6), indexOid: fixtureOid(6), wtOid: null,
  status: 'uninitialized',
}
```

### 8.3 Error + slow seams

Extend `failSeam` into a `submoduleSeam(id)` reading `query('submodule')`, and **call it from
`initSubmodule` and `updateSubmodule`** (today neither does — the error path of the two commands
P73 changes is currently unreachable in the harness, which is its own defect):

| `?submodule=` | Behaviour |
|---|---|
| `fail` | existing: `{ kind: 'git', message: 'Mock: submodule operation failed' }` (message gains a trailing period so §5.2's composed sentence is well-formed) |
| `notEmpty` | `{ kind: 'git', message: 'The folder already has files in it. Move or delete everything inside <path>, then try again.' }` with `<path>` = the requested submodule path |
| `urlMismatch` | `{ kind: 'git', message: 'Bonsai has cached data for a different remote URL. Run Sync on this submodule, then try again.' }` |
| `auth` | `{ kind: 'authFailed', message: 'Authentication failed for https://dev.azure.com/example/_git/protocol.' }` — proves §5.2 leaves remote copy alone |
| `slow` | `await delay(4000)` before succeeding — the only way to observe the §6 busy badge and the sweep |

`#fail` in the name keeps working.

---

## 9. Acceptance criteria (harness-verifiable)

Run `pnpm dev:mock`, default repo, Submodules section expanded.

**Badge copy (§4)**
1. The `vendor/libcore` row's badge text is exactly `not checked out`.
2. Its `title` is exactly `No files on disk yet. Right-click the row → Initialize and check out.`
3. `vendor/theme` reads `up to date`; `docs/spec` reads `out of sync`; `tools/ci` reads `modified`;
   each has the §4 `title`, and no badge's `title` equals its own visible text.

**Menu (§3.2)**
4. Right-click `vendor/libcore` (uninitialized): item 1's accessible name is
   `Initialize and check out` and it is **enabled**; `Update` is **disabled** (`aria-disabled="true"`);
   `Deinitialize…` disabled; `Open in new tab` disabled; `Sync`, `Remove…` and the three external-
   tool items enabled.
5. Right-click `vendor/theme` (upToDate): `Initialize and check out` is **disabled**; `Update`
   **enabled**; `Deinitialize…` enabled; `Open in new tab` enabled.
6. No menu item's label is `Init`. Exactly one item carries `tone: 'danger'` (`Remove…`).

**Success path (§5.1)**
7. Invoke `Initialize and check out` on `vendor/libcore` → a `success` toast reading exactly
   `Checked out vendor/libcore`, and after the refetch the row's badge reads `up to date`.
   Badge and toast agree — this is the Bug-1 regression test.
8. Invoke `Update` on `docs/spec` → toast `Updated docs/spec`, badge `up to date`.

**Failure path (§5.2, §8.3)**
9. With `?submodule=notEmpty`, `Initialize and check out` on `vendor/libcore` produces a sticky
   `role="alert"` toast reading exactly:
   `Couldn't check out vendor/libcore. The folder already has files in it. Move or delete everything inside vendor/libcore, then try again.`
   and the badge still reads `not checked out`.
10. With `?submodule=urlMismatch`, the same action produces:
    `Couldn't check out vendor/libcore. Bonsai has cached data for a different remote URL. Run Sync on this submodule, then try again.`
11. With `?submodule=auth`, the toast is
    `Couldn't check out vendor/libcore. Authentication failed for https://dev.azure.com/example/_git/protocol.` —
    the remote copy passes through untouched.
12. Pressing the failing action three times leaves **exactly one** `.toast-error` in the DOM
    (dedupe key `submodule:vendor/libcore`); failing on a second row makes it two.
13. No toast anywhere contains the substring `attempt to reinitialize` or `.git/modules`.

**Busy state (§6)**
14. With `?submodule=slow`, during the op: the acted-on row's badge reads `checking out…` with class
    `submodule-badge-busy`, the `<li>` has `aria-busy="true"`, `.header-progress` is in the DOM, and
    the other four rows' badges are unchanged. All clear within one refetch of completion.
15. `Update` on a slow row shows `updating…`; `Sync` shows `syncing…`.
16. Right-clicking the busy row still opens the menu, with items 1-5 disabled and 6-9 enabled.

**Overflow, themes, density (§5.3, §7)**
17. The §8.2 long row: name is ellipsized, `title` is the full path, the badge is fully visible and
    not clipped at 240px sidebar width, and the row is still 24px tall.
18. Its failure toast wraps to multiple lines inside the 360px toast with **no** ellipsis and no
    horizontal overflow of `.toast-stack`.
19. All of 1-18 hold in `[data-theme='light']` and in both `panelDensity` values, with identical
    sidebar row geometry (24px) in both densities.
20. `getComputedStyle` on the busy and muted badges resolves to the `--text-2` value in both
    themes; no hardcoded hex appears in the P73 diff.

**USER CHECKPOINT (not AI-verifiable)**
- The real reconnect against `D:\Repos\ham-digi-backend` — Init on the wedged
  `src/Hamilton.Voyager.Protocol/protocol` row genuinely populating the folder, and the two
  refusals firing from real backend conditions rather than the mock seam.
- Perceived responsiveness of the busy badge over a multi-second real fetch (the harness is
  headless; `requestAnimationFrame` does not fire, so the sweep's smoothness cannot be judged here).

---

## 10. Flagged for the orchestrator

1. **OPT-1 / OPT-2 (§7.3)** — the pre-existing light-theme AA shortfall on the `ok`/`warn` badges
   and the both-theme shortfall on toast text. My recommendation: take **OPT-1 in P73** (10 lines,
   it is the exact surface being edited, and it makes the badges word+glyph per §9); **defer OPT-2**
   to a dedicated toast pass. Recorded in `ui-reference.md` §2 either way. Needs a yes/no.
2. **Backend message wording (§5.2)** — the two refusal sentences are UI copy that must live in
   Rust. Please route the table in §5.2 to the architect / senior-dev as a requirement on
   `AppError::Git`, and tell me if they diverge so I amend this file rather than the code drifting.
3. **No cancel for a multi-second submodule fetch (§6.4)** — accepted for P73 (no cancellable IPC
   exists). Worth a roadmap line if real submodules turn out to be slow enough to matter.
4. **`ipc.initSubmodule` becomes unused by the frontend** (§8.1). I have specified keeping the
   command and its mock handler. If the architect prefers to retire the command outright, that is a
   wire decision and this contract needs one line changed (§8.1's comment).
