# P20 — USER CHECKPOINT checklist (Daily essentials: amend / reset / discard / cherry-pick / revert / abort)

Run these in the **native app** (`pnpm tauri dev`) against a **throwaway scratch repo** — NEVER a
real repository you care about (every step here rewrites history and/or discards working-tree
changes). These items are exactly what the AI gate could NOT self-verify: the native commit-box
Amend affordance + push-guard note, the commit-row Reset menu + destructive ConfirmDialog gating,
the Discard `↺` control + ConfirmDialog restoring a real worktree, and the cherry-pick/revert
pause → actionable `OpBanner` → resolve/Continue and Abort flows against a real git2 index.

Keep a **second terminal** open in the scratch repo for the `git` verifications below. All commands
assume the repo is at `D:\Temp\p20-scratch\work` — adjust if you chose another path.

---

## 0. Prepare a scratch repo (throwaway — safe to delete)

In a terminal, in a throwaway folder:

```
cd /d D:\Temp\p20-scratch

REM --- a local bare "remote" so we can test the amend push-guard (step 1.4) ---
git init --bare -b main origin.git

git init -b main work
cd work
git config user.name "P20 Tester" && git config user.email "p20@example.com"
git config core.autocrlf false
git remote add origin ..\origin.git

REM --- linear history c1..c4 on main ---
echo one>  a.txt && git add -A && git commit -m "c1 base"
echo two>  a.txt && git add -A && git commit -m "c2 edit a"
echo b>    b.txt && git add -A && git commit -m "c3 add b"
echo three>a.txt && git add -A && git commit -m "c4 edit a again"

REM --- a side branch whose commits we will cherry-pick / revert ---
git checkout -b feature c1
echo feat> feature.txt && git add -A && git commit -m "feat: add feature.txt"
echo conflictA> a.txt  && git add -A && git commit -m "feat: rewrite a.txt (will conflict)"
git checkout main
```

Open **`D:\Temp\p20-scratch\work`** in Bonsai. The graph shows `main` (c1..c4) and a `feature`
branch with two commits. Nothing is pushed yet.

> Tip: after every Bonsai mutation below, cross-check with the paired `git` command in the second
> terminal. If a Bonsai result and the `git` output ever disagree, STOP and report the step number
> plus both outputs to the orchestrator.

---

## 1. Amend — rewrite the tip (same parents, new tree) + push-guard note

1. Select **no commit** (so the right panel shows working-dir status). In the Changes area, modify
   a tracked file: in the terminal run `echo amended> a.txt` inside `work`, then click Bonsai's
   **refresh** so `a.txt` appears under Changes. **Stage** it (the `+` control).
2. Above the commit box, tick **"Amend last commit"**. Confirm:
   - The message box **prefills** with the current tip's message (`c4 edit a again`).
   - The commit button label changes to **"Amend"**.
   - (Message-only sanity) Even with nothing staged the Amend button is enabled — unlike a normal
     commit which requires a staged file.
3. Edit the message to `c4 amended` and click **Amend**. A success toast **"Amended last commit"**
   appears. Cross-check the tip was **rewritten in place** (new oid, new tree, SAME parent):
   ```
   git -C D:\Temp\p20-scratch\work log -2 --format="%h %p %s"
   git -C D:\Temp\p20-scratch\work show --stat HEAD
   ```
   Confirm: HEAD's subject is now `c4 amended`, HEAD's parent (`%p`) is unchanged (still c3), the
   tree contains the amended `a.txt` (`show` reflects `amended`), and the old c4 oid is gone from
   the branch.
4. **Push-guard note.** First push main so the tip is "already pushed":
   ```
   git -C D:\Temp\p20-scratch\work push -u origin main
   ```
   Refocus Bonsai (click the window) so branch ahead/behind refreshes. Stage another change
   (`echo again> b.txt`, refresh, stage) and tick **Amend** again. Confirm a **warning line**
   appears next to the checkbox: *"This commit is already pushed — amending rewrites published
   history."* It is informational and does **not** block the Amend button.
