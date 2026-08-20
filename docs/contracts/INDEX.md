# Contract index

One line per file in `docs/contracts/`, so no session has to grep 130+ files to find where something
was specced. **Curated by `docs-curator`; the contracts themselves belong to `architect` and
`ui-designer`.**

- **Status is a pointer, not an independent verification.** It reflects the board (`TODO.md`) and the
  archives (`docs/history/todo-archive-2026-08.md`, `docs/history/todo-archive.md`). Vocabulary:
  `done` (AI gate + native USER CHECKPOINT both passed) · `awaiting USER CHECKPOINT` (code-complete,
  AI gate passed, native half outstanding) · `pending` · `in-progress` · `deferred` · `living` ·
  `superseded` (content folded into another contract; the file is a pointer stub, kept so old
  `TODO.md` lines and commit messages still resolve).
- Contract *files* are frequently written before the code and amended during it. A `done` status
  means the milestone shipped, **not** that the file is free of drift. Known drift is tracked under
  `🐞 SPUN-OUT ITEMS` in `TODO.md`.
- `*-user-checklist.md` files are the native USER CHECKPOINT scripts. They are checkpoint artifacts,
  not specs.
- `ui-reference.md` is the canonical design system and is **owned by `ui-designer`** — no other agent
  edits it.

## Cross-cutting / living

| File | Scope | Status |
|---|---|---|
| `ui-reference.md` | Canonical design system: tokens, geometry, graph metrics, ref pills, states, both themes. | living |
| `phase2-ai-native-overview.md` | Phase 2 shared conventions, anchor for P53–P57. | done |
| `phase4-forge-overview.md` | Phase 4 shared conventions, anchor for P62–P64. | awaiting USER CHECKPOINT (P62–P65) |

> `ui-reference.md` was last amended in **P69l (2026-08-20)**: §12 "Settings surface" trued up to the
> shipped two-pane shell, new §7.2 and §12.3.4, and §2 now prohibits `color: var(--accent)` on text
> that can land on a `--selection` fill (see `TODO.md` follow-up **A9**).
> The `ui-reference.md` §10 "Git is not available" notice bar was added by P70 (`P70-ui.md`).

## MVP — M0–M6

All `done`, USER CHECKPOINTs confirmed; gate split archived in `docs/history/milestones-mvp.md`.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `M0-scaffold.md` | M0 | Tauri v2 + React/Vite/TS scaffold, folder picker, repo/HEAD detection. | done |
| `M0-user-checklist.md` | M0 | Native smoke checklist. | done |
| `M1-status.md` | M1 | Working-directory status via git2 + notify/refresh/refocus rescan. | done |
| `M1-user-checklist.md` | M1 | Native checklist (status + refresh paths). | done |
| `M2-graph.md` | M2 | `GraphLayout` lane/edge algorithm + canvas rendering + virtualization + perf gate. | done |
| `M2-user-checklist.md` | M2 | Native checklist (20k-commit scroll feel). | done |
| `M3-commit.md` | M3 | File-level stage / unstage / commit. | done |
| `M3-user-checklist.md` | M3 | Native checklist (stage/commit round-trip). | done |
| `M4-diff.md` | M4 | Working-dir diffs and commit-vs-first-parent diffs. | done |
| `M4-user-checklist.md` | M4 | Native checklist (diff view). | done |
| `M5-branches.md` | M5 | Branch list / create / checkout / delete with confirmation. | done |
| `M5-user-checklist.md` | M5 | Native checklist (branch ops + confirm dialog). | done |
| `M6-remotes.md` | M6 | Fetch / fast-forward-only pull / push with credential handling. | done |
| `M6-user-checklist.md` | M6 | Native checklist (round-trip against a real remote). | done |

## Polish and feature phases — P1–P27

