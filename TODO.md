# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).
Avoid tauri "test" feature on this machine (STATUS_ENTRYPOINT_NOT_FOUND); use runtime-free
inner functions for command tests.
**USER MANDATE (2026-07-28, updated 2026-08-04 for cross-platform support): on Windows, never use
C: for temp/scratch/mock repos — C: is critically full. Use `D:\Temp\bonsai-scratch`; when running
cargo tests set TMP/TEMP to `D:\Temp` (tempfile honors them). On macOS/Linux, `scratch_dir()` now
falls back to the OS temp dir (`std::env::temp_dir()/bonsai-scratch`) automatically — no special
handling needed there. Include the Windows-specific guidance in every subagent prompt that runs
tests or creates repos only when running on a Windows machine.**


## Board conventions

Status vocabulary: `pending` · `in-progress` · `done` · `awaiting USER CHECKPOINT` · `deferred`
(deferred always carries a one-line reason). A milestone is `done` only when the AI gate **and** the
native USER CHECKPOINT have both passed — the orchestrator never self-declares the second half.

**History is archived, not deleted.** Anything no longer on this board is in
`docs/history/todo-archive-2026-08.md` (P65 → P28 + the Phase 1–4 banners moved 2026-08-18; Parts
5–9 — P69/P67/P68 build detail, the 2026-08-17 batch mapping + spike facts, resolved spun-out
items — moved 2026-08-19), `docs/history/todo-archive.md` (P27 → P2, M0–M6) and
`docs/history/milestones-mvp.md` (the M0–M6 AI-gate vs USER CHECKPOINT split). Contract files are
indexed in `docs/contracts/INDEX.md`.

## 🎯 P69 — 1.0.0 release readiness — ✅ **SHIPPED 2026-08-18** (tag `v1.0.0`)

**Current step:** none — `v1.0.0` tagged and pushed at `bd52483`; the release workflow
builds/publishes from the tag. Reviewer verdict was APPROVE WITH NITS (no MUST-FIX); its two
SHOULD-FIXes were landed (changelog `v0.3.1` compare link + note; CONTRIBUTING apt-list claim)
except the branch-protection check-name question, which needs repo-settings access — **FOR USER:
if branch protection requires the status check `Frontend — vitest + build`, the CI matrix renamed
it to `… (ubuntu-22.04)` / `… (macos-latest)` and the old name will never report again.**

**FOR USER — two open items I could not close:**
1. **Back up `.tauri/updater-prod.key`.** It is correctly gitignored and untracked, so it exists
   in exactly ONE place: this working copy. Losing it permanently breaks auto-update for every
   installed client. (The committed `tauri.conf.json` pubkey was verified to match it.)
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only (vite/vitest → postcss → nanoid), never
   in the shipped app, deliberately ignored in `pnpm-workspace.yaml`. **The moderate is
   unidentified** — `gh` is not installed here, and both project gates are green
   (`cargo deny --all-features check` = advisories/bans/licenses/sources all ok; `pnpm audit`
   shows only the one ignored high). Check the Dependabot page.

**User decisions taken 2026-08-18** (full text: archive Part 5): scope = macOS defects +
contributor docs only · ship 1.0.0 **unsigned** (per `docs/code-signing.md`; its "decision needed"
is ANSWERED for 1.0: defer) · tag WITHOUT the six outstanding native USER CHECKPOINTs
(P62–P65, P67, P68), forge/PR shipped **flagged beta**.

**v1.0.0 AI gate at `bd52483`:** cargo `--workspace --no-fail-fast` exit 0, 0 failed / 4 ignored
(perf gates) · clippy `-D warnings` exit 0 · vitest 1596 / 130 files · e2e 104 passed, 1 skipped
(the permanent `08-stash` one) · eslint 0 errors, 29 warnings (budget 40) · `lint:size` OK ·
tsc+vite clean. Increment detail (P69a–P69e) + a known Playwright-CI limitation
(the `msedge`→`win32` fix is never exercised in CI): archive Part 5.

