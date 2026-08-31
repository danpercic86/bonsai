# P32 — USER CHECKPOINT smoke checklist (native `pnpm tauri dev`)

Manual verification for P32 (per-repo container + named worktrees + copy selected
uncommitted changes). The AI gate (unit + fs-oracle tests, clippy, `pnpm build`) is
green; these steps require the native Tauri window and human perception, so they
cannot be self-declared by the orchestrator.

Prep: `pnpm tauri dev`; open a real scratch repo that has **at least two local
branches** (e.g. `main` + `feature`) so a worktree can check out a distinct branch.
Seed the working directory with a mix of files before opening the create dialog:
- an **untracked** file (e.g. `.claude/skills/new-skill.md`),
- an **edited tracked** file (e.g. modify an existing `.claude/skills/*.md`, leave unstaged),
- a **gitignored** file (add a pattern to `.gitignore`, create a matching file),
- optionally a **staged** change (`git add` one file).

## 1. Create a worktree with a custom name
- [ ] Open the worktree create dialog; the **Name** field defaults to the selected branch.
- [ ] Change the branch → name auto-syncs to the new branch (until you edit it).
- [ ] Type a custom name distinct from the branch (e.g. branch `feature`, name `my-experiment`).
- [ ] The derived-path preview shows `<parent>/.worktrees/<repo>/my-experiment` (nested per-repo container).
- [ ] Submit → worktree is created on disk at `.worktrees/<repo>/my-experiment` and appears in the list.
- [ ] `git -C <that path> rev-parse --abbrev-ref HEAD` == `feature` (name decoupled from branch).

## 2. Copy a mix of files
- [ ] After picking the branch, candidate checkboxes load, grouped **Staged / Unstaged / Untracked / Gitignored**, all **unchecked** by default.
- [ ] The untracked skill file appears under **Untracked**; the edited tracked skill under **Unstaged** (or **Staged** if staged); the gitignored file under **Gitignored** (and NOT also under Untracked).
- [ ] Check a mix: the untracked skill + the edited tracked skill + the gitignored file.
- [ ] Submit → after creation, verify each checked file exists inside `.worktrees/<repo>/<name>/…` with the **main-workdir bytes** (open the copied edited skill; it shows your uncommitted edit, not the committed version).

## 3. Seeded conflict — Overwrite vs Skip
- [ ] Seed a conflict: pick a file that exists on the target branch with **different content** than the base, and edit it in the main workdir.
- [ ] In the dialog that file shows a **conflict** badge with an **Overwrite / Skip** toggle, defaulting to **Skip**.
- [ ] Leave it **Skip**, create → the file in the worktree keeps the **target branch's** checked-out version (your main-workdir edit is NOT written).
- [ ] Repeat with **Overwrite** selected → the worktree file now contains your **main-workdir** bytes.

## 4. Path landing
- [ ] All created worktrees and copied files land under `<parent>/.worktrees/<repo>/<name>/…` — never a sibling of the repo, never outside the container.
- [ ] Name-in-use advisory: type a name matching an existing worktree leaf → inline "name in use — backend will append -N" note; submit still succeeds and lands at `…/<name>-2`.

## 5. Plain create still works
- [ ] Open the dialog, pick a branch, leave every candidate **unchecked**, submit.
- [ ] A plain worktree is created (branch checked out, no extra copied files) — identical to pre-P32 behavior.
- [ ] Leaving conflicts as **Skip** with no clean files checked also routes to the plain-create path (no copy).

Report any deviation (wrong path, missing group, conflict default not Skip, bytes
mismatch, escape written outside the container) back to the orchestrator.
