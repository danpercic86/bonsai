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

**USER CHECKPOINT BATCH CONFIRMED (2026-07-30):** the user confirmed in native `pnpm tauri dev`
that ALL previously-pending milestones work — P4, P3a/P3b/P3c/P3d/P3e, P7, P7e, P7f, P3f, P8, P9.
Every "awaiting USER CHECKPOINT" below is now CONFIRMED as of 2026-07-30. (P5/P6 were already
confirmed earlier.)

**USER CHECKPOINT BATCH CONFIRMED (2026-08-03):** the user confirmed ALL remaining pending
checkpoints — the P18–P23 batch, P24, P25, P26, and P27. Every "awaiting USER CHECKPOINT" below is
now CONFIRMED as of 2026-08-03. P18–P27 are fully DONE. Next: P28 (approved plan
`~/.claude/plans/what-are-the-next-quiet-marble.md`): B3 what-changed digest →
P29 D1 repo-health dashboard → P30 B5 scheduler → P31 per-worktree AI contexts.

## P37 — force-push-with-lease (Git completeness, Phase 1) — **in-progress** (2026-08-04)

New phase (approved plan `~/.claude/plans/if-we-think-about-eager-hoare.md`): the 4-theme roadmap is
~90% shipped, so the next phase is **Git completeness (parity) + productization**. User decision
(2026-08-04): lead with Git completeness, close-out first (close-out was already done — #5 committed
`bf152be`, P36 done+confirmed, git2 bumped to 0.21). Sequence: **P37 force-push-with-lease → P38
reflog → P39 bisect → P40 config editing → P41 LFS (optional) → P42 packaging/signing/auto-update →
P43 onboarding**. Parked for a later phase: forge/PR integration (GitHub/GitLab/Azure), security &
compliance (B6/D3), the cohesion pass.

**P37 goal:** safe force-push (`--force-with-lease` equivalent) so the already-shipped rebase (P23) /
amend (P20) can be published without clobbering a remote that moved unexpectedly. Extends the
`git/remote.rs` push path (must respect the recent credential rework — auth resolves through real git
`credential fill` + the P35 in-process cred cache — and git2 0.21.0). Confirm-gated in the UI; refuses
when the remote ref advanced past the expected oid. No new AppError variant unless justified.
Standard loop; guardrails unchanged (scratch under `D:\Temp\bonsai-scratch`, TMP/TEMP=D:\Temp for
cargo on Windows, no concurrent test+clippy, mock.ts kept compiling, orchestrator makes all commits).

Contract: `docs/contracts/P37-force-push-with-lease.md` (architect). Approach: libgit2 `Remote::push`
`+`-force refspec + manual ls-remote lease pre-check (reuses `acquire_cred`/P35 cred cache exactly
like `tags.rs::push_tag`); lease baseline = remote-tracking ref oid (backend-derived); no new
AppError (reuse `PushRejected`); one `force_push` command; UI = toolbar Push split-button caret.
KNOWN LIMITATION (documented in confirm dialog, accepted): client-side compare-and-swap with a small
TOCTOU window — libgit2 can't do the server-side atomic CAS real `git --force-with-lease` gets; still
strictly safer than bare force-push. Sub-increments: **P37a** backend+command+IPC+mock+CLI-oracle →
**P37b** UI (toolbar split-button + confirm dialog).

- **P37a** (reviewer APPROVE, 0 must-fix; lease safety property verified directly) — committed `b2013ac`.
  force_push_with_lease + force_push command + IPC triple + mock + force_push_cli 5/5. Nits (cosmetic,
  per-spec): mock omits upToDate outcome (§5.3); lease-message strings duplicated Rust↔mock.
- **P37b** (reviewer APPROVE, 0 must-fix) — UI. WorkspaceToolbar Push split-button + caret →
  ContextMenu "Force-push with lease…"; RepoWorkspace pendingForcePush/handleForcePush/doForcePush
  (mirrors pushCurrentBranch; PushRejected → "— fetch and retry" hint) + canForcePush gating
  (canPullPush && headBranch.upstream != null); WorkspaceDialogs danger ConfirmDialog naming
  branch+remote, warning it rewrites published history + documenting the client-side-lease limitation;
  styles.css split-button. tsc + build clean. Nits (cosmetic, left): dead `busy` prop on the confirm
  (dialog closes before remoteOp flips); upToDate toast 'info' vs push's 'success'.
- **P37b AI GATE PASSED (2026-08-04).** Browser harness (mock :1420, hidden pane → DOM/JS-driven real
  handlers): Push split-button caret reveals "Force-push with lease…"; confirm dialog reads "Force-push
  with lease? This rewrites the published history of main on origin…" + the client-side race-window
  note vs `git push --force-with-lease`; confirm → "Force-pushed main → origin/main" toast (mock
  advances the remote-tracking tip); `?remote=leasefail` → error toast "force-push refused: 'origin/
  main' has moved on the remote since you last fetched … — fetch and retry"; zero console errors.
- **P37 tester** — full workspace regression PASS, zero regressions: bonsai-core lib 344 + src-tauri
  (bonsai_lib) 92 + force_push_cli 9 (5 orig + 4 added: nested-branch lease, real history-drop verified
  on the bare origin via merge-base, detached-HEAD reject, unborn-HEAD reject) + remote_cli 18 (the
  known-flaky pull_fast_forwards_ref_and_worktree PASSED, did not flake) + bonsai_mcp 10 + all other
  integration green; clippy --workspace --tests clean; tsc + build clean. Checklist:
  docs/contracts/P37-user-checklist.md.
- **P37 AI GATE PASSED (2026-08-04).** Commits: b2013ac P37a · 0b05b93 P37b (+ this tester closeout).
  Backend ls-remote-lease oracle suite + frontend browser harness both verified; zero regressions.
  Git-completeness Phase 1 milestone #1 delivered.

**P37 awaiting USER CHECKPOINT** (native pnpm tauri dev on a SCRATCH repo w/ a bare origin, per
docs/contracts/P37-user-checklist.md): rewrite local history (rebase/amend) → force-push-with-lease
succeeds (verify origin via `git ls-remote`/`git log`); teammate advances the origin ref → lease
REFUSED ("has moved / fetch first"), origin unchanged; after fetch, retry succeeds; the caret is
disabled on a branch with no upstream.
**Current step:** P37 DONE (AI gate passed, awaiting USER CHECKPOINT). Next: P38 (reflog viewer).

