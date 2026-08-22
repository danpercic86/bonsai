# P87 — Git & hook output observability (UI contract)

Input contract: `docs/contracts/P87-git-observability.md` (event model, `useGitActivity` store shape).
This spec owns everything downstream of that data: the phase→copy map, the toolbar in-flight
treatment (**View C**), and the session log dock (**View D**). It does **not** change the store, the
IPC surface, or `HookOutputDialog`'s existing behaviour.

House pattern mirrored throughout: the **bottom AI activity dock** (`ui-reference.md` §9, files
`AiActivityPanel.tsx` / `AiActivityHeader.tsx` / `AiActivityLog.tsx` / `aiDockFormat.ts`). View D is
the git analogue and reuses that dock's geometry, tokens, pill recipe, live-region rule, and log
surface. Reuse over invention: everything visual here already exists in §9/§11 — the only genuinely
new pieces are the phase-label helper, the run-row anatomy, and the determinate progress-bar mode.

---

## 0. Decisions locked in this spec (from the orchestrator brief)

- View C label transitions RunningHook → Network (the "it's not hung" fix). No cancel affordance.
- View D is session-scoped, read-only, has a **Clear**, and captures passing runs too. A failed
  blocking-hook run appears in **both** `HookOutputDialog` (the modal, verbatim + skip-retry) **and**
  the log (the persistent record). The dialog stays exactly as shipped.

---

## 1. Phase → copy map (the canonical table)

Backend emits structured `GitActivityCategory` × `GitPhase{kind,hook}` only. The UI derives every
user string here, in one pure helper `phaseLabel(category, phase)` in `gitActivityFormat.ts`. Used by
**both** View C (toolbar readout) and View D (the running row's live sub-label + the polite
announcer). Copy is sentence case, ends in `…` while in flight.

| category | phase.kind | phase.hook | String |
|---|---|---|---|
| push | preparing | – | `Preparing…` |
| push | runningHook | `pre-push` | `Running pre-push hook…` |
| push | network | – | `Sending objects…` |
| forcePush | runningHook | `pre-push` | `Running pre-push hook…` |
| forcePush | network | – | `Force-pushing…` |
| fetch | network | – | `Fetching…` |
| pull | network | – | `Fetching…` → (see note) |
| commit | preparing | – | `Preparing…` |
| commit | runningHook | `pre-commit` | `Running pre-commit hook…` |
| commit | runningHook | `commit-msg` | `Running commit-msg hook…` |
| commit | runningHook | `post-commit` | `Running post-commit hook…` |
| commit | finalizing | – | `Writing commit…` |
| amend | finalizing | – | `Amending…` |
| mergeCommit | finalizing | – | `Writing merge commit…` |
| — | runningHook | `<other>` | `Running <hook> hook…` (generic fallback) |
| — | any unknown | – | `Working…` (generic fallback) |

Notes:
- **Fetch vs pull network copy:** during transfer both read `Fetching…` (a fast-forward pull *is* a
  fetch then a ref move); once `finalizing`/`Finalizing…` arrives for pull, or if you prefer a
  distinct pull string, use `Pulling…`. RECOMMENDATION: keep `Fetching…` during transfer for both
  (it names the actual network work) and let the terminal row title carry `Pull`. Flagged §9-Q3.
- **Toolbar button participle** (short, layout-stable) is separate from this phase string and does
  **not** come from this table — see §2.

`categoryMeta(category)` → `{ verb, participle, glyph, noun }`, also in `gitActivityFormat.ts`:

| category | verb | participle (button) | noun (row) | glyph |
|---|---|---|---|---|
| push | Push | `Pushing…` | Push | `PushIcon` |
| forcePush | Force-push | `Force-pushing…` | Force-push | `PushIcon` |
| fetch | Fetch | `Fetching…` | Fetch | `FetchIcon` |
| pull | Pull | `Pulling…` | Pull | `PullIcon` |
| commit | Commit | `Committing…` | Commit | `RefDotIcon` |
| amend | Amend | `Amending…` | Amend | `RefDotIcon` |
| mergeCommit | Merge | `Merging…` | Merge commit | `MergeIcon` |

