# TODO archive — 2026-09 sweep

Moved out of `TODO.md` on **2026-09-01** by `docs-curator`. Continues the part numbering of
`docs/history/todo-archive-2026-08.md` (which ends at Part 21). **Verbatim** extraction — every
line below stood in `TODO.md` exactly as written; nothing was summarized away.

Curator's merge-state note (verified 2026-09-01 with `git branch --contains`, reported as fact, not
as a status upgrade): the branches several parts below record as "UNMERGED/UNPUSHED, awaiting merge
decision" are **now contained in `dev`** — `feat/pr-local-diff` (P89/P90), `perf/git-action-round2`
(P88), `chore/dep-refresh-2026-08` (DEP REFRESH; also in `main`). `feat/p91-observability` is still
contained in no other branch, matching the live P91 board entry.

Board headers were archived exactly as they read, including the header-vs-body contradictions listed
in the curator report (e.g. P88 headed `in-progress` while its body records both gate halves green;
P85/P86/P87/P87d headed `pending` while their bodies record DONE).

---

## Part 22 — P94 — e2e parallel-worker isolation — DONE (AI gate only, no USER CHECKPOINT)

## ✅ P94 — e2e parallel-worker isolation — DONE

**Current step:** done. Committed `c7e3dfe`. AI gate only — no USER CHECKPOINT (test infra, no
native-window behaviour). Verified **160 passed / 1 skipped** across three consecutive bare
`playwright test` runs (2.9 / 4.2 / 3.1 min) plus a fourth after my adaptive-worker edit (4.3 min).
The 1 skip is pre-existing, present in the baseline before any change.

**Root cause was NOT shared mutable state** — the original hypothesis (localStorage / mock module
state leaking across workers) was disproved with evidence: Playwright gives each test a fresh
context, no `storageState` is configured, and the mock's module-level state lives in each page's JS
realm, so it is per-test by construction. Two real causes instead:
1. **Machine oversubscription.** Playwright's default is half the cores = 11 concurrent Edge
   instances on this box, all loading a canvas-heavy app through ONE Vite dev-server transform
   pipeline. Signature was resource death, not wrong data: CDP "session closed" mid-test,
   `page.goto` timing out at `load`, whole spec files dying in cascade — which is exactly why the
   failing set moved between runs. Measured ladder: 11 workers → 7-12 failures in 5.8-6.3 min;
   6 → 3 failures in 3.6 min; 4 → 0-1 in ~4.5 min. **Capping is not a throughput trade — 4 workers
   is faster than 11.** Orchestrator edit: made the cap adaptive (`min(4, cores/2)`) so it can never
   oversubscribe a smaller box, since Bonsai ships cross-platform.
2. **`reuseExistingServer` on the shared port 1420** — the suite silently adopted whatever was
   listening, including a hand-run `pnpm dev` *without* `VITE_MOCK_IPC`, which boots against real
   Tauri IPC and leaves every spec on the empty state. **That is precisely the "mock repo never
   seeds" symptom P92 recorded.** e2e now owns port 1430 (`PORT` still overrides).

Plus a real row-map race, hardened behind a new `clickGraphRowUntilVisible` helper at the five
mutation→click→assert sites: `scrollHeight` settling does not mean the display-row map has settled,
because the WIP row can still toggle as the same refresh round's status slice lands, shifting every
row by one. The helper retries the **click**, never the assertion.

Nothing serialised, no `retries` bump, no test skipped or weakened.

