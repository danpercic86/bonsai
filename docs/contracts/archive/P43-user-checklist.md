# P43 — First-run onboarding + empty-state polish — USER CHECKPOINT checklist

Native-only verification. Run `pnpm tauri dev` (the AI harness cannot open the native window,
and — critically — the browser mock's `settings.json` resets per page load, so the **real
cross-restart persistence** of `onboardingSeen` can only be proven here).

Prereqs / setup:
- Build & launch: `pnpm tauri dev`.
- The real prefs file is `settings.json` under the app config dir
  (`%APPDATA%/com.bonsai.app/settings.json` on Windows).
- To re-arm first-run without deleting other prefs: close the app, edit that `settings.json`
  and set `"onboardingSeen": false` (or delete the file to reset everything), then relaunch.
- Do NOT point the app at a real important repo for the identity write step — use a scratch repo
  (e.g. under `D:\Temp\bonsai-scratch`). The identity step writes to your **global** git config;
  see note in (b).

---

## Checklist

### (a) First launch shows onboarding at Welcome
- [ ] With `onboardingSeen:false` (fresh profile, deleted `settings.json`, or hand-set false),
      launch the app.
- [ ] The onboarding overlay appears automatically, centered over the empty state, on the
      **Welcome** step (product name "Bonsai" + value prop + "Get started" + "Skip").
- [ ] Backdrop is visible; graph keyboard shortcuts are suppressed while the overlay is open.
- [ ] Esc, the ✕, and clicking the backdrop each dismiss (Skip) from any step.

### (b) Walk the full flow: Welcome → Open/Clone → Identity → Tour → Finish
- [ ] "Get started" advances to the **Open or clone a repo** step.
- [ ] Open an existing scratch repo (folder picker) OR clone one — the native folder picker /
      clone dialog behave correctly.
- [ ] On a successful open the step **auto-advances** to **Identity**.
- [ ] Identity step, **unset** case: if global `user.name`/`user.email` are not set, two inputs
      appear (prefilled with any value already set). Enter a name + email, Save.
  - [ ] Cross-check on the CLI: `git config --global user.name` and
        `git config --global user.email` now return exactly what you entered.
      (WARNING: this writes your machine-wide identity — use throwaway values or restore your
      own afterward if this is not a test machine.)
- [ ] Identity step, **already-set** case: relaunch onboarding on a profile whose global
      identity is set → the step shows "Identity ready" with name/email greyed, no inputs, and
      Save is not required (no write happens).
- [ ] Next advances to the **Tour** step (static cards: commit graph, AI-assets panel 🤖,
      health dashboard 📊).
- [ ] The **Finish** button closes the overlay.
- [ ] Back is disabled on Welcome and walks backwards correctly on later steps.
- [ ] Skip-tolerance: if you advance past Open/Clone WITHOUT opening a repo, the Identity and
      Tour steps render their informational variant ("Open a repository to finish setup") and the
      overlay can still be closed.

### (c) Persistence across restart (THE key native-only checkpoint)
- [ ] After Finish (or Skip), fully **quit and relaunch** `pnpm tauri dev`.
- [ ] The onboarding overlay does **NOT** reappear.
- [ ] Confirm `settings.json` now contains `"onboardingSeen": true`.
- [ ] (Repeat once dismissing via **Skip** instead of Finish — Skip must persist `true` too.)

### (d) Settings re-trigger
- [ ] Open Settings (gear). Near the top there is a **"Show welcome tour"** button.
- [ ] Clicking it reopens the onboarding overlay at Welcome.
- [ ] Dismissing it again does NOT reset the flag (it was already true) — a later restart still
      does not auto-show onboarding.

### (e) `?onboarding=1` force-show
- [ ] (Dev harness convenience.) With `onboardingSeen:true`, appending `?onboarding=1` to the
      dev URL force-opens the overlay regardless of the flag. Primarily a browser-harness seam;
      confirm it does not throw in the native build if reachable.

### (f) No-repo EmptyState polish
- [ ] Close any open repo so the app is in the no-repo state (dismiss onboarding first).
- [ ] The EmptyState shows the friendlier copy + a short sub-headline / icon, plus the three
      primary actions **Open**, **Clone…**, **New…** and the **recents** list.
- [ ] Each of Open / Clone / New still works (no regression from before P43).
- [ ] Recent-repo entries still open on click.
- [ ] Trigger an error path (e.g. open a non-repo folder) → the error banner still renders; the
      loading state still shows during a slow open.

### (g) Unborn-HEAD (empty repo) friendlier card
- [ ] Create an empty repo: `git init` in a new folder with **no commits**, open it.
- [ ] The workspace/right panel shows the friendlier "no commits yet / first commit" card
      (not the old bare copy).
- [ ] Where the global identity is unset, a **"Set your Git identity"** button appears and
      opens Settings focused on the Identity section (reuses the existing
      `onOpenIdentitySettings` wiring).
- [ ] The first-commit staging path is unchanged: stage a file, commit, and the graph shows the
      first commit.

---

## Notes for the tester restoring state
- To reset onboarding for a re-run: set `"onboardingSeen": false` in `settings.json` (or delete
  the file) while the app is closed.
- If you used your real machine for the identity write in (b), restore your own global
  `user.name`/`user.email` with `git config --global user.name "…"` afterward.
