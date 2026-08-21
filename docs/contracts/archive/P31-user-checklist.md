# P31 — Per-Worktree AI Contexts — USER CHECKPOINT checklist

Run in the native app (`pnpm tauri dev`) against a **SCRATCH repo only** — never a real repo.

## Fixture setup (once, in a shell)

```powershell
mkdir D:\Temp\bonsai-scratch\p31-check; cd D:\Temp\bonsai-scratch\p31-check
git init -b main main; cd main
git config user.name "Test"; git config user.email "test@example.com"
"# base claude" | Out-File -Encoding utf8 CLAUDE.md
"# base agents" | Out-File -Encoding utf8 AGENTS.md
git add -A; git commit -m init
git worktree add -b feature-a ..\wt-a
git worktree add -b feature-b ..\wt-b
git worktree lock --reason "pinned for QA" ..\wt-b
```

Open `D:\Temp\bonsai-scratch\p31-check\main` in Bonsai. In the AI assets panel,
create two profiles, each targeting CLAUDE.md + AGENTS.md with distinct content:
`opus` ("# opus …") and `haiku` ("# haiku …").

## Checklist

1. **Matrix lists real worktrees correctly.** Open the dialog via the Worktrees
   sidebar menu "AI context…" AND via the "Worktrees" button in the AI assets
   panel header (same dialog). Expect 3 rows: main (marked main/current),
   `wt-a` (branch feature-a), `wt-b` (locked badge). Active profile empty
   everywhere; drift/missing chips present.
2. **Activate different profiles into two worktrees.** Activate `opus` onto the
   main row and `haiku` onto `wt-a`. Each activation MUST show the per-file
   diff preview BEFORE anything is written; confirm both. Then verify via CLI:
   - `type ..\wt-a\CLAUDE.md` → haiku content; `type CLAUDE.md` (main) → opus content.
   - `git -C ..\wt-a status --porcelain` shows ONLY ` M CLAUDE.md` and ` M AGENTS.md`.
   - `git status --porcelain` in main shows the two docs + `?? .bonsai/profiles.json` only.
   - `..\wt-b` files unchanged, `git -C ..\wt-b status` clean.
   - Matrix now shows opus/haiku per row and per-worktree drift chips updated.
3. **Locked worktree blocked with reason.** The `wt-b` row's Activate is
   disabled; the badge/tooltip shows the lock reason ("pinned for QA"). No way
   to preview or activate onto it.
4. **Dirty tracked target blocks, content intact.** Hand-edit
   `..\wt-a\CLAUDE.md` in an editor, commit NOTHING, then try activating `opus`
   onto `wt-a`. Expect a clear error mentioning uncommitted changes to
   CLAUDE.md, nothing written (AGENTS.md not touched either), and your edit
   byte-intact afterward. Restore with `git -C ..\wt-a checkout -- CLAUDE.md`.
5. **Preview always precedes writes; Cancel writes nothing.** Start an
   activation onto `wt-a`, review the diff, press Cancel. Verify via
   `git -C ..\wt-a status` that nothing changed and the matrix still shows the
   old active profile.
6. **Persistence across reopen.** Open `main\.bonsai\profiles.json`: it has
   `"version": 2`, `worktreeActivations` with `"@main"` and `"wt-a"` entries,
   and `activeProfile` mirroring `"@main"`. Close and reopen the repo (or the
   app): the matrix shows the same per-worktree active profiles.

Cleanup: delete `D:\Temp\bonsai-scratch\p31-check` (unlock the worktree first
or use `git worktree remove --force`, then remove the folder).
