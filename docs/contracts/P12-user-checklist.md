# P12 — USER CHECKPOINT checklist (rich conflict-resolution editor)

Run these steps yourself in the **native** app (`pnpm tauri dev`). The AI gate — Rust unit +
`conflict_cli.rs` CLI-oracle tests (`resolve_conflict_text` stages the same stage-0 blob oid as
`git add`), `pnpm build` / `tsc`, and the browser-harness `conflictSelfTest` — has already passed.
This checklist covers only what a human at the real Tauri window can verify.

> **Why native-only.** CodeMirror 6 (`@codemirror/merge`) lays out its editor, scrollbar, gutters,
> and overview-ruler ticks from DOM measurement driven by `requestAnimationFrame`. The browser
> harness pane runs with `document.hidden === true`, so the browser THROTTLES rAF and CM cannot
> measure/paint correctly there. Therefore every visual/interactive editor check below MUST be done
> in `pnpm tauri dev` and MUST NOT be self-declared from the harness.

Everything stays on **D:** — the scratch repo lives under `D:\Temp\bonsai-scratch`.

## 1. One-time setup: build a real `bothModified` conflict (PowerShell)

Creates a repo with a base file, two branches editing the SAME lines of TWO files (a small file
`app.js` and a larger one `notes.md` so you can exercise scrolling + the overview ruler), ready to
merge. Safe to re-run; it deletes and recreates the folder.

```powershell
$repo = 'D:\Temp\bonsai-scratch\p12-checkpoint'
if (Test-Path $repo) { Remove-Item -Recurse -Force $repo }
New-Item -ItemType Directory -Force $repo | Out-Null
git -C $repo init -b main
git -C $repo config user.name  "Checkpoint User"
git -C $repo config user.email "checkpoint@example.com"
git -C $repo config core.autocrlf false

# Base: app.js with several lines, and a long notes.md.
Set-Content "$repo\app.js" "function greet(name) {`n  const msg = 'hello ' + name;`n  console.log(msg);`n  return msg;`n}`n"
Set-Content "$repo\notes.md" (1..60 | ForEach-Object { "line $_ base" })
git -C $repo add -A
git -C $repo commit -m "base"

# feature/login = THEIRS side.
git -C $repo checkout -b feature/login
Set-Content "$repo\app.js" "function greet(name) {`n  const msg = 'hi ' + name + '!';`n  console.log(msg);`n  return msg;`n}`n"
$n = 1..60 | ForEach-Object { if ($_ -eq 10 -or $_ -eq 40) { "line $_ THEIRS" } else { "line $_ base" } }
Set-Content "$repo\notes.md" $n
git -C $repo add -A
git -C $repo commit -m "feature edits"

