# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).
Avoid tauri "test" feature on this machine (STATUS_ENTRYPOINT_NOT_FOUND); use runtime-free
inner functions for command tests.
**USER MANDATE (2026-07-28): never use C: for temp/scratch/mock repos — C: is critically full.
Use `D:\Temp\bonsai-scratch`; when running cargo tests set TMP/TEMP to `D:\Temp` (tempfile
honors them). Include this in every subagent prompt that runs tests or creates repos.**

**USER CHECKPOINT BATCH CONFIRMED (2026-07-30):** the user confirmed in native `pnpm tauri dev`
that ALL previously-pending milestones work — P4, P3a/P3b/P3c/P3d/P3e, P7, P7e, P7f, P3f, P8, P9.
Every "awaiting USER CHECKPOINT" below is now CONFIRMED as of 2026-07-30. (P5/P6 were already
confirmed earlier.)

**USER CHECKPOINT BATCH CONFIRMED (2026-08-03):** the user confirmed ALL remaining pending
checkpoints — the P18–P23 batch, P24, P25, P26, and P27. Every "awaiting USER CHECKPOINT" below is
now CONFIRMED as of 2026-08-03. P18–P27 are fully DONE. Next: P28 (approved plan
`~/.claude/plans/what-are-the-next-quiet-marble.md`): B3 what-changed digest →
P29 D1 repo-health dashboard → P30 B5 scheduler → P31 per-worktree AI contexts.

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

## P28 — Discard hunk + status-panel UX (double-click stage, section styling) — **DONE (USER CHECKPOINT CONFIRMED 2026-08-03)**

Current step: **complete** — USER CHECKPOINT confirmed in native app 2026-08-03 (all three items). A 14c17c5, B ec411de,
C f98b285, tests d-next — all reviewer-APPROVE. AI-gate evidence: 13 unit + 9 CLI-oracle
tests green (byte-for-byte vs git apply --reverse / git checkout --, index invariant),
full bonsai-core suite green, clippy -D warnings clean, tsc+vite build green; harness:
dbl-click stage+unstage verified, tints verified, discard-hunk flow verified end-to-end
(button gated to unstaged, ConfirmDialog, mock state mutates, staged untouched).
USER CHECKPOINT: in pnpm tauri dev — dbl-click feel, discard a real hunk, styling check.
Plan (user-approved):
`~/.claude/plans/1-discard-hunk-button-shimmering-harbor.md`.
Scope (user-requested 2026-08-03):
1. Discard hunk button in diff view (full stack: bonsai-core `discard_partial` reusing
   stage_partial reconstruct with sides swapped, write to worktree; command + IPC + mock;
   danger button in DiffView hunk header, unstaged only, ConfirmDialog).
2. Double-click file row stages/unstages (symmetric, user-confirmed); folder dbl-click already
   exists in tree view (P3f).
3. Subtle tint + accent header differentiating Staged vs Changes sections (user chose option 1).
Increments: A = items 2+3 (frontend only) → review → commit. B = item 1 backend+command,
C = item 1 IPC+UI → review → tester (CLI-oracle tests) → commit.
Reviewer follow-up notes (non-blocking, contract-conformant): (1) §2.4 normalize_terminators
rewrites ALL bare-LF lines in a CRLF-majority file under autocrlf=true (mixed-ending files get
silently normalized; defeats no-op guarantee there) — provenance-aware fix candidate for a
follow-up; (2) contract §6.1.4 renamed guard is unreachable via diff_index_to_workdir (rename
states reject as "stale" instead — still zero writes).
Acceptance (AI gate): cargo suite green (sequential, TMP=D:\Temp), pnpm build, harness: tint
screenshot, dbl-click stage/unstage vs mock, discard hunk mutates mock state behind ConfirmDialog.
Acceptance (USER CHECKPOINT): native app — dbl-click feel, real discard hunk, styling check.

## P27 — Git power feature C1: worktree management — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-03)

Roadmap Theme C item **C1** (roadmap #3; `~/.claude/plans/if-we-think-about-eager-hoare.md`;
memory: repo-management-vision). Highest-demand power feature, already on Bonsai's deferred roadmap. Started
autonomously after P24+P25+P26 shipped (user away 8–10h). Scoped CONSERVATIVELY to mirror the existing
Submodules sidebar-section + open-in-tab pattern so it's fully harness-verifiable (no native folder picker
in v1): list worktrees (path, branch, HEAD, locked, is-current, prunable), create a worktree for a branch at
a DERIVED path (default `.worktrees/<branch>` beside the repo — no folder picker v1), remove (confirm-gated;
libgit2 prune, refuse the main/current worktree), lock/unlock, open-in-tab via the existing openRepo/tab
flow. Defer: custom-path picker (native dialog = USER CHECKPOINT), per-worktree AI contexts (ties Theme A,
later), worktree-move. git2 has `Repository::worktrees()/worktree()/find_worktree()`, `Worktree::{validate,
lock,unlock,is_locked,is_prunable,prune}`, `Repository::worktree(name,path,opts)` for create.
Autonomy rules unchanged: architect→senior-dev→reviewer→orchestrator-commits→tester→AI gate→USER CHECKPOINT;
read-only list first, gate destructive remove behind confirmation; scratch under D:\Temp\bonsai-scratch,
TMP/TEMP=D:\Temp for cargo, no concurrent test+clippy, mock.ts kept compiling. Contract:
docs/contracts/P27-worktrees.md (architect). Sub-increments: **P27a** core list + read cmd + IPC →
**P27b** create/remove/lock/unlock cmds + IPC + stateful mock → **P27c** UI (Worktrees sidebar section +
menu + New affordance + confirm). Key defaults (architect, accepted): create path = `<main_parent>/
.worktrees/<slug>` (sanitized, collision-suffixed); remove refuses main/current/locked/DIRTY (data-loss
guard) via git2 prune(valid+working_tree) + remove_dir_all fallback; lock/unlock; add returns the created
WorktreeInfo. Deferred: native folder-picker custom path, create-new-branch, per-worktree AI contexts.

- **P27a** (reviewer APPROVE, 0 must-fix) — git/worktree.rs: WorktreeInfo, list_worktrees (synthesizes the
  MAIN row + linked rows via Repository::worktrees()/find_worktree; isCurrent vs opened workdir incl. from a
  linked wt via commondir; branch/detached-oid, locked+reason, prunable/valid, stale→valid:false no-panic),
  sanitize_slug + derive_worktree scaffolding (escape-safe BY CONSTRUCTION — no separator/`..` survives).
  list_worktrees command + IPC listWorktrees + stateful mock seed (main+linked+locked+stale). 4 unit + 4
  worktree_cli (git-worktree-list --porcelain oracle) green; clippy + tsc + build clean.
  **P27b CARRY-FORWARDS (from P27a review):** (1) add a stale-worktree no-panic list test; (2) when
  add_worktree actually creates dirs, promote derive_worktree's `debug_assert` containment to a RUNTIME check.
- **PRE-EXISTING (not P27):** `remote_cli::pull_fast_forwards_ref_and_worktree` FAILS at the tip AND at
  f451bb3 (verified via stash) — post-fast-forward `git status` shows `M hello.txt / M shared.txt` (Windows
  CRLF/autocrlf artifact in the pull TEST, or a real dirty-worktree pull bug). Unrelated to P24–P27. Flagged
  as a spawn_task chip (task_944fc6bf) for separate investigation. P25/P26 testers reported it green, so it's
  environment/config-sensitive.
- **P27b** (reviewer APPROVE, 0 must-fix; destructive-path safety trace passed) — commit b315a19. worktree.rs:
  add_worktree (derived `.worktrees/<slug>`, collision-suffixed, returns WorktreeInfo, BranchNotFound /
  already-checked-out → Git), remove_worktree (refuses main-by-name+path / current / locked / DIRTY
  [staged+unstaged+untracked via is_dirty]; prune valid+working_tree WITHOUT locked(true) so a TOCTOU lock
  still refuses; guarded remove_dir_all fallback on the git-owned path only), lock/unlock. CARRY-FORWARDS
  LANDED: stale-list no-panic test; derive_worktree containment promoted to runtime ensure_contained (also
  rejects container-as-leaf). 4 commands (P19 template) + IPC triple + stateful mock (refusal messages mirror
  backend). worktree_cli 11 green; clippy + tsc + build clean. Reviewer SHOULD-FIX logged for the force/
  prune-stale FOLLOW-UP: an invalid-but-present worktree (corrupt gitdir, dir intact) skips the dirty check
  and can be pruned with working_tree(true) — matches contract pseudocode, known data-loss edge.
- **P27c** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX folded by orchestrator) — commit e392f26. Sidebar
  Worktrees section (always shown, name+branch/detached, badges current/main/locked(title=reason)/stale via
  existing badge classes, '+' New button); worktreeMenuItems: Open-in-tab (disabled current/stale) /
  Lock…(PromptDialog reason) / Unlock / Remove… (disabled main/current/locked; ConfirmDialog names the exact
  absPath + dirty-refusal note; backend refusals → error toast); WorktreeCreateDialog (branch select w/
  checked-out disabled, display-only derived-path preview, in-dialog errors; FOLDED: Cancel/overlay/Esc
  disabled while create in flight — no swallowed errors); RepoWorkspace worktrees state + reqId guard +
  refetch in all 4 batches + dialogOpen wiring. ipc/index.ts WorktreeInfo re-export added. Deviations
  (accepted): centered create dialog instead of context-menu branch picker; lock reason prompt added.
- **P27c AI GATE PASSED (2026-08-03).** Harness (mock :1420, hidden pane → DOM-driven): 4 seeded rows w/
  correct badges; '+' dialog (main "(checked out)" disabled, preview /mock/.worktrees/feature-sidebar) →
  Create appends row; locked-row menu Lock/Remove disabled + Unlock flips badge; Remove feature-login →
  confirm names exact dir, Cancel keeps row, confirm removes it; main row all items disabled; stale row
  open-in-tab disabled; zero console errors.
- **P27 tester** — full regression PASS, no bugs: bonsai_lib 69 + core unit 233 + worktree_cli 11→**14**
  (add→list-from-inside flips isCurrent, dirty-refusal preserves content byte-for-byte, lock→refuse→unlock→
  remove e2e, collision dirs proven on disk) + all ~35 integration suites 0 failed; clippy --workspace
  --tests clean; tsc + build clean. NOTE: the flagged pre-existing remote_cli::pull_fast_forwards_ref_and_
  worktree failure did NOT reproduce this run (remote_cli 18/18) — environment-sensitive, chip stays open.
  Checklist: docs/contracts/P27-user-checklist.md.
- **P27 AI GATE PASSED (2026-08-03).** Commits: e962ff1 P27a · b315a19 P27b · e392f26 P27c · 660591e tests.
  Backend oracle suites + frontend browser harness both verified; zero regressions. Roadmap Theme C item C1
  delivered (v1 scope; deferred: custom-path picker, create-new-branch, worktree-move, force-remove,
  per-worktree AI contexts).

**P27 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P27-user-checklist.md, SCRATCH
repo only): Worktrees section lists real worktrees w/ badges vs `git worktree list`; '+' creates a real dir
at `.worktrees/<slug>` (verify with CLI); Open-in-tab opens the worktree as its own tab; lock/unlock flip vs
`git worktree list --porcelain`; Remove is confirm-gated, refuses main/current/locked/dirty with clear
errors (dirty content intact), success really deletes the dir, Cancel removes nothing.
**Current step:** P27 DONE — USER CHECKPOINT CONFIRMED 2026-08-03. Milestone fully closed.

## P26 — AI-asset management A3: skills / subagents / commands manager — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-01)

