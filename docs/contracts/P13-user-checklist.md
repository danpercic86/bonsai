# P13 — USER CHECKPOINT checklist (Local-AI foundation + AI merge-conflict resolution)

Run these steps yourself in the **native** app (`pnpm tauri dev`) with a **real, logged-in
`claude` CLI** on this machine's PATH. The AI gate — Rust unit + `ai_resolve_cli.rs` CLI-oracle
tests (all against the committed stub `claude`, no network), `cargo clippy -- -D warnings`,
`pnpm build`, and the browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`, `?op=merge`) — has already
passed. This checklist covers only what a human at the real Tauri window can verify: a live
subscription `claude` call, real conflict resolution, and the logged-out/absent fallback.

> **Why native + real `claude` only.** The AI gate never runs the real CLI (no network, no quota) —
> everything autonomous uses the stub via `BONSAI_CLAUDE_BIN`. A genuine subscription resolve, its
> latency/quality, and the "logged out / not installed" surface can only be judged by a human at the
> real window. Do NOT self-declare these from the harness.

Everything stays on **D:** — the scratch repo lives under `D:\Temp\bonsai-scratch`.

## 0. Preflight: confirm the real `claude` CLI

```powershell
claude --version          # prints e.g. "2.1.220 (Claude Code)"
claude -p "say ok" --output-format json   # exits 0 with a JSON envelope (proves you are logged in)
```

- [ ] `claude --version` succeeds (installed + on PATH).
- [ ] The `-p` probe returns a JSON envelope with a non-empty `result` (proves the subscription
      session is logged in). If it prompts to log in, run `claude` once interactively and log in
      first.

## 1. One-time setup: build a real `bothModified` conflict (PowerShell)

Two branches edit the SAME lines of the same file, then we pause a merge on the conflict. Safe to
re-run; it deletes and recreates the folder.

```powershell
$repo = 'D:\Temp\bonsai-scratch\p13-checkpoint'
if (Test-Path $repo) { Remove-Item -Recurse -Force $repo }
New-Item -ItemType Directory -Force $repo | Out-Null
git -C $repo init -b main
git -C $repo config user.name  "Checkpoint User"
git -C $repo config user.email "checkpoint@example.com"
git -C $repo config core.autocrlf false

# Base commit.
Set-Content "$repo\config.js" "export const settings = {`n  timeout: 30,`n  retries: 3,`n  verbose: false,`n};`n"
git -C $repo add -A
git -C $repo commit -m "base config"

# feature/tuning = THEIRS side: edits the SAME lines.
git -C $repo checkout -b feature/tuning
Set-Content "$repo\config.js" "export const settings = {`n  timeout: 60,`n  retries: 5,`n  verbose: false,`n};`n"
git -C $repo add -A
git -C $repo commit -m "tune timeout + retries"

