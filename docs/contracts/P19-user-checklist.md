# P19 — USER CHECKPOINT checklist (Submodule support: list / init / update / sync / open-in-tab)

Run these in the **native app** (`pnpm tauri dev`) against a **scratch superproject with a real
submodule** — NOT a real repository you care about. These items are exactly what the AI gate could
NOT self-verify: the native Submodules sidebar section driven by real git2 status, the init/update/
sync operations mutating a real worktree (cross-checked with `git submodule status`), "Open in new
tab" wiring the submodule into the multi-repo tab flow, and — the part the autonomous `file://`
tests could **not** cover — the **credential path** on Update against a private remote (Windows
credential helper / SSH agent, no in-app prompt).

Keep a **second terminal** open in the superproject for the `git` verifications below.

---

## 0. Prepare a scratch superproject + submodule (throwaway — safe to delete)

You need a submodule whose remote is a **local bare repo** (no network) for steps 1–5, and — for
step 6 — a submodule pointing at a **private remote** you have credentials for (GitHub/Azure/etc.).

In a terminal, in a throwaway folder (e.g. `D:\Temp\p19-scratch`):

```
cd /d D:\Temp\p19-scratch

REM --- a local bare "remote" for the submodule, with two commits A then B ---
git init --bare -b main sub-origin.git
git clone sub-origin.git sub-seed
cd sub-seed
git config user.name "P19 Tester" && git config user.email "p19@example.com"
git config core.autocrlf false
echo A> mod.txt && git add -A && git commit -m "submodule commit A"
echo B> mod.txt && git add -A && git commit -m "submodule commit B"
git push -u origin main
cd ..

REM --- the superproject, pinned at submodule tip B ---
git init -b main super
cd super
git config user.name "P19 Tester" && git config user.email "p19@example.com"
git config core.autocrlf false
echo super> README.md && git add -A && git commit -m "superproject initial"
git -c protocol.file.allow=always submodule add ..\sub-origin.git sub
git commit -am "add submodule sub"
cd ..

REM --- a FRESH clone that does NOT recurse the submodule => "sub" is uninitialized ---
git clone super work
```

Open **`D:\Temp\p19-scratch\work`** in Bonsai (this is the repo whose `sub` starts uninitialized).

> Note: `-c protocol.file.allow=always` is only needed for the local-path submodule fixture; a real
> `https://`/`ssh://` submodule (step 6) does not need it.

---

## 1. Submodules sidebar section lists real submodules with correct status badges

1. In the left sidebar, find a **Submodules** section (below **Stashes**). It must list one row:
   `sub`.
2. Because `work` was cloned without recursing, the badge on `sub` must read **not initialized**
   (muted/neutral).
3. Cross-check in the terminal — the leading sigil is `-` for uninitialized:
   ```
   git -C D:\Temp\p19-scratch\work submodule status
   ```
   Confirm the line starts with `-` and the badge matches.
4. Hover the row: the tooltip (`title`) shows the submodule path (`sub`).
5. (Empty-state sanity) Open any repo **without** submodules (e.g. the `super`… no, that has one —
   use any other scratch repo). Confirm the Submodules section shows **No submodules** or is
   collapsed/hidden, and never shows a phantom row.

## 2. Init on an uninitialized submodule checks in its config (no checkout yet)

6. Right-click the `sub` row. A context menu appears with **Init / Update / Sync / Open in new
   tab**. Confirm on this uninitialized row: **Init is enabled**, **Open in new tab is disabled**
   (no worktree to open yet).
7. Click **Init**. A success toast appears (e.g. "Initialized sub").
8. Verify `.git/config` now carries the submodule (init registers config; it does NOT check out
   files):
   ```
   git -C D:\Temp\p19-scratch\work config submodule.sub.url
   ```
   Confirm it prints the `sub-origin.git` path. `git submodule status` may still show `-` (not yet
   checked out) — the badge may still read **not initialized** until you Update. This is expected:
   Init alone does not fetch/checkout.

## 3. Update brings an out-of-sync submodule to the pinned commit (cross-check before/after)

9. Right-click `sub` → **Update**. This init-if-needed + fetches (over the local `sub-origin.git`)
   + checks out the pinned commit **B**. Success toast appears.
10. The badge on `sub` must flip to **up to date** (green).
11. Cross-check with the CLI — record the submodule's HEAD and status **before vs after**:
    ```
    git -C D:\Temp\p19-scratch\work submodule status
    git -C D:\Temp\p19-scratch\work\sub rev-parse HEAD
    ```
    Confirm the `submodule status` line now starts with a **space** (up-to-date) and the `sub`
    HEAD equals the pinned commit B (the same oid `git -C ...\super ls-tree HEAD sub` records).
