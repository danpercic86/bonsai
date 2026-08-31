# P25 — USER CHECKPOINT checklist (B1 AI review of worktree/branch + B4 stale-branch cleanup)

Run these in the **native app** (`pnpm tauri dev`) against a **scratch repo you create** — NOT a
real repository you care about. **B4 deletes local branches for real**, so use a throwaway repo.
These are exactly the items the AI gate could NOT self-verify: the real `claude` CLI review path
under the consent gate, and destructive branch deletion driven by real refs on disk.

Keep a **second terminal** open in the scratch repo for the `git` verifications below. The AI gate
(orchestrator-side) already confirmed: `cargo test -p bonsai-core` / `-p bonsai` green (including the
new `stale::` unit suite and `tests/stale_cli.rs` end-to-end CLI oracle), `cargo clippy` clean,
`tsc --noEmit` + `pnpm build` clean, and the browser-harness (`VITE_MOCK_IPC=1`) renders both review
actions and the full clean-up dialog / confirm / post-delete shrink from mock data.

---

## 0. Prepare a scratch repo with merged, unmerged, and gone-upstream branches (throwaway)

In a terminal, under `D:\Temp` (never C:). A local **bare** repo stands in for a remote so a real
"gone upstream" is reproducible with no network:

```
cd /d D:\Temp
git init -b main p25-scratch
cd p25-scratch
git config user.name "P25 Tester" && git config user.email "p25@example.com"
git config core.autocrlf false

printf "a\n" > a.txt && git add -A && git commit -m "C0"
printf "b\n" > b.txt && git add -A && git commit -m "C1"
printf "c\n" > c.txt && git add -A && git commit -m "C2"

REM two branches fully merged into main (ancestors of the tip)
git branch merged-a HEAD~2
git branch merged-b HEAD~1

REM an unmerged branch (a unique commit main never sees) — must NEVER be offered/deleted
git checkout -b feature-unmerged HEAD~2
printf "u\n" > u.txt && git add -A && git commit -m "unique work"
git checkout main

REM a real gone-upstream branch: push it, then delete it on the "remote", then prune-less fetch
git init --bare ..\p25-remote.git
git remote add origin ..\p25-remote.git
git checkout -b feature-gone HEAD~2
printf "g\n" > g.txt && git add -A && git commit -m "gone work"
git push -u origin feature-gone
git checkout main
git push origin --delete feature-gone
git fetch origin
git remote prune origin
```

After this: `git branch` shows `main` (current), `merged-a`, `merged-b`, `feature-unmerged`,
`feature-gone`. `git branch -vv` shows `feature-gone` tracking `origin/feature-gone` marked **[gone]**.

Open **`D:\Temp\p25-scratch`** in Bonsai. (Adjust the `printf` calls for your shell — the point is
two merged branches, one unmerged branch, and one branch whose upstream was deleted server-side.)

---

## Part B1 — AI review of the working tree and of a branch

Requires the real **`claude` CLI** installed on PATH, **AI enabled** and **consent granted** in
Settings.

### 1. Review the whole working tree ("✨ Review" on the Changes panel)

1. Make some uncommitted edits so there is a working-tree change set:
   ```
   printf "a\nmore\n" > D:\Temp\p25-scratch\a.txt
   printf "new\n"     > D:\Temp\p25-scratch\newfile.txt
   git add newfile.txt         REM leave a.txt unstaged, newfile.txt staged
   ```
2. In the right-hand **Changes** panel header, click the **✨ Review** button (tooltip
   *"Review all changes with AI"*). It reads *"✨ Reviewing…"* while it runs.
3. Confirm the AI output panel (✨ header) opens with a **sane review of the whole working tree** —
   it should reference the modified `a.txt` and the new `newfile.txt` together (staged + unstaged +
   untracked in one review), not just one of them.
4. **Gate check:** open **Settings**, **disable AI** (or revoke consent). Reopen the Changes panel:
   the **✨ Review** button must be **absent** (the action is AI-eligibility gated) — it must NOT
   call the CLI. Re-enable AI + consent afterward.

### 2. Review a branch ("Review branch…" on a branch's context menu)

5. In the left **Sidebar**, right-click a **local branch** (e.g. `feature-unmerged`) to open its
   context menu. Click **Review branch…**.
6. Confirm the AI output panel opens titled **"Review branch feature-unmerged"** and returns a review
   of that branch's diff **vs its auto-resolved base** (the branch's upstream if set, else
   `origin/HEAD` / `main` / `master`). The prose should reflect the branch's unique change
   (`u.txt`).
