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

**History is archived, not deleted.**

---

## ✅ P89 — PR files & local diff view — DONE (AI gate + USER CHECKPOINT both green 2026-08-25)

**Goal:** Show a PR's changed-files list and per-file diffs directly in Bonsai, with correct
+/−/changed-files counts. Counts + diffs are computed **locally** from the PR's base and head
commits (reuse `bonsai-core` diff engine `collect_headers` / `get_commit_diff`), not from the forge
API (which returns `+0/−0` on several forges/endpoints). Auto-fetch the PR's base+head refs on open
so fork PRs and un-fetched branches still work. Forge-agnostic (Azure DevOps, GitHub, GitLab, all).

**User decisions (2026-08-25):** (1) Auto-fetch PR refs on open, diff `merge-base(base,head)..head`;
(2) Full scope — correct counts + changed-files list + click-to-view per-file diff reusing the
existing diff viewer; (3) Forge-agnostic.

**Acceptance criteria:**
- Opening a PR shows correct `+X / −Y / N files` computed locally (matches `git diff` base...head).
- Changed-files list rendered in the PR panel; selecting a file shows its diff in the existing viewer.
- Works when head is a fork branch / not yet fetched (auto-fetch of PR refs).
- Graceful states: fetch-in-progress, fetch-failed/offline, base or head unresolved.
- Forge-agnostic: each forge exposes base+head ref info; local diff path is shared.
- No Rust/React boundary violation; heavy git2 work in spawn_blocking; files under ~500 lines.

**Current step:** contract DONE (`docs/contracts/P89-pr-local-diff.md`). OQs accepted w/ architect
recs (no backend TTL guard; Azure head via lastMergeSourceCommit; defer PR-ref cleanup to Polish).
IPC: `forge_pr_diff` (auto-fetch base+head, local base…head diff → PrDiffStats) + `forge_pr_file_diff`
(pure-local per-file hunks). UI contract DONE (`P89-ui.md`: inline accordion in PR panel, reuse DiffView/DiffCard, no new
tokens; components `prPanel/PrChangesSection.tsx`+`PrFileRow.tsx`+`usePrFileDiffs`). **P89a backend
DONE** (working tree, gate-clean): `pr_diff.rs` engine (fetch base+head, merge_base→three-dot
tree diff, per-file hunks), `PrRefs`+`pr_refs` trait, **GitHub+GitLab impl; Azure+Bitbucket stubbed
→ P89a2**. Cmds `forge_pr_diff`/`forge_pr_file_diff` registered. cargo check + clippy -D clean, 5 tests.
**Azure matters (user uses Azure DevOps PRs) → P89a2 prioritized.**
**P89a reviewer APPROVED** (no MUST/SHOULD-FIX; 3 cosmetic NITs → fold into cleanup) + committed
`1e0dfff` on branch `feat/pr-local-diff` (off main). **P89b DONE + committed `23285d7`** (PrDiffStats TS type, invoke wrappers, mock/fixtures; tsc clean).
Flag: `github/dto.rs` now 532 lines (>500 soft limit) from P89a → split folded into P89a2.
**P89a2 DONE** (working tree, gate-green): Azure + Bitbucket `pr_refs` in new `azure/refs.rs` +
`bitbucket/refs.rs`; all 4 forges now implemented; 11 ref tests pass, clippy -D clean. **P89c DONE** (working tree): prPanel/{PrChangesSection,PrFileRow}.tsx + usePrDiff/usePrFileDiffs
hooks; PrDetailView thin composer w/ local-count header + forge fallback; all states + a11y; tsc
clean, PrDetailView.test 3/3. **Harness verified** (mock ?forge=auth): Changed-files section renders
(+12/5 files), expanding src/pr/view.rs shows its diff hunk inline via DiffView. Screenshot blocked
(headless pane) → pixel proof = USER CHECKPOINT. **dto.rs split DONE** (refactorer, working tree):
github/dto.rs 532→386 + new dto_tests.rs 148; 211 tests identical, clippy clean.
**Reviewer APPROVED** (no MUST-FIX). SHOULD-FIX follow-ups: (SF1) empty-state header should show
local 0-counts not forge fallback (`PrDetailContainer.tsx:70`); (SF2) stale head-advance refetch
should keep prior rows dimmed (`.diff-stale`) not collapse to skeleton (`PrChangesSection.tsx:106`);
NIT: Azure old-TFS fork fallback when `lastMergeSourceCommit` absent (`azure/refs.rs:76`, acceptable).
**ui-designer PASS** (no changes requested; SF1/SF2 stay follow-ups). **All P89 committed** on
`feat/pr-local-diff`: P89a `1e0dfff`, P89b `23285d7`, P89a2+split `71171d4`, P89c `a0f0575`.
**Tester GREEN** (5 pr_diff tests incl. git-CLI three-dot ground truth; vitest 3). **Full `pnpm gate`
GREEN** after ratchet fix `48ae17f` (split pr_diff.rs tests → pr_diff_tests.rs; engine 244/tests 282):
nextest+doctests+clippy 0-warn, eslint, file-size OK, vitest, tsc+build all pass. 2 e2e fails
(`16-history-undo-health`, `17-ai-dock ?aiFlood`) are **pre-existing flakes** — pass 16/16 isolated;
P89 touches no e2e/AI-dock/health code. Commits on `feat/pr-local-diff` (off main): `1e0dfff` `23285d7`
`71171d4` `a0f0575` `a988388` `48ae17f`. **NOT merged/pushed.**
**USER CHECKPOINT VERIFIED (2026-08-25):** user confirmed everything OK on the native app (Azure +
GitHub PR changed-files list + correct three-dot counts + expand-to-diff; fork-head auto-fetch;
offline/Retry). Branch `feat/pr-local-diff` still **NOT merged/pushed** — awaiting merge decision.
Open follow-ups (non-blocking): SF1 empty-state header local 0-counts (`PrDetailContainer.tsx:70`),
SF2 stale-refetch keep dimmed rows (`PrChangesSection.tsx:106`), NIT Azure old-TFS fork fallback
(`azure/refs.rs:76`).

---

## ⚡ P88 — Git-action perf round 2 (refresh-scope cluster + repo-handle cache) — in-progress

