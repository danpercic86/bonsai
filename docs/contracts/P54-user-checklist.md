# P54 — Commit composer (WIP → N logical commits) — USER CHECKPOINT checklist (native-only)

These items require the native Tauri window, a **real `claude` CLI**, a **real dirty repo**, and human
judgement of AI grouping quality — they CANNOT be self-declared by the orchestrator. The AI gate only
proves the normalizer/partition logic (unit tests), the atomic apply engine with ref+index rollback and
an untouched working tree (unit + `git diff-tree` oracle), the pure review-UI reducers (vitest), and the
**mock-driven** UI wiring (browser harness with canned `composeHandlers`). Run via `pnpm tauri dev`
against a **REAL repo** with the real, authenticated `claude` binary on PATH and **AI consent enabled**
in Settings.

> **Backups first.** These steps CREATE COMMITS on the current branch. Use a scratch repo or a throwaway
> branch. The composer never touches the working tree, but it does move HEAD/branch refs.

## Already proved by the AI gate (do NOT re-verify manually)
- **Propose (unit):** `parse_compose_response` is ALWAYS an apply-able partition of the real change set —
  full coverage + disjoint groups, overlap → first-wins + note, unknown path → dropped + note, empty
  group → dropped, > `MAX_COMPOSE_GROUPS` → tail folded into `unassigned` + note, unparseable model
  output → `groups:[]` + all-unassigned + note (never a hard error); grounding shape carries
  `CHANGED FILES (...)` + the exact paths + `===== FILE:` per-file diff blocks; clean tree →
  `NothingToCommit` BEFORE any CLI spawn; consent gate → `AiUnavailable` before repo access;
  `ComposeGroup`/`ComposeProposal`/`ComposePlan`/`ComposeCommit` camelCase wire shapes; `costUsd`
  `None`→`null`; all prompts single-line.
- **Apply (unit + `git diff-tree` oracle):** 2 groups → 2 commits whose per-commit delta-to-parent is
  EXACTLY each group's files (oracle agrees); uncovered files left uncommitted; ALL validation failures
  (empty message, empty file list, duplicate path across groups, path not in the change set, empty plan,
  unset identity) reject BEFORE any mutation with HEAD unchanged; mid-sequence failure rolls back on a
  **branch HEAD**, a **detached HEAD**, and an **unborn HEAD** (HEAD + index restored, working tree
  byte-identical, zero commits landed); a successful apply never touches working-tree bytes.
- **Reducers (vitest):** move / move-to-and-from-unassigned / drop / merge / edit / add all preserve the
  file-level PARTITION INVARIANT (each changed file in exactly one place, no dup, no loss); `planIsApplicable`
  is false when any group has an empty/whitespace message or zero files, true otherwise.
- **Browser harness (`VITE_MOCK_IPC=1`, mock data):** the "Compose commits ✨" entry is gated on
  `aiEligible && workingDirty`; the dialog opens with ≥1 group; reassign / edit / drop / merge work;
  an empty-message group disables "Create N commits"; apply adds N rows to the graph, committed files
  leave the status list, unassigned files remain with the "left uncommitted" note; `?ai=off` shows the
  error banner; a `#fail` message drives the rollback path and leaves the mock status UNCHANGED; Esc
  closes preview-then-dialog and nav keys are inert while open.

So the native checkpoint is about **real-model grouping quality, real per-commit diffs on real git state,
real detached/unborn-HEAD behavior, real CLI-missing/consent states, and privacy** — not whether the
buttons and reducers work.

## A. Entry gating — "Compose commits ✨" (real repo)
- [ ] With a **clean** working tree, the "Compose commits ✨" button (in the working-changes panel) is
      **disabled**.
- [ ] Make the tree **dirty** (edit ≥3 files across ≥2 concerns — e.g. a code change, a test, a doc). With
      AI consent ON and `claude` on PATH, the button becomes **enabled**.
- [ ] Turn AI consent OFF (or remove `claude` from PATH): the button is **disabled** (or opening it surfaces
      a clear `aiUnavailable`/`aiFailed` message), and nothing is committed.

## B. Propose quality — coherent, intent-focused groups (real model)
- [ ] Click "Compose commits ✨". After a short spinner the dialog shows **1–N groups**, each with a set of
      files and a generated message.
- [ ] The grouping is **coherent**: files that belong to the same logical change land together (e.g. a
      feature and its test may split or pair sensibly), unrelated concerns are separated. Judge on a tree
      you deliberately made span ≥2 concerns.
- [ ] Each message is **intent-focused** (imperative summary of the WHY, Conventional-Commits style), **not**
      a mechanical restatement of the diff ("changed lines in foo.rs"). Pick a change whose purpose isn't
      obvious from the file names and confirm the stated intent is right.
- [ ] Every changed file is accounted for: it is either in a group or in the **Unassigned** bucket (the
      "N file(s) will be left uncommitted" note appears when the bucket is non-empty). Nothing is silently
      dropped, and no path you didn't change appears.