5. Untick Amend without amending (leave the pushed tip intact) to avoid diverging from `origin` for
   the remaining steps, OR just note the warning behaved and continue — the later steps do not
   depend on `origin`.

## 2. Reset — soft / mixed / hard from a commit-row menu

Reset semantics reminder: **soft** moves HEAD only; **mixed** moves HEAD + resets the index;
**hard** moves HEAD + index + **discards the worktree**.

6. Right-click an **older commit row** (e.g. `c2 edit a`). The context menu shows three items:
   **"Reset main to here (soft)"**, **"(mixed)"**, and **"(hard)…"** (the hard item is suffixed
   with `…` to signal a destructive dialog). Confirm all three are **disabled** when you right-click
   the current tip (can't reset to where you already are).
7. **Soft.** Click **Reset main to here (soft)**. A ConfirmDialog appears (moving the ref orphans
   commits). Confirm. Success toast `Reset main to <short-oid> (soft)`. Cross-check:
   ```
   git -C D:\Temp\p20-scratch\work log --oneline -1
   git -C D:\Temp\p20-scratch\work status --short
   ```
   HEAD is now at the c2 commit, but the changes from c3/c4 are **staged** (index still has b.txt +
   the a.txt edits) and the worktree is unchanged. (Then in the terminal move back:
   `git -C ...\work reset --hard <the c4 oid>` — grab it from `git reflog` — to restore for the next
   sub-step. Or just re-open the repo; either way return HEAD to the branch tip before step 8.)
8. **Mixed.** Right-click the same older commit → **(mixed)** → confirm. Toast shows `(mixed)`.
   Cross-check:
   ```
   git -C D:\Temp\p20-scratch\work status --short
   ```
   The later changes are now **unstaged** (index reset to the target; worktree still has the files).
   No file content was lost. Restore HEAD to the tip again (as in step 7) before step 9.
9. **Hard (destructive — must gate).** First make a dirty edit so you can prove it gets discarded:
   `echo DIRTY> a.txt` in the terminal (do NOT stage). Right-click the older commit →
   **Reset main to here (hard)…**. Confirm the ConfirmDialog:
   - Title/label reads **hard** and the body includes the extra warning *"Uncommitted changes in
     your working tree will be permanently discarded."*
   - This dialog is a **hard requirement** — there is no way to hard-reset without it.
   Confirm. Toast `(hard)`. Cross-check the worktree and index were both reset:
   ```
   git -C D:\Temp\p20-scratch\work status
   git -C D:\Temp\p20-scratch\work log --oneline -1
   type D:\Temp\p20-scratch\work\a.txt
   ```
   `status` is **clean**, HEAD is at the target commit, and `a.txt` holds the **target's** content
   (the `DIRTY` edit AND the c3/c4 files are gone). Confirm `b.txt` no longer exists if the target
   predates c3.

> After step 9 the branch is behind its old tip; that's fine for the remaining steps. If you want
> the feature commits reachable again for steps 4/5, `git -C ...\work reset --hard <c4 oid>` from the
> reflog first.

## 3. Discard — restore a tracked file to the index, preserving an unrelated staged change

10. Set up two independent changes in the terminal, then refresh Bonsai:
    ```
    cd /d D:\Temp\p20-scratch\work
    echo staged-a>  a.txt && git add a.txt      REM a.txt has a STAGED change
    echo worktree-a>a.txt                        REM ...and a further UNSTAGED edit on top
    echo worktree-b>b.txt                         REM b.txt has an UNSTAGED change (untouched control)
    ```
    (If `b.txt` was removed by step 9's hard reset, first `git checkout <a commit that has b.txt> -- b.txt`
    or pick any second tracked file.) Refresh Bonsai.
11. In the right panel's **Changes** (unstaged) section, the `a.txt` row shows a secondary **`↺`
    discard** control beside the `+` stage button. Confirm **untracked** rows show **no** `↺`
    control (add `echo x> untracked.txt`, refresh — its row has only the stage/ignore affordances,
    no discard).
12. Click **`↺`** on `a.txt`. A ConfirmDialog appears: *"Discard changes to 1 file(s)? This
    permanently reverts them to the last staged/committed version and cannot be undone."* Confirm.
    Toast `Discarded changes to 1 file(s)`.
13. Cross-check: `a.txt`'s **worktree** is restored to the **index (staged)** version, while the
    staged content itself and the unrelated `b.txt` edit are **preserved**:
    ```
    type D:\Temp\p20-scratch\work\a.txt
    git -C D:\Temp\p20-scratch\work diff -- a.txt
    git -C D:\Temp\p20-scratch\work diff --cached -- a.txt
    type D:\Temp\p20-scratch\work\b.txt
    ```
    Confirm: `a.txt` now reads **`staged-a`** (the index version, NOT `worktree-a` and NOT HEAD);
    `git diff -- a.txt` is **empty** (worktree == index); `git diff --cached -- a.txt` still shows
    the staged `staged-a` change (staging preserved); `b.txt` still reads **`worktree-b`**
    (untouched).

## 4. Cherry-pick — clean commit, and a conflicting pause → resolve → Continue

14. **Clean pick.** Ensure you are on `main` with a clean status. Right-click the feature commit
    **"feat: add feature.txt"** (the non-conflicting one) → **"Cherry-pick onto current"**. Toast
    `Cherry-picked <short-oid>`. Cross-check the pick landed on top of main with the original
    message + author preserved:
    ```
    git -C D:\Temp\p20-scratch\work log -1 --format="%h %an %s"
    git -C D:\Temp\p20-scratch\work show --stat HEAD
    ```
    HEAD's subject is `feat: add feature.txt`, author is the feature commit's author, and
    `feature.txt` is present in the tree. `git status` is clean.
15. **Conflicting pick.** Right-click the feature commit **"feat: rewrite a.txt (will conflict)"** →
    **"Cherry-pick onto current"**. Because both main and feature rewrote `a.txt`, this **pauses**:
    - An **info toast** `Cherry-pick paused: 1 conflict(s) to resolve` appears.
    - An actionable **OpBanner** appears titled **"Cherry-picking"**, sub-line `1 conflict(s)
      remaining`, with **Continue** (disabled) and **Abort** buttons. The commit box stays blocked.
    Cross-check the repo is paused mid cherry-pick:
    ```
    git -C D:\Temp\p20-scratch\work status
    dir D:\Temp\p20-scratch\work\.git\CHERRY_PICK_HEAD
    ```
    `status` reports "You are currently cherry-picking" with `a.txt` unmerged; `CHERRY_PICK_HEAD`
    exists.
16. Open `a.txt` in Bonsai's **conflict editor**, resolve it to a chosen final content, and mark it
    resolved. As conflicts drop to 0 the banner sub-line flips to **"All conflicts resolved"** and
    **Continue becomes enabled**. Click **Continue**. Toast `Cherry-picked <short-oid>`. Cross-check:
    ```
    git -C D:\Temp\p20-scratch\work status
    git -C D:\Temp\p20-scratch\work log -1 --format="%s"
    ```
    `status` is **clean** (no cherry-pick in progress, no `CHERRY_PICK_HEAD`); the new commit reuses
    the picked message; `a.txt` holds your resolved content.

## 5. Revert — clean commit, and a conflicting pause → resolve → Continue

17. **Clean revert.** Right-click a commit whose change is cleanly reversible on the current tip
    (e.g. the just-cherry-picked `feat: add feature.txt`, which only *added* `feature.txt`) →
    **"Revert commit"**. Toast `Reverted <short-oid>`. Cross-check the revert commit + byte-exact
    message:
    ```
    git -C D:\Temp\p20-scratch\work log -1 --format="%B"
    git -C D:\Temp\p20-scratch\work show --stat HEAD
    ```
    The message is exactly `Revert "feat: add feature.txt"` then a blank line then
    `This reverts commit <full-40-hex-oid>.`; the tree no longer contains `feature.txt`. `status`
    is clean.
18. **Conflicting revert.** Create a target that will conflict on revert: commit an edit, then edit
    the same line again, then revert the first edit.
    ```
    cd /d D:\Temp\p20-scratch\work
    echo r1> r.txt && git add r.txt && git commit -m "r: add r.txt"
    echo r2> r.txt && git add r.txt && git commit -m "r: edit r.txt (target)"
    REM grab the target oid:
    git log --oneline -1
    echo r3> r.txt && git add r.txt && git commit -m "r: edit r.txt again"
    ```
    Refresh Bonsai. Right-click the **"r: edit r.txt (target)"** commit → **"Revert commit"**. It
    **pauses**: info toast `Revert paused: 1 conflict(s) to resolve`, OpBanner titled **"Reverting"**
    with a disabled **Continue** and an **Abort**. Cross-check:
    ```
    git -C D:\Temp\p20-scratch\work status
    dir D:\Temp\p20-scratch\work\.git\REVERT_HEAD
    ```
    `status` says "You are currently reverting"; `REVERT_HEAD` exists.
19. Resolve `r.txt` in the conflict editor → Continue becomes enabled → click **Continue**. Toast
    `Reverted <short-oid>`. Cross-check `git status` is clean and no `REVERT_HEAD` remains.

## 6. Abort — start a conflicting pick/revert, then Abort restores HEAD

20. Trigger a conflict again without resolving it. Easiest: right-click a commit that will conflict
    (e.g. re-create the revert conflict from step 18, or cherry-pick a conflicting feature commit
    onto a divergent tip) so Bonsai shows the actionable OpBanner. Record HEAD **before** aborting:
    ```
    git -C D:\Temp\p20-scratch\work rev-parse HEAD
    ```
21. Click **Abort** in the OpBanner. Confirm a ConfirmDialog appears titled **"Abort cherry-pick"**
    / **"Abort revert"** with body: *"This resets your branch and working tree to HEAD. The
    in-progress cherry-pick/revert and any conflict resolutions will be lost."* Confirm.
22. Cross-check the repo is back exactly at HEAD, fully clean:
    ```
    git -C D:\Temp\p20-scratch\work status
    git -C D:\Temp\p20-scratch\work rev-parse HEAD
    dir D:\Temp\p20-scratch\work\.git\CHERRY_PICK_HEAD 2>NUL
    dir D:\Temp\p20-scratch\work\.git\REVERT_HEAD 2>NUL
    ```
    Confirm: `status` is **clean** (no operation in progress, no unmerged paths), HEAD equals the
    oid recorded in step 20 (unchanged), and **neither** `CHERRY_PICK_HEAD` **nor** `REVERT_HEAD`
    exists (both `dir` commands report "File Not Found"). The OpBanner is gone and the commit box is
    usable again.

---

**Sign-off:** every numbered item behaves as described, and every paired `git` verification matches
the toast/UI Bonsai showed. In particular: Amend rewrites the tip in place preserving its parent(s)
and shows the push-guard warning only when the tip is pushed (step 1); Reset soft/mixed/hard match
`git reset` semantics and Hard is impossible without its destructive dialog (steps 6–9); Discard
restores a file to the **index** version while a staged change and an unrelated file are preserved
(steps 10–13); clean cherry-pick/revert commit with the correct message/author and conflicting ones
pause into an actionable OpBanner whose **Continue stays disabled until conflicts resolve**
(steps 14–19); and Abort — behind its ConfirmDialog — returns the repo to a clean HEAD with no
`*_HEAD` sequencer file left behind (steps 20–22). Report any deviation to the orchestrator with the
failing step number and the exact `git` output.
