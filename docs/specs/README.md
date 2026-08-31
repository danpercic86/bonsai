# `docs/specs/` — ad-hoc spec-driven workflow

This directory holds specs produced by the `/specify` → `/plan` → `/tasks` slash commands
(`.claude/commands/specify.md`, `plan.md`, `tasks.md`). Use it for **scoped, ad-hoc feature work
or fixes** — the kind of thing that doesn't warrant spinning up a full milestone.

This is a *lighter-weight sibling* to the milestone loop in `CLAUDE.md`, not a replacement for it:

| | `docs/specs/<NNN-slug>/` | `docs/contracts/M<N>-<slug>.md` |
|---|---|---|
| Written by | you, via `/specify` + `/plan` | the `architect` subagent |
| Scope | one small feature/fix | a full milestone |
| Contents | `spec.md`, `plan.md`, `tasks.md` | interfaces, IPC surface, algorithm pseudocode |
| Execution | you delegate tasks.md items directly | full workflow loop (steps 1-7 in `CLAUDE.md`) |

If a `/plan` reveals the work is actually milestone-sized (new panel, broad IPC surface change,
cross-cutting architecture work), stop and route it through the normal milestone loop with
`architect` instead — don't force it through `/tasks`.

## Layout

Each spec gets its own numbered folder, oldest first:

```
docs/specs/
  001-example-feature/
    spec.md    # what & why (from /specify)
    plan.md    # how, respecting CLAUDE.md's architecture invariants (from /plan)
    tasks.md   # ordered, delegatable checklist (from /tasks)
```

Numbering is sequential across all specs (not per-status). Once a spec's tasks are all complete
and committed, mark its `**Status:**` fields `done`; `docs-curator` may archive finished specs
into `docs/history/` during its normal sweeps, same as it does for milestones.
