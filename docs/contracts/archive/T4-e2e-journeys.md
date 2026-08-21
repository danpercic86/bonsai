# T4 — Playwright e2e journeys (contract)

Phase T4 of the testing campaign (plan: `~/.claude/plans/the-end-goal-is-misty-crayon.md`).
Playwright drives the real React UI against the mock-IPC harness (`pnpm dev:mock`, port 1420).
Infra already landed in T1: `playwright.config.ts` (msedge locally / chromium in CI, webServer
wired), `e2e/fixtures.ts` (console-error-forbidding `test`/`expect`), `e2e/smoke.spec.ts`.

**Non-negotiables**
- Every spec imports `test`/`expect` from `./fixtures` — NEVER from `@playwright/test`.
  Any console.error / pageerror fails the test unless allowlisted per test with a justifying
  comment (`test.use({ allowConsoleErrors: ['…'] })`). The allowlist is for *expected*
  handled-error logging only; the default budget is **zero**.
- No application-code changes except the explicit `data-testid` allowlist in §4.
- Each spec file ≤300 lines. Shared flows live in `e2e/helpers.ts` only.
- Tests are order-independent and parallel-safe (§6). No test mutates another's state:
  mock state is per-page (module-scope `repos` Map resets on every `page.goto`), localStorage
  is per-context (Playwright gives a fresh context per test).

## 1. File layout

```
e2e/
  fixtures.ts            (exists — unchanged)
  helpers.ts             (NEW — shared flows, §3)
  smoke.spec.ts          (exists — keep; becomes part of @smoke)
  01-boot-onboarding.spec.ts
  02-graph-interaction.spec.ts
  03-graph-20k.spec.ts
  04-working-dir.spec.ts
  05-branches.spec.ts
  06-merge-conflicts.spec.ts
  07-rebase.spec.ts
  08-stash.spec.ts
  09-search-palette.spec.ts
  10-settings-persistence.spec.ts
  11-forge.spec.ts        (LANDS-LAST — see §5.11)
  12-ai-consent.spec.ts
  13-keyboard-shortcuts.spec.ts
  14-destructive-confirms.spec.ts
  15-error-injection.spec.ts
  16-history-undo-health.spec.ts
```

## 2. Mock seeding conventions (verified against `src/ipc/mock/**`)

**localStorage keys** (see `src/ipc/mock/persistence.ts`; readers tolerant-parse per field, so
partial seeds merge over defaults):
- `bonsai.mockUiSettings` — `UiSettings` partial. Key seeds: `{onboardingSeen:true}` (skip the
  P43 overlay — default seed for every spec except 01), `{aiConsented:true}` (light AI entry
  points), `{theme:'light'}`, `graph:{…}` prefs.
- `bonsai.mockSession` — `{openRepos: string[], activeRepo: string|null}`. Seeding a path here
  auto-reopens it on boot — the ONLY way to open a non-default repo shape (path substrings:
  `unborn`, `detached`, `merge`, `rebase`; see `repoState.ts` `createRepoState`).
- `bonsai.mockRecents` — `[{path, lastOpened}]` for recent-repos assertions.

**URL flags** (read at module init or repo open — must be on the FIRST `goto`, they cannot be
added after load): `?fixture=20k|detached|noconfig` · `?op=merge|rebase` (seeded paused
conflicted op, `opStateSeed.ts`) · `?remote=authfail|network|rejected|conflict|rebaseconflict` ·
`?rebase=conflict` (interactive-rebase pause) · `?hooks=fail|failpush` · `?ai=off` ·
`?forge=off|auth|gitlab|bitbucket` · `?undo=commit|merge|switch|none` · `?historyFail` ·
`?submodule=fail` · `?branch=cbhconflict` · `?sign=…` · `?update=…`.

**In-band sentinels**: `#fail` in search query / external path / compose message / PR head
branch; `#hookfail` in a commit message; token containing `bad` in forgeSetToken; branch/stash
name containing `conflict` (mergeBranch / applyStash triggers); commit oid suffixes `c0ffee`
(pick/revert conflict) and `deadbe` (stash-pop conflict).

**Fixture facts** (for assertions): default repo path `C:\mock\bonsai-fixture`; HEAD branch
`main`; locals incl. `feature/sidebar` (↑2↓1), `fix/watcher-debounce` (conflicted re-apply on
checkout), `experiment-unmerged` (delete → `unmergedBranch` error), `feature/merged-a` (FF on
checkout); remote-only `origin/release`; 7 tags `v0.1.0…v2.0.0-rc.1`. Default status is dirty
(so a WIP row may occupy display row 0 of the graph). `?fixture=noconfig` drops git identity.