## P36 — six UX/safety fixes (worktree checkout guard, bulk discard, tab UX) — **DONE**

**Goal:** six reported issues. (1) **Data-loss fix:** `checkout_branch_autostash` has no
worktree-occupancy guard, so git2 silently moves HEAD onto a branch already checked out in another
worktree (corrupt dual-checkout → lost untracked files). Refuse it git-style, mutating nothing.
(2) "Discard all" control in the Changes panel. (3) Folder-level bulk actions (hover buttons) in the
tree view. (4) Drag-to-reorder repo tabs. (5) Remove redundant "Bonsai" label left of the tabs.
(6) Seat the "+" button next to the last tab (not far-right).

**Decisions (user, 2026-08-04):** (1) block-with-message on worktree collision; (2/3) bulk/folder
discard reverts modified tracked files AND deletes untracked/new files, behind a confirm dialog;
(3) folder actions = inline hover buttons. Plan:
`~/.claude/plans/i-have-the-following-playful-koala.md` (approved).

**Acceptance criteria:**
- `checkout_branch_autostash` returns `Err(BranchCheckedOutElsewhere(<path>))` when the target branch
  is checked out in another worktree; no stash created, HEAD unchanged, workdir untouched.
- New `discard_paths_force`: tracked→restore from index, untracked→delete file, mixed→both,
  empty→no-op (no match-all clobber), invalid path→reject.
- Changes panel "Discard all" header button + folder hover stage/discard buttons; confirm dialog
  warns when new files will be permanently deleted.
- Tabs drag-reorder and persist across reload; no "Bonsai" label; "+" sits after the last tab.

**Current step:** AI gate PASSED. reviewer APPROVED (0 must-fix). tester 37/37 green (13 new,
incl. the data-loss assertion: refusal creates no stash, HEAD unchanged, dirty+untracked files
preserved); clippy clean. Browser harness verified: no "Bonsai" label, `+` seats 4px after last
tab (`.tab-scroll` now `flex:0 1 auto`), draggable tabs, "Discard all" + 36 folder hover buttons,
confirm dialog reads "Revert 6 files and permanently delete 3 files?". Contract:
`docs/contracts/P36-ux-safety-fixes.md`. Committed (`704139b`). **USER CHECKPOINT CONFIRMED
(2026-08-04):** the user verified worktree-checkout refusal + no data loss, bulk discard, and tab
drag-reorder in the native app. P36 fully DONE.

## P35 — in-process HTTPS credential cache — **in-progress**

**Goal:** the M6 credential fix (`4e4b8f4`, shell out to `git credential fill`) made every
fetch/pull/push/clone slow on Windows — each call cold-starts `git-credential-manager.exe` (.NET).
Add an in-process, app-lifetime credential cache (TTL backstop + stale-while-revalidate refresh +
invalidation-on-rejection) so we resolve through the REAL helper at most once per host per session,
keeping resolution semantically identical. User asked for both app-lifetime + TTL, plus a cache
warmer. Contract: `docs/contracts/P35-credential-cache.md` (15 AI-gate acceptance criteria).

**Design (locked):** new module `crates/bonsai-core/src/git/cred_cache.rs`; key = scheme://host;
`CRED_TTL=10min`, refresh at 80%; `std::sync::LazyLock<Mutex<HashMap>> + Condvar` single-flight;
background refresh via `std::thread` (no tokio); injectable-filler test seam (no git spawn in tests,
cross-platform). `CredAttempts.helper` → `HelperState` machine so a stale cache-hit that's rejected
gets exactly ONE fresh re-fill before falling through. **Warm-on-open WIRED** (orchestrator decision,
beyond contract §16): outer `open_repo` command enumerates HTTPS remotes and calls
`cred_cache::warm` fire-and-forget so the first op is warm too.

**Current step:** DONE + committed (`129ba99`). AI gate green — clippy `-D warnings` clean,
`cargo test -p bonsai-core cred` 19 pass (11 cred_cache incl. panic-recovery + single-flight, 8
remote.rs state-machine), src-tauri compiles; reviewer approved (0 must-fix, 2 NIT hardenings
folded). Follow-up (`b2ef361`): the `credential_fill_*` tests were popping the real GCM GUI on
Windows (they let git fall through to the inherited system/global helper) — made them hermetic
(reset the helper list per test repo) + cross-platform (`!`-inline sh fixtures). b2ef361
cleared the inherited GCM helper, but git's *askpass* GUI ("Username for '<url>'") still popped on
the failure-mode / no-helper cases — `GIT_TERMINAL_PROMPT=0` gates only the terminal, not askpass.
**Fixed in `f77adb2`:** `credential_fill` now also neutralizes askpass (`-c core.askpass=` +
env_remove `GIT_ASKPASS`/`SSH_ASKPASS`) — a production fix too, since P35 warm-on-open fires
`credential_fill` in the background on repo open. All `credential_fill_*` tests now pass prompt-free. **Awaiting USER CHECKPOINT:** real HTTPS remote w/ GCM —
first fetch resolves via helper; 2nd+ ops visibly faster (git-credential-manager does NOT relaunch,
check Task Manager); externally rotate/expire the cred → next op recovers (evict+refill), not fail.

## Security: git2 0.20.4 → 0.21.0 (2026-08-04) — **DONE (branch `chore/git2-0.21`)**

`cargo audit` flagged two *unsound* advisories in git2 0.20.4 (used directly, ships on all
platforms): RUSTSEC-2026-0183 (`Remote::list()` UB) + RUSTSEC-2026-0184 (`BlameHunk` signature UB),
both fixed in git2 0.21.0. Bumped in `7a6ade2`. 0.21 is breaking two ways, both handled:
(a) it dropped `ssh`/`https`/`cred` from default features → re-enabled `["https","ssh"]`,
`default-features=false`, dropped `cred` (we shell out to `git credential fill`, not git2's
CredentialHelper); (b) string accessors `Option<&str>` → `Result<&str>` / `Result<Option<&str>>`
— migrated ~62 sites via `.ok()` / `.ok().flatten()` (behavior-preserving: non-UTF-8/absent → None
as before); `Oid::zero()` → `Oid::ZERO_SHA1`. AI gate: `cargo clippy --workspace --all-targets -D
warnings` clean; full suite green; `cargo audit` now **0 vulnerabilities**.
Remaining audit noise (informational, not fixable here): glib 0.18 unsound (RUSTSEC-2024-0429,
MODERATE — the Dependabot alert) is a Linux-only transitive dep pinned by wry/tauri's gtk-rs 0.18
stack, absent from Win/macOS builds and on an unreached path → **dismiss on GitHub as "vulnerable
code isn't actually used"**; plus 16 unmaintained (gtk-rs / unic-*). Note: `perf_ceiling_on_20k_fixture`
can flake under full-suite CPU contention (~2.05s vs 2.0s budget); passes standalone at ~1.9s.

