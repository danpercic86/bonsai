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

**History is archived, not deleted.** Anything no longer on this board is in
`docs/history/todo-archive-2026-08.md` (P65 → P28 + the Phase 1–4 banners, moved 2026-08-18),
`docs/history/todo-archive.md` (P27 → P2, M0–M6) and `docs/history/milestones-mvp.md` (the M0–M6
AI-gate vs USER CHECKPOINT split). Contract files are indexed in `docs/contracts/INDEX.md`.

## 🎯 P69 — 1.0.0 release readiness — ✅ **SHIPPED 2026-08-18** (tag `v1.0.0`)

**Current step:** none — `v1.0.0` tagged and pushed at `bd52483`; the release workflow
builds/publishes from the tag. Reviewer verdict was APPROVE WITH NITS (no MUST-FIX); its two
SHOULD-FIXes were landed (changelog `v0.3.1` compare link + note; CONTRIBUTING apt-list claim)
except the branch-protection check-name question, which needs repo-settings access — **FOR USER:
if branch protection requires the status check `Frontend — vitest + build`, the CI matrix renamed
it to `… (ubuntu-22.04)` / `… (macos-latest)` and the old name will never report again.**

**FOR USER — two open items I could not close:**
1. **Back up `.tauri/updater-prod.key`.** It is correctly gitignored and untracked, so it exists
   in exactly ONE place: this working copy. Losing it permanently breaks auto-update for every
   installed client. (The committed `tauri.conf.json` pubkey was verified to match it.)
2. **GitHub reported 2 Dependabot alerts (1 high, 1 moderate)** on push. The high is the known
   `nanoid` GHSA-2v37-7h3g-55p8 — build/test tooling only (vite/vitest → postcss → nanoid), never
   in the shipped app, deliberately ignored in `pnpm-workspace.yaml`. **The moderate is
   unidentified** — `gh` is not installed here, and both project gates are green
   (`cargo deny --all-features check` = advisories/bans/licenses/sources all ok; `pnpm audit`
   shows only the one ignored high). Check the Dependabot page.

**Final AI gate at `bd52483`:** cargo `--workspace --no-fail-fast` exit 0, 0 failed / 4 ignored
(perf gates) · clippy `-D warnings` exit 0 · vitest 1596 / 130 files · e2e 104 passed, 1 skipped
(the permanent `08-stash` one) · eslint 0 errors, 29 warnings (budget 40) · `lint:size` OK ·
tsc+vite clean. Split equivalence proven: 1742 passed / 0 failed / 4 ignored **identical**
before and after.

Triggered by a full-project analysis requested 2026-08-18, after the user's first real macOS
testing pass. Goal: the minimum credible 1.0.0, not new features. User decisions taken this
session (all three explicit):
- **Scope** = macOS defects + contributor docs. Deeper tech debt stays deferred.
- **Code signing** = ship 1.0.0 **unsigned**, as already locked in `docs/code-signing.md`.
  README's per-OS Gatekeeper/SmartScreen workarounds stand. The "decision needed" in that file
  is therefore ANSWERED for 1.0: defer.
- **Native checkpoints** = tag 1.0.0 WITHOUT running the six outstanding USER CHECKPOINTs
  (P62–P65, P67, P68), but ship the forge/PR surface (P62–P64) **flagged beta** in README +
  CHANGELOG, since it needs real per-provider tokens to verify.

### Increments
- **P69a — macOS Rust fixes.** `external.rs` editor ladder silently no-ops on a Mac without
  VS Code (`open -a` always *spawns*; it fails at *exit*, and `run()` never waits) → add
  `wait_for_exit` for macOS `open` specs. Plus: `history_index/store.rs` keyed the repo path
  off `cfg(windows)` so APFS split the cache per path casing → drive it off `core.ignorecase`
  like `stage.rs:143-152` already does. Plus: `settings.rs` recents dedupe used
  `eq_ignore_ascii_case`, merging genuinely distinct repos on ext4 → reuse the `fs::canonicalize`
  approach from `commands/repo.rs:344`.