**Current step:** **P88a DONE + committed `2412d8b`** (reviewer APPROVED, no MUST-FIX; tsc clean, eslint 0-err,
110 vitest pass). 2 SHOULD-FIX = tester gaps for batch-end pass: add `refreshAll('stash')` assertion to stash-pop
test (`useStashActions.test.tsx` ~:107) + `refreshAll('worktree')` assertion to `stageResolvedText` test
(`useMergeActions.test.tsx` ~:104, incl. the `deferRefresh:true`→not-called branch). **B2a DONE** (reviewer
APPROVED; 6 `*_with` twins + new `worktree_reuse.rs`; 4 composites now open once — `checkout_branch_autostash`,
`create_branch_here`, `checkout_commit_detached`, `checkout_remote`; bare-repo guard restored → byte-identical;
1486 core tests unchanged, clippy -D clean both crates). AC-b2 counter not observable in bonsai-core (repo_opens
instrumented only at src-tauri seam) → B2b makes the round-level drop measurable; B2a proven by inspection (one
`open_repo_at`/composite). **B2b DONE + committed `52f5d74`** (reviewer APPROVED; config-staleness AUTHORITATIVELY
CLEARED via libgit2 1.9.6 source — config auto-refreshes on read). New `src-tauri/repo_handle.rs` (`with_repo`/
`with_repo_mut`, generation-keyed eviction on open/close); `read_status_with` forces `index.read(true)`. bonsai-core
1486 unchanged (byte-identical), bonsai 289 (+2). **Honest scope:** list trio (branches/stashes/worktrees) get
cross-round reuse (0 opens warm); `stream_graph` fuses seed+walk+reprobe to 1 open/call; **`get_status`+`stream_graph`
do NOT get cross-round reuse** — they run inside `run_with_git_timeout` (fresh watchdog thread/call) so open once per
call (no regression: status was always 1/call, graph improved 3→1). **PB-1 DONE + committed `cc5fdac`** (50k-node
store cap; byte-identical emit; bonsai 293). **P88a test-gap fill committed `709c9cc`.**

**✅ P88 BATCH AI GATE GREEN (2026-08-24).** Full `pnpm gate` PASS: nextest 2019 passed (6 skipped), doctests clean,
clippy 0-warn workspace, eslint 38≤40, file-size ok, vitest 2288 passed, tsc+build clean, playwright e2e 159 passed
(1 skip; 1 flake = `e2e/07-rebase.spec.ts` timing, PASSES isolated 1.8m — rebase is outside the P88a matrix, not a
regression). Commits: contract `31bf4bd`, P88a `2412d8b`, B2a `f4c060c`, B2b `52f5d74`, PB-1 `cc5fdac`, P88a-tests
`709c9cc`. Branch `perf/git-action-round2` (off c0825a3/1.3.0), NOT merged/pushed. **PENDING: native USER CHECKPOINTs
+ merge decision.** User confirmed "go ahead with B2 after this" (2026-08-24). **Then user chose "Do FU-B2c now"
(2026-08-24) → FU-B2c IN PROGRESS. Architect design DONE (contract §FU-B2c): Option 1 = move owned `Repository`
in/out through new `run_with_git_timeout_owned` + `with_repo_timed`/`_mut_timed` wrappers (Send-only, one owner at a
time; leak-on-timeout self-heals; watchdog abandonment preserved). ODs resolved: accept leak no-cap, delegate the
existing `run_with_git_timeout_with` to the owned variant, `get_graph` out of scope. Repurpose the 2 now-false
once-per-call tests. **DONE + committed `97190de`** — reviewer APPROVED (watchdog abandonment byte-identical, move/leak
sound, one-owner-at-a-time; no MUST/SHOULD-FIX, 2 informational NITs). New `run_with_git_timeout_owned{,_with}` +
`with_repo_timed`/`_mut_timed`; `run_with_git_timeout_with` delegated to the owned variant (1 recv loop, byte-identical).
Warm status+graph round now opens **0** (was 2/call). bonsai-core 1489, bonsai 297; `repo_handle` tests split to
`repo_handle/tests.rs`. Rust gate (`pnpm gate --rust`) GREEN @ `97190de`: nextest + doctests + clippy -D all pass
(frontend untouched → prior full-gate green at `cc5fdac` still holds). **P88 batch (incl. FU-B2c) AI-gate COMPLETE.**
**User chose HOLD ON BRANCH (2026-08-24): no merge, no push — awaiting native USER CHECKPOINTs in `pnpm tauri dev`
before the merge decision.** Branch `perf/git-action-round2` tip = `059caf3` (10 commits off `c0825a3`/1.3.0).
Native checks to run: create/delete tag, stash push/pop/drop, commit, add/remove remote+submodule, delete local
branch, file-by-file conflict resolve — all snappy + consistent UI (no stale ahead/behind after commit); no
regression in checkout/fetch/pull/push/rebase/merge.

**NEW FOLLOW-UPS (this batch):**
- **FU-B2c (perf, MED — the remaining B2 win):** hoist `with_repo` OUTSIDE `run_with_git_timeout` for `get_status`+
  `stream_graph` so they reuse the pooled handle across rounds too. Non-trivial — the corrupt-object watchdog
  (`timeout.rs:114`) spawns a fresh detachable thread per call, so a shared `&mut Repository` can't cross safely; needs
  either a persistent watchdog worker with its own handle cache, or move-in/move-back ownership of the handle (leak one
  on timeout). Modest win (open overhead is a constant factor; the O(worktree)/O(commits) work is unaffected) — decide
  if worth the risk to the safety path. Recommend a dedicated increment, not inline.
- **Known flake (pre-existing, untouched):** `watcher::tests::git_internals_filtered` (`watcher.rs`) is a timing flake
  (`unwrap_err` on an `Instant`); passes on isolated re-run. Not caused by this batch.
- **P88a tester gaps (carry to batch tester):** stash-pop `refreshAll('stash')` assertion + `stageResolvedText`
  `refreshAll('worktree')` assertion (incl. `deferRefresh:true`→not-called).