---

## 2. View C — toolbar in-flight treatment

Evolves `src/components/WorkspaceToolbar.tsx` (`~150–182` op-button labels; `~261` the
`.header-progress` bar) and adds one small presentational child (below). Symmetric treatment on the
commit button lives in `src/components/CommitBox.tsx`.

### 2.1 Two-part in-flight display (RECOMMENDED)

Do **not** stuff the phase string into the button label — `Running pre-push hook…` on a split Push
button reflows the toolbar mid-op (jank, and it fights the packed toolbar). Instead:

```
[ ⤴ Pushing…  ▾ ]   Running pre-push hook…          ← during hook phase
[ ⤴ Pushing…  ▾ ]   12,340 / 50,000 objects          ← during network (counts present)
[ ⤴ Pushing…  ▾ ]   Sending objects…                 ← during network (no counts)
```

- **Button label** = `categoryMeta(category).participle` — stable width for the whole op (`Pushing…`,
  `Fetching…`, `Pulling…`). This is the existing `remoteOp === 'push' ? 'Pushing…' : 'Push'` site;
  drive it from `activeRun.category` instead so force-push reads `Force-pushing…`.
- **Phase readout** = a new inline span rendered immediately after the active op's button, reusing the
  existing `.toolbar-job-status` treatment (11px `--text-2`, already used by `autoFetchReadout`).
  Class `.toolbar-phase`. Content: `phaseLabel(...)` from §1, EXCEPT during a `network` phase with
  transfer counts where it shows `objectsReadout(run)` (see §2.3). `title` = the fuller phase string.
  Only rendered while `activeRun !== null && activeRun.status === 'running'`.

The readout is the reassuring live detail; the announcer (§6) carries the same to AT. This keeps the
toolbar layout fixed while still transitioning RunningHook → Network visibly. **Alternative** (label
carries the phase, toolbar reflows) is worse; flagged §9-Q1 in case the orchestrator disagrees.

### 2.2 Commit button (`CommitBox.tsx`)

The commit button's busy label stays the short participle (`Committing…`). Render the same
`.toolbar-phase` readout beside it, fed by the active `commit`/`amend`/`mergeCommit` run, so
`Running pre-commit hook…` / `Running commit-msg hook…` is visible where the commit was launched
(not only in the toolbar). Same span, same class, same helper — no new idiom.

### 2.3 Progress bar — indeterminate ↔ determinate

The `.header-progress` 2px bar (bottom edge of `.workspace-toolbar`) gains a **determinate** mode:

- **Default (indeterminate):** unchanged — `@keyframes header-progress-sweep`, shown when
  `remoteOp !== null || refreshing || netBusy`. This covers preparing, all hook phases, push network
  (libgit2/git has no reliable %), and refresh.
- **Determinate:** during a **fetch/pull** `network` phase **when a fraction is derivable**. Add
  `data-determinate` + a `--progress: <0..1>` custom property on `.header-progress`; its `::after`
  renders a full-width fill scaled by `transform: scaleX(var(--progress))` (transform, not width — no
  layout), `transition: transform 150ms ease-out`. When the fraction is unknown, omit
  `data-determinate` → the sweep as today.
- **`objectsReadout(run)`** and the fraction both need received/total. The event model (§2 of the
  data contract) currently only carries transfer progress as a throttled **text line**. Parsing
  git's `Receiving objects:  N% (x/y)` line in the store is possible but fragile. **STRONG
  RECOMMENDATION → architect:** surface `git2` `transfer_progress` as a structured optional field
  (`received_objects` / `total_objects` / `received_bytes`) on the `network` phase (or a tiny new
  `progress` event kind) rather than only as text. Cheap (the callback already has the ints) and it's
  what makes both the determinate bar and the count readout robust. **Degradation:** if it stays
  text-only, the store best-effort-parses the canonical line; if parsing yields nothing, the bar
  stays indeterminate and the readout shows `Fetching…`. Flagged §9-Q2.