## UX fix batch (user-found issues, 2026-08-04) — **awaiting USER CHECKPOINT**

Branch `fix/ux-issues-batch`. Four issues the user hit in the app. All committed, AI-gate
verified (tests + browser harness `pnpm dev:mock`, no console errors). Plan:
`~/.claude/plans/issues-that-i-found-silly-popcorn.md`.
- **#2 console flicker on repo-tab switch** — DONE. `run_process` (ai/mod.rs) spawned the `claude`
  `.cmd` shim without `CREATE_NO_WINDOW`; added the Windows-gated creation flag. commit 3f8e605.
- **#3 Commit split button** — DONE. Normal mode shows primary "Commit & Push" + secondary
  "Commit"; no-upstream branch confirms via dialog then pushes+sets upstream; cancel preserves the
  typed message; detached/unborn HEAD falls back to single Commit. commit 934a351.
- **#4 auto-stash branch switch** — DONE. New `checkout_branch_autostash` (stash → switch →
  best-effort no-fetch FF to upstream when behind>0&&ahead==0 → re-apply; stash kept on conflict).
  Repurposed the `checkout_branch` command/IPC to return `CheckoutResult`. 12 tests. commit 6c8a41a.
- **#1 stash scopes + staging-area button** — DONE. `StashScope { All | AllWithUntracked | Staged }`;
  staged-only is a hand-rolled libgit2 stash that FOLDS mixed staged+unstaged files (user decision),
  keeps the durable stash on failed mutation, single reflog entry (no stack corruption), captures
  `rm --cached`. New `StashSplitButton` in staging panel. 14 tests. commit e200aed.
- **#5 stash apply/pop reserved-name recovery (Windows)** — DONE (uncommitted; awaiting user stage).
  Applying a stash whose untracked `^3` tree holds a Windows reserved device-name path (e.g.
  `.../NUL`) failed with a raw libgit2 `cannot checkout to invalid path` and aborted the whole apply.
  New `is_windows_reserved` + `stash_path_sets` detect reserved leaf paths; `apply_stash`/`pop_stash`
  gain `skip_reserved: bool`. First attempt returns `ApplyStashOutcome::ReservedPaths{paths}` (nothing
  applied, stash retained); a skip retry applies everything except the reserved LEAF paths via a
  `CheckoutBuilder` `.path()` allowlist (`disable_pathspec_match(true)`) + post-apply guard, returning
  `AppliedSkippingReserved{skipped}`. Pop stays lossless (never drops when skipping). Threaded through
  Tauri commands, bonsai-mcp tools (`skip_reserved` arg), IPC/mock, and a RepoWorkspace confirm-retry
  dialog. Mechanism verified against vendored libgit2 1.9.6 (two-phase checkout shares the pathspec;
  untracked phase drops `disable_pathspec_match` → leaf-only enumeration is load-bearing). 27 core
  tests (incl. 2 Linux-CI-gated real-NUL end-to-end); browser-harness AI gate passed.
- Contracts: docs/contracts/P33-checkout-autostash.md, P34-stash-scopes.md (commit de8746d).
- **USER CHECKPOINT (native `pnpm tauri dev`):** (a) switch repo tabs → no console flicker;
  (b) Commit & Push on a no-upstream branch confirms then pushes, one-click when upstream set;
  (c) switch branches with local changes → auto-stash/switch/re-apply; conflict keeps stash;
  branch behind upstream fast-forwards; (d) staging-area stash button + 3 scopes, staged-only
  leaves unstaged work in place.

## P28 — AI "what changed" digest (roadmap B3) — **in-progress** (2026-08-03)

First of the four approved P28-scope features (B3 → D1 → B5 → per-worktree contexts; plan
`what-are-the-next-quiet-marble.md`). AI-generated digest of what changed over a range (since
last fetch / between refs / last N days), reusing run_claude + the ai_analyze_diff/ai_summary
patterns and the existing AiOutputPanel; write-free; 256 KiB payload cap like P25. Standard loop;
guardrails unchanged (D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp, no concurrent test+clippy, mock.ts
compiling, orchestrator commits).
Contract: docs/contracts/P28-what-changed-digest.md (architect, defaults accepted): NEW ai_digest
command + AiDigestRange (betweenRefs merge-base `from...to` w/ unrelated-histories fallback;
sinceCommit = sugar for betweenRefs{from,to:HEAD}; lastDays first-parent committer-time cutoff);
payload = RANGE header + ≤200 commit-meta lines + diff → existing 256 KiB cap; result = AiAnalysis;
errors = existing AppError kinds; empty range → AiFailed before CLI. Mock aiDigest canned per range
kind + ?ai=off gate. UI: toolbar "✨ What changed…" → WhatChangedDialog (3-mode picker) → runDigest →
AiOutputPanel. Sub-increments: **P28a** core+cmd+IPC triple+tests → **P28b** UI.
- **P28a** (reviewer APPROVE, 0 must-fix) — commit 70f8182. ai_explain.rs: AiDigestRange (serde
  kind-tagged camelCase), resolve_digest_range (betweenRefs merge-base TOPOLOGICAL|TIME + empty-tree
  unrelated-histories fallback; sinceCommit → BetweenRefs{oid,HEAD}; lastDays first-parent
  committer-time cutoff + boundary tree, days=0→InvalidName, clamp 3650), digest_changes (empty range
  → AiFailed BEFORE CLI; RANGE/COMMITS(≤200 + overflow)/DIFF payload → cap_review_payload; WRITE-FREE
  verified — no ODB writes). ai_digest command (consent gate first) + IPC triple + mock (?ai=off gate).
  8 unit + 6 ai_digest_cli oracle tests (git-log oracles); workspace 656 green; clippy/tsc/build clean.
  NITs (cosmetic, deferred): detached-HEAD label dead branch (contract quirk); unrelated-histories note
  wording drift vs gather_branch; stub-fixture coupling.
