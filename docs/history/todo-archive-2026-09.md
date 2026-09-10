# TODO archive — 2026-09 sweep

Moved out of `TODO.md` on **2026-09-01** by `docs-curator`. Continues the part numbering of
`docs/history/todo-archive-2026-08.md` (which ends at Part 21). **Verbatim** extraction — every
line below stood in `TODO.md` exactly as written; nothing was summarized away.

Curator's merge-state note (verified 2026-09-01 with `git branch --contains`, reported as fact, not
as a status upgrade): the branches several parts below record as "UNMERGED/UNPUSHED, awaiting merge
decision" are **now contained in BOTH `dev` and `main`** — `feat/pr-local-diff` (P89/P90),
`perf/git-action-round2` (P88) and `chore/dep-refresh-2026-08` (DEP REFRESH) all verified
2026-09-01 with `git merge-base --is-ancestor` against both branches. (An earlier version of this
note said only dep-refresh had reached `main`; re-verification showed all three had.) The USER
confirmed on 2026-09-01 that the merge decision is settled, so each stale "UNMERGED/UNPUSHED" line
below now carries an inline correction. `feat/p91-observability` is still contained in no other
branch, matching the live P91 board entry.

Board headers were archived exactly as they read, including the header-vs-body contradictions listed
in the curator report (e.g. P88 headed `in-progress` while its body records both gate halves green;
P85/P86/P87/P87d headed `pending` while their bodies record DONE).

## Second sweep — **2026-09-03** (Parts 36-50)

`TODO.md` went 2337 lines -> ~400. Extraction is **verbatim**; nothing was summarized away, and no
status was upgraded by the curator.

Five milestones with a **pending USER CHECKPOINT stayed on the board** and were NOT archived —
P102+P105 (AC18/19/20), P106 (AC14/AC15 + the real-repo half of AC9), P107 (AC11/12/13),
P108 (AC12/13/14, and AC11 recorded as OWED/unverified) and P91 (its checkpoint was never
presented). For those five, only the **review/implementation transcript** moved here (Parts 37-40,
42-45); the milestone entry, its status, its checkpoint items and its evidence stay live in
`TODO.md`.

Four **open user decisions** also stayed live rather than being archived: the e2e bundle-default
flip, security **F6** (`usage.json` disclosure), home-directory/username masking in raw log paths,
and **D3** (whether `.op-worktree-warning` should be painted danger at all).

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
`feat/pr-local-diff` (branch still UNMERGED/UNPUSHED — see P89 merge decision). **[CORRECTED 2026-09-01 — this branch is now contained in BOTH `dev` and `main`; verified with `git merge-base --is-ancestor`. The merge decision was made; the text above is the historical record.]**

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
`71171d4` `a0f0575` `a988388` `48ae17f`. **NOT merged/pushed.** **[CORRECTED 2026-09-01 — this branch is now contained in BOTH `dev` and `main`; verified with `git merge-base --is-ancestor`. The merge decision was made; the text above is the historical record.]**
**USER CHECKPOINT VERIFIED (2026-08-25):** user confirmed everything OK on the native app (Azure +
GitHub PR changed-files list + correct three-dot counts + expand-to-diff; fork-head auto-fetch;
offline/Retry). Branch `feat/pr-local-diff` still **NOT merged/pushed** — awaiting merge decision. **[CORRECTED 2026-09-01 — this branch is now contained in BOTH `dev` and `main`; verified with `git merge-base --is-ancestor`. The merge decision was made; the text above is the historical record.]**
Follow-ups SF1+SF2+NIT **DONE + committed `fe23d08`** (reviewer APPROVED; tsc clean, 24 vitest,
clippy -D clean, 6 azure::refs tests incl. 3 new TFS-fallback cases). **All P89 follow-ups cleared.**
Branch `feat/pr-local-diff` (8 P89 commits) stays UNMERGED/UNPUSHED per user. Nothing left on P89. **[CORRECTED 2026-09-01 — this branch is now contained in BOTH `dev` and `main`; verified with `git merge-base --is-ancestor`. The merge decision was made; the text above is the historical record.]**

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
`709c9cc`. Branch `perf/git-action-round2` (off c0825a3/1.3.0), NOT merged/pushed. **[CORRECTED 2026-09-01 — this branch is now contained in BOTH `dev` and `main`; verified with `git merge-base --is-ancestor`. The merge decision was made; the text above is the historical record.]** **PENDING: native USER CHECKPOINTs
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
awaiting merge decision** (checkpoints done, merge is a separate call). **[CORRECTED 2026-09-01 — this branch is now contained in BOTH `dev` and `main`; verified with `git merge-base --is-ancestor`. The merge decision was made; the text above is the historical record.]**

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

## Part 33 — P84 (sidebar reveal-in-graph + tag auto-sync) — **done + verified** (checkpoint confirmed by USER 2026-09-01)

**Read this entry before assuming P84 is done.**

- P84's **code shipped**: `cce9eb9`, `90b315c`, `1803391`, merge-back `6868be6`.
- P84 **never had a board section** in `TODO.md` and never had a `docs/history/` entry.
- Its **USER CHECKPOINT was never recorded at the time** and could not be verified from the record.
  **RESOLVED 2026-09-01: the USER confirmed that P84's checkpoint DID pass.** Recorded on the
  user's direct confirmation on that date — not from a contemporaneous 2026-08 record, which
  never existed. P84 is therefore **done and verified**.
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



---

## Part 36 — File-size refactor pass — 2026-09-02 — DONE (verbatim off the board; its still-open follow-ups stay live in `TODO.md`)

## 🧹 File-size refactor pass — 2026-09-02 — DONE

Behavior-preserving split of the 9 largest files back toward the ~500-line limit.
Ratchet baseline: **27 offenders / 6241 excess → 20 / 3528**. Full `pnpm gate` green
(8/8, 603s) on a quiet tree. 17 files modified, 60 new files, 454 insertions / 9636 deletions.

| File | Before | After |
|---|---|---|
| `src/components/RepoWorkspace.tsx` | 2760 | 2309 (77 → 34 `useState`; 9 hooks extracted) |
| `crates/bonsai-core/src/assets/bundle.rs` | 1366 | → `bundle/`, 9 modules, max 355 |
| `crates/bonsai-core/src/git/stash/tests.rs` | 1330 | 349 (+4 modules) |
| `crates/bonsai-core/src/assets/profiles.rs` | 1205 | → `profiles/`, 7 modules, max 353 |
| `crates/bonsai-core/tests/diff/diff_cli.rs` | 1026 | 307 (+3) |
| `crates/bonsai-core/tests/rebase_merge/rebase_interactive_cli.rs` | 1021 | 279 (+3) |
| `crates/bonsai-core/src/graph/tests.rs` | 1012 | 93 (+4) |
| `src/App.tsx` | 977 | 602 (+6 hooks, 2 dialogs) |
| `src/graph/GraphCanvas.tsx` | 973 | 784 (+4) |
| `tests/rebase_merge/{rebase,merge,conflict}_cli.rs` | 923/763/640 | 239/222/147 |
| `tests/diff/{stage,discard}_partial_cli.rs` | 760/504 | 342/355 |

Equivalence proof per increment: identical before/after test counts (`bonsai-core` 1554,
`h_diff` 80, `h_rebase_merge` 102, vitest 229 files / 2644 tests, graph e2e 25), plus
line-multiset diffs showing zero logic-line changes on the Rust splits.