### 2.4 States (View C)

| state | button | readout | bar |
|---|---|---|---|
| idle | verb (`Push`) | none | hidden |
| preparing | participle | `Preparing…` | indeterminate |
| runningHook | participle | `Running pre-push hook…` | indeterminate |
| network (push/force) | participle | `Sending objects…` | indeterminate |
| network (fetch/pull, counts) | participle | `12,340 / 50,000 objects` | **determinate** |
| network (fetch/pull, no counts) | participle | `Fetching…` | indeterminate |
| finalizing | participle | `Finalizing…` | indeterminate |
| done (success) | verb (re-enabled) | none | hidden |
| done (failed) | verb (re-enabled) | none | hidden (row in View D + dialog if blocking) |

Buttons remain `disabled` during the op exactly as today; the readout is the sole "still working"
signal beyond the bar. No spinner glyph is added (reduced-motion-safe, cheap for the canvas).

---

## 3. View D — Git activity dock

A twin of the AI dock (§9): a bottom, full-width, collapsible/resizable dock. Its own small component
tree (never grafted onto `AiActivityPanel`, which is already carefully tuned and out of scope).

### 3.1 Placement & visibility

- **Placement:** child of `.workspace-host`, **after** `.ai-dock` (so, bottom-to-top: git dock →
  AI dock → `.panes`). `flex: none; overflow: hidden;` full width. Both docks collapse independently;
  in the common case (no AI use) there is exactly one dock. Two collapsed bars = ~56–60px — acceptable
  and informative; note the stacking order to `senior-dev`.
- **Visibility (deliberate divergence from §9):** the AI dock returns `null` at zero runs. The git
  dock returns `null` **only before the first git op of the session**; once it has shown, it **stays
  mounted** for the session even after Clear. Rationale: View D is an *always-on record* — the whole
  point is a stable place to look, and its live region (§6) must exist to announce ops. This gives a
  real empty state (§4) and a stable entry point instead of a surface that vanishes.
- **Collapsed = one status bar,** 30px cozy / 28px compact (matches §9). **Does NOT auto-expand** —
  git ops are frequent; auto-expanding on every push would be hostile (contrast the AI dock, which
  auto-expands on `awaitingInput`). It expands only on explicit user action (§5).
- **Expanded = bar + body,** persisted-or-session height **120–600px** (default `180`), capped at 60%
  of window height. Resizer: 4px top-edge strip (8px pointer band), reuse `PaneDivider`
  (`role="separator" aria-orientation="horizontal"`, keyboard ±8px, double-click → 180).

### 3.2 Component decomposition (all new; all < 500 lines)

| File | Responsibility |
|---|---|
| `src/components/GitActivityDock.tsx` | Shell/container: collapsed bar + resizer + expandable body (`<ol>` of rows) + the polite live region. Reads `GitActivityApi`. ~200 |
| `src/components/GitActivityHeader.tsx` | The collapsed status bar: active-run pill + subject + live elapsed + phase readout + collapse toggle + **Clear**. ~130 |
| `src/components/GitActivityRow.tsx` | One run: summary line (glyph, noun→target, pill, duration, timestamp, chevron) + expanded body (hook sub-rows, output log, Copy). ~200 |
| `src/components/gitActivityFormat.ts` | Pure helpers + LOCKED copy: `phaseLabel`, `categoryMeta`, `statusPill`, `durationLabel`, `timeLabel`, `objectsReadout`. ~150 |

The per-run output body reuses the **`.ai-log`** visual treatment verbatim (mono, `--bg-0`,
`white-space: pre-wrap`, the `.ai-log-dropped` / `.ai-log-trunc` chips) rather than a new log
component — it renders inside `GitActivityRow`'s expanded section. No stick-to-bottom hysteresis is
needed (a row is opened deliberately and is short; the active row streams but the list is
newest-first so it is already at top). If the output body grows past ~120 lines in the row file,
split it to `GitActivityRunLog.tsx`; otherwise inline it.

