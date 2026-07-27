# Bonsai — Milestone TODO

> Single source of truth for session resume. Keep the "Current step:" line of the
> in-progress milestone updated at every workflow transition.

## M0 — Scaffold — **in-progress**

Current step: M0 gate — all sub-increments committed (c090459, 0cd1d4d, b60fc36); tester
running; orchestrator verifying AI gate (cargo test / builds / browser harness). M0a committed (c090459), M0b committed (0cd1d4d).
Carry-over note for M1 contract: define bare-repo semantics (open_ext accepts bare repos;
M1 status/watcher assume a workdir). Prereqs installed: Rust 1.97.1 stable-msvc, VS Build Tools
2022 17.14, pnpm 11.17.0, Node 24, WebView2. Architect contract + `docs/contracts/ui-reference.md`
done.

Goal: Tauri v2 + React/Vite/TS project named **Bonsai** that opens a window via `pnpm tauri dev`
on Windows; folder picker (`tauri-plugin-dialog` + capability); Rust detects Git repo and reads
HEAD; pinned toolchain (`rust-toolchain.toml`, `packageManager`); mock IPC browser harness
(`src/ipc/mock.ts`, `VITE_MOCK_IPC=1`); architect UI reference spec.

Acceptance (AI gate): `cargo check` green; `pnpm build` green; browser harness renders;
Rust unit test opens fixture repo and reads HEAD.
Acceptance (USER CHECKPOINT): window opens via `pnpm tauri dev`; folder picker selects a repo;
HEAD shown.

## M1 — Working-directory status — pending
## M2 — Commit graph (centerpiece; sub-increments M2a–M2d) — pending
## M3 — Stage / unstage / commit — pending
## M4 — Diff view — pending
## M5 — Branches — pending
## M6 — Remotes (fetch / pull ff-only / push) — pending
## Polish — pending