--- earlier ---
contract DONE (`docs/contracts/P88-git-action-perf.md`, ~267 lines). 3 open decisions RESOLVED
by orchestrator (accept architect recs): **OD-P88-1** keep set-url raw `refetchRemotes()` (config-only, watcher
ignores → no echo); **OD-P88-2** B2 = **thread-local handle cache keyed `(repo_id, generation)`** (NOT a `Mutex`
— a mutex would serialize the round's ~11 concurrent `spawn_blocking` commands); **OD-P88-3** stage B2 as B2a
(thread `&Repository` through composite ops, zero freshness risk) → B2b (round handle cache + index `read(true)`
freshness guard). Next: branch off `main` @ `c0825a3`, delegate **P88a** to senior-dev. **No ui-designer** (no
visible UI change; same data, fewer/narrower refreshes). Peer session `bonsai-c9` = release, tree clean.

**Audit result (verified clean — do NOT "fix"):** `spawn_blocking` discipline fully clean (198 cmds/198 wraps, no
git2 on async thread); network off critical path (tag auto-sync fire-and-forget, 1 round-trip); `runRefreshRound`
is parallel (`Promise.all`, RepoWorkspace.tsx:1209); ref-only refresh skips O(worktree) status scan (P86a works);
render hot-paths memoized. **PB-2 resolved** (post-walk re-probe guards the TOCTOU) — retire it.

### P88a — Theme 1: frontend refresh-scope cluster (FIRST — quick, low-risk, extends P85/P86a)
A class of handlers never adopted the P85 echo-arming pattern: they refetch via raw `refetchX()` instead of
`refreshAll(scope)`, so they skip the coalescer AND don't arm echo-suppression → the op's own `.git` write triggers
an **unsuppressed `full` watcher round ~300 ms later** (the exact P85 double-refresh). Plus several over-broad scopes.
- **Under-armed (route through `refreshAll`):** tag create/delete/sync `useTagRemoteActions.ts:31` (HIGH); stash drop
  `useStashActions.ts:119` (MED-HIGH); commit-composer `useCommitComposer.ts:256` (MED, also leaves ahead/behind stale);
  remote add/remove/rename + set-url `useTagRemoteActions.ts:153/192` + submodule add/deinit/remove `useSubmoduleActions.ts:48` (MED-LOW).
- **Over-broad scope:** delete LOCAL branch `full`→`refsOnly` `useBranchActions.ts:112` (HIGH — inconsistent w/ deleteRemoteTracking:200);
  stash push/apply/pop `full`→new `stash` scope `useStashActions.ts:28/71/105` (MED); merge conflict-resolve `full`→`worktree`
  **per-file** `useMergeActions.ts:81/112` (MED).
- **Blocker:** 3 hooks type dep as `refreshAll: () => Promise<void>` (no scope) → can't narrow. Fix: widen to
  `(scope?: RefreshScope)`; add a `stash` scope (status+graph+stashes) to `refreshScope.ts`; route the ~6 bypass handlers.
- **Acceptance:** each listed action fires exactly ONE refresh round (verify via `window.__bonsaiRefreshRounds`), at the
  minimal-correct scope; no unsuppressed watcher echo; UI still consistent (no stale ahead/behind after commit).

### P88b — Theme 2: backend repo-handle cache (B2) + PB-1 memory cap (SECOND — bigger, careful)
- **B2 (HIGH, biggest structural win):** no `git2::Repository` cached in `RepoEntry` (`state.rs:58`); ~9–11 opens per
  full round; multi-step actions re-open ~5× in one op (dirty checkout `checkout.rs:98/118/128/132/150`). Fix
  (OD-P88-2): **thread-local** `Repository` cache keyed `(repo_id, generation)` + `&Repository`-taking `*_with` core
  variants (NOT a `Mutex` — preserves the round's parallel fan-out); measurable via `repo_opens` (`perf.rs:17` —
  "would drive toward 1"). B2a (composite single-open) first, then B2b (round cache). Naturally fixes the multi-step
  5× and the miss-path 3× open.
- **PB-1 (MED, bundle w/ B2):** `graph_cache.rs` retains the whole chunk stream up to STREAM_MAX_COMMITS=1M (~150–250
  MB/repo at cap, no eviction). Fix: skip the store above a row threshold (or intern strings).
- **Delete-branch cache miss (MED, defer/document):** `graph_cache.rs:139` requires old-tips ⊆ new-tips for a redecorate;
  tip removal always Misses → full re-walk even for a merged branch. Frontend `refsOnly` (P88a) is the cheap win; the
  backend reachability check is hard — accept a documented conservative miss for now.
- **Scheduler note (LOW, no action):** `healthRefresh` fires a full round per tick but is OFF by default.

**Follow-up verdicts (this audit):** B2 still-stands (highest value) · PB-1 still-stands · PB-2 RESOLVED (retire) ·
FU-1..4 still-open (non-perf, low) · RepoWorkspace refactor still-stands (maintainability, not perf).

## ✅ PERF + OBSERVABILITY BATCH (P85–P87) — AI GATE GREEN (2026-08-23) — USER CHECKPOINTS PENDING

**Full `pnpm gate` re-run CLEAN @ `b802482`: all 8 steps passed (GATE_EXIT=0)** — cargo test/clippy (both
crates), frontend tsc+build, vitest, eslint (38≤40), file-size ratchet, playwright e2e. Commits: P85 `fde91d7`,
P86a `e294500`, P86b `88ba86a`, P87a `3bc616a`, M1 `9eceafe`, P87b `298e45b`, P87c `c9a523c`, P87d `b802482`.
Browser-harness verified (mock): determinate fetch progress (12.5k→25k/50k objects, scaleX .25→.5), "Running
pre-push hook…" phase, `.toolbar-phase` = --text-2, hook-fail dialog + failed dock row. Security-audited (no
HIGH/CRITICAL; M1 fixed). **PENDING: native USER CHECKPOINTs** (real branch-create/fetch wall-time; live look of
phase readout + progress bar + log dock in `pnpm tauri dev`). Non-blocking follow-ups: PB-1/PB-2 (cache mem cap +
cold-walk fp), B2 (repo-handle cache), FU-1..4 (P87b: target row label, commitAmend row, row role/aria-expanded,
clickable dock bar), RepoWorkspace refactor, AiActivityPanel aria-label (NIT).

## 🔴 P87c — batch gate-fix (full `pnpm gate` after P87b) — DONE

**Current step:** full gate RED → senior-dev fixing the two real batch regressions. Full gate at commit
298e45b: cargo/tsc/build GREEN; **eslint FAIL** (41/40 warnings — unused `eslint-disable` in CommitBox.tsx:173,184
+ RepoWorkspace.tsx:854, my-modified files; new P87 files clean); **playwright e2e FAIL** — P87b's git-dock
`span.sr-only[role="status"]` live region collides with the peer's `RevealAnnouncer`, so `e2e/20-sidebar-reveal.spec.ts`'s
`announcer()` locator matches 2 elements (strict-mode violation, 3 tests). **vitest** 1 failed/2267 passed = the
PRE-EXISTING peer `useWorkspaceKeyboard.test.tsx > "nav is inert…"` (590f2ef; NOT this batch — separate decision).
Fix: (1) remove the unused eslint-disable directives → ≤40; (2) disambiguate the live regions (aria-label + scope the
e2e helper). Then re-run the full gate CLEAN (no harness on 1420).

**P87c DONE (senior-dev, verified): eslint 41→38 (exit 0); e2e 20-sidebar-reveal + 26-a11y-toasts 12 passed; tsc clean.**
CORRECTION: the e2e collision was PEER-vs-PEER (`RevealAnnouncer` + `GraphSelectionAnnouncer`, both span.sr-only[role=status]),
NOT this batch — GitActivityDock renders a `<p>`, never matched. So BOTH the e2e and vitest gate failures were pre-existing
in the peer's 590f2ef (committed pending USER CHECKPOINT, gate not green). Fix disambiguates via aria-label ("Reveal status",
"Graph selection", "Git activity") + name-scoped e2e locator. Files: RevealAnnouncer.tsx, GraphSelectionAnnouncer.tsx,
GitActivityDock.tsx, e2e/20-sidebar-reveal.spec.ts + eslint-directive removals in CommitBox.tsx/RepoWorkspace.tsx.

