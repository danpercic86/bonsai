# P49 — External Integrations — USER CHECKPOINT checklist

The AI gate proves argv correctness (unit tests) + the mock-driven UI (menus, toolbar, settings,
error toast). It **cannot** prove a real launch — that needs the native Tauri window. Run
`pnpm tauri dev`, open a real repo, and confirm each item. The orchestrator must NOT self-pass these.

## A. Launch — all three actions, every entry point
For each entry point (repo **toolbar button**, **tab right-click menu**, **worktree row menu**,
**submodule row menu**):
1. **Open in terminal** — a terminal opens **at the repo/worktree/submodule directory** (verify the
   prompt's working directory is that path, not the home dir).
2. **Reveal in file manager** — the OS file manager opens showing that directory.
3. **Open in editor** — the configured/auto-detected editor opens that directory.

## B. Per-OS specifics
- **Windows:** terminal auto-detect opens **Windows Terminal** (`wt`). Then disable the `wt` App
  Execution Alias (Settings → Apps → Advanced app settings → App execution aliases) and confirm it
  **falls back to PowerShell**, then (rename/hide pwsh) to **cmd** — all at the correct directory,
  each a **visible** window (no hidden/instant-closing console). Editor `code` resolves even though
  it is `code.cmd` (PATHEXT). Reveal opens Explorer (ignore its nonzero exit code).
- **macOS:** terminal opens **Terminal.app** at the directory; setting the template to
  `open -a iTerm {path}` opens **iTerm**. Reveal opens **Finder**. Editor opens VS Code.
- **Linux:** terminal opens the available emulator (GNOME Terminal / Konsole / `x-terminal-emulator`)
  at the directory. Reveal opens the default file manager via `xdg-open`. Editor opens VS Code.

## C. Paths with spaces
Use a repo whose path contains a space (e.g. `.../my repo/`). All three actions land on the exact
directory with the space intact — no truncation, no "cannot find path", no split into two args.

## D. Configuration persists across restart
1. Settings → External tools: set a custom **terminal command** (e.g. a non-default emulator) and a
   custom **editor command**; confirm they launch as configured.
2. Quit and relaunch Bonsai; confirm both fields still hold the custom values and still launch
   correctly.
3. Click **Reset to auto-detect** (clears to blank); confirm auto-detect behavior returns.

## E. Failure surfaces as a toast (never silent)
Set the terminal command to a bogus program (e.g. `definitely-not-a-real-terminal {path}`) and
trigger it: a clear **error toast** appears (via the AppError→toast path); the app does not hang or
no-op silently. Then trigger an action on a directory that has since been deleted/renamed: an
`io`-kind "path no longer exists" toast appears.

## Sign-off
- [ ] A (3 actions × 4 entry points)  - [ ] B (per-OS incl. Windows fallback ladder)
- [ ] C (spaces)  - [ ] D (persist across restart + reset)  - [ ] E (error toasts)