### Still open after P69 (explicitly NOT in scope)
`cargo fmt` adoption (1773 hunks / 221 files, no `rustfmt.toml`), SECURITY.md +
CODE_OF_CONDUCT.md, issue/PR templates, refetch-storm coalescing (now a spun-out item below),
`CommandPalette` highlight, persisted-settings write path, `NumberSlider` mid-typing clamp, and
the six native USER CHECKPOINTs themselves.

## 🔍 Audit #2 (`docs/audit-2026-08-18.md`) + fix batch — all confirmed bugs & SHOULD-FIXes **fixed** 2026-08-18/19

Audit baseline at `3a0a153`: cargo 1727/1/4-ignored (the 1 = the §2.1 watcher flake) · vitest 1580
· e2e 104/1-skip · harness clean. Every finding above NIT is now closed:
- **§2.2** CommitPanel mid-stream crash (MUST-FIX) — `ffa80d0` (P69c) · **§3.1** worktree-copy
  symlink write-through — `55acb98` (P69c). Both shipped in 1.0.0.
- **§3.8** streamAssembler throw containment · **§3.9** BulkAiConfirmDialog in `dialogOpen` ·
  **§3.10** AI runs cancelled on workspace unmount — `84cedb7`.
- **§3.2** F-T5-4 corrupt-object hang — `7edd23e`: `run_with_git_timeout` (`git/timeout.rs`,
  30 s inactivity deadline, `BONSAI_GIT_TIMEOUT_MS` override) wraps `get_status`/`get_graph`/
  `stream_graph`/history-index build; C1 now pins Err-not-Hung for read surfaces;
  `create_commit` deliberately UNWRAPPED (a false timeout on a mutation could race a late
  commit — rationale in `corrupt_repo_cli.rs` C1). · **§3.3** hook spawn failures →
  `HookRunInfo::warning`/`hookWarning`; indexer skips → `skippedCommits` + toast — same commit.
- **§3.4** dedupe canonicalize moved off the repos lock · **§3.5** registration filter now skips
  `tests_*` · **§3.6** forge HTTP `redirect(Policy::none())` + bounded body read · **§3.7** AI
  pid zeroed in `reap()`/`complete()` — `67539fd`.
- **§2.1** watcher-test sentinel-file positive sync (5× green solo AND in full workspace) + CI
  `cargo test --no-fail-fast` — `29e72a7`.
- **§4.1** `e2e/11-forge.spec.ts` written (9 tests, +`?forge=unsupported` seam) · **§4.2**
  `usePartialStaging.test.tsx` (24 tests) — `83a9b2f`.

**Gate at fix-batch HEAD `83a9b2f`:** cargo workspace 1754/0/4-ignored · clippy `-D` clean ·
vitest 1629 / 134 files · e2e 114 passed / 1 skipped · lint:ci 0 errors · lint:size OK.

**Still open from the audit:** §4.3–§4.8 test gaps (CommandPalette/NumberSlider pins once fixed,
streaming-graph e2e, 08-stash conflicted-apply fixture, Linux case-sensitivity assertions,
low-value untested units, missing journeys: updater/AI-PR-description/clone-init/worktrees etc.) ·
§7's 13 NITs (recorded in the audit, no action required) · §5.6 perf/visual ACs stay USER
CHECKPOINT (headless harness cannot observe rAF/compositing).

## ✅ Confirmed checkpoints and accepted decisions (condensed — full text in the archive)

- **All native USER CHECKPOINTs for P2 → P61 are CONFIRMED.** Batches: 2026-07-30 (P4, P3a–P3f, P7,
  P7e, P7f, P8, P9), 2026-08-03 (P18–P23, P24, P25, P26, P27), **2026-08-08** ("mark everything as
  checked" — P28 through P61 inclusive: P32, P37–P46, the credential-cache and UX-fix batches,
  Phase 1 P49–P52, Phase 2 P53–P57, Phase 3 P58–P61). P5/P6 were confirmed earlier still.
- **The entire approved roadmap P49–P65 is code-complete** (2026-08-10). P49–P61 are fully done;
  P62–P65 are still `awaiting USER CHECKPOINT` (below).
- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated preview) · P57 retriever = BM25
  lexical, no embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, autonomous, accepted):** new Rust deps `reqwest{blocking,json,
  rustls-tls}` + `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order
  GitLab → Bitbucket → Azure DevOps.
- Full text of every banner and decision: `docs/history/todo-archive-2026-08.md` Part 1.

## 🚀 PHASE 4 — forge/PR + paged loading (P62–P65) — code-complete, **awaiting USER CHECKPOINT**

Build detail for all four milestones (per-increment bullets, contract amendments, gate numbers) was
moved verbatim to `docs/history/todo-archive-2026-08.md` **Part 2**. Contracts:
`docs/contracts/phase4-forge-overview.md` + `P62`/`P63`/`P64`/`P65-*.md`. Command count reached
**157** here (P62 +7, P63 +1, P64 +1, P65 +1). Shipped in 1.0.0 **flagged beta** (forge/PR).

- **P62 — forge foundation (GitHub first)** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P62-user-checklist.md`). AI gate green: new `crates/bonsai-forge/` (60/0) +
  7 commands (cmd 154) + right-pane PR panel; harness verified connect→list→detail→create.
  Native half = a real GitHub PAT against real PRs.
- **P63 — forge signals on the graph** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P63-user-checklist.md`). AI gate green: batch `forge_commit_statuses` (cmd 155),
  `forgeBadges.ts` + `useForgeSignals`, PR/CI badges on branch-tip pills, Settings toggles.
  Native half = the canvas badge pixels and canvas click→PR.
- **P64 — GitLab + Bitbucket + Azure DevOps + AI PR descriptions** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P64-user-checklist.md`). AI gate green: `ai_generate_pr_description` (cmd 156),
  three more providers on the same trait (`bonsai-forge` 153/0), per-provider connect hints.
  Native half = live AI generation + real tokens per provider; two reviewer NITs are parked in that
  checklist §D (Azure bad-PAT HTTP-203 message; `dev.azure.com/{org}/_git/{repo}` shorthand).
