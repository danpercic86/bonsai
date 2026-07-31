# P22 — USER CHECKPOINT checklist (Tags & remotes management)

Run these in the **native app** (`pnpm tauri dev`) on Windows. These are exactly the items the AI
gate could NOT self-verify: the autonomous `tags_cli.rs` / `remote_mgmt_cli.rs` oracles exercise the
core git2 functions and (for push) only the **local bare / `file://` transport** — which never
invokes the credential callback. So the **real-network tag push over the Windows credential helper /
SSH agent** (step 3B) and the **live UI wiring** — commit-row "Create tag here", the tag/remote
context menus, the two new dialogs (`TagCreateDialog`, `RemoteEditDialog`), the `ConfirmDialog`
gates, and the sidebar Remotes/Tags rendering — are verified here. The mock browser harness fakes
all of this; only the native app talks to real git2 + GCM.

Prerequisites:
- Start the app once: from the repo root run **`pnpm tauri dev`** and wait for the Bonsai window.
- Keep a **second terminal** open for the `git` cross-checks below.
- Prepare a throwaway scratch repo with a couple of commits, e.g.:
  ```
  mkdir D:\Temp\p22-scratch
  cd D:\Temp\p22-scratch
  git init -b main
  git config user.name "Test User"
  git config user.email "test@example.com"
  echo one> a.txt && git add -A && git commit -m "first"
  echo two> b.txt && git add -A && git commit -m "second"
  ```
- Prepare a **local bare remote** for the autonomous push in step 3A (no network needed):
  ```
  git init --bare -b main D:\Temp\p22-origin.git
  ```
- For the real-network push in step 3B (the credential-helper-only part), have a **remote you can
  already push to from the CLI** (Git Credential Manager or an SSH key/agent already configured — do
  NOT set up new credentials for this test). If you have none, do step 3A only and note 3B as
  skipped.
- Open **`D:\Temp\p22-scratch`** in Bonsai (tab `+` → Browse…). Confirm the commit graph shows your
  two commits.

> Tip: after each operation, cross-check with the paired `git` command in the second terminal. If a
> Bonsai result and the `git` output ever disagree, STOP and report the step number plus both
> outputs to the orchestrator.

---

## 1. Create a tag (lightweight AND annotated) from a commit row

1. In the center commit graph, **right-click a commit row** (e.g. the "first" commit). Confirm the
   context menu contains **"Create tag here"** listed next to **"Create branch here"**.
2. **Lightweight tag.** Click **"Create tag here"**. The **Create tag** dialog (`TagCreateDialog`)
   opens with a focused **name** field, an **"Annotated"** checkbox (unchecked by default), and a
   **message** textarea that is hidden/disabled while Annotated is off. Type name `v-light`, leave
   Annotated **off**, click **Create tag**. Confirm:
   - A success toast **"Created tag v-light"** appears.
   - The **left sidebar Tags section** now lists **`v-light`** (expand the Tags section if it is
     collapsed).
   - Cross-check in the terminal:
     ```
     git -C D:\Temp\p22-scratch tag -l v-light
     git -C D:\Temp\p22-scratch cat-file -t v-light
     git -C D:\Temp\p22-scratch rev-parse refs/tags/v-light
     ```
     `tag -l` prints **`v-light`**; `cat-file -t` prints **`commit`** (a lightweight tag points
     straight at the commit); `rev-parse` matches the oid of the commit row you right-clicked.
3. **Annotated tag.** Right-click a commit row again → **"Create tag here"**. Type name `v-annot`,
   **check "Annotated"** (the message textarea becomes enabled), type a message e.g.
   `release notes v-annot`, click **Create tag**. Confirm:
   - Success toast + **`v-annot`** appears in the sidebar Tags section.
   - Cross-check:
     ```
     git -C D:\Temp\p22-scratch tag -l v-annot
     git -C D:\Temp\p22-scratch cat-file -t v-annot
     git -C D:\Temp\p22-scratch for-each-ref --format="%(*objectname) %(contents:subject)" refs/tags/v-annot
     ```
     `cat-file -t` prints **`tag`** (a real annotated tag object, unlike the lightweight one above);
     `for-each-ref` shows the peeled commit oid followed by **`release notes v-annot`**.