- **P69b — macOS frontend fixes.** ⌘+Enter did not commit (`CommitBox.tsx:257` was the ONLY
  handler missing `metaKey`). `--font-mono` lacked `ui-monospace`, so the DOM fell to Courier
  while the canvas (`metrics.ts:161`) used SF Mono — the two stacks now match. New
  `src/utils/platform.ts` so shortcut labels show ⌘ instead of a hardcoded "Ctrl". Playwright
  pinned `channel: 'msedge'` on `!CI`, so `pnpm test:e2e` could not run on a Mac → gate on
  `process.platform === 'win32'`.
- **P69c — crash + symlink.** Both from `docs/audit-2026-08-18.md`, re-verified at every cited
  line by the orchestrator before scheduling. (1) MUST-FIX: `WorkspaceRightPanel.tsx:265` gates
  on `!== null` but never bounds-checks, and `setGraph` publishes the first 512-row batch
  *before* the selection remap → a selection at row ≥512 renders `CommitPanel` with
  `node === undefined` → TypeError → ErrorBoundary takes down the workspace. (2) worktree-copy
  symlink write-through (`worktree_copy.rs:232-245`): lexical containment only, so a hostile
  branch can make Overwrite clobber a file outside the worktree.
- **P69d — docs.** CONTRIBUTING.md was materially stale: it claimed "there is no
  `.github/workflows` directory at all" and "there is currently no frontend test runner" — both
  false, both listed as *good first contributions*, and the second contradicted TESTING.md
  outright. Also a dead README anchor, "72 documents" (actually 139), clippy missing
  `-- -D warnings`, and a PR checklist omitting four real CI gates. CHANGELOG backfill: P49–P65
  and the T1–T6 campaign appear in NO entry, so a 1.0.0 cut from `[Unreleased]` (P67/P68 only)
  would misrepresent the release.
- **P69e — CI.** `frontend` job matrixed onto `macos-latest` (was ubuntu-only, which is exactly
  why P69b's defects reached a release branch). `e2e`/`audit` stay ubuntu-only.

### Known limitation (recorded deliberately)
The Playwright `msedge`→`win32` fix is only ever exercised **locally** — in CI the config always
picks bundled chromium, so no CI leg can regress-test it.

### Still open after P69 (explicitly NOT in scope)
`cargo fmt` adoption (1773 hunks / 221 files, no `rustfmt.toml`), SECURITY.md +
CODE_OF_CONDUCT.md, issue/PR templates, audit §3.10 refetch storm, `CommandPalette` highlight,
persisted-settings write path, `NumberSlider` mid-typing clamp, and the six native USER
CHECKPOINTs themselves.

## ✅ Confirmed checkpoints and accepted decisions (condensed — full text in the archive)

- **All native USER CHECKPOINTs for P2 → P61 are CONFIRMED.** Batches: 2026-07-30 (P4, P3a–P3f, P7,
  P7e, P7f, P8, P9), 2026-08-03 (P18–P23, P24, P25, P26, P27), **2026-08-08** ("mark everything as
  checked" — P28 through P61 inclusive: P32, P37–P46, the credential-cache and UX-fix batches,
  Phase 1 P49–P52, Phase 2 P53–P57, Phase 3 P58–P61). P5/P6 were confirmed earlier still.
- **The entire approved roadmap P49–P65 is code-complete** (2026-08-10). P49–P61 are fully done;
  P62–P65 are still `awaiting USER CHECKPOINT` (below).
- **Accepted defaults (2026-08-08, "ACCEPTED AS-IS"; changeable any time):** P55 `undoLastMerge` =
  reset-to-first-parent (Mixed, rewrites history, confirm-gated preview) · P57 retriever = BM25
  lexical, no embeddings · P61 image-diff base64 = hand-rolled, no new crate.
- **OD1 (confirmed):** AI stays **local-`claude`-CLI-only**; model tiers deferred.
- **Forge defaults (2026-08-08, autonomous, accepted):** new Rust deps `reqwest{blocking,json,
  rustls-tls}` + `keyring` · auth = **PAT-only** v1 (OAuth device-flow deferred) · provider order
  GitLab → Bitbucket → Azure DevOps.
- Nothing in the FOR-USER block is outstanding. Full text of every banner and decision:
  `docs/history/todo-archive-2026-08.md` Part 1.

## 🚀 PHASE 4 — forge/PR + paged loading (P62–P65) — code-complete, **awaiting USER CHECKPOINT**

Build detail for all four milestones (per-increment bullets, contract amendments, gate numbers) was
moved verbatim to `docs/history/todo-archive-2026-08.md` **Part 2**. Contracts:
`docs/contracts/phase4-forge-overview.md` + `P62`/`P63`/`P64`/`P65-*.md`. Command count reached
**157** here (P62 +7, P63 +1, P64 +1, P65 +1).

- **P62 — forge foundation (GitHub first)** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P62-user-checklist.md`). AI gate green: new `crates/bonsai-forge/` (60/0) +
  7 commands (cmd 154) + right-pane PR panel; harness verified connect→list→detail→create.
  Native half = a real GitHub PAT against real PRs.
- **P63 — forge signals on the graph** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P63-user-checklist.md`). AI gate green: batch `forge_commit_statuses` (cmd 155),
  `forgeBadges.ts` + `useForgeSignals`, PR/CI badges on branch-tip pills, Settings toggles.
  Native half = the canvas badge pixels and canvas click→PR.