- **P65 — paged/streaming graph loading** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P65-user-checklist.md`). AI gate green: shared `LaneWalker` + `stream_graph`
  channel (cmd **157**), `compute_graph` output byte-for-byte unchanged, frontend stream assembler,
  120k-commit correctness test. **Shipped with an honest first-paint reframe** — see the finding
  below. Native half = scroll feel + progressive load on a real large repo.
  - **FINDING (kept on the live board because it constrains future work):** libgit2's
    `Sort::TOPOLOGICAL` runs an eager `prepare_walk` before yielding row 0, so first paint is
    **O(total commits)** (release/warm: 40k ≈ 0.73 s, 120k ≈ 1.37 s, 200k ≈ 2.3 s), **not**
    O(first 512). Streaming still gives lane-stable progressive render, scroll-ahead and no giant
    IPC, but "instant first paint on 1M repos" is **not** met by the topo walk. `stream_perf.rs`
    deliberately does not assert a `<150 ms` target.

- **P66 — lazy generation-number topo order** — **deferred** (user decision 2026-08-10: the proper
  fix for the P65c finding is a Large build; not taking it on now). Feasibility spike:
  `docs/contracts/P65a-lazy-topo-spike.md` (VERDICT: TRACTABLE via path (c), effort **L**).
  Shape: reimplement git's lazy `--topo-order` (Stolee generation numbers) in Rust as a shared order
  stage replacing `seeded_revwalk`, sourcing generation numbers from the P52 commit-graph file via
  `gix-commitgraph` or an own parser (git2 0.21 / libgit2-sys 0.18 expose none — grepped, 0 hits).
  Committed P65a/P65b stay as-is (IPC / `GraphChunk` / `LaneWalker` unchanged); only the internal
  walk order changes. Costs: regenerate ALL graph fixtures (lazy order differs from libgit2 in
  commit-date **tie-breaks** only), guarded by a `get_graph ≡ stream_graph` equivalence test + a
  differential test vs `git rev-list --topo-order`. Pre-build: re-verify newest git2 still lacks the
  generation API (F5); confirm `gix-commitgraph` reads P52 split/chain graphs (F1). Architect advises
  **against** shelling out to `git log --topo-order` (would make the git binary a hard runtime dep of
  the core read path). Sets the deferred `stream_perf.rs` first-batch threshold.

## 🐛 USER-REPORTED BATCH (2026-08-17) — P67 UX polish + P68 AI conflict resolution

7 issues from real use (real repo, live merge conflict); user-approved plan:
`~/.claude/plans/1-the-dotted-line-cozy-llama.md`. Item→milestone mapping + `claude` CLI spike
facts: archive Part 6.

**LOCKED USER DECISIONS (asked + answered 2026-08-17; do not re-litigate):**
- AI timeout = **no hard timeout + Cancel button**; idle-output watchdog ~300s; optional hard cap
  configurable, default 0 (unbounded).
- AI visibility = **live log stream + interactive prompts** (user can answer Claude mid-run).
- Log panel location = **bottom dock**, collapsible, full width.
- Conflict-resolve repo access = **read-only allowlist** `--tools "Read,Grep,Glob"` (no
  write/edit/bash; Bonsai still writes nothing — staging stays an explicit post-review call).
- Bulk resolve = **ONE AI run for all conflicts**, with per-file attribution.
- Right-panel density = **tighter default AND** a Cozy/Compact toggle.
- "Stash all" = **demote to a `⋯` overflow menu** (keeps all 3 scopes; sidebar keeps 1-click stash).
- Panel density is **independent** of graph Compact rows (cross-reference hint in Settings only).
- Dashed HEAD line = **always visible while scrolling**.

### P67 — HEAD guideline + right-panel density — ✅ **AI GATE PASSED** (awaiting native USER CHECKPOINT)
Contract: `docs/contracts/P67-ux-polish-batch.md` (+ `P67-user-checklist.md`). **+0 Tauri commands.**

**Current step:** ✅ **P67 CODE-COMPLETE — AI gate PASSED, awaiting native USER CHECKPOINT**
(`docs/contracts/P67-user-checklist.md`). All five sub-increments committed and reviewer-approved:
**P67a** `0ec69f9` · **P67b** `e607d2c` · **P67c** `5e68db5` · **P67e** `d50361a` · **P67d** `f3ca6e5`.
Nothing here is user-confirmed: **the dashed guideline has never been seen by anyone** (headless
harness — no compositing, `rAF` paused, no canvas pixel is ever produced; geometry proven
arithmetically + via the `window.__bonsai.p7` seam). Line visibility while scrolling, dash crawl,
halo termination, marker direction, and compact readability are all native-only. Build detail,
measured reclaim (+115.5 px ≈ 4.8 rows cozy, +30 px compact), contract amendments A5/A6.1–A6.3
(binding, canonical in the contract), the Chromium-only `field-sizing: content` guard, and the
harness gate results: archive **Part 7**.

### P68 — streaming/interactive/bulk AI conflict resolution — ✅ **AI GATE PASSED** (awaiting native USER CHECKPOINT)

**Current step:** ✅ **P68 CODE-COMPLETE — awaiting native USER CHECKPOINT**
(`docs/contracts/P68-user-checklist.md`; the four runs that matter are listed at the end of it).
Every sub-increment committed and reviewer-approved (commit list: archive **Part 8**). **Nothing
about appearance or real-CLI behaviour has been verified by anyone** — no live log, no real tool
use, no real cost and no pixel has been seen.

**Gate history:** P68-final AI gate measured at `44067af` (2026-08-18): tsc 0 · vitest 1580/128 ·
e2e 104/1-skip · cargo core 764/0/1 + bonsai 238/0 · clippy clean · IPC commands **160**.
Superseded by the v1.0.0 gate at `bd52483` and then the audit-#2 fix-batch gate at `83a9b2f`
(cargo 1754/0/4 · vitest 1629 · e2e 114/1-skip — see the audit section above).

Contracts: `docs/contracts/P68-ai-conflict-streaming.md` (invariants D1–D16; canonical for the
durable warnings — D16 stdin/reader ordering, the `RunOpts::default()` non-migration,
`StreamLogItem.assistant_text`; do NOT "fix" those back) · `P68e-ai-activity-dock.md` ·
`P68g-ui.md` · `P68-security-audit.md` · `P68-user-checklist.md`.
**Commands 157 → 160** (`ai_resolve_conflict_stream` = Channel, `ai_cancel_run`, `ai_reply_run`).

**ADDITIONAL LOCKED DECISIONS (asked + answered 2026-08-17):**
- **Spend cap = NONE by default**, configurable (`ai_max_budget_usd` default `0.0` = no cap).
  `--max-budget-usd` is only passed when > 0; live cost shows in the dock so a runaway is visible.
- **Bulk + `autoResolve` = STAGE the marker-free files** from the run; any file still carrying
  conflict markers falls back to review (`hasUnresolvedMarkers` gate). First place Bonsai stages
  several files from one AI call — nothing is committed, everything stays visible and revertible.

**Security follow-ups still OPEN** (audit items 7–11, each with its rationale in
`docs/contracts/P68-security-audit.md` — that file is canonical, do not duplicate them here): the
novel-content gate (the structural defeat for H1), proposals shown as a diff, bulk path-count cap +
per-batch reads + batch count in the dialog, process-group kill off Windows + zeroing `ctl.pid`
after reap (the *zeroing* half landed in `67539fd`; the process-group half is still open), and a
symlink-safe `resolve_conflict_text` write.

Orchestrator-settled OQs, sub-increment commits, durable warnings, accepted limitations:
archive **Part 8**. Original (drifted) board text: archive Part 4.

## 🐞 SPUN-OUT ITEMS — open follow-ups (resolved ones move to archive Part 9)

### CommandPalette highlight resets on `actions` array identity — **OPEN**
`src/components/CommandPalette.tsx:103→107→118` re-lands the highlight on the first enabled row whenever
the `actions` **array identity** changes, and `filterActions` always returns a fresh array. Any producer
whose memo deps churn therefore steals the user's keyboard selection mid-typing:
- **P65** bumps `graph` identity per streamed batch → the highlight jumps while a large graph streams
  (this is the real root cause of the `e2e/09-search-palette` flake, which was worked around by
  settling the stream first rather than fixed).
- **P68e** made it fire ~once a **second** during any live AI run (inline-arrow palette thunks +
  `focusDock` closing over `orderedRuns`). The P68e-side churn was fixed; the component was not.
**Correct fix (reviewer's recommendation):** reset on the filtered **ids**, not array identity — that
immunises every future `actions` producer instead of requiring each one to stay identity-stable forever.
Do NOT keep patching producers.

### Refetch storm: every mutation double-fetches per open tab — **OPEN** (audit #1 §3.10, 2026-08-07)
Every mutation runs `refreshAll` (~9 parallel fetches) and the watcher-debounced `repo-changed` for
the same writes re-runs the identical 9 ~300 ms later, per open tab (`RepoWorkspace.tsx`, was
:871-905/:1008-1046 at audit time). Fix = self-event suppression/coalescing; structural. Was tracked
only in `docs/audit-2026-08-07.md:163-166`; moved onto this board 2026-08-19 (audit #2 §5.3).

### Stash `expectedOid` UI wiring — **OPEN, now unblocked** (campaign deferral)
The Rust side is oid-verified (F-A6-B/F-A7-6) but the UI does not yet pass `expectedOid`. Was parked
on the `ipc/types.ts` freeze (`docs/testing-campaign-2026-08/SUMMARY.md`); the freeze lifted when
forge landed. Moved onto this board 2026-08-19 (audit #2 §5.4).

### Submodule dirty-deinit force flag (F-A7-7) — **OPEN, now unblocked** (campaign deferral)
Same parking condition, now lifted; details in `docs/testing-campaign-2026-08/FINDINGS.md` F-A7-7.
Moved onto this board 2026-08-19 (audit #2 §5.4).

### Persisted-settings write path has three latent defects — **OPEN** (all pre-existing, found 2026-08-18)
Found while extracting `useUiSettings`; none introduced by it (each verified against `git show HEAD`).
1. **Four writers bypass the debounced merge.** `handleSettingsChange` coalesces into one 300 ms
   `setUiSettings`, but `closeOnboarding` (`{onboardingSeen}`), `toggleTheme`, `toggleListView` and
   `commitPaneWidths` each fire their own independent write. Benign **only** because the key sets happen
   to be disjoint today — the ordering is unguaranteed, so any future overlap silently loses a field.
2. **A failed write is dropped and never retried.** `pendingSettingsPatchRef` is emptied *before* the
   `ipc.setUiSettings` call, so on rejection the merged patch is gone: the user gets a toast while the UI
   keeps showing values the disk does not have. Pinned as current behaviour by a test, not fixed.
3. **No unmount flush** for `settingsSaveTimerRef`. A pending patch still fires after unmount (so it is
   not lost in-session), but it is lost if the JS context dies inside the 300 ms window — app quit or
   window close right after a knob change — and a late write can outlive the unmount and race a read.

### `docs/contracts/P68e-ai-activity-dock.md` is 1064 lines and now under-describes shipped code — **OPEN**
Twice the ~500-line house limit. Also stale: P68g-1 (audit M3) added two elements to the ask block — an
untrusted-model-output attribution line and a fixed "Bonsai never asks for passwords or tokens" guard —
and made `aria-describedby` a two-id list, none of which §4.1/§4.2 describe. `ui-designer` produced
splice-ready replacement blocks in `docs/contracts/P68g-ui.md` §3.1–§3.5 rather than rewriting a
1064-line canonical contract wholesale with no line-level edit tool (correct call — a truncated write
would have destroyed it). **Needs: apply the splice, then split the file.**

### `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale — **OPEN**
Says `session_drain_tests.rs` is `#[path]`-included "as a child of `session`". After the `session.rs`
split it is a child of `session::session_drain` — still a descendant, so the privacy claim holds, but the
wording is out of date.