### 3.3 Collapsed status bar (`GitActivityHeader`)

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ ⤴ Push → origin/main   ● Running · Sending objects…      2.4s      Clear   ▸    │  ← running
│ ✓ Fetch                 ✓ Success                        0.8s      Clear   ▸    │  ← idle (latest)
└───────────────────────────────────────────────────────────────────────────────┘
```

- Left: `activeRun ?? runs[0]` — category glyph + noun + target (`→ origin/main`, ellipsized, `title`).
- Middle: status pill (§4.4) + (while running) the `phaseLabel`/`objectsReadout` `· detail`.
- Right cluster: live elapsed (`--text-2`, ticks per §store `tick` while running), then **Clear**
  (text button, §4.6), then the collapse chevron toggle (reuse `.file-chevron`, ≥24px hit box).
- Collapsed bar height/padding = §9 dock bar; density via `data-density`.

### 3.4 Run row (`GitActivityRow`) — collapsed

```
▸  ⤴ Push → origin/main                       ⋯ trimmed   ✓ Success   1.2s   14:32
```

- **Chevron** `▸/▾`: `.file-chevron`, real `<button>`, toggles the row's disclosure, ≥24px box.
- **Glyph** = `categoryMeta.glyph` (`aria-hidden`); **noun** in `--text-1`; **target** (branch /
  `origin/name`) in `--text-2`, single-line ellipsis + `title`. Target is `→ <upstream>` for
  push/pull/force-push, the branch for commit/amend, `(merge)` for mergeCommit, blank for fetch-all.
- **`⋯ trimmed` chip** (only when `linesDropped > 0`): hueless informational pill (§11), `title`
  `Bonsai keeps the last 500 output lines per run`.
- **Status pill** (§4.4).
- **Duration** (`durationLabel`, `--text-2`) and **timestamp** (`timeLabel` HH:MM, `--text-2`,
  `title` = full local date-time). Both `--text-2` per the §2 timestamp rule (never `--text-3` for
  text the user reads).
- Row height = a list row in the dock body; cozy/compact via `data-density` (see §12.10 metrics).

### 3.5 Run row — expanded

Disclosure opens, in order:
1. **Per-hook sub-rows** (one per `run.hooks[]`), indented under the summary:
   ```
     • pre-commit   ✓ exit 0
     • commit-msg   ⚠ exit 1
   ```
   hook name in `--text-1` (mono for the name is wrong — it is a label, use UI font); a small
   verdict pill `✓ exit 0` / `⚠ exit 1` (§4.4; the exit code is inside the pill label so code is
   never colour-only). `code === null` → `⊘ killed` (defensive; no cancel path exists yet).
2. **Output log:** the run's `lines[]` in a `.ai-log`-style `<ol>` (mono, `--bg-0`, pre-wrap). A
   leading `↑ N earlier lines trimmed` row when `linesDropped > 0`. stdout lines default `--text-2`;
   **stderr lines** carry a 2px `--warning` left border **and** are `--text-1` (shape + brightness,
   never colour alone) with a visually-hidden `stderr:` prefix for AT. A `truncated` chip on any line
   that hit 2000 chars (reuse `.ai-log-trunc`).
3. **Copy** button (top-right of the output block): copies the run's full combined output, reusing
   `AiOutputPanel`'s Copy idiom (`Copy` → transient `Copied`). `aria-label="Copy output"`.
4. **Failed blocking-hook note** (only when the run failed on a blocking hook): one `--text-2` line
   `This hook blocked the <op>. The full output opened in a dialog.` — the tie to `HookOutputDialog`.
   The row **never** re-offers "skip hooks" (that is a point-in-time decision the dialog owns; the
   row is read-only observability).

### 3.6 Empty output

A terminal run with zero captured lines and zero hooks (e.g. an up-to-date push short-circuit) shows,
when expanded, one `--text-2` meta line: `No output.` A running run with nothing yet: `Working…`.

---

## 4. States (View D) — complete

### 4.1 Running
Row pill `● Running` (hueless, accent `--h`), live elapsed ticking, expanded body streams lines +
shows the live `phaseLabel` as a `--text-2` sub-line under the summary. The row is never evicted
while running (store rule). Dock bar shows it as the active run.

### 4.2 Success
Pill `✓ Success`, final duration + timestamp. Passing hooks recorded as `✓ exit 0` sub-rows. This is
the case the log adds that was previously invisible (passing-hook output was discarded).

### 4.3 Failed
Pill `⚠ Failed`. Expanded: the hook sub-row with `⚠ exit N`, the stderr output, and the §3.5-4 dialog
note when blocking. A post-commit failure is **success at the run level** (`✓ Success`) with a
`⚠ exit N` sub-row for `post-commit` — matching current non-blocking semantics (data contract §9).

### 4.4 Status-pill vocabulary (all §11 verdict/hueless recipe — word + glyph, never colour alone)

| context | pill | glyph | recipe |
|---|---|---|---|
| run running | `Running` | `●` | hueless-ish; `--h: var(--accent)`, glyph + 40% border |
| run success | `Success` | `✓` | verdict, `--h: var(--success)` |
| run failed | `Failed` | `⚠` | verdict, `--h: var(--danger)` |
| hook passed | `exit 0` | `✓` | verdict, `--h: var(--success)` |
| hook failed | `exit N` | `⚠` | verdict, `--h: var(--danger)` |
| hook killed | `killed` | `⊘` | verdict, `--h: var(--danger)` (defensive) |

`●` for running matches the toast `info ●` (§10.2) and avoids `✨` (that is AI's mark) and `⚠`
(that already means failed). Label `--text-1`; hue in the glyph (100% over 14% tint) + 40% border,
exactly §11. Glyphs `✓`/`⚠` are the measured accessible carriers (§2/§11).

### 4.5 Empty (dock shown, no runs)
After a Clear that leaves zero runs, or before any op while the dock is (rarely) forced open: the body
shows a centered empty state (reuse `EmptyState` / the §8 idiom):
`No git activity yet.` / `Fetch, pull, push, or commit and it will show up here.`

### 4.6 Clear
Text button in the header (right cluster, before the collapse chevron), label `Clear`,
`aria-label="Clear git activity log"`, ≥24px. **Disabled** (`--text-3`, not clickable) when there are
no terminal runs. Clears **only terminal** runs (`clear()` in the store never touches a running run).
**No confirmation dialog** — it discards only ephemeral, session-scoped observability data that is
already gone on restart; nothing recoverable is lost, so a confirm would be friction, not safety.
(Contrast real destructive-git actions, which keep their confirm.) After Clear, if a run is still
running the dock shows just that run; if none, the §4.5 empty state.

### 4.7 Long content
Long branch/remote names and deep paths in the summary line ellipsize with `title`. Hook output wraps
(`pre-wrap`); per-run lines cap at 500 with the `↑ trimmed` header + collapsed `⋯ trimmed` chip; the
run list caps at 200 (oldest **terminal** evicted, running preserved — store rule). No horizontal
scroll in rows.

---

## 5. Entry points (how the user opens the log)

Restraint: **no new top-level toolbar button.** Three entry points, all reusing existing affordances:

1. **The collapsed dock bar itself** — always visible once the first op ran; click anywhere on it (or
   its chevron) to expand. Primary discovery path.
2. **Command palette** — one row: `Git activity` (id `git.activity`, keywords
   `log output hooks push pull fetch history`), rendered via the existing palette-entries plumbing
   (mirror `aiPaletteEntries` → add a `gitPaletteEntries`/entry). Enabled once `runs.length > 0`
   (mirrors the AI dock's `hasAiRuns` gate). Selecting it expands the dock and focuses the list.
3. **The toolbar phase readout** (§2.1) is clickable while an op runs → expands the dock and scrolls
   the active run into view. Reassurance-to-detail in one click. `role="button"`, `aria-label="Show
   git activity"`, ≥24px hit box (padding, not glyph inflation, per §3.1).

