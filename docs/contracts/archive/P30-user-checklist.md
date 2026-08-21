# P30 — Background-Job Scheduler — USER CHECKPOINT checklist

Native-app verification (`pnpm tauri dev`). The orchestrator must NOT self-declare these
passed — every item needs your confirmation. Items marked **SCRATCH** must use a scratch
repo (e.g. a clone under `D:\Temp\bonsai-scratch`), never a real work repo.

Prep: Settings → "Background jobs" → enable **Auto-fetch**, interval **1 minute** (test
speed; restore your preferred value afterwards). Leave **Health refresh** as you find it
unless a step says otherwise.

## 1. Real background fetch via credential helper (network remote)

Repo: any repo whose `origin` is a real network remote (https with Windows Credential
Manager, or ssh with agent). Read-only — background fetch only updates remote-tracking
refs; it never touches your worktree or local branches.

- [ ] Open the repo, enable auto-fetch (1 min), then just leave the app running **10+
      minutes** while you do something else.
- [ ] NO credential prompt ever appears — not once, and no repeated "prompt storm".
- [ ] The toolbar readout next to Fetch shows "Fetched Xm ago" and the X resets roughly
      every interval.
- [ ] If someone/you push a commit to the remote from elsewhere (optional), the graph's
      `origin/...` ref pill moves within ~1–2 intervals without you clicking anything,
      and a quiet "Fetched N refs" info toast appears (only when refs actually updated).

## 2. Offline → backoff → recovery

- [ ] With auto-fetch running against the network remote, disconnect the network
      (unplug / disable Wi-Fi / airplane mode).
- [ ] Wait through at least 4 failed intervals (~4+ min at 1-min interval). Expect
      EXACTLY ONE warning toast ("Auto-fetch failing — backing off") — on the 3rd
      consecutive failure — and silence after that. No error dialog, no prompt.
- [ ] The toolbar readout switches to the paused form ("Auto-fetch paused — retrying in
      Xm") and the retry horizon visibly stretches (backoff: 2×, 4×, up to 8× interval).
- [ ] Reconnect the network. Within one backed-off interval (worst case 8 min at 1-min
      base — or use the Fetch button / run-now to shortcut), fetch succeeds, the readout
      returns to "Fetched Xm ago", and NO extra toast celebrates the recovery.

## 3. Settings persist across restart

- [ ] Set auto-fetch enabled + a non-default interval (e.g. 2 min) and enable Health
      refresh with a non-default interval (e.g. 5 min). Close the app fully; relaunch.
- [ ] Settings panel shows exactly the values you set (both jobs), and the auto-fetch
      readout resumes updating without touching anything.

## 4. All-open-tabs fetch (P11e behavior change)

Behavior change to verify deliberately: auto-fetch now runs for EVERY open repo tab, not
just the active one (the old frontend timer only fetched the active tab).

- [ ] Open two repos with remotes in two tabs. Stay on tab A for 2+ intervals.
- [ ] Switch to tab B: its readout shows a recent "Fetched Xm ago" (it fetched in the
      background while inactive), not "never"/stale-by-the-whole-session.
- [ ] **SCRATCH**: push a commit to tab-B's remote from a second clone while tab A is
      active; switch to B after an interval — the `origin/...` pill already moved.

## 5. Idle CPU sanity

- [ ] With auto-fetch (and optionally health refresh) enabled, leave the app idle and
      watch Task Manager → the Bonsai process(es) for a few minutes.
- [ ] Between fetches CPU sits at ~0% (the 15 s tick is a cheap map scan; no busy loop);
      brief small blips at fetch time are fine. No sustained CPU, no memory creep over
      10 min.

## 6. Suppression during a merge with conflicts

**SCRATCH repo only** — this step creates a real merge conflict.

- [ ] In a scratch repo with a remote and auto-fetch at 1 min, create a conflict:
      commit conflicting edits to the same line on two branches, `git merge` the other
      branch so the repo enters a conflicted-merge state (MERGE_HEAD present).
- [ ] Leave the app on that repo for 2–3 intervals: NO background fetch runs against it
      (readout does not advance to a new "Fetched just now"), no toast, no interference
      with the conflict UI. (Scheduler records "suppressed" quietly.)
- [ ] Resolve/abort the merge; within an interval or two the auto-fetch resumes and the
      readout updates again.

## Sign-off

- [ ] All six sections confirmed in the native app.
- Notes / anomalies:
