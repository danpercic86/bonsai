# P20 — Daily essentials: amend, cherry-pick, revert, reset, discard

Add the five everyday history/worktree operations that Bonsai is still missing. Rust owns all
git2 logic; React only renders and confirms destructive actions. **Cherry-pick and revert REUSE
the existing pause/continue/abort framework** (`opstate.rs` detection, `conflict.rs` resolution,
the `OpBanner` UI) — this contract specifies reuse, not reinvention.

Two `senior-dev` sub-increments (see §11):
- **P20a** — amend + reset + discard (no conflict machinery, no `OpBanner` change).
- **P20b** — cherry-pick + revert (conflict outcome enums + `OpBanner`/`opstate`/`conflict.rs`
  reuse).

---

## 0. Invariants held

- git2 only; no network. All heavy calls run under the existing `spawn_blocking` wrapper in each
  `commands.rs` inner fn (same template as `merge_branch_inner` / `create_stash`).
- Cores stay Tauri-free / runtime-free → directly unit-testable (no "test" Tauri feature), same
  rule as `merge.rs` / `rebase.rs` / `stash.rs`.
- Every command carries `repoId` first (resolved via `repo_path(state, repo_id)`), mirrors the
  existing surface, and does **NOT** emit `repo-changed` — the frontend refetches imperatively
  after every successful mutation (the watcher fires too and is absorbed by request-id guards).
- **No `error.rs` change and no new `AppError` kind** — every failure maps to an existing variant
  (§9). The TS `AppError` union in `types.ts` is unchanged.
- **No `opstate.rs` change.** `read_op_state` ALREADY returns `RepoOpState::CherryPick` /
  `RepoOpState::Revert` (opstate.rs:124-125) and the TS `RepoOpState` union already has
  `{kind:'cherryPick'}` / `{kind:'revert'}` (types.ts:247-248). Cherry-pick/revert plug into the
  EXISTING detection with zero wire change.
- Destructive operations (reset hard, discard, cherry-pick/revert abort) require **explicit UI
  confirmation** (ConfirmDialog) per the repo guardrails.

---

## OPEN DECISIONS (recommended default in brackets; contract proceeds on the default)

1. **Amend push-guard: backend signal or frontend-derived?** [**Frontend-derived** — no backend
   change.] The frontend already has `BranchesSnapshot`; the current branch's tip is "already
   pushed" when `upstream !== null && ahead === 0` (the tip is contained in the upstream). The
   Amend affordance shows a warning note in that case. No backend field is added.
2. **Amend message prefill source.** [**Reuse `getCommitDiff(repoId, head.oid).details.message`** —
   no new getter.] `CommitDetails.message` is the full HEAD message; `head.oid` comes from
   `HeadInfo`. Adding a dedicated `get_commit_message` command is the rejected alternative.
3. **Discard target — restore to INDEX or to HEAD?** [**Restore worktree to the INDEX version**
   (`git checkout -- <path>`).] The affordance lives on the *Changes* (unstaged) rows, so
   discarding unstaged edits back to the index is the least-surprising meaning; staged content is
   untouched. For a file with no staged content, index == HEAD, so this also restores to HEAD.
   Untracked files are **out of scope** (deleting them is a separate future confirm) — the UI only
   offers Discard on tracked Changes rows; the backend errors on an untracked path (defensive).
4. **Cherry-pick/revert: autostash dirty worktree like merge?** [**No** — require a clean index
   (index == HEAD), allow unstaged, fail as `checkoutConflict` if the checkout would clobber, same
   as `rebase_branch`.] Autostash parity is a later polish item.
5. **Cherry-pick/revert HEAD requirement.** [**Attached born HEAD only**, like merge/rebase.]
   Detached-HEAD cherry-pick is a later item; a detached/unborn HEAD errors before mutation.
6. **Empty pick/revert (result tree == HEAD tree).** [**`nothingToCommit`** after cleaning up the
   sequencer state], matching git's default refusal. No `--allow-empty` in v1.
7. **Cherry-pick/revert continue — editable message?** [**No** — `Continue` in `OpBanner` commits
   directly, reusing the original message/authorship. The `CommitBox` stays `blocked` for these
   ops (it already is, CommitBox.tsx:19-20).] Editing the pick message is a later item.
8. **Reset mode chooser UI.** [**Three explicit menu items** ("soft", "mixed", "hard…"), each
   opening the shared reset ConfirmDialog with a mode-specific body.] A dedicated radio dialog is
   the rejected heavier alternative.
