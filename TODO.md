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
