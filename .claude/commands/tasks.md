---
description: Break a plan into an ordered, delegatable task list
argument-hint: [spec folder name or path — defaults to the most recent spec]
---

You are turning a technical plan into an **ordered task list** ready for delegation. This is step
3 of `/specify` → `/plan` → `/tasks`. After this, you (the orchestrator) execute the tasks by
delegating to subagents per `CLAUDE.md` — this command only produces the checklist, it does not
implement anything itself.

Target spec: $ARGUMENTS

## What to do

1. **Locate the plan.** If `$ARGUMENTS` names a folder/path under `docs/specs/`, use it;
   otherwise use the most recently modified `docs/specs/*/plan.md`. Read both `spec.md` and
   `plan.md` in full.
2. **Decompose into tasks**, each one sized for a single fresh-context `senior-dev` pass (same
   sizing discipline as milestone sub-increments in `CLAUDE.md`). Order by dependency. Mark
   independent tasks `[P]` (parallelizable) if they touch disjoint files.
3. For each task, name: the file(s) it touches, a one-line description of the change, and which
   subagent owns it (`senior-dev` for implementation, `tester` for tests, `reviewer` for the
   review pass — mirroring the per-milestone loop). Include a final review + test pass even for
   small task lists — don't skip straight from implementation to "done".
4. **Write `docs/specs/<NNN-slug>/tasks.md`** using the template below.
5. Report back: the tasks file path and the task count, and ask the user whether to start
   executing now.

## Tasks template

```markdown
# Tasks: <Feature/Fix Title>

**Plan:** ./plan.md
**Status:** ready

- [ ] 1. <description> — `path/to/file` — owner: senior-dev
- [ ] 2. [P] <description> — `path/to/other_file` — owner: senior-dev
- [ ] 3. Review changes from tasks 1-2 — owner: reviewer
- [ ] 4. <test description> — owner: tester
...

## Notes
Anything a fresh-context subagent executing one of these tasks needs that isn't obvious from the
plan alone (e.g. "pass plan.md's IPC shape verbatim, don't re-derive it").
```

## Execution reminder (for you, the orchestrator)

When the user says go: delegate each task per `CLAUDE.md`'s delegation rule — pass the subagent
the plan/spec file paths, the specific task, and exact file paths; don't paste file contents.
Run reviewer after implementation tasks, tester after review passes. Commit with
`wip(spec-<NNN>): ...` once reviewer approves, same as a milestone sub-increment. Update the
tasks.md checkboxes as you go so this file stays the resumable source of truth, and flip
`**Status:**` in spec.md/plan.md/tasks.md to `done` once everything lands.
