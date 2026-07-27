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

## Suggested project layout

```
src/                  React frontend (Vite)
  ipc/                typed wrappers over invoke()/listen()
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

## Milestones (MVP-first; one at a time, each with a passing gate)

- **M0 — Scaffold.** Tauri v2 + React/Vite/TS project that opens a window via `pnpm tauri dev` on
  Windows; a folder picker (use `tauri-plugin-dialog` and grant its capability in
  `src-tauri/capabilities/`); Rust detects whether the folder is a Git repo and reads HEAD. Pin the
  toolchain (`rust-toolchain.toml`, `packageManager` in package.json). Architect also delivers a
  one-page UI reference spec (lane color palette, spacing, typography, dark/light) reused by all
  later milestones.
  *Done when:* window opens on Windows, a repo path can be selected, HEAD is read.
- **M1 — Working-directory status.** Show staged / unstaged / untracked files via git2; auto-refresh
  via notify + manual refresh + refocus rescan.
  *Done when:* output matches `git status` on the scratch repo, and refresh works even if the watcher
  misses an event.
- **M2 — Commit graph (centerpiece).** Rust computes `GraphLayout` from a commit walk; React renders
  the GitKraken-style canvas graph, virtualized, with ref pills.
  *Done when:* lanes/edges are correct on a fixture repo with merges and multiple branches, AND the
  perf gate passes: a scripted generator builds a synthetic 20k+ commit fixture repo, a criterion
  benchmark shows layout computes in < 500 ms for 20k commits, and scrolling that repo in the app
  stays smooth (no visible jank; render only visible rows, HiDPI-scaled canvas).
- **M3 — Stage / unstage / commit.** *Done when:* results verified against the `git` CLI.
- **M4 — Diff view.** File diffs via git2, unified or side-by-side. *Done when:* diffs match `git diff`.
- **M5 — Branches.** List, create, checkout, delete; show current branch/HEAD.
  *Done when:* verified via CLI; destructive ops require UI confirmation.
- **M6 — Remotes.** Fetch / pull / push with credential handling. Credential strategy: use git2's
  `CredentialHelper` (delegates to Git's configured helper, i.e. Windows Credential Manager) first,
  then SSH agent for ssh URLs; never prompt for or store raw passwords ourselves. Confirm this with
  the user at milestone start before implementing.
  *Done when:* a round-trip works against a test remote.
- **Polish.** Keyboard shortcuts, error toasts, empty/loading states, GitButler-clean styling.

## Workflow loop — run this for every milestone

1. Write the milestone goal + acceptance criteria into the running TODO — **`TODO.md` in the repo
   root** (one section per milestone with status: pending / in-progress / done). This file is the
   single source of truth for resuming a session.
2. Delegate to `architect`: interfaces/algorithm for this milestone (pass prior contracts + current
   file state).
3. Delegate to `senior-dev`: implement to that spec (pass the spec + the exact file paths).
4. Delegate to `reviewer`: review the diff against contract + acceptance criteria.
5. Route must-fix items back to `senior-dev`; repeat 3–4 until the reviewer approves.
6. Delegate to `tester`: write/run tests + smoke checklist against the scratch repo.
7. Integrate, run `pnpm tauri dev` yourself as a sanity check, commit the milestone, update
   `TODO.md`, move to the next.

**Commit ownership:** the orchestrator makes the git commits (at each green milestone, and at
intermediate working increments if useful). Subagents do NOT commit — senior-dev leaves changes in
the working tree for review.

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
  commit-graph algorithm. Design only; never edits code.
- **`senior-dev`** — the implementer. Writes all Rust/TS/React to the architect's contracts.
- **`reviewer`** — read-only diff review (correctness, boundary, performance, safety). Reports
  MUST-FIX / SHOULD-FIX / NIT + a verdict; never edits code.
- **`tester`** — writes/runs `cargo test` + fixtures and a frontend smoke checklist. Touches only test
  code and fixtures; never edits application code to make a test pass.

To begin or resume, follow `.claude/orchestrator-kickoff.md`.
