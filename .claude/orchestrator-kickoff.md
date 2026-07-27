# Orchestrator kickoff

Before starting: open this folder in Claude Code and select **Fable 5** via `/model` (the subagents
use `model: inherit`, so they follow the session model).

Paste the message below as your first message to start a fresh build. On later sessions, paste it
again — `CLAUDE.md` is auto-loaded, so the orchestration context is already present; the resume note
tells you to pick up where the last session left off.

---

Follow `CLAUDE.md`. You are the orchestrator.

0. State which model you are running as. If it is not Fable 5, stop and tell me to switch via
   `/model` before doing anything else.
1. Confirm the plan back to me in a few lines.
2. Verify the Windows prerequisites before writing anything: Rust (rustup + MSVC build tools), Node
   LTS + pnpm, WebView2, and the Tauri CLI. Report what's present/missing.
3. Then begin **M0**. Run the per-milestone workflow loop from `CLAUDE.md`: delegate design to
   `architect`, implementation to `senior-dev`, review to `reviewer`, tests to `tester`; integrate,
   sanity-check with `pnpm tauri dev`, and commit at each green milestone.
4. Report at each milestone gate. If a decision is ambiguous or you're blocked, ask me — don't guess.

If you are **resuming** an in-progress build: first read `TODO.md` in the repo root (milestone
statuses + the "Current step" line), the contract files in `docs/contracts/`, and the git log to
find the exact position, tell me where things stand, then continue the loop from there.
