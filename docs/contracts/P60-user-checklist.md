# P60 — Parity batch (branch rename · non-FF pull · one-click undo · submodule add/deinit/remove) — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, a **real remote** (a throwaway GitHub/GitLab repo you
own) for the non-FF pull, a **real submodule URL + your credential helper** for submodule add, and
human perception of the new dialogs (`NonFfPullDialog`, `UndoDialog`, the submodule Add/Deinit/Remove
dialogs) and of the **canvas graph HEAD-pill refresh** after renaming the current branch. They CANNOT
be self-declared by the orchestrator.

The AI gate proves each **mechanism** hermetically on **git 2.51 (Windows)** with `file://` local
subrepos/remotes (no credentials ⇒ deterministic). It CANNOT prove that a *real* server enforces the
divergence you hit, that *your* credential helper clones a submodule with no prompt, that the graph
pill visibly refreshes, or that the dialog copy reads legibly to a human.

Run via `pnpm tauri dev`. Renames, pulls, resets (undo), and submodule teardown genuinely mutate the
repo, so use a **scratch repo** and a **throwaway remote you own**.

> SAFETY REMINDER: use a scratch repo + a throwaway remote you own. Note each branch tip
> (`git rev-parse HEAD`) before you rename / pull / undo / remove-submodule so you can `git reflog`
> back. **Undo of a merge/rebase/pull is a HARD reset** and **Remove submodule** deletes the
> submodule worktree — both are destructive. NEVER run these against a real shared project.

## Overlay / pixel note (what the headless harness cannot drive)

Like P59, most new P60 surfaces are **DOM overlays** the 0×0 headless browser harness cannot
meaningfully drive or perceive: the `NonFfPullDialog`, the `UndoDialog`, the submodule
Add (url + path) prompt, and the Deinit/Remove confirms. The one **canvas** surface is the graph
**HEAD pill** that must refresh to the new name after renaming the current branch. The AI gate drives
the underlying DATA paths through mock seams:

- **Rename:** the `Rename…` menu item opens the prefilled `PromptDialog`; the mock rejects
  `invalidName` / `branchExists` / `branchNotFound` and preserves `upstream` on success.
- **Non-FF pull:** a diverged branch's `pull()` returns `wouldNotFastForward` carrying `upstream`;
  Merge/Rebase reuse the existing `mergeBranch`/`rebaseBranch` mocks (a rebase-conflict path is
  reachable via the existing conflict seam).
- **Undo:** `?undo=commit` (Commit → mixed), `?undo=merge` (Merge → hard, blocks while dirty),
  `?undo=switch` (branch switch → not undoable + reason), `?undo=none` (empty reflog → nothing to
  undo); default = a `reset` (mixed). Confirm reuses the shipped `resetBranch` mock.
- **Submodules:** the Submodules section `+` opens the Add prompt; a `#fail`-in-name / `?submodule=`
  seam surfaces a `git` error; Deinit flips the row to `uninitialized`, Remove drops it.

## Already proved by the AI gate (do NOT re-verify manually)

- **Rename oracle** (`cargo test -p bonsai-core branches`, 7 new tests + CLI oracle): rename moves the
  ref; **upstream/tracking survives** (`branch.<new>.remote`/`.merge` present); renaming the
  checked-out branch sets `wasHead=true` and HEAD resolves to `refs/heads/<new>`; new-name-exists ⇒
  `BranchExists`; unknown old ⇒ `BranchNotFound`; invalid new ⇒ `InvalidName`; same-name is a no-op.
  State after `rename_branch` matches `git branch -m old new` (`git rev-parse`,
  `git config --get branch.new.remote`).
- **Non-FF pull oracle** (`cargo test -p bonsai-core remote`): the diverged `pull_ff` case now returns
  `upstream` matching `git rev-parse --abbrev-ref @{u}`. **No new command** — the backend still does
  ONLY fetch + fast-forward; Merge/Rebase run exclusively via the already-shipped `merge_branch` /
  `rebase_branch` commands (each already native-checkpointed in M6/its own milestone, with its own
  autostash + conflict UX).
- **Undo oracle** (`cargo test -p bonsai-core undo`, 10 tests): the classifier truth-table (each
  reflog-message prefix ⇒ kind / reset mode / requires-clean-worktree) as pure units; a CLI oracle
  builds a scratch repo doing commit → merge → reset and asserts `describe_last_undo` picks the right
  kind + target for each latest op (`target_oid == git rev-parse HEAD@{1}`). `undo.rs` is READ-ONLY
  (no mutation calls — reviewer-greppable).
- **Submodule oracle** (`cargo test -p bonsai-core submodule`, 9 tests + CLI-oracle roundtrip): the
  `deinit_args` / `rm_args` exact vecs (path is the final token **after `--`**; a space/`;`-bearing
  path stays ONE token — argv-injection test); a superproject with a local (`file://`) submodule where
  `add_submodule` produces a `.gitmodules` + staged gitlink matching `git submodule add`,
  `deinit_submodule` clears `submodule.<n>.url` and empties the worktree while KEEPING `.gitmodules`
  (re-init-able), and `remove_submodule` drops the gitlink + `.gitmodules` entry and deletes the
  worktree — parity with the real `git submodule deinit` / `git rm` sequence.
- **5 new commands** (`rename_branch`, `describe_last_undo`, `add_submodule`, `deinit_submodule`,
  `remove_submodule`) registered; `tsc` / `pnpm build` clean; workspace `cargo test` + `clippy -D
  warnings` clean.

So below is strictly what a **real remote / real submodule URL + credentials / the native canvas +
overlay dialogs** must confirm.

---

## A. Branch rename (P60a)

Entry points: a **`Rename…`** item on a local-branch context menu (sidebar branch row **and** the
graph branch pill), which opens the shared `PromptDialog` prefilled with the current name. For the
**current** (HEAD) branch, `Rename…` is prepended on its **graph HEAD-pill / commit-row** menu.

- [ ] **Rename a non-current branch.** Right-click a non-checked-out local branch (sidebar or its
      graph pill) → **Rename…** → the prompt is prefilled with the current name → change it → the
      sidebar branch list and the branch's graph pill both show the new name; `git branch --list`
      confirms the rename; the branch's tip is unchanged (`git rev-parse`).
      *(AI gate proved the ref move + list update mechanism.)*
- [ ] **Rename the CURRENT (HEAD) branch — HEAD pill refreshes, tracking preserved.** With a tracking
      branch checked out, use the graph **HEAD pill** → **Rename…** → after confirming, the on-canvas
      **HEAD pill shows the new name** (this is the `wasHead` refresh path — the pill must visibly
      update, not go stale). Then verify tracking survived on the CLI:
      `git config --get branch.<new>.remote` and `git config --get branch.<new>.merge` still resolve,
      and `git rev-parse --abbrev-ref @{u}` still names the upstream. *(AI gate proved upstream
      survives + `wasHead=true`; native confirms the canvas pill actually repaints + human perception.)*
- [ ] **new-name-exists error.** Rename a branch to a name that already exists → a clear
      `branchExists` error is surfaced (toast), and nothing changes.
- [ ] **invalid-name error.** Rename to a blank / `-`-leading / illegal ref name → a clear
      `invalidName` error; nothing changes.
- [ ] **same-name is a no-op.** Rename a branch to its current name → no error, nothing changes.

## B. Non-fast-forward pull (P60b)

Set up a scratch repo tracking your throwaway remote, then **diverge** the local branch from its
upstream: make ≥1 local commit AND advance the remote branch (push from a 2nd clone / the web UI) so
the branches diverge. Then **Pull** in Bonsai.

- [ ] **Diverged Pull opens the reconcile dialog.** Pull on the diverged branch → instead of a
      silent FF, the **`NonFfPullDialog`** opens: *"Fast-forward isn't possible"* — *"'<branch>' has
      diverged from '<upstream>' — N local commit(s) / M upstream commit(s)"* — with **Merge** /
      **Rebase** / **Cancel**, each labeled with its effect (Merge = merge commit; Rebase = replay
      your commits on top, rewrites local history). *(AI gate proved `wouldNotFastForward` carries the
      resolved `upstream`; native confirms the real-server divergence triggers it.)*
- [ ] **Merge reconciles (merge commit).** Click **Merge** → the branch reconciles via a merge commit
      (`git log --merges` shows it); the working tree is clean; the graph/status refresh.
- [ ] **Rebase reconciles (rewrites local history).** Re-diverge, Pull → **Rebase** → your local
      commit(s) are **replayed on top** of upstream (new SHAs — `git reflog` shows the rewrite); the
      branch is now fast-forwardable / up to date.
- [ ] **Conflicts route to the existing conflict UX.** Diverge with **conflicting** edits to the same
      lines, Pull → Merge (and separately Rebase) → the operation stops in the **existing conflict
      overlay / op-state banner** (the same UX as a manual merge/rebase), NOT a new/parallel flow.
      Resolve → the operation completes. *(Merge/Rebase reuse the already-shipped commands; this
      confirms the reuse wires through untouched.)*
- [ ] **Cancel is a no-op.** Open the dialog → **Cancel** → nothing changes (branch tip, index, and
      working tree identical; the fetch that already happened is harmless).
- [ ] **FF-able branches still fast-forward silently.** On a branch that is strictly *behind* (no
      local divergence), Pull → it **fast-forwards with no dialog**, exactly as before.

## C. One-click undo (P60c)

