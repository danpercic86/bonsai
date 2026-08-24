---
description: Turn a spec into a technical plan respecting Bonsai's architecture invariants
argument-hint: [spec folder name or path, e.g. 003-branch-rename — defaults to the most recent spec]
---

You are turning an approved spec into a **technical plan**. This is step 2 of `/specify` →
`/plan` → `/tasks`.

Target spec: $ARGUMENTS

## What to do

1. **Locate the spec.** If `$ARGUMENTS` names a folder/path under `docs/specs/`, use it; otherwise
   use the most recently modified `docs/specs/*/spec.md`. Read it in full.
2. **Check for open `[NEEDS CLARIFICATION]` items.** If any remain unresolved, stop and ask the
   user — do not guess a resolution.
3. **Sanity-check scope against `CLAUDE.md`.** Re-read the "Architecture invariants" and "Product
   decisions (v1 — locked)" sections. If the spec implies violating an invariant (e.g. Git logic
   in TypeScript, a blocking git2 call off `spawn_blocking`, a new god-file) or deviates from a
   locked product decision, flag it in the plan's "Risks" section rather than silently working
   around it — ask the user before proceeding if the conflict is load-bearing.
4. **Locate relevant existing code** before proposing new structure — use `context-explorer` or
   `Grep`/`Glob` to find the modules this touches. Don't guess file layout; check
   `docs/architecture-reference.md` for the canonical directory layout.
5. **Write `docs/specs/<NNN-slug>/plan.md`** using the template below.
6. Report back: the plan's file path, a short summary, and whether this is small enough for
   `/tasks` to hand straight to `senior-dev`, or should instead be escalated to a full milestone
   (delegate to `architect` per `CLAUDE.md`'s workflow loop — e.g. it touches the IPC surface
   broadly, adds a new panel, or is otherwise milestone-sized).

## Plan template

```markdown
# Plan: <Feature/Fix Title>

**Spec:** ./spec.md
**Status:** draft

## Approach
2-5 sentences: the chosen technical approach and why, including any rejected alternatives worth
recording.

## Rust/TS boundary
What lives in Rust (git2 calls, graph math, business logic) vs. what React only renders, per
CLAUDE.md's invariant. Name the IPC command(s)/event(s)/channel(s) if any, with request/response
shapes.

## Files touched
- `path/to/file.rs` — what changes, ~how many lines
- `path/to/Component.tsx` — what changes
(Flag any file that would cross the ~500-line soft limit after this change — it needs a split in
the same increment, per CLAUDE.md.)

## New files (if any)
- `path/to/new_file.rs` — purpose

## Data model / types
Any new/changed Rust structs, TS types, or serde shapes. Keep this concrete enough that `/tasks`
and `senior-dev` don't need to re-derive it.

## Testing
What `tester` (or you, inline) should cover: unit tests, fixture repo scenarios, frontend smoke
checks via the mock IPC harness.

## Risks / open questions
Anything uncertain, including architecture-invariant conflicts found in step 3.
```