Optional keyboard shortcut: **`Ctrl/Cmd+Shift+L`** (L = log; pairs with `Ctrl/Cmd+Shift+A` for AI).
Verified free against the current map (`F/P/U` remote ops, `R` refresh, `K` palette, `F` find,
`A` AI dock). RECOMMENDED but flagged §9-Q4 for the orchestrator to confirm/assign, since it adds a
binding to `useWorkspaceKeyboard`.

---

## 6. Accessibility

- **Polite live region (required):** one always-mounted (once the dock exists) visually-hidden
  `role="status" aria-live="polite" aria-atomic="true"` span in `GitActivityDock`. Announces the
  **active run's phase transitions and terminal result only** — e.g. `Running pre-push hook`,
  `Fetching`, `Push finished — success`, `Push failed`. It does **NOT** announce output lines (a
  streaming log in a live region is hostile — same rule as §9). This is the single announcer for both
  View C and View D; the toolbar button is `disabled` mid-op so its label change is not a reliable
  announcement source.
- **The log list is not a live region.** The body `<ol>` is `tabIndex={0}`, `aria-label="Git activity
  log"`, focusable and read on demand (mirror `AiActivityLog`).
- **Keyboard nav:** Tab reaches the dock bar controls (chevron, Clear) then the list. Within the list,
  `ArrowUp/ArrowDown` move row focus; `Enter`/`Space` toggle the focused row's disclosure; the
  chevron and Copy are real buttons in tab order when the row is expanded. Esc collapses the dock when
  focus is inside it (does not lose the log).