Entry point: a prominent **`↶ Undo`** button in the workspace toolbar (title *"Undo the last
operation (commit, merge, rebase, reset…)"*), enabled on a born repo. Clicking it describes the last
HEAD-moving op (read-only) and opens the **`UndoDialog`**; confirming runs the shipped `resetBranch`.

- [ ] **Undo a commit (mixed — worktree kept).** Make a commit → **↶ Undo** → the dialog reads
      **"Undo commit?"** … *"This will **move your branch back to** <short>."* → confirm → the branch
      moves back one commit (`git rev-parse HEAD` == the pre-commit tip) and the committed changes are
      **back in the working tree** (mixed reset — files untouched, staged as modified). *(AI gate
      proved the classifier picks Commit→mixed and the target == `HEAD@{1}`.)*
- [ ] **Undo a merge (hard) — blocked while dirty.** Perform a merge (that creates a merge commit).
      **First** dirty the worktree (edit a tracked file, don't commit) → **↶ Undo** → the dialog reads
      **"Undo merge?"** … *"reset your branch and working tree to <short>"* with the destructive
      styling, and the **Undo button is DISABLED** with *"Commit or stash your changes first."* Now
      commit/stash the change → re-open → **Undo** is enabled → confirm → a **hard reset** returns the
      branch to the pre-merge tip and the working tree matches it. *(AI gate proved Merge→hard +
      `requiresCleanWorktree`; native confirms the button gating + destructive wording legibility.)*
- [ ] **Undo an amend notes the discarded message.** Amend the last commit → **↶ Undo** → the dialog
      (Amend → mixed) shows the note that **the amended commit message is discarded** and its changes
      return to the working tree → confirm → the pre-amend commit is restored.
- [ ] **Branch switch / nothing to undo disables Undo with a reason.** After a `git checkout`
      (branch switch) → **↶ Undo** → the dialog shows the op is **not undoable** with a reason
      (switch back to the previous branch instead), Undo disabled. On a fresh repo with an empty HEAD
      reflog, the Undo button is disabled (nothing to undo).

## D. Submodules — add / deinit / remove (P60d)

Entry points: the sidebar **"Submodules"** section header has a **`+`** ("Add submodule") button →
opens a two-field **Add submodule** prompt (URL + path, path auto-derived from the URL). Each
submodule row's context menu has **Deinitialize…** and **Remove…** (destructive).

- [ ] **Add from a REAL URL (clones the subrepo).** Section **`+`** → **Add submodule** → enter a
      real submodule repository URL (one your credential helper can reach) and a repo-relative path →
      submit → Bonsai **clones** the subrepo at that path, writes `.gitmodules`, and stages the
      gitlink; the new submodule row appears (status up-to-date). Verify on the CLI: `.gitmodules` has
      the entry and `git submodule status` lists it. **No credential prompt inside Bonsai** — creds
      come from your helper. *(AI gate proved the add against a local `file://` subrepo with no creds;
      native confirms a REAL URL + your credential path.)*
- [ ] **Deinit (empties worktree, keeps .gitmodules, re-initable).** Row context menu →
      **Deinitialize…** → confirm ("Deinitialize submodule") → the submodule **worktree empties** and
      `git config --get submodule.<name>.url` is gone, but the **`.gitmodules` entry is retained**
      (`git config -f .gitmodules --get submodule.<name>.url` still resolves). Re-init via the row's
      **Update/Init** repopulates it. *(AI gate proved the deinit teardown + retained `.gitmodules`.)*
- [ ] **Remove (DESTRUCTIVE — drops the submodule).** Row context menu → **Remove…** → the
      **destructive** confirm ("Remove submodule") → confirm → the gitlink + `.gitmodules` entry are
      gone (`git submodule status` no longer lists it) and the submodule worktree is deleted. *(AI
      gate proved the deinit + `git rm` + `.git/modules` teardown sequence.)*
- [ ] **Errors surface.** Attempt an add with an unreachable URL / a bad path (traversal / absolute)
      → a clear `git` / `invalidName` error is surfaced and nothing partial is left behind; the
      console stays clean.

---

## Sign-off

- [ ] A — Rename: non-current branch renames (sidebar + graph pill); the CURRENT branch renames with
      the **canvas HEAD pill refreshing** and **tracking preserved** (`branch.<new>.remote`);
      exists / invalid errors surface; same-name is a no-op.
- [ ] B — Non-FF pull against a **real remote**: the diverged dialog appears; **Merge** (merge commit)
      and **Rebase** (replay/rewrite) both reconcile; conflicts route to the **existing** conflict UX;
      **Cancel** is a no-op; FF-able branches still fast-forward silently.
- [ ] C — Undo: commit → "move your branch back to <short>" (mixed, worktree kept); merge → hard-reset
      wording **blocked while dirty** ("stash first"); amend notes the discarded message;
      branch-switch / empty-reflog disables Undo with the reason.
- [ ] D — Submodules: **Add** from a real URL clones the subrepo (no in-app credential prompt);
      **Deinit** empties the worktree but keeps `.gitmodules` (re-initable); **Remove** (destructive)
      drops the submodule; errors surface.
