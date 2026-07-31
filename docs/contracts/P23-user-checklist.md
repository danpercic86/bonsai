# P23 — USER CHECKPOINT checklist (Interactive rebase + Blame / File-history)

Run these in the **native app** (`pnpm tauri dev`) on Windows. These are exactly the items the AI
gate could NOT self-verify. The autonomous oracles — `rebase_interactive_cli.rs` (20 tests) and
`blame_cli.rs` (6 tests) — exercise the core git2 engine directly (tree oids, author identity,
messages, topology, opstate probe, abort restore) and the mock browser harness fakes the whole UI.
What is verified HERE is the **live native wiring against real git2**: the commit/branch context
actions, the `RebasePlanEditor`, the reused `OpBanner` + Continue/Skip/Abort during an interactive
pause, the `ConfirmDialog` abort gate, the `BlameView` / `FileHistoryView` overlays, and
`revealCommitByOid` scrolling the real graph — plus the **feel** of Up/Down reordering.

Prerequisites:
- Start the app once: from the repo root run **`pnpm tauri dev`** and wait for the Bonsai window.
- Keep a **second terminal** open for the `git` cross-checks below.
- Prepare a throwaway scratch repo with a small topic branch. Author dates are fixed so the base
  history is stable; committer dates will be "now" (interactive rebase always rewrites — this is
  expected, cross-check **trees + messages + author identity**, never committer time):
  ```
  mkdir D:\Temp\p23-scratch
  cd D:\Temp\p23-scratch
  git init -b main
  git config user.name "Test User"
  git config user.email "test@example.com"
  echo base> base.txt && git add -A && git commit -m "base"
  git checkout -b topic
  echo a> a.txt && git add -A && git commit -m "c1 add a"
  echo b> b.txt && git add -A && git commit -m "c2 add b"
  echo c> c.txt && git add -A && git commit -m "c3 add c"
  ```
  This leaves `topic` = base → c1 → c2 → c3 (three disjoint-file commits above `base`).
- Open **`D:\Temp\p23-scratch`** in Bonsai (tab `+` → Browse…). Confirm the commit graph shows
  `base`, `c1`, `c2`, `c3` on `topic`, with `topic` and `main` ref pills.

> Tip: after each operation, cross-check with the paired `git` command in the second terminal. If a
> Bonsai result and the `git` output ever disagree, STOP and report the step number plus both
> outputs to the orchestrator.
>
> **v1 limits to keep in mind** (do not report these as bugs):
> - Reorder is via **Up/Down buttons**, not drag (drag is a Polish item).
> - Blame is against the **committed** version (HEAD, or the selected commit) — uncommitted worktree
>   edits are not attributed.
> - File history follows a **single** rename best-effort.
> - `.git/bonsai-rebase/` is Bonsai's OWN sequencer. Do NOT run `git rebase` / `git cherry-pick` in a
>   terminal while a Bonsai interactive rebase is paused (it would create a colliding git-native
>   sequencer). Drive Continue/Skip/Abort from Bonsai only.

---

## PART A — INTERACTIVE REBASE

For every case below, record the `topic` tip and trees BEFORE, run the rebase in Bonsai, then
cross-check AFTER. Baseline snapshot to take once now:
```
git -C D:\Temp\p23-scratch rev-parse topic
git -C D:\Temp\p23-scratch log --format="%H %s" topic
git -C D:\Temp\p23-scratch rev-parse "topic^{tree}"
```

### 1. Open the plan editor from a commit row

1. In the center commit graph, **right-click the `base` commit row**. Confirm the context menu
   contains **"Interactive rebase from here…"** (next to the existing cherry-pick / create-branch
   items). It is enabled only on an ancestor of HEAD while no other op is active.
2. Click it. The **`RebasePlanEditor`** opens listing the commits `base..HEAD` **oldest-first**
   (execution order): row 1 = `c1 add a`, row 2 = `c2 add b`, row 3 = `c3 add c`. Each row shows a
   short-oid + summary, an **action `<select>`** (pick / reword / squash / fixup / drop), and **Up /
   Down** buttons (disabled at the ends). Confirm **Cancel** closes it with no change
   (`git rev-parse topic` unchanged).