4. **Dialog validation.** Right-click a commit → "Create tag here" and try to create a tag named
   **`v-light`** again (a name already in the Tags section). Confirm the dialog **blocks it inline**
   (disabled Create button / validation message) — it does not fire a duplicate create. Also confirm
   a **blank name** and a name **starting with `-`** are rejected, and that with Annotated **on** an
   **empty message** is rejected. **Cancel** the dialog.
5. **Graph tag pill.** Confirm a **tag pill** (`v-light` / `v-annot`) is drawn beside its commit in
   the graph. Right-click the **pill** and confirm it opens the **same tag menu** as the sidebar row
   (Delete tag / Copy tag name / Push tag…).

## 2. Delete a tag via the tag menu + ConfirmDialog

6. In the sidebar Tags section (or on the graph pill), **right-click `v-light`**. Confirm the menu
   shows **Delete tag**, **Copy tag name**, and one **"Push tag to <remote>"** item per configured
   remote (see step 4-setup below; if no remote is configured yet, no push item appears).
7. Click **Delete tag**. A **ConfirmDialog** titled **"Delete tag"** opens with the note that it
   deletes the **local** tag only (a tag already pushed to a remote is not removed there) and a
   **"Delete tag"** confirm button. Click **Delete tag**. Confirm:
   - Success toast; **`v-light`** disappears from the Tags section and its graph pill is gone.
   - Cross-check:
     ```
     git -C D:\Temp\p22-scratch tag -l
     ```
     lists **`v-annot`** but **not** `v-light`.
8. (Optional) **Copy tag name** on `v-annot` → paste elsewhere and confirm the clipboard holds
   `v-annot`.

## 3. Push a tag to a remote

> Part **3A** uses the local bare remote and needs no network — do it first. Part **3B** is the
> credential-helper-only USER-CHECKPOINT proper.

### 3A — Push to the local bare remote (no credentials)

9. First wire the bare remote in-app (this also seeds steps 4–7). Right-click in the sidebar
   **Remotes** section header → **Add remote** (`+`), or use the Add affordance, and add
   name **`origin`**, URL **`D:\Temp\p22-origin.git`** (see step 11 for the full Add flow). Then push
   `main` once from the terminal so the bare repo is a real repo (tags need the commit present):
   ```
   git -C D:\Temp\p22-scratch push origin main
   ```
10. In the sidebar Tags section, **right-click `v-annot`** → **"Push tag to origin"**. Confirm a
    success toast e.g. **"Pushed tag v-annot → origin"**. Cross-check:
    ```
    git ls-remote --tags D:\Temp\p22-origin.git
    git --git-dir=D:\Temp\p22-origin.git cat-file -t refs/tags/v-annot
    ```
    `ls-remote --tags` lists **`refs/tags/v-annot`**; `cat-file -t` prints **`tag`** (the annotated
    tag object transferred, not just a ref).

### 3B — Push to a REAL network remote via the credential helper (no in-app password prompt)

11. Add your real network remote in-app (name e.g. `github`, an HTTPS or SSH URL you can already push
    to). Create a fresh tag on a commit (step 1), then **right-click the tag → "Push tag to github"**.
    Confirm the push **succeeds with NO in-app password/passphrase prompt** — Bonsai delegates to Git
    Credential Manager / the SSH agent exactly like the M6 push path. Cross-check:
    ```
    git ls-remote --tags <your-remote-url>
    ```
    shows the pushed tag. Then point a tag push at a **bogus** remote URL and confirm you get a clear
    **networkError** toast (no hang, no crash). *(Note: deleting a tag on the remote is intentionally
    NOT offered in v1 — §OPEN-3.)*

## 4. Add a remote

12. In the sidebar **Remotes** section, click the header **Add remote** (`+`) button. The
    **`RemoteEditDialog`** opens with **name** and **URL** fields (both empty). Type name **`backup`**,
    URL **`https://example.com/backup.git`**, click the confirm button. Confirm:
    - Success toast; a new **configured-remote row `backup`** (with a `☁` glyph and the URL as its
      `title` tooltip) appears **at the top of the Remotes section**, above the remote-tracking-branch
      tree.
    - Cross-check:
      ```
      git -C D:\Temp\p22-scratch remote -v
      ```
      lists **`backup  https://example.com/backup.git`** (fetch + push).
