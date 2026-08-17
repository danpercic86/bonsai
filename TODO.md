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

**USER CHECKPOINT BATCH CONFIRMED (2026-08-08):** the user confirmed ALL remaining pending checkpoints in a
single batch ("mark everything as checked"). Every item previously marked "awaiting USER CHECKPOINT" below is
now CONFIRMED as of 2026-08-08 — this clears every outstanding native checkpoint from **P28 through P61**
(incl. P32, P37–P46, the credential-cache + UX-fix batches, Phase 1 P49–P52, Phase 2 P53–P57, and Phase 3
P58–P61). The FOR-USER defaulted decisions (P55 undo-merge = reset-to-first-parent, P57 BM25 retrieval,
P61 hand-rolled base64) were reviewed and **ACCEPTED AS-IS** (no changes requested). The **entire approved
roadmap P49–P61 is now fully DONE** (AI gate + USER CHECKPOINT both passed). Only Phase 4 (forge/PR P62–P64)
and paged-loading P65 remain unbuilt — deliberately out of scope of this grant.

## ✅ FOR USER — RESOLVED 2026-08-08 (decisions accepted + all native checkpoints confirmed)

User granted an overnight autonomous run ("implement everything from the plan; leave things you need from
me to choose/approve tomorrow"), then on 2026-08-08 said "mark everything as checked" — so the defaulted
decisions below are **ACCEPTED AS-IS** and all native checkpoints are **CONFIRMED** (see the batch banner
above). Kept here for the record; nothing outstanding. All items were confirm-gated at runtime or trivially
changed in code.

**Defaulted decisions — ACCEPTED AS-IS 2026-08-08 (change anytime later):**
- **P55 `undoLastMerge` semantics** = reset-to-first-parent (Mixed; REWRITES history), flagged Destructive
  + shared-history warning, shown in a read-only preview before an explicit Confirm. Alt: `revert -m 1`
  (adds a commit, never rewrites). One-line SafeOp mapping to switch, or add a dialog toggle. ← riskiest default.
- **P57 retriever** = BM25 lexical (no embeddings/model download — ASR-safe, zero new deps). Alt: a local
  embedding model (needs a runtime/model download; ties to the model-tier decision).
- **P61 image-diff base64** = HAND-ROLLED (no new `base64` crate) to avoid touching your dirty
  Cargo.lock/Cargo.toml. Swap to the crate anytime.
- **OD1** (already confirmed): AI stays local-`claude`-CLI-only; model tiers deferred.

**Native USER CHECKPOINTs — ALL CONFIRMED 2026-08-08** (user marked everything checked): Phase 1 P49–P52;
Phase 2 P53–P57; Phase 3 P58 (signing) + P59 (hooks) + P60 + P61 (intraline + image diff across
workdir/commit/compare). Checklists retained for reference: `docs/contracts/P<N>-user-checklist.md`.

**Untouched throughout:** your 4 in-progress files (Cargo.lock, package.json, src-tauri/Cargo.toml,
tauri.conf.json = the 0.3.0→0.3.1 bump) and `docs/audit-2026-08-07.md` (another session's file).

## 🚀 PHASE 4 — forge/PR integration + paged loading (P62–P65) — ✅ **COMPLETE** (2026-08-10; P66 deferred)

> **STATUS 2026-08-10:** Phase 4 shipped — **P62** forge foundation (GitHub) · **P63** graph forge signals ·
> **P64** GitLab + Bitbucket + Azure DevOps + AI PR descriptions · **P65** streaming/paged graph loading
> (shipped with an honest first-paint reframe — see the P65/P65c sections + finding). **The entire approved
> roadmap P49–P65 is now code-complete.** One item deliberately deferred by the user: **P66** (lazy
> generation-number topo-order for instant first paint on huge repos — the fix for the P65c finding; spike in
> `docs/contracts/P65a-lazy-topo-spike.md`). Native USER CHECKPOINTs for P62–P65 remain (checklists in
> `docs/contracts/P6*-user-checklist.md`) — the AI gate is green.

Final phase of the approved roadmap `~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`. User
granted autonomous implementation 2026-08-08 ("the other AI finished, you can start implementing
autonomously") after the other AI committed the 5 Phase-4 contracts (`15067e6`) + an audit fix batch.
Started from clean HEAD `6686108`. Sequencing: **forge-first** (P62 → P63 → P64) then P65 (independent perf).

Contracts: `docs/contracts/phase4-forge-overview.md` + `P62`/`P63`/`P64`/`P65-*.md`.
**Command math:** 147 → **157** (P62 +7, P63 +1, P64 +1, P65 +1 — RECOUNT `generate_handler!` at each increment).

**FOR-USER (accepted defaults, autonomous — change anytime):**
- **NEW Rust deps** for forge: `reqwest{blocking,json,rustls-tls}` + `keyring` (OS keychain for the PAT).
  Accepted as default — Cargo files are now clean/committed, so adding is mechanically clean. Alts:
  `ureq` instead of reqwest; `git credential` helper instead of keyring (no keychain guarantee).
  `keyring` pulls a Secret-Service/D-Bus backend on Linux.
- **Auth** = PAT-only v1 (paste → keychain); OAuth device-flow deferred.
- **P64 provider order** = GitLab → Bitbucket → Azure DevOps (Azure may split to P64b/c).
- **OD1** still holds: AI PR descriptions (P64) run local-`claude`-CLI only.

**Cross-contract couplings to reconcile during build:**
- P62 gets a `viewer()` trait method (`forge_set_token` validates via `GET /user`; P64 reuses it).
- P64 later adds `ForgeTarget.project` + the extra provider arms (additive to P62 types).
- P63 later adds an `openToPr?` prop on `PrPanel` + `showPrBadge`/`showCiStatus` `GraphPrefs` fields (additive).
- P65 touches `GraphCanvas` edge handling + `RepoWorkspace.refetchGraph` — land after any in-flight graph-pane work.

### P62 — forge foundation (GitHub first) — **DONE ✅ (AI-gate + harness verified; awaiting native USER CHECKPOINT)**
Contract: `docs/contracts/P62-forge-foundation.md`. New pure crate `crates/bonsai-forge/` + 7 commands +
right-pane PR panel. **+7 cmd (147→154, RECOUNT at impl).** Sub-increments:
- **P62a** — pure `bonsai-forge` crate: `Cargo.toml` (reqwest+keyring), `lib.rs`, `types.rs` (+wire tests),
  `detect.rs` (+table test), `provider.rs` (trait + `viewer()`), `http.rs` (HttpTransport seam + redaction),
  `auth.rs` (keyring TokenStore + in-proc cache), `github/{mod,rest,dto}.rs` (REST v3 + status rollup);
  + 4 `AppError` variants in `bonsai-core/src/error.rs`. Offline tests via a fake transport.
- **P62b** — Tauri `commands/forge.rs` (7 triples) + register + `shared.rs` re-exports; frontend IPC
  (`types.ts` mirrors + 7 `IpcApi` methods + 4 `AppError.kind`, `tauri.ts`, `index.ts`) +
  `mock/handlers/forge.ts` + `fixtures/forge.ts` (offline parity).
- **P62c** — PR panel: `PrPanel` container + `PrList`/`PrListItem`/`PrDetailView`/`PrReviewComments`/
  `PrCreateForm`/`ForgeConnect` + right-pane `'work'|'prs'` tab in `RepoWorkspace`/`WorkspaceRightPanel`.

**Current step:** ✅ **P62 COMPLETE** (P62a crate + P62b IPC/mock + P62c PR panel UI, all committed).
Reviewer approve across all three (0 must-fix); P62c should-fix (commit-draft preserved across tab
switch) + nits folded. **Cmd = 154**. AI gate: `bonsai-forge` 60/0, `bonsai --lib` 117/0, clippy -D clean,
workspace build 0-warn, tsc+build green. Browser harness (`pnpm dev:mock`) verified the full flow:
connect→list (3 open PRs, merged hidden)→detail (labels/mergeable/+−/9 files/3 comments incl. 2 review
w/ path)→create→new #999; `?forge=off`→offline+Retry; commit draft survives the Working↔PRs toggle;
working panel still fills/scrolls; console clean. Native USER CHECKPOINT (real GitHub PAT/PRs) pending.
Next: **P63** — forge signals on graph (PR + CI badges; +1 cmd `forge_commit_statuses`).

### P63 — forge signals on graph — **DONE ✅ (AI-gate + harness verified; canvas visuals = native USER CHECKPOINT)**
- **P63a** ✅ DONE (committed) — batch `forge_commit_statuses` (154→**155**; omits per-sha 404, propagates
  fatal, dedup + cap 100) + `GraphPrefs.showPrBadge`/`showCiStatus` (default off, back-compat) + mock/fixtures
  aligned to graph branch tips (all 5 CheckRollups). bonsai-forge 62/0, bonsai --lib 117/0, clippy -D clean,
  tsc+build, vitest 163. Reviewer approve (per-sha-404 should-fix folded).
- **P63b** ✅ DONE (committed) — `forgeBadges.ts` (pure, 17 tests) + `useForgeSignals` hook (reqId last-wins,
  300ms debounce, TTL 60s, silent-fail, zero-IPC-when-off, bounded `rebuildCiCache`) + canvas PR+CI badges on
  branch-tip pills (toggleable + compact-suppressed, atomic overflow) + click→PR (`onOpenPr`→`PrPanel.openToPr`
  via `PrNavRequest`) + SettingsPanel toggles. Reviewer approve (0 must/should-fix; 2 nits folded). AI gate:
  vitest **197**, tsc 0, build green. Harness: toggles persist, hook runs error-free, `?forge=off` silent.
  Canvas badge pixels + canvas click→PR = native USER CHECKPOINT.

**Current step:** ✅ **P63 COMPLETE** (P63a data/settings + P63b render/interaction, committed). Cmd **155**.
Next: **P64** — more providers (GitLab→Bitbucket→Azure) + AI PR descriptions; recommend split (P64 = AI
descriptions + GitLab, then Bitbucket/Azure as P64b/c).
### P64 — more providers + AI PR descriptions — **DONE ✅ (AI gate passed; native USER CHECKPOINT pending — `docs/contracts/P64-user-checklist.md`)** (contract `docs/contracts/P64-forge-providers-ai-pr.md`)
Split (architect rec): **P64a** AI PR descriptions (Part B, +1 cmd 155→156) → **P64b** GitLab provider
(Part A #1, +0 cmd; adds `ForgeKind` arms + detect host + `ForgeTarget.project` + `viewer()` reuse) →
**P64c** Bitbucket provider + deferred §3e connect hints (+0 cmd) → **P64d** Azure DevOps provider,
closes P64 (+0 cmd). OD1: AI stays local-`claude`-CLI-only.
- **P64a** ✅ DONE (committed) — `ai_generate_pr_description` (cmd **156**) reusing `run_claude` +
  `resolve_digest_range` + `payload::render_*`/`cap_review_payload` + AI consent gate; writes nothing; empty-range
  guard pre-CLI; byte-safe title parse (multibyte). Create-form seam wired shown-but-disabled (contract §4e /
  CommitBox lane; `aiEligible` drives disabled+tooltip). Mock `?ai=off`/`#fail` sentinels. Reviewer approve
  (multibyte-panic must-fix + eligibility-lane should-fix folded). AI gate: bonsai-core 610/1-ign +
  ai_pr_description_cli 3/0, bonsai --lib 118, clippy -D clean, tsc+build, vitest 197. Harness: Generate button
  gated to eligible+range → click fills a structured title/body from mock, no auto-submit, console clean.
  Live real-CLI generation = native USER CHECKPOINT.

- **P64b** ✅ DONE (committed) — GitLab provider: `gitlab/{mod,rest,dto}.rs` on the same trait (MR→PR,
  notes/discussions→comments de-duped, pipeline→CI rollup); shared neutral `crate::rollup` extracted from
  github (GitHub byte-identical + still covered); `ForgeKind::GitLab` + nested-group detection + URL-encoded
  project path; `PRIVATE-TOKEN` auth (redacted); provider-aware `build_provider` (open + validate_token).
  Frontend `ForgeKind+='gitLab'`, PrPanel/useForgeSignals guards `!== 'unknown'`, `?forge=gitlab` mock.
  Reviewer approve (0 must-fix). AI gate: bonsai-forge 93/0 (29 GitLab), clippy -D clean, build, tsc+build+vitest
  197. Harness: `?forge=gitlab` → supported (host-aware connect → paste → PR list renders). Cmd 156.
  ↳ DEFERRED to P64c: per-provider ForgeConnect hint (contract §3e — GitLab `api`-scope guidance + token-help
  link); do all providers' hints in one pass. GitLab 403 msg already mentions the `api` scope as a fallback.

- **P64c** ✅ DONE (committed `509d450`) — Bitbucket Cloud provider: `bitbucket/{mod,rest,dto,dto/tests}.rs`
  on the same trait; `ForgeKind::Bitbucket` + `bitbucket.org` detection; Bearer access-token auth (token only
  in the `Authorization` header, never in a URL/log); body-based pagination (`has_next` from `next`); PR state
  OPEN/MERGED/DECLINED/SUPERSEDED→neutral; inline-vs-general comment partition; build-status→shared
  `crate::rollup`. Delivered the deferred §3e per-provider connect hints (`CONNECT_HINTS Record<ForgeKind,
  ConnectHint>`). Reviewer approve (0 must-fix). **Reviewer correctness fix:** Bitbucket has NO `state=all`
  and defaults to OPEN when omitted → `list_prs` now fans out repeated `state` params (open→OPEN,
  closed→MERGED&DECLINED&SUPERSEDED, all→OPEN&MERGED&DECLINED&SUPERSEDED) — merged PRs were invisible before;
  contract §3c amended. Also split dto.rs 581→395; connect-hint URL→access-token docs. AI gate: bonsai-forge
  **122/0**, clippy -D clean, `cargo check --workspace` green, tsc 0, cmd **156** (unchanged). Harness
  (`?forge=bitbucket`): supported forge, connect→list shows 3 PRs mapped; Working subtree hidden under PR tab.

- **P64d** ✅ DONE (committed `502ce01`) — Azure DevOps provider: `azure/{mod,rest,dto,dto/tests}.rs` on the
  same trait (REST 7.1). Basic auth `base64(":"+PAT)` via a hand-rolled RFC-4648 encoder (no crate dep,
  known-vector tested; PAT only in the `Authorization` header, never in a URL/log — redaction test proves
  plaintext AND base64 absent from Debug); `api-version=7.1` on every URL; `$skip/$top` paging; ref-name
  stripping both ways; cross-host `viewer()` (app.vssps.visualstudio.com); state active/completed/abandoned→
  neutral; `combined_status` by sha→shared `crate::rollup`. 3-part identity via new `ForgeTarget.project` +
  `ForgeRepoContext.project` (`Option<String>`/`string|null`, `None` for the other 3), threaded
  detect→build_provider→coords→repo_context→TS. `detect_azure`+`detect_table_azure` (dev.azure.com / ssh /
  legacy visualstudio.com); `ForgeKind::AzureDevOps` + TS `'azureDevOps'` + CONNECT_HINTS entry; `?forge=azure`
  mock. Reviewer approve (0 must-fix/0 should-fix; base64 traced vs RFC-4648). AI gate: bonsai-forge **153/0/0**
  (+31 Azure), clippy -D clean, `cargo check -p bonsai` links, tsc 0, cmd **156**. Harness (`?forge=azure`):
  supported forge, connect→list shows 3 PRs mapped.
  ↳ P64 polish (committed `699dc2d`): generalized 2 GitHub-specific UI strings for all 4 providers
  (PrDetailView "Open on GitHub"→"Open in browser"; Settings forge-signals hint lists all 4) — reviewer NIT.
  ↳ Reviewer NITs deferred to native checkpoint (in P64 checklist §D): Azure bad-PAT HTTP-203→"malformed"
  (not "auth failed"); `dev.azure.com/{org}/_git/{repo}` shorthand→Unknown.

**Current step:** ✅ **P64 COMPLETE** (P64a AI PR descriptions + P64b GitLab + P64c Bitbucket + P64d Azure
DevOps — all committed, AI-gate-passed, native checkpoint `docs/contracts/P64-user-checklist.md` pending).
**Phase 4 forge/PR (P62–P64) is now code-complete.** Next: **P65** — paged/streaming graph loading
(independent perf milestone; land after graph-pane work). Cmd 156→**157** (P65 +1 `stream_graph`).
### P65 — paged/streaming graph loading — **IN PROGRESS** (contract `docs/contracts/P65-paged-loading.md`)
Decision (contract §0): a single `stream_graph` Tauri channel (Meta→Batch*→Done), NOT stateless `loadMore`
paging — the lane algorithm needs the accumulated walk state, so a channel makes lane-color stability true
by construction. 3 senior-dev passes: **P65a** Rust core (extract shared `LaneWalker`; `compute_graph` output
UNCHANGED; `stream_graph_core` + `stream_graph` channel cmd) → **P65b** frontend (incrementalEdgeIndex +
streamAssembler + GraphCanvas 2 optional props + RepoWorkspace.refetchGraph switches to streamGraph) →
**P65c** mock + harness (20k scroll gate) + 200k first-paint latency test. Architect OQ recs all ACCEPTED:
channel-drop `is_ok()`-break cancellation (OQ1), `Meta.total=None` grow-as-you-go (OQ2), 1M cap + `truncated`
(OQ3), OQ4 bounded/resume deferred, running-max lane width (OQ5), keep `ord` (OQ6). **Command count 156→157**
(contract's "147→148" is design-time stale — recount `generate_handler!`).

- **P65a** ✅ DONE (committed `5568f2f`) — shared `LaneWalker` (`graph/lane.rs`) + streaming core
  (`graph/stream.rs`: StreamNode/GraphStreamEdge/GraphChunk + `stream_graph_core`) + `stream_graph` channel
  command. `compute_graph` output byte-for-byte UNCHANGED (E1–E6 fixtures intact + equivalence test at batch
  sizes 1/2/3/7/512 + 5 serde wire-shape tests guarding the P65b seam). graph.rs test module → `graph/tests.rs`
  (523→470). Reviewer approve (0 must-fix; equivalence verified); both should-fix landed. Cmd **157**. Gate:
  bonsai-core 674/0/1-perf-ignored, bonsai --lib 220/0, clippy -D clean, cargo check --workspace links.

- **P65b** ✅ DONE (committed `e009034`) — frontend + mock end-to-end: TS wire types + tauri Channel bridge +
  `incrementalEdgeIndex.ts` (order-independent, gen-stamped dedupe) + `streamAssembler.ts` (folds chunks →
  GraphLayout, deep-equals getGraph on a complete walk) + GraphCanvas 2 optional props (one-shot path
  unchanged when absent) + refetchGraph streams via streamGraph (last-wins guard, progressive selection
  remap) + mock `streamGraph` (supersede generation). Reviewer request-changes → fixed a real crash
  (3 unguarded `graph.nodes[selectedIndex]` derefs during the mid-stream partial-layout window). Gate: tsc 0,
  build, vitest **1331/0** (+14). Harness (`?fixture=20k`): streamed load fills the 20 001-row extent, canvas
  renders, full-range scroll clean (no console errors).

- **P65c** ✅ DONE (honest reframe) — `crates/bonsai-core/tests/stream_perf.rs`: `#[ignore]` release-gate test
  builds a 120k-commit git2 fixture and asserts full-stream correctness (`total_rows==120000`,
  `truncated==false`, incremental delivery) + PRINTS the measured latencies. It deliberately does NOT assert
  `<150 ms` — that target is unachievable (see finding). Checklist revised to match.
  - **FINDING:** libgit2's `Sort::TOPOLOGICAL` runs an eager `prepare_walk` (full reachable in-degree pass)
    before yielding row 0, so first paint is **O(total commits)** (release/warm: 40k≈0.73s, 120k≈1.37s,
    200k≈2.3s), NOT O(first 512). P65 streaming still delivers lane-stable **progressive** render +
    scroll-ahead + no giant IPC, but "instant first paint on 1M repos" is NOT met by the topo walk.

**Current step:** ✅ **P65 COMPLETE (shipped with an honest first-paint reframe).** P65a (streaming core) +
P65b (frontend+mock) + P65c (correctness test + finding) all committed; native scroll/visual + progressive-load
checkpoints in `docs/contracts/P65-user-checklist.md`. **This completes the entire approved roadmap P49–P65.**
USER decided (2026-08-10): **defer the instant-first-paint fix to a future P66** rather than take on the Large
build now.

### P66 — lazy generation-number topo order (instant first paint on huge repos) — **DEFERRED (approved future work)**
The proper fix for the P65c finding, scoped by the feasibility spike `docs/contracts/P65a-lazy-topo-spike.md`
(VERDICT: TRACTABLE via path (c), effort **L**). Reimplement git's lazy `--topo-order` (Stolee generation
numbers) in Rust as a shared order stage replacing `seeded_revwalk`, sourcing generation numbers from the P52
commit-graph file (git2 0.21 / libgit2-sys 0.18 expose NONE — grepped, 0 hits) via pure-Rust `gix-commitgraph`
or an own parser. Committed P65a/P65b stay as-is (IPC / `GraphChunk` / `LaneWalker` unchanged); only the
internal walk order changes. Costs: one-time regeneration of ALL graph fixtures (lazy order differs from
libgit2 in commit-date TIE-BREAKS only — still "topological, then commit date"), guarded by a new
`get_graph ≡ stream_graph` equivalence test + a differential test vs `git rev-list --topo-order`. Pre-build:
re-verify newest git2 still lacks the gen API (F5); confirm `gix-commitgraph` reads P52 split/chain graphs (F1).
Architect advises AGAINST the `git log --topo-order` shell-out (would make the git binary a hard runtime dep of
the core read path). Sets the deferred `stream_perf.rs` first-batch threshold.

## 🐛 USER-REPORTED BATCH (2026-08-17) — P67 UX polish + P68 AI conflict resolution

User reported 7 issues from real use (real repo, live merge conflict). Plan (user-approved):
`~/.claude/plans/1-the-dotted-line-cozy-llama.md`. Split into two milestones per the user's
sequencing choice — **UX polish first, then AI**. Started from clean HEAD `0ac5444`.

**User's 7 items → milestone mapping:**
1. Dashed HEAD guideline disappears on scroll → **P67 §1** (confirmed bug, not intended)
2. Right panel wastes vertical space vs the changes tree → **P67 §2**
3. "Propose & review" shows no proposals → **P68** (not broken — the proposal opens in the CENTER
   pane, invisible from the right panel; plus item 5 destroys it. No separate work item)
4. Resolve ALL conflicts with AI, not one at a time → **P68 §D**
5. No feedback on AI resolve; result lost when switching files → **P68 §C** (root cause found:
   single-slot `aiResolvingPath` + the SHARED `fileDiffReqId` guard discarding a computed proposal)
6. Claude timed out at 90s on an i18n JSON conflict → **P68 §A/§B** (real cause: `--tools ""` makes
   Claude blind to the repo, so no timeout would help; fix = no-timeout + Cancel + read-only tools)
7. Show AI logs live; let Claude ask questions mid-run → **P68 §B/§E**

**LOCKED USER DECISIONS (asked + answered 2026-08-17; do not re-litigate):**
- AI timeout = **no hard timeout + Cancel button**; idle-output watchdog ~300s; optional hard cap
  configurable, default 0 (unbounded).
- AI visibility = **live log stream + interactive prompts** (user can answer Claude mid-run).
- Log panel location = **bottom dock**, collapsible, full width (does not compete with the space
  P67 reclaims in the right panel).
- Conflict-resolve repo access = **read-only allowlist** `--tools "Read,Grep,Glob"` (no
  write/edit/bash; Bonsai still writes nothing — staging stays an explicit post-review call).
- Bulk resolve = **ONE AI run for all conflicts** (single run sees them together), with per-file
  attribution back into a per-path store.
- Right-panel density = **tighter default AND** a Cozy/Compact toggle.
- "Stash all" = **demote to a `⋯` overflow menu** (keeps all 3 scopes; sidebar keeps 1-click stash).
- Panel density is **independent** of graph Compact rows (cross-reference hint in Settings only).
- Dashed HEAD line = **always visible while scrolling**.

**SPIKE FACTS (verified against installed `claude` v2.1.233 — do not re-verify):**
- `-p --output-format stream-json` **requires** `--verbose`.
- NDJSON order: `system(init)` → `rate_limit_event` → `system(thinking_tokens)` heartbeats →
  `assistant` → `system(post_turn_summary)` (carries `status_category`/`needs_action`) → `result`.
- The `result` line is **byte-compatible** with today's `--output-format json` envelope → the
  existing parse at `ai/mod.rs:332-366` is reused verbatim.
- A turn ends at `result`, **NOT** process exit; with `--input-format stream-json` the child stays
  alive on open stdin and accepts a second turn → this is the interactive mechanism.
- **DEAD END:** the CLI's own `SendMessage` tool cannot ask the user anything — in `-p` mode the CLI
  answers its own tool call and discards an injected `tool_result`. Mid-run questions therefore use
  a prompt-level sentinel `BONSAI_NEEDS_INPUT: <question>`.
- `--tools "Read,Grep,Glob"` is a valid allowlist (init echoes the exact subset).

### P67 — HEAD guideline + right-panel density — ✅ **AI GATE PASSED** (awaiting native USER CHECKPOINT)
Contract: `docs/contracts/P67-ux-polish-batch.md` (+ `P67-user-checklist.md`). **+0 Tauri commands**
(157 unchanged; `panelDensity` rides the existing `set_ui_settings` patch).
- **P67a** — HEAD guideline: new pure `headGuide()` in `src/graph/viewport.ts`; split
  `drawWipRow` (`src/graph/draw.ts:200-212` moves out) into `drawHeadGuide` + `drawHeadEdgeMarker`;
  drop the `scrollTop < rowHeight + 56` gate for the connector at `GraphCanvas.tsx:345`; clamp BOTH
  ends (today only the end is clamped → perf + dash-crawl regressions if the gate is merely
  removed); `selfTest.ts` + `window.__bonsai.p7` seam; `P1-polish.md` §9.3 SUPERSEDED markers.
- **P67b** — right-panel structure + tighter default (~110px reclaimed ≈ 4–5 more file rows): new
  `RightPanelActionsRow.tsx` + `CommitOptionsRow.tsx`, DELETE `StashSplitButton.tsx`, `--rp-*`
  custom properties on `.right-panel` with cozy = the new tighter values (every consumer uses the
  `var(--rp-x, <today>)` fallback form because `.file-row`/`.tree-dir-row`/`.section-label` are also
  rendered OUTSIDE `.right-panel` by DiffFileTree/Sidebar/EmptyState/OnboardingSteps).
- **P67c** — `panelDensity: 'cozy'|'compact'` end-to-end (types → App → SettingsPanel →
  mock persistence → `settings.rs` + `ui_settings.rs` mirror + migration test) + one
  `[data-density='compact']` block.
- **P67d** — contract + user checklist + TODO update.
- **P67e** — `StatusPanel.tsx` 700→~250 split (pure refactor, droppable; `StatusPanel.test.tsx`
  must pass UNTOUCHED — that is the acceptance test).

**Current step:** ✅ **P67 CODE-COMPLETE — AI gate PASSED, awaiting native USER CHECKPOINT**
(`docs/contracts/P67-user-checklist.md`). All five sub-increments committed and reviewer-approved:
**P67a** `0ec69f9` · **P67b** `e607d2c` · **P67c** `5e68db5` · **P67e** `d50361a` · **P67d** = this
docs pass. Next milestone: **P68** (contract being written).

**Baseline before P67** (measured 2026-08-17, so later regressions are attributable): vitest
**1331/0 across 111 files**; cargo workspace green except the one pre-existing load-sensitive flake
noted under P68a. **Final P67 AI gate:** vitest **1361/0 across 112 files**, tsc 0, build green,
`cargo test -p bonsai --lib` **222/0**, `cargo clippy --workspace --tests -- -D warnings` clean,
command count **157 (+0)**.

**MEASURED result for the user's item 2** (not estimated — the contract's ~110px was a guess and
told the implementer to re-measure): `.status-panel` height **452.47 → 568.00 px = +115.53 px ≈ 4.8
file rows** in the cozy default, plus ~14px per two sections inside the scroller (≈129.5px, ≈5.4
rows). Compact adds a further **+30px** (408 → 438px measured live in the harness) ≈ 5.5 rows total.

**Contract amendments written during P67** (all binding, in `P67-ux-polish-batch.md`):
- **A5** — the collapse check ran before `edge`, making `edge:'top'` unreachable whenever a WIP row
  exists (the WIP row is always above HEAD, so scrolling *past* HEAD clamped both ends together and
  drew neither line nor marker). Now suppresses only the segment, never the marker.
- **A6.1** — `dashOffset` sign was inverted. Canvas advances the pattern by `lineDashOffset`, so the
  on-screen grid sits at `y ≡ y0 - off`; content-anchoring needs `off ≡ y0 - anchor`. The inverted
  form is wrong by `-2·(y0-anchor)`, which VARIES WITH SCROLL → ~1px/px dash crawl, and a regression
  versus pre-P67 (which was content-anchored for free via its unclamped start).
- **A6.2** — the crawl guard only asserted 6-periodicity, which passes with the sign inverted; it now
  asserts dash *phase* across 1px scroll steps, and was negative-controlled against the old sign.
- **A6.3** — dropped a redundant `dir === 0` early return that could still suppress the marker for one
  scroll position. ⚠️ `dir === 0` REMAINS REACHABLE and is tested — do not "simplify" it back.
- Acceptance corrections: `headIndex 1500`→`2000` (the original was arithmetically impossible),
  `CommitBox.tsx ≤ ~310`→`≤ ~350` (unreachable from §5.5's own change list), case count 11→14, and
  §1.1a bullet 3 `segment===false`→`true` after implementation measured it.

**Cross-platform fix worth remembering:** `field-sizing: content` (the auto-growing commit box) is
**Chromium-only**. WebView2 has it; macOS WKWebView and Linux webkit2gtk do NOT, and without a guard
the textarea became FIXED and *shorter* than before on those platforms. Guarded with
`@supports not (field-sizing: content) { min-height: 70px }` — **70px, not the pre-P67 declared
60px**, because that 60px was inert: the real pre-P67 height came from `rows={3}` (~70.5px
border-box). Chosen over setting `rows={3}` in the TSX, which risks Chromium honouring `rows` for the
initial height and giving back ~22px of the reclaim on the platform that does support field-sizing.

**Harness gate results (`pnpm dev:mock`, measured live):** cozy `data-density="cozy"` / `--rp-row-h`
24px / `.file-row` 24px+13px; compact `"compact"` / 20px / 20px+12px; toggles both directions;
persists across reload; **`graph.compact` stays `false` while panel density is compact** (the
independence the user asked for, proven empirically); **D8 proven** — the sidebar renders the SAME
`.file-row`/`.section-label` classes but sits outside `.right-panel`, where `--rp-row-h` resolves to
empty, so it kept 24px/11px while the panel shrank. Post-P67e `?op=merge` re-check: section order
Conflicts(2) → Staged → Changes preserved, and the ✨AI button still appears on only the
both-modified row.

⚠️ **NOT self-certifiable:** the harness pane is headless (`requestAnimationFrame` paused, pane not
compositing) so **no canvas pixel is ever produced** and `computer{screenshot}` fails outright. The
dashed guideline's geometry is proven arithmetically + via the `window.__bonsai.p7` seam, but nobody
has SEEN it. Line visibility while scrolling, absence of dash crawl, halo termination, marker
direction, and whether compact is readable on the user's display are all native-only.

### P68 — streaming/interactive/bulk AI conflict resolution — **PENDING** (after P67)
Contract to write: `docs/contracts/P68-ai-conflict-streaming.md` (+ `P68-user-checklist.md`).
**+3 commands: 157 → 160** (`ai_resolve_conflict_stream` = Channel, `ai_cancel_run`, `ai_reply_run`
— RECOUNT `generate_handler!` at impl). Existing `ai_resolve_conflict` stays registered/unchanged,
and the 13 `RunOpts::default()` call sites are deliberately NOT migrated (90s default preserved for
unrelated AI features; streaming is an additive sibling).
- **P68a** Rust runner core (`ai/stream.rs`, `ai/session.rs`, `ai/registry.rs`, `RunLimits`) +
  claude_stub NDJSON modes. Key behaviour change: partial output is KEPT on cancel/watchdog
  (today `ai/mod.rs:180-190` discards it and never joins the reader threads).
  ⚠️ **PRE-EXISTING FLAKE in the code P68a rewrites** (measured at the P67 baseline, 2026-08-17,
  BEFORE any change): `bonsai-core` `ai::tests::run_claude_slow_times_out_and_reaps_child`
  (`ai/mod.rs:577`) asserts the 1s test-override deadline returns "near 1s" and took **2.97s** under
  parallel load — it passes in isolation, fails under a loaded `--workspace` run. Do NOT read it as
  a P68 regression, and prefer replacing the wall-clock assertion with a monotonic lower-bound +
  generous upper bound when the streaming watchdog lands.
- **P68b** streaming conflict resolve + the 3 commands (`commands/ai_stream.rs`, managed
  `AiRunRegistry`, `AppError::AiCancelled`, new settings, read-only allowlist).
- **P68c** TS types + Channel bridge + `mock/handlers/aiStream.ts` (`?aiSlow`/`?aiAsk`/`?aiFail`;
  also finally honour `?ai=off`, which `ai.ts:29` ignores today).
- **P68d** per-path store `useAiRuns.ts` + row feedback; delete `aiResolvingPath`; break the
  `fileDiffReqId` coupling ← **the item-5 fix**.
- **P68e** bottom dock `AiActivityPanel` + `AiActivityLog` (third child of `.workspace-host`).
- **P68f** bulk single-run resolve (`useBulkAiResolve.ts` + `AiRunQueue.tsx`).
- **P68g** Settings UI + contracts + command math + CHANGELOG.

**Known limitations accepted up front:** sentinel-based questions (a model ignoring the convention
degrades to a normal answer, still caught by `hasUnresolvedMarkers`); no re-attach to an in-flight
run after a window reload; `total_cost_usd` may be cumulative across turns (display the last
`result`'s value, don't sum).

---

## P61 — diff quality: word-level/intraline highlighting + image diff (Phase 3 · milestone 4/4, FINAL) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P61-diff-quality.md`. Both backend-computed + React-rendered + opt-in/toggleable.
Adds +1 command (get_image_diff; 146→147; intraline adds NONE). NO OD1 dependency.

**P61 goal:** (a) intraline/word-level highlighting — a backend token-diff pass emits char-offset `spans` on
paired changed DiffLines, gated by a new `intraline:bool` param on the 3 hunk-diff commands (OFF = wire
byte-identical); DiffView/Split render emphasis on changed sub-ranges (intraline WINS over syntax highlight
per changed line; context keeps highlight); "Highlight changes" toggle. (b) image diff — new `get_image_diff`
returns both sides as base64-over-IPC (D2: FORCED by the harness invariant — asset:// can't be mocked;
HAND-ROLLED base64, NO new crate per FOR-USER); DiffImageView side-by-side/onion/swipe; extension-gated
(svg stays text).

**Orchestrator OQ decisions — accept ALL recs + FOR-USER:** OQ1 intraline-wins-per-changed-line (mutually
exclusive w/ syntax highlight) · OQ2 MAX_IMAGE_BYTES=8 MiB · OQ3 SVG=text diff · OQ4 code-point offsets +
Array.from slicing · **OQ5 HAND-ROLL base64 (NO base64 crate — avoids touching the user's dirty Cargo files)**.
D6 no new AppError.

Sub-increments: **P61a** intraline (`intraline.rs` annotate_hunk/token_diff/tokenize/lcs; DiffLine.spans;
intraline param on 3 diff cmds [NO new cmd]; DiffView/Split render + "Highlight changes" toggle;
`intralineSegments.ts`) → **P61b** image diff (`image_diff.rs` get_image_diff + hand-rolled base64; cmd→147;
DiffImageView 3 modes).

- **P61a** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 non-blocking nits) — intraline/word-level
  highlighting. New pure `intraline.rs` (annotate_hunk pairs del/add index-by-index; token_diff = hand-rolled
  LCS over tokens [alnum+_/ws/each-punct]; merge_adjacent; CODE-POINT offsets; MAX_INTRALINE_CHARS=2000 skip;
  pure-add/del→empty). `DiffLine.spans` (skip_serializing_if empty → **wire BYTE-IDENTICAL when intraline=false**,
  serde-tested). `intraline:bool` param on the 3 diff cmds (NO new cmd) — diff-fn caller fan-out
  (ai_*/stage_partial/mcp/tests all pass false). Frontend: `intralineSegments.ts` (Array.from code-point
  slicing) + `intralineContent.tsx` render (emphasis ONLY on changed lines w/ spans; context keeps syntax
  highlight — D5) + DiffOverlay "Highlight changes" toggle (refetch-once) + `intralineMock.ts`. Reviewer
  VERIFIED the multibyte code-point contract (👍 distinguishes from byte AND UTF-16). intraline 9/9 + diff
  15/15 + lib 573; vitest 147/147; clippy -D + build/tsc clean. **Cmd = 147** (lib.rs untouched — my earlier
  "146" label was off-by-one; no new command). Nits: segmentLine zero-len-span dup (unreachable, doc); lcs
  nested-Vec DP (contract-blessed); mock tokenizer exotic-Unicode parity (harness-only).
- **P61b** (reviewer APPROVE, 0 must-fix; 1 should-fix FOLDED; 3 nits) — image diff. New pure
  `image_diff.rs` (`get_image_diff` resolves the old/new blob pair per `ImageDiffRequest` tagged on `kind`:
  Workdir index↔workdir / HEAD↔index · Commit first-parent-tree↔commit-tree, root→old None · Compare
  HEAD-tree↔to-tree; ADD→old None, DELETE→new None, RENAME→old via orig_path; each side >8 MiB→None+`*TooLarge`).
  **HAND-ROLLED RFC 4648 base64** (all 7 RFC vectors + full 256-byte/3-tail roundtrip) — NO new crate, so the
  user's dirty Cargo.lock/Cargo.toml stay untouched. `diff.rs` `commit_trees`+`head_endpoint`→`pub(crate)`
  (visibility only). Tauri `get_image_diff` cmd+`_inner` (spawn_blocking, read-only, emits nothing). Frontend:
  `DiffImageView.tsx` (side-by-side / onion-opacity / swipe-divider; `data:${mime};base64` URLs) +
  `imagePaths.ts` `isImagePath` (svg EXCLUDED). Workdir wired in RepoWorkspace→DiffOverlay (reqId race guard,
  refetch on status change); **commit/compare wired via new `DiffImageCard.tsx`** (the folded should-fix — its
  own local getImageDiff fetch off the DiffBrowser source; the FileDiff bounded queue is left untouched since
  images are `binary:true`). types/tauri/index + mock `handlers/diff.ts` canned decodable 2×2 png pair
  (added./deleted./huge. seams) + styles. image_diff 16/16 (14 in-module + 2 CLI) incl. base64 vectors;
  vitest 155/155; clippy -D + tsc + build clean. **Cmd 146→147** (get_image_diff). Nits: button label
  "Side by side" vs DiffOverlay "Side-by-side" (cosmetic); 3 scoped `.diff-image-card` CSS rules (auto-height
  stacked card); no shared image cache (unmount-on-collapse refetches — matches text card). Committed 68163b6.
- **P61 tester** (no bugs) — 2 git-oracle integration tests (`tests/image_diff_cli.rs`: staged add on unborn
  HEAD → old None; workdir rename via orig_path) + 8 `isImagePath` unit tests (`src/utils/imagePaths.test.ts`:
  raster set, case-insensitivity, svg-excluded, basename-only, dotfile) + native checklist
  `docs/contracts/P61-user-checklist.md`. Full-workspace regression GREEN: cargo test 1113-passed/0-failed
  (3 perf ignored) · vitest 155/155 · clippy --all-targets --all-features -D warnings clean.

**Current step:** ✅ **P61 DONE (AI gate passed, awaiting USER CHECKPOINT).**

---

## 🎉 PHASE 3 (P58–P61) FINISHED (2026-08-08) — per user instruction "when you finish mark in TODO that phase 3 finished"

**Phase 3 (correctness & parity) COMPLETE** — all four milestones AI-gate-passed, awaiting a batch of
native USER CHECKPOINTs:
- **P58** real commit signing (SSH+GPG) + signature verification + verified badge
- **P59** git hooks execution (pre-commit/commit-msg/post-commit + push) + force-push atomic-lease hardening
- **P60** parity batch: branch rename · non-FF pull (merge/rebase, confirm-gated) · one-click undo · submodule add/deinit/remove
- **P61** diff quality: intraline/word-level highlighting + image diff (side-by-side/onion/swipe)

**This completes the entire approved roadmap `~/.claude/plans/do-thorough-analysis-of-purrfect-moth.md`
through P61** (Phase 4 forge/PR P62–P64 and paged-loading P65 were listed but scheduled *after* P61;
they were never part of this autonomous grant). Phase 1 (P49–P52) + Phase 2 (P53–P57) + Phase 3 (P58–P61)
= **all AI gates green**. The only outstanding work is the batched native USER CHECKPOINTs
(`docs/contracts/P{49..61}-user-checklist.md`) which require `pnpm tauri dev` + real CLI/keys and
cannot be self-verified. Command count: **147**.

## P60 — parity batch: rename · non-FF pull · undo · submodule add/deinit/remove (Phase 3 · milestone 3/4) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P60-parity-batch.md`. 4 independent table-stakes items, max reuse of shipped
primitives. Adds +5 commands (141→146; non-FF pull adds NONE). NO OD1 dependency.

**P60 goal:** (a) branch rename (git2 `Branch::rename` — preserves upstream+reflog; `wasHead`); (b) non-FF
pull (WouldNotFastForward gains `upstream`; frontend NonFfPullDialog offers Merge/Rebase → REUSE
merge_branch/rebase_branch, NO new git logic); (c) one-click undo (READ-ONLY `describe_last_undo` classifies
HEAD reflog[0] → UndoPlan{kind,target,mode,safety}; execution reuses reset_branch; mixed for commit/amend/
reset, hard for merge/rebase/ff/cherry-pick/revert requiring a clean worktree); (d) submodule add (git2 +
acquire_cred) / deinit / remove (shell-out via GitRunner, path after `--`).

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 branch-switch undo OUT of v1 · OQ2 submodule
add via git2 · OQ3 hard-undo on dirty → undoable:true + requiresCleanWorktree (show plan, block button "stash
first") · OQ4 amend-undo loses the amended message (dialog says so). D5 no new AppError; D6 mutations don't
emit repo-changed (frontend refetches).

Sub-increments: **P60a+b** rename (`rename_branch` cmd→142) + non-FF pull (PullResult.upstream field +
NonFfPullDialog, no cmd) → **P60c** undo (`undo.rs` describe_last_undo cmd→143 + Undo toolbar + UndoDialog
reusing resetBranch) → **P60d** submodules (add/deinit/remove cmds→146 + menu). Command counts RELATIVE —
recount vs lib.rs (base 141).

- **P60a+b** (reviewer REQUEST-CHANGES → 2 SHOULD-FIX FOLDED → approve; nits) — rename + non-FF pull.
  P60a: `rename_branch` (git2 Branch::rename non-force; validate-new-first; was_head pre-capture;
  Exists→BranchExists/NotFound→BranchNotFound; upstream re-read + PRESERVED) + RenameBranchResult; cmd
  (141→142); IPC + mock; "Rename…" in branchMenuItems + (FOLDED FIX1) the current-HEAD pill fallback → the
  checked-out branch is now renamable (graph pill; wasHead refresh reachable). FOLDED FIX2: same-name submit
  = clean no-op. rename 7/7 + CLI oracle (upstream survives, wasHead). P60b: PullResult.WouldNotFastForward
  +`upstream` (derived from resolved post-fetch upstream, NOT recomputed; backend does ONLY fetch+FF);
  NonFfPullDialog (Merge/Rebase/Cancel) is the confirm gate → REUSES mergeBranch/rebaseBranch outcome
  handlers (no new git logic); `?remote=rebaseconflict` seam. pull-diverged oracle returns upstream==`@{u}`.
  clippy -D + tsc/build clean; cmd 142. Nits: sidebar HEAD-row rename parity (TODO); dead mock
  upstream-fallback; branches.rs 2197 lines (pre-existing).
- **P60c** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 cosmetic nits) — one-click undo. New READ-ONLY
  `undo.rs`: `classify()` (prefix truth-table, first-match-wins: commit(amend)→Amend before commit→Commit;
  merge/pull-merge→Merge; ff→FastForward; rebase/cherry-pick/revert/reset; checkout-moving→BranchSwitch) +
  `describe_last_undo` (target=reflog[0].oldOid; MIXED for commit/amend/reset, HARD for merge/rebase/ff/
  cherry-pick/revert; zero-oid/empty/branchswitch/unknown→undoable:false; hard+dirty→undoable:true+
  requiresCleanWorktree; worktree_dirty tracked-only [hard reset preserves untracked]). Reviewer CONFIRMED
  zero mutation in production (only #[cfg(test)]). `describe_last_undo` cmd (142→143; read-only). Undo toolbar
  button + `UndoDialog` (disabled on !undoable/reason or requiresCleanWorktree&&dirty; hard=destructive; amend
  message-discarded note) → execution REUSES `resetBranch(targetOid, resetMode)` (no new mutation command).
  IPC + mock (mirrors classifier; `?undo=` seam). 10 tests (truth-table + wire + CLI oracle
  commit→merge→reset→branch-switch, target==`HEAD@{1}`); clippy -D + build/tsc clean. Nits: `pull <ref>:
  Merge`→fastForward (both Hard→same reversal); Undo in toolbar-center; summary shows raw prefix.
- **P60d** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — submodules. `add_submodule` (git2
  Repository::submodule + acquire_cred clone + init(false) + add_finalize; validate_rel_path; blank→
  InvalidName) / `deinit_submodule` (git submodule deinit -f -- <path>; clears config + empties worktree,
  RETAINS .gitmodules) / `remove_submodule` (deinit → git rm -f -- <path> → best-effort rm .git/modules/<n>;
  drops gitlink+.gitmodules+worktree) — deinit/remove via GitRunner shell-out, path=git's own sm.path() as
  the FINAL token after `--` (reviewer: injection-safe). Pure deinit_args/rm_args. 3 cmds (143→146); none
  emit repo-changed. list_submodules refactored onto a shared submodule_info builder (byte-identical). IPC +
  mock (moved to submodules.ts). Frontend: Sidebar submodule section + "+"; Add(url+path)/Deinit(confirm)/
  Remove(DESTRUCTIVE). submodule 9/9 + CLI oracle roundtrip (parity w/ real git) + argv injection test;
  clippy -D + build/tsc clean. Nits: add-traversal→Other (doc); always-shown empty section; empty .git/modules parent.
- **P60 tester** — orchestrator-verified (the tester's cargo run was interrupted at the Agent boundary; it
  wrote only the checklist): `cargo test --workspace` GREEN (0 failed; bonsai 110 + bonsai-core 563 lib +
  all integration; 3 perf gates ignored), vitest **139/139**, clippy -D clean. Wrote
  `docs/contracts/P60-user-checklist.md`.
- **P60 AI GATE PASSED (2026-08-08).** Backend workspace green incl. rename 7 + CLI oracle (upstream
  preserved, wasHead), undo 10 (truth-table + CLI oracle), submodule 9 + add/deinit/remove roundtrip oracle
  + argv injection test, pull-diverged oracle (upstream==@{u}) + reviewer verification of all 4 (current-
  branch rename reachable; non-FF pull = NO backend merge/rebase; undo READ-ONLY + safe mixed/hard
  classifier; submodule shell-out injection-safe). vitest 139. Native rename(current branch) / non-FF-pull-
  vs-real-remote / undo-dialogs / submodule-add-from-URL = USER CHECKPOINT (`docs/contracts/P60-user-checklist.md`).
  Commits `869ef8c`(a+b) · `92125bb`(c) · `f72d056`(d) · tester.
**Current step:** P60 DONE (AI gate passed, awaiting USER CHECKPOINT). Phase 3 FINAL milestone: **P61 (diff
quality: intraline/word diff + image diff)** — then mark **Phase 3 finished** (per user instruction).

## P59 — git hooks execution + force-push-lease hardening (Phase 3 · milestone 2/4) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P59-hooks-and-lease-hardening.md`. Reuses the P58 `exec.rs` seam. Two trust fixes.

**P59 goal:** (a) run pre-commit/commit-msg/post-commit (+pre-push) hooks via `git hook run` (Git ≥2.36)
around the existing commit/merge/push ops — a failing BLOCKING hook shows its output + blocks (NEVER a
silent success); per-repo `bonsai.runHooks` opt-out + per-action `skip_hooks` (≡ --no-verify). (b) rewrite
force_push_with_lease to git's atomic `git push --force-with-lease=<ref>:<expected> --force-if-includes`
(closes P37's client-side TOCTOU). +1 AppError `HookRejected` (carries hook output for a dialog); NO new
command (skip_hooks param on commit/amend/merge/push; toggle via read_config/set_config; force_push
signature unchanged). Cmd stays 141.

**Orchestrator OQ decisions — accept ALL architect recs:** OQ-A1 require Git ≥2.36 (else a one-time error
if hooks exist — never silent bypass) · OQ-A2 add HookRejected · OQ-A3 include pre-push (P59a-2) · OQ-A4
"Run git hooks" repo-scoped Settings row via config commands · OQ-B1 include --force-if-includes · OQ-B2
accept git's credential helper for the lease op (price of the atomic guarantee; same never-prompt helper as reads).

Sub-increments: **P59a** commit-hooks backend (`hooks.rs` run_hook/run_hook_nonblocking/hooks_enabled +
pure builders; commit.rs/amend/merge.rs orchestration [pre-commit→reload→commit-msg→create→post-commit];
`HookRejected`; commands `skip_hooks`; IPC types + mock + §A6 oracle) → **P59a-ui** frontend (HookOutputDialog
+ "Commit anyway" retry; CommitBox "Skip hooks"; Settings "Run git hooks" toggle) → **P59b** remote.rs
hardening (pre-push in push_current + force_push_with_lease; lease rewrite to git --force-with-lease;
build_force_push_args/classify_push_stderr; force_push_cli oracle). `skip_hooks` = 2nd mechanical caller
fan-out (like P58 sign; all pass false) — flag a future `CommitOpts` struct.

- **P59a** (reviewer APPROVE, 0 must-fix; 2 should-fix, 5 nits) — commit-hooks backend. New `hooks.rs`
  (`run_hook` = `git hook run <name>` [--to-stdin][-- args]; non-zero→HookRejected w/ output, missing/exit0
  →Ok, spawn-fail→Git, NEVER silent success; `run_hook_nonblocking` post-commit; `hooks_enabled`; Git<2.36
  fallback = block-if-hook-present else proceed; pure build_hook_run_args). commit.rs/amend/merge.rs
  orchestration (pre-commit→index.read(true)→commit-msg[temp file, re-read→EmptyMessage]→create[signed/
  unsigned]→post-commit-nonblocking); resolve_signature ConfigMissing still before writes; hooks-off path
  byte-identical. `AppError::HookRejected` (kind hookRejected). `skip_hooks` param — 2nd 100-caller fan-out
  (all pass false; TODO(P59) fold sign+skip_hooks into CommitOpts). IPC + mock (`?hooks=fail`/`#hookfail`).
  Reviewer VERIFIED the trust-critical pre-check (core.hooksPath set→always delegate; unset→correct
  commondir/hooks incl. worktree; never skips a real hook). **ORCHESTRATOR FIX applied:** compose_apply
  passes skip_hooks=TRUE (a re-staging pre-commit hook would break the composer's file-level partition via
  index.read(true); MCP/normal commits keep hooks-on). Hooks oracle 12 RAN (git 2.51); clippy -D + build/tsc
  clean; cmd 141. Notes: merge.rs 1289-line god-file (pre-existing); clean auto-merge runs no hooks (contract
  gap; comment→P59a-ui); post-commit output dropped (surface in P59a-ui); hooks.rs stat-error→Skip nit.
- **P59a-ui** (reviewer APPROVE, 0 must-fix; 1 should-fix FOLDED, 2 nits) — hooks frontend. New
  `HookOutputDialog` (on ConfirmDialog; hook name + scrollable monospace output; "Commit anyway (skip
  hooks)" primary), `useHookGate` (runWithHookGate parks ONLY on hookRejected — other errors pass through;
  retry re-runs the SAME attempt with skipHooks:true preserving message/mode/sign + resolves the original
  submit; cancel → COMMIT_HOOK_CANCELED keeps the message, no banner), `SettingsHooksToggle` ("Run git hooks"
  → bonsai.runHooks Local via get/setConfig; unset⇒ON). CommitBox "Skip hooks" checkbox threaded through all
  commit/amend/merge/push paths (incl. set-upstream resolver). **FOLDED the should-fix:** onHookSkipRetry
  nulls gateRef BEFORE the async retry → a concurrent Cancel no-ops (fixes the cancel-during-retry
  double-settle). Frontend-only; tsc/build clean. Nits: skip-hooks checkbox persists across commits (like
  P58c sign — consider per-commit reset); composer path ungated (intentional — P59a composer skips hooks).
- **P59b** (reviewer REQUEST-CHANGES → 1 MUST-FIX fixed → approve; 1 should-fix, 3 nits) — force-push-lease
  hardening. `force_push_with_lease` rewritten to git's ATOMIC `git push --force-with-lease=<ref>:<expected>
  --force-if-includes` (closes P37's client-side TOCTOU); git2 resolution + UpToDate short-circuit preserved
  (before any spawn); `runner: &dyn GitExec`; pure build_force_push_args + classify_push_stderr (lease-refuse
  → PushRejected first, wrapped w/ lease_moved_msg). NO IPC/mock/UI change; cmd unchanged. **CRITICAL CATCH:**
  the senior-dev DROPPED the leading `+` from the refspec — reviewer EMPIRICALLY CONFIRMED `+` is an
  unconditional force that OVERRIDES --force-with-lease (would bypass the lease entirely); no-`+` makes it
  conditional (refuse on stale, force when held). **MUST-FIX FIXED (orchestrator):** `SpawnGitExec` now
  injects `-c core.askpass=` (GIT_TERMINAL_PROMPT=0 + askpass-env-removal did NOT cover a CONFIGURED
  core.askpass — P59b is the first credential-requiring push through the seam; a GUI askpass could pop a
  hidden dialog/hang on a destructive op). force_push_cli 9/9 (A lease-refuses+origin-unchanged, B held-lease
  non-ff succeeds, C up-to-date no-spawn via PanicExec, D/E pre-checks) + remote unit 26/26; full bonsai-core
  542+oracles green after the shared-seam fix; clippy -D + build clean. Notes: dropped NoRemote pre-check
  (→Git); remote.rs 1593-line god-file (future split); classify network `ssl`/`tls` substrings broad.
  ⚠ **CONTRACT BUG:** P59 §B2 pseudocode's `+`-refspec would bypass the lease — the CODE is correct (no `+`).
- **P59a-2** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — pre-push. `push_current` +
  `force_push_with_lease` run `pre-push` (hooks::run_hook, args=[remote,url], stdin=per-ref
  `<lref> <loid> <rref> <roid|zeros>`) BEFORE the push; non-zero→HookRejected, abort; gated skip_hooks/
  bonsai.runHooks. **Double-run fix:** force_push_with_lease pushes via the git binary (fires pre-push
  itself) → we run the hook ourselves (HookRejected UX + skip semantics) + `--no-verify` so git doesn't
  re-run → hook runs EXACTLY once; skip_hooks=true truly skips (oracle: ref advances w/ failing hook
  present). push_current (libgit2, no hooks) → only our run. push/force_push cmds +skip_hooks + &SpawnGitExec
  (~20-site fan-out). IPC push/forcePush skipHooks?; mock `?hooks=failpush`. Push-gate: push/forcePush →
  useHookGate → HookOutputDialog "Push anyway" (pushed OUT of the commit gate → sequence not nest). lib 544
  + force_push_cli 12 + remote_cli 20; pre-push oracle RAN; clippy -D + build clean. **Cmd = 141** (lib.rs
  untouched; "142" was a #[tauri::command]-attr miscount). Nits: remote_url_of uses fetch-url not pushurl;
  COMMIT_HOOK_CANCELED reused for push (stale name); opLabel unused (label infers 'pre-push' prefix — 3-way coupling).
- **P59 tester** — `cargo test --workspace` **1071 passed / 0 failed / 3 ignored**; vitest **139/139** (+12
  `hooksGate.test.ts`: pure gate helpers + the load-bearing `pre-commit`/`pre-push hook failed:` prefixes —
  also hardens the reviewer's label-coupling nit); clippy -D clean. Wrote `docs/contracts/P59-user-checklist.md`.
  No bugs.
- **P59 AI GATE PASSED (2026-08-08).** Backend: 1071 workspace incl. the hermetic hooks oracle (12:
  fail-blocks-with-output/HEAD-unchanged, commit-msg rewrite, post-commit non-blocking, core.hooksPath,
  opt-out, re-stage) + force_push_cli (12: atomic lease refuse+origin-unchanged / held-lease succeeds /
  up-to-date no-spawn / pre-push skip) + remote_cli (20) + reviewer verification (pre-check never skips a
  hook; --no-verify double-run fix; the no-`+` lease empirically confirmed; -c core.askpass= never-prompt)
  + hooksGate 12 vitests. Real hook managers (Husky/pre-commit/lint-staged/gitleaks) blocking + dialog
  legibility + Windows shell hooks + real-remote force-with-lease atomic-refuse + no-prompt creds = USER
  CHECKPOINT (`docs/contracts/P59-user-checklist.md`). Commits `87a49c9`(a)·`b17ae64`(ui)·`69d7f16`(b)·`83fe64e`(a-2)·tester.
**Current step:** P59 DONE (AI gate passed, awaiting USER CHECKPOINT). Phase 3 milestone 3/4 next: **P60
(parity batch: rename / non-FF pull / one-click undo / submodule add-deinit-remove)**.

## P58 — real commit signing + verification (Phase 3 · milestone 1/4) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P58-commit-signing.md`. Phase 3 (correctness & parity) — NO OD1 dependency.
Lights the P51 verified-badge stub.

**P58 goal:** sign commits at creation (SSH-first + GPG, via `git commit-tree -S` + `git update-ref` —
git2 keeps the M3 path/guards, the git binary does the crypto in BOTH formats; unsigned path byte-identical)
following `commit.gpgsign` + a per-commit `sign` override; verify signatures (`git log --format=%G?…`, one
subprocess per visible batch) to LIGHT the P51 badge + a CommitPanel signature line + a CommitBox "will sign"
indicator/toggle. Adds `verify_commits` + `signing_status` (139→141); `commit`/`commit_amend`/`commit_merge`
gain a `sign` param (no new cmd). New shared `exec.rs` git seam (reused by P59). New `GraphPrefs.showSignatureBadge`
(default true, toggleable). NATIVE USER CHECKPOINT (real keys — SSH is AI-gate-testable hermetically; GPG native).

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 mechanism C (commit-tree -S + update-ref) ·
OQ2 openpgp-no-key → git selects by committer email (only ssh requires a key) · OQ3 dedicated signing_status ·
OQ4 sign merge commits too (in P58a) · OQ5 update-ref ref-move (CAS old-oid) · OQ6 UI sends explicit bool ·
OQ7 badge glyphs green-check/neutral-hollow/amber-warn/none (final look = checkpoint) · OQ8 oid cache, drop
on Refresh + after commit.

Sub-increments: **P58a** signing backend (`exec.rs` GitExec/SpawnGitExec; `signing.rs`
resolve_signing/create_signed_commit/signing_status; commit.rs/merge.rs `sign` param + signed branch;
`signing_status` cmd →140) + IPC + mock + SSH+config oracle → **P58b** verification (`verify_commits` +
build_verify_args/parse_verify_output/map_status_code; cmd →141) + oracle → **P58c** frontend (light badge +
panel line + sign toggle + `showSignatureBadge` pref).

- **P58a** (reviewer APPROVE, 0 must-fix / 0 should-fix; 4 nits) — signing backend. New `exec.rs`
  (GitExec/SpawnGitExec: never-prompt env + GIT_ASKPASS/SSH_ASKPASS removed → locked agent fails fast,
  CREATE_NO_WINDOW, argv vector no shell injection) + `signing.rs` (resolve_signing; create_signed_commit
  = `git commit-tree -S` + `git update-ref` CAS, mechanism C; signing_status). commit.rs/merge.rs +`sign:
  Option<bool>` — **unsigned path reviewer-verified BYTE-IDENTICAL** (signing branch only when resolved
  true; resolve_signature/ConfigMissing before any spawn; no-gpgsig asserted via cat-file). ssh+no-key →
  ConfigMissing (before spawn); openpgp-no-key → git decides. `signing_status` cmd (139→140). ~80-caller
  `sign` fan-out — all PRODUCTION sites (compose_apply, bonsai-mcp ×2, clean-merge auto-commit) pass None
  (follow config); staging/merge thread the user's sign. SSH oracle RAN (ssh-keygen 2.51): signed commit
  verify-commits + has gpgsig; signed amend preserves author+date. cargo -p bonsai-core 918 pass; clippy -D
  + build/tsc clean. Nits: extract signing.rs oracle (562 lines) → tests/signing_cli.rs in P58b; behavior
  delta (correct D3): composer/MCP commits now sign when commit.gpgsign=true; cherry-pick/revert/rebase
  still unsigned (out of scope, follow-up).
- **P58b** (reviewer APPROVE, 0 must-fix; 1 should-fix→P58c) — verification. `signing.rs` +verify_commits
  + pure build_verify_args/parse_verify_output/map_status_code (`git log --no-walk
  --format=%H%x1f%G?%x1f%GS%x1f%GK`; map G/U/B/X/Y/R/E/N; splitn(4) preserves spaces in signer/key; empty
  oids→no spawn; non-hex dropped; wholesale git-fail→CannotCheck-for-all, never Err; MAX_VERIFY_BATCH=512).
  Extracted the P58a oracle → `tests/signing_cli.rs` (signing.rs 562→497, under limit; no coverage lost).
  `verify_commits` cmd (140→141; read-only). IPC + mock (deterministic per-nibble, all 8 badge states).
  signing lib 8 + signing_cli 12 (verify oracle RAN); clippy -D + build/tsc clean. Finding: SSH-without-
  allowed-signers = N/Unsigned (real git behavior), faithfully mirrored. **SHOULD-FIX → fold into P58c:**
  add `--ignore-missing` to build_verify_args (a valid-hex-nonexistent oid — e.g. stale layout post-rebase
  — currently fatal-exits git → whole batch degrades to CannotCheck; the flag makes "unresolvable omitted"
  true) + extend the oracle with a real nonexistent 40-hex oid.
- **P58c** (reviewer APPROVE, 0 must-fix; 1 should-fix = doc reconciliation) — frontend + the P58b fold.
  Folded `--ignore-missing` into `build_verify_args` (valid-hex-nonexistent oid now per-oid omitted, not
  whole-batch CannotCheck) + oracle extended (real+ghost oid). GraphPrefs `showSignatureBadge` (default
  true, back-compat test). New `useCommitVerification` (oid cache; only UNCACHED visible oids; debounce
  150ms; ≤MAX_VERIFY_BATCH; reqId last-wins; refresh() on Refresh + after commit) + `verifyBadge.ts`
  (shared glyph kind + label). GraphCanvas verifyStatus prop + onVisibleRangeChange (lastRangeRef guard);
  drawBadge state-aware (OQ7: good=filled check, bad/expired/…=warn, unsigned=nothing; slot geometry
  UNCHANGED from P51). CommitPanel signature line; CommitBox sign toggle (default=signingStatus.enabled;
  explicit bool OQ6; no-key warning); SettingsGraphSection "Signature badge" checkbox. Reviewer VERIFIED
  the D4 double-gate: showSignatureBadge=false → ZERO verifyCommits requests AND faint-stub draw (identical
  to P51). cmd 141 unchanged; signing 8 + signing_cli 12 + settings 40 + vitest src/graph 33; clippy -D +
  build/tsc clean. **SHOULD-FIX (deferred to USER CHECKPOINT — glyph = perception call):** the unknown/
  cannotCheck badge is drawn SOLID neutral (not "hollow" per OQ7) — intentional (a hollow neutral would be
  indistinguishable from the P51 faint not-yet-checked stub); `verifyBadge.ts:13` comment says "hollow" and
  is now stale → reconcile when the user confirms final iconography. Nits: hardcoded #ffffff badge fg;
  external (watcher) commits at an unchanged visible range show the faint stub until scroll/Refresh (OQ8-consistent).
- **P58 tester** — `cargo test --workspace` **1045 passed / 0 failed / 3 ignored**; vitest **127/127** (+18
  `verifyBadge.test.ts`: all 8 VerifyStatus → badge kind + label + exhaustiveness); clippy -D clean. Wrote
  `docs/contracts/P58-user-checklist.md`. Findings (non-blocking): (1) stale `verifyBadge.ts:13` "hollow"
  comment (solid is intentional; user confirms glyphs at checkpoint); (2) PRE-EXISTING rustdoc warning
  `ai_compose.rs:180` (P54 em-dash in a doc code-block; non-failing, clippy clean — cosmetic, fix opportunistically).
- **P58 AI GATE PASSED (2026-08-08).** Backend: 1045 workspace incl. the HERMETIC SSH sign oracle (RAN:
  signed commit git-verify-commits, unsigned byte-identical) + verify/config/ConfigMissing/--ignore-missing
  + GraphPrefs back-compat + reviewer verification (unsigned byte-identical, create_signed_commit correct,
  D4 double-gate, ~80-caller fan-out clean); frontend: verifyBadge 18 vitests + reviewer wiring. Real SSH+GPG
  signing, the verified-badge VISUALS (+ OQ7 glyph confirm), sign toggle, missing-key error, cross-platform
  no-console = USER CHECKPOINT (`docs/contracts/P58-user-checklist.md`; GPG native-only, badge pixels
  headless-unverifiable). Commits `14a869d`(a)·`<b>`·`7f0514f`(c)·tester (see `git log`).
**Current step:** P58 DONE (AI gate passed, awaiting USER CHECKPOINT). Phase 3 milestone 2/4 next: **P59
(git hooks execution + force-push-lease hardening)** — reuses the P58 `exec.rs` seam.

## P57 — semantic commit-history search (Phase 2 · milestone 5/5) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P57-semantic-history-search.md`. OD1 = local-`claude`-CLI-only. Highest build
cost; Phase-2 FINAL. **Retriever = BM25 v1 (embeddings DEFERRED per OD1 / FOR USER).**

**P57 goal:** ask an NL question about history → prose answer grounded in REAL commit diffs + ranked
commits (jump-to-graph). 3-stage: persisted per-commit BM25 doc index (app_data_dir, NOT .git;
incremental; schema-invalidated; progress channel) → pure-IR retrieval → local-claude synthesis
re-fetching real diffs for the top-K. Complements P50 (does NOT touch search.rs). Adds 4 commands (135→139).

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 BM25 v1 (embeddings deferred — user's call,
flagged in FOR USER) · OQ2 app_data_dir/history-index/<repo-hash>/store.json serde-JSON atomic · OQ3
missing-index→AiFailed (no new error variant) · OQ4 plain BM25 (k1 1.2/b 0.75) + MSG_BOOST=3 · OQ5
history_search its own command · OQ6 dedicated HistorySearchPanel · OQ7 immutable oid-keyed docs + Rebuild ·
OQ8 git2-only extraction · OQ9 reuse `seed_all_refs` via a pub(crate) visibility bump (DEFER the full
`git/refs.rs` extraction — minimize overnight churn to working code) · OQ10 load-per-query.

Sub-increments (strictly ordered): **P57a** index builder + persistence + status + progress channel
(`history_index/{mod,doc,bm25,store}.rs`; `history_index_build`[channel] + `history_index_status` →137) →
**P57b** retrieval (`search_history` + `history_search` →138) → **P57c** AI synthesis (`ai_history.rs`) + UI
(`HistorySearchPanel`/`HistoryResultsList`/`useHistorySearch`; `ai_search_history` →139).

- **P57a** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — index builder. New 4-file
  `history_index/` module (mod 500 / doc 443 / bm25 205 / store 207): PURE Okapi BM25 (k1 1.2/b 0.75,
  non-neg idf, MSG_BOOST=3 — reviewer verified exact), `extract_doc` (git2 first-parent diff, byte-capped
  line sampling, paths always tokenized, binary skipped, never stores raw diff), incremental build
  (absent-only, immutable docs) + schema invalidation + atomic JSON persist under app_data_dir (NOT .git)
  keyed by FNV-1a repo hash, staleness via tip-set compare. `history_index_build` (channel, mirrors
  clone_repo) + `history_index_status` cmds (135→137; not AI-gated; no repo-changed). OQ9: `seed_all_refs`
  → pub(crate) reused (+ collect_tip_hexes). No new dep (FNV hand-rolled; git2-only). IPC + mock
  (`?historyFail`). 21 tests; clippy -D + build/tsc clean. Nits→P57b: uses `stage::open_workdir_repo`
  (bare→wrong "cannot modify index" msg; unreachable) → switch to `open_repo_at`; index_status loads full
  store (OQ10 defer); tf/df HashMap non-byte-deterministic (value-equal, fine).
- **P57b** (reviewer APPROVE, 0 must-fix / 0 should-fix; 4 optional nits) — retrieval. New
  `history_index/search.rs` (279): `HistoryQuery`/`HistoryHit`/`HistorySearchResults` + `search_history`
  (loads store — no-store/schema-mismatch → empty+indexStale; empty text → empty; SAME `doc::tokenize`;
  reuses `bm25::rank`; top_k 0→DEFAULT_TOP_K clamp MAX_TOP_K; touches no git). `history_search` cmd
  (137→138; pure IR, not AI-gated, no repo-changed). Folded the P57a nit: `build_index`/`index_status` now
  use bare-agnostic `open_repo_at`. IPC + mock. 28 history_index tests; clippy -D + build/tsc clean. Nits
  (optional): mod.rs 530 (non-test ~285); duplicate open_repo_at (OQ9 defer); redundant trim; mock tokenizer naive.
- **P57c** (reviewer REQUEST-CHANGES → 1 MUST-FIX fixed → approve; 3 nits) — AI synthesis + UI. New
  `ai_history.rs` (`answer_history`: retrieve top-K → no-store/zero-hits → AiFailed BEFORE CLI → re-fetch
  REAL first-parent diffs for top SYNTH_DIFF_K=8 → §3.5 grounding (QUESTION/RELEVANT COMMITS/TOP MATCHES
  with MESSAGE+CHANGES=real diffs) → run_claude → parse_cited 7-hex-prefix-of-retrieved; `HistoryAnswer`).
  `ai_search_history` cmd + _inner (consent gate before repo_path, read-only; 138→139). UI:
  `HistorySearchPanel`+`HistoryResultsList`+`useHistorySearch` (reqId last-wins; matchRows reuse the single
  GraphCanvas prop; answer in shared AiOutputPanel; Ask-AI gated aiEligible && built); distinct
  `historySearchOpenRef` Esc-layer (below P50; ≠ P23d file-history); palette "Ask history…". Mock
  aiSearchHistory (?ai=off). **MUST-FIX (reviewer-caught, FIXED):** `ai_search_history_inner` did
  `top_k.clamp(1,MAX)` → UI's topK:0 grounded the answer on only 1 commit (mock's 0→20 hid it); fixed to
  `resolve_ai_top_k=top_k.min(MAX)` (0→DEFAULT_TOP_K=20) + unit test (0→0, fails vs old clamp) +
  ai_history_cli test (top_k=0 retrieves all 3 matching, not 1). Folded matchRows staleness nit. 6
  ai_history + 28 history_index tests; clippy -D + build/tsc + vitest 109 clean; cmd 139. ⚠ contract §4
  pseudocode has the same latent `max(1)` bug — noted (the code is correct).
- **P57 tester** — `cargo test --workspace` **1024 passed / 0 failed / 3 ignored** (perf gates); vitest
  **109/109**; clippy -D clean. Wrote `docs/contracts/P57-user-checklist.md`. Optional frontend vitest
  SKIPPED (score-bar/progress helpers module-private; no exported pure fn without a refactor/RTL dep). No bugs.
- **P57 AI GATE PASSED (2026-08-08).** ~37 P57 tests across the full pipeline (pure BM25, doc extraction,
  build/status/incremental/schema/staleness/round-trip, retrieval, ai_history synthesis + the top_k=0
  regression) within the 1024-green workspace + reviewer verification (incl. the caught+fixed top_k
  MUST-FIX); vitest 109. Live index-build → ask → grounded-answer flow is USER CHECKPOINT (headless pane
  0×0; mock fixtures carry no diffs). Retriever = **BM25 v1 (embeddings deferred — FOR USER)**. Commits:
  `<a>`·`<b>`·`<c+fix>`·tester (see `git log`).
**Current step:** P57 DONE. ✅ **PHASE 2 (P53–P57) COMPLETE** — all AI-gate-passed, awaiting batched native
USER CHECKPOINTs (`docs/contracts/P5{3,4,5,6,7}-user-checklist.md`). Next: **Phase 3 (correctness & parity)
— P58 commit signing** (no OD1 dependency).

## P56 — local AI changelog / release-notes (Phase 2 · milestone 4/5) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P56-local-changelog.md`. OD1 = local-`claude`-CLI-only. Smaller milestone (S–M).

**P56 goal:** a tag/ref range (or "since last tag") → grouped, categorized Markdown release notes, fully
local, READ-ONLY (writes nothing to git). New `ai_changelog(repoId, range) → AiChangelog`; reuses the
shipped range resolver (`resolve_digest_range` → pub(crate)) + `resolve_last_tag` +
`render_commit_list`/`render_headers`; AI grouping guided by a conventional-commits HINT (works on
non-conventional repos too); fixed taxonomy Features/Fixes/Performance/Refactoring/Documentation/Tests/
Other (empty omitted). Renders in `AiOutputPanel` (Copy + opt-in editable). Cmd 134→135.

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 AI-grouping-with-conventional-hint · OQ2
the 7-heading taxonomy · OQ3 tag-pill "Release notes since previous tag" + `ChangelogDialog` (+ optional
palette) · OQ4 dedicated `AiChangelog` (echoes resolved fromRef/toRef) · OQ5 opt-in `editable` textarea +
Copy on AiOutputPanel · OQ6 promote `resolve_digest_range`/`format_commit_meta` to pub(crate) · OQ7
SinceLastTag = from tag-before-T, to=T.

Sub-increments: **P56a** core (`ai_changelog.rs`: `generate_changelog` + `resolve_last_tag`; reuse resolver)
+ `ai_changelog` cmd(→135) + IPC + mock + tests → **P56b** UI (`ChangelogDialog` + tag-pill entry +
`runChangelog` + AiOutputPanel Copy/editable).

- **P56a** (reviewer APPROVE, 0 must-fix / 0 should-fix; 2 nits) — core+IPC+mock. New `ai_changelog.rs`:
  `generate_changelog` (reuses promoted `resolve_digest_range`; `SinceLastTag`→`resolve_last_tag`→from;
  empty range / no earlier tag → AiFailed BEFORE any CLI; read-only) + `resolve_last_tag` (reviewer-verified:
  reachable-ancestor tags via merge_base, excludes the tip tag, both lightweight+annotated, committer-time
  order) + fixed-taxonomy prompt (AI grouping, no Rust parser). `ai_changelog` cmd + _inner (consent before
  repo_path, read-only; 134→135). IPC + barrel + mock (`?ai=off`→aiFailed). New `ai_changelog_cli.rs` (git
  oracle: set==`git log <from>..<to>` for tag-range + sinceLastTag). 10 tests; clippy -D + build/tsc clean;
  `ai_changelog.rs` 485 lines. Nit: `format_commit_meta` promoted but unused by changelog (harmless).
- **P56b** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — UI. New `ChangelogDialog.tsx`
  (between-refs / since-last-tag range picker, Combobox ref fields). `runChangelog` mirrors runDigest
  (aiPanelReqId last-wins; title `Release notes: <from>..<to>`; read-only). Tag-pill "Release notes since
  previous tag" (graph pills + sidebar rows; gated !aiEligible; target=tagName per OQ7) + palette "Release
  notes…". `AiOutputPanel` gains a Copy button (all callers) + OPT-IN `editable` textarea (changelog only)
  — reviewer CONFIRMED backward-compat (non-editable callers keep the exact `<pre>` path; sole render site
  WorkspaceGraphPane; draft reseeds via intermediate null → no stale-draft). tsc/build clean; paletteActions
  vitest 11/11. Nits: edits lost on Esc/close (Copy is the persistence, per spec); refNames not deduped;
  RepoWorkspace AI-runner bloat (future useAiPanel hook).
- **P56 tester** — orchestrator-verified regression: `cargo test --workspace` GREEN (exit 0; 3 perf gates
  ignored), clippy -D clean, vitest 109/109. ChangelogDialog range vitest SKIPPED (range logic inline in the
  submit closure — no exported pure builder; vitest env is node w/o @testing-library; barred from adding a
  dep/refactoring; palette entry already covered by paletteActions 11/11 → follow-up: extract a pure
  `buildChangelogRange()`). Wrote `docs/contracts/P56-user-checklist.md`.
- **P56 AI GATE PASSED (2026-08-08).** Backend: 10 ai_changelog tests (resolve_last_tag + git-oracle +
  pre-CLI bails + wire/deserialize) + reviewer verification; frontend: reviewer-verified runChangelog
  last-wins + AiOutputPanel back-compat + tag-pill/palette entries; workspace green + vitest 109. Live flow
  (tag-pill → grouped notes; dialog; Copy/editable) is USER CHECKPOINT (headless pane 0×0; canvas tag pill
  not drivable).
**Current step:** P56 DONE (AI gate passed, awaiting USER CHECKPOINT). Phase 2 FINAL milestone: **P57
(semantic commit-history search — BM25 v1, embeddings deferred per OD1 / FOR USER)**.

## P55 — natural-language → SAFE git operation (Phase 2 · milestone 3/5) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

Contract: `docs/contracts/P55-nl-to-safe-git-op.md`. OD1 = local-`claude`-CLI-only. The highest-trust-risk
Phase-2 feature — the safety model (§2) is the centerpiece.

**P55 goal:** turn a free-text request into a STRUCTURED, PREVIEWABLE, CONFIRM-GATED git op — NEVER a raw
shell string. Read-only planner `ai_plan_operation(repoId, request) → OperationPlan`: the model may only
SELECT+PARAMETERIZE ONE intent from a CLOSED allowlist (fixed `AiOpIntent` enum); Rust resolves refs/oids +
builds a read-only preview + `DangerLevel`, or returns `unsupported`; on explicit Confirm a thin
`safeOpDispatch` invokes the EXISTING typed command (resetBranch/revertCommit/checkoutBranch/…). 7 structural
safety layers; the AI path NEVER mutates and no shell string exists anywhere. Cmd 133→134 (planner only; no
new mutation command). Allowlist v1 (10 intents): undoLastCommit/undoLastMerge/resetToCommit/revertCommit/
switchBranch/createBranch/deleteBranch/stashChanges/discardChanges/mergeBranch + unsupported.

**Orchestrator OQ decisions — accept ALL architect recs:** OQ1 execute via existing typed commands (AI
surface stays read-only) · OQ2 undoLastMerge = reset-to-first-parent + Destructive warning (FLAGGED for
user — see FOR USER above) · OQ3 the 10-intent allowlist · OQ4 palette "Ask Bonsai to…" + toolbar ✨ · OQ5
remote-only switch → checkoutRemoteBranch · OQ6 model = default sonnet · OQ7 show rationale · OQ8 network
ops (push/pull/fetch) OUT of v1.

Sub-increments: **P55a** safety core + reset/revert family (`ai_operation.rs` all types + `plan_operation` +
grounding + `resolve_intent`/`build_preview` for undo-commit/undo-merge/reset/revert + the 2 NON-NEGOTIABLE
tests `plan_never_mutates` + `out_of_allowlist_is_unsupported`) + `ai_plan_operation` cmd(→134) + IPC + mock
→ **P55b** remaining allowlist (switch/create/delete/stash/discard/merge resolution+preview+tests) →
**P55c** UI (`ProposedOpDialog` + `safeOpDispatch` + palette/toolbar entry + `runPlanOperation`).

- **P55a** (reviewer APPROVE, 0 must-fix; 1 should-fix = file-size split→P55b; 2 nits) — safety core +
  reset/revert family. New `ai_operation.rs`: closed `AiOpIntent` enum (L1) + fail-closed parse (L2 —
  unparseable/unknown-tag/shell-string→Unsupported, distinct from AiFailed) + READ-ONLY `plan_operation`
  (tested `plan_never_mutates`) + `resolve_intent`/`build_preview` for undoLastCommit / undoLastMerge
  (first-parent Mixed Destructive + upstream warning) / resetToCommit / revertCommit (other 6 → "not yet
  supported"). Reviewer VERIFIED IN CODE: no model-text→shell path (argv fixed consts, request+state via
  STDIN, model output only → serde+Rust resolution), no model-text→unconfirmed-mutation. `ai_plan_operation`
  cmd + _inner (consent before repo_path, read-only; 133→134). New `ai_operation_cli.rs` (2 process-isolated
  write-nothing end-to-end tests). reset.rs ResetMode +Serialize. IPC + full §10 mock. 10 tests (incl. both
  non-negotiables); clippy -D + build/tsc clean. `rationale` = Rust-generated (safer). ⚠ ai_operation.rs 1406
  lines → MUST split into `ai_operation_resolve.rs` in P55b.
- **P55b** (reviewer APPROVE, 0 must-fix / 0 should-fix; 4 nits) — 6 remaining intents (switch local/remote,
  createBranch [reuse `validate_branch_name`, reject existing], deleteBranch local-non-current, stash dirty,
  discard∩tracked-modified, merge) — all op-in-progress-gated, read-only, fail-closed→Unsupported. Split
  `ai_operation.rs` (877 logic) → spine 348 + `ai_operation_grounding.rs` 176 + `ai_operation_preview.rs`
  226 + `ai_operation_resolve.rs` 496 (all <500 logic). `plan_never_mutates` now enumerates ALL 10 intents.
  14 ai_operation tests; clippy -D + build/tsc clean; cmd unchanged (134); no mock change (P55a mock already
  covered the branches).
- **P55c** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — UI. New `safeOpDispatch.ts` (pure switch
  → EXISTING typed commands only; no git logic/shell; stash→`createStash` with `StashScope`
  'allWithUntracked'|'all') + `ProposedOpDialog.tsx` (on `ConfirmDialog`, Cancel-focused, danger-variant,
  presentational). `runPlanOperation` (last-wins guard; proposed→dialog, unsupported→calm info toast,
  error→error toast) + `confirmProposedOp` (the ONLY mutation, via safeOpDispatch, then refresh+toast).
  Entries: palette "Ask Bonsai to…" (registry, gated aiEligible) + toolbar ✨ Ask → shared NL PromptDialog.
  Reviewer VERIFIED read-only-until-Confirm (single dispatch path). ipc/index.ts barrel re-exports. tsc/build
  clean. Nit→future polish: merge/revert that PAUSE on conflicts still show a 'success' toast (the conflict
  DOES surface via the existing op-state banner; contract-sanctioned).
- **P55 tester** — `cargo test --workspace` **978/0/3-ignored**; vitest **109/109** (+13 `safeOpDispatch`
  routing tests locking the §6 table with compile-time exhaustiveness + one-method-fires); wrote
  `docs/contracts/P55-user-checklist.md`; clippy clean; no bugs.
- **P55 AI GATE PASSED (2026-08-08).** Backend: 978 workspace tests incl. the two non-negotiables
  `plan_never_mutates` (all 10 intents) + `out_of_allowlist_is_unsupported`, + 13 `safeOpDispatch` routing
  vitests; reviewer VERIFIED IN CODE: no model-text→shell path, read-only-until-Confirm (single dispatch
  path), dispatch→existing typed commands only. Harness smoke SKIPPED to conserve the overnight budget —
  the same app's clean AI-eligible load was confirmed at P54, and the live NL→propose→confirm flow can't be
  driven headless (pane 0×0). The live flow is USER CHECKPOINT (`docs/contracts/P55-user-checklist.md`).
  Commits: `01df34e`? (a) · P55b · `<c>` · tester — see `git log`.
**Current step:** P55 DONE (AI gate passed, awaiting USER CHECKPOINT). Phase 2 milestone 4/5 next: **P56
(local AI changelog / release-notes)**.

## P54 — commit composer: WIP → N logical commits (Phase 2 · milestone 2/5) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-08)

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
- **P54b** (reviewer APPROVE, 0 must-fix; 1 should-fix→tester; 4 nits) — written by an agent that CRASHED
  mid-run (API error); orchestrator INDEPENDENTLY verified before commit: 7/7 compose_apply tests pass
  (mid-sequence rollback, workdir-untouched, unborn-HEAD, validate-before-commit, two-group deltas +
  have_git `show --stat` oracle), clippy -D + build/tsc clean, cmd registered (133). New `compose_apply.rs`
  `apply_composed_commits` — reviewer verified IN CODE all 3 safety guarantees: ATOMIC (whole-plan
  validation before any mutation; any loop Err → rollback, ZERO commits), HEAD+index ROLLBACK for
  branch/detached/unborn, WORKDIR NEVER TOUCHED (no checkout/hard-reset; `reset_index_to_head` reads HEAD
  tree into index only). No new crate dep. `files_with_rename_origs` stages both sides. `commands/compose.rs`
  (house shape of commit, NO consent gate, no repo-changed; 132→133). Types + mock (`compose.ts` #fail→git
  mutating nothing; mock.ts spread). SHOULD-FIX→tester: add a detached-HEAD mid-sequence rollback test
  (only branch+unborn tested). Nits: rollback masks trigger err; rename-in-group test; 739 lines (prod
  ~200 + inline tests, ok).
- **P54c** (reviewer APPROVE, 0 must-fix / 0 should-fix; 3 nits) — frontend review UI. New
  `useCommitComposer.ts` (PURE partition-preserving reducers editMessage/moveFile/addGroup/dropGroup/
  mergeInto; `apply()` builds ComposePlan OMITTING unassigned → left uncommitted; reqId last-wins;
  refetch+toast+close), `ComposerDialog.tsx` (modal: notes + "N left uncommitted" + group list +
  Unassigned bucket + "+ New group" + "Create N commits" gated `canApply`), `ComposerGroupCard.tsx`
  (presentational, no ipc). Entry = "Compose commits ✨" in **CommitBox** (the real working-changes panel,
  not CommitPanel) gated `aiEligible && workingDirty`. Esc-layer composerOpenRef above diffSlot (preview
  peels first; ignored while applying); nav + Ctrl/F/K inert while open. Preview reuses `getWorkdirFileDiff`
  (embedded in modal to dodge z-index occlusion). tsc/build clean; new files 325/211/174. Nits
  (non-blocking): partial-staged preview shows workdir-vs-index not HEAD→workdir (needs a new backend diff
  mode → track for checkpoint); defensive moveFile out-of-range; group card key={i}.
- **P54 tester** — regression `cargo test --workspace` GREEN (exit 0; 3 perf gates ignored) after an
  unrelated-fixture fix (below); vitest **96/96** (22 new composer-reducer partition tests). Added the
  reviewer should-fix `apply_rolls_back_on_mid_sequence_failure_detached_head` (compose_apply 8/8 —
  detached-HEAD rollback now covered alongside branch+unborn). Wrote `docs/contracts/P54-user-checklist.md`.
  **Pre-existing unrelated fix (test-only):** M6 `remote_cli.rs` FF-pull oracle failed on this box due to
  global `core.autocrlf=true` (CLI clone checked out CRLF worktree vs LF blobs → dirty before any pull);
  fixed the fixture to `git -c core.autocrlf=false clone` (tester root-caused; NOT a P54 regression;
  restores a fully green workspace suite).
- **P54 AI GATE PASSED (2026-08-08).** Backend: propose partition invariant + apply ATOMIC / rollback
  (branch, detached, unborn) / WORKDIR-UNTOUCHED all unit-proven (8 compose_apply + ai_compose + have_git
  oracle) + reviewer file:line verification; frontend: 22 reducer partition vitests + reviewer
  wiring/gating/Esc verification. Harness (mock :1420, aiConsented seeded): app loads clean (no error
  boundary, zero console errors); "Compose commits ✨" renders ENABLED (aiEligible && workingDirty). The
  live propose→review→apply MODAL flow is USER CHECKPOINT (headless pane can't composite/drive — 0×0).
  Commits: `0999af4` (a) · `6f7f3ef` (b) · `68c1ae1` (c) · tester-closeout next.
**Current step:** P54 DONE (AI gate passed, awaiting USER CHECKPOINT `docs/contracts/P54-user-checklist.md`).
Next Phase-2 milestone: **P55 (natural-language → safe git op)**.

## P53 — AI "why" layer: blame-why + explain-commit + branch naming (Phase 2 · milestone 1/5) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-07)

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

## P52 — adopt git's commit-graph file (Phase 1 · large-repo perf) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-07)

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

## P51 — commit-graph polish + clutter controls (Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-07)

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

## P50 — commit/content search + command palette + list filtering (Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-07)

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

## P49 — external integrations: open in terminal / file manager / editor (Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-07)

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

## P46 — diff viewer: split view + copyable selection + auto-advance — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-05)

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

## P45 — per-line discard action (mirrors "Stage 1 line") — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-05)

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

## P44 — settings improvements (4 user requests) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-05)

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

## P42 — packaging + auto-update (Productization) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-05)

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

## P43 — first-run onboarding + empty-state polish (Productization) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-04)

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

## P40 — git config editing (Git completeness, Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-04)

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

## P39 — git bisect (Git completeness, Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-04)

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

## P38 — reflog viewer + restore (Git completeness, Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-04)

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

## P37 — force-push-with-lease (Git completeness, Phase 1) — **DONE ✅ USER-CONFIRMED 2026-08-08** (2026-08-04)

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
