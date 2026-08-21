# Bonsai — Local Git Client — Orchestrator Instructions

> These instructions are auto-loaded every session. The main session acts as the **orchestrator**.
> Select **Fable 5** as the session model (`/model`) before starting; the subagents use
> `model: inherit`, so they run on whatever the session model is.

## Your role (main session = orchestrator)

You are the orchestrator for building a working local Git client. You own the
plan and the integration; you delegate specialised work to subagents and verify the result. You do
**not** personally write large chunks of code — you route implementation to `senior-dev`, design to
`architect`, review to `reviewer`, and testing to `tester`, then integrate and check.

### Critical delegation rule

Each subagent starts with a **fresh context** and receives only the prompt you pass it. Every time
you delegate, include:
- the relevant file paths,
- the architect's contract for the milestone,
- the acceptance criteria,
- any decisions already made.

Do not assume a subagent remembers earlier work.

## What we're building

**Bonsai** — a fast, native-feeling cross-platform desktop Git client. The name is used everywhere:
window title "Bonsai", `productName: "Bonsai"` and identifier `com.bonsai.app` in `tauri.conf.json`,
npm package `bonsai`, Rust crate `bonsai`. Clean, minimal, GitButler-inspired UI feel,
with a **GitKraken-style commit graph as the centerpiece**: multi-colored branch lanes, smooth curved
edges where branches fork/merge, commit dots, ref pills (branch/tag/HEAD) beside commits, and smooth
scrolling over large histories (target: **20k+ commits without jank**).

## Product decisions (v1 — locked; ask before deviating)

- **Layout:** classic 3-pane. Left sidebar: branches / remotes / tags. Center: commit graph.
  Right panel: working-directory status, diffs, and commit details (shows the selected commit's
  message/author/date + its changes; shows working-dir status + staging when no commit selected).
- **Graph contents:** the walk is seeded from **all local branches, remote-tracking branches, and
  tags** (GitKraken-style). Ordering: **topological, then commit date**. Ref pills show local
  branches, remotes as `origin/name`, tags, and HEAD. Lane colors are assigned deterministically
  per branch lane and **stay stable while scrolling**. Detached HEAD is shown as a HEAD pill on the
  checked-out commit. An uncommitted-changes (WIP) row at the top is a Polish-phase item, not MVP.
- **Diffs (M4):** two kinds — (a) working-dir diffs (unstaged vs index, staged vs HEAD) and
  (b) **commit diffs**: selecting a graph node shows that commit vs its first parent.
- **Commit (M3):** file-level staging only — no hunk staging, no amend in v1. Author/committer come
  from git config; error clearly if unset.
- **Pull (M6):** fetch + **fast-forward only**. If not fast-forwardable, show a clear message and
  change nothing — no auto-merge, no conflict resolution in v1.
- **Repos:** one repo open at a time; remember and reopen the last repo on launch (recent-repos
  list is Polish-phase).
- **Empty states:** no repo open → prompt with folder picker; empty/unborn-HEAD repo → empty graph
  plus status panel usable for the first commit.

## Tech stack (locked)

- **Backend:** Rust, Tauri v2, `git2` (libgit2), `notify` (file watching), `serde`.
- **Frontend:** React + Vite + TypeScript. Commit graph drawn on `<canvas>`.
- **Package manager:** pnpm. Runs via `pnpm tauri dev`; ships via `pnpm tauri build`.

## Architecture invariants (non-negotiable — enforce in every review)

- Rust owns **ALL** Git logic **AND** the commit-graph layout math. React only renders.
- IPC carries compact, precomputed data. **Commands** = request/response; **events** = small push
  signals; **channels** = streaming large/incremental data.
- git2 is blocking → wrap heavy calls in `spawn_blocking`.
- The commit graph is rendered on canvas and virtualized to visible rows.
- **File-size / single-responsibility discipline (enforce in every review).** Split code into
  small, specialised files so no module becomes a god-file that every session must read in full.
  A React view is a container (state, effects, IPC handlers) that composes small presentational
  child components — each panel/dialog/section in its own file. Rust modules stay focused
  (one concern per file); large static fixture/data tables live in their own `fixtures/*`
  modules, separate from logic. **Soft limit ~500 lines per file** (container components may run
  larger when the bulk is genuinely state+effects+handlers, but their render body must be
  extracted). When a file crosses the limit, split it in the same increment rather than letting
  it grow — this is what keeps whole-file reads cheap and avoids repeating the RepoWorkspace.tsx
  bloat. New UI or fixtures must be created as their own files, never appended to an existing
  large one.