9. **CLI-started cherry-pick/revert SEQUENCES.** git2 has no sequencer support. [Bonsai only ever
   *starts* a **single** pick/revert; `*_continue` commits the one in-progress operation and
   `cleanup_state`s. A CLI-started multi-commit sequence's `.git/sequencer` todo is NOT advanced.]
   Documented limitation; the banner still lets the user finish/abort the current step.

None of these block implementation; all defaults are safe.

---

## 1. Module boundaries & file responsibilities

New core modules under `crates/bonsai-core/src/git/` (each registered with a `pub mod` line in
`crates/bonsai-core/src/git/mod.rs`, alphabetical with the existing block):

| File | Responsibility | Increment |
|------|----------------|-----------|
| `commit.rs` (extend) | add `amend_commit` as a sibling of `create_commit`; reuse `resolve_signature`, `CommitResult`, and the shared message-normalize step | P20a |
| `reset.rs` (new) | `ResetMode`, `reset_branch` | P20a |
| `discard.rs` (new) | `discard_paths` (worktree restore from index) | P20a |
| `cherrypick.rs` (new) | `CherrypickOutcome`, `cherrypick_commit`, `cherrypick_continue`, `cherrypick_abort`, `finalize_cherrypick` | P20b |
| `revert.rs` (new) | `RevertOutcome`, `revert_commit`, `revert_continue`, `revert_abort`, `finalize_revert` | P20b |

Unchanged but REUSED verbatim: `opstate.rs` (detection), `conflict.rs`
(`list_conflicts`/`get_conflict`/`resolve_conflict`/`resolve_conflict_text` — cherry-pick/revert
index conflicts flow through these identically to merge/rebase), `stage.rs`
(`open_workdir_repo`, `validate_rel_path`), `repo.rs` (`read_head_info`).

Command layer: `src-tauri/src/commands.rs` gains one `#[tauri::command]` + runtime-free `_inner`
per operation (§7); `src-tauri/src/lib.rs` registers each in `generate_handler!`.

Frontend: IPC triple (`src/ipc/{types.ts,tauri.ts,mock.ts}`), `OpBanner.tsx` (P20b),
`CommitBox.tsx` + `RepoWorkspace.tsx` + `StatusPanel.tsx` wiring.

---

## 2. Amend (P20a)

### 2.1 Core — `commit.rs`

```rust
/// Blocking. Replaces HEAD with a new commit built from the current index, on
/// HEAD's EXISTING parents (preserves merge parents), reusing HEAD's ORIGINAL
/// author and stamping a fresh committer. `message` is the final message (the
/// frontend prefills + lets the user edit HEAD's message). Mirrors
/// `git commit --amend -m <message>`.
pub fn amend_commit(workdir: &Path, message: &str) -> Result<CommitResult, AppError>;
```

Flow (cheap checks first, nothing mutates until the final `commit`):
1. `let repo = open_workdir_repo(workdir)?;`
2. `if repo.state() != Clean` → `AppError::OperationInProgress("an operation is in progress — finish or abort it first")` (amending mid-merge/rebase/pick is nonsense).
3. HEAD commit: `repo.head()?.peel_to_commit()` — on `UnbornBranch`/`NotFound` →
   `AppError::Git("nothing to amend: the repository has no commits yet")`.
4. Normalize `message` exactly like `create_commit` (`replace("\r\n","\n").replace('\r',"\n")`, then `trim`); empty → `AppError::EmptyMessage`.
5. `let committer = resolve_signature(&repo.config()?.snapshot()?)?;` (`ConfigMissing` before any write).
6. `let author = head_commit.author().to_owned();` (preserve original author + author-time, like git).
7. Parents = HEAD's parents: `let parents: Vec<git2::Commit> = head_commit.parents().collect();` → `parent_refs: Vec<&Commit>`.
8. Tree from the index: `let tree = repo.find_tree(repo.index()?.write_tree()?)?;`
   **No `NothingToCommit` guard** — a message-only amend (tree == HEAD's tree, 0 staged) is valid.
9. `let oid = repo.commit(Some("HEAD"), &author, &committer, &format!("{msg}\n"), &tree, &parent_refs)?;`
   (updating `HEAD` moves the current branch ref to the new commit; the old tip is orphaned/reflogged.)
10. Return `CommitResult { oid: oid.to_string(), summary: first-line, branch: current-branch-or-None }` (same construction as `create_commit`).

### 2.2 Wire / command

No new wire type — returns the existing `CommitResult`. Command `commit_amend(repoId, message)`
(§7). Errors: `operationInProgress | git | emptyMessage | configMissing | noRepo`.

### 2.3 UI — Amend affordance in the commit box (RepoWorkspace + CommitBox)

- RepoWorkspace owns `amend: boolean` state and a checkbox **"Amend last commit"** rendered above
  the `CommitBox` (commit mode only; hidden/disabled when `opState.kind !== 'none'`,
  `mutating`, or `head` is `null`/`unborn`).
- On toggling amend **on**: fetch HEAD's full message once via
  `ipc.getCommitDiff(repoId, head.oid)` → `details.message`, then remount `CommitBox` with
  `key="amend"`, `initialMessage={that message}`, and an `onCommit` bound to `ipc.commitAmend`.
  On toggling **off**: remount with `key="commit"` and the normal `onCommit → ipc.commit`.
- **Push-guard note** (OPEN #1): when the current `BranchInfo` has `upstream !== null && ahead === 0`,
  render a warning line next to the checkbox: *"This commit is already pushed — amending rewrites
  published history."* (informational; does not block).
- `CommitBox.tsx` gains one optional prop `amend?: boolean` that (a) sets the button label to
  **"Amend"** (else "Commit"/"Commit merge"), and (b) relaxes the submit-disabled gate so
  `stagedCount === 0` does NOT disable submit in amend mode (message-only amend is valid). Merge
  mode is unaffected.

---

## 3. Reset (P20a)

### 3.1 Core — `reset.rs`

```rust
/// Reset MODE. Wire: "soft" | "mixed" | "hard".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResetMode { Soft, Mixed, Hard }

/// Blocking. Moves the CURRENT branch (HEAD) to `target_oid`.
/// Soft: move ref only. Mixed: ref + index. Hard: ref + index + worktree
/// (destructive — the UI confirms first). Mirrors `git reset --soft/--mixed/--hard <oid>`.
pub fn reset_branch(workdir: &Path, target_oid: &str, mode: ResetMode) -> Result<(), AppError>;
```

Flow:
1. `let repo = open_workdir_repo(workdir)?;`
2. `if repo.state() != Clean` → `OperationInProgress`.
3. HEAD must be born: `repo.head()` on `UnbornBranch`/`NotFound` → `AppError::Git("nothing to reset: the repository has no commits yet")`. (Detached HEAD is allowed — git reset moves the detached HEAD; the UI only offers it on attached HEAD, §3.3.)
4. `let oid = git2::Oid::from_str(target_oid).map_err(|_| AppError::Git("invalid commit id"))?;`
   `let obj = repo.find_object(oid, None)?;` (a non-commit / unknown oid surfaces as `AppError::Git`). Peel to a commit to reject non-commit targets: `obj.peel_to_commit()` else `AppError::Git("not a commit")`.
5. `let kind = match mode { Soft => Soft, Mixed => Mixed, Hard => Hard };` → `repo.reset(commit.as_object(), kind, None)?;`
   (git2 handles index/worktree per mode; no `CheckoutBuilder` needed. Hard uses libgit2's forced checkout internally.)
6. `Ok(())`.

### 3.2 Wire / command

No result payload (`()`). Command `reset_branch(repoId, oid, mode)` (§7). `mode` deserializes the
`ResetMode` enum. Errors: `operationInProgress | git | noRepo`.

### 3.3 UI — "Reset … to here" with a mode chooser + mandatory Hard confirm

- Add to `commitMenuItems(oid)` and `branchMenuItems(name, kind)` in `RepoWorkspace.tsx`, gated on:
  attached born HEAD (`head !== null && !head.unborn && !head.detached`), not `mutating`/`opActive`,
  and the target is not already the current tip.
- Three items (OPEN #8), where `<b>` is the current branch name:
  - `Reset <b> to here (soft)` → `setPendingReset({ oid, mode: 'soft' })`
  - `Reset <b> to here (mixed)` → `setPendingReset({ oid, mode: 'mixed' })`
  - `Reset <b> to here (hard)…` → `setPendingReset({ oid, mode: 'hard' })`
- A single shared **ConfirmDialog** (new `pendingReset: { oid, mode } | null` state, same pattern
  as `pendingDeleteBranch`) confirms all three (moving the ref orphans commits). **Hard is a HARD
  requirement to confirm**; its body additionally warns: *"Uncommitted changes in your working
  tree will be permanently discarded."* On confirm → `ipc.resetBranch(repoId, oid, mode)` then
  `refreshAll()`.

---

## 4. Discard (P20a)

### 4.1 Core — `discard.rs`

```rust
/// Blocking. Restores each tracked path's WORKTREE content to the INDEX version
/// (`git checkout -- <paths>` / `git restore --worktree`), discarding unstaged
/// edits and recreating unstaged deletions. Staged content is untouched.
/// All-or-nothing validation (like stage_paths): validate every path first.
/// An empty `paths` vec is a no-op Ok(()). Destructive — the UI confirms first.
pub fn discard_paths(workdir: &Path, paths: &[String]) -> Result<(), AppError>;
```

Flow:
1. `if paths.is_empty() { return Ok(()); }`
2. `for p in paths { validate_rel_path(p)?; }` (reuses `stage.rs`; `..`/absolute/backslash → `AppError::Other("invalid path: …")`, consistent with existing staging).
3. `let repo = open_workdir_repo(workdir)?;` `let index = repo.index()?;`
4. Defensive tracked-only guard (OPEN #3): for each `p`, if `index.get_path(Path::new(p), 0).is_none()` → `AppError::Git("cannot discard '<p>': not a tracked file")`. (The UI never offers Discard on untracked rows, so this is a belt-and-suspenders check.)
5. Force-checkout exactly those paths from the current index:
   ```rust
   let mut cb = git2::build::CheckoutBuilder::new();
   cb.force().remove_untracked(false);
   for p in paths { cb.path(p.as_str()); }
   repo.checkout_index(None, Some(&mut cb))?; // None target = the repo's current index
   ```
   **CRITICAL** (same lesson as `abort_merge`): a `CheckoutBuilder` with **zero** `.path()` calls
   matches ALL paths — the `paths.is_empty()` early return in step 1 guarantees at least one
   `.path()` is set here, so a whole-worktree clobber is impossible.

### 4.2 Wire / command

No result payload (`()`). Command `discard_paths(repoId, paths)` (§7). Errors:
`other`(invalid path) `| git | noRepo`.

### 4.3 UI — "Discard changes" on Changes-section rows

- `StatusPanel.tsx` gains an `onDiscard(paths: string[])` prop. The **Changes** section renders a
  per-row discard control (a secondary `↺` button beside the existing `+` stage button, shown on
  the row's tracked entries only — i.e. rows whose resolved origin section is `unstaged`, NOT
  `untracked`). Untracked rows show no discard control (OPEN #3).
- `RepoWorkspace.tsx` owns `pendingDiscard: string[] | null` and a **ConfirmDialog** (same pattern
  as `pendingDropStash`): body *"Discard changes to N file(s)? This permanently reverts them to the
  last staged/committed version and cannot be undone."* On confirm →
  `ipc.discardPaths(repoId, paths)` then `refreshAll()`.

---

## 5. Cherry-pick (P20b)

### 5.1 Core — `cherrypick.rs`

```rust
/// Wire: tagged "kind", camelCase (identical recipe to MergeOutcome).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CherrypickOutcome {
    /// Clean pick, auto-committed. `oid` = the new commit.
    Committed { oid: String },
    /// Index/worktree hold conflict markers; CHERRY_PICK_HEAD written; repo
    /// paused in state CherryPick. `paths` = sorted conflicted paths (the exact
    /// set list_conflicts returns).
    Conflicts { paths: Vec<String> },
}

/// Blocking. Cherry-picks `oid` onto the current branch. Clean → commit
/// immediately (reusing the picked commit's message + ORIGINAL author, fresh
/// committer, like git). Conflict → pause for the OpBanner/conflict.rs flow.
pub fn cherrypick_commit(workdir: &Path, oid: &str) -> Result<CherrypickOutcome, AppError>;

/// Blocking. Finalizes a paused (resolved) cherry-pick — commits the resolved
/// index reusing CHERRY_PICK_HEAD's message/author (parallels commit_merge).
pub fn cherrypick_continue(workdir: &Path) -> Result<CherrypickOutcome, AppError>;

/// Blocking. Aborts a paused cherry-pick: reset --hard to HEAD + cleanup_state
/// (parallels git `cherry-pick --abort`; destructive — the UI confirms first).
pub fn cherrypick_abort(workdir: &Path) -> Result<(), AppError>;
```

`cherrypick_commit` flow (preconditions all before any mutation, per merge/rebase pattern):
1. `open_workdir_repo`; `if state != Clean` → `OperationInProgress`.
2. `read_head_info`: unborn → `Git("cannot cherry-pick: no commits yet")`; detached → `Git("cannot cherry-pick: HEAD is detached")` (OPEN #5).
3. Resolve target: `let pick = repo.find_commit(git2::Oid::from_str(oid).map_err(|_| AppError::Git("invalid commit id"))?)?;` (unknown oid → `AppError::Git`).
4. Dirty-index guard (mirror `rebase.rs:239-247`): `index.has_conflicts() || index.write_tree_to(&repo)? != head_commit.tree_id()` → `Git("cannot cherry-pick: your index contains uncommitted changes — commit or unstage them first")`.
5. `let sig = resolve_signature(&repo.config()?.snapshot()?)?;` (`ConfigMissing` early — the clean path auto-commits).
6. `repo.cherrypick(&pick, None)?` — sets index/worktree + writes `CHERRY_PICK_HEAD`, state → `CherryPick`. **Failure handling:** on `Err(e)`, `let _ = repo.cleanup_state();` (guarantee: a failed start leaves state `Clean`), then map `e.code()==Conflict → CheckoutConflict("cannot cherry-pick: local changes would be overwritten. Commit or discard them first.")`, else `e.into()`.
7. `if repo.index()?.has_conflicts()` → `Ok(Conflicts { paths: list_conflicts(workdir)? … })` (state stays `CherryPick`; banner drives continue/abort).
8. Else → `finalize_cherrypick(&mut repo, &sig)` → `Committed { oid }`.

`finalize_cherrypick(repo, committer)` (shared by the clean path AND `cherrypick_continue`):
1. Read the picked oid from `CHERRY_PICK_HEAD`: `let s = std::fs::read_to_string(repo.path().join("CHERRY_PICK_HEAD"))?; let pick = repo.find_commit(git2::Oid::from_str(s.trim())?)?;` (missing file → `Git("CHERRY_PICK_HEAD missing")`).
2. `let author = pick.author().to_owned();` (original author preserved), `let message = pick.message().unwrap_or("");` (reused verbatim; normalize CRLF/CR → `\n` + ensure single trailing `\n`, matching commit.rs).
3. `let head_commit = repo.head()?.peel_to_commit()?;` `let tree = repo.find_tree(repo.index()?.write_tree()?)?;`
4. **Empty guard** (OPEN #6): `if tree.id() == head_commit.tree_id() { repo.cleanup_state()?; return Err(AppError::NothingToCommit); }`
5. `let new = repo.commit(Some("HEAD"), &author, committer, &message, &tree, &[&head_commit])?;`
6. `repo.cleanup_state()?;` (removes `CHERRY_PICK_HEAD` → state `Clean`).
7. `Ok(CherrypickOutcome::Committed { oid: new.to_string() })`.

`cherrypick_continue`:
1. `open_workdir_repo`; `if state != CherryPick` → `NoOperationInProgress("no cherry-pick in progress")`.
2. `if repo.index()?.has_conflicts()` → `UnresolvedConflicts("cannot continue: N unresolved conflict(s) remain")`.
3. `let sig = resolve_signature(…)?;` → `finalize_cherrypick(&mut repo, &sig)`. A HARD error returns `Err` and **leaves the on-disk state intact** (no cleanup), same discipline as `rebase_continue` (§3.9 of P3d).

`cherrypick_abort`:
1. `open_workdir_repo`; `if state != CherryPick` → `NoOperationInProgress`.
2. `let head_obj = repo.head()?.peel_to_commit()?.into_object();` `repo.reset(&head_obj, git2::ResetType::Hard, None)?;` then `repo.cleanup_state()?;` (git-consistent `cherry-pick --abort`). Destructive → UI confirms (§8.3).

### 5.2 Cherry-pick UI

Commit-row context menu (`commitMenuItems(oid)`), gated on attached born HEAD + not
`mutating`/`opActive`: **"Cherry-pick onto current"** → `handleCherrypick(oid)` calling
`ipc.cherrypickCommit`. Toast mapping (§8.1). On `conflicts`, the subsequent refresh surfaces
`opState.kind === 'cherryPick'` and the `OpBanner` becomes actionable (§8.2).

---

## 6. Revert (P20b)

Structurally identical to §5 with `repo.revert` + `REVERT_HEAD`. Differences below.

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RevertOutcome {
    Committed { oid: String },
    Conflicts { paths: Vec<String> },
}

pub fn revert_commit(workdir: &Path, oid: &str) -> Result<RevertOutcome, AppError>;
pub fn revert_continue(workdir: &Path) -> Result<RevertOutcome, AppError>;
pub fn revert_abort(workdir: &Path) -> Result<(), AppError>;
```

- `revert_commit`: same preconditions/guards as `cherrypick_commit`; `repo.revert(&commit, None)?`
  (writes `REVERT_HEAD`, state → `Revert`); failed-start cleanup + conflict mapping identical.
- `finalize_revert(repo, sig)`: reads `REVERT_HEAD` for the reverted commit; **both author AND
  committer = the resolved current signature** (git revert authors the revert as you); message =
  the byte-exact `git revert --no-edit` form:
  ```
  Revert "<subject>"

  This reverts commit <full-oid>.
  ```
  where `<subject>` is the first line of the reverted commit's message and `<full-oid>` its 40-hex
  id. Empty-result guard (`nothingToCommit`) and `cleanup_state` identical to §5.1.
- `revert_continue` / `revert_abort`: identical shape to cherry-pick, guarding on
  `state == Revert`, "no revert in progress" messages.
- UI: commit-row menu **"Revert commit"** → `handleRevert(oid)` → `ipc.revertCommit`. Toast + banner
  identical to cherry-pick (§8).

---

## 7. Command surface (`commands.rs` + `lib.rs`)

Each command follows the established `pub async fn NAME(state, repo_id, …) -> Result<T, AppError>`
+ runtime-free `NAME_inner` + `spawn_blocking(move || core_fn(&path, …))` template (see
`merge_branch` / `create_stash`), resolving the path with `repo_path(state, &repo_id)?`.

| Command (snake) | IPC method (camel) | Args | Returns | Error kinds |
|---|---|---|---|---|
| `commit_amend` | `commitAmend` | `repoId, message` | `CommitResult` | `operationInProgress \| git \| emptyMessage \| configMissing \| noRepo` |
| `reset_branch` | `resetBranch` | `repoId, oid, mode` | `void` | `operationInProgress \| git \| noRepo` |
| `discard_paths` | `discardPaths` | `repoId, paths` | `void` | `other \| git \| noRepo` |
| `cherrypick_commit` | `cherrypickCommit` | `repoId, oid` | `CherrypickOutcome` | `operationInProgress \| git \| checkoutConflict \| configMissing \| nothingToCommit \| noRepo` |
| `cherrypick_continue` | `cherrypickContinue` | `repoId` | `CherrypickOutcome` | `noOperationInProgress \| unresolvedConflicts \| configMissing \| nothingToCommit \| git \| noRepo` |
| `cherrypick_abort` | `cherrypickAbort` | `repoId` | `void` | `noOperationInProgress \| git \| noRepo` |
| `revert_commit` | `revertCommit` | `repoId, oid` | `RevertOutcome` | same as `cherrypick_commit` |
| `revert_continue` | `revertContinue` | `repoId` | `RevertOutcome` | same as `cherrypick_continue` |
| `revert_abort` | `revertAbort` | `repoId` | `void` | `noOperationInProgress \| git \| noRepo` |

Register all nine in `lib.rs` `generate_handler!`. `mode` arg on `reset_branch` deserializes the
`ResetMode` enum (Tauri passes the camelCase string). **No events, no channels.**

### 7.1 TypeScript wire types (`src/ipc/types.ts`)

```ts
export type ResetMode = 'soft' | 'mixed' | 'hard';

export type CherrypickOutcome =
  | { kind: 'committed'; oid: string }
  | { kind: 'conflicts'; paths: string[] };

export type RevertOutcome =
  | { kind: 'committed'; oid: string }
  | { kind: 'conflicts'; paths: string[] };
```

### 7.2 `IpcApi` additions (`types.ts`) + `tauri.ts` implementations

```ts
// IpcApi:
commitAmend(repoId: string, message: string): Promise<CommitResult>;
resetBranch(repoId: string, oid: string, mode: ResetMode): Promise<void>;
discardPaths(repoId: string, paths: string[]): Promise<void>;
cherrypickCommit(repoId: string, oid: string): Promise<CherrypickOutcome>;
cherrypickContinue(repoId: string): Promise<CherrypickOutcome>;
cherrypickAbort(repoId: string): Promise<void>;
revertCommit(repoId: string, oid: string): Promise<RevertOutcome>;
revertContinue(repoId: string): Promise<RevertOutcome>;
revertAbort(repoId: string): Promise<void>;
```

`tauri.ts` — one thin `invoke` wrapper each, matching arg names exactly (snake→camel is automatic
for the command name only; the *args object* uses the camelCase keys Tauri expects, e.g.
`invoke('reset_branch', { repoId, oid, mode })`, `invoke('discard_paths', { repoId, paths })`).

---

## 8. Frontend behavior — toasts, OpBanner, mock

### 8.1 Toast mapping (`RepoWorkspace.tsx`, after each mutation → `refreshAll()`)

- `commitAmend` success → `success` "Amended last commit". `configMissing` surfaces in the commit
  box's own error banner (CommitBox catches), same as `commit`.
- `resetBranch` success → `success` `Reset <branch> to <short-oid> (<mode>)`.
- `discardPaths` success → `success` `Discarded changes to N file(s)`.
- `cherrypickCommit` / `revertCommit`:
  - `committed` → `success` `Cherry-picked <short-oid>` / `Reverted <short-oid>`.
  - `conflicts` → `info` `Cherry-pick paused: N conflict(s) to resolve` / `Revert paused: …`.
- `cherrypickContinue` / `revertContinue`: `committed` → `success`; `conflicts` can't recur (single
  pick) but map to `info` defensively.
- `nothingToCommit` (empty pick/revert) → `info` "Nothing to apply — the change is already present".
- `checkoutConflict` → `error` with the backend message.

### 8.2 `OpBanner.tsx` — make cherry-pick / revert ACTIONABLE (P20b)

Today `OpBanner` renders `cherryPick`/`revert` as an informational strip (OpBanner.tsx:113-121).
Replace that branch with an actionable one mirroring the merge branch:

- Title: `Cherry-picking` / `Reverting`.
- Sub: `${conflictCount} conflict(s) remaining` when `> 0`, else `All conflicts resolved`.
- **Continue** (`btn-primary`, disabled when `conflictCount > 0 || mutating`) → new prop
  `onOpContinue()`.
- **Abort** (`btn-danger`, disabled when `mutating`) → the existing `onAbort()` prop (App opens a
  ConfirmDialog). **No Skip** (single-step ops).

New `OpBannerProps`: add `onOpContinue(): void`. `RepoWorkspace` wires `onOpContinue` to
`opState.kind === 'cherryPick' ? handleCherrypickContinue : handleRevertContinue`, and extends the
Abort ConfirmDialog (§8.3) to cover both.

The `CommitBox` stays `blocked` for `cherryPick`/`revert` (it already does per its `blocked` prop
doc, CommitBox.tsx:19-20) — Continue lives only in the banner.

### 8.3 Abort ConfirmDialog (P20b)

Extend the existing abort ConfirmDialog (RepoWorkspace.tsx:2255) so `opState.kind` of
`cherryPick`/`revert` yields title/label `Abort cherry-pick` / `Abort revert` and body: *"This
resets your branch and working tree to HEAD. The in-progress cherry-pick/revert and any conflict
resolutions will be lost."* On confirm → `ipc.cherrypickAbort` / `ipc.revertAbort`.

### 8.4 Mock IPC (`src/ipc/mock.ts`) — keep the browser harness implementable

All nine methods are added to `mockIpc`, reusing the existing per-repo `state` (`opState`,
`conflicts`, `conflictTexts`, `status`, `commits`, `headOid`, `headBranch`) and helpers
(`requireRepo`, `randomOid`, `delay`, `upsert`, `sortByPath`). Behaviors:

- `commitAmend(message)`: `state.headOid = randomOid()`; replace the top `commits[0]` summary with
  `firstLine(message)`; return `{ oid: state.headOid, summary, branch: state.headBranch }`.
- `resetBranch(oid, mode)`: `void` (optionally drop `commits` above the target for visual fidelity;
  a plain `void` is acceptable for the harness).
- `discardPaths(paths)`: remove `paths` from `state.status.unstaged`; `void`.
- `cherrypickCommit(oid)` / `revertCommit(oid)`: guard `state.opState.kind !== 'none'` →
  `operationInProgress`. **Demo trigger** (mirrors the merge `name.includes('conflict')`
  convention, but keyed on oid): if `oid` matches the designated conflicting fixture oid (document a
  constant, e.g. an oid ending in `"c0ffee"`), set `state.opState = { kind: 'cherryPick' }` /
  `{ kind: 'revert' }`, populate `state.conflicts` + `state.status.conflicted` + `conflictTexts`
  with `['src/app.ts']` (reuse the merge-conflict fixture builder), and return
  `{ kind: 'conflicts', paths: ['src/app.ts'] }`. Otherwise push a new top commit
  (`state.headOid = randomOid()`, `commits.unshift`) and return `{ kind: 'committed', oid }`.
- `cherrypickContinue()` / `revertContinue()`: guard `state.opState.kind !== 'cherryPick'|'revert'`
  → `noOperationInProgress`; guard `state.conflicts.length > 0` → `unresolvedConflicts`; else clear
  op state + conflicted list, push a top commit, return `{ kind: 'committed', oid: state.headOid }`.
- `cherrypickAbort()` / `revertAbort()`: clear `opState`, `conflicts`, `conflictTexts`,
  `status.conflicted` (mirrors `abortMerge`).

Existing `getOpState` already returns `state.opState` (mock.ts:1499) and already types
`cherryPick`/`revert`, so the banner renders from the mock with no extra plumbing.

---

## 9. Error mapping (no `error.rs` change)

| Situation | Variant | TS kind |
|---|---|---|
| Op already in progress (start-time) | `OperationInProgress` | `operationInProgress` |
| No op in progress (continue/abort) | `NoOperationInProgress` | `noOperationInProgress` |
| Unresolved conflicts on continue | `UnresolvedConflicts` | `unresolvedConflicts` |
| Missing git identity | `ConfigMissing` | `configMissing` |
| Empty amend/continue message | `EmptyMessage` | `emptyMessage` |
| Empty pick/revert (tree == HEAD) | `NothingToCommit` | `nothingToCommit` |
| Local changes would be overwritten | `CheckoutConflict` | `checkoutConflict` |
| Invalid discard path (`..`/abs/backslash) | `Other` | `other` |
| Unborn/detached/unknown-oid/not-a-commit, `*_HEAD` missing, bare repo, git2 errors | `Git` | `git` |
| Unknown `repoId` | `NoRepo` | `noRepo` |

---

## 10. Tests (AI gate)

### 10.1 Rust unit tests (in each core module's `#[cfg(test)]`)

Every new outcome enum gets a `wire_shapes_are_camel_case_tagged` test asserting exact JSON (mirror
`merge.rs`), e.g. `{"kind":"committed","oid":…}`, `{"kind":"conflicts","paths":[…]}`, and
`ResetMode` deserializing `"soft"|"mixed"|"hard"`. Precondition tests on a fresh repo (unborn HEAD;
"no operation in progress" for every `*_continue`/`*_abort`) mirror `merge_preconditions_on_fresh_repo`.

Fixtures use `crate::testutil::scratch_dir()` + git2 builders (deterministic identity + `core.autocrlf=false`), reusing the `p8_*`/`s9_*` helper style.

### 10.2 CLI-oracle suite — `crates/bonsai-core/tests/essentials_cli.rs`

Scratch repos under `D:\Temp\bonsai-scratch`; **`TMP`/`TEMP=D:\Temp`** (USER MANDATE); Bash uses
forward-slash paths; `cargo test` and `clippy` run **sequentially** (target-dir race). Each test
builds the identical history twice (once through Bonsai cores, once through real `git`) and
compares **tree oids / index / worktree state** — never commit oids (timestamp-dependent). Degrade
gracefully (skip, don't fail) when `git` is absent, like `p8_git_cli_autostash_ff_oracle`.

1. **Amend** — stage a change + amend; assert the new HEAD tree == `git commit --amend`'s tree AND
   parent set is preserved (incl. a merge-commit amend keeps both parents). Assert message-only
   amend (0 staged) succeeds and keeps the original author, new committer.
2. **Cherry-pick clean** — pick a commit onto a divergent branch; assert resulting tree oid ==
   `git cherry-pick`'s, HEAD advanced by one, message == the picked commit's, author preserved.
3. **Cherry-pick conflict** — construct a guaranteed conflict; assert `Conflicts{paths}` +
   `state==CherryPick`; resolve via `conflict::resolve_conflict`/`resolve_conflict_text` →
   `cherrypick_continue`; assert the final tree == git's hand-resolved result and `state==Clean`.
4. **Revert clean** — assert tree == `git revert --no-edit`'s tree, and the message is byte-exact
   `Revert "<subject>"\n\nThis reverts commit <oid>.\n`.
5. **Revert conflict** — conflict → resolve → `revert_continue`; assert final tree == git's.
6. **Reset soft/mixed/hard** — one case each; assert HEAD ref, index (`write_tree_to` vs target),
   and worktree state match `git reset --soft/--mixed/--hard`. Hard: worktree reset; soft: worktree
   & index unchanged; mixed: index reset, worktree unchanged.
7. **Discard** — modify a tracked file (unstaged) + a second tracked file; discard only the first;
   assert file 1's bytes are byte-exact the index/HEAD blob and file 2 is untouched. Assert an
   untracked path errors (`not a tracked file`).
8. **Abort** — start a conflicting cherry-pick, then `cherrypick_abort`; assert `state==Clean`,
   HEAD unchanged, worktree back at HEAD (same for revert).

### 10.3 Frontend AI gate

`pnpm build` + `tsc` clean; browser harness (`VITE_MOCK_IPC=1`) renders: the Amend checkbox +
push-guard note; the reset menu items → confirm dialog (hard warning); the discard row control →
confirm; the cherry-pick/revert commit-row items; and — via the mock conflict trigger — the
actionable cherry-pick/revert `OpBanner` (Continue disabled until conflicts resolve; Abort confirm).

---

## 11. Sub-increment breakdown (each = one fresh-context `senior-dev` pass)

- **P20a — Amend + Reset + Discard (no conflict machinery).**
  - Rust: `amend_commit` in `commit.rs`; new `reset.rs`, `discard.rs`; `mod.rs` registrations;
    unit tests.
  - Commands: `commit_amend`, `reset_branch`, `discard_paths` (+ `lib.rs`).
  - IPC triple: `ResetMode` + three methods in `types.ts`/`tauri.ts`/`mock.ts`.
  - UI: Amend checkbox + push-guard note (RepoWorkspace) + `amend` prop (CommitBox); reset menu
    items + reset ConfirmDialog; discard row control (StatusPanel `onDiscard`) + discard
    ConfirmDialog.
  - Tests: oracle suite rows 1, 6, 7.
- **P20b — Cherry-pick + Revert (conflict / OpBanner reuse).**
  - Rust: new `cherrypick.rs`, `revert.rs` (outcome enums + start/continue/abort + finalize helpers
    reading `CHERRY_PICK_HEAD`/`REVERT_HEAD`); `mod.rs`; unit tests.
  - Commands: six commands (+ `lib.rs`).
  - IPC triple: `CherrypickOutcome`/`RevertOutcome` + six methods; mock conflict trigger + op-state
    plumbing.
  - UI: `OpBanner` actionable branch (`onOpContinue`); commit-row menu items; toast mapping; abort
    ConfirmDialog extension.
  - Tests: oracle suite rows 2, 3, 4, 5, 8.

Commit each approved sub-increment as `wip(P20a): …` / `wip(P20b): …` (orchestrator owns commits).

---

## 12. Acceptance criteria — AI gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable, no network, no native window):**
- `cargo check` + `clippy` clean; `pnpm build` + `tsc` clean.
- All §10.1 unit tests + §10.2 oracle suite green (oracle degrades to git2-only when `git` absent).
- Browser-harness screenshots per §10.3.

**USER CHECKPOINT (native `pnpm tauri dev`, real repo):**
- Amend a commit (message edit + restage) and confirm the tip is rewritten; confirm the
  already-pushed warning shows when the tip is pushed.
- Reset a branch to an older commit in each mode; confirm the Hard dialog gates and worktree is
  discarded only on Hard.
- Discard a modified file and confirm it reverts (and an unrelated modified file is untouched).
- Cherry-pick and revert a commit: confirm the clean path commits + toasts; drive a conflicting
  pick through the actionable `OpBanner` (resolve → Continue) and an Abort (confirm dialog restores
  state).