- **P64 — GitLab + Bitbucket + Azure DevOps + AI PR descriptions** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P64-user-checklist.md`). AI gate green: `ai_generate_pr_description` (cmd 156),
  three more providers on the same trait (`bonsai-forge` 153/0), per-provider connect hints.
  Native half = live AI generation + real tokens per provider; two reviewer NITs are parked in that
  checklist §D (Azure bad-PAT HTTP-203 message; `dev.azure.com/{org}/_git/{repo}` shorthand).
- **P65 — paged/streaming graph loading** — **awaiting USER CHECKPOINT**
  (`docs/contracts/P65-user-checklist.md`). AI gate green: shared `LaneWalker` + `stream_graph`
  channel (cmd **157**), `compute_graph` output byte-for-byte unchanged, frontend stream assembler,
  120k-commit correctness test. **Shipped with an honest first-paint reframe** — see the finding
  below. Native half = scroll feel + progressive load on a real large repo.
  - **FINDING (kept on the live board because it constrains future work):** libgit2's
    `Sort::TOPOLOGICAL` runs an eager `prepare_walk` before yielding row 0, so first paint is
    **O(total commits)** (release/warm: 40k ≈ 0.73 s, 120k ≈ 1.37 s, 200k ≈ 2.3 s), **not**
    O(first 512). Streaming still gives lane-stable progressive render, scroll-ahead and no giant
    IPC, but "instant first paint on 1M repos" is **not** met by the topo walk. `stream_perf.rs`
    deliberately does not assert a `<150 ms` target.

- **P66 — lazy generation-number topo order** — **deferred** (user decision 2026-08-10: the proper
  fix for the P65c finding is a Large build; not taking it on now). Feasibility spike:
  `docs/contracts/P65a-lazy-topo-spike.md` (VERDICT: TRACTABLE via path (c), effort **L**).
  Shape: reimplement git's lazy `--topo-order` (Stolee generation numbers) in Rust as a shared order
  stage replacing `seeded_revwalk`, sourcing generation numbers from the P52 commit-graph file via
  `gix-commitgraph` or an own parser (git2 0.21 / libgit2-sys 0.18 expose none — grepped, 0 hits).
  Committed P65a/P65b stay as-is (IPC / `GraphChunk` / `LaneWalker` unchanged); only the internal
  walk order changes. Costs: regenerate ALL graph fixtures (lazy order differs from libgit2 in
  commit-date **tie-breaks** only), guarded by a `get_graph ≡ stream_graph` equivalence test + a
  differential test vs `git rev-list --topo-order`. Pre-build: re-verify newest git2 still lacks the
  generation API (F5); confirm `gix-commitgraph` reads P52 split/chain graphs (F1). Architect advises
  **against** shelling out to `git log --topo-order` (would make the git binary a hard runtime dep of
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
**P67a** `0ec69f9` · **P67b** `e607d2c` · **P67c** `5e68db5` · **P67e** `d50361a` · **P67d** `f3ca6e5`
(docs). Nothing here is user-confirmed: **the dashed guideline has never been seen by anyone** (see
the NOT-self-certifiable note below). Still green at HEAD `44067af` after P68 landed on top of it:
tsc 0 · vitest **1580 / 128 files** · e2e **104 passed / 1 skipped** ·
`cargo clippy --workspace --all-targets -- -D warnings` clean.

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

## 🐞 SPUN-OUT ITEMS (found during P68, deliberately NOT bundled into it)

### CommandPalette highlight resets on `actions` array identity — **OPEN**
`src/components/CommandPalette.tsx:103→107→118` re-lands the highlight on the first enabled row whenever
the `actions` **array identity** changes, and `filterActions` always returns a fresh array. Any producer
whose memo deps churn therefore steals the user's keyboard selection mid-typing:
- **P65** bumps `graph` identity per streamed batch → the highlight jumps while a large graph streams
  (this is the real root cause of the `e2e/09-search-palette` flake, which was worked around by
  settling the stream first rather than fixed).
- **P68e** made it fire ~once a **second** during any live AI run (inline-arrow palette thunks +
  `focusDock` closing over `orderedRuns`). The P68e-side churn was fixed; the component was not.
**Correct fix (reviewer's recommendation):** reset on the filtered **ids**, not array identity — that
immunises every future `actions` producer instead of requiring each one to stay identity-stable forever.
Do NOT keep patching producers.

### `App.tsx` per-field settings pattern is driving unbounded growth — **DONE 2026-08-18**
Was: 1212 lines, each new setting costing N × `useState` + N × `if (patch.x !== undefined)` +
N × `setX(s.x)` + N × prop-pass. P67c (`panelDensity`) and P68e (dock height/collapsed) each paid it.
**Closed** by extracting `src/hooks/useUiSettings.ts` (210) — App.tsx **1212 → 1120**, zero call sites
changed (no prop name or type moved, so the blast radius stayed inside App + the hook). Same pass took
`RepoWorkspace.tsx` **3087 → 2948** via `repoWorkspace/usePartialStaging.ts` (245). Reviewed APPROVE;
all six equivalence claims verified, including the one non-verbatim edit (11 names added to 8 dep
arrays, each confirmed a container-level `useState` setter or `useRef`). Guarded by
`useUiSettings.test.tsx` — 9 tests, **all 7 behaviours negative-controlled**. Baseline re-locked, so the
reclaimed floor cannot silently regrow; `session.rs`, `stream.rs` and `useAiRuns.test.tsx` fell off the
over-500 list entirely.

### Persisted-settings write path has three latent defects — **OPEN** (all pre-existing, found 2026-08-18)
Found while extracting `useUiSettings`; none introduced by it (each verified against `git show HEAD`).
1. **Four writers bypass the debounced merge.** `handleSettingsChange` coalesces into one 300 ms
   `setUiSettings`, but `closeOnboarding` (`{onboardingSeen}`), `toggleTheme`, `toggleListView` and
   `commitPaneWidths` each fire their own independent write. Benign **only** because the key sets happen
   to be disjoint today — the ordering is unguaranteed, so any future overlap silently loses a field.
2. **A failed write is dropped and never retried.** `pendingSettingsPatchRef` is emptied *before* the
   `ipc.setUiSettings` call, so on rejection the merged patch is gone: the user gets a toast while the UI
   keeps showing values the disk does not have. Pinned as current behaviour by a test, not fixed.
3. **No unmount flush** for `settingsSaveTimerRef`. A pending patch still fires after unmount (so it is
   not lost in-session), but it is lost if the JS context dies inside the 300 ms window — app quit or
   window close right after a knob change — and a late write can outlive the unmount and race a read.

### `docs/contracts/P68e-ai-activity-dock.md` is 1064 lines and now under-describes shipped code — **OPEN**
Twice the ~500-line house limit. Also stale: P68g-1 (audit M3) added two elements to the ask block — an
untrusted-model-output attribution line and a fixed "Bonsai never asks for passwords or tokens" guard —
and made `aria-describedby` a two-id list, none of which §4.1/§4.2 describe. `ui-designer` produced
splice-ready replacement blocks in `docs/contracts/P68g-ui.md` §3.1–§3.5 rather than rewriting a
1064-line canonical contract wholesale with no line-level edit tool (correct call — a truncated write
would have destroyed it). **Needs: apply the splice, then split the file.**

### `docs/contracts/P68-ai-conflict-streaming.md:304` is one module level stale — **OPEN**
Says `session_drain_tests.rs` is `#[path]`-included "as a child of `session`". After the `session.rs`
split it is a child of `session::session_drain` — still a descendant, so the privacy claim holds, but the
wording is out of date.

