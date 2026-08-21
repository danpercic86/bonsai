# P52 — USER CHECKPOINT checklist (native `pnpm tauri dev`)

The AI cannot see the native window or verify real-repo timing. Run these on a real machine.
Prereq: a **large real repo** (thousands of commits, several branches — e.g. a clone of a big
public project) that does NOT already have a commit-graph.

## 1. Commit-graph file appears on open
- [ ] Delete any existing `.git/objects/info/commit-graph` in the test repo.
- [ ] Open the repo in Bonsai.
- [ ] Within a few seconds, `.git/objects/info/commit-graph` **exists** (it is written fire-and-forget
      after open).

## 2. It refreshes after fetch
- [ ] Note the commit-graph file's mtime.
- [ ] Fetch (or let autoFetch run) so new commits arrive.
- [ ] The commit-graph file's mtime **updates** after the fetch that brought new refs.

## 3. Perceived speed (subjective)
- [ ] The commit graph loads and scrolls smoothly over the large history.
- [ ] The repo-health **Branches** section (stale / ahead-behind) completes quickly.
- [ ] Blame / file-history feels responsive.
      (No hard number required — this is the felt-improvement check.)

## 4. Graceful degrade with NO git on PATH (load-bearing)
- [ ] Temporarily remove `git` from PATH (or test on a machine without git).
- [ ] Open a repo, render the graph, view blame, open the health panel.
- [ ] Everything still works (libgit2 needs no commit-graph) and there is **NO error toast / no
      failure** — the write is silently skipped.

## 5. No refresh loop on open
- [ ] Watch the UI right after opening: the commit-graph write must NOT cause a visible
      refresh/reload loop (the file is under `.git/objects/**`, which the watcher filters).

## 6. Nothing else regressed
- [ ] Fetch / pull / push, commit, stage/unstage, branch ops all behave as before.
- [ ] The user's repo Local config is unchanged (Bonsai does not write `core.commitGraph`).
