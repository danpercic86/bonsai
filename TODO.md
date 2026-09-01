# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).
Avoid tauri "test" feature on this machine (STATUS_ENTRYPOINT_NOT_FOUND); use runtime-free
inner functions for command tests.
Harness traps (cost a session each): the hidden Browser pane reports `innerWidth/innerHeight = 0`, so
every `vh`/`vw` rule evaluates to 0 — call `resize_window` (1440×900) before any layout measurement;
`setTimeout` is throttled to ~1 s in a hidden page, so batch tool dispatches instead of many `await`s;
never remove React-owned DOM nodes to "reset" a menu (throws `removeChild` on the next render) —
dismiss with Escape; headless preview pauses `requestAnimationFrame`, so canvas repaint/scroll-feel
ACs can only be checked in the native window.
**USER MANDATE (2026-07-28, updated 2026-08-04 for cross-platform support): on Windows, never use
C: for temp/scratch/mock repos — C: is critically full. Use `D:\Data\Temp\bonsai-scratch`; when running
cargo tests set TMP/TEMP to `D:\Data\Temp` (tempfile honors them). On macOS/Linux, `scratch_dir()` now
falls back to the OS temp dir (`std::env::temp_dir()/bonsai-scratch`) automatically — no special
handling needed there. Include the Windows-specific guidance in every subagent prompt that runs
tests or creates repos only when running on a Windows machine.**

## Board conventions

Status vocabulary: `pending` · `in-progress` · `done` · `awaiting USER CHECKPOINT` · `deferred`
(deferred always carries a one-line reason). A milestone is `done` only when the AI gate **and** the
native USER CHECKPOINT have both passed — the orchestrator never self-declares the second half.

**History is archived, not deleted.**

**Where the rest of the board went:** `docs/history/todo-archive-2026-09.md` (Parts 22-32, moved
2026-09-01) and `docs/history/todo-archive-2026-08.md` (Parts 1-21). See the Archive table at the
bottom. Velocity/gate-cost measurements: `docs/history/velocity-2026-09-01.md`.

---

## 🚀 P100 + P101 + DX-e2e — IN PROGRESS (started 2026-09-01, USER: "do P100 and P101 and DX: build-bundle e2e")

**Current step:** all three requested items DONE — P100 (`e118375`), P101 (`4fec07a`), DX-e2e (`46088e0`). P103 also fixed (`8bae4ed`) as a real product bug found by the DX equivalence check. **Awaiting USER CHECKPOINT: P100 (4 items) + P101 (AC12-AC16).** Filed while here: **P102**, **P104**, **P105**. Flipping the e2e bundle default is gated on P104 alone now.

**Sequencing, and why it is not arbitrary.** P100 **must** land before P101. P101's audit method
(P98 contract §8.8 step 1) requires measuring every declaration against its *composited backdrop
per state* — and P100 changes the active/selected-row fill that those backdrops composite against.
Auditing first would measure figures P100 then invalidates. DX-e2e touches only `scripts/`+config,
so it runs concurrently with the design pass.

Slots: **1** P100 contract ∥ DX-e2e → **2** P100 impl + the 3 fixtures, review, commit →
**3** P101 audit against the committed P100 tree (never overlapped with P100 review — MUST-FIX churn
on the active rows would stale the measurements) → **4** P101 impl, review, tester, full gate, commit.

**Fixture ask GRANTED (orchestrator decision, 2026-09-01).** P98 §8.8 asked for three mock fixtures
and capped that as the only fixture ask for the whole `--text-3` programme. Granted, and folded into
P100's implementation pass so they exist before P101 mounts anything: (a) a mock state opening
`DiffOverlay` on a **conflicted** scope; (b) a hint on one **enabled** and one **active** combobox
option plus one **disabled palette** option; (c) a route to `WorktreeContextDialog` with one
**blocked** row. Rationale: without them ~6 of 10 P98 declarations were source-derived rather than
measured, and P101 has 120+ — an audit asserted from CSS source is the exact failure mode P101
exists to correct. `.diff-tree-count` stays structurally unreachable (canvas-driven selection) and
remains a USER CHECKPOINT; do not try to automate it.

**P100 carries a USER CHECKPOINT.** It changes the app's most-used selection affordance's visual
identity — the designer called this a product call, not a sweep. AI-gate evidence (contrast ratios,
AC19, harness screenshots) is necessary but NOT sufficient; the perceptual result (a quieter active
row; whether the `inset 2px 0 0 var(--accent)` leading bar restores the punch) needs the user's eyes.


---

## 🚧 P91 — observability — WIP ON BRANCH, NOT READY (do not merge)

