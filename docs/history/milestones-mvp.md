# Bonsai — MVP milestones (M0–M6) — archived

> Archived from `CLAUDE.md`. These MVP milestones are **shipped**; the project has since moved
> on to the repo-management roadmap (P24+). Kept for historical reference and to document each
> milestone's AI gate vs USER CHECKPOINT split. The live workflow loop and gate-verification
> rules remain in `CLAUDE.md`.

Milestones were built MVP-first, one at a time, each with a passing gate:

- **M0 — Scaffold.** Tauri v2 + React/Vite/TS project that opens a window via `pnpm tauri dev`
  on Windows; a folder picker (`tauri-plugin-dialog`, capability granted in
  `src-tauri/capabilities/`); Rust detects whether the folder is a Git repo and reads HEAD.
  Toolchain pinned (`rust-toolchain.toml`, `packageManager` in package.json). Architect also
  delivered a one-page UI reference spec (3-pane layout, lane color palette, spacing,
  typography, dark/light) reused by later milestones.
  *AI gate:* `cargo check`, `pnpm build`; browser harness renders; Rust unit test opens a
  fixture repo and reads HEAD. *USER CHECKPOINT:* window opens, folder picker selects a repo,
  HEAD shown.
- **M1 — Working-directory status.** Staged / unstaged / untracked files via git2; auto-refresh
  via notify + manual refresh + refocus rescan.
  *AI gate:* Rust tests vs `git status --porcelain` on scratch repos; harness renders file
  lists from mock data. *USER CHECKPOINT:* auto-refresh + manual refresh + refocus rescan.
- **M2 — Commit graph (centerpiece).** Rust computes `GraphLayout`; React renders the
  GitKraken-style canvas graph, virtualized, with ref pills. Built in four sub-increments:
  M2a layout engine + unit tests; M2b canvas rendering of a static layout; M2c virtualization,
  scrolling, ref pills, HiDPI scaling; M2d perf gate (fixture generator + criterion benchmark).
  *AI gate:* lane/edge unit tests on tricky histories; harness screenshots show lanes, curved
  fork/merge edges, dots, ref pills; scripted generator builds a synthetic 20k+ commit fixture
  (git2 or `git fast-import`); criterion shows layout < 500 ms for 20k commits; harness scroll
  test logs rAF frame timings with no sustained frames > 33 ms. *USER CHECKPOINT:* scrolling the
  20k repo in the native app feels smooth.
- **M3 — Stage / unstage / commit.** *AI gate:* Rust tests vs the `git` CLI on scratch repos.
  *USER CHECKPOINT:* stage/unstage/commit round-trip in the native app.
- **M4 — Diff view.** git2 unified/side-by-side: working-dir diffs (unstaged vs index, staged
  vs HEAD) AND commit diffs (selected node vs first parent, details in the right panel).
  *AI gate:* diff output matches `git diff` / `git show`; harness renders both diff kinds.
  *USER CHECKPOINT:* selecting a commit shows its details + changes in the native app.
- **M5 — Branches.** List, create, checkout, delete; show current branch/HEAD.
  *AI gate:* verified vs the CLI; review confirms destructive ops require UI confirmation.
  *USER CHECKPOINT:* branch operations + confirmation dialog in the native app.
- **M6 — Remotes.** Fetch / pull (fast-forward only) / push with credential handling
  (git2 `CredentialHelper` → Windows Credential Manager, then SSH agent for ssh URLs; never
  prompt for or store raw passwords).
  *AI gate:* fetch/pull/push round-trip against a local bare repo (`git init --bare`,
  `file://` remote) — no network/credentials needed. *USER CHECKPOINT:* one round-trip against a
  real network remote with the credential helper.
- **Polish.** Keyboard shortcuts, error toasts, empty/loading states, GitButler-clean styling.
