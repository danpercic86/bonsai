# M3 — USER CHECKPOINT checklist (stage / unstage / commit)

Run these steps yourself in the native app. The AI gate (Rust CLI-oracle tests, `pnpm build`,
browser-harness smoke) has already passed; this checklist covers what only a human at the real
Tauri window can verify. M3 is done only when every step below behaves as described.

Everything stays on **D:** — the scratch repo lives under `D:\Temp\bonsai-scratch`.

## 1. One-time setup: create the scratch repo (PowerShell)

```powershell
$repo = 'D:\Temp\bonsai-scratch\checkpoint-repo'
New-Item -ItemType Directory -Force $repo | Out-Null
git -C $repo init -b main
git -C $repo config user.name  "Checkpoint User"
git -C $repo config user.email "checkpoint@example.com"
Set-Content "$repo\readme.txt"  "hello bonsai"
Set-Content "$repo\notes.txt"   "first draft"
git -C $repo add -A
git -C $repo commit -m "base commit"
# leave some work in progress:
Set-Content "$repo\readme.txt"  "hello bonsai - edited"   # -> Unstaged (M)
Set-Content "$repo\new-file.txt" "brand new"              # -> Untracked (U)
```

## 2. Launch

```powershell
cd D:\Repos\Playground\bonsai
pnpm tauri dev
```

- [ ] Window opens titled **Bonsai**; use the folder picker to open
      `D:\Temp\bonsai-scratch\checkpoint-repo`.
- [ ] Header shows the repo name, branch `main`, and a HEAD oid.
- [ ] Right panel shows: Unstaged (1) `readme.txt` [M], Untracked (1) `new-file.txt` [U],
      no Staged section content.

## 3. Stage via row button

- [ ] Hover the `readme.txt` row → a `+` button appears at the row's right edge.
- [ ] Click it → the row moves to **Staged** immediately (no ~300 ms debounce pause);
      buttons are disabled while the call is in flight.

## 4. Unstage

- [ ] Hover the staged `readme.txt` row → `−` button appears; click → row returns to Unstaged.
- [ ] Terminal cross-check: `git -C D:\Temp\bonsai-scratch\checkpoint-repo status` agrees
      (readme.txt not staged).

## 5. Stage all

- [ ] Click **Stage all** on the Unstaged section header, then **Stage all** on Untracked
      → both files land in Staged (`new-file.txt` shows badge A).

## 6. Commit button disabled states

- [ ] With staged files but an **empty message**: Commit button is disabled.
- [ ] Type a message, then **Unstage all** so Staged is empty: button is disabled again.
- [ ] Re-stage (Stage all on both sections); with a message present the button enables.
- [ ] (Nice-to-see) Type a first line longer than 72 chars → an amber `n/72` counter appears.

## 7. Commit

- [ ] Type `checkpoint commit` and click **Commit** (or press **Ctrl+Enter** in the textarea).
- [ ] Status panel clears to "No changes"; the textarea empties; the header HEAD oid changes.
- [ ] The graph shows the new commit at the top with the `main` branch pill
      (click Refresh if it has not already updated).
- [ ] Terminal cross-check:
      `git -C D:\Temp\bonsai-scratch\checkpoint-repo log --oneline -2` shows
      `checkpoint commit` on top; `git status` is clean.

## 8. (Optional) configMissing error surface

The repo has a local identity, so remove it temporarily:

```powershell
git -C D:\Temp\bonsai-scratch\checkpoint-repo config --unset user.name
```

- [ ] Only meaningful if you have **no global `user.name`** — check with
      `git config --global user.name`. If a global value exists, this step will still commit
      fine (config levels cascade); skip it or expect success.
- [ ] Otherwise: edit a file, stage it, try to commit → an inline red banner appears starting
      with "Set your Git identity:" and naming `user.name`, with the `git config` command hint;
      the banner is dismissible; the staged list is unchanged.
- [ ] Restore: `git -C D:\Temp\bonsai-scratch\checkpoint-repo config user.name "Checkpoint User"`
      → the same commit now succeeds.

## 9. Cleanup

Close the app (Ctrl+C in the `pnpm tauri dev` terminal), then:

```powershell
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\checkpoint-repo'
```

Leftover `bonsai-*` folders under `D:\Temp\bonsai-scratch` are abandoned test temp dirs and can
be deleted at any time.