## 3. `e2e/helpers.ts` — exact surface

```ts
import type { Locator, Page } from '@playwright/test';

export const FIXTURE_REPO = 'C:\\mock\\bonsai-fixture';
export const DEFAULT_ROW_HEIGHT = 32; // GraphPrefs default (persistence.ts)

export interface HarnessOptions {
  /** URL query flags, e.g. { fixture: '20k', op: 'merge' }. */
  flags?: Record<string, string>;
  /** Partial UiSettings merged into bonsai.mockUiSettings BEFORE load.
   *  Defaults to { onboardingSeen: true } — pass {} to see onboarding. */
  uiSettings?: Record<string, unknown>;
  /** Seed bonsai.mockSession (auto-reopens repos on boot). */
  session?: { openRepos: string[]; activeRepo?: string | null };
  /** Seed bonsai.mockRecents paths (lastOpened = now). */
  recents?: string[];
}

/** addInitScript(localStorage seeds) → page.goto('/?' + flags). */
export async function gotoHarness(page: Page, opts?: HarnessOptions): Promise<void>;

/** Clicks Skip on the Welcome dialog if visible (only spec 01 needs it). */
export async function skipOnboarding(page: Page): Promise<void>;

/** gotoHarness → EmptyState "Open repository" click → wait for the graph
 *  canvas to be visible. THE standard entry for most tests. */
export async function openRepo(page: Page, opts?: HarnessOptions): Promise<void>;

/** Graph scroll container + canvas locators (data-testid, §4). */
export function graphScroller(page: Page): Locator;
export function graphCanvas(page: Page): Locator;

/** Click a DISPLAY row (WIP row, if present, is display row 0):
 *  y = row*rowHeight + rowHeight/2 - scroller.scrollTop, x = canvas midwidth.
 *  rowHeight read from the live GraphPrefs seed (default 32). */
export async function clickGraphRow(page: Page, displayRow: number): Promise<void>;

/** Set scroller.scrollTop via evaluate; returns the resulting scrollTop. */
export async function scrollGraphTo(page: Page, px: number): Promise<number>;

/** Ctrl+K (the app binds Ctrl on win32 UA; use 'ControlOrMeta+K'). */
export async function openPalette(page: Page): Promise<Locator>; // returns the dialog

/** Locate the ConfirmDialog (role=dialog, aria-label = title). */
export function confirmDialog(page: Page, title: string | RegExp): Locator;
/** Assert the dialog is visible, then click its confirm button by name. */
export async function confirm(page: Page, title: string | RegExp, button: string | RegExp): Promise<void>;

/** Error toast locator: role=alert inside .toast-stack (Toasts.tsx). */
export function errorToast(page: Page, text?: string | RegExp): Locator;

/** Sidebar branch row + its context menu (right-click). */
export async function openBranchContextMenu(page: Page, name: string): Promise<Locator>;
```

Implementation notes: seeds go through `page.addInitScript` (runs on-origin before app code);
`gotoHarness` builds the query string from `flags`. All waits are Playwright auto-waits or
`expect(...).toBeVisible()` — **no `waitForTimeout`** (the mock's 150 ms delays are covered by
auto-waiting). If the tester finds row-click math unstable (WIP-row presence varies with
status), stabilize by staging/committing first or asserting on the commit-details panel content
rather than a specific row index.

## 4. Selector strategy + data-testid allowlist

Priority: `getByRole` (dialogs carry `role=dialog` + `aria-label`, palette is a labeled
combobox/listbox, error toasts are `role=alert`) → `getByLabel`/`getByPlaceholder` (palette
input, "Search commits") → `getByText` scoped to a container. **No CSS-class selectors** except
the two container classes already named here (`.toast-stack` scope only).

The app currently has **zero** `data-testid`. senior-dev MAY add exactly these (nothing else),
each only if no accessible handle exists:

| testid | element | why |
|---|---|---|
| `graph-scroller` | scroll container div in `GraphCanvas.tsx` | scrollTop control + row math |
| `graph-canvas` | the `<canvas>` | click coordinates (locator('canvas') is ambiguous once diff views mount) |
| `commit-details` | commit-details root in the right panel | assert selection landed |
| `status-panel` | working-dir status root | scope file-row queries |
| `diff-view` | diff viewer root | scope hunk/row assertions |
| `conflict-editor` | ConflictEditor root | scope region queries |