### 2. Reorder two commits (Up/Down)

3. Reopen the editor from `base`. On row **`c3 add c`** click **Up** so the order becomes
   `c1`, `c3`, `c2`. Click **Start rebase**. Confirm a success toast **"Rebased onto base
   (3 commit(s))"** and the graph redraws.
4. Cross-check:
   ```
   git -C D:\Temp\p23-scratch log --format="%H %s" topic
   git -C D:\Temp\p23-scratch rev-parse "topic^{tree}"
   ```
   The subjects, newest-first, read **`c2 add b`**, **`c3 add c`**, **`c1 add a`**, **`base`** (c3
   now precedes c2). The **tree oid is UNCHANGED** from the baseline (files a/b/c are disjoint, so
   reordering leaves the final tree identical). `git status` is clean.
5. Reset for the next case:
   ```
   git -C D:\Temp\p23-scratch reset --hard <baseline topic tip from the snapshot>
   ```
   Refresh Bonsai (refresh button) so the graph shows c1/c2/c3 again.

### 3. Squash two into one (combined message)

6. Open the editor from `base`. On row **`c2 add b`** set the action to **squash** — a **message
   textarea** appears, pre-filled with the concatenation of c1's + c2's messages. Replace it with
   **`c1+c2 combined`**. Leave c1 as pick, c3 as pick. **Start rebase**.
7. Cross-check:
   ```
   git -C D:\Temp\p23-scratch log --format="%H %s" topic
   git -C D:\Temp\p23-scratch rev-parse "topic^{tree}"
   ```
   The log shows **three** commits above base — `c3 add c`, **`c1+c2 combined`**, `base` — i.e. the
   commit **count dropped by one** (c1 and c2 fused). The combined commit's message is
   `c1+c2 combined`. The **final tree is unchanged** vs baseline (a.txt + b.txt + c.txt all present).
   `git status` clean. Then `reset --hard` back to baseline + refresh (as step 5).

### 4. Fixup (message discarded)

8. Open the editor from `base`. On row **`c2 add b`** set the action to **fixup**. Confirm **no
   message textarea** appears for a fixup (fixup discards its own message). **Start rebase**.
9. Cross-check `git log --format="%H %s" topic`: again **one fewer commit**; the fused commit keeps
   the **predecessor's** message **`c1 add a`** (c2's message is gone). `git rev-parse "topic^{tree}"`
   is unchanged vs baseline. `reset --hard` back + refresh.

### 5. Reword (new message, same tree)

10. Open the editor from `base`. On row **`c2 add b`** set the action to **reword** — a textarea
    appears pre-filled with `c2 add b`. Change it to **`c2 reworded`**. **Start rebase**.
11. Cross-check:
    ```
    git -C D:\Temp\p23-scratch log --format="%H %s" topic
    git -C D:\Temp\p23-scratch rev-parse "topic^{tree}"
    ```
    Still **three** commits above base; the middle now reads **`c2 reworded`**; the **tree oid is
    unchanged** vs baseline (reword touches only the message). `reset --hard` back + refresh.

### 6. Drop (commit gone)

12. Open the editor from `base`. On row **`c2 add b`** set the action to **drop** (the row's label
    shows struck-through). **Start rebase**.
13. Cross-check:
    ```
    git -C D:\Temp\p23-scratch log --format="%H %s" topic
    git -C D:\Temp\p23-scratch ls-tree -r --name-only topic
    ```
    Only **`c1 add a`** and **`c3 add c`** remain above base (c2 gone); `ls-tree` lists `a.txt`,
    `base.txt`, `c.txt` but **NOT `b.txt`**. `git status` clean. `reset --hard` back + refresh.

### 7. Conflicting interactive rebase → resolve → Continue, and Abort restores the original tip

