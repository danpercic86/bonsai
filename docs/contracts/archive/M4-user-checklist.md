# M4 — USER CHECKPOINT checklist (diff view)

Run these steps yourself in the native app. The AI gate (19 CLI-oracle diff tests, unit tests,
`pnpm build`, browser-harness smoke) has already passed; this checklist covers what only a human
at the real Tauri window can verify: that the diffs Bonsai renders match what a terminal
`git diff` / `git show` prints for the same repo. M4 is done only when every step below behaves
as described.

Everything stays on **D:** — the scratch repo lives under `D:\Temp\bonsai-scratch`.

## 1. One-time setup: create/extend the checkpoint repo (PowerShell)

This rebuilds the M3 checkpoint repo from scratch with a small history so both diff modes have
something to show (safe to re-run; it deletes and recreates the folder).

```powershell
$repo = 'D:\Temp\bonsai-scratch\checkpoint-repo'
if (Test-Path $repo) { Remove-Item -Recurse -Force $repo }
New-Item -ItemType Directory -Force $repo | Out-Null
git -C $repo init -b main
git -C $repo config user.name  "Checkpoint User"
git -C $repo config user.email "checkpoint@example.com"
git -C $repo config core.autocrlf false

# Commit 1: base
Set-Content "$repo\readme.txt" "hello bonsai`nline two`nline three`nline four`nline five"
Set-Content "$repo\notes.txt"  "first draft`nsecond line`nthird line"
git -C $repo add -A
git -C $repo commit -m "base commit"

# Commit 2: modify readme + add a new file (multi-line message for the details panel)
Set-Content "$repo\readme.txt" "hello bonsai`nline two EDITED`nline three`nline four`nline five"
Set-Content "$repo\extra.txt"  "extra one`nextra two"
git -C $repo add -A
git -C $repo commit -m "feat: edit readme and add extra`n`nThis body line should appear in the commit details panel."

# Working-dir state for mode A:
Set-Content "$repo\notes.txt" "first draft`nsecond line CHANGED`nthird line"   # -> Unstaged (M)
Set-Content "$repo\readme.txt" "hello bonsai STAGED`nline two EDITED`nline three`nline four`nline five"
git -C $repo add readme.txt                                                    # -> Staged (M)

# Small binary file (a real 1x1 PNG), untracked:
$png = [Convert]::FromBase64String('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==')
[IO.File]::WriteAllBytes("$repo\dot.png", $png)
git -C $repo add dot.png    # stage it so its diff row appears under Staged as Added
```

## 2. Launch

```powershell
cd D:\Repos\Playground\bonsai
pnpm tauri dev
```

- [ ] Window opens titled **Bonsai**; open `D:\Temp\bonsai-scratch\checkpoint-repo` via the
      folder picker. Right panel shows Staged: `dot.png` [A], `readme.txt` [M];
      Unstaged: `notes.txt` [M].

## 3. Mode A — unstaged diff matches `git diff`

- [ ] Click the **text** of the `notes.txt` row under Unstaged → the row highlights, a chevron
      rotates, and (after a brief skeleton) a unified diff expands inline: hunk header
      `@@ -1,3 +1,3 @@`, red `-second line` / green `+second line CHANGED` rows with old/new
      line numbers.
- [ ] Terminal cross-check — the hunks (numbers, +/− lines, context) match exactly:
      `git -C D:\Temp\bonsai-scratch\checkpoint-repo diff -- notes.txt`
- [ ] Click the row text again → the diff collapses.

## 4. Mode A — staged diff matches `git diff --cached`

- [ ] Click the `readme.txt` row text under **Staged** → its diff expands (change on line 1,
      `hello bonsai` → `hello bonsai STAGED`).
- [ ] Terminal cross-check:
      `git -C D:\Temp\bonsai-scratch\checkpoint-repo diff --cached -- readme.txt`
- [ ] Only one row is expanded at a time: expanding `readme.txt` collapsed `notes.txt` (and
      vice versa).

## 5. Mode A — binary placeholder

- [ ] Click the staged `dot.png` row text → the expansion shows the **"Binary file"**
      placeholder message, not hunks and not garbage bytes.

## 6. Mode A — staging invalidates the expansion

- [ ] Expand `notes.txt` (Unstaged), then click its `+` stage button → the row moves to Staged
      and the expansion collapses (no orphaned diff left behind).
- [ ] Unstage it again (`−`) for the later steps; its row returns to Unstaged.

## 7. Mode B — commit details

- [ ] Click the top commit (`feat: edit readme and add extra`) in the graph → the right panel
      swaps to commit details: summary line, mono short oid, author
      `Checkpoint User <checkpoint@example.com>`, date, a "Parents:" link, and the message body
      line "This body line should appear in the commit details panel."
- [ ] The status panel and commit box are gone while a commit is selected.
- [ ] File list shows `Changes (2)`: `extra.txt` [A] `+2`, `readme.txt` [M] `+1 −1`.

## 8. Mode B — commit file diff matches `git show`

- [ ] Expand `readme.txt` in the commit's file list → one hunk: red `-line two` /
      green `+line two EDITED`.
- [ ] Terminal cross-check — hunks match:
      `git -C D:\Temp\bonsai-scratch\checkpoint-repo show HEAD -- readme.txt`
- [ ] Expand `extra.txt` → all-green added diff (`@@ -0,0 +1,2 @@`), matching
      `git -C D:\Temp\bonsai-scratch\checkpoint-repo show HEAD -- extra.txt`.

## 9. Navigation

- [ ] Click the parent short-oid link in the details header → selection jumps to `base commit`
      (graph highlight moves, panel now shows the root commit: no parent links, all files [A]).
- [ ] Press **Esc** → back to the status view (Staged/Unstaged lists + commit box).
- [ ] Select a commit again, then click empty canvas space → also returns to the status view.
- [ ] Select a commit, click "×" in the panel's top corner → same.

## 10. Cleanup

Close the app (Ctrl+C in the `pnpm tauri dev` terminal), then:

```powershell
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\checkpoint-repo'
```

Leftover `bonsai-*` folders under `D:\Temp\bonsai-scratch` are abandoned test temp dirs and can
be deleted at any time.