### `NumberSlider` clamps mid-typing, so a field's own minimum is hard to type — **OPEN**
`src/components/NumberSlider.tsx` commits on every `change` while the input is controlled, so with
`min = 60` a user typing `6` then `0` lands on **600**, not 60 (the field snaps to 60 after the first
keystroke, then the next digit appends). Verified. Pre-existing and shared by **every** settings slider,
so it is not P68-specific — but P68g's new limits fields (idle `min 60`, default 300) are where it is
most likely to bite. The USD field in `SettingsAiLimits` already dodges it with a local draft string,
because `Number('12.')` → `12` makes `12.50` otherwise unenterable. **Fix = move NumberSlider to
draft-string + commit-on-blur/Enter**, which changes commit semantics for every settings slider and so
needs its own increment with its own review — deliberately NOT bolted onto a security milestone.
Related and already fixed in P68g-2: clearing the field used to snap the setting to `min`
(`Number('') === 0`), contradicting the component's own doc comment.

### `STDERR_GRACE_TOTAL` is not the absolute cap its doc comment claims — **OPEN**
`drain_stderr` checks `Instant::now() < deadline` *before* each `recv_timeout(STDERR_GRACE)`, so the
drain can run up to `STDERR_GRACE_TOTAL + STDERR_GRACE` (~1150 ms vs the documented 1000 ms). Visible
only as ≤150 ms extra shutdown latency; the existing test's 500 ms slack passes either way.

