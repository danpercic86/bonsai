# Screenshots

The PNGs in this folder are embedded in the root [README.md](../../../README.md)'s
Screenshots section. They're captured against the **mock-IPC browser harness**
(`pnpm dev:mock`, `VITE_MOCK_IPC=1`), not a real repo — the visible commits, branches,
and pull requests all come from the fixture data in `src/ipc/mock/`.

| File | Shows |
| --- | --- |
| `workspace-graph.png` | Default workspace: multi-lane commit graph + working-tree staging (dark theme) |
| `commit-diff.png` | A commit selected in the graph, with one of its changed files opened as a diff |
| `pull-requests.png` | The right panel's Pull requests tab, connected, showing the fixture PR list with CI badges |
| `workspace-light.png` | The same workspace in light theme |

## Regenerating

There's no in-app screenshot button, and the AI cannot see the native Tauri window — so these
are taken by scripting a real browser against the running mock server:

1. Start the harness: `pnpm dev:mock` (serves on `http://localhost:1420`).
2. Drive it with Playwright (already a devDependency) at a 1440×900 viewport,
   `deviceScaleFactor: 2`, following the same flow each screenshot needs:
   - dismiss the onboarding tour (`Skip`),
   - click **Open repository** (the mock's `pickFolder` returns a canned fixture repo path),
   - then interact with the workspace (select a commit, switch the right-panel tab, toggle
     the theme button, etc.) before calling `page.screenshot({ path })`.
3. Save the output PNGs into this folder, overwriting the ones being replaced.
4. Re-check image weight — keep each PNG well under ~1 MB; downscale the viewport or trim to a
   region if a new capture comes in heavy.

A throwaway script is fine for this (it isn't part of the app or its test suite) — write it
outside the repo, or as an untracked file you delete before committing.