Build a small conflict fixture in the second terminal:
```
cd D:\Temp\p23-scratch
git checkout main
echo main-line> shared.txt && git add -A && git commit -m "main edits shared"
git checkout -b feature
echo feature-line> shared.txt && git add -A && git commit -m "feature edits shared"
git rev-parse feature      # <-- record as FEATURE_TIP (the pre-rebase tip; used for the abort check)
```
Refresh Bonsai; check out / select `feature`. `feature` and `main` both changed `shared.txt`, so
replaying `feature` onto `main` conflicts.

14. Right-click the **`main` commit** row (or the `main` branch row) → **"Interactive rebase from
    here…"** with `main` as the onto base. In the editor keep the single `feature edits shared` row
    as **pick** → **Start rebase**.
15. Confirm the rebase **pauses**: an info toast **"Rebase paused at step 1/1: 1 conflict(s) to
    resolve"** and the **`OpBanner`** appears reading **`Rebasing feature`** with **`step 1/1`**.
    Confirm the banner's **Continue** action is **disabled** while the conflict is unresolved.
16. Cross-check the paused state in the terminal:
    ```
    git -C D:\Temp\p23-scratch status
    git -C D:\Temp\p23-scratch rev-parse feature
    dir D:\Temp\p23-scratch\.git\bonsai-rebase
    ```
    `status` shows an unmerged `shared.txt`; `rev-parse feature` **still equals FEATURE_TIP** (the
    branch ref is NOT moved until finish); `.git\bonsai-rebase\state.json` **exists**. (Note the
    Bonsai sequencer, NOT `.git\rebase-merge`.)
17. Open the conflict editor on `shared.txt` (the conflict entry in the right panel). Resolve it
    (choose ours/theirs or hand-edit to a single clean version) and mark resolved. Confirm the
    OpBanner **Continue** becomes **enabled**.
18. Click **Continue**. Confirm a success toast **"Rebased onto main (1 commit(s))"**, the OpBanner
    clears, and the graph redraws with `feature` on top of `main`. Cross-check:
    ```
    git -C D:\Temp\p23-scratch log --format="%H %s" feature
    git -C D:\Temp\p23-scratch rev-parse "feature~1"     # == main tip
    git -C D:\Temp\p23-scratch status
    dir D:\Temp\p23-scratch\.git\bonsai-rebase
    ```
    `feature~1` equals the `main` tip (replayed on top); the resolved `shared.txt` content is what
    you chose; `status` is **clean**; `.git\bonsai-rebase` is **gone**.
19. **Abort safety.** Recreate the pause: `reset --hard FEATURE_TIP` on `feature` + refresh, then
    repeat steps 14–15 to pause again on the conflict. Record `git rev-parse feature` (== FEATURE_TIP).
    Now click the OpBanner **Abort**. Confirm an **`Abort rebase?` `ConfirmDialog`** appears (copy:
    restores your branch and working tree to their pre-rebase state) — click confirm. Then
    cross-check:
    ```
    git -C D:\Temp\p23-scratch rev-parse feature       # MUST equal FEATURE_TIP exactly
    git -C D:\Temp\p23-scratch symbolic-ref HEAD        # refs/heads/feature (re-attached)
    git -C D:\Temp\p23-scratch status                   # clean, no unmerged paths
    git -C D:\Temp\p23-scratch cat-file -p feature:shared.txt   # original feature content
    dir D:\Temp\p23-scratch\.git\bonsai-rebase          # gone
    ```
    **The branch tip after abort MUST be byte-identical to the pre-rebase tip (FEATURE_TIP)**, HEAD
    re-attached to `feature`, worktree clean with the original `shared.txt`, and no Bonsai sequencer
    left. A git-native `git rebase --continue`/`--abort` must NOT be needed — Bonsai owns the
    sequencer.
20. (Optional) **Skip.** Recreate the pause once more (step 19 setup). This time click the OpBanner
    **Skip**. Confirm the offending op is dropped and the rebase completes (here the single op is
    skipped, so `feature` ends at `main` with no new commit); `git status` clean;
    `.git\bonsai-rebase` gone.

---

## PART B — BLAME + FILE HISTORY