## 🔴 P87d — fix the pre-existing peer nav test (USER APPROVED 2026-08-23) — pending
`useWorkspaceKeyboard.test.tsx > "nav is inert (and not default-prevented) with no selection or no graph"` fails (peer's
590f2ef "first arrow seeds selection"). User chose "Fix it too". **Diagnosis: TEST-ONLY fix — the app is
CORRECT.** `useWorkspaceKeyboard.ts:320-338` implements the intended M2 feature: graph present + `selectedIndex===null`
→ first arrow SEEDS selection (preventDefault + setSelectedIndex). The old test's `{selectedIndex:null}` (graph-present)
case wrongly asserts inert; the `{graph:null}` case is still correctly inert. Fix the TEST (split: no-graph→inert;
graph+no-selection→seeds), do NOT touch useWorkspaceKeyboard.ts. Routed to tester.
**P87d DONE (tester):** split into two tests (no-graph→inert; graph+no-selection→seeds anchor: ArrowDown/Home→0,
ArrowUp/End→9); 40 passed, no app code touched. Committing, then full-gate re-run CLEAN.

---

## ⚡ P85 — Refresh perf: route ref-mutations through echo-suppressed refresh — pending

**Current step:** DONE (committed) — reviewer APPROVED (no MUST-FIX; A2 arm/disarm + A3 emit both
scrutinized sound). Targeted checks green (cargo check, clippy, tsc, eslint, 20/20 refresh vitest +
watcher test). Two SHOULD-FIX + 3 deviations carried into P86 (see P86 block). Full `pnpm gate` deferred
to the batch integration (after P86, since P86 re-touches these files). Contract
`docs/contracts/P85-refresh-perf.md`. **AI gate for the branch-create fix itself: GREEN; native
wall-time confirmation is a USER CHECKPOINT after the batch integrates.** Decisions: A1 routes the 7 bypass handlers through the EXISTING `refreshAll()` (scope param
deferred to P86 → P85 does NOT touch RepoWorkspace.tsx); A2 round-anchored echo suppression; A3 fetch
fire-and-forget `auto_sync_tags` + watcher ignores `refs/bonsai-tagsync/**` + keep `tag-auto-sync` event
(OD-P85-1=keep). Measurement: `window.__bonsaiRefreshRounds`. Shared working tree with peer session
bonsai-c9 — path-scoped commits only; peer owns graph visuals/a11y (disjoint files).

**Goal (workstream A of the 2026-08-22 perf investigation).** P81 (`be01422`) added refetch
coalescing + watcher self-echo suppression, but several handlers bypass it — they call raw
`refetchGraph()`/`refetchBranches()` instead of `refresh('mutation')`, so `armEcho` never fires and
the `.git/refs/**` write they cause triggers a **second, un-suppressed full `runRefreshRound`** ~300 ms
later via the watcher. A trivial `git branch` (one ref write) therefore pays for **two O(all-commits)
graph walks** contending in `spawn_blocking`, each bounded only by a 30 s timeout → the reported
**15–20 s branch create**. Same bug in fetch and push.

Bypass handlers to fix: `handleCreateBranch` `useBranchActions.ts:31`, `handleDeleteBranch` `:100`,
`handleRenameBranch` (non-head) `:132`, `handleDeleteRemoteTracking` `:164`, `handleFetch`
`useRemoteOps.ts:61`, `pushCurrentBranch` `:117`, `doForcePush` `:151`. Also: harden echo suppression
(the 600 ms `ECHO_TTL_MS` wall-clock window in `echoSuppression.ts` is fragile on large repos where a
round + 300 ms debounce exceeds it → even commit/checkout/pull can double-refresh); and take fetch's
awaited `auto_sync_tags` second network fetch (`remotes.rs:11` → `tag_auto_sync.rs`) off the critical
path.

**Acceptance:** each listed mutation triggers exactly ONE refresh round (verified via a refresh
counter / instrumentation); echo suppression is robust regardless of round duration; fetch returns
after a single network round-trip. Full graph/status behavior otherwise unchanged.

## ⚡ P86 — Refresh perf: graph-layout & repo-handle caching, scoped refresh — pending

**Current step:** COMPLETE (AI gate) — P86a `e294500` + P86b `88ba86a`. Follow-ups open: PB-1 (cache
memory cap), PB-2 (cold-walk store fingerprint), B2 (repo-handle cache). Native wall-time = USER
CHECKPOINT (batch). Original split into two serial increments: **P86a** (B3 scoped/reason-aware refresh + carry-ins CI-1..CI-4) —
senior-dev DONE + reviewer APPROVED (no MUST/SHOULD-FIX; 2 informational NITs), **committed e294500**;
**P86b** (B1 graph-layout cache) — DONE + reviewer APPROVED (no MUST-FIX; classifier soundness traced,
no false-hit path), **committed**. B2 (repo-handle cache) deferred as staged — clean follow-up needing
pervasive `&Repository` core-fn variants. Deviation: `get_graph` left uncached (cap 100k vs stream 1M;
off the hot path — frontend uses streamGraph).

**P86b review follow-ups (non-blocking, recommend before batch ships):**
- **PB-1 (memory cap — higher priority):** `graph_cache.rs` retains the full `Vec<GraphChunk>` up to
  `STREAM_MAX_COMMITS` (1M) per open repo with NO cap/eviction; `buf.push(chunk.clone())` doubles the
  transient alloc during each cold walk. Fine at the 20k target (~tens of MB) but unbounded on huge repos
  (doubles the frontend copy). Fix: skip the store above a node-count threshold.
- **PB-2 (cold-walk store race):** the store guard fingerprints the PRE-walk seed, not what the walk
  observed, leaving a residual net-zero-double-mutation TOCTOU (astronomically improbable, self-heals).
  Fix: derive the stored fingerprint from the walk's own observed seed (surface `stream_graph_core`'s
  internal seed / fingerprint the emitted tips+node_oids).
- NIT: `repo_opens` undercounts on Miss (post-walk probe not counted — matters only if B2's AC leans on it);
  cache mutex held across chunk emission on hits (harmless today via per-repo `graphReqId` serialization).
Deviations (contract matrix over prompt parenthetical): pull→`full` (moves HEAD), discard→`worktree` — both correct.
NIT carry (non-blocking): backend double-emits `repo-changed{tags}` + `tag-auto-sync` for one sync (coalesced, harmless).

**⚠ FLAG FOR USER (peer session, now ended):** `src/components/repoWorkspace/useWorkspaceKeyboard.test.tsx`
fails in ISOLATION on the committed baseline (1 graph-nav `defaultPrevented` case), introduced by the peer's
graph-a11y commit `590f2ef`. Passes in the combined run → likely test-isolation flakiness, but it's in the
peer's file (outside this batch's scope; I did not touch it). Needs a look before the branch merges.