**Three files deliberately stopped short of 500** — each remaining cut would have produced a
file forwarding 15–100 values to exactly one consumer (an unreadable pair, not a smaller module):
`RepoWorkspace.tsx` 2309 (render body already fully extracted; rest is state+effects+handlers),
`GraphCanvas.tsx` 784 (per-frame paint path + a 29-value handler closure — hot path, 20k-commit
target), `App.tsx` 602 (launch effect needs 6 of App's own setters threaded in).

### Follow-ups spun out of this pass (all filed as tasks, none blocking)

- **Reflog overlay not torn down** when a repo goes unusable — `runRefreshRound` clears blame +
  history + compare + opState + tagSync, but not reflog. Real bug.
- **Fold-pill cursor is dead** in `GraphCanvas.handleMouseMove` — P92 §1.4's overflow-cursor write
  unconditionally clobbers spec-004 §1/§2's `foldCursorFor`, and `computeHoverTarget` returns null
  on exactly those rows. Real regression; no vitest mounts `GraphCanvas`, so e2e is the only net.
- **Shortcuts stay live during confirm dialogs** — `pendingForcePush`, `pendingCommitPush`,
  `pendingBisectBad` are absent from `dialogOpen` (`abortConfirmOpen` is handled separately).
  Force-push is destructive, so this one matters most.
- **`ai::session*` is load-flaky** — wall-clock watchdog margins (2s idle timeout vs a ~3s stub)
  fail under CPU contention, pass in isolation. Hit independently by 3 agents; makes the crate
  suite unreliable as a gate under load. Needs a clock seam, not wider sleeps.
- **Contract divergences** the tests document as bugs-in-the-contract (both say "reported to the
  orchestrator"): rebase §3.1.5/§9.7 unstaged-changes precondition, and the libgit2-vs-CLI
  rename/delete conflict index-entry count. Plus a near-tautological `expected_presence` oracle.
- **Duplicated external-tool launchers** — `App.tsx`'s trio is statement-for-statement identical to
  `repoWorkspace/useExternalTools.ts`; hoist to `src/hooks/`. Also two timers with no unmount
  cleanup (`sessionSaveTimer`, toast auto-dismiss).
- **Duplicated helpers left visible, not merged** (behavior risk, not a move): atomic-write helpers
  across `assets/bundle/write.rs` + `assets/profiles/store.rs`; test helper families across
  `tests/diff/` and the four `tests/rebase_merge/*_support.rs`.
- **`image_diff_cli_2.rs`** numbered split remains — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff.
- **`cargo fmt --check` is not clean at HEAD** repo-wide and is not gated. New files inherit the
  existing drift deliberately (reformatting would have destroyed the proof-by-diff). Repo-wide
  `cargo fmt` is a separate decision.


---

## Part 37 — P102 + P105 — hue audit — full board narrative (verbatim). **The milestone itself stays LIVE on the board**: AC18/AC19/AC20 are pending USER CHECKPOINTs. Only the review/implementation transcript moved here.

## 🎨 P102 + P105 — hue audit — AI GATE GREEN, ⏳ AWAITING USER CHECKPOINT (AC18/19/20)

**Current step:** ✅ **AI GATE GREEN — MILESTONE COMPLETE bar the USER CHECKPOINT.**
contract `7c623d8` → impl `0e5dcab` → both reviews APPROVE after fixes → fixes `185c352` →
**FULL gate green**: `--quick` all 7 steps (255.9s) **and e2e 181 passed / 1 skipped (2.7m)**.
e2e was deliberately run *after* the ui-designer released harness port 1420 — running two things
against one dev server is how e2e specs flake.

**⏳ AWAITING USER CHECKPOINT: AC18 / AC19 / AC20.** Not self-declared; the user's 2026-09-02
checkpoint authority was scoped to P100 + P101 and explicitly does **not** reach work created that
session. Evidence gathered to make the check fast: **AC18** — the updater panel is Tauri-only, but
its `.btn-danger` shares the exact rule measured at 4.80/4.93 rest and 5.18/5.49 hover, so the
question is "does it look right", not "is it legible". **AC19** — `--accent-strong` holds hue within
**0.7°** of `--accent` in both themes (219.0° vs 219.7°); the open question is purely whether the
dark `#7fabff` reads washed-out. **AC20** — no `filter` remains on any of the four hover targets, so
there is no compositing pass left to flicker and layout cannot move (background-only swap); what is
being judged is fill-change feel alone.

**Gate `--quick` @ `185c352`:** cargo nextest 139.0s ✓ · doctests 4.0s ✓ · clippy 22.4s ✓ ·
eslint 16.2s ✓ · **file-size ratchet 1.0s ✓ (the blocker is cleared)** · vitest 59.6s ✓ ·
tsc+build 13.7s ✓.

**The predicted grep residue matched reality on every line** — this is the milestone's central
claim, and it held: `color: var(--accent)` 30→**7**, `--accent-strong` 0→**18**, hex ink 3→**0**,
`filter: brightness` 4→**1**, `background: var(--danger)` 4→**4**, `--danger-text` 0→**4**,
`--success-text` 0→**1**. (Raw greps return 4 hex / 5 brightness; both extras are *comment* lines at
`dialogs-forms.css:164` and `controls.css:96` — declaration counts match.) Predicting the residue
*before* implementing is what makes "closed" checkable instead of asserted, and it is the practice
P95/P98/P74 skipped.

**Two implementer deviations, both flagged rather than buried (verdicts pending review):**
1. **Six `.asset-chip-*` modifiers fixed, not four.** `.asset-chip-sync` (4.02 dark / 3.61 light)
   and `.asset-chip-drifted` (4.86 / 3.50) are the same hue-over-own-tint defect inside the same
   rule block and render *beside* the fixed chips in `ProfileActivateDialog` /
   `StaleBranchesDialog`; fixing 4 of 6 would leave exactly the split the contract's own C3b
   reasoning forbids.
2. **One rule added that the contract missed:** `.settings-toggle-btn.is-active:hover:not(:disabled)`.
   `.btn-secondary:hover:not(:disabled)` is specificity (0,3,0) and out-specifies C5's (0,2,0), so
   the selected `--selection` fill vanished on hover. If the specificity claim holds this is a
   contract gap, not scope creep — **a contrast fix that is out-specified by an existing rule is a
   no-op that still passes a grep**, which is worth remembering as a review heuristic.

### Design review — **APPROVE** with one MUST-FIX, and the designer named it as its OWN defect

`ui-designer` verified the grep invariants independently (7 / 18 / 0 / 1, tokens in both blocks,
all five ratios recomputed to ±0.01) and did the visual half the implementing agent could not:
**both themes, 1440×900, computed styles with tints flattened to their composited backdrop.** Every
measured pair clears its bar — pills 4.80-6.24 dark / 4.93-5.08 light, `.right-pane-tab.active`
9.36/13.29, the six `.asset-chip` modifiers 8.99-9.67 / 11.29-11.85. **AC12 confirmed by
measurement:** all seven chip variants *and* the bare base measure exactly **19.9375 px**, so the
transparent base border really does hold outer height.

**MUST-FIX — `.forge-connect-link` (`forge-pr.css:592`) lost its RESTING distinction.** It is an
inline `<a>` in a `--text-2` `<p>` with `text-decoration: none` at rest. Because `--accent-strong` is
tuned to sit *near* `--text-2`, the swap drove link-vs-prose luminance **1.43 → 1.02** dark and
**1.72 → 1.28** light; WCAG G183 wants ≥3:1 when colour is the only resting carrier. **The designer
attributed this to its own contract** — B6 claimed "the underline is the second carrier and stays",
which was wrong about the resting state, while the implementer's code comment ("hover underline") was
accurate. Fix is `text-decoration: underline` at rest; routed to the in-flight senior-dev.

**Both deviations ACCEPTED, and both traced to contract gaps rather than implementer licence.**
(1) The six-chip expansion was **mandatory**: `ProfileActivateDialog.tsx:225-229` renders "new file"
beside "changed" and "unchanged" *in one row*, so fixing 4 of 6 would have shipped two label
treatments inside a single line of a single dialog. The miss was the contract keying its enumeration
on `color: var(--accent)`, so C3b was hand-added rather than swept. (2) The added hover rule closed a
real gap — §3.6 claimed per-call-site hover coverage but never walked the base component's cascade.
Both are now generalised into `ui-reference.md` §2 as **the specificity trap**, sibling to the
child-rule trap.

**The methodological correction behind the wrong count is the durable part.** §2's "6 live instances"
is replaced by a **16-row table carrying `file:line`, selector, hue and tint percentage**, plus a
note on `.checks-rollup-pill--pending` / `.graph-filter-chip-stale`: **descendant-selector cases
where the ink and the tint live in different rules — which a same-rule-block grep can never find.**
That is why the count was wrong, and it is the search that P107 must actually run. The failed-claim
tally in §2 is now **FIVE**, the fifth being that file's own figure, and the "no AA colour shortfall
remains" line is replaced with an explicit prohibition on ever writing it again.

### Review round 1 — both reviewers say **request changes**, one MUST-FIX each

**Code review MUST-FIX — the D4 fix was DEAD CSS, and this is the finding that justifies the whole
method.** `.diff-float-discard` is a `<button>` inside `.diff-stage-float`, so the generic descendant
rule `.diff-stage-float button` **(0,1,1)** out-specifies the modifier **(0,1,0)**: `--accent` beat
`--danger`, and `--accent-text` beat the new `--danger-text`. The hover was *newly* dead — the old
`filter` did not compete (different property), whereas `background` does. **The destructive discard
button was rendering in accent blue with no danger hue at all**, while passing every AC grep. The
contract had recorded D4 as "no visual change — ink on a `--danger` fill," which is not what rendered.
**Heuristic worth keeping: a contrast fix that is out-specified by an existing rule is a no-op that
still passes a grep.** Routed to senior-dev with the fix (`.diff-stage-float button.diff-float-discard`)
plus a requirement to prove it in the harness, in both themes.

**Design-review findings routed with it:** the "last colour literal" claim is **false** —
`src/graph/forgeBadges.ts:22` still holds `PR_MERGED_COLOR = '#8957e5'`, and light theme now visibly
**diverges** (CSS pill `#8250df` vs canvas badge `#8957e5`, same concept, same screen). And the C5
comment claims "matching specificity" when the selector is (0,4,0), not (0,3,0) — it works *only*
because it is strictly higher, so the comment as written invites a silent revert.

**Open question sent back for evidence, not guessed at:** `.settings-toggle-btn` may never render
with `is-active` at all (all 29 call sites look static). If so, C5's recipe change **and** the added
hover rule are unreachable — and the more interesting possibility is that the settings toggles never
indicate their active state to the user, which would be a **product bug, not a CSS one**. Asked for a
verdict with evidence before anything is changed.

**Confirmed good by independent recomputation** (worth recording, since the claims were the point):
all five tokens exist in **both** themes — no silent dark-value inheritance; all seven residue counts
reproduce exactly; the 7 `--accent` survivors are genuinely glyphs with a named non-colour carrier;
`--accent-strong` worst case **4.93** dark / **4.87** light really is surface-independent. Deviation 1
(6 chips not 4) **upheld** — `.asset-chip-drifted` is the *most* widely rendered chip in the app, so
fixing 4 of 6 would have left the worst-exposed one broken. Deviation 2's specificity claim is real.

**AC16 partial / AC17 half-open, stated plainly rather than rounded up:** AC16's stylelint clause is
**unsatisfiable — no stylelint exists in this repo**; tsc, build, eslint and 235 targeted tests pass.
AC17's fixtures are added and verified, but the implementing agent had **no browser tooling**, so the
visual half went to the ui-designer review pass. Branch: `feat/p91-observability`
(USER decision 2026-09-02 — everything this session lands on that one branch; no push, local
commits only).

**The enumeration overturned the filing rather than confirming it — this is the method working.**
Two seed-list facts were simply wrong: `controls.css:70` is **`.pill-detached`**, not `.btn-danger`
(the real one is `updates.css:109`); and a **third** site the seed list never named,
`.pr-state-pill`, puts **one `color: #ffffff` over three different fills** — including white on the
dark `--success` fill at **2.85:1**, worse than the P102 seed defect and below even the 3:1 graphics
bar. Had this milestone "fixed the two known sites", it would have shipped and left the worst
instance in place. Same story on P105: the loose `var(--accent)` grep returns 140 hits in 34 files,
but only **30** are `color:` declarations — 110 are legitimate graphics-bar uses, and of the 30,
**7 are glyphs that correctly stay** at 3:1. A blanket sweep would have wrecked those.

**Counts:** P105 — 30 `color: var(--accent)` declarations in 16 files → 7 KEEP (glyph, 3:1, each
with a named non-colour carrier), 18 → `--accent-strong`, 5 recipe changes, +1 scope addition
(`.asset-chip-active`). 21 of 30 measurably sub-AA. P102 — 3 hardcoded-ink declarations + 4
`background: var(--danger)` fills.

**Tokens:** `--accent-strong` `#7fabff` dark / `#2a5cbe` light — hue-preserving, and deliberately
**surface-independent** (clears 4.5:1 on `--bg-0`…`--bg-3`, `--selection` and an accent tint in both
themes; worst case 4.87). That independence is what makes the post-fix check a single grep instead
of a per-site ancestor argument. Plus `--danger-text` / `--success-text`, all `#16181d` dark /
`#ffffff` light, mirroring `--accent-text`.

**ORCHESTRATOR DECISION (§5.4, Option A).** The merged-PR purple `#8957e5` **passes** contrast
(4.61 both themes) so it is not a P102 defect, but it is the last theme-invariant hue literal in
`src/styles/` and it sits inside the rule block P102 must rewrite. Chose **Option A** — add
`--merged` / `--merged-text` — so AC7 is a clean zero-count and the light theme gets a purple tuned
for a white page rather than a dark-theme value reused. Taken as a token-consistency call inside the
design system's own logic, **not** a product-identity call; it therefore did not need the user and is
not a checkpoint. Recorded because the contract explicitly forbade the implementer choosing silently.

**Scope calls.** IN, on measured merit: three of P100's four `filter: brightness` residuals —
`.btn-danger:hover` (4.17:1 light), `.diff-float-discard:hover` (4.32:1 light),
`.diff-stage-float button:hover` (the `.btn-primary` 3.95 case). OUT: the fourth,
`.settings-switch-track`, measures 3.80:1 **with no ink on it** — compliant, stays a NIT.

**AC18/AC19/AC20 are USER CHECKPOINTs and remain PENDING** — the updater panel's `.btn-danger` is
Tauri-only and invisible in the harness; whether `--accent-strong` still reads as *the Bonsai blue*
and how hover feels after the `filter`→`background` swap are both perceptual. The user's 2026-09-02
checkpoint authority was scoped to P100 + P101 and does **not** reach these.

**Why the two are one milestone.** Both are "a hue token used against an insufficiently contrasting
surface", and both want the same enumerate-then-bucket method. Kept as two clearly separated
sections inside one contract so implementation and review can still be staged. This is the USER's
call, recorded here because P100 §6-C deliberately kept one defect class per milestone — the
exception is that these two share a *method*, not just a symptom.

**No architect pass.** Consistent with P100 and P101, which both ran on a ui-designer contract
alone: a CSS contrast audit has no module boundary, IPC surface, or algorithm for the architect to
design. Recorded so a future session does not read the skipped step as an oversight.

**Scope in one line each.**
- **P105 (🚨)** — enumerate every `color: var(--accent)` call site, resolve each one's composited
  backdrop per state, bucket text (4.5:1) vs glyph/border/bar (3:1), and correct the false
  `ui-reference.md` §2 sentence claiming `--accent` is fine on `--bg-0`/`--bg-1`/`--bg-2`. It is not:
  `--bg-1` fails in light (4.34) and `--bg-2` fails in both themes (4.48 dark / 4.00 light).
- **P102 (📋)** — sweep every `#fff`/`#ffffff` on a `var(--danger)` fill (not just the two known
  sites), and introduce a `--danger-text` token mirroring `--accent-text`'s per-theme split. The
  remedy already ships in-repo at `partial-staging.css:104` (the `--bg-0`-ink flip, 4.80:1 dark).

**The standing lesson this milestone must honour.** Three app-wide claims in this programme have now
failed on inspection — P95's enabled-control class (3 escapes found by P101), P98's "`--text-3`
family closed" (122 declarations never classified), and P74's hue-as-text sweep (this milestone).
**A bucket + verdict per call site, P101 §3 style, or it is not closed.** Do not accept another
"~30 call sites and it's fine" sentence as evidence.

**Acceptance criteria:** owned by the contract (AC1..ACn), not restated here. Any AC needing the
native window is a **USER CHECKPOINT and stays pending** — the user's 2026-09-02 checkpoint
authority was scoped to P100 + P101 only and explicitly does NOT extend to work created this
session.


---

## Part 38 — P106 — status-badge ink — full board narrative (verbatim). **Milestone stays LIVE on the board**: AC14/AC15 and the real-repo half of AC9 are pending USER CHECKPOINTs.

## ✅ P106 — status-badge ink — SHIPPED `10ce967`, ⏳ AWAITING USER CHECKPOINT (AC14/AC15 + AC9 real-repo half)

**8 render sites in 7 files, 6 ink declarations under verdict, 6 distinct composited backdrops.**
Buckets: **5 FIX**, **1 KEEP** (`.file-status-renamed` on `--accent-strong`, min 4.93/5.01),
**2 no-hue** (compliant by inheritance but inconsistent — the same status reads differently in
different surfaces).

**The letter is the SOLE status carrier at all 8 sites, so nothing is exempt.** Shape and position
are identical across statuses, no word in the row names the status, and 11px/600 is far below the
large-text threshold — and the badge does **not** scale with `--rp-row-font`, so this holds in both
densities. Two consequences recorded: a `Conflicts` section header does **not** excuse the `C`
letter, and the badge has no accessible name at all (→ P109).

**Per-state resolution earned its keep here more than anywhere prior:** `D` measures **4.41 at rest,
3.89 hovered, 3.05 selected** — *the worst figure is the state a user actually reads a diff in.* `M`
passes everywhere in dark and fails **4 of 6** backdrops in light.

**A FOURTH search pass was added over P107's three — imperative canvas rendering** (`rg "FileStatus"
src/graph` → 0) — explicitly because "the search couldn't see a whole class" is how **both** prior
counts in this programme went wrong. That is the lesson transferring rather than being re-learned,
and it is the reason to trust this count.

**`--warning-text` is explicitly NOT the answer, and would be actively wrong.** It resolves to
`--bg-0`, which as ink measures **1.19/1.16** on the staged tint, **1.09/1.07** on `--bg-1`,
**1.57/1.24** on `--selection`. The `--*-text` family is **fill-ink**. Hence three new ink-only
tokens mirroring `--accent-strong`: `--danger-strong`, `--success-strong`, `--warning-strong`
(family floor **5.18**, all four minima in a 4.93-5.94 band). **`--warning` as a letterform is now
measured for the first time in this design system**: dark is adequate on neutrals (5.03-7.92),
light is not (3.91-4.87, clearing `--bg-1` by 0.04). `--text-1`-for-everything was measured
(9.36/13.29) and rejected on the two-recipe rule.

**Orchestrator decisions:** (1) **D1 accepted** — hue-code the two colourless badges, kept isolated
as the deliberately droppable AC12 so it can be reverted alone. (2) **`--warning-strong` dark =
`#e3b341`** (family coherence) over `#d4a72c` (zero dark churn) — the designer's recommendation;
visual language is its call. (3) **AC13's `ui-reference.md` update lands AFTER implementation**, from
the shipped commit — this programme was already bitten once by the reference describing tokens as
"specified — not yet shipped" and going stale.

**15 ACs with a 10-row predicted residue table.** **AC14/AC15 + the real-repo half of AC9 are USER
CHECKPOINTs and stay PENDING.**

**✅ RESOLVED — and the answer is better than either contract having been wrong: it was a BASE
AMBIGUITY.** `--accent-strong` on a 14% accent tint measures **6.42/5.19 over `--bg-0`**,
**5.85/4.87 over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's). All three
reproduce exactly under one method. **The contracts never disagreed — neither stated its base**, and
the base alone accounts for **1.26** of dark-theme spread.

That is now a rule in `ui-reference.md` §2: **a contrast figure is meaningless without its
composited base; every ratio must name the ink, the tint, AND the base**, and a figure naming only
the tint is incomplete evidence that may not be used to close an AC.

**Shipped result (`10ce967`):** worst live badge anywhere is **5.18** — the `C` on a selected row in
light — exactly the predicted floor. Verified live, not derived: **11/11** recorded historical values
and the full **30/30** pre-fix matrix reproduced *before* any new number was trusted, then all 8
render sites reached in a real browser in both themes with the full ancestor stack composited.

**8 of 10 predicted residue rows matched exactly; the two misses were the CONTRACT'S BASELINES being
wrong, not the fix.** R2's recorded "now" was 50 but measured **53**; R10's was 0 but measured **12**
(7 hex literals quoted inside prose comments, 5 `var()` fallbacks, all in files P106 never touched).
Both *deltas* were exact, so the intent held — and **P108's real inventory is 48, not 45.**

**This is the third time grep-counting has bitten**, so it is now also a rule: an acceptance grep
counts **text**, so prose comments and `var()` fallbacks inflate it. The implementer had to
deliberately avoid literal strings in its own comments — without that care two counts would have read
11 and 2 instead of 8 and 1. A residue prediction must state whether it counts **declarations or raw
matches**, a baseline must be **measured against the real pre-fix tree** rather than inherited from a
prior contract, and when a grep and a prediction disagree the baseline is re-measured **before**
touching code. Related: a `file:line` prediction should state that the **count** is the criterion —
comments added by the fix itself shift the lines (216/227 → 220/231 here, count unchanged).


---

## Part 39 — P108 — hue-as-text on neutral surfaces — full board narrative (verbatim). **Milestone stays LIVE on the board**: AC12/AC13/AC14 are pending USER CHECKPOINTs and **AC11 is OWED/unverified**.

## ✅ P108 — hue-as-text on neutral surfaces — SHIPPED `42206fd`, ⏳ AC12/13/14 USER CHECKPOINT, ⚠ AC11 OWED

Contract `8027cef` → impl `42206fd`. **17 CSS files, no TS/TSX, no DOM change, no new tokens.**
All five residue rows matched, and **all five pre-fix baselines were re-measured in this tree rather
than inherited**: raw hue-as-text 54→**26**, `-strong` 5→**35**, `var(--warn)` 1→**0**, hex outside
the token file 12→**6**, `--badge-unknown` 3→**2**. Post-fix minimum anywhere is **5.10**
(`--danger-strong` on `--bg-3`, light), matching the predicted family minimum exactly.

**The count was 62, not the 48 handed over — the FIFTH wrong count in this programme.** 48 predated
`settings-dev.css` and ignored three `border-color` matches the pattern catches.

### Two defects a grep structurally could not find
- **`settings-legacy-sections.css:134` was `color: var(--warn, #b8860b)` and `--warn` is defined
  NOWHERE** — verified independently by grep *and* live
  (`getComputedStyle(:root).getPropertyValue('--warn')` is `""` in both themes). So it had **always**
  painted its hardcoded fallback: a theme-invariant literal at **3.25:1** in light. **A `var()`
  fallback masking a missing token is invisible to any hue-name search, because the hue name appears
  only in the fallback.** Fixed by pointing at a token that exists, with **no** fallback.
- **`commit-panel.css:98-100` served ONE declaration to TWO selectors**, icon half a compliant glyph,
  text half failing at **4.41 dark**. Split, so each is judged on what it actually is. Bucketing per
  *declaration* rather than per *selector* would have got both wrong.

### A third alias — and it makes P101 a SIXTH failed app-wide claim
**`--badge-unknown` is byte-identical to `--text-3`**, which is how a **2.96 light** glyph escaped
P101's *exhaustive* `--text-3` audit — a family recorded as **CLOSED**. Token aliasing has now hidden
instances three times (`--badge-good`/`--badge-warn` ≡ `--success`/`--danger` was the first pair).
**An audit scoped by token NAME cannot see an alias; scope by resolved VALUE.**

### The canvas pass found zero — and that is still evidence
7 draw sites in `src/graph/**`, all glyph, all ≥3:1 on both canvas backdrops. **0 fixes, but only
because it was measured.** Assuming a pass would find nothing is precisely how the earlier counts
went wrong.

### Two corrections to the orchestrator's brief, both caught by the implementer
1. I **accepted D1** but then quoted the **D1-rejected** residue arithmetic (29/32). The accepted
   figures are **26/35**, which is what shipped. Flagged rather than silently resolved.
2. I wrote "leave `.op-worktree-warning` **exactly as-is**" when my reasoning was only about D3's
   **tone** change (repaint danger→warning). The contrast fix keeps the danger hue and changes only
   legibility, so it belongs in scope — and the contract's own count of 18 `--danger-strong`
   includes it. Over-broad phrasing on my part.

### ⚠ AC11 is OWED — recorded, not quietly dropped
Two states could not be reached: `.file-count-del` selected (3.05) and
`.context-menu-item[data-tone='danger']` hovered (3.05). **The orchestrator also tried and failed**,
across ~8 harness rounds: keyboard nav focuses `.graph-scroll` but never mounts the diff panes;
`?forge=auth` does not render `.file-count-*`; and synthetic `contextmenu` events do not open the
menu because React requires **trusted** input. Both figures are source-derived and unverified.
Recording it as owed rather than manufacturing a pass — the same call the implementing agent made,
and the standard this programme applies to its agents applies to the orchestrator too.

### Also flagged
- **`--warning-text` is undefined** (only `--accent-text`/`--danger-text`/`--success-text` exist), so
  a naive probe of it returns the **inherited** colour (13.54/15.42), not the 1.09/1.07 the contract
  cites. **Same undefined-property trap class as `--warn`** — worth knowing before anyone re-measures
  that row.
- Contract §3.1 records `--badge-unknown` on `--bg-0` as 3.68; measured **3.67**. No verdict changes.
- **D3 `.op-worktree-warning` is a warning painted danger** — a *tone/semantics* question, not a
  contrast one. Deliberately **not** folded in: changing it alters what the UI means, not whether it
  can be read. Filed for the user.
- §9's four mock fixture states were **not** added: three are already reachable (the mock maps verify
  status off the oid's first nibble), only the long-error and `dev-status-write-failed` states would
  need new mock code.


---

## Part 40 — P107 — hue-over-own-tint — full board narrative (verbatim, header included). **Recorded as it stood**: the header says "CONTRACT DONE, IMPL PENDING" although `2168057` shipped the implementation — the contradiction is flagged live on the board, not resolved here.

## 📋 P107 — hue-over-own-tint: 38 call sites (§2 had claimed 6) — CONTRACT DONE, IMPL PENDING

**The fourth app-wide claim in this programme to fail on inspection**, after P95's enabled-control
class (3 escapes found by P101), P98's "`--text-3` family closed" (122 declarations never
classified), and P74's hue-as-text sweep (which became P105). `ui-reference.md` §2 asserted **6**
live hue-over-own-tint instances. A full scan during the P102/P105 implementation found **16**
outside that milestone's scope:

`.pr-mergeable-clean` / `-conflict` / `-pending`, `.asset-badge-ok` / `-warn`,
`.danger-badge.safe` / `.caution` / `.destructive`, `.asset-readonly-banner`,
`.asset-issue-error` / `-warning`, `.conflict-kind`, `.error-banner`, `.graph-truncated-banner`,
`.settings-ai-status-warn`, `.wt-copy-chip`.

**CONTRACT DELIVERED 2026-09-02 — `docs/contracts/P107-hue-over-own-tint-ui.md` (`59061b2`), and
the population is 38, not 16.** The brief required a *broader* search rather than a reconfirmation,
on the grounds that a count agreeing with the wrong count is not evidence. It moved:

- **Pass (i)** same-rule-block — the search that produced the wrong 6.
- **Pass (ii)** a multiline descendant-combinator grep — recovers cases where ink and tint live in
  different rules (the `.pr-state-open` pattern).
- **Pass (iii)** two classes even (ii) cannot see, and this is the durable lesson: **unqualified
  child classes whose only parent is tinted** (found by *reading the 25 tinted containers'
  components*, not by grepping), and **custom-property indirection via `--h`** — 11 instances across
  4 families that **no hue-name grep can ever reach**. A fifth family was hidden by **token
  aliasing**: `--badge-good`/`--badge-warn` are byte-identical to `--success`/`--danger`.

**Buckets: 17 failing text · 19 compliant glyph KEEPS · 1 failing glyph state · 1 owned by P106.**
The 19 keeps matter as much as the fixes — a blanket sweep would have wrecked the toast, submodule,
ai-dock and git-dock pill recipes. The single glyph-state failure, `.error-dismiss:hover` at
**2.96 light**, is **compliant at rest** and surfaced only because the contract resolves each call
site's backdrop *per state*.

**Zero new tokens**, and `--warning-text` is explicitly **not needed** — no non-compliant
solid-warning fill exists, and `--bg-0` already resolves to exactly what such a token would carry.
**Recorded trap: `--danger-text` on a 14% danger tint is 1.27:1 — the `--*-text` tokens are
fill-inks and must never be put on a tint.**

**A `ui-reference.md` §2 recipe was retracted, not patched:** it offered "a 35% border **or** a
leading bar/glyph at the 3:1 bar". A 35% hue edge measures **1.58/1.69** — decoration, never an
identity carrier. Solid-hue bars do pass (4.41-7.92) and stay.

**13 ACs with a predicted post-fix residue** (`--danger` 35→27, `--success` 12→9, `--warning` 24→17,
`--text-1` 159→171, `--h:` unchanged at 15, 9 files, 0 outside `src/styles`, 0 `.tsx`).
**AC11/AC12/AC13 are USER CHECKPOINT and stay PENDING.** Implementation NOT started; it edits 9
files under `src/styles/`, so it must not overlap another CSS pass.

**Method note worth carrying forward:** measurement was calibrated by reproducing six independently
recorded historical values *before* trusting any new number, and a `color(srgb …)` vs `rgba()`
serialization trap invalidated the first run outright. Both are recorded in the contract.

**The pattern is now established well enough to state as a rule.** Every one of the four failures
had the same shape: a sentence claiming an app-wide property, with a call-site count that nobody
enumerated. P101 §3 is the template — enumerate, bucket, record a verdict per site, predict the
post-fix residue, then verify the prediction. **Do not accept a "~N call sites and it's fine"
sentence as evidence.** P102/P105 is the first milestone in the programme to hit its predicted
residue exactly on every metric; that is the standard P107 inherits.


---

## Part 41 — Superseded pre-ship filings and one resolved blocker (verbatim). Each was overtaken by a later board section; kept here so the reasoning that produced the milestone survives.


### P105 — the original filing (2026-09-01), superseded by the P102+P105 milestone

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


### P102 — the original filing (2026-09-01), superseded by the P102+P105 milestone

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


### P106 — the original filing (2026-09-02), superseded by the shipped P106 section

## 📋 P106 — the A/M/D/U/R letter badges are TEXT, not glyphs — PENDING (filed 2026-09-02 from the hue audit)

Filed by `ui-designer` during the P102/P105 enumeration, **with the measurements already taken**
(contract §3.5), so this follow-up starts from evidence rather than from a suspicion.

The status letter badge (`A`/`M`/`D`/`U`/`R`) is rendered as a **character**, which puts it at the
**4.5:1 read-text bar**, not the 3:1 graphics bar it is currently judged against. Its
`--danger` / `--success` / `--warning` backings therefore need the same ink-flip treatment P102
applies to the button fills — and `--warning` in particular has not been measured against a
letterform anywhere yet.

Same defect class as P102/P105, deliberately **not** folded in: this milestone already spans two
hue tokens and 30+ call sites, and one defect class per milestone is what kept P95/P98/P100
reviewable. The `--danger-text` / `--success-text` tokens P102 introduces are the prerequisite, so
P106 should be cheap once this lands.

### Filed alongside, not fixed (all from the same enumeration)
- **`.wt-copy-chip`** — a sixth hue-over-own-tint instance, found after the contract's bucket table
  was closed.
- **`.settings-switch-track` NIT** — 3.80:1, but carries no ink, so it is compliant; recorded only
  so a future sweep does not "fix" a non-defect.
- **`src/styles/forge-pr.css` is ~710 lines** and over the ~500-line soft limit. This milestone adds
  no new rule block there, so the split is **not** in scope — hand it to `refactorer` as a
  standalone behavior-preserving pass.


### P106 — backdrops measured for it by P107 (2026-09-02), superseded by the shipped P106 section

## 📋 P106 — backdrops now measured for it by P107 (2026-09-02)

P107 found **exactly one** overlap and **left it to P106** rather than both contracts claiming it.
The hand-off is the set of backdrops P106 had not measured for `.file-status-deleted .file-badge`:

| backdrop | dark | light |
|---|---|---|
| `--bg-1` | 4.41 | 4.60 |
| `--bg-2` hover | **3.89** | **4.24** |
| `--selection` | **3.05** | **3.96** |

And the *added* badge sits on `.status-section--staged`'s 6% `--success` tint at **5.26 / 4.38** —
**success ink on a success tint, failing in light at rest.** P107 does not touch `status-panel.css`.


### P106 — priority RAISED (2026-09-02), superseded by the shipped P106 section

## 📋 P106 — priority RAISED (2026-09-02, on measurement)

Filed earlier from `--danger`/`--success`/`--warning` letter badges being *text* at the 4.5:1 bar.
The design review then measured `.file-status-deleted .file-badge` at **4.41 dark on `--bg-1`** —
**it fails at rest, not merely on hover**, which is worse than §3.5 implied when P106 was filed.
Treat as the next hue item after P107.

### Also filed from the design review (non-blocking)
- The 90-char branch-name chip becomes a 50 px two-line stadium at `border-radius: 999px`.
  Pre-existing; newly *visible* because the milestone added the fixture that reaches it.
- `ui-reference.md` is growing fast (§2 now carries a 16-row evidence table). Worth a
  `docs-curator` pass; `TODO.md` is also past 1400 lines against its ~300-line target.


### P108 — PROPOSED, not enumerated (2026-09-02), superseded by the shipped P108 section

## 📋 P108 — hue-as-text over NEUTRAL surfaces — PROPOSED, not enumerated (filed 2026-09-02)

Proposed by `ui-designer` while enumerating P107, and deliberately filed **without** a count or a
closure claim — which is the programme's method working, since five prior claims failed by asserting
app-wide scope before enumerating.

Distinct from P105 (which covered `color: var(--accent)` specifically) and from P107 (hue text over
its *own* tint): this is any hue token used as text over a **neutral** `--bg-*` surface.

Seed observation, and it is pointed: **`.dev-status-write-failed .dev-status-state` measures
4.41 dark** — that is P91 §8.4's own **"▲ Not writing"** string, the indicator whose entire job is to
stay legible during a disk failure. Do not treat the seed as the scope; run the three-pass search
P107 established, including the `--h` indirection and the tinted-parent child classes no hue-name
grep reaches.


### DEAD CSS `.settings-toggle-btn.is-active` — DECISION NEEDED (2026-09-02). Superseded by the investigation that CLOSED it as (b), dead styling, NOT a product bug (board line of 2026-09-02, retained live).

## 🧭 DEAD CSS — `.settings-toggle-btn.is-active` matches nothing — DECISION NEEDED (found 2026-09-02)

Found independently by **both** the code reviewer and the designer, which is why it is recorded as
established rather than suspected: no component composes `is-active` onto `.settings-toggle-btn`.
All 29 call sites are static `className="btn-secondary settings-toggle-btn"`. The `is-active`
consumers are `.conflict-editor-mode-btn`, `.search-toggle` and `.command-palette-option` only.

So P105's C5 recipe change **and** the hover rule added in `0e5dcab` are both **correct and both
currently dead**, and AC6/AC17 cannot be closed for C5 by observation.

**Two dispositions, and they are not equivalent:**
- **(a) Wire it up** — `ui-designer` recommends this: several `settings-toggle-btn` call sites
  (`ProfileManager`, `SettingsMcpSection`) are segmented controls that clearly *want* a selected
  state. If so, **the real defect is that those toggles never indicate their state to the user** —
  a product bug, not a CSS one.
- **(b) Delete both rules** as dead styling.

**Held for the user**: (a) changes what the app does and is not a contrast fix, so it does not
belong inside this milestone. The CSS stays (it is correct, and harmless while unmatched).


### GATE BLOCKER — `pnpm lint:size` (found 2026-09-02). Archived because the board itself records it cleared: `3df21f2` split `ai/session.rs` + `ai/session_tests.rs`, and the P102/P105 gate line reads "file-size ratchet 1.0s OK (the blocker is cleared)". No status was upgraded by the curator.

## 🔧 GATE BLOCKER — `pnpm lint:size` fails on files this branch grew — PENDING (found 2026-09-02)

**Independently confirmed by three agents**, the second by stashing its own CSS changes and
re-running to prove the failure was not its own. ⚠ **One agent misattributed this to "the concurrent
hue-audit/P91 work" and called it "the P107 blocker" — both wrong**, and recorded here so the error
does not propagate: it predates today's session entirely, and P107 is the hue-over-own-tint
enumeration, a different item. The cause is: `crates/bonsai-core/src/ai/session.rs` (505 → **538**) and
`crates/bonsai-core/src/ai/session_tests.rs` (509 → **540**) breach the size ratchet. Both grew in
commit **`734b310`** ("test(ai): drive the session watchdog from an injectable clock, not wall
time") **on this branch**, and the baseline was never updated.

**The fix is a split, not a baseline bump.** Per CLAUDE.md the ratchet is a deliberate work queue,
so raising the baseline would discard the signal rather than answer it. Route to `refactorer` as a
strictly behavior-preserving pass (identical before/after test counts). Held until the P104 e2e
investigation finishes, because `refactorer` proves equivalence by running tests and P104 is
measuring wall-clock timings on the same machine.

**This blocks the full `pnpm gate`**, so it must land before the hue-audit milestone's step-7 gate
can be called green.


---

## Part 42 — P91 SECURITY ARC — code + contract complete (2026-09-03) — full board narrative (verbatim). **P91 itself stays LIVE on the board** (its milestone USER CHECKPOINT has never been presented).

## ✅ P91 SECURITY ARC — CODE + CONTRACT COMPLETE (2026-09-03)

**Full `pnpm gate` at `b26833f`:** all 7 non-e2e steps green (nextest 126.0s, doctests 3.0s,
clippy 9.8s, eslint 10.6s, **file-size ratchet 0.77s**, vitest 58.1s, tsc+build 10.5s). e2e was 179 passed / 1 skipped / **2 failed**; both were diagnosed as **not regressions**, fixed
in `23ee2b0`, and the suite is now **181 passed / 1 skipped / 0 failed**.
`cargo obs::` went **135 → 146 → 158 → 162** across the arc.

### Commits
`a287a53` audit → `834f2d1` ruling + raw-args contract → `120cadd` F2-F5 → `c0abbe1` F1 →
`2523426` review + re-audit follow-ups → `2fb647e` ratification → `0a785b3` copy contract →
`b26833f` copy implementation.

### Three binding rules are now IN THE CONTRACT, not just in this session's memory

Each was learned by a bug that was green the whole time it was broken:

1. **A writer rule is covered only by a `LogRecord` → `append_record` → read-back round-trip.**
   Synthetic-`Value` tests are additive, never substitutive. *This is what let W6 ship dead in
   production while its test passed* — it was the one rule of six that skipped the round-trip.
2. **A validator's tests must use inputs the REAL producer emits**, and a cross-boundary vocabulary
   must be pinned by a drift test that re-derives it from the producer's own source. *A predicate
   that rejects everything is indistinguishable from one that works, unless something asserts a real
   input is ACCEPTED.* This is the `cmd.*` camelCase bug.
3. **A negative test must be PROVEN to fail on the unfixed code.** Saying "this must fail on today's
   code" is not proof. AC6 as originally written specified a payload the scrubber **already caught**,
   so it would have been green on the buggy code — a negative test that could not go red.

### The writer's limit, recorded honestly in both contracts
`raw_args.rs` enforces **shape + vocabulary, not semantics**. Content under an identifier-named key,
≤512 chars, single-line, **survives**. Its only defence is the **positional drift guard** in
`rawArgPolicy.test.ts` (a reordered signature relabelling a `message` as a `targetOid` is
structurally invisible to the writer). **Neither guard may be removed citing the other.**

### Architect decision worth not re-opening: NO global cap on metric keys
Worst case is genuinely per-map-per-bucket, ~**616k keys**, and that is accepted rather than glossed.
A global cap is **rejected** because it would make *today's* recording depend on *history*: a
long-lived install would stop minting keys and dump current activity into `meta.overflow`,
destroying the week-over-week comparison §8.1 exists to serve. 512 sits above a ~212-key reachable
set, so it is a **runaway stop, not a sizing parameter**. File-size pressure has a different lever
(size-triggered early roll-up), recorded as a **revisit trigger only, ~8 MB — no work now.**

### ✅ GATE FULLY GREEN at `23ee2b0` — e2e 181 passed / 1 skipped / **0 failed**

Both failures below are **fixed** (`23ee2b0`), and the fold diagnosis is worth keeping because the
first reading was wrong in an instructive way.

**The fold spec was never a timing-tolerance problem.** The graph extent has **two independent async
inputs** — the graph stream and the first working-dir status round — and `openRepo` waits for
**neither**: it waits only for the canvas to become *visible*, which the pane renders before any data
lands. The WIP display row is derived from status, so a baseline read too early is **exactly one row
short**, and every assertion derived from it inherits the error. The arithmetic pins it: expected
`456 = 1064 − 19×32` where 1064 is **33 rows without WIP**; received `488 = 1096 − 19×32` where 1096
is **34 rows with WIP**. **The folded measurement was correct all along — the baseline was wrong.**
That is why the failing line wandered across 75/95/137/178 and why it passed on an idle box.

Fixed by waiting on the two real conditions, **not** by raising a timeout — it was never a timeout.
Returning both measurements from one sample also closed a second latent inconsistency: the WIP offset
and the full height were separate reads, so status landing *between* them yielded `wip=0` alongside a
WIP-inclusive height.

**The date-locator defect was a class, not an instance.** `getByText('1/2')` matched a commit date
rendering as `1d · 9/1/2026`, because **`9/1/2026` contains `1/2`**. Note `{ exact: true }` would
**not** have fixed it: `getByText` compares against each *candidate element's* text, and it is the
date element's text that contains the counter — exactness on the needle never changes the candidate
set. The sweep found **three more** instances, each one date away from firing, including two in
`09-search-palette.spec.ts` that nobody had flagged.

Verified by repetition rather than one pass: **5/5** at `--workers=1`, **5/5** at `--workers=4` (how
the gate runs it), **48/48** under `--repeat-each=3` contention stress.

### Original triage (kept — the reasoning is the point)
- **`30-graph-rail.spec.ts:56` — a latent DATE-dependent test bug.** `getByText('1/2')` resolved to
  two elements: the search counter **and** a commit date rendering as `1d · 9/1/2026`, because
  **`9/1/2026` contains the substring `1/2`**. The clock rolling to 2026-09-03 mid-session exposed
  it. It passes in isolation again now, which is precisely what makes it dangerous — it will recur
  and look like a flake.
- **`29-graph-fold.spec.ts` — an unstable spec, PRE-EXISTING.** Verified, not assumed: it failed a
  **different** test in isolation (95/137/178) than in the gate (75), and **still fails with today's
  CSS reverted**. The delta is consistently **32px = exactly one `DEFAULT_ROW_HEIGHT`**, which says
  the poll is racing the graph's async paint settle. Today's only graph CSS change was `color` +
  `box-shadow: inset`, neither of which participates in layout.
- Both routed to `tester` as genuine test-quality defects rather than annotated as flakes.

### Still open from the arc
- **P91 SHOULD-FIX pair** (unchanged): the `SAVE_LOCK` snapshot race that lets `metrics_reset` be
  silently undone on disk; the `last_fire` prune's unstated monotonic-`ts` premise.
- **`P91-observability-ui.md:496` and `:951` are stale** — both still describe
  `log_export_session(dest)` and a native save dialog **that never existed**. → `ui-designer`.
- **Contract consolidation (architect recommendation, not yet done):** fold
  `P91-raw-args-privacy.md` into `P91-observability.md` (≈ −250 active lines) when `docs-curator`
  archives §13 rows 1-15; keep `P91-privacy-copy-ui.md` standalone since it is `ui-designer`-owned.
- **NEW, found while implementing the copy: this repo has NO prettier config** — no `.prettierrc*`,
  no `prettier.config.*`, no `package.json` key, no `.editorconfig`. Running prettier therefore falls
  back to its defaults and rewrites whole files (74 insertions for a ~20-line edit, double quotes at
  80 columns against the repo's ~100). **Do not run prettier here** until a config exists.


---

## Part 43 — SECURITY AUDIT of the P91 surface — 2026-09-02 — full text (verbatim), F1 through F9. **F6 (`usage.json` disclosure) and the home-directory/username masking question stay LIVE on the board as open user decisions.**

## 🔐 SECURITY AUDIT of the P91 surface — 2026-09-02 — **F1 IS A SHIP BLOCKER**

Run deliberately *now* because **P91 has never shipped** (absent from `dev`): this is the last point
at which a privacy defect can be fixed before it starts writing durable files on real users' disks.
No user is affected today and there is no migration problem.

### 🚨 F1 — HIGH, reachable today. Raw mode writes commit messages, search text and forge tokens to disk — while the consent dialog promises it does not

`src/obs/ipcProxy.ts:106-123` serialises **every positional argument verbatim** in raw mode. So with
Dev mode + "Include raw repository names" on, these land in `logs/*.jsonl`:
`commit(repoId, message, …)` → **the full commit message**; `searchCommits(repoId, query)` → **the
search text**; `forgeSetToken` / `forgeAddAccount` / `forgeSetTokenForHost` → **a forge PAT in
cleartext**.

**The credential scrubber cannot save it, for a structural reason worth remembering:**
`scrub_value` (`scrub.rs:345`) applies `is_sensitive_key` to **JSON object keys**, but raw `args` is
keyed **positionally** (`"0"`, `"1"`, …) — so no key ever matches `token|secret|password|auth`.
Survival then rests on shape alone, and `looks_like_opaque_secret` (`scrub.rs:106`) requires
length >= 32 **and explicitly exempts all-hex words** (a deliberate guard for commit SHAs). Net
effect: GitHub `ghp_`/`github_pat_` and GitLab `glpat-` are caught by prefix, but a **40-hex
Gitea/Forgejo token and a 20-char Bitbucket app password pass through unredacted.**

**What makes this a blocker rather than a bug:** the consent dialog
(`DevConfirmDialogs.tsx:39-42`) states, at the moment of consent, that *"Commit messages, file
contents, author names and email addresses are still never written, and passwords and access tokens
are never written in any mode."* And the designed workflow is enable → reproduce → **export the zip
and send it to a maintainer.** The product would be telling the user something untrue precisely when
they are deciding whether to trust it.

**Root cause is the contract, not just the code.** Line 910 says args are "included as `args`" in
raw; line 914 says commit messages and search queries are "never, in either mode"; line 918 says
tokens are "NEVER, under any setting". Those cannot all hold, and **the implementation resolved the
conflict in the leaking direction.**

**✅ RULED + CONTRACT DELIVERED `834f2d1` — `docs/contracts/P91-raw-args-privacy.md`.** Line 910 was
the defect; the two absolute "never" rows stand. **One rule: raw mode widens IDENTIFIER fidelity
(repo path, file paths, ref names, remote URLs, full SHAs) and never CONTENT fidelity.** Free text
and credentials are outside both modes, permanently. Grounds: two absolute "never"s outrank one
mechanism description that never mentions them; consent is bounded by what the dialog promised at
the moment of consent; §7.1 already defines raw's purpose as real *names*; and the failure is
unrecoverable in **one direction only** — a PAT in a mailed zip — versus mere reviewer legibility in
the other.

**Mechanism:** sparse per-command allow-list at `src/obs/rawArgPolicy.json`, **default DENY**, keyed
by parameter **name** rather than position, **scalars only**. Name-keying is what makes
`is_sensitive_key` meaningful inside `args` for the first time; scalars-only kills
`searchCommits(_, query: SearchQuery)` by shape alone. `looks_like_opaque_secret` and the hex
exemption are deliberately **unchanged** — §13 row 19 ratified them and they are load-bearing for
SHAs.

**The strongest part of the design, worth not eroding later:** the writer-side check
(`obs/raw_args.rs`) **deliberately does NOT consult the allow-list.** A shared table would be
worthless against the failure that actually matters — a wrong row, or code that ignores the table,
which is precisely today's bug. It enforces a shape+vocabulary invariant it can decide alone, and
**its key-shape rule (`^[a-z][A-Za-z0-9]*$`) kills the current leak even if the producer is never
fixed**, because today's leak is keyed `"0"`/`"1"`. Violations drop the **whole** `args` object and
set a writer-set `argsPolicyViolation` that a producer cannot forge.

**AC6 is the negative test**, and it is well chosen: it plants a 40-hex token *specifically because
that shape passes the hex exemption*, so only the key rule can catch it. It must fail on current
branch code.

**Orchestrator decision on the architect's escalation:** ACCEPTED — add the path-/ref-taking command
rows under the §B.2 derivation rule **in the same increment**. The six seed rows mostly document
denials, so raw mode would currently yield almost no argument values, and *a raw mode that shows
nothing invites someone to "fix" it by reverting to a blanket include.* That is the failure mode this
whole ruling exists to prevent.

**Still owed:** implementation (held until the F2-F5 agent releases `writer.rs`), and a
`ui-designer` pass so the consent copy is not merely true but **complete** — it should name search
terms and other free-text arguments, and describe raw as "real repository, file, branch and remote
names" rather than "arguments". AC12 gates that before the USER CHECKPOINT.

### F2 — MEDIUM, **latent**. The `bump_counter` guard is a `debug_assert`, compiled out of release

The audit's answer to the question posed as highest-consequence: **the release path does NOT drop an
invalid key — it records it verbatim.** There is no `[profile.release]` override, and
`is_valid_counter_key` is referenced *only* from that assert, so in a shipped binary the function has
no effect whatsoever.

**Not reachable today** — `fold_perf` inlines its own `PERF_KEYS` loop and there is **no production
caller at all**. The real hazard is the doc comment claiming "today: `fold_perf` and tests", which is
**false** and invites the next developer to believe a validated caller exists. Combined with a guard
that silently evaporates in release, the first person to wire this to anything repo-derived writes
permanent, un-deletable content to a user's disk **with no test failing.** The sibling validators
`is_valid_cmd_name` and `is_valid_err_code` are already real `if` guards — this is the odd one out.

### ✅ F2-F5 FIXED + COMMITTED `120cadd` (2026-09-02). F1 implementation in flight.

`cargo obs::` **146 passed** (was 135), clippy clean, tsc clean, vitest 364 passed, `lint:size` OK.

**🐞 A latent bug found en route, and it is the most instructive thing in this batch.** `cmd` is the
**frontend** method name (camelCase, taken from `ipcProxy`'s `prop`), but `is_valid_cmd_name`
**rejected uppercase** — so **the entire `cmd.*` histogram family recorded NOTHING in production.**
Only the snake_case names used in tests ever passed the predicate. Every test was green and agreed
with a feature that did not work at all, because the tests and production supplied different-shaped
inputs to the same validator. Membership in the generated 199-name allow-list is now the authority
and the shape check merely bounds it. `usage.json` keys are camelCase from here; no migration
concern, since P91 has never shipped.

**One judgment call beyond the literal finding, flagged rather than buried:** `is_valid_counter_key`
now *requires* the `<domain>.<action>` dot. A bare lowercase token — `ghp_0123456789abcdef`, an oid,
an id — was otherwise shape-valid and would have been persisted the moment the F2 guard started
actually running in release.

**Regression coverage is written to fail on the defect, not merely to pass** — worth imitating:
the F2 test asserts the **drop**, so it fails whether the guard reverts to a `debug_assert` (panics
in debug) *or* vanishes entirely (records in release); the F5 guard **scans source** for raw
`File::create` / `create_dir_all` chains so it provides real coverage on Windows, where the 0600 bits
cannot be asserted; and a drift test pins the allow-list to the `IpcApi` interfaces as an exact
**bijection**, so adding a command without a decision fails the build.

**Contract follow-ups owed to `architect`** (batched with the F1 ratification, not yet done):
§6/§10 still specify `log_export_session(dest?)`, now removed; §8/§8.1 must record that `cmd.*` keys
are camelCase `IpcApi` names *and that they recorded nothing before this fix*; `MAX_KEYS_PER_MAP =
512` plus the `meta.overflow` bucket is a **new §8 surface needing ratification**; and decision 25's
counter-key shape now literally requires the dot.

### ✅ F1 FIXED + COMMITTED `c0abbe1` (2026-09-03) — ruling `834f2d1`, copy `0a785b3`

`cargo obs::` **158 passed** (135 → 146 → 158 across the three security commits). Independent
security re-audit and code review both in flight; **do not call F1 closed until they report** — a
security fix reviewed only by its author is not reviewed.

**A defect in the contract's OWN negative test, caught by the implementer.** AC6 as written specified
a `forgeSetToken` record keyed `{"repoId","token"}` — a form `scrub_value` **already caught before
the fix**. As specified, the test would never have gone red on the token half: it would have looked
like proof while proving nothing. The implementer kept it *and* added a third record in the form the
buggy producer actually emitted, `{"0","1"}` with a 40-hex token (that shape passes the hex
exemption, so **only the key rule can catch it**), then captured both leaking lines on disk before
wiring the fix. **A negative test that cannot fail proves nothing** — worth a §13 row.

**One forced deviation, flagged not buried:** `fullContext` (a boolean) had to be denied, because the
free-text vocabulary matches `text` inside `fullCon**text**`. Listing it would fail the drift test and
the writer would drop the whole `args` object anyway. Costs nothing — it is a boolean.

**118 allow-list rows**, derived by walking every signature in `ipc-api*.ts` and listing a row **only**
for commands taking a path/ref/branch/remote/tag/URL/SHA identifier. Commands whose args are only
`repoId` (+ booleans) are left unlisted, because raw mode would add no name and `argsShape` already
carries the arity. Several rows exist purely to **document a denial**.

### 🔎 NEW, found by the consent-copy pass — three things were undisclosed, one is a real gap

- **Nothing masks home directories.** `scrub.rs` has **no username rule** (verified independently by
  grep), so a **raw absolute repo path carries the OS account name** into a mailed export zip. Not a
  leak of repo content, but it is identifying, and the export workflow mails it to a third party.
  Now disclosed in the copy; **whether to also mask it is an open question** — masking would
  undercut raw mode's stated purpose of showing real paths, so this is a judgement call, not an
  obvious fix.
- **Remotes were missing** from the raw-mode list on both consent surfaces, and **full commit SHAs
  were disclosed nowhere.**
- **A third surface the brief never named:** `ExportConfirmDialog` carried its own, shorter "never"
  list — and it is the dialog shown at the **exact moment the zip is created**. One flow was
  presenting two different guarantees. Now respecified together.

**F6 (`usage.json` disclosure) — HELD FOR USER, with a recommendation.** Three options; the designer
recommends **A: disclose, no reset button**. Recorded plainly: the panel is headed "What a log file
contains" and ends with "removes every one of them", so **by omission it reads as though Dev mode is
the only thing recorded and the delete button clears it** — while `usage.json` is always-on, outside
`logs_delete_all`, outside exports, with 400-day buckets folded into a never-aged lifetime total.
Note the proposed remedy names the **`metrics` folder**, not the file: deleting `usage.json` alone
lets `usage.json.bak` restore it. Option B (a reset row) reverses ratified §10 and belongs on a
future Statistics page. **Until the user rules, only §2-§5 of the copy contract get implemented.**

### F3 — MEDIUM, reachable. Unbounded metric-key cardinality

`log_append` takes records straight from the webview, and `is_valid_cmd_name` is a **shape**
predicate rather than membership in the real command set; the maps have **no cardinality cap**. It
lands in a file `logs_delete_all` does not cover and only a headless `metrics_reset` can clear.

### F4 — MEDIUM, reachable from the webview. `log_export_session(dest)` is an arbitrary-path write

The code comment justifies taking `dest` verbatim because it "is the result of the OS save dialog".
**That dialog does not exist** — the only caller passes no argument. So it is an unmediated
arbitrary directory-create + file-write primitive, and it defeats the export-scope rule that exports
stay in `exports/` where delete-all covers them.

### F5 — MEDIUM. Default umask permissions

Log parts, export zips and `usage.json` are created with no `set_mode`; on Linux/macOS that is
typically `0644`, world-readable, for files carrying absolute repo paths and real branch names.

### F6 — LOW/MEDIUM, a **product** call for the user, not a security fix

`usage.json` is **always-on durable local telemetry**, independent of Dev mode, from first launch:
`firstSeen`, launch count, and a **400-day** per-day profile of which operations were performed and
how long they took. It survives `logs_delete_all` **by design**, `metrics_reset` has **no UI**, and
the privacy panel never mentions the file exists. Content is non-identifying by construction, so this
is a **disclosure** question, not a leak. **Since P91 has never shipped, now is the moment to decide
whether a local Git client should keep an undeletable 400-day usage profile with no disclosure.**
Held for the user.

### F7 — LOW, mostly latent. `redact_names` misses bare ref/file names and never touches JSON keys

A branch like `feature/acme-client-migration` has one separator, is unrooted and has no extension, so
`is_path_shaped` returns false and it would be written **verbatim into a strict file**. Not reachable
today — every free-form payload traced to a static allow-listed literal — but see F9:
`strict::enforce` is the **sole** enforcement point for both Rust and frontend records, so a gap here
is a single point of failure for the whole system.

### Verified CLEAN (recorded so a later session does not re-audit)

`react.ts` redaction is now safe **by construction**, not by disuse. The IPC dispatch shim cannot
alter, drop, reorder or convert a failure, and carries `cmd` + trace ids only. Error strings never
cross IPC. Zero-cost-when-off is **structural on both sides** — nothing collects-then-suppresses.
Log rotation and purge have no traversal. **CSP and capabilities are well hardened**: script-src is
self-only with no unsafe-inline or unsafe-eval, and there is no shell or fs plugin, so a renderer
compromise is not shell access. Updater trust chain sound; no key material in the repo.
External-process launching uses argument vectors, never a shell string. Forge credential storage via
the OS keychain is well built — **F1 is a leak *around* it, not a defect in it.** And
`no_proxy_client()`'s raw `.expect` sits in a `#[cfg(test)]` module that never compiles into the
shipped binary — **the long-standing DEP-REFRESH follow-up about it can be CLOSED.**

### F9 — INFO, and it reframes the "do the two redactors disagree?" question

They cannot disagree, because **only one enforces**: `redact.ts` has no equivalent of `redact_names`;
all name redaction for both sides happens once in `strict::enforce` on the writer thread. That is the
correct architecture — a frontend bug cannot write an unredacted name into a strict file — and it is
exactly why F7's gaps matter more than their current reachability suggests.

**Order of work:** F1 before merge (architect + senior-dev in progress) → F2 → F4 → F5 → F3.
F6 is the user's call; F7 is a judgement call.


---

## Part 44 — P91 — Observability — the increment-by-increment build diary and planning record (verbatim), plus the "WIP on branch, do not merge" note. **The P91 milestone entry stays LIVE on the board**; only the diary moved.

## 📐 P91 — Observability: Dev mode, structured logs, local telemetry & metrics — PLANNING (awaiting user approval)

**Current step:** ✅ **INCREMENT 1 DONE + COMMITTED `1b94529`** on `feat/p91-observability`
(cargo --lib 360 passed / obs 43, clippy -D warnings clean, tsc clean, all new files <500 lines).
Reviewer round 1 = "request changes" (2 MUST-FIX in the privacy layer); all fixed + re-verified.
**✅ INCREMENT 2 DONE + COMMITTED `6fc3131`** (frontend pipeline: proxy, trace, ui:-prefixed redact
mirror, batcher; tsc clean, vitest src/obs+src/ipc 348 passed, broad IPC-consumer pass 1989 passed).
Reviewer round 1 = "request changes" on 1 MUST-FIX, fixed + independently re-verified.
**✅ INCREMENT 3 DONE + COMMITTED** (Rust dispatch shim + emit→emit_logged migration + §3.1 spans +
2 Layer-A redaction patterns + mock span fixtures). Shim compiled against tauri 2.11.5 (§2.3.1
contingency NOT needed). cargo --lib 370 passed / 1 ignored (overhead bench), clippy -D clean both
crates, tsc + build clean, vitest 2503 passed. **Reviewer round 1 = APPROVE, no MUST-FIX.** All 3
deviations accepted by reviewer (see below).
**✅ INCREMENT 4 DONE + COMMITTED** — the flicker payload: `obs/react.ts` (useRenderCount/
useTracedEffect/useStateTransitionLog) + `obs/renderTally.ts` (aggregate mode) + `obs/gesture.ts`;
ten surfaces instrumented per the pinned §9.2 map; echo/refresh causality (`armEcho(repoId,trace)` +
suppressed-`watcher` record + `pendingTracesRef`/`refresh` records); gesture trace origination for
the mutation/refresh cluster; `withTrace` now emits one `gesture` record per mint + no-ops when off;
`frameStats` `onWindow`→`frame` routing. ui-designer pass first (a843174), architect ratified D3
(3a5c800). tsc + eslint clean, 920 vitest green (incl. real-Sidebar churn test). **Reviewer +
ui-designer both APPROVE, no MUST-FIX.**
**✅ INCREMENT 5 DONE + COMMITTED** — anomaly detector (backend Rust): `obs/anomaly.rs` +
`anomaly/window.rs` + `anomaly/slow.rs` + shared `obs/histogram.rs`; hooked into `sink.rs`
`writer_loop` (observe after seq assignment, derived anomalies written back, `on_session_end` on
shutdown+disconnect). All §5 sink rules + §5.1 perf rules + `SLOW_RULES` table. `dup-ipc` filters
EXPLICITLY on `LogPayload::IpcCall` (synthetic-argsHash negative test genuine). Redaction-independent
(byte-identical raw vs `path#N`). 100 obs tests / 46 new. clippy -D clean. **Reviewer APPROVE, no
MUST-FIX.** Percentile = linear interpolation (§8.1) clamped to `max_ms` (needed for §12(a) to fire).
**Decision: mock-side anomaly (5b) IN PROGRESS** — the milestone AI-gate + §6 require the browser
harness (`__bonsaiDumpLogs()`) to show anomalies with no Tauri, but the detector runs only on the
Rust writer thread. Building a mock-only, dump-time batch analyzer scoped to EXACTLY the 3 gate-named
rules (`dup-ipc`/`slow-command`/`slow-phase`); Rust `obs/anomaly.rs` stays authoritative.

**✅ INCREMENT 5b DONE + COMMITTED `74cfef5`** — mock-only dump-time anomaly analyzer
(`src/ipc/mock/obsAnomaly.ts`) scoped to the 3 gate rules (dup-ipc/slow-command/slow-phase) so the
browser harness can assert anomalies with no Tauri; Rust `obs/anomaly.rs` stays authoritative.
Ratified §6+§13 row 22. 7 vitest. (Orchestrator fixed a raw-NUL key separator → `` escape so
the file stays plain-text.)
**✅ INC-5 RATIFICATIONS COMMITTED `64e842c`** (clamped percentile, BatchMark, mock-split).
**✅ INCREMENT 6 DONE + COMMITTED** — durable metrics (backend): `obs/metrics.rs` + `metrics_file.rs`
(atomic save + `.bak` recovery) + `histogram.rs` derived p50/p95 (never persisted); perf.rs absorbed
at flush; `<domain>.<action>` allow-list (no user-derived key); `metrics_snapshot`/`metrics_reset`
(headless, no catalog row) + mock. All §12 row-6 + §8.1 acceptance verified; clippy clean, 438 tauri
/952 core cargo, tsc clean. inc-5 histogram NIT fixed. **Reviewer APPROVE, no MUST-FIX.** §8
"always on" ratified (counters always-on; duration histos Dev-mode-only — §11 zero-cost-off).
**✅ INCREMENT 7 DONE + COMMITTED (7a/7b/7c)** — Dev settings page + `logs_delete_all` + truncation
+ disk-write-error. ui-designer reconciliation `333a157` (§16). **7a `cbafdc7`** (backend: RollAndPurge
roll-then-purge, `logs_delete_all`, `truncate` record + SessionPayload.truncated/droppedParts,
LogSessionInfo.droppedParts; purge scope reviewer-verified confined; all §12 row-7 (a)-(l) backend).
**7b `35b410e`** (frontend: DevCategory page + small sections + header pill + catalog `dev` category
after About + DevSettings threaded through the settings pipeline; all §16.9 states; catalog-parity
+343 vitest; reviewer + ui-designer APPROVE, 1 design MUST-FIX (aria-disabled dimming) applied by
orchestrator). **7c** (disk-write-error: `LogSessionInfo.writeFailed` bool — never the error string;
`▲ Not writing` status row + DevModePill danger variant; sticky-until-flush semantics; + 2 NIT fixes
+ inc-3 pool-gauge doc narrowing; reviewer APPROVE, privacy PASS). writeFailed field ratified inline
in contract (§8.4/LogSessionInfo). **§8.4 danger toast (dedupe `dev-sink`) DEFERRED** (status card +
pill already surface the state persistently — ratified as follow-up in contract).

**🎉 ALL 7 INCREMENTS IMPLEMENTED + REVIEWED + COMMITTED.** Endgame:
- ✅ **Refactor DONE + committed `5d6321c`** — 4 size targets split behavior-preserving (writer.rs
  598→455, useWorkspaceKeyboard.ts 508→381, GraphCanvas.tsx 926→897, RepoWorkspace.tsx 2837→2778);
  identical test counts; `lint:size` GREEN.
- ✅ **FULL `pnpm gate` GREEN except 1 known flake** — rust nextest (430s) + doctests + clippy +
  eslint + **file-size ratchet** + vitest (2545) + tsc+build ALL ✓. Only e2e red = 1 test
  `16-history-undo-health.spec.ts` "repo health panel renders sections", which **PASSES 6/6
  isolated** — the documented pre-existing flake (P89 memory), P91 touches no history/undo/health
  code. Gate log: `scratchpad/gate.log`.
- ✅ **INCREMENT 7d DONE + COMMITTED `e769108`** — closed a CRITICAL integration gap found during the
  harness-evidence step: the frontend obs pipeline was fully built + instrumented but **never activated**
  because `configureObs(dev)` (the `obsEnabled()` master switch) had ZERO production callers. (`attachSink`
  was already wired via `instrumentIpc` at module load — my initial finding mis-named it.) Fix: a
  `[dev]`-effect in `useUiSettings` calling `configureObs(dev)` on boot + every change + an anti-regression
  guard test (globs source, asserts both `configureObs`/`attachSink` keep a production caller). Backend was
  already correct. **Lesson: per-layer unit tests all passed (each correctly gates on obsEnabled()); only an
  end-to-end activation check exposes a switch nobody flipped.** THIS is why the harness/gate step exists.
- ✅ **HARNESS AI-GATE EVIDENCE (headless, deterministic) — `src/obs/pipeline.e2e.test.tsx`**: drives the
  REAL chain (configureObs → instrumentIpc proxy → batcher flush → ring → analyzer → `__bonsaiDumpLogs()`).
  Two real identical instrumented invokes → exactly one `dup-ipc` whose refs resolve to two `ipc.call`
  records; seeded slow fixture → `slow-command`+`slow-phase` with refs resolving to span+result. 8/8 green.
  Stronger than a browser screenshot (asserts exact refs/seqs).
- ✅ **FRONTEND GATE RE-VERIFIED @ e769108**: vitest 2553 passed (223 files), tsc+build clean, eslint 0
  errors (35 warn ≤40), lint:size OK (both refactored files shrank below baseline). Rust gate stands (7d
  frontend-only). Full gate = GREEN except the 1 known `16-history-undo-health` flake (passes 6/6 isolated).
- ⏳ **Real `logs/*.jsonl` parse** → folded into USER CHECKPOINT (needs native `pnpm tauri dev`; the
  schema/redaction validation it does is already covered by the Rust redaction/strict/schema unit tests).
- ⏳ Present AI-gate + USER CHECKPOINT — asked, not declared.
- ⏳ Real `logs/*.jsonl` parse from a `pnpm tauri dev` boot+idle (native build cost — likely folded
  into USER CHECKPOINT (c)).
- ⏳ Present AI-gate + USER CHECKPOINT (a)-(f) + 7b/7c native items — asked, not declared.

**Increment 7 follow-ups (non-blocking):**
- **SHOULD-FIX (7c):** `writeFailed` can flap healthy during a PERSISTENT ROTATION-OPEN block —
  `open_part(next)` fails (record dropped) but `self.file` still points at the healthy old BufWriter,
  so the idle-flush clears `write_failed` until the next record re-sets it → pill/status flickers.
  Disk-full/permission (write_all/flush) paths are correctly sticky. Fix: gate the flush-clear on
  "rotation healthy" (`writer.rs` ~227/330). Narrow edge (needs cap-hit + dir-unwritable + timing).
- **NIT:** pill poll 3s vs status card 2s (intentional; note so nobody "fixes" it).
- **7b follow-ups:** PushToast has no action-button support → "Show in folder" toast actions omitted;
  command-palette Dev entries (§1.4) skipped; byte formatter is house `formatBytes` → "MiB" not "MB".
- **inc-3 shim exclusion-list NIT CLOSED:** `metrics_snapshot`/`metrics_reset`/`logs_delete_all`
  landed in inc-6/7a and both reviewers verified the exclusion-list names match the registered cmds.

**Increment 5b DONE + COMMITTED `74cfef5`** — see below (kept for detail).

**Increment 5 follow-ups (non-blocking):**

**Increment 6 follow-ups (non-blocking, SHOULD-FIX):**
- **Concurrent-save race:** `metrics_file.rs:63` uses a fixed `usage.json.tmp` and `save()` runs
  OUTSIDE the `MetricsState` mutex — two overlapping savers (60s flush vs `metrics_reset`, or
  exit-flush vs timer) could interleave and rename a torn file onto primary. `.bak` still holds the
  prior good copy so no total loss, but fix: unique tmp name (pid+counter) or a dedicated write mutex.
- **No parent-dir fsync** after the renames (`metrics_file.rs:75-79`) — power-loss can lose the
  rename (reverts to prior good primary; no total loss). Known durability gap.
- **NIT:** startup span between `set_active` and `init()` is discarded (`init()` sets `g.file`
  wholesale) — Dev-mode+startup only. `bump_counter` is `pub`+unvalidated — consider `#[doc(hidden)]`
  or a debug-assert the key has no `/`,`.`,space.

**Increment 5 follow-ups (non-blocking):**
- **SHOULD-FIX:** `ipc_calls.last_fire` (`window.rs:84`) is keyed by `cmd\0argsHash` and never
  pruned — argsHash keyspace is unbounded (every other last_fire map keys on a finite catalogue).
  Fire-gated + Dev-mode-only + session-scoped so growth is tiny in practice, but it falsifies the
  "bounded (§11)" claim at `anomaly.rs:47`. One-line fix: prune in `Sliding::prune` or cap the map.
- **SHOULD-FIX:** ratify the `max_ms` percentile clamp into §8.1 (batched into the architect call).
- **NIT:** `histogram.rs:73` "only reachable at i==7" comment is wrong — `percentile_ms(0.0)` with
  empty bucket[0] returns `max_ms`; inert now (only 0.95 used) but inc-6 reuses this — fix in inc-6.
- **NIT:** `BatchMark` arm skips `emit_pending_drops` (minor ordering); `unbatched-sink` 10th-mark ref
  points at the anomaly not the batch's first record (info-severity); `jank-trace` "overlapped 0
  span(s)" when no overlapping span (confirm §5 wording — likely intended: unattributed jank is jank).

**Increment 4 — the trap that was avoided (worth keeping):** the ambient trace is SYNCHRONOUS (§2.5)
and dies across `await`; a mutation handler does `await ipc.mutate()` THEN `refreshAll()`, by which
point `currentTrace()` is empty. Pattern chosen: capture the trace at each handler's sync entry and
thread it BY VALUE `refreshAll(scope,trace)`→`refresh(origin,scope,trace)`→`armEcho(repoId,trace)`;
`currentTrace()` is only ever read synchronously (fallback for callers inside their own gesture's
sync extent, e.g. manual refresh). A negative-control test proves an unbound post-await refresh logs
`causedBy:undefined`, never a stale trace. `withTrace` also made a no-op when Dev mode off.

**Increment 4 follow-ups (non-blocking):**
- **SHOULD-FIX:** `react.ts` `useStateTransitionLog`'s `briefValue` caps length but does NOT route
  `from`/`to` through `obs/redact.ts` — would leak raw values (branch names) IF wired. Safe today
  (called nowhere); gate wiring behind redaction before any use. Documented as a known limitation.
- **NIT:** `frame` records split paint vs gap into two records with the other dimension hard-`0`
  (`GraphCanvas.tsx emitFrameRecord`); a consumer could misread `gapMs:0` on a paint record. Add a
  `dim:'paint'|'gap'` discriminator to disambiguate (trace-level, low severity).
- **NIT:** `useRenderCount` 'each' calls `logRecord` in the render body, so React StrictMode dev
  double-invoke can double container render counts (fine for a dev-only tool; worth a comment).
- **Deviations accepted (consistent w/ §12 row 4):** `refresh.ms` = round execution duration;
  gesture origination wired only for the mutation/refresh cluster.
- **Gestures NOT wired in v1** (deferred by design): command-palette/appCommands entries; manual
  Refresh button + focus/activation origins; graph context-menu checkoutCommit/checkoutRemote
  (shared deps object — wrapping churns hook identity); non-echo-arming submodule init/update/sync +
  handleSetRemoteUrl (config-only, no refresh round); merge/rebase/bisect/cherrypick/revert/conflict/
  AI flows. Revisit if the six surfaces + inc-5 anomalies prove insufficient.

**Increment 3 deviations (reviewer-accepted, recorded here per D3's "record in contract" ask):**
(D1) global `ACTIVE_SINK` static for `PhaseRecorder`/watcher instead of threading `AppHandle` —
avoids the §2.2-forbidden command-signature churn; set/cleared under the writer slot lock, tests
serialize via `test_sink_lock()`. (D2) `queuedMs`/pool gauge + `deadlineFrac` wired at the 3
span-emitting command sites, not in `repo_handle.rs`; queue delta still captured across the real
`spawn_blocking` boundary (reviewer: correct). (D3) `status.scan`/`diff.compute` collapsed to a
single phase each (contract specced 3) because finer splits need phase hooks INSIDE `bonsai-core`,
which the crate-boundary invariant forbids (`bonsai_core_has_no_obs_reference`); only `graph.get` is
fully phased. **Architect should ratify D3 into §3.1.2/§13.**

**Increment 3 follow-ups (non-blocking, for tester / later increments):**
- **SHOULD-FIX (tester):** `POOL_INFLIGHT` is bumped only by the 3 span-site `PoolGuard::enter()`
  calls, so `poolInflight` counts "instrumented git ops in flight," NOT true tokio blocking-pool
  saturation; `POOL_MAX = 512` is hardcoded, not read from tokio. Row-3 acceptance (b)'s
  `poolInflight >= poolMax` is unreachable in a realistic test — write the tester assertion against
  what the gauge actually counts, or narrow the field doc wording (`phase.rs:75-101`, `:35`).
- **NIT:** `invoke_shim.rs:42-48` exclusion list names `logs_delete_all` + `metrics_snapshot` which
  aren't registered yet (ship in inc 6/7) — dead entries now; confirm the strings match when those
  commands land, else self-amplification silently re-enables.
- **NIT:** recorder-overhead bench for acceptance (e) is `#[ignore]` — tester to run it explicitly.
- **NIT:** `stream_graph_cached` (`graph_cache.rs:239`) always finishes `SpanOutcome::Ok` even on
  `Err` (non-routed diagnostic path; routed path correct).
- **NIT:** diff span covers only `get_workdir_file_diff`; commit-vs-parent diffs uninstrumented (v1).

**Increment 2 notes worth keeping:** (a) MUST-FIX was that `__trace` injection sat one layer too
high — the proxy appended trace metadata as an extra JS argument, but every real IPC method is
fixed-arity/positional and builds its own payload, so **nothing ever reached the backend**; the two
object-arg methods (`setUiSettings`, `setSession`) got keys nested INSIDE a payload that is
persisted verbatim. Increment 3's core criterion was unachievable as built, and the test masked it
by using a 2-arg signature that does not exist in the real api. Fixed by creating
`src/ipc/tauri/invoke.ts` (the codebase had NO central invoke wrapper — 21 modules imported it
straight from Tauri) which stamps at the payload top level; injection removed from the proxy, which
also kills the mock-persistence pollution by construction. A glob-based guard test forbids direct
`@tauri-apps/api/core` invoke imports so an untraced command cannot be reintroduced silently.
(b) The guard's own first two versions **passed on a real offender** because of a regex escaping
slip — only the negative control caught it. Lesson: assert a guard fails before trusting that it
passes. (c) `argsHash` is computed BEFORE injection, else every call hashes uniquely and `dup-ipc`
could never fire. (d) Reviewer resolved the canonical-form worry with stronger evidence than the
implementer's: `IpcRecvPayload` has NO `argsHash` field, so only one canonical form exists in the
stream and `dup-ipc` cannot be fooled.

**Carry-in for increment 3:** the transport stamps only when an ambient trace exists (§2.5 — never
guess), so increment 3's criterion reads as "every **traced** dispatch yields a stamped
`ipc.recv`"; untraced calls dispatch with no `__trace` rather than a fabricated one. Wiring traces
to real gestures is increment 4's job. Also: `configureObs()` and `pendingRestart()` are unused in
production by design — activation is increments 4/7.

**Increment 1 notes worth keeping:** (a) strict mode was NOT enforced — the writer trusted the
producer, so a frontend bug or mode-toggle race would write real paths/refs into a file whose own
header claimed `redaction:"strict"`. Now enforced writer-side in `obs/strict.rs` (args dropped
unconditionally; URLs/refs/paths ordinalised in every string field), because §7 makes this a
Rust-owned guarantee. (b) Error messages were exempt entirely — an `AppError` Display string leaked
BOTH a username and a repo name; same pass fixes it. (c) `glpat-` (26 chars) was *structurally*
uncatchable under a global 40-char floor — `TOKEN_PREFIXES` now carries per-prefix minimums.
(d) senior-dev self-review caught strict enforcement mangling the header's OWN `redactionNote`
(the disclosure text contained slashes); note is now asserted verbatim. (e) salt-never-on-disk test
found a real gap — a producer echoing the salt back reached disk; `scrub_salt` added.
(f) Rotation now drops the OLDEST part instead of refusing to rotate (orchestrator call — the old
behaviour left a storm session completely unbounded, since prune only ran at startup);
**needs architect ratification into §6.**

**Deferred to the architect (non-blocking):** ratify the rotation drop-oldest rule into §6; ratify
or veto senior-dev's base64-secret heuristic (slash-bearing candidates qualify only when a
`/`-free run is ≥24 chars + alpha/digit mix, minus pure-hex so SHAs survive; floor lowered 40→32).
**Known over-redaction (fail-safe, tested):** scp-style remotes ordinalise as `path#` not `remote#`;
UUID-shaped strings ≥32 chars now over-redact. 🛑 **USER GATE (2026-08-27): finish
increment 1 — review → MUST-FIX → commit — then STOP AND WAIT. Do NOT start increment 2 without an
explicit go from the user. Each subsequent increment requires its own go.**
**Perf-diagnosis addendum DONE (decision 8, 2026-08-27)** — added after the user asked whether the
plan also identifies performance issues. New §3.1 `span` record kind carrying a flat `phases[]`
array per completed operation (≤16, dotted labels), plus optional `queuedMs`, `poolInflight/Max`,
`deadlineFrac`, `cache`, `items`. Timing is taken at the **src-tauri caller layer** so
`crates/bonsai-core` gains NO dependency on `obs/`; the mechanism is an explicit `PhaseRecorder`
value, not a task-local (same reasoning as §2.2's rejection of an ambient backend trace). Exactly
three instrumented ops in v1 — `graph.get` (revwalk/decorate/lane/filter/serialize), `status.scan`,
`diff.compute` — ~11 phase labels; a fourth needs a new §13 row. New §5.1 rules: `slow-command`
(self-calibrating `ms > max(floor, k × rolling_p95)` with a MIN_SAMPLES gate + rate limit, so it
does not fire constantly on a 20k repo), `slow-phase`, `queue-delay`, `pool-saturation`,
`watchdog-pressure`, `cache-collapse`. New §8.1 derives p50/p95 from the frozen 8 histogram buckets
— no new storage type, percentiles never persisted, ~3 KB/day. **All ADDITIVE:** one new `LogKind`
variant + optional serde-default fields, `OBS_SCHEMA_VERSION` stays 1.
**Absorption: increment 1 UNTOUCHED**, phase work → increment 3, rules → increment 5, percentiles →
increment 6. **Deferred:** interaction latency (gesture → visible paint) — needs rAF-after-commit
plumbing in all six surfaces and is unverifiable in the headless harness (0×0 pane, no rAF); the
existing gesture → ipc.result.ms → span.phases → render.tally → jank-trace chain already
triangulates the motivating complaints.
**Nit for the increment-5 prompt:** §5.1 pseudocode says p95 = bucket upper bound, §8.1 says linear
interpolation — pick one at implementation time.
**⚠️ PRE-EXISTING GATE FAILURE (not P91):** `pnpm lint:size` is RED on `main` as a result of the
graph-features merge (fa499e2), verified independent of increment 1 —
`src/components/repoWorkspace/useWorkspaceKeyboard.ts` is **508 lines and a NEW offender** absent
from `scripts/file-size-baseline.json` (grew in 4cbc75f, PR center-diff), and
`src/components/RepoWorkspace.tsx` is **2800 vs baseline 2787 (+13)**. Neither file is touched by
P91. Must be fixed by `refactorer` (behavior-preserving split, identical before/after test counts)
before any full `pnpm gate` can go green — spun out as its own task.

**Orchestrator scope error corrected:** I briefed senior-dev to build `logs_delete_all` in
increment 1; the contract assigns it to increment 7. Told it to keep the work if already done
(backend-only, correct per §6.1) and record it as pulled forward, else skip.

**Goal:** make unintended app behaviour mechanically visible. The user reports UI flickers and
"things that don't look right"; they want to enable a Dev mode, reproduce, and send the resulting
log file to an AI that can identify double triggers, redundant IPC calls, effects firing on
unchanged deps, echo-induced refreshes and superseded results — without eyeballing 50k lines.

**Contracts:** `docs/contracts/P91-observability.md` (architecture) ·
`docs/contracts/P91-observability-ui.md` (Dev-mode settings surface) ·
`docs/contracts/ui-reference.md` §12.11 + §1 header order (applied by orchestrator from the
ui-designer's staged patch — its Edit tool was unavailable; patch file consumed and deleted).

**Design centrepieces:** per-gesture trace ids threaded UI → invoke → Rust span → emitted events →
the refresh round they cause; instrumentation at two choke points only (`src/ipc/index.ts` Proxy,
which covers real and mock by construction, and the Rust dispatch shim) rather than scattered log
lines; first-class `anomaly` records (dup-ipc, redundant-refresh, effect-no-change, effect-thrash,
event-storm, watcher-storm, jank-trace, superseded-result, orphan-trace) computed sink-side and
carrying `refs` into the implicated records.

**User decisions (2026-08-27):** redaction conservative by default with an opt-in raw-names toggle
(logs must be safe to send to a third party unreviewed; credentials never logged in any mode);
metrics storage delegated to the architect (recommends rolled-up JSON over SQLite); stop at plan.

**Delivery:** 7 increments — (1) Rust log core, (2) frontend pipeline, (3) Rust dispatch/events/
watcher, (4) **refresh+echo+React causality = the flicker payload (UI)**, (5) anomaly detector,
(6) metrics, (7) Settings Dev page (UI). Increments 4 and 7 need the ui-designer pass first.

**All 6 open decisions RESOLVED by user (2026-08-27)** — see `docs/contracts/P91-observability.md` §13:
(1) trace transport approved as specced (injected `__trace` + Rust `ipc.recv` shim; documented
fallback = drop the shim if a Tauri upgrade breaks the unstable `tauri::ipc::Invoke`, losing only
backend-receipt visibility); (2) metrics storage = **rolled-up JSON**, not SQLite; (3) React
instrumentation = **SIX surfaces** — the user added the **left sidebar** to the original five
(RepoWorkspace+hooks, DiffBrowser, GraphCanvas, right-panel tabs, PR panel) because the left pane
is where they saw flickering; (4) logs are NOT auto-deleted when Dev mode goes off (prune by caps
only); (5) per-session log files; (6) `metrics_reset` ships headless.

**Decision 7 (2026-08-27) — "Delete all log files" ships in v1**, not as a follow-up. ui-designer
raised, and the user accepted, that because logs persist after Dev mode goes off and pruning needs
10 *newer* sessions, a single **raw-names** session can leave real branch/tag/file/repo names on
disk indefinitely for an occasional debugger. **Model = roll-then-purge** (`logs_delete_all` →
`LogsDeleteResult { deletedFiles, deletedBytes, failedFiles, activeFile, rolled }`): the writer
flushes and CLOSES the current file, opens a fresh one (`session` header carries `afterPurge:true`),
then deletes every other `*.jsonl`. **This resolved a direct contract conflict** — ui-designer had
specced "always exclude the active file" to dodge the Windows sharing-violation / Unix
unlinked-inode hazard; roll-then-purge removes the open handle *before* deletion, so both hazards
vanish AND the active file (the one actually holding the raw names) is erased. Excluding it would
have shown a success toast while the exposure stayed on disk. Scope hard-limited to `logs/*.jsonl`
+ `.tmp` — never `metrics/` or `settings.json`. Success copy must say "Still recording" when
`rolled:true`, or users re-toggle Dev mode and lose the records gathered since the purge.
Decision 4 above forbids **automatic** deletion only; user-initiated delete is in scope.

**Parked for the P91 design-review pass (not blocking):** re-measure the header pill contrast (the
figure was taken on `--bg-2`, the header is `--bg-1`); status card polls `log_session_info()` every
2s while visible (keep); >24h Dev-mode escalation to a notice bar (declined — the always-visible
pill suffices); selective per-file deletion remains a follow-up.


**Where the rest of the board went:** `docs/history/todo-archive-2026-09.md` (Parts 22-32, moved
2026-09-01) and `docs/history/todo-archive-2026-08.md` (Parts 1-21). See the Archive table at the
bottom. Velocity/gate-cost measurements: `docs/history/velocity-2026-09-01.md`.


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


---

## Part 45 — P91 — SHOULD-FIX follow-ups from the increment-4-7 review (2026-09-02), full text (verbatim). **Still OPEN** — a condensed one-line-per-item form stays live on the board.

## 🐞 P91 — SHOULD-FIX follow-ups from the increment-4-7 review (filed 2026-09-02, non-blocking)

Both are **documentation-accuracy** defects: the prose claims more completeness than the code
delivers. Filed rather than routed back, per velocity mode.

- **`SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`, `:409-425`).
  `(path, file, dirty)` is read under the `MetricsState` mutex and the guard is *released* before
  `metrics_file::save`. Two savers can therefore snapshot in order A→B but acquire `SAVE_LOCK` in
  order B→A, so the older bytes land last. **The dangerous instance is exactly the pair the new
  `SAVE_LOCK` doc cites as its motivation:** `reset()` snapshots the emptied file, an in-flight
  flush holding an older non-empty snapshot commits after it, and **the reset is silently undone on
  disk**. Both paths then set `dirty = false`, so nothing reschedules a corrective write until the
  next counter bump. The reasoning behind choosing a mutex over a unique tmp name is sound as far as
  it goes; the doc at `metrics_file.rs:52-66` just overstates it as "serializes the whole commit
  sequence". Fix: take `SAVE_LOCK` around snapshot+save, or add a generation stamp, or amend the doc
  to name the residual staleness. **No deadlock risk** — verified both call sites drop the guard
  before `save`, `save` never re-enters `MetricsState`, and it is a plain sync fn.
- **The `last_fire` prune is answer-preserving on the window axis but assumes non-decreasing `ts`**
  (`window.rs:52-63`), and the comment states it unconditionally. Record `ts` comes from two
  **unsynchronised clocks** — `Date.now()` on the frontend (`src/obs/log.ts:43`) and `now_ms()` in
  Rust (`sink.rs:385`) — merged into one writer stream via batched `log_append`, with no monotonic
  clamp anywhere. If `ts` regresses by more than the window, a pruned entry that would have
  debounced a fire is gone and the rule can double-emit. **Blast radius is a duplicate anomaly
  record, never a missed one**, and `events` already carried the same exposure — so this is a
  comment fix stating the monotonic-`ts` premise, unless a `cutoff` guard is wanted.

**NIT worth keeping:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms
apart against a 300 ms window, which makes `len <= 4` nearly tautological. It proves pruning happens,
not the bound — a dense burst of distinct keys inside one window still grows the map to that burst's
cardinality. Unlike `open_calls` (FIFO 1024) and `slow` (LRU 200), `last_fire` has **no numeric cap**,
so §11's "bounded" is genuinely weaker for this map than for its neighbours.

### Also filed 2026-09-02
- **`.forge-connect-link:hover` is now a no-op** — the resting-underline MUST-FIX means hover declares
  the same underline, so the link has **no hover feedback at all**. A thickness bump is the
  contrast-safe option; the implementer correctly declined to invent a treatment. → `ui-designer`.
- **`docs/contracts/pr-badge-placement-ui.md:106,116,156`** still documents the canvas merged pill as
  `#8957e5`, now stale after the `--merged` token landed. → contract owner.
- **`.settings-toggle-btn.is-active` — the investigation CLOSED it as (b), dead styling, NOT a
  product bug.** Git history pins it: the rule was introduced in `cf174ff` for the git-config
  **Local | Global** level toggle, which `7354aca` (P69h) then replaced with `SettingsSegmented`
  (`role="radiogroup"`, styled via `.settings-segment.is-selected` — a different class). `is-active`
  was orphaned at that moment and has matched nothing since. All 29 surviving call sites are one-shot
  **action** buttons (Refresh, Edit, Delete, Activate…) that should *not* carry a selected state. So
  **no settings control is missing a state indicator** — the earlier "possible product bug" reading is
  retracted. The honest close is to delete both rules and optionally rename the misnamed class. CSS
  left in place (correct but unreachable); deletion is a trivial follow-up.


---

## Part 46 — VELOCITY — workspace test wall cut 14% (`737cc4b`, `5731d37`, 2026-09-03), full text (verbatim). The two "filed, deliberately not taken" items stay live on the board.

## ⚡ VELOCITY — workspace test wall cut 14% (`737cc4b`, `5731d37`, 2026-09-03)

`cargo nextest run --workspace` **106.5s → 92.5s**, with tests **increasing 2298 → 2316**. Every
figure is from paired or repeated runs: concurrent agents on this box produced one 136s outlier
purely from CPU contention, so single-run numbers would have been worthless.

**The most useful result was a negative one.** Banding `prop_status::status_matches_porcelain`
(44.6s → 13.7s max band, 3.25x) and `prop_stash_roundtrip` (3.2x) barely moved workspace wall — 108.8s
→ 102.9s — because **a different test immediately became the floor.** *Optimising the
measured-slowest thing does not necessarily move the number you care about.* Case totals were checked,
not assumed: status **32 → 32**, stash **96 → 100** (the +4 deliberate, so the marginal over `n` stays
uniform rather than skewed by rounding).

**The real floor was `corrupt_repo_matrix_never_panics`** — 44.0s contended but **24.7s alone**, so
intrinsic rather than contention. It held **13** cells, not the 10 the diagnosis assumed. Split into
13 test fns: suite alone 24.65s → **17.02s**, other 12 cells now 1.16-2.68s each and off the critical
path. Shared setup was *verified* rather than assumed — the isolated suite fell by exactly C1's
runtime, proving the split added no setup cost.

**The proptest regression seeds were worse than useless.** proptest keys its persistence file **per
source file, not per test fn**, so after banding all 4 bands replayed all 3 seeds (12 replays, ~25% of
each band). Worse, they were **stale**: a `cc` seed regenerates values through the *current* strategy,
and the strategy changed when op kind 5 was re-added — so they no longer reproduced the inputs in
their own `# shrinks to` comments. **They were random cases wearing a regression label, which is worse
than no coverage because it looks like coverage.** Converted to explicit pinned-input tests, verified
mechanically (both the deleted file and the generated Rust re-parsed into tuples and compared
element-by-element, 3/3 identical). Replays 12 → 0.

**`common::init_repo()` spent 5 process spawns where 1 does.** `git init` stays a CLI spawn so repo
layout, templates and `init.defaultBranch` remain exactly git's; the 4 `git config` calls became
in-process `git2` writes. **−8.7s (−8.6%) across the workspace**, ~0.44s per fixture repo. Equivalence
is **proven by a guard test, not asserted**: a helper repo's sorted `git config --local --list` is
identical to a control built with the literal four invocations; both the git CLI and libgit2 read all
four keys; `repo.signature()` resolves (the property the fixture exists for); and a later write still
*replaces* rather than creating an ambiguous multivar. **Residual risk stated:** equivalence is proven
at the `--list` level, not textually, so a *future* test asserting on `.git/config` as text could
differ. No current test reads it as text (checked).

### Filed, deliberately not taken
- **C1 could drop 17s → 11s** by giving one surface its own test and its own corrupted repo. **Not
  taken:** it changes the shape of a crash-safety test for ~6s, and C1's deliberate deadline-burning
  is the point of the cell.
- **`crates/bonsai-mcp/tests/common/mod.rs:127`** — the twin fixture helper still spawns 3 `git
  config` calls. The same fix applies verbatim; left alone to keep the blast radius inside one crate.
- **`cargo fmt` is NOT clean repo-wide** (thousands of pre-existing diffs) and fmt is not a gate step
  — do not run it, it would bury a real diff in noise. Same class as the **missing prettier config**
  for the frontend.


---

## Part 47 — Milestones DONE + USER CHECKPOINT VERIFIED, moved off the board 2026-09-03 (verbatim): the P100+P101+DX-e2e banner, P99, P100, P101 (incl. its original filing), P98, P95, P96, P97.


### P100 + P101 + DX-e2e — the batch banner (COMPLETE)

## 🚀 P100 + P101 + DX-e2e — IN PROGRESS (started 2026-09-01, USER: "do P100 and P101 and DX: build-bundle e2e")

**Current step:** ✅ **COMPLETE.** All three requested items DONE — P100 (`e118375`), P101 (`4fec07a`), DX-e2e (`46088e0`). P103 also fixed (`8bae4ed`) as a real product bug found by the DX equivalence check. **USER CHECKPOINTs for P100 (4 items) + P101 (AC12-AC16) CONFIRMED VERIFIED by the user on 2026-09-02** — see the note on each milestone for the basis. Filed while here: **P102**, **P104**, **P105**; P102+P105 are now running as one combined hue-audit milestone and P104 is in progress.

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



### P99 — `repo` state dead in a production bundle — DONE + VERIFIED (`fea4a71`, USER 2026-09-01)

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


### P100 — accent-fill contrast — DONE + VERIFIED (`e118375`, USER 2026-09-02)

## ✅ P100 — accent-fill contrast — DONE + VERIFIED (`e118375`, USER 2026-09-02)

> **USER CHECKPOINT recorded 2026-09-02 on the user's direct instruction**, not on a
> contemporaneous native run. The user was going away for an unattended session and explicitly
> directed the orchestrator to mark the two owed checkpoints (P100 + P101) verified, scoped to
> those two only. Recorded plainly so the basis is not later mistaken for an observed native
> confirmation: the perceptual call on the quieter active row and the
> `inset 2px 0 0 var(--accent)` leading bar was **accepted without an orchestrator-observed
> `pnpm tauri dev` run**. AI-gate evidence (contrast ratios, AC19, harness screenshots) stands as
> recorded below and was green.

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


### P95 — a11y: graph scroller semantics, keyboard reachability, toolbar contrast — DONE + VERIFIED (`f9a9209`, USER 2026-09-01)

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


### P101 — the full `--text-3` audit — DONE + VERIFIED (`4fec07a`, USER 2026-09-02), incl. the original filing

## ✅ P101 — the full --text-3 audit — DONE + VERIFIED (`4fec07a`, USER 2026-09-02)

> **USER CHECKPOINT recorded 2026-09-02 on the user's direct instruction** (same basis as P100
> above — an unattended session, scoped by the user to P100 + P101 only). AC12-AC16 (hierarchy,
> density, the 15 `forge-pr` declarations behind a token screen, colour-only dot states,
> `.rebase-plan-commit.dropped` on `line-through` alone) were **accepted without an
> orchestrator-observed native run**. The AI-gate caveat below stands unchanged and is NOT
> retracted by this: AC7 coverage was **5 of 94 selectors measured**, the remaining 84
> source-derived because they are unreachable in the default mock state.

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


### P98 — `--text-3` read-text sweep — DONE + VERIFIED (`be668e0`, USER 2026-09-01)

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


### P96 — P93 review follow-ups — DONE + VERIFIED (`cf8bdda`, USER 2026-09-01)

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


### P97 — split ContextMenu.tsx — DONE + VERIFIED (`59d3a41`, USER 2026-09-01)

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

## Part 48 — DX — built-bundle e2e (delivered opt-in), P103 (a real product bug), P104 (the "post-suite hang" that was silent Edge teardown), and the dev/prod gap inventory — full text (verbatim). **The bundle-default FLIP stays live on the board as an open user decision.**

## ✅ DX — built-bundle e2e — DELIVERED opt-in; P104 cleared, default flip is now a free choice

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

**Flipping the e2e bundle default was gated on P104**, which is now cleared — see below: there was
never a hang, only Playwright's silent Edge-teardown phase degrading under machine load.

### ✅ P104 — the "4-worker post-suite hang" — DIAGNOSED + NARRATED (it is not a hang)

**It never hung. It goes silent.** Playwright prints *nothing* between the last test result and
the summary line, and on Windows that gap is Edge teardown. On an idle box it is 0.2 s per browser;
on an oversubscribed box it is minutes of dead air, which is what got read as a hang and cost two
10-minute timeouts. Every run always completed with the correct results and exit code.

**Not reproducible idle — the suite is fast and finishes cleanly.** Four full 4-worker runs on this
tree, both invocation paths (`pnpm exec playwright test`, `pnpm test:e2e`) and both temp drives
(`C:\Temp`, `D:\Data\Temp` — the temp-volume theory was tested and is *not* it):

| mode | wall | result | teardown |
| --- | --- | --- | --- |
| dev server (default) | **162-169 s** | 182: 181 passed / 1 skipped, 0 failed | 0.2-0.8 s per browser |
| built bundle (`E2E_BUNDLE=1`) | **122 s** | 182: 181 passed / 1 skipped, 0 failed | 0.2-0.8 s per browser |

So the "5.8 min → 1.3 min" claim is now verifiable and roughly holds: **2.8 min → 2.0 min**. One
dev-mode run scored 4 failures, but only while a *second* heavy job shared the box — the same load
that produces the teardown stall also produces `page.goto` timeouts, so a "flaky" e2e reading taken
during concurrent work is not evidence about the code.

**Mechanism, reproduced on demand** by running the same 21-test 4-worker batch under 24 spinning
CPU hogs on the 22-core box (`DEBUG=pw:browser`, timestamped). Three costs stack, all in Playwright's
`launchProcess` teardown, none of them ours:

```
t+47s   last test result printed
t+47s   <gracefully close start>  x4     one Edge tree per worker
t+77s   <kill> +30s               x4     CDP `Browser.close` never answered. The window is a
                                         HARDCODED 30 s (DEFAULT_PLAYWRIGHT_TIMEOUT in
                                         playwright-core) — there is no config lever.
t+183s  taskkill returns          x4     `spawnSync('taskkill /pid N /T /F', {shell:true})` BLOCKS
                                         the worker ~106 s under load, and still reports
                                         "PID <n> could not be terminated" for part of the tree
                                         (the msedge network-service / gpu-process utilities —
                                         named by polling Win32_Process during the stall)
t+232s  <process did exit>        x4     Playwright awaits the browser process `close` before the
                                         run may finish
t+233s  "21 passed"                      summary + correct exit code
```

**185 s of total silence, then a correct result.** That explains every reported symptom: worker-count
dependent (N workers = N Edge trees, all missing the same 30 s window; single-worker keeps the one
browser responsive enough to close in 0.2 s), present in *both* modes (it is browser teardown, not
the server), "before the summary line" (worker shutdown precedes `reporter.onEnd`), and the earlier
"30 s then 85 s" reading is this same shape with a smaller tree.

**Fix shipped — `scripts/e2e-teardown-reporter.mjs`** (always on, both tiers): once every expected
result is in *and* nothing is executing, it names the phase and ticks every 15 s, so the silence can
never be misread again. Diagnostic only; its interval is `unref()`d so it can never itself hold the
runner open. Guarded by `E2E_TEARDOWN_REPORTER=0`, grace via `E2E_TEARDOWN_GRACE_MS`. Six unit tests
in `scripts/e2e-teardown-reporter.test.mjs` cover the two hazards (never cry teardown while a test
is running; never hold a handle); vitest's node project now also picks up `scripts/**/*.test.mjs`.

**Residual, not fixable from this repo:** the 30 s close window and the blocking `taskkill` are
inside playwright-core. The operational rule is therefore: **do not run the e2e suite concurrently
with other heavy jobs** (a second suite, a `cargo` build, the harness). `gate.mjs` is already strictly
serial, so the gate itself is safe. Rejected as too risky for the value: `--disable-gpu` to shrink
the Edge tree (several specs assert painted colours, so changing the rasterizer is not free).

**Bundle default is now unblocked.** Nothing else gates it; the flip is one line in
`playwright.config.ts` (`const BUNDLE = process.env.E2E_BUNDLE !== '0'`) plus the `gate.mjs` flag
inversion — left to the orchestrator, since it is a default-behaviour decision, not a bug fix.

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


---

## Part 49 — DX — dev-loop acceleration (condensed stub) and the Velocity / gate-cost stub, verbatim. The two user decisions inside them (P75 HALTED, P76 held as contract-only) stay live on the board.

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

## Part 50 — OPEN follow-ups as they stood on the board on 2026-09-03, verbatim, before the curator condensed them to one line per item. **Every item here is still OPEN unless the live board says otherwise** — this part exists so the condensation is lossless, not because anything was closed.

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
`ai::session_tests::watchdog_tests::watchdog_does_not_fire_while_awaiting_input` (path updated
2026-09-02 by the size-ratchet split; was `ai::session_tests::…`) failed once under load and passed on
immediate re-run.

`src/App.test.tsx > App shell > an Arrow-key pane nudge persists the POST-nudge width` — same shape
(added 2026-09-02). Failed once in a full `pnpm gate` run at 2662ms (`setUiSettings` never called,
i.e. the debounced persist had not fired before the assertion), then passed 4/4 isolated and
2644/2644 on a full-suite re-run. Timing-sensitive under parallel load, not a correctness bug.

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

## Part 51 — Gate states `5c2dcd2` and `c6cd7dd`, and the e2e-contention narrative (verbatim), moved off the board 2026-09-03. **The `c218258` gate block stays LIVE on the board** as the current gate state; the `c6cd7dd` block was kept on the board "for the reasoning, not the status" and that reasoning is preserved here in full. The two operational rules it produced (verify machine state before trusting a timing failure; run the gate idle or e2e with `--workers=1`) stay live on the board.

## ✅ GATE STATE at `5c2dcd2` — **ALL 8 STEPS PASSED** (2026-09-03)

Run on a machine **verified idle first** — CPU sampled six times (~1% after the sampler's own
startup spike), **zero** `bonsai-scratch`/`load.mjs` processes, 42 GB free. 362.0 s total:

| step | time |
|---|---|
| `cargo nextest` | 108.0 s |
| doctests | 3.8 s |
| clippy | 8.5 s |
| eslint | 11.1 s |
| **file-size ratchet** | 0.73 s |
| vitest | 56.2 s |
| tsc + vite build | 10.9 s |
| **e2e playwright** | **162.7 s** |

This covers everything shipped in the session, including P109's badge consolidation, the metrics
snapshot-ordering fix, and both contract ratifications.

**Verifying machine state before running is now a standing pre-gate step**, not a nicety — see the
caveat section below for why. The earlier gate state at `c6cd7dd` is kept underneath because the
reasoning is the point.

---

## 📌 The earlier gate state at `c6cd7dd` — kept for the reasoning, not the status

**All 7 non-e2e steps have been green in every run tonight, regardless of machine load** —
`cargo nextest`, doctests, clippy, eslint, the file-size ratchet, vitest, and tsc+build.

**e2e: 181 passed / 1 skipped / 0 failed in 172.2 s**, run **alone on a verified-idle machine**
(CPU sampled 2-19%, 42 GB free, zero scratch processes). That is the only uncontaminated reading of
the night and it is clean.

### ⚠ The e2e leg is contention-sensitive, and I mis-diagnosed that twice before getting it right

Three consecutive full-gate runs showed e2e failures. **None was a code defect.** Each was the
machine being saturated — and twice the saturation was caused by this session's own tooling:

1. **Gate-5** failed at the *newly raised* 15 s `FIRST_PAINT_TIMEOUT`, which looked like proof the
   new number was still too small. It was not. **My own diagnosis agent had left an 18-thread
   synthetic CPU load generator running** (`bonsai-scratch/load.mjs`, 342 CPU-minutes, machine pinned
   at 100%), inflating every timing ~2× — `cargo nextest` 151 s against a 73 s norm. **Had I trusted
   the surface reading I would have bumped the timeout a second time to hide an artifact of my own
   tooling.**
2. **Gate-6** I asserted was run on a clean machine. **It was not** — the diagnosis agent was still
   finishing its own e2e and load runs and overlapped it; its completion notice arrived in the same
   block as the gate start, and I read that as "already done".
3. I also claimed **34 stray Playwright browsers were leaking** and degrading the box. **False** —
   the count had matched `msedgewebview2` as well; a proper `Win32_Process` check found **zero**
   Playwright-owned processes. The 22 `msedge` are the user's own browser.

**RULE, earned the hard way: before trusting any timing-sensitive failure, verify machine state AT
THE TIME IT RAN** — sample CPU repeatedly, look for scratch/load processes, and confirm no agent is
mid-run. A slow timing number is evidence about the machine until proven otherwise. This is the
timing analogue of the grep-counting rule: *measure the baseline, do not infer it.*

### What this means for the gate
The `pnpm gate` e2e leg will fail intermittently on a loaded machine, and that is **expected**
behaviour documented at P104 (Edge misses a hardcoded 30 s CDP close window, then a blocking
`taskkill` runs). `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removes the largest source, measured at
7.6× p99. **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**


---

## Part 52 — Items closed on 2026-09-03, full narratives (verbatim). Each keeps a one-line record with its SHA on the live board; only the narrative moved. Covers P107 F2 (`8337d9b`), the four items ticked in `112800c`, and the struck-with-evidence entries from the `4002ad2` staleness sweep. **The `ai::session*` clock-seam bullet is NOT closed** — its formal closure belongs to FOR USER item 6, which stays live; only its evidence paragraph moved.

### 52.1 — P107 F2 — the copy-candidate chip said "unchecked" on ticked rows — SHIPPED `8337d9b`

### ✅ P107 F2 — the copy-candidate chip said "unchecked" on ticked rows — SHIPPED `8337d9b` (2026-09-03)

Closed the last open item from P107's design review. Contract:
`docs/contracts/P107-F2-copy-chip-ui.md`.

`WorktreeCopyCandidates.tsx` rendered one danger-tinted chip for two conditions, and the
`previewFailed` branch rendered the word **`unchecked`** — on rows where
`needsDecision = isChecked && (…)`, so the chip appears **only when the box IS ticked**, inches from
an actual checkbox. The word read as the exact opposite of the truth. Now `unknown`, with a neutral
`.wt-copy-chip--unknown` modifier: danger means "this will destroy something", which fits `conflict`
and not "we could not compute a verdict". `.wt-copy-chip` itself is byte-identical — P107 A16's
contrast remediation (11.31 / 12.21) stands. Both branches gained a `title`; the row checkbox gained
`aria-describedby` → the chip, so the reason reaches a screen reader.

**Same increment, P91:** two shipped strings still promised the folder picker that security audit
**F4** removed (`confirmLabel="Choose location…"`, and "Choose a different folder" on permission
denial) — plus the success toast, which named no location while the page's `Show in folder` reveals
`logs/` and the zip lands in the sibling `exports/`.

**Why both went unnoticed: neither state was reachable in the harness.** Added
`?wtCopyPreviewFail=1` and `?obsExportFail=space|permission|other`, the latter reaching three
`exportErrorText` branches that had **no route at all**. Both verified in the harness.


### 52.2 — Two queued-housekeeping entries struck by the staleness sweep

- ~~**`.settings-toggle-btn.is-active` is dead styling**~~ — **ALREADY DONE**, board was stale.
  P107 `2168057` deleted both rules (its own message says so: "re-confirmed dead before deleting").
  Verified 2026-09-03: zero `settings-toggle-btn` + `is-active` pairings in `src/**`, and the only
  surviving rule is the live `min-width: 72px` at `settings-legacy-sections.css:93`. Second board
  entry this session found stale in the same way as P109 — worth a curation sweep for more.
- ~~**`pr-badge-placement-ui.md` documents the merged pill as `#8957e5`**~~ — **DONE** `fc9c36e`.
  Worse than filed: `ui-reference.md` itself carried the stale literal at `:1038`/`:1048` while `:71`
  recorded it as replaced — the canonical design system contradicting itself. All five repointed to
  `--merged` and reworded, since the passages also called it theme-invariant and `--merged` is not.

### 52.3 — `ai::session*` clock seam — evidence paragraph (the item itself stays live under FOR USER item 6)

- ~~**`ai::session*` is load-flaky** — wall-clock watchdog margins; needs a clock seam, not wider
  sleeps.~~ — **the clock seam LANDED in `734b310`** (`test(ai): drive the session watchdog from an
  injectable clock, not wall time`), verified an ancestor of HEAD 2026-09-03. `crates/bonsai-core/
  src/ai/clock.rs` exists and `crates/bonsai-core/src/ai/session_watchdog_tests.rs:41/67/115` drives
  the watchdog with `TestClock::new()` + `clock.advance(...)`, not sleeps. The board asked for
  exactly this fix and it is in the tree. Per FOR USER item 6 the formal close is the
  orchestrator's; the evidence is no longer in doubt.

### 52.4 — Duplicated external-tool launchers — DONE `9273238`, and the deliberately REVERTED toast-timer half

- ~~**Duplicated external-tool launchers**~~ — **DONE** `9273238`. Hoisted to
  `src/hooks/useExternalTools.ts`; `App.tsx` 602 → 590, ratchet lowered by hand (not
  `--update-baseline`, which rewrites the whole file and would have swept in in-flight work).
  `sessionSaveTimer` fixed, with a **proven-red** test. **The toast auto-dismiss timer fix is
  deliberately REVERTED** — `React.StrictMode` is on in `main.tsx`, so every dev mount runs
  effects → cleanups → effects, and a toast pushed during the FIRST pass has already armed its
  handle when that cleanup fires; cancelling on unmount strands it on screen permanently. A
  dev-only behaviour regression traded for a dev-only timer leak. `useToastQueue.test.tsx` carries
  the finding so nobody re-applies it. `useRepoTabs` is immune for a *specific* reason, not by luck:
  its persist effect is gated on `sessionReadyRef`, which `App.tsx:339` sets only after an awaited
  async restore, so nothing is armed during the synchronous double-mount.

### 52.5 — `no_proxy_client()` — closed by the orchestrator 2026-09-03

- ~~**`no_proxy_client()`** in `src-tauri/src/mcp/http_support.rs` still uses `.expect("build reqwest
  client")`.~~ — **CLOSED by the orchestrator 2026-09-03**; this bullet was the stale half of the
  contradiction FOR USER item 6 already resolved. The `.expect` is still there
  (`src-tauri/src/mcp/http_support.rs:221`, re-verified 2026-09-03) but the module is declared
  `#[cfg(test)]` (`src-tauri/src/mcp.rs:410`) `mod http_support;` (`:411`), so it never compiles into a
  shipped binary. Item 6 is the canonical record; this line is a pointer, not a second opinion.

---

## Part 53 — "Durable lessons — the audit method, and what it cost to learn", full text (verbatim) as it stood on 2026-09-03 before the curator split rules from stories. **The RULES stay live on the board** — the six-failed-claims tally, the aliasing rule, the grep-counting rules, the BASE rule, the three P91 testing rules, and the two named failure modes. The stories, the worked numbers and the measurement narrative are here.

## Durable lessons — the audit method, and what it cost to learn

These are the reusable findings. They are on the board, not in the archive, because every one of
them was learned by a claim that was green the whole time it was wrong.

### The six failed app-wide claims, and the distinction between them

Every one had the same shape: **a sentence claiming an app-wide property, with a call-site count
that nobody enumerated.** The first five failed for want of an enumeration. The sixth is different
and worse — the enumeration **existed** and was still blind, because it was **scoped by token name**.

1. **P95** — the enabled-control class; 3 escapes found by P101.
2. **P98** — "`--text-3` family closed"; 122 declarations were never classified.
3. **P74** — the hue-as-text sweep; became P105.
4. **`ui-reference.md` §2** — "6 live hue-over-own-tint instances"; the real population is **38**.
5. **P106's hand-over count of 48** for P108; the real inventory was **62**.
6. **P101** — an *exhaustive* `--text-3` audit recorded as CLOSED, which still missed a **2.96
   light** glyph, because **`--badge-unknown` is byte-identical to `--text-3`**.

**The rule:** a bucket + verdict per call site, P101 §3 style, or it is not closed. Enumerate,
bucket, record a verdict per site, predict the post-fix residue, then verify the prediction.
**Do not accept a "~N call sites and it's fine" sentence as evidence.**

### The aliasing rule

- **An audit scoped by token NAME cannot see an alias. Scope by resolved VALUE.**
- Token aliasing has hidden instances three times: `--badge-good`/`--badge-warn` are byte-identical
  to `--success`/`--danger` (the first pair), then `--badge-unknown` to `--text-3`.
- A `var()` **fallback masking a missing token is invisible to any hue-name search**, because the
  hue name appears only in the fallback (`--warn`, which is defined nowhere).
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
  over `--bg-1`** (P107's figure) and **5.16/4.52 over `--bg-2`** (P106's). All three reproduce under
  one method — the two contracts never disagreed, **neither stated its base**, and the base alone
  accounts for **1.26** of dark-theme spread.

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

### The measurement lessons (archive Part 46 for the numbers)

- **Optimising the measured-slowest test did not move workspace wall, because a different test
  became the floor.** Banding `prop_status` (3.25x) and `prop_stash_roundtrip` (3.2x) moved wall
  only 108.8s → 102.9s. The real floor was `corrupt_repo_matrix_never_panics` — 44.0s contended but
  **24.7s alone**, so intrinsic, and it held **13** cells, not the 10 the diagnosis assumed. Net for
  the pass: `cargo nextest --workspace` **106.5s → 92.5s** with tests *increasing* 2298 → 2316.
- **Concurrent agents on this box produce outliers** — one 136s run came purely from CPU contention;
  single-run numbers are worthless, use paired or repeated runs.
- **proptest regression seeds were worse than useless**: proptest keys its persistence file **per
  source file, not per test fn**, so after banding all 4 bands replayed all 3 seeds — and a `cc`
  seed regenerates values through the *current* strategy, so they no longer reproduced the inputs in
  their own shrink comments. **Random cases wearing a regression label.**
- **Do not run the e2e suite concurrently with other heavy jobs.** Playwright's Edge teardown has a
  hardcoded 30 s CDP close window and a blocking `taskkill`; under load that is minutes of dead air
  that reads as a hang. `gate.mjs` is strictly serial, so the gate itself is safe.
- **A flake that reproduces deterministically in a production bundle is not a flake** (P103).

---

## Part 54 — The `USER CHECKPOINTS — ALL CONFIRMED 2026-09-10` block, verbatim, moved off the board 2026-09-10 after the user confirmed all eight native-verification items (`548cc0a`). Covers P102+P105, P106, P107, P108 and P91, plus the `8dd5b24` CSP item named in the banner. **Two things did NOT move and stay live on the board:** P108's `AC11` (an owed AI-gate contrast measurement, not a native checkpoint — carried verbatim on the board including its qualifier), and P91's owed `logs/*.jsonl` parse plus its do-not-merge instruction. P91's user decisions and its three architectural rulings were relocated to the board's `## Accepted decisions that must survive compaction` section; its open security findings (F7, F9) and its **seven** SHOULD-FIX / contract follow-ups were relocated to `### P91 — open items` under `## OPEN follow-ups`. Nothing here was closed by the curator.

**Sub-part map** (the section headings below are verbatim, so the `54.x` numbers cited from
`TODO.md`, `docs/history/README.md` and `docs/contracts/INDEX.md` resolve as follows):
**54.1** = the `USER CHECKPOINTS — ALL CONFIRMED 2026-09-10` banner ·
**54.2** = `P102 + P105 — hue audit` ·
**54.3** = `P106 — status-badge ink` ·
**54.4** = `P107 — hue-over-own-tint` ·
**54.5** = `P108 — hue-as-text on neutral surfaces` ·
**54.6** = `P91 — Observability`.

## ✅ USER CHECKPOINTS — ALL CONFIRMED 2026-09-10

The user confirmed **all eight** native-verification items in one pass on 2026-09-10: the seven
milestones below plus the `8dd5b24` CSP change. Each section keeps its own AC list as the record of
what was verified. **This whole block is now archivable** — it was the thing blocking ~470 lines of
compaction.

**One item in this block is NOT closed by that confirmation: P108's `AC11`.** It is an owed
*AI-gate* contrast measurement, not a native checkpoint — two states could not be reached in the
harness and the two figures are source-derived. A human cannot confirm a 3.05 contrast ratio by
looking at it, so it stays open. See P108 below.

None may be archived. The orchestrator never self-declares the native half. Each entry below is the
resume summary; the full review/implementation narrative is in the archive part named.

**Two of the seven are written further down, under PENDING / queued, to keep their narrative next to
the work that produced them: P110 (selection flicker) and P109 (status-badge semantics). They are
awaiting a checkpoint exactly like the five here — the placement is chronological, not a status.**

### ✅ P102 + P105 — hue audit — **DONE** (AI gate green; AC18 / AC19 / AC20 **USER CHECKPOINT CONFIRMED by the user 2026-09-10.**)

**Current step:** AI gate green; milestone complete bar the checkpoint.
Contract `7c623d8` (`docs/contracts/P102-P105-hue-audit-ui.md`) → impl `0e5dcab` → both reviews
APPROVE after fixes `185c352`. Full narrative: archive Part 37.

- **Gate:** `--quick` all 7 steps green (255.9s) **and** e2e **181 passed / 1 skipped** (2.7m).
- **AC18** — the updater panel is Tauri-only, but its `.btn-danger` shares the exact rule measured at
  4.80/4.93 rest and 5.18/5.49 hover, so the question is "does it look right", not "is it legible".
- **AC19** — `--accent-strong` holds hue within **0.7°** of `--accent` in both themes (219.0° vs
  219.7°); the open question is purely whether the dark `#7fabff` reads washed-out.
- **AC20** — no `filter` remains on any of the four hover targets, so nothing is left to flicker and
  layout cannot move (background-only swap); fill-change feel alone is judged.
- The user's 2026-09-02 checkpoint authority was scoped to P100 + P101 and explicitly does **not**
  reach work created that session.
- **The predicted grep residue matched reality on every line** — the first milestone in the
  programme to do so, and the standard the later ones inherited.
- **Orchestrator decision (§5.4, Option A):** added `--merged` / `--merged-text` rather than keep the
  merged-PR purple `#8957e5` literal — a token-consistency call inside the design system's own
  logic, **not** a product-identity call, therefore not a checkpoint.
- **AC16 is partial:** its stylelint clause is **unsatisfiable — no stylelint exists in this repo**.
- **No architect pass**, consistent with P100/P101: a CSS contrast audit has no module boundary, IPC
  surface or algorithm. Recorded so the skipped step is not read as an oversight.

### ✅ P106 — status-badge ink — **DONE** `10ce967` (AC14 / AC15 + the real-repo half of AC9 **USER CHECKPOINT CONFIRMED by the user 2026-09-10.**)

Contract `docs/contracts/P106-status-badge-ink-ui.md`. Full narrative: archive Part 38.

- 8 render sites in 7 files; buckets **5 FIX · 1 KEEP · 2 no-hue**.
- **The letter is the SOLE status carrier at all 8 sites**, so nothing is exempt; the badge does not
  scale with `--rp-row-font`, so this holds in both densities.
- Three new **ink-only** tokens (`--danger-strong`, `--success-strong`, `--warning-strong`) mirroring
  `--accent-strong`. **`--warning-text` was explicitly NOT the answer** — the `--*-text` family is
  **fill-ink** and measures 1.19/1.16 as ink on the staged tint.
- **Shipped result:** worst live badge anywhere is **5.18** (the `C` on a selected row in light) —
  exactly the predicted floor, verified live in both themes with the full ancestor stack composited.
- A **fourth** search pass was added over P107's three — imperative canvas rendering
  (`rg "FileStatus" src/graph` → 0) — because "the search couldn't see a whole class" is how both
  prior counts in this programme went wrong.
- Orchestrator decisions: D1 accepted (the two colourless badges get hue, isolated as the
  deliberately droppable AC12); `--warning-strong` dark = `#e3b341`; AC13's `ui-reference.md` update
  lands **after** implementation, from the shipped commit.

### ✅ P107 — hue-over-own-tint — **DONE** `2168057` (AC11 / AC12 / AC13 **USER CHECKPOINT CONFIRMED by the user 2026-09-10.**)

Contract `docs/contracts/P107-hue-over-own-tint-ui.md` (`59061b2`). Impl shipped **`2168057`**
(7/7 predicted residue metrics matched); errata **`ef06e6b`**. Full narrative: archive Part 40.

**Status restated by the orchestrator, 2026-09-03.** The old heading "CONTRACT DONE, IMPL PENDING"
was written before `2168057` and was simply stale. The curator correctly refused to change it —
restating a status is not a curator's call — so it is corrected here instead. **This records what
shipped; it does not upgrade a checkpoint:** AC11 / AC12 / AC13 remain **pending USER CHECKPOINTs**
and only the user can close them.

- Population is **38 call sites**, not the 16 found first and not the **6** `ui-reference.md` §2
  claimed. Buckets: **17 failing text · 19 compliant glyph KEEPS · 1 failing glyph state · 1 owned
  by P106**. All seven predicted residue metrics matched exactly.
- The 19 keeps matter as much as the fixes — a blanket sweep would have wrecked the toast,
  submodule, ai-dock and git-dock pill recipes.
- The three-pass search is the durable deliverable: (i) same-rule-block; (ii) a multiline
  descendant-combinator grep; (iii) two classes even (ii) cannot see — **unqualified child classes
  whose only parent is tinted** (found by reading the 25 tinted containers' components) and
  **custom-property indirection via `--h`** (11 instances across 4 families no hue-name grep reaches).
- **Zero new tokens.** Recorded trap: `--danger-text` on a 14% danger tint is **1.27:1**.
- A `ui-reference.md` §2 recipe was **retracted, not patched**: a 35% hue edge measures 1.58/1.69 —
  decoration, never an identity carrier. Solid-hue bars pass (4.41-7.92) and stay.
- **Errata reported rather than papered over** (`ef06e6b`): AC2's thresholds were default-state
  minima, restated per state; §5's "2.76 / 3.07" does not reproduce — the real value is 3.37 / 3.30.
  **P107 must not be re-opened against the original AC2 wording.**

### ⚠️ P108 — hue-as-text on neutral surfaces — `42206fd`; AC12 / AC13 / AC14 **USER CHECKPOINT CONFIRMED by the user 2026-09-10.**
**NOT fully done — `AC11` is still OWED** and is an AI-gate measurement, not a native checkpoint, so
the user's confirmation does not close it. This is the only milestone in the block that stays open.

Contract `8027cef` (`docs/contracts/P108-hue-as-text-on-neutral-ui.md`) → impl `42206fd`.
17 CSS files, no TS/TSX, no DOM change, no new tokens. Full narrative: archive Part 39.

- All five residue rows matched, and all five pre-fix baselines were **re-measured in this tree**
  rather than inherited. Post-fix minimum anywhere is **5.10** (`--danger-strong` on `--bg-3`,
  light), matching the predicted family minimum exactly.
- **The count was 62, not the 48 handed over — the FIFTH wrong count in this programme.**
- Two defects a grep structurally could not find: `settings-legacy-sections.css:134` was
  `color: var(--warn, #b8860b)` and **`--warn` is defined NOWHERE**, so it had always painted a
  theme-invariant literal at 3.25:1 in light; and `commit-panel.css:98-100` served **one declaration
  to two selectors**, icon half a compliant glyph, text half failing at 4.41 dark.
- The canvas pass found **zero** — 7 draw sites in `src/graph/**`, all glyph, all ≥3:1. 0 fixes,
  **but only because it was measured**.
- `--warning-text` is **undefined**, so a naive probe returns the *inherited* colour (13.54/15.42),
  not the 1.09/1.07 the contract cites — same undefined-property trap class as `--warn`.
- §9's four mock fixture states were **not** added: three are already reachable; only the long-error
  and `dev-status-write-failed` states would need new mock code.

**AC11 is OWED — recorded, not quietly dropped** (carried verbatim; the qualifier is the point):

> Two states could not be reached: `.file-count-del` selected (3.05) and
> `.context-menu-item[data-tone='danger']` hovered (3.05). **The orchestrator also tried and
> failed**, across ~8 harness rounds: keyboard nav focuses `.graph-scroll` but never mounts the diff
> panes; `?forge=auth` does not render `.file-count-*`; and synthetic `contextmenu` events do not
> open the menu because React requires **trusted** input. Both figures are source-derived and
> unverified. Recording it as owed rather than manufacturing a pass — the same call the implementing
> agent made, and the standard this programme applies to its agents applies to the orchestrator too.

### ⚠️ P91 — Observability: Dev mode, structured logs, local telemetry & metrics — checkpoint **USER CHECKPOINT CONFIRMED by the user 2026-09-10.**
**One AI-gate item may still be owed:** the real `logs/*.jsonl` parse from a `pnpm tauri dev`
boot+idle. If the user's verification run booted the app, those logs now exist on disk and this is
finally doable — the orchestrator should offer to parse them rather than assume either way.
**Branch merge is a separate decision and is NOT covered by the checkpoint confirmation** — see the
do-not-merge note below.

Branch `feat/p91-observability` (this branch). **Not merged to `dev`; do not merge without the user**
— the user confirmed 2026-08-31 that the branch is WIP, and its own commit messages must not be read
as an authoritative status. Build diary: archive Part 44. Security arc: Part 42. Audit: Part 43.

- **Contracts:** `docs/contracts/P91-observability.md` · `P91-observability-ui.md` ·
  `P91-raw-args-privacy.md` · `P91-privacy-copy-ui.md` · `ui-reference.md` §12.11.
- **Goal:** make unintended app behaviour mechanically visible — double triggers, redundant IPC,
  effects on unchanged deps, echo-induced refreshes, superseded results — without reading 50k lines.
- All 7 increments implemented, reviewed and committed; the security arc is complete
  (`a287a53` → `834f2d1` → `120cadd` → `c0abbe1` → `2523426` → `2fb647e` → `0a785b3` → `b26833f`).
- **Full gate green at `b26833f`** (7 non-e2e steps); **e2e 181 passed / 1 skipped / 0 failed** after
  `23ee2b0`. `cargo obs::` went **135 → 146 → 158 → 162** across the arc.
- **Still owed:** the real `logs/*.jsonl` parse from a `pnpm tauri dev` boot+idle, and USER
  CHECKPOINT (a)-(f) plus the 7b/7c native items — **asked, not declared**.

**User decisions on P91 that must survive compaction (all 2026-08-27 unless noted):**

- Redaction conservative by default, opt-in raw-names toggle; credentials never logged in any mode.
- Metrics storage = **rolled-up JSON**, not SQLite.
- React instrumentation = **SIX surfaces** — the user added the **left sidebar** to the original five.
- Logs are **NOT** auto-deleted when Dev mode goes off (prune by caps only); per-session log files;
  `metrics_reset` ships headless.
- Decision 7 — **"Delete all log files" ships in v1**, model = **roll-then-purge**, scope hard-limited
  to `logs/*.jsonl` + `.tmp`, never `metrics/` or `settings.json`. Success copy must say "Still
  recording" when `rolled:true`.
- **USER GATE (2026-08-27):** each increment requires its own explicit go from the user.
- **Branch policy (USER, 2026-09-02):** everything this session lands on `feat/p91-observability`;
  local commits only, no push.

**Three architectural rulings not to re-open:**

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

**Security findings still open** (F1-F5 fixed; full text archive Part 43):

- **F6** — `usage.json` disclosure → FOR USER item 2 above.
- **Home-directory / username masking** → FOR USER item 3 above.
- **F7 — LOW, mostly latent.** `redact_names` misses bare ref/file names and never touches JSON keys;
  a branch like `feature/acme-client-migration` would be written verbatim into a strict file. Not
  reachable today, but `strict::enforce` is the **sole** enforcement point for both Rust and frontend
  records, so a gap there is a single point of failure.
- **F9 — INFO.** The two redactors cannot disagree, because **only one enforces**: `redact.ts` has no
  equivalent of `redact_names`. That is the correct architecture, and it is why F7's gaps matter more
  than their reachability suggests.
- **Verified CLEAN, so a later session does not re-audit:** CSP + capabilities (script-src self-only,
  no unsafe-inline/eval, no shell or fs plugin) · updater trust chain, no key material in the repo ·
  external-process launching uses argument vectors, never a shell string · forge credentials via the
  OS keychain · log rotation/purge have no traversal · error strings never cross IPC ·
  zero-cost-when-off is structural on both sides.

**P91 SHOULD-FIX follow-ups, still open** (non-blocking; full text archive Part 45):

- **`SAVE_LOCK` orders the rename pair but NOT the snapshot** (`metrics.rs:379-382`, `:409-425`).
  Two savers can snapshot A→B but acquire `SAVE_LOCK` B→A, so older bytes land last — and the
  dangerous instance is exactly the pair the doc cites as its motivation: **a `metrics_reset` can be
  silently undone on disk**. No deadlock risk (verified). Fix, or amend the overstated doc at
  `metrics_file.rs:52-66`.
- **The `last_fire` prune assumes non-decreasing `ts`** (`window.rs:52-63`) and the comment states it
  unconditionally. `ts` comes from two unsynchronised clocks (`src/obs/log.ts:43`, `sink.rs:385`)
  with no monotonic clamp. Blast radius is a **duplicate** anomaly record, never a missed one.
- **NIT:** `dup_ipc_debounce_map_stays_bounded_over_a_long_session` spaces events 100 ms apart against
  a 300 ms window, making `len <= 4` nearly tautological. `last_fire` has **no numeric cap**, unlike
  `open_calls` (FIFO 1024) and `slow` (LRU 200).
- **`P91-observability-ui.md:496` and `:951` are stale** — both still describe
  `log_export_session(dest)` and a native save dialog **that never existed**. → `ui-designer`.
- **Contract consolidation (architect recommendation, not done):** fold `P91-raw-args-privacy.md`
  into `P91-observability.md` (≈ −250 active lines) when §13 rows 1-15 are archived; keep
  `P91-privacy-copy-ui.md` standalone since it is `ui-designer`-owned.
- **Contract follow-ups owed to `architect`:** §6/§10 still specify the removed
  `log_export_session(dest?)`; §8/§8.1 must record that `cmd.*` keys are camelCase `IpcApi` names
  **and that they recorded nothing before the fix**; `MAX_KEYS_PER_MAP = 512` + the `meta.overflow`
  bucket needs ratification; decision 25's counter-key shape now literally requires the dot.
- **`.forge-connect-link:hover` is now a no-op** — the resting-underline MUST-FIX means hover
  declares the same underline, so the link has **no hover feedback at all**. → `ui-designer`.


---

## Part 55 — P110 (selection flicker on a background graph re-stream + watcher burst scoping) and P109 (status-badge semantics), verbatim, moved off the board 2026-09-10. Both were in the `## PENDING / queued` section rather than the checkpoint block, "chronological, not a status" per the board's own note. Both had their USER CHECKPOINT confirmed by the user 2026-09-10 (`548cc0a`). **P110's "latent pre-existing gap" candidate follow-up (op-state files excluded from the watcher filter) is carried forward as a live one-liner on the board.**

## PENDING / queued

### ✅ P110 — selection flicker on a background graph re-stream — **DONE** (**USER CHECKPOINT CONFIRMED by the user 2026-09-10.**)

**Current step:** both parts AI-gate green (flicker fix `84bbf85`, 479.3s; watcher scoping, 617.9s);
native-window confirmation is the only half left.

User report: after checking out an older branch with many commits after it, the right panel and the
graph selection flipped between the selected commit and the working-directory ("Uncommitted changes")
view several times over a few seconds, on every background refresh round.

- **Root cause:** the selection was a ROW INDEX, but every refresh round re-streams the graph from
  row 0. Between the first paintable chunk and the chunk carrying the selected commit's row,
  `graph.nodes[selectedIndex]` was `undefined`; `WorkspaceRightPanel` fell back to the status panel
  by design, and `selectedOid` went null — also resetting `scope` and closing the commit browser.
  The deeper the selected commit sits, the longer that gap is visible.
- **Fix:** `useStickySelection.ts` anchors the selection to an `(index, node)` pair and holds it ONLY
  while the index is unchanged (the re-stream gap). `WorkspaceRightPanel` now takes `selectedNode`
  and never indexes `graph.nodes`, so the audit §2.2 deref is structurally impossible.
  `useWipSummary.ts` stabilises `WipSummary` identity, and `useGraphCanvasEffects` deps on
  `wip !== null` — the boolean it actually reads — so staging a file no longer re-runs the
  scroll-into-view adjustment.
- **Reviewer:** approve, no MUST-FIX. SHOULD-FIX 1 was pulled forward, not deferred: the anchor
  originally survived an index CHANGE to an unstreamed row, reachable via `handleSelectParent`, which
  would have shown the previous commit's details *and its action targets* under a moved selection.
  Its regression test was verified failing (2 failed / 8 passed) against the pre-fix implementation.
- **Deliberately NOT done:** the canvas selected-row highlight is not sticky. The highlight is
  row-index-based and there is no honest row to draw while the row is absent from the partial layout;
  anchoring it to a stale index is exactly the wrong-commit hazard the fix removes.
- **USER CHECKPOINT:** `pnpm tauri dev`, check out an old branch with heavy history after it, select a
  commit deep in the graph and leave the window alone through several refresh rounds — the right panel
  must stay on that commit.
- **Follow-up DONE (user asked for it the same session):** the *frequency* of these rounds. A checkout
  that rewrites thousands of working-tree files storms the `notify` watcher, and each debounced burst
  ran a `full` scope round including a complete graph re-stream. Contract:
  `docs/contracts/P110-watcher-burst-scoping.md`.
  - The watcher ALREADY classified paths (`is_relevant`) and threw the answer away. `classify()` is now
    both the relevance filter and the graph-affecting classification, so the two cannot drift.
  - A burst is refs-class if ANY path in it is refs-class (`refs |= t.refs`); worktree-only bursts emit
    `reason: "fsWorktree"` → `refresh('watcher', 'worktree')`. Same origin (still echo-suppressible),
    narrower scope. `"fs"` keeps its exact prior meaning; unknown reason still → `full`.
  - **`.git/index` is refs-class, deliberately.** Post-change, correctness depends on observing the
    REF-file event specifically, and ReadDirectoryChangesW drops events on Windows — the index write is
    the redundancy that survives a dropped `refs/heads/main`. Costs one graph re-stream per checkout
    (the index is written once), not one per burst.
  - **`submodules` added to the `worktree` slice** — a regression the increment itself introduced:
    `git -C sub checkout other` writes refs under `.git/modules/sub/refs/**` (classified noise,
    pre-existing) + worktree files, so the burst is worktree-only and the submodule panel went stale.
    Fixed by restoring the refresh BEHAVIOUR, not by widening WHICH bursts fire.
  - Reviewer walked every git op (commit/reset/merge/rebase/cherry-pick/stash/fetch/gc/submodule) for a
    graph-affecting change landing in a worktree-only burst: none found. Relevance set proven unchanged.
  - The accumulation rule is now proven by mutation: deleting `refs |= t.refs` fails
    `burst_accumulation_is_conservative` deterministically. The prior test passed without it.

**Latent pre-existing gap, NOT introduced here (candidate follow-up):** op-state files (`MERGE_HEAD`,
`REBASE_HEAD`, `rebase-merge/**`, `CHERRY_PICK_HEAD`) are excluded by the filter and never triggered a
refresh before or after P110. Op-state freshness during a conflicted rebase rides on incidental worktree
churn. Deliberately left alone — changing it would widen which bursts fire.


### ✅ P109 — status-badge semantics — **DONE** `5a254ba` / `5c2dcd2` (AC13 / AC14 **USER CHECKPOINT CONFIRMED by the user 2026-09-10.**)

**Current step:** AC1–AC12 green; AC13/AC14 are the native-window half and are the only thing left.
*(This entry said "pending" until 2026-09-03 — a board defect. The work shipped in this session; the
filing text was never upgraded. Corrected here so no future session re-opens it.)*

**The defect, as filed from P106:** two distinct statuses rendered the **same character**
(`StatusFileRow.tsx:15`) and the badge carried **no accessible name** — the distinction was
unavailable to a screen reader *and* ambiguous visually. Deliberately **not** rolled into P106: one
defect class per milestone is what kept P95/P98/P100/P102/P105/P107 reviewable, and this is
a11y/semantics, not contrast. P106 makes the letter *legible*; P109 is about the letter being
*insufficient*.

**Shipped:** one `src/components/FileStatusBadge.tsx` (58 lines) replaces **six** independently
drifted `BADGES` tables. `role="img"` + `aria-label`, names `Added / Modified / Deleted / Renamed /
Type changed / Conflicted / Untracked / Status unknown`. AC8 held — `src/styles/**` byte-identical,
zero token or geometry diff; contrast is carried over from P106, not re-derived (AC9).

**It also corrected P106's premise.** P106 was filed believing `added` and `untracked` shared `A`
across the board; they shared it in **3 of 6** tables. The delta was exact both times — see the
grep-counting rules below.

**Contract:** `docs/contracts/P109-status-badge-semantics-ui.md` (AC13/AC14 spelled out in §13).
- **AC13** — screen-reader read-through in the native window with a real AT.
- **AC14** — the `U` glyph read at 11 px in the native window: confirm `U` is instantly legible.


---

## Part 56 — The `Closed on 2026-09-03` one-liners and the full `SEC-2026-09-03` external-process-launching section, verbatim, moved off the board 2026-09-10. Their remediations are all committed. **What stays live on the board:** SEC's residual symlink case, its test-gap line (now partially closed and flagged as a contradiction), and MEDIUM-2 / LOW-1, which are FOR USER item 3. The section's INFO(CSP) item was fixed in `8dd5b24` and user-confirmed 2026-09-10.

### ✅ Closed on 2026-09-03 — one line each, narratives in archive Part 52

The board's own record of having been wrong stayed useful twice today, so these lines stay even
though the work is done. Full text: `docs/history/todo-archive-2026-09.md` Part 52.

- **P107 F2 — the copy-candidate chip said "unchecked" on ticked rows** — SHIPPED `8337d9b`.
  Contract `docs/contracts/P107-F2-copy-chip-ui.md`. `WorktreeCopyCandidates.tsx`'s `previewFailed`
  branch read as the exact opposite of the truth; now `unknown` on a neutral
  `.wt-copy-chip--unknown`. `.wt-copy-chip` itself byte-identical, so P107 A16's 11.31 / 12.21 stands.
- **Same increment, P91:** two shipped strings still promised the folder picker security audit **F4**
  removed; corrected. Both states were unreachable in the harness until `?wtCopyPreviewFail=1` and
  `?obsExportFail=space|permission|other` were added — the latter reached three `exportErrorText`
  branches that had **no route at all**. Both verified in the harness.
- ~~**`.settings-toggle-btn.is-active` is dead styling**~~ — **ALREADY DONE**; P107 `2168057` had
  deleted both rules two commits earlier. Board was stale (2nd such miss that day, after P109).
- ~~**`pr-badge-placement-ui.md` documents the merged pill as `#8957e5`**~~ — **DONE** `fc9c36e`.
  Worse than filed: `ui-reference.md` carried the stale literal at `:1038`/`:1048` while `:71`
  recorded it replaced — the canonical design system contradicting itself. All five repointed.

### ✅ SEC-2026-09-03 — external-process launching: repo-authored paths were unvalidated — REMEDIATED `0806596`

Full report: `docs/audit-2026-09-03-external-launch.md` (`7e426c3`). Remediation delegated the same
day; this entry is the resume point.

**The finding in one line:** `crates/bonsai-core/src/external.rs:21-23` accepts residual risks on the
stated grounds that `{path}` "is a repo path the user already opened — so this is self-inflicted at
worst, never attacker-controlled". That is false. `{path}` is also `sub.absPath`, built at
`crates/bonsai-core/src/git/submodule.rs:113` as `sm_workdir.join(sm.path())` with **no validation**,
from a `.gitmodules` value authored by whoever wrote the cloned repo.

- **HIGH-1** — a `;` in a tracked directory name reaches `wt -d`, and `wt` splits its own arguments
  into sub-commands. Reachable with **no user template**, since `wt` is rung 1 of the default ladder.
- **HIGH-2** — `Path::join` **discards the base** for a rooted or UNC path, so `//host/share` reaches
  `Path::exists()` (`src-tauri/src/commands/external.rs:72`) and Win32 dials it → NetNTLMv2.
- **MEDIUM-1** — no containment check anywhere. A hostile `payload.exe` as a submodule path is
  currently blocked only **by accident**: `cwd` is the target and `current_dir` on a file fails with
  `NotADirectory` before ShellExecute. Adding `explorer /select,<file>` would silently remove it.
- **MEDIUM-2 / LOW-1 / LOW-2 / LOW-3 / INFO(CSP)** — see the report; each is its own increment.

**Both formerly-unverified steps were measured, and one moved a severity:**
- `wt` splitting `;` inside one argv token — **CONFIRMED**. HIGH-1 stands.
- HIGH-2 zero-click via `submodule_status` — **DISPROVEN**, so it is **click-triggered**. libgit2
  concatenates `sm->path` under the workdir and does not honour the escape Rust's `join` does. The
  SMB callout is real (UNC-loopback `exists()` 6.6 ms vs 19 µs local — it dials); it needs the click.
- The audit was **wrong** that `C:/Windows` and `..` reach `join` — libgit2 drops both first. The
  residual escaping set is exactly **rooted** and **UNC**. The path must also be **quoted** in
  `.gitmodules`, or `;`/`#` are config comments and never reach us.

**Fixed** by `contained_abs_path` (`crates/bonsai-core/src/git/submodule_abs_path.rs`): all-`Normal`
+ relative proves containment **without touching the filesystem**, so an uninitialized submodule
still works; when the target exists it also canonicalizes and re-checks against symlink escape. A
rejected submodule is **still listed** with `absPath: null` rather than dropped — hiding a submodule
git itself reports is its own correctness bug. Proven red both ways, with positive controls that
stayed green so the validator is not the vacuous kind.

**STILL OPEN, each its own increment:** MEDIUM-2 (`terminalCommand`/`editorCommand` unvalidated —
renderer compromise still converts to local execution), LOW-1 (cwd DLL search order), INFO (CSP
`form-action` / `base-uri` / `object-src`).

**One residual documented, not closed:** a symlink introduced inside an already-checked-out
superproject at a not-yet-created leaf bypasses the canonicalize recheck (`canonicalize` fails on a
missing leaf). Primary vectors are closed lexically regardless of filesystem state.

**What is well defended, recorded so it is not re-audited:** shell-free spawn on every platform,
`tauri-plugin-shell` not a dependency at all, `parse_template` substitutes inside an already-tokenized
argv element, private URL ladder behind mandatory validation, `resolve_program` never consults the
CWD, capabilities narrowed deliberately, `script-src 'self'` with no `unsafe-inline`.

**Test gap, and it is the same shape as the comment:** `external_tests.rs` is thorough on hostile
**templates** and has **zero** path-hostility cases. `submodule_info`'s `abs_path` has no test at all.


---

## Part 57 — The `Queued housekeeping (none blocking)` section, verbatim, moved off the board 2026-09-10. The `src/styles/forge-pr.css` split (`e149382`) is DONE and its proof narrative moved here in full. **What stays live on the board, condensed to one line each:** the three items found during the split and deliberately NOT fixed (`context-menu.css:89` pointing into `forge-account.css`, the duplicate `.pr-create-actions` rule, the misplaced generic `.btn-secondary-danger`), the owed `image_diff_cli_2.rs` numbered split, the 90-char branch-name chip, the `ui-reference.md` curation pass, and the two deliberately-not-taken velocity items.

### Queued housekeeping (none blocking)

- ~~**`src/styles/forge-pr.css` is 890 lines**~~ — **DONE** `e149382`. Split into five modules
  (213 / 207 / 220 / 180 / 87). Cascade order was the constraint, so every chunk is a **contiguous
  original line range in original order** — correct by construction. Proven twice: the concatenation
  diffs empty against the original body (887 lines each side), and the **emitted stylesheet is
  byte-identical** — `pnpm build` yields the same Vite content-hashed filename before and after, and
  since Vite derives that name from the content, the match *is* the proof.
  *(The curator observed this mid-flight and correctly logged it as "in flight, not a status
  upgrade" rather than ticking it. It was this session's own uncommitted work, not a peer's.)*
- **The ~500-line limit had never been enforced for CSS** — `scripts/check-file-size.mjs`
  `SCAN_TARGETS` covered `crates/**.rs`, `src-tauri/src/**.rs`, `src/**.{ts,tsx}` and **no `.css` at
  all**, which is how 890 lines went unnoticed. `.css` added in `e149382`, and it was **free**: after
  the split zero CSS files exceed the limit, so it added no baseline entries and
  `file-size-baseline.json` is byte-unchanged. Three items were found during the split and
  deliberately NOT fixed (each would reorder the cascade or cross into another file):
  `context-menu.css:89` now points at a rule that lives in `forge-account.css`; a duplicate
  `.pr-create-actions` rule in `forge-pr-create.css`; and the generic `.btn-secondary-danger`
  sitting in `forge-pr-create.css` where it thematically belongs with `controls.css`.
- **`image_diff_cli_2.rs`** numbered split still owed — renaming changes nextest IDs, so it needs its
  own increment where that IS the expected diff. Path re-verified 2026-09-03:
  `crates/bonsai-core/tests/diff/image_diff_cli_2.rs` (it moved under `tests/diff/`; the board never
  carried a path).
- **The 90-char branch-name chip** becomes a 50 px two-line stadium at `border-radius: 999px` —
  pre-existing, newly visible because P102/P105 added the fixture that reaches it.
- **`ui-reference.md` is growing fast** (§2 now carries a 16-row evidence table) — worth its own
  curation pass.
- **Two velocity items filed and deliberately NOT taken** (2026-09-03): C1 could drop 17s → 11s by
  giving one surface its own test and its own corrupted repo — **not taken**, it changes the shape of
  a crash-safety test for ~6s; and `crates/bonsai-mcp/tests/common/mod.rs:131-133` still spawns 3
  `git config` calls (board said `:127`; re-measured 2026-09-03 — same fix applies verbatim; left
  alone to keep the blast radius in one crate).


---

## Part 58 — The `GATE STATE at c218258` block and the `Earlier gate states` block, verbatim, moved off the board 2026-09-10. Both are **superseded**: the current gate state is the full 8-step green at `1d8c6f9` (2026-09-10, 452.5s) recorded in the board's RESUME HERE "Verification state", plus the later 542.4s run over the `e9d025d` + `7f9f16b` pair (`7f9f16b` is docs/contracts only, so the gate cannot have run *at* it). **The rules these blocks earned stay live on the board** — they were consolidated into `## Durable lessons — the rules` as `### The gate-running rules`, and they are: verify machine state at the time a timing-sensitive failure ran; a slow timing number is evidence about the machine until proven otherwise; verifying machine state is a standing pre-gate step; the e2e leg will fail intermittently on a loaded machine (P104); run the gate idle or e2e with `--workers=1`; **give subagents `--workspace`, not `-p <crate>`, whenever a change crosses a crate boundary**; redirect the whole gate log to a file; and read the `gate summary` block, never the exit status alone. The `FIRST_PAINT_TIMEOUT = 15 s` fact (`c6cd7dd`, measured at 7.6x p99) is recorded here.

## ✅ GATE STATE at `c218258` — **ALL 8 STEPS PASSED** (2026-09-03, later run)

> **Superseded by `1d8c6f9` (2026-09-10, 452.5s, all 8 green)** — see "Verification state" in the
> RESUME HERE block for the per-step numbers. Kept for the archive trail; curator may fold it in.

496.4 s total. Machine **not** fully idle this time — CPU sampled 21-64% with no build running (that
is the harness/MCP floor), so these are pass/fail evidence and **must not be compared as timings**
against the `5c2dcd2` idle baseline (archive Part 51). Recorded that way deliberately.

| step | time |
|---|---|
| `cargo nextest` | 152.4 s (2334 passed, 8 skipped, 1 leaky) |
| doctests | 3.8 s |
| clippy | 5.5 s |
| eslint | 12.7 s |
| **file-size ratchet** | 0.89 s |
| vitest | 73.7 s |
| tsc + vite build | 13.4 s |
| **e2e playwright** | **234.0 s** |

Covers the 12 commits after `5c2dcd2`: the mock-IPC `window` guard, the launcher hoist + timer
fix/revert, the P107 F2 chip, the `#8957e5` and P91-export doc corrections, the staleness sweep, and
the SEC-2026-09-03 remediation.

**The FIRST run of this gate FAILED**, and how it failed is worth keeping:

- `cargo nextest` failed one test. My remediation brief scoped verification to
  `cargo nextest -p bonsai-core`, so the `bonsai` crate's tests under `src-tauri/` never ran — and one
  of them asserted the exact path-echo behaviour audit LOW-2 removed. **Give subagents
  `--workspace`, not `-p <crate>`, whenever a change crosses a crate boundary.** Fixed in `c218258`
  by repointing the assertion (not deleting it — its echo was the discriminator proving the error came
  from *our* guard, a job the category string now does) and adding the file-target case MEDIUM-1 was
  actually about, which had no test at all.
- **I lost the failure detail by piping the gate through `tail -60`** and had to re-run the Rust leg
  to find it. Redirect the whole log to a file; read the summary from there.
- **The background wrapper reported exit 0 while the gate had failed** — that was the pipeline's exit
  code, not the gate's. Read the `gate summary` block, never the exit status alone.

**Minor, filed not fixed:** the gate script itself emits Node `DEP0190` — it passes args to a child
process with `shell: true`, which concatenates rather than escapes them. Repo tooling, not shipped
code, but it is the same argv-vs-shell class the audit just spent a day on.

---

## Earlier gate states — archived, with the rules they earned

- **`5c2dcd2` — ALL 8 STEPS PASSED, 362.0 s (2026-09-03)**, on a machine verified idle first (CPU
  ~1%, zero `bonsai-scratch`/`load.mjs` processes, 42 GB free). This is the **idle baseline** the
  `c218258` block above must not be timed against. Per-step table: archive Part 51.
- **`c6cd7dd`** — kept on the board until 2026-09-03 for its reasoning, not its status; now archive
  Part 51. All 7 non-e2e steps green in every run regardless of load; e2e 181 passed / 1 skipped in
  172.2 s run alone on a verified-idle machine.
- The **e2e mis-diagnosis story** (three "failing" gate runs, none a code defect: my own diagnosis
  agent's 18-thread `load.mjs` generator left running, an overlapping agent run I asserted was clean,
  and 34 "leaking Playwright browsers" that were the user's own `msedge`) is archive Part 51.

**Rules earned there — these stay on the board:**

- **Before trusting any timing-sensitive failure, verify machine state AT THE TIME IT RAN** — sample
  CPU repeatedly, look for scratch/load processes, confirm no agent is mid-run.
- A slow timing number is **evidence about the machine** until proven otherwise. Timing analogue of
  the grep-counting rule: *measure the baseline, do not infer it.*
- **Verifying machine state before running the gate is a standing pre-gate step**, not a nicety.
- The e2e leg **will** fail intermittently on a loaded machine — expected, documented at P104 (Edge
  misses a hardcoded 30 s CDP close window, then a blocking `taskkill` runs).
- `FIRST_PAINT_TIMEOUT = 15 s` (`c6cd7dd`) removed the largest source, measured at 7.6× p99.
- **Run the gate on an otherwise-idle machine, or run e2e with `--workers=1`.**


---

## Part 59 — The 2026-09-10 session: the P111 pill-truncation fix, the P87b FU-1 run-target milestone, and the closure of the six remaining reviewer follow-ups — verbatim, moved off the board 2026-09-10. **All three contracts explicitly declare no USER CHECKPOINT item** (`P111-pill-truncation-ui.md:326` "Not a USER CHECKPOINT — every surface here is reachable in the browser harness"; `P87b-FU1-run-target.md:425` "AI gate — **no USER CHECKPOINT item**"; `P87b-FU1-FU4-git-dock-ui.md:385` "**No USER CHECKPOINT item** in this contract"), so archiving their detail does not archive a pending native half. **What stays live on the board:** P111's `.asset-chip` R2 `max-width` residue; RESUME HERE items `1b` / `1c` / `1d` (the doc + hygiene residue owed to `architect` / `ui-designer`), item `2b` (the `src/obs/types.ts:68` A26 letter-scheme decision), item `3` (the two open security calls); and the `whichAll` PATH_EXTS trap, kept as a one-line standing warning because it is exactly the kind of fix a later session would "simplify" back.

### 59.1 — P111 (`1192f2a`)

### ✅ P111 is DONE — `1192f2a`

The interrupted senior-dev work was finished rather than reverted: it compiled and its unit tests
passed, so the cheaper path was to verify it against the contract. Doing so found a defect the
interrupted work had **decided to accept** (its own fixture comment said the no-slash case
"hard-clips and the `title` recovers it"): `.ref-label-leaf` was `flex: none`, so a leaf wider than
the chip overflowed by a measured **174px** and was hard-clipped by `overflow: hidden` — cut with no
ellipsis. Worse than the original bug, because a hard clip makes a truncated ref look **complete**
and `title` is hover-only. Fixed by shrink ratio (head `flex-shrink: 999` vs leaf `1`), so the leaf
ellipsizes only as a last resort. `e2e/33-pill-truncation.spec.ts` guards it, proven red.

**Residue, filed not fixed:** `.asset-chip` got the R1 line guard but no R2 `max-width`, so a model
id far longer than the fixture would widen the chip rather than ellipsize.


### 59.2 — P87b FU-1 (`1d8c6f9`)

1. **✅ FU-1 is DONE — `1d8c6f9`.** Both halves shipped, both reviews approved with **no
   MUST-FIX**, all 5 SHOULD-FIX + 7 NITs applied, **full 8-step gate green (452.5s, 2344 Rust
   tests, 185 e2e)**. Contracts: `docs/contracts/P87b-FU1-run-target.md` and
   `docs/contracts/P87b-FU1-FU4-git-dock-ui.md` (the board previously mis-cited the latter as
   `P87b-FU1-git-dock-ui.md`, which never existed — fixed 2026-09-10).
   Built as two concurrent senior-dev passes on disjoint file sets (run-target F-5 said the halves
   are independent; that held). Harness ACs §9.9-13 all verified — the bidi proof is a code-point
   dump (`6f 72 69 67 69 6e 2f 6d 61 69 6e`, no U+202E), and immutability was proven with a
   **positive control** (7 observer callbacks, elapsed ticking 0.0→2.0→3.0s, target unchanged).
   **AC §9.11 substituted push for commit** — `?gitNoTarget` forces `null` for every category, so
   push exercises the identical path without driving the commit form. Same assertion, not the
   literal AC text.
   **Deliberately folded in and now closed:** the `GitActivityRow.tsx` chevron follow-up
   (`<button aria-hidden>` inside `role="button"` → `<span>`), verified live: 0 buttons inside
   `.git-run-summary`.
   **Two things worth keeping:**
   - `push_target_matches_push_result` was renamed `push_target_agrees_with_push_result_remote`
     because `PushResult.branch` carries the **local** branch name — the old name overclaimed and
     the test would have failed *falsely* on a divergent upstream. The real gap (Push's use of
     `branch.<x>.merge` for divergent names) is now pinned on the `renamed` fixture, and the fix
     was **mutation-proved**: mutating the resolver makes the new assertion fail while the old test
     still passes.
   - The mock's `mockActivityTarget` was verified **code-point-equivalent** to Rust's
     `strip_control_chars`, including that Rust's `bidi ∪ zero_width` sets happen to form the
     contiguous `200B–200F` range the mock uses.


### 59.3 — The six remaining reviewer follow-ups (`e9d025d` / `7f9f16b` / `c03d11d`)

2. **✅ All reviewer follow-ups from the `a82740ff` pass are CLOSED — `e9d025d` / `7f9f16b`**
   (full 8-step gate green, 542.4s). Its MUST-FIX + two SHOULD-FIX had landed earlier in `151232d`;
   the chevron closed in `1d8c6f9` as FU-1 scope. The last six closed 2026-09-10. Three of the six
   were not what the board said, so the corrections are recorded here:
   - **`whichAll` in `scripts/lib/spawn-tool.mjs` — the obvious fix was a trap.** Prepending `''`
     to the Windows `PATH_EXTS` would have *regressed* tool resolution: npm/corepack install
     **extensionless POSIX shell scripts** beside every shim (`pnpm`, `npm`, `npx`, `corepack` all
     exist that way on this machine), and `resolveTool` picks `hits.find(p => !isBatch(p))` as the
     real executable — so `''` first selects an unrunnable script. `pnpm` itself survives only
     because `pnpmJsEntry` short-circuits, so this would have surfaced on some *other* tool. `''`
     is now gated behind `hasExecExt`. Also `existsSync` accepted a **directory** named `pnpm.EXE`
     as an executable hit. 3 regression tests, 9/9.
   - **`watcher/classify.rs` was cosmetic, not a bug** — and the board's path was wrong
     (`src-tauri/src/watcher/`, not `crates/bonsai-core/src/watcher/`). `Path::starts_with` matches
     whole components, so `starts_with("packed-refs")` never matched `packed-refs.lock` either. The
     change buys consistency with the `==` lines above it, nothing more.
   - **The `forge-pr.css` "verbatim" claim was misattributed.** The split (`e149382`) *was*
     byte-identical; the grouped-rule expansion happened later in `833f2f9` and only in
     `forge-pr-create.css`. Separately `forge-pr-detail.css` is **also** not verbatim, for a reason
     nobody had noticed: P111 (`1192f2a`) added the `.pr-label` one-line guard — a real behaviour
     change. `forge-pr-changes.css` is genuinely verbatim. Each header now states its own
     provenance. `src/styles/forge-account.css` carries the same claim and it is currently **true**.
   - `external.rs` wording now true on all three branches (missing / a file / **stat failure** —
     permission denied, unreachable share, broken reparse point), still category-only per audit
     LOW-2. Its one pinning test was updated in the same increment.
   - `raw_args.rs` doc ref fixed; that orphaned three siblings still on the retired Amendment-A26
     letter scheme, so they moved to live section numbers too.
   - `useStickySelection.ts` ↔ `GitActivityDock.tsx` now cross-reference each other. The stated
     distinction is the true one: the anchor re-derives from current props, the latch records
     **history**, so a discarded render's write would be observable as the dock staying shown after
     Clear — which is why only one of them may write during render.


---

## Part 60 — The board's own record of the 2026-09-10 confirmation (the `The checkpoints are CLEARED` sub-block of RESUME HERE), verbatim, moved off the board 2026-09-10. Retained because it is the only place that states *why* the `8dd5b24` CSP change needed a native checkpoint at all. Its two "does NOT close" items stay live on the board as `### P108 — AC11 is still OWED` and `### P91 — open items` under `## OPEN follow-ups`; the confirmation fact itself is recorded on the board under `## Accepted decisions that must survive compaction`.

### ✅ The checkpoints are CLEARED (2026-09-10)

**The user confirmed all eight native-verification items** — P102+P105, P106, P107, P108, P91, P110,
P109, and the `8dd5b24` CSP change (which applies to the Tauri webview only, so neither the harness
nor e2e ever exercised it). The ~470 board lines they were blocking are now archivable; hand that to
`docs-curator`.

**Two things the confirmation does NOT close** — both are AI-gate items, not native checkpoints:
- **P108 `AC11`** — two contrast states unreachable in the harness (`.file-count-del` selected and
  `.context-menu-item[data-tone='danger']` hovered, both source-derived at 3.05). A person cannot
  confirm a numeric contrast ratio by eye, so this stays owed.
- **P91's `logs/*.jsonl` parse** from a real `pnpm tauri dev` boot+idle. Now *possible* for the first
  time if the verification run booted the app — the logs would exist on disk.


---

## Part 61 — Superseded curator bookkeeping from the board, verbatim, replaced 2026-09-10. Nothing here is project history; it is the board's own navigation and self-measurement text, kept so the 2026-09-10 pass is lossless down to the meta-lines it rewrote. Contains: the pre-pass `Where the rest of the board went` body, the pre-pass RESUME HERE `Next, in order` preamble, the pre-pass `Verification state` block, and the **2026-09-03 curator note with its measured line-count composition** (the note whose "~470 lines become archivable the instant the seven USER CHECKPOINTs and seven FOR USER decisions clear" prediction this pass tested — 557 lines were archived, ~193 of which were live remnant that had to be relocated rather than removed, so the board fell 1244 → 915, not 1244 → 774).

### 61.1 — `Where the rest of the board went`, as it stood before 2026-09-10

## Where the rest of the board went

Full detail for everything compacted out of this file is in `docs/history/` — start at
`docs/history/README.md`. The 2026-09-03 sweeps are `docs/history/todo-archive-2026-09.md`
**Parts 36-53**; the archive table at the bottom is the short form. Nothing below was closed by the
curator: a milestone with a pending USER CHECKPOINT stays here, and open follow-ups stay here
however old they are.

**2026-09-03 compaction pass (Parts 51-53).** Moved off the board: the `5c2dcd2` and `c6cd7dd` gate
states plus the e2e-contention narrative (Part 51); the narratives behind items closed that day
(Part 52); and the stories behind the durable lessons (Part 53). The **rules** those stories taught
stayed here. **1068 → ~1000 lines** — the ~300-line target is unreachable while seven USER
CHECKPOINTs and seven FOR USER decisions are open; see the residual note in the Archive section.

**2026-09-03 staleness sweep (no archiving, facts only).** Prompted by two same-day misses of one
shape — work landed, the entry that filed it was never upgraded (P109 fixed in `5a68e00`;
`.settings-toggle-btn.is-active` fixed in `112800c`). **35 open entries were cross-checked against
the tree; 11 had drifted.** Corrections are inline below, each marked `re-verified 2026-09-03` or
struck through with its SHA. The board's own record of being wrong is kept deliberately. Two
findings are worth reading before trusting anything nearby: the `ai::session*` clock seam **is in
the tree** (`734b310`), and the M1 design-review residue was telling sessions to delete
`aria-activedescendant`, which `ui-reference.md:865` explicitly sanctions.


### 61.2 — RESUME HERE `Next, in order` preamble, as it stood before 2026-09-10

### Next, in order

**Current step: nothing is in progress.** FU-1 and every reviewer follow-up are done and committed.
The next items on this list are **all FOR-USER decisions, not patches** — item 3's two security
calls, plus the seven open decisions in the FOR USER section and the eight native USER CHECKPOINTs.
There is no unblocked implementation work left in this block; pick from `1b`/`1c`/`1d` (doc + small
hygiene) or the "OPEN follow-ups" section further down if you want code work.


### 61.3 — `Verification state`, as it stood before 2026-09-10

### Verification state

- **Full 8-step gate green at `1d8c6f9` (2026-09-10, 452.5s)** — 2344 Rust tests, 185 e2e passed /
  1 skipped. Per step: nextest 133.2s · doctests 3.7s · clippy 22.8s · eslint 12.7s · size ratchet
  0.9s · vitest 82.9s · tsc+build 15.3s · e2e 181.1s. **The e2e leg no longer needs a re-run** —
  this superseded the `c218258` state, which had been the last full run.
- Exit code 0 is not sufficient evidence on its own: **grep the log** for failures. Many test NAMES
  contain `error`/`failed`, so a naive scan returns false positives — filter them.
- Verify the machine is idle first, and **redirect the whole log to a file** — piping `pnpm gate`
  through `tail` lost a failure detail and cost a re-run.
- Port **1420 is free**. Keep it so: `strictPort: true` means a held port breaks `pnpm tauri dev`.


### 61.4 — The 2026-09-03 curator note, verbatim (its composition numbers are the baseline this pass is measured against)

### Why this board is ~1000 lines, not ~300 (curator note, 2026-09-03)

Residual composition, measured after the Parts 51-53 pass: FOR USER decisions **135** · awaiting
USER CHECKPOINT **198** · P110 + P109 **~84** · SEC-2026-09-03 **~52** · durable-lesson rules
**~97** · accepted decisions **~44** · open follow-ups **~214** · header/conventions/gate/archive
**~180**.

**~470 of those lines become archivable the instant the seven USER CHECKPOINTs and seven FOR USER
decisions clear** — and not before. Going further today would mean archiving a pending checkpoint
(forbidden) or stripping the 2026-09-03 sweep's freshly-verified `file:line` citations out of the
open follow-ups, which is the evidence a cold resume needs most.