All `done`; USER CHECKPOINTs confirmed 2026-07-30 (P1–P9, P3x, P4–P7) and 2026-08-03 (P10–P27).
Board text: `docs/history/todo-archive.md`.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P1-polish.md` | P1 | Polish: shortcuts, toasts, empty/loading states, styling. | done |
| `P2-followups.md` | P2 | Post-v1 follow-up batch. | done |
| `P3a-diff-overlay.md` | P3a | Diff overlay in the center pane. | done |
| `P3b-tree-grouping.md` | P3b | Tree-grouped sidebar and status lists. | done |
| `P3c-merge-conflicts.md` | P3c | Merge + conflict handling. | done |
| `P3d-rebase.md` | P3d | Rebase. | done |
| `P3e-multi-repo-tabs.md` | P3e | Multi-repo tabs. | done |
| `P3f-changes-panel.md` | P3f | Changes-panel refinements (user feedback 2026-07-29). | done |
| `P4-ux-refinements.md` | P4 | UX refinements after P3 user feedback. | done |
| `P5-graph-context-menus.md` | P5 | Graph context menus. | done |
| `P6-unified-context-menus.md` | P6 | Unified branch/remote context menus. | done |
| `P7-gitkraken-layout.md` | P7 | GitKraken-style graph layout. | done |
| `P8-merge-autostash.md` | P8 | Merge with autostash. | done |
| `P9-stash-management.md` | P9 | Stash management. | done |
| `P10-stash-as-node.md` | P10 | Stash as a graph node + context-menu icons. | done |
| `P11-followup.md` | P11 | Feature follow-up batch (5 user requests). | done |
| `P11g-revision.md` | P11g | DiffBrowser rework (changes A–D). | done |
| `P12-conflict-editor.md` | P12 | Rich conflict-resolution editor. | done |
| `P12-user-checklist.md` | P12 | Native checklist (conflict editor). | done |
| `P13-ai-foundation.md` | P13 | Local-AI foundation (Claude Code CLI) + AI merge-conflict resolution. | done |
| `P13-user-checklist.md` | P13 | Native checklist (AI foundation + conflict resolution). | done |
| `P14-mcp-server.md` | P14 | `bonsai-core` crate extraction + standalone `bonsai-mcp` server. | done |
| `P15-ai-features.md` | P15 | In-app AI tier 1: commit-message gen, explain/review, branch/range summary. | done |
| `P16-embedded-mcp.md` | P16 | Embedded MCP server (tier 3: shared live workspace). | done |
| `P17-partial-staging.md` | P17 | File/Diff toggle + partial (hunk/line) staging. | done |
| `P17-user-checklist.md` | P17 | Native checklist (partial staging). | done |
| `P19-submodules.md` | P19 | Submodule support: list / init / update / sync / open-in-tab. | done |
| `P19-user-checklist.md` | P19 | Native checklist (submodules). | done |
| `P20-daily-essentials.md` | P20 | Amend, cherry-pick, revert, reset, discard, abort. | done |
| `P20-user-checklist.md` | P20 | Native checklist (daily essentials). | done |
| `P21-repo-lifecycle.md` | P21 | Repo lifecycle: clone + init. | done |
| `P21-user-checklist.md` | P21 | Native checklist (clone + init). | done |
| `P22-tags-remotes.md` | P22 | Tags and remotes management. | done |
| `P22-user-checklist.md` | P22 | Native checklist (tags + remotes). | done |
| `P23-interactive-rebase-blame.md` | P23 | Interactive rebase + blame / file history. | done |
| `P23-user-checklist.md` | P23 | Native checklist (interactive rebase + blame). | done |
| `P24-ai-context-profiles.md` | P24 | AI-asset inventory + drift + per-model context profiles (A1+A2). | done |
| `P24-user-checklist.md` | P24 | Native checklist (AI assets + context profiles + AI translate). | done |
| `P25-ai-review-stale-branches.md` | P25 | AI review of worktree/branch (B1) + stale-branch cleanup (B4). | done |
| `P25-user-checklist.md` | P25 | Native checklist (AI review + stale branches). | done |
| `P26-skills-agents-manager.md` | P26 | Skills / subagents / slash-commands manager (A3). | done |
| `P26-user-checklist.md` | P26 | Native checklist (agent-assets manager). | done |
| `P27-worktrees.md` | P27 | Worktree management: list, create, remove, lock/unlock, open-in-tab (C1). | done |
| `P27-user-checklist.md` | P27 | Native checklist (worktrees). | done |

> **No contract file exists** for the P18 slot (it was folded into the P18–P23 batch section of the
> archive) or for P41 (Git LFS — **deferred**, the user chose to skip it as niche).

## Repo-management + Git-completeness phases — P28–P48

All `done`; USER CHECKPOINTs confirmed 2026-08-08. Board text:
`docs/history/todo-archive-2026-08.md` Part 3.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P28-discard-hunk.md` | P28 | Discard hunk + status-panel UX (double-click stage, section styling). | done |
| `P28-what-changed-digest.md` | P28 | AI "what changed" digest (roadmap B3). | done |
| `P28-user-checklist.md` | P28 | Native checklist (what-changed digest). | done |
| `P29-repo-health.md` | P29 | Repo-health dashboard (D1). | done |
| `P29-user-checklist.md` | P29 | Native checklist (repo health). | done |
| `P30-scheduler.md` | P30 | Background-job scheduler (B5). | done |
| `P30-user-checklist.md` | P30 | Native checklist (scheduler). | done |
| `P31-worktree-ai-contexts.md` | P31 | Per-worktree AI contexts. | done |
| `P31-user-checklist.md` | P31 | Native checklist (per-worktree AI contexts). | done |
| `P32-user-checklist.md` | P32 | Native checklist for named worktrees + copy uncommitted changes. | done |
| `P33-checkout-autostash.md` | P33 | Auto-stash branch switch with auto fast-forward. | done |
| `P34-stash-scopes.md` | P34 | Stash scopes + staging-area affordance. | done |
| `P35-credential-cache.md` | P35 | In-process HTTPS credential cache. | done |
| `P36-ux-safety-fixes.md` | P36 | UX / safety fixes, backend + IPC (worktree checkout guard, bulk discard, tab UX). | done |
| `P37-force-push-with-lease.md` | P37 | Force-push with lease. | done |
| `P37-user-checklist.md` | P37 | Native checklist (force-push with lease). | done |
| `P38-reflog.md` | P38 | Reflog viewer + restore. | done |
| `P38-user-checklist.md` | P38 | Native checklist (reflog). | done |
| `P39-bisect.md` | P39 | `git bisect` with an on-disk sequencer. | done |
| `P39-user-checklist.md` | P39 | Native checklist (bisect). | done |
| `P40-config-editing.md` | P40 | Git config editing (closes the identity-unset gap). | done |
| `P40-user-checklist.md` | P40 | Native checklist (config editing). | done |
| `P42-packaging-autoupdate.md` | P42 | Packaging + Tauri v2 auto-update behind Bonsai IPC. | done |
| `P42-user-checklist.md` | P42 | Native checklist (packaging + updater trust chain). | done |
| `P43-onboarding.md` | P43 | First-run onboarding + empty-state polish. | done |
| `P43-user-checklist.md` | P43 | Native checklist (onboarding persistence across restart). | done |
| `P44-identity-profiles.md` | P44d | Named identity profiles (item 2 of the P44 settings batch). | done |
| `P45-discard-line.md` | P45 | Per-line discard, mirroring "Stage 1 line". | done |
| `P46-diff-viewer-enhancements.md` | P46 | Split view, copyable selection, auto-advance. | done |
| `P47-cherry-pick-enhancements.md` | P47 | Cherry-pick enhancements + commit-action menu consolidation. | done |
| `P47-user-checklist.md` | P47 | Native checklist (cherry-pick + menus). | done |