13. **Add validation.** Open Add remote again and try name **`backup`** (already present) → confirm
    the dialog blocks it inline; try a name with a **space** (`bad name`) → rejected; try an **empty
    URL** → rejected. **Cancel**.
14. **Remotes section layout.** Confirm the Remotes section lists the **configured remotes**
    (`origin`, `backup`) as rows at the **top**, with the **remote-tracking-branch tree** (e.g.
    `origin/main` once fetched) rendered **below**. With at least one remote configured the section is
    not the "No remotes" empty state.

## 5. Rename a remote (name + tracking refs move)

15. First give `origin` a tracking ref so the move is observable — fetch it once from the terminal:
    ```
    git -C D:\Temp\p22-scratch fetch origin
    git -C D:\Temp\p22-scratch branch -r
    ```
    `branch -r` should list **`origin/main`**. Refresh Bonsai (refresh button) so the tracking row
    shows.
16. In the sidebar, **right-click the `origin` remote row** → **Rename…**. A **PromptDialog** opens
    labeled **"New remote name"** pre-filled with `origin`. Change it to **`upstream`** and confirm.
    Then refresh Bonsai. Confirm:
    - The configured-remote row now reads **`upstream`**; the tracking row now reads **`upstream/main`**.
    - Cross-check:
      ```
      git -C D:\Temp\p22-scratch remote -v
      git -C D:\Temp\p22-scratch branch -r
      ```
      `remote -v` shows **`upstream`** (no `origin`); `branch -r` shows **`upstream/main`** (the
      tracking ref moved out of `origin/*`).
17. **Rename validation.** Try renaming `backup` to **`upstream`** (already exists) → confirm the
    error toast or inline block; the rename does not happen.

## 6. Edit a remote's URL

18. **Right-click the `upstream` remote row** → **Edit URL…**. A **PromptDialog** opens labeled
    **"Fetch URL"** pre-filled with the current URL. Change it to
    **`D:\Temp\p22-origin.git`** (or another valid URL) and confirm. Cross-check:
    ```
    git -C D:\Temp\p22-scratch remote get-url upstream
    ```
    prints the **new** URL. Confirm the row's `title` tooltip (hover) also shows the new URL.

## 7. Remove a remote

19. **Right-click the `backup` remote row** → **Remove…**. A **ConfirmDialog** titled
    **"Remove remote"** opens noting it removes the remote and its remote-tracking branches from this
    repo (the server is not affected), with a **"Remove remote"** confirm button. Confirm. Cross-check:
    ```
    git -C D:\Temp\p22-scratch remote -v
    ```
    no longer lists **`backup`**.
20. Now **remove `upstream`** the same way and confirm both that it is gone from `git remote -v` and
    that its tracking refs are cleaned up:
    ```
    git -C D:\Temp\p22-scratch remote -v
    git -C D:\Temp\p22-scratch branch -r
    ```
    `remote -v` is empty (or lists only remotes you did not remove); `branch -r` no longer lists
    `upstream/main`. With no remotes left, confirm the Remotes section shows the **"No remotes"**
    empty state.

---

**Sign-off:** every numbered item behaves as described, and every paired `git` verification matches
what Bonsai showed. In particular: **Create tag here** on a commit row creates a **lightweight**
(`cat-file -t` → `commit`) and an **annotated** (`cat-file -t` → `tag`, message preserved) tag that
appear in the sidebar Tags section and as graph pills (steps 1–5); **Delete tag** via the tag menu +
ConfirmDialog removes it locally (`git tag -l`) (steps 6–8); **Push tag** transfers the tag to the
bare remote autonomously and to a **real network remote via the credential helper with no in-app
password prompt** (`git ls-remote --tags`) (steps 9–11); **Add / Rename / Edit URL / Remove** a
remote each match `git remote -v` / `git remote get-url` / `git branch -r`, with rename moving the
tracking refs and remove cleaning them up (steps 12–20); and the Remotes section lists configured
remotes at the top with the tracking tree below, falling back to "No remotes" when empty. Report any
deviation to the orchestrator with the failing step number and the exact `git` output.
