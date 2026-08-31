# M5 — USER CHECKPOINT checklist (branches)

Run these steps yourself in the native app. The AI gate (24 CLI-oracle branch tests incl. 5
adversarial probes, full `cargo test`, `pnpm build`, browser-harness smoke) has already passed;
this checklist covers what only a human at the real Tauri window can verify: sidebar rendering,
the delete confirmation dialog's focus/keyboard behavior, and that checkout/delete visibly track
the real repo. M5 is done only when every step below behaves as described.

Everything stays on **D:** — the scratch repo lives under `D:\Temp\bonsai-scratch`.

## 1. One-time setup: create the checkpoint repo (PowerShell)

Builds a repo with: `main` (current, upstream `origin/main` ahead 2 / behind 1), `merged-topic`
(fully merged → deletable), `unmerged-topic` (own commit → delete blocked), `side` (its
`file.txt` differs from main → dirty-checkout conflict target), one remote-tracking ref, and
two tags. Safe to re-run; it deletes and recreates both folders.

```powershell
$repo = 'D:\Temp\bonsai-scratch\m5-checkpoint'
$bare = 'D:\Temp\bonsai-scratch\m5-checkpoint-remote.git'
if (Test-Path $repo) { Remove-Item -Recurse -Force $repo }
if (Test-Path $bare) { Remove-Item -Recurse -Force $bare }
New-Item -ItemType Directory -Force $repo | Out-Null
git init --bare $bare
git -C $repo init -b main
git -C $repo config user.name  "Checkpoint User"
git -C $repo config user.email "checkpoint@example.com"
git -C $repo config core.autocrlf false

# Base commit + a branch that stays fully merged + a lightweight tag
Set-Content "$repo\file.txt"   "main v1"
Set-Content "$repo\shared.txt" "shared v1"
git -C $repo add -A
git -C $repo commit -m "base commit"
git -C $repo branch merged-topic
git -C $repo tag v0.9.0

# side: file.txt DIFFERS from main (dirty-checkout conflict target)
git -C $repo checkout -b side
Set-Content "$repo\file.txt" "side v1"
git -C $repo add -A
git -C $repo commit -m "side change"

# unmerged-topic: one commit of its own -> delete must be blocked
git -C $repo checkout -b unmerged-topic main
Set-Content "$repo\topic.txt" "topic work"
git -C $repo add -A
git -C $repo commit -m "unmerged topic work"
git -C $repo checkout main

# Upstream with real ahead/behind: push base, force-move origin/main onto a
# diverged commit (behind 1), then advance main twice (ahead 2)
git -C $repo remote add origin $bare
git -C $repo push -u origin main
git -C $repo checkout -b tmp-remote
Set-Content "$repo\remote.txt" "remote-only change"
git -C $repo add -A
git -C $repo commit -m "remote-only commit"
git -C $repo checkout main
git -C $repo update-ref refs/remotes/origin/main tmp-remote
git -C $repo branch -D tmp-remote          # scratch-repo-only cleanup of the helper branch
Set-Content "$repo\file.txt" "main v2"
git -C $repo add -A
git -C $repo commit -m "main v2"
Set-Content "$repo\file.txt" "main v3"
git -C $repo add -A
git -C $repo commit -m "main v3"
git -C $repo tag -a v1.0.0 -m "release v1.0.0"
```

## 2. Launch + sidebar renders

```powershell
cd D:\Repos\Playground\bonsai
pnpm tauri dev
```

- [ ] Window opens titled **Bonsai**; open `D:\Temp\bonsai-scratch\m5-checkpoint` via the
      folder picker.
- [ ] Sidebar shows three sections. **BRANCHES**: `main`, `merged-topic`, `side`,
      `unmerged-topic` (case-insensitive alphabetical order). **REMOTES**: `origin/main`.
      **TAGS**: `v0.9.0`, `v1.0.0`.
- [ ] `main` is visually current: accent color, 600 weight, `●` dot instead of the branch glyph
      — and matches the branch name in the header.
- [ ] `main` shows the ahead/behind badge **`↑2 ↓1`**; the other branches show no badge.
- [ ] Hovering the `main` row shows **no** checkout/delete action buttons (current branch).
- [ ] Section headers collapse/expand on click (chevron); all default expanded.

