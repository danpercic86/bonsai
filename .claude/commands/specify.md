---
description: Write a business-level spec for a feature or fix, before any design or code
argument-hint: <feature or fix description>
---

You are writing a **spec**, not a design or a plan. No tech stack, no file names, no APIs, no
code. This is step 1 of Bonsai's lightweight spec-driven flow (`/specify` → `/plan` → `/tasks`),
which sits *alongside* the milestone loop in `CLAUDE.md` — use it for scoped, ad-hoc feature work
or fixes that don't warrant spinning up a full milestone with the `architect` agent. If the ask
here turns out to be milestone-sized (new panel, new IPC surface, cross-cutting change), say so
and point back to the normal workflow loop in `CLAUDE.md` instead of proceeding.

Feature/fix request: $ARGUMENTS

## What to do

1. **Number and slug it.** List `docs/specs/`, find the highest `NNN-*` prefix, use `NNN+1`
   zero-padded to 3 digits, and a short kebab-slug from the request. Create
   `docs/specs/<NNN>-<slug>/spec.md`.
2. **Write the spec** using the template below. Pull only what's implied by the request and by
   Bonsai's locked product decisions (`CLAUDE.md` → "Product decisions (v1 — locked)") — don't
   invent scope. Where something is genuinely ambiguous, write
   `[NEEDS CLARIFICATION: <question>]` inline rather than guessing, and call it out to the user
   at the end instead of silently picking an answer.
3. **Do not** open source files to plan implementation, name Rust/TS types, or propose module
   structure — that's `/plan`'s job. This step is scoped to *what* and *why*, for *whom*, and
   *how we'll know it's done*.
4. Report back: the spec's file path, a one-paragraph summary, and any
   `[NEEDS CLARIFICATION]` items that need the user's answer before `/plan` can proceed.

## Spec template

```markdown
# <Feature/Fix Title>

**Status:** draft
**Created:** <today's date>

## Problem
What's broken or missing, for whom, and why it matters. 2-4 sentences.

## Goals
- Bullet list of what this must achieve.

## Non-goals
- Explicitly out of scope, so `/plan` doesn't over-build.

## User-facing behavior
Describe the observed behavior change as a user would experience it (screens, states, error
messages) — not implementation. Reference existing Bonsai UI conventions
(`docs/contracts/ui-reference.md`) by name where relevant instead of restating them.

## Acceptance criteria
Numbered, testable, unambiguous:
1. Given <context>, when <action>, then <outcome>.
2. ...

## Edge cases & error states
- What can go wrong, and what should the user see.

## Open questions
- `[NEEDS CLARIFICATION: ...]` entries, if any.
```
