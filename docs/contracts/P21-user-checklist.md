# P21 — USER CHECKPOINT checklist (Repo lifecycle: clone + init)

Run these in the **native app** (`pnpm tauri dev`) on Windows. These are exactly the items the AI
gate could NOT self-verify: the autonomous `lifecycle_cli.rs` oracle only exercises the **local
`file://` transport** (which never invokes the credential callback), so **real-network clone over
the Windows credential helper / SSH agent** and the **live progress-bar / new-tab open** flow are
verified here. The mock browser harness animates the progress bar and opens a seeded tab, but only
the native app talks to real git2 + GCM.

Prerequisites:
- Start the app once: from the repo root run **`pnpm tauri dev`** and wait for the Bonsai window.
- Keep a **second terminal** open for the `git` cross-checks below.
- Have a **small public GitHub repo** in mind for step 1 (suggested:
  `https://github.com/octocat/Hello-World.git` — tiny, fast) and, for step 2, a **private** repo you
  can already clone from the command line (i.e. Git Credential Manager or an SSH key/agent is already
  configured for it — do NOT set up new credentials for this test).
- Pick a throwaway parent folder for all clones, e.g. **`D:\Temp\p21-clones`** (create it first:
  `mkdir D:\Temp\p21-clones`). Nothing here is destructive to any existing repo.

> Tip: after each clone/init, cross-check with the paired `git` command in the second terminal. If a
> Bonsai result and the `git` output ever disagree, STOP and report the step number plus both
> outputs to the orchestrator.

---

## 0. Empty-state affordances (no repo open)

1. If a repo is already open, close all tabs so the app shows the **no-repo empty state**. Confirm
   the empty state offers three actions side-by-side: the existing **Browse…/open** affordance plus
   **"Clone repository…"** and **"New repository…"** buttons. (You will also reach these from the tab
   `+` menu once a repo is open — see step 5.)

## 1. Clone a REAL public repo over HTTPS (progress bar → new tab)

2. In the empty state click **"Clone repository…"** (or, if a repo is open, click the tab-strip **`+`**
   and choose **"Clone repository…"**). The **Clone repository** dialog opens with a focused
   **"Repository URL"** field.
3. Type the public URL, e.g. `https://github.com/octocat/Hello-World.git`.
4. Click **"Choose…"** and pick the **parent** folder `D:\Temp\p21-clones`. Confirm the dialog now
   shows a **"Will clone into"** preview reading exactly **`D:\Temp\p21-clones\Hello-World`** — i.e.
   the repo name is derived from the URL (trailing `.git` stripped), appended to the parent you
   picked. (Change the URL to `.../Hello-World` without `.git` and confirm the preview name is
   unchanged; restore the `.git` URL.)
5. Click **"Clone"**. Confirm:
   - The button label changes to **"Cloning…"** and a **determinate progress bar** appears with a
     phase caption that reads **"Receiving objects…"** (with a climbing percentage and a
     `… received` byte readout) and then flips to **"Resolving deltas…"** for the second phase. The
     bar advances to ~100% — it must NOT sit frozen or show an indeterminate spinner.
   - On completion the dialog **closes automatically** and a **new tab opens** focused on the cloned
     repo, showing its **commit graph** populated and the **status panel** (a fresh clone is clean —
     "No changes").
6. Cross-check in the second terminal that this is a real clone, not a copy:
   ```
   git -C D:\Temp\p21-clones\Hello-World log --oneline -3
   git -C D:\Temp\p21-clones\Hello-World remote -v
   git -C D:\Temp\p21-clones\Hello-World branch -r
   ```
   Confirm: the log shows the upstream history (matching what the Bonsai graph shows), `remote -v`
   lists **`origin  https://github.com/octocat/Hello-World.git`** (fetch + push), and `branch -r`
   lists `origin/HEAD` + the remote branches. The checked-out branch matches the graph's HEAD pill.

## 2. Clone a PRIVATE repo via the credential helper (NO in-app password prompt)

7. Open **"Clone repository…"** again. Enter the URL of a **private** repo you can already clone from
   the CLI:
   - **HTTPS** (`https://github.com/<you>/<private>.git`) → auth must come from **Git Credential
     Manager / Windows Credential Manager**.
   - **SSH** (`git@github.com:<you>/<private>.git`) → auth must come from your **SSH agent**.
8. Choose the same parent folder and click **Clone**. Confirm the clone **succeeds with NO in-app
   password/passphrase prompt** — Bonsai never prompts for or stores raw credentials; it delegates to
   the configured helper/agent exactly like the M6 fetch/pull/push path. The progress bar advances
   and a new tab opens on the private repo.
