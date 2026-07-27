# M2 — USER CHECKPOINT checklist (commit graph)

The AI gate for M2 has passed (unit tests, adversarial tests, release perf gate
< 500 ms, browser-harness scroll test). This checklist covers the parts only a
human at the native window can verify. Please run through it and report
pass/fail per item.

## Setup

1. A ready-made large test repo already exists on disk — the perf-gate fixture:

   ```
   D:\Repos\Playground\bonsai\src-tauri\target\graph-fixture\repo
   ```

   It is a real Git repository with **31,000 commits** (20k main-line commits,
   ~400 merged feature branches, 3 long-lived branches, 100 tags). If the
   folder is missing (e.g. after `cargo clean`), regenerate it with:

   ```
   cd src-tauri
   cargo test --release --test perf_gate -- --ignored --nocapture
   ```

2. Start the app:

   ```
   pnpm tauri dev
   ```

## Checklist — 31k-commit fixture repo

Open the fixture repo above via the folder picker.

- [ ] **Graph renders**: the center pane shows the commit graph — colored
      lanes, commit dots, curved fork/merge edges where branches split and
      land, and summary / author / relative-date columns.
- [ ] **Ref pills**: a solid `⌂ main` pill on the tip; outline pills for
      `long-0`/`long-1`/`long-2` and `feat-*` branches; yellow `# v...` tag
      pills roughly every 200 commits as you scroll.
- [ ] **Scrolling 31k commits feels smooth** (the actual M2 checkpoint):
      scroll with the wheel, drag the scrollbar thumb top-to-bottom, and fling
      through long stretches. No visible jank/stutter, no blank frames, no
      smearing or leftover pixels, and lane colors stay stable (the same
      branch keeps its color at any scroll position).
- [ ] **Scrollbar jump**: grab the scrollbar and jump straight to the middle
      and to the very bottom — rows render immediately and correctly (root
      commit at the bottom, no missing edges).
- [ ] **Selection highlight**: click a commit row — it gets the selection
      highlight and an accent ring on its dot; clicking empty space below the
      last row clears the selection. Hover highlights the row under the cursor.
- [ ] **HEAD marker**: the checked-out commit's dot has an extra ring.
- [ ] **Resize / zoom**: resize the window and try Ctrl+scroll browser-zoom or
      moving the window to a monitor with different scaling — the canvas stays
      crisp (no blur), columns re-flow.

## Checklist — small real repo (shape sanity)

Open a small real repository you know (e.g. this `bonsai` repo itself, or any
project with a few branches/merges).

- [ ] The graph shape matches `git log --graph --oneline --all` run in that
      repo: same commit order (topological, newest first), forks and merges in
      the right places, merge commits joining two lanes.
- [ ] Branch pills sit on the correct tip commits; the current branch shows
      the solid `⌂` pill; tags (if any) show `# name` pills; remote branches
      show as `origin/...`.
- [ ] Status panel (right) still works alongside the graph: staged/unstaged
      lists update on file changes, manual refresh, and window refocus.

## Reporting

Reply with pass/fail per box; for any failure include the repo used, what you
did, and what you saw (a screenshot helps). Failures go back to the
orchestrator → senior-dev with a reproduction.