- [ ] (Optional) Re-run with a **guidance hint** (if surfaced) like "keep tests in their own commit" and
      confirm the grouping shifts accordingly.

## C. Review editing — reassign / edit / drop / merge (real state)
- [ ] **Reassign** a file: use a row's "Move to…" to move it to another group and to **Unassigned**, then
      back. The file always shows in exactly one place.
- [ ] **Edit** a group message (summary + optional body). Clearing a message to blank **disables**
      "Create N commits"; restoring it re-enables.
- [ ] **Drop** a group: its files fall back to **Unassigned** (not lost).
- [ ] **Merge** a group into the next: the two file lists combine and the two messages join (blank line
      between); the group count drops by one; oldest-first order is preserved.
- [ ] **+ New group**: add an empty group, move a couple of files into it, give it a message.

## D. Apply — exactly N commits, per-commit diffs match the groups IN ORDER
- [ ] Click **"Create N commits"**. A success toast reports the count; the dialog closes; the graph gains
      **exactly N** new rows on top of the previous HEAD.
- [ ] For **each** created commit, open its diff (select the graph node): its changes are **exactly** that
      group's files — no more, no fewer — and the commits are in **plan order** (first group = oldest of the
      new commits, on the current branch). Cross-check with `git log --oneline -n N` and
      `git show --stat <oid>` in a terminal.
- [ ] Committed files **leave** the working-changes/status list.
- [ ] **Unborn HEAD (first commits):** in a freshly `git init`'d repo (no commits yet) with a dirty tree,
      composing produces the repo's **root commit** as the first group and the rest as its descendants; the
      branch now points at the newest.

## E. Unassigned files remain uncommitted
- [ ] Leave at least one changed file in the **Unassigned** bucket before applying. After apply, that file
      is **still present** in the working-changes list (uncommitted, unstaged) — the composer committed only
      the grouped files.
- [ ] **Index takeover (D5):** if you had files **staged** in the normal commit panel before opening the
      composer, confirm that after a cancel or a successful apply the staging reflects the composer's view
      (the composer resets the index to HEAD as step 1) and **no working-tree content was lost** — only the
      staged/unstaged split changed. The review UI states the composer manages staging.

## F. Atomic failure — nothing committed, HEAD + working tree unchanged
- [ ] Induce a mid-sequence failure. Easiest: after proposing, **edit the working tree so one group's files
      now match HEAD** (revert that group's changes on disk) but leave the group in the plan — that group
      nets to no change and apply must fail. (Or reproduce however is convenient.)
- [ ] The apply **fails with a clear error** naming the offending group (e.g. "group 2: …refresh the
      composer"), and:
  - [ ] `git log` shows **zero** new commits (HEAD is exactly where it was — on a branch, a **detached
        HEAD**, or unborn),
  - [ ] the **working tree is byte-identical** to before the apply (nothing on disk changed),
  - [ ] `git status` / the panel shows the same changes as before (the plan can be edited and retried).
- [ ] Repeat once from a **detached HEAD** (`git checkout <sha>` first) to confirm the failure leaves you on
      the same detached commit, not re-attached to a branch.

## G. Preview caveat — partially-staged files (reviewer NIT 1)
- [ ] Open a row's **"Preview"** on a file that has changes. For a file that is **fully** unstaged or fully
      untracked, the preview matches the change the commit will include.
- [ ] **Known limitation:** for a **partially-staged** file (some hunks staged, some not), the row Preview
      shows the **workdir-vs-index** diff (the unstaged portion), NOT the full **HEAD→workdir** change — while
      the actual commit (apply stages the WHOLE file) includes both the staged and unstaged parts. Confirm
      this discrepancy is acceptable for v1 (the composer commits whole files; the preview under-shows for
      partially-staged files). Flag if it is confusing enough to warrant a follow-up.

## H. Privacy — no code leaves the device except via your local CLI
- [ ] The ONLY egress path is the local `claude` CLI you already authenticated: with `claude` off PATH the
      compose entry is disabled/errors — Bonsai has **no built-in or remote AI fallback** for grouping.
- [ ] Apply is **pure git** and **not** AI-gated: once a plan is reviewed, "Create N commits" works even with
      AI toggled OFF afterwards — no CLI is spawned for apply, and no network egress occurs.
- [ ] (Optional, with a process/network monitor) A propose spawns a local `claude` child process and passes
      grounding via stdin/argv; Bonsai itself opens **no** network connection to any AI endpoint. Egress is
      identical to you running `claude` yourself.

## Sign-off
- [ ] A (entry gating)  - [ ] B (propose quality: coherent + intent-focused)
- [ ] C (reassign/edit/drop/merge)  - [ ] D (exactly N commits, per-commit diffs match in order + unborn HEAD)
- [ ] E (unassigned remain uncommitted + index takeover)  - [ ] F (atomic failure: nothing committed, HEAD + tree intact, incl. detached HEAD)
- [ ] G (partial-staged preview caveat)  - [ ] H (privacy: local CLI only, apply not AI-gated)
