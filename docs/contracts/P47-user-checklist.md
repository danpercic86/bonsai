# P47 — Native QA checklist (USER CHECKPOINT)

Run `pnpm tauri dev` against a real repo. These items require the native Tauri window / human
perception and cannot be self-verified by the orchestrator. Use a SCRATCH repo for anything
destructive — never a repo you care about. Tick each box.

## Setup
- [ ] Open a real repo with several branches and at least one tag, plus a **dirty working tree**
      (edit/stage a couple of tracked files so autostash has something to stash).

## A. Menu consolidation
- [ ] Right-click a **non-current branch pill** in the graph → the menu shows **Create branch
      here, Create tag here, Compare with HEAD, Cherry-pick onto current…, Revert commit**
      (in addition to Checkout / Copy / Merge / Rebase / Delete). No duplicate "Create branch
      here" or "Compare with HEAD".
- [ ] Right-click the **current HEAD branch pill** → no cherry-pick/revert onto itself (menu is
      the reduced/empty set as before).
- [ ] Right-click a **tag pill in the graph** → the same commit-action group appears.
- [ ] Right-click a **tag row in the sidebar** → only Delete / Copy / Push tag (commit actions
      are intentionally NOT offered here — expected).
- [ ] Right-click a **commit row** → still shows the commit actions plus Interactive rebase from
      here and the bisect items (unchanged).

## B1/B2. Autostash + editable message (clean pick)
- [ ] With a dirty tree, Cherry-pick a non-conflicting commit from a branch pill.
- [ ] The **message dialog** opens, prefilled with the source commit's full message; edit it.
- [ ] Confirm → a new commit lands with YOUR edited message; the picked commit's **author** is
      preserved; **your dirty changes are restored** (worktree looks as before the pick).
- [ ] A success toast mentions the stash was restored.

## B1. Autostash on conflict (retained stash)
- [ ] With a dirty tree, cherry-pick a commit that WILL conflict.
- [ ] The op pauses; the OpBanner shows and the **conflicted files are listed**.
- [ ] The **"Continue" button is DISABLED** until you resolve all conflicts (this is the
      B3 fix — previously it was wrongly enabled).
- [ ] Resolve the conflicts → Continue → the pick commits with the edited message (if you had
      edited it before the conflict).
- [ ] Your original dirty changes are recoverable: `git stash list` shows the
      `bonsai: autostash before cherry-pick` entry; `git stash pop` restores them.
- [ ] Alternatively press **Abort** → HEAD resets cleanly; the autostash entry is still present
      in `git stash list` (not lost).

## Revert parity
- [ ] With a dirty tree, Revert a commit from a pill → revert commits with the standard
      `Revert "…"` message (NO editable dialog — expected), stash restored on the clean path.
- [ ] A conflicting revert pauses with the same Continue-disabled-until-resolved behavior.

## Regression / feel
- [ ] A cherry-pick on a CLEAN tree still behaves as before (no spurious stash).
- [ ] `git stash list` is empty after a fully clean pick+restore (no leaked autostash).
- [ ] Menu ordering and placement feel natural on branch / tag / commit right-clicks.
- [ ] Detached HEAD: cherry-pick / revert are correctly ABSENT from the menus.
