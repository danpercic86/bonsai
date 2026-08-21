# P38 — Reflog viewer + restore — USER CHECKPOINT checklist

Native `pnpm tauri dev` verification. The AI gate (CLI-oracle + unit tests, clippy,
tsc/build, browser harness) is verified by the orchestrator; the steps below require the
real Tauri window + human perception and are confirmed by the user.

**All work on a SCRATCH repo — never a real repository. No reset/rebase against anything
but the scratch repo below.**

Reference: contract `docs/contracts/P38-reflog.md` §10 (USER CHECKPOINT).

---

## Scratch setup (single repo with a rewrite-heavy history)

Run once in a throwaway location (`D:\Temp\bonsai-scratch\p38-manual`):

```sh
mkdir -p /d/Temp/bonsai-scratch/p38-manual && cd /d/Temp/bonsai-scratch/p38-manual
git init -b main
git config user.name  "Test User"
git config user.email "test@example.com"
git config core.autocrlf false

printf 'base\n'      > f.txt && git add -A && git commit -m "c1: base"
printf 'base\nA\n'   > f.txt && git add -A && git commit -m "c2: add A"
printf 'base\nA\nB\n'> f.txt && git add -A && git commit -m "c3: add B"

# amend the tip (writes a "commit (amend):" reflog entry)
printf 'base\nA\nB\nb\n' > f.txt && git add -A && git commit --amend -m "c3: add B (amended)"

# a rebase that replays the tip (writes rebase reflog entries)
git rebase --exec "git --version" HEAD~1

# a destructive hard reset back one commit, then re-advance
#   (writes a "reset: moving to HEAD~1" reflog entry)
git reset --hard HEAD~1
printf 'base\nA\nC\n' > f.txt && git add -A && git commit -m "c3': add C"
```

Open **`p38-manual`** in Bonsai (`pnpm tauri dev`, then open the folder). You should see the
`main` branch and its history in the graph.

---

## Step (a) — HEAD reflog matches `git reflog`

1. In the toolbar, click **↺ Reflog** (tooltip: "View the HEAD reflog (recover prior
   positions after reset/rebase/amend)"). The overlay opens with header label **HEAD**.
2. Confirm the list is **newest-first** and each row shows: a `HEAD@{N}` badge, the
   `oldOid → newOid` short-oid transition, the reflog message, committer, and a relative
   date. Expect to see the distinct message kinds produced above — `commit:`,
   `commit (amend):`, `rebase …`, `reset: moving to HEAD~1`, `commit (initial):`.
3. **Cross-check with real git** — the overlay must match the CLI entry-for-entry:
   ```sh
   git -C /d/Temp/bonsai-scratch/p38-manual reflog
   # same order, same @{N} indices, same messages; the top row's short oid == HEAD
   git -C /d/Temp/bonsai-scratch/p38-manual rev-parse --short HEAD
   ```
4. Click a row → it **reveals** (selects + scrolls to) that entry's `newOid` in the graph if
   that commit is present. Press **Esc** → the reflog overlay closes (it is the first read
   overlay to take Esc). No console/UI errors on open/reveal/close.

---

## Step (b) — "Create branch here" creates a branch at the entry's newOid

1. Re-open **↺ Reflog**. On an **older** entry (e.g. a `HEAD@{N}` with N ≥ 2), open the row's
   kebab (⋯) action menu → **"Create branch here"**.
2. The shared **new-branch PromptDialog** appears. Enter a name, e.g. `recover-a`, confirm.
3. Expect a **success toast** and the new branch pill in the sidebar/graph.
4. **Cross-check** — the branch sits exactly at that entry's `newOid`:
   ```sh
   # read the target oid straight from the reflog selector you used (replace N):
   git -C /d/Temp/bonsai-scratch/p38-manual rev-parse "HEAD@{N}"
   git -C /d/Temp/bonsai-scratch/p38-manual rev-parse recover-a
   # the two oids must be EQUAL
   ```

---

## Step (c) — "Reset (hard)" moves the branch after the destructive confirm; Cancel is a no-op

1. Note the current tip first: `git -C /d/Temp/bonsai-scratch/p38-manual rev-parse HEAD`.
2. Re-open **↺ Reflog** on `main` (HEAD). On an **older** entry, kebab → **"Reset main to
   this (hard)…"**.
