# P27 Worktrees — USER CHECKPOINT checklist (native `pnpm tauri dev`)

Use a **scratch repo only** (e.g. copy/init one under `D:\Temp\bonsai-scratch\p27-manual`).
Never a real repo. Prep: `git init -b main`, two commits, `git branch feature`,
`git branch feat/x`. Keep a terminal open in the repo for the `git worktree` cross-checks.

## 1. List + badges

- [ ] Open the scratch repo in Bonsai. Sidebar shows a **Worktrees** section with one row
      (the main worktree) badged **current** + **main**, branch `main`.
- [ ] In the terminal: `git worktree add ..\wt-feature feature` and
      `git worktree lock --reason "pinned" ..\wt-feature`, then refresh (button or refocus).
      The new row appears with branch `feature` and a **locked** badge; hovering the badge
      shows the reason. Rows match `git worktree list` exactly (paths + branches).
- [ ] `git worktree unlock ..\wt-feature` + `git worktree remove ..\wt-feature` in the
      terminal, refresh — the row disappears (cleanup for the next steps).

## 2. Create via "+"

- [ ] Click the section **+**. A branch picker lists eligible local branches
      (`feature`, `feat/x`; NOT `main` — it is already checked out).
- [ ] Pick `feature`. Success toast names the derived path. Verify on disk: a real
      directory exists at `<repo parent>\.worktrees\feature` and
      `git worktree list` shows it with branch `feature`.
- [ ] Pick `feat/x` via **+** again → creates `.worktrees\feat-x` (slug sanitization).

## 3. Open in new tab

- [ ] Right-click the `feature` worktree row → **Open in new tab**. A new repo tab opens
      on the worktree directory; its status/graph work and its HEAD is `feature`.
- [ ] On the row for the CURRENTLY open tab, "Open in new tab" is disabled.

## 4. Lock / unlock

- [ ] Right-click the `feature` row → **Lock**. Badge flips to **locked**;
      `git worktree list --porcelain` shows a `locked` line for that path.
- [ ] **Unlock** → badge clears; the `locked` line is gone from the porcelain output.

## 5. Remove — refusals and success

- [ ] **Remove…** is disabled in the menu for the main row and for the current worktree's row.
- [ ] Lock the `feature` worktree, try Remove (menu disabled — if reached via any path,
      backend refuses with "locked; unlock it first"). Unlock again.
- [ ] Make the worktree dirty: create a file inside `.worktrees\feature`. Remove… → confirm
      → clear error toast "uncommitted changes; commit or stash them first"; the directory
      AND the file are still there. Delete the file afterwards.
- [ ] Remove… on the clean `feature` worktree: a confirm dialog names the **absolute path**
      and warns the directory will be deleted. Click **Cancel** → nothing changes
      (dir still exists, `git worktree list` unchanged).
- [ ] Remove… again and **confirm** → success toast; the directory is really gone from disk
      and `git worktree list` no longer shows it (admin entry pruned too).
- [ ] Repeat remove for `feat-x` to leave the scratch area clean.