- **Focus behaviour:** opening from the palette/shortcut expands the dock and moves focus to the list;
  on collapse, focus returns to the invoker (the palette re-focuses its trigger; the toolbar readout
  re-focuses itself). Mirror the AI dock's `focusDock`/restore.
- **No colour-only meaning anywhere:** run + hook status are word + glyph pills (§4.4); stderr lines
  use a left-border shape cue + `--text-1` + a visually-hidden `stderr:` prefix, not colour; the
  determinate bar is backed by the numeric readout.
- **Hit targets ≥24px:** chevron, Clear, collapse toggle, Copy, the clickable phase readout — box
  grown by padding, glyph unchanged (§3.1).
- **Contrast:** every pair is an existing §2 token used in a §2/§11-sanctioned way — labels `--text-1`
  (≥9:1), metadata/timestamps/log lines `--text-2` (7.9:1 dark / 4.9:1 light on `--bg-0`), glyphs at
  100% hue over 14% tint (≥3:1 graphics). **No new token, no hex** → nothing new to measure. `Clear`
  disabled uses `--text-3` (inert control, WCAG 1.4.3 exempt).
- **Focus ring:** 2px `--accent`, 1px offset, `:focus-visible` only.

---

## 7. Motion

- **No height animation** on the dock (a height transition forces 20k-row canvas relayouts — the §9
  prohibition). Collapse/expand/resize snap.
- Chevrons: `.file-chevron` 120ms transform (existing).
- Determinate bar fill: `transform: scaleX()` transition 150ms ease-out; indeterminate = existing
  `header-progress-sweep`.
- `prefers-reduced-motion`: sweep off (static 100% width @ 0.6 opacity — the §9 header-progress
  treatment), chevron + fill snap. Add the git-dock/bar selectors to the existing §9
  reduced-motion block.

---

## 8. Microcopy (actual strings)

- Phase strings: the §1 table (locked).
- Toolbar count readout: `12,340 / 50,000 objects` (thousands-separated); byte fallback
  `4.2 MB received` when only bytes are known.
- Pills: `Running` · `Success` · `Failed` · `exit 0` · `exit N` · `killed`.
- Trimmed: chip `⋯ trimmed`; log header `↑ 1,240 earlier lines trimmed`; line chip `truncated`.
- Empty output (expanded): `No output.` / running `Working…`.
- Empty log: `No git activity yet.` / `Fetch, pull, push, or commit and it will show up here.`
- Blocking-hook row note: `This hook blocked the push. The full output opened in a dialog.`
  (`push`/`commit`/`merge` per category).