### `NumberSlider` clamps mid-typing, so a field's own minimum is hard to type — **OPEN**
`src/components/NumberSlider.tsx` commits on every `change` while the input is controlled, so with
`min = 60` a user typing `6` then `0` lands on **600**, not 60 (the field snaps to 60 after the first
keystroke, then the next digit appends). Verified. Pre-existing and shared by **every** settings slider,
so it is not P68-specific — but P68g's new limits fields (idle `min 60`, default 300) are where it is
most likely to bite. The USD field in `SettingsAiLimits` already dodges it with a local draft string,
because `Number('12.')` → `12` makes `12.50` otherwise unenterable. **Fix = move NumberSlider to
draft-string + commit-on-blur/Enter**, which changes commit semantics for every settings slider and so
needs its own increment with its own review — deliberately NOT bolted onto a security milestone.
Related and already fixed in P68g-2: clearing the field used to snap the setting to `min`
(`Number('') === 0`), contradicting the component's own doc comment.

### `STDERR_GRACE_TOTAL` is not the absolute cap its doc comment claims — **OPEN**
`drain_stderr` checks `Instant::now() < deadline` *before* each `recv_timeout(STDERR_GRACE)`, so the
drain can run up to `STDERR_GRACE_TOTAL + STDERR_GRACE` (~1150 ms vs the documented 1000 ms). Visible
only as ≤150 ms extra shutdown latency; the existing test's 500 ms slack passes either way.

