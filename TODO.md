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
To drive the git dock in the harness you must seed `bonsai.mockUiSettings` (`{onboardingSeen:true}`)
and `bonsai.mockSession` (`{openRepos:['C:\mock\bonsai-fixture']}`) into localStorage before load —
otherwise you land on the no-repo empty state; and select the toolbar Push button by its `title`, not
its `aria-label` (the label changes with the ahead-count once a push has landed).
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

---

---

## Where the rest of the board went

Full detail for everything compacted out of this file is in `docs/history/` — start at
`docs/history/README.md`. The **Archive** table at the bottom is the short form. Nothing below was
closed by the curator: a pending USER CHECKPOINT, an owed AI-gate item and an open follow-up all
stay here however old they are.

**2026-09-16 pass — Parts 71-75.** P112's four sub-increments all landed and its AI gate is green,
so the **build and review transcript** moved out while **the milestone, its status
(`awaiting USER CHECKPOINT`) and all five checkpoint items stayed** — the rule Parts 37-40 set.
Archived: the sub-inc 3/4 transcript incl. the `P112-ui.md` §17 rulings, the four bad citations, the
coalescing lesson and the sub-inc-3 audit (71) · the superseded gate states and the completed
2026-09-14 queue — F6, P77, the e2e measurement, the UNC clearance (72) · the sub-inc 2 + P113
phase-1 review transcript (73) · the two items **closed 2026-09-16** with their evidence, plus the
`.cmd` launch audit (74) · superseded curator bookkeeping and one consolidated duplicate (75).
**Not archived:** the P112 USER CHECKPOINT, the two decisions owed by the user, the four user
actions, the nine-file second review pass (in flight), every open follow-up, both ruling ledgers,
the durable rules, the accepted decisions.

**The previous pass truncated instead of archiving, and it had to be undone.** `c5b3ea5` cut 1950
lines from this file without writing them anywhere; `67e2ce6` restored them wholesale. Every range
removed on 2026-09-16 was extracted to `docs/history/` **first** and diffed byte-identical against
`git show HEAD:TODO.md` **before** removal — 20 ranges, 935 lines, all 20 verified.

**Earlier passes.** 2026-09-14 → Parts 62-70 (the 22 FOR-USER rulings); 2026-09-10 → Parts 54-61
(the eight confirmed native checkpoints); 2026-09-03 → Parts 36-53, plus a staleness sweep that
found **11 of 35 open entries had drifted**; 2026-09-01 → Parts 22-35. The board's own record of
being wrong is kept deliberately.

---

## ⏸ RESUME HERE — updated 2026-09-16

**Current step:** see the **P112** entry below — *all four sub-increments in, AI gate green,
awaiting USER CHECKPOINT (native window)*. That entry owns the line; keep it updated there, not here.
Its five checkpoint items are the only thing left in P112.