7. Try **Review branch…** on `feature-gone` (no live upstream): it should still resolve a base
   (falls through to `main`) and produce a review, OR degrade gracefully with a clear message if no
   base can be determined — it must never hang or crash.
8. **Gate check:** with **AI disabled/consent revoked**, the **Review branch…** item must be
   **absent** from the context menu (local, AI-eligible branches only).

### 3. Large-diff truncation note (informational)

9. The backend caps the review payload at **256 KiB** (`MAX_REVIEW_PAYLOAD_BYTES`): a very large
   working tree or branch diff is truncated on a character boundary with a visible
   *"… (payload truncated at 256 KiB for review) …"* marker appended before the model sees it. If
   you review a repo with a huge diff, confirm the review still returns (truncated) rather than
   erroring. Not required for sign-off — just be aware the cap exists.

---

## Part B4 — Clean up stale branches (DESTRUCTIVE — scratch repo only)

No AI required. **This deletes local branches for real.** Use only the throwaway `p25-scratch`.

### 4. "Clean up branches…" lists the right branches with correct chips

10. In the **Sidebar**, in the **Branches** section header, click the **Clean up branches…** action
    (button `aria-label`/tooltip *"Clean up branches…"*). The **Clean up branches** dialog opens.
11. Confirm the subhead reads *"Branches merged into `main` or with a gone upstream."* (the base is
    the resolved base — here `main`).
12. Confirm the listed rows are exactly: **`merged-a`**, **`merged-b`** (chip **merged**, neutral/
    green), and **`feature-gone`** (chip **gone upstream**, amber). Cross-check in the second
    terminal:
    ```
    git branch --merged main      REM → main, merged-a, merged-b (dev/current excluded)
    git branch -vv                REM → feature-gone shows [gone]
    ```
13. Confirm **`main`** (the base / current branch) and **`feature-unmerged`** (unmerged, no gone
    upstream) are **NOT listed** — they must never be offered.
14. Confirm the default selection: **merged rows are pre-checked**; the **gone-upstream row is
    unchecked** (it shows a *"gone upstream — force delete (unchecked by default)"* hint). Try the
    **Select all / Select none** toggle.

### 5. Cancel deletes nothing

15. With some rows checked, click **Delete selected (N)** → a **confirm dialog** opens listing the
    **exact branch names** to be deleted. Click **Cancel** (or Close the whole dialog).
16. In the second terminal, confirm **nothing was deleted**:
    ```
    git branch      REM → all of main, merged-a, merged-b, feature-unmerged, feature-gone still present
    ```

### 6. Confirm actually deletes exactly the checked branches

17. Reopen **Clean up branches…**. Check **`merged-a`**, **`merged-b`**, and also opt-in
    **`feature-gone`** (force). Click **Delete selected (3)** → the confirm dialog must list exactly
    those three names. Confirm.
18. A summary toast reports the outcome (e.g. *Deleted 3 branch(es)*). Verify on disk that exactly
    those three local branches are gone and the others survive:
    ```
    git branch
    REM expect only:  * main   feature-unmerged
    ```
19. Reopen **Clean up branches…**: the list has **shrunk** — `merged-a` / `merged-b` /
    `feature-gone` are no longer offered (an empty list shows *"No stale branches — nothing to
    clean up."* if nothing remains).

### 7. The current and base branches are never deletable (server-side safety)

20. The dialog never offers `main` (base) or the checked-out branch. Even if a race ever surfaced
    one, the backend independently refuses: the current branch → `skippedCurrent`, the base →
    `skippedBase`, an unmerged/non-stale name → `skippedNotStale` — none are deleted. (This
    defense-in-depth is covered by `tests/stale_cli.rs`; you only need to confirm the UI never lets
    you delete `main` or the current branch.) To spot-check, checkout a merged branch as current and
    reopen the dialog:
    ```
    git checkout merged-b        REM (only if it still exists; else create + checkout a merged one)
    ```
    Confirm the now-current branch is absent from the clean-up list.

---

**Sign-off:** every numbered item behaves as described. In particular — B1: the **✨ Review**
(working tree) and **Review branch…** (context menu) actions return sane reviews via the real
`claude` CLI and are **hard-gated** on AI enabled + consent (absent when disabled), with the 256 KiB
truncation cap understood (steps 1–9). B4: the clean-up dialog lists exactly the merged + gone
branches with correct chips against the real base, never the current or base branch (steps 10–14);
**Cancel deletes nothing** (steps 15–16) while **Confirm deletes exactly the checked set** — verified
with `git branch` (steps 17–19); and the current/base branches are never deletable (step 20). Report
any deviation to the orchestrator with the failing step number and the exact `git` / file output.