- Clear: button `Clear`; `aria-label="Clear git activity log"`.
- Announcer: `Running pre-push hook` / `Fetching` / `Push finished — success` / `Push failed`.
- `HookOutputDialog` copy is unchanged.

No raw libgit2 text is ever used as chrome copy — hook stdout/stderr is only ever the log body or the
dialog body (verbatim, by design), never a label or announcement.

---

## 9. Open questions / flags for the orchestrator

- **Q1 (View C label placement).** Recommend the button keeps a stable short participle and the phase
  string lives in an adjacent `.toolbar-phase` readout (§2.1), NOT in the button label (avoids
  mid-op toolbar reflow). Confirm, or have the label itself carry the phase.
- **Q2 (network progress data — architect).** For a determinate bar + a real object/byte count,
  recommend the backend surface `git2 transfer_progress` as a **structured** field
  (`received/total objects`, `received bytes`) rather than only a throttled text line. Without it the
  bar stays indeterminate and the readout shows `Fetching…`. Needs an architect decision on the event
  shape.
- **Q3 (pull network copy).** Recommend `Fetching…` during a pull's transfer (names the real work),
  with `Pull` as the terminal row title. Confirm vs a distinct `Pulling…` during transfer.
- **Q4 (shortcut).** Recommend `Ctrl/Cmd+Shift+L` to open the log (free in the current map; pairs
  with `+Shift+A`). Confirm/assign, or drop the shortcut and rely on the bar + palette.
- **Q5 (dock geometry persistence).** The dock's collapsed/height can be **session-only** (no
  settings change, simplest) or **persisted** like the AI dock (2 new keys
  `gitActivityDockHeight`/`gitActivityDockCollapsed` — consistent, but a settings-type change owned
  by the architect). Recommend **session-only** for P87 to keep the settings contract untouched;
  revisit persistence later. Confirm.

---

## 10. Harness states (`VITE_MOCK_IPC=1`)

The data contract already specs the query seams; this maps them to what each must show:

- `?prePushHook` → View C: `Pushing…` + readout `Running pre-push hook…` → `Sending objects…`; View D:
  one `push` run, `✓ pre-push exit 0` sub-row, `✓ Success`.
- `?prePushFail` → `HookOutputDialog` opens (verbatim) **and** a `⚠ Failed` push row with `⚠ pre-push
  exit 1`, stderr in the body, the §3.5-4 dialog note.
- `?pushSlow` / `?fetchSlow` → a `● Running` row, live elapsed ticking, live phase sub-line; ends
  `✓ Success`. `?fetchSlow` should exercise the **determinate bar + count readout** if Q2 lands
  (emit structured transfer counts); otherwise indeterminate + `Fetching…`.
- Flood seam → per-run cap 500 with `↑ trimmed` header + `⋯ trimmed` chip; run list cap 200.
- Empty: run all seams then `Clear` → the §4.5 empty state (dock stays mounted).

**Everything here is browser-harness-verifiable** (dock, rows, pills, bar, dialog, empty, Clear).
Only frame-timing/scroll-feel of the live readout is a USER CHECKPOINT (headless harness pauses rAF).

---

## 11. Tokens (summary — all existing)

Component-scoped `--git-dock-*` alias block on `.git-activity-dock` (mirrors `--ai-dock-*`), each an
alias of a §2 theme token — one rule set serves both themes, **no new `:root`/`[data-theme]` token,
no hex**:
`--git-dock-bg: var(--bg-1)` · `--git-dock-log-bg: var(--bg-0)` · `--git-dock-meta: var(--text-2)` ·
`--git-dock-border: var(--border)`. Status hue via local `--h` (`--success`/`--danger`/`--accent`)
per §11. Density swaps via `.git-activity-dock[data-density='compact']`. Geometry + states recorded
in `ui-reference.md` §12.10.