12. **Out-of-sync round-trip.** In the terminal, detach the submodule onto the *older* commit A:
    ```
    cd D:\Temp\p19-scratch\work\sub
    git checkout HEAD~1
    cd ..\..
    ```
    Back in Bonsai, click the manual **refresh** (or refocus the window). The `sub` badge must now
    read **out of sync** (amber), and `git -C work submodule status` starts with `+`.
13. Right-click `sub` → **Update** again. Badge returns to **up to date**; `git -C work submodule
    status` starts with a space again and `sub` HEAD is back at B. Confirm the before/after `git`
    outputs match the badge transitions exactly.

## 4. Sync after changing a submodule URL in `.gitmodules`

14. Make a second local bare repo to point at, then rewrite the tracked `.gitmodules` URL:
    ```
    cd /d D:\Temp\p19-scratch
    git clone --bare sub-origin.git sub-origin2.git
    cd work
    git config -f .gitmodules submodule.sub.url D:/Temp/p19-scratch/sub-origin2.git
    ```
    Confirm `.git/config` still has the **old** URL (Sync has not run yet):
    ```
    git -C D:\Temp\p19-scratch\work config submodule.sub.url
    ```
    It should still print the original `sub-origin.git`.
15. In Bonsai, right-click `sub` → **Sync**. Success toast (e.g. "Synced URL for sub").
16. Verify Sync copied the `.gitmodules` URL into `.git/config`:
    ```
    git -C D:\Temp\p19-scratch\work config submodule.sub.url
    ```
    It must now print the **new** `sub-origin2.git` path. (Sync changes config only — no worktree
    change, no badge change.)

## 5. "Open in new tab" opens the submodule as its own repo tab

17. Ensure `sub` is initialized/updated (badge **up to date** or **out of sync**, i.e. it has a
    worktree). Right-click `sub` → confirm **Open in new tab** is now **enabled**.
18. Click **Open in new tab**. A new repo tab opens for the submodule (`...\work\sub`), focused.
19. In that new tab, confirm the submodule loads as an ordinary repo: its **commit graph** renders
    (you should see submodule commit B, and A behind it), the **status** panel loads, and the
    branch list shows the submodule's branch. Switching back to the `work` tab still shows the
    superproject.

## 6. Credential path on Update against a PRIVATE remote (native-only — NOT harness/file:// coverable)

This is the item the autonomous tests could not cover: the local `file://` transport never invokes
the credentials callback. Here you exercise the real M6 credential chain (Windows Credential Manager
helper first, then SSH agent for `ssh://`), which must **never** pop an in-app password prompt.

Set up a superproject whose submodule points at a **private** remote you can already fetch from on
this machine (i.e. `git fetch` in a normal clone of that remote succeeds via the credential
helper / SSH agent):

```
cd /d D:\Temp\p19-scratch
git init -b main super-priv
cd super-priv
git config user.name "P19 Tester" && git config user.email "p19@example.com"
echo x> README.md && git add -A && git commit -m init
REM HTTPS example (Windows Credential Manager) OR ssh:// for SSH-agent:
git submodule add https://your.host/private/repo.git priv
git commit -am "add private submodule"
cd ..
git clone super-priv work-priv
```

20. Open `D:\Temp\p19-scratch\work-priv` in Bonsai. The `priv` row shows **not initialized**.
21. Right-click `priv` → **Update**. It fetches from the private remote using the configured
    credential helper / SSH agent.
    - Confirm **no in-app password/credential dialog** appears (Bonsai never prompts for or stores
      raw passwords — this is the locked v1 rule).
    - On success the badge flips to **up to date** and a toast appears.
22. Cross-check the checkout matches the pin:
    ```
    git -C D:\Temp\p19-scratch\work-priv submodule status
    ```
    The `priv` line starts with a space and the oid matches the superproject's recorded pointer.
23. **Auth-failure surface (optional).** If you have a remote you are NOT authorized for, point a
    submodule at it and Update: Bonsai must show a clear **error toast** (auth failed / network),
    change nothing, and still never prompt for a raw password.

---

**Sign-off:** every numbered item behaves as described, and every `git submodule status` /
`git config submodule.<name>.url` / `rev-parse` check matches the badge and toast Bonsai showed.
In particular: badges track `-`/` `/`+` sigils (steps 1, 3, 12–13), Sync rewrites `.git/config`
(step 16), Open-in-tab loads the submodule as its own repo (steps 18–19), and Update over a private
remote uses the OS credential helper with **no in-app prompt** (step 21). Report any deviation to
the orchestrator with the failing step number and the exact `git` output.