# main = OURS side (checked out at the end, ready to merge feature/login).
git -C $repo checkout main
Set-Content "$repo\app.js" "function greet(name) {`n  const msg = 'hey ' + name;`n  console.log(msg);`n  return msg;`n}`n"
$n = 1..60 | ForEach-Object { if ($_ -eq 10 -or $_ -eq 40) { "line $_ OURS" } else { "line $_ base" } }
Set-Content "$repo\notes.md" $n
git -C $repo add -A
git -C $repo commit -m "main edits"
```

## 2. Launch + start the merge

```powershell
cd D:\Repos\Playground\bonsai
pnpm tauri dev
```

- [ ] Open `D:\Temp\bonsai-scratch\p12-checkpoint`. Graph shows `main` (current) and
      `feature/login` diverged from `base`.
- [ ] Right-click `feature/login` in the sidebar → **Merge feature/login into main**. The merge
      stops with conflicts; the **OpBanner** appears (merge in progress, N conflicts to resolve),
      and the right panel lists `app.js` and `notes.md` as conflicted.

## 3. The editor mounts (text kinds only)

- [ ] Click the `app.js` conflict row → the center pane opens the **ConflictEditor** (a CodeMirror
      editor), NOT the old read-only `<pre>` marker view.
- [ ] Header shows the file path in mono + a language chip (`javascript` for `app.js`), a
      **Unified / Side-by-side** toggle, and **Cancel** + **Stage resolved** buttons.
- [ ] Editor renders with **line numbers** and **syntax highlighting** (JS keywords/strings
      colored). The conflict region is **visibly tinted** (ours vs theirs blocks distinguishable).

## 4. Per-region accept widgets (Accept Ours / Theirs / Both)

- [ ] A small toolbar sits at each `<<<<<<<` region: **Accept Ours**, **Accept Theirs**,
      **Accept Both**, with the `HEAD` / `feature/login` labels as captions.
- [ ] Click **Accept Ours** → the region collapses to the OURS body only; markers vanish for that
      region. Undo (Ctrl+Z) restores it.
- [ ] Click **Accept Theirs** → collapses to the THEIRS body only.
- [ ] Click **Accept Both** → keeps BOTH bodies, **ours block FIRST then theirs block** (matching
      git marker order); no markers remain for that region.

## 5. Direct editing + Save gate

- [ ] With markers still present, **Stage resolved** is **disabled**. Directly type in the editor to
      hand-merge; **Stage resolved** stays disabled until the doc contains **zero** conflict markers
      (`<<<<<<<`, `=======`, `>>>>>>>`), then it enables.
- [ ] Edits typed by hand persist (the merged result is the single source of truth).

## 6. Unified ⇄ Side-by-side toggle preserves edits (both directions)

- [ ] Open `notes.md` (has two conflict regions, lines ~10 and ~40 — requires scrolling). Resolve
      region 1 (Accept Ours) and hand-edit somewhere.
- [ ] Toggle to **Side-by-side** → left pane = read-only **Ours**, right pane = the editable
      **Result** seeded from your in-progress edits. Your region-1 resolution and hand edit are
      **still there** (not lost).
- [ ] In split mode, use the chunk-accept gutter (a→b) AND/OR the region toolbar to resolve
      region 2. Toggle back to **Unified** → all resolutions preserved in both directions.

## 7. Overview-ruler ticks + click-jump

- [ ] While `notes.md` still has an unresolved region, the scrollbar/right-edge **overview ruler**
      shows an accent tick per unresolved region at its vertical position.
- [ ] Click a tick → the editor **scrolls that region into view** (centered).
- [ ] As you resolve regions, their ticks **disappear**; when zero regions remain, no ticks show.

## 8. Save stages the file and the conflict clears

- [ ] Fully resolve `app.js` (zero markers) → click **Stage resolved** → a success toast
      (`Staged resolution for app.js`); the `app.js` conflict row **disappears** and the OpBanner's
      conflict count **drops by one**.
- [ ] Verify on disk (optional): `git -C 'D:\Temp\bonsai-scratch\p12-checkpoint' status` shows
      `app.js` staged (no longer "both modified"), and its worktree content equals what you saved.
- [ ] Repeat for `notes.md` → its row disappears; the OpBanner shows **0 conflicts remaining**.

## 9. Commit the merge via OpBanner

- [ ] With all conflicts resolved, use the **OpBanner** to **commit the merge** (default merge
      message). The merge completes: OpBanner clears, the graph shows the new merge commit with both
      `main` and `feature/login` as parents.
- [ ] (Alternative) **Cancel** in the editor collapses the slot without staging (row stays
      conflicted); **Abort merge** in the OpBanner unwinds the whole merge — verify each once.

## 10. Non-text kinds still use the fallback

Confirm the editor is additive, not a replacement, for non-text conflicts:

- [ ] Build a `deletedByThem` conflict (delete a file on one side, edit it on the other) and merge
      it. Clicking that row opens the **read-only placeholder / marker view** with the existing
      ours/theirs/mark-resolved quick actions — **not** the CodeMirror editor. Binary and
      too-large (> 1 MiB) conflicts likewise stay on the placeholder path.

## Cleanup

Close the app (Ctrl+C in the `pnpm tauri dev` terminal), then:

```powershell
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\p12-checkpoint'
```

Leftover `bonsai-*` folders under `D:\Temp\bonsai-scratch` are abandoned test temp dirs and can be
deleted anytime.