### `cargo fmt` has never been run on this repo — **OPEN**
No `rustfmt.toml` anywhere, no fmt check in any hook or CI. `cargo fmt --all --check` reports **1773
hunks across 221 files**; `--config use_small_heuristics=Max` is *worse* (2065), so no single config
matches the existing hand style. Right shape: its own commit — pick a config, add `rustfmt.toml`,
one-shot reformat, then add `cargo fmt --check` to the gate. **Do it between milestones, never inside
one** (it would bury a review in a mechanical diff).

---

### P68 — streaming/interactive/bulk AI conflict resolution — ✅ **AI GATE PASSED** (awaiting native USER CHECKPOINT)

**Current step:** ✅ **P68 CODE-COMPLETE — AI gate PASSED at HEAD `44067af`, awaiting native USER
CHECKPOINT** (`docs/contracts/P68-user-checklist.md`; the four runs that matter are listed at the end
of it). Every sub-increment is committed and reviewer-approved. **Nothing about appearance or
real-CLI behaviour has been verified by anyone** — the harness is headless (no compositing, `rAF`
paused, `computer{screenshot}` fails outright), so no live log, no real tool use, no real cost and no
pixel has been seen.

**Final AI gate (measured at HEAD `44067af`, 2026-08-18):** tsc **0** · vitest **1580 passed / 128
files** · e2e **104 passed / 1 skipped / 0 failed** · eslint **29 warnings, 0 errors** ·
`check-file-size` exit **0** · cargo `bonsai-core --lib` **764 passed / 0 failed / 1 ignored** ·
cargo `bonsai --lib` **238 / 0** · `cargo clippy --workspace --all-targets -- -D warnings` clean ·
IPC commands **160**.