3. The **destructive hard-reset ConfirmDialog** appears (blocking, warns the working tree
   will be overwritten). **Click Cancel.**
   - **Cross-check nothing changed:**
     ```sh
     git -C /d/Temp/bonsai-scratch/p38-manual rev-parse HEAD   # UNCHANGED from step 1
     ```
4. Repeat the kebab → **"Reset main to this (hard)…"**, and this time **confirm**.
5. Expect a **success toast** + the graph/status refresh; `main`/HEAD now points at the
   chosen entry.
   - **Cross-check HEAD moved to the entry's newOid:**
     ```sh
     git -C /d/Temp/bonsai-scratch/p38-manual rev-parse "HEAD@{N}"   # the entry you picked (before the reset)
     git -C /d/Temp/bonsai-scratch/p38-manual rev-parse HEAD          # equals that oid
     ```
   - The reflog gains a new top `reset: moving to …` entry after refresh (a fresh reset
     writes a new `HEAD@{0}`).
6. (Optional) The soft/mixed reset variants in the same menu behave like the graph context
   menu's reset modes (index/worktree kept per mode); only **hard** shows the destructive
   confirm.

> Note: the **Reset** items are hidden when HEAD is **detached or unborn** (Create-branch
> stays available). Verify by checking out a commit detached
> (`git checkout --detach HEAD`) and re-opening the reflog: the Reset items are absent; the
> toolbar ↺ Reflog is still enabled. Return with `git checkout main`.

---

## Step (d) — Branch-menu "View reflog" shows that branch's reflog

1. Create a second branch with its own history so it has a distinct reflog:
   ```sh
   git -C /d/Temp/bonsai-scratch/p38-manual checkout -b feature
   printf 'feat\n' >> /d/Temp/bonsai-scratch/p38-manual/f.txt
   git -C /d/Temp/bonsai-scratch/p38-manual commit -am "feat: work"
   git -C /d/Temp/bonsai-scratch/p38-manual checkout main
   ```
   Refresh Bonsai (toolbar refresh / window focus) so `feature` appears.
2. Right-click the **`feature`** branch in the sidebar → **"View reflog"**.
3. The overlay opens with header label **branch: feature** and lists that branch's reflog
   (its commit entry on top).
4. **Cross-check:**
   ```sh
   git -C /d/Temp/bonsai-scratch/p38-manual reflog show feature
   # same entries/order as the overlay
   ```

---

## Step (e) — A branch with no reflog shows an empty overlay, not an error

1. Create a branch WITHOUT moving it (a fresh branch at HEAD has no reflog of its own until
   it is updated). The most reliable way to get a truly empty reflog is a ref that has never
   been updated; the simplest observable case here:
   ```sh
   # a brand-new branch that is then deleted-and-recreated has no reflog entries yet on
   # some git versions; if `feature`/`main` always carry one, use a detached name instead.
   git -C /d/Temp/bonsai-scratch/p38-manual branch fresh-no-log
   ```
   Refresh Bonsai. Right-click **`fresh-no-log`** → **"View reflog"**.
2. Expect the overlay to open showing the empty placeholder **"No reflog entries for
   branch: fresh-no-log"** — a clean empty state, **NOT** an error toast or a spinner that
   never resolves. (If your git writes a creation entry for new branches, this branch shows
   that single entry instead; the pass criterion is *no error* — empty or one-entry are both
   acceptable, an error is a failure.)

---

## Pass criteria

- (a) HEAD reflog lists the true history newest-first; `@{N}` indices, messages, and the top
  oid match `git reflog` / `git rev-parse HEAD`; row-click reveals the commit; Esc closes.
- (b) "Create branch here" creates a branch whose oid equals the entry's `newOid`
  (`git rev-parse` cross-check).
- (c) "Reset (hard)" is gated by a blocking destructive confirm: **Cancel** changes nothing;
  **confirm** moves HEAD to the entry's `newOid` (cross-check) and the reflog gains the new
  `reset:` entry. Reset items are hidden on detached/unborn HEAD.
- (d) Branch-menu "View reflog" opens that branch's reflog (header `branch: <name>`),
  matching `git reflog show <branch>`.
- (e) A branch with no reflog shows the empty placeholder, never an error.

## Cleanup

```sh
rm -rf /d/Temp/bonsai-scratch/p38-manual
```
