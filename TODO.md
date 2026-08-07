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

## P54 — commit composer: WIP → N logical commits (Phase 2 · milestone 2/5) — **IN PROGRESS** (2026-08-07)

Phase 2 milestone 2. User chose "Continue to P54" after P53. Contract: `docs/contracts/P54-commit-composer.md`
(+ overview). OD1 remains local-`claude`-CLI-only.

**P54 goal:** propose grouping the working tree into N logical commits (FILE-LEVEL v1), each with a
generated message; user reviews/edits/reassigns/drops/merges; apply as an ORDERED, ATOMIC stage+commit
sequence. Two commands: `ai_compose_commits` (PROPOSE — AI, consent-gated, WRITES NOTHING; result is
ALWAYS an apply-able partition regardless of model output — Rust is the referee) + `apply_composed_commits`
(APPLY — pure git, NOT AI-gated; validate-whole-plan → reset index to HEAD → commit each group → roll
HEAD+index back on ANY failure; WORKDIR NEVER TOUCHED, no data-loss risk). Cmd 131→133.

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 apply auto-resets index to HEAD (workdir
untouched; UI states it) · OQ2 FILE-LEVEL v1 (line-level split deferred to a future P54d/P66) · OQ3 ship
optional `guidance` hint · OQ4 reuse P53's already-promoted `gather_worktree` + `cap_review_payload`
(both pub(crate) — NO new visibility change) · OQ5 keep empty-message groups (UI/apply validate) · OQ6
single apply command, no progress channel.

Sub-increments: **P54a** propose backend (`ai_compose.rs`: `parse_compose_response` referee → guaranteed
apply-able partition; grounding) + `ai_compose_commits` cmd + IPC + mock → **P54b** apply engine
(`compose_apply.rs`: atomic reset / commit-loop / rollback) + `apply_composed_commits` cmd + IPC + mock →
**P54c** review UI (`useCommitComposer` + `ComposerDialog` + `ComposerGroupCard`; CommitPanel entry;
Esc-layer). a→b→c (b reuses `ComposeGroup` from a; c needs both).

- **P54a** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — new `ai_compose.rs`: `ComposeGroup`/
  `ComposeProposal`/`MAX_COMPOSE_GROUPS=10`; `compose_commits` (clean tree → `NothingToCommit` before any
  CLI; CLI hard-fail → `AiFailed`; reuses P53's pub(crate) `gather_worktree` + `cap_review_payload`, no
  dup); WHY-not-WHAT grounding (§3.2). The pure REFEREE `parse_compose_response` — reviewer PROVED the
  partition invariant (groups∪unassigned==changed, disjoint, order-preserving, no phantom) across ALL
  branches (unparseable→all-unassigned, overlap first-wins, unknown dropped, empty dropped, cap→tail
  unassigned; `extract_json` opens-first handles the bare-array fallback). `ai_compose_commits` cmd +
  `_inner` (consent gate before repo_path, read-only; generate_handler! 131→132) + consent-gate test. IPC
  + mock (`?ai=off`→aiFailed, clean→nothingToCommit). 8/8 lib + 1/1 integration
  (`tests/ai_compose_cli.rs` stub-echo, sibling isolation pattern); clippy -D + build/tsc clean. Nit
  (accepted): ai_compose.rs 616 lines (prod ~309 + mandated inline §8 suite needing private access; under
  the ai_explain 1178 precedent). `ComposeProposal` = PartialEq only (Option<f64>).
**Current step:** P54a DONE (committed). Next: **P54b** (apply engine — `compose_apply.rs` atomic
reset/commit-loop/rollback + `apply_composed_commits` cmd →133 + `compose.ts` mock).

## P53 — AI "why" layer: blame-why + explain-commit + branch naming (Phase 2 · milestone 1/5) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-07)

Phase 2 first milestone. User greenlit implementation (main clear) + chose **P53 first** + **OD1 =
local-`claude`-CLI-only, model-tier trait DEFERRED**. Contract: `docs/contracts/P53-ai-why-layer.md`
(+ `phase2-ai-native-overview.md`). Standard architect→senior-dev→reviewer→commit→tester loop.

**P53 goal:** three read-only/low-risk AI features that establish the Phase-2 grounding plumbing P54–P57
reuse — (a) blame "why did this line change" (new `ai_explain_line`, line-focused grounding via
single-line blame), (b) "Explain this commit" from a graph node (reuse `ai_analyze_diff` + enrich the
Commit grounding with the commit MESSAGE — WHY-not-WHAT), (c) AI branch naming (new
`ai_suggest_branch_name`, returns sanitized kebab-case candidates, WRITES NOTHING). Cmd 129→131.

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 self-contained `ai_explain_line` ·
OQ2 sources Working+CommitRange (drop Staged) · OQ3 branch-name model = default (sonnet, local CLI) ·
OQ4 ask 3 / cap 5 · OQ5 accept D2 commit-MESSAGE grounding enrichment as an improvement (does change
existing commit-explain/review output — intended, not a regression) · OQ6 disable "Suggest name" on a
clean worktree · OQ7 defer rename-follow in blame-why (`orig_path=None`) · OQ8 promote `epoch_to_ymd`
to a shared helper.

Sub-increments (order-independent; b smallest): **P53a** blame-why (`blame_line` + `ai_line.rs` +
`ai_explain_line` cmd + IPC/mock + BlameView "Why?") → **P53b** explain-commit (`ai_explain` MESSAGE
enrichment + graph-node menu entry, NO new cmd) → **P53c** branch naming (`ai_branch_name.rs` +
`ai_suggest_branch_name` cmd + `BranchNameSuggest.tsx` + branch-create dialog).

- **P53a** (reviewer APPROVE, 0 must-fix / 0 should-fix; 4 cosmetic nits noted) — `blame.rs::blame_line`
  (single-line blame, `BlameOptions` min/max_line, out-of-range→`Git`); new `ai_line.rs` `explain_line` +
  `render_line_payload` (§3.4 template) + line-why consts; new `timefmt.rs` (`epoch_to_ymd` promoted, OQ8,
  no dup); `cap_review_payload`→pub(crate) (build_payload untouched — D2 stays P53b). `ai_explain_line`
  cmd + `_inner` (consent gate before repo_path, read-only, no repo-changed; generate_handler! 129→130).
  IPC `aiExplainLine` + mock (`?ai=off`→aiFailed); BlameView per-block "Why?" (gated `aiEligible`) →
  RepoWorkspace `runExplainLine` (aiPanelReqId last-wins, atOid=null v1). cargo -p bonsai-core 439 pass;
  clippy -D warnings + pnpm build/tsc clean. Nits (noted, not folded): 3 repo-opens/call (behind CLI,
  negligible); blame.rs doc says Git for a traversing path (actually Other, test correct); mock delay
  before requireRepo; blame.rs now 519 lines (split on next blame addition).
- **P53b** (reviewer APPROVE, 0 must-fix / 0 should-fix; 2 nits) — D2 grounding enrichment:
  `build_payload` Commit arm appends `MESSAGE:\n<full commit message>` (reuses `CommitDetails.message` —
  full body, trailing-trimmed) after COMMIT/AUTHOR, before the file blocks — Commit arm ONLY (intentionally
  improves shipped commit explain/review per OQ5). "Explain this commit" added to the shared
  `commitActionItems` (commit rows + branch/tag pills), read-only group (after Compare-with-HEAD), gated
  `!aiEligible` → `runAnalyze({kind:'commit'},'explain')`. No new command (130), no mock change. cargo -p
  bonsai-core ai_explain 19 pass; clippy -D + build/tsc clean. Nit: summary appears in both the COMMIT
  header and the MESSAGE first line (harmless redundancy).
- **P53c** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — new `ai_branch_name.rs`
  (`suggest_branch_name`, pure `sanitize_branch_name` — output strictly `[a-z0-9/-]`, `.` never emitted
  so `..`/`.lock`/dot hazards are structurally impossible, never an uncreatable ref; `Working` reuses
  `gather_worktree`, `CommitRange` mirrors `summarize_range`; empty grounding → `AiFailed` BEFORE any CLI
  spawn; parse→sanitize→stable-dedup→cap 5). `ai_suggest_branch_name` cmd + `_inner` (consent gate,
  read-only; generate_handler! 130→131). IPC + mock (`?ai=off`→aiFailed); new `BranchNameSuggest.tsx`
  (button gated `aiEligible && workingDirty`, last-wins guard, WRITES NOTHING) in the create-branch
  dialog via a new optional `PromptDialog.extraContent` slot (additive; Worktree/Remote callers
  untouched). `BranchNameProposal` = PartialEq only (Option<f64>; matches AiAnalysis). cargo -p
  bonsai-core ai_branch_name 6 pass; clippy -D + build/tsc clean. Nits→tester: symmetric CommitRange
  empty-range pre-CLI test; `feat/-x` post-slash-dash doc/test; list-number `1-` prefix leak (cosmetic).
- **P53 tester** — full regression: `cargo test --workspace` **945 passed / 0 failed / 3 ignored** (the
  3 ignored = intentional perf gates); vitest **74/74**; clippy -D clean. Added 2 tests (both pass):
  `suggest_branch_name_range_empty_fails_before_cli` (symmetric CommitRange empty-range → `AiFailed`
  before any CLI spawn) + a post-slash-dash sanitize lock (`feat/-fix` stays valid). Wrote
  `docs/contracts/P53-user-checklist.md`. No bugs found.
- **P53 AI GATE PASSED (2026-08-07)** — backend: 945 workspace tests + reviewer file:line verification of
  all 3 entry points. Harness (mock :1420): app loads clean (no server/console errors, full workspace +
  P53 code integrated); the `aiEligible` gate chain verified END-TO-END — seeding `aiConsented` flipped
  the AI affordances (commit Generate / Review staged+all / AI digest) disabled→enabled, proving
  `aiEnabled && aiConsented && installed` propagates to the gate that controls ALL 3 P53 entry points.
  The 3 entry points are CANVAS/overlay-driven (blame "Why?" overlay · graph-node "Explain this commit"
  menu · "Create branch here"→Suggest dialog) → headless pane can't composite/right-click → live visuals
  are USER CHECKPOINT (per `docs/contracts/P53-user-checklist.md`).
**Current step:** P53 DONE (AI gate passed to harness limit; awaiting USER CHECKPOINT). Commits 94fd9b3
(a) · 4371141 (b) · 33c1783 (c) · tester-closeout next. Next milestone: **P54 commit composer** — needs
user greenlight to start.

## 🗂️ PHASE 2 & 3 — CONTRACTS PREPARED (design only; NO code yet) (2026-08-07)

Prepared autonomously while another session implemented an unrelated task on branch `feature/next-steps`.
Per user: "prepare Phase 2 and 3 without changing any code." **11 architect contracts written to
`docs/contracts/`; zero application code touched.** Implementation has NOT started. The standard loop
resumes at step 3 (decompose → senior-dev) per milestone once the user greenlights.
Roadmap: `~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`.

### ⛔ BLOCKING open decision before ANY Phase-2 build (OD1)
Confirm Phase 2 stays **local-`claude`-CLI-only**, with the model-tier seam (BYO-key / hosted /
local-model `AiBackend` trait) DOCUMENTED but DEFERRED. Privacy is both gate and differentiator. Every
Phase-2 contract (P53–P57) depends on this. Phase 3 (P58–P61) has NO such dependency and could start
independently (even before Phase 2, if preferred).

### Phase 2 — AI-native edge (contracts ready, unbuilt)
- **`phase2-ai-native-overview.md`** — shared-conventions anchor: grounding payload on `ai/payload.rs`;
  generate→review→accept/edit in `AiOutputPanel`; `ai_*` IPC request/response triple + camelCase +
  `?ai=off` mock parity; local-first; model-tier seam = OD1.