Contracts: `docs/contracts/P68-ai-conflict-streaming.md` (invariants D1–D16, ambiguities A1–A12
pre-resolved) · `docs/contracts/P68e-ai-activity-dock.md` (dock UI) · `docs/contracts/P68g-ui.md`
(settings + consent copy + ask-block) · `docs/contracts/P68-security-audit.md` (audit) ·
`docs/contracts/P68-user-checklist.md` (native checklist).
**Commands 157 → 160** (`ai_resolve_conflict_stream` = Channel, `ai_cancel_run`, `ai_reply_run`).

**Sub-increments — all committed:**
- **P68a** `0f154b2` — Rust streaming runner core (`ai/stream.rs`, `ai/session.rs`, `ai/registry.rs`,
  `RunLimits`/`ToolPolicy`/`run_claude_streaming`, extracted `parse_result_envelope` +
  `kill_pid_tree`, `AppError::AiCancelled`, 6 NDJSON stub modes). Partial output is now **kept** on
  cancel/watchdog (the old `ai/mod.rs` discarded it).
- **P68b** `451457e` — the 3 commands (`commands/ai_stream.rs`, managed `AiRunRegistry`, new
  settings, read-only `--tools "Read,Grep,Glob"` allowlist, bulk split/attribution in Rust).
- **P68c** `8d727de` — TS types + Channel bridge + `mock/handlers/aiStream.ts`
  (`?aiSlow`/`?aiAsk`/`?aiFail`, and `?ai=off` now honoured).
- **P68d** `76de1bb` — per-path store `useAiRuns.ts` + row feedback; `aiResolvingPath` deleted; the
  `fileDiffReqId` coupling broken ← **the item-5 fix**.
- **P68e** `a75a585` — bottom dock `AiActivityPanel` + `AiActivityLog` (third child of
  `.workspace-host`).
- **P68f** `f1096aa` — bulk single-run resolve (`useBulkAiResolve.ts`, `AiRunQueue.tsx`,
  `BulkAiResolveButton.tsx`, `dialogs/BulkAiConfirmDialog.tsx`) + `e2e/18-ai-bulk-resolve.spec.ts`.
- **P68g-1** `96295ef` — security hardening (audit items 1, 3, 4, 5, 6 + L6).
- **P68g-2** `44067af` — the eight AI-run settings UI, honest consent copy, `autoResolve` caveat at
  the point of choice, bulk-dialog "one or more runs" correction, ask-block attribution + the
  never-asks-for-secrets guard (audit item 2 — the one P68g-1 deferred).
- Supporting: `cb85e55` `useUiSettings` + `usePartialStaging` extraction · `8254b46` e2e
  persistence-race fix.

**ADDITIONAL LOCKED DECISIONS (asked + answered 2026-08-17):**
- **Spend cap = NONE by default**, configurable (`ai_max_budget_usd` default `0.0` = no cap).
  `--max-budget-usd` is only passed when > 0; live cost shows in the dock so a runaway is visible.
- **Bulk + `autoResolve` = STAGE the marker-free files** from the run; any file still carrying
  conflict markers falls back to review (`hasUnresolvedMarkers` gate). First place Bonsai stages
  several files from one AI call — nothing is committed, everything stays visible and revertible.

**Orchestrator-settled OQs (recorded so they are not re-litigated):**
- OQ1 concurrency cap = **3** (`AI_MAX_CONCURRENT_RUNS`). 1 would re-create the reported "every AI
  button disabled" bug; unbounded invites a rate-limit wall.