# main = OURS side: edits the SAME lines differently (checked out at the end).
git -C $repo checkout main
Set-Content "$repo\config.js" "export const settings = {`n  timeout: 45,`n  retries: 2,`n  verbose: true,`n};`n"
git -C $repo add -A
git -C $repo commit -m "main tweaks"
```

## 2. Enable AI in Settings → consent dialog → Enable

```powershell
cd D:\Repos\Playground\bonsai
pnpm tauri dev
```

- [ ] Open **Settings**. There is an **AI assistance** section.
- [ ] The availability line reads something like **"Claude Code 2.1.220 ready"** (matches your
      `claude --version`). This confirms the backend probe found the real CLI.
- [ ] Toggle **Enable AI** ON. Because consent has not been given yet, a **consent dialog** appears:
      title **"Enable AI features?"**, body explaining that conflicted-file contents are sent to the
      local `claude` CLI under your subscription (nothing to Bonsai's servers, nothing changed
      without review), confirm button **"Enable"**.
- [ ] Click **Enable**. The toggle stays ON; the **autonomy radio** ("Propose & review" /
      "Auto-resolve, then review") becomes enabled. Default is **Propose & review**.
- [ ] (Persistence) Close and reopen Settings — AI stays enabled and consent is remembered (no
      second consent prompt).

## 3. Start the merge

- [ ] Open `D:\Temp\bonsai-scratch\p13-checkpoint`. The graph shows `main` (current) and
      `feature/tuning` diverged from base.
- [ ] Right-click `feature/tuning` in the sidebar → **Merge feature/tuning into main**. The merge
      stops with conflicts; the **OpBanner** appears; the right panel lists `config.js` as
      conflicted (kind **both modified**).

## 4. Propose & review (autonomy = Propose & review)

- [ ] The `config.js` conflict row shows a **✨ AI** action (title "Resolve with AI"), alongside the
      manual **ours / theirs / mark-resolved** buttons.
- [ ] Click **✨ AI**. A brief busy state shows (the real call takes a few seconds). Then a review
      overlay opens the **ConflictEditor seeded with the AI-proposed, markerless merged file** —
      NOT the raw marker view. The proposed body integrates both sides and contains **no**
      `<<<<<<<`/`=======`/`>>>>>>>` markers.
- [ ] Edit the proposal by hand (e.g. change one value) to confirm it is editable before accepting.
- [ ] Click **Accept / Stage resolved**. The `config.js` conflict row **disappears**, the OpBanner
      conflict count drops to **0**, and the file is staged.
- [ ] Verify on disk (optional):
      `git -C 'D:\Temp\bonsai-scratch\p13-checkpoint' status` shows `config.js` staged, no longer
      "both modified".

## 5. Commit the merge

- [ ] Use the **OpBanner** to **commit the merge** (default merge message). The merge finalizes:
      OpBanner clears.
- [ ] Verify a clean, 2-parent merge:
      ```powershell
      git -C 'D:\Temp\bonsai-scratch\p13-checkpoint' log --oneline --graph -n 5
      git -C 'D:\Temp\bonsai-scratch\p13-checkpoint' log -1 --format=%P   # prints TWO parent hashes
      git -C 'D:\Temp\bonsai-scratch\p13-checkpoint' status              # "working tree clean"
      ```
- [ ] `git log` shows the new merge commit with both `main` and `feature/tuning` as parents;
      `git status` is clean.

## 6. Auto-resolve autonomy → repeat

Rebuild the conflict (re-run the section-1 PowerShell block to recreate `p13-checkpoint`), then:

- [ ] In **Settings → AI assistance**, switch autonomy to **Auto-resolve, then review**.
- [ ] Start the merge again (section 3) and click **✨ AI** on the `config.js` row.
- [ ] This time NO review overlay opens: the file is **staged directly** and a **success toast**
      appears (e.g. *"Resolved config.js with AI — review the staged result"*).
- [ ] The staged result is still reviewable before committing: open the staged diff for `config.js`
      and confirm it is markerless and sensible. Then commit the merge as in section 5 and confirm
      the 2-parent commit + clean status.

## 7. Logged-out / absent `claude` fallback

Simulate the CLI being unavailable so you can see the graceful-disable path. Easiest: temporarily
rename/remove `claude` from PATH, OR log out (`claude` interactive `/logout`), then restart
`pnpm tauri dev`.

- [ ] **Not installed / not on PATH:** Settings AI section shows an amber note like *"Claude Code
      CLI not found on PATH — install it and log in to use AI features"*, and the **✨ AI** conflict
      action is **disabled** (or hidden) with guidance. The manual **ours / theirs / mark-resolved**
      buttons still work — you can resolve `config.js` manually and commit the merge.
- [ ] **Logged out (CLI present):** availability may still read "ready" (auth is not cheaply
      probed), but clicking **✨ AI** surfaces a clear **error toast** carrying the CLI's own auth
      message (an `aiFailed`), nothing is written, and the manual buttons remain usable.

## Cleanup

Restore `claude` on PATH / log back in if you changed it, close the app (Ctrl+C in the
`pnpm tauri dev` terminal), then:

```powershell
Remove-Item -Recurse -Force 'D:\Temp\bonsai-scratch\p13-checkpoint'
```

Leftover `bonsai-*` folders under `D:\Temp\bonsai-scratch` are abandoned test temp dirs and can be
deleted anytime.