Forge components (`PrPanel.tsx`, `ForgeConnect.tsx`, `PrCreateForm.tsx`) are being modified by
a paused session — spec 11 must use only role/label/text selectors there, **no testids**.

## 5. Journey specs

Feasibility legend: **[FULL]** end-to-end state mutation verified in the mock ·
**[RENDER]** mock is read-only/outcome-only for this flow → assert render + error/outcome
surface only (downgrade noted).

### 01-boot-onboarding.spec.ts — @smoke
Mock: fresh storage (pass `uiSettings: {}`); session seeding for repo shapes.
1. Fresh boot → Welcome dialog visible → Skip → EmptyState with "Open repository". [FULL]
2. Onboarding persistence: complete/skip, reload → no Welcome (localStorage `onboardingSeen`). [FULL]
3. Open repo → canvas + sidebar `main` (overlaps smoke.spec — keep both; this one asserts the
   right panel shows working-dir status when nothing is selected). [FULL]
4. Unborn repo: seed `session.openRepos: ['C:/mock/unborn-repo']` → empty graph, status panel
   usable, no console errors. [FULL]
5. Non-repo open: seed `session.openRepos: ['C:/mock/not-a-repo']` → error UI, app usable. [RENDER]
6. Recents: seed `bonsai.mockRecents` → EmptyState/palette lists the entry. [FULL]

### 02-graph-interaction.spec.ts — @smoke
Mock: default fixture; `getGraph` serves the canned layout + prepended mock commits.
1. `openRepo` → `clickGraphRow(1)` → commit-details panel shows message/author/date; a diff
   file list renders (getCommitDiff). [FULL]
2. Click a different row → details update.
3. Ref pills: the canvas is opaque to DOM queries — assert via sidebar + details instead:
   selecting the HEAD row shows the HEAD/branch context (do NOT pixel-inspect pills). [RENDER]
4. Scroll: `scrollGraphTo(2000)` → scrollTop applied, canvas still visible, no errors; scroll
   back to 0. [FULL]
5. Detached fixture (`flags: {fixture:'detached'}`): boots, renders, sidebar shows no head dot
   /detached indication. [RENDER]

### 03-graph-20k.spec.ts — @smoke @slow
Mock: `flags: {fixture:'20k'}` (`fixtures/graph20k.ts`).
1. Boots + canvas visible with 20k rows (scroller scrollHeight > 20_000 * rowHeight * 0.9).
2. Jump-scroll to middle and end (`scrollGraphTo`), then rapid wheel scrolls via
   `page.mouse.wheel` ×10 — canvas stays visible, **zero console errors** throughout.
3. Row click after deep scroll still selects (details panel renders). [FULL]
No frame-budget assertion (headless timing is unreliable) — perf stays an AI-gate/manual check.

### 04-working-dir.spec.ts — @smoke @destructive
Mock: default (dirty INITIAL_STATUS + live `src/main.rs` three-way). All flows [FULL] —
`status.ts` genuinely mutates staged/unstaged/untracked and `commit` prepends a graph row.
1. Stage a file: unstaged row → stage action → appears under Staged.
2. Unstage it back. Stage an untracked file → appears as added; unstage → returns to untracked.
3. Discard with confirm: pick an unstaged file → Discard → ConfirmDialog (danger) → confirm →
   row disappears (`discardPaths`). Cancel path: dialog dismiss leaves the row.
4. Commit validation: empty message → commit blocked or `emptyMessage` toast; nothing staged →
   `nothingToCommit` error toast.
5. Happy commit: stage → type message → commit → Staged empties, graph gains a top row with
   the message summary, branch ahead-count chip bumps.
6. Identity gap: `flags: {fixture:'noconfig'}` → commit → `configMissing` error surfaced. [RENDER]

### 05-branches.spec.ts — @destructive
Mock: `branches.ts` — all [FULL].
1. Create: sidebar action → name `e2e/topic` → appears sorted in the local list. Invalid name
   (`bad name`) → `invalidName` error; duplicate → `branchExists`.
2. Checkout: context menu on `feature/merged-a` → checkout → head dot moves; FF happens
   silently (behind clears). Checkout `fix/watcher-debounce` → conflicted re-apply outcome
   surfaced (toast/conflict row), app usable.
3. Rename: context menu → rename `e2e/topic` → `e2e/topic2` → list updates.
4. Delete with confirm: delete `e2e/topic2` → ConfirmDialog → confirm → gone. Delete
   `experiment-unmerged` → `unmergedBranch` error toast, branch remains.