- **P28b** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX + plural nit folded by orchestrator) — commit
  a524f5b. WhatChangedDialog.tsx (3 radio modes, to=current-branch default, days=7 min 1, branch
  datalist, first-submit validation, capture-phase Esc + overlay cancel); RepoWorkspace runDigest
  (sibling of runAnalyze, shared aiPanelReqId guard) + toolbar "✨ What changed…" gated aiEligible +
  whatChangedOpen in globalModalOpen; dialog-radio CSS. FOLDED: focus effect keyed [open,mode] (reopen
  focus loss) + "last 1 day" pluralization. tsc + build clean.
- **P28b AI GATE (frontend) PASSED (2026-08-03).** Harness (mock, hidden pane → DOM/JS-driven):
  toolbar button appears only when aiEligible (localStorage consent) and vanishes under ?ai=off;
  dialog defaults correct (betweenRefs checked, to=main, focus lands on from-input); empty submit →
  "Enter both refs" + dialog stays; betweenRefs feature/sidebar..main → AiOutputPanel "What changed:
  feature/sidebar..main" + canned prose + $0.01; lastDays 1 → title "last 1 day" (singular); since
  abc1234def → "What changed since abc1234"; Esc closes; zero console errors.
- **P28 tester** — full regression PASS, no bugs: bonsai 71 + core lib 255 + ai_digest_cli 6→**10**
  (tag/short-oid refs; first-parent-vs-full-walk divergence on merge history; unicode subjects/authors
  verbatim in COMMITS; 250-commit git2 fixture → exactly 200 meta lines + "... and 50 more") + all
  ~35 integration suites 0 failed (remote_cli 18/18 — the tracked pull flake did NOT reproduce);
  clippy --workspace --tests clean; tsc + build clean. Checklist: docs/contracts/P28-user-checklist.md.
- **P28 AI GATE PASSED (2026-08-03).** Commits: 70f8182 P28a · a524f5b P28b · 8611cc9 tests. Backend
  oracle suites + frontend browser harness both verified; zero regressions. Roadmap B3 delivered.

**P28 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P28-user-checklist.md,
real claude CLI): digest between real refs / last-7-days / since a pasted oid produce sane prose;
AI-disabled hides the toolbar button; huge range shows the truncation note; digest writes nothing.
**Current step:** P28 DONE (AI gate passed, awaiting USER CHECKPOINT).

## P29 — D1 repo-health dashboard — **in-progress** (2026-08-03)

Second of the four approved P28-scope features. READ-ONLY health overlay panel. Contract:
docs/contracts/P29-repo-health.md (architect, defaults accepted): new crates/bonsai-core/src/health.rs,
RepoHealth = 4 independent Section<T>{data,error,elapsedMs} sections (stats/branches/workingState/
structure — one failing section can't sink the panel); single get_repo_health command, sequential
collectors in one spawn_blocking; perf caps REVWALK_CAP 100k / ODB_SCAN_CAP 500k header-only /
200k-entry dir walks / top-10 heaps / 10 MiB large-file threshold, `capped` flags rendered as ≥;
reuse find_stale_branches/status/opstate/list_worktrees/list_submodules/scan_inventory/ahead-behind.
D6 trim (FLAG FOR USER): largest pack blobs = oid+size only (no blob→path history walk); worktree
largest files DO get paths. Sub-increments: **P29a** core+tests → **P29b** cmd+IPC+mock (-err repo-id
harness hook) → **P29c** RepoHealthPanel overlay (mirrors AiAssetsPanel).
- **P29a** (reviewer APPROVE, 0 must-fix; 3 deviations ACCEPTED) — commit 2ac2f35. health.rs (~1080
  lines): full §4 wire types, 6 caps as consts w/ capped-only-on-overflow semantics, ODB read_header-
  only scan, top-10 min-heaps, D4 never-errs fold, READ-ONLY trace clean (no ODB/index/fs writes in
  production paths), reuse verified (stale/status/opstate/worktrees/submodules/scan_inventory,
  no duplicated logic). ACCEPTED deviations: (1) Cargo.toml [profile.dev.package.libgit2-sys]
  opt-level=2 (perf bound at -O0 impossible; one-time rebuild); (2) perf test warm-up + best-of-3
  (perf_gate precedent; 31k fixture total 1733 ms < 2 s); (3) section-isolation via injected-Err +
  non-repo dir. 11 new health unit tests (rev-list/for-each-ref oracles); core lib 266 + all 36
  integration suites green; clippy clean.
  **P29b CARRY-FORWARDS:** (1) add a mixed-state section test (one real collector fails, three
  succeed) at the command layer or via collect_stats_with_caps on a sabotaged repo; (2) SHOULD-FIX
  health.rs:236 — find_commit failure inside revwalk sinks the whole stats section; degrade to
  count-and-skip on unreadable commits.
- **P29b** (reviewer APPROVE, 0 must-fix; wire parity verified field-by-field) — commit bc89616.
  get_repo_health command (repo_path→spawn_blocking, NoRepo-only surface, list_worktrees pattern) +
  registration + test; IPC triple (9 TS types mirror Rust serde 1:1 incl. RepoOpState reuse); mock
  fixture covers every §7 warn state (stale 3/1, ahead 2/behind 5, capped counts, 2 large files,
  drift 2, locked+prunable worktree, out-of-sync submodule, merge opState, stash 2) + `-err` repo-id
  hook (stats section → error envelope). CARRY-FORWARDS LANDED: find_commit degrade (count-and-skip,
  documented as organically unreachable — revwalk iterator errors first) + mixed_state_real_collector_
  failure test (deleted loose object → stats errors, 3 sections live). core health 12 tests + bonsai
  72 green; clippy/tsc/build clean.
- **P29c** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX folded by orchestrator) — commit 1402c99.
  RepoHealthPanel.tsx (AiAssetsPanel-pattern overlay: fetchIdRef stale guard, fetch on open/repoId
  change, repo-changed refresh only-while-open/this-repo, Refresh, per-section skeleton + inline
  error-banner w/o hiding siblings, capped→"≥"+chip, warn/ok chips via existing asset-chip classes);
  📊 header button (icon-only w/ title/aria "Health", mirrors 🤖 — accepted deviation) + healthOpen
  in globalModalOpen/Escape; utils/format.ts formatBytes extracted from CloneDialog + GiB (CloneDialog
  ≥1 GiB now renders GiB — accepted improvement). FOLDED: "unknown" chip when ahead/behind null
  (upstream present but graph_ahead_behind failed). tsc + build clean.
