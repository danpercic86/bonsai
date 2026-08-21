# P17 — USER CHECKPOINT checklist (Interactive diff: File/Diff toggle + partial staging)

Run these in the **native app** (`pnpm tauri dev`) against a **scratch repo** — NOT a real
repository. These items are exactly what the AI gate could NOT self-verify: the native window,
canvas commit-selection driving the DiffBrowser, and byte-level index effects confirmed with the
`git` CLI in a terminal. The browser harness only exercises `src/main.rs` in the working-dir
overlay; the commit / Compare-with-HEAD toggle and all real-git effects need human eyes.

## 0. Prepare a scratch repo (throwaway — safe to delete)

In a terminal (any temp folder, e.g. `D:\Temp\p17-scratch`):

```
git init -b main p17-scratch && cd p17-scratch
git config core.autocrlf false
git config user.name  "P17 Tester"
git config user.email "p17@example.com"
printf 'alpha\nbravo\ncharlie\ndelta\necho\nfoxtrot\n' > notes.txt
git add -A && git commit -m "base"
```

Then edit `notes.txt` so there are several separated changes (so it has multiple hunks), e.g.
change `bravo` -> `BRAVO`, delete `delta`, and append two new lines at the end. Leave it
**unstaged**. Open this repo in Bonsai.

Keep a second terminal open in the repo for the `git diff` verifications below.

---

## 1. File / Diff toggle — working-dir overlay

1. In the right panel, click `notes.txt` under **Changes** (unstaged) to open its diff overlay.
2. In the overlay header, find the segmented **File | Diff** toggle (left of the `×`).
3. Toggle to **Diff**: the view shows separate `@@` hunks with a few context lines each.
4. Toggle to **File**: the view shows the whole file as one continuous listing (no `@@`
   headers), added/deleted lines still tinted. Confirm the two layouts genuinely differ.

## 2. File / Diff toggle — COMMIT diff and Compare-with-HEAD (native-only, NOT harness-verifiable)

The hidden browser pane cannot drive canvas commit-selection, so this was never harness-tested.

5. In the center commit graph, **click a past commit** so the right panel shows that commit's
   details + its changed files. Open one changed file's diff.
6. Confirm the **File | Diff** toggle appears in this commit-diff overlay and switches whole-file
   vs hunks. Confirm there are **NO** gutter `+`/`−` controls, **no** "Stage hunk" button, and
   **no** floating "Stage N lines" button — commit diffs are read-only.
7. Trigger a **Compare-with-HEAD** diff (select a commit and use the compare affordance / the
   DiffBrowser path that compares a file against HEAD). Confirm the same File | Diff toggle is
   present and it is likewise read-only (toggle only, no staging controls).

## 3. Stage a single line via the gutter `+`

8. Back on `notes.txt` (unstaged), hover a single changed line; a `+` appears in the marker
   gutter. Click it.
9. The file should now appear in **BOTH** the **Staged** and **Changes** sections (partial stage).
10. In the terminal verify EXACTLY that line moved:
    ```
    git diff --cached -- notes.txt   # shows ONLY the line you clicked
    git diff -- notes.txt            # shows the remaining changes, still unstaged
    ```

## 4. Stage a whole hunk via "Stage hunk"

11. In **Diff** view on the unstaged `notes.txt`, click **Stage hunk** on one hunk's header.
12. Verify with the CLI that exactly that hunk's lines are now staged and the other hunks are not:
    ```
    git diff --cached -- notes.txt
    git diff -- notes.txt
    ```

## 5. Stage a mouse-selected range → floating "Stage N lines"

13. Reset for a clean run: `git restore --staged notes.txt` in the terminal, refresh Bonsai.
14. In the diff, **click-drag** across a range covering at least two changed lines. A floating
    **"Stage N lines"** button appears (N = count of changed lines in the range). Click it.
15. Verify with `git diff --cached -- notes.txt` that **exactly** those lines moved and
    `git diff -- notes.txt` shows the rest still unstaged.
16. Repeat the drag but press **Escape** (or click away) instead of the button — the selection
    highlight clears and nothing is staged (confirm `git diff --cached` unchanged).

## 6. Symmetric unstage — hunk / line / selection from the staged diff

17. Stage the whole file (`git add notes.txt` in the terminal, or use the app's stage-all), then
    open the **Staged** entry's diff in Bonsai.
18. Use the gutter `−` on a single staged line → it unstages. Verify:
    ```
    git diff --cached -- notes.txt   # that line no longer staged
    git diff -- notes.txt            # that line back to unstaged
    ```
19. Repeat with **Unstage hunk** and with a mouse-range **"Unstage N lines"** floating button;
    verify each with `git diff --cached` / `git diff`.

## 7. Edge cases

20. **Emptied tracked file → Modified, not deleted.** In the terminal: `printf '' > notes.txt`
    (truncate to empty), refresh Bonsai. Stage the whole file. It should show as a **modified**
    empty file, NOT a deletion. Confirm: `git status --porcelain` shows `M ` (not `D `).
21. **CRLF, no phantom `^M`.** Create a CRLF file and commit it, then modify one line:
    ```
    printf 'one\r\ntwo\r\nthree\r\n' > crlf.txt && git add crlf.txt && git commit -m crlf
    printf 'one\r\ntwo CHANGED\r\nthree\r\n' > crlf.txt
    ```
    In Bonsai, stage a line of `crlf.txt`. Confirm `git diff --cached -- crlf.txt` shows a clean
    single-line change with **no `^M` / no whole-file rewrite**.
22. **Binary / renamed → whole-file stage only.** Add a binary file (or `git mv` + edit a file so
    it is a detected rename) and open its diff. Confirm the File | Diff toggle still shows, but the
    file offers only whole-file stage — **no** gutter `+`/`−`, hunk, or floating controls.

## 8. Round-trip / both-sections sanity

23. Partially stage a line, then unstage that same line from the staged diff; the file should
    return to its original state (a pure unstaged change) — `git diff --cached -- <file>` empty,
    `git diff -- <file>` shows the full original change. Staging/restaging should round-trip
    cleanly.
24. Confirm that while a file is **partially** staged it legitimately appears in **both** the
    Staged and Changes sections at once (this is expected, not a bug).

---

**Sign-off:** every numbered item behaves as described, and every `git diff --cached` / `git diff`
check shows exactly the lines you chose moved (and only those). Report any deviation to the
orchestrator with the failing step number and the two `git diff` outputs.