Build a small multi-author, renamed file in the second terminal (distinct authors so blame is
visibly multi-colored):
```
cd D:\Temp\p23-scratch
git checkout topic
git -c user.name=Alice -c user.email=alice@x.com commit --allow-empty -m "noop"   # (optional marker)
printf "one\ntwo\nthree\n" > story.txt
git add -A
git -c user.name=Alice -c user.email=alice@x.com commit -m "story: create"
printf "one\nTWO-edited\nthree\n" > story.txt
git add -A
git -c user.name=Bob -c user.email=bob@x.com commit -m "story: edit line 2"
git mv story.txt tale.txt
git -c user.name=Carol -c user.email=carol@x.com commit -m "tale: rename story->tale"
printf "one\nTWO-edited\nthree\nfour\n" > tale.txt
git add -A
git -c user.name=Dave -c user.email=dave@x.com commit -m "tale: add line 4"
```
Refresh Bonsai so the new commits show on `topic`.

### 8. Blame

21. Select a commit in the graph so a diff / file list is visible (or use the working-dir status
    file list). On a **`tale.txt` file row**, open the context action **"Blame"** (right-click menu
    item or hover button). The **`BlameView`** overlay opens with, per line, a left **gutter**
    (short-oid pill + author + relative date) beside the monospace line text with `finalLineNo`.
    Consecutive same-commit lines collapse into one gutter block.
22. Cross-check the gutter against `git blame` in the terminal:
    ```
    git -C D:\Temp\p23-scratch blame tale.txt
    ```
    Confirm, per line, the author/short-oid in the Bonsai gutter matches the `git blame` output:
    line 1 → **Alice** (`story: create`), line 2 → **Bob** (`story: edit line 2`), line 3 → **Alice**,
    line 4 → **Dave** (`tale: add line 4`). (Blame is as of HEAD, or as of the selected commit if one
    is selected — it follows the committed version, not any unsaved worktree edit.)
23. Click a **gutter block** (e.g. Bob's line 2). Confirm the corresponding commit is **selected and
    revealed** (scrolled into view) in the center graph. If the commit is not part of the current
    walk, confirm an info toast **"Commit not in the current view"** instead of a crash.

### 9. File history (with a rename)

24. On the **`tale.txt`** file row open **"File history"**. The **`FileHistoryView`** overlay lists
    the commits that touched the file, newest-first, **following the rename** back through `story.txt`.
25. Cross-check:
    ```
    git -C D:\Temp\p23-scratch log --follow --oneline -- tale.txt
    ```
    The Bonsai list order and oids match: **`tale: add line 4`** (Dave) → **`tale: rename
    story->tale`** (Carol) → **`story: edit line 2`** (Bob) → **`story: create`** (Alice).
26. Click a history **row** → confirm that commit is **selected + revealed** in the graph. Confirm a
    secondary action opens that commit's **diff** (reusing the commit-diff overlay).
27. **Empty history.** Open File history on a path that never existed (if a UI affordance allows;
    otherwise skip) → confirm it shows **"No history for this file"** rather than an error.

---

**Sign-off:** every numbered item behaves as described and every paired `git` verification matches
what Bonsai showed. In particular: **"Interactive rebase from here…"** opens the plan editor
oldest-first; **reorder** (Up/Down) changes `git log` order with the final tree unchanged (step 2);
**squash** fuses two commits into one with the combined message, tree unchanged (step 3); **fixup**
fuses and discards the message (step 4); **reword** changes only the message (step 5); **drop**
removes the commit and its file (step 6); a **conflicting** rebase pauses in the reused `OpBanner`
with Continue disabled until resolved, **Continue** completes onto the new base, and — critically —
**Abort restores the branch tip byte-identically to the pre-rebase tip** with HEAD re-attached and
no `.git/bonsai-rebase/` left (step 7); **Blame** gutter authors/oids match `git blame` and a click
reveals the commit (step 8); **File history** matches `git log --follow --oneline` across the rename
and a row reveals the commit (step 9). Report any deviation to the orchestrator with the failing step
number and the exact `git` output.