Lives only on `feat/p91-observability` (7 increments, 356 files, last commit 2026-08-28, now 20+
commits behind `dev`). **The user confirmed 2026-08-31 that this branch is work in progress and not
ready** — do NOT merge it, and do not treat its own commit messages ("all 7 increments +
activation, harness evidence green; board final") as an authoritative status.

Recorded here only so future sessions stop rediscovering it as a mystery: it is absent from `dev`
entirely (no commits, no `docs/contracts/P91-*`, nothing in `docs/history/`), which is why the board
reads P90 → P92.

Known overlap to expect whenever it does land: `docs/contracts/ui-reference.md` (+159 lines, all
*new* sections §4.2/§5.1/§5.2/§12.11 — it does not touch the §2/§4.1 text P95 rewrote, but its §4.2
inserts immediately after, so expect one conflict hunk) and `src/styles/forge-pr.css` (+13 lines,
same file P95 edits). It adds 18 lines to `tokens-and-base.css` but does **not** change
`--text-2`/`--text-3`, so P95's contrast figures remain valid.

## ✅ P99 — `repo` state dead in a production bundle — DONE + VERIFIED (`fea4a71`, USER 2026-09-01) — **DOWNGRADED, NOT A PRODUCT BUG**

**The original HIGH framing was WRONG and is retracted.** It was filed (from P94 instrumentation) as
"an unborn repo renders the full graph instead of *No commits yet*", i.e. a shipped-in-1.5.0
violation of a locked v1 product decision. Investigated 2026-09-01: **there is no product defect.**
The P94 observations were real; the *attribution* was wrong.

### What was actually true
- **The mechanism is real.** The activation self-heal effect (`RepoWorkspace.tsx:1233`) skips its
  first run via `activeFlipRef`, and it was the only path to `setRepo`. Under React StrictMode
  (dev only) setup->cleanup->setup runs on the same instance, the ref persists, and the second setup
  fires the refresh **by accident**. In a production bundle nothing calls `setRepo` at boot.
  `tauri.conf.json` confirms `pnpm tauri dev` serves the vite dev server (`beforeDevCommand: pnpm
  dev`, `devUrl: 1420`) while `pnpm tauri build` ships `../dist` — so dev masks it, the release
  bundle does not. **No `pnpm tauri build` was needed to settle this**; the config plus React's
  production semantics are decisive.
- **Correction to the filing:** not "never set / null forever" but **null until the first `full`
  refresh** (manual Refresh, window focus, activation flip, mutation).
- **Correction to the blast radius:** the filing said "anything keyed on `repo?.<field>` needs
  auditing". There was exactly **one** consumer (`:641`), confirmed with an unfiltered
  `grep -n "\brepo\b"` — the filtered grep I first ran could have hidden a `fn(repo, repoId)` line.

### Why it was harmless — and the real culprit
`head` was `repo?.head ?? branches?.head ?? null`, and the backend derives **both** from one shared
`read_head_info` (`crates/bonsai-core/src/git/repo.rs:73`; called by `repo.rs:62` for `openRepo` and
`branches/list.rs:24` for the snapshot). They cannot disagree.

**The observed symptom was 100% MOCK infidelity.** `src/ipc/mock/handlers/branches.ts` hardcoded
`unborn: false` in *both* arms and had no unborn case, while `buildInfo` honoured the unborn kind —
a divergence the real backend structurally cannot have. Worse, the unborn mock state seeded the full
`INITIAL_BRANCHES` clone: **~13 phantom local branches**, 5+ remote-tracking branches and every tag,
none of which can exist in a real unborn repo.

### Evidence (the decisive experiment)
Against a **production** bundle (`vite build --mode mock` + `vite preview`, unborn repo opened):
- with the mock fix -> **"No commits yet"** + "No branches yet", 0 console errors;
- with **only** the mock fix reverted -> no empty state, **7+ phantom branch rows**.

That single experiment proves BOTH that `repo` really is null in the production bundle (otherwise
`repo.head.unborn` would have rendered the empty state anyway) AND that the mock was the sole cause.

### What shipped
1. **Rust tests** — `crates/bonsai-core/src/git/branches/unborn_boot_tests.rs` (6 tests) prove that
   on a real unborn repo `list_refs` returns `Ok` with `head.unborn == true`, `oid == ""`,
   `branch_name == Some("main")` and empty ref lists, and that every other boot slice (repo info,
   status, graph seed, graph stream) also returns `Ok`. **This was the load-bearing gap: dev's
   accidental refresh had been masking any branches-side failure too, so nothing had ever tested the
   unborn boot path.** In-crate because `read_head_info` is `pub(crate)`. Mutation-checked (flipping
   the assertion goes red) and the fixture self-guards on `UnbornBranch` so it cannot pass vacuously.
2. **Mock fidelity fix** — one exported `buildHead(state)` (the mock analogue of `read_head_info`)
   now used by `buildInfo`, `listBranches` and the state seed, so the handlers **cannot drift again**;
   unborn seeds `{local: [], remote: [], tags: []}`. Plus: `commitInner` now flips `kind` off
   `'unborn'` and seeds the branch the first commit creates — previously the harness would show
   "No commits yet" + "No branches yet" *while a commit row existed*, which real git cannot do.
3. **Dead-state removal** — dropped the `repo`/`setRepo` `useState`; `head` is now
   `branches?.head ?? null`. `RepoWorkspace.tsx` 2778 -> 2774. **This fixed a latent bug:** on an
   unusable repo the old code took `head` from a `RepoInfo` the UI had *just* declared unusable and
   wiped (and a bare repo does have a HEAD). It now fails closed.
4. Corrected two comments that P99 falsified (`refreshScope.ts:21`, `RepoWorkspace.tsx:1108`) — they
   claimed `openRepo` maintains the header HEAD; it is now used only for the usability check and
   watcher self-heal.

### Why single-sourcing HEAD is safe (reviewer's invariant — record this)
In the scope matrix (`refreshScope.ts:67-86`) **every scope with `openRepo: true` also has
`branches: true`**, and the `openRepo: false` scopes never move HEAD. That — not merely "the two
heads are equal" — is the structural reason the snapshot cannot strand a stale value.

**The StrictMode-masking bug class is now closed by construction:** the `openRepo` block writes **no
state at all**, it only clears. `activeFlipRef` remains but gates a refresh call, not a sole writer.

Reviewer verdict: **approve, zero MUST-FIX**. Gate: **full 8/8 green, 566s** (one earlier run showed
the known e2e parallel flake at `24-settings-shell.spec.ts:238` — 54/54 passed isolated, that test
3/3, unrelated to P99). No USER CHECKPOINT owed: verified in a real production bundle.

Remaining NIT, deliberately not actioned: `buildHead` hardcodes `branchName: 'main'` for unborn
where the real `read_head_info` reads HEAD's symbolic target — an accepted mock simplification.

## ✅ P100 — accent-fill contrast — AI GATE GREEN, ⏳ AWAITING USER CHECKPOINT (`e118375`)

Closed the last AA shortfall in `ui-reference.md` §2 apart from the `--text-3` remainder (P101).
Contract `docs/contracts/P100-accent-fill-ui.md`; design review
`docs/contracts/design-review-2026-09-01-P100.md`.

**A retracted premise, recorded so it does not come back.** P98 §5-A concluded *"white is the
ceiling, so no foreground fixes this; only the fill can"* — and the orchestrator repeated it when
briefing P100. **It is false.** White is the ceiling only among *lighter* inks; going **darker**
passes. `#16181d` on the dark accent is **5.52:1** — the reference's own §5 lane-0 row read
symmetrically, and already shipped at `partial-staging.css:86`. The designer found this by not taking
the brief on faith. Hence two recipes instead of one blanket demotion:
- **A — a state** (selected row, active option, segment) → change the fill: `--selection`,
  `--text-1` label (**9.36/13.29**), `--text-2` secondary (**5.01/6.42**), plus a **mandatory**
  non-colour carrier (`--selection` vs `--bg-1` is only ~1.3:1). Six surfaces.
- **B — an action** (`.btn-primary`) → keep the loud hue fill, flip the ink. One token value.
  `.btn-primary` verified (twice, independently) as the token's **only** live consumer, so B is
  zero-collateral.

Survey was complete, not seeded: 7 text-bearing accent fills (the brief listed 4), 11 decorative
fills fine at the 3:1 bar, 15 `color-mix` tints classified out. Also retired the phantom
`--accent-fg` (a *second* phantom in the file P98 §4 just repaired), dropped two `!important` from
`.wt-copy-toggle-on`, rewrote the `tokens-and-base.css` comment that asserted the now-retired "both
themes use white on accent" invariant, and removed the dead `GraphColors.accentText`.

**The hover deviation, approved on measured evidence.** `filter: brightness()` moves ink *and* fill
together, so light-theme `.btn-primary:hover` went 4.65 → **3.94 ✗**. The contract's own sanctioned
remedy also failed (**4.06 ✗**) *and* would have added a literal hex outside `tokens-and-base.css`.
senior-dev substituted `color-mix(in srgb, var(--accent) 92%, var(--text-1))` — brightens in dark,
deepens in light, leaves ink alone: **5.99/5.15 ✓**. The designer measured all three mounted,
**retracted its own remedy**, and amended AC16 to accept a deeper light hover. Now a house rule in
§2 (hunk-3 addendum) and the device P102 must use for `.btn-danger`.

**USER CHECKPOINT owed (4 items — do not self-declare):** (1) the active row is now *quieter*
(`--selection` not accent) — does it still read as active, and does the `inset 2px 0 0 var(--accent)`
leading bar restore the punch? (2) light-theme `.btn-primary:hover` reads as **hover, not pressed**
(amended AC16). (3) the new PR Base/Compare short-oid hint — new visible microcopy in both themes.
(4) `.wt-copy-toggle` grew **~2px taller and ~4px wider**.

**Residuals (filed, non-blocking):**
- `.is-disabled.is-active` compound **may be effectively dead** — arrowing skips disabled rows and
  pointer-over does not set active, so the designer measured it by *injecting* `is-active`. Either it
  is genuinely unreachable (delete the rule, and AC5/AC19 with it) or a filter-reset path can land
  the index on a disabled row (keep it). Decide before deleting.
- Hover mechanism now inconsistent with four sibling accent/danger buttons still on
  `filter: brightness`: `partial-staging.css:92,111`, `settings-primitives.css:278`,
  `updates.css:123`. **Folds into P102** with the house device above.
- `.combobox-option--active .combobox-option-hint` and its `search.css` twin are now **identical to
  their base** (`--text-2`) — dead declarations, kept deliberately to hold the AC19 triple symmetric.
  Note, do not "clean up".
- `.wt-copy-toggle-off` (`WorktreeCopyCandidates.tsx:112,122`) has **no CSS rule at all** —
  pre-existing dead class, found in review.
- The contract's own verbatim comments defeat its mechanical greps: `conflicts.css:202` makes AC2
  ("`--accent-fg` returns zero matches") fail *literally* while being clean in substance;
  `dialogs-forms.css:164` makes AC1/AC3 report false hits. Reword future contract comments in prose.
- Stale pre-extraction P78 comment at `RepoWorkspace.tsx:1433-1435`, redundant beside the new P100
  pointer. NIT. And `padding: 1px 7px` on `.wt-copy-toggle button` would restore exact parity.
- `pnpm lint:size` reports 23 reclaimable lines across 14 files; baseline update still deliberately
  not run (it would move other milestones' accounting).

**Process change adopted (P100 §6-D).** `ui-reference.md` is now ~1322 lines / ≈40k tokens and
`ui-designer` has **no `Edit` tool** — only whole-file `Write`, which **truncates mid-file** at that
size. *That* is the structural cause of the P95 "silently unapplied patch", not carelessness. From
P100 on: the designer supplies **verbatim line-anchored hunks**, the orchestrator applies them with
`Edit`, and verifies line count + section count + tail sentinel + hunk confinement. Verified this
pass: 1299 → **1322** lines, **13** sections throughout, untouched regions byte-identical, §2's P101
pointer preserved. **This deviates from CLAUDE.md's "no other agent edits `ui-reference.md`" —
raise with the user whether to give the designer `Edit` or split the file.**

## ✅ DX — built-bundle e2e — DELIVERED opt-in; default gated on P104 only (2026-09-01)

P94's stated reason for abandoning this ("the built bundle is not behaviour-equivalent to dev") no
longer holds for the reason P94 gave — but the equivalence check **found a real difference**, so the
default stays on the dev server. Opt-in only: `node scripts/gate.mjs --e2e-bundle`, or
`E2E_BUNDLE=1 pnpm test:e2e`.

Delivered: `scripts/e2e-server.mjs` (new, 72 lines — Vite's JS API, not a spawned CLI, with
SIGTERM/SIGINT/stdin-EOF handling); `playwright.config.ts` (`E2E_BUNDLE`/`E2E_BUNDLE_PORT`,
`gracefulShutdown`); `scripts/gate.mjs` (`--e2e-bundle`, default unchanged); `dist-mock/` ignored.
Ports: harness 1420, e2e dev 1430, **e2e bundle 1440**.

**Equivalence result: 161 tests, 2 full runs per mode. 160/161 identical pass/fail identity.**
Bundle mode is **~1.7-2x faster** on summed per-test time (326-363s vs 485-685s); the
`vite build --mode mock` itself is negligible (~0.6-2.1s warm). Gate artifact reuse is **not**
possible — the gate's `tsc + vite build` step is **real** mode; the specs need `VITE_MOCK_IPC=1`,
which only `--mode mock` supplies via `.env.mock`. Documented at the e2e step in `gate.mjs`.

### ✅ P103 — `24-settings-shell.spec.ts:238` — FIXED (`8bae4ed`), a REAL product bug

**Mechanism (proven, not inferred).** `IdentityMenu` lifted its open state to `App` **only from a
passive effect**, so `setMenuOpen(false)` landed on React's *default (deferrable)* lane. In a
production bundle `App`'s re-render — and with it the re-subscription of `useAppShortcuts`' window
`keydown` listener — was deferred **past the next keypress**, so the stale listener still closed over
`menuOpen === true` and **swallowed the first `Ctrl+,` after Esc**. Exactly one keypress: a second
`Ctrl+,` 300 ms later always worked. Instrumented bundle logs showed the menu already gone from the
DOM while `App render menuOpen=false` printed only *after* the `,` key had been seen with
`menuOpen=true`.

**Fix:** open/confirm state is mirrored into refs and lifted **synchronously from the discrete
handlers**, so the release rides the discrete lane and is flushed before the next event. The old
effect stays as a reconcile safety net. `confirmRef` (not a plain `false`) is what keeps shortcuts
suppressed under the confirm dialog, because `ContextMenu` calls `onClose()` in the same tick as
`onSelect()`.

**Fail-before / pass-after — bundle is the discriminator.** Bundle arm: **3/3 failed → 3/3 passed**.
Dev arm: the whole spec **18 passed**. Independently re-verified by the orchestrator after commit:
`E2E_BUNDLE=1 ... -g "Esc dismisses the menu" --repeat-each=2 --workers=1` → **2/2 passed (4.1m)**,
clean exit.

**My briefed hypothesis was the wrong defect class, and the agent said so.** I framed this as a
consumed-latch bug of the P99 shape. It is effect-lane deferral + a stale handler closure — same
dev/prod asymmetry (StrictMode's extra work usually flushes the deferred render in time, so dev is
*flaky* where prod is *deterministic*), different mechanism. The four consumed mount-skip latches
(`GraphCanvas.tsx:502/513/541`, `RepoWorkspace.tsx:1236`) were checked and are genuinely a different
class: they make **dev do extra work prod correctly skips**, not prod miss work. Untouched; filed as
follow-ups, with `RepoWorkspace.tsx:1236`'s extra mount-time `refresh('activation','full')` the most
likely to matter.

**This closes the P95 loop.** The P95 tester note *"Menu key → Esc → Menu key again ⇒ no menu"*,
dismissed as a "degenerate mount transient" with "ambiguous evidence", is the same
one-swallowed-keypress shape. **Three write-offs of this one bug** — P95's tester, my own P99
"parallel flake" dismissal, and the DX pass's initial full-suite reading — all now explained by one
mechanism. The lesson is on the board at P105: a flake that reproduces deterministically in a
production bundle is not a flake.

**Flipping the e2e bundle default is still gated on P104** (the 4-worker post-suite hang, present in
*both* modes), which is independent of this fix.

### 🚨 P104 — the 4-worker e2e suite hangs after the last test — PENDING, PRE-EXISTING

**Reproducible in BOTH modes and observed before any DX change** (the very first run, stock config on
`pnpm dev:mock`, hit a 10-min timeout having reached test 161). The suite completes all 161 tests then
**hangs before printing the summary line**; both full runs had to be killed. Does **not** happen
single-worker. The board's 407-566s gate baseline implies e2e used to complete, so this is a
regression from something outside the DX change. **Consequence: full-suite e2e wall clock is
currently unverifiable, so the "5.8 min -> 1.3 min" claim cannot be confirmed either way.** Run e2e
single-worker until fixed.

Two costs already isolated, so P104 does not start from zero: **Edge teardown is ~116s per browser**
(30s graceful-close timeout, then 85s force-kill to process exit; timestamped under
`DEBUG=pw:browser`) — a fixed per-run floor that caps the "1.3 min" target regardless of bundle mode.
And one hang variant is **already fixed** in this pass: the old `command: 'pnpm dev:mock'` orphaned
the server on Windows (port stayed bound, Playwright waited on it) — 446s wall for 178s of testing.
In-process server + `gracefulShutdown` cut teardown to 0.3s, which accounted for ~4.5 min of the
original "e2e is 5.8 min" figure.

### Dev/prod gaps found by step 3 (recorded so P103 has a suspect list)

Latches where **dev does extra work prod skips** — `if (!ref.current) { ref.current = true; return; }`
with no cleanup reset, so StrictMode's second setup passes the latch:
`src/graph/GraphCanvas.tsx:504` (activeMountRef), `:514` (firstDataPaintSkippedRef), `:543`
(metricsMountRef) — an extra `resize()`/`paintNow()` at mount in dev only; and
`src/components/RepoWorkspace.tsx:1237` (activeFlipRef) — an extra `refresh('activation','full')` at
mount in dev only (**latent:** if a spec ever relied on that refresh, prod would not do it).
Correct in both (`if (ref.current) return;`): `RepoWorkspace.tsx:1216`, `src/App.tsx:609`, `:697`.
Benign prev-value baselines: `OnboardingOverlay.tsx:86`, `useReadOverlays.ts:150`, `CommitBox.tsx:156`,
`PrDetailContainer.tsx:111-112`, `AiActivityPanel.tsx:104`, `RepoWorkspace.tsx:203`.

A dev/prod gap **besides** StrictMode: `import.meta.env.DEV`-gated code absent from a production
bundle — `ConflictEditor.tsx:73` (`window.__bonsai.conflictSelfTest`), `GraphCanvas.tsx:287`/`:629`
(`[bonsai] frames`, `[bonsai] scroll-test` logs), `selfTest.ts:300`, `conflictSelfTest.ts:143`,
`useCoalescedRefresh.ts:12` (`__bonsaiRefreshRounds`), `settings/GitConfigAdvanced.tsx:36`,
`SettingsRow.tsx:72`, `SettingsSegmented.tsx:37`. **No e2e spec consumes `window.__bonsai` or those
logs** (grep for `__bonsai` in `e2e/` is empty), so none is load-bearing for the suite today — but a
future spec that reaches for them would pass in dev and fail in a bundle.
`GraphCanvas.tsx:136` uses `DEV || MOCK_MODE`, so graph stats stay on in a mock bundle.

## ✅ P95 — a11y: graph scroller semantics, keyboard reachability, toolbar contrast — DONE + VERIFIED (`f9a9209`, USER 2026-09-01)

**Current step:** IMPLEMENTED and committed `f9a9209`. Reviewer + ui-designer both **approve**,
zero MUST-FIX. AI gate green — full `pnpm gate` **8/8** first try (Rust 2051/2051, vitest 2397/2397,
Playwright 160 passed in default parallel mode — which independently re-validates P94 — plus
clippy/eslint/tsc/size-ratchet clean). Contrast measured per selector in both themes in the
harness; orchestrator separately confirmed zero console errors and the exact rendered attribute set.

**USER CHECKPOINT — CONFIRMED by the user 2026-09-01.** All four items below were checked in the real Tauri window:
- **AC8** — clicking a commit in the graph while a centre overlay is open leaves focus in the
  scroller (the P93 rule; click path untouched by P95, but only a real canvas click proves it).
- **AC14** — with a screen reader, arrow-key navigation produces exactly ONE utterance per row and
  the keyboard hint is discoverable.
- **AC15** — the selection ring follows keyboard nav and focus does not fight scroll (needs a real
  canvas repaint; rAF never fires in the harness).
- **AC16** — the partial-staging gutter buttons still read as dim-at-idle now that they are
  `--text-2` (perceptual judgement).

**Verified in review, worth knowing:** the reviewer confirmed the load-bearing assumption by
sweeping **every** `keydown` listener in the app — nothing upstream `preventDefault`s an arrow key,
so the new `defaultPrevented` guard cannot silently kill graph navigation. Both new files
(`GraphKeyboardHint.tsx`, `GraphTooltip.tsx`) exist because `GraphCanvas.tsx` is already ~860 lines;
the tooltip extraction is verbatim and the file **shrank** 868 → 862, so no ratchet bump was needed.
Two contract faults found in review (§1.4's "no new file", AC10's unsatisfiable "exactly") were
corrected in the contract in place.

**Follow-ups filed, not blocking:**
- `useWorkspaceKeyboard.p95.test.tsx` spies `focusScroller`, so nothing pins the
  `focus({ preventScroll: true })` argument that §2.2 calls "required". Needs a `GraphCanvas`-level
  test, not a hook-level one.
- **For `tester`:** *Menu key → Esc → Menu key again* returned "no menu" on the second open once in
  the harness, but only during a degenerate mount transient (scroller measured 480×24 mid-mount) and
  a clean reload was reliable. Ambiguous evidence; pre-existing P92 focus-restore territory, not
  introduced by P95. Wants a real-window check.
- `ui-reference.md:70` "one known AA shortfall remains" now reads stale in tone (still factually
  true of the read-text residual) — fold into P98.

**Chosen ARIA model — live-region-only.** `role="grid"`, `aria-rowcount` and
`aria-activedescendant` are **dropped and now forbidden** by `ui-reference.md` §4.1. The scroller
becomes `role="group"` + `aria-label="Commit graph"` + `aria-describedby` → a new `.sr-only`
keyboard hint; the existing `GraphSelectionAnnouncer` stays the sole announcement channel and
already speaks "Row {n+1} of {N}". This is what the canvas + virtualization invariant forces: with
no per-row DOM there is nothing for an IDREF to point at, so the "active row scrolled out of the
rendered window" problem stops existing rather than being managed. Rejected: visually-hidden rows
per visible row (reintroduces DOM into the one component premised on having none, and the IDREF
dangles intermittently); a one-option `listbox` (misreports a 20k-row graph and double-announces).

**Contrast finding is bigger than filed.** The `≈4.0:1` in the original follow-up was optimistic —
`.diff-overlay` is opaque `--bg-0`, so the toggle composites onto a solid backdrop, giving
**3.68:1 dark / 3.17:1 light**. And it is **10 selectors, not one**: `.diff-intra-toggle`,
`.diff-view-toggle button`, `.right-pane-tab`, `.diff-hunk-discard-btn`, `.tab-close`, the two
partial-staging gutter buttons, AI asset chips, the checks-panel neutral rollup glyph, and the
settings swatch hover border. Seven disabled-state rules are explicitly exempt. ui-designer also
caught a **stale figure in `ui-reference.md` §2** (`--text-2` on light `--bg-0` written as 4.9:1,
actually 7.99:1 — the old number used the dark `--text-3` hex on white); corrected.

**Bonus defect found, folded in as AC17.** `GitActivityDock.tsx:116-133` calls `preventDefault()` on
arrow keys **without** `stopPropagation()`, so today the window-level handler *also* moves the graph
selection silently while the dock has focus. P95 must add an `if (e.defaultPrevented) return;` guard
before those branches — without it, P95 would upgrade a silent bug into focus being yanked out of
the dock on every keypress.

**Orchestrator decisions (2026-08-31)** on the three questions ui-designer flagged:
- **(A) Deferred `--text-3` read-text sweep:** defer all 7 selectors to **P98**, as recommended —
  do not pull the two `*-hint` ones forward. Keeps P95 a single reviewable class of change
  (enabled-control labels) instead of mixing in a second, differently-motivated class.
- **(B) `role="group"` vs `role="region"`:** **`group`**, as recommended — `region` is a landmark
  and would add navigation noise for a pane that is not a document region.
- **(C) AC17 (the dock arrow-key guard):** **keep it.** It is a behaviour change, but the current
  behaviour is a silent bug, and P95 cannot ship its focus-follows-consumption rule without making
  that bug user-visible. Fixing it is the smaller change.

**Harness-verifiable:** AC1-7, AC9-13, AC17. **USER CHECKPOINT:** AC8 (real canvas click), AC14
(screen reader), AC15 (canvas repaint needs rAF), AC16 (perceptual).

Original problem statement, as filed (the contrast figure here is superseded by the measured
per-selector figures above):

- Graph scroller has a dangling `aria-activedescendant` IDREF and `role="grid"` with no
  `role="row"` children (pre-existing).
- Window-level arrow-key row nav can select a row without focusing the scroller, so the keyboard
  row-menu is unreachable that way.
- `.diff-intra-toggle` off-state label is `--text-3` on the transparent overlay toolbar (≈4.0:1),
  under the 4.5:1 AA floor; `--text-2` fixes it. Shared overlay chrome, not P93's doing.

## 🚨 P105 — `--accent` as TEXT fails AA on `--bg-1`/`--bg-2` — PENDING (found 2026-09-01, measured mounted)

**Found by the orchestrator's P101 harness pass, and it contradicts a claim `ui-reference.md` §2
actively makes.** §2 states `color: var(--accent)` "is a house-wide pattern (~30 call sites) and is
**fine on `--bg-0` / `--bg-1` / `--bg-2`**". Measured mounted at `4fec07a` from the shipped tokens
(`--accent` `#4f8cff` dark / `#2f6fe4` light):

| Backdrop | Dark | Light |
|---|---|---|
| `--bg-0` | **5.52** ✓ | **4.65** ✓ |
| `--bg-1` | **5.07** ✓ | **4.34 ✗** |
| `--bg-2` | **4.48 ✗** (marginal) | **4.00 ✗** |

So the §2 sentence is wrong for **`--bg-1` in light** and for **`--bg-2` in both themes**. Only
`--bg-0` is clean in both. `--bg-2` light at 4.00 is not a rounding argument.

**How it surfaced, because the route matters.** A P101 probe of `.branch-glyph` returned 4.34 in
light instead of the expected `--text-2` 7.45. The cause was *not* a P101 defect and *not* opacity —
`.branch-row-head .branch-glyph` overrides to `color: var(--accent)`, so the probe had grabbed the
HEAD row's glyph. `.branch-glyph` itself is a 12px glyph, judged at the **3:1** graphics bar, so
**4.34 passes for that element** and P101 is unaffected. Chasing the anomaly rather than dismissing
it is what exposed the documented-vs-measured gap.

**Why this is the pattern, not an isolated bug.** §2 credits **P74** with retro-fitting "the
hue-as-text family". That is now the **third** app-wide claim in this programme to fail on
inspection — P95's enabled-control class (3 escapes found by P101), P98's "`--text-3` family closed"
(122 declarations never classified), and now P74's hue-as-text sweep. Every one was asserted
app-wide without an enumeration. **P101 §3 is the template for the fix: enumerate, bucket, record a
verdict per call site, and only then claim closure.** Do not accept a "~30 call sites and it's fine"
sentence as evidence again.

**Scope for the pass:** enumerate every `color: var(--accent)` call site, resolve each one's
composited backdrop per state, and split text (4.5:1) from glyph/border/bar (3:1) — the split P100
established and P101 §2 formalised. Expect many to be legitimately glyphs. Related and already
filed: **P102** (`--danger` fills at 3.70:1, hardcoded `#ffffff`) and P100's residual noting four
sibling buttons still on `filter: brightness`. Consider doing P102 and P105 as one hue-audit
milestone, since both are "a hue token used against an insufficiently contrasting surface" and both
want the same enumerate-then-bucket method.

## 📋 P102 — `--danger` fill contrast — PENDING (filed 2026-09-01 from P100's survey)

Same defect class as P100, deliberately **not** folded into it (P100 contract §6-C; orchestrator
agreed — one defect class per milestone is what kept P95/P98/P100 reviewable).

`.btn-danger` (`src/styles/controls.css:70-71`) and `src/styles/updates.css:114-116` put a
**hardcoded `#ffffff`** on `var(--danger)`. `ui-reference.md` §6 already measures that pair at
**3.70:1 in dark** — below the 4.5:1 read-text bar, on destructive-action buttons where misreading
the label is the worst case.

**The remedy is already shipped in-repo**, so this is a small pass: `partial-staging.css:104` uses
the `--bg-0`-ink flip on `--danger` at **4.80:1** dark. P100 establishes the precedent and the
decision rule (reference §2 ACCENT FILL bullet, recipe 2: an action keeps its loud hue fill and
flips the ink; only *states* get demoted to `--selection`). Expect a `--danger-text` token mirroring
`--accent-text`'s per-theme split rather than two inline literals.

Scope check before implementing: sweep for every `#fff`/`#ffffff` on a `var(--danger)` fill, not
just these two — P100's survey found 7 accent fills where the seed list had 4.

## ✅ P101 — the full --text-3 audit — AI GATE GREEN, ⏳ AWAITING USER CHECKPOINT (`4fec07a`)

**All 124 declarations carry a recorded bucket and verdict** (`docs/contracts/P101-text3-audit-ui.md` §3) — the first pass in this programme to meet its own standard, so §2's "family closed" claim finally has an enumeration behind it. **31 exempt** (16 disabled, 10 placeholder/empty, 3 group titles, 2 glyphs clearing 3:1 on every state) + **93 fixed**. Post-fix grep is exactly **32 in 14 files** as predicted. Verified exhaustively, not sampled: the diff is 92 plain `color:` + 1 `border-color:` + 1 `background:` + the one §4.2 rule, and nothing else. Mounted spot-check (orchestrator, 5 of 94 selectors reachable in the default state): `.section-label`, `.tree-dir-name`, `.branch-badge`, `.file-chevron` all **7.25 dark / 7.45 light** — matching the predicted `--text-2`-on-`--bg-1` figures exactly, which validates the source-derivation method. 84 selectors are not reachable in the default mock state; AC7 coverage is therefore **5/94 measured**, the rest source-derived — stated plainly rather than called verified. **USER CHECKPOINT owed:** AC12-AC16 (hierarchy, density, the 15 `forge-pr` declarations behind a token screen, colour-only dot states, `.rebase-plan-commit.dropped` now carrying on `line-through` alone). Prior text archived below.

### Original filing (kept for the reasoning)

## 🚨 P101 — audit the 122 unaudited `--text-3` uses — RESOLVED by `4fec07a`

**Count reconciliation, done 2026-09-01 by the orchestrator — the contract pins 122, a fresh grep
says 124. Do not "correct" either number; both were right when taken.** The delta is exactly the
**two disabled-hint overrides P98's own MUST-FIX-1 added** (`src/styles/search.css:243`,
`src/styles/dialogs-forms.css:250`) — per-file counts confirm it: `search.css` 8 -> 9 and
`dialogs-forms.css` 6 -> 7, everything else unchanged. Both new declarations are legitimately in the
`disabled` bucket, so the audit's *work* is still 122 items. Authoritative surface as of `0fe0102`:
**124 in `src/styles/` across 33 files**, plus **one outside** it — `src/components/conflictCmSetup.ts:36`
(`.cm-gutters`, already sanctioned decorative in `ui-reference.md` §2 at 3.68/3.17 with a revisit
trigger). `src/components/settings/SettingsEmpty.tsx` mentions the token in a comment only — not a
declaration, not an audit item. Re-pin at audit time; §8.8 step 6's "family closed" claim must rest
on an enumeration that matches a fresh grep, which is the exact failure P101 exists to fix.

Distribution (highest first): `forge-pr` 15, `settings-legacy-sections` 11, `search` 9,
`commit-panel` 9, `sidebar` 7, `dialogs-forms` 7, `blame-history` 6, `repo-health` 5,
`diff-content`/`dialogs`/`controls`/`commit-box`/`agent-assets` 4 each, then 3s/2s/1s.

**One more seed found during reconciliation:** `.search-result-date`
(`src/styles/search.css:331`) — a date, the same class of string as the three `blame-history.css`
timestamps §8.8 already rules read-text. Audit it, do not assume the verdict.

**`ui-reference.md` §2's claim that the `--text-3` family is "closed" is FALSE — corrected during
P98, do not let it come back.** P95 swept the enabled-control class and P98 an *enumerated* read-text
set, but **122 `color: var(--text-3)` declarations remain in `src/styles/` and have never been
classified.** P95's AC10 grep hit ~140 and the AC was reworded precisely because enumerating them was
unsatisfiable; nobody went back.

Confirmed violations by §2's own test, in `src/styles/blame-history.css` — a file P98 never opened:
- `.blame-date` (:93), `.file-history-date` (:167), `.reflog-date` (:241) — **timestamps**, which §2
  explicitly names as read-text requiring `--text-2`. 3.68:1 dark / 3.17:1 light on `--bg-0`, worse
  on `--bg-1`/`--bg-2`.
- Borderline, needs a ui-designer call: `.reflog-oid-old`, `.reflog-oid-arrow`, `.reflog-oid-root`
  (:214/:218) — abbreviated oids in the reflog.

**Why this is filed as HIGH and not a NIT:** the discovery rate is the signal. Two gaps
(`.pr-merge-method-desc`, `.cm-gutters`) turned up by casual inspection, then three more by grepping
a *single* extra file. The orchestrator's P98 decision #3 was justified by "this is the last gap
making the closed claim true" — that premise was simply wrong, and a tidy-but-false "closed" claim
is worse than an honest scope statement because it stops a future sweep from ever looking.

Method to use (ui-designer is writing it into the P98 contract as the durable deliverable): classify
each of the 122 against the "**must the user read it to act?**" test, not against how the text looks
— small/uppercase/letter-spaced does not make text decorative.

## ✅ P98 — `--text-3` read-text sweep — DONE + VERIFIED (`be668e0`, USER 2026-09-01)

**Implemented; ui-designer APPROVED after one MUST-FIX. AI gate green; USER CHECKPOINT confirmed.**

Landed: 10 read-text swaps `--text-3` -> `--text-2` (8 selectors incl. the orchestrator's 8th,
`.pr-merge-method-desc`), 4 hardcoded white literals -> `var(--accent-text)`, and the §4 dead-token
repair (5x `var(--border-0)` -> `var(--border)`). Diff verified colour-only outside that one
sanctioned hunk; `git diff --stat` byte-identical to `--ignore-all-space --stat`.

**MUST-FIX-1 (found by ui-designer, a regression P98 itself introduced).** The `*-hint` rules are
*child* selectors, so they also matched inside a **disabled** option and beat the `--text-3` the
disabled state relies on inheriting: label 3.38:1 / hint 7.25:1 — the qualifier twice as bright as
the text it qualifies, disabled dimming half-applied, AC7 broken. Fixed with two rules
(`.combobox-option--disabled .combobox-option-hint`, `.command-palette-option.is-disabled
.command-palette-option-hint`). **Placement is load-bearing and must not be reordered:** each ties on
specificity with its `--active`/`.is-active` override, so source order alone decides the
disabled+active state. Measured specificity is (0,2,0) in `dialogs-forms.css` but **(0,3,0)** in
`search.css` (compound selectors on one element) — both carry a comment saying why the order matters.
All four disabled states verified to compute an identical label/hint colour in both themes.

**Verification honesty — read before trusting the numbers.** Only **4 of 10** declarations were
measured on a mounted instance (`.diff-overlay-kind`, `.command-palette-option-hint` idle + active,
and the active row's own label). The other 6 are **CSSOM-rule-confirmed but source-derived**, not
composited: `.diff-tree-count` needs canvas-driven commit selection (synthetic clicks don't hit the
canvas), and there is no mock fixture for a conflicted scope, the worktree dialog, or an *enabled*
combobox hint. The AC19 disabled-state figures are rule-level (injected nodes against the real
CSSOM), not a React-mounted option.

**Two mechanism traps recorded so they aren't re-hit:**
- `resize_window colorScheme` measures **dark twice** — the app themes off a `data-theme` attribute
  on `<html>` and never reads `prefers-color-scheme`. Set the attribute directly.
- `border-style: none` has a **used width of 0px**, so the §4 repair adds a real **1px** (bar height,
  each label's left edge). Below the 4px grain and it cannot reflow the `flex: none` panes, but it is
  not a no-op — this is a USER CHECKPOINT item, not something the AI gate can clear.

**USER CHECKPOINT — CONFIRMED by the user 2026-09-01.** Checked: (a) the 1px border appearing in the merge editor looks intentional and
doesn't crowd the panes; (b) on the *active* palette/combobox row the hint's colour step is now
exactly **1.00x** by design — subordination rests on 11px-vs-13px + right-edge placement, which is
the one place perception can disagree; (c) the 6 unreachable selectors read correctly in the real app.

**Routing correction:** ui-designer filed two pre-existing NITs "-> P99", but P99 is the
production-bundle `repo`-state bug. Both are accent-fill issues and belong with **P100**:
`conflicts.css:201` `var(--accent-fg, #fff)` — **`--accent-fg` is not a token** either, a second
phantom in the file §4 just repaired, surviving on its fallback (prescribed `var(--accent-text)`);
and `dialogs-forms.css:163` `.wt-copy-toggle-on` hardcoding `#fff !important`.

**`.cm-gutters` -> sanctioned decorative** (ui-designer's call): line numbers are universal editor
chrome and a coordinate duplicating visible structure, and the act-carrying text in that pane is
already `--text-2`. Recorded in §2's sanctioned list **with a revisit trigger** — any go-to-line,
line-range or line-naming feature makes them read text.
**Reflog oids (for P101):** `.reflog-oid-old`/`-root` are read text (you read them to pick a reset
target); `.reflog-oid-arrow` clears 3:1 so contrast doesn't force it — move it for **cohesion** only,
flagged as such so the precedent isn't misread as a contrast fix.

**Orchestrator decisions on the contract's three open questions (2026-09-01):**
1. **Accept the `--accent`-fill deferral (contract §5-A) as its own milestone → P100.** White text on
   the `--accent` fill measures **3.22:1 in dark** — below the 4.5:1 bar — and this affects the active
   row's **own primary label**, not just the hint. White is the ceiling, so no hint colour can pass in
   dark; the real fix is the fill (the designer measured a `--selection` recipe: label `--text-1`
   9.36/13.29, hint `--text-2` 5.01/6.42). Out of scope for a colour-swap milestone. Consequence
   accepted for now: in the active row the hint loses its colour step and leans on 11px-vs-13px +
   right-edge placement.
2. **Include the `--border-0` fix (contract §4)** — the one sanctioned non-colour change. `--border-0`
   is **not a real token**; its 5 uses in `conflicts.css` mean the merge editor's split-label bottom
   border and the OURS/THEIRS divider **do not render at all today**. Latent bug in a file P98 is
   already editing; leaving it would be worse than the small scope impurity. Must land as a clearly
   separated hunk. Note it makes a previously-invisible border appear — a real visual change.
3. **Fold in the 8th selector `.pr-merge-method-desc`** (designer excluded it as §5-D to respect the
   locked seven). Reason to override: §2 now asserts "the `--text-3` family is **closed**". A merge-method
   description is text you read *in order to choose* — read-text by the contract's own test — so leaving
   it `--text-3` makes that claim false. It is sanctioned in `ui-reference.md` §12.9, so ui-designer must
   amend §12.9 as part of its P98 design-review pass.

Split out of P95 by orchestrator decision (see P95 decision A). Seven selectors use `--text-3` for
text the user must actually **read**, violating the long-standing `ui-reference.md` §2 rule (these
are read-text, not the enabled-control class P95 sweeps): `.diff-overlay-kind`, `.diff-tree-count`,
`.conflict-editor-split-label`, `.wtctx-branch`, `.wtctx-blocked`, `.combobox-option-hint`,
`.command-palette-option-hint`. The two `*-hint` selectors are the worst offenders.

## ✅ P96 — P93 review follow-ups — DONE + VERIFIED (`cf8bdda`, USER 2026-09-01)

All four items landed; reviewer approved with **zero MUST-FIX**. Scope was the four filed items only
— the "carried in from P93" list below stays recorded context, **not** promoted work.
Gate: frontend tier 4/4 green (78.6s). prPanel suite 35 -> 38 tests, overlayMeta +7.

**Item 4 is worth remembering: it was filed as a NIT and was not one.** "Both effects fire
`onClosePrFileDiff`; idempotent, just a double call" made it look trivial. It took three attempts,
and the first two were wrong in ways only an exact-count test caught:
- Comparing PR numbers fixes only a *same-render* swap. `usePrDiff` calls `setStats` solely from its
  fetch effect, so on a switch `stats` still holds the OLD PR's oid and flips a commit **later** —
  the number guard no longer suppresses and C3 fires a second close. Distinct PRs normally have
  distinct heads, so this was the **common** path.
- Resetting the baseline to `null` then depends on a later oid change to re-establish it. Two PRs
  **can share a head sha**, leaving the baseline stuck at `null` and swallowing the next genuine
  advance — trading a harmless double call for a **missed** close (orphaned overlay showing the old
  head's file). A strict trade-down.
Final shape: the switch episode is bracketed and closed on stats **object identity** (changes when
the new PR's stats land on either the cache-hit or fetch-resolve path, even when the oid does not).
Fails safe toward over-fire on a handler whose prop contract permits it, never toward a missed close.

**Process lesson:** `reviewer`'s round-1 approval of item 4 reasoned only about synchronous
same-commit effect ordering (destroy-before-create) and missed the async late-arrival path; the
orchestrator's own check made the same error. The bug was found only by requiring each new test to
be shown FAILING against the unfixed code. Keep that requirement — a "was called" assertion would
have passed on the very double-fire being removed.

- SHOULD-FIX: add `overlayMeta.test.ts` pinning the load-bearing prefix ordering
  (`conflict:`/`ai-proposal:`/`pr:` before the `WorkdirSection` cast). Currently only indirect.
- NIT: `PrChangesSection.tsx` focus restore resolves the row by positional index into
  `listRef.current.children` — switch to `data-path` + `querySelector` (render-order independent).
- NIT: `overlayMeta.ts:41` `parsePrSlotPath(key) ?? key` surfaces a raw `pr:<oid>:<oid>` key as the
  overlay path for a malformed key.
- NIT: `PrDetailContainer.tsx:550-554` — C2 (unmount) and C3 (headOid change) both fire
  `onClosePrFileDiff` on a PR switch. Idempotent, just a double call.
- Carried in from P93 (deferred there, not blocking): stale `prOverlayCtx` after slot replacement
  (latent, all consumers key-gated); `onManageAccounts` callback identity; the fixture `fail`
  sentinel matches `includes('fail')` too broadly; PR rows ignore `panelDensity` (pre-existing since
  P89, contract §12.5). Full text → archive Part 23.

## ✅ P97 — split ContextMenu.tsx — DONE + VERIFIED (`59d3a41`, USER 2026-09-01)

Strictly behaviour-preserving (refactorer). `ContextMenu.tsx` **486 -> 112 lines**, into
`src/components/contextMenu/`: `MenuList.tsx` (313) and `types.ts` (70).

**Equivalence proof: 113/113 tests identical before and after**, across the same 7 affected test
files. That identity — not the split — was the deliverable.

**The types had to move too, and not for tidiness:** `MenuList` needs `ContextMenuItem`, so leaving
the interfaces in `ContextMenu.tsx` would have created a container<->child import **cycle**.
`ContextMenu.tsx` re-exports all three from the original path via `export type { ... } from
'./contextMenu/types'` — `export type`, not a plain re-export, so it survives
`isolatedModules`/`verbatimModuleSyntax`. **Zero consumer updates**; all ~37 referencing files
untouched, and no barrel `index.ts` (CLAUDE.md prefers narrow explicit imports). `MenuList` was
module-private before and stays unexported from `ContextMenu.tsx`, so the public surface is unchanged.

Net 486 -> 495 total lines: ordinary move overhead (imports, the re-export block, one `export`).
Structure was the goal, not line reclaim.

**Size baseline NOT updated** (deliberate, decision below). `ContextMenu.tsx` at 486 was never a
baselined offender, so the ratchet output is byte-identical before and after: `18 line(s) reclaimed
across 13 file(s)`, 29 offenders over limit.

Pre-existing smells moved verbatim and deliberately **not** fixed (refactorer scope): the
`react-hooks/exhaustive-deps` disable on `MenuList`'s `autoFocus` effect (calls `focusFirst`,
declared later, deliberately omitted from deps), and the index-based `key={i}` on rows, which the
surrounding comment already justifies. Strictly behaviour-preserving
(refactorer), proven by identical before/after test counts.

---

## 🛠️ DX — dev-loop acceleration — in-progress (condensed; full text → archive Part 30)

8 of 10 improvements landed & verified (`3ada322`, `2019e71`, `8e55be8`): dev-profile
`debug = "line-tables-only"` + rust-lld linker, `cargo-nextest` (`pnpm test:rust`), the one-command
gate `scripts/gate.mjs` (clippy in its own `target/clippy` dir), the CLAUDE.md process changes
(concurrent code+design review, velocity mode), and the branches.rs / stash.rs / RepoWorkspace
overlay god-file splits.

Two user decisions that must survive compaction:
- **P75 (IPC codegen) — HALTED 2026-08-21 (user decision).** Linking `tauri-specta` breaks app launch
  on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`). Spike
  reverted; findings + crate pins kept in `docs/contracts/P75-ipc-codegen.md`. Revisit only if
  validated on Windows 11 or with a link-order fix.
- **P76 (native-checkpoint automation) — HELD as contract-only per user (2026-08-20).**
  `docs/contracts/P76-native-checkpoint-automation.md`.

Deferred cleanups (noted, not done): lock the file-size baseline reclaim for App.tsx (P74) and
RepoWorkspace.tsx; de-duplicate the private `open_repo_at` helper across the `git/` modules.

---

## ⏱️ Velocity / gate cost — measured 2026-09-01

All numbers: `docs/history/velocity-2026-09-01.md`. Why it matters: full `pnpm gate` is ≈5-7 min and
orchestration ceremony — not machine time — is ~75-85% of per-task wall clock, which is what the
CLAUDE.md velocity-mode and batching rules exist to cut. Gate child processes now use
`D:\Data\Temp\bonsai-build`, not Defender-scanned `C:\Temp`.

---

## ✅ Accepted decisions that must survive compaction

- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated) · P57 retriever = BM25 lexical, no
  embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, accepted):** new Rust deps `reqwest{blocking,json,rustls-tls}` +
  `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order GitLab →
  Bitbucket → Azure DevOps.
- **v1.0.0 shipped** 2026-08-18 (tag `bd52483`), unsigned; forge/PR flagged beta.
- **P62-P74 native USER CHECKPOINTs were WAIVED and marked `done` 2026-08-20** (user decision).
- All native USER CHECKPOINTs for **P2 → P61** are confirmed; P70, P77, P78/P79/P80, P80b/P81/P82,
  P83, P85-P90, P92, P93 and DEP REFRESH are confirmed too. Full per-milestone text →
  `docs/history/todo-archive-2026-08.md` Parts 17-21 and `todo-archive-2026-09.md` Parts 22-31.

**FOR USER — two open items from the 1.0.0 release I could not close (carry forward):**
1. **Back up `.tauri/updater-prod.key`.** Correctly gitignored and untracked, so it exists in exactly
   ONE place: this working copy. Losing it permanently breaks auto-update for every installed client.
   (The committed `tauri.conf.json` pubkey was verified to match it.) **Also: P71 must not touch it.**
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only, deliberately ignored in
   `pnpm-workspace.yaml`. **The moderate is unidentified** — `gh` is not installed here; both project
   gates are green (`cargo deny` all ok; `pnpm audit` shows only the one ignored high). Check the
   Dependabot page.

---

## 🐞 OPEN follow-ups (spun out — genuine unresolved items, not checkpoints)

Items resolved in the 2026-08-21 fix batch were archived → `todo-archive-2026-09.md` Part 32
(underlying full text: `todo-archive-2026-08.md` Part 19).

### Velocity follow-ups from the 2026-09-01 measurement pass (still open)

Context + all baseline numbers: `docs/history/velocity-2026-09-01.md`. Done in that pass:
proptest banding (`d635464`), doc curation (`0174abf`), 78 → 8 test harnesses (`12882f9`).

- **`prop_status::status_matches_porcelain` is now the workspace's slowest test at 23.5s** in a
  single test fn (measured at full baked counts). nextest parallelizes per test fn, so this is the
  new critical-path floor. Same fix as `prop_graph_layout`: band the input axis into N fns with
  cases allocated proportional to band width. Expected ~23.5s → ~5s.
- **`prop_stash_roundtrip` 14.3s** across 2 fns — same banding treatment, lower priority.
- **`submodule_cli::oracle_add_deinit_remove_roundtrip` 12–14s** — NOT a proptest (a git-CLI
  oracle roundtrip), so banding does not apply; needs its own look if the ~12s floor matters.
- **vitest jsdom construction dominates the frontend leg.** CPU-aggregate across workers:
  `environment` 613s vs `tests` 147s, for 199 files / 2397 tests in 62–68s wall. Try `happy-dom`,
  or `environmentMatchGlobs` so only DOM-touching files pay for one. Untried — measure after.
- **`pnpm gate --quick` is 305s and only drops e2e**, so it is not a fast tier. `cargo nextest
  --workspace` alone is 181s of it. Either add a genuinely narrow tier or lean on
  `--rust` / `--frontend`. CLAUDE.md velocity mode now says so.
- **Ceremony, not machine time, is the dominant per-task cost**: ~75–85% of wall clock. Of the 200
  commits before this pass, 61 were `docs:` bookkeeping vs 24 `feat` + 27 `fix`, and small tasks
  (P92/P93/P95) ran 1h45–2h45 end to end against a ~5–7 min gate. Candidate process changes, NOT
  yet adopted — needs a USER decision: batch small P-tasks through one senior-dev spawn, skip the
  architect contract for single-component fixes, fold the board update into the feat commit.

### ⚠ FOR USER — record inconsistencies surfaced by the 2026-09-01 curation sweep

The curator refused to resolve these itself (it never upgrades a status). All are record-keeping,
not code:
- ~~**P88** headed `in-progress`, **P85 / P86 / P87 / P87d** headed `pending`, against bodies that
  read DONE with checkpoints verified 2026-08-25.~~ **RESOLVED by USER 2026-09-01: all five are
  done and verified.** Headings corrected in `docs/history/todo-archive-2026-09.md` (they were
  already archived); no body text was changed.
- ~~**P88/P89/P90/DEP-REFRESH** all say "UNMERGED/UNPUSHED, awaiting merge decision".~~
  **RESOLVED by USER 2026-09-01: the merge decision is settled.** Re-verified the same day with
  `git merge-base --is-ancestor`: `feat/pr-local-diff`, `perf/git-action-round2` and
  `chore/dep-refresh-2026-08` are contained in **both `dev` and `main`** (all three, not just
  dep-refresh as first noted). The six stale lines in
  `docs/history/todo-archive-2026-09.md` now carry inline corrections; the historical text was kept.
  `feat/p91-observability` remains in no other branch — consistent with the live P91 entry.
- ~~**P84's USER CHECKPOINT was never recorded.**~~ **RESOLVED by USER 2026-09-01: the user
  confirmed P84's checkpoint DID pass**, so P84 is done and verified. Recorded on that direct
  confirmation, not on a contemporaneous 2026-08 record — none was ever written. Its code shipped
  (`cce9eb9`, `90b315c`, `1803391`, `6868be6`); its two contracts are in
  `docs/contracts/archive/` and `todo-archive-2026-09.md` Part 33 carries the corrected status.
- No contract file was ever written for **P94**.

### Hoisted off milestones archived 2026-09-01 (still open)
- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature (`windows-native` → `windows-native-keyring-store`, etc.), drops
  `crypto-rust`, and requires explicit credential-store registration instead of feature-driven
  resolution — i.e. real changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- `no_proxy_client()` in `src-tauri/src/mcp/http_support.rs` still uses
  `.expect("build reqwest client")` — why the missing rustls provider surfaced as a raw panic rather
  than a message. (DEP REFRESH, archive Part 24.)
- **P87b FU-1..4** still open: target row label, commitAmend row, row `role`/`aria-expanded`,
  clickable dock bar. Plus the `AiActivityPanel` aria-label NIT. (P85-P87 batch, archive Part 27.)
- **RepoWorkspace refactor** — still stands for maintainability (not perf); P88's audit re-confirmed
  it. (archive Parts 26-27.)
- **P90.1 deferred:** per-check timing fields; header commit-summary text; command-palette
  `Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.
  (P90, archive Part 25.)
- **Known flake (pre-existing, untouched):** `watcher::tests::git_internals_filtered`
  (`watcher.rs`) is a timing flake (`unwrap_err` on an `Instant`); passes on isolated re-run.
  (P88, archive Part 26.)
- **⚠ FLAG FOR USER (peer session, now ended):**
  `src/components/repoWorkspace/useWorkspaceKeyboard.test.tsx` failed in ISOLATION on the committed
  baseline (1 graph-nav `defaultPrevented` case), introduced by the peer's graph-a11y commit
  `590f2ef`. Likely test-isolation flakiness. Raised in the P86 block; carried here on archive
  (archive Part 27). Status unverified since 2026-08-23.
- **P84 record gap** → written up in `docs/history/todo-archive-2026-09.md` Part 33 (2026-09-01):
  code shipped, USER CHECKPOINT never recorded, contracts archived on user instruction.

### Residue of the two dated 2026-08-22 design reviews (still open)
Both review files now live in `docs/contracts/archive/`; per-finding dispositions +
verification evidence → `docs/history/todo-archive-2026-09.md` Part 35.
- **`graph-design-review-2026-08-22.md` M1 is SUPERSEDED — do not implement.** `role="grid"` /
  `aria-rowcount` / `aria-activedescendant` are forbidden by `ui-reference.md` §4.1 (`:250-252`,
  revised 2026-08-31 by P95). Verified 2026-09-01.
- **`graph-design-review-2026-08-22.md` M2/M3/M4/S2/S3/N1/N2 — resolution unverified.** Not checked
  by the 2026-09-01 sweep (bounded effort); do not assume they landed.
- **`review-2026-08-22-ui.md` NIT-1 — Sidebar ignores `panelDensity`** (confirmed still open
  2026-09-01: no density reference in `Sidebar.tsx`, `src/components/sidebar/**`, or
  `src/styles/sidebar.css`; `.branch-row` is a fixed height).
- **`review-2026-08-22-ui.md` NIT-2 —** `src/components/OnboardingOverlay.tsx:229` is still
  `aria-label="Close"`; the review preferred "Close the tour".
- `review-2026-08-22-ui.md` SHOULD-3 (`--accent` text over `--selection` fails AA) is the **same
  item** as the live P69 **A9** follow-up below — A9 is the canonical entry.

### Known load-flake (still open) — timing-sensitive, not a correctness bug
`ai::session_tests::watchdog_does_not_fire_while_awaiting_input` failed once under load and passed on
immediate re-run.

### P80 forge follow-ups — **OPEN** (SHOULD-FIX/NIT, non-blocking; spun off the archived P80 milestone)
- (a) `forge_set_token_inner` validates before the `host.is_empty()` guard — guard host first to skip
  a wasted round-trip on unparseable origin.
- (b) keychain-write-then-settings ordering: a failed `settings::update` leaves an orphaned keychain
  token (currently `let _ =`) — surface the error.
- (c) re-connecting a migrated legacy `login:None` host creates a 2nd three-part account + orphans the
  bare-host keychain entry (contract §1.2 rekey, optional) — cleanup ticket.
- (e) `ContextMenu` has no separator concept, so the switcher's account/command rows run contiguous
  (same gap as the P69i identity menu) — add a separator item.
- (f) Settings Accounts group ordering is alphabetical only (no repoId in scope for "current host
  first").
- (g) disabled Default radio's `aria-describedby` points at a `hidden` span — switch to a
  visually-hidden class.
- (h) switcher trigger has no busy affordance during a pin/reset write (menu shows aria-busy; trigger
  doesn't) — consider `opacity:0.6`. (i) §1.1 wireframe middot between host and caption omitted
  (cosmetic).

### `cargo fmt` has never been run on this repo — **OPEN**
No `rustfmt.toml` anywhere, no fmt check in any hook or CI. `cargo fmt --all --check` reports **1773
hunks across 221 files**; `--config use_small_heuristics=Max` is *worse* (2065). Right shape: its own
commit — pick a config, add `rustfmt.toml`, one-shot reformat, then add `cargo fmt --check` to the
gate. **Do it between milestones, never inside one.**

### Audit #2 remainder — **OPEN** (all confirmed bugs & SHOULD-FIXes fixed 2026-08-18/19)
Full audit `docs/audit-2026-08-18.md`; the resolved fix-batch mapping is in archive Part 16. Still
open: **§4.3–§4.8 test gaps** (CommandPalette/NumberSlider pins once fixed, streaming-graph e2e,
08-stash conflicted-apply fixture, Linux case-sensitivity assertions, low-value untested units,
missing journeys: updater / AI-PR-description / clone-init / worktrees) · **§7's 13 NITs** (recorded
in the audit, no action required) · **§5.6** perf/visual ACs stay USER CHECKPOINT (the headless
harness cannot observe rAF/compositing).

### P68 contract debt — **OPEN** (P68 is done, but its contracts are stale/oversized)
- `docs/contracts/P68e-ai-activity-dock.md` is **1064 lines** (twice the ~500 house limit) and now
  under-describes shipped code: P68g-1 added two elements to the ask block (an untrusted-model-output
  attribution line + a fixed "Bonsai never asks for passwords or tokens" guard) and made
  `aria-describedby` a two-id list, none of which §4.1/§4.2 describe. `ui-designer` produced
  splice-ready replacement blocks in `docs/contracts/P68g-ui.md` §3.1–§3.5. **Needs: apply the splice,
  then split the file.**
- `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale — says
  `session_drain_tests.rs` is `#[path]`-included "as a child of `session`"; after the split it is a
  child of `session::session_drain` (still a descendant, so the privacy claim holds; the wording is
  out of date). P68 invariants D1–D16 remain canonical in that contract — do NOT "fix" them back.
- P68 security follow-ups (audit items 7–11) still OPEN; rationale in
  `docs/contracts/P68-security-audit.md` (canonical): the novel-content gate (structural defeat for
  H1), proposals shown as a diff, bulk path-count cap + per-batch reads + batch count in the dialog,
  process-group kill off Windows (the pid-zeroing half landed in `67539fd`), and a symlink-safe
  `resolve_conflict_text` write.

### P69 Settings follow-ups awaiting a user decision — **OPEN** (nothing is blocked on them)
- **A8 — bundle the two specced-but-unimplemented items into one increment** (both `ui-designer` and
  the orchestrator recommend bundling): (a) the help-text highlight fallback,
  `docs/contracts/archive/P69-settings-ui.md` §3.2.1, `[NOT IMPLEMENTED]` — the flagship query `graph` returns
  5 hits and highlights **nothing** (every hit matched via `keywords`/`help` while the labels read
  "Row height" / "Lane width" / "Compact rows"); and (b) the half-landed draft-hint feature, §13. Note:
  the draft-hint CSS is genuinely **dead** but costs no visible layout today — the case for A8 is the
  missing feature, not a rendering bug.
- **A9 — a scoped a11y sweep of `color: var(--accent)` on text.** Fine on `--bg-0/1/2`; a latent AA
  failure anywhere accent text lands on a `--selection` fill (measured 3.51–3.74:1). ~30 call sites,
  unaudited. Now **prohibited** in `docs/contracts/ui-reference.md` §2 so new code cannot add to the
  backlog. The one deviation P69k shipped: the rail hit-count is `--text-1`, not the `--accent`
  ui-designer ruled for (accent as 11px text measures 3.74:1 / 3.51:1 on a selected item's
  `--selection` fill); the exact declaration to flip is marked in `settings-shell.css`.
- **A3 — the frozen AI gate-note copy is still unsigned.** §5.4's replacement for
  `Turn on "Enable AI features" above to change these.`; ui-designer prefers
  `These take effect once AI features are on.` The current string ships until the user rules.

### P77 tag-sync deferred follow-ups — **OPEN** (carried off the archived P77 milestone, 2026-08-21)
- **Collapsed-rollup needs first expand (FOR-USER decision):** §1.2 wants "see a problem without
  expanding", but the ls-remote check only fires on the first Tags expand per session (to avoid an
  eager network call on every repo open). So the `⚠ N` rollup can't appear until the user expands Tags
  once. Decide whether a cheap unprompted first check on repo-open is worth the network cost.
- NIT: rollup aria-label lacks singular/plural ("1 tags"); `useTagSync` re-hits network on rapid
  collapse→expand while `unavailable` (no cache stamp on the error path); confirm dialogs close
  optimistically so `busy` never paints (matches existing house pattern); tag-filter box gate counts
  local tags only (a repo with only remote-only tags shows no filter); item-7 "Delete tag on origin…"
  also shows on remote-only ghost rows (coherent — only place the tag exists).
- Backend NIT: `delete_remote_tag` doesn't `evict_fresh_on_auth_fail` (matches existing `push_tag`);
  `validate_tag_name` duplicated from `tags.rs` (module-private) — promote to shared if a 3rd caller.
  Full P77 detail: `docs/history/todo-archive-2026-08.md` Part 18.

### macOS ad-hoc code signing — config DONE 2026-08-30, **RELEASE STILL PENDING**
Only the pending half stays here; full detail → `docs/history/todo-archive-2026-09.md` Part 34.
- `bundle.macOS.signingIdentity: "-"` is in `src-tauri/tauri.conf.json` but **has not shipped**: the
  last tag is `v1.5.0` (2026-08-26), which predates the fix. It takes effect on the next tagged
  release — verify the sealed ad-hoc signature then.
- Not fixed by ad-hoc at all: Gatekeeper "unidentified developer"; a new version re-prompts once for
  TCC (cdhash changes). Full fix = Developer ID + notarization (Apple Developer Program); the
  `APPLE_*` env block in `.github/workflows/release.yml` is already scaffolded.

---

---

## Archive

**Start at `docs/history/README.md`** — it is the navigable index of every archived milestone and
part number. The table below is the short form.

| File | Covers |
|---|---|
| `docs/history/README.md` | **The archive index** — which file/part holds which milestone. |
| `docs/history/todo-archive-2026-09.md` | **Parts 33-35 (moved 2026-09-01):** the P84 record gap (code shipped, USER CHECKPOINT never recorded) · the macOS ad-hoc-signing detail (release still pending) · per-finding dispositions of the two dated 2026-08-22 design reviews. **Parts 22-32 (moved 2026-09-01, verbatim):** P94 · P93 + P92 · DEP REFRESH 2026-08-28 · P90 + P89 · P88 · the P85-P87 perf+observability batch incl. P87c/P87d · P82 + P83 · divergence reconcile + Release 1.1.0 + P80b/P81/P82 · the full DX dev-loop text · the full confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 → P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail, the 2026-08-17 batch mapping, resolved spun-out items. **Parts 10-16 (moved 2026-08-20): the P62-P74 checkpoint waiver + P71, P72, P73, P74, the P69 Settings redesign, and the Audit #2 fix batch, condensed. Parts 17-18 (moved 2026-08-21): P70 and P77, both checkpoints verified. Part 19 (moved 2026-08-21): the OPEN follow-ups resolved in the 2026-08-21 fix batch (read_status/palette/refetch/stash/submodule/STDERR/cred-split), verbatim. Part 20 (moved 2026-08-21): P78/P79/P80 forge milestones, condensed. Part 21 (moved 2026-08-21): P80b/P81/P82, done + checkpoints confirmed, condensed.** |
| `docs/history/todo-archive.md` | P27 → P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board.