Roadmap Theme A item **A3** (the #1-priority remaining item; `~/.claude/plans/
if-we-think-about-eager-hoare.md`; memory: repo-management-vision). Direct continuation of the P24 flagship
— promotes the currently "detected (not managed)" `.claude/` group (skills/agents/commands) into a MANAGED
surface in the AI Assets panel: inventory the individual `.claude/skills/*/SKILL.md`, `.claude/agents/*.md`,
`.claude/commands/*.md`, parse + validate their YAML frontmatter, and offer form-based create/edit/delete
with templates. Reuses the P24 assets module + IPC-triple + AiAssetsPanel patterns. Started autonomously
(user away 8–10h) after P24 + P25 shipped. Defer (later): invocation preview/execution, MCP config
manager (A4), Copilot prompt/instructions dirs beyond inventory.
Autonomy rules unchanged: architect→senior-dev→reviewer→orchestrator-commits→tester→AI gate→USER CHECKPOINT;
land read-only inventory first, gate every file-WRITE (create/edit/delete of skill/agent/command files)
behind explicit confirmation; scratch under D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp for cargo, no concurrent
test+clippy, mock.ts kept compiling. Contract: docs/contracts/P26-skills-agents-manager.md (architect).
Sub-increments: **P26a** core parse/validate/inventory + read cmds + IPC → **P26b** write path (create/edit/
delete) + cmds + mock → **P26c** UI (AiAssetsPanel section + AgentAssetEditor) → **P26d** (optional) templates/
advanced FM editor. Key defaults (architect, accepted; flag for user): skill delete = recursive `.claude/
skills/<name>/` dir removal (confirm-gated); NO serde_yaml → complex/multi-line YAML frontmatter is READ-ONLY
+ flagged (never silently rewritten); comments in frontmatter dropped on save; module = assets/bundle.rs;
claudeDir taxonomy row stays managed:false (P26 is a separate managed surface, not wired into P24 drift).

- **P26a** (reviewer APPROVE, 0 must-fix) — assets/bundle.rs: AgentKind(skill|agent|command bare strings),
  per-kind spec, hand-rolled `---`-fence frontmatter parser → ordered Vec<FrontmatterField> (unknown keys
  preserved; complex/multi-line/block-list/nested → complex=true + Error → read-only; flat round-trips
  byte-stable), validate (required fields per kind, name-mismatch, complex), scan_agent_assets +
  read_agent_asset. 2 read commands + read-only IPC triple + mock seed (valid set + a `broken` agent missing
  description + a `release-notes` complex skill). 9 tests; lib 219 green; clippy + tsc + build clean.
  Reviewer verified NO false-negative in complex detection (the anti-lossy-rewrite property). Nits (cosmetic):
  mock localeCompare vs Rust str::cmp; CRLF→LF on serialize; add a nested-map complex test.
- **P26b** (reviewer APPROVE, 0 must-fix; path containment exhaustively traced) — write path. bundle.rs:
  AgentAssetInput (flat frontmatter → complex impossible on write), save_agent_asset (validate name →
  validate_rel_path → atomic temp+rename, create parent+skill dir, serialize_asset; WRITE-ANYWAY per
  contract — invalid content writes + re-scan flags valid:false, only bad NAME hard-errors), delete_agent_
  asset (skill → remove_dir_all `.claude/skills/<name>/` provably confined; agent/command → remove_file;
  missing = no-op). 2 write commands + IPC + stateful mock. 6 tests (bundle 15 total); lib green; clippy +
  tsc + build clean. Reviewer NITs to FOLD INTO P26c: (1) frontmatter field values must stay single-line
  (editor: single-line inputs / strip embedded \n+`---` before save); (2) validate_asset_name should also
  reject Windows reserved device names (CON/NUL/PRN/COM#/LPT#) + trailing dot/space (Windows is target).
- **P26c** (reviewer APPROVE, 0 must-fix; SHOULD-FIX hardened + nits folded) — UI. AiAssetsPanel "Agent
  assets" section: 3 groups (Skills/Subagents/Slash commands) with validation chips (valid / N issue(s) /
  complex — read-only) + New-<kind> buttons, folded into the panel's request-guarded fetch. New
  AgentAssetEditor (per-kind known-field single-line inputs + preserved unknown key/value + body textarea
  + create-mode templates; complex assets open READ-ONLY w/ banner + Save disabled; single-line stripping
  on all frontmatter key+value; save surfaces write-anyway validation issues; skill-delete ConfirmDialog
  names the full .claude/skills/<name>/ dir removal). Folded: validate_asset_name rejects Windows reserved
  device names (CON/NUL/COM#/LPT#) + trailing dot/space (Rust + mock mirror + test). HARDENED the reviewer
  SHOULD-FIX (fail-open data-loss risk): added structural `complex: bool` on the AgentAsset wire type +
  backend save-time re-guard (save_agent_asset refuses overwriting an on-disk complex file → Other, writes
  nothing) so a frontend guard failure can never lossily rewrite complex YAML; frontend guard now keys off
  the flag. bundle 17 tests; lib green; clippy + tsc + build clean.
- **P26c AI GATE PASSED (2026-08-01).** Harness (mock :1420): Agent-assets section lists code-review(valid)/
  release-notes(complex — read-only) skills, broken(1 issue)/test-runner(valid) agents, changelog(valid)
  command; opening release-notes → editor read-only (8 inputs + Save disabled, complex banner); opening
  broken → editable (Save enabled) surfacing "requires frontmatter field 'description'"; skill delete
  confirm reads "permanently removes the entire .claude/skills/code-review/ directory and every file inside
  it"; zero console errors.
- **P26 tester** — full regression PASS, no bugs: bonsai-core 518 (incl. 17 assets::bundle + 5 new
  bundle_cli) + bonsai 69; clippy --workspace --tests clean; tsc + build clean. New crates/bonsai-core/
  tests/bundle_cli.rs: create→scan→read round-trip (all 3 kinds), edit preserves unknown keys in order +
  atomic (no .bonsai-tmp), the complex re-guard (block-list agent → flat overwrite returns Other + file
  byte-UNCHANGED; new/flat saves still work), skill delete removes the whole dir (incl. siblings) vs
  agent/command single-file, validation + Windows-reserved/`..` name safety through the fs. Checklist:
  docs/contracts/P26-user-checklist.md.
- **P26 AI GATE PASSED (2026-08-01).** Commits: ae5ecdf P26a · 1024571 P26b · 9b85263 P26c. Backend fs/
  oracle suites + frontend browser harness both verified; zero regressions. Roadmap Theme A A3 delivered —
  the "detected (not managed)" .claude/ group is now a managed create/edit/delete surface.

**P26 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P26-user-checklist.md, on a
SCRATCH repo — skill delete is a recursive dir removal): 🤖 AI Assets → "Agent assets" lists real .claude
skills/agents/commands with valid / N-issue / complex-read-only chips; create via templates writes real
files (missing-field still saves + flags); a complex-YAML asset opens read-only and cannot be overwritten
(backend refuses); deleting a skill removes the whole .claude/skills/<name>/ dir after a confirm that names
it, Cancel removes nothing.
**Current step:** P26 DONE — AI gate passed, awaiting USER CHECKPOINT. THREE roadmap milestones (P24, P25,
P26) complete this autonomous session.

## P25 — Cheap AI-automation wins: AI review (B1) + stale-branch cleanup (B4) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-01)

Roadmap bucket #2 (`~/.claude/plans/if-we-think-about-eager-hoare.md`; memory:
repo-management-vision). Started autonomously after P24 shipped (user away 8–10h). Two features, both
extending SHIPPED primitives:
- **B1 — AI review of the whole working tree / a branch.** Today `git/ai_explain.rs::analyze_diff`
  supports Review mode over Staged / a single WorkdirFile / a Commit. B1 adds: review the ENTIRE
  working-tree change set (all staged+unstaged together) and review a BRANCH (its diff vs merge-base
  with the default/upstream branch) — surfacing issues before commit/push. WRITE-FREE (produces text).
- **B4 — stale/merged-branch cleanup.** Detect local branches fully merged into the default branch
  (and remote-tracking branches gone from the remote); list them; batch-delete behind explicit UI
  confirmation. Reuses branches.rs + existing confirmation-gated delete.
Autonomy rules unchanged: architect→senior-dev→reviewer→orchestrator-commits→tester→AI gate→USER
CHECKPOINT; land safe/read-only first; gate every destructive/write path behind confirmation; scratch
under D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp for cargo, no concurrent test+clippy, mock.ts kept
compiling. Contract: docs/contracts/P25-ai-review-stale-branches.md (architect). Sub-increments:
**P25a** B1 core+IPC (no new command) → **P25b** B1 UI → **P25c** B4 core+commands+IPC → **P25d** B4 UI.
Key defaults (architect, accepted): B1 worktree = HEAD-tree vs workdir index-aware incl untracked; branch
base auto = explicit→upstream→origin/HEAD→main→master→error; 256 KiB payload cap w/ truncation note. B4
delete uses direct git2 Branch::delete() gated on a SERVER-SIDE freshly-recomputed safe set (never trusts
client) + not-current + not-base; gone-upstream rows unchecked by default, merged pre-checked.

- **P25a** (reviewer APPROVE, 0 must-fix) — B1 core: diff.rs collect_file_diffs (multi-file); ai_explain.rs
  AiDiffTarget::Worktree + Branch{name,base?}, gather_worktree/gather_branch/resolve_branch_base,
  MAX_REVIEW_PAYLOAD_BYTES 256 KiB cap (reserves note len, UTF-8-boundary safe). Reuses the EXISTING
  ai_analyze_diff Review path (0 new commands/types). TS union + mock extended. WRITE-FREE. lib 202 +
  ai_explain_cli 8 green; clippy + tsc + build clean. Nits (cosmetic): redundant untracked opts;
  empty-tree object write (benign, mirrors ai_summary); note says "256 KiB" vs actual cap−note.
- **P25b** (frontend-only; harness-verified worktree, structural for branch item) — B1 UI reusing the
  existing runAnalyze/AiOutputPanel. StatusPanel: new onReviewWorktree + "✨ Review all changes with AI"
  button on the Changes section header (gated aiEligible, disabled while analyzing) → runAnalyze({kind:
  'worktree'},'review','Review working tree'). RepoWorkspace branchMenuItems: "Review branch…" item after
  "Summarize branch…" (localBranch && aiEligible) → runAnalyze({kind:'branch',name},'review',…), base
  auto-resolved backend-side. tsc + build clean. Harness (mock :1420): "Review all changes with AI" opens
  AiOutputPanel titled "Review working tree" with mock review prose + cost. Branch-menu item = exact clone
  of the proven Summarize-branch item (menu render is a USER CHECKPOINT — synthetic contextmenu doesn't
  fire in the hidden pane). No reviewer agent (trivial write-free wiring); folded into P25 tester smoke.
- **P25c** (reviewer APPROVE, 0 must-fix; exhaustive delete-safety trace passed) — B4 core: new
  git/stale.rs. find_stale_branches (merged = base contains all branch commits via graph_descendant_of
  argument-order-correct + CLI-oracle-pinned; goneUpstream = upstream configured but remote-tracking ref
  missing; base chain explicit→origin/HEAD→main→master→current→error; excludes base+current). delete_
  branches: recomputes the safe set server-side (never trusts client), order current→base→notStale→
  notFound→delete, direct git2 Branch::delete(), per-branch Failed is data not a whole-call error,
  detached-HEAD-safe. 2 commands (no consent gate, pure git) + IPC triple + stateful mock. 9 tests incl.
  git-branch-merged oracle + delete-safety survival assertions; lib 210 green; clippy + tsc + build clean.
  Deviation (per orchestrator instruction): CLI oracle lives in stale.rs #[cfg(test)] not a tests/ file.
- **P25d** (reviewer APPROVE, 0 must-fix; 1 SHOULD-FIX + glyph nit folded) — B4 cleanup UI. New
  StaleBranchesDialog (lists stale branches vs baseName; merged rows pre-checked, gone-upstream unchecked
  + amber force-delete hint; empty state; cancelled-guard fetch; per-row outcome annotations for skipped/
  failed after delete). SAFETY GATE: "Delete selected (N)" opens a nested ConfirmDialog listing the EXACT
  names (Cancel-focused, no bypass path) → deleteBranches → summary toast (success/info/error by results)
  → onDeleted refetches branches+graph → list shrinks. Sidebar Branches-header cleanup button (DeleteIcon
  SVG, gated on branch list). RepoWorkspace staleCleanupOpen + globalModalOpen/Escape. tsc + build clean.
- **P25d AI GATE PASSED (2026-08-01).** Harness (mock :1420): cleanup dialog opens with merged-a/merged-b
  pre-checked + feature/gone unchecked (force-delete hint); "Delete selected (2)" → ConfirmDialog lists
  feature/merged-a + feature/merged-b + "Delete 2"; confirm → "Deleted 2 branches" toast + both merged
  rows disappear (stateful mock), leaving only feature/gone at "Delete selected (0)"; zero console errors.
- **P25 tester** — full regression PASS, no bugs: bonsai-core lib 210 (incl. 8 stale:: + B1 ai_explain/
  diff) + all integration (new stale_cli 3, ai_explain_cli 8, branches_cli 25, diff_cli 19, others 0
  failed) + bonsai 69; clippy --workspace --tests clean; tsc + build clean. New crates/bonsai-core/tests/
  stale_cli.rs: end-to-end destructive-path cross-check vs the real git CLI — merged set == `git branch
  --merged main`, gone-upstream flagged; delete leaves the UNMERGED branch intact (key safety), base→
  skippedBase, current→skippedCurrent survive, rerun idempotent. Checklist: docs/contracts/P25-user-
  checklist.md. Clarification (not a bug): a bogus/already-deleted name → skippedNotStale (safe-set check
  precedes the find_branch not-found check; skippedNotFound only via TOCTOU) — matches contract §4.3.
- **P25 AI GATE PASSED (2026-08-01).** Commits: 17e2e8f P25a · ff3e094 P25b · 5d33a7e P25c · da53847 P25d.
  Backend CLI/oracle suites + frontend browser harness (B1 worktree review renders; B4 select→confirm→
  delete→rows-shrink) both verified; zero regressions. Roadmap bucket #2 delivered.

**P25 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P25-user-checklist.md):
B1 — with the real claude CLI + AI enabled, "✨ Review all changes with AI" reviews the whole working
tree and "Review branch…" reviews a branch vs its auto base (256 KiB cap truncates huge diffs); AI-off
hides them. B4 — "Clean up branches…" lists real stale branches vs the real base (merged pre-checked /
gone unchecked), the confirm lists exact names, confirming deletes exactly that set (verify with `git
branch`) and Cancel deletes nothing, current/base never offered. Test on a SCRATCH repo only.
**Current step:** P25 DONE — AI gate passed, awaiting USER CHECKPOINT. Two roadmap milestones (P24, P25)
complete this autonomous session.

## P24 — AI-asset management: context profiles + unified instruction editor — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-07-31)

**Flagship of the "repository management system" roadmap** (approved 2026-07-31,
`~/.claude/plans/if-we-think-about-eager-hoare.md`; memory: repo-management-vision).
Theme A (A2 + A1): manage the AI-asset layer — per-model/per-agent context profiles + generate/sync
instruction files (CLAUDE.md / AGENTS.md / Cursor / Copilot / Windsurf) with drift detection. This is
the white space no incumbent (GitKraken/GitButler) owns; Bonsai already sits on the repo and speaks MCP.

**Autonomous session (user away 8–10h, 2026-07-31):** work the standard loop autonomously; accept
sensible architect defaults and DOCUMENT them; land safe read-only slices first, gate every file-WRITE
path behind explicit UI confirmation + a diff preview; leave all perceptual/native items as USER
CHECKPOINT (never self-declare). Reuse shipped patterns: bonsai-core pure module + CLI/fs-oracle tests,
IPC "triple" (types.ts/tauri.ts/mock.ts) + #[tauri::command] in commands.rs registered in lib.rs, new
components under src/components/. Guardrails: scratch under D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp for
cargo, no concurrent cargo test + clippy, mock.ts kept compiling, orchestrator makes ALL commits.

Contract: docs/contracts/P24-ai-context-profiles.md (architect). Sub-increments: **P24a** core
assets module (taxonomy+inventory+drift+read) → **P24b** profiles store (CRUD+preview+activate write
path) → **P24c** IPC/mock polish (may fold into a/b) → **P24d** UI (inventory/drift panel + profile
manager + diff-preview-gated activation) → **P24e** (optional) AI translate helper.

- **P24a** (reviewer APPROVE-WITH-NITS, 0 must-fix) — commit a139fbb. New pure crates/bonsai-core/
  src/assets/ (taxonomy 12 rows, scan_inventory + read_asset, LOCKED normalize §4.2, drift vs
  auto/override canonical), list_ai_assets/read_ai_asset commands + IPC triple + stateful mock
  inventory fixture. 12 unit tests (git hash-object oracle, normalization, drift); full lib 179 green;
  clippy + tsc + pnpm build clean. NITs (cosmetic, deferred): mock size uses UTF-16 length; hashing
  oracle reuses git2 (contract-sanctioned). Also chore 35fdf6f (vite watcher ignores workspace target).
- **P24b** (reviewer APPROVE-WITH-NITS, 0 must-fix) — assets/profiles.rs: .bonsai/profiles.json store
  (lazy default, corrupt→Other, atomic temp+rename, version:1), ContextProfile/ProfileTarget CRUD,
  preview_profile (writes nothing), activate_profile (validate-all-first, same-dir atomic write,
  Created/Written/Unchanged, path-traversal safe) + 5 commands + IPC triple + stateful mock (activate
  mutates inventory → drift recomputes). 12 tests; lib 191 green; clippy + tsc + build clean.
  Reviewer NITs to FOLD INTO P24d: (1) remove .bonsai-tmp on rename-failure branch (profiles.rs
  atomic_write); (2) mock name-validation misses C1 controls U+0080-009F; (3) pre-existing mock bug:
  MOCK_ASSET_CONTENT keys use backslashes in single-quoted JS (corrupt to CR/TAB) → forward-slash them.
- **P24d** (reviewer APPROVE-WITH-NITS, 0 must-fix; SHOULD-FIX + nits folded) — AI Assets overlay:
  AiAssetsPanel (drift badge + managed/detected groups + sync chips + drift-row two-pane compare +
  Refresh + repo-changed refresh), ProfileManager (list + active chip + create/edit form + targets
  editor with managed-single-file dropdown + Load-from-current + delete-confirm + store hint),
  ProfileActivateDialog (SAFETY GATE: previewProfile first, per-target current-vs-proposed compare,
  Activate-&-write disabled until preview loads, success/info/error toasts). App.tsx header 🤖 button
  (hidden when no repo) + globalModalOpen + Escape wiring. Reuse: simple TextComparePane (not DiffView
  — DiffView is FileDiff/hunk-coupled). Folded: profiles.rs temp-cleanup on rename-fail; mock C1-control
  validation; AiAssetsPanel fetchIdRef + compareIdRef stale-response guards; ProfileManager stable uid
  keys + comment fix. tsc + build clean; profiles tests 12 green.
- **P24d AI GATE PASSED (2026-08-01).** Browser harness (mock :1420, hidden pane → DOM/JS-driven real
  onClick handlers): panel badge "1 file drifted"; CLAUDE.md=canonical, AGENTS.md=drifted, copilot=in
  sync, GEMINI/windsurf/cursorrules=missing; detected group (.cursor/rules 2 files, .mcp.json, .claude);
  drift-row → asset-compare shows AGENTS vs canonical CLAUDE; activate opus-rich → preview dialog (per-
  target current/proposed, CLAUDE.md "changed") → write → AGENTS flips in-sync, opus-rich "active" chip,
  copilot correctly re-drifts vs the NEW canonical (live recompute); New-profile blank form + Add-target
  dropdown = exactly the 6 managed single-file assets + textarea + Load-from-current; cheap-terse
  activate → success toast "Activated 'cheap-terse' — wrote 1 file"; ZERO console errors throughout.
- **P24e** (reviewer APPROVE, 0 must-fix) — optional AI translate helper. assets/generate.rs:
  generate_asset (single-line system+`-p` prompts, source piped via stdin, guidance newline-collapsed
  for the .cmd shim) reusing ai::run_claude; WRITES NOTHING. ai_generate_asset command with the consent
  gate FIRST (before repo_path, mirrors generate_commit_message_inner) → reads sourceAssetId via
  read_asset → spawn_blocking. IPC triple + mock (canned, gated on ?ai=off). ProfileManager per-target
  "Translate for <agent>…" button (source auto = canonical/first managed single-file; target agent =
  row assetId's agent; fills the row textarea, no auto-save; disabled when aiEnabled false; aiUnavailable
  → info toast). aiEnabled threaded App→panel→manager (enabled&&consented&&installed). 3 stub tests; lib
  194 green; clippy + tsc + build clean. Nits (cosmetic, deferred): translate disables all rows in
  flight; agent lookup recomputed 3×.
- **P24e AI GATE PASSED (2026-08-01).** Backend: claude_stub.cmd success→proposed text, error→AiFailed,
  wire camelCase (3 tests). Frontend harness: "Translate for Claude Code…" button fills the target
  textarea with the mock canned translation, cycles Translating…→label, zero console errors. USER
  CHECKPOINT (real claude CLI): translate produces a sane instruction file; AI-disabled blocks it.
- **P24 tester** — full regression PASS, no bugs: bonsai-core lib 194 + integration 280 (4 new
  assets_cli) + bonsai 69 = 543 green; clippy --workspace --tests clean; tsc + pnpm build clean. New
  crates/bonsai-core/tests/assets_cli.rs: real `git hash-object` oracle (external-binary, stronger than
  the git2 unit oracle), activate-writes-real-files end-to-end (preview writes nothing, byte-exact
  writes, no .bonsai-tmp remnant, re-activate Unchanged), drift-flips-after-activation, path-safety +
  rules-dir member listing. Checklist: docs/contracts/P24-user-checklist.md (25 native steps).
- **P24 AI GATE PASSED (2026-08-01).** Commits: a139fbb P24a · a457cdf P24b · 8af41c0 P24d · bffe021
  P24e (+ chore 35fdf6f). Backend CLI/fs-oracle suites + frontend browser harness both verified; zero
  regressions. Milestone = the flagship of the repo-management roadmap (Theme A: A1 unified instruction
  inventory/drift + A2 per-model context profiles + optional AI translate).

**P24 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P24-user-checklist.md):
open a repo with real instruction files → 🤖 AI Assets panel lists them with correct drift chips; edit
to drift AGENTS.md + Refresh flips the chip; create a profile (Load-from-current) → Activate shows an
accurate per-target diff preview → confirm writes byte-exact real files (Cancel writes nothing) →
re-activate reports no changes; .bonsai/profiles.json is created + commit-able; (P24e) with the real
claude CLI + AI enabled, "Translate for &lt;agent&gt;" fills a sane file, disabled-AI blocks it. NOTE:
activating a profile that rewrites the canonical (CLAUDE.md) makes previously-in-sync files show drifted
— that is correct, not a bug.
**Current step:** P24 DONE — AI gate passed, awaiting USER CHECKPOINT. Next: pick the following roadmap
milestone (Theme A A3 skills/subagents manager, or Theme B B1/B4 cheap AI-automation wins).

## P18–P23 — Feature batch (submodules, UI polish, public-release gap) — **in-progress** (2026-07-31)

Source: user request (2026-07-31), 4 asks. Approved plan:
~/.claude/plans/1-add-support-for-fluffy-gizmo.md. Decisions locked (AskUserQuestion):
submodules = read + common ops (list/status + init/update/sync + open-in-tab); settings = wider single
column (~360→560px); feature gap = build ALL four bundles (daily essentials, repo lifecycle, tags &
remotes, interactive rebase + blame). Sequencing: P18 (frontend polish) → P19 submodules → P20 daily
essentials → P21 repo lifecycle → P22 tags & remotes → P23 interactive rebase + blame. Deferred roadmap
(documented, not built now): reflog, worktrees, commit search/filter, LFS, commit signing, config
editing, force-push-with-lease, host/PR integration. Standard loop per milestone (architect→senior-dev→
reviewer→orchestrator commits→tester→AI gate→USER CHECKPOINT); guardrails: scratch under
D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp for cargo, no concurrent test+clippy, mock.ts kept compiling.

### P18 — UI polish (frontend-only): wider settings + whole-row context menu — **in-progress**
- **P18a** — settings dialog wider: `.dialog-card.settings-card { width: 560px }` in src/styles.css
  (higher specificity beats the later `.dialog-card{width:360px}`; keep max-width guard).
- **P18b** — whole-row graph context menu: GraphCanvas.tsx handleContextMenu — when no pill precisely
  hit AND row has a branch/remoteBranch ref, resolve to the preferred branch ref (groupRefs+targetRefOf)
  instead of falling through to commit; RepoWorkspace buildContextItems/handleGraphContextMenu —
  commit-menu fallback when a branch row's branchMenuItems is [] (current HEAD branch).

- **P18 AI GATE PASSED (2026-07-31).** Commit b9b6c69. Reviewer APPROVE-WITH-NITS (0 must-fix; kept
  vite.config.ts out of the commit per the one SHOULD-FIX). pnpm build clean. Browser harness (mock
  :1420, hidden pane → DOM + synthetic contextmenu events): (P18a) `.dialog-card.settings-card`
  computed width = 560px, all 5 sections present; (P18b) synthetic right-click at x=420 (summary zone,
  far past the 180px ref band) on the `feat`/`exp` branch rows opens the FULL branch menu (Checkout…
  Delete), commit-only + current-branch(main) rows open the commit menu (Create branch here / Compare),
  WIP row opens nothing; zero console errors.

**P18 AI gate passed — awaiting USER CHECKPOINT** (native pnpm tauri dev): settings dialog visibly wider;
right-clicking anywhere on a branch row opens the branch menu, on a commit-only row the commit menu.

### P19 — Submodules (read + common ops) — **in-progress**
Contract: docs/contracts/P19-submodules.md (architect). Open decisions RESOLVED (orchestrator accepted
architect defaults): (1) backend returns both `path` (relative) + `absPath` (absolute); (2)
SubmoduleIgnore::None (full dirtiness detection); (3) reuse AppError::Git for not-found (no new variant).
Sub-increments: **P19a** Rust core `git/submodule.rs` (SubmoduleInfo + SubmoduleStatus classify_status +
list/init/update/sync, credential reuse via pub(crate) acquire_cred) + 4 commands + lib.rs registration +
submodule_cli.rs oracle; **P19b** IPC triple (types/tauri/mock stateful) + Submodules sidebar section +
context menu (Init/Update/Sync/Open in new tab) + open-in-tab via existing openRepo flow.
status.rs:89 .exclude_submodules(true) stays AS-IS.

- **P19a** (reviewer APPROVE, 0 must-fix) — commit 52272ad. git/submodule.rs (classify_status via
  SubmoduleIgnore::None: WD_UNINITIALIZED→Uninitialized; INDEX_*|WD_MODIFIED→OutOfSync; WD_INDEX/WD/
  UNTRACKED→ModifiedWorkdir; else UpToDate), list/init/update/sync keyed by name, credential reuse
  (remote.rs acquire_cred→pub(crate)), 4 commands + lib.rs. submodule_cli 4 + 5 lib unit tests green,
  clippy clean. NIT (deferred): WD_DELETED/WD_ADDED unhandled → a deleted-workdir submodule reads
  UpToDate (rare).
- **P19b** (reviewer APPROVE, 0 must-fix) — IPC triple (types/tauri/mock stateful: seeds vendor/libcore
  uninitialized, vendor/theme upToDate, docs/spec outOfSync, tools/ci modifiedWorkdir) + index.ts
  re-export; Sidebar "Submodules" section (name + status badge) + submoduleMenuItems (Init/Update/Sync/
  Open-in-tab, gated: Init only when uninitialized, Open-in-tab disabled when uninitialized);
  RepoWorkspace submodules state + submodulesReqId guard + refetch in all 4 batches + 3 handlers;
  App onOpenRepoPath→openTab (reuses existing openRepo/tab flow, no new command); CSS badge pills.
  pnpm build clean.
- **P19 AI GATE (frontend) verified (2026-07-31).** Browser harness (mock :1420): Submodules section
  renders 4 rows with badges not-initialized/up-to-date/out-of-sync/modified; context menu shows
  Init/Update/Sync/Open-in-tab (Open-in-tab disabled for uninitialized); Init flips vendor/libcore →
  up-to-date, Update flips docs/spec → up-to-date, other rows unchanged; zero console errors. Backend
  AI gate = P19a CLI-oracle suite (green). Pending: tester full-regression + USER CHECKPOINT checklist.

- **P19 tester** — full regression PASS, no regressions/bugs: bonsai-core lib 141 (incl. 5 submodule
  unit) + all integration (submodule_cli 4, remote_cli 18, + all others 0 failed) + bonsai 65; clippy
  --workspace --tests clean; pnpm build clean. Checklist: docs/contracts/P19-user-checklist.md.
- **P19 AI GATE PASSED (2026-07-31).** Commits: 52272ad P19a · f99debe P19b. Backend CLI-oracle +
  frontend harness both verified; zero regressions from the remote.rs shared-signature change.

**P19 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P19-user-checklist.md):
list/init/update/sync round-trip on a real superproject cross-checked with `git submodule status/update`;
Open-in-tab opens the submodule as its own repo tab; the private-remote credential path (no in-app prompt).

### P20 — Daily essentials: amend, cherry-pick, revert, reset, discard — **in-progress**
Contract: docs/contracts/P20-daily-essentials.md (architect). Open decisions RESOLVED (orchestrator
accepted architect defaults): (1) discard restores worktree to the INDEX version (discards unstaged
only); untracked-file deletion OUT of scope v1; (2) empty pick/revert → nothingToCommit (no
--allow-empty); (3) Bonsai starts SINGLE picks only — CLI-started multi-commit sequences not advanced by
*_continue (git2 has no sequencer). Cherry-pick/revert REUSE opstate.rs (already detect CherryPick/Revert,
zero wire change) + conflict.rs + an actionable OpBanner (Continue gated on conflictCount===0, Abort via
ConfirmDialog, no Skip). No AppError change. Amend push-guard frontend-derived (ahead===0 && upstream!==
null); message prefill reuses getCommitDiff(head.oid).details.message. Sub-increments:
- **P20a** — amend + reset + discard (no conflict machinery): commit.rs amend_commit, reset.rs, discard.rs
  + commands + IPC + CommitBox Amend affordance + reset/discard context actions + ConfirmDialogs (hard
  reset + discard). Oracle rows 1,6,7.
- **P20b** — cherry-pick + revert: cherrypick.rs/revert.rs (Repository::cherrypick/revert, outcome
  Committed{oid}|Conflicts{paths}, finalize_* mirroring finalize_merge_commit) + continue/abort commands
  + OpBanner actionable extension + commit-row context actions. Oracle rows 2,3,4,5,8.

- **P20a** (reviewer APPROVE, 0 must-fix) — amend_commit (git2 Commit::amend — correct primitive over
  the contract's rejected repo.commit pseudocode; preserves parents incl. merge, reuses author, fresh
  committer), reset.rs (ResetMode soft/mixed/hard → Repository::reset), discard.rs (discard_paths:
  empty-paths early return prevents whole-worktree clobber; force checkout_index to INDEX version,
  tracked-only, validate_rel_path). 3 commands + lib.rs; IPC triple + index; CommitBox amend button +
  RepoWorkspace amend affordance (prefill via getCommitDiff.details.message; push-warning ahead===0 &&
  upstream); reset menu items (both commit + branch menus, gated) → shared pendingReset ConfirmDialog
  (hard adds destructive warning); StatusPanel ↺ discard control (unstaged tracked rows only) → pending
  discard ConfirmDialog. essentials_cli rows 1/6/7 (8 tests, incl. merge-amend + hard-reset-removes-file
  + discard-preserves-staged) green; clippy + pnpm build clean. NITs (deferred, non-blocking): clear
  amend state when opState leaves 'none' (backend already rejects amend-during-op); import ordering.

- **P20b** (reviewer APPROVE, 1 SHOULD-FIX folded) — cherrypick.rs/revert.rs (CherrypickOutcome/
  RevertOutcome Committed{oid}|Conflicts{paths}; clean→finalize+cleanup_state, conflict→leave state,
  continue→finalize (conflict-free-index guard like commit_merge), abort→reset Hard+cleanup, failed
  start→cleanup, empty→nothingToCommit AFTER cleanup; cherry-pick reuses picked author+message/fresh
  committer, revert authors current sig + git's `Revert "..."` body). REUSE: conflict.rs list/resolve,
  opstate.rs CherryPick/Revert (zero wire change), merge.rs finalize/abort pattern — no parallel conflict
  code. 6 commands + lib.rs; IPC unions + mock (conflict-demo reuses merge marker fixture); OpBanner
  actionable pick/revert (Continue gated conflictCount===0, Abort ConfirmDialog, no Skip, onOpContinue
  dispatch); commit-row menu items. SHOULD-FIX folded: pick/revert items now excluded on detached HEAD
  (match resetMenuItems). essentials_cli rows 2/3/4/5/8 → 14 tests total green (incl. conflict→resolve→
  continue tree≡git, abort restore); clippy + pnpm build clean. NITs deferred: continue error handlers
  don't refreshAll (self-heals via watcher); mock *Continue conflicts-array asymmetry.

- **P20 tester** — full workspace regression PASS, no regressions/bugs: bonsai_lib 65, bonsai_core lib
  153, essentials_cli 14, + new essentials_error_paths.rs 4 (detached-HEAD pick/revert error, reset
  allowed on detached, amend-during-paused-op OperationInProgress), all other integration green; clippy
  --workspace --tests clean; pnpm build clean. Checklist: docs/contracts/P20-user-checklist.md.
- **P20 AI GATE PASSED (2026-07-31).** Commits: eb711c7 P20a · 9f771a8 P20b. Browser harness (mock :1420,
  hidden pane → DOM + synthetic events): Amend affordance renders; 6 discard ↺ controls; commit-row menu
  gains Cherry-pick/Revert/Reset(soft/mixed/hard…); branch rows carry them too; hard-reset ConfirmDialog
  ("recoverable via the reflog … permanently discarded"); discard ConfirmDialog ("reverts to last staged/
  committed … cannot be undone"); zero console errors. Amend toggle+prefill and cherry-pick/revert→OpBanner
  conflict flow are USER CHECKPOINT items (controlled-input + conflict-editor compositing limits, per
  P12/P13). Backend fully oracle-tested.

**P20 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P20-user-checklist.md): amend
rewrites the tip + push-warning; reset soft/mixed/hard (hard confirm); discard restores to index preserving
a staged change; cherry-pick/revert clean + conflict→resolve→Continue (tree≡git) + Abort restores HEAD.

### P21 — Repo lifecycle: clone + init — **in-progress**
Contract: docs/contracts/P21-repo-lifecycle.md (architect). Clone progress via a Tauri
Channel<CloneProgress> (first channel usage in src-tauri — establishes the precedent; no cancellation
v1). Open decisions RESOLVED: (Q2/dest UX) GitKraken-style — FRONTEND picks a PARENT dir + derives the
subfolder name from the URL (last path segment minus .git) → computes full dest=parent/<name>; backend
clone_repo(url, dest) UNCHANGED (dest=final path, still errors if non-empty); (Q1) reuse existing
pickFolder, proper dialog title deferred to Polish; init on an already-a-repo path opens idempotently;
dest file/non-empty → AppError::Io; clone creds via Config::open_default(). Sub-increments:
- **P21a** — backend: git/clone.rs (CloneProgress, clone_repo via RepoBuilder+FetchOptions reusing the M6
  credential chain — NO remote.rs change; init_repo via Repository::init) + 2 non-repo-scoped commands
  (clone_repo takes Channel<CloneProgress>) + lib.rs + lifecycle_cli.rs oracle (local file:// transport).
- **P21b** — IPC triple (types/tauri Channel wiring/mock progress ticks/index) + CloneDialog.tsx (URL +
  parent-folder picker + URL→name derivation + progress bar) + New-repo folder picker; TabStrip + menu +
  empty-state entries; App handlers reusing openTab.

- **P21a** (reviewer APPROVE, 0 must-fix) — commit 24c1885. git/clone.rs CloneProgress + clone_repo
  (RepoBuilder+FetchOptions, shared M6 creds via Config::open_default, transfer_progress→FnMut; dest
  non-empty/file pre-check → Io) + init_repo (idempotent open_ext NO_SEARCH else init); core tauri-free;
  remote.rs UNCHANGED. 2 non-repo-scoped commands (clone_repo takes tauri::ipc::Channel<CloneProgress> —
  FIRST channel usage, sent from spawn_blocking, dropped-channel-safe) + lib.rs. lifecycle_cli 7 green;
  clippy + build clean.
- **P21b** (reviewer APPROVE, 1 SHOULD-FIX folded) — IPC triple (CloneProgress + cloneRepo/initRepo;
  tauri.ts wraps a new Channel<CloneProgress>; mock simulates 30 monotonic ticks + auth/network error
  paths + returns openable/unborn paths) + index; CloneDialog.tsx (URL + parent-folder picker,
  deriveRepoName/joinRepoPath → live "Will clone into parent/<name>", progress bar, in-dialog errors,
  cloneSessionRef race guard); TabStrip + menu + empty-state "Clone repository…"/"New repository…";
  App handlers reuse openTab. SHOULD-FIX folded: deriveRepoName now rejects `.`/`..`/all-dots →
  'repository' (path-traversal guard). pnpm build clean.
- **P21 AI GATE (frontend) verified (2026-07-31).** Browser harness (mock :1420): + menu + empty state
  show Browse…/Clone/New; CloneDialog derives bar.git→bar and scp acme/widget.git→widget, live dest
  preview, progress bar animates 5%→100%, success closes dialog + opens the cloned tab (active); New
  repository… opens an unborn/empty repo tab; zero console errors. Backend AI gate = lifecycle_cli 7.
  Pending: tester full regression + USER CHECKPOINT (real network clone with credential helper).

- **P21 tester** — full regression PASS, no regressions/bugs: bonsai_lib 65, bonsai_core 154, lifecycle_cli
  7→**9** (added clone_brings_all_branches_and_tags + clone_into_path_with_spaces), all integration; clippy
  --workspace --tests clean; pnpm build clean. Checklist: docs/contracts/P21-user-checklist.md.
- **P21 AI GATE PASSED (2026-07-31).** Commits: 24c1885 P21a · 263b135 P21b. lifecycle_cli oracle +
  frontend harness (clone dialog derivation/progress/tab-open + init unborn tab) both verified.

**P21 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P21-user-checklist.md): real
public HTTPS clone (name preview, progress, auto-open tab) + private clone via credential helper (no in-app
prompt) + auth-fail inline error; init → unborn tab → first commit; idempotent init-on-existing.

- **P22a** (reviewer APPROVE, 0 must-fix) — git/tags.rs: create_tag (annotated Repository::tag + tagger
  resolve_signature vs lightweight tag_lightweight; force overwrite; Reference::is_valid_name blocks bad
  names/path escapes; ConfigMissing before mutation), delete_tag, push_tag (refs/tags/x:refs/tags/x, +
  only on force, reuses push_current credential path + push_update_reference→PushRejected). All return ();
  tags re-surface via list_refs. 3 commands + lib.rs; remote.rs UNCHANGED. tags_cli 8 (lightweight/
  annotated parity vs git tag[-a], delete, push-to-bare-remote via show-ref, force-moves, dup/bad errors)
  + unit + noRepo. clippy + build clean.

- **P22b** (reviewer APPROVE, 0 must-fix) — remote.rs additions: RemoteInfo{name,url:Option} +
  list_remotes (Repository::remotes+find_remote, case-insensitive sort, non-UTF8/no-url tolerant),
  add/remove/rename/set_remote_url (is_valid_name→InvalidName, Exists→Git, NotFound→NoRemote;
  rename logs non-default-refspec StringArray + continues; set_remote_url pre-checks find_remote for
  NoRemote parity since libgit2 writes unconditionally). Additive only — fetch/pull/push + credential
  chain untouched. 5 commands + lib.rs. remote_mgmt_cli 5 (parity vs git remote -v/get-url/show-ref
  incl. rename moves tracking refs + all error paths) + 2 unit + noRepo. clippy + build clean.

- **P22c** (reviewer APPROVE, 0 must-fix) — IPC triple (8 methods, wire parity exact incl. push_tag
  remote/tagName; RemoteInfo url string|null) + index; stateful mock (createTag/deleteTag mutate
  branches.tags, pushTag no-op + ?remote= error triggers, remotes list add/remove/rename/set-url mutate
  + tracking-ref adjust, dup/missing throw). New TagCreateDialog (name + lightweight/annotated toggle →
  message textarea) + RemoteEditDialog (name+url; nameReadOnly for Edit-URL); Rename reuses PromptDialog.
  Sidebar: tag-row menu + configured-remote rows (top) + Add-remote (+); RepoWorkspace tagMenuItems
  (Delete confirm / Copy / Push-per-remote) reachable from graph pill + sidebar, remoteMenuItems
  (Rename/Edit-URL/Remove), "Create tag here" in commitMenuItems, remotes state + remotesReqId guard +
  refetch in all batches. Both new files byte-clean (regex-corruption self-fixed). pnpm build clean.
- **P22 AI GATE (frontend) verified (2026-07-31).** Browser harness (mock :1420): commit-row "Create tag
  here" → dialog toggles message textarea on Annotated; Tags section lists tags, tag menu = Delete/Copy/
  Push tag to origin; Remotes section shows origin+url + Add(+); Add-remote dialog → new upstream remote
  appears; zero console errors. Backend AI gate = tags_cli 8 + remote_mgmt_cli 5.

- **P22 tester** — full regression PASS, no regressions/bugs: bonsai_lib 67, bonsai_core 157, tags_cli
  8→**9** (push_annotated_tag_transfers_object), remote_mgmt_cli 5→**7** (rename-without-tracking-refs,
  add-remote-name-collides-with-branch), all integration; clippy + build clean. Checklist:
  docs/contracts/P22-user-checklist.md.
- **P22 AI GATE PASSED (2026-07-31).** Commits: c385efd P22a · 4867a85 P22b · fd41bcc P22c. Backend
  oracle (tags_cli + remote_mgmt_cli) + frontend harness (create-tag dialog, tag menu, remotes section +
  add-remote round-trip) both verified.

**P22 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P22-user-checklist.md): create
lightweight+annotated tag (cat-file -t), delete tag, push tag to a real remote (credential helper, no
prompt), add/rename/edit-url/remove remote cross-checked with git remote -v / branch -r.

### P23 — Interactive rebase + Blame/File-history — **in-progress** (final milestone)
Contract: docs/contracts/P23-interactive-rebase-blame.md (architect). Execution model: **Bonsai-owned
cherry-pick replay on a detached HEAD** with an on-disk JSON sequencer at `.git/bonsai-rebase/state.json`
(git2 has no sequencer; original branch ref untouched until finish → trivial safe abort; squash/fixup
reparent to head.parent(0); reword/squash messages supplied UP FRONT — no message-prompt pause; only
pause is conflict-on-apply). REUSE: conflict.rs + OpBanner + the existing rebase_continue/skip/abort
commands (core gains a delegation branch when `.git/bonsai-rebase/` exists) → ZERO frontend/OpBanner
change for the conflict flow; opstate probes the Bonsai state file FIRST and returns the unchanged
RepoOpState::Rebase. No new AppError/RepoOpState wire. Open decisions RESOLVED (orchestrator accepted):
(1) blame/history line-click → select-with-scroll if a graph revealRow handle is easy, else select-only
(scroll → Polish); (2) file_history best-effort single-rename --follow + no-follow fallback; (3) v1
reorder = Up/Down buttons (drag → Polish); (4) blame against committed HEAD only (atOid=None; dirty-
worktree blame later); (5) `.git/bonsai-rebase/` coexisting with a terminal git rebase is an accepted v1
edge. Sub-increments: **P23a** interactive-rebase engine (rebase_interactive.rs + rebase.rs delegation +
opstate probe) + rebase_interactive_cli oracle → **P23b** IPC + RebasePlanEditor.tsx (per-row action
dropdown + Up/Down + inline reword/squash message) + wiring (reuse OpBanner) → **P23c** blame.rs
(blame_file + file_history) + blame_cli oracle → **P23d** blame/history IPC + BlameView/FileHistoryView +
reveal-in-graph.

- **P23a** (reviewer REQUEST-CHANGES → re-review APPROVE after fixes) — rebase_interactive.rs: cherry-pick
  replay on detached HEAD + on-disk JSON sequencer `.git/bonsai-rebase/state.json` (InteractiveState
  version/headName/originalTip/onto/todos/cursor/committed/paused, atomic write); start (detach + drive),
  drive (Pick/Reword/Squash/Fixup/Drop, conflict→persist+Conflicts{paths}), finish (move branch ref last +
  reattach), continue/skip/abort delegated from rebase.rs when sequencer exists; opstate probes the Bonsai
  file FIRST → unchanged RepoOpState::Rebase. get_interactive_plan seeds default all-pick. 2 new commands +
  lib.rs; continue/skip/abort commands UNCHANGED (core delegates). Reviewer caught 3 SAFETY issues — ALL
  FIXED + re-review APPROVE: M1 (continue out-of-range cursor → finish, no panic), M2 (unified
  restore_to_original FORCE-resets branch ref to original_tip in every abort path incl. post-partial-finish;
  finish keeps remove_state last so a failed finish stays abortable), S1 (squash/fixup with committed==0 →
  Git error, never reparent onto base), N1 (empty-drop guard vs head_tree_id). rebase_interactive_cli 19
  (reorder/squash/fixup/reword/drop tree+topology+message+author, conflict→continue≡git, skip, abort-
  restores-exact-tip, out-of-range-cursor, skip-before-squash-refused) + 7 lib; plain rebase_cli 18 still
  green; clippy clean. NO new AppError/RepoOpState wire.

- **P23c** (reviewer APPROVE, 0 must-fix) — git/blame.rs: BlameLine {oid, authorName/Email/Ts, summary,
  origLineNo, finalLineNo, lineText} + FileHistoryEntry; blame_file (Repository::blame_file, at_oid=None→
  HEAD, content read from THAT commit's tree not worktree, per-commit meta HashMap, binary/missing→Git,
  MAX_BLAME_LINES) + file_history (revwalk topo+time, pathspec-restricted first-parent diff for touches,
  best-effort single-rename follow via rename-detecting diff on first-appearance, limit→MAX_HISTORY).
  2 commands + lib.rs. blame_cli 5 (per-line oid+author vs git blame --line-porcelain, at older commit,
  --follow rename parity, error cases) + 3 unit + noRepo. clippy clean. NITs (optional): oracle twins
  author_name not email; CRLF \r stripped (LF fixture); copies(true) in rename detect.

- **P23b** (reviewer APPROVE, 2 SHOULD-FIX folded) — IPC (getInteractivePlan/startInteractiveRebase,
  wire parity exact; continue/skip/abort reuse existing rebase methods) + RebaseTodoOp/RebaseAction types
  + index; stateful mock (plan = real graph oids base..HEAD all-pick oldest-first; applyInteractivePlan
  reorder/drop/squash/fixup/reword; conflict trigger ?rebase=conflict/c0ffee → opState=rebase + conflict
  fixture, reused continue/skip/abort branch on state.interactive). New RebasePlanEditor.tsx (rows oldest→
  newest, per-row action select, Up/Down reorder, reword/squash inline message, client validation mirrors
  validate_todos: all-drop / squash-fixup-first / empty reword blocked). Entry: commit-menu "Interactive
  rebase from here…" (selected commit = onto base) + branch-menu "Rebase onto … (interactive)", gated
  born+attached HEAD + idle. Folded: removed dead upToDate/fastForwarded outcome cases; mock now removes
  the replayed originals (true rewrite, no graph duplicates). pnpm build clean.
- **P23 (rebase) AI GATE (frontend) verified (2026-07-31).** Browser harness (mock :1420): commit-menu
  "Interactive rebase from here…" → editor "onto 0303030", 3 rows w/ pick/reword/squash/fixup/drop selects
  + Up/Down + message; squash-as-first-row disables Start (validation); clean start (drop last) → "Rebased
  onto 0303030 (2 commits)" + editor closes; ?rebase=conflict start → "Rebase paused at step 1/3" + OpBanner
  Continue/Skip/Abort; zero console errors. Backend = rebase_interactive_cli 19.

- **P23d** (reviewer REQUEST-CHANGES → APPROVE after fixes) — IPC (blameFile/fileHistory, wire parity
  exact) + BlameLine/FileHistoryEntry types + index; mock (blame/history for src/main.rs + README.md using
  real graph-node oids so reveal resolves). New BlameView.tsx (groupBlocks collapses consecutive same-oid
  lines → clickable gutter short-oid+author+relativeDate beside numbered monospace code) + FileHistoryView.tsx
  (oid/summary/author/relative-time rows); StatusPanel threads onBlame/onFileHistory to tracked FileRows
  (Staged+Changes, untracked excluded); RepoWorkspace blame/history overlay state + reqId stale-guards +
  revealCommitByOid (reuses selectedIndex→existing scroll effect = select+scroll free). MUST-FIX FIXED:
  stale-guard gap (close paths + cross-open now bump reqIds → a late response can't reopen a closed overlay,
  no double-overlay); SHOULD-FIX: clear blame/history on unusable repo; NIT: reveal closes the overlay so
  the graph scroll is visible. pnpm build clean.
- **P23 (blame) AI GATE (frontend) verified (2026-07-31).** Browser harness (mock :1420): Blame/History
  buttons on tracked rows; BlameView on src/main.rs shows commit-grouped gutter (0101010 Grace Hopper 1h /
  0505050 5h / 0000000 Ada Lovelace now) + numbered code; clicking a gutter closes the overlay + selects the
  commit (CommitPanel → feat: polish 0101010); FileHistoryView lists 4 entries (oid/summary/author/relative);
  history-row click closes overlay + reveals; zero console errors. Backend = blame_cli 5.

- **P23 tester** — full workspace regression PASS, no regressions/bugs: bonsai_lib 69, bonsai_core 167,
  rebase_interactive_cli 19→**20** (git-native-rebase-in-progress refused), blame_cli 5→**6** (single-commit
  blame), plain rebase_cli 18 + merge/conflict/essentials all intact; clippy + build clean. Checklist:
  docs/contracts/P23-user-checklist.md.
- **P23 AI GATE PASSED (2026-07-31).** Commits: 5fd4f37 P23a · 8ce136d P23b · 418bddb P23c · e7a20f1 P23d.
  Interactive-rebase (engine + plan editor, conflict→OpBanner, abort-restores-exact-tip) and blame/file-
  history (BlameView/FileHistoryView + reveal-in-graph) all backend-oracle + frontend-harness verified.

**P23 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P23-user-checklist.md):
interactive rebase reorder/squash/fixup/reword/drop with git cross-checks; conflict→OpBanner→resolve→
Continue; Abort restores the exact original branch tip; blame per-line vs git blame + click-to-reveal;
file history vs git log --follow.

## ★ BATCH COMPLETE (P18–P23) — ALL AI GATES PASSED, awaiting USER CHECKPOINTS (2026-07-31)

The 4-ask feature batch is fully implemented, reviewed, tested, and AI-gate-verified:
1. **Submodules** (P19) — read + init/update/sync + open-in-tab.
2. **Wider settings dialog** (P18a).
3. **Whole-row graph context menu** (P18b).
4. **Public-release feature gap** — P20 daily essentials (amend/cherry-pick/revert/reset/discard),
   P21 repo lifecycle (clone+init), P22 tags & remotes, P23 interactive rebase + blame/file-history.
Deferred roadmap (documented, not built per user scope): reflog, worktrees, commit search/filter, LFS,
commit signing, git config editing, force-push-with-lease, host/PR integration.
Per-milestone USER CHECKPOINT checklists: docs/contracts/P{18-skip,19,20,21,22,23}-user-checklist.md
(P18 is trivial-visual, folded into its TODO note). All backend features have CLI-oracle parity vs the
real `git` CLI; all frontend surfaces harness-verified against the mock (native-only interactions —
folder pickers, canvas drag, real credentials/network — remain USER CHECKPOINT items).

**Current step: batch complete — awaiting user to run the native checkpoints.**

### P22 — Tags & remotes management — **queued** (contract ready)
Contract: docs/contracts/P22-tags-remotes.md (architect). Open decisions RESOLVED (orchestrator accepted
defaults): (1) delete-tag-on-remote OUT of scope v1 (local delete only); (2) Remotes section renders
configured-remote rows (new list_remotes/RemoteInfo) at top + the existing tracking-branch tree unchanged
below; (3) no new AppError variant (dup tag/remote → git, frontend pre-validates); (4) two small dialogs
(TagCreateDialog, RemoteEditDialog); Rename/Edit-URL reuse PromptDialog; (5) keep `force` through the
stack (UI always sends false v1). Confirmed: branches::list_refs re-reads tag_names fresh each call
(refetchBranches re-surfaces created/deleted tags — no new list_tags); M6 creds already pub(crate)
(push_tag needs NO remote.rs change); list_remotes is NEW (Remotes section today only knows tracking
branches). Sub-increments: **P22a** tags backend (tags.rs create/delete/push + 3 cmds + tags_cli) →
**P22b** remotes backend (remote.rs RemoteInfo + list/add/remove/rename/set_url + 5 cmds + remote_mgmt_cli)
→ **P22c** IPC triple + frontend (8 methods; Sidebar tag menu + configured-remote rows + Add; RepoWorkspace
menus/handlers/dialogs). Launch P22a only after the P21 tester finishes (avoid concurrent cargo runs).

## P17 — Interactive diff: File/Diff toggle + partial staging — **AI GATE PASSED, awaiting USER CHECKPOINT** (2026-07-31)

Source: user request (2026-07-31) — improve the commit/workdir diff view. Five asks: (1) File View
(whole file + inline diff) vs Diff View (hunks only) toggle; (2) stage whole file (exists); (3) hunk
staging; (4) line-by-line staging (+ gutter per changed line); (5) mouse-selection staging.
Deliberately expands past the M4 "read-only, no hunk staging" lock. Plan:
~/.claude/plans/i-want-an-improved-zesty-tarjan.md.
Contract: **docs/contracts/P17-partial-staging.md** (architect to write).

Locked decisions (user, 2026-07-31 AskUserQuestion): (a) SYMMETRIC — granular controls both stage
and unstage; (b) mouse selection → FLOATING "Stage N lines" button near the selection; (c) granular
staging in WORKDIR diffs only (unstaged/untracked→stage, staged→unstage; commit/compare stay
read-only), File/Diff toggle available EVERYWHERE.

Backend approach (design-vetted): BLOB RECONSTRUCTION, not patch-text synthesis. Frontend sends a
selection = list of changed lines by coordinate {kind, oldNo, newNo}; backend recomputes the diff,
reads RAW blob bytes (never the lossy wire content), splices line-slices by line number
(terminator-preserving → CRLF/EOFNL exact), writes via Index::add_frombuffer (mode from an
IndexEntry template). Stage: old=index,new=workdir. Unstage: old=HEAD,new=index. File View = same
diff with context_lines(u32::MAX). Reject binary/too_large/rename/stale-selection.

Sub-increments (each its own implement→review→commit loop):
- **P17a** — Rust: crates/bonsai-core/src/git/stage_partial.rs (stage_partial/unstage_partial +
  LineSelection), diff.rs full_context + pub(crate) collect_file_diff + Deserialize LineKind,
  commands + registration, CLI-oracle + unit tests (partial stage ≡ git apply --cached).
- **P17b** — IPC + mock: types.ts/tauri.ts (LineSelection, stagePartial/unstagePartial, fullContext),
  mock.ts + fixtures three-way line model for the demo file (partial state visible in harness).
- **P17c** — Frontend: DiffView interactive (File/Diff mode, gutter +/−, hunk button, mouse-selection
  floating button), DiffOverlay File/Diff toggle + stageable, RepoWorkspace handlers + refetch, CSS.

Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests (Bash tool:
forward slashes D:/Temp); NO concurrent cargo test + clippy; orchestrator makes all commits
(`wip(P17): …`); mock.ts kept compiling with every IPC change.

- **P17a** (reviewer APPROVE-WITH-NITS; 0 must-fix) — `stage_partial.rs` (blob reconstruction:
  split_keep_terminator/reconstruct/assemble, both directions, guards, synthesize_entry) +
  stage_partial/unstage_partial commands + `full_context` on the 3 file-diff getters
  (`FULL_CONTEXT_LINES=1_000_000` + interhunk_lines — `u32::MAX` overflows libgit2 xdiff). Folded
  SF-1: `should_remove` now presence-based (Stage→status==Deleted, Unstage→HEAD lacks path) not
  byte-emptiness — an emptied tracked file stages an empty blob, not a spurious deletion; +2 pinning
  tests. cargo test 13 unit + 18 CLI-oracle green (git apply --cached tree-equivalence + byte-exact
  CRLF/no-newline); clippy --workspace --tests clean.

- **P17b** (reviewer APPROVE; 0 must/should-fix) — commit d441882. types.ts/tauri.ts LineSelection +
  stagePartial/unstagePartial + fullContext on the 3 getters; mock src/main.rs three-way line model
  (head/index/workdir) with reconstructLines (faithful port of Rust §2.4) so partial stage/unstage
  visibly moves lines and the file shows in both sections. pnpm build green.
- **P17c** (reviewer APPROVE; 0 must-fix) — DiffView interactive (File/Diff mode, per-line gutter
  +/− "Stage/Unstage this line", hunk-header "Stage/Unstage hunk", mouse-range → floating "Stage N
  lines" button), DiffOverlay File|Diff segmented toggle, RepoWorkspace diffViewMode + deriveStageable
  + handleStageLines/handleStageHunk + fullContext fetcher wiring, CSS. pnpm build green. Orchestrator
  harness-verified (mock :1420, hidden pane → DOM + synthetic events): File↔Diff toggle (whole file vs
  hunks); "Stage this line" on unstaged main.rs moves the del to index (hunk -2,7→-2,6) and the file
  shows in BOTH sections; staged main.rs shows "Unstage this line" (symmetric); Stage-hunk button;
  mouse-drag surfaces floating "Unstage N lines"; data-hunk/data-line wired; zero console errors.
  Reviewer S1 (toggle-everywhere gap): commit/compare render via DiffBrowser which lacks the toggle →
  addressed by P17d to honor the locked "toggle everywhere incl. commit/compare" decision.

- **P17d** (orchestrator self-reviewed; frontend-only, single file) — File/Diff toggle in DiffBrowser
  (commit/compare stacked cards) to honor the locked "toggle everywhere". Local `mode` state + modeRef;
  cache key now `${oid}:${path}:${mode}` (pump/enqueue/retry/render) so toggling genuinely refetches
  whole-file vs 3-context; `mode` added to the enqueue effect deps; `.diff-view-toggle` markup (reuses
  the P17c-verified component) in the browser header; DiffView gets only `viewMode` (read-only — NO
  staging in commit/compare). Concurrency/unmount guards preserved. pnpm build green. NOTE: the mock
  harness cannot open DiffBrowser (compare→"unknown commit" mock limit; commit→needs canvas selection,
  blocked while the Browser pane is hidden — same limitation as P7/P10/P11g) → DiffBrowser toggle is a
  USER CHECKPOINT visual item; the toggle component itself is P17c-harness-verified.

- **P17 tester** — full-workspace regression PASS (bonsai-core lib 136 + all integration incl.
  stage_partial_cli 18→20; bonsai --lib 65; bonsai-mcp 5+5; clippy --workspace --tests clean); zero
  regressions from the P17a shared-signature changes. Added 2 adversarial tests (crlf_no_final_newline,
  stage_then_unstage_same_line_round_trips) — both green. No defects. Checklist:
  docs/contracts/P17-user-checklist.md.
- **P17 AI GATE PASSED (2026-07-31).** cargo test (all crates) + clippy --workspace --tests + pnpm build
  green. Browser harness (mock :1420, hidden pane → DOM + synthetic events): File↔Diff toggle (whole
  file vs hunks); "Stage this line" on unstaged src/main.rs moves the del into the index (hunk -2,7→-2,6)
  and the file shows in BOTH Staged & Changes; staged side shows "Unstage this line" (symmetric); Stage
  hunk button; mouse-drag → floating "Unstage N lines"; data-hunk/data-line wired; zero console errors.
  Commits: 59342a0 P17a · d441882 P17b · 56c7a62 P17c · cdfa550 P17d · (tester pending).

**Current step: P17 AI gate passed — awaiting USER CHECKPOINT** (native pnpm tauri dev on a scratch repo,
per docs/contracts/P17-user-checklist.md): stage a single line / a hunk / a mouse-selected range and
verify with `git diff --cached` + `git diff` that EXACTLY those lines moved and the remainder stayed
unstaged; symmetric unstage; the File/Diff toggle on a selected COMMIT's diff and a Compare-with-HEAD
diff (DiffBrowser — could NOT be harness-verified, hidden pane can't drive canvas commit-selection);
edge checks (emptied tracked file → Modified not Deleted; CRLF no phantom ^M; binary/renamed →
whole-file-only + toggle still shows). Next milestone after checkpoint: TBD.

## P15 — In-app AI features (Tier 1) — **AI GATE PASSED, awaiting USER CHECKPOINT** (2026-07-31)

Source: user request (2026-07-31) — after P14 (Tier 2 MCP), build the remaining Tier 1 in-app AI
features from the analysis plan. P13 already shipped the reusable `bonsai_core::ai::run_claude`
text-transform primitive + `check_availability` + the consent gate (`ai_enabled` / `ai_consented` /
`ai_conflict_autonomy`) and ONE consumer (AI conflict resolution). P15 adds the other Tier 1 features,
each reusing `run_claude` (build prompt+payload → call → return text) + a frontend affordance.
Contract: **docs/contracts/P15-ai-features.md** (architect to write).

Sub-increments (each its own implement→review→commit loop):
- **P15a** — Commit-message generation: AI writes a commit message from the staged diff; button in the
  commit box. Smallest lift, highest daily value.
- **P15b** — Explain / review diffs: "explain this diff/commit" + "review my staged changes" fed the
  typed diff data; surfaced in the diff panel.
- **P15c** — Summarize branch / range: "what's unique to this branch" / "what happened between these
  refs", from graph + compare data (+ small range-selection UI).

Reuse the P13 consent gate + availability check for all three; no new AI infra expected.

Decisions confirmed by orchestrator (architect §7): empty inputs reuse AiFailed/NothingToCommit (no
new error kind); P15c range = merge-base (mb..target); "✨ Generate" over a non-empty box confirms
first; base auto-select main→master→HEAD→upstream; explain+review = one `ai_analyze_diff` command.

- **P15a** (reviewer APPROVE-WITH-NITS, 3 cosmetic nits deferred) — commit e592d75. Shared
  `ai/payload.rs` renderer + `git/ai_commit.rs` `generate_commit_message` + command + IPC/mock +
  CommitBox "✨ Generate". cargo build/clippy/test -p bonsai-core + pnpm build green.

- **P15b** (reviewer APPROVE-WITH-NITS) — `git/ai_explain.rs` (`analyze_diff` over `AiDiffTarget`
  commit/workdirFile/staged × explain/review) + `ai_analyze_diff` command + `AiOutputPanel.tsx` +
  three triggers (explain-commit in CommitPanel, explain-file in DiffOverlay, review-staged in
  StatusPanel). Senior-dev caught + fixed a contract serde bug (struct-variant field needs explicit
  `rename="origPath"`; enum `rename_all` doesn't cascade) — locked by a non-vacuous round-trip test.
  Folded reviewer NIT 1 (added `nothingToCommit` to the two doc error lists). Green.

- **P15c** (reviewer APPROVE-WITH-NITS) — `git/ai_summary.rs` (`summarize_range`, merge-base range
  `mb..target`, `AI_SUMMARY_MAX_COMMITS=200` + "(+N more)" note, empty-range→AiFailed before CLI) +
  `ai_summarize_range` command + IPC/mock + Sidebar "Summarize branch…" (base main→master→HEAD→upstream,
  reuses AiOutputPanel). diff.rs: 3 helpers → pub(crate) (surgical, no behavior change). Green.
  Reviewer NIT 1 noted for later: checked-out (HEAD) branch has no context menu so can't be
  summarized — a future graph-commit/HEAD affordance could cover it (contract's optional hook).

- **P15 tester** — 3 CLI-stub integration suites under `crates/bonsai-core/tests/` (ai_commit_cli 3,
  ai_explain_cli 5, ai_summary_cli 5 = 13 new; merge-base count cross-checked vs `git rev-list`).
  Added a `dump_stdin` mode to `claude_stub.cmd` to assert payload content. Found + orchestrator FIXED
  a contract/reality gap: bad-path on `analyze_diff` returned `Other`, not the documented `InvalidName`
  — added the `validate_rel_path`→`InvalidName` guard in ai_explain.rs (mirrors ai_resolve.rs).
- **P15 AI GATE PASSED (2026-07-31).** `cargo build/clippy(lib+tests)/test -p bonsai-core` all green;
  `pnpm build` clean. Browser harness (`pnpm dev:mock`) verified: P15a Generate fills the commit box;
  P15b Review-staged renders AiOutputPanel ("Review staged changes", $0.0060); P15c "Summarize branch…"
  context item → panel "Summary: main → dev"; consent flow gates then enables (Enable→consent dialog→
  active); `?ai=off` disables Generate + hides Review/Summarize; zero console errors. Commits: e592d75
  P15a · f0a9a61 P15b · cab7a79 P15c · (tests+fix pending).

**Current step: P15 AI gate passed; awaiting USER CHECKPOINT** — in native `pnpm tauri dev` with a real
logged-in `claude` CLI: (1) stage real changes → Generate yields a sane message, edit+commit works;
(2) select a commit → Explain gives a coherent summary; Review staged flags real issues; (3) right-click
a feature branch → Summarize produces a sensible vs-base summary; (4) with claude absent/logged-out the
affordances disable cleanly. Next milestone after checkpoint: P16 (Tier 3 embedded MCP).

## P16 — Embedded MCP server (Tier 3, shared live workspace) — **AI GATE PASSED, awaiting USER CHECKPOINT** (2026-07-31)

Source: same user request (2026-07-31). In-app HTTP MCP server (rmcp streamable-http) targeting the
ACTIVE repo tab, so an external client (Claude Code) operates on the same live repo the user sees; the
existing `repo-changed` watcher makes the UI live-update as the AI acts. Contract:
**docs/contracts/P16-embedded-mcp.md**.

Architect (2026-07-31) VERIFIED rmcp 3.0.1 has the HTTP transport (feature `transport-streamable-http-server`;
`StreamableHttpService` mounted in axum via `nest_service("/mcp")`, `LocalSessionManager`) — NO version
bump. Key factoring insight: rmcp's per-session `service_factory` builds a server per session, so the
workdir must resolve at EACH tool call → a `WorkdirSource` enum (`Fixed` for standalone bin / `Dynamic`
closing over `AppState.active_repo` for embedded) is the entire shared-code story (one field + one line
in run_blocking). Sub-increments: P16a factor shared tool layer (bonsai-mcp lib+bin split); P16b embedded
http server + active_repo + `set_active_repo` + bearer token, read-only, default-off UI toggle; P16c
write-gate toggle + 20 mutation tools (bounce-on-change); P16d in-process http MCP client integration
test + live-update demo + `claude mcp add --transport http` docs.

OPEN DECISIONS (surfaced to user, awaiting answers before P16a starts): D-1 axum/token wiring in
src-tauri/src/mcp.rs (rec); D-2 implicit active-tab, no repo-selection tools (rec); D-3 reject any
request bearing an Origin header (rec); D-4 port/token persistence — persisted ephemeral port +
persisted token in settings.json for a stable `claude mcp add` vs per-run token (SECURITY/UX, needs
user); D-5 bounce server on allow_write change (rec); D-6 no per-repo serialization for P16 (rec);
D-7 server default OFF + write default OFF + one-time consent (rec). New deps: axum, rand/getrandom,
subtle — must build on the pinned Windows MSVC toolchain. First inbound network listener in the app →
load-bearing security section (§8).

All 7 decisions RESOLVED (contract §14): D-1 mcp.rs; **D-2 EXPOSE repo-selection tools** (list_repos/
select_repo, per-session selection, 14 read/34 write); D-3 reject Origin; D-4 persist token+port; D-5
bounce on write-gate change; D-6 no per-repo lock; D-7 defaults off + consent. Contract revised in place.

- **P16a** (reviewer APPROVE-WITH-NITS) — `bonsai-mcp` lib+bin split; `WorkdirSource{Fixed,Session}` +
  `SessionRepos` (per-session selection, resolve at call time → NoRepo/InvalidName, no panic);
  `BonsaiServer.workdir` field-type change + `with_session` + private `with_source`; `run_blocking`
  resolves before spawn_blocking. ZERO `#[tool]` bodies changed; the 5 `mcp_stdio.rs` tests pass
  VERBATIM (Fixed path behavior-identical) + 5 new SessionRepos unit tests. clippy clean, workspace
  builds. Two cosmetic nits deferred to P16b (redundant dead_code allow on selected_id).

- **P16b** (reviewer **APPROVE**, clean) — embedded read-only HTTP MCP server. `src-tauri/src/mcp.rs`
  (McpServerState/McpStatus, lazy start/stop, per-session `service_factory` seeding from active_repo,
  `StreamableHttpService` under axum `/mcp`, `auth_layer`); `AppState.active_repo` + `set_active_repo`;
  the 2 D-2 read tools (`bonsai_list_repos`/`bonsai_select_repo`) in server.rs; `get_mcp_status`/
  `set_mcp_enabled`; `mcp-server-changed`; RunEvent::ExitRequested shutdown; settings mcp_* fields;
  frontend types/tauri/mock + Settings "AI access (MCP server)" section. rmcp 3.0.1 / axum 0.8.9 API
  matched contract §2 (no fallback bridge). Security triad verified: 127.0.0.1-only bind + reject-any-
  Origin + Host allowlist + constant-time bearer (subtle) + no CORS; write FORCED off (14 read tools).
  cargo build/clippy/test (mcp_stdio 14/34) + pnpm build green. Write-gate + 20 mutation tools = P16c.

  Browser harness verified: "AI access (MCP server)" section renders; enable toggle raises an accurate
  consent dialog → "Running on port 8765 · 14 tools (read-only)" with Server URL / Bearer token / Register
  command surfaced; toggle off → Stopped; no console errors.

- **P16c** (reviewer **APPROVE**, clean) — MCP write-gate toggle. `service_factory` now reads persisted
  `mcp_allow_write` (default off) → 14↔34 tools; `set_mcp_allow_write` persists then BOUNCES the running
  server (stop old listener + kill sessions BEFORE rebinding the SAME token/port; Windows bind-retry ~5×
  over 250ms then ephemeral fallback with persist). Separate `mcp_write_consented` gate (stronger "modify"
  wording, does NOT reuse read consent); write toggle disabled unless server enabled; status line
  "34 tools (read + write)" vs "14 tools (read-only)". mcp_stdio still 14/34; clippy + pnpm build green.
  Nits deferred: mock resets allowWrite on disable vs real persists (harness fidelity); P16d should assert
  the write→off transport-level revoke.

- **P16d** — testability refactor (senior-dev: extracted AppHandle-free `spawn_server` core from mcp.rs;
  `start()` now thin glue; security wiring relocated-not-changed; 57 tests stay green) + integration test
  (tester: 7 `#[tokio::test]` in mcp.rs `http_integration`, hand-rolled reqwest streamable-HTTP client).
  ALL 7 PASS: (1) 401 no/bad token + 403 Origin-present + 403 bad-Host + valid passes; (2) 14 tools +
  get_graph == compute_graph; (3) 34 tools + full conflict flow tree-oid == git CLI oracle; (4) no
  selection → noRepo; (5) select non-seed repo B → acts on B; (6) unknown→invalidName, closed→noRepo;
  (7) write-off bounce re-negotiates to 14. README gained the embedded-HTTP `claude mcp add --transport
  http` section. No app bugs.

**P16 AI GATE PASSED (2026-07-31).** cargo test -p bonsai 64 + bonsai-mcp 5 green; clippy + pnpm build
clean; browser harness verified the Settings MCP section (P16b). Commits: 3421603 P16a · d1714d8 P16b ·
44c3ae8 P16c · (P16d pending). rmcp 3.0.1 streamable-HTTP, axum 0.8.9. Security: 127.0.0.1-only +
reject-any-Origin + Host allowlist + constant-time bearer + no CORS; read-only + write both default off.

**Current step: P16 AI gate passed; awaiting USER CHECKPOINT** — enable the server in Settings → "AI access
(MCP server)", copy the `claude mcp add --transport http --header "Authorization: Bearer <token>"
http://127.0.0.1:<port>/mcp` line, register with real Claude Code, confirm the bonsai_* tools appear incl.
list/select_repo, the AI acts on the selected open tab, and (write on) an AI stage+commit/conflict-resolve
makes the Bonsai GUI live-update via repo-changed — including on a non-focused tab.

## P14 — `bonsai-core` crate + standalone `bonsai-mcp` MCP server — **AI GATE PASSED, awaiting USER CHECKPOINT** (2026-07-30)

Source: user request (2026-07-30) — "analyze if it makes sense to integrate Bonsai into an AI (e.g.
MCP)". Analysis + approved plan: ~/.claude/plans/analyze-this-app-and-modular-gizmo.md
(Tier 2 chosen: extract a reusable core crate + a standalone MCP server exposing only Bonsai's
*differentiated* surface — graph topology, structured diffs, the conflict trio + safety-railed
mutations — NOT a 1:1 mirror of `git`). Contract: **docs/contracts/P14-mcp-server.md**.

**P14 AI GATE PASSED (2026-07-30) — awaiting USER CHECKPOINT.** Commits: 9d6d52b P14a · be8d538 P14b ·
a47235d P14c · b8dfa46 P14d. `cargo build/test/clippy --workspace` all green (all pre-existing suites +
new `crates/bonsai-mcp/tests/mcp_stdio.rs` 5/5). rmcp pinned **3.0.1** (stdio).

- **P14a** (reviewer APPROVE-WITH-NITS) — converted the single `src-tauri` crate into a Cargo workspace;
  moved pure Git/graph/error/AI logic into `crates/bonsai-core` (lib `bonsai_core`, ZERO `tauri::`
  coupling). Pure move; all 158 unit + 18 integration suites stay green. `testutil` kept `#[cfg(test)]`
  + `tempfile` dev-dep (not shipped in prod — folded the one SHOULD-FIX).
- **P14b** (reviewer APPROVE-WITH-NITS) — `crates/bonsai-mcp` stdio server skeleton + 12 read tools;
  `--repo` startup validation; `run_blocking` keeps git2 off the async path; `AppError {kind,message}`
  preserved as structured tool errors. Dropped unused `anyhow` (the NIT).
- **P14c** (reviewer APPROVE-WITH-NITS) — 20 mutation tools behind `--allow-write`, gated by
  rmcp router-merge so `tools/list` is truthful (read-only=12, write=32; a mutation call in read-only
  mode → JSON-RPC -32602, no side effect). `ConflictResolution` mapped locally (no schemars in core).
- **P14d** (tester) — `mcp_stdio.rs` drives the built binary over stdio: gating, get_graph fidelity,
  the **headline conflict round-trip** (merge→get_conflict→resolve_conflict_text→commit_merge asserting
  the resolved **tree oid == a git-CLI hand-resolved merge**, `32b83b4…`), error discriminant, write-gate.
  README has the full tool catalog + `claude mcp add` wiring.

Decisions locked: `ai`/`ai_resolve` moved into core but NEVER exposed via MCP (consumer is itself an AI);
`fetch`/`pull`/`push` excluded from v1; read-only by default.

**Current step: P14 AI gate passed; awaiting USER CHECKPOINT** — user registers the server with real
Claude Code (`claude mcp add bonsai -- <abs>\target\release\bonsai-mcp.exe --repo <repo> [--allow-write]`),
confirms the `bonsai_*` tools appear and are callable, and that a real conflict resolves end-to-end via
`bonsai_get_conflict` + `bonsai_resolve_conflict_text`. See crates/bonsai-mcp/README.md.

## P13 — Local-AI foundation (Claude Code CLI) + AI merge-conflict resolution — **in-progress** (2026-07-30)

Source: user request (2026-07-30) — integrate AI WITHOUT API keys by driving the user's locally
installed `claude` CLI (subscription auth). Approved plan:
~/.claude/plans/analyze-the-possibility-to-logical-yeti.md.
Contract: **docs/contracts/P13-ai-foundation.md** (numbered P13 because P11/P12 are already spent by
shipped work — feature-followup + conflict editor; all NEW code comments/commits use the `P13` tag).
Deliverable = reusable "run local claude CLI" layer (Rust) + first consumer = AI merge-conflict
resolution. Autonomy is a SETTING: default ProposeReview (propose→review/accept before write+stage)
switchable to AutoResolve (write+stage immediately, review staged diff before commit_merge). Both
finalize via the existing commit_merge (UNCHANGED).

**P13 AI GATE PASSED (2026-07-30) — awaiting USER CHECKPOINT.** Commits: 4e91d8c kickoff · 44fca6e
P13a · e39b444 P13b · d7bfb11 P13c · 051d375 P13d · ad7f61a P13e · 42c3547 autoResolve marker guard ·
d1bd041 tester adversarial suite + user checklist. Tester: full suite 328 passed / 0 failed, clippy
+ pnpm build clean (only pre-existing watcher timing flakes, 5/5 in isolation). Post-review hardening:
autoResolve now refuses to silently stage a markerful proposal (falls back to the review editor +
warning) — reuses hasUnresolvedMarkers; harness-reverified markerless auto-resolve unchanged.
USER CHECKPOINT checklist: docs/contracts/P13-user-checklist.md (native pnpm tauri dev + real
logged-in claude: consent flow; real bothModified merge; Propose&review edit→Accept→2-parent commit;
Auto-resolve repeat; logged-out/absent fallback). proposeReview overlay (CodeMirror) not harness-
compositable → belongs to this checkpoint (same limit as P12).

**Current step: P13 AI gate passed; awaiting USER CHECKPOINT confirmation.**
P13e reviewer APPROVE-WITH-NITS (0 must-fix); folded NIT 1 (guard the proposeReview getConflict await
with fileDiffReqId — was clobber-prone). NIT 2 (probe-state button title) + NIT 3 (shared ConfirmDialog
red button) left as cosmetic. AI GATE (mock :1420, `?op=merge`, hidden pane → DOM/synthetic events):
✨ AI shows ONLY on src/auth.ts (bothModified), hidden on README.md (deletedByThem); button disabled +
"Enable AI features in Settings" title until consent; Settings→enable→consent dialog (verbatim §8.4)→
Enable persists {aiEnabled:true,aiConsented:true}; autonomy radio→autoResolve persists; AutoResolve
click clears src/auth.ts (Conflicts 2→1) + toast "Resolved src/auth.ts with AI — review the staged
result"; no console errors. proposeReview overlay uses CodeMirror (can't composite in hidden pane) →
USER CHECKPOINT (same limit as P12).
Commits: 4e91d8c kickoff, 44fca6e P13a (AI layer, APPROVE-WITH-NITS), e39b444 P13b (settings, self-
review), d7bfb11 P13c (resolver + check_ai_availability/ai_resolve_conflict commands + 4 oracle tests,
reviewer APPROVE), P13d (IPC mirror types/tauri/index + stateful mock — self-review, pnpm build clean;
`?ai=off` availability twin, aiResolveConflict returns markerless proposal without mutating state).
Backend verified: `.cmd`-shim newline fix confirmed vs real claude.cmd; consent gate fires first.
ENV NOTE (2026-07-30): the `D8050`/`c1.dll` build failure some subagents hit when "using D:\Temp" is a
RED HERRING — in the **Bash tool** an inline `TMP=D:\Temp` loses the backslash → `D:Temp` (relative
path) and the libgit2-sys C build dies. FIX: forward slashes in Bash (`TMP=D:/Temp TEMP=D:/Temp cargo
…`); backslashes are fine in the **PowerShell** tool (`$env:TMP='D:\Temp'`). D:\Temp itself is fine.
Scratch repos under D:\Temp\bonsai-scratch. (C: now ~6.4 GB free but the user mandate to keep scratch
off C: stands.)

VERIFIED live on installed CLI **v2.1.220** (de-risk done before contract, one real call succeeded):
- `claude -p --output-format json --safe-mode --tools "" --no-session-persistence --model sonnet`
  runs headless on the **subscription session, NO ANTHROPIC_API_KEY**. Envelope fields used:
  `result` (text), `is_error`, `total_cost_usd`, `session_id`, `subtype`, `type:"result"`.
- **DO NOT use `--bare`** — it forces ANTHROPIC_API_KEY/apiKeyHelper and never reads OAuth/keychain.
  `--safe-mode` keeps subscription auth AND disables the repo's own CLAUDE.md/hooks/skills/MCP.
- `--tools ""` (NOT `--allowedTools`) disables all built-in tools → pure text transform (no
  disk/network/git access). Default resolution model = `sonnet` (opus default ≈ $0.037/trivial call).

Design (locked in contract): backend-spawn via `std::process::Command` under `spawn_blocking` — NO
tauri-plugin-shell, NO new Tauri capability, ZERO new crates for MVP. Claude returns the merged file
body via stdin→stdout (read `result`, defensive fence-strip; NO --json-schema in v1). Bonsai owns
write+stage by REUSING the shipped `resolve_conflict_text` command (P12) — so NO new apply command;
proposal review REUSES `ConflictEditor` seeded with the proposed body (edit-before-accept free).
Backend gate: `ai_resolve_conflict` refuses unless `ai_enabled && ai_consented`. 90s std-only
timeout (drain-and-poll, no crate). Streaming (tauri Channel) deferred to P13f.

Sub-increments (each its own contract-driven senior-dev→review→commit pass):
- **P13a** `src-tauri/src/ai/mod.rs` (`run_claude`, `check_availability`, `RunOpts`, `AiResult`,
  `AiAvailability`); `error.rs` `AiUnavailable`/`AiFailed`; unit tests via a stub `claude`
  (`BONSAI_CLAUDE_BIN`, `tests/fixtures/`).
- **P13b** settings: `ai_enabled`(true)/`ai_conflict_autonomy`(ProposeReview)/`ai_consented`(false)
  additive fields + `AiAutonomy` enum (settings.rs); UiSettings/UiSettingsPatch/apply_patch (commands.rs).
- **P13c** `src-tauri/src/git/ai_resolve.rs` + 2 commands (`check_ai_availability`,
  `ai_resolve_conflict` — proposal only, writes nothing); apply = existing `resolve_conflict_text`;
  tests `tests/ai_resolve_cli.rs`.
- **P13d** IPC mirror types.ts/tauri.ts/index.ts/mock.ts (canned proposal; `?ai=off` toggles
  availability; harness works with no claude installed).
- **P13e** frontend: SettingsPanel AI section (enable + autonomy + availability + consent dialog);
  StatusPanel "✨ AI" conflict-row action (text kinds only); RepoWorkspace `handleAiResolveConflict`
  branching on autonomy (ProposeReview → `ai-proposal:<path>` overlay reusing ConflictEditor; Auto →
  resolve_conflict_text + toast).
Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests; NO
concurrent cargo test + clippy; orchestrator makes all commits (`wip(P13): …`); mock.ts kept
compiling with every IPC change.

## P12 — Rich conflict-resolution editor — **in-progress** (2026-07-30)

Source: user request (2026-07-30). Replace the read-only conflict marker `<pre>` (P3c) with a real
resolution editor. Five asks:
1. **Side-by-side** conflict diff (2-way: ours | theirs — user decision).
2. **Unified** conflict diff.
3. In-editor buttons to select **ours / theirs / combination** per conflict region.
4. **Edit the merged result directly** in the editor.
5. **Scrollbar overview-ruler markers** showing conflict locations in the file (user decision: ticks,
   not a full minimap).

Locked decisions: CodeMirror 6 + @codemirror/merge (net-new deps, user-approved); 2-way ours|theirs
side-by-side; scrollbar overview ruler; combination = ours-block then theirs-block; rich editor only
for text kinds (bothModified/bothAdded) — deleted/added/binary/tooLarge keep the whole-file
ours/theirs/resolved quick actions.

Plan: ~/.claude/plans/i-need-you-to-floofy-crown.md
Sub-increments: P12a Backend (ConflictFile ours/theirs + resolve_conflict_text command + IPC/mock) →
P12b Editor foundation + unified mode (CodeMirror deps, ConflictEditor.tsx, conflictRegions.ts,
conflictSelfTest) → P12c Per-region accept + overview ruler → P12d Side-by-side (2-way) + toggle.
Contract: docs/contracts/P12-conflict-editor.md.
Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests; no
concurrent cargo test + clippy; orchestrator makes all commits; mock.ts kept compiling.

**P12 AI GATE PASSED (2026-07-30) — awaiting USER CHECKPOINT.** Commits: P12a (8ce88cf backend
ours/theirs + resolve_conflict_text + IPC), P12b (de81a4f unified editor + conflictRegions helpers,
CM lazy-split), P12c (74d6dfc per-region accept widgets + overview ruler), P12d (d0af0a2 side-by-side
MergeView + toggle + lazy highlighting), tests (07da23b conflict_cli oracle + user checklist). All
sub-increments reviewer-APPROVED (P12b fixed: lazy-load CM out of main chunk; P12c fixed: undefined
--fg-* CSS vars → --text-*, ignoreEvent semantics).

AI-gate evidence:
- Rust: cargo test 302 passed / 1 known flake (watcher::git_internals_filtered — timing, passes in
  isolation, NOT a regression) / 2 ignored (perf gates). conflict_cli now 13 (4 new resolve_conflict_text
  oracles: stage-0 blob oid + index snapshot + worktree bytes + porcelain all byte-identical to `git add`,
  across several hand-merged contents; leftover-marker + error-path oracles). clippy clean.
- Frontend: tsc + pnpm build clean; mock.ts compiles; CodeMirror code-split OUT of the main chunk (main
  index ~320 kB, CM in a lazy ConflictEditor chunk ~341 kB, language grammars in on-demand siblings).
- Browser harness (mock on an alt port; native pane NOT compositing → document.hidden, so CM
  rendering/interaction + row-click overlay-open are USER CHECKPOINT — verified logic/wire in the REAL
  bundle via dynamic import): shipped conflictRegions.ts self-test 12/0 (parse indices/labels/bodies,
  empty input, hasUnresolvedMarkers, applyResolution ours/theirs/both incl. ours-then-theirs ordering,
  two-region re-indexing after a rewrite); mock getConflict('src/auth.ts') returns kind bothModified with
  non-empty ours+theirs and marker-bearing text; ipc.resolveConflictText wired; lazy ConflictEditor+CM
  chunk resolves with no import errors. ?op=merge seeds the 2 conflicts (src/auth.ts bothModified →
  editor path; README.md deletedByThem → fallback quick-actions).

USER CHECKPOINT (native pnpm tauri dev, self-declare FORBIDDEN) — see docs/contracts/P12-user-checklist.md:
create a real bothModified conflict, open src/auth.ts → (1) ConflictEditor renders with line numbers +
syntax highlighting + tinted regions; (2) per-region Accept Ours/Theirs/Both rewrite correctly (Both =
ours-then-theirs); (3) direct editing works, Stage-resolved disabled until zero markers; (4) unified⇄
side-by-side toggle preserves in-progress edits both ways; (5) overview-ruler ticks render at each
conflict + click-jump; (6) Save stages the file and the conflict clears; resolve all → Commit merge via
OpBanner. Non-text kinds (README.md) still show the read-only fallback + ours/theirs/resolved quick actions.

**Current step:** P12 AI gate passed — awaiting USER CHECKPOINT confirmation.

## P11 — Feature follow-up batch (5 requests) — **in-progress** (2026-07-30)

Source: user request (2026-07-30) with an Azure DevOps PR diff reference screenshot. Five asks:
1. **"Create branch here"** context action on commits AND branches — creates the branch at that
   commit, auto-stashes the working dir, checks out the new branch, then tries to apply the stash.
2. **Tags section collapsed by default** in the left sidebar.
3. **Azure-DevOps-style scrollable all-files diff** — all files' diffs stacked + scrollable; a file
   tree filters by root (whole diff) / folder (its files) / single file. Applies to BOTH
   Compare-with-HEAD AND single-commit (vs first-parent) diffs (user decision).
4. **Auto-fetch** on an interval — OFF by default; active tab only; default 5 min; set in Settings.
5. **Settings page** — expose all four knob groups: auto-fetch interval & toggle; commit node &
   avatar sizes; row height & lane spacing; theme & list view (user decision).

Plan: ~/.claude/plans/this-is-a-folloup-structured-milner.md
Sub-increments: P11a Tags collapsed (trivial, done) → P11f Create-branch-here (backend + menu +
prompt dialog) → P11b Settings model/IPC → P11c Settings page UI → P11d graph-knobs→renderer →
P11e Auto-fetch timer → P11g Scrollable all-files diff. Contract: docs/contracts/P11-followup.md.
Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests; no
concurrent cargo test + clippy; orchestrator makes all commits; mock.ts kept compiling.

**P11 AI GATE PASSED (2026-07-30) — awaiting USER CHECKPOINT.** Commits: kickoff+P11a (contract,
TODO, tags collapsed), P11f (create-branch-here), P11b (settings model+IPC), P11c/d/e (settings
page + graph knobs + auto-fetch), P11g (scrollable DiffBrowser), P11f tests. All reviewer-APPROVED
(P11c/d had a fix round: dead post-P7 dotRadius control dropped, rings scale with node size; P11g a
polish round: sticky header, unmount fetch guard, dead-branch cleanup).

AI-gate evidence: cargo test 136/0 at --test-threads=4 (6 new create_branch_here scenarios;
apply_patch settings tests); cargo clippy + tsc + pnpm build clean; mock.ts compiles. Browser
harness (mock on an alt port; native pane was NOT compositing → document.hidden=true, so
IntersectionObserver-driven lazy hunk fetch and canvas clicks could not run — verified everything
else via read_page/JS + real ref-clicks):
- P11a Tags aria-expanded=false on load; others expanded; no console errors.
- P11c/d/e Settings gear opens 3-section panel (Auto-fetch toggle+interval DISABLED since off by
  default; Graph = node size 6-16 / row 24-48 / lane 10-28, NO dead dot control; Appearance). Ranges
  match. Changing row height 32→48 re-mapped the graph-spacer 1096→1640px live; persisted to mock
  storage and RESTORED across reload. p7SelfTest 29/0.
- P11f "Create branch here" present on branch menu (after Checkout); PromptDialog (input-focused,
  validated); submit created+checked-out the new branch (HEAD moved, Push target updated).
- P11g DiffBrowser opens from compare mode; tree root "All files (n)" + folders + files (flat &
  tree); scope filter verified: root=3 cards, folder src/core=2, file=1; binary file shows the
  "Binary file" placeholder (no fetch); non-binary cards show the loading skeleton (lazy fetch
  pending IO, which is dormant while the pane is hidden); ✕ closes the browser, compare panel stays.

USER CHECKPOINT (native pnpm tauri dev, self-declare FORBIDDEN): (1) "Create branch here" on a real
repo actually stashes → switches → re-applies working changes (clean + conflict cases); (2) enable
auto-fetch in Settings → it hits a real remote on the interval, active tab only; (3) settings persist
across an app restart and the graph node/row/lane changes look right at the extremes; (4) the
DiffBrowser (see P11g REVISION) actually populates diffs on a real large comparison and the
multi-file scroll feels good.

### P11g REVISION — DiffBrowser rework (2026-07-30) — **AI GATE PASSED, awaiting USER CHECKPOINT**

User feedback after testing P11g natively (3 items) + the infinite-loading bug. Contract:
docs/contracts/P11g-revision.md (architect). Commit: 61f8588. Reviewer APPROVED (one NIT fixed:
removed now-dead FileHeaderRow/splitPath/BADGES from CommitPanel). Frontend-only, no Rust/IPC/mock
wire change.
- A: DiffBrowser lost its internal left "All files" tree (was redundant with the right Changes pane)
  → now header + one stacked scroll column. Tree extracted to shared src/components/DiffFileTree.tsx
  (canonical DiffScope), rendered by ComparePanel/CommitPanel as the SOLE scope navigator.
- B: right-pane root/folder/file click drives one lifted `scope` (highlighted) that filters the cards.
- C: compare mode AUTO-OPENS the diff once compareData loads (no "View all changes"); commit mode
  stays explicit-open.
- D (bug): dropped IntersectionObserver (never fired in the user's WebView2, and is suspended by
  document.hidden=true in our harness) → single mount/scope-change effect eagerly enqueues the
  visible scope into the bounded (max 4) queue. THIS is the fix for "diff keeps loading forever".
AI-gate evidence (mock harness, dev-tip compare via a temporary revertible mock fallback, since
reverted): compare auto-opened and cards rendered REAL hunks on first paint with NO scroll/visibility
event; binary file → "Binary file" placeholder (no fetch); refilter root=3 / folder src/core=2 /
file=1 with ZERO skeletons (cache reuse, idempotent enqueue); ✕ in compare exits compare (browser +
panel close, graph restored). tsc + pnpm build clean.
USER CHECKPOINT (native, self-declare FORBIDDEN): on a real ~125-file comparison the diff auto-opens
and every file actually POPULATES (no infinite loading), and the stacked multi-file scroll feels
smooth.

## P10 — Stash-as-node graph redesign + context-menu polish — **in-progress** (2026-07-30)

Source: user request (2026-07-30) with GitExtensions + GitKraken reference screenshots. Four asks:
1. (done, orchestrator) mark all prior USER CHECKPOINTs confirmed — see banner above.
2. **Icons in context menus** — every context-menu action gets a small leading icon (frontend only).
3. **Stash right-click context menu** — right-click a stash in the graph opens Apply/Pop/Drop
   (today the stash target returns no menu). Reuses the existing `stashMenuItems(index)`.
4. **Stashes as their own graph NODES** (GitKraken/GitExtensions style) — REPLACES the P9b
   "pill on the base commit" model. The stash commit becomes a real node on an offshoot lane
   connected to its base, drawn with a stash glyph + its "WIP on <branch>" summary. Rust seeds
   the walk with stash commits and HIDES their index/untracked parents so only the WIP node shows.
Sub-increments: P10a Rust graph (stashes seed walk as own nodes; hide index/untracked; stash
label on the node) → P10b frontend graph render (node glyph + fixtures/mock rework + task-3 stash
context menu) → P10c context-menu icons (task 2). Contract: docs/contracts/P10-stash-as-node.md.
Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests; no
concurrent cargo test + clippy; orchestrator makes all commits; mock.ts kept compiling.

**P10 AI GATE PASSED (2026-07-30) — awaiting USER CHECKPOINT.** Commits: c825e68 (P10a),
52375ea (P10b), da1c74b (P10c). Reviewer APPROVE (0 must-fix; 1 trivial NIT). Key design note:
the contract's `revwalk.hide()` was WRONG (hide(I) transitively excludes the base B) — senior-dev
correctly implemented a **skip-emit set** instead (HashSet + `continue`, row from nodes.len(),
hidden oids filtered from parent lists); contract §1.4 corrected to match. Wire UNCHANGED
(renderer detects a stash node via `node.refs.some(kind==='stash')`); M2d 20k perf gate untouched
(skip-emit is a no-op for stash-free repos). Orphan-stash behavior reversed (now rendered + pulls
its base into the walk) — encoded in the rewritten test.
AI-gate evidence (mock harness :1425, hidden pane → canvas pixel-sampling + synthetic events):
cargo test --lib 125 pass / 0 fail (new `stash_appears_as_own_node`, 3 scenarios); cargo clippy
-D warnings clean; pnpm build clean; p7SelfTest 29/0. Harness (tight STASH_COLOR match): 3 stash
node discs (266 stash-color px each) with white drawer glyphs on offshoot lanes at the top rows;
`stash@{n}` pills ONLY on the stash node rows (base rows core work 4/2 carry no stash pill);
right-click a stash pill → Apply/Pop/Drop menu (icons present); branch menu = 6 items all with
SVG icons (Checkout/Copy/Merge/Rebase/Compare/Delete); stash menu = 3 icons; no console errors.
USER CHECKPOINT (native pnpm tauri dev): (1) create a real stash → it appears as its own node on
an offshoot lane linked to its base, with the stash glyph + "WIP on <branch>" summary; scrolling
stays smooth; (2) right-click the stash node's pill → Apply/Pop/Drop work end-to-end (Drop
confirms); (3) every context-menu action shows a crisp leading icon in both light and dark themes.

## P7 — GitKraken-style graph layout — **in-progress** (2026-07-29)

Current step: P7 AI GATE PASSED (2026-07-29) — awaiting USER CHECKPOINT. Commits: e96c0fa (kickoff
+ contract), 1f0e337 (P7a pure helpers+metrics+self-test), d77788e (P7b renderer restructure), f23d0b8
(P7c hit-test parity + tooltips + dead-pill removal), 78c8048 (P7d fixtures). Contract:
docs/contracts/P7-gitkraken-layout.md. Rust/wire UNCHANGED (frontend-only). Reviews: P7a/b/c/d all
APPROVE (0 must-fix). Locked decisions honored: initials-only HSL avatars, collapse only same-commit,
all refs in left column.
AI-gate evidence (mock :1424, verified by canvas pixel-sampling + synthetic events since the Browser
pane is hidden — screenshots + rAF-scheduled/scroll-sweep repaints unavailable while hidden): pnpm
build clean every increment; window.__bonsai.p7SelfTest() = 26 pass/0 fail (initials, avatarColor
determinism+format, groupRefs same-commit collapse + diverged-separate, layoutRefLabels overflow,
refColArea, avatarHit, relativeDate). Harness: three zones (refs LEFT / graph+avatars CENTER / summary
+relative-time RIGHT), NO right-side pills, NO author text; avatars = HSL initials discs + lane ring +
HEAD/sel rings; row 0 collapses main(L+R) & dev(L+R) with laptop+cloud + "+N" chip; "+N" hover tooltip
lists hidden entities (release / # v1.0 / # v0.9); diverged feat = row1 laptop-only local (5-item local
menu) vs row4 origin/feat cloud-only (remote menu) — separate; single-word author row3 → avatar tooltip
"torvalds" (initials TO); avatar hover → full-name tooltip (role=tooltip, Ada Lovelace); left-column
right-click parity — feat/exp/dev local 5-item menu, origin/feat remote menu, current main → NO menu;
LIGHT theme forced-sync-repaint shows white canvas bg + saturated (theme-invariant) avatar disc; DARK
theme pixel-verified; no console errors. Dead pill helpers (pillStyle/pillArea/layoutRowPills/pillWidth/
LaidPill) removed; PillStyle kept.
NOTE (harness limits, hidden pane): scroll-sweep + theme-toggle repaints use requestAnimationFrame which
is PAUSED while the Browser pane is hidden — could not run the rAF scroll perf sweep. Perf is
structurally preserved (Rust layout math untouched; only visible rows draw) but the perceptual 20k
smoothness re-check belongs to the USER CHECKPOINT.
USER CHECKPOINT (native pnpm tauri dev): (1) rows read as three clean zones (refs left, graph+avatars
center, message+relative-time right); (2) avatars show correct initials for real authors, hover shows
full name, same author → same color; (3) a level local/remote branch shows ONE label with laptop+cloud,
and after committing locally (diverge) it splits into two labels; (4) "+N" hover lists hidden refs;
right-clicking refs in the left column drives the identical P6 menu; (5) scrolling a large repo stays
smooth (perf).

### P7e — post-checkpoint layout refinement (2026-07-29) — AI GATE PASSED, awaiting USER CHECKPOINT
User screenshots showed two overlap defects + a sizing request; contract §13 (P7-gitkraken-layout.md).
Frontend-only (Rust/wire UNCHANGED). Commit: cc6acd4. Reviewer APPROVE after 1 must-fix round (a
pre-existing p7SelfTest fixture budget regressed under the chip-fit clamp; fixed by widening the test
budget). Changes: (A) layoutRefLabels reserves room so the "+n" overflow chip always fits inside the
LEFT ref band — no spill into the graph/avatar; still pure + single source of truth for both hit-tests.
(B) Viewport.rightInset = scroller.offsetWidth-clientWidth → relative-time/summary drawn clear of the
vertical scrollbar (default 0 == old geometry). (C) bigger avatars: rowHeight 28→32, avatarRadius 8→10,
head/sel rings + avatarFont scaled (all parametric off METRICS.rowHeight).
AI-gate evidence (mock :1424, pixel-sampling + synthetic events, hidden pane): pnpm build clean;
p7SelfTest 27 pass/0 fail (new chip-fit assertion last.x+w ≤ startX+budget); (A) row-0 ref content ends
x=115 ≤ band-end 172, wide gap to avatar, "+4" chip overflow tooltip lists dev/release/# v1.0/# v0.9;
(B) date right edge 624 = width(652)−scrollbar(15)−colGap(12) (old would be 640, under the scrollbar);
(C) avatar disc+rings ~28px wide (was ~22), avatar hover tooltip "Ada Lovelace" resolves at new geometry.
USER CHECKPOINT (native): overlaps gone (no "+n"/graph collision; timestamps not clipped by the
scrollbar) and avatars read as comfortably larger, at whatever window size / theme.

### P7f — ref-collapse fix + branch tooltip + copy (2026-07-29) — AI GATE PASSED, awaiting USER CHECKPOINT
Frontend-only (Rust/wire UNCHANGED). Contract §14 (P7-gitkraken-layout.md). Commit: 00ac7f2.
(A) groupRefs collapse key bug: keyed remote branches by lastIndexOf('/') → a slashed branch
(origin/topic/x) mismatched local topic/x and showed twice; fixed to indexOf('/') (strip only the
remote name) → collapses to one laptop+cloud pill. (B) hovering a shown branch pill shows the full
branch name (new 'ref' TooltipState variant). (C) "Copy branch name" added to the shared
branchMenuItems (graph pills + sidebar), clipboard-undefined-guarded. Also removed an embedded-NUL
delimiter in sameTarget so GraphCanvas.tsx diffs as text now.
AI-gate: pnpm build clean; p7SelfTest 28/0 (new slashed-collapse assertion); harness: no chevrons
in file rows, dir hints correct. USER CHECKPOINT (native): a level local+remote slashed branch shows
ONE pill; branch-pill hover shows full name; "Copy branch name" works in both menus.

### P3f — changes-panel refinements (2026-07-29) — AI GATE PASSED, awaiting USER CHECKPOINT
Frontend-only. Contract: docs/contracts/P3f-changes-panel.md. Commit: d546662.
#1 double-click a directory row applies the section action to all descendants (Changes → stage all;
Staged → unstage all) via a new generic Tree onActivateDir + dirActionHint; #2 removed the misleading
chevron from file leaf rows (diff opens in center overlay; row already highlights). Dir chevrons and
ConflictRow untouched. AI-gate (mock :1424, tree view): 9 dir toggles with correct hints, 0 chevrons
in file rows; double-click "assets" dir moved Changes 8→7 / Staged 4→5. USER CHECKPOINT (native):
double-click stages/unstages the whole folder; leaves read cleanly without arrows.

## P8 — Merge with autostash — AI GATE PASSED, awaiting USER CHECKPOINT (2026-07-29)
Contract: docs/contracts/P8-merge-autostash.md. Reviewer APPROVED (stash never silently dropped).
The tester's 7-row matrix caught TWO real regressions, both fixed (no test-expectation hacks):
(1) this libgit2's stash_pop returns Ok() for a CONTENT conflict (writes markers) and DROPS the stash
→ false green "restored". Fixed by using stash_apply + index.has_conflicts() + conditional stash_drop
in pop_after_success + rollback_and_map (a conflicted re-apply RETAINS the stash → StashPopConflicts).
(2) the borrow reorg made repo.merge use an Oid-based annotated commit → conflict markers stamped with
the 40-char oid instead of the branch name. Fixed by rebuilding a reference-based annotated commit for
the normal-merge path. Also: rewrote 3 pre-P8 merge_cli tests to the new autostash semantics (staged
autostashed / unstaged-merge-touched → StashPopConflicts / abort keeps edit on stash), pinned #2 against
a real `git merge --autostash` oracle; compile-only fixes for the enum growth in sibling test files.
AI GATE: cargo clippy clean; pnpm build/tsc clean; FULL suite 271 passed / 0 failed / 2 ignored (perf,
separate) — 115 unit + 156 integration. Env note: native bonsai.exe locks target/debug/bonsai.exe →
use `cargo test --lib` + separate CARGO_TARGET_DIR for integration.
USER CHECKPOINT (native pnpm tauri dev): right-click a branch → Merge with a dirty worktree; FF/merge
lands and the local changes reappear; the paused-merge and pop-conflict toasts read clearly and the
stash is findable via `git stash list`.
User request: context-menu Merge should FF-by-default (already does) AND stash dirty changes → merge
→ re-apply (git merge --autostash). Backend+frontend. Two OPEN QUESTIONS resolved by orchestrator with
the architect's safe defaults (user asleep, authorized autonomous best-option choice): (1) NO
REINSTATE_INDEX — staged returns unstaged; (2) deferred re-apply on a paused conflicted merge — leave
autostash on the stack + toast, never dropped. New MergeOutcome shape: stashed:bool on FF/Merged/
Conflicts + new StashPopConflicts{head,paths}. Command surface UNCHANGED. AI gate: cargo clippy +
pnpm build clean; 7-row behavioral test matrix (tester) vs scratch repos; harness renders each toast
via mock triggers. USER CHECKPOINT (native): merge a branch with a dirty tree; changes reappear.

## P9 — Stash management (list/apply/pop/drop/create + view in graph) — AI GATE PASSED, awaiting USER CHECKPOINT (2026-07-29)
Contract: docs/contracts/P9-stash-management.md. Commits: da53c41 (P9a), 324be0d (P9b), b5a0cf4 (P9c).
All 5 OPEN QUESTIONS resolved to the recommended defaults (user-authorized autonomous): include
untracked on create; pop-with-conflict allowed+retained; apply/pop require clean state, drop any state;
no REINSTATE_INDEX; pill label `stash@{n}` (message in sidebar). Orchestrator direction: sidebar
Stashes section + a `stash` RefLabel PILL on each stash's base commit (reusing ref-pill machinery) —
NOT synthetic nodes in the perf-gated walk.
- P9a (git/stash.rs + 5 commands + registration): reviewer APPROVE; tester 8-row matrix green; reuses
  the P8 apply+inspect+conditional-drop so a conflicted pop/apply RETAINS the stash.
- P9b (graph.rs collect_stash_bases + layout_walk step 6.5 attach; draw.ts stash entity/violet pill/
  drawer icon; types.ts RefKind 'stash'; fixtures rows 3 & 6; buildContextItems no-menu for stash):
  reviewer APPROVE (+ multi-stash-on-one-base graph test); harness violet pills render.
- P9c (Sidebar Stashes section + RepoWorkspace handlers/menu/Drop-confirm + mock 5 cmds + tauri.ts +
  IpcApi): harness-verified end-to-end (create 2->3, pop 3->2, drop-with-confirm 3->2, re-index correct).
AI GATE: pnpm build clean; cargo test --lib 125 passed / 0 failed; p7SelfTest 29/0; no console errors.
USER CHECKPOINT (native pnpm tauri dev): the Stashes sidebar lists real stashes with Apply/Pop/Drop
(Drop confirms) + a "Stash changes" action; each stash shows as a pill on its base commit in the graph;
operations round-trip against a real repo (cross-check `git stash list`).

Original step: P7 kickoff. Source: user request (2026-07-29) after
confirming P6 works. Four requirements:
1. **Three-zone row layout** like GitKraken: LEFT = ref labels, CENTER = graph lanes+dots,
   RIGHT = commit summary + relative timestamp. (Today everything is one left→right run:
   dot → pills → summary → author-text → date.)
2. **Author initials avatar** on each commit node (colored circle w/ initials, color hashed from
   author NAME — no network, no gravatar). Hover → full author name tooltip.
3. **Multi-ref overflow**: keep the "+N" chip, but on hover list the hidden refs stacked
   vertically (HTML tooltip/popover).
4. **Collapse local+remote** into ONE label with icons (laptop = local, a remote/cloud icon =
   remote) INSTEAD of showing `main` and `origin/main` separately — but ONLY when they sit on the
   SAME commit; when diverged, show separately (locked decision).
Locked decisions (user, 2026-07-29 AskUserQuestion): initials-only colored circles (no network);
collapse local+remote only when same commit (separate when diverged); ALL refs (branches, remotes,
tags) move to the LEFT column (right side is purely summary + timestamp).
Orchestrator assessment: expected to be MOSTLY FRONTEND — Rust already sends author name + all refs
per node, so graph.rs likely UNCHANGED. Work concentrates in src/graph/{draw.ts,metrics.ts,
GraphCanvas.tsx}, plus ref-grouping + avatar helpers + an HTML tooltip overlay; hit-testing/context-
menu (P5/P6 parity) must follow the refs to the left column. Rules: scratch repos under
D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests; orchestrator makes all commits.

## P6 — Unified branch/remote context menus — **done** (USER CONFIRMED 2026-07-29)

Current step: P6 DONE — USER CONFIRMED (2026-07-29, "confirmed it works"). Commits: e3ae58e (P6a
backend tip + checkout_remote/delete_remote_tracking + 6 CLI-oracle tests), e3a2e39 (P6b IPC/mock/
fixtures), 1334a5c (P6cd unified branchMenuItems + strip sidebar buttons + moved/new delete
confirms), a8fc529 (P6b mock compareWithHead "No differences" for HEAD-tip refs). Contract:
docs/contracts/P6-unified-context-menus.md. Reviews: P6a APPROVE (0 must-fix; safe-checkout-first
ordering + all 6 tests verified strong), P6cd APPROVE (0 must-fix; menu parity + derived dialogOpen
+ sidebar strip verified). Remote menu scope = FULL set (Checkout+Delete) per user choice
(2026-07-29). P6c+P6d landed as ONE frontend commit (§4.5 removes setDialogOpen while §4.6 removes
the onDialogOpenChange prop — split would not typecheck).
AI-gate evidence: cargo test --lib 108 pass + branches_cli 25 pass (independently re-run, isolated
target dir on D: to dodge the bonsai.exe lock from the user's tauri dev); clippy clean; pnpm build
EXIT 0. Harness (mock :1424, both themes, no console errors): sidebar rows glyph+full-width name,
ZERO inline buttons; local non-current row/pill → identical 5-item menu (Checkout/Merge/Rebase/
Compare with HEAD/Delete), current main → no menu, tag → no menu; remote row/pill (incl.
origin/release with no local) → 5-item remote menu; graph-pill parity confirmed (same branchMenuItems
path, cur recomputed live); Compare on origin/main (tip=HEAD) → "No differences"; remote Checkout on
origin/release → creates+switches to local "release", origin/release intact; remote Delete → "Delete
remote-tracking reference" confirm (local-only wording) → row removed, other remotes intact; local
Delete → "Delete branch" confirm. NOTE: non-HEAD branch Compare-with-HEAD not renderable in the mock
(branch tips intentionally decoupled from graph-row ids) — wiring verified (panel opens, calls
compareWithHead(tip)); real diffs covered by P5a Rust tests.
USER CHECKPOINT (native pnpm tauri dev): (1) right-click a remote pill/row → Checkout creates a local
tracking branch + switches (upstream set); (2) right-click a remote → Delete removes ONLY the local
remote-tracking ref (server branch untouched, a fetch can bring it back); (3) the unified menu behaves
identically from graph pill and sidebar row for checkout/merge/rebase/compare/delete on local + remote.

Original step: P6 kickoff. Source: user request (2026-07-29) after
confirming P5 works. Three requirements:
1. **One context menu per branch** (graph pill AND sidebar row) with ALL applicable actions:
   Checkout, Merge …into current, Rebase current onto …, Compare with HEAD, Delete. Single place
   that acts on a branch. Current/HEAD branch → no actions (empty menu, no open), as today.
2. **Strip inline action buttons from the sidebar branch rows** — names take all available space;
   actions live only in the right-click menu.
3. **Same unification for remotes** — right-click menu (Merge/Rebase/Compare with HEAD — checkout
   and delete of remote-tracking refs are NOT supported in v1, so they are omitted), names fill
   available space.
Implementation shape (orchestrator default): a single shared `branchMenuItems(name, kind)` builder in
RepoWorkspace (owns every handler) consumed by BOTH `buildContextItems` (graph) and a new Sidebar
`onContextMenu(name, kind, x, y)` prop. Compare-with-HEAD needs the ref's tip oid → add `tip: string`
(full oid) to backend `BranchInfo`/`RemoteBranchInfo` + IPC types + mock fixtures; menu resolves tip
by name from the branches snapshot, reusing existing `compareWithHead(repoId, tip)`. Move the
delete-confirm ConfirmDialog + pendingDelete state UP from Sidebar to RepoWorkspace so the graph
pill's Delete works too (wire pendingDelete→dialogOpen for shortcut suppression). Graph
GraphContextTarget unchanged (tip resolved via snapshot). Rules: scratch repos under
D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests; orchestrator makes all commits
(`wip(P6): …`); mock.ts kept compiling with every IPC change.

## P5 — Graph context menus — **done** (2026-07-29, USER CONFIRMED)

Current step: DONE — USER CONFIRMED it works (2026-07-29). All 4 sub-increments
committed: 31082c1 (P5a backend), c6da6db (P5b IPC/mock), 621006f (P5c graph hit-test + merge/
rebase menu), 5813902 (P5d Compare panel + overlay). Contract: docs/contracts/P5-graph-context-menus.md.
Reviews: P5a APPROVE-WITH-NITS (non-blocking), P5b orchestrator self-review (mechanical IPC mirror),
P5c APPROVE (pass-5a refactor verified pixel-identical; useCallback NIT folded in). P5d was
finished by the orchestrator directly (user interrupted the senior-dev delegation and said
"continue implementation") on top of pre-existing uncommitted scaffolding found in the working
tree (compare state cluster + clearCompare/refetchCompare + ComparePanel.tsx + DiffOverlay/
CommitPanel edits); orchestrator added the missing wiring (handleCompareWithHead/
handleToggleCompareDiff, buildContextItems commit branch, ComparePanel render precedence,
onSelect exit-compare wrap, Esc layering, refetchCompare in the 3 refresh batches).
AI-gate evidence: cargo test --lib 108 pass (7 new compare tests, P5a; full `cargo test` bin link
blocked by user's running tauri dev on 1420 — lib verified); clippy clean; pnpm build EXIT 0
(tsc clean). Harness (mock :1423, no console errors) verified: (P5c) feat/origin-main pills open
merge/rebase menus with correct wording, main(own)/tag/head/commit-row open none, Esc/outside/
scroll dismiss, both themes, pass-5a pixel-identical; (P5d) commit row -> "Compare with HEAD" ->
ComparePanel "HEAD (main) 9fceb02 -> 0303030 core work 4" + file list -> compare: overlay w/ hunks,
Esc closes overlay then compare, left-click exits compare, both themes. NOTE: HEAD-vs-itself
"No differences" empty state not reachable in the mock (MOCK_OID decoupled from graph row ids) —
covered by Rust test compare_head_to_itself_is_empty + trivial JSX.
USER CHECKPOINT (native pnpm tauri dev): (1) right-click a branch pill in the graph -> Merge/Rebase
runs against the real repo; (2) right-click a commit -> "Compare with HEAD" shows the correct
git-diff-HEAD-<commit> file list + per-file diffs, two-endpoint header reads correctly, HEAD-vs-itself
shows "No differences".

Original step: P5 kickoff — writing architect contract. Source: user request (2026-07-29) after
P4 toolbar refinements (committed 9cdd635; P4 AI gate still awaiting USER CHECKPOINT).
Two features:
1. **Merge/rebase on graph branch pills** — right-click a local (non-current) or remote-tracking
   branch ref pill in the commit graph → context menu with Merge/Rebase, mirroring the sidebar
   affordances EXACTLY (same gating: currentBranch != null, not busy/opActive). Reuses existing
   `handleMergeBranch`/`handleRebaseBranch` in RepoWorkspace. Needs ref-pill hit-testing in the
   canvas (draw.ts currently draws pills but exposes no geometry). Frontend-only.
2. **Compare HEAD ↔ selected commit** — right-click a commit row/dot → "Compare with HEAD" showing
   the tree-vs-tree diff between HEAD and that commit. Needs a NEW backend command
   (`diff_tree_to_tree` engine already exists in git/diff.rs) + IPC contract addition + mock twin +
   a UI surface (recommend a right-panel Compare mode reusing the FileDiffHeader list + DiffOverlay
   per-file pattern). Backend + IPC + frontend.
Direction decision (orchestrator default, surface to user): old=HEAD, new=selected commit
(= `git diff HEAD <commit>`), header labels both endpoints clearly.
Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests;
orchestrator makes all commits (`wip(P5): …`); mock.ts kept compiling with every IPC change.

Note: P4 post-gate toolbar refinements committed after the AI-gate record: 9cdd635 (Fetch/Pull/Push
moved into the top toolbar, centered; Refresh pinned right). P4 USER CHECKPOINT still pending.

## M0 — Scaffold — **done** (2026-07-27)

AI gate passed (cargo test 6/6 + CLI cross-check, clippy, pnpm build, browser harness live incl.
mock isolation); USER CHECKPOINT confirmed by user: all 5 checklist items pass in `pnpm tauri dev`.
Contract: docs/contracts/M0-scaffold.md. Sub-increments: c090459, 0cd1d4d, b60fc36, a3cfadd.

## M1 — Working-directory status — **done** (2026-07-27)

AI gate passed (29/29 tests incl. 14 porcelain oracle, watcher 3× stable, builds, live harness);
USER CHECKPOINT confirmed by user: watcher auto-update, CLI git add, refresh button, refocus
rescan, bare-repo banner all pass. Contract: docs/contracts/M1-status.md.
Sub-increments: ee3bb52, 65c6c73, 04a87df, a87a41e.

## M2 — Commit graph (centerpiece; M2a–M2d) — **done** (2026-07-28)

AI gate passed (46 tests + 2 release perf gates: layout 317ms/500 limit, serialize 10ms/5.44MB,
sweep maxWindow5Avg 23.6ms over100=0, harness renders verified). USER CHECKPOINT confirmed by
user: scrolling the 31k fixture repo feels smooth in the native app, all checklist items pass.
Contract: docs/contracts/M2-graph.md. Sub-increments: cfba129, cccbb2f, 5ad41a6, d2a6cf5, 8ed7807.
Perf notes: relax_odb_hash_verification() at app init; wire 5.44MB@31k (streaming fallback
documented §8.2 if ever needed); regression lever = cached repo handle in AppState. Perf notes: mempack fixture gen;
strict_hash_verification(false) global opt-out needed for <500ms; next lever if regression =
cached repo handle in AppState. Polish candidates: refresh-failure alignment, frame-log tagging.
WARNING: C: drive ~0 bytes free — flag to user.

Goal: Rust computes GraphLayout from a commit walk seeded from all local branches,
remote-tracking branches, and tags; topological-then-date ordering; deterministic lane colors
stable while scrolling; React renders GitKraken-style canvas graph (curved fork/merge edges,
commit dots, ref pills incl. HEAD/detached), virtualized to visible rows. Sub-increments:
- M2a — layout engine + unit tests (pure Rust, no UI)
- M2b — canvas rendering of a static precomputed layout (harness screenshot)
- M2c — virtualization, scrolling, ref pills, HiDPI canvas scaling
- M2d — perf gate: fixture generator + criterion benchmark

Acceptance (AI gate): lane/edge unit tests on tricky fixture histories; harness screenshots show
correct lanes/curves/dots/pills; synthetic 20k+ commit fixture built via git2 or fast-import
(NOT CLI commits); criterion layout < 500 ms for 20k commits; harness scroll test logs rAF frame
timings with no sustained frames > 33 ms.
Acceptance (USER CHECKPOINT): scrolling the 20k repo in the native app feels smooth.

## M3 — Stage / unstage / commit — **done** (2026-07-28)

AI gate passed (80 tests incl. 18 CLI-oracle + 3 adversarial; CRLF normalization fixed; harness
round-trip verified). USER CHECKPOINT confirmed by user: stage/unstage/commit round-trip works
in the native app, all checklist items pass. Contract: docs/contracts/M3-commit.md.
Sub-increments: ab0f943, 2ce24be, e484daa.
Polish notes: textarea disabled during stage-in-flight (focus drop); dismissed-error
string-compare; ignored-file staging = git add -f semantics (documented).

## M4 — Diff view — **done** (2026-07-28)

AI gate passed (106 tests + 2 ignored: 19 diff CLI-oracle incl. 3 adversarial probes — BOM
parity, rename+edit vs `git diff -M`, empty-added vs emptied; harness verified: mode A exact
hunks, mode B commit panel with details/parents/merge note). USER CHECKPOINT confirmed by
user: selecting a commit shows details + changes in the native app, all checklist items pass.
Contract: docs/contracts/M4-diff.md. Sub-increments: 3ab213a, 7200824, 4bac8e6.
Polish candidates: keep old diff visible during same-key refetch (skeleton flash on
focus/watcher tick); React.memo(DiffView); CommitPanel messageBody first-line strip.

Goal: via git2, working-dir diffs (unstaged vs index, staged vs HEAD) AND commit diffs
(selected graph node vs first parent, with commit details — message/author/date — in the right
panel). Unified or side-by-side per architect.

Acceptance (AI gate): diff output matches `git diff` / `git show` in tests; harness renders
both diff kinds from mock data.
Acceptance (USER CHECKPOINT): selecting a commit in the graph shows its details + changes in
the native app.

## M5 — Branches — **done** (2026-07-28)

AI gate passed (131 tests + 2 ignored: 19 branches CLI-oracle + 5 adversarial probes;
reviewer confirmed delete reachable ONLY via ConfirmDialog, backend blocks head/unmerged
delete with no force path; harness verified all flows live). USER CHECKPOINT confirmed by
user: branch ops + confirmation dialog work in the native app.
Contract: docs/contracts/M5-branches.md. Sub-increments: 7264523, 043faba, d4727f9.
Polish candidates: ?fixture=detached mock header not overridden; shared isAppError util
(3rd copy in Sidebar); dialog actionable if branch removed externally (falls back to
branchNotFound banner — acceptable).

## M6 — Remotes (fetch / pull ff-only / push) — **done** (2026-07-28)

AI gate passed (165 tests + 2 ignored: 18 bare-repo round-trips + 5 adversarial probes incl.
force-rewrite guard; no force-push/force-checkout paths; credential attempt guard unit-tested;
harness verified all UI flows live). USER CHECKPOINT confirmed by user (local Part A + real
network Part B round-trip with credential helper, no in-app prompt).
Contract: docs/contracts/M6-remotes.md. Sub-increments: 1862dbf, 40c9a65, 71baa54.
Credential strategy user-confirmed 2026-07-28: helper (Windows Credential Manager) →
SSH agent → default; never prompt/store in-app. Documented divergence: stale-tracking push →
Bonsai upToDate vs CLI non-ff reject (fetch-first resolves).
**MVP COMPLETE** — v1 definition-of-done core loop all shipped.

## Polish — **done** (2026-07-28)

USER CHECKPOINT confirmed by user: shortcuts, toasts, WIP row, recents/reopen, empty states
all pass in the native app. Release build verified: MSI + NSIS bundles at
src-tauri/target/release/bundle (Bonsai 0.1.0 x64). **v1 COMPLETE** per CLAUDE.md
definition of done.

## P4 — UX refinements (post-P3 feedback) — **in-progress** (2026-07-29)

Current step: P4 AI GATE PASSED (2026-07-29) — awaiting USER CHECKPOINT. All 5 increments committed:
969bd70 (P4a+b layout), 6b6befd (P4c Changes), a7070d9 (P4d sidebar), 086a85e (P4e-1 chip),
25844ae (P4e-2 highlighting). Each reviewer-APPROVE (P4a/P4c/P4d/P4e-2 APPROVE; P4a+b & P4e-1
APPROVE-WITH-NITS, nits trivial/fixed). pnpm build EXIT 0 (highlight.js langs code-split into lazy
chunks). Harness (mock on :1421, no console errors) verified all 5: P4a .tab-strip overflow now
visible + .tab-scroll wraps pills + Browse… fully visible (menu escapes); P4b .graph-toolbar
justify-content:center holds Fetch/Pull/Push, Refresh stays flex-end top-right; P4c sections
Staged + Changes(8) + one Stage all, no Unstaged/Untracked headers, untracked→green A; P4d branch
folders feature/fix + remote origin render collapsed, current branch main sorted first (HEAD=main
root-level so nothing auto-expands — correct); P4e rust diff chip "rs" (#dea584) + 23 hljs token
spans, keyword color = --syn-keyword in BOTH dark(#ff7b72) and light(#cf222e), add/del backgrounds +
marker + gutters preserved. USER CHECKPOINT: confirm the 5 in native pnpm tauri dev (see below).

Original step: P4 kickoff — contract written (docs/contracts/P4-ux-refinements.md), starting Inc 1.
Source: user returned from P3 checkpoint with 5 UI issues; decisions locked via AskUserQuestion:
- #1 tab `+` menu overflow: FIX (bug — `.tab-strip` overflow-x forces overflow-y auto, clips menu).
- #2 Fetch/Pull/Push: centered bar ABOVE the graph; Refresh (⟳) STAYS top-right.
- #3 Unstaged+Untracked: MERGE into one "Changes" section (untracked badge → green A; one Stage all;
  Staged + Conflicts stay separate). Presentation-only — WorkdirSection union + diff-key grammar unchanged.
- #4 sidebar: tree mode ref folders collapsed by default EXCEPT the current-branch chain (auto-expanded);
  current branch sorted first; sections stay open; flat mode = current-first.
- #5 diff colors: BOTH file-type accent chip AND full per-line syntax highlighting (highlight.js, lazy,
  CSS-var themed via --syn-*). Client-side presentation only — zero IPC/wire/Rust change.
Frontend-only milestone. Zero backend/IPC change (architect confirmed). Mock stays compiling
(fixture-data-only additions allowed for P4e visuals).
Sub-increment plan (orchestrator): Inc1 = P4a+P4b (layout), Inc2 = P4c, Inc3 = P4d,
Inc4 = P4e-1 (language util + accent chip), Inc5 = P4e-2 (syntax highlighting). Each implement→review→commit.

## P3 — Diff overlay, trees, merge/rebase, multi-repo tabs — **in-progress** (2026-07-28)

Current step: P3e — IN PROGRESS. Contract written: docs/contracts/P3e-multi-repo-tabs.md (5 sub-increments
P3e-a..e). Orchestrator resolved §10 open Qs: repoId==canonical path; focus-rescan=active-tab-only;
tab branch/dirty badge + toast repo-prefix DEFERRED to polish (keep TabStrip lean).
Done: P3e-a backend core (be49130, reviewer APPROVE, cargo test 97 lib green). P3e-b session
persistence (4324272, orchestrator self-review mirroring ui-settings). P3e-c IPC mirror + multi-repo
mock (6324d06, reviewer APPROVE — INTERMEDIATE red full-tree build: ~31 App.tsx errors by design,
src/ipc/* tsc-clean, goes green at P3e-e). Now: P3e-d GraphCanvas active prop + zero-size guard +
remeasure-on-show — DONE (8dce359, orchestrator self-review; zero new graph tsc errors).
P3e-e frontend refactor — DONE (dc0d827, reviewer APPROVE; RepoWorkspace + TabStrip + slim App +
ToastContext + reopen-all; 3 review fixes folded: selection-preserve-by-OID, Ctrl+W typing guard,
back-compat session persist). Full-tree pnpm build GREEN.
ALL 5 SUB-INCREMENTS COMMITTED: be49130 / 4324272 / 6324d06 / 8dce359 / dc0d827.
P3e AI GATE PASSED (2026-07-29). Tester added isolation_in_progress_merge_does_not_leak (drives repo A
into a real paused merge, asserts B's op-state/branches/status/graph untouched — closes reviewer NIT) +
isolation_close_preserves_other_repos_in_progress_op. Full suite: cargo test 251 passed / 0 failed /
2 ignored (perf gates) incl. commands lib 101, merge_cli 17, rebase_cli 18, conflict_cli 9; clippy
-D warnings EXIT 0; pnpm build GREEN. Harness multi-tab §9.2 fully verified: reopen-all rehydrates 3
tabs from persisted session (no console errors); all tabs mounted inactive display:none (hostCount=3);
tab independence (repo-merge merge-banner vs repo-rebase rebase-banner step 2/3, hidden tab retains
state); TabStrip folder-name pills + close + .tab-active + `+` recents/Browse/dedupe; per-repo
workspace-toolbar; selection-preserve-by-OID across switch (commit stays selected + panel); scroll
preserved across display:none (afterShow=200); close active tab -> right neighbor activates + host
unmounts + session persisted; bogus-path skip -> skipped, no crash, session pruned, valid tabs open,
toast system verified functional (sticky error toast renders). AWAITING USER CHECKPOINT §9.3.
Sub-increments: be49130 / 4324272 / 6324d06 / 8dce359 / dc0d827 + tests commit.
USER-CHECKPOINT DEBT carried (native pnpm tauri dev): P3a diff overlay, P3b tree grouping, P3c merge,
P3d rebase, P3e multi-repo tabs.
P3d AI gate PASSED + committed (56a43f7/f902ce0/c39af2a/95141ba); awaiting USER CHECKPOINT (debt below).

--- P3d history (complete) ---
User said "proceed to P3d" (2026-07-29) without confirming the P3c
USER CHECKPOINT; proceeding with development per that instruction. Contract written + orchestrator-reviewed:
docs/contracts/P3d-rebase.md (plain non-interactive rebase; reuses P3c opstate.rs/conflict.rs/OpBanner
verbatim; RepoOpState::Rebase wire type unchanged; on-disk state re-opened per call, no cleanup_state
mid-rebase; §11.3 merge-linearization accepted as the locked "plain non-interactive" default). Sub-increments:
P3d-a backend (git/rebase.rs + 4 commands) — DONE, reviewer verdict resolved (sole MUST-FIX was a
let_and_return false-positive; clippy -D warnings verified clean exit 0, cargo check clean, 3 unit tests
pass; map_conflict judgment call approved as-is). P3d-b IPC mirror + ?op=rebase mock — DONE (orchestrator
self-reviewed the mechanical §7 mirror; pnpm build green). P3d-c frontend (actionable OpBanner rebase mode
+ App handlers/generalized Abort dialog + Sidebar ⤵) — DONE, reviewer APPROVE (no must-fix/should-fix;
pnpm build green). ?op=rebase HARNESS FULLY VERIFIED (OpBanner step n/m, Continue/Skip/Abort gating, marker view,
resolve→continue completes + HEAD advances, skip w/ conflict, Abort dialog rebase copy, sidebar ⤵
clean rebase, plain regression; no console errors).
Tester wrote rebase_cli.rs: 17 pass / 1 ignored, and found (1) CONFIRMED BUG: rebase_skip first-op
corrupts .git/rebase-merge (repo.reset Hard) + returns empty branch name — senior-dev FIXING NOW
(un-ignoring skip_first_op test); (2) contract divergence: rebase requires clean worktree (unstaged
NOT allowed, unlike merge) — CONTRACT AMENDED §3.1.5/§9.7/§11.11, Bonsai already matches CLI (no code
change). Skip bug FIXED (senior-dev: lightweight index read_tree(HEAD) + force checkout_index instead
of repo.reset Hard; first-op + later-op skip both match git twin; branch name correct). AI GATE PASSED:
cargo test 242 pass / 0 fail (2 perf ignored) incl. rebase_cli 18 + merge_cli 17 + conflict_cli 9, lib 92;
clippy -D warnings clean; pnpm build green; ?op=rebase harness fully verified. AWAITING USER CHECKPOINT
for P3d (docs/contracts/P3d §10). Sub-increments: 56a43f7 backend, f902ce0 IPC/mock, c39af2a frontend,
+ final commit (tester rebase_cli 18-case suite + skip fix + contract amendments).
USER-CHECKPOINT DEBT (not self-declared — user must confirm in native `pnpm tauri dev`):
  - P3c merge/conflicts — AI gate PASSED 9e187e7 (reviewer APPROVE; cargo test 220 incl. merge_cli 17 +
    conflict_cli 9; clippy clean; pnpm build clean; harness ?op=merge fully verified). Checkpoint items:
    docs/contracts/P3c §10. Sub-increments: 51487db backend, e367c3d IPC/mock, bc13d90 frontend, 9d72ef1 tests.
  - P3a diff overlay — 65aa4d2/dfc2b3b.
  - P3b tree grouping — 64c0358/8dde13e/a96446e.

Plan approved by user 2026-07-28 (see ~/.claude/plans/for-this-bonsai-git-moonlit-metcalfe.md).
Sequencing locked (UI wins first): P3a diff overlay → P3b tree grouping → P3c merge+conflicts →
P3d rebase → P3e multi-repo tabs. Locked decisions: rebase = plain non-interactive only;
conflicts = file-level (Take ours/theirs/Mark resolved + read-only marker view); diff overlay =
unified only; tabs last (repoId threading over final command set).

- **P3a — Diff overlay in center pane** (frontend only): diffSlot machinery stays; DiffSlotView
  renders as full-pane overlay over `.graph-pane` (header w/ path+badge+close, Esc closes);
  StatusPanel/CommitPanel drop inline `<li class="diff-expansion">`; remove 45vh diff-scroll cap.
  AI gate: harness screenshots (workdir + commit diff overlay, Esc/close/toggle-off), pnpm build.
  USER CHECKPOINT: overlay feels right in native app.
- **P3b — Tree-grouped sidebar & status lists** (frontend only): pathTree.ts builder + Tree.tsx
  recursive renderer (render-prop leaves); applied to Sidebar branches/remotes/tags +
  StatusPanel/CommitPanel file lists; tree/flat toggle persisted via additive listView ui-setting.
  AI gate: harness tree screenshots, actions still work from leaves, toggle persists.
- **P3c — Merge + conflicts** (backend+frontend): git/opstate.rs (RepoOpState + get_op_state),
  git/conflict.rs (list/get/resolve — shared with rebase), git/merge.rs (merge_branch analysis
  UpToDate/FF/Merged/Conflicts, commit_merge 2-parent, abort_merge); OpBanner.tsx; actionable
  Conflicts section routed through center overlay (`conflict:<path>`). AI gate: merge_cli oracle
  tests + harness ?op=merge round-trip.
- **P3d — Rebase** (backend+frontend): git/rebase.rs start/continue/skip/abort, on-disk state
  re-opened per call (never inmemory; no cleanup_state mid-rebase); OpBanner rebase mode
  step n/m + Continue/Skip/Abort. AI gate: rebase_cli oracle tests + harness ?op=rebase.
- **P3e — Multi-repo tabs** (backend state + IPC + frontend refactor): AppState keyed
  HashMap<repoId, RepoEntry{path,watcher}>; all repo-scoped commands gain repoId; close_repo;
  repo-changed payload gains repoId; RepoWorkspace.tsx owns per-repo state cluster (all tabs
  mounted, inactive display:none, GraphCanvas zero-size guard + re-measure on show); TabStrip.tsx
  replaces RepoSwitcher; openRepos/activeRepo persisted, reopen-all on launch. AI gate: two-repo
  isolation tests + harness multi-tab flows.

Rules: scratch repos under D:\Temp\bonsai-scratch only; TMP/TEMP=D:\Temp for cargo tests;
orchestrator makes all commits (`wip(P3x): …`); mock.ts kept compiling with every IPC change.

## P2 — Post-v1 follow-ups — **done** (2026-07-28)

AI gate passed and USER CHECKPOINT confirmed by user: pane resizing/persistence, theme
toggle/persistence, extended keyboard nav, new app icon all pass in the native app.
Contract: docs/contracts/P2-followups.md.
Open item for the user (no code work pending): code-signing decision per
docs/code-signing.md — needs a user-provided certificate.
Sub-increments (all reviewer-APPROVE): P2a pane resizing + ui_settings commands; P2b light
theme (data-theme + themeVersion repaint); P2c keyboard nav (GraphCanvasHandle);
P2d generated bonsai icon + tauri icon set + favicon + docs/code-signing.md (docs-only;
signing needs a user cert decision). Orchestrator-found+fixed bug: keyboard-nudge pane
persist read a stale ref (commit now reads ref at debounce fire time).
AI-gate evidence: cargo test 73 lib + all integration suites green, clippy -D warnings
clean, pnpm build clean; harness verified — dividers render + nudge clamps to graph floor
+ persists to uiSettings; theme toggle flips data-theme/glyph, lane colors invariant,
survives reload; End/Home/PageDown move selection + follow-scroll, no-ops with no
selection; icon set regenerated (icon.ico 6 sizes), favicon wired.
Scope (user-approved 2026-07-28):
- Pane resizing (3-pane layout: draggable sidebar/right-panel dividers, persisted widths)
- Light-theme toggle (dark stays default; theme tokens already CSS variables; persist choice)
- Extended graph keyboard navigation (deferred from P1: PageUp/PageDown/Home/End, Enter
  semantics per architect)
- App icon + branding (replace default Tauri icons; window/taskbar/installer icons)
- Code signing for installers — investigate/document only; needs a user-provided
  certificate, cannot be completed autonomously.
Same workflow loop: architect contract → senior-dev sub-increments → reviewer → orchestrator
commits → tester → AI gate → USER CHECKPOINT.
P1a 185628d, P1b 348d751, P1c 7100cd3, P1d a3eb70d (all reviewer-APPROVE).
AI-gate evidence: full cargo suite exit 0; pnpm build green; harness verified — toast
success (M6 string byte-identical) + sticky error toast w/ manual dismiss; ? overlay
open/Esc close; shortcuts inert while RepoSwitcher open; WIP row offset live (spacer
(N+1)*28+8); recents in localStorage + dropdown; 20k scroll WITH WIP row: 240 rAF frames
avg 16.5ms, max 17.6ms, zero frames >33ms.
Architect findings: reopen-last-repo was never built (designed now); no repo-switch UI
existed (RepoSwitcher added). WIP row = frontend-composited +1 render offset (orchestrator
accepted §12.1: no lane/edge math in TS, GraphLayout unchanged).

Goal (CLAUDE.md): keyboard shortcuts, error toasts, empty/loading states, GitButler-clean
styling. Accumulated backlog:
- WIP (uncommitted changes) row at top of graph (deferred from MVP per product decisions)
- Recent-repos list + reopen last repo polish (deferred from MVP)
- Keep old diff visible during same-key refetch (skeleton flash on focus/watcher tick)
- React.memo(DiffView) for 5000-row diffs
- CommitPanel messageBody unconditional first-line strip
- Commit textarea disabled during stage-in-flight (Windows focus drop)
- Dismissed-error string-compare pattern; shared isAppError util (3 copies)
- Refresh-failure path alignment in App.tsx; frame-log stream tagging
- Mock: prepend synthetic graph row on commit (TODO(polish) in mock.ts)
Acceptance (AI gate): full suite stays green; harness verification of new UI states.
Acceptance (USER CHECKPOINT): shortcuts, toasts, empty states feel right in the native app.
