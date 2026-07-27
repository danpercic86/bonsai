# M1 — USER CHECKPOINT checklist (working-directory status)

Run these in the native app: `pnpm tauri dev` from the repo root. Use a **scratch repo you
create** (e.g. `git init` in a fresh folder, add a commit) or any real repo you don't mind
touching — do NOT use the Bonsai repo itself. Keep a terminal open in that repo for the CLI
steps. Check each box only when you observed the behavior yourself.

- [ ] **Open repo → status shows.** Click the folder picker, select the scratch repo. Header
      shows the branch/HEAD; the right panel shows Staged / Unstaged / Untracked sections
      (or "No changes" if clean).
- [ ] **Watcher auto-update (~1 s).** With the app focused, edit a tracked file in an editor
      (or `echo x >> file.txt` in the terminal) — the panel updates by itself within about
      1 second (300 ms debounce + fetch), with no click needed.
- [ ] **CLI stage reflected.** In the terminal run `git add <file>` — the file moves to the
      Staged section automatically (index change → watcher fires).
- [ ] **Manual refresh button.** Make any change, then click the header refresh button — the
      panel updates; button is disabled while the refresh is in flight, then re-enables.
- [ ] **Refocus rescan.** Alt-tab away from Bonsai, change a file (create/edit/delete) while
      it is unfocused, then click back into the Bonsai window — the panel updates on focus
      (this is the fallback for missed Windows fs events, so verify it even though the
      watcher probably already caught it: try a change under a deep new directory, or simply
      confirm the panel is correct immediately on refocus).
- [ ] **Delete + untracked round-trip.** Delete a tracked file (shows under Unstaged as D),
      create a brand-new file (shows under Untracked), restore them — panel tracks each step.
- [ ] **Bare repo rejected.** `git init --bare` in another fresh folder, open it in Bonsai —
      app stays on the empty state with a "Bare repositories are not supported" banner (plus
      the path); no status panel, no crash.
- [ ] **No console errors.** Open devtools in the Tauri window; none of the steps above
      produced uncaught errors.

If any item fails: note the exact step, the repo state (`git status` output), and any devtools
console message, and report back to the orchestrator.
