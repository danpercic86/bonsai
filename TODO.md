# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

Environment: Rust 1.97.1 stable-msvc, VS Build Tools 2022 17.14, pnpm 11.17.0, Node 24, WebView2.
Cargo not on default PATH — `$HOME/.cargo/bin`. Browser harness: `pnpm dev:mock` (port 1420).

## M0 — Scaffold — **done** (2026-07-27)

AI gate passed (cargo test 6/6 + CLI cross-check, clippy, pnpm build, browser harness live incl.
mock isolation); USER CHECKPOINT confirmed by user: all 5 checklist items pass in `pnpm tauri dev`.
Contract: docs/contracts/M0-scaffold.md. Sub-increments: c090459, 0cd1d4d, b60fc36, a3cfadd.

## M1 — Working-directory status — **in-progress**

Current step: M1a (status core + command + porcelain tests) — implemented, cargo test 19/19 +
clippy green; awaiting reviewer round 1. (Split: M1a core, M1b watcher, M1c frontend.)

Goal: show staged / unstaged / untracked files via git2 in the right panel; auto-refresh via
notify watcher (debounced ~300 ms, emits "repo-changed") + manual refresh button + rescan on
window focus (notify misses events on Windows).

Acceptance (AI gate): Rust tests compare status output to `git status --porcelain` on scratch
repos; browser harness renders the file lists from mock data.
Acceptance (USER CHECKPOINT): auto-refresh + manual refresh + refocus rescan behave correctly
in the native app.

Carry-over decisions for the M1 contract: define bare-repo semantics (open_ext currently accepts
bare repos; status/watcher assume a workdir); fix cargo pdb filename collision warning
(bin/lib both named `bonsai`).

## M2 — Commit graph (centerpiece; sub-increments M2a–M2d) — pending
## M3 — Stage / unstage / commit — pending
## M4 — Diff view — pending
## M5 — Branches — pending
## M6 — Remotes (fetch / pull ff-only / push) — pending
## Polish — pending