- **P29c AI GATE PASSED (2026-08-03).** Harness (mock, hidden pane → DOM/JS-driven): 📊 opens panel;
  all 4 sections + "generated just now"; every §7 warn chip renders (capped ×3, 2 large, ↑2/↓5,
  3 merged/1 gone, 1 conflicted, merge-in-progress, 1 uninitialized/1 out-of-sync submodule,
  1 locked/1 prunable worktree, 2 files drifted); ≥ rendered for capped counts; `-err` repo id →
  stats section shows inline "simulated slow scan failed" while the other 3 sections render data;
  Esc closes. Console: only a stale HMR dep-array artifact from the live-edit session (App Esc
  effect legitimately grew 4→5 deps under Fast Refresh); count did NOT grow across a hard reload +
  fresh panel exercise — zero real errors.
- **P29 tester** — full regression PASS, no bugs: workspace **681 passed, 0 failed** (bonsai 72 +
  core lib 267 + NEW health_cli 8 + all suites; both known flakes passed this run); clippy clean;
  tsc + build clean. health_cli.rs: rev-list/count-objects/porcelain/stash/for-each-ref/left-right/
  worktree-porcelain oracles + 3 edge repos (unborn/detached/gitdir-only, no panics) + the §6
  READ-ONLY invariant (status/refs/HEAD/stash/index byte-identical before/after). WATCH ITEMS:
  (1) perf headroom thin — branches section (find_stale_branches) dominates, worst-of-3 1986 ms vs
  2000 ms budget; consider a stale-subscan budget in a follow-up; (2) doc-level deviation: unborn
  repo returns currentBranch Some(symbolic target)+unborn:true vs contract's None — behavior pinned
  by test, amend contract wording later (more useful as-is).
- **P29 AI GATE PASSED (2026-08-03).** Commits: 2ac2f35 P29a · bc89616 P29b · 1402c99 P29c ·
  61bdbdc tests. Backend oracle suites + frontend harness both verified; zero regressions.
  Roadmap D1 delivered.

**P29 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P29-user-checklist.md):
📊 panel on a real repo shows sane numbers vs git CLI; responsive on a big repo; Refresh +
repo-changed refresh work; opening the panel changes nothing (`git status` identical).
**Current step:** P29 DONE (AI gate passed, awaiting USER CHECKPOINT).

## P30 — B5 background-job scheduler — **in-progress** (2026-08-03)

Third of the four approved P28-scope features. v1 jobs strictly NON-DESTRUCTIVE: auto-fetch +
read-only health/status refresh; suppressed while opstate != none; no-overlap; backoff after 3
failures (base*2^(f-2), cap 8x); background fetch NEVER prompts (silent-fail into backoff).
Contract: docs/contracts/P30-scheduler.md (architect, defaults accepted; FLAGGED FOR USER:
(1) job config is GLOBAL not per-repo — reuses existing Settings.auto_fetch machinery;
(2) behavior change: auto-fetch now covers ALL open tabs, not just the active one). Design:
SUBSUMES the P11e frontend timer (RepoWorkspace.tsx:964-988 setInterval deleted) → one global
tokio 15s coarse tick loop in src-tauri/src/scheduler.rs, membership from AppState.repos each
tick, pure time-injected planner plan(...)→Run|SkipOverlap|Wait, new sibling health_refresh
setting; commands get_job_status/run_job_now; config via existing get/set_ui_settings; new
job-status-changed event (enteredBackoff → single toast) + existing repo-changed for refresh;
mock ticks minutes-as-seconds + failure shim. Sub-increments: **P30a** Rust core+cmds+tests →
**P30b** IPC triple + Settings/status UI + mock.
- **P30a** (reviewer APPROVE, 0 must-fix; safety trace passed) — commit 467be65. scheduler.rs: pure
  planner (due/first-sight D13/backoff 2^(f-2) cap 8x/overlap), SchedulerState(Arc<Inner>+Deref —
  accepted deviation, detached futures need 'static), tick_once w/ injectable emitter+time, execute_
  job (opstate suppression → fetch_all ONLY — reviewer independently confirmed no-prompt: acquire_
  cred helper→agent→default once each → CRED_EXHAUSTED, no prune/merge/push anywhere), job-status-
  changed (enteredBackoff exactly on 2→3), run_job_now/get_job_status commands; settings
  health_refresh sibling w/ serde back-compat + double clamp; P11e frontend timer DELETED (autoFetch
  prop chain removed; App keeps Settings ownership). bonsai 87 green ×2; clippy/tsc/build clean.
  Watcher test drain 1.5s→2.5s (test-only, accepted; if a 3rd bump is ever needed, serialize the
  fixture instead).
  **P30b CARRY-FORWARDS:** (S1 SHOULD-FIX) poisoned-mutex paths silently kill the scheduler + leak
  running=true (scheduler.rs:277-285, 415-417, apply_config:119) → recover via PoisonError::into_
  inner(); (N1) per-tick Skipped event chatter during a slow fetch → emit only first skip;
  (§6.2 gap) "Fetched N refs" toast gone until P30b lands the status surface.
- **P30b** (reviewer REQUEST-CHANGES → orchestrator folded the MUST-FIX + SHOULD-FIX + NIT, gates
  re-run green) — commit cb2f802. IPC triple (JobStatus/JobStatusChangedPayload/HealthRefreshSettings,
  onJobStatusChanged on IpcApi — accepted house-pattern deviation); SettingsPanel "Background jobs"
  (autoFetch + healthRefresh, §6.3 all-open-repos help text); RepoWorkspace D11 readout ("Fetched Xm
  ago" / "Auto-fetch paused — retrying in Xm") + single enteredBackoff toast + restored "Fetched N
  refs" toast; mock minutes-as-seconds ticks + bonsaiMockJobFail shim + onRepoChanged real registry.
  CARRY-FORWARDS LANDED: S1 lock_recover all scheduler sites + poisoned_locks_recover test; N1
  first-skip-only emission. ORCHESTRATOR FOLDS (from review): MUST-FIX get_job_status_inner now uses
  pub(crate) lock_recover (poison no longer bricks the command forever); SHOULD-FIX event handler
  upserts (readout self-heals when the mount snapshot predates enabling or failed) + enabled:true on
  event; NIT serde skip_serializing_if on updatedRefs/error. bonsai 88 green; clippy/tsc/build clean.
- **P30b AI GATE PASSED (2026-08-03).** Harness (mock, minutes-as-seconds): Background-jobs settings
  round-trip through localStorage (autoFetch on/5m, healthRefresh off/30); ticks fire "Fetched 2 refs"
  toast + readout "Fetched <1m ago"; bonsaiMockJobFail=1 → readout "Auto-fetch paused — retrying in
  1m" + error tooltip; shim off → recovers to "Fetched <1m ago"; zero new console errors (only the
  2 stale HMR dep-array entries from the live-edit session). NOTE: earlier duplicate toasts were a
  stale two-tab session (one workspace per tab, each toasts — expected), verified single after reload.
- **P30 tester** — full regression PASS, no P30 bugs: bonsai_lib 91/92 (scheduler 14→**18**: legacy
  settings through apply_config; repo closed mid-flight → no ghost running/no panic; vanished remote
  → Failed into backoff without wedging; run-now w/o remotes → Failed "no remotes configured" —
  NOTE: a remoteless repo with autoFetch on accumulates silent backoff, by design) + core 267 + all
  41 integration suites green; clippy/tsc/build clean. FLAKE (pre-existing category, NOT P30):
  watcher fires_once_after_touch / git_internals_filtered fail only under full-parallel load (stale
  debounced event escapes the 2.5s drain); pass 5/5 isolated. Flagged as chip task_07e392f9 —
  serialize the watcher fixture, do NOT widen the drain again. Checklist:
  docs/contracts/P30-user-checklist.md.
- **P30 AI GATE PASSED (2026-08-03).** Commits: 467be65 P30a · cb2f802 P30b · 4bad505 tests.
  Planner/backoff/no-overlap/suppression state machine + local-bare-remote integration + browser
  harness all verified; zero regressions. Roadmap B5 delivered (v1: non-destructive jobs only).

**P30 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P30-user-checklist.md):
10-min real network auto-fetch via credential helper with NO prompt storms; offline → exactly one
backoff toast + paused readout, recovery when back online; settings persist across restart;
all-open-tabs fetch; idle CPU sane; suppression during a conflicted merge (SCRATCH repo).
**Current step:** P30 DONE (AI gate passed, awaiting USER CHECKPOINT).