> **No contract file** for P48 (New Worktree dialog UX) or for the v1-prep release-readiness batch —
> both are recorded on the board only (`docs/history/todo-archive-2026-08.md` Part 3).

## Phase 1 — large-repo perf and daily UX — P49–P52

All `done`; USER CHECKPOINTs confirmed 2026-08-08.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P49-external-integrations.md` | P49 | Open in terminal / file manager / editor. | done |
| `P49-user-checklist.md` | P49 | Native checklist (external launches). | done |
| `P50-search-command-palette.md` | P50 | Commit/content search + Ctrl/Cmd-K palette + list filtering. | done |
| `P50-user-checklist.md` | P50 | Native checklist (search + palette). | done |
| `P51-graph-polish.md` | P51 | Graph polish + clutter controls (SHA/date/author columns, compact, ahead/behind). | done |
| `P51-user-checklist.md` | P51 | Native checklist (graph polish). | done |
| `P52-commit-graph.md` | P52 | Adopt git's on-disk commit-graph file (~5× faster health scan). | done |
| `P52-user-checklist.md` | P52 | Native checklist (commit-graph adoption). | done |

## Phase 2 — AI-native edge — P53–P57

All `done`; USER CHECKPOINTs confirmed 2026-08-08. OD1 (local-`claude`-CLI-only) applies to all five.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P53-ai-why-layer.md` | P53 | AI "why" layer: blame-why + explain-commit + AI branch naming. | done |
| `P53-user-checklist.md` | P53 | Native checklist (why layer). | done |
| `P54-commit-composer.md` | P54 | Commit composer: WIP → N logical commits. | done |
| `P54-user-checklist.md` | P54 | Native checklist (composer). | done |
| `P55-nl-to-safe-git-op.md` | P55 | Natural language → SAFE git operation, confirm-gated. | done |
| `P55-user-checklist.md` | P55 | Native checklist (NL → safe op). | done |
| `P56-local-changelog.md` | P56 | Local AI changelog / release notes. | done |
| `P56-user-checklist.md` | P56 | Native checklist (changelog generation). | done |
| `P57-semantic-history-search.md` | P57 | Semantic commit-history search (BM25 retrieval; embeddings deferred). | done |
| `P57-user-checklist.md` | P57 | Native checklist (history search). | done |