5. Remote: context menu on `origin/release` → checkout → local `release` created + checked out.

### 06-merge-conflicts.spec.ts — @destructive
Mock: the **seeded** paused merge (`flags: {op:'merge'}`) is the authoritative editor flow;
a *fresh* `mergeBranch('…conflict')` only returns a conflicts outcome WITHOUT seeding opState
(verified `merge.ts:34`) — so the fresh-merge case is outcome-toast-only. [FULL via seed]
1. Open with `?op=merge` → OpBanner shows the paused merge; conflicted rows `README.md`,
   `src/auth.ts` listed.
2. Open `src/auth.ts` in the ConflictEditor → ours/theirs/result regions render with marker
   fixture text; resolve (take ours / edit text) → `resolveConflictText` → row leaves the
   conflicted list.
3. Resolve `README.md` via quick action (deletedByThem: keep ours / delete).
4. Commit merge: message prefilled from opState → `commitMerge` → OpBanner clears, graph gains
   the merge commit.
5. Abort path (separate test, fresh page with `?op=merge`): Abort → ConfirmDialog → opState
   clears, conflicts gone.
6. Fresh clean merge: createBranch `demo-clean` → merge → merged toast + new graph top row. [FULL]
7. Fresh conflicted merge (downgraded): createBranch `demo-conflict` → merge → conflicts
   outcome surfaced (toast/dialog); assert app stays usable. [RENDER]

### 07-rebase.spec.ts — @destructive
1. Seeded pause (`flags: {op:'rebase'}`): OpBanner "step 2/3" → resolve `src/auth.ts` →
   Continue → rebase finishes (banner clears, replayed commits on graph). [FULL]
2. Abort (fresh page, `?op=rebase`): Abort → confirm → banner clears. [FULL]
3. Clean rebase: rebase current branch onto another via context menu → `rebased` toast +
   3 replayed rows on the graph. [FULL]
4. Plain-rebase conflict route (`flags: {remote:'rebaseconflict'}`): start rebase → pauses at
   1/3 with conflict → Skip / Continue paths. [FULL]
5. Interactive plan: graph row context menu → "Interactive rebase…" → `RebasePlanEditor`
   renders the todo ops (`getInteractivePlan`); reorder/reword if the dialog supports it →
   Start → finishes (rewritten commits replace originals). With `flags: {rebase:'conflict'}`
   the Start pauses → abort restores. [FULL]

### 08-stash.spec.ts — @destructive
`stash.ts` — all [FULL] except conflicted apply (trigger = seeded stash whose message contains
`conflict`; tester: verify `fixtures/stashes.ts` seeds one — if not, downgrade that case to
skip).
1. List renders the seeded stack.
2. Save: dirty status → stash (choose scope via `StashSplitButton`) → stack gains stash@{0},
   status sections clear per scope.
3. Apply: stack unchanged, `applied` outcome.
4. Pop: entry removed, survivors re-indexed.
5. Drop with confirm: ConfirmDialog → confirm → entry gone; cancel keeps it.
6. Reserved-path stash (seed contains `RESERVED_STASH_PATHS`): first apply blocked with the
   reserved-paths outcome → retry "skip reserved" → appliedSkippingReserved. [RENDER]

### 09-search-palette.spec.ts — @smoke
1. Graph search bar (aria-label "Search commits"): type a fixture summary term →
   `searchCommits` results render (`SearchResultsList`); pick one → row selected (details
   panel updates). [FULL]
2. Palette: `openPalette` → dialog (aria-label "Command palette", combobox + listbox) → type a
   branch name → option appears → Enter dispatches (checkout or reveal — assert the effect).
   Arrow-key navigation moves `aria-selected`. Esc closes. [FULL]
3. Palette command dispatch: run a safe command (e.g. toggle theme / open settings) and assert
   the effect — pairs with spec 16's action-dispatch cases.
4. Sidebar list filtering (`ListFilterInput`, ≥6 tags fixture): type `v1` in the Tags filter →
   list narrows; clear restores. [FULL]
5. Error path: search for text containing `#fail` → error toast, app usable
   (`allowConsoleErrors` only if the app legitimately logs it — prefer zero).

### 10-settings-persistence.spec.ts — @smoke
All [FULL] — `setUiSettings` round-trips through `bonsai.mockUiSettings`.
1. Theme: open Settings → switch to light → `document.documentElement` theme attribute/class
   flips → `page.reload()` → still light.