**Goal:** the Playwright suite must be trustworthy in its DEFAULT parallel mode. Today it fails a
varying 2-10 specs per run (P92 saw 6-10; P93's gate saw 2 then 4, different specs each time) and
is green at `--workers=1`. Because the failures move around, a real regression can hide in the
noise — every gate run currently needs manual triage.

**Symptom (from P92):** the mock repo never seeds; the app sits on the empty state and no
`graph-canvas` ever appears. Strongly suggests cross-worker shared state (localStorage / a
persisted "last repo" key / a fixed port / a shared scratch dir) rather than a product bug.

**Acceptance:** `pnpm exec playwright test` green in default parallel mode across 3 consecutive
runs, with no `--workers=1` pin and no test weakened or skipped to get there. The root cause is
named in the commit message.


---

## Part 23 — P93 (PR diffs in the center overlay) and P92 (actionable multi-ref commits) — DONE, both USER CHECKPOINTs verified 2026-08-31

## ✅ P93 — PR diffs open in the center overlay — DONE

**Current step:** done. AI gate green + USER CHECKPOINT verified by the user 2026-08-31.

Round 2 reviewed and APPROVED by both reviewer and ui-designer (2026-08-31,
zero MUST-FIX). AI gate GREEN — full `pnpm gate` **8/8 steps** (Rust nextest 2051/2051,
cargo test --doc + clippy clean, eslint 0 errors, file-size ratchet OK, vitest 2379/2379,
tsc + vite build clean, Playwright e2e 160 passed). Committed `09eb5d9` on `dev`.
The unrelated scratch-path relocation was split into its own commit `d78b04e` (57 files).

~~**Remaining: USER CHECKPOINT**~~ — **VERIFIED by the user 2026-08-31.** AC20 (native-window
feel, large-diff scroll in the center overlay) and the end-to-end half of AC17 (clicking a commit
in the graph while a PR overlay is open leaves focus in the graph scroller) both confirmed on the
native app. Nothing outstanding on this milestone.

**Gate triage 2026-08-31 — all five initial red steps were non-P93:**
- `[rust] cargo nextest` / `cargo test --doc` / `cargo clippy` — cargo's cached `tauri` build-script
  output had a **stale absolute path from an old checkout** (`D:\Repos\Playground\bonsai`, which no
  longer exists) baked in, so the build script died reading plugin permissions. Fixed by
  `cargo clean -p tauri -p bonsai` plus deleting `target/clippy/debug/build/{tauri,bonsai}-*`
  (clippy uses its own target dir, so it needed clearing separately). nextest then went green:
  **2051 tests pass**. Not a code defect — if the repo is moved again, expect this and clean again.
- `[e2e] playwright` — 2 failed in the first run, 4 in the second, differing specs each time: the
  **known P92 parallel-worker isolation flake**. Re-ran the two originally-failing specs
  (`14-destructive-confirms`, `17-ai-dock`) at `--workers=1`: **18/18 pass**.
- `bonsai-core ai::session_tests::watchdog_does_not_fire_while_awaiting_input` — took 30s under
  parallel load, **passes isolated in 5.5s**. A watchdog timing test starved by CPU contention.
- `[frontend] vitest` — genuine but **pre-existing and unrelated to P93**: the 1000-branch Sidebar
  render in `adversarial-dto.test.tsx` takes ~7s against vitest's 5s default. The test file is
  unchanged since HEAD (last touched P77/P80) and Sidebar's only P93-touched import is
  `repoWorkspace/types`, which is type-only and erased at runtime. **User decision 2026-08-31:**
  bump that one test's timeout to 30s (it is a deliberate adversarial-scale render, so the 5s
  default is arbitrary for it). Now 22/22 pass in 4.5s.

**Goal:** the Pull requests tab was the last place a diff rendered INLINE in the narrow right
panel; every other diff (workdir staged/unstaged, commit files, compare) opens in the center
`DiffOverlay` over the graph. User-reported on 1.5.0.

**Approach (user-locked):** per-file center overlay — slot key `pr:<baseOid>:<headOid>:<path>`,
new `DiffOverlayMeta.kind` `'pr'`, data via the existing `ipc.forgePrFileDiff`. **No Rust/IPC
changes.** Rejected alternative: an all-files DiffBrowser `pr` source.

