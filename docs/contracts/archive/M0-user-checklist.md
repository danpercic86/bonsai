# M0 — USER CHECKPOINT smoke checklist

Run these steps by hand in the native app. All must pass before M0 is declared done.

Setup: from the repo root run `pnpm tauri dev` and wait for the native window.

| # | Step | Expected result | Pass? |
|---|------|-----------------|-------|
| 1 | Look at the window that opens. | Native window titled **"Bonsai"**, roughly 1280x800, dark theme, showing the empty state (app name, tagline, "Open repository" button). | ☐ |
| 2 | Click **Open repository** and select a real Git repository folder (one with at least one commit, e.g. this Bonsai repo). | Native folder picker opens. After selecting, the header shows the repo folder name + full path, and `⎇ <branch> @ <7-char oid>`. The 3-pane shell (Branches / Commit graph / Status placeholders) is visible. | ☐ |
| 3 | Cross-check the HEAD display: in that repo run `git symbolic-ref --short HEAD` and `git rev-parse --short=7 HEAD`. | Branch name and short OID in the header match the CLI output. | ☐ |
| 4 | Click **Open repository** again and select a folder that is NOT a Git repo (e.g. `C:\Windows\Temp` or any plain folder). | Inline error-styled message "Not a Git repository" plus the chosen path; the Open button remains available; app does not crash. | ☐ |
| 5 | Click **Open repository** again and press **Cancel** in the picker. | Nothing changes — no error, no state reset, previously shown repo (if any) still displayed. | ☐ |
| 6 | Optional: select a freshly `git init`-ed folder with no commits. | Header shows `<branch> (no commits yet)` with an `unborn` label pill. | ☐ |

Report any deviation to the orchestrator with the step number and what was seen instead.