**Watcher flake fix (chip task_07e392f9, user-initiated) — DONE (2026-08-03), commit 1af2c3a:**
fixture-based watcher tests serialized via a static poison-recovering mutex (is_relevant_rules
stays parallel); drain window NOT widened per reviewer guidance. 10 consecutive green
`cargo test -p bonsai --lib` runs post-fix. Caveat: the first post-edit run failed 2 tests whose
names went uncaptured (before 10× green); if the lib suite ever fails again under load, capture
names first — do not assume watcher.

## P31 — per-worktree AI contexts — **in-progress** (2026-08-03)

Fourth/last of the approved P28-scope features (Theme A tie-in deferred from P27). Contract:
docs/contracts/P31-worktree-ai-contexts.md (architect, defaults accepted incl. the two flagged
judgment calls: in-store worktreeActivations map (not sidecar) + BLOCK on dirty tracked targets /
allow untracked overwrites). Design: profiles SHARED in main worktree's .bonsai/profiles.json
(commondir().parent() resolution; worktree.rs main_workdir → pub(crate)); schema v2 adds
worktreeActivations BTreeMap keyed by git worktree NAME ("@main" reserved; v1 loads unchanged,
v2 stamped on next save, legacy activeProfile mirrors "@main"); ONE write path activate_profile_
for_worktree (activate_profile becomes wrapper); guards: locked/prunable/invalid refusal + dirty-
target block + P24 path containment; 3 new commands list_worktree_contexts/preview_worktree_
profile/activate_worktree_profile; WorktreeContextDialog matrix + ProfileActivateDialog
worktreeName ext; P29 drift-rollup integration deferred (D9). Sub-increments: **P31a** core+tests
→ **P31b** commands+IPC+mock → **P31c** UI.
- **P31a** (reviewer APPROVE, 0 must-fix; both deviations accepted) — commit f0122ae. profiles.rs
  schema v2 (worktreeActivations BTreeMap serde-default/skip-empty, version stamped only in persist,
  reads never rewrite — v1 byte-safety tested), "@main" key + legacy mirror both directions + stale-
  key GC, resolve_store_root (commondir), worktree_key_for (gitdir-basename + find_worktree +
  canonical fallback, move-stable), single write path activate_profile_for_worktree (legacy fns →
  wrappers), ensure_eligible invalid→prunable→locked, ensure_targets_clean w/ ACCEPTED bytes-equal
  exemption (raw-bytes compare: CRLF drift can only over-block; equal bytes → loop writes nothing —
  provably un-losable; needed for §9.3 idempotency); worktree_context.rs list_worktree_contexts
  matrix. 13 new tests; core lib 280 + integration green; clippy clean.
  **P31b CARRY-FORWARDS:** (1) SHOULD-FIX gitignored targets false-block — Status::IGNORED must be
  treated like WT_NEW (teams gitignore AI instruction files; use intersect-style tracked-modified
  checks, also covers NIT 5); (2) SHOULD-FIX D5 wrappers' unwrap_or(@main) silently retargets a
  linked worktree whose identity fails to resolve — fall back to @main only when open_repo_at fails,
  propagate identity errors for real repos.
- **P31b** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX folded by orchestrator) — commit 5b8af5f.
  3 commands (house pattern, preview-is-the-gate posture matching P24) + registration + round-trip
  test; IPC triple (WorktreeContextStatus parity pinned by serde snapshot test; worktreeActivations
  optional Record); stateful mock (shared per-worktree file maps; seeds feature-login drifted+missing
  / release-1.2 locked / hotfix-stale invalid; refusal strings byte-identical to backend; activation
  flips only the target row; legacy activateProfile records under the tab's key). CARRY-FORWARDS
  LANDED: intersect-style tracked-dirty guard (IGNORED/WT_NEW never block) + calling_worktree_key
  (@main fallback only for non-openable dirs, identity errors propagate). ORCHESTRATOR FOLD:
  CONFLICTED added to the tracked-dirty set (mid-merge target = most losable). Accepted deviation:
  no D7 dirty-target mock fixture (core fs-oracle covers it; backlog note). core 282 + bonsai 93
  green; clippy/tsc/build clean.
- **P31c** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX folded by orchestrator) — commit 39b8850.
  WorktreeContextDialog (matrix: badges/active-profile/drift chips/blockedReason; per-row select +
  Activate gated EXCLUSIVELY through ProfileActivateDialog — reviewer traced no bypass path; reqId
  guards; Esc bow-out while gate is up); ProfileActivateDialog worktreeName routing (legacy tab path
  byte-identical; worktree confirm errors in-dialog); entry points: worktree menu "AI context…" +
  AiAssetsPanel "Worktrees" button (nested, onActivated→refresh, no re-open loop). FOLDED: preview
  nonce — a confirm failure clears + refetches the preview so re-confirm is always against the
  post-failure diff (banner persists across the reload). tsc + build clean. NOTE: dfea4c6 briefly
  mixed in unrelated staged .claude churn (stashed-skills deletions staged outside this session) —
  reset + recommitted clean as 39b8850; the churn is back to UNSTAGED, still awaiting user decision.