Contract: `docs/contracts/P93-pr-diff-center-overlay-ui.md` (rev 2 — §6.1 focus rule + AC18/AC19
were ui-designer errata found in review; rev 1 was implemented faithfully).

**Acceptance criteria:** AC1–AC19 in the contract. AC17 (focus stays in the graph scroller on a
commit click while a PR overlay is open) is a **USER CHECKPOINT** — the harness is headless-ish,
rAF never fires, so canvas clicks are no-ops; unit tests cover the C5 + head-advance cases instead.

**Deferred follow-ups (not blocking):** stale `prOverlayCtx` after slot replacement (latent, all
consumers key-gated); `onManageAccounts` callback identity; the fixture `fail` sentinel matches
`includes('fail')` too broadly; PR rows ignore `panelDensity` (pre-existing since P89, contract
§12.5).

**Round-2 review follow-ups (both agents approved; velocity mode — filed, not blocking):**
- SHOULD-FIX: no `overlayMeta.test.ts`. `overlayMeta.ts` was extracted to make the load-bearing
  prefix ordering (`conflict:`/`ai-proposal:`/`pr:` before the `WorkdirSection` cast) testable, but
  no test pins it. AC3 is covered only indirectly. A 5-line `deriveOverlayMeta('pr:…') → kind:'pr'`
  assertion would guard against regression.
- NIT: `PrChangesSection.tsx` focus restore resolves the row by positional index into
  `listRef.current.children`; a `data-path` + `querySelector` would be render-order independent.
- NIT: `overlayMeta.ts:41` `parsePrSlotPath(key) ?? key` would surface a raw `pr:<oid>:<oid>` key as
  the overlay path for a malformed key (unreachable via `prSlotKey()`).
- NIT: `PrDetailContainer.tsx:550-554` — on a PR switch, C2 (unmount) and C3 (headOid change) both
  fire `onClosePrFileDiff`. Idempotent, just a double call.
- NIT (pre-existing, not P93): `.diff-intra-toggle` off-state label is `--text-3` on the transparent
  overlay toolbar ≈4.0:1, under the 4.5:1 AA floor; `--text-2` would fix it. Shared overlay chrome.
- NIT (pre-existing house pattern): `PrDetailFallback.tsx:23` carries `error-banner-dismissible`
  with no dismiss button — `CommitPanel`/`ComparePanel`/`ComposerDialog` all do the same.

---

## ✅ P92 — Actionable multi-ref commits (branch picker + "+N" chip) — DONE

**Current step:** done. AI gate green + USER CHECKPOINT verified by the user 2026-08-31.
Committed `f5948d2` on `dev`. Evidence: vitest 2344/2344; Playwright e2e 160 passed / 1 skipped
(serial); tsc, vite build, file-size ratchet, eslint clean; reviewer + ui-designer approved.
`ui-reference.md` §6.2 + §4.1 corrected post-commit (defective clamp replaced by what shipped).

Open follow-ups spun out of P92 (do NOT block the increment):
- **e2e parallel-worker isolation** — the suite fails 6-10 specs in default parallel mode (mock repo
  never seeds; app stuck on empty state, no `graph-canvas`), green at `--workers=1`. Pre-existing,
  surfaced by P92. A real regression could hide in this noise — worth its own increment.
- `ContextMenu.tsx` is 486 lines (limit 500); `MenuList` is the extraction seam — split it in the
  next increment that touches the file.