## 3. Create branch

- [ ] Click the `+` in the BRANCHES header → an inline input appears with focus.
- [ ] Type `bad..name`, Enter → inline red error under the input (invalid branch name);
      nothing added.
- [ ] Clear it, type `main`, Enter → inline error "branch 'main' already exists".
- [ ] Clear it, type `feature/demo`, Enter → input closes; `feature/demo` appears sorted into
      the list. `main` is **still** the current branch (create does NOT check out) and a new
      `feature/demo` pill appears in the graph on the HEAD commit.

## 4. Checkout

- [ ] Hover `feature/demo` → `⇄` and delete buttons fade in; click `⇄`.
- [ ] The `●` current-dot moves to `feature/demo`, the header branch name updates to
      `feature/demo`, and the graph's HEAD pill follows to the same commit.
- [ ] **Double-click** the `main` row → checks out `main` again (dot, header, HEAD pill all
      return).

## 5. Delete blocked on unmerged branch

- [ ] Hover `unmerged-topic` → click the delete button → the confirmation dialog opens.
- [ ] Confirm with the danger button → dialog closes and a **sidebar error banner** appears:
      "branch 'unmerged-topic' is not fully merged into HEAD … use `git branch -D
      unmerged-topic` if you are sure."
- [ ] `unmerged-topic` is **still in the list** (nothing was deleted). Dismiss the banner
      with its ✕.

## 6. Delete merged branch — dialog behavior

- [ ] Hover `merged-topic` → click delete → dialog: title **Delete branch**, body
      `Delete branch "merged-topic"?` plus the "fully merged, but this cannot be undone from
      Bonsai." line, buttons `Cancel` + danger `Delete branch`.
- [ ] **Initial focus is on Cancel**: press Enter immediately → the dialog closes and the
      branch is NOT deleted.
- [ ] Reopen the dialog → press **Esc** → closes, nothing deleted. Reopen → click the overlay
      (outside the card) → closes, nothing deleted.
- [ ] Reopen → click the red **Delete branch** button → the row disappears from the sidebar and
      the `merged-topic` pill disappears from the graph.
- [ ] Also delete `feature/demo` the same way (cleanup for step 8's tag pill visibility).

## 7. Dirty-checkout conflict changes nothing

- [ ] In PowerShell (app stays open):
      `Set-Content 'D:\Temp\bonsai-scratch\m5-checkpoint\file.txt' "dirty local edit"`
      → refocus the app; `file.txt` appears under Unstaged.
- [ ] Hover `side` → click `⇄` → sidebar error banner: "cannot switch to 'side': local changes
      would be overwritten. Commit or discard them first."
- [ ] Nothing changed: header still `main`, current-dot still on `main`, `file.txt` still
      listed as modified, and `Get-Content 'D:\Temp\bonsai-scratch\m5-checkpoint\file.txt'`
      still prints `dirty local edit`.
- [ ] Discard it: `git -C 'D:\Temp\bonsai-scratch\m5-checkpoint' restore file.txt` → refocus
      the app → Unstaged is empty again; now checkout `side` → succeeds (dot + header move);
      then checkout `main` again.

## 8. Detached HEAD display

- [ ] In PowerShell: `git -C 'D:\Temp\bonsai-scratch\m5-checkpoint' checkout v1.0.0`
      (detaches HEAD at the tag) → refocus the app.
- [ ] Sidebar BRANCHES section shows a pinned, non-clickable first row
      `◎ HEAD detached @ <short-oid>` in warning tint; **no** branch row is highlighted;
      the header shows the detached state; the graph shows the HEAD pill on the tagged commit.
- [ ] Recover: `git -C 'D:\Temp\bonsai-scratch\m5-checkpoint' checkout main` → refocus →
      `main` is highlighted again and the detached row is gone.

## 9. Cleanup

Close the app (Ctrl+C in the `pnpm tauri dev` terminal), then:

```powershell
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\m5-checkpoint'
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\m5-checkpoint-remote.git'
```

Leftover `bonsai-*` folders under `D:\Temp\bonsai-scratch` are abandoned test temp dirs and can
be deleted at any time.