## Phase 3 — correctness and parity — P58–P61

All `done`; USER CHECKPOINTs confirmed 2026-08-08.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P58-commit-signing.md` | P58 | Real SSH/GPG commit signing + verification. | done |
| `P58-user-checklist.md` | P58 | Native checklist (signing with real keys). | done |
| `P59-hooks-and-lease-hardening.md` | P59 | Git-hooks execution + force-with-lease atomic hardening (closes the P37 TOCTOU). | done |
| `P59-user-checklist.md` | P59 | Native checklist (hooks + lease). | done |
| `P60-parity-batch.md` | P60 | Branch rename · non-FF pull (merge/rebase) · one-click undo via reflog · submodule add/deinit/remove. | done |
| `P60-user-checklist.md` | P60 | Native checklist (parity batch). | done |
| `P61-diff-quality.md` | P61 | Word-level/intraline highlighting + image diff. | done |
| `P61-user-checklist.md` | P61 | Native checklist (diff quality). | done |

## Phase 4 — forge/PR + paged loading — P62–P66

Code-complete, **native halves outstanding**. Board status: `TODO.md`; build detail:
`docs/history/todo-archive-2026-08.md` Part 2.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P62-forge-foundation.md` | P62 | Provider-abstracted forge foundation (GitHub first), `crates/bonsai-forge/` + PR panel. | awaiting USER CHECKPOINT |
| `P62-user-checklist.md` | P62 | Native checklist (real GitHub PAT + real PRs). | awaiting USER CHECKPOINT |
| `P63-forge-graph-signals.md` | P63 | PR + CI badges on the commit graph. | awaiting USER CHECKPOINT |
| `P63-user-checklist.md` | P63 | Native checklist (canvas badge pixels, click→PR). | awaiting USER CHECKPOINT |
| `P64-forge-providers-ai-pr.md` | P64 | GitLab + Bitbucket + Azure DevOps providers + AI PR descriptions. | awaiting USER CHECKPOINT |
| `P64-user-checklist.md` | P64 | Native checklist (per-provider tokens, live AI generation; §D holds 2 parked NITs). | awaiting USER CHECKPOINT |
| `P65-paged-loading.md` | P65 | Incremental / paged commit loading over a `stream_graph` channel. | awaiting USER CHECKPOINT |
| `P65-user-checklist.md` | P65 | Native checklist (scroll feel + progressive load on a large repo). | awaiting USER CHECKPOINT |
| `P65a-lazy-topo-spike.md` | P66 | Feasibility spike for lazy generation-number topo order (VERDICT: tractable, effort L). | deferred — P66 is approved future work, not scheduled (user decision 2026-08-10) |