- **P53 `P53-ai-why-layer.md`** — (a) per-line blame "why", (b) "Explain this commit" from a graph node,
  (c) AI branch naming. Read-only/low-risk; builds the grounding plumbing P54–P57 reuse. Sub: P53a
  blame-why (`ai_line.rs` + `ai_explain_line`) · P53b explain-commit (menu + MESSAGE enrichment, no new
  cmd) · P53c branch naming (`ai_branch_name.rs`). +2 cmds.
- **P54 `P54-commit-composer.md`** — WIP → N logical commits (file-level v1). `ai_compose_commits`
  (AI propose) + `apply_composed_commits` (pure git, atomic all-or-nothing, HEAD/index rollback anchor,
  workdir never touched). Sub: P54a propose · P54b apply-engine · P54c review UI. +2 cmds. Reuses P53's
  `gather_worktree`/`cap_review_payload` (promote to pub(crate)).
- **P55 `P55-nl-to-safe-git-op.md`** — NL → ONE allowlisted typed op → read-only preview → explicit
  confirm → execute via existing tested command. 7-layer structural safety (closed `AiOpIntent` enum,
  fail-closed parse, Rust owns all ref/oid resolution, AI path never mutates, no raw shell string). v1
  allowlist: undo-last-commit/merge, reset, revert, switch/create/delete branch, stash, discard, merge.
  Sub: P55a safety core + reset/revert · P55b rest of allowlist · P55c UI. +1 cmd.
- **P56 `P56-local-changelog.md`** — tag/ref range → grouped Markdown release notes, fully local;
  reuses `ai_summary` range resolver (promote `resolve_digest_range`) + new `resolve_last_tag`. Sub:
  P56a core · P56b UI (tag-pill entry). +1 cmd.
- **P57 `P57-semantic-history-search.md`** — NL Q&A grounded in real diffs. 3-stage: persisted
  per-commit doc index (app_data_dir, NOT `.git`) w/ progress channel → **BM25 retrieval (recommended
  v1; embeddings DEFERRED — Defender ASR blocks model downloads on this box; ties to OD1)** → local
  `claude` synthesis in `AiOutputPanel`. Complements P50 (does NOT touch `search.rs`). Sub: P57a
  index+channel · P57b retrieval · P57c synthesis+UI. +4 cmds.

### Phase 3 — Correctness & parity (contracts ready, unbuilt; NO OD1 dependency)
- **P58 `P58-commit-signing.md`** — SSH-first + GPG signing at commit (via `git commit-tree -S` +
  `git update-ref`; unsigned path byte-identical) + verification (`git log --format=%G?…`) that lights
  the P51 badge stub + a commit-panel signature line. Introduces shared `exec.rs` git shell-out seam.
  Sub: P58a sign · P58b verify · P58c frontend. +2 cmds. NATIVE USER CHECKPOINT (real key material).
- **P59 `P59-hooks-and-lease-hardening.md`** — (a) run pre-commit/commit-msg/post-commit (+pre-push)
  hooks via `git hook run` (**requires Git ≥2.36**), per-repo `bonsai.runHooks` opt-out, new
  `AppError::HookRejected` carries hook output (never silent). (b) force-push →
  `git push --force-with-lease=<ref>:<expected> --force-if-includes` (atomic lease, closes P37's
  TOCTOU). Sub: P59a hooks · P59a-2 pre-push · P59b lease (independent). Shares `exec.rs` with P58.
- **P60 `P60-parity-batch.md`** — 4 independent items: P60a branch rename (git2 `Branch::rename`) ·
  P60b non-FF pull (offer merge/rebase, confirm-gated, reuse existing cmds — no new git logic) · P60c
  one-click undo (read-only `describe_last_undo` → reflog classify → reuse `reset_branch`) · P60d
  submodule add/deinit/remove. +5 cmds.
- **P61 `P61-diff-quality.md`** — P61a intraline/word diff (backend LCS pass, `intraline:bool` param on
  the 3 hunk cmds, off = byte-identical wire) · P61b image diff (base64-over-IPC — chosen for mock/
  harness parity; 8 MiB cap; new `DiffImageView.tsx` side-by-side/onion/swipe). May add `base64` crate.
  +1 cmd.

### Integration notes for whoever implements (carry into each senior-dev prompt)
- **Command-count renumbering:** every "+N cmds" is RELATIVE; the absolute `generate_handler!` tail
  depends on LANDING ORDER — senior-dev must recount against `src-tauri/src/lib.rs` at each increment
  (architects deliberately left absolute numbers open).
- **Shared primitives — introduce once, then reuse:** `exec.rs` git shell-out by whichever of P58/P59
  lands first; P53's `gather_worktree`/`cap_review_payload` promoted for P54; `resolve_digest_range`
  promoted for P56; a shared `git/refs.rs` ref-seeder + `app_data_root(app)` helper suggested by P57.
- **User sign-offs still needed (beyond OD1):** P55 `undoLastMerge` = reset-to-first-parent (destructive,
  rewrites history; recommended, with shared-history warning) vs `revert -m 1` (safe, no rewrite);
  P57 BM25-vs-embeddings; P61 adding the `base64` crate.
- **Recommended build order:** confirm OD1 → P53 (foundation) → P54 / P55+P56 → P57 (last, highest cost).
  Phase 3 is independent of OD1: P58+P59 (share `exec.rs`) → P60/P61 in any order.
- **STATUS: contracts committed, uncommitted-code guardrail respected** — only the 10 new
  `docs/contracts/*.md` + this TODO were staged; the 4 pre-existing dirty files (Cargo.lock,
  package.json, src-tauri/Cargo.toml, tauri.conf.json) were left untouched.

**Current step:** IMPLEMENTATION STARTED — user chose P53-first + OD1 = local-`claude`-CLI-only (model
tiers deferred). See the "## P53 …" section at the TOP of this file for live status. Phase 1 (P49–P52)
native USER CHECKPOINTs still pending; the P55 undo-merge + P61 `base64`-crate sign-offs are still
needed when those milestones are reached.

## P52 — adopt git's commit-graph file (Phase 1 · large-repo perf) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-07)

Phase 1 milestone 4 of 4 (final). Roadmap: `~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`.

**P52 goal:** write/refresh git's `commit-graph` file so libgit2 skips re-parsing commits —
accelerating the graph-layout revwalk AND blame/file-history AND the repo-health branches scan. Also
the intended fix for the failing `health::tests::perf_ceiling_on_20k_fixture` gate (task_e253301f).

**Approach (plan):** on repo open + after fetch, shell out to `git commit-graph write --reachable
--changed-paths` (best-effort; the app already shells `git` in `scheduler.rs`; skip cleanly if git
absent), behind a setting. Ensure libgit2 (git2 0.21) actually reads the commit-graph
(`core.commitGraph`). Re-run the perf gate to quantify; write the commit-graph in the perf fixtures so
the gates reflect it.

**Key questions for the architect:** trigger points (`open_repo` + fetch/scheduler) + setting default;
does git2 0.21 auto-use the commit-graph or need `core.commitGraph=true`? `--changed-paths` Bloom
filters for file-history/pathspec; how to update `perf_gate.rs` + the health 20k perf fixture to write
+ benefit from the commit-graph (and get the health gate GREEN); best-effort/no-git fallback.

**Contract:** `docs/contracts/P52-commit-graph.md` + `P52-user-checklist.md` (architect — DONE).
KEY FINDING: libgit2 (v1.8.1, vendored by git2 0.21) uses the commit-graph **automatically +
unconditionally** — NO `core.commitGraph` needed; `graph.rs` revwalk + `health.rs` merge-base consume
it with zero code change. Bonsai only WRITES the file. Backend-only (no IPC/TS/mock). File lands at
`.git/objects/info/commit-graph` (already watcher-filtered → no spurious repo-changed).
**Orchestrator OQ decisions:** accept ALL recs — no `core.commitGraph` write; **always-on, no toggle**;
plain `--reachable` (not `--split`); keep `--changed-paths`; triggers gated fetch/autoFetch on
updated-count, pull unconditional. Sub-increments: **P52a** `maintenance.rs` + 3 triggers + unit/CLI
tests (load-bearing: `compute_graph` output identical before/after) → **P52b** fixture/perf-gate/
health-gate wiring (report per-section timings; do NOT raise budgets silently).
- **P52a** (reviewer APPROVE, 0 must-fix/should-fix; 2 cosmetic nits) — `git/maintenance.rs`
  (`CommitGraphOutcome` Written/Skipped, never Err; `commit_graph_args`; `write_commit_graph` +
  best_effort, reusing search `GitRunner`/`SpawnGitRunner`). 4 fire-and-forget un-awaited triggers:
  open_repo, fetch (gated `updated_refs>0`), pull (unconditional), scheduler autoFetch (`updated>0`) —
  each returns the command result independently; never blocks/errors. NO graph.rs/health.rs/IPC/TS/mock
  change, no `core.commitGraph`, no new repo-changed. 6 tests incl. load-bearing `compute_graph` +
  health-branches IDENTICAL before/after + Skipped-on-no-git. clippy --all-targets + build clean.
- **P52b** (reviewer APPROVE, 0 must-fix/should-fix; 1 harmless nit) — fixture `ensure_commit_graph`
  (existence + have_git guarded, at-most-once, on both cache paths) writes the graph so perf tests
  measure the realistic state; health perf test gets best-of-3 per-section reporting + graph-presence
  assert + `#[ignore]` (matching the existing `perf_gate.rs` gates); **budgets UNCHANGED (1500/2000)**.
  **Commit-graph perf win (20k health, isolated):** total ~8100ms → **~1600ms** (branches 6000→~1280,
  stats 1558→~300) — under budget; layout `compute_graph` 254.9ms<500 with the graph. `#[ignore]` is
  for parallel-suite CPU-contention flakiness (convention), NOT a budget dodge. No prod-logic change
  (only `#[cfg(test)]` + test-only fixture; `ensure_default_fixture` has no runtime caller).
- **P52 tester** — final Phase-1 regression: `cargo test --workspace` **932 passed / 0 failed / 3
  ignored** (DEFAULT SUITE GREEN for the first time; the 3 ignored are the intentional perf gates);
  vitest **74/74**; maintenance 6/6; explicit perf gate PASSES (debug total 1784ms<2000, stats
  404<1500 — commit-graph win holds); clippy --all-targets + tsc/build clean; checklist complete.
  No new tests needed.
- **P52 AI GATE PASSED (2026-08-07).** Commits: `4b0583a` P52a · `0a77165` P52b. Backend-only; the
  tracked health-perf regression is fixed (~5×) + `#[ignore]`d per perf-gate convention; zero regressions.

**P52 awaiting USER CHECKPOINT** (native, per `docs/contracts/P52-user-checklist.md`): open a large
real repo → `.git/objects/info/commit-graph` appears (and mtime refreshes after fetch/autoFetch);
graph/blame/health feel faster; NO error when `git` is absent.