**Branch `feat/post-p91-rulings`, no upstream — 86 commits ahead of `origin/dev` (`8b88efd`),
unpushed, and it stays unpushed (ruling #25, do not raise it again).** HEAD is the board commit
below `934a280`. Measured 2026-09-16 with `git rev-list --count origin/dev..HEAD`: **85 at
`934a280`**, +1 for this board commit. The curator's "80" was true when measured, before the six
commits of the 2026-09-16 review-and-split session; the earlier "18 commits ahead, last commit
`8026622`" was 2026-09-14 and had already gone stale (archive Part 75.2). **Each of these three
numbers was correct when written** — which is the argument for measuring rather than carrying one
forward.

**The P91 branch merge is DONE (2026-09-11, ruling #1)** — `feat/p91-observability` was
fast-forwarded onto `dev` and pushed. Every "DO NOT MERGE" / "unmerged by user instruction" line
this board used to carry is **void**; where one survives inside an archived part it is history, not
instruction. Curator-verified 2026-09-14: `dev` = `origin/dev` = `8b88efd`, and
`git rev-list --count cb70f4a..8b88efd` = **165** — the ledger's "164" was measured before `8b88efd`
(the jbcontext commit of ruling #2) existed. Both were true when measured.

### 🚨 PROCESS FAILURE (mine) — I let one review diff grow to 41 modified + 13 new files

`CLAUDE.md` says **"commit each approved sub-increment so review diffs stay small and resume points
exist."** I did not. P113 phase 2 came back **approved with one MUST-FIX**; instead of committing the
approved parts and reviewing only the fix, I routed the fix, then stacked the P112 bridge on top, then
stacked A5 on top of that. By the final review `git diff HEAD` was no longer "exactly the increment" —
**13 new files and most of the 41 modified predated the pass being reviewed.**

**The cost was not hypothetical.** The reviewer had to open with a scope note and name **nine files it
did NOT re-review**: `useMcpControls.ts`, `useToastQueue.ts`, `SettingsMcpSection.tsx`,
`useUiSettings.ts`, `mcpOutcomeSlots.ts`, `useOutcomeScrollCorrection.ts`, `useSettingsOpenSignal.ts`,
`useSettingsSaveFailure.ts`, `settingsToastGuard.test.tsx`. **Those carry exactly one review pass**,
and it happened before two subsequent passes changed files around them. That is a real gap, not a
bookkeeping complaint — and it is the direct consequence of my batching.

**Fix, applied from here:** commit the moment an increment is approved, even when a MUST-FIX is
routed; review the fix against a small diff. The nine files above should get a targeted second pass
before this branch is considered done.

**Status of that second pass: `in-progress`.** A `reviewer` agent is executing it as of 2026-09-16.
No outcome is recorded here because none exists yet — do not read this entry as closed.

# ✅ P112 — AI GATE GREEN, ALL FOUR SUB-INCREMENTS IN. **USER CHECKPOINT IS THE ONLY THING LEFT.**

**Per the workflow, a milestone is done when BOTH halves pass. The AI half is done; the native half
is not, and I must not self-confirm it.**

**Current step: P112 — all four sub-increments in, AI gate green, awaiting USER CHECKPOINT (native
window).** This is the canonical `Current step:` line; `## ⏸ RESUME HERE` points here.

**AI gate, 2026-09-15 at `9fca997`: 437.6s, exit 0, all 8 steps, zero FAIL lines.** nextest 136.5s
(**2556 passed, 10 skipped**) · doctests 3.5s · clippy 0.94s · eslint 13.2s · ratchet 0.69s · vitest
58.1s (**2963 passed, 265 files**) · tsc+build 10.9s · e2e 213.7s (**185 passed, 1 skipped**).
**Windows-only evidence** — unchanged by this green; see the CI note below.

## 🚨 THE USER CHECKPOINT — `pnpm tauri dev`, then Settings → General

Five things, and **not one of them is reachable from any tier here**:
1. **The page itself.** Two free-text command fields are **gone**, replaced by a strict picker and a
   `Browse…` button. Verified by me in the **mock** harness only.
2. **The popover's real scroll geometry** (UC-UI-1) — the editor picker sits near the bottom of the
   pane; the harness measured it at one synthetic viewport.
3. **Focus returns to `Browse…`** after the OS modal closes (UC-UI-2), per platform.
4. **The dialog's title and filter as Windows renders them** (UC-UI-3), and on macOS that a `.app`
   bundle is selectable as **one** item. Also: **does the dialog behave as a child of the Bonsai
   window** — it is built from `AppHandle` with **no `set_parent`**, and I deliberately did **not**
   add one blind (`tauri-plugin-dialog` **2.7.2**; the auditor could not establish statically whether
   it already attaches to the focused window, so the call could be a no-op or could misbehave, and
   neither outcome is observable without the window).
5. **Whether a real 2.1 s cold scan reads as *loading* rather than *broken*.** Measured, not
   estimated: 55 PATH dirs × 11 `PATHEXT` ≈ 4400 stats; 0.45 s warm. The contract decided a
   placeholder is enough. That decision has never been seen by a human.

## ⚠ STILL UNVERIFIABLE FROM HERE — do not let a green gate imply otherwise

**`pnpm gate` runs Windows only; `.github/workflows/ci.yml` runs
`[ubuntu-22.04, windows-latest, macos-latest]`.** The AMEND-8 host-bound test fix is therefore
**reasoned, not executed** — the unix accept chain was traced line by line, but **ruling #25 keeps the
branch unpushed, so CI cannot run it either.** The first real CI run is the verification.

## Decisions still owed by the user (neither blocks the checkpoint)

1. **`CLAUDE.md` trigger** — whether `BrowsedProgram::from_settings_field` joins the mandatory
   `security-auditor` path trigger. Not added unilaterally: the sibling rule is a user ruling.
2. **BLOCKING one fix** — what a **partial** account removal tells the user.
   `forge_remove_account_inner` discards both the `delete_token` and `settings::update` results and
   returns `Ok`, so **"account removed" can be reported while the token is still in the keychain.**
   The two swallowed calls fail differently, so the copy must be able to say which half happened.

## Follow-ups, ranked, none blocking

- **✅ SPLIT 2026-09-16 (`934a280`) — both zero-slack files now have room, and the baseline is
  regenerated so the gain is locked.** `App.tsx` **590 → 559**, `useSettingsPanelAdapter.ts`
  **499 → 445**, `SettingsPanel.test.tsx` **526 → 325**. New files: `useMcpWiring.ts` (95),
  `useSettingsConsentGates.ts` (123), `settingsPanelKit.tsx` (134),
  `mcpOutcomeLifetime.test.tsx` (90). Equivalence **2978 = 2978**, with the 27 `it()` titles diffed
  against the 23 + 4 they became. `scripts/file-size-baseline.json` regenerated — `App.tsx` 590→559
  plus **14 lines three untouched files had already reclaimed** (`ai_digest_cli.rs` 525→519,
  `ai_stream_bulk_cli.rs` 532→525, `RepoWorkspace.tsx` 2265→2264). **Shrinking only REPORTS a
  reclaim** — without `pnpm lint:size -- --update-baseline` the record is not rewritten and the file
  creeps back unnoticed. That is why the regen is part of the increment.
- **🆕 STILL PACKED, deliberately: `App.tsx:480`/`:481` (and `:423`/`:426`).** Same defect class as
  the `:506` line `934a280` removed — `:481` packs six props including `onChange`. **Why they were
  NOT taken:** unpacking costs ~+7 lines, which was safe against the old 590 but now **grows a
  freshly-baselined 559 file and fails the ratchet** unless paired with another extraction. So this
  is an extraction task, not a formatting one.
- **`Combobox.tsx`'s NUL byte** is fixed in the working tree but git will class the *pair* binary
  until the commit that lands it is itself the base — so the shared control was undiffable during
  its own review.
- **`cargo doc` reports 127 pre-existing findings** crate-wide (not a gate step; doctests are, and
  are green with `-D warnings`). One lands on `PickedTool` — public doc links to a private item.
- **Narrow, reasoned-not-tested:** a stale rescan landing after a Browse confirm. The mock **cannot
  reproduce it** — it builds its payload at *resolve* time where the real command reads settings at
  *entry* — so a regression guard needs a mock change.
- `useExternalToolScan.ts`'s mount-recovery-scan rejection and the second-browse-failure token case
  are reasoned only, for the same reason.
- **StrictMode** double-runs the owed-adopt mount effect → two identical microtask `report`s (legal,
  one visible note, one extra dev `flushSize`). No e2e reaches that state.

# ✅ The 2026-09-14/15 queue — what landed (full detail: archive Part 72)

- **P112 sub-increments 1-4** — all in. Sub-inc 2 `a2eb091`, sub-inc 3 `d0e6cf0`, sub-inc 4
  `e13ff2d` + `9fca997`. **AI half done, native half not** — the milestone entry above is the live
  record, and the curator did not upgrade it.
- **F6 — `usage.json` 90-day window + deletable** (ruling #3) — `done` 2026-09-14, `d46c98e` +
  `b53618a`. Both reviews approved; R13a/R13b/R13c fixed; **all five new mock seams verified in the
  harness by the orchestrator**, including strings that had never once been rendered. Narrative and
  the seam table: Part 72.2.
- **P77 — trigger `list_tag_sync` on auto-fetch completion** (ruling #11) — `done` 2026-09-14,
  `d46c98e`. Rides the existing 5-min cycle; no repo-open call. No `useJobStatus` test file exists at
  all (pre-existing gap); the receiving end is covered.
- **The e2e cold-timing MEASUREMENT** (ruling #9) — `done` 2026-09-14, `6a6f284`. **102 s cold bundle
  vs 191.4 s dev**, build included, cold-vs-warm 1 s. **Not flipped** — the decision now has its
  number and remains the user's.
- **The UNC / `\wsl$` `canonicalize` check on `216ca45`** — ship-blocker **cleared** 2026-09-14 by a
  real UNC probe. `\wsl$` and OneDrive placeholders remain untested — see the section below.
- **Superseded gate greens** — `d0e6cf0` 457.5s, `dcff54b` 454.0s, the 427.4s confirming run over
  HEAD's source tree, and the `e9ed93d` Rust tier 386.0s. All superseded by `9fca997`; see
  `### Verification state`. Parts 72.1-72.4.

### Still open from that queue (numbering as filed)


6. **`h_ai` stub isolation** — the only code item left besides P112. Serialised for now by a nextest
   test-group (`d46c98e`, both `profile.default` and `profile.ci`; 57/57 in 45 s). **Root cause found,
   not fixed:** twelve test modules each define their own `env_lock()` with its own `static LOCK`
   (`tests/ai/ai_changelog_cli.rs:37`, `ai_commit_cli.rs:42`, `ai_explain_cli.rs:40`, …) — twelve
   mutexes guarding one process-global `BONSAI_STUB_MODE`/`BONSAI_CLAUDE_BIN`. One-line fix per file:
   delegate to a single `crate::common::env_lock`. Also note `scripts/gate.mjs:148`'s
   `cargo test --workspace` fallback gets **no** test-group serialisation, so the nextest fix does not
   cover it.

### Four USER ACTIONS — only the user can clear these

- **Boot `pnpm tauri dev` with Dev mode ON** so P91's owed AI-gate item (a real `logs/*.jsonl` parse)
  can be done (ruling #16). Verified 2026-09-11: no Dev-mode key in the persisted `settings.json`, so
  it cannot be pre-set from disk, and the 2026-09-10 confirmation run produced no `logs/` directory.
  **Any boot is not enough — Dev mode must be on.**
- **Back up `.tauri/updater-prod.key`** (ruling #14, deferred). Gitignored and untracked, so it exists
  in exactly ONE place: this working copy. Losing it permanently breaks auto-update for every
  installed client. The committed `tauri.conf.json` pubkey was verified to match it. **P71 must not
  touch it.**
- **Identify the Dependabot moderate alert** (ruling #15 — "not now", and **no `gh` install
  authorised**, so the orchestrator cannot read the page). The *high* is the known `nanoid`
  GHSA-2v37-7h3g-55p8: build/test tooling only, deliberately ignored in `pnpm-workspace.yaml`.
- **macOS ad-hoc signing** — ⏸ PARKED 2026-09-11 as blocked-on-release, not open work (detail below).

### Verification state

- **Full 8-step gate GREEN at `934a280` — 2026-09-16, 414.2s, exit 0, all 8 steps, zero FAIL
  lines.** nextest 147.2s (**2556 run, 2556 passed, 1 leaky, 10 skipped**) · doctests 3.9s · clippy
  1.3s (**cache, not a change in work**) · eslint 14.1s · size ratchet 668ms · vitest 56.9s
  (**2978 / 267 files**) · tsc+build 11.5s · e2e 178.7s (**185 passed, 1 skipped**).
- **Read from the `gate summary` block in the log file, not from the wrapper's exit status** — the
  backgrounded wrapper also reported 0, which is exactly the coincidence the gate-running rules warn
  about. Log: `D:/Data/Temp/claude/bonsai-gate/gate-934a280.log`.
- **The 1 leaky is the KNOWN one** — `h_misc external_spawn::detached_spawn_ignores_nonzero_exit`,
  recorded as intermittent (0 or 1 across runs) and settled as a detached child's timing, not a
  defect. No new information.
- **Rust is unchanged this session and the count proves it:** 2556 matches `d0e6cf0` exactly. The
  8-test drop from `dcff54b`'s 2564 predates today — sub-inc 3 **deleted** `external_cmd.rs` and its
  validator, which took their tests with them.
- **Pre-gate machine state, recorded so a future slow number is attributed and not inferred:** CPU
  **40%**, 18 `node` processes (this session's own tooling — no `cargo`, `rustc` or `vite`), port
  1420 free. Not an idle machine; the e2e leg passed anyway.
- **⚠ The previous entry claimed `9fca997`'s green covered HEAD because only `TODO.md`-only commits
  followed it. That went FALSE the moment `2b3dfd6` landed ten `src/` files**, and four more `src/`
  commits followed. Superseded rather than patched, because piecemeal amendment is how a
  verification block starts lying — the same failure the 2026-09-16 curation pass fixed here.
- **It is Windows-only evidence, and must not be read as three platforms.** `pnpm gate` runs Windows;
  `.github/workflows/ci.yml` runs `[ubuntu-22.04, windows-latest, macos-latest]`. The AMEND-8
  host-bound test fix is **reasoned, not executed** — the unix accept chain was traced line by line —
  and **ruling #25 keeps the branch unpushed, so CI cannot run it either.** The first real CI run on
  this branch is the verification.
- **Exit code 0 is not sufficient evidence, and neither is a piped log** — see `### The gate-running
  rules` below, which those two facts earned.
- Port **1420 is free**. Keep it so: `strictPort: true` means a held port breaks `pnpm tauri dev`.
- Superseded gate states: archive Parts 58 and 72.

---

> **Curator note, 2026-09-14 — the ledger below is reproduced verbatim and is authoritative.** One
> thing around it changed: the evidence blocks its preamble points at ("the sections that follow")
> were archived to `docs/history/todo-archive-2026-09.md` **Part 63** once every item was ruled, and
> the second round of rulings was moved up to sit directly beneath it. Nothing inside either block
> was shortened.

## ✅ USER DECISION LEDGER — 2026-09-11 (all 17 open items ruled)

**Every FOR-USER decision below this section is now RULED.** The evidence that justified each
ruling is kept in place in the sections that follow; this ledger is the authoritative record of
*what was decided*. Do not re-open any of these without the user.

| # | Item | Ruling (user, 2026-09-11) |
|---|---|---|
| 1 | Branch `feat/p91-observability` | **MERGE to `dev`.** See the correction note below — the merge is a **fast-forward of 164 commits**, not 30. |
| 2 | Uncommitted `CLAUDE.md` + `context-explorer.md` | **COMMIT them** (jbcontext CLI→MCP migration), fixing the tool-list line that contradicts the agent's own frontmatter. |
| 3 | F6 `usage.json` | **Stay always-on, but 90-day window + deletable.** Statistics page stays viable. Disclosure copy still required (naming the `metrics` folder, not the file). |
| 4 | Security MEDIUM-2 `terminalCommand`/`editorCommand` | **Validate the shape now** (absolute path to an existing executable, no shell metachars, no arg injection) **and put removal of user-supplied commands on the roadmap.** |
| 5 | Username / home masking in raw log paths | **Mask the home prefix — and it MUST be cross-platform** (user's explicit addition): resolve the actual home dir per-OS so Windows `C:\Users\x`, macOS `/Users/x` and Linux `/home/x` all collapse. Do NOT pattern-match `C:\Users`. |
| 6 | Security LOW-1 cwd DLL search order | **Fix it in the same increment as #4's validation** (one call site, near-free while there). |
| 7 | D3 `.op-worktree-warning` | **Repaint as the warning hue.** Danger stays reserved for destructive/irreversible actions. |
| 8 | happy-dom | **ADOPT** (`docs/proposals/happy-dom.patch`) **and also do the lazy `window.location` fix** in `src/ipc/mock/repoState.ts:160`. jsdom stays installed so the shim self-guard stays meaningful. |
| 9 | e2e bundle default | **MEASURE and REPORT, do NOT flip.** One cold `E2E_BUNDLE=1` run vs the 162s dev figure *including build*. The number goes on the record; the default does not change without the user seeing it. |
| 10 | P69 A3 AI gate-note copy | **ui-designer finalises it** with the surrounding copy in view; its signed string ships. |
| 11 | P77 tag-sync check | **Fold into the existing auto-fetch cycle** (on, 5-min). No new trigger, no repo-open network call. |
| 12 | Process changes | **ALL THREE ADOPTED** — see the new rules block below. |
| 13 | P108 `AC11` | **ACCEPT the source-derived figures** for the two unreachable states; record the limitation and **close AC11**. |
| 14 | Back up `.tauri/updater-prod.key` | **Not now** — stays on the board as a user action. Still single-copy; still breaks auto-update for every installed client if lost. |
| 15 | Dependabot moderate alert | **Not now**, and **no `gh` install authorised** — so the orchestrator cannot read the page. Stays open as a user action. |
| 16 | P91 owed AI-gate item (real `logs/*.jsonl`) | **User will boot `pnpm tauri dev` with Dev mode ON.** Orchestrator parses the files once they exist. Verified 2026-09-11: no Dev-mode key in persisted `settings.json`, so it cannot be pre-set from disk. |
| 17 | macOS ad-hoc signing | **PARK as blocked-on-release.** Re-raise when a tag is next cut. Not open work. |

### Correction to the board's own commit count (verified 2026-09-11)

The RESUME block said "30 commits ahead of `5c2dcd2`". Measured:

- `dev` is at `cb70f4a` and is a **strict ancestor** of this branch → the merge is a **pure
  fast-forward**, no merge commit, nothing to resolve.
- `cb70f4a..5c2dcd2` = **126 commits**; `5c2dcd2..HEAD` = **38** (the board's "30" is stale by 8).
- Total landing on `dev`: **164 commits**, of which 126 predate the board's own reference point.
  The "30 commits" framing described only the recent P91 window, not what `dev` has never seen.
- Also on the remote: `origin/Dev` (capital D) at `691f48b`, a separate ref from `origin/dev`
  (`cb70f4a`). A case-collision artefact — not touched, but do not confuse the two.

### The three process rules adopted 2026-09-11 (user)

1. **Batch small P-tasks through ONE senior-dev spawn.** Every fresh subagent re-pays this
   CLAUDE.md + its agent def (~6-8k tokens) before doing anything.
2. **Skip the architect contract for single-component fixes.** Accepted cost: nothing survives on
   disk if the session dies mid-increment, so keep such increments short.
3. **Fold the board update into the feat commit.** Accepted cost: a docs-only commit can no longer
   be identified as such from the log.

---

## 🆕 THIRD ROUND OF USER RULINGS — 2026-09-14 (two, from the harness measurement)

Same authority as the 23 before them. Both were asked with evidence in hand, not speculatively.

| # | Item | Ruling |
|---|---|---|
| 24 | Settings toasts render behind Settings' own `.dialog-overlay` — scope of the fix | **SWEEP EVERY CALL SITE** (asked as "all 17"; the true figure is **10** — see the correction below). Not the delete outcome alone (which is what I recommended, on the grounds that it was the highest-stakes one and would build the recipe cheaply). The user chose the full surface. So: every `pushToast` reachable from Settings moves to an inline note, bringing the surface into line with `ui-reference.md:2377`'s standing rule instead of leaving 16 known violations behind a fixed one. |
| 25 | The 30 unpushed commits on `feat/post-p91-rulings` | **DO NOT PUSH.** Stays local. **Do not raise this again** — it has now been asked and answered, and re-raising it is noise. |
| 26 | UNC paths for external tools — refused by **both** detection and Browse, so a share-installed tool was unusable with no workaround | **ALLOW UNC VIA BROWSE ONLY.** Detection keeps refusing it — stat-ing a share inside the 1500 ms budgeted scan can hang or go over the wire. An explicit pick through the native dialog is a deliberate one-time act naming an exact file, so it is allowed. **This overrides contract line 577**, which mandates UNC refusal on the browse path; `custom::is_absolute_for`/`is_unc` must be amended for sub-inc 2/3. `looks_absolute` (detection) keeps refusing UNC. |

**Scope facts**, so the next session does not re-derive them:
- **CORRECTION.** The question was put to the user as "17 call sites". **The real figure is 10
  invocations in 2 files** — the 17 came from a `wc -l` that also counted `usePushToast()`
  declarations and `[pushToast]` dependency-array entries. The ruling is unaffected (a sweep is a
  sweep) but the increment is materially smaller than the user was told when deciding. The
  exhaustive list:
  - `src/components/settings/categories/DevCategory.tsx` — `:96` (**reveal/"open logs folder"
    failure** — I mislabelled this as the delete outcome when briefing; `ui-designer` corrected it),
    `:115` (export success), `:118` (export failure), `:149` (**the delete outcome — this is the one
    I measured as occluded**), `:152` (delete thrown path)
  - `src/components/settings/SettingsAccountsSection.tsx` — `:57` (token page), `:79` (default
    account), `:97` (remove host), `:138` (added login), `:149` (connected)
  **⚠ THAT LIST IS INCOMPLETE AND MY "VERIFIED EXHAUSTIVE" CLAIM WAS WRONG. The real count is 15.**
  I searched by **directory** (`grep -rn "pushToast(" src/components/settings/ src/components/Settings*.tsx`)
  when the right question is **reachability**. Hooks that take `pushToast` as a **parameter** and are
  wired from `App.tsx` raise Settings toasts from outside those directories and are invisible to that
  grep. The five I missed — found by the P113 implementer, then verified by me:
  - `src/hooks/useMcpControls.ts` — `:69` (start/stop MCP server), `:80` / `:82` (register with Claude
    Code), `:103` (allow-write). All **Settings rows**; wired at `App.tsx:208`.
  - **`src/hooks/useUiSettings.ts:287`** — `Could not save settings: {e}`, raised on **EVERY failed
    settings write**, wired at `App.tsx:112`. **This is the highest-traffic Settings toast in the
    application** — every toggle, radio and field goes through that patch path — and it has been
    rendering behind the scrim like all the others. Throttled by `if (streak === 0)`, not that it
    helps visibility.

  **Not in scope (checked):** `useExternalTools.ts:22/28/34` are triggered from the repo UI, not
  Settings — though P112 sub-inc 4's picker could make them Settings-reachable, so re-check then.
  `useAppCommands.ts` and `useRepoTabs.ts` are not Settings-reachable.

  **Ruling #24 said "sweep every call site", so these are in scope by the ruling's own terms** — the
  work is simply not finished. P113 as contracted and implemented covers **10 of 15**. `ui-designer`
  has been asked to place the remaining five; `useUiSettings:287` is the hard one, being a *global*
  save failure with no obvious row to attach to.

  **The lint guard cannot see either hook**, because the call sites live outside
  `src/components/settings/`. If the guard is what keeps this from regressing, it needs a different
  mechanism — flagged to `ui-designer` and to the reviewer.

  **Method note, since I have now given three different counts for this sweep (17, then 10, now 15):**
  a grep scoped to a directory answers "where are the calls in these files", not "what can a user
  trigger from this screen". The second question spans the call graph. Count by reachability.
- **The `--warn` note recipe is NOT built.** `ui-designer` called the fix "pure reuse of an existing
  signed recipe"; that is true of the *design* and false of the *code*. `.settings-row-note` is
  widely used with its rule at `src/styles/settings-primitives.css:180`, but
  `.settings-row-note--warn` has **zero users and no CSS rule anywhere in `src/`**. The recipe must
  be written before it can be reused — 12% `--warning` tint, `inset 3px 0 0 var(--warning)`,
  **`--text-1` ink** (the `--text-1` is what passes AA in light).
- Two riders from `ui-reference.md`: the note sits **beside** the row's state note, not instead of
  it, with `aria-describedby` composing both ids and the element permanently present with empty text
  when idle (`:empty` collapses its chrome; it must **not** be `display: none`, which costs the
  announcement). And **count live regions per section, not per element** (`:2386`) — if the note
  goes live, the `DevCategory` announcer must not also fire, or AT queues one sentence per note.

---

## 🆕 SECOND ROUND OF USER RULINGS — 2026-09-11 (four more, from the review findings)

These came out of the reviews of the first increment, not from the original 17. Same authority.

| # | Item | Ruling |
|---|---|---|
| 18 | MCP audit scope (`stage_paths` HIGH + MEDIUM + 4 LOWs) | **DO EVERYTHING IN THE AUDIT.** Not just the symlink guard. **QUEUED, not started.** |
| 19 | MCP review gate | **Snapshot test + review trigger.** A test snapshotting `list_all()` tool descriptions, **and** a rule that any diff touching `crates/bonsai-mcp/src/server/tools_*.rs` requires a `security-auditor` pass regardless of the commit subject. **QUEUED, not started.** |
| 20 | macOS `open -a` / `.app` regression from the `{path}` removal | **Teach the ladder app bundles** — Bonsai supplies the arguments itself, so no injection surface. **SUPERSEDED IN PLACE by #21:** this moves into the *detection* logic of the removal milestone rather than into the configured-program path, which is being deleted. Recorded so the intent is not lost. |
| 21 | Security MEDIUM-2, after the auditor showed validation insufficient | **DO THE REMOVAL NOW**, as its own milestone — not the interim native-confirmation dialog. Free-text program entry is eliminated and replaced by a detected-list picker. Contract in flight. |
| 22 | Fail-open home masking + the LOW-1 doc claim | **TAKE BOTH.** |

### Why #21 — shape validation does NOT achieve its claimed property

The first increment's own comment claims "renderer compromise ≠ arbitrary local execution". The
security auditor refuted it with **three routes that survive validation**, all needing only script
execution in the renderer plus a repo that was cloned once:

1. **The absolute branch accepts any existing file** — `external_cmd.rs:145-155` gates on `is_file()`
   alone: not executability, not location, not trust. A hostile repo ships `payload.exe`; the
   renderer points `editorCommand` at that absolute path; it validates; and `external.rs:319` sets
   `hide_console = true`, so it runs under `CREATE_NO_WINDOW`. **PRECISION 2026-09-11 (architect
   refuted my looser wording):** that suppresses the console of a **console-subsystem** image only —
   a GUI payload still shows its own windows — and the file must be a PE or `.cmd`/`.bat` and contain
   no `is_shell_syntax` character. The route is real; "silent, invisible execution" was my overstatement. The
   increment's own test (`external_cmd_tests.rs:60-67`) documents the shape: it writes a stub named
   `my-editor.exe` containing `b"stub"` and asserts acceptance.
2. **Bare interpreter + repo as argument** — `node <repo>` executes the repo's own `package.json`
   `main` or `index.js` (`external.rs:244`). More likely than the `python` the doc cited, on a
   developer machine and in a JS project.
3. **Bare build tool as the TERMINAL command** — `make` / `nmake` / `just` / `msbuild` with
   cwd = repo and **zero arguments** (`external.rs:245`). This route **breaks the residual's own
   "at most one argument, which must be an existing directory" wording.**

Root cause: it is a **free-text program string the renderer can write** via `set_ui_settings`.
Validating its shape cannot fix that. The audit status is being downgraded from "Closed 2026-09-11"
to **partially closed** for both MEDIUM-2 and LOW-1.
Also: `src-tauri/src/commands/external.rs:89` `launch_inner` accepts **any existing directory the
renderer names**, not the selected repo — pre-existing P49 design, so the argument was never bounded
to the open repo either.

**Mitigating context (why this was MEDIUM, not HIGH):** the CSP at `src-tauri/tauri.conf.json:23`
is `script-src 'self'`, `object-src 'none'`, `base-uri 'none'`, which blocks inline script and
inline event handlers. Noted by the auditor as premise context: two `dangerouslySetInnerHTML` sites
render syntax-highlighted diff content (`DiffView.tsx:294`, `DiffViewSplit.tsx:162`) whose
highlighter escaping was **not** audited — pre-existing and CSP-mitigated, **not** opened as a
finding, but it is the premise the whole MEDIUM rests on.

### LOW-1's surviving rung is the Windows 10 DEFAULT, not an edge case

`wt` is **not present on stock Windows 10** (this machine is Win10 Enterprise), so the live rung is
`powershell` (`external.rs:278`), which keeps `cwd = <repo>`. A hostile repo shipping a hijackable
DLL at its root therefore has its DLL-planting primitive against the **default** Win10 "Open in
terminal". Keeping cwd is still correct — the alternative,
`powershell -Command "Set-Location '<path>'"`, turns a repo-authored path containing a quote into
PowerShell injection, a strictly worse trade. **`x-terminal-emulator` is being dropped from the
residual list:** neither the Linux dynamic linker nor macOS dyld searches the current directory by
default, so only the Windows rungs carry the risk.

### Home masking was FAIL-OPEN — fixed under #22

`obs/mod.rs:177-182`: if `app.path().home_dir()` returns `Err`, `set_home_dir` is never called,
`HOME` stays unset, `mask_home` returns the input untouched, and raw mode writes
`C:\Users\<account>\…` into every part that goes into the mailed export zip. The only signal was an
`eprintln!`, **which goes nowhere in a release GUI build.** This contradicted `scrub_home.rs:30`'s
own stated failure direction. Compounding it: **nothing tested the wiring** — deleting the
`set_home_dir` call would have failed no test. Fix: move the home string into `writer::WriterConfig`
(testable, no process-global `OnceLock`) and stamp `homeMasking: <bool>` into the session header so
an export reader knows whether to trust it.

### Verified CLEAN by the security auditor — do not re-audit

Every reader of the two settings (exactly two consumption sites, both behind validating entry
points; the MCP server and AI CLI driver read neither key) · validate-then-launch trim equivalence
(both sides `str::trim` on the same `&str`) · argument smuggling through the permissive absolute
branch is **structurally impossible** (the string becomes `LaunchSpec::program` only, never an arg;
`Command::args` with an argv vector; nothing reaches a shell) · UNC, drive-relative, rooted-no-drive,
control/bidi chars all refused and tested · ADS / 8.3 aliases / trailing dots / `..` / symlinks /
the `is_file()`→spawn TOCTOU are all **subsumed** by route 1 rather than separate findings · the cwd
carve-out is exactly the four rungs claimed, with an iff test · `procutil::resolve_program` searches
PATH only, never cwd · the export zip contains only `bonsai-*.jsonl` parts (no manifest, no metrics
file, no environment dump) and every part goes through `append_record`, so no emit site bypasses the
scrub order · `metrics/usage.json` and its `.bak` **cannot carry a path** (no path fields;
`metrics_keys.rs` rejects separators) and are not in the zip regardless · `set_home_dir` runs before
`Sink::start`, and `apply_dev_settings` is the sole production sink constructor.

**Could NOT verify (so the CLEAN register does not over-claim):** Rust std's Windows `resolve_exe`
`.exe`-suffix appending and its `.bat`/`.cmd` → `cmd.exe` wrapping — `rust-src` is not installed in
this sysroot. Medium-high confidence from the post-CVE-2024-24576 implementation; neither changes
route 1, which stands on `.exe` alone. No launch was executed; all findings are from code reading.

---

## 🔄 The 2026-09-11 ruling queue — what is NOT in a contract

The rulings are in the two ledgers above; the contracts are on disk and indexed in
`docs/contracts/INDEX.md`. Only what neither carries is repeated here. Narrative: archive Part 64.

### P112 — remove user-supplied `terminalCommand` / `editorCommand` (rulings #4 → #21)

- Contracts: `P112-external-tool-detection.md` (behaviour) · `P112-tool-catalog.md` (data) ·
  `P112-ui.md` (signed 2026-09-11). Status `pending` (contracted).
- Both settings are **empty strings** in the user's real `settings.json` — nothing installed depends
  on this capability.
- **`safe_cwd()` must STAY.** The board once claimed removal retires it; the architect refuted that —
  LOW-1 is about a hostile **repo** as cwd on the **auto** rungs, unrelated to user-supplied commands,
  and `external_url.rs:127` depends on it. P112 moves it verbatim into `procutil.rs` (contract §0/§7).
- `dc295c5`'s shape validation is **the stopgap, not the design** — its launch-path comment must keep
  saying so, and **MEDIUM-2 / LOW-1 stay `partially closed` until P112 ships.**

### F6 — `usage.json` 90-day window + deletable (ruling #3, contract `P91-F6-usage-retention.md`)

- Status `pending` (contracted). The contract's §2 SUPERSEDED pointers and §11 decision rows 34-36 were spliced
  into `P91-observability.md` by the curator 2026-09-14, so that file no longer reads stale.
- **The one failure mode a test will not catch:** the delete must reset the in-memory `MetricsState`
  as well as remove the `metrics` **directory** — otherwise the next flush rewrites the deleted data
  **and a test that only asserts the file is gone PASSES while the button does nothing.**
- **The board's own "always-on" claim was an OVERCLAIM:** always-on is **counts**; the **durations
  half is Dev-mode-only** (`metrics.rs:148-152`). `P91-privacy-copy-ui.md` §6.1.1 is the source of
  truth over this board. `RETAIN_DAYS` = `obs/metrics.rs:40`, still 400.

### P77 tag-sync — the design, settled by measurement rather than guesswork

- Auto-fetch **already downloads tags**: `src-tauri/src/scheduler/exec.rs:151` → `fetch_all()` →
  `crates/bonsai-core/src/git/remote_activity.rs:52`, `opts.download_tags(AutotagOption::Auto)`.
- **A purely local compare is NOT sufficient** — fetched tags land in `refs/tags/*` beside local ones,
  so classification needs the `ls-remote`: `crates/bonsai-core/src/git/tag_sync.rs:275-288` →
  `ls_remote_tags()` → `remote.list()` at `:132-173`.
- **So the increment is: trigger the existing `list_tag_sync` on auto-fetch completion.** That honours
  ruling #11 — it rides a cycle the user already opted into (5-min, enabled), adds no repo-open call
  and no new network policy. Trigger to replace/augment:
  `src/components/sidebar/TagsSection.tsx:165-172` → `RepoWorkspace.tsx:1878`
  `onTagsExpand={() => void refetchTagSync()}`, 10 s cache guard at
  `src/components/repoWorkspace/useTagSync.ts:57-60`.

### The e2e bundle default — MEASURE and REPORT, do NOT flip (ruling #9)

- The gap: **162 s dev vs 122 s bundle**, and it is **not established that the 122 s includes the
  bundle build**. If it does not, flipping makes the default gate *slower* — the opposite of the point.
- **Time one `E2E_BUNDLE=1` run from a cold build**, compare against the 162 s dev figure *including
  build*, and put the number on the record. **The default does not change without the user seeing it.**
- The flip, if ever authorised, is one line each: `playwright.config.ts:38` →
  `const BUNDLE = process.env.E2E_BUNDLE !== '0'`, plus inverting `--e2e-bundle` in `gate.mjs`.
  Bundle mode is equally green and higher-fidelity (P103 was visible only there). Context: Part 48.

### ✅ UNC `canonicalize` — ship-blocker on `216ca45` CLEARED by measurement 2026-09-14

`stage_paths` inherits `ensure_within_workdir`'s **`fs::canonicalize(workdir)`** dependence —
pre-existing for `discard` / `stage_partial` / `conflict`, newly extended to the highest-traffic
write primitive. The auditor's concern was that on a UNC, `\\wsl$` or cloud-placeholder workdir a
canonicalize failure becomes `AppError::Io` and **refuses EVERY stage, including from the UI.**

**Tested for real**, not reasoned about: a standalone replica of `ensure_within_workdir` (an exact
copy of the function as of `d46c98e`) run against a real UNC workdir via the `\\localhost\D$` admin
share.

| Case | Result |
|---|---|
| Drive workdir (control) | OK — `canonicalize` yields the `\\?\D:\…` long-path form |
| **UNC workdir, normal file** | **OK** — `canonicalize` yields `\\?\UNC\localhost\D$\…` |
| UNC, nested existing dir | OK |
| UNC, not-yet-created dir | OK — the walk-up loop terminates correctly |
| UNC, `../outside.txt` escape | **correctly REFUSED** |

`canonicalize` **succeeds** on UNC, and both `base` and `real` come back in the same `\\?\UNC\` form,
so `starts_with` compares consistently — the guard neither fails open nor fails
closed-on-everything, which was the actual worry.

**Honest limits of this check:** `\\wsl$` could **not** be tested (WSL is not installed on this host
— `Test-Path` on it returns false), and **cloud placeholders (OneDrive) were not tested**. Both go
through the same `canonicalize` call that UNC now demonstrably survives, so the *mechanism* is
exercised — but neither path is empirically confirmed, and this entry should not be read as saying
otherwise. The probe is kept at `D:\Data\Temp\claude\unc-test\unc_check.rs` for whoever has such a
host. Original review: archive Part 66.

---

### ✅ e2e bundle-vs-dev — MEASURED 2026-09-14 (user ruling #9: measure and report, do NOT flip)

**The blocker is resolved.** The board held this decision because "the figures are 162 s dev server
vs 122 s bundle, and **it is not established that the 122 s includes the bundle build step**. If it
does not, flipping could make the default gate *slower* — the opposite of the purpose."

| Run | Wall | Result |
|---|---|---|
| `E2E_BUNDLE=1`, **truly cold** (`dist-mock` removed) | **102 s** | 185 passed, exit 0 |
| `E2E_BUNDLE=1`, warm (`dist-mock` present) | **103 s** | 185 passed, exit 0 |
| Dev server — today's full gate e2e step (`ba406ba`) | **191.4 s** | 185 passed |

**The build IS inside the number, and cold-vs-warm is a 1-second difference** (102 vs 103 s) because
`vite build` is not incremental — it rebuilds either way. So the feared failure mode (bundle looking
fast only by reusing an artifact someone else paid for) **does not exist**.

**Bundle is ~89 s faster per gate run, roughly a 47% cut on the e2e leg.**

**Why the two old figures were never comparable** — worth recording, because it is the real reason
the question stayed open. The bundle path builds to **`dist-mock`** with `--mode mock`, and
`scripts/e2e-server.mjs:13-16` states plainly that the gate's `tsc + vite build` artifact is **NOT
reusable**: a plain `pnpm build` is REAL mode and boots against the Tauri IPC. They are different
artifacts in different directories. (Method note: my first attempt cleared `dist`, which is the
wrong directory — the run was warm, not cold. The build still ran, proven by vite's reporter output
appearing under `[WebServer]` inside the timed window, but it was not the cold measurement the
ruling asked for. Corrected by removing `dist-mock` and re-running.)

**NOT FLIPPED, per the ruling.** `playwright.config.ts` and `gate.mjs` are untouched; bundle mode
stays opt-in via `E2E_BUNDLE=1`. The decision now has its number and is the user's to make. The
fidelity argument recorded earlier still stands independently: P103 was a real product bug that
**only** the bundle check exposed, because `import.meta.env.DEV` code is absent from a production
bundle.

**One known bundle-mode divergence, unchanged and still on the board:**
`e2e/24-settings-shell.spec.ts:238` ("Esc dismisses the menu and hands global shortcuts back") fails
3/3 in bundle mode against 1/3 in dev — reproducible on a built bundle, merely flaky on the dev
server. Mechanism never identified. It did **not** fail in either run above (185/185 both times), so
its status is unchanged rather than resolved.

---

### ✅ FULL 8-STEP GATE GREEN UNDER happy-dom — 2026-09-14, `b53618a`

`GATE_EXIT=0`, **461.9s total**, zero FAIL lines. Per step: nextest 170.2s (**2467 passed**, 9
skipped) · doctests 3.4s · clippy 19.4s · eslint 13.2s · size ratchet 0.7s · **vitest 52.1s
(2848 passed, 253 files)** · tsc+build 11.4s · **e2e 191.4s (185 passed)**.

**This verifies the user's 2026-09-11 ruling** ("keep happy-dom, fix the tests" — not revert, not a
blanket `testTimeout` raise). Be precise about what it proves:

- happy-dom was **0/2** in the full gate before the fixes and is **1/1** after. That is a meaningful
  flip, **not** a guarantee.
- **Only 2 of the 5 originally-failing tests were changed.** The other three were deliberately left
  alone: two are fully synchronous and one is already macrotask-flushed, so **no test edit can
  immunise them against a wall-clock check** — vitest 4's `withTimeout` tests elapsed time **on
  completion**, so a stalled machine fails a test that passed every assertion.
- The measured tail inflation in the gate's own condition (rust tier → vitest) was **1.3-2.8×**, and
  three tests already cross 5000 ms — green only because they carry explicit `20_000`/`30_000`
  budgets. That fragility is unchanged by this run.

**The speed prize is bigger than estimated:** gate vitest is **52.1s under happy-dom vs 86.4s under
jsdom** — a **34.3s** saving per gate run, against the 22s figure recorded on 2026-09-11 (that
earlier happy-dom number, 64.4s, was itself measured on a run that failed).

**The `testTimeout` / gate-reorder question the user dismissed 2026-09-11 stays dismissed.** Do not
re-raise it unprompted; this green is the reason it may not need raising at all.

---

## Durable lessons — the rules

The reusable rules. They are on the board, not in the archive, because every one was learned by a
claim that was green the whole time it was wrong. **The stories, worked numbers and measurement
narrative behind them are archive Part 53** — cite the rule here, read the story there.

### The six failed app-wide claims

Every one had the same shape: **a sentence claiming an app-wide property, with a call-site count that
nobody enumerated.** The first five failed for want of an enumeration. The sixth is worse — the
enumeration **existed** and was still blind, because it was **scoped by token name**.

1. **P95** — the enabled-control class; 3 escapes found by P101.
2. **P98** — "`--text-3` family closed"; 122 declarations were never classified.
3. **P74** — the hue-as-text sweep; became P105.
4. **`ui-reference.md` §2** — "6 live hue-over-own-tint instances"; the real population is **38**.
5. **P106's hand-over count of 48** for P108; the real inventory was **62**.
6. **P101** — an *exhaustive* `--text-3` audit recorded as CLOSED, which still missed a **2.96 light**
   glyph, because **`--badge-unknown` is byte-identical to `--text-3`**.

**The rule:** a bucket + verdict per call site, P101 §3 style, or it is not closed. Enumerate,
bucket, record a verdict per site, predict the post-fix residue, then verify the prediction.
**Do not accept a "~N call sites and it's fine" sentence as evidence.**

### The aliasing rule

- **An audit scoped by token NAME cannot see an alias. Scope by resolved VALUE.**
- Token aliasing has hidden instances three times: `--badge-good`/`--badge-warn` are byte-identical
  to `--success`/`--danger` (the first pair), then `--badge-unknown` to `--text-3`.
- A `var()` **fallback masking a missing token is invisible to any hue-name search**, because the hue
  name appears only in the fallback (`--warn`, which is defined nowhere).
- A naive probe of an **undefined** custom property returns the *inherited* value, not the value the
  contract cites — same trap class.

### The grep-counting rules

- **An acceptance grep counts *text*.** Prose comments and `var()` fallbacks inflate it. P106's R10
  read 0 predicted vs **12** measured: 7 hex literals quoted inside comments, 5 `var()` fallbacks.
- A residue prediction must state whether it counts **declarations or raw matches**.
- **A baseline must be measured against the real pre-fix tree**, never inherited from a prior
  contract. Both of P106's misses were the contract's *baselines* being wrong, not the fix.
- **When a grep and a prediction disagree, re-measure the baseline BEFORE touching code.**
- Where a prediction names `file:line`, **the count is the criterion** — comments added by the fix
  itself shift the lines (216/227 → 220/231 in P106; count unchanged).
- An implementer must deliberately avoid literal strings in its own comments: without that care two
  P106 counts would have read 11 and 2 instead of 8 and 1.

### The BASE rule

- **A contrast figure is meaningless without its composited base.** Every ratio must name the ink,
  the tint, **AND** the base.
- A figure naming only the tint is **incomplete evidence and may not be used to close an AC**.
- Proof: `--accent-strong` on a 14% accent tint measures **6.42/5.19 over `--bg-0`**, **5.85/4.87
  over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's) — one method, three bases.
  The two contracts never disagreed; **neither stated its base**, and the base alone accounts for
  **1.26** of dark-theme spread.

### The three P91 testing rules — now in the contract, not only in session memory

1. **A writer rule is covered only by a `LogRecord` → `append_record` → read-back round-trip.**
   Synthetic-`Value` tests are additive, never substitutive. This is what let W6 ship dead in
   production while its test passed — the one rule of six that skipped the round-trip.
2. **A validator's tests must use inputs the REAL producer emits**, and a cross-boundary vocabulary
   must be pinned by a drift test that re-derives it from the producer's own source. *A predicate
   that rejects everything is indistinguishable from one that works, unless something asserts a real
   input is ACCEPTED.* This is the `cmd.*` camelCase bug — the **entire `cmd.*` histogram family
   recorded NOTHING in production** while every test was green.
3. **A negative test must be PROVEN to fail on the unfixed code.** Saying "this must fail on today's
   code" is not proof. AC6 as originally written specified a payload the scrubber **already caught**,
   so it would have been green on the buggy code — a negative test that could not go red.

### Two more failure modes, each named after it cost a session

- **Evidence lost in transcription** (named at `ef06e6b`) — a figure or a qualifier that survives the
  measurement and dies in the summary. P108's AC11 is the live example: **source-derived and
  unverified** must travel with the number.
- **The specificity trap** — **a contrast fix that is out-specified by an existing rule is a no-op
  that still passes a grep.** `.diff-stage-float button` (0,1,1) out-specified `.diff-float-discard`
  (0,1,0), so the destructive discard button rendered in accent blue with no danger hue at all while
  passing every AC grep. Sibling to the child-rule trap; both are in `ui-reference.md` §2.

### The measurement rules (numbers: archive Part 46; the worked narrative: archive Part 53)

- **Optimising the measured-slowest test may not move wall clock, because a different test becomes
  the floor.** Net for the 2026-09-03 pass: `cargo nextest --workspace` **106.5s → 92.5s** with tests
  *increasing* 2298 → 2316.
- **Concurrent agents on this box produce outliers** — single-run numbers are worthless; use paired
  or repeated runs.
- **proptest regression seeds can be worse than useless**: proptest keys its persistence file **per
  source file, not per test fn**, and a `cc` seed regenerates values through the *current* strategy.
  Random cases wearing a regression label.
- **Do not run the e2e suite concurrently with other heavy jobs.** `gate.mjs` is strictly serial, so
  the gate itself is safe.
- **A flake that reproduces deterministically in a production bundle is not a flake** (P103).

### The gate-running rules (the gate states that earned them: archive Parts 51 and 58)

- **Before trusting any timing-sensitive failure, verify machine state AT THE TIME IT RAN** — sample
  CPU repeatedly, look for scratch/load processes, confirm no agent is mid-run.
- A slow timing number is **evidence about the machine** until proven otherwise. Timing analogue of
  the grep-counting rule: *measure the baseline, do not infer it.*
- **Verifying machine state before running the gate is a standing pre-gate step**, not a nicety.
- The e2e leg **will** fail intermittently on a loaded machine — expected, documented at P104 (Edge
  misses a hardcoded 30 s CDP close window, then a blocking `taskkill` runs).
  `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removed the largest source, measured at 7.6× p99.
- **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**
- **Give subagents `--workspace`, not `-p <crate>`, whenever a change crosses a crate boundary.**
  A remediation brief scoped to `cargo nextest -p bonsai-core` never ran the `bonsai` crate's tests
  under `src-tauri/`, and one of them asserted the exact behaviour the change removed — the gate's
  first run failed on it.
- **Redirect the whole gate log to a file**; read the summary from there. Piping through `tail -60`
  lost the failure detail and cost a re-run of the Rust leg.
- **Read the `gate summary` block, never the exit status alone** — a background wrapper reported
  exit 0 while the gate had failed (that was the pipeline's exit code, not the gate's).
- **That rule applies to a bare `cargo test` too, not only to `pnpm gate` — 2026-09-11 cost proves
  it.** The orchestrator piped three `cargo test` runs through `head`/`tail`, read exit 0, saw a
  truncated log with six `FAILED` lines and no `test result:` summary, and reported six phantom
  `h_ai` failures as a possible regression. Serialized and unpiped, `h_ai` was **45/57 passed, 0
  failed** — the truncating pipe manufactured the failure *and* hid the evidence that would have
  disproved it. **A cargo run whose log has no `test result:` line has not finished; it has been
  cut off.**
- **SERIALIZE cargo. Concurrent `cargo` invocations queue on the build-directory lock, and a queued
  cargo is INDISTINGUISHABLE FROM A HANG** — several `cargo.exe`, **zero `rustc`**, negligible CPU,
  no output for 20+ minutes. That is what *waiting for a lock* looks like, not a deadlock. On
  2026-09-11 the orchestrator read that signature as wedged and killed the processes **twice**;
  they would have drained on their own, and each kill discarded build progress and forced a cold
  vendored-libgit2 rebuild. **Do not kill them. Run one cargo at a time and wait** — CLAUDE.md's
  "never conclude failure from a timeout" covers this exact case.
- **Check port 1420 after any harness-heavy pass** — `Get-NetTCPConnection -LocalPort 1420` is the
  whole check. An agent that drives `pnpm dev` by hand **orphans** it (once for **3.5 hours**,
  2026-09-14, PID 12712, 13:18:31 → 16:50:55), and `vite.config.ts` sets **`strictPort: true`**, so
  `pnpm tauri dev` then **fails outright** rather than falling back — i.e. it breaks the USER
  CHECKPOINT. Brief agents to the Playwright-managed path (`scripts/e2e-server.mjs`), never a
  hand-run dev server. `playwright.config.ts` already carries the warning. Narrative: Part 71.2.
- **`cargo fmt --check` is NOT a gate step** and is dirty at baseline, so never read its output on a
  diff as a regression — see `### cargo fmt has never been run on this repo`.

### The coverage and evidence rules (earned 2026-09-14/15; narratives archive Parts 71 and 73)

- **A test that passes in the correct AND the broken state is not coverage.** Four instances in the
  P112 work alone. The negative must be proven red on the unfixed code first.
- **The limit case is a test that is never in any state.** `external_picked_tests.rs:186` is
  `#[cfg(not(debug_assertions))]`, so it compiles only under `--release`, which the gate never runs.
- **A green gate says nothing about a Rust/TS DTO change.** Every frontend tier — vitest, tsc, e2e,
  the browser harness — consumes the mock, so all of them stay green while the real app is broken.
  Only `src-tauri/src/settings_defaults_parity_tests.rs` spans the two languages, so **weakening the
  parity oracle is never routine**, and a DTO change must land its Rust and TypeScript halves in the
  **same** increment. The mock is not inventing anything in this failure mode; it is **stale**, and
  staleness is invisible to every test that consumes it.
- **A green `pnpm gate` is Windows-only evidence.** Any test that passes an explicit `TargetOs` while
  touching the real filesystem is host-bound; the absoluteness rule is what makes it so.
- **When a contract signs an error string that names a recovery, the recovery is part of the same
  contract item.** §16.4a signed `BROWSE_STALE`'s string and its report but **not its verb**, so as
  written the message named an action that did nothing.
- **Cite from the file, not from a summary.** Four citations in one brief were wrong, and one had
  already propagated a false claim into a contract — where the next reader treats it as established.
  A `file §section` pair is the dangerous shape: the section number can be right for a *different*
  file. Verify a citation **before** delegating on it.
- **Mislabelled evidence is this repository's named defect.** A test message that claims more than
  the test discriminates (`tests_tools_pick.rs:148`: "both fields in ONE update cycle", which two
  sequential `settings::update` calls would also satisfy) is a defect even when the code is right.

### 📌 TWO DURABLE CONSTRAINTS discovered in the fix pass — keep these

1. **`tracing` DOES NOT EXIST in this workspace.** My brief said "`settings.rs` has zero `tracing::`
   calls, so there is nothing to piggyback on" — that understated it: **no crate depends on
   `tracing` at all**, so there is no facade to add a call to. The project's actual non-fatal
   diagnostic facade is `eprintln!("bonsai: …")` (as in `commands::repo`, `lib.rs`,
   `commands::ui_settings`). Use that, and do not write `tracing::` into a brief again.
2. **The settings-load path CANNOT use the observability sink — a bootstrapping constraint.** The
   P91 `obs` sink is **configured from the very settings** that `load_from` is in the middle of
   reading, so it cannot be running yet when migration code executes. Any diagnostic inside
   `load_from` has to be `eprintln!`. This is a real ordering constraint, not a preference, and it
   will bite anyone who tries to route settings-layer diagnostics through `obs`.

---

## Accepted decisions that must survive compaction

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
  P83, P84, P85-P90, P92, P93, P95, P96, P97, P98, P99, P100, P101 and DEP REFRESH are confirmed too.
  Full per-milestone text → `docs/history/todo-archive-2026-08.md` Parts 17-21 and
  `todo-archive-2026-09.md` Parts 22-31, 47.
- **P100 + P101 checkpoints were recorded 2026-09-02 on the user's direct instruction**, not on a
  contemporaneous native run — the user was going away for an unattended session and scoped that
  authority **to those two milestones only**. It does **not** reach anything created that session.
- **P75 (IPC codegen) — HALTED 2026-08-21 (user decision).** Linking `tauri-specta` breaks app launch
  on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`). Spike
  reverted; findings + crate pins kept in `docs/contracts/P75-ipc-codegen.md`. Revisit only if
  validated on Windows 11 or with a link-order fix.
- **P76 (native-checkpoint automation) — HELD as contract-only per user (2026-08-20).**
  `docs/contracts/P76-native-checkpoint-automation.md`.
- **Process change adopted (P100 §6-D).** `ui-reference.md` is ~1322 lines / ~40k tokens and
  `ui-designer` has **no `Edit` tool** — only whole-file `Write`, which **truncates mid-file** at that
  size. That is the structural cause of the P95 "silently unapplied patch". From P100 on: the designer
  supplies **verbatim line-anchored hunks**, the orchestrator applies them with `Edit`, and verifies
  line count + section count + tail sentinel + hunk confinement. **This deviates from CLAUDE.md's
  "no other agent edits `ui-reference.md`" — raise with the user whether to give the designer `Edit`
  or split the file.**
- **Do NOT run prettier in this repo** until a config exists — there is no `.prettierrc*`, no
  `prettier.config.*`, no `package.json` key and no `.editorconfig`, so prettier falls back to its
  defaults and rewrites whole files (74 insertions for a ~20-line edit; double quotes at 80 columns
  against the repo's ~100).
- **Do NOT run `cargo fmt`** as part of another change — see the open item below.
- Gate child processes use `D:\Data\Temp\bonsai-build`, not Defender-scanned `C:\Temp`.
- Ports: browser harness **1420** · e2e dev server **1430** · e2e built bundle **1440**.
- Velocity/gate-cost numbers: `docs/history/velocity-2026-09-01.md`. Ceremony — not machine time — is
  **~75-85%** of per-task wall clock, which is what CLAUDE.md's velocity-mode and batching rules
  exist to cut.

- **All eight native USER CHECKPOINTs were confirmed by the user on 2026-09-10** (`548cc0a`):
  P102+P105, P106, P107, P108, P91, P110, P109, and the `8dd5b24` CSP change — the last of these
  needed a native run because it applies to the Tauri webview only, so neither the harness nor e2e
  ever exercised it. Full per-milestone text: archive Parts 54-55, with the board's own record of
  the confirmation in Part 60. ~~**The confirmation does not reach two AI-gate items** (P108 `AC11`,
  P91's `logs/*.jsonl` parse) and **is not authorisation to merge `feat/p91-observability`**.~~
  **Superseded 2026-09-11 — see the dated additions below:** the user ruled MERGE (#1) and closed
  P108 `AC11` (#13). **Only P91's `logs/*.jsonl` parse is still owed**, and it is still true that no
  native confirmation can close it.

**P91 user decisions that must survive compaction (all 2026-08-27 unless noted).** Build diary:
archive Part 44; the milestone entry is archive Part 54.6.

- Redaction conservative by default, opt-in raw-names toggle; credentials never logged in any mode.
- Metrics storage = **rolled-up JSON**, not SQLite.
- React instrumentation = **SIX surfaces** — the user added the **left sidebar** to the original five.
- Logs are **NOT** auto-deleted when Dev mode goes off (prune by caps only); per-session log files;
  `metrics_reset` ships headless.
- Decision 7 — **"Delete all log files" ships in v1**, model = **roll-then-purge**, scope hard-limited
  to `logs/*.jsonl` + `.tmp`, never `metrics/` or `settings.json`. Success copy must say "Still
  recording" when `rolled:true`.
- **USER GATE (2026-08-27):** each increment requires its own explicit go from the user.
- ~~**Branch policy (USER, 2026-09-02):** everything this session lands on `feat/p91-observability`;
  local commits only, no push.~~ **Superseded 2026-09-11 (ruling #1):** the branch was merged to
  `dev` and pushed. The policy is history, not instruction.

**P91 — three architectural rulings not to re-open.**

- **NO global cap on metric keys.** Worst case is genuinely per-map-per-bucket, ~616k keys, accepted
  rather than glossed: a global cap would make *today's* recording depend on *history*. 512 is a
  **runaway stop, not a sizing parameter**. File-size pressure has a different lever (size-triggered
  early roll-up) — revisit trigger only, ~8 MB, no work now.
- **The writer's limit, recorded honestly in both contracts:** `raw_args.rs` enforces **shape +
  vocabulary, not semantics**. Content under an identifier-named key, ≤512 chars, single-line,
  **survives**. Its only defence is the positional drift guard in `rawArgPolicy.test.ts`.
  **Neither guard may be removed citing the other.**
- **The raw-args ruling (`834f2d1`):** raw mode widens **IDENTIFIER** fidelity (repo path, file
  paths, ref names, remote URLs, full SHAs) and **never CONTENT** fidelity. Free text and credentials
  are outside both modes, permanently.

**Added 2026-09-11 (the ruling session) — facts, not narrative. Detail: archive Parts 62-68.**

- **All 22 open FOR-USER items were RULED** (17 + a second round of 5). Both ruling blocks above are
  authoritative; do not re-open any of them without the user.
- **`feat/p91-observability` was MERGED to `dev` and pushed** (ruling #1), a pure fast-forward;
  `dev` = `origin/dev` = `8b88efd`, `cb70f4a..8b88efd` = **165** commits (curator-verified
  2026-09-14). Every "do not merge" instruction on this board is void.
- **P108 `AC11` — CLOSED by ruling #13:** the two source-derived **3.05** figures for the two
  unreachable states are ACCEPTED, limitation recorded. Verbatim text: archive Part 67.
- **happy-dom ADOPTED as the vitest DOM environment** (ruling #8, `1953c0a`); `jsdom` stays installed
  so the shim's self-guard stays meaningful and `docs/proposals/happy-dom.patch` stays on disk so a
  revert is one command. The shim is a **hand-maintained subset of the UA stylesheet** — an inline tag
  outside that set silently gets `display: block`, landing precisely in accessibility-name
  computation (715 `ByRole(…, { name })` call sites across 77 files). **That quiet failure mode is an
  ACCEPTED risk.** Measured win: −26% wall, −44% environment CPU.
- **Home/username masking is FAIL-CLOSED and cross-platform** (rulings #5/#22, `dc295c5`): the home
  string moved into `writer::WriterConfig` (testable, no process-global `OnceLock`) and every session
  header carries a `homeMasking: <bool>` stamp so an export reader knows whether to trust it. Folded
  into `P91-observability.md` §7.5. It was **fail-open** before, with `eprintln!` as its only signal —
  which goes nowhere in a release GUI build.
- **The MCP tool-description review trigger is now in CLAUDE.md** (ruling #19): any diff touching
  `crates/bonsai-mcp/src/server/tools_*.rs` requires a `security-auditor` pass **regardless of the
  commit subject**, with a description-snapshot test guarding text drift. `rmcp-macros` concatenates
  every `///` line into the JSON-Schema `description`, so those comments are the **tool contracts a
  model reads** before invoking worktree-destructive operations.
- **P94 has no contract file — confirmed, accepted as debt.**

> **The second round of user rulings (items 18-22) used to sit here, after this section.** It was
> moved up to sit directly beneath the first ledger on 2026-09-14 — see `## 🆕 SECOND ROUND OF USER
> RULINGS — 2026-09-11` above. Nothing in it was shortened.

---

## OPEN follow-ups (genuine unresolved items, not checkpoints)

Condensed to one line per item on 2026-09-03 and again 2026-09-14; pre-condensation text is archive
Part 50 and **Part 69**. Nothing here was closed by the curator.

### 🆕 NEW 2026-09-16 — the nine-file second review pass: 2 MUST-FIX routed, 4 filed here

**The pass that closed the coverage gap my batching created.** The nine files named in the earlier
scope note carried exactly one review pass; this was the second. Verdict **APPROVE WITH MUST-FIX**.
Reviewed `fd98dd8..HEAD` for those paths (699 insertions / 190 deletions) plus all nine files in
full. Targeted evidence: 9 suites / **65 tests pass**, `tsc --noEmit` exit 0. Nothing crossed the
Rust/React boundary; all nine are under the 500-line limit.

**Two MUST-FIX, both verified by me against source before routing, both in the MCP note lifetime:**

1. **MCP outcome notes outlive the Settings surface.** `useMcpControls.ts:54` holds the
   `useOutcomeNotes()` instance and the hook mounts in `App.tsx` for the app's lifetime, but
   `SettingsPanel.tsx:32` (`if (!open) return null`) unmounts the section — so the note map survives
   a close. A failed Add (Globally) leaves `Could not register: …` on screen to be re-read tomorrow.
   Violates `P113-settings-inline-notes.md:363` verbatim: *"reopening Settings is a clean page."*
2. **The stale-register-slot clear misses the event-driven stop.** `useMcpControls.ts:100-103` clears
   the slots only in `handleSetMcpEnabled`'s success continuation, but `mcp-server-changed` is emitted
   from **both** `mcp.rs` ~`:399` (`stop()`) and ~`:213` (`start_or_signal_stopped`'s **error arm** — a
   failed restart during a write-gate bounce), and the subscription at `:75` only calls
   `setMcpStatus`. So `handleSetMcpAllowWrite` can reach stopped with no `handleSetMcpEnabled(false)`.
   **`mcpOutcomeNotes.test.tsx:202-218` is green in both the fixed and the broken state — the FIFTH
   recurrence of that pattern in this work.**

**ORCHESTRATOR RULING on the §7-vs-§17.3 conflict the reviewer raised: both passages stand.**
`P113-settings-inline-notes.md:1088` (§17.3) governs *who owns the instance* and *where the live
region lives* — the announcer must stay in the section, count **1**. `:363` (§7) governs *lifetime*.
They constrain different things, so the reconciling fix (expose a reset from `useOutcomeNotes`, fire
it on the section's unmount) satisfies both. **No contract edit is owed; neither was rewritten.**

**Filed, NOT routed (velocity mode):**

- **`useOutcomeScrollCorrection.ts` has ZERO automated coverage** — 125 lines of contract-critical DOM
  math. Its `Math.ceil` residual (`:95`) is what `P113-settings-inline-notes.md:641-647` calls
  load-bearing (*"without it the four-edge test in AC2b cannot be met"*) and it rests on **one**
  harness measurement. The jsdom bailout at `:67` makes the module a no-op in every current suite.
  **→ `tester`**: three cases with `scrollIntoView`/`getBoundingClientRect` stubbed.
- **`adoptToolSelection` cites a STRUCK, REVERSED contract bullet.** `useUiSettings.ts:50` and `:126`
  (echoed `SettingsContext.ts:139`, `useExternalToolScan.ts:106`) cite "P112 §16.16-5", struck and
  reversed at `P112-ui.md:1594-1601`; the governing §17 passage (`:1683-1690`) rules *"accept it as
  shipped; do not rework"*. No passage in `docs/contracts/` specifies `adoptToolSelection` at all — so
  the shipped setter is an approved **deviation** cited to a dead bullet. → `architect` to record it.
- **`useSettingsSaveFailure.ts:71` decides banner-vs-toast ONCE, at failure time.** Fail with Settings
  open → banner, no toast; user closes Settings → the condition persists with **no surface at all**,
  backoff having stopped after 3 attempts. `P113-settings-inline-notes.md:1133-1137` promises the
  banner *"persists exactly as long as the condition does"*. Fix: raise the toast lazily on the
  open→closed transition while `settingsSaveFailed` is still true.
- **Four NITs.** `useSettingsOpenSignal.ts:20` cites `App.tsx:90`, actual wiring `:83` · §10.3's
  `while (deficit > 0)` shipped as one pass + a height guard (both correct, but **a note taller than
  the scrollport gets no correction, so AC2b is unachievable for it by construction** — one clause in
  §10.3 would close it) · `useToastQueue.ts:63` reads an effect-synced ref, so a handler that opens
  Settings and pushes in the same commit escapes the DEV guard · `useMcpControls.ts:71-74` swallows a
  failed `getMcpStatus` and the section then renders "Stopped.", indistinguishable from a genuinely
  stopped server.

**Three worries CLOSED by this pass, by verification — do not re-open:** `useUiSettings.ts` lost
nothing load-bearing in its 237-line cut (the write machine moved **verbatim** into
`useSettingsWriteQueue.ts` — streak, merge-back, `disposedRef`, `pagehide`/`beforeunload`, StrictMode
reset all identical) · **`hydrateUiSettings` has exactly ONE runtime caller**, `App.tsx:272` (launch),
so the `metricsVersion` full-re-measure concern is dead · the `announceOnly` approval condition on
`useOutcomeNotes.ts` **was met** (the JSDoc and the ref proof comment both name it; the range shifted
off the cited `:59-79` because the edit moved the blocks).

### ✅ CLOSED 2026-09-16 — both MUST-FIX fixed (`2b3dfd6`), the 4 SHOULD-FIX resolved, gate green

**The full chain, so a resume does not re-litigate it:** second pass found 2 MUST-FIX (`2b3dfd6`) →
focused re-review **APPROVED, no MUST-FIX** → 4 SHOULD-FIX (`9aa25f4`) → batched review **APPROVED
both sets** → 2 coverage holes + 3 file splits (`934a280`) → **full 8-step gate GREEN 414.2s**.

- **MUST-FIX 1 fixed** — `AiCategory` resets the notes on mount and unmount. Hook keeps the instance,
  section keeps the announcer, so §17.3 **and** §7:363 both hold and **neither contract was edited.**
- **MUST-FIX 2 fixed** — one effect keyed on the status discards both register slots on every
  not-running observation, covering the command resolve *and* all **three** `mcp-server-changed`
  emit sites (the third, `set_allow_write`'s already-stopped arm at `mcp.rs:270-279`, was uncited).
- **`discardNotes` not `begin`** — the event's `setMcpStatus` and a rejection's `report` can land in
  one commit, where `begin` would blank the ALLOW_WRITE announcement that just explained the stop.
- **The announcer-writer set is now CLOSED at four** (`begin`, `report`, `announceOnly`, `reset`),
  reviewer-verified by grep: every `setAnnounce` lives in exactly those four, and the setter is
  never returned.
- **Accepted, not suppressed:** Settings **search** is a **third** notes-clearing trigger
  (`SettingsShell.tsx:65,213` → `SettingsResults.tsx:82`). Fail-safe — early, never stale — so the
  comment was the only defect. **Orchestrator ruling; do not add code to suppress it.**
- **✅ `useOutcomeScrollCorrection` coverage: 0 → 8 cases**, each shown red against a broken module.
  The `Math.ceil` is pinned **twice** (floor → 1269, bare assignment → 1269.171875). Case 5 pins the
  over-tall-note limitation *as intended* rather than hiding it.

**TWO MORE INSTANCES of "green in both states" were found and closed — bringing the count to SEVEN.**
Both were in the tests written to close the previous instance, which is the point worth keeping:
`previous.current = notes` (`useOutcomeScrollCorrection.ts:116`) could be deleted with all six cases
green, and the layout-timing clear survived a swap back to `useEffect` at 26/26. **The reviewer's
suggested fix for the first did NOT work** — after the first correction the note sits inside the clip,
so a re-detected change bails at condition 1 and still looks like a skip; the real discriminator is
that the note must be **clipped at the moment of the second rerender**. Found by trying it, not by
trusting it.

**🆕 FILED — citation drift the `934a280` split created (none is a defect, all four will mislead):**

- `docs/contracts/P113-settings-inline-notes.md:1088` cites `App.tsx:208` for the `useMcpControls`
  wiring. **App no longer calls the hook at all** — it is `src/hooks/useMcpWiring.ts:67`, and
  `App.tsx:201` is the `useMcpWiring` call. **Contract-owner's fix — deliberately not edited here.**
- The closed "`hydrateUiSettings` has exactly ONE runtime caller" note: `App.tsx:272` → **`:259`**.
- `mcpOutcomeNotes.test.tsx:41` cites the adapter at `:413` for the `mcpEnabled` derivation → **`:358`**.
- `useMcpControls.ts`'s header still says it "is wired from `App.tsx`" — now via `useMcpWiring.ts`.

**🆕 FILED — three from the same rounds, each needing a pass this one could not take:**

- **`useOutcomeScrollCorrection.ts:65-66`'s comment is STALE and `:67` is unreachable here.** It
  blames jsdom; this repo runs **happy-dom** (`vite.config.ts:45`), which implements `scrollIntoView`
  as a no-op, so `src/test/setup.ts:29`'s `??=` never fires. The module is inert in other suites
  because of **zero geometry** — every rect is the zero rect, so `fullyInside` is trivially true.
  **The stale comment actively invited a wrong diagnosis in my own brief.** The guard is still pinned
  (case 6 removes the method from the instance).
- **Selector-constant risk:** the module hardcodes `.settings-pane` / `.settings-row`
  (`useOutcomeScrollCorrection.ts:61,64`) and so does the test fixture — so a rename in
  `SettingsShell.tsx:206` / `SettingsRow.tsx:109` breaks **production** while test and module keep
  agreeing. Only an exported constant *consumed by production* closes it.
- **`adoptToolSelection` cites a struck, reversed bullet** — `useUiSettings.ts:50`/`:126`,
  `SettingsContext.ts:139`, `useExternalToolScan.ts:106` cite "P112 §16.16-5", struck at
  `P112-ui.md:1594-1601`; the governing §17 passage (`:1683-1690`) rules *"accept it as shipped; do
  not rework"*. **No passage specifies `adoptToolSelection` at all** — an approved deviation cited to
  a dead bullet. **→ `architect`.**

**🆕 FILED — still open from the first pass, unchanged:** `useSettingsSaveFailure.ts:71` decides
banner-vs-toast **once, at failure time**, so a failure raised with Settings open has **no surface at
all** after the user closes it (backoff having stopped after 3 attempts) — against
`P113-settings-inline-notes.md:1133-1137`'s promise that the banner "persists exactly as long as the
condition does". Its own increment: a behaviour change with UX implications. · The narrow
**pre-existing** late-`report` race (Add → stop → clear → `report` writes with rows unmounted →
re-enable takes the `enabled === true` early return), reachable only without a Settings close.

**One process note, since the board exists to catch this:** `tester` ran `tsc` and `eslint` but
**not the size ratchet**, and `SettingsPanel.test.tsx` at 526 was a **hard fail** (not baselined). I
nearly committed on the agent's report alone — the gate would then have gone red at step 7 after a
414 s run instead of before it. **Run `node scripts/check-file-size.mjs` before committing any pass
that adds lines to a test file.**

### 🆕 NEW 2026-09-14 — every Settings toast renders behind Settings' own scrim (§6.11.6)

**Found by `ui-designer`, verified by me against source. This is not an F6 item** — it is the whole
Settings surface, and it is a MUST-FIX whose *scope* is a user decision, so it is parked here rather
than routed.

`SettingsPanel.tsx:1` states it plainly: Settings is *"the Settings overlay. `.dialog-overlay`
backdrop"*. `.dialog-overlay` is `z-index: 100` (`dialogs.css:14`); `.toast-stack` is **`z-index: 90`**
(`toasts-and-overlays.css:11`, whose own comment reads "below `.dialog-overlay` (100)"). So every
toast raised from inside Settings renders **under the 45% scrim**, and is **clipped** by the 880px
card where they overlap vertically — measured by the designer at 172px of 360 at 1280×800; at
1920×1080 a short toast clears the card and is merely dimmed.

**`ui-reference.md:2377` already rules this**: *"A Settings-surface error is inline, never a toast"*,
with the signed P107 `.settings-row-note--warn` recipe (12% `--warning` tint,
`inset 3px 0 0 var(--warning)`, **`--text-1` ink** — the `--text-1` is what passes AA in light). So
the fix is pure reuse of an existing signed recipe, not new design. Two riders: if that note goes
live the `DevCategory` announcer must **not** also fire (`ui-reference.md:2386`, one live region per
section per tick), and the confirm dialog itself is fine — toast push and dialog close land in the
same synchronous continuation, one React commit.

**Why neither the harness nor two code reviews caught it:** every harness pass on this UI, mine
included, asserted via `innerText` / `get_page_text`. **Text extraction is blind to z-index
stacking** — it proves a string exists in the DOM, never that a human can see it. Any future claim
that something is *visible* needs computed style or bounding boxes.

**The decision the user owes:** fix only the F6 delete outcome, or sweep every toast Settings
raises. §6.11's copy is channel-independent, so the strings shipping now are correct either way and
nothing is blocked on the answer.

**MEASURED by the orchestrator in the harness, 2026-09-14 — the claim is confirmed and is worse than
stated.** `document.elementFromPoint` at the toast's own centre returns **`dialog-overlay <DIV>`**,
not the toast (`onTopIsToast: false`). So it is **not merely dimmed — the toast is unclickable, and
its ✕ dismiss button cannot be reached at all.** Numbers at 1280×720: `.toast-stack` computed
`z-index: 90`, `.dialog-overlay` `z-index: 100` with `background: rgba(0,0,0,0.45)`; toast
360×37 at (908, 52); settings card 880×656 at (200, 32); **horizontal overlap 172 of 360 px**
— matching `ui-designer`'s independently-derived 172 px exactly — and the toast's full 37 px height
falls inside the card's vertical extent. A screenshot confirms it visually: only the string's tail
and the ✕ escape the card's edge, dimmed.

**This is the method correction that matters more than the finding.** The reason to use
`elementFromPoint` rather than `innerText` is that the previous verification of this very UI — mine
included — proved only that a string was in the DOM. Going forward: **a claim that something is
*visible* or *clickable* needs `elementFromPoint`, computed style, or bounding boxes. Text
extraction cannot support it.**

### 🆕 P113 — Settings inline notes (ruling #24). CONTRACT SIGNED 2026-09-14, implementation in flight

`docs/contracts/P113-settings-inline-notes.md` (new) + `ui-reference.md` **§12.14** "Outcome notes —
the Settings surface has no toasts (SIGNED 2026-09-14)". §12.13's two old bullets at `:2377-2390`
became a pointer to it, since they were cross-section rules living inside the P112 picker section.
**No new tokens.**

**The design question I flagged got a better answer than the one I framed.** I asked whether success
outcomes should get an inline note, stay as toasts, or something else. The decisive argument turned
out to have nothing to do with the scrim: **`deleteResultToast` computes `tone` at runtime**, so one
button press yields `success` or `error` — routing by tone would put the result of a single action in
**two different places on screen depending on what the backend returned.** So: one component, two
tones, and the success variant is deliberately quieter (`.settings-row-note--result`, `--text-1` ink,
no tint, no bar, no `role`). A `--success` tint was rejected as new chrome, louder than the row's own
state note, for the least consequential four of the ten.

**Two Dev slots, not five** — all three Dev actions are `anyBusy`-gated so they cannot collide.
**Row 8 (remove-host failure) goes in `.dialog-error`, not a row note**, because `confirmRemove`'s
failure branch never clears `removeTarget`, so its confirm dialog stays open — a row note would have
sat behind a **second** overlay, reproducing the defect one layer up. Live regions: DevCategory keeps
the **one** it has; Accounts gains **one**; row 8 announces via the dialog's own `role="alert"` and
the section announcer stays silent.

**A live a11y bug this uncovered, fixed in passing:** a live region does **not** re-fire on an
identical string. Export twice and React skips the identical state, so **the second outcome is
announced silently — today, in shipped code.** The sweep would have carried it forward; `begin(key)`
now clears the announcer to `''` at operation start. Relatedly the reveal failure sets **no**
announcement at all today (toast-only), so inline-only would have made it visible-only — adding one
is a MUST in the contract.

**SEQUENCING CONSTRAINT — do not lose this.** P112 **sub-inc 4** references
`.settings-row-note--warn`, which has **no CSS until P113 lands**. Either land P113 first, or
sub-inc 4 must carry the recipe itself.

**P113 residuals, each independently deferrable:**
- **`P113-F1-accounts-error-copy` (own increment):** rows 6-8 append **raw `errorMessage(e)`** to a
  mapped lead — raw backend text, now **permanent instead of transient**. Mapping it needs the
  backend error-kind inventory, so P113 renders verbatim with `overflow-wrap: anywhere`.
- **Recorded, not fixable by lint:** a toast raised by a **background** event while Settings is open
  is equally invisible, and no lint rule can catch that. Deliberately changing no z-index.
- **NIT, deliberately not done:** `DevToast` / `deleteResultToast` / `devDeleteToastRows.test.ts`
  become misnomers. Renaming churns two test files for no behaviour change; the lint guard covers the
  real risk. But the comments at `DevCategory.tsx:111-114` and `:119-120` that *assert* toast
  behaviour are being corrected in place.

### ✅ P112 sub-increment 2 — reviewed, audited, committed `a2eb091` (transcript: archive Part 73.1)

- **The audit property, kept because it is what a reviewer checks a diff against:** every
  renderer-reachable write to `settings::Settings` funnels through
  `commands::ui_settings::apply_patch`, whose input type `UiSettingsPatch` has **no field able to
  carry a path**, and whose two `String` fields are never stored — they are replaced by a
  `&'static str` catalog literal via `coerce_tool_id`. No CRITICAL/HIGH/MEDIUM. The three **non-type**
  facts it rests on are in `### The security record`.
- **The AMEND-6 LOW is CLOSED** — the heterogeneous-separator `is_unc` hole was sub-inc 2's MUST-FIX,
  and the two new mixed-separator cases were run **red before, green after**. The most load-bearing
  artifact in the increment is the exhaustive **36-field destructure with no `..`**: a new field on
  the patch type becomes a compile error.
- **STILL OPEN — the observability-capture privacy decision.** `setUiSettings`/`getUiSettings` are
  both already in the captured-command list (`obs/metrics_cmds.rs:130,209`) and `obs/record.rs:207`
  has an optional raw-payload capture. Now that `pick_external_tool` returns a browsed path and
  `tool_scan` returns `DetectedTool.detail`, a **filesystem path can be written to a log file on disk
  under dev mode**. Decide whether those two commands stay in the capture set — same class as P91's
  home-masking work.
- **Trust boundary, stated once so nobody later reads it as a gap:** a **hand-edited `settings.json`**
  carrying `customEditorPath` **is honoured**, deliberately and in scope. The property is scoped to
  *a **renderer-written** program path is unrepresentable*; someone who can edit `settings.json` can
  equally replace the binary it names.

### ✅ P113 PHASE 2 — LANDED `dcff54b`, REVIEWED + BOTH MUST-FIX FIXED `2b3dfd6` (2026-09-16). AC1 says zero, and it was earned

**AC1's full accounting**, the check that has never been run in this form and whose absence lost five
call sites: `rg -n "pushToast\(" src/` → **233**, every one classified.
**0 reachable from the Settings surface.** One deliberate survivor
(`useSettingsSaveFailure.ts:70`, gated on `!settingsOpen.current`); **11 background callers** that can
fire *while* Settings is open; 3 in `useExternalTools` **accounted-for, not swept** (repo-UI today, in
scope when P112-4 lands a picker); 3 interface *declarations*, not calls; 7 in tests; 1 mock seam;
207 repo-UI where a toast is correct and visible.

**§13.3's background case is now proven, not theoretical.** Auto-fetch raised `Fetched 2 refs` with
Settings open, the new guard tripped, and the toast's centre hit-tested `dialog-overlay`.

Numbers: tsc 0 · eslint 0 errors in touched files · ratchet clean · **full frontend vitest 2903/259**
· **full Playwright e2e 185 passed, 1 skipped, 0 `console.error`**. `useMcpControls`'s `pushToast`
**parameter is deleted** — the hook can no longer regress into raising one.

### 🚨 A4, FINDING 6 — my approval was ambiguous and the implementation lost the raw error

The implementer read "Settings closed → the toast, unchanged" as **the channel** unchanged, and ships
**one string in both channels**. **I approve that half** — a single string cannot drift, and the new
copy is true in both contexts.

**But I checked what happened to the underlying error and it is gone.** `useSettingsWriteQueue.ts:128`
calls `noteFailure(streak)` with **only the streak**; there is no `errorMessage(e)`, no `console`, no
log call anywhere on the new path. The raw OS error previously rode in the toast text
(`Could not save settings: ${errorMessage(e)}`) and **now nothing captures it at all.**

That is a **diagnostic regression**, and a pointed one: the approved copy tells the user to *"check
that Bonsai can write to its config folder"*, which is a **guess** — disk-full, permission-denied,
path-too-long and a locked file all produce the same sentence, and the raw error is exactly what would
distinguish them. P91's whole observability programme exists so failures are diagnosable; this change
quietly removed the only record of a real one. **Routing: keep the user-facing string, and preserve
the raw error on the diagnostic path (dev-mode log).** Not a copy change — the string stays as
approved.

### 📐 Two contract corrections earned by measurement — DELIVERED to `ui-designer`

§10.3's sub-pixel residue (`scrollIntoView({block:'nearest'})` alone settles at note-bottom
**687.171875** against a clip bottom of exactly **687**, so **`Math.ceil` → 1270** is what makes the
four-edge test pass) and §8.2's key-scoped clearing, **withdrawn in `2aee970`**. Both measurements
and both concrete cross-key cases: archive Part 73.3.

### ✅ `watcher::tests::git_internals_filtered` — SETTLED, and my mechanism was wrong

**I characterised this four times and was wrong three times.** Final, evidence-based reading, from the
reviewer who actually ran it rather than reasoned about it:

**Three for three green today**, including the exact `gate.mjs:148` fallback: `cargo test --workspace`
green with this test included, `cargo test -p bonsai --lib` green, `nextest --workspace` green.

**My "nextest isolates processes, `cargo test` uses threads" mechanism is refuted by two facts:**
- `serialize_watcher_test()` (`src-tauri/src/watcher/tests.rs:20-24`) is a **process-wide mutex**.
  Under `cargo test` it serialises the watcher tests against each other; under nextest,
  process-per-test makes it a **no-op**. If the process model were the variable, **nextest would be
  the LESS protected configuration** — the opposite of what I claimed.
- The one recorded failure was under `-p bonsai --lib`, the **lightest** configuration, not under
  full-suite load.

**What actually fits: ambient load.** The mutex excludes only the ~4 other watcher tests, so ~538
same-process tests still contend with the `notify` backend and debounce threads.
`git_internals_filtered` (`tests.rs:127-153`) declares quiet after a 1 s residual sweep, then asserts
**nothing arrives for 1500 ms of wall clock**. Under enough ambient load a stale init event delayed
past the sweep lands inside that negative window. **The real variable is almost certainly my own
parallelism** — concurrent agents running vitest/tsc/e2e/cargo while a wall-clock negative assertion
is timing out. Same root cause as the happy-dom timeout class.

**Two facts about the gate worth keeping:**
- The `cargo test --workspace` fallback is a **fresh-contributor path, not a pipeline path** —
  `hasNextest` gates it (`gate.mjs:86`), this box has nextest, and CI installs it explicitly
  (`.github/workflows/ci.yml:92`). It is real but never exercised by our own gates.
- **The local gate is STRICTER than CI on flakes:** CI runs `--profile ci` with `retries = 1`
  (`.config/nextest.toml`); `gate.mjs` uses `default` with **no retries**.

**Deterministic fix direction (follow-up, not urgent):** `watcher::classify::is_relevant` already
pins `objects/aa/bb ⇒ false` as a **pure unit test** (`classify.rs:118`), so the negative-window half
of `git_internals_filtered` is **redundant coverage carrying all of the timing risk**. The `.git/HEAD`
positive (`tests.rs:149-152`) is the half that earns its keep.

**Stop re-characterising this.** It is ambient-load sensitivity in a wall-clock negative assertion.

**Two earlier characterisations of this test are history, not competing claims:** the "orphaned vite
dev server as a candidate contributor" mechanism (archive Part 71.2) and the "did not reproduce" note
further below. **The canonical reading is the one in this section.**

### ✅ P113 PHASE 1 COMMITTED `0c86376` — approved, no MUST-FIX (review narrative: archive Part 73.4)

Ten call sites moved from toasts to inline notes. Its three review follow-ups were all routed into
phase 2 (the stale host note **resurrecting** when a removed host is re-added; rows 9/10 emitting
byte-identical announcements so the second is silent, fixed by `flushSync(() => begin(key))`; the
`scrollIntoView` relaxation). **Its NITs are still filed:** `begin(key)` clears the announcer
**globally** while clearing one key's note, and Accounts has no busy gate; the mock path citations in
`forge.ts:438,459` are off (`:340`, and the path needs `src-tauri/`); `?forgeRemoveFail=long` omits
§14's 60-char host half.

### 🔧 P113 PHASE 2 — the five missed sites, the guard redesign, the approved banner

- **MCP four** → row slots, `REGISTER` **scope-keyed** (two register rows would otherwise share one
  key). After the sweep `useMcpControls` has no `pushToast` caller: **the parameter is deleted**, so
  the hook cannot regress into one.
- **`useUiSettings:287` → banner when Settings is open, toast when it is not.** The hook serves the
  whole app, so density and sidebar writes fire it with Settings **closed**, where the toast is
  already right. **This is the one call site in the sweep where `pushToast` must survive.** The
  distinction that licenses it: routing by *where the user is looking* is legitimate; routing by
  *what the backend returned* is not — the same rule that settled one-component-two-tones.
- **The banner is a rendering of state that already exists.** `useUiSettings` already keeps a failure
  streak and already fires one toast per streak, not per retry. Strictly better than the toast, which
  fired once and vanished while the condition persisted. (The streak is a `ref`; needs a state
  mirror.) Retry button calls `armSettingsSave(0)` — the backoff is 300/600/1200 ms then **stops**,
  after which the pending patch sits unsent.
- **A4 copy APPROVED**, verified clause-by-clause against `useUiSettings.ts:265-295` before approval.
- **The guard is redesigned producer-side.** A path lint is **import-graph-based and structurally
  cannot** see `pushToast` passed as a *parameter*; widening the glob achieves nothing, since the
  hooks never import the toast context. Replaced by a DEV-only assertion inside `pushToast` that
  fires when the Settings surface is mounted — one mechanism covering all five missed sites **and**
  §13.3's background-toast case, which the contract concedes no lint can catch. **AC1 rewritten to
  enumerate every `pushToast(` in `src/` and account for each; it has never been run in that form,
  which is why five sites went missing.**
- **`?settingsSaveFail=1` added** — nothing in the mock rejects `setUiSettings` today, so the app's
  highest-traffic Settings toast **has never been seen rendered by anyone**.

**One more site for when P112-4 lands:** `useExternalTools.ts:22/:34` are repo-UI only today, but
sub-inc 4 puts a tool picker **in** Settings, and a Browse failure raised there hits the same scrim.

### 🚨 NEW 2026-09-14 — "Remove account" reports success even when the token was NOT deleted

Found by the P113 implementer while tracing which of row 8's error strings are actually reachable;
**I verified it against source.** This is not a copy issue — it is a credential-storage defect, which
is explicitly on the `security-auditor`'s standing mandate.

`src-tauri/src/commands/forge_accounts.rs:280-310`, `forge_remove_account_inner`:

```rust
let _ = bonsai_forge::delete_token(&r.keychain_key);   // swallowed
...
let _ = settings::update(&file, |s| { ... });          // swallowed
Ok(())                                                  // unconditional
```

**Both substantive failures are discarded and the closure returns `Ok(())` regardless.** The only
error the command can ever surface is `task join error` (plus a `cannot resolve app config dir` in
the caller). Consequences:

1. **A user who removes an account is told it succeeded while the credential may still be in the OS
   keychain.** That is a trust statement the app cannot actually back.
2. A failed `settings::update` means the account **reappears on next launch**, again after a success
   report.
3. Knock-on for P113: the `.dialog-error` path specced for row 8 is **near-unreachable in the real
   app**, so nobody should over-invest in that copy — the mock exercises it, the product barely can.

**Fix is not just propagating the errors** — the two halves have different semantics. A failed
`delete_token` leaves a live credential and should be surfaced loudly; a failed `settings::update`
leaves the account listed. Removing one without the other is a partial state, and the copy has to be
able to say which half happened. Needs a contract decision before implementation, and a
`security-auditor` pass on the result.

### 🆕 SECURITY AUDIT of the `.cmd` launch change — CLEAN (full reasoning: archive Part 74.2)

Nothing CRITICAL/HIGH/MEDIUM, and clean **structurally**: `is_dir()` is itself the character filter,
because the only characters that defeat std's batch quoting — `\r`, `\n` and `"` — are **illegal in
Win32 path components**, so the dangerous inputs are *unreachable*, not blocklisted. VS Code's
`code.cmd` was read rather than recalled: no `call`, so no double expansion. **`idea.cmd` is
UNVERIFIED** (JetBrains not installed on this host). The one datum that would upgrade this if wrong:
whether git-on-Windows can be coerced into checking out a directory name containing `"`.

### 🆕 A ROBUSTNESS REGRESSION the `.cmd` fix introduced (LOW-1) — recorded, not fixed

`code.CMD` **spawns successfully whenever `cmd.exe` exists** — even if `Code.exe` has been deleted —
because the failure then happens *inside* the batch file, **after `spawn()` returned `Ok`**. With
`hide_console = true` the user sees nothing. Previously the extension-less shim's `os error 193` made
rung 1 fail and the ladder advanced to `code-insiders`. **Net: a broken primary install now yields a
silent no-op instead of falling through.** No cheap fix — `wait_for_exit` is wrong here, it would
block on the editor's lifetime. Deliberately recorded rather than patched.

### 🆕 Further audit items — filed, not routed

- **INFO-2, pre-existing: a bearer token transits a cmd.exe command line.** `ai/mod.rs:399` passes
  `format!("Authorization: Bearer {token}")` as an **argv element** to `claude`, which on Windows
  resolves to `claude.cmd`. Bonsai-generated so not attacker-controlled, and std's `%` handling
  protects the value — but it is a secret **visible in process listings**. Standard practice for that
  CLI; recorded because secrets-in-argv is in scope.
- **A UNC `PATH` entry sends `is_file()` to the network, inside `resolve_in`.** Passes
  `is_absolute()`, and is inconsistent with the house `is_unc` rule applied everywhere else. This is
  the **real** hang source — see the AMEND-6 correction: detection's UNC refusal is *post-hoc* for the
  `OnPath` rung, so preventing the I/O needs a guard **inside `procutil::resolve_in`**, which changes
  app-wide resolution and is its own change.
- **`procutil.rs:41-43`'s separator shortcut bypasses both guards**, returning `PathBuf::from(program)`
  verbatim. Unreachable in production — `external_cmd::validate_command_setting` accepts only a bare
  allow-listed name or an absolute existing file — but it is a **structural dependency on upstream
  validation**, which is worth knowing before anyone adds a caller.
- **RECOMMENDED PROCESS CHANGE (needs the user, since the sibling rule is a user ruling):** add
  `BrowsedProgram::from_settings_field` to the same mandatory-`security-auditor` path trigger
  `CLAUDE.md` already carries for `crates/bonsai-mcp/src/server/tools_*.rs`. The type's whole value is
  that wiring a request body into a launch becomes a **deliberate, greppable act** — and a greppable
  constructor only buys something if someone greps. Not added unilaterally.
- **Truncation is silent** — no ellipsis, so a 512-char-truncated subtitle can read as a complete
  path. Routed as a cheap fix. Grapheme clusters can also split (base kept, combining mark dropped);
  no attacker under the stated trust model.

### 🆕 P112 follow-up pass — IMPLEMENTED, reviewed + security-audited (transcript: archive Part 74.3)

Seven routed items plus the shipped resolver fix (`fd93616`). `looks_absolute` is **deleted**, and
the **UNC arm is deliberately NOT shared** between detection (`detect::locally_absolute`) and browse
(inlined in `validate_custom_program`), each citing AMEND-6 — a fused `is_local_absolute` was
reverted as the reviewer's MUST-FIX. **Its own residue is still open and is listed on this board:**
the LOW-1 robustness regression above, the further audit items above, and the contract deltas owed to
`architect` below.

### 🆕 MEASURED, LEFT UNFIXED — a cold first scan can spend the registry budget before using it

`SCAN_REG_BUDGET`'s clock starts at `HostToolEnv::new()`, **before any filesystem work**, and the
scan is dominated by the PATH walk, not the registry: **55 directories × 11 `PATHEXT` entries
≈ 4400 stats, measured at 2.1 s cold / 0.45 s warm, against a 1500 ms budget.** So on the first scan
after a cold boot the budget can be fully spent before the first `AppPaths` rung runs.

**I verified the safety argument and it holds, with a sharp limit.** All three `AppPaths` rows do have
`WinFolder` siblings — `Code.exe` (LOCALAPPDATA + ProgramFiles), `sublime_text.exe` (ProgramFiles),
`notepad++.exe` (ProgramFiles + ProgramFiles(x86)). **But `WinFolder` only covers DEFAULT install
locations, which means an exhausted budget degrades precisely the case `AppPaths` uniquely exists to
serve:** a tool installed somewhere non-standard is silently not offered. Browse is the workaround
(and per ruling #26 now accepts UNC). Fix is its own change — start the deadline at the first
registry call. The `SCAN_REG_BUDGET` doc was corrected; "a few tens of milliseconds" was wrong.

This also makes the scan itself a **UX** concern for sub-inc 4: 2.1 s cold is a visible wait in a
picker, and §3's cache/refresh design has to absorb it.

### 🆕 `BrowsedProgram` is a PARTIAL control — recorded so nobody reads it as stronger

B1 asked for a newtype constructible only inside the settings module. **That is not expressible
here:** `settings` lives in the `bonsai` (src-tauri) crate and `tools` in `bonsai-core`, and Rust has
no cross-crate module privacy (a sealed trait would also block src-tauri). So
`BrowsedProgram::from_settings_field` is `pub`, with **no `From` / `FromStr` / `Deserialize`** and
that prohibition documented on the type. **Delivered property: a request-body `&str` no longer
type-checks into `tool_scan` / `picked`. It does NOT prove origin.** Both `reviewer` and
`security-auditor` were asked independently whether it earns its keep; if either says ceremony,
delete it rather than keep a control that reads stronger than it is.

### 🆕 Two failures observed during the follow-up pass, neither caused by it

- **`watcher::tests::git_internals_filtered` — DID NOT REPRODUCE. I overstated this.** I recorded it
  as "a gate flake" because the gate runs the whole workspace; the reviewer then ran the full
  `-p bonsai --lib` and got **531 passed / 0 failed** with that test `... ok` (the original
  "530 passed / 1 failed" sums to the same 531). **One unreproduced observation is not a flake** — no
  gate action. Worth filing only if it recurs.
- **`health::tests_sections::perf_ceiling_on_20k_fixture` — NOT drift. I overstated this too.** I
  recorded 2067 ms against the 2000 ms budget as "possible drift". Best-of-3 on the same host is
  **1542 ms, 23% UNDER budget**. The 2067 ms reading was host load: `branches` alone swung
  **1155 → 2273 ms across three passes in one run**, which is the entire variance budget. Nothing to
  file beyond noting the headroom is thin — and noting that a single timing sample on a loaded
  machine is not evidence of drift, which is the same mistake in both of these bullets.

**Contract deltas owed to the architect** on `P112-external-tool-detection.md`: the §2 signatures now
take `BrowsedProgram`; the §4 example becomes
`tools::picked(&s.terminal_tool, ToolKind::Terminal, BrowsedProgram::from_settings_field(&s.custom_terminal_path))`;
AMEND-5 items 1, 2 and 7 are now reflected in code; and the `.cmd`-hit consequence needs stating.

### ✅ CLOSED 2026-09-16 — "Open in editor" was broken on Windows; the fix shipped in `fd93616`

- **Fixed.** `procutil::resolve_program` now prefers `PATHEXT` matches over the bare name, skips
  **empty** `PATH` components and requires `is_absolute()`. Curator-verified 2026-09-16 against the
  shipped doc comment on `resolve_program`, which records the mechanism and the measurement.
- **What was broken:** the bare name resolved to VS Code's extension-less POSIX shim (2073 bytes,
  `#!/usr/bin/env sh`), and spawning it fails with **`os error 193`** — so the Windows
  `editor_ladder`, which opens with a bare `code` rung, **failed outright on a standard install**.
  The `spawn()` measurement table (shim **ERR 193** · `bin/code.cmd` OK · `Code.exe` OK) is preserved
  in **archive Part 74.4**.
- **AI resolution measured unmoved:** `claude` resolves to `claude.EXE` under **both** orders, `git`
  unchanged, stat counts **4375 vs 4376** — the reorder is free. Windows git resolution also inherits
  the empty-component / `is_absolute` guards, because `gitbin`'s `resolve_on_path` delegates here.
- **The security consequence is LIVE, not archived** — see `### The security record`: the launch
  surface flipped `Registry → Path` for `vscode`, so the app now launches `code.CMD` rather than a
  PE, through the mitigated CVE-2024-24576 / "BatBadBut" path.

### 🆕 NEW 2026-09-14 — follow-ups both P112 reviews produced, NOT routed

Ranked. None is a MUST-FIX; `reviewer` and `security-auditor` both passed the increment.

1. **The house bidi/control predicate is incomplete, in three identical copies** (LOW-4, and
   **pre-existing — the new module faithfully reused it**): `tools/custom.rs:44-48`,
   `external_cmd.rs:142-144`, `ai/stream.rs:383-385`. The set is `char::is_control()` (Cc only) plus
   U+200E/200F, U+202A-202E, U+2066-2069. **Omitted:** U+061C ARABIC LETTER MARK (a genuine bidi
   control of the same family); U+200B-200D, U+2060, U+FEFF, U+00AD, U+180E (invisible ⇒ look-alike
   paths); and **U+2028/U+2029, which are Zl/Zp — NOT `is_control()` — and render as line breaks in
   a DOM label**, defeating the "a newline cannot appear in a program name" intent outright. Fix once
   as a category predicate (reject Cc + Cf + Zl + Zp) applied at all three sites so they cannot
   drift. Homoglyphs (Cyrillic `с` for `c`) are not strippable by any filter — accepted residual,
   worth one contract sentence. ANSI escapes are already adequate (ESC is Cc, leaving inert `[31m`).
2. **`crates/bonsai-core/src/gitbin.rs` is at EXACTLY 500 lines** — zero headroom; one added line
   trips the ratchet, which also means **comment-only fixes there are blocked**. `refactorer`.
3. **A second Delete click reports deleting a `usage.json` that is already gone.**
   `src/ipc/mock/obsLogFixture.ts:194-199` returns `deletedMetrics: 1` unconditionally and the
   fixture's `logFiles` is never consumed by the delete, so click #2 re-renders a success against an
   emptied fixture. **Pre-existing, not a regression** (the old constants behaved identically), but it
   is exactly the class that file's own header condemns, and §6.4 dropped the `hasLogs` gate so the
   button stays enabled. **If fixed, it must ship with a reset export wired into
   `obsDeleteCounts.test.ts`'s `beforeEach` beside `ringClear()`**, or the test after a
   successful-delete test inherits the flag and goes flaky.
4. **`procutil`/`gitbin` duplication for `refactorer`:** `custom.rs:155` `is_mac_bundle` duplicates
   `HostToolEnv::is_bundle` (`detect.rs:110-112`), and the `mode() & 0o111` check now exists **three**
   times (`custom.rs:160-163`, `detect.rs:120-123`, `gitbin.rs:199-202`).
5. **Mock seam polish** (all NIT, `src/ipc/mock/obsLogFixture.ts`): flag precedence undocumented and
   `?obsMetricsFresh=1&obsDeleteFail=1` self-contradicts (reports a held-open file that `fresh`
   asserts is absent); `DeleteFailMode`'s `'metrics'` member is unreachable by any query value;
   `countFlag`'s doc overstates strictness (`parseInt` makes `3abc` ⇒ 3); three exports
   (`MOCK_USAGE_BYTES`, `MOCK_ROLLED_PART_BYTES`, `MOCK_EXPORT_ZIP_BYTES`) are module-local.
6. **One signed string has no test:** `Deleted 1 export.` (T1 with `logParts === 0, exports === 1`).
   The invariant loop's fixture has `logParts 3`, so it exercises ` and 1 export` instead.
7. **Picker subtitles will show uppercase extensions** — the host scan returns `detail` values like
   `...\wt.EXE` and `...\pwsh.EXE`, because `PATHEXT` entries are uppercase. `ui-designer` polish for
   sub-inc 4, not a backend bug.

**Two `ui-designer` contract amendments owed on `P91-privacy-copy-ui.md`** — the code is right and
the signed text is wrong, so these are text fixes, not code fixes:
- **§6.11.3's T2 template omits `{rolled?}`, but shipped T2 already carried the clause.** Omitting it
  would regress R13c's own complaint. Add it, and add the row the harness now reaches
  (`?obsLogFiles=0`, Dev ON): `Usage counts cleared. 4.0 KiB freed. Still recording — Bonsai started
  a new log file.` — the table's claim to cover "every state the harness and the tests reach" is
  false by exactly that row.
- **AC5 is unsatisfiable as worded.** It says a user with `totalFiles === 0` can produce no string
  containing `log file` and explicitly includes the Dev-ON case — but Dev ON ⇒ `rolled` ⇒ `Bonsai
  started a new log file.`, which R13c requires. The two clauses contradict each other. Reword to
  "no **counted** log file (`{N} log file`)", which is what the implementation asserts.

### 🆕 NEW 2026-09-14 — per-category failure counts need a `purge_scope` split, not just a struct field

§6.11.4's precise failure copy is **conditional** on `failedLogs` / `failedExports` existing (two
fields, not the architect's three — see the F6 entry above). The non-obvious part:
`src-tauri/src/obs/sink.rs:60-67`'s `PurgeCounts`/`PurgeReply` carry **one** `failed_files` across
log parts *and* export zips, so the attribution has to be split **inside `writer::purge_scope`** —
the IPC struct is the last place it surfaces, not where the information is lost. Until it ships,
§6.10 R10's three-row table stays the live spec. Architect's call on the `purge_scope` signature.

### ✅ Closed 2026-09-11 by orchestrator verification (user assented) — one line each, Part 64

- **The four 2026-09-02 file-size refactor follow-ups — CLOSED:** `52c815e`, `6092eb3`, `338d71f`
  (overlay teardown), `1d9d9bf` (disarm armed dialogs), `734b310` (watchdog on an injectable clock).
- **The two 2026-09-01 velocity follow-ups — CLOSED as superseded** by `737cc4b` (band the two
  slowest proptests) and `5731d37` (workspace test wall −14%).
- **`P91-raw-args-privacy.md` — CLOSED, already done:** folded into `P91-observability.md` §7.4 by
  `12b0ab6`; the file is absent.
- **`P91-observability-ui.md:496`/`:951` — `INDEX.md` was right, the BOARD was stale**; `fc9c36e`
  fixed it. The architect's own `P91-observability.md` §6/§10 are the copies **still** stale.
- **P87b `FU-1..4` — three of four were NOT open.** FU-1 shipped `1d8c6f9`; **FU-2's premise is
  false** (`commitAmend` *is* activity-wrapped, `staging.rs:179`); FU-3 closed by `833f2f9`; FU-4
  answered by *rejecting* the change (`763866a`). **Do not re-open them — the board has been wrong
  about this entry three times.** Only the `AiActivityPanel` aria-label NIT is arguably live, and
  `AiActivityPanel.tsx:192-193` already carries `role="region"` + `aria-label="AI activity"`.
- **P94 has no contract file — CONFIRMED, accepted as debt.**

### Still open, short form

- **P87b contract-hygiene residue — filed, NOT verified closed.** Five corrections in
  `docs/contracts/P87b-FU1-run-target.md` (§1 line-count estimates · §3's drifted
  `remote_push_activity.rs` ranges · §4's wrong unborn-HEAD rationale — the `if head.unborn ||
  head.detached` guard is **load-bearing**, not redundant · §8's understated seams + its 83-char
  `MOCK_LONG_TARGET` · §8/§9's **literal U+202E / zero-width chars in the very section that forbids
  them**), plus `P87b-FU1-FU4-git-dock-ui.md` F-F(a). `2aa1e06` and `f00fad3` claim to have closed
  some; **which ones is unverified.** Hand the list to the next `architect` spawn on that file.
- **STANDING WARNING — do NOT ungate `''` from `hasExecExt` in `whichAll`**
  (`scripts/lib/spawn-tool.mjs`). npm/corepack install **extensionless POSIX shell scripts** beside
  every shim and `resolveTool` picks `hits.find(p => !isBatch(p))`, so an unconditional `''` selects
  an unrunnable script. 3 regression tests guard it.
- **STANDING WARNING — the toast auto-dismiss timer fix is deliberately REVERTED.**
  `React.StrictMode` makes cancel-on-unmount strand a toast on screen permanently;
  `useToastQueue.test.tsx` carries the finding. Reasoning: archive Part 52.4.
- **Deliberately not taken:** `src/obs/types.ts:68` still cites `A26 §D`, the dead lettered scheme
  retired inside `raw_args.rs`. A repo-wide letter→section migration is a decision, not a drive-by.

### P91 — open items (branch merged 2026-09-11; one AI-gate item still owed)

Milestone detail: Part 54.6 · security arc 42 · audit F1–F9 43 · build diary 44 · SHOULD-FIX full
text 45 · pre-condensation board text 69.1. User decisions + architectural rulings: see
`## Accepted decisions` above.

- **OWED AI-GATE ITEM — the real `logs/*.jsonl` parse** → the USER ACTIONS block above. A native
  checkpoint confirmation does not reach it.
- **F7 — LOW, mostly latent.** `redact_names` misses bare ref/file names and never touches JSON keys
  (`feature/acme-client-migration` would be written verbatim into a strict file). `strict::enforce`
  is the **sole** enforcement point for Rust *and* frontend records, so a gap there is a single point
  of failure. **F9 — INFO:** the two redactors cannot disagree because only one enforces (`redact.ts`
  has no `redact_names`) — which is why F7's gaps matter more than their reachability suggests.
- **SHOULD-FIX: `SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`,
  `:409-425`) — two savers can snapshot A→B but lock B→A, so **a `metrics_reset` can be silently
  undone on disk**. **⚠️ CONTRADICTION flagged 2026-09-14, unresolved:** `P91-observability.md` §13
  **row 32** records this exact defect as **fixed in `bcb3720`** (monotone `rev`/`commit_rev` stamp)
  and §8.3 ratifies the design. Verify against the tree before doing any work here.
- **SHOULD-FIX: the `last_fire` prune assumes non-decreasing `ts`** (`window.rs:52-63`), which merges
  two unsynchronised clocks (`src/obs/log.ts:43`, `sink.rs:385`) with no monotonic clamp; blast radius
  is a **duplicate** anomaly record, never a missed one. **⚠️ Same contradiction class:** §13 **row
  33** records the premise as stated in code and a `cutoff` guard as **rejected on merit**.
- **NIT:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms apart against
  a 300 ms window, making `len <= 4` nearly tautological; `last_fire` has **no numeric cap**, unlike
  `open_calls` (FIFO 1024) and `slow` (LRU 200).
- **Contract follow-ups owed to `architect`:** §6/§10 still specify the removed
  `log_export_session(dest?)` — **confirmed live**. The other three on the old list (`cmd.*`
  camelCase, `MAX_KEYS_PER_MAP` ratification, decision 25's counter-key shape) **appear already
  discharged** by §13 rows 28, 29 and 25 — **contradiction flagged 2026-09-14, not closed.**
- **`.forge-connect-link:hover` is a no-op** — the resting-underline MUST-FIX means hover declares the
  same underline, so the link has **no hover feedback at all**. → `ui-designer`.

### The security record — where it lives, and what is still open

- **`SEC-2026-09-11`** (MCP tool-contract audit of `2a0b8f1`: HIGH `stage_paths` symlink escape,
  MEDIUM `git add -f` over MCP, 4 LOWs, the PROCESS finding) — **implemented `216ca45`**. Report:
  `docs/audit-2026-09-11-mcp-tool-contracts.md`; board narrative **archive Part 65**.
  **`SEC-2026-09-11b`** (review of that implementation) — **archive Part 66**; its one open item is
  the UNC ship-blocker in the queue above.
- **Both parts carry a verified-CLEAN register — READ THEM BEFORE RE-AUDITING THIS GROUND**, together
  with each register's explicit "NOT checked, so this does not over-claim" list (in Part 65 that
  list is `merge_branch`/`rebase_branch` `operationInProgress`, merge autostash +
  `stashPopConflicts`, `create_stash` claims, stash apply/pop outcome tags, `unstage` atomicity, the
  `commit` `hookRejected` vs `configMissing` mapping, `rebase_skip`, `list_repos`/`select_repo`).
- **Pre-existing, deliberately out of scope:** the **webview** path —
  `src-tauri/src/commands/staging.rs:19-23` still passes frontend paths straight to `stage_paths`,
  keeping `git add -f` semantics with **no status-membership check**; the *escape* half is closed for
  that caller too. Also: `ensure_within_workdir` treats `.git` as inside the boundary.

### The external-tool launch surface — SEC-2026-09-14/15 (three audits, all CLEAN at HIGH and above)

- **The three audits:** sub-inc 2 ("unrepresentable, not rejected" **holds**), sub-inc 3 (*"a net
  reduction in attack surface"* — it **deletes** the free-text program capability rather than putting
  a validator in front of it), and the `.cmd` launch change. Board narratives: archive Parts 71.3,
  73.1 and 74.2.
- **THE LAUNCH SURFACE NOW RUNS A BATCH FILE.** With `PATHEXT`-first resolution (`fd93616`),
  `vscode`'s provenance flips **`Registry` → `Path`** and its program becomes `…\bin\code.CMD`
  instead of `Code.exe` — **so Bonsai launches a batch file where it previously launched a PE**,
  through std's case-insensitive batch detection, i.e. the **mitigated CVE-2024-24576 /
  "BatBadBut"** path. Audited CLEAN. **Deliberately not tightened to `.exe`-only:** the Windows
  `idea` row has **only** a `Rung::OnPath` and JetBrains ships `idea.cmd`, so tightening would delete
  a catalog row. The only attacker-influenced argv source is **a crafted filename inside a cloned
  repository**, not `PATH`. MSRV is sufficient and deliberate — `rust-toolchain.toml` pins
  `channel = "1.97"` and the mitigation landed in 1.77.2.
- **THE SINGLE FACT A FUTURE REFACTOR MUST NOT BREAK:** *no code outside `bonsai-core` constructs a
  `PickedTool` literal.* Everything AC6 claims rests on it, and it is enforced by **convention, not
  by the compiler**: `PickedTool` (`tools/mod.rs:119-131`) is `pub` with five `pub` fields and no
  `#[non_exhaustive]`, while `terminal_ladder` (`:357`), `editor_ladder` (`:389`),
  `open_in_terminal` (`:427`) and `open_in_editor` (`:447`) are **all `pub` and all take
  `Option<&PickedTool>`**. Verified: `grep 'PickedTool' src-tauri/src/` returns **exactly one hit**,
  a return type (`commands/external.rs:140`), with **zero field reads**. Fix at the right layer
  (zero-caller): make the five fields `pub(crate)`, or add `#[non_exhaustive]`.
- **`src-tauri/capabilities/default.json` grants NO `fs:` permission** — only `core:default`,
  `dialog:allow-open`, `updater:default`, `process:default`. **Adding any `fs:` write permission
  scoped to the app config dir would defeat P112 entirely without touching a single line of Rust**,
  and it would not show up in any Rust review. Cheapest way to lose the property.
- **`.exe`-only is sufficient and not a heuristic.** A batch file **renamed** to `.exe` is handed to
  `CreateProcess`, which validates the **image header** and fails with **error 193** — it never
  reaches `cmd.exe`. The dialog filter is cosmetic (`custom.rs:262-267`); the gate is
  `custom.rs:268-275`, and `tests_tools_pick.rs:78-109` writes **real** `payload.cmd`/`.bat`/`.ps1`
  files and asserts all three are refused.
- **"No path is ever an argument" holds only for the two NEW commands.** `openInTerminal`,
  `openInEditor` and `revealInFileManager` carry `["path"]` in `rawArgPolicy.json`, so paths **do**
  reach raw-mode logs on the neighbouring external surface (pre-existing). **Pin the dependency this
  rests on:** the pipeline logs **arguments and never result values** — a future change that logged
  result values in raw mode would break the property **without touching either command or the policy
  file**.
- **INFO, on record:** `browsed_tool_row` validates the real `&Path` but stores `to_string_lossy()`.
  For a non-UTF-8 path the stored string differs from the validated one. It **fails closed**
  (re-validated on every launch; `is_file()` false, auto ladder runs), so there is no security
  consequence — but "validated one value, stored another" is a shape worth having on record.

### SEC-2026-09-03 — external-launch residue (remediated `0806596`)

Full narrative: archive Part 56. Report: `docs/audit-2026-09-03-external-launch.md` (`7e426c3`).

- **MEDIUM-2 + LOW-1 are `partially closed`** by `dc295c5`'s shape validation — the auditor showed it
  does **not** achieve the claimed property (three surviving routes, second-round ruling #21).
  **P112 is the real closure.**
- **INFO (CSP `form-action` / `base-uri` / `object-src`) — FIXED `8dd5b24`**, native half confirmed by
  the user 2026-09-10.
- **One residual, documented not closed:** a symlink introduced inside an already-checked-out
  superproject at a not-yet-created leaf bypasses the canonicalize recheck. Primary vectors are closed
  lexically regardless of filesystem state.
- **Test gap, partially closed — CONTRADICTION still open.** `c218258` added the file-target case and
  `151232d` covered UNC end to end; whether the set is now adequate is **unverified**.

### Residue of the archived `Queued housekeeping` section (archive Part 57)

The `src/styles/forge-pr.css` split is **DONE** (`e149382`, five modules, emitted stylesheet proven
byte-identical) and `.css` is now in `scripts/check-file-size.mjs` `SCAN_TARGETS`. Still open:

- Three items found during that split and deliberately NOT fixed (each would reorder the cascade or
  cross into another file): `context-menu.css:89` now points at a rule that lives in
  `forge-account.css`; a duplicate `.pr-create-actions` rule in `forge-pr-create.css`; and the
  generic `.btn-secondary-danger` sitting in `forge-pr-create.css` where it belongs with
  `controls.css`.
- **`image_diff_cli_2.rs`** numbered split still owed — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff. Path re-verified 2026-09-03:
  `crates/bonsai-core/tests/diff/image_diff_cli_2.rs`.
- **The 90-char branch-name chip** becomes a 50 px two-line stadium at `border-radius: 999px` —
  pre-existing, newly visible because P102/P105 added the fixture that reaches it.
- **`ui-reference.md` is growing fast** (§2 now carries a 16-row evidence table) — worth its own
  curation pass.
- **Two velocity items filed and deliberately NOT taken** (2026-09-03): C1 could drop 17s → 11s by
  giving one surface its own test and its own corrupted repo — **not taken**, it changes the shape of
  a crash-safety test for ~6s; and `crates/bonsai-mcp/tests/common/mod.rs:131-133` still spawns 3
  `git config` calls (board said `:127`; re-measured 2026-09-03 — same fix applies verbatim; left
  alone to keep the blast radius in one crate).
- **The gate script emits Node `DEP0190`** — it passed args to a child with `shell: true`, which
  concatenates rather than escapes. **CONTRADICTION flagged 2026-09-10:** `833f2f9`'s commit message
  is "the gate stops concatenating argv", so this looks already closed. Not closed by the curator.

### P110 / P111 residue (milestones done; archive Parts 55 and 59.1)

- **P110 — latent pre-existing gap, NOT introduced by P110 (candidate follow-up):** op-state files
  (`MERGE_HEAD`, `REBASE_HEAD`, `rebase-merge/**`, `CHERRY_PICK_HEAD`) are excluded by the watcher
  filter and never triggered a refresh before or after P110. Op-state freshness during a conflicted
  rebase rides on incidental worktree churn. Deliberately left alone — changing it would widen which
  bursts fire.
- **P110 — deliberately NOT done:** the canvas selected-row highlight is not sticky. The highlight is
  row-index-based and there is no honest row to draw while the row is absent from the partial layout;
  anchoring it to a stale index is exactly the wrong-commit hazard the fix removes.
- **P111 — filed not fixed:** `.asset-chip` got the R1 line guard but no R2 `max-width`, so a model
  id far longer than the fixture would widen the chip rather than ellipsize.

### From the 2026-09-02 file-size refactor pass (archive Part 36; pre-condensation text 69.2)

Ratchet baseline moved **27 offenders / 6241 excess → 20 / 3528**; full gate green 8/8, 603s.

- **Fold-pill cursor is dead** in `GraphCanvas.handleMouseMove` — `src/graph/GraphCanvas.tsx:535`
  writes `foldCursorFor(...)` and `:551` unconditionally overwrites it with
  `next?.kind === 'overflow' ? 'pointer' : ''`, no guard between. Real regression; no vitest mounts
  `GraphCanvas`, so e2e is the only net. Re-verified 2026-09-03.
- **Shortcuts stay live during confirm dialogs**, and the old candidate-fix citation (`1d9d9bf`) was
  wrong. `anyDialogArmed` (`repoWorkspace/useWorkspaceDialogState.ts:265-300`) enumerates ~35 flags
  but **not** `pendingForcePush` (`:208`), `pendingCommitPush` (`:205`) or `abortConfirmOpen`
  (`:178`); `pendingBisectBad` lives outside the hook at `RepoWorkspace.tsx:299`. **Four flags.**
- **Contract divergences the tests document as bugs-in-the-contract:** rebase §3.1.5/§9.7
  unstaged-changes precondition; the libgit2-vs-CLI rename/delete conflict index-entry count; a
  near-tautological `expected_presence` oracle.
- **Duplicated helpers left visible, not merged** (behavior risk, not a move): atomic-write helpers in
  `assets/bundle/write.rs` + `assets/profiles/store.rs`; test helper families in `tests/diff/` and the
  four `tests/rebase_merge/*_support.rs`.
- **Three files deliberately stopped short of 500** (each further cut forwards 15-100 values to one
  consumer): `RepoWorkspace.tsx` **2264**, `src/graph/GraphCanvas.tsx` **784**, `src/App.tsx` **590**.

### Velocity follow-ups from the 2026-09-01 pass (`docs/history/velocity-2026-09-01.md`)

- **`submodule_cli::oracle_add_deinit_remove_roundtrip` 12-14s** — not a proptest (a git-CLI oracle
  roundtrip), so banding does not apply; needs its own look if the ~12s floor matters.
- **The vitest-environment item is DONE** (happy-dom, `1953c0a`). **Still owed from the same ruling
  #8: make `src/ipc/mock/repoState.ts:160` lazy** — it calls `new URLSearchParams(
  window.location.search)` at **module init**, the single root cause keeping 19 otherwise-DOM-free
  files in the DOM project. ~3 lines, worth ~70 s of CPU. **Whether `1953c0a` included it is
  unverified.**
- **`pnpm gate --quick` is 305s and only drops e2e** — not a fast tier; `cargo nextest --workspace`
  alone is 181s of it. Add a genuinely narrow tier or lean on `--rust` / `--frontend`.

### Hoisted off milestones archived 2026-09-01

- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature, drops `crypto-rust`, requires explicit credential-store registration — real
  changes to `crates/bonsai-forge/src/auth.rs`. (DEP REFRESH, archive Part 24.)
- ~~`no_proxy_client()` `.expect(...)`~~ — **CLOSED 2026-09-03:** it survives at
  `src-tauri/src/mcp/http_support.rs:221`, but the module is `#[cfg(test)]` (`mcp.rs:410-411`), so it
  never reaches a shipped binary. Detail: archive Part 52.5.
- **RepoWorkspace refactor** still stands for maintainability (not perf); P88's audit re-confirmed it.
- **P90.1 deferred:** per-check timing fields; header commit-summary text; command-palette
  `Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.
- **Known flake, untouched:** `watcher::tests::git_internals_filtered` (`src-tauri/src/watcher/
  tests.rs:127`); the flaky assertion at `:144` is `rx.recv_timeout(1500ms).unwrap_err() ==
  RecvTimeoutError::Timeout` — an `unwrap_err` on a **channel-recv Result**, not "on an `Instant`".
  The test now defends that negative window as sound after `watch_into_channel`'s sentinel sync
  (`29e72a7`), so whether it still flakes is **undetermined** (not re-run since 2026-09-03).
- **FLAG FOR USER (peer session, ended):** `repoWorkspace/useWorkspaceKeyboard.test.tsx` failed in
  ISOLATION on the committed baseline (1 graph-nav `defaultPrevented` case), introduced by the peer's
  graph-a11y commit `590f2ef`. Likely test-isolation flakiness. **Unverified since 2026-08-23.**

### Known load-flakes and test-budget fragility (full narrative: archive Part 68)

- **The 5-second default test budget is the real fragility, and the reporting mechanism reframes
  every "timeout" here.** vitest 4's `withTimeout` checks wall clock **on completion**
  (`@vitest/runner` `chunk-artifact.js:2288-2294`): a test that passed every assertion is still
  rejected with "Test timed out in 5000ms" once `performance.now() - startTime` crosses the budget —
  **proved with a purely synchronous 6000 ms busy-wait**. So **"timed out" does NOT imply a pending
  async chain.** In the gate's own condition (rust tier → vitest), **tail inflation is 1.3-2.8×** and
  **three tests already cross 5000 ms**, green only on explicit `20_000`/`30_000` budgets. Most
  exposed default-budget tests: `App.test.tsx` "Arrow-key pane nudge" (2394 ms), `Sidebar.churn`
  (2110 ms).
- **happy-dom's causal role in the two failed gate runs was NOT established** (2 failing runs vs 1
  passing jsdom run; all 112 tests in the five affected files pass under **both** environments).
  Correlation over three gate runs, not a demonstrated cause — `8026622` corrected the board's own
  overclaim. Two **real** defects were found and fixed (`9422e8b`): a click on a present-but-
  **disabled** checkbox (`SettingsGitConfigSection.test.tsx:337`, the only component in the repo with
  that inert-but-visible design) and a 1 s poll on a **microtask-only** boundary
  (`Sidebar.test.tsx:184`). Three of the five were deliberately **not** changed and their timeouts
  deliberately **not** raised.
- **`h_ai` is genuinely flaky in PARALLEL — a real defect, not a timing artifact.** **0 of 57 fail
  with `--test-threads=1`** (57 passed, 130s); under default threading it fails or stalls, because
  **57 tests concurrently spawn the `claude_stub.cmd` harness on Windows** — presenting as **stalls**
  (the 5 `ai_stream_bulk_cli` tests, 18.6s serially) **or cross-talk** (e.g. `ai_explain` receiving
  another test's `createBranch` stub body). Pre-existing; not from the 2026-09-11 security work.
  **Follow-up: isolate the AI stub per test.** Until then run `h_ai` with `--test-threads=1`.
  **A flake you cannot see the summary for is indistinguishable from a regression.**
- **`rust-lld: failed to write output … permission denied` on a stale `.exe`** hit an `h_ai` link
  twice on 2026-09-11; deleting the file fixed it. A lock/AV artifact, likely caused by force-killing
  cargo mid-link (see the serialize-cargo rule).
- `ai::session_tests::watchdog_tests::watchdog_does_not_fire_while_awaiting_input` — failed once under
  load, passed on re-run. The clock seam it needed **landed in `734b310`**
  (`crates/bonsai-core/src/ai/clock.rs`; `session_watchdog_tests.rs:41/67/115` drive `TestClock`), so
  re-verify before treating this as live.
- `src/App.test.tsx > App shell > an Arrow-key pane nudge persists the POST-nudge width` — failed once
  at 2662ms (`setUiSettings` never called), then 4/4 isolated and 2644/2644 on a full re-run.

### Residue of the two dated 2026-08-22 design reviews (archive Part 35)

- **`graph-design-review-2026-08-22.md` M1 is SUPERSEDED — do not implement.** Facts corrected
  2026-09-03; the verdict is unchanged but the citation was wrong twice over. `role="grid"`,
  `aria-rowcount`, `role="row"` and `aria-rowindex` are forbidden by `ui-reference.md` §4.1 — now at
  `:860-864`, **not** `:250-252` (§4.1's header is `:848`; `:250-252` is unrelated text today).
  **`aria-activedescendant` is NOT forbidden** — `ui-reference.md:865` says it "is kept and is
  valid" (amended 2026-09-02, spec-004 merge), so the board was directing sessions to remove a
  shipped, sanctioned attribute.
- **M2/M3/M4/S2/S3/N1/N2 — resolution unverified.** Not checked by the 2026-09-01 sweep; do not
  assume they landed.
- **`review-2026-08-22-ui.md` NIT-1 — Sidebar ignores `panelDensity`** (re-confirmed still open
  2026-09-03: `src/styles/sidebar.css:92` is `.branch-row { height: 24px; }`, a hard literal, and no
  file under `src/components/` that reads `panelDensity` is a sidebar file).
- **NIT-2 —** `src/components/OnboardingOverlay.tsx:229` is still `aria-label="Close"` (re-verified
  2026-09-03, line number still exact); the review preferred "Close the tour".
- SHOULD-3 (`--accent` text over `--selection` fails AA) is the **same item** as P69 **A9** below —
  A9 is the canonical entry.

### P80 forge follow-ups (SHOULD-FIX/NIT, non-blocking)

- (a) `forge_set_token_inner` validates before the `host.is_empty()` guard — guard host first.
  Still open, re-verified 2026-09-03: `src-tauri/src/commands/forge.rs:290` calls
  `validate_repo_token`, `:291` is the `host.is_empty()` guard.
- (b) keychain-write-then-settings ordering: a failed `settings::update` leaves an orphaned keychain
  token (currently `let _ =`) — surface the error.
- (c) re-connecting a migrated legacy `login:None` host creates a 2nd three-part account + orphans the
  bare-host keychain entry (contract §1.2 rekey, optional).
- (e) `ContextMenu` has no separator concept, so the switcher's account/command rows run contiguous.
- (f) Settings Accounts group ordering is alphabetical only (no repoId in scope for "current host
  first").
- (g) disabled Default radio's `aria-describedby` points at a `hidden` span — use a visually-hidden
  class.
- (h) switcher trigger has no busy affordance during a pin/reset write. (i) §1.1 wireframe middot
  between host and caption omitted (cosmetic).

### `cargo fmt` has never been run on this repo

- No `rustfmt.toml` anywhere, no fmt check in any hook or CI (re-verified 2026-09-03: zero
  `rustfmt.toml` in the tree). **`gate.mjs` does not run it**, so it is not a gate step.
- **Two dated measurements, both kept because neither was re-measured against the other's tree:**
  `cargo fmt --all --check` reported **1773 hunks across 221 files** (original measurement;
  `--config use_small_heuristics=Max` was *worse*, 2065), and a 2026-09-14 measurement reported
  **2290 hunks** dirty at baseline. Treat each as of its own date.
- **Do not read `cargo fmt` output on a diff as a regression** — files nobody touched (e.g.
  `ai/bin_resolve.rs`) are dirty, so it will show pre-existing lines in any file you happen to open.
- Right shape: its own commit — pick a config, add `rustfmt.toml`, one-shot reformat, then add
  `cargo fmt --check` to the gate. **Do it between milestones, never inside one.**
- Unrelated fact from the same pass, kept because a brief got it wrong: **`h_ai` / `h_misc` are
  `bonsai-core` test targets, not `bonsai`** (`cargo test -p bonsai --test h_ai` errors).

### Audit #2 remainder (full audit `docs/audit-2026-08-18.md`; fix-batch mapping archive Part 16)

- **Sections 4.3-4.8 test gaps** — CommandPalette/NumberSlider pins, streaming-graph e2e, 08-stash
  conflicted-apply fixture, Linux case-sensitivity assertions, low-value untested units, and the
  missing journeys: updater / AI-PR-description / clone-init / worktrees.
- **Section 7's 13 NITs** — recorded in the audit, no action required.
- **Section 5.6** perf/visual ACs stay USER CHECKPOINT (the headless harness cannot observe
  rAF/compositing).

### P68 contract debt (P68 is done; its contracts are stale/oversized)

- `docs/contracts/P68e-ai-activity-dock.md` is **1123 lines** (re-measured 2026-09-03; the board and
  `docs/contracts/INDEX.md` both said 1064, 59 lines stale) and under-describes shipped code
  (P68g-1 added an untrusted-model-output attribution line, a fixed "Bonsai never asks for passwords
  or tokens" guard, and a two-id `aria-describedby`). Splice-ready replacements are in
  `docs/contracts/P68g-ui.md` §3.1-§3.5. **Needs: apply the splice, then split the file.**
- `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale (`session_drain_tests.rs`
  is now a child of `session::session_drain`). **Invariants D1-D16 remain canonical — do NOT "fix"
  them back.**
- **P68 security follow-ups 7-11 still OPEN**; rationale in `docs/contracts/P68-security-audit.md`
  (canonical): the novel-content gate (structural defeat for H1), proposals shown as a diff, bulk
  path-count cap + per-batch reads + batch count in the dialog, process-group kill off Windows (the
  pid-zeroing half landed in `67539fd`), and a symlink-safe `resolve_conflict_text` write.
### P69 Settings follow-ups — A3 ✅ SIGNED 2026-09-11; A8/A9 still backlog

> **A3:** the user handed the gate-note copy to `ui-designer` to finalise with the surrounding copy
> in view (ruling #10); whatever it signed ships — and it signed the **shipped** string. **A8/A9 were
> deliberately NOT put to the user** — they are backlog, not blocked on a decision.


- **A8 — bundle the two specced-but-unimplemented items into one increment** (both `ui-designer` and
  the orchestrator recommend bundling): (a) the help-text highlight fallback
  (`docs/contracts/archive/P69-settings-ui.md` §3.2.1) — the flagship query `graph` returns 5 hits and
  highlights **nothing**; and (b) the half-landed draft-hint feature (§13). The draft-hint CSS is
  genuinely dead but costs no visible layout today.
- **A9 — a scoped a11y sweep of `color: var(--accent)` on text over `--selection`** (measured
  3.51-3.74:1). Now **prohibited** in `ui-reference.md` §2 so new code cannot add to the backlog. The
  one deviation P69k shipped: the rail hit-count is `--text-1`; the exact declaration to flip is
  marked in `settings-shell.css`.
- **A3 — ✅ SIGNED 2026-09-11, no code change owed to the copy.** `ui-designer` withdrew its own
  preferred reword (`These take effect once AI features are on.`) after finding the shipped string
  is a **pattern, not a string**: `SettingsAiSection.tsx:110` is byte-identical modulo `this`/`these`
  and `SettingsDevCaptureSection.tsx:52` is a third instance. The shipped
  `Turn on "Enable AI features" above to change these.` stays, signed into `ui-reference.md` §12.12 +
  `P68g-ui.md`. The only owed work is clearing two stale "pending A3 sign-off" comments — see
  RESUME HERE, `in-progress`.

### P77 tag-sync deferred follow-ups (full detail: `docs/history/todo-archive-2026-08.md` Part 18)

- **Collapsed-rollup needs first expand — ✅ RULED 2026-09-11: fold the check into the existing
  auto-fetch cycle** (enabled, 5-min interval). No repo-open network call and no new trigger; the
  rollup warning appears within one cycle. The old text: the ls-remote check only fires on the first
  Tags expand per session, so the rollup warning cannot appear until the user expands Tags once.
- NITs: rollup aria-label lacks singular/plural ("1 tags") · `useTagSync` re-hits network on rapid
  collapse-then-expand while `unavailable` · confirm dialogs close optimistically so `busy` never
  paints · the tag-filter box gate counts local tags only · item-7 "Delete tag on origin" also shows
  on remote-only ghost rows (coherent).
- Backend NITs: `delete_remote_tag` doesn't `evict_fresh_on_auth_fail` · `validate_tag_name` is
  duplicated from `tags.rs` — promote to shared if a 3rd caller appears.

### macOS ad-hoc code signing — ⏸ PARKED 2026-09-11 (user): blocked-on-release, NOT open work

- `bundle.macOS.signingIdentity: "-"` is in `src-tauri/tauri.conf.json` but **has not shipped**: the
  last tag is `v1.5.0` (2026-08-26), which predates the fix. Verify the sealed ad-hoc signature on
  the next tagged release.
- Not fixed by ad-hoc at all: Gatekeeper "unidentified developer"; a new version re-prompts once for
  TCC (cdhash changes). Full fix = Developer ID + notarization; the `APPLE_*` env block in
  `.github/workflows/release.yml` is already scaffolded. Full detail: archive Part 34.


---

## Archive

**Start at `docs/history/README.md`** — it is the navigable index of every archived milestone and
part number. The table below is the short form.

| File | Covers |
|---|---|
| `docs/history/README.md` | **The archive index** — which file/part holds which milestone. |
| `docs/history/todo-archive-2026-09.md` | **Parts 71-75 (moved 2026-09-16; P112 itself STAYED — its USER CHECKPOINT is pending):** the P112 sub-inc 3/4 build + review transcript, incl. the `P112-ui.md` §17 rulings, the four bad citations, the coalescing lesson and the sub-inc-3 audit (71) · superseded gate states (`d0e6cf0`, `dcff54b`, the 427.4s confirming run, the `e9ed93d` Rust tier) and the completed 2026-09-14 queue — F6, P77, the e2e cold-timing measurement, the UNC clearance (72) · the P112 sub-inc 2 + P113 phase-1 review transcript (73) · **the two items CLOSED 2026-09-16 with their evidence** — "Open in editor" (fixed `fd93616`, with its `os error 193` measurement table) and the false General subtitle (resolved by the picker landing) — plus the `.cmd` launch-path audit (74) · superseded curator bookkeeping, the duplicated `cargo fmt` measurement and the pre-consolidation `cargo fmt` section (75). **Parts 62-70 (moved 2026-09-14, after the user ruled all 22 FOR-USER items on 2026-09-11):** the stale 2026-09-10 resume block + FU-1 residue (62) · the FOR-USER evidence blocks for items 0-6 (63) · the `IN FLIGHT` queue, the 2026-09-11 orchestrator closures, the unreviewed-MCP-merge warning (64) · **`SEC-2026-09-11`**, the MCP tool-contract audit, with its verified-CLEAN register (65) · **`SEC-2026-09-11b`**, the review of that implementation, with its verified-CLEAN register (66) · P108 `AC11`, closed by ruling #13 (67) · the happy-dom load-flake narrative (68) · the open follow-ups as they stood pre-condensation (69.1 P91 · 69.2 SEC-2026-09-03 through the 2026-09-01 hoisted items · 69.3 P69 Settings) · superseded curator bookkeeping (70). **Parts 54-61 (moved 2026-09-10):** the whole USER-CHECKPOINT block — P102+P105, P106, P107, P108, P91 (54) · P110 + P109 (55) · the 2026-09-03 closures + SEC-2026-09-03 remediation (56) · `Queued housekeeping` incl. the `e149382` CSS-split proof (57) · the superseded `c218258` and earlier gate states (58) · the 2026-09-10 session: P111, FU-1, six reviewer-follow-up closures (59) · the board's record of the confirmation (60) · superseded curator bookkeeping (61). **Parts 51-53 (2026-09-03):** the `5c2dcd2` + `c6cd7dd` gate states and the e2e-contention mis-diagnosis · the full narratives of everything closed 2026-09-03 · the durable-lessons stories and worked numbers. **Parts 36-50 (2026-09-03):** the file-size refactor pass · P102+P105, P106, P107, P108 and the P91 security arc + audit + build diary · superseded pre-ship filings · the 2026-09-03 velocity pass · P99, P100, P101, P98, P95, P96, P97 · built-bundle e2e + P103 + P104 · the DX/velocity stubs · the pre-condensation open-follow-up text. **Parts 33-35 (2026-09-01):** the P84 record gap · macOS ad-hoc signing · the two 2026-08-22 design reviews. **Parts 22-32 (2026-09-01):** P94 · P93+P92 · DEP REFRESH · P90+P89 · P88 · the P85-P87 batch · P82+P83 · divergence reconcile + Release 1.1.0 · the DX dev-loop text · the confirmed-checkpoints block · the 2026-08-21 resolved follow-ups. |
| `docs/history/todo-archive-2026-08.md` | Parts 1-9: P65 to P28 build detail, the Phase 1-4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail. Parts 10-16: the P62-P74 checkpoint waiver + P71-P74, the P69 Settings redesign, the Audit #2 fix batch. Parts 17-18: P70 and P77. Part 19: the follow-ups resolved 2026-08-21, verbatim. Part 20: P78/P79/P80. Part 21: P80b/P81/P82. |
| `docs/history/todo-archive.md` | P27 to P2, M0-M6 |
| `docs/history/milestones-mvp.md` | the M0-M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/history/velocity-2026-09-01.md` | gate wall-clock, test-suite hotspots, inner-loop rebuild cost, ceremony-vs-machine-time split (2026-09-01) |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into the current dated archive file only once **both** halves of its gate
have passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT
stays on this board. **An owed AI-gate item also keeps its entry here** — P91's `logs/*.jsonl` parse
is the live example (P108's `AC11`, the other one, was closed by user ruling on 2026-09-11).

### Why this board is ~1800 lines, not ~300 (curator note, 2026-09-16)

**2438 → 1811.** 942 lines left the board. **935 lines were extracted verbatim into archive Parts
71-75 across 20 ranges, and every range was diffed byte-identical against `git show HEAD:TODO.md`
before removal** (7 of those 935 duplicate a paragraph deliberately kept live here, so 928 of the 942
are archived). The remaining **14** are: 12 lines of the two durable constraints **relocated**
byte-identical into `## Durable lessons — the rules`, 1 structural blank, and 1 Archive-table row
this pass rewrote. Nothing was summarized away and **no status was
upgraded by the curator.**

**Why the verify-before-remove order is now mandatory, in writing.** The previous attempt
(`c5b3ea5`) cut 1950 lines from this file and wrote them nowhere; `67e2ce6` had to restore them
wholesale. **Truncation is not compaction.** Extract → verify byte-identical → *then* remove, and
leave a Part pointer where the text stood.

**Two items were closed this pass, both verified against the tree first:** "Open in editor is broken
on Windows" (fixed in `fd93616`; the measurement table is Part 74.4 and the batch-file security
consequence is live under `### The security record`) and the false-General-subtitle ruling (resolved
by implementation — `settingsCatalog.ts:42-43` reads true again now that sub-inc 4 restored the
picker). **Nothing else was closed**, and four things were explicitly refused: P112 (its USER
CHECKPOINT is pending), the nine-file second review pass (a `reviewer` is executing it), both
decisions owed by the user, and the four USER ACTIONS.

Residual composition, measured after this pass: header + conventions + navigation **69** · RESUME
HERE incl. the nine-file process failure **41** · the **P112 milestone entry, its checkpoint, the two
owed decisions and its ranked follow-ups 67** · the landed queue + four user actions + verification
state **73** · the three ruling blocks, kept **verbatim and authoritative** **207** · the ruling queue
**153** · durable-lesson rules **177** · accepted decisions **115** · **open follow-ups 845** ·
archive + this note **64**.

**What sets the floor is one number: 845.** The open-follow-up backlog is now larger than the whole
board was after the 2026-09-14 pass (988), because P112 and P113 filed ~40 new items — three security
audits' residue, two reviews' worth of "filed, not routed" findings, the measured-but-unfixed scan
budget, the standing warnings, and the contract deltas owed to `architect` and `ui-designer`. Every
one carries a fresh `file:line` citation, which is exactly what a cold resume needs most. **Working
that backlog down is the only thing that moves this number; curating cannot.** The other three
blocks — the ruling ledgers (207), the durable rules (177) and the accepted decisions (115) — are
marked must-survive-compaction and were not touched.

**The one structural option, unchanged from the last note and still not a curator's call:** give the
durable rules their own file under `docs/` and leave a pointer here. That trades ~177 board lines for
one more hop on the session's most load-bearing content. **User/orchestrator decision, not a curator
one.**