2. GraphPrefs: toggle `showSha`/`showAuthor`/`compact`; slide rowHeight → reload → Settings
   reflects persisted values (assert via the controls' state, not canvas pixels).
3. Pane widths / list view toggle persist across reload.
4. Corrupt storage resilience: seed `bonsai.mockUiSettings` = `"garbage{"` via init script →
   app boots on defaults, zero console errors.

### 11-forge.spec.ts — @forge — **LANDS-LAST**
PrPanel/ForgeConnect/PrCreateForm are mid-edit by a paused session: write this spec LAST,
selectors = roles/labels/visible text ONLY, and expect a follow-up selector pass after that
session lands. Flows verified against `handlers/forge.ts`:
1. Connect: default = unauthenticated → ForgeConnect visible → enter token → `forgeSetToken`
   flips `authenticated` → PR list renders (`FORGE_PR_LIST`). Bad token (contains `bad`) →
   `authFailed` error, still disconnected. [FULL]
2. Warm start: `flags: {forge:'auth'}` → PR list immediately; open a PR → detail view
   (title/body/review comments). [FULL]
3. Create PR: form → title/branches → submit → detail renders. [FULL]
4. AI generate description: with `uiSettings: {aiConsented:true}` → Generate → mock proposal
   fills the body, **never auto-submits**. With `flags: {ai:'off'}` → aiUnavailable state.
   Head branch containing `#fail` → error surfaced, form usable. [FULL]
5. Offline: `flags: {forge:'off'}` → every forge surface shows the offline/error state; a
   Retry affordance re-fetches (still failing) without crashing. [RENDER]

### 12-ai-consent.spec.ts
1. Declined (default `aiConsented:false`): AI entry points (explain commit, generate commit
   message, branch-name suggest…) hidden/disabled. **Zero AI IPC calls**: assert no AI error
   toasts appear AND instrument via `page.addInitScript` that wraps `console` is NOT viable —
   instead assert the entry points are absent; (mock IPC is in-page, not network — request
   interception cannot count calls; this is the honest downgrade). [RENDER]
2. Accepted (`uiSettings: {aiConsented:true}`): select a commit → Explain/analyze entry point
   → mock AI output overlay/panel renders canned text. [FULL]
3. `flags: {ai:'off'}` with consent → availability probe fails → entry points show the
   unavailable state, no errors. [RENDER]

### 13-keyboard-shortcuts.spec.ts
Binding table source of truth: `src/components/repoWorkspace/useWorkspaceKeyboard.ts` (tester
reads it and spot-checks ~6 bindings — do not enumerate all here to avoid drift).
1. `ControlOrMeta+K` opens the palette; Esc closes.
2. Shortcut overlay: its binding (likely `?` / `F1` — confirm in the hook) shows
   `ShortcutOverlay`; Esc closes.
3. Manual-refresh binding fires without error; a couple of navigation/selection bindings.
4. Shortcuts are suppressed while a text input is focused (type `?` into the commit box →
   no overlay).

### 14-destructive-confirms.spec.ts — @destructive
Sweep: every danger-variant ConfirmDialog requires explicit confirm; Esc/cancel is a no-op.
Mock-reachable enumeration (each gets a confirm + a cancel case):
- Reset branch hard (graph row context menu → `resetBranch`) — also moves HEAD on confirm.
- Force push (`forcePush`; use default remote state).
- Branch delete (sidebar) + stale-branches bulk delete (`StaleBranchesDialog`).
- Discard file(s) (`discardPaths`) + discard-all if surfaced.
- Stash drop.
- Tag delete; remote remove.
- Worktree remove; submodule deinit/remove (`?submodule=fail` NOT set).
- Merge/rebase abort (covered in 06/07 — reference, don't duplicate).
Assert dialog is `role=dialog` with an accurate title, the confirm button names the action,
and cancel leaves state untouched (re-query the list).

### 15-error-injection.spec.ts
Every case: error toast (`role=alert`) renders with the mapped message, app remains usable
(canvas still visible, a follow-up benign action succeeds). All [FULL] error paths:
1. `flags: {remote:'authfail'}` → fetch/push → authFailed toast; `remote:'network'` →
   networkError; `remote:'rejected'` → push rejected (non-FF message).
2. `flags: {hooks:'fail'}` → commit → HookOutputDialog / hook-rejection surface; retry with
   skip-hooks succeeds. `#hookfail` sentinel in a message on a default repo does the same.
3. `flags: {hooks:'failpush'}` → push blocked → "Push anyway (skip hooks)" retry works.
4. Search `#fail` (if not covered in 09), submodule action with `flags: {submodule:'fail'}`,
   `flags: {historyFail:'1'}` → history index build/search error state.
5. Composer: plan message containing `#fail` → atomic rollback error, status untouched
   (only if the Composer dialog is reachable without AI consent; else drop this case).

### 16-history-undo-health.spec.ts
1. Reflog viewer (`ReflogView`): open → `MOCK_HEAD_REFLOG` entries render. [FULL]
2. Undo: default flag → "undo reset (mixed)" description (`describeLastUndo`); confirm →
   executes via `resetBranch` (graph/HEAD moves). `flags: {undo:'switch'}` → not-undoable
   reason shown, action disabled; `{undo:'none'}` → nothing-to-undo; `{undo:'merge'}` with a
   dirty tree → blocked (requiresCleanWorktree). [FULL]
3. Repo health panel (`RepoHealthPanel`, `getRepoHealth`): renders metrics; refresh works. [RENDER]
4. Palette actions dispatch (with 09): run "open settings", "manual refresh", one branch
   checkout via palette — assert effects.
5. File history / blame render for a fixture path (`fileHistory`, `blameFile`). [RENDER]

## 6. Isolation, tags, CI

- **Isolation**: keep `fullyParallel: true`. Playwright's default per-test browser context ⇒
  fresh localStorage; each `goto` re-initializes the in-page mock (`repos` Map, module flags).
  No `test.describe.serial` anywhere; no shared files/state between tests. Multi-step flows
  that need sequencing live inside ONE `test()` as steps (`test.step`).
- **Tags** (in titles, Playwright-greppable): `@smoke` (01–04, 09, 10, smoke.spec) ·
  `@destructive` (04–08, 14) · `@forge` (11) · `@slow` (03). Runs:
  `pnpm test:e2e --grep @smoke` for quick gates; full run has no grep.
- **CI**: chromium project already wired (`ci.yml` per T1). Add nothing to config except (if
  needed) `expect.timeout` bumps — flag to orchestrator before changing retries. `@forge` runs
  in CI too, but if the paused forge-UI session breaks selectors, temporarily gate with
  `--grep-invert @forge` and record it in TODO.md.

## 7. Acceptance criteria

1. All 16 spec files + existing smoke green locally on the `msedge` project, **3 consecutive
   full runs** (flake check): `pnpm test:e2e` ×3, zero failures, zero `.skip` except cases this
   contract explicitly downgrades (each `test.skip`/`test.fixme` carries a comment referencing
   the §5 downgrade).
2. Zero console errors tolerated: no spec uses `allowConsoleErrors` without an inline comment
   naming the expected message and why it is legitimate; total allowlisted tests ≤ 3.
3. Each spec ≤300 lines; all shared logic in `helpers.ts`; no imports from `@playwright/test`
   in specs.
4. App-code diff limited to the §4 data-testid allowlist (6 ids max) — reviewer verifies.
5. `pnpm build` + `pnpm test` (vitest) still green (testid additions must not break RTL/unit
   tests).
6. Spec 11 lands last, after the paused forge-UI session's changes are committed (or with the
   orchestrator's explicit go-ahead to land against current HEAD).

## 8. Flags for the orchestrator (ambiguities + recommendations)

1. **Fresh-merge conflict does not seed opState** (`merge.ts` returns the outcome without
   mutating state) — journey 5 is delivered via the `?op=merge` seed; the fresh path is
   outcome-only (§5.6). If full fidelity is wanted, that's a small mock enhancement — out of
   T4 scope; recommend logging it as a T3 mock-layer finding instead.
2. **AI "zero IPC calls" is not directly countable** from Playwright (mock IPC is in-page, no
   network) — §5.12 downgrades to entry-points-absent. Alternative: expose a
   `window.__mockCallLog` in the mock behind `VITE_MOCK_IPC` for tests; recommend NOT doing it
   in T4 (app-code change beyond the allowlist) unless you want the stronger assertion.
3. **20k frame budget**: excluded from e2e (headless timing flaky); stays an AI-gate manual
   check with frame-stats console logs. Confirm this matches your gate expectations.
4. **Conflicted stash-apply seed**: depends on `fixtures/stashes.ts` containing a
   `…conflict…`-named entry; tester verifies and skips-with-comment if absent (§5.8).
5. **Forge spec timing** (§7.6): needs your call on when the paused PrPanel/ForgeConnect
   session is considered landed.