9. Cross-check:
   ```
   git -C D:\Temp\p21-clones\<private> rev-parse HEAD
   git -C D:\Temp\p21-clones\<private> remote get-url origin
   ```
   The HEAD oid and origin URL match what you expect for the private repo.

   > If you have neither a private repo nor a configured helper, substitute a private repo whose
   > credentials are **deliberately absent** from the helper and confirm you get the **auth-failure**
   > message described in step 10 instead — either way, confirm Bonsai never shows its own password
   > box.

## 3. Auth / bad-URL failure shows a clear in-dialog error (no hang, no crash)

10. Open **"Clone repository…"**. Enter a URL guaranteed to fail authentication — e.g. a **private
    repo you do NOT have access to**, or a made-up private path like
    `https://github.com/this-org-does-not-exist-xyz/private-nope.git`. Choose the parent folder, click
    **Clone**. Confirm:
    - After a short attempt, an **inline error message** appears **inside the dialog** (the
      `.dialog-error` line), e.g. an *authentication failed* / *could not read from remote* style
      message. The dialog **stays open** so you can correct the URL and retry.
    - **No new tab** is added, the app does **not hang** on a stuck progress bar, and it does **not**
      crash.
11. (Bad-URL variant) Retry with a transport-level bad URL, e.g.
    `https://nonexistent.invalid/foo/bar.git`. Confirm you get a **network/transport** error message
    inline (again no tab, no hang). Then **Cancel** the dialog.

## 4. Init a brand-new empty repo → new tab → first commit

12. Create an empty folder in the terminal: `mkdir D:\Temp\p21-clones\fresh-init`.
13. In the app choose **"New repository…"** (from the empty state or the tab `+` menu). The **folder
    picker** opens. Select **`D:\Temp\p21-clones\fresh-init`**. Confirm a **new tab opens** on this
    repo in the **empty / unborn-HEAD state**: the **commit graph is empty** and the **status panel is
    usable for a first commit** (HEAD shows an unborn branch, not an error).
14. Cross-check:
    ```
    git -C D:\Temp\p21-clones\fresh-init rev-parse --is-inside-work-tree
    git -C D:\Temp\p21-clones\fresh-init status
    ```
    `--is-inside-work-tree` prints **`true`**; `status` reports **"No commits yet"**.
15. Make a first commit **from Bonsai**: in the terminal create a file
    (`echo hello> D:\Temp\p21-clones\fresh-init\readme.txt`), click Bonsai's **refresh**, **stage**
    `readme.txt`, type a message (e.g. `first commit`) and **Commit**. Confirm the commit succeeds and
    now appears as the first node in the graph. Cross-check:
    ```
    git -C D:\Temp\p21-clones\fresh-init log --oneline
    ```
    shows your `first commit` with a real oid (HEAD is now born).

## 5. Init on an EXISTING repo just opens it (idempotent)

16. Choose **"New repository…"** again and this time pick a folder that is **already a git repo** —
    e.g. `D:\Temp\p21-clones\Hello-World` from step 1 (or the repo you cloned in step 2). Confirm
    Bonsai simply **opens it in a tab** (focusing the existing tab if it is already open) — it does
    **not** error and does **not** reinitialize / wipe history. Cross-check the HEAD is unchanged:
    ```
    git -C D:\Temp\p21-clones\Hello-World rev-parse HEAD
    ```
    matches the oid you saw in step 6.

## 6. Tab `+` menu ordering (with a repo open)

17. With at least one repo open, click the tab-strip **`+`**. Confirm the menu lists **"Browse…"**,
    then **"Clone repository…"**, then **"New repository…"** — all three reachable, each closing the
    menu when chosen.

---

**Sign-off:** every numbered item behaves as described, and every paired `git` verification matches
what Bonsai showed. In particular: a real public HTTPS clone shows the two-phase determinate progress
bar and opens a populated new tab, with `origin`/branches wired (steps 2–6); a private clone
authenticates via the Windows credential helper / SSH agent with **no in-app password prompt**
(steps 7–9); an auth failure and a bad URL each surface a **clear inline error** with the dialog
staying open, no tab, no hang, no crash (steps 10–11); **New repository…** creates an empty/unborn
repo that opens in a tab and accepts a first commit (steps 12–15), and pointing it at an existing repo
just opens it without touching history (step 16); the `+` menu exposes Browse / Clone / New (step 17)
and the no-repo empty state exposes the same Clone/New affordances (step 1). Report any deviation to
the orchestrator with the failing step number and the exact `git` output.
