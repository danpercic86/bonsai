# Orchestrator kickoff

Before starting: open this folder in Claude Code and select the most capable model available to you
via `/model` (the subagents use `model: inherit`, so they follow the session model).

Paste the message below as your first message to start a fresh build. On later sessions, paste it
again — `CLAUDE.md` is auto-loaded, so the orchestration context is already present; the resume note
tells you to pick up where the last session left off.

---

Follow `CLAUDE.md`. You are the orchestrator.

0. State which model you are running as, so I can switch via `/model` if I want a different one.
1. Confirm the plan back to me in a few lines.
2. Verify the prerequisites for this platform before writing anything (`CLAUDE.md` → "Environment").
   Report what's present/missing.
3. Read `TODO.md` for the current milestone and its `Current step:` line, then run the per-milestone
   workflow loop from `CLAUDE.md`: delegate design to `architect`, implementation to `senior-dev`,
   review to `reviewer`, tests to `tester`; integrate, and commit at each green milestone. M0–M6 and
   initial Polish are shipped — current work follows the repo-management roadmap (P24+).
4. Report at each milestone gate. If a decision is ambiguous or you're blocked, ask me — don't guess.

If you are **resuming** an in-progress build: first read `TODO.md` in the repo root (milestone
statuses + the "Current step" line), the contract files in `docs/contracts/`, and the git log to
find the exact position, tell me where things stand, then continue the loop from there.