**Goal (workstream B).** Even with P85, every refresh re-walks the ENTIRE commit graph from scratch
(`compute_graph` `graph.rs:129` / `stream_graph_core` `graph/stream.rs:110`), re-opens the
`git2::Repository` on every command (`state.rs:12` flags a handle cache as an unimplemented perf
lever — 11 opens per `runRefreshRound`), and re-scans the whole working tree (`read_status`
`git/status.rs:134`, `recurse_untracked_dirs(true)`) even for ref-only mutations. Add: a `GraphLayout`
cache reused when the tip set is topologically unchanged (branch create adds a tip at an existing
commit → identical topology, only a new ref pill); a repo-handle cache; scoped refresh rounds
(`runRefreshRound` `RepoWorkspace.tsx:1230` refetches 11 things regardless of what changed); skip the
status rescan on ref-only mutations. Also: background auto-fetch (`scheduler.rs:412`) fires the same
full round on a timer → periodic jank.

**Acceptance:** ref-only mutations reuse the cached layout (no full re-walk); repo handle reused across
a refresh round; refresh rounds fetch only what the change reason implies; measurable drop in
branch-create / fetch wall time on a large fixture.

**MUST-DO carry-ins from the P85 review (do these in P86):**
- **CI-1 (P85 SHOULD-FIX #1 — regression fix):** a genuine backend `repo-changed{reason:"tags"|"fetch"}`
  emitted by the async tag-sync is currently DROPPED by echo suppression (it lands inside the fetch's own
  armed window), so adopted/moved tags don't appear until the next refresh — a P84 regression. The
  reason-aware refresh must route backend-CONFIRMED genuine changes through a NON-suppressed origin (they
  are not the mutation's own fs echo). This is the crux of B3's reason taxonomy.
- **CI-2 (P85 SHOULD-FIX #2 — faithful mock):** `src/ipc/mock/handlers/remotesSync.ts` currently defers
  the tag emit ~1500 ms specifically to clear the echo window, which MASKS CI-1 in the harness. Emit at a
  realistic offset so the mock can actually expose this class of bug.
- **CI-3 (P85 deviation a):** wire the `onTagAutoSync` subscriber (+ tag-count toast if wanted) in
  `RepoWorkspace.tsx` (the per-repo subscription point P85 couldn't touch).
- **CI-4 (P85 deviation b):** remove the now-unused `refetchBranches`/`refetchGraph` args from the two
  hooks + their `RepoWorkspace.tsx` call-site literals (P85 left them as accepted-but-unused).
- NIT (P85): `disarmEcho` late `.finally` after `clearEchoSuppression` on unmount can re-insert one stale
  `disarmUntil` entry per repoId — bounded, self-heals; fix opportunistically.

## ⚡ P87 — Git & hook output observability: live progress + session log — pending

**Current step:** architect contract DONE (`docs/contracts/P87-git-observability.md`); ui-designer DONE
(`docs/contracts/P87-ui.md` + ui-reference §12.10 — verified purely additive, 71/0, §1–§12.9 intact).
Implementation QUEUED behind P85 → P86.

Architect open-Q decisions: (1) Option B global `git_activity_subscribe` channel — confirmed;
(2) fetch/pull network progress via git2 sideband — IN scope; (3) log session-scoped only, no on-disk
retention v1; (4) NO cancel affordance v1 (read-only).

**Impl split:** **P87a** (backend: `GitActivityEvent` stream + `GitActivityHub`/subscribe + streaming
exec seam + hook/push phase + git2 `transfer_progress`) — senior-dev DONE (gate-clean: 917 core + 283
tauri tests, clippy -D both crates, tsc/build/size; additive guarantees held, HookRejected byte-identical;
god-files shrank via `_with_activity` sibling wrappers), reviewer APPROVED (no MUST/SHOULD-FIX; byte-identity,
deadlock, cap, sanitization all verified), **committed**. NITs: hub mutex held across `Channel::send`
(non-blocking, harmless); no-trailing-newline line delivery untested; command-layer active-path needs a
P87b integration check. (+ security-auditor on full P87 before batch integrates — git hook output → UI is
an untrusted surface); **P87b** (frontend:
`useGitActivity` store + git-activity dock + toolbar phase readout + determinate progress bar + mock)
— senior-dev DONE (full View C + View D + store + a11y + mock seams; gate-clean on touched surface: tsc,
build, eslint, lint:size, 34/34 new + 332/332 flow tests; CommitNote.tsx extracted to keep CommitBox <500;
Ctrl+Shift+L confirmed free). **Code review APPROVED (no MUST-FIX; dangerouslySetInnerHTML security concern
CONFIRMED CLEAN — all untrusted strings render as React text nodes; store/View C/HookDialog/mock all correct).
Design review APPROVED w/ 1 MUST-FIX — `.toolbar-phase` contrast (`--text-3`→`--text-2`), APPLIED INLINE by
orchestrator (exact ui-designer directive, 1-line CSS). COMMITTING.**

**P87b follow-ups (non-blocking):**
- **FU-1 (row target):** rows show bare "Push"/"Pull" — `GitActivityRun` has no `target`/ref field; add
  `target?: string|null` to the event+store (architect), render per §3.4. (design SF#2 + P87b flag.)
- **FU-2:** `commitAmend` (stash.ts) not activity-wrapped → no dock row for amend.
- **FU-3 (a11y):** row focus target is a role-less `<div>` — move `role="button"`+`aria-expanded` onto the
  roving `.git-run-summary` (chevron `aria-hidden`). (design SF#4 + code NIT.)
- **FU-4:** collapsed dock bar toggles only via the glyph, not the whole bar (design SF#3) — reconcile with
  the AI-dock twin (change both or the §5-1 wording).
- NITs: `everShown.current` written during render (GitActivityDock.tsx:75); duplicate consecutive announcer
  sentences won't re-announce; contract-wording reconciliations (chevron glyph, EmptyState reuse, dangling
  `aria-controls`); pre-existing autoFetchReadout shares the old `--text-3` contrast defect.
- **RepoWorkspace baseline +12** (3036→3048) — recommend refactorer split.
- **useWorkspaceKeyboard.test.tsx nav case** fails identically on HEAD (peer's 590f2ef, pre-existing) — fix to
  green the branch (see ⚠ FLAG FOR USER in P86 block).

**P87 security audit (P87a backend, commit 3bc616a): no CRITICAL/HIGH.** Sanitization funnel, 64 MiB byte
cap, integer/panic safety, progress throttle, command-exec all verified sound; hook code-exec is git's
own pre-existing behavior (P59a disclosure), not new.
- **M1 (MEDIUM — fixing now, backend, parallel to P87b):** line-EVENT emission is unbounded in count
  (unbounded mpsc + IPC fan-out); a hostile hook flooding stdout → tens of millions of tiny events (~2GB
  transient RSS + UI freeze) before the 64 MiB byte cap trips. Fix: bounded `sync_channel` backpressure in
  `exec_stream.rs` + per-activity line-event cap in `ActivityEmitter::line` (emit one "output truncated — N
  more" marker; GitOutput/HookRejected capture stays full & byte-identical). **DONE (senior-dev, gate-clean:
  sync_channel(1024) backpressure + MAX_ACTIVITY_LINE_EVENTS=5000 + L1 zero-width strip; byte-identity
  preserved; new activity_tests.rs keeps activity.rs <500; 25 cargo tests) — reviewer APPROVED (deadlock-safe,
  byte-identity preserved, no off-by-one; 653 tests), COMMITTED.** Note: M1's lint:size flagged CommitBox.tsx + RepoWorkspace.tsx
  over the ratchet — those are P87b's WIP → P87b must extract to stay under limit (added to P87b review).
- **L1 (LOW):** `activity_line` strips C0/C1+bidi but not zero-width (U+200B–200D/FEFF) — optional cheap add.
- **L2 (LOW, future):** activity events are app-global (no repoId) — fine under one-repo-open; tag+filter if
  multi-repo lands. **L3 (INFO):** reader/child leak only on a sink panic (no panic path today).
- **P87b review MUST confirm:** the log row renders `line` as a TEXT node, NOT `dangerouslySetInnerHTML`
  (the audit couldn't verify the frontend; text + control-strip is the safety basis).

ui-designer open-Q decisions: (Q1) button keeps stable participle ("Pushing…") + granular phase in an
adjacent `.toolbar-phase` readout — confirmed; (Q3) pull copy = "Fetching…" during transfer, "Pull" as
terminal row title — confirmed; (Q4) `Ctrl/Cmd+Shift+L` toggles the git-activity dock — confirmed;
(Q5) dock geometry session-only, no settings keys v1 — confirmed.
**(Q2) RESOLVED:** structured `Progress` event added — `GitTransferProgress {received/total/indexed
Objects, receivedBytes, total/indexedDeltas?}` from git2 `RemoteCallbacks::transfer_progress`, throttled
≤20/s in `remote.rs::fetch_remote`, additive default-no-op recorder method. Two sources, one stream:
CLI exec seam (hook/force-push lines) + git2 counts (fetch/pull). Both contracts agree (P87 §14 +
P87-ui §2.3/§9). **Push transfer-progress DEFERRED** (fetch/pull suffice; push slow-time is hooks via
exec seam). **P87 impl now UNBLOCKED — queued behind P86.**

**Goal (workstreams C + D).** Push/fetch/pull are blocking `invoke()`s with only a generic spinner;
the exec seam (`git/exec.rs:137`) captures stdout/stderr and returns it **only after the process
exits** — so a long `pre-push` hook (run via `git hook run`, `remote.rs:385`) is a silent
"forever" spinner. (a) **Live progress:** emit a phase signal so the UI shows a distinct "Running
pre-push hook…" state, and stream hook stdout/stderr live via a Tauri channel (reuse the AI streaming
pattern: `ai_stream.rs`, `crates/bonsai-core/src/ai/stream.rs`, `AiActivityLog.tsx`, `useAiRuns.ts`).
(b) **Session log:** a persistent, session-retained "Git output" log of every git command + hook run
(argv, exit code, stdout/stderr, timestamp), viewable anytime — including successful/passing hooks
whose output is currently captured then discarded (`hooks.rs:161`). Failure path already good
(`HookOutputDialog`, verbatim output + skip-hooks retry) — keep it and feed the same events into the
log. Architecture: ONE git-activity event stream, two views (live + log). New UI surface → routes
through `ui-designer` before senior-dev.

**Acceptance:** during a push with a slow hook the UI shows "running hook" + live output; every git
command/hook run is recorded in a reviewable session log with exit status; hook failure still opens the
existing dialog. Streaming is line-buffered and bounded (reuse the 64 MiB cap).

--- Archiving is a **move**, never a delete: condense on the board,
keep the full text in the archive, and leave a pointer. Archive files are listed at the bottom;
contract files are indexed in `docs/contracts/INDEX.md`.

---

## ✅ P82 — color-coded git identity profiles — done

**Current step:** none — AI gate GREEN + USER CHECKPOINT CONFIRMED (user 2026-08-21).

Each P44 identity profile carries a color so same-named profiles are distinguishable at a glance.
Closed 9-value named palette (`ProfileColor` = Neutral + 8 vetted hues), additive field-level
`#[serde(default)]` (legacy → Neutral, no `SETTINGS_VERSION` bump, git-config apply untouched).
Auto-distinct-on-upgrade is a **UI display fallback** (index hue for color-less profiles) + next-free
hue on create — no persistence rewrite; concrete color written only when the user touches the picker.
Surfaces: header avatar hue ring, identity-menu rows, Settings profile cards + a `role=radiogroup`
swatch picker. Tokens `--profile-*` both themes (ui-reference §12.8); no hardcoded hex; color never the
sole a11y carrier. Commit `c51db0f`. Contracts `P82-color-profiles.md` + `P82-ui.md`. Reviewer +
ui-designer approved (no MUST-FIX). Decision (user 2026-08-21): auto-distinct existing profiles on upgrade.

**USER CHECKPOINT (`pnpm tauri dev`):** two same-named profiles show distinct swatches; both themes
legible; active-profile color unmistakable in header/menu; picker keyboard nav + focus ring; pre-P82
settings.json migrates to distinct fallback hues; colors persist across restart.

**NIT follow-ups (non-blocking):** dead `[data-profile-color='neutral']` avatar-ring rule; `sanitizeProfiles`
shadows outer `raw` param; nextFreeHue-vs-autoDistinct first-slot overlap for legacy lists (per contract §6).

---

## ✅ P83 — merge & close/decline PRs from the panel (all 4 forges) — done

**Current step:** none — AI gate GREEN + USER CHECKPOINT CONFIRMED (user 2026-08-21; merge + close/decline verified per forge).

Adds Merge and Close/Decline/Abandon to the PR detail panel across GitHub, GitLab, Bitbucket, Azure.
`ForgeProvider::merge_pr`/`close_pr`; `MergeMethod` (Merge/Squash/Rebase/FastForward) filtered per forge
via `supported_for` ⟺ `SUPPORTED_MERGE_METHODS`; `HttpMethod::{Put,Patch}`. Unsupported methods rejected
before any request; not-mergeable/conflict → clear per-forge `ForgeApi`, nothing forced/retried/auto-resolved.
IPC `forge_merge_pr`/`forge_close_pr` via `open_with_key`; Azure head_sha backfilled backend-side, gated to
Azure kind. UI: `PrActionsBar` (primary Merge…, danger-secondary per-forge close verb), `PrMergeDialog`
(method picker, optional commit fields, delete-source-branch hidden for GitHub, Cancel-first focus + restore),
close reuses `ConfirmDialog`. Commits `4ea8a31` (P83a core+GitHub+IPC+UI), `8f5a82b` (P83b/c/d providers),
`651e2cc` (tests). Contracts `P83-pr-actions.md` + `P83-ui.md`, ui-reference §12.9. Reviewer + ui-designer
approved (no MUST-FIX); 3 SHOULD-FIX landed. cargo nextest 203 forge, +30 P82/P83 acceptance tests.

**USER CHECKPOINT (`pnpm tauri dev`, per forge, real PRs):** method dropdown lists only that forge's methods;
Merge disabled + reason on a conflicted PR; real merge reflects merged; real close/decline/abandon reflects
closed; Azure merge completes without the UI supplying a head sha.

**SHOULD-FIX/NIT follow-ups (non-blocking):** verify `.btn-secondary-danger` text contrast ≥4.5:1; app-wide
`ConfirmDialog` focus-restore gap; "using a squash/rebase" toast grammar; Bitbucket `post_merge` helper
factoring; per-provider `not_*_error` doc-comment grammar; true IPC-level Azure-backfill test needs a
transport DI seam.

---

## 🔀 Divergence reconcile — origin/main ⋈ local main (2026-08-21)

Local main (P77 tag-sync, P78 token guidance, P79/P80 multi-account forge) had diverged from origin/main
(merged PR #1 + the concurrent commit-panel UX overhaul, tracked here as **P80b**). Merged (not rebased),
4 conflicts resolved, full gate green — commit `77c815f`. **Not yet pushed** (awaiting user go-ahead).

---

## 🚢 Release 1.1.0 — cut 2026-08-20

Version files bumped to 1.1.0; `CHANGELOG.md` `[1.1.0]` finalized 2026-08-20 (Settings redesign,
audit-2 fixes, P70–P74). **Final tag → `e3cd2ea`** (the first `v1.1.0` tag failed macOS+Linux CI —
`gitbin::parse_reg_query` was dead code off Windows; `e3cd2ea` gates it `#[cfg(windows)]`, tag moved
onto it).

**P62–P74 native USER CHECKPOINTs were WAIVED and marked `done` 2026-08-20** (user decision): P62–P65,
P67, P68, P69 Settings (P69a–P69l), P71–P74. **P70 was NOT waived — its checkpoints were run and
confirmed (item 1 2026-08-20, items 2–8 2026-08-21); P70 fully `done`, archived → Part 17.** Full
build detail: `docs/history/todo-archive-2026-08.md` Parts 2, 4, 5, 7, 8, 11–15. Open follow-ups
spun out of those milestones are on this board below (NOT closed by the waiver).

---

## ✅ P80b / P81 / P82 — `done` (AI gate GREEN + native USER CHECKPOINT confirmed 2026-08-21)

- **P80b — commit-panel UX overhaul + next-file bug** — `done`. Merged to main via `56413b6` (Merge
  PR #1 from `worktree-commit-panel-ux`) + `77c815f`; commits `7ebe7fd`…`03a6453`. Contract
  `docs/contracts/archive/P80-commit-panel-ux-ui.md`. Archived → `docs/history/todo-archive-2026-08.md`
  Part 21.
- **P81 — refetch coalescing + watcher self-echo suppression** — `done`. Commit `be01422`. Contract
  `docs/contracts/archive/P81-refetch-coalescing.md`. Archived → Part 21.
- **P82 — submodule dirty-deinit requires explicit force (F-A7-7)** — `done`. Commit `ede7674`.
  Contracts `docs/contracts/archive/P82-submodule-force.md` + `P82-submodule-force-ui.md`. Archived → Part 21.

---

## 🛠️ DX — dev-loop acceleration — in-progress

**Goal:** act on the full-workflow velocity analysis (2026-08-20) — 68 GB `target/`, no build
acceleration, serial clippy/test, the 4-file IPC lockstep, a ~12-milestone deferred native-checkpoint
backlog. Ten improvements.

**Current step:** 8 of 10 landed & verified (`3ada322`, `2019e71`, `8e55be8`). **P75 (IPC codegen)
HALTED 2026-08-21 (user decision)** — a Phase 6.1 spike found that linking `tauri-specta` breaks app
launch on Windows 10 (`kernel32!WaitOnAddress` not exported → `STATUS_ENTRYPOINT_NOT_FOUND`); it's a
dev-velocity refactor with no user value, on RC crates, and the Win10 regression is unavoidable
because completing it requires linking tauri-specta into the app. 6.1 changes reverted; findings kept
in `docs/contracts/P75-ipc-codegen.md`. P76 (native-checkpoint automation) **held as contract-only**
per user.

**Landed & verified:**
- Build loop (`3ada322`): `[profile.dev] debug = "line-tables-only"` + `.cargo/config.toml` rust-lld
  linker (windows-msvc; Linux/macOS left opt-in). Verified.
- `cargo-nextest`: `.config/nextest.toml`, `pnpm test:rust`, `cargo nt` (bonsai-core 1417 / 6 skipped
  under it, ~184 s).
- One-command gate `scripts/gate.mjs` → `pnpm gate [--quick|--full|--rust|--frontend]`; clippy runs in
  its OWN target dir (`target/clippy`), so the test⟂clippy shared-target race is structurally
  impossible.
- Process (CLAUDE.md): step 4 runs code + design reviews concurrently; step 5 makes velocity mode
  (MUST-FIX-only, SHOULD-FIX/NIT filed as follow-ups, targeted intermediate gates) the default;
  senior-dev gains a pre-handoff self-review checklist.
- God-file splits (`2019e71`, `8e55be8`): branches.rs 2284→114 and stash.rs 2197→121 into focused
  submodules (public paths preserved, 1417 tests identical); RepoWorkspace.tsx overlay cluster →
  WorkspaceOverlays.tsx (382 tests identical). **Finding:** RepoWorkspace.tsx is a *legitimate*
  container — only a modest 85-line trim was safe.

**In-progress / designed:**
- **P75 — HALTED 2026-08-21 (user decision).** Would generate the IPC boundary with tauri-specta v2
  (kill the types.ts / tauri.ts / mock-layer lockstep). Phase 6.1 spike outcome: RC crates build &
  pin (`specta rc.22`/`tauri-specta rc.21`/`specta-typescript 0.0.9`), `AppError` manual `specta::Type`
  works, `bonsai-core` 881 green — BUT linking `tauri-specta` forces `tauri/specta`, whose binary
  statically imports `kernel32!WaitOnAddress`/`WakeByAddress*`; Windows 10 (this dev box, 19045) does
  not export those from `kernel32.dll` (KernelBase/api-set only) → `STATUS_ENTRYPOINT_NOT_FOUND` on
  load, so the app itself won't launch on Win10. Since P75 is dev-velocity only (no user value), on
  RC crates, and can't be completed without linking tauri-specta into the app (Phase 6.5), the Win10
  regression is unavoidable — halted. 6.1 code/deps reverted; the pinned trio, the bigint tradeoff
  (0.0.12 drops `BigIntExportBehavior::Number`), and the full blocker note are preserved in
  `docs/contracts/P75-ipc-codegen.md`. **Revisit only if** validated on Windows 11 or with a
  link-order fix forcing the `api-ms-win-core-synch` import lib ahead of `kernel32.lib`.
- **P76 — designed (HELD as contract-only per user).** Automate the native USER CHECKPOINT backlog
  with tauri-driver + WebdriverIO (~60–70% automatable; macOS has no WebDriver so its checkpoints
  stay human). `docs/contracts/P76-native-checkpoint-automation.md`.

**Deferred cleanups (noted, not done):** lock the file-size baseline reclaim for App.tsx (P74) and
RepoWorkspace.tsx once those land; the duplicated private `open_repo_at` helper across many `git/`
modules (a real refactor with call-graph impact, not a leaf move).

---

## ✅ Confirmed checkpoints and accepted decisions (condensed — full text in the archive)

- **P70 — git-executable resolution.** USER CHECKPOINT verified by user (item 1 confirmed 2026-08-20;
  items 2–8 verified 2026-08-21); shipped in 1.1.0 (`f0e9aee`). Archived → `todo-archive-2026-08.md`
  Part 17. (Refactorer follow-up RESOLVED 2026-08-21 — already split, see resolved-this-session note.)
- **P77 — tag sync management.** USER CHECKPOINT (items 1–6) verified by user 2026-08-21; AI gate
  GREEN. Commits `721349d`/`67c42b4`/`d2695bd`/`97ae417`/`e76b20b`. Archived →
  `todo-archive-2026-08.md` Part 18. (Deferred follow-ups carried to OPEN follow-ups below.)
- **P78 / P79 / P80 — forge fine-grained-token guidance, account management, and multi-account
  (host default + per-repo override).** All three `done` — AI gate GREEN + USER CHECKPOINT CONFIRMED
  (user 2026-08-21). Commits: P78 `d50cd42`; P79 `74cdfe0`+`813d305` (+settings.rs split `3386c3d`);
  P80 `01bb97e`+`323f8c5`. Resolution order (P80): repo override → owner-match (login==owner,
  lowercased, exactly one) → host default → single → first+nudge. Full condensed detail →
  `docs/history/todo-archive-2026-08.md` Part 20. Genuinely-open P80 SHOULD-FIX/NIT follow-ups are in
  the OPEN follow-ups section below.
- **All native USER CHECKPOINTs for P2 → P61 are CONFIRMED.** Batches: 2026-07-30 (P4, P3a–P3f, P7,
  P7e, P7f, P8, P9), 2026-08-03 (P18–P27), **2026-08-08** ("mark everything as checked" — P28 through
  P61 inclusive: P32, P37–P46, the credential-cache and UX-fix batches, Phase 1 P49–P52, Phase 2
  P53–P57, Phase 3 P58–P61). P5/P6 were confirmed earlier still.
- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated) · P57 retriever = BM25 lexical, no
  embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, accepted):** new Rust deps `reqwest{blocking,json,rustls-tls}` +
  `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order GitLab → Bitbucket
  → Azure DevOps.
- **v1.0.0 shipped** 2026-08-18 (tag `bd52483`), unsigned; forge/PR flagged beta. Full text of every
  banner and decision: `docs/history/todo-archive-2026-08.md` Part 1 + Part 10.

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

### ✅ Resolved this session (2026-08-21) — full text archived → `todo-archive-2026-08.md` Part 19
- **`read_status` vs `git status --porcelain` discrepancy — RESOLVED** (`f0eea9e`). Windows racy-git
  `WT_MODIFIED` phantom suppressed on Windows only (`#[cfg(windows)]`, git's `ie_match_stat`
  racy-clean rule); non-Windows unchanged. Regression seed appended to
  `crates/bonsai-core/tests/prop_status.proptest-regressions`.
- **CommandPalette highlight resets on `actions` array identity — RESOLVED** (`0798c55`). Reset now
  keys on the ordered visible row-id set, not array identity. vitest 14/14.
- **Refetch storm (audit #1 §3.10) — RESOLVED** (`be01422`, now milestone P81 above — native
  checkpoint pending).
- **Stash `expectedOid` UI wiring — RESOLVED** (`f36683e`). UI threads the rendered `StashEntry.oid`
  through the F-A6-B wrong-target guard. vitest 2079.
- **Submodule dirty-deinit force flag (F-A7-7) — RESOLVED** (`ede7674`, now milestone P82 above —
  native checkpoint pending).
- **`STDERR_GRACE_TOTAL` absolute cap — RESOLVED** (`95b7632`). `drain_stderr` now clamps each
  per-recv wait to the remaining time, so total ≤ `STDERR_GRACE_TOTAL`.
- **P70 credential-subsystem split (refactorer) — RESOLVED** (no action needed; item was stale).
  `crates/bonsai-core/src/git/cred.rs` (462 lines) already holds the full subsystem
  (`next_cred_method`, `credential_fill`, `acquire_cred*`, `map_remote_err`, `exhausted_error`,
  `evict_fresh_on_auth_fail`, `FillOutcome`, `CredAttempts`); `remote.rs` imports it — landed with
  P70's finalized tree.

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

---

## Archive

| File | Covers |
|---|---|
| `docs/history/todo-archive-2026-08.md` | Parts 1–9: P65 → P28 build detail, the Phase 1–4 banners, resolved FOR-USER decisions, P69(1.0.0)/P67/P68 detail, the 2026-08-17 batch mapping, resolved spun-out items. **Parts 10–16 (moved 2026-08-20): the P62–P74 checkpoint waiver + P71, P72, P73, P74, the P69 Settings redesign, and the Audit #2 fix batch, condensed. Parts 17–18 (moved 2026-08-21): P70 and P77, both checkpoints verified. Part 19 (moved 2026-08-21): the OPEN follow-ups resolved in the 2026-08-21 fix batch (read_status/palette/refetch/stash/submodule/STDERR/cred-split), verbatim. Part 20 (moved 2026-08-21): P78/P79/P80 forge milestones, condensed. Part 21 (moved 2026-08-21): P80b/P81/P82, done + checkpoints confirmed, condensed.** |
| `docs/history/todo-archive.md` | P27 → P2, M0–M6 |
| `docs/history/milestones-mvp.md` | the M0–M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into `todo-archive-2026-08.md` only once **both** halves of its gate have
passed (or the native half is explicitly waived). A milestone with a pending USER CHECKPOINT stays on
this board.