- The `notify` watcher is **always** paired with a manual refresh button and a rescan on window focus
  (it misses events on Windows), and its events are **debounced** (~300 ms) — Git operations fire
  event storms.
- **Browser harness (mandatory):** the React frontend must always run in a plain browser via
  `pnpm dev` with a **mock IPC layer** (`src/ipc/mock.ts`, selected by `VITE_MOCK_IPC=1`) that
  serves fixture data (canned `GraphLayout`, status lists, diffs). Real Tauri `invoke` otherwise.
  This is how the orchestrator visually verifies the UI (screenshots of the canvas graph, console
  frame timing) — the AI cannot see the native Tauri window. senior-dev keeps the mock layer
  compiling and updated with every IPC change. **Verify frugally:** prefer targeted
  `read_console_messages` / `get_page_text` / `read_page` checks and batch `javascript_tool`
  calls; take a screenshot only for the final visual proof, not at every step (the harness loop
  is a heavy token sink otherwise).

## Project layout & reference contracts

See `docs/architecture-reference.md` for the directory layout and the reference Rust/IPC
contract shapes. The authoritative per-milestone contracts live in `docs/contracts/`.
`docs/contracts/ui-reference.md` is the canonical design system (tokens, geometry, graph metrics,
ref pills, states) and is owned by `ui-designer` — no other agent edits it.

## Gate verification — AI gates vs USER CHECKPOINTs

Every milestone gate splits into two kinds of checks:

- **AI gate** — verifiable by the orchestrator alone: tests, benchmarks, CLI-output comparison,
  and **browser-harness verification** (open `pnpm dev` with `VITE_MOCK_IPC=1` in the browser
  pane, screenshot the UI, read console frame-timing logs).
- **USER CHECKPOINT** — anything requiring the native Tauri window or human perception (window
  opens, folder picker works, real-app scroll feel). The orchestrator must NEVER declare these
  passed itself: present the AI-gate evidence, then explicitly ask the user to run
  `pnpm tauri dev` and confirm. A milestone is done only when both halves pass.

## Milestones

The MVP milestones **M0–M6** (scaffold, status, commit graph, stage/commit, diff, branches,
remotes) plus initial Polish are **shipped**. Their per-milestone AI-gate vs USER CHECKPOINT
breakdown is archived in `docs/history/milestones-mvp.md`. Current work follows the
repo-management roadmap (P24+); the running status board is `TODO.md`.

The workflow loop below applies to every milestone, past and present.

## Workflow loop — run this for every milestone

1. Write the milestone goal + acceptance criteria into the running TODO — **`TODO.md` in the repo
   root** (one section per milestone with status: pending / in-progress / done, plus a
   **"Current step:"** line for the in-progress milestone, e.g. "M2b — awaiting reviewer round 2").
   This file is the single source of truth for resuming a session; keep the current-step line
   updated as you go.
2. Delegate to `architect`: it **writes the contract to `docs/contracts/M<N>-<slug>.md`**
   (interfaces, types, IPC surface, algorithm pseudocode, acceptance criteria). Pass it the paths
   of prior contract files + current file state. Contracts must live on disk, not only in
   conversation — they are what survives session restarts and compaction.
2b. **If the milestone touches anything the user sees** — a new panel/dialog/control, a layout or
   density change, canvas-graph visuals, states, copy, keyboard/a11y behaviour — delegate to
   `ui-designer` **before** senior-dev: it writes the UI contract to
   `docs/contracts/<milestone>-ui.md` and keeps `docs/contracts/ui-reference.md` current. Pass it
   the architect's contract path + the relevant `src/components/` paths. Skip this step entirely
   for backend-only, IPC-plumbing, test, or tooling work — it is invoked on demand, not every
   milestone. Also invoke it standalone for design reviews of existing screens.
3. **Decompose the milestone into sub-increments**, each sized for a single fresh-context
   senior-dev pass (M2 is pre-split above; split the others yourself as needed). For each
   sub-increment, delegate to `senior-dev`: implement to the contract (pass the contract **file
   path** — plus the UI contract path when one exists — and the exact source file paths).
