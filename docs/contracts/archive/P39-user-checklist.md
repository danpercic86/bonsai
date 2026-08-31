# P39 — git bisect — USER CHECKPOINT checklist

Native `pnpm tauri dev` manual smoke test. **This is a USER CHECKPOINT, not an AI
gate**: the two-click bisect entry lives on the **canvas commit context-menu** and
the whole flow is driven by the in-progress **OpBanner**, neither of which the AI
browser harness (`VITE_MOCK_IPC=1`) can drive against a real repo — the mock only
walks a canned range. A human must run these steps against a real scratch repo and
confirm each checked-out midpoint actually changes the files on disk.

Automated coverage that already passed (context for the reviewer, not part of this
manual run): `bisect.rs` unit tests (14) + `tests/bisect_cli.rs` oracle suite (5,
incl. reset-restore and `ensure_on_current` guard cross-checked vs the real `git`).
The steps below are the parts those tests **cannot** exercise: the canvas menu, the
banner, and the Reset confirm dialog.

---

## 0. Build the scratch fixture (KNOWN first-bad)

Do this in a throwaway directory — **never a real repo**. Windows scratch root is
`D:\Temp\bonsai-scratch`. A linear history of 12 commits where a file gains a "bug"
line at commit **c6**, so **c6 is the known first-bad commit**:

```sh
# bash (Git Bash). Pick any empty scratch dir under D:/Temp/bonsai-scratch.
mkdir -p /d/Temp/bonsai-scratch/p39-manual && cd /d/Temp/bonsai-scratch/p39-manual
git init -b main
git config user.name  "Test User"
git config user.email "test@example.com"
git config core.autocrlf false
for i in $(seq 0 11); do
  echo "line $i" >> app.txt
  if [ "$i" -ge 6 ]; then echo "BUG"  > bug.txt; fi   # bug appears at c6 onward
  git add -A
  GIT_AUTHOR_DATE="2026-01-02T03:04:05+0000" GIT_COMMITTER_DATE="2026-01-02T03:04:05+0000" \
    git commit -q -m "c$i"
done
git log --oneline            # note the oids; the tip (c11) is BAD, c0 is GOOD
```

The **predicate** at each checked-out midpoint is simple and eyeball-verifiable:
**`bug.txt` exists in the working tree → the commit is BAD; otherwise GOOD.**

Independently confirm the oracle answer with a manual `git bisect` in a second
terminal (or after the app run) so you have git's authoritative culprit to compare:

```sh
git bisect start HEAD <c0-oid>
# at each checkout: `ls bug.txt` present -> `git bisect bad`, else `git bisect good`
# git prints: "<oid> is the first bad commit"  -> this MUST equal c6
git bisect reset
```

Open this repo in Bonsai: `pnpm tauri dev`, then open the folder above.

---

## 1. Start the bisect from the commit context-menu (two-click)

- [ ] Right-click the **tip commit (c11)** in the graph → menu shows
      **"Start bisect: mark this BAD"**. Click it.
- [ ] A toast appears: **"Bisect: now pick an older known-GOOD commit to start"**.
      No banner yet; nothing on disk has changed.
- [ ] Right-click the **oldest commit (c0)** → **"Mark GOOD & start bisect"** is now
      **enabled** (it is disabled until a pending-BAD exists and on the same commit).
      Click it.
- [ ] The **OpBanner** appears with title **"Bisecting"** and a sub-line
      **"N revisions left, ~K steps"** (N > 0), plus **Good / Bad / Skip / Reset**
      buttons. HEAD is now detached on the first midpoint.
- [ ] Cross-check in a terminal: `git rev-parse --abbrev-ref HEAD` prints `HEAD`
      (detached), and the checked-out midpoint matches the banner's current commit.

## 2. Answer midpoints to convergence (follow the bug)

At **each** midpoint the app checks out, decide from the working tree on disk:

- [ ] `ls bug.txt` (or look at the file panel): **present → click Bad**,
      **absent → click Good**. Confirm the files on disk actually change between
      midpoints (`cat app.txt` line count shrinks/grows; `bug.txt` appears/vanishes).
- [ ] After each click the banner's **"N revisions left"** count **decreases**.
- [ ] Repeat until the banner switches to **"Bisect found first bad commit"** showing
      an oid (and a short summary if available).
- [ ] Cross-check: the reported first-bad oid **equals c6** and equals git's manual
      `git bisect` answer from step 0. `git rev-parse HEAD` equals that same oid
      (HEAD stays detached on the culprit until Reset).

## 3. Skip an untestable midpoint (do this on a fresh run)

Reset (step 4) then start a new bisect and, at one midpoint:

- [ ] Click **Skip**. The banner picks a different adjacent midpoint (the count/step
      line updates) and the search continues to a correct **found c6** result.
- [ ] (Edge) If you skip until only skipped commits remain, a toast reports the
      cannot-determine case and the banner offers only **Reset** — no crash.

## 4. Reset returns to the original branch + tip (the #1 safety property)

- [ ] Click **Reset**. A **confirm dialog** appears (Reset is worktree-mutating and
      confirm-gated). Confirm it.
- [ ] The banner disappears. Cross-check in a terminal:
      - `git rev-parse --abbrev-ref HEAD` → **`main`** (re-attached, NOT detached),
      - `git rev-parse HEAD` → the **original tip (c11)** oid,
      - `git status --porcelain` → **empty** (clean worktree; `bug.txt` restored).
- [ ] Cancelling the confirm dialog instead leaves the bisect in progress unchanged.

## 5. Cleanup

- [ ] Ensure the scratch repo is **not left mid-bisect on a detached HEAD**: either
      you Reset in step 4, or run `git bisect reset` / delete the scratch dir.
      `D:\Temp\bonsai-scratch\p39-manual` can be deleted entirely afterwards.

---

### Pass criteria

All boxes checked, and specifically: (a) the banner counts decrease to a **found**
result, (b) the reported first-bad **== c6 == git's manual `git bisect` answer**,
(c) each midpoint checkout visibly changes files on disk, (d) **Reset re-attaches
`main` at the original tip with a clean worktree** (verified via `git rev-parse`).
Report any deviation to the orchestrator with the failing step number.