- **P31c AI GATE PASSED (2026-08-03).** Harness (mock, hidden pane → DOM/JS-driven): AiAssetsPanel
  "Worktrees" → matrix w/ 4 seeded rows (@main active opus-rich; feature-login active cheap-terse +
  1 drifted/4 missing; release-1.2 locked → disabled + "pinned for QA"; hotfix-stale invalid →
  disabled); locked/invalid Activate buttons disabled; Activate feature-login → preview gate
  "Activate opus-rich in feature-login" w/ per-target Current/Proposed diffs → confirm flips ONLY
  that row (active opus-rich, missing 4→3), gate closes; @main activation round-trip after the fold
  also green; sidebar "AI context…" menu item verified structurally (synthetic contextmenu doesn't
  fire in the hidden pane — USER CHECKPOINT). Zero console errors.
- **Tester (2026-08-03)** — commit 52689d9. New `crates/bonsai-core/tests/worktree_context_cli.rs`
  (5 fs-oracle tests against the REAL git CLI: two-worktree end-to-end w/ byte-exact files +
  porcelain-status oracle + v2 store JSON + matrix; `git worktree lock --reason` refusal until
  unlock; dirty tracked target zero-write refusal; REAL merge-conflict CONFLICTED block w/
  markers byte-preserved; activation from inside a linked worktree records `wt-a` key, never
  `@main`) + `docs/contracts/P31-user-checklist.md`. Note: the tester agent was interrupted by a
  session restart — deliverables verified + run by the orchestrator: new suite 5/5 green, full
  `cargo test --workspace` exit 0, `pnpm build` (tsc + vite) green.
**Current step:** P31 DONE (AI gate passed, awaiting USER CHECKPOINT). P28–P31 all at USER
CHECKPOINT — see docs/contracts/P28..P31-user-checklist.md.

## P32 — named worktrees + copy uncommitted changes (extends P27) — **in-progress** (2026-08-04)

Approved plan `~/.claude/plans/was-it-intended-to-serene-biscuit.md`. Three changes:
(A) worktree dir moves to `<parent>/.worktrees/<repo-name>/<worktree-name>` — per-repo grouping +
a user-editable NAME decoupled from the branch (default = branch; a worktree's HEAD is independent,
so name ≠ branch); (B) on create, let the user pick which uncommitted / gitignored files to copy
into the new worktree, with pre-flight conflict detection (three-way CONTENT compare: base = main
HEAD blob, target = selected-branch blob, source = main workdir bytes → `clean` if target absent or
== base, else `conflict`) and per-file Overwrite/Skip (NO 3-way merge — raw file-content copy, since
the crate has no `Repository::apply` primitive and diff text is lossy).
Acceptance: worktrees land under `.worktrees/<repo>/<name>`; default name == branch keeps CLI-oracle
tests valid; custom name honored; selected untracked/gitignored/tracked files copy in; conflicts
detected and Overwrite/Skip honored; containment guard blocks path escape; deletions/renames handled
per B5; all gates green (cargo test sequential, clippy, tsc, build, browser harness).
Guardrails unchanged (D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp, no concurrent test+clippy, mock.ts
compiling, orchestrator commits). Sub-increments: **P32a** Part A (path+name) → **P32b** Part B
backend (worktree_copy.rs) → **P32c** Part B commands+IPC+UI.
Contract: docs/contracts/P27-worktrees.md `# P32 extension` (architect done; 4 flagged decisions
ACCEPTED as orchestrator: (1) name collision → `-N` suffix + dialog advisory, preview shows resolved
name; (2) source = currently-open repo workdir & its HEAD ("copy what you're looking at"), not
strictly main_workdir; (3) no size cap v1; (4) non-transactional copy v1). ensure_contained →
pub(crate) for the copy guard.
- **P32a** (reviewer APPROVE, 3 preview-only NITs; NIT-1 folded) — commit 6a62828. derive_worktree(name)
  nests container under dir_basename(main); add_worktree(workdir, branch, name) — branch drives
  checkout, blank name→branch; ensure_contained→pub(crate); command+IPC addWorktree(repoId,branch,name);
  WorktreeCreateDialog editable Name field (default=branch, auto-sync until dirty, path preview slugs
  name, "name in use" advisory, slugify mirrors backend '..' reject). worktree_cli.rs: 3-arg + <repo>
  segment + name-decoupled case → 15 passed. NITs left: usedNameSlugs approximates from branch slugs;
  degenerate no-slash preview path.
- **P32b** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX contract-sync folded) — commit pending.
  New crates/bonsai-core/src/git/worktree_copy.rs (~330 lines): CopyGroup/Verdict/Action enums +
  CopyCandidate/PlanEntry/Selection; list_copy_candidates (read_status + 2nd include_ignored pass,
  deletions excluded, groups disjoint, renames→new path), classify_copy (base=HEAD blob vs
  target=branch-tree blob, Clean iff target None or ==base, else Conflict; unborn-HEAD safe;
  dir/submodule→None; BranchNotFound), add_worktree_with_changes (create via add_worktree, per-Copy
  guarded write: is_unsafe_rel + ensure_contained, NotFound-source skipped, non-transactional). 4
  smoke tests + clippy clean. FOLDED: synced contract text (missing-source = skip, not Io) at
  P27-worktrees.md B.1.4/B5. NITs: staged+unstaged same path = 2 candidates (idempotent copy);
  2nd repo open for ignored pass. Reviewer's tester oracle list captured for P32b tests.