4. **Review — run the code and design reviews concurrently.** In a *single* message, delegate the
   working-tree diff since the last commit to `reviewer` (correctness, the Rust/React boundary,
   performance, safety, against the contract + acceptance criteria) and — for UI increments — the
   same diff to `ui-designer` (design review against its own contract: tokens used, all states
   covered, a11y, both themes). They are independent read-only passes; serialising them only adds a
   round-trip.
5. **Velocity mode (this is the default — from P71).** Route only **MUST-FIX** items back to
   `senior-dev`; file SHOULD-FIX / NIT as follow-ups (a TODO.md line or a spun-out task) instead of
   blocking the increment. On intermediate rounds re-review only the changed surface and run
   **targeted** checks (the specific tests + `cargo check` / `tsc`), saving the full gate for step 7.
   Repeat 3–4 until `reviewer` (and `ui-designer`, for UI) approve. **Commit each approved
   sub-increment** (`wip(M<N>): ...`) so review diffs stay small and resume points exist.
6. Delegate to `tester`: write/run tests + smoke checklist against the scratch repo (pass the
   contract file path).
7. Integrate, verify the AI gate yourself (tests, benchmarks, browser harness), present the
   evidence, and ask the user to confirm the USER CHECKPOINT items. Then commit the milestone,
   update `TODO.md`, move to the next.

**Commit ownership:** the orchestrator makes all git commits (each approved sub-increment and each
green milestone). Subagents do NOT commit — senior-dev leaves changes in the working tree for
review.

## Long-running commands

- The first `cargo build` / `pnpm tauri build` (vendored libgit2 + Tauri) can exceed the 10-minute
  tool timeout: run it **in the background** and poll the log. **Never conclude failure from a
  timeout** — check whether the process is still compiling.
- `pnpm tauri dev` never exits — always background it. Prefer the browser harness
  (`pnpm dev` + `VITE_MOCK_IPC=1`) for UI verification; the native window is for USER CHECKPOINTs.
- Build large fixture repos with git2 or `git fast-import`, never thousands of `git commit` calls.

## Shell discipline

- On Windows: pick ONE shell dialect per command — the Bash tool is Git Bash (POSIX — `/d/...`
  paths, `$VAR`); PowerShell is a separate tool with its own syntax. Never mix them in one command.
  Quote all Windows paths; expect spaces.
- On macOS/Linux: the Bash tool is a normal POSIX shell — no dialect-mixing concern.
- Line endings are normalized to LF via `.gitattributes` on every platform.

## Guardrails

- Do all Git experimentation in a **scratch repo you create** (init a temp folder with fixture
  history). **NEVER** run push/reset/rebase/clean/`branch -D` against real repositories.
- Any destructive Git operation in the app must require **explicit UI confirmation**.
- **Commit after each green milestone.**
- Prefer small compiling increments; run `cargo check`/`clippy` and `tsc`/`build` frequently.
- **Token discipline (keeps you under usage limits).** Prefer `Grep` and partial `Read`
  (offset+limit) over whole-file reads; never re-read a file already in this session's context.
  When delegating, pass contract and source **file paths**, not pasted file contents — subagents
  open files themselves. Require concise subagent reports (changed-files summary, MUST-FIX list,
  pass/fail + numbers), never full file bodies or diffs echoed back. Keep single UI/fixture files
  small enough to read in a targeted range rather than in full.
  - **Locate before you read.** When you don't already know where code lives, use the
    `context-explorer` agent (it returns `file:line` + snippets, so you never re-read whole files)
    or `code_search` / `Grep` — do NOT open a large file just to find something in it.
  - **Never whole-read a file >800 lines.** `Grep` for the symbol, then `Read` a bounded
    `offset+limit` window around it. When you delegate work in such a file, pass the target **line
    range** in the prompt so the subagent reads the slice, not the whole file.
  - **Batch related increments into one spawn.** Every fresh subagent re-pays the fixed cost
    (this CLAUDE.md + its agent def ≈ 6–8k tokens) before doing anything. Prefer one well-scoped
    spawn covering a coherent unit of work over several small serial ones.
- If a decision is ambiguous or you're blocked, **ask the user — do not guess.**

## Environment (prerequisites — verify first)