## User-reported batch, 2026-08-17 — P67, P68

Code-complete, **native halves outstanding**. Neither milestone is done.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P67-ux-polish-batch.md` | P67 | Always-visible HEAD guideline + right-panel space reclaim + Cozy/Compact density. | awaiting USER CHECKPOINT |
| `P67-user-checklist.md` | P67 | Native checklist (guideline visibility, dash crawl, density readability). | awaiting USER CHECKPOINT |
| `P68-ai-conflict-streaming.md` | P68 | Streaming / interactive / bulk AI conflict resolution; invariants D1–D16. | awaiting USER CHECKPOINT |
| `P68e-ai-activity-dock.md` | P68e | AI activity dock UI (bottom dock, live log, cancel, ask block). | awaiting USER CHECKPOINT — file is 1064 lines and stale vs shipped code (see `TODO.md` spun-out items) |
| `P68g-ui.md` | P68g-2 | The eight AI-run settings, honest consent copy, ask-block hardening; §5 documents harness seeds 1–9. | awaiting USER CHECKPOINT |
| `P68-security-audit.md` | P68g | Security audit of the AI conflict surface (1 HIGH, 5 MEDIUM, 7 LOW/INFO); items 7–11 are open follow-ups (the pid-zeroing half of item 10 landed in `67539fd`). | audit delivered 2026-08-18; follow-ups open |
| `P68-user-checklist.md` | P68 | Native checklist (real CLI past 90 s, cancel, mid-run question, read-only tools, bulk, settings, consent copy, refused read). | awaiting USER CHECKPOINT |

## Settings redesign — P69 (2026-08-19 → 2026-08-20)

Code-complete, AI gate GREEN at `a13b729`; **native half outstanding** — the milestone is not done.
Eleven build increments P69a–P69k, plus P69l (this docs pass). Board section: `TODO.md` §"P69 —
Settings redesign".

> ⚠️ **The label "P69" names two different milestones on the board.** This settings redesign, and the
> earlier **P69 — 1.0.0 release readiness** (shipped 2026-08-18, tag `v1.0.0`, commit `bd52483`),
> which has no contract file of its own. Both sections are live in `TODO.md`.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P69-settings-ui.md` | P69 | UI contract: two-pane settings shell, control vocabulary (switches / segmented / per-row reset), row anatomy + help text, header identity menu, search. | awaiting USER CHECKPOINT |
| `P69-settings-shell.md` | P69 | Structural contract: settings catalog, rail/pane shell, props→context, `searchSettings` + `settingsAvailability.ts`, the anti-drift DOM guard. | awaiting USER CHECKPOINT |
| `P69c-draft-feedback-ui.md` | P69c | **Superseded pointer stub** → `P69-settings-ui.md` §13 (the `NumberSlider` out-of-range/blank draft affordance). | superseded (P69l) |
| `P69-settings-shell-amendment-A.md` | P69 | **Superseded pointer stub** → `P69-settings-shell.md`; maps AM-1…AM-8 onto §4.1 / §4.3 / §4.4 / §7.3 / §8. | superseded (P69l) |
| `P69-user-checklist.md` | P69 | Batched native checklist for all eleven increments (identity menu, shell, search, git-config scope, the two fixed defects, a11y, both themes). | awaiting USER CHECKPOINT |

> **What P69l changed in the two live contracts.** Both were amended in place so they describe what
> shipped rather than what was proposed.
> - `P69-settings-ui.md` gained a **§0.1 post-ship amendment log**, and its **§13** absorbed the
>   whole of `P69c-draft-feedback-ui.md`.
> - `P69-settings-shell.md` folded in **Amendment A** and records **OQ-2 as resolved**.
> - Both stubs are deliberately retained: past `TODO.md` lines and commit messages cite them by path
>   and by AM-number, and a dangling reference is worse than a pointer.