- **P32c** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX + 2 NITs folded) — commit pending. 3 commands
  (list_copy_candidates / preview_worktree_copy / add_worktree_with_changes, _inner + spawn_blocking +
  repo_path, registered) + IPC triple + barrel re-export; mock spans 4 groups w/ seeded conflict
  src/staged-change.ts. UI: NEW WorktreeCopyCandidates.tsx (grouped checkboxes, conflict/unchecked
  chip, Overwrite/Skip toggle) + WorktreeCreateDialog container (fetch on open, debounced preview via
  previewIdRef guard, submit builds CopySelection[]: verified-clean→copy, conflict-or-previewFailed→
  omit unless Overwrite). FOLDED: preview-failure no longer silently copies (previewFailed → needs-
  decision, default Skip + inline advisory); prune conflictActions on uncheck; _inner doc wording.
  Orchestrator self-verified the submit safety path. cargo check + pnpm build exit 0.
- **P32 tester** — full regression PASS, no bugs. New crates/bonsai-core/tests/worktree_copy_cli.rs
  (11 fs-oracle tests: classify unborn-HEAD/equal-vs-diverged/dir-treated-absent/BranchNotFound;
  add_worktree_with_changes copy-writes-workdir-bytes / skip-keeps-branch-version / containment guard
  rejects ../+absolute+C:-prefix / empty=plain; list ignored-vs-untracked / staged+unstaged-same-path /
  rename-new-path+delete-excluded). cargo test --workspace all passed (perf_gate 2 ignored by design;
  no watcher/remote flake); clippy --workspace --tests clean; tsc+build clean. Checklist:
  docs/contracts/P32-user-checklist.md.
- **P32 FRONTEND AI GATE PASSED (2026-08-04).** Browser harness (pnpm dev:mock, DOM/accessibility-tree
  verified — pane not displayed so no screenshot): New-worktree dialog path preview
  /mock/.worktrees/repo/feature-sidebar (per-repo nested + name-slug); Name defaults to branch; 4
  candidate groups unchecked by default; checking seeded conflict src/staged-change.ts → "conflict" chip
  + Overwrite/Skip toggle; checking a clean untracked file → NO toggle; zero console errors.
- **P32 AI GATE PASSED (2026-08-04).** Commits: 6a62828 P32a · 9821279 P32b · a02cbb0 P32c · tests
  pending. Backend fs-oracle suite + frontend harness both verified; zero regressions. Named worktrees
  + per-repo container + copy-uncommitted-changes-with-conflict-detection delivered.

**P32 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P32-user-checklist.md, real
scratch repo): create a worktree with a custom name; copy a mix of untracked skill files + an edited
tracked skill + a gitignored file into it; a real conflict honors Overwrite vs Skip; files land at
`.worktrees/<repo>/<name>/…`; plain create (nothing selected) still works; path-escape is impossible.
**Current step:** P32 DONE (AI gate passed, awaiting USER CHECKPOINT).

---

## Credential-helper fetch/pull/push auth fix — **in-progress** (2026-08-04)

User-reported bug (real repo, not scratch): `git pull`/`git push` succeed via the CLI (a
credential helper already has cached HTTPS creds) but Bonsai fails with its own `AuthFailed`
message ("Configure a Git credential helper...") even from `pnpm tauri dev` in the same terminal
— identical on the user's current machine (macOS) and Bonsai's primary target (Windows), so this
is not a PATH/environment issue. Root cause: the locked M6 Helper credential step
(`crates/bonsai-core/src/git/remote.rs:139-143`) calls `git2::Cred::credential_helper(...)` —
libgit2's own, less-robust reimplementation of the credential-helper protocol — instead of the
real `git` CLI's resolution, so it can fail even when `git credential fill` (what `git pull`
actually uses) succeeds against the exact same config. Plan:
`~/.claude/plans/if-i-use-git-hazy-axolotl.md` (user-approved). Fix: change the Helper step to
shell out to `git credential fill` (cross-platform — no GCM/OS-specific detection or bundling;
`git` itself resolves whatever helper is configured on any OS), with `GIT_TERMINAL_PROMPT=0` to
preserve the locked never-prompt policy, threaded through all 5 `acquire_cred` call sites
(`remote.rs` fetch+push, `tags.rs`, `clone.rs`, `submodule.rs`). Optional: sharpen the `AuthFailed`
message when `credential.helper` is unset in config. This amends the locked M6 credential design
(contract addendum), not a new milestone number — small, single sub-increment.
- Architect wrote the M6 contract addendum (`docs/contracts/M6-remotes.md`, "Addendum
  2026-08-04"). senior-dev implemented `credential_fill` (shells out to real `git credential
  fill` with `GIT_TERMINAL_PROMPT=0`) + rewired `acquire_cred`'s Helper arm + all 5 call sites
  (`remote.rs` fetch/push, `tags.rs`, `submodule.rs`, `clone.rs`) + optional sharper `AuthFailed`
  wording when no helper is configured. reviewer APPROVED (0 must-fix; orchestrator folded one
  NIT — reap the child on a stdin-write failure instead of leaving a zombie). tester added 3 new
  `credential_fill` unit tests in `remote.rs` (well-formed response, 3 failure modes, no-hang
  guard) using `tempfile::tempdir()` instead of the Windows-only `common::scratch_dir()` (a
  deliberate scoped deviation for this macOS session — the shared helper itself was left
  untouched). `cargo test -p bonsai-core --lib git::remote::` 15/15 green; `cargo clippy
  --workspace --tests -- -D warnings` clean; `cargo build --workspace` clean.
  **Separately noted, NOT fixed here (pre-existing, out of scope):** ~35 test failures elsewhere
  in the workspace on this macOS machine, all tracing to `testutil::scratch_dir()` /
  `tests/common/mod.rs::scratch_dir()` being hardcoded to the Windows-only path
  `D:\Temp\bonsai-scratch`; plus one unrelated pre-existing timing flake in
  `watcher::tests::git_internals_filtered`.
**Current step:** AI gate passed (build/tests/clippy green). Awaiting USER CHECKPOINT: real
fetch/pull/push against your actual remote via `pnpm tauri dev` on macOS, confirming Bonsai now
succeeds without changing your existing `credential.helper` config.

---

## Archive

Completed and awaiting-USER-CHECKPOINT milestones (P27 → P2, M0–M6) are archived in
`docs/history/todo-archive.md` to keep this board small. Move a milestone's section there
once it reaches a terminal state.