### `cargo fmt` has never been run on this repo — **OPEN**
No `rustfmt.toml` anywhere, no fmt check in any hook or CI. `cargo fmt --all --check` reports **1773
hunks across 221 files**; `--config use_small_heuristics=Max` is *worse* (2065), so no single config
matches the existing hand style. Right shape: its own commit — pick a config, add `rustfmt.toml`,
one-shot reformat, then add `cargo fmt --check` to the gate. **Do it between milestones, never inside
one** (it would bury a review in a mechanical diff).

---

## Archive

| File | Covers |
|---|---|
| `docs/history/todo-archive-2026-08.md` | P65 → P28 build detail, the Phase 1–4 banners, resolved FOR-USER decisions (Parts 1–4, moved 2026-08-18); P69/P67/P68 build detail, the 2026-08-17 batch mapping + spike facts, resolved spun-out items (Parts 5–9, moved 2026-08-19) |
| `docs/history/todo-archive.md` | P27 → P2, M0–M6 |
| `docs/history/milestones-mvp.md` | the M0–M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into `todo-archive-2026-08.md` only once **both** halves of its gate have
passed. A milestone with a pending USER CHECKPOINT stays on this board. Archiving is a **move**, never
a delete — condense on the board, keep the full text in the archive, and leave a pointer.