---
## ✅ PHASE 1 (P49–P52) COMPLETE — all AI-gate-passed, awaiting a BATCH of native USER CHECKPOINTs (2026-08-07)
Delivered autonomously (user away 3h): **P49** open-in-terminal/file-manager/editor · **P50** commit/
content search + Ctrl/Cmd-K command palette + sidebar list filtering · **P51** graph polish (SHA/date/
author columns, author-vs-committer, compact, ahead/behind chip — all toggleable) · **P52** commit-graph
file adoption (5× faster health scan; faster graph/blame). Native checklists:
`docs/contracts/P{49,50,51,52}-user-checklist.md`. The four pre-existing dirty files (Cargo.lock,
package.json, src-tauri/Cargo.toml, tauri.conf.json) were left untouched throughout.
NOTE: the earlier-spawned "fix health perf gate" task chip was already started by the user in a
separate worktree — P52 already fixes it on `main`, so that session is redundant (discard, don't merge).
Next phase (NOT started — needs user OK): **Phase 2 (AI-native edge)** P53 blame/explain "why" +
branch naming → P54 commit composer → P55 NL→safe git op → P56 local changelog → P57 semantic history search.
**Current step:** Phase 1 DONE + **Phase 2/3 contracts now PREPARED** (design-only; see the "🗂️ PHASE 2
& 3 — CONTRACTS PREPARED" section at the top of this file). PAUSED — implementation NOT started.
Do NOT start Phase 2 without user approval + confirming OD1 (their grant was "finish Phase 1" then
"prepare Phase 2 and 3 without changing code" only).

## P51 — commit-graph polish + clutter controls (Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-07)

Phase 1 milestone 3 of 4. User chose "Finish Phase 1 (P51+P52)" + granted a 3h autonomous window.
Roadmap: `~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`.

**P51 goal:** add the commit-graph row details users want — every one individually TOGGLEABLE, plus a
compact mode (worldwide #1 graph complaint = clutter). Add (sensible defaults; toggles in Settings →
Graph):
- short SHA per row; absolute date on hover (relative stays inline); ahead/behind on branch-tip rows
  (reuse `BranchInfo` ahead/behind in `types.ts`); author-vs-committer date choice; a **verified/
  signed badge slot** (stub now, lit by P58 signing); compact mode (denser rows); column show/hide
  (+ reorder if cheap). Remove the dead `dotRadius` graph pref (`SettingsPanel.tsx`).

**Current graph draw (Explore): SHOWN** = ref pills (branch/remote/tag/HEAD/stash + overflow),
initials avatar, lane color, HEAD/selection rings, stash node, summary (first line), relative date,
edges, WIP row, hover tooltips (`src/graph/draw.ts:683-825`). **NOT shown** = per-row SHA, verified
badge, absolute date, ahead/behind on rows, committer. Geometry knobs exist in UiSettings
(commitNodeSize/rowHeight/laneWidth); `dotRadius` is dead.

**Key questions for the architect:** author-vs-committer → does `GraphNode` need a backend
`committerTs` add (+ graph.rs + mock) or is it frontend-only? Column reorder scope (show/hide vs
drag). Toggle persistence (extend UiSettings graph fields). Canvas draw stays additive + virtualization
intact; clutter toggles must actually hide elements.

**Contract:** `docs/contracts/P51-graph-polish.md` + `P51-user-checklist.md` (architect — DONE).
Toggles nested in existing `GraphPrefs` (serde `graph`, whole-struct patch → NO ui_settings/UiSettings
changes): `showSha`(T)/`showAuthor`(F)/`showDate`(T)/`dateBasis`(author|committer=author)/
`showAheadBehind`(T)/`compact`(F). Backend: ADD `GraphNode.committer_ts` (committer NAME not added).
Column model = new pure `rightColumns.ts` (author→SHA→date, right-packed, disabled reserves nothing);
compact = geometry preset in `effectiveMetrics`; verified-badge = faint unlit stub left of SHA (P58
lights it); ahead/behind = LEFT ref band on the local-branch pill from `BranchInfo`. Sub-increments:
**P51a** data model/settings/geometry/mock + committerTs + `dotRadius` removal (no visual change) →
**P51b** right-columns + SHA + badge stub + date basis + absolute-date hover + compact + SettingsPanel
(extract `refLabels.ts` + `drawRowText.ts` → draw.ts <500) → **P51c** ahead/behind chip.
**Orchestrator OQ decisions:** accept ALL architect recs — ahead/behind LEFT band; column reorder
DEFERRED (show/hide only); badge faint-unlit-glyph; `showSha` default TRUE; `committer_ts` add / name
defer; compact preset override; toggles nested in GraphPrefs.
- **P51a** (reviewer APPROVE, 0 must-fix/should-fix) — `GraphNode.committer_ts` + 6 `GraphPrefs`
  toggles (`GraphDateBasis` author|committer) nested in the whole-struct graph patch (ui_settings.rs
  UNCHANGED); compact geometry preset (inert while false); `dotRadius` removed end-to-end (back-compat
  tested). No visual change (draw.ts untouched). cargo 108 (src-tauri) + 427 (bonsai-core) pass;
  clippy --all-targets clean; tsc/build clean. **Independently re-confirmed** the health 20k perf gate
  is pre-existing (senior-dev stashed all changes → identical fail on clean tree; health.rs has no
  graph dep). P51b nit to fold: `diffs.ts:344` CommitDiff `committerTs` should read `node?.committerTs`.
- **P51b** (reviewer APPROVE, 0 must-fix/should-fix; 3 cosmetic nits) — right-column system
  (`rightColumns.ts`) + SHA/author/date columns + `dateBasis` + absolute-date hover + compact +
  `SettingsGraphSection` toggles; extracted `refLabels.ts`/`drawRowText.ts`/`dates.ts`/`textMeasure.ts`
  → **draw.ts 838→453** (<500), SettingsPanel 501→400. 16 new unit tests (57 total). diffs.ts nit
  folded (committerTs). **P51b AI GATE PASSED** (harness, mock :1420, zero console): Graph section shows
  Short SHA / Author name / Date / Date basis (Author·Committer) / Compact rows; toggling Author-name ON
  + Short-SHA OFF each repaint the canvas non-blank with no errors (column add + remove/reflow paths).
  Canvas visuals (SHA text/layout/compact density/hover) = USER CHECKPOINT (pane not screenshottable
  headless). Nit noted: GraphCanvas.tsx 944 lines (pre-existing) — future split candidate.
- **P51c** (reviewer APPROVE, 0 must-fix; 1 should-fix = test-coverage gap → tester; 2 cosmetic nits)
  — `branchStats` from `BranchInfo.local` threaded via `display`; `↑N ↓M` chip on diverged local-branch
  pills in the LEFT band (`refLabels.ts`; reserves `chipGap+chipWidth`, integrated with overflow `+n`
  pop-rewind — reviewer hand-verified EXACT, no off-by-one/overlap); gated by `showAheadBehind`
  (SettingsGraphSection toggle). textMeasure NUL→space tidy. 13 new tests (28 graph). **P51c AI GATE
  PASSED** (harness, mock :1420): repo loads clean with default toggle ON + diverged `feat` (↑1↓1) →
  chip draw path runs, canvas non-blank, zero console errors. Chip visual = USER CHECKPOINT. tsc/build clean.
- **P51 tester** — regression: cargo **926 passed / 1 failed** (ONLY the expected pre-existing health
  20k perf gate; nothing else regressed) / 2 ignored; vitest **74 passed** (5 new chip-overflow tests
  closing the reviewer gap: exact `+n`, no overlap, chip-bearing pop, toggle-off zero-width, budget
  sweep). clippy --all-targets + tsc/build clean. Added a crowded-row chip bullet to the checklist.
- **P51 AI GATE PASSED (2026-08-07).** Commits: `367a064` P51a · `03ba1e2` P51b · `49a54ce` P51c
  (+ tester closeout). Settings toggles + all new draw paths harness-verified (canvas non-blank, zero
  console errors); zero P51 regressions.

**P51 awaiting USER CHECKPOINT** (native `pnpm tauri dev`, per `docs/contracts/P51-user-checklist.md`):
canvas visuals the harness can't judge — SHA/date column legibility, author column, compact density,
author↔committer date basis, absolute-date hover, and the `↑a ↓b` chip on diverged tips incl.
crowded-row `+n` folding.
**Current step:** P51 DONE (AI gate passed, awaiting USER CHECKPOINT). Starting **P52** (adopt git's
commit-graph file — Phase-1 milestone 4/4; also targets the health 20k perf gate).

## P50 — commit/content search + command palette + list filtering (Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-07)

Phase 1 milestone 2 of 4 (roadmap `~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`).
Three keyboard-first discovery features, all ABSENT today (Sublime Merge's signature strength).

**P50 goal:**
- **Commit/content search** — find commits by message / author / path, and by CONTENT (pickaxe
  `-S` literal / `-G` regex). Matches surfaced in the EXISTING graph (highlight dots + jump next/prev),
  reusing the reveal-in-graph infra; `compute_graph` stays unfiltered/stable (search is a SEPARATE cmd).
- **Command palette** (Ctrl/Cmd-K) — fuzzy-launch existing actions + jump to branch/tag/commit +
  trigger search.
- **List filtering** — type-to-filter boxes on the sidebar Branches / Remotes / Tags lists (reuse
  `Combobox.tsx`).

**Key design decisions (architect settles in the contract):**
- Search perf: message/author/path via bounded git2 revwalk; CONTENT `-S`/`-G` likely shell-out to
  `git log` (optimized pickaxe — a git2 diff-per-commit walk janks large histories; app already shells
  `git`). Cap results + a truncated signal.
- `search_commits(query)` returns matching oids (+ minimal match metadata) — NOT a graph re-filter.
- Command-palette v1 scope (action registry + branch/commit jump + search entry); own increment or bundled.

**Acceptance criteria:** finds by message/author/path/content + jumps to matches in the graph; palette
opens on Ctrl/Cmd-K, filters + launches actions, routes to search/nav, Esc-layers with existing
overlays, never fires destructive ops without the existing confirms; sidebar lists filter as you type;
large-history search stays responsive (bounded/capped); tsc + build clean; cargo test + clippy clean;
mock in lockstep; CLI-oracle for search vs real `git log`/`git log -S/-G`.

**Contract:** `docs/contracts/P50-search-command-palette.md` + `P50-user-checklist.md` (architect —
DONE). Backend: message/author/`all` → git2 revwalk (header-only, scan cap 200k); path + content →
shell-out `git log` (`-S`/`-G`) via injected `GitRunner` (credential_fill idiom); result cap 1000
(cap+1 → `truncated`). Sub-increments: **P50a** backend+IPC+mock+oracle → **P50b** search UI +
graph highlight/jump → **P50c** command palette → **P50d** list filtering (b/c/d order-independent).
**Orchestrator decisions on flagged OQs:** OQ1 bind Ctrl/Cmd-F to open commit search + visible entry
+ palette fallback; OQ2 DEFER message/author regex + dedicated invalidRegex error (content `-S`/`-G`
only; bad regex → generic Git error); OQ3 palette only when a repo is open; OQ4 minimal match meta
(single `matched` + path-only snippet); OQ5 ship branch `scopeRef`, **DEFER date scope (since/until)**
for v1 (omit those fields from the wire type).
- **P50a** (reviewer APPROVE, 0 must-fix/should-fix) — `git/search.rs` (git2 revwalk msg/author/all +
  `git log -S/-G` shell path/content via injected `GitRunner`; caps 1000 / scan 200k; cap+1
  truncation exact both backends), `search_commits` cmd (**129**), IPC + mock (`resolveLayout` shared
  with getGraph; `#fail`→Git; truncated path). 25 tests (8 real-git oracles + FakeGitRunner argv +
  PanicRunner + cap/empty/invalid-regex + wire shape). Deviations (validated): `--glob-pathspecs`
  before `log` (git 2.51 fix); since/until omitted (OQ5); `parse_log_output` takes field+text;
  `resolveLayout` extracted (pure refactor). Tester NITs to fold: remotes+tags oracle fixture for
  `seed_all_refs`; `all` message-wins `matched` assertion.
- **P50b** (reviewer APPROVE, 0 must-fix; 1 should-fix + 2 nits FOLDED) — `useCommitSearch` hook +
  pure `searchHelpers` + `CommitSearchBar` + `SearchResultsList`; `--match-ring` draw pass on the
  canvas (additive, no re-layout, visible rows only); next/prev + results-click reuse
  `revealCommitByOid` (current match = normal selection); cheap modes debounced live, content
  submit-only; regex gated to Content; Ctrl/Cmd-F + graph-pane FAB open, capture-phase Esc layering.
  Folded: match-ring staleness (graph identity now in the `matchRows` memo deps + reactive `graph`
  prop), refocus-on-reopen (`openNonce`), dead badge CSS dropped.
  **P50b AI GATE PASSED** (harness, mock :1420, zero console errors): FAB opens the bar; message
  live-search "graph" → **1/1** + results list (`cd08f97` · summary · 2h · **"Message"** badge); regex
  toggle disabled in message mode, ENABLED in Content (`Regular expression (git -G)`, disabled=false).
  tsc + build clean; 20/20 vitest. (Canvas ring draw + scroll feel = reviewer-verified / USER CHECKPOINT.)
- **P50c** (reviewer APPROVE, 0 must-fix/should-fix; 3 cosmetic nits noted) — `CommandPalette` +
  `paletteActions` (`PaletteAction`/`buildPaletteActions` + pure `fuzzyScore`/`filterActions`) +
  `usePalette` hook. Ctrl/Cmd-K toggle (distinct from search's Ctrl/Cmd-F), ↑↓ skip-disabled, Enter
  runs+closes, capture-phase Esc at the TOP of the peel order. Actions reuse toolbar handlers with
  matching disabled-gating; branch/tag/commit jumps = `revealCommitByOid` (non-mutating); create
  entries only OPEN dialogs. **No destructive op fires directly** (reviewer-verified). 11 fuzzy tests.
  **P50c AI GATE PASSED** (harness, mock :1420, zero console errors): Ctrl+K → palette w/ 36 options
  (no destructive entries); typing "push" → 2 (dynamic "Search commits for push" + Push); Esc closes
  ONLY the palette (workspace intact). tsc + build clean; 11/11 vitest.
- **P50d** (reviewer APPROVE, 0 must-fix/should-fix; nits noted) — `listFilter.ts`
  (`filterByName`/`filterItems`/`filterTree` — flat + tree ancestor-keep, pure) + `ListFilterInput`
  (capture-phase Esc-clear, focus-scoped) wired into Branches/Remotes/Tags (shown when expanded &
  ≥6 rows; query forced '' when hidden; tree filter-active key expands matches; no-match hint).
  Mock tags 4→7 so the Tags filter is reachable in-harness (reviewer-confirmed safe: tag pills come
  from layout node refs, not `branches.tags`). 10 unit tests.
  **P50d AI GATE PASSED** (harness, mock :1420, zero console): Branches + Remotes filter inputs appear
  (≥6 rows); typing "gh" → only `gh-pages`; "zzzznomatch" → "No branches match" hint. tsc+build clean;
  41/41 vitest.
- **P50 tester** — regression: **cargo 923 passed / 1 failed / 2 ignored**, vitest **41/41**. The 1
  failure is `health::tests::perf_ceiling_on_20k_fixture` — a PRE-EXISTING perf-ceiling gate
  (`health.rs`, NOT touched by P50; branches scan ~6s vs 2s budget, I/O-bound on this Defender box,
  consistent across runs, fails even isolated). NOT a P50 regression (P50 only added `search.rs`).
  ⚠ real perf watch item (aligns with parked C5 large-repo perf + the P29 health-branches watch) —
  candidate for a perf pass / fold into P52. Added 2 search oracles
  (`oracle_all_refs_seeds_remotes_and_tags`, `oracle_all_matched_label_message_wins`) + fixed the
  pre-existing `essentials_autostash_cli.rs:128` doc nit → `clippy --all-targets -D warnings` now CLEAN.
  tsc/build clean. `P50-user-checklist.md` complete.
- **P50 AI GATE PASSED (2026-08-07).** Commits: `ab1194b` P50a · `f60c308` P50b · `0a01118` P50c ·
  `0fd2232` P50d (+ tester closeout). Search (backend oracle vs real `git` + browser harness), command
  palette, and list filtering all verified; zero P50 regressions.

**P50 awaiting USER CHECKPOINT** (native `pnpm tauri dev`, per `docs/contracts/P50-user-checklist.md`):
large-repo search responsiveness feel; match-ring highlight + next/prev canvas scroll + "not in view"
hint; content `-S`/`-G` on a real repo; Ctrl/Cmd-K palette + Ctrl/Cmd-F search + sidebar Branches/
Remotes/Tags filters on a real repo.
**Status:** P50 DONE (AI gate passed, awaiting USER CHECKPOINT). User chose Finish-Phase-1 + granted a
3h autonomous window (2026-08-07) → resumed straight into P51/P52 without pausing; native checkpoints
batched. ⚠ Follow-up: health 20k perf-ceiling gate failing (see P50 tester) — task chip spawned
(task_e253301f); fold into P52.

## P49 — external integrations: open in terminal / file manager / editor (Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-07)

Roadmap approved 2026-08-06 from a feature-gap analysis (6 research streams: codebase inventory +
worldwide Git-client reviews + AI+git use-cases + terminal/graph/perf). Full plan:
`~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`. Sequenced **P49–P65** across four themes
(quick wins · AI-native edge · forge/PR · correctness/parity) + large-repo perf. **Phase 1** = P49
external integrations → P50 search + command palette → P51 graph polish/clutter controls → P52
adopt git's commit-graph file.

**P49 goal:** launch the user's external tools from Bonsai at a repo/worktree/submodule path — (a)
**open in terminal**, (b) **reveal in file manager**, (c) **open in editor**. Currently ABSENT — no
shell/opener Tauri plugin is registered at all (`src-tauri/capabilities/default.json` has only
dialog/updater/process). Research verdict: external launch satisfies most users; an embedded terminal
is a heavy build CLI power-users bypass → external only for v1.

**Locked decisions (from approved plan):**
- External launch only; embedded terminal deferred.
- Terminal = user-configurable **per-OS command template** with a `{path}` placeholder; auto-detected
  defaults (Win: `wt -d {path}` → PowerShell → cmd; macOS: Terminal/iTerm via `open -a`; Linux:
  template with `--working-directory`/`-e`).
- **Safety:** spawn as a child process with explicit cwd + path-as-arg — NEVER interpolate the repo
  path into a shell string (dodges the `wt` `;`/PATH gotchas + injection); handle paths with spaces.
- Three commands: `open_in_terminal`, `reveal_in_file_manager`, `open_in_editor`.
- Entry points: repo/worktree/submodule context menus + tab `+`/tab menu + a toolbar button.

**Acceptance criteria:** the three actions launch the correct external app at the correct path on
Windows (+ macOS/Linux defaults present); a failed launch surfaces a clear toast, never a silent
no-op; terminal template editable in Settings + persisted, default auto-detected per-OS; path with
spaces handled; tsc + pnpm build clean; cargo check + clippy clean; `src/ipc/mock.ts` in lockstep;
USER CHECKPOINT = real OS launch per platform (harness cannot verify a real terminal opening).

Sub-increments: **P49a** Rust (plugin + capabilities + 3 commands + settings field + IPC triple +
mock) → **P49b** frontend (Settings terminal-template section + context-menu/tab/toolbar entries).

**Contract:** `docs/contracts/P49-external-integrations.md` + `P49-user-checklist.md` (architect — DONE).
**Orchestrator decisions on flagged OQs:** OQ1 reveal = open the directory; OQ2 editor auto-detect =
VS Code family only (else `editor_command`); OQ3 single per-machine template string (auto-detected
default per current OS); OQ4 add `AppError::ExternalToolFailed(String)`. Plugin: self-contained Rust
`std::process::Command` — no plugin/capability change (`capabilities/default.json` unchanged).
- **P49a** (reviewer APPROVE, 0 must-fix/should-fix) — committed `20ec096`. core `external.rs`
  (TargetOs-branched builders + CommandRunner/SpawnRunner + ladders), `ExternalToolFailed`, settings
  `terminal_command`/`editor_command`, 3 commands (128), IPC + mock (`#fail` sentinel). 18 core + 2
  command tests. Nits deferred: missing-path command test → tester; `wt` alias pick → native checkpoint.
- **P49b** (reviewer APPROVE, 0 must-fix/should-fix) — `SettingsExternalToolsSection.tsx` + shared
  `externalToolsItems(path)` in `workspaceMenus.ts` (spread into worktree/submodule menus) +
  App-owned per-tab context menu + toolbar "Open externally" dropdown + container handlers →
  error toasts. Deviations (sound, reviewer-validated): toolbar takes `ContextMenuItem[]`; tab menu
  owned by App (TabStrip is App-level).
**P49 AI GATE PASSED** (browser harness, mock :1420, zero console errors): EXTERNAL TOOLS settings
section renders (terminal/editor inputs + per-field reset + `{path}` "separate argument, never a
shell" helper); toolbar "Open externally" → Open in terminal / Reveal in file manager / Open in
editor, click = silent success; tab right-click → same 3-item menu. Success path proven live;
`#fail` error-toast + within-session persistence reviewer-verified (no ipc handle exposed to the
harness). tsc + pnpm build clean.
- **P49 tester** — full-workspace regression PASS (`cargo test --workspace` **897 passed / 0 failed /
  2 ignored** pre-existing; zero regressions). Added `external_launch_rejects_missing_path_before_
  spawning` (drives the shared `launch_inner` `!exists` guard via `reveal_in_file_manager` — the
  runtime-free command — returns `Io`, launches nothing). clippy + tsc/build clean.
  `P49-user-checklist.md` covers all native items.
- **P49 AI GATE PASSED (2026-08-07).** Commits: `20ec096` P49a · `e186825` P49b (+ this tester
  closeout). Backend argv/settings tests + browser-harness UI both verified; zero regressions.

**P49 awaiting USER CHECKPOINT** (native `pnpm tauri dev`, per `docs/contracts/P49-user-checklist.md`):
real per-OS launch of terminal / file-manager / editor from all 4 entry points (toolbar, tab menu,
worktree + submodule rows); Windows `wt`→PowerShell→`cmd` fallback + `code.cmd` PATHEXT resolution;
a repo path **with spaces**; terminal-template edit + Reset-to-auto-detect persisting across restart;
failure toasts. This is the USER CHECKPOINT because the AI harness can prove argv correctness + mock
behavior but cannot launch a real external app.
**Current step:** P49 DONE (AI gate passed, awaiting USER CHECKPOINT). Next Phase-1 milestone: **P50**
(commit/content search + command palette + list filtering) — architect contract pending.

## P48 — New Worktree dialog UX (searchable branch picker, wider dialog, per-category select-all) — **DONE (AI gate + USER CHECKPOINT confirmed)** (2026-08-06)

User reported three pain points in the New Worktree dialog: (1) dialog too small / long file
paths unreadable, (2) branch `<select>` doesn't scale to 100+ branches, (3) no bulk selection for
copy candidates. Approved plan: `~/.claude/plans/about-worktrees-1-the-ancient-island.md`.

**Scope (locked with user):**
- Widen the dialog card to 520px + full-path `title` tooltip on each copy-candidate row.
- New reusable `src/components/Combobox.tsx` (type-to-filter, keyboard nav, disabled "checked out"
  branches, select-on-focus, capture-phase Escape that closes only the open dropdown). Replaces the
  native branch `<select>` in `WorktreeCreateDialog.tsx`; also applied in free-input mode to the
  `WhatChangedDialog.tsx` ref fields (replacing the native `<datalist>`).
- Per-category "Check all / Uncheck all" toggles for Staged / Unstaged / Untracked (NOT Gitignored)
  in `WorktreeCopyCandidates.tsx`.
- Reusable-everywhere note: the only true branch dropdowns were the worktree `<select>` and the
  WhatChanged datalist; checkout/merge/rebase/create-from are sidebar-row + context-menu driven
  (different interaction) and were left unchanged.

**AI GATE PASSED (2026-08-06).** Reviewer found + fixed one blocking bug (capture-vs-bubble Escape
ordering) and browser-harness verification found + fixed one UX gap (typing appended to the
pre-filled label → added select-on-focus). Both re-verified live. tsc clean, no console errors.
Committed `10155e8` (5 files, +450/-62; new `Combobox.tsx`). **USER CHECKPOINT CONFIRMED
(2026-08-06)** in native `pnpm tauri dev`.

## P47 — cherry-pick enhancements + commit-action menu consolidation — **DONE (AI gate passed)** (2026-08-06)

User asked to "implement cherry-pick commits"; investigation found single-commit cherry-pick
already exists (P20b) but was only reachable from the commit-row menu (user right-clicked a branch
pill and didn't find it), and fails on a dirty worktree. Approved plan:
`~/.claude/plans/i-need-you-to-replicated-goose.md`.

**Scope (locked with user):**
- **A. Menu consolidation** — extract a shared `commitActionItems(oid)` sub-builder (mirroring the
  existing shared `resetMenuItems`) so oid-based actions (create branch/tag here, compare,
  cherry-pick, revert) are reachable from branch AND tag pills, not only the commit row. Reverse
  direction (checkout/copy-name/reflog/summarize) stays ref-specific by design.
- **B. Single-pick enhancements** — autostash a dirty worktree (mirror `merge.rs` autostash;
  likely add `StashPopConflicts` to `CherrypickOutcome`, apply parity to revert); editable commit
  message (optional message param → new `CherrypickMessageDialog.tsx`); fix paused pick/revert
  conflict-fetch bug at `RepoWorkspace.tsx:555`.
- **Excluded:** detached-HEAD cherry-pick; multi-commit/range; `--allow-empty`.

Sub-increments: P47a Rust core → P47b IPC+commands → P47c menu consolidation → P47d message
dialog + bug fix.

**Contract:** `docs/contracts/P47-cherry-pick-enhancements.md` + `P47-user-checklist.md` (done).
Orchestrator decisions on flagged forks: F1 extract shared `autostash.rs` + migrate merge onto it;
F2 revert keeps deterministic message (no editor); F3 graph tag pills + all branch pills get commit
actions, sidebar tag rows scoped out; F4 editable-message dialog; F5 continue/abort leave retained
autostash for manual pop (mirrors merge).

**AI GATE PASSED (2026-08-06).** All four increments committed & reviewer-APPROVED: P47a `c456e73`
(core), P47b `dc67360` (IPC), P47c `aeb5bb8` (menu consolidation), P47d `7498215` (message dialog +
paused-conflict fix). Tests `5319f34`: 7 new autostash/message CLI oracles + 380 lib + all
integration green, 0 regressions. Browser-harness fidelity fix (ref-tip commit diffs in mock so
branch/tag-pill actions resolve — also fixes pre-existing Compare-with-HEAD gap) in the same commit.

**AI-gate evidence (browser harness, VITE_MOCK_IPC):**
- Menu consolidation ✅ — branch pill menu now lists Create tag here / Cherry-pick onto current… /
  Revert commit (were commit-row-only before); no duplicated create-branch/compare.
- Cherry-pick dialog ✅ — opens from a branch pill prefilled with the source commit's multi-line
  message; editing + confirm → toast "Cherry-picked de3dc5d · stashed changes restored" (autostash).
- Revert ✅ — from a branch pill → "Reverted 607f7d5 · stashed changes restored".
- Dialog error path ✅ — message-fetch failure closes cleanly with an error toast.
- B3 paused-conflict fix — code-verified (reviewer trace: refetchOpState now feeds conflictCount for
  cherryPick|revert → OpBanner Continue disabled); not drivable in-harness (conflict trigger is a
  hardcoded oid suffix, unreachable from a pill) — covered by the native checkpoint.

**Current step:** DONE — marked done at user's explicit request (2026-08-06). NOTE: the native
USER CHECKPOINT (`pnpm tauri dev`, `docs/contracts/P47-user-checklist.md`) was **not run** — the
user chose to close P47 on the AI-gate evidence alone; native verification is deferred/waived, not
confirmed. Accepted as-is: unborn-HEAD branch pill no longer offers create-branch/tag (now uniform
with the commit-row menu, which already hid these when HEAD is unborn).

## v1-prep — release-readiness for a public 1.0.0 — **DONE (AI gate passed)** (2026-08-05)

Approved plan: `~/.claude/plans/i-want-to-prepare-floofy-crystal.md`. Based on a 3-agent
readiness audit (roadmap / code-incompleteness / release-engineering). **Prepare only — NOT
cutting the release** (user has further changes coming; version stays `0.2.0`, no tag).

**Scope decisions (locked with user):** public open-source · ship the current codebase
**as-is** (all built P28–P45 features stay visible; a light native smoke test precedes the
eventual tag, full formal checkpoints deferred to v1.x) · installers ship **unsigned**
(documented SmartScreen/Gatekeeper steps) · **keep** auto-update (fix plumbing).

**Landed (3 commits, `da97e00`, `ca90865`, `da49852`):**
- Tier 0: `LICENSE` (MIT) + `license`/`repository` in `src-tauri/Cargo.toml`; end-user
  `README.md`; `CHANGELOG.md` (`[Unreleased]` 1.0.0-prep); `docs/code-signing.md` records
  the ship-unsigned decision + macOS notarization path.
- Tier 0.4: React error boundaries — new `src/components/ErrorBoundary.tsx`, wrapping app
  root + commit-graph / diff-view / conflict-editor panes (was: a render throw white-screened
  the whole app). Only substantive code gap the audit found.
- Tier 2.2: gated 4 self-test/perf `console.log`s behind `import.meta.env.DEV`.
- Tier 2.1: set `app.security.csp` (was `null`). Tier 1.2: `releaseDraft: true → false` so
  the updater `latest.json` resolves via `/releases/latest/`.

**AI gate:** green — `pnpm build` (tsc+vite) ✅, `vitest` 4/4 ✅, `cargo check -p bonsai` ✅
(validates the manifest + tauri.conf CSP via tauri-build), browser harness renders with no
console/build errors and no boundary fallback.

**Remaining before an eventual tag (NOT done here):**
- **USER CHECKPOINT — native smoke test:** `pnpm tauri dev` — graph renders, core happy path
  (open repo → stage/commit → diff → branch → fetch/pull/push), and **confirm the new CSP
  doesn't white-screen the native app** (browser harness cannot enforce the Tauri CSP; if it
  breaks, relax `script-src`/`connect-src` or revert csp to null). Trip an error boundary to
  see the fallback.
- **USER action for auto-update:** the committed `plugins.updater.pubkey` is still the DEV
  key (per `docs/contracts/P42-user-checklist.md` A2). Generate a prod keypair
  (`pnpm tauri signer generate`), replace the pubkey, set `TAURI_SIGNING_PRIVATE_KEY`(+`_PASSWORD`)
  CI secrets. Else clients reject updates as bad signatures. Also make the GitHub repo public.
- **Release cut (deferred):** bump to `1.0.0` across the 3 manifests, tag `v1.0.0` → CI matrix
  builds/publishes. Do after the user's pending changes.

## P46 — diff viewer: split view + copyable selection + auto-advance — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-05)

Three user-requested diff-viewer enhancements. Approved plan:
`~/.claude/plans/1-what-it-would-fluttering-thimble.md`. Contract + architect design appendix:
`docs/contracts/P46-diff-viewer-enhancements.md`. **Frontend-only** — no Rust/IPC/fetch changes
(`FileDiff → Hunk[] → DiffLine[]` already sufficient; split fetches 3-context like Diff).

Locked decisions (user): **interactive** split (staging works in the columns) · **free native
text selection** (stage-drag moves to the line-number gutter so content is copyable) · stage the
last file → **close** the overlay.

- **WS2 copy** (`70df786`): moved the range-staging drag handle off the whole `.diff-line` row onto
  the `.diff-lineno` gutter spans (`user-select:none`, `cursor:row-resize`); `.diff-content` now
  natively selectable → `Ctrl+C` yields clean code (no line numbers/markers). `DiffView.tsx` +
  `styles.css`.
- **WS3 auto-advance** (`70df786`): `handleStage` opens the next changed file's diff after staging
  the open one (pure `src/utils/nextFile.ts::nextFileAfter`, merged `unstaged++untracked` order),
  closing when the staged file was last. Added `statusRef`. Only fires when the open slot's path is
  the one staged.
- **WS1 split** (`f74bb53`): third **Split** toggle (File/Diff/Split); old-vs-new two-column layout
  via pure `src/utils/splitRows.ts::pairSplitRows` rendered by stateless `DiffViewSplit.tsx`.
  `DiffView` keeps ALL selection/range state; per-cell `data-g` = global line index reuses the
  existing stage/discard/range machinery unchanged.
- **WS1b synced scroll** (`ba24f2b`, user follow-up): replaced per-cell scrollbars with a two-pane
  layout (one horizontal scrollbar per column) whose `scrollLeft` is kept in lock-step (left/right
  refs + rAF reentrancy guard); cells `min-height:1.5em` keep filler rows aligned across the two
  independent panes.

**Current step:** DONE pending native checkpoint. architect→senior-dev(×3)→reviewer(APPROVE, no
MUST-FIX)→tester loop complete. AI GATE PASSED: `tsc --noEmit` clean; vitest **20/20** (new:
`splitRows.test.ts` 10, `nextFile.test.ts` 6; pre-existing 4 green). Browser-harness
(VITE_MOCK_IPC=1, zero console errors): split renders two columns with del-left/add-right tinting +
center divider + shared `data-g`; content selectable while gutters/markers `user-select:none`
(selection excludes line numbers/markers); staging README (non-last)→overlay advances to next
unstaged file; staging scratch.rs (last)→overlay closes. **USER CHECKPOINT (must NOT self-pass):**
native `pnpm tauri dev` — confirm OS-clipboard `Ctrl+C` from a diff, split-view scroll/resize feel
+ per-cell/gutter staging, and the stage→next-file / stage-last→close flow on a scratch repo.

## P45 — per-line discard action (mirrors "Stage 1 line") — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-05)

User request: a per-line discard, similar to the existing per-line stage. Approved plan:
`~/.claude/plans/i-want-to-have-adaptive-engelbart.md`. User chose **both** affordances (question
prompt): a floating **"Discard N lines"** button beside "Stage N lines" AND a per-line gutter
discard control beside the `+`. Every discard arms a confirm dialog (destructive-op guardrail).

**UI-only change** — backend/IPC/mock already support arbitrary `LineSelection[]` discard via the
existing `discard_partial` / `ipc.discardPartial` (P28 "Discard hunk" drives the same core at hunk
granularity). No Rust, IPC-signature, or mock changes.

Scope (all in `src/components/`): new `onDiscardLines?(selection)` prop threaded
`DiffView` → `DiffSlotView` → `DiffOverlay` → `WorkspaceGraphPane` (gated `kind==='unstaged' &&
stageable==='stage'`, same as `onDiscardHunk`); one App handler `handleDiscardLines` arming
`pendingLineDiscard`, confirmed via `handleConfirmLineDiscard` → `ipc.discardPartial`; one
`ConfirmDialog` in `WorkspaceDialogs.tsx` modeled on "Discard hunk?"; CSS for the float danger
button + gutter discard button (marker-gutter layout is the one detail for the contract).

Acceptance: both affordances revert exactly the selected line(s) toward the index (index untouched);
absent on staged/untracked/read-only diffs; confirm dialog required; tsc + pnpm build clean;
`discard_partial` cargo tests stay green.

**Current step:** DONE pending native checkpoint. Committed: impl `fa90a31` (8 files, UI-only),
tests next commit. Architect→senior-dev→reviewer (APPROVE-WITH-NITS, no MUST-FIX)→tester loop
complete. AI GATE PASSED: tsc + pnpm build clean; `cargo test -p bonsai-core discard` 24 passed
(incl. new `single_added_line_discarded` / `single_deleted_line_discarded`), `stage_partial` 13
passed. Browser-harness (VITE_MOCK_IPC=1, zero console errors) confirmed on mock src/main.rs: gutter
`×` + float "Discard N lines" each arm a confirm dialog then revert exactly the selected line(s)
toward the index; both absent on staged diffs (no `.diff-view--discardable`); marker gutter widened
to 34px. **USER CHECKPOINT (must NOT self-pass):** native `pnpm tauri dev` on a scratch repo —
discard one line (gutter) + a range (float), verify `git diff` shows only the non-discarded changes
and `git diff --cached` is unchanged.

## P44 — settings improvements (4 user requests) — **DONE (code AI-gate passed, awaiting USER CHECKPOINT)** (2026-08-05)

Direct user feedback, 4 items. Approved plan: `~/.claude/plans/i-want-the-following-fluffy-lerdorf.md`.
Decisions locked via question prompt: #2 = lightweight **named identity profiles** (name/email
bundles applied to a repo's Local git config; no per-repo app-settings storage); #3 = collapse
**Behaviour + custom keys** under one collapsed "Advanced" (Identity stays visible).

Sub-increments (independent; A/C small, B core+contract, D new IPC surface+contract):
- **P44a** (item #1) — remember MCP enablement across restart. Bug: `mcp_enabled` is persisted but
  never read back at launch + the toggle reflects the live server, not the flag. Fix in
  `lib.rs .setup()`: auto-start MCP when persisted `mcp_enabled` true (restore `mcp_allow_write`),
  start-failure non-fatal. Update `mcp.rs` module doc (reverses the "never auto-start at launch"
  privacy choice — by explicit prior user consent). Backend only.
- **P44b** (item #4) — exclude gitignored files from repo health stats walk
  (`crates/bonsai-core/src/health.rs` `walk_dir` → ignore-aware via `repo.is_path_ignored`, prune
  ignored dirs; `.git` sizing walk stays unfiltered). Architect updates `docs/contracts/P29-repo-health.md`
  §3/D1 ("ignored files ARE counted" → excluded). tester updates health tests. Small label note in
  `RepoHealthPanel.tsx`.
- **P44c** (item #3) — collapse Behaviour + custom-keys under one `<details>` "Advanced" in
  `SettingsGitConfigSection.tsx` (keep Level toggle + Identity visible). Frontend only.
- **P44d** (item #2) — named identity profiles. Architect writes `docs/contracts/P44-identity-profiles.md`.
  Backend: `IdentityProfile` + `profiles` in `settings.rs`/`UiSettings`/`UiSettingsPatch`/`apply_patch`;
  new `apply_identity_profile` command (writes user.name/email/signingkey to Local via core
  `set_config`). Frontend: types + IPC wrapper + mock; new `SettingsProfilesSection.tsx`; wire into
  `SettingsPanel.tsx`.

**USER CHECKPOINT (I must NOT self-pass):** A — enable MCP + write in `pnpm tauri dev`, quit,
relaunch → still enabled + listening. D — create profile, Apply to repo, verify `git config --local
user.email` changed, restart → profiles persist. B — open a real repo with a large ignored dir →
health count/bytes exclude it. C — collapsed Advanced feels less overwhelming, Identity usable.
**AI gate:** cargo test/clippy (B health tests) + cargo check + tsc + pnpm build clean; harness
(C collapsed Advanced expander; D profiles list/create/delete/apply on mock).

**Current step:** ALL FOUR increments committed + reviewed + tested. P44a (33168eb), P44c
(e6ca2b7), P44b (64ce3dd), P44d (3489200), tests (6f8c70c). Reviewer APPROVED all; one SHOULD-FIX
folded on A (dropped redundant set_allow_write launch bounce — start already restores the persisted
write gate) and one on D (edit-then-Apply staleness race → command now carries identity FIELDS, not
a profile_id, reading App's live in-memory state; contract addendum records it). AI GATE PASSED:
cargo clippy clean (bonsai + bonsai-core); tests green — health 13 (incl. stats_excludes_gitignored_
files), config 14 (incl. 4 apply_identity_profile_*), settings 30 (incl. profiles_roundtrip_and_
backcompat), commands apply_identity_profile NoRepo 1; pnpm build (tsc+vite) clean. Harness
(VITE_MOCK_IPC=1, zero console errors): C — Git config shows Identity + a collapsed "Advanced"
<details> containing Behaviour + Custom keys; D — seeded Work/Personal profiles render, applying
Work wrote mock Local user.email=work@bonsai.dev and lit the "Active on this repo" badge on Work.
Awaiting USER CHECKPOINT (native, listed in section head).

## P42 — packaging + auto-update (Productization) — **DONE (code AI-gate passed, awaiting USER CHECKPOINT)** (2026-08-05)

Productization milestone. **User explicitly requested auto-updates (2026-08-04, "do not forget about
auto updates").** Tauri v2 updater plugin + a check-for-update/update-available/download-install flow,
WRAPPED behind Bonsai's IPC-triple (INV-1: React never calls @tauri-apps/plugin-updater directly →
harness-verifiable via a `?update=available|none|error` mock seam) + per-OS packaging config. Contract:
docs/contracts/P42-packaging-autoupdate.md (architect). Accepted defaults: updater logic lives in
tauri.ts driving the JS plugin (NOT a Rust command — Tauri v2's flow is stateful JS-side); GitHub
Releases as the placeholder endpoint host; pubkey-in-config / private-key-in-CI-secret split; Windows
installMode=passive; no separate release-doc (folded into the user-checklist); new AppError `updateFailed`
+ additive `auto_check_updates` settings field. Sub-increments: **P42a** config + plugin registration +
capabilities + settings field + IPC-triple + mock seam (generate updater keypair — pubkey committed,
private key GITIGNORED + user generates their own for prod) → **P42b** UI (UpdateNotification/UpdateDialog/
SettingsUpdatesSection + auto-check-on-launch + harness verification).
**USER CHECKPOINT (I must NOT self-pass):** real endpoint URL; updater private key + code-signing
(Authenticode/notarization) secrets; the full SIGNED download→install→relaunch round-trip; native
installer smoke per OS. **AI gate:** cargo/clippy/tsc/pnpm build clean + settings test; unsigned
`pnpm tauri build` emits updater artifacts (background + poll — NEVER conclude from a timeout); the
mock-seam UI flows (available→notify→progress→restart; up-to-date; error) in the harness.

- **P42a** (reviewer APPROVE, 0 must-fix; SECURITY clean — committed pubkey decodes to a genuine minisign
  public key, private key gitignored + not tracked, INV-1 boundary verified only tauri.ts imports the
  plugin) — updater/process plugins + capabilities + config (createUpdaterArtifacts, windows installMode
  passive, OWNER/REPO endpoint placeholder, signing placeholders) + auto_check_updates settings field (2
  tests) + IPC-triple (checkForUpdate/downloadAndInstallUpdate/relaunchApp, updateFailed AppError) +
  ?update= mock seam. cargo check/clippy + settings 29 + tsc + build clean. Nits (cosmetic): heuristic
  error-message mapping; mock error-mode only covers check-time.
- **P42b** (reviewer APPROVE, 0 must-fix; INV-3 verified — launch only checks/notifies, download+install
  +restart all explicit user actions) — useUpdateController hook (idle→checking→available|upToDate|error→
  downloading→readyToRestart) + UpdateNotification banner + UpdateDialog (version+notes→Download&install→
  progress→Restart now/Later; Esc/backdrop disabled mid-download; error+Retry) + SettingsUpdatesSection
  (current version, Check for updates, auto-check-on-launch toggle) + App auto-check-on-launch (gated
  autoCheckUpdates || ?update=). AI GATE PASSED (fresh tab :1420, zero console): ?update=available →
  banner→dialog→0.7→4.7MB progress→"Update ready…Restart now"; ?update=none → "You're up to date";
  ?update=error → inline "could not reach the update endpoint". tsc + build clean. Nits (cosmetic, left):
  infoRef not cleared on check-error; focus fallback; dead class; unreachable idle fallback text.
- **P42 tester** — full workspace regression PASS, zero regressions (bonsai_lib 101, bonsai_core 373, all
  integration 0 failed; remote_cli flaky test passed); clippy clean; tsc + build clean. Added
  set_ui_settings_patch_auto_check_updates_is_partial. Private-key security re-confirmed (git ls-files
  .tauri empty; gitignored). Checklist: docs/contracts/P42-user-checklist.md (Part A user-provides:
  endpoint + production updater keypair→CI secrets + code-signing certs; Part B native round-trip).
- **P42 CODE AI GATE PASSED (2026-08-05).** Commits: 67fe3bc P42a · b1cfeac P42b (+ tester closeout).
  cargo check/test/clippy + tsc + build all clean; full auto-update UI browser-harness-verified (available
  →download→ready→restart / up-to-date / error, zero console errors). Updater plugin + config + IPC-triple
  + UI all reviewer-APPROVED. Productization milestone #2 (auto-update) delivered at the code level.

**P42 awaiting USER CHECKPOINT** (per docs/contracts/P42-user-checklist.md): the `pnpm tauri build`
bundling + the real SIGNED endpoint→check→download→install→relaunch round-trip, all of which need
user-supplied secrets (endpoint owner/repo, production updater private key, code-signing certs). NOTE:
`pnpm tauri build` bundling (NSIS/WiX) may be blocked on this machine by the IT-enforced Defender ASR
rule (see memory: defender-asr-blocks-appdata-exes) — the code compiles clean via cargo check regardless.
- **P42 release-build bonus gate (2026-08-05):** `pnpm tauri build --no-bundle` compiled ALL deps +
  workspace crates cleanly WITH the updater/process plugins (tauri 2.11.5) — the code builds for release.
  It failed ONLY at the final step: `failed to remove target\release\bonsai.exe — Access is denied
  (os error 5)`, because a running `bonsai.exe` (PID 59136, started 2026-08-04 18:16 — the user's open
  native app) holds an exclusive lock on the exe. NOT a code error; not killed (user's process). A real
  release build needs the app closed first (noted in the checklist). Combined with P42a's clean cargo
  check (which fully linked the debug binary), the release-compile gate is effectively passed.
**Current step:** ALL requested session milestones COMPLETE & committed. P37–P40 (Git completeness) +
untracked-clobber safety fix + P43 onboarding + P42 auto-update. P41 LFS deferred (user). Awaiting the
stacked native USER CHECKPOINTs (docs/contracts/P*-user-checklist.md) + P42 release secrets. Autonomous
session wrap-up.

## P43 — first-run onboarding + empty-state polish (Productization) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-04)

Productization milestone. User decision (2026-08-04): **skip P41 LFS (niche) and P42 packaging (needs
signing secrets) for now → build P43 onboarding.** Goal: a guided first-run experience (welcome →
identity check reusing P40 config → open/clone a repo via existing flows → brief feature tour of the
commit graph + AI-assets panel + health dashboard) plus tightened empty states (no-repo-open, empty/
unborn-HEAD repo). Frontend-mostly; a small persisted "seen onboarding" flag. Contract:
docs/contracts/P43-onboarding.md (architect). Guardrails unchanged (mock.ts compiling, orchestrator
commits, browser-harness AI gate). P41 (LFS) + P42 (packaging/signing/auto-update) DEFERRED, not dropped.

Contract: docs/contracts/P43-onboarding.md (architect). Accepted defaults incl. the STEP REORDER
(Welcome → Open/Clone → Identity → Tour) — identity-first is impossible without new backend since P40
config is repo-scoped (no repoId before a repo opens); reorder = zero backend, reuses P40 getConfig/
setConfig(global) once a repo is open. Persistence = new additive `onboarding_seen: bool` on the UI
settings (settings.rs #[serde(default)] + get/set_ui_settings mapping — NO new command); overlay surface;
static tour cards; Settings "Show welcome tour" re-trigger + ?onboarding=1 harness seam; additive
empty-state extract. Sub-increments: **P43a** overlay + step machine + persistence + re-trigger + mock
seam (the one backend field) → **P43b** empty-state polish (extract EmptyState.tsx). Note: P42 (auto-
update) architect running in parallel (design-only) — P42a implementer must NOT run concurrently with
P43a/b (both edit SettingsPanel/commands/types/mock); sequence P43 fully, then P42.

- **P43a** (reviewer APPROVE, 0 must-fix) — OnboardingOverlay + OnboardingSteps (Welcome→Open/Clone→
  Identity→Tour, reuses App open/clone/init + P40 getConfig/setConfig global) + onboarding_seen persisted
  field (settings.rs #[serde(default)] back-compat + 2 unit tests, get/set_ui_settings mapping) + App
  startup gate (!onboardingSeen || ?onboarding=1) + Settings "Show welcome tour" re-trigger. Harness
  (mock :1420): full flow Welcome→Open/Clone("✓ alpha-repo is open")→Identity(reads global name/email)→
  Tour(graph/AI/health cards)→Finish closes; re-trigger reopens. BUT harness caught a React
  setState-in-render error (OnboardingOverlay updates App during render — auto-advance/reset logic);
  fixed: onClose() was called inside a setStep updater (runs during render) → moved to the click handler
  (OnboardingOverlay goNext/goBack). AI GATE PASSED — re-verified in a FRESH browser tab (the original
  tab's console buffer was stale across HMR/reloads, showing phantom errors): full flow Welcome→Open/Clone
  →Identity→Tour→Finish + Settings re-trigger, ZERO console errors. cross-reload persistence (onboarding_
  seen prevents re-show) = USER CHECKPOINT (mock resets per load; reviewer verified the logic statically).
  Reviewer nits left (cosmetic): Esc no-return; dual identity forward paths; idempotent re-persist.
- **P43b** (reviewer APPROVE, 0 must-fix) — EmptyState.tsx extracted from App (behavior-preserving:
  same Open/Clone/New handlers + recents + error/loading; friendlier hero, no dead-end identity CTA) +
  friendlier unborn-HEAD card in WorkspaceGraphPane (first-commit hint + "Set your Git identity" button
  reusing the wired onOpenIdentitySettings, valid since a repo is open) + styles. AI GATE PASSED (fresh
  tab :1420): no-repo EmptyState renders "🌱 Bonsai · A tidy Git client" + 3 actions + recents; zero
  console errors. tsc + build clean. Nits (cosmetic, left): folderName duplicated App↔EmptyState;
  onOpenIdentitySettings required-vs-optional mismatch. Unborn-card visual = USER CHECKPOINT (mock can't
  easily stage an unborn repo).
- **P43 tester** — full workspace regression PASS, zero regressions: `cargo test --workspace` 854 passed
  / 0 failed / 2 ignored (perf_gate); remote_cli flaky test passed; clippy --workspace --tests clean; tsc
  + build clean. Added `set_ui_settings_patch_onboarding_seen_is_partial` (apply_patch overwrites only
  when Some → a later empty patch doesn't reset onboardingSeen; the backend half of no-reappear).
  Checklist: docs/contracts/P43-user-checklist.md.
- **P43 AI GATE PASSED (2026-08-04).** Commits: 4b657d3 P43a · 761dcd2 P43b (+ tester closeout). Frontend
  browser harness (fresh tab, zero console) + backend settings tests both verified. Productization
  milestone #1 (onboarding + empty states) delivered.

**P43 awaiting USER CHECKPOINT** (native pnpm tauri dev, per docs/contracts/P43-user-checklist.md): first
launch (or onboarding_seen:false) shows the overlay; Welcome→Open/Clone→Identity(set global user.name/
email, `git config --global` cross-check)→Tour→Finish; RESTART → onboarding does NOT reappear (real
settings.json onboardingSeen:true — the key checkpoint the AI harness can't verify); Settings "Show
welcome tour" reopens; ?onboarding=1 force-shows; no-repo EmptyState + unborn "first commit" card render.
**Current step:** P43 DONE. Onboarding shipped. Next: P42 (auto-update — user-requested; the last
in-flight milestone this session).

## P40 — git config editing (Git completeness, Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-04)

Git-completeness milestone #4. Read/edit git config in-app: curated identity/behavior keys (user.name/
email, core.autocrlf, init.defaultBranch, pull.ff|rebase) + arbitrary `section.key` entries, at Local
(repo) and Global levels; a Settings section; and close the long-standing "identity unset" gap (a "Set
identity…" button on the commit-error banner opens Settings). Contract: `docs/contracts/P40-config-editing.md`
(architect). Approach: runtime-free `git/config.rs` over git2 0.21 `Config` (read_config→ConfigView =
curated effective+level + advanced list; set_config/unset_config, validated server-side, Local|Global);
self-contained `SettingsGitConfigSection.tsx` keeps SettingsPanel lean; no new AppError (reuse Git/
InvalidName); no events. Accepted architect defaults: Local+Global (System read-only/out); curated form +
advanced list; last-value read (multivar edit deferred); "Set identity…" button. **Load-bearing test risk
(flag to tester):** Global writes touch the real ~/.gitconfig — the config_cli oracle MUST redirect
GIT_CONFIG_GLOBAL/HOME to a scratch dir under D:\Temp\bonsai-scratch + git2::opts::set_search_path, and
prove the dev's real global config is untouched. **Verify at P40a:** the architect reused SettingsPanel's
existing `repoPath` prop as the config `repoId` — confirm that value is what the config commands resolve
via `repo_path` (repo_id==path). Sub-increments: **P40a** core config.rs + 3 cmds + IPC + fixtures/mock +
config_cli oracle → **P40b** Settings UI + identity-gap linkage (App/RepoWorkspace/CommitBox). Guardrails
unchanged (scratch D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp on Windows, no concurrent test+clippy, mock.ts
compiling, orchestrator commits).

**Current step:** P40a implemented (gates green: config unit 10 + config_cli 1 + no-repo cmd 1, clippy/
tsc/build clean; global-config isolation verified — real ~/.gitconfig untouched). Deviations (documented):
unit test asserts target_value None not effective (hermetic — effective inherits dev's real global);
no set_bool (curated enums are tri-state strings); open_repo NotFound→NoRepo. Reviewer APPROVE (0 must-fix). Folding 1 should-fix (test hermeticity: assert target_value not effective
for pull.ff — avoids spurious failure if dev has pull.ff global) + 2 nits (map_level Worktree comment;
remove dead mock noConfig field) before commit.
(Landed in parallel: user-requested untracked-clobber safety fix — commit 46a34d4, task #9 DONE: shared
ensure_no_untracked_collision guard in both rebase+bisect engines, incl. type-swap dir→blob case.)
- **P40a** (reviewer APPROVE, 0 must-fix; 1 should-fix + 2 nits folded) — config.rs (read/set/unset_config
  over git2 Config; curated effective+level+target + advanced list; Local|Global via open_target +
  find_global fallback; server-side validate_key/validate_curated_value; multivar→last) + RepoOpState... no,
  + 3 cmds + IPC triple + fixtures/config.ts stateful store + config_cli oracle (Local + isolated-Global
  via GIT_CONFIG_GLOBAL/HOME + set_search_path). Mock commit()/commitAmend() gate on hasIdentity (identity
  gap demos e2e). Global-config isolation verified (real ~/.gitconfig untouched). config 10 unit + 1 oracle
  + no-repo cmd 1; clippy/tsc/build clean. Deviations (documented): NotFound→NoRepo; no set_bool (tri-state
  enums); Worktree→Local.
- **P40b** (reviewer APPROVE, 0 must-fix) — SettingsGitConfigSection.tsx (level toggle Local|Global,
  Identity + Behaviour curated selects + Advanced add/edit/remove, own reqId guard, save-on-blur/change,
  inline validation) + SettingsPanel section + identity-gap linkage (CommitBox "Set identity…" on
  configMissing → App configFocus → Settings focused on Identity). AI GATE PASSED (harness, mock :1420):
  GIT CONFIG renders; editing user.name at Local persists + inherited hint clears; ?fixture=noconfig
  commit → identity error banner + "Set identity…" → opens Settings on Identity input; zero console
  errors. Folding 1 should-fix (post-write refetch clobbered an unsaved sibling field mid-edit in the
  name→email path — preserve focused/dirty drafts) + 1 nit before commit.
  Folded (reviewer should-fix): mergeDraftsPreservingEdits preserves focused/dirty drafts on post-write
  refetch (view always refreshed unconditionally → no stale display); addEntry error nit. Re-verified in
  harness: email draft survives the name-write refetch (alice@dev.io preserved); section renders+editable
  post-fold; zero console errors. tsc + build clean.
- **P40 tester** — full workspace regression PASS, zero regressions (`cargo test --workspace` exit 0;
  bonsai-core lib 373 incl. 10 config unit; src-tauri lib 95; remote_cli flaky test passed); clippy
  --workspace --tests clean; tsc + build clean. Strengthened config_cli (appended to the single
  isolated-global fn to avoid a set_search_path race): Local-overrides-Global effective; unset falls
  back to Global; advanced multivar→last; malformed key rejected. **Real ~/.gitconfig untouched (md5
  identical before/after).** Checklist: docs/contracts/P40-user-checklist.md.
- **P40 AI GATE PASSED (2026-08-04).** Commits: 9ca9b3f P40a · cf174ff P40b (+ tester closeout). Backend
  config oracle (Local + isolated-Global) + frontend browser harness both verified; zero regressions.
  Git-completeness Phase 1 milestone #4 delivered.

**P40 awaiting USER CHECKPOINT** (native pnpm tauri dev on a SCRATCH repo, per docs/contracts/
P40-user-checklist.md): Settings → Git config shows Identity/Behaviour/Advanced + Local/Global toggle
with real values; set Local user.name/email (cross-check `git config --local`, hint clears); Global set
edits the REAL ~/.gitconfig (warned — use a throwaway); advanced add/edit/remove; identity-gap commit
block → "Set identity…" → unblock; invalid key rejected inline.
**Current step:** P40 DONE (AI gate passed, awaiting USER CHECKPOINT). Four Git-completeness milestones
(P37-P40) landed. PAUSED for user P41 (LFS) scope decision.

## P39 — git bisect (Git completeness, Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-04)

Git-completeness milestone #3. Binary-search for the regression-introducing commit: start (mark
good+bad), mark good/bad/skip the checked-out midpoint, reset (abort → original HEAD/branch); the
engine checks out each midpoint and, on convergence, reports the first-bad commit. In-progress OpBanner
shows revisions-left. Contract: `docs/contracts/P39-bisect.md` (architect). Approach: **Bonsai-owned
on-disk state machine** (git2 has NO bisect sequencer) — versioned JSON `.git/bonsai-bisect/state.json`
(atomic write, re-read each IPC call), mirroring the P23 interactive-rebase engine. Simpler than rebase:
the original branch ref NEVER moves (only a detached HEAD slides across midpoints) → reset is a pure
re-attach. Midpoint = positional split over `revwalk().push(bad).hide(good)`. New `RepoOpState::Bisect`
variant (probe-first in opstate.rs, mirrored in TS). No new AppError (reuse OperationInProgress/
NoOperationInProgress). Accepted architect defaults: two-click context-menu entry (mark-bad→mark-good);
positional midpoint — oracle asserts FINAL first-bad equality + midpoint-set membership (mathematically
sound: any correct bisect over a monotonic good→bad range converges to the same culprit regardless of
midpoint choice), NOT the exact intermediate sequence; progress rides existing get_op_state (no separate
get_bisect_state cmd). Sub-increments: **P39a** bisect.rs engine + midpoint/reset + RepoOpState::Bisect
+ opstate probe + 4 cmds + IPC + stateful mock + `bisect_cli.rs` oracle (vs real `git bisect run`) →
**P39b** OpBanner bisect arm (Good/Bad/Skip/Reset + found + counts) + two-click entry + Reset confirm.
Guardrails unchanged (scratch D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp on Windows, no concurrent
test+clippy, mock.ts compiling, orchestrator commits).

**Current step:** P39a implemented (gates green: bisect unit 13 + bisect_cli 3 + opstate 6 + no-repo cmd
1, clippy/tsc/build clean). Deviations (all sound): oracle uses a manual git bisect good/bad loop (same
first-bad, hermetic on Windows, no sh); on convergence HEAD is checked out at first-bad (more correct —
last-tested ≠ culprit when the final mark is good); commands don't emit repo-changed (mirror rebase step
cmds, progress rides get_op_state). Reviewer APPROVE (0 must-fix, 2 should-fix, 3 nits). Safety path (reset always restores original HEAD/
branch from every state) verified sound. Folding SHOULD-FIX #1 (add HEAD==state.current guard to
bisect_mark/skip — contract §2, prevents recording good/bad against the wrong commit if HEAD moved
externally); SHOULD-FIX #2 (force-checkout untracked clobber, inherited from rebase precedent) → spawned
as a separate cross-engine task (task_d8879761), not fixed here to keep the two engines consistent.
- **P39a** (reviewer APPROVE, 0 must-fix; SHOULD-FIX #1 folded) — bisect.rs on-disk state machine
  (.git/bonsai-bisect/state.json, atomic write) + positional midpoint + detached-HEAD checkout + dirty
  guard + start/mark/skip/reset/get_state; RepoOpState::Bisect + opstate probe-first; 4 cmds + IPC +
  stateful mock + bisect_cli oracle (final first-bad vs real `git bisect`). Folded: `ensure_on_current`
  HEAD==state.current guard on mark/skip (rejects recording a verdict against the wrong commit if HEAD
  moved; test asserts state byte-identical). Deviations (sound): manual git bisect good/bad oracle loop
  (hermetic); checkout first-bad on convergence (more correct); no repo-changed (progress rides
  get_op_state). bisect unit 14 + bisect_cli 3 + opstate 6 + no-repo cmd 1; clippy/tsc/build clean.
  SHOULD-FIX #2 (force-checkout untracked clobber, inherited from rebase) → task_d8879761.
- **P39b** (reviewer APPROVE, 0 must-fix) — OpBanner bisect arm (testing: revisions-left/~steps +
  Good/Bad/Skip/Reset; found: first-bad short-oid + summary + Reset; cannotDetermine handled) +
  RepoWorkspace handlers (start/mark/skip/reset → IPC → refreshAll, errors→toast) + two-click
  commitMenuItems entry ("Start bisect: mark this BAD" → pending-bad; "Mark GOOD & start bisect" gated
  on a distinct pending bad → startBisect(bad,[good])) + bisect-specific Reset ConfirmDialog + BisectIcon.
  Reviewer statically verified arg order, distinct-bad gate, hidden-mid-bisect, dispatch+error toasts,
  OpBanner rendering, confirm copy. tsc + build clean. Nits (cosmetic): bisectSummaries maps unused
  current oid; cannotDetermine leaves Good/Bad enabled (per contract — bounces off backend guard);
  RepoWorkspace.tsx now 3224 lines (pre-existing god-file, future extraction).
- **P39b AI GATE (partial — canvas/op-flow is USER CHECKPOINT).** The two-click START lives in the
  canvas commit context menu and the controls in the in-progress OpBanner; the hidden browser pane can
  NOT drive a canvas contextmenu (coordinate clicks need a screenshot; synthetic contextmenu doesn't hit
  canvas dots) NOR stage a real in-progress op — same documented limit that made P20's OpBanner flow +
  P25b's canvas menu USER CHECKPOINT. AI gate here = reviewer static APPROVE + tsc/build clean + P39a
  engine oracle (3 vs real git bisect) + 14 unit + stateful mock (driveMockBisect converges to found).
- **P39 tester** — full workspace regression PASS, zero regressions (`cargo test --workspace` exit 0;
  bonsai-core lib 363 incl. bisect unit 14; remote_cli flaky test passed); clippy --workspace --tests
  clean; tsc + build clean. Strengthened bisect_cli → 5 (added: reset-from-mid-bisect restores the EXACT
  pre-bisect tip + re-attaches `main` + clean status, cross-checked with git rev-parse — the #1 safety
  property; mark/skip after a real `git checkout --detach` errors via ensure_on_current + state
  byte-unchanged). Checklist: docs/contracts/P39-user-checklist.md.
- **P39 AI GATE PASSED (2026-08-04).** Commits: 86501b8 P39a · 25507f8 P39b (+ tester closeout). Engine
  oracle vs real `git bisect` (5) + 14 unit + reviewer static UI approve; interactive canvas→OpBanner
  flow deferred to USER CHECKPOINT (hidden-pane limit). Git-completeness Phase 1 milestone #3 delivered.

**P39 awaiting USER CHECKPOINT** (native pnpm tauri dev on a SCRATCH repo with a known first-bad commit,
per docs/contracts/P39-user-checklist.md): two-click canvas start (mark BAD → mark GOOD) → OpBanner
"Bisecting, N revisions left"; Good/Bad converges to "found first bad <oid>" with HEAD there
(cross-check git rev-parse); Skip works; Reset returns HEAD/branch to the original tip. This flow is the
USER CHECKPOINT because the canvas commit-menu + in-progress OpBanner aren't drivable in the AI harness.
**Current step:** P39 DONE (AI gate passed, awaiting USER CHECKPOINT). Next: P40 (config editing).

## P38 — reflog viewer + restore (Git completeness, Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-04)

Git-completeness milestone #2. The safety net for force-push (P37) / rebase (P23) / amend (P20) /
reset: read HEAD + per-branch reflog, present entries (short oid, old→new, message, committer/time,
`<ref>@{N}` index), and offer confirm-gated recovery — **"Create branch here"** + **"Reset current
branch to this"**. Contract: `docs/contracts/P38-reflog.md` (architect). Approach: runtime-free
read-only `git/reflog.rs` returning `Vec<ReflogEntry>` (cap `MAX_REFLOG_ENTRIES=2000`), one read cmd
`read_reflog` (spawn_blocking, no repo-changed); restore actions are PURE REUSE of the shipped
`create_branch_here` / `reset_branch` commands via existing PromptDialog / reset ConfirmDialog — zero
new mutation code (explicit invariant). git2 0.21: `Repository::reflog(name)`, index 0 = newest →
`<ref>@{N}` == iteration index; branch refs get `refs/heads/` prefix; never-updated ref → empty Vec
(NotFound mapped, not an error). No new AppError/events/channels. Accepted architect defaults:
local-only reflogs v1; soft/mixed/hard reset parity (reuse resetMenuItems); auto-refetch after restore;
`message().unwrap_or("")` fallback. Sub-increments: **P38a** core+cmd+IPC+mock+`reflog_cli.rs` oracle →
**P38b** ReflogView overlay + entry points (branch-menu "View reflog", toolbar "View HEAD reflog") +
restore actions. Guardrails unchanged (scratch D:\Temp\bonsai-scratch, TMP/TEMP=D:\Temp on Windows, no
concurrent test+clippy, mock.ts compiling, orchestrator commits).

- **P38a** (reviewer APPROVE, 0 must-fix; index/oid/cap/resolution/read-only all verified) — committed
  `bd84774`. reflog.rs read module + read_reflog cmd + IPC triple + MOCK_HEAD_REFLOG fixture (oids
  overlap graph nodes) + reflog_cli oracle 3 + 5 unit + 1 no-repo cmd. Nits (cosmetic): import ordering.
- **P38b** (reviewer APPROVE, 0 must-fix; safety invariant verified — restore actions only arm the
  shared create-branch/reset dialogs, hard→destructive confirm, newOid target, (root) never a target) —
  ReflogView overlay (index badge, old→new oid, message, committer, relative time, reveal-in-graph,
  per-row kebab) + RepoWorkspace overlay state/reqId-guard/Esc-layering/auto-refetch-after-restore +
  toolbar "↺ Reflog" (HEAD) + branch-menu "View reflog" + HistoryIcon. Deviation (improvement):
  onReset widened to (newOid, mode) → soft/mixed/hard parity. Nits (cosmetic, left): reflogRestoreRef
  not cleared on dialog-cancel (idempotent extra refetch); dead `index` in menu state. tsc + build clean.
- **P38b AI GATE PASSED (2026-08-04).** Browser harness (mock :1420, orchestrator-driven): toolbar
  "↺ Reflog" → overlay lists HEAD@{0..5} (reset/amend/rebase/commit/pull/initial, (root) on the initial
  old oid); row kebab → Create branch here + Reset main to this soft/mixed/hard…; Create branch here →
  branch-name prompt; Reset (hard) → destructive "Reset (hard)" confirm; zero console errors.
- **P38 tester** — full workspace regression PASS, zero regressions: `cargo test --workspace` 813
  passed / 0 failed / 2 ignored (M2 perf-gate, unrelated); remote_cli flaky test PASSED; clippy
  --workspace --tests clean; tsc + build clean. Strengthened reflog_cli → 6 (added: cap-to-newest-2000
  via a synthesized over-cap .git/logs/HEAD, full branch reflog vs `git log -g main`, reset old/new-oid
  direction). Checklist: docs/contracts/P38-user-checklist.md. No impl/contract discrepancies.
- **P38 AI GATE PASSED (2026-08-04).** Commits: bd84774 P38a · 0d5c340 P38b (+ tester closeout).
  Backend reflog oracle + frontend browser harness both verified; zero regressions. Git-completeness
  Phase 1 milestone #2 delivered.

**P38 awaiting USER CHECKPOINT** (native pnpm tauri dev on a SCRATCH repo, per docs/contracts/
P38-user-checklist.md): after commit/amend/hard-reset/rebase, toolbar ↺ Reflog shows entries matching
`git reflog`; "Create branch here" on an older entry creates the branch at its newOid; "Reset (hard)"
moves the branch there after the destructive confirm (Cancel = no-op); branch-menu "View reflog" works;
a no-reflog branch shows an empty placeholder, not an error.
**Current step:** P38 DONE (AI gate passed, awaiting USER CHECKPOINT). Next: P39 (bisect).

## P37 — force-push-with-lease (Git completeness, Phase 1) — **DONE (AI gate passed, awaiting USER CHECKPOINT)** (2026-08-04)

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