- OQ4 = **both** "Resolve all with AI" entry points (conflicts header + merge banner).
- OQ5 = dock adoption by the other six AI runners stays **deferred** past P68e (props are generic
  over run key, so they can adopt it later without rework).
- OQ6 = P68b ships a small Rust echo-helper binary via `BONSAI_CLAUDE_BIN` rather than treating
  "bulk payload + mid-run question" as checkpoint-only: that exact combination is where both serious
  P68a defects lived, and the `.cmd` stub provably cannot reach it (`set /p` ~1 KB ceiling).
- A1 (architect): `ai_resolve_conflict_stream(repo_id, paths: Vec<String>, on_event)` returning
  `AiResolveBatch` — a single-path call is just `paths.len() == 1`. Keeps bulk split/attribution in
  Rust (D1) and holds the command count at +3.

**⚠️ Durable warnings — do NOT "fix" these back:**
- **D16 — the session loop thread never blocks on I/O.** Reader threads are spawned **before**
  anything is written to stdin. The contract's original §3.3 pseudocode wrote the payload first,
  which deadlocks on the pipe buffer for any payload ≥64 KB (P68f's bulk payload is ~400 KB). The
  corollary is *exactly one* `WriteTx`, created once and never cloned — dropping it **is** the
  child's EOF. Contract re-synced accordingly (§2a, §3.1–§3.4, §4.x, §6.3, §8.x, §10.1, §12/A13).
- The 13 `RunOpts::default()` call sites are deliberately **not** migrated — the 90 s default stays
  for the unrelated AI features; streaming is an additive sibling. `commands/ai.rs` is unmodified.
- The pre-existing load-sensitive flake `ai::tests::run_claude_slow_times_out_and_reaps_child` was
  **fixed** in P68a (wall-clock assertion → monotonic lower bound + generous upper bound). It
  measured 2.97 s for a 1 s deadline at the P67 baseline; it was never a P68 regression.
- `StreamLogItem.assistant_text: bool` exists because `{ text }` alone cannot separate assistant
  prose from `⚙ tool(...)`/system/stderr decoration (D2/A5). Partial deltas deliberately do **not**
  set it, to avoid double-counting.

**Known limitations accepted up front:** sentinel-based questions (a model ignoring the convention
degrades to a normal answer, still caught by `hasUnresolvedMarkers`); no re-attach to an in-flight
run after a window reload; `total_cost_usd` may be cumulative across turns (display the last
`result`'s value, don't sum).

**Security follow-ups still OPEN** (audit items 7–11, each with its rationale in
`docs/contracts/P68-security-audit.md` — that file is canonical, do not duplicate them here): the
novel-content gate (the structural defeat for H1), proposals shown as a diff, bulk path-count cap +
per-batch reads + batch count in the dialog, process-group kill off Windows + zeroing `ctl.pid`
after reap, and a symlink-safe `resolve_conflict_text` write.

Full original board text for this milestone (with the drift it had accumulated):
`docs/history/todo-archive-2026-08.md` **Part 4**.

---

## Archive

| File | Covers |
|---|---|
| `docs/history/todo-archive-2026-08.md` | P65 → P28 build detail, the Phase 1–4 banners, and the resolved FOR-USER decisions (moved 2026-08-18) |
| `docs/history/todo-archive.md` | P27 → P2, M0–M6 |
| `docs/history/milestones-mvp.md` | the M0–M6 AI-gate vs USER CHECKPOINT split |
| `docs/history/context-pollution-audit.md` | the context/token-cost audit |
| `docs/contracts/INDEX.md` | one line per contract file — milestone, scope, status |

Move a milestone's section into `todo-archive-2026-08.md` only once **both** halves of its gate have
passed. A milestone with a pending USER CHECKPOINT stays on this board. Archiving is a **move**, never
a delete — condense on the board, keep the full text in the archive, and leave a pointer.