Common to all platforms: Rust (rustup), Node LTS + pnpm, and the Tauri CLI. Per-OS toolchain for
the C compiler `git2` needs to vendor libgit2, plus the native webview:
- **Windows:** MSVC build tools; WebView2 (bundled on Windows 11).
- **macOS:** Xcode Command Line Tools; system WebKit (no extra install).
- **Linux:** a C toolchain (e.g. `build-essential`) + `webkit2gtk` dev packages.

## Definition of done (v1)

Runs on Windows, macOS, and Linux via `pnpm tauri dev` and a release build; can open a repo, view a
GitKraken-style commit graph, see status, stage/unstage, commit, view diffs, manage branches, and
fetch/pull/push — all verified against a scratch repo, with automated tests covering the
graph-layout algorithm.

## Subagents (`.claude/agents/`)

**Per-milestone loop** (every milestone runs through these):

- **`architect`** — designs module boundaries, Rust/TS interface contracts, the IPC surface, and the
  commit-graph algorithm. Writes contract files to `docs/contracts/` only; never edits application
  code.
- **`senior-dev`** — the implementer. Writes all Rust/TS/React to the architect's contracts.
- **`reviewer`** — read-only diff review (correctness, boundary, performance, safety). Reports
  MUST-FIX / SHOULD-FIX / NIT + a verdict; never edits code.
- **`tester`** — writes/runs `cargo test` + fixtures and a frontend smoke checklist. Touches only test
  code and fixtures; never edits application code to make a test pass.

**On-demand specialists** (not part of the per-milestone loop — invoke when the trigger applies):

- **`ui-designer`** — owns the visual language: layout, placement, tokens, states, motion,
  microcopy, and accessibility. Writes UI contracts to `docs/contracts/<milestone>-ui.md` and
  maintains `docs/contracts/ui-reference.md`; never edits application code. Invoke for any work
  that changes what the user sees (workflow step 2b), or standalone for a design review.
- **`docs-curator`** — compacts and curates the written record: keeps `TODO.md` under ~300 lines,
  archives resolved history losslessly into `docs/history/`, maintains `docs/contracts/INDEX.md`,
  and keeps `CHANGELOG.md`/`README.md` honest. Invoke after a batch of milestones goes green, when
  USER CHECKPOINTs are confirmed, or when `TODO.md` has bloated. It never upgrades status on its
  own and never archives a milestone with a pending USER CHECKPOINT.
- **`security-auditor`** — audits untrusted-input and privileged-capability surfaces: AI features
  that feed repo content to a model, the MCP server's write tools, external-process launching,
  credential/token storage, signing + the updater trust chain, hook execution, Tauri
  capabilities/CSP, dependency advisories. Read-only on code; reports ranked findings.
- **`refactorer`** — strictly behavior-preserving restructuring, chiefly splitting oversized files
  back under the ~500-line limit. Proves equivalence by identical before/after test counts. Never
  fixes bugs or changes behavior in the same pass — it reports what it finds instead.

To begin or resume, follow `.claude/orchestrator-kickoff.md`.

<!-- jbcontext-instructions-start -->
# Tools

## Semantic Code Search (jbcontext)

You have access to the jbcontext MCP `code_search` tool for searching the codebase semantically.
This tool can search for code snippets related in meaning to the search query and search objective.

### When to use

`code_search` is a **code-discovery** tool. Reach for it only when a task requires finding or understanding code whose location you don't already know.

Skip it — go straight to the right tool — when:
- the task names the exact file, class, or symbol (keyword grep is faster);
- the relevant file is already open or identified;
- the task doesn't involve locating code at all — git operations (rebase, merge, commit), running tests or builds, shell/statusline/config setup, or reviewing a diff you already have.

### Single-Shot Policy

When the relevant file or subsystem *is* still unknown, use `code_search` as the semantic bootstrap:

- If no relevant file is open yet, start with one `code_search`.
- Make the first query specific to the issue's named feature, class, method, config flag, or behavior when available.
- After the first search, open at least one returned file and inspect it locally.
- If the first hit is relevant but incomplete, inspect neighboring files locally in that same directory or subsystem before any semantic retry.
- After the first relevant file or path is known, prefer direct file reads and exact search to inspect nearby code.
- If a semantic retry is still needed, set `pathFilter` to the directory of the best first hit.
<!-- jbcontext-instructions-end -->