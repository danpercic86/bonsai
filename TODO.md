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

## P3 — Diff overlay, trees, merge/rebase, multi-repo tabs — **in-progress** (2026-07-28)

Current step: P3e — IN PROGRESS. Contract written: docs/contracts/P3e-multi-repo-tabs.md (5 sub-increments
P3e-a..e). Orchestrator resolved §10 open Qs: repoId==canonical path; focus-rescan=active-tab-only;
tab branch/dirty badge + toast repo-prefix DEFERRED to polish (keep TabStrip lean).
Done: P3e-a backend core (be49130, reviewer APPROVE, cargo test 97 lib green). P3e-b session
persistence (4324272, orchestrator self-review mirroring ui-settings). P3e-c IPC mirror + multi-repo
mock (6324d06, reviewer APPROVE — INTERMEDIATE red full-tree build: ~31 App.tsx errors by design,
src/ipc/* tsc-clean, goes green at P3e-e). Now: P3e-d GraphCanvas active prop + zero-size guard +
remeasure-on-show. Then P3e-e frontend refactor (TabStrip/RepoWorkspace/App slim + reopen-all).
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
