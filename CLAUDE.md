# Bonsai — Local Git Client — Orchestrator Instructions

> These instructions are auto-loaded every session. The main session acts as the **orchestrator**.
> Select **Fable 5** as the session model (`/model`) before starting; the subagents use
> `model: inherit`, so they run on whatever the session model is.

## Your role (main session = orchestrator)

You are the orchestrator for building a working local Git client on a Windows machine. You own the
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

**Bonsai** — a fast, native-feeling desktop Git client for Windows. The name is used everywhere:
window title "Bonsai", `productName: "Bonsai"` and identifier `com.bonsai.app` in `tauri.conf.json`,
npm package `bonsai`, Rust crate `bonsai`. Clean, minimal, GitButler-inspired UI feel,
with a **GitKraken-style commit graph as the centerpiece**: multi-colored branch lanes, smooth curved
edges where branches fork/merge, commit dots, ref pills (branch/tag/HEAD) beside commits, and smooth
scrolling over large histories (target: **20k+ commits without jank**).

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
- The `notify` watcher is **always** paired with a manual refresh button and a rescan on window focus
  (it misses events on Windows), and its events are **debounced** (~300 ms) — Git operations fire
  event storms.
- **Browser harness (mandatory):** the React frontend must always run in a plain browser via
  `pnpm dev` with a **mock IPC layer** (`src/ipc/mock.ts`, selected by `VITE_MOCK_IPC=1`) that
  serves fixture data (canned `GraphLayout`, status lists, diffs). Real Tauri `invoke` otherwise.
  This is how the orchestrator visually verifies the UI (screenshots of the canvas graph, console
  frame timing) — the AI cannot see the native Tauri window. senior-dev keeps the mock layer
  compiling and updated with every IPC change.

## Suggested project layout

```
src/                  React frontend (Vite)
  ipc/                typed wrappers over invoke()/listen(); mock.ts (VITE_MOCK_IPC harness)
  graph/              canvas renderer for the precomputed layout
  components/
src-tauri/            Rust backend
  src/lib.rs          builder, generate_handler!, .manage(state)
  src/commands.rs     #[tauri::command] API surface
  src/git/            git2 wrappers: status, log, diff, commit, branches, remotes
  src/graph.rs        lane/edge layout engine
  src/watcher.rs      notify -> emit("repo-changed")
  Cargo.toml
  tauri.conf.json
docs/contracts/       architect's per-milestone contract files (M<N>-<slug>.md)
TODO.md               running milestone status + current step (session resume source of truth)
```

## Reference contracts (starting point — architect may refine)

```rust
struct GraphNode { id: String, lane: u32, row: u32, parents: Vec<String>, refs: Vec<RefLabel> }
struct GraphEdge { from: String, to: String, lane: u32 }
struct GraphLayout { nodes: Vec<GraphNode>, edges: Vec<GraphEdge>, lane_count: u32 }
// commands: open_repo(path), get_status(), get_graph(), get_diff(path),
//           stage(path), unstage(path), commit(msg), list_branches(),
//           checkout(name), create_branch(name), fetch(), pull(), push()
// events: "repo-changed"    channels: streamed large diffs / batched log
```

## Gate verification — AI gates vs USER CHECKPOINTs

Every milestone gate splits into two kinds of checks:

- **AI gate** — verifiable by the orchestrator alone: tests, benchmarks, CLI-output comparison,
  and **browser-harness verification** (open `pnpm dev` with `VITE_MOCK_IPC=1` in the browser
  pane, screenshot the UI, read console frame-timing logs).
- **USER CHECKPOINT** — anything requiring the native Tauri window or human perception (window
  opens, folder picker works, real-app scroll feel). The orchestrator must NEVER declare these
  passed itself: present the AI-gate evidence, then explicitly ask the user to run
  `pnpm tauri dev` and confirm. A milestone is done only when both halves pass.

## Milestones (MVP-first; one at a time, each with a passing gate)

- **M0 — Scaffold.** Tauri v2 + React/Vite/TS project that opens a window via `pnpm tauri dev` on
  Windows; a folder picker (use `tauri-plugin-dialog` and grant its capability in
  `src-tauri/capabilities/`); Rust detects whether the folder is a Git repo and reads HEAD. Pin the
  toolchain (`rust-toolchain.toml`, `packageManager` in package.json). Architect also delivers a
  one-page UI reference spec (lane color palette, spacing, typography, dark/light) reused by all
  later milestones.
  *AI gate:* project compiles (`cargo check`, `pnpm build`); browser harness renders; a Rust unit
  test opens a fixture repo and reads HEAD. *USER CHECKPOINT:* window opens via `pnpm tauri dev`,
  folder picker selects a repo, HEAD shown.
- **M1 — Working-directory status.** Show staged / unstaged / untracked files via git2; auto-refresh
  via notify + manual refresh + refocus rescan.
  *AI gate:* Rust tests compare output to `git status --porcelain` on scratch repos; harness renders
  the file lists from mock data. *USER CHECKPOINT:* auto-refresh + manual refresh + refocus rescan
  behave correctly in the native app.