- Graph scroller has a dangling `aria-activedescendant` IDREF and `role="grid"` with no `role="row"`
  children (pre-existing, not P92's doing) — own increment.
- Window-level arrow-key row nav can select a row without focusing the scroller, so the keyboard
  row-menu is unreachable that way.
- ~~USER CHECKPOINT (native)~~ — verified by the user 2026-08-31.

Problem (user, 2026-08-31): a commit carrying several refs shows a dead "+N" chip whose hidden
refs are hover-only and not actionable; and the commit context menu binds branch actions
(Merge/Rebase/…) to a single ref with no way to pick which branch.

- Contract: `docs/contracts/P92-multi-ref-commit-ui.md`; design system §6.2 in
  `docs/contracts/ui-reference.md`. Frontend-only — no Rust/IPC change.
- Design: "+N" chip becomes clickable and opens a `{n} more refs` menu, each row's flyout being
  that ref's existing full `branchMenuItems` menu; the commit-row menu prepends the same per-ref
  picker when ≥2 actionable refs. ≤1 ref ⇒ menu stays flat and identical to today.
- **Orchestrator decision (2026-08-31):** right-clicking a *visible* pill stays direct (no picker)
  — the contract §7 open question is resolved as "No".
- Removes the `fallbackBranchRef` chip-right-click fallthrough in `GraphCanvas.tsx`; adds an
  app-wide `max-height`/scroll clamp on `.context-menu`.
- Acceptance: contract ACs + vitest coverage (picker at ≥2 refs, absent at ≤1, `groupRefs`
  ordering, `main`+`origin/main` collapse, HEAD included) + browser-harness verification.
  USER CHECKPOINT: native right-click on a multi-ref commit.


---

## Part 24 — DEP REFRESH 2026-08-28 — DONE (AI gate + USER CHECKPOINT both green 2026-08-28)

## ✅ DEP REFRESH — 2026-08-28 — DONE (AI gate + USER CHECKPOINT both green 2026-08-28)

Goal: bring every frontend, Rust, and CI-action dependency to its current version, then
resync the user-facing docs (README/CHANGELOG/CONTRIBUTING) with the shipped 1.5.0 app.

**Current step:** done, nothing outstanding. AI gate green — full `pnpm gate` 8/8, nextest
2042 passed / 0 failed, cargo-deny advisories+bans+licenses+sources ok, `pnpm audit` clean,
reviewer verdict approve.

**USER CHECKPOINT verified 2026-08-28** — the user ran the native app and connected **GitHub**
and **Azure DevOps** with real access tokens; PRs list correctly under the new OS-trust-store
TLS path. GitLab and Bitbucket remain real-token unverified, and the README says so. The branch
is ready to merge; not pushed.

Shipped on `chore/dep-refresh-2026-08`: cfefb8e frontend majors (ESLint 10, Vite 8,
TS 6.0, jsdom 30) · 13084ca CI action pins (incl. tauri-action v1) · 1fd0a47 docs resync ·
450715a Rust majors (criterion 0.8, rand 0.10, reqwest 0.13 + ring provider, rmcp 3.1).
Backed out deliberately: TypeScript 7 (typescript-eslint 8.68 rejects the TS 7 API) and
keyring 4 (restructured onto keyring-core; needs its own increment).

Follow-ups filed, not blocking:
- **keyring 3 → 4** needs a dedicated increment: 4.x moves onto `keyring-core`, renames every
  per-backend feature (`windows-native` → `windows-native-keyring-store`, etc.), drops
  `crypto-rust`, and requires explicit credential-store registration instead of feature-driven
  resolution — i.e. real changes to `crates/bonsai-forge/src/auth.rs`.
- `no_proxy_client()` in `src-tauri/src/mcp/http_support.rs` still uses
  `.expect("build reqwest client")`; fine for a test harness, but it is why the missing rustls
  provider surfaced as a raw panic rather than a message.
- `TODO.md` is ~760 lines against the ~300 target and wants a `docs-curator` compaction pass.



---

## Part 25 — P90 (per-branch CI Checks view) and P89 (PR files & local diff view) — DONE, both USER CHECKPOINTs verified 2026-08-25

## ✅ P90 — Per-branch CI Checks view — DONE (AI gate + USER CHECKPOINT both green 2026-08-25)

**Current step:** none — AI gate passed (tsc/build clean, 52 vitest, size ratchet OK; harness-verified all
per-branch states, no-auto-switch, connect, links, live-region). Reviewer + ui-designer both approved
(MUST-FIX push-refresh fixed). **USER CHECKPOINT VERIFIED (2026-08-25):** user confirmed on the native
app — Checks tab shows live per-check detail and refreshes on fetch/pull/push. Committed `b0e880c` on
`feat/pr-local-diff` (branch still UNMERGED/UNPUSHED — see P89 merge decision).

**Follow-ups (deferred):** P90.1 per-check timing fields; header commit-summary text; command-palette
`Refresh checks` / `Show checks`; mock fixtures for noForge/error reachable by click.

**User decisions (2026-08-25):** (1) Defer per-check timing fields to P90.1 — ship v1 with
name/state/description/link (all already on `StatusContext`); (2) No auto-switch to Checks tab on
branch click (content updates, focus stays); (3) Placement = third right-panel tab "Checks".

**Goal:** A dedicated right-panel view (new tab near "Working" / "Pull requests", exact placement
decided by ui-designer) that shows CI check details for the branch the user clicked in the sidebar.
Shows per-check detail (name, state, description, link) beyond the existing graph rollup badge, and
refreshes to latest status on every fetch / pull / push.

**Known backend surface (already shipped, reuse):**
- `CommitStatus { sha, state, total, passed, failed, pending, contexts }` and
  `StatusContext { name, state, description, target_url }` — `crates/bonsai-forge/src/types.rs:280-302`.
- IPC `forgeCommitStatuses(repoId, shas[]) -> CommitStatus[]` — `src/ipc/tauri/forge.ts`.
- `forgeSignals` already refreshes CI verdicts after fetch/pull — `src/components/repoWorkspace/useForgeSignals.ts`.
- Right-panel tabs `'work' | 'prs'` — `src/components/WorkspaceRightPanel.tsx:251-270`.

**Open scope decisions (architect/ui-designer to resolve, flag to user):** whether `StatusContext`
needs new timing fields; behavior when forge unconfigured / branch has no upstream.

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
Follow-ups SF1+SF2+NIT **DONE + committed `fe23d08`** (reviewer APPROVED; tsc clean, 24 vitest,
clippy -D clean, 6 azure::refs tests incl. 3 new TFS-fallback cases). **All P89 follow-ups cleared.**
Branch `feat/pr-local-diff` (8 P89 commits) stays UNMERGED/UNPUSHED per user. Nothing left on P89.

---


---

## Part 26 — P88 — Git-action perf round 2 (refresh-scope cluster + repo-handle cache)

Archived as it read: header `in-progress`, body records the batch AI gate green (2026-08-24) and the
native USER CHECKPOINTs verified 2026-08-25, with only the merge decision outstanding — and that
branch is now contained in `dev`.

## ⚡ P88 — Git-action perf round 2 (refresh-scope cluster + repo-handle cache) — **done** (status confirmed by USER 2026-09-01)

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
**USER CHECKPOINTs VERIFIED (2026-08-25):** user confirmed on the native app — create/delete tag, stash
push/pop/drop, commit, add/remove remote+submodule, delete local branch, file-by-file conflict resolve all
snappy + consistent UI (no stale ahead/behind after commit); no regression in checkout/fetch/pull/push/rebase/merge.
Branch `perf/git-action-round2` tip = `059caf3` (10 commits off `c0825a3`/1.3.0) — **still UNMERGED/UNPUSHED,
awaiting merge decision** (checkpoints done, merge is a separate call).

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


---

## Part 27 — PERF + OBSERVABILITY BATCH (P85–P87), incl. P87c and P87d

Archived as it read: the batch banner records AI gate green + USER CHECKPOINTS VERIFIED (2026-08-25),
while the individual P85/P86/P87/P87d headers still read `pending` and their bodies read DONE.

## ✅ PERF + OBSERVABILITY BATCH (P85–P87) — AI GATE GREEN + USER CHECKPOINTS VERIFIED (2026-08-25)

> **Status reconciled 2026-09-01.** The five milestone headings below (P85, P86, P87, P87d, P88)
> read `pending` / `in-progress` while their bodies recorded both gate halves green and the native
> USER CHECKPOINTs verified on 2026-08-25. The 2026-09-01 curation sweep flagged the mismatch and
> declined to resolve it (agents never upgrade a status). **The USER settled it on 2026-09-01:
> all five are done and verified.** Headings updated accordingly; no body text was changed.

**Full `pnpm gate` re-run CLEAN @ `b802482`: all 8 steps passed (GATE_EXIT=0)** — cargo test/clippy (both
crates), frontend tsc+build, vitest, eslint (38≤40), file-size ratchet, playwright e2e. Commits: P85 `fde91d7`,
P86a `e294500`, P86b `88ba86a`, P87a `3bc616a`, M1 `9eceafe`, P87b `298e45b`, P87c `c9a523c`, P87d `b802482`.
Browser-harness verified (mock): determinate fetch progress (12.5k→25k/50k objects, scaleX .25→.5), "Running
pre-push hook…" phase, `.toolbar-phase` = --text-2, hook-fail dialog + failed dock row. Security-audited (no
HIGH/CRITICAL; M1 fixed). **USER CHECKPOINTs VERIFIED (2026-08-25):** user confirmed real branch-create/fetch
wall-time + live phase readout + progress bar + log dock in `pnpm tauri dev`. Non-blocking follow-ups: PB-1/PB-2 (cache mem cap +
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

## 🔴 P87d — fix the pre-existing peer nav test (USER APPROVED 2026-08-23) — **done** (status confirmed by USER 2026-09-01)
`useWorkspaceKeyboard.test.tsx > "nav is inert (and not default-prevented) with no selection or no graph"` fails (peer's
590f2ef "first arrow seeds selection"). User chose "Fix it too". **Diagnosis: TEST-ONLY fix — the app is
CORRECT.** `useWorkspaceKeyboard.ts:320-338` implements the intended M2 feature: graph present + `selectedIndex===null`
→ first arrow SEEDS selection (preventDefault + setSelectedIndex). The old test's `{selectedIndex:null}` (graph-present)
case wrongly asserts inert; the `{graph:null}` case is still correctly inert. Fix the TEST (split: no-graph→inert;
graph+no-selection→seeds), do NOT touch useWorkspaceKeyboard.ts. Routed to tester.
**P87d DONE (tester):** split into two tests (no-graph→inert; graph+no-selection→seeds anchor: ArrowDown/Home→0,
ArrowUp/End→9); 40 passed, no app code touched. Committing, then full-gate re-run CLEAN.

---

## ⚡ P85 — Refresh perf: route ref-mutations through echo-suppressed refresh — **done** (status confirmed by USER 2026-09-01)

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

## ⚡ P86 — Refresh perf: graph-layout & repo-handle caching, scoped refresh — **done** (status confirmed by USER 2026-09-01)

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

## ⚡ P87 — Git & hook output observability: live progress + session log — **done** (status confirmed by USER 2026-09-01)

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


---

## Part 28 — P82 (color-coded git identity profiles) and P83 (merge & close/decline PRs from the panel) — done

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


---

## Part 29 — Divergence reconcile (2026-08-21), Release 1.1.0 (cut 2026-08-20), P80b/P81/P82 confirmation block

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


---

## Part 30 — DX — dev-loop acceleration (full text; a condensed stub stays on the board)

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


---

## Part 31 — Confirmed checkpoints and accepted decisions (full text; the accepted defaults and the two FOR USER items stay on the board)

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


---

## Part 32 — OPEN follow-ups resolved in the 2026-08-21 session (as it stood on the board; the underlying full text is archive-2026-08 Part 19)

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

---

## Part 33 — P84 (sidebar reveal-in-graph + tag auto-sync) — record gap, archived on user instruction 2026-09-01

**Read this entry before assuming P84 is done.**

- P84's **code shipped**: `cce9eb9`, `90b315c`, `1803391`, merge-back `6868be6`.
- P84 **never had a board section** in `TODO.md` and never had a `docs/history/` entry.
- Its **USER CHECKPOINT was therefore never recorded** and cannot be verified from the record.
  Nothing in this archive should be read as the checkpoint having passed.
- Its two contracts were moved to `docs/contracts/archive/` on **2026-09-01**, on the user's
  explicit instruction ("cleanup everything until P91, archive history"), purely to close the
  dangling record gap — **not** because the checkpoint was confirmed:
  - `docs/contracts/archive/P84-sidebar-reveal-and-tag-autosync.md` — sidebar click-to-reveal-in-graph
    (frontend) + automatic tag sync on fetch (one core fn + one command).
  - `docs/contracts/archive/P84-reveal-in-graph-ui.md` — UI contract for reveal-in-graph: single-click
    sidebar → scroll + flash.
- The previous curation sweep (2026-09-01, earlier the same day) **refused** to archive these for
  exactly this reason; that refusal is superseded by the user's instruction, but the underlying fact
  (unrecorded checkpoint) stands and is preserved here.

---

## Part 34 — macOS ad-hoc code signing — config DONE 2026-08-30, release still pending (verbatim off the board)

Archived 2026-09-01. A one-line live pointer stays in `TODO.md` because the **release half is still
pending**: the last tag is `v1.5.0` (2026-08-26), which predates the 2026-08-30 config change, so
the fix has not shipped in any release yet.

- **Symptom:** installed release repeatedly triggers the macOS "Bonsai would like to access your
  Downloads folder" TCC prompt, multiple at once, and re-prompts after Allow. Root cause: the release
  `.app` was only linker-ad-hoc-signed — `Info.plist=not bound`, `Sealed Resources=none`, identifier
  `bonsai-<hash>` not `com.bonsai.app`, `codesign --verify` → "not signed at all". TCC has no stable
  identity to anchor the grant to.
- **Fix applied:** `bundle.macOS.signingIdentity: "-"` added to `src-tauri/tauri.conf.json` → Tauri now
  runs a proper sealed ad-hoc `codesign` on the bundle. Takes effect on the next tagged release.
- **User's currently-installed app** was manually re-signed on 2026-08-30
  (`codesign --force --deep --sign - --identifier com.bonsai.app`) + `tccutil reset` — prompt should
  now stick after one Allow.
- **Still not fixed by ad-hoc:** Gatekeeper "unidentified developer" warning; a new version re-prompts
  once (cdhash changes). Full fix = Developer ID + notarization (needs Apple Developer Program) — the
  `APPLE_*` env block in `.github/workflows/release.yml` is already scaffolded for it.

---

## Part 35 — The two dated 2026-08-22 design reviews — dispositions (archived 2026-09-01)

Both review contracts were moved to `docs/contracts/archive/` on **2026-09-01**. Findings verified
by the curator against the current tree at HEAD `ed5bb11`; anything **not** confirmed resolved is
listed as still-open here **and** kept as a live line in `TODO.md`.

### 35.1 `docs/contracts/archive/graph-design-review-2026-08-22.md`

- **M1 — "make the graph a focusable, announced composite widget" (`role="grid"` +
  `aria-rowcount` + `aria-activedescendant`) is SUPERSEDED BY P95 — DO NOT IMPLEMENT.**
  `docs/contracts/ui-reference.md` §4.1 (heading: "ARIA model revised 2026-08-31, P95",
  `ui-reference.md:250-252`) now states verbatim: "**`role="grid"`, `aria-rowcount` and
  `aria-activedescendant` are forbidden here.**" The shipped model is live-region-only:
  `.graph-scroll` carries exactly `tabIndex={0}`, `role="group"`, `aria-label="Commit graph"` and
  `aria-describedby` → the `.sr-only` keyboard hint, with `GraphSelectionAnnouncer` as the sole
  announcement channel. Verified by the curator 2026-09-01.
- **S1 — ui-reference §4 accuracy** — marked DONE in the review's own pass.
- **M2, M3, M4, S2, S3, N1, N2 — resolution NOT verified** by this sweep (bounded effort). Carried
  as a live line in `TODO.md` so nobody assumes they landed.

### 35.2 `docs/contracts/archive/review-2026-08-22-ui.md`

Verified **resolved** (evidence at HEAD `ed5bb11`):

- **MUST-1 — `ui-reference.md` truncated to 4 subsections** → RESOLVED. The file is now 1224 lines
  with §1–§13 (layout, tokens, typography, graph metrics, lane palette, ref pills, file-status
  colors, states, AI dock, notice bar, status pills, settings surface, icon system).
- **MUST-2 — sidebar rows keyboard-inaccessible** → RESOLVED. The §D `role="tree"` contract shipped:
  `src/components/Sidebar.tsx:117` ("the six sections compose one `role="tree"`"), with
  `role="treeitem"` + `aria-level` on `src/components/sidebar/SectionHeader.tsx:39`,
  `SubmoduleRow.tsx:36`, `TagsSection.tsx:61`, and `sidebar/rows.tsx`.
- **MUST-3 — icon-only toolbar buttons lack accessible names** → RESOLVED. In
  `src/components/WorkspaceToolbar.tsx` the three icon-only buttons carry `aria-label`
  ("More push actions" `:212`, "Open externally" `:271`, "Refresh" `:284`); the remaining buttons
  carry visible text labels plus a `title`.
- **SHOULD-1 — emoji as the app-wide icon language** → RESOLVED. `lucide-react ^1.34.0` is a
  declared dependency (`package.json:37`) and the SVG icon system is specced in `ui-reference.md`
  §13.
- **SHOULD-2 — onboarding last step exposes three dismiss controls** → RESOLVED per spec B.1.
  `src/components/OnboardingOverlay.tsx:166-167,251-254`: `isLast` computed, Skip rendered only
  before the last step, primary label `Get started`/`Next`/`Finish`, `✕` always present.
- **SHOULD-4 — HEAD branch name marginal on hover** → RESOLVED per spec B.2. `src/styles/sidebar.css`
  now carries the B.2 comment verbatim ("the checked-out branch is conveyed by weight + the accent
  glyph + aria-current — NOT by hue on the label") and `.branch-row-head .branch-name` is
  `font-weight:600; color: var(--text-1)`.
- **NIT-3 — obscure list-view toggle glyph `⋔`** → RESOLVED. Zero occurrences of `⋔` remain under
  `src/components/`.

**Still open** (kept as live lines in `TODO.md`):

- **SHOULD-3 — `--accent` as text colour over `--selection` fails AA.** This is the same item as the
  live **P69 A9** follow-up; `ui-reference.md` §2 now prohibits new call sites, but the ~30 existing
  ones are unaudited.
- **NIT-1 — Sidebar ignores `panelDensity`.** Confirmed still open: no `panelDensity`/`density`
  reference exists in `src/components/Sidebar.tsx`, `src/components/sidebar/**`, or
  `src/styles/sidebar.css`; `.branch-row` remains a fixed height.
- **NIT-2 — onboarding `✕` accessible name.** Confirmed still open:
  `src/components/OnboardingOverlay.tsx:229` is still `aria-label="Close"` (the review preferred
  "Close the tour").

Everything else in that review (§B redesign specs, §C icon-system verdict, §D sidebar keyboard
contract, positive findings) is descriptive/shipped and preserved verbatim in the archived file.

