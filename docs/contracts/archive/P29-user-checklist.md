# P29 — Repo-Health Dashboard — USER CHECKPOINT checklist

Run the native app: `pnpm tauri dev`. Open a real repo (your normal working repo).
Contract: `docs/contracts/P29-repo-health.md` §10.2.

## 1. Open the panel
- [ ] A `📊 Health` button appears in the header next to `🤖 AI Assets`.
- [ ] Clicking it opens the overlay; `Esc` and `✕` close it; backdrop click behaves like other overlays.
- [ ] Four sections render in order: **stats → branches → working state → structure**, each with an elapsed-ms caption and a "generated <relative time>" line.

## 2. Numbers are sane (spot-check vs git CLI in that repo)
- [ ] Commit count matches `git rev-list --count HEAD` (or shows `≥` with a "(capped)" chip on a 100k+ repo).
- [ ] Local branch / remote / tag counts match `git branch | wc -l`, `git branch -r | wc -l`, `git tag | wc -l` (i.e. `git for-each-ref`).
- [ ] Current branch + ahead/behind match `git status -sb`.
- [ ] Staged/unstaged/untracked counts match `git status`.
- [ ] Stash count matches `git stash list | wc -l` (0 is fine).
- [ ] Worktree count matches `git worktree list` (includes the main worktree).
- [ ] Largest files list looks plausible (real big files in the repo); sizes formatted KiB/MiB; blob rows read `blob <7-char-oid>`.

## 3. Big-repo responsiveness
- [ ] Open the panel on your **largest** local repo: populated in under ~2 s; the UI stays responsive (no frozen window) while it loads.
- [ ] Per-section skeletons show while loading; sections fill in without layout jumping.

## 4. Refresh behavior
- [ ] The Refresh button re-fetches and updates the "generated" caption.
- [ ] With the panel open, touch a file in the repo (e.g. `echo x >> foo.txt` then undo) → the panel refreshes via the `repo-changed` event (debounced; may take ~a second).
- [ ] With the panel **closed**, no health scan runs in the background (no visible churn/lag on repo changes).

## 5. Read-only sanity (hard invariant)
- [ ] Note `git status` output before opening the panel; open the panel, refresh a few times, close it; `git status`, `git stash list`, and `git branch -vv` output are unchanged. No new files, no index changes.

## 6. Errored-section isolation (informational)
- Not reproducible natively on a healthy repo — a section error only occurs on a broken repo (e.g. corrupted odb). Verified via the browser harness (`-err` repo id shows one errored section beside three healthy ones) and automated tests. **No native action required** unless you happen to have a broken repo; if you do, the panel should show an inline error in that section only, others still populated.

## Sign-off
- [ ] All boxes above checked → P29 USER CHECKPOINT passed.