- **M2 — Commit graph (centerpiece).** Rust computes `GraphLayout` from a commit walk; React renders
  the GitKraken-style canvas graph, virtualized, with ref pills. Run as four sub-increments, each
  its own implement→review loop:
  - **M2a** — layout engine + unit tests (pure Rust, no UI).
  - **M2b** — canvas rendering of a static precomputed layout (verified by browser-harness
    screenshot).
  - **M2c** — virtualization, scrolling, ref pills, HiDPI canvas scaling.
  - **M2d** — perf gate: fixture generator + criterion benchmark.
  *AI gate:* lane/edge unit tests pass on tricky fixture histories; harness screenshots show correct
  lanes, curved fork/merge edges, dots, and ref pills; a scripted generator (git2 or
  `git fast-import` — NOT 20k CLI commits) builds a synthetic 20k+ commit fixture repo; criterion
  shows layout < 500 ms for 20k commits; harness scroll test over the 20k layout logs rAF frame
  timings to the console with no sustained frames > 33 ms. *USER CHECKPOINT:* scrolling the 20k repo
  in the native app feels smooth.
- **M3 — Stage / unstage / commit.** *AI gate:* Rust tests verify results against the `git` CLI on
  scratch repos. *USER CHECKPOINT:* stage/unstage/commit round-trip in the native app.
- **M4 — Diff view.** File diffs via git2, unified or side-by-side. *AI gate:* diff output matches
  `git diff` in tests; harness renders diffs from mock data. *USER CHECKPOINT:* diffs display for a
  real repo in the native app.
- **M5 — Branches.** List, create, checkout, delete; show current branch/HEAD.
  *AI gate:* verified against the CLI in tests; code review confirms destructive ops require UI
  confirmation. *USER CHECKPOINT:* branch operations + confirmation dialog work in the native app.
- **M6 — Remotes.** Fetch / pull / push with credential handling. Credential strategy: use git2's
  `CredentialHelper` (delegates to Git's configured helper, i.e. Windows Credential Manager) first,
  then SSH agent for ssh URLs; never prompt for or store raw passwords ourselves. Confirm this with
  the user at milestone start before implementing.
  *AI gate:* fetch/pull/push round-trip works against a **local bare repo** (`git init --bare`,
  added as a `file://` remote on the scratch repo) — no network or credentials needed, fully
  autonomous. *USER CHECKPOINT:* one round-trip against a real network remote with the credential
  helper.
- **Polish.** Keyboard shortcuts, error toasts, empty/loading states, GitButler-clean styling.

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
3. **Decompose the milestone into sub-increments**, each sized for a single fresh-context
   senior-dev pass (M2 is pre-split above; split the others yourself as needed). For each
   sub-increment, delegate to `senior-dev`: implement to the contract (pass the contract **file
   path** + the exact source file paths).
4. Delegate to `reviewer`: review the working-tree diff since the last commit against the contract
   file + acceptance criteria.
5. Route must-fix items back to `senior-dev`; repeat 3–4 until the reviewer approves. **Commit each
   approved sub-increment** (`wip(M<N>): ...`) so review diffs stay small and resume points exist.
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

## Windows shell discipline

- Pick ONE shell dialect per command: the Bash tool is Git Bash (POSIX — `/d/...` paths, `$VAR`);
  PowerShell is a separate tool with its own syntax. Never mix them in one command.
- Quote all Windows paths; expect spaces. Line endings are normalized to LF via `.gitattributes`.

## Guardrails

- Do all Git experimentation in a **scratch repo you create** (init a temp folder with fixture
  history). **NEVER** run push/reset/rebase/clean/`branch -D` against real repositories.
- Any destructive Git operation in the app must require **explicit UI confirmation**.
- **Commit after each green milestone.**
- Prefer small compiling increments; run `cargo check`/`clippy` and `tsc`/`build` frequently.
- If a decision is ambiguous or you're blocked, **ask the user — do not guess.**

## Environment (Windows prerequisites — verify first)

Rust (rustup + MSVC build tools, which also give the C toolchain the `git2` crate needs to vendor
libgit2), Node LTS + pnpm, WebView2 (bundled on Windows 11), and the Tauri CLI.

## Definition of done (v1)

Runs on Windows via `pnpm tauri dev` and a release build; can open a repo, view a GitKraken-style
commit graph, see status, stage/unstage, commit, view diffs, manage branches, and fetch/pull/push —
all verified against a scratch repo, with automated tests covering the graph-layout algorithm.

## Subagents (`.claude/agents/`)

- **`architect`** — designs module boundaries, Rust/TS interface contracts, the IPC surface, and the
  commit-graph algorithm. Writes contract files to `docs/contracts/` only; never edits application
  code.
- **`senior-dev`** — the implementer. Writes all Rust/TS/React to the architect's contracts.
- **`reviewer`** — read-only diff review (correctness, boundary, performance, safety). Reports
  MUST-FIX / SHOULD-FIX / NIT + a verdict; never edits code.
- **`tester`** — writes/runs `cargo test` + fixtures and a frontend smoke checklist. Touches only test
  code and fixtures; never edits application code to make a test pass.

To begin or resume, follow `.claude/orchestrator-kickoff.md`.