## Post-1.0.0 batch — P70–P76 (2026-08-19 → )

None of these is `done`. Board sections are all live in `TODO.md`.

| File | Milestone | Scope | Status |
|---|---|---|---|
| `P70-git-resolution.md` | P70 | One shared resolver for the `git` binary, an honest error taxonomy, and a startup preflight instead of N misleading auth toasts. | in-progress |
| `P70-ui.md` | P70 | UI: the "Git is not available" notice bar; adds `ui-reference.md` §10. | in-progress |
| `P71-updater-relaunch-env.md` | P71 | Post-auto-update relaunch must inherit the Start-menu environment, not `msiexec`'s. | in-progress (research) |
| `P72-forge-connect-fixes.md` | P72 | Azure DevOps 401 on connect + dead external forge links. | in-progress |
| `P72-ui.md` | P72 | UI: copy, states, and link wiring only — no new components, files, or tokens. | in-progress |
| `P73-submodule-reconnect.md` | P73 | Reconnect an orphaned `.git/modules` gitdir; the two user-reported submodule defects. | awaiting USER CHECKPOINT |
| `P73-submodule-reconnect-ui.md` | P73 | UI: submodule row + badge, row context menu, toast copy, pending state. | awaiting USER CHECKPOINT |
| `P73-user-checklist.md` | P73 | Native checklist against the real superproject that produced the report. | awaiting USER CHECKPOINT |
| `design-review-2026-08-19-p73-submodules.md` | P73 | `ui-designer` design review of the P73 frontend; findings S-2 / S-3 were deferred out and became P74. | delivered 2026-08-19 |
| `P74-a11y-toasts-hit-targets.md` | P74 | Toast-tone contrast + sidebar hit targets (S-2 / S-3 from the P73 design review). | awaiting USER CHECKPOINT |
| `P75-ipc-codegen.md` | P75 | Generate the IPC boundary from Rust with tauri-specta v2 — wire-identical, all 173 commands, no call-site churn. | pending — starts only after P74's tree is committed (user decision 2026-08-20) |
| `P76-native-checkpoint-automation.md` | P76 | tauri-driver + WebdriverIO harness to automate ~60–70% of the native USER CHECKPOINT backlog. | deferred — HELD as contract-only per user (2026-08-20) |
| `P77-tag-sync.md` | P77 | Local↔remote tag reconciliation: live ls-remote classification (in-sync/local-only/stale/remote-only), force-refresh + delete-remote ops, 3 IPC commands. | AI-gate GREEN, awaiting USER CHECKPOINT (2026-08-20) |
| `P77-ui.md` | P77 | UI: inline sidebar tag status badges, collapsed-section ⚠ rollup, status-gated context menu, two destructive-op confirm dialogs. Reuses existing tokens. | AI-gate GREEN, awaiting USER CHECKPOINT (2026-08-20) |

> **No contract file** for the DX dev-loop acceleration initiative (`pnpm gate`, nextest, rust-lld,
> velocity-mode workflow, 2026-08-20); it is recorded on the board only (`TODO.md` §"DX").

## Testing campaign — T1–T5

Pre-release hardening campaign (2026-08). Gate reported GREEN; 47 bugs fixed.

| File | Phase | Scope | Status |
|---|---|---|---|
| `T1-test-infrastructure.md` | T1 | Test infrastructure. | done |
| `T2-rust-audit.md` | T2 | Rust audit + tests, module by module. | done |
| `T4-e2e-journeys.md` | T4 | Playwright e2e journeys. | done |
| `T5-adversarial-property.md` | T5 | Redundancy, adversarial and property-based pass. | done |

> **No contract file** for T3 or T6; both are recorded in the campaign memory note only.
> The one open T5 decision, `F-T5-4` (truncated-object hang), was **RESOLVED for read surfaces**
> on 2026-08-19 (commit `7edd23e`, audit #2 §3.2): `run_with_git_timeout` wraps
> status/graph/stream/history-index; `create_commit` deliberately unwrapped (rationale in
> `corrupt_repo_cli.rs` C1). The T4-contracted `e2e/11-forge.spec.ts` was written in `83a9b2f`.
