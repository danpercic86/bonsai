# P3d — Rebase: Implementation Contract

Status: authoritative for P3d. Scope: **plain non-interactive rebase** of the current branch onto a
chosen target (local branch OR remote-tracking shorthand like `origin/main`). No interactive
rebase, no todo-list editing/reordering/squashing. Clean rebases replay + finish automatically;
conflicts pause into `RepoOpState::Rebase` and reuse P3c's conflict machinery verbatim; the paused
rebase is actionable via **Continue / Skip / Abort**. Builds on `P3c-merge-conflicts.md` (the
sibling engine — this contract mirrors it closely and says "identical to P3c §X" wherever the design
is shared), `M5-branches.md` (branch resolution, safe checkout), `M6-remotes.md` (safe-FF recipe),
`M3-commit.md` (`resolve_signature`, commit step order).

**Reuse mandate (locked):** `git/opstate.rs` and `git/conflict.rs` are SHARED and
operation-agnostic. P3d drives the exact same `RepoOpState::Rebase` wire type (**unchanged** — see
`opstate.rs` §2 note), the same `list_conflicts`/`get_conflict`/`resolve_conflict`, the same
`ConflictKind`/marker view, and the same frontend conflict-row + `conflict:<path>` DiffOverlay
marker view. **Nothing rebase-specific may leak into `conflict.rs`.** Confirm-and-reuse; do NOT
duplicate.

Invariants (unchanged from P3c §intro): Rust owns all Git logic + layout math; IPC carries compact
precomputed data; commands = req/resp; git2 under `spawn_blocking` via the established runtime-free
`*_inner(state: &AppState, ...)` pattern; `src/ipc/mock.ts` updated with EVERY IpcApi change and
kept a faithful stateful twin; destructive ops (abort rebase) require explicit `ConfirmDialog`
confirmation AND a backend state guard.

---

## 1. Scope split (sub-increments)

| # | Increment | Content |
|---|---|---|
| 1 | **P3d-a** | Backend: `git/rebase.rs` (engine + algorithm), 4 new commands in `commands.rs` + `lib.rs` registration, error mapping (NO new variants), `opstate.rs` confirmation (no code change), module unit tests (wire shapes + preconditions). Read §2–§6. |
| 2 | **P3d-b** | IPC mirror: `src/ipc/types.ts`, `tauri.ts`, `index.ts`, stateful mock (`?op=rebase`). Read §7. |
| 3 | **P3d-c** | Frontend: `OpBanner.tsx` actionable rebase mode; `App.tsx` handlers/wiring/gating; `Sidebar.tsx` rebase affordance. Read §8. |

Each is a self-contained fresh-context senior-dev pass (this file + the exact source paths). Tester
(after P3d-a lands): §9 CLI-oracle suite `src-tauri/tests/rebase_cli.rs`.

---

## 2. `src-tauri/src/git/opstate.rs` — confirmation, NO code change

The `RepoOpState::Rebase` wire type (`head_name`, `onto`, `current_step`, `total_steps`) is **fully
shaped and MUST NOT change.** P3d only makes it *actionable*.

**Confirmation of the read path (locked, no refactor):** `read_rebase_state` keeps its best-effort
plain-file reads of `rebase-merge/{head-name,onto,msgnum,end}` (fallback `rebase-apply/{msgnum,end}`).
This is **still correct** now that Bonsai starts rebases via libgit2:

- A libgit2-driven rebase writes `RepositoryState::RebaseMerge` with `rebase-merge/head-name`
  (`refs/heads/<branch>`), `rebase-merge/onto` (full oid), `rebase-merge/msgnum` (1-based current
  step) and `rebase-merge/end` (total). The existing derivation reads all four exactly.
- The read path **must stay** on plain file reads, NOT `repo.open_rebase()`: `read_op_state` is a
  refresh-batch call that must also survive CLI-started rebases (apply-backend, `rebase-apply/…`)
  which `open_rebase()` may refuse. `open_rebase()` is used ONLY by the mutating rebase commands
  (§4), never by `read_op_state`.

Senior-dev change here: **none.** Tester adds an integration assertion (§9) that a Bonsai-started
paused rebase yields `Rebase { head_name: Some("<branch>"), onto: Some("<40-hex>"), current_step,
total_steps }` with `current_step`/`total_steps` matching the paused engine's own counters (§3.4).

---

## 3. `src-tauri/src/git/rebase.rs` — rebase engine

```rust
//! Plain non-interactive rebase of the current branch onto a target (local or
//! remote-tracking). Clean rebases replay + finish automatically; conflicts
//! pause into RepoOpState::Rebase and reuse git/conflict.rs verbatim. Pure
//! git2, no Tauri types, no network (rebasing onto origin/x uses the local
//! remote-tracking ref — Fetch first is the user's job, same as merge). (P3d §3.)

use std::path::Path;

use crate::error::AppError;
use crate::git::commit::resolve_signature;
use crate::git::conflict::list_conflicts;
use crate::git::repo::read_head_info;
use crate::git::stage::open_workdir_repo;

/// Wire: tagged "kind", camelCase (identical recipe to MergeOutcome, P3c §4).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RebaseOutcome {
    /// `onto` is already an ancestor of HEAD (branch already based on it, or
    /// ahead) — nothing to replay. HEAD unmoved.
    UpToDate,
    /// HEAD was an ancestor of `onto`: the branch was fast-forwarded to `onto`
    /// (full oid `to`). No commits were rewritten.
    FastForwarded { branch: String, to: String },
    /// Rebase ran to completion (rebase.finish()). `branch` = the rebased
    /// branch, `head` = its new tip (full oid), `steps` = number of operations
    /// in the plan (rebase.len(); dropped-empty picks are still counted).
    Rebased { branch: String, head: String, steps: u32 },
    /// Replay paused on a conflict. Index + worktree hold the conflict markers;
    /// on-disk rebase-merge state persists. `paths` = sorted conflicted paths
    /// (same set list_conflicts returns); `current_step`/`total_steps` mirror
    /// the git msgnum/end (1-based current, total).
    Conflicts { paths: Vec<String>, current_step: u32, total_steps: u32 },
}

/// Blocking. Starts a rebase of the current branch onto `onto_name` (local
/// shorthand "main" OR remote-tracking shorthand "origin/main").
pub fn rebase_branch(workdir: &Path, onto_name: &str) -> Result<RebaseOutcome, AppError>;

/// Blocking. Resumes the paused rebase at `workdir`: commits the current
/// (resolved) operation, then replays until done or the next conflict.
pub fn rebase_continue(workdir: &Path) -> Result<RebaseOutcome, AppError>;

/// Blocking. Skips the current operation (`git rebase --skip` semantics:
/// discards its changes, does NOT commit it) and resumes.
pub fn rebase_skip(workdir: &Path) -> Result<RebaseOutcome, AppError>;

/// Blocking. Aborts the paused rebase, restoring the original HEAD/branch and
/// worktree (destructive — the UI confirms first; backend guard §4.4).
pub fn rebase_abort(workdir: &Path) -> Result<(), AppError>;
```

### 3.1 `rebase_branch` preconditions (exact order, cheap first — ALL before any mutation)

Identical shape to merge §4.1; error strings are rebase-specific for the oracle.

1. `open_workdir_repo(workdir)` (the shared `NO_SEARCH` open — same helper merge/opstate use).
2. `repo.state() != RepositoryState::Clean` → `AppError::OperationInProgress("an operation is
   already in progress — commit or abort it first")` (byte-identical to merge — one shared string).
3. `read_head_info(&repo)`: `unborn` → `AppError::Git("cannot rebase: the repository has no commits
   yet")`; `detached` → `AppError::Git("cannot rebase: HEAD is detached")`. Keep `head_branch`
   (`head.branch_name`, `Some` past this point).
4. Resolve onto: `repo.find_branch(onto_name, Local)`, else `find_branch(onto_name, Remote)`, else →
   `AppError::BranchNotFound("branch '<onto_name>' not found (local or remote-tracking)")`
   (identical phrasing to merge §4.1.4). Rebasing onto the current branch falls out as `UpToDate`.
5. **Dirty-index guard (locked):** `index.has_conflicts()` OR
   `index.write_tree_to(&repo)? != head_commit.tree_id()` → `AppError::Git("cannot rebase: your
   index contains uncommitted changes — commit or unstage them first")`.
   **AMENDED 2026-07-29 (tester finding, §11.11 — diverges from merge §4.1.5):** unlike merge,
   a rebase requires a **clean worktree**. Both libgit2 (`repo.rebase()` → `Git("unstaged changes
   exist in workdir")`) and `git rebase` (`error: cannot rebase: You have unstaged changes`) refuse
   to START a rebase with ANY unstaged change — not just an overwriting one. Bonsai matches the CLI
   exactly: unstaged changes are NOT allowed, and they surface as `AppError::Git` (the generic
   libgit2 message) at the `repo.rebase()` call in §3.3, NOT as `CheckoutConflict`. The earlier
   "unstaged worktree changes / untracked files are ALLOWED … fail later as CheckoutConflict" text
   (copied from the more-permissive merge contract) was WRONG for rebase and is retracted. The
   `CheckoutConflict` mapping in §3.2 (FF) / §3.3 (real replay) still stands for the rare case
   libgit2 does raise `ErrorCode::Conflict`, but the common dirty-worktree rejection is `Git`.
6. Identity EARLY: `resolve_signature(&repo.config()?.snapshot()?)?` — replay commits, so
   `ConfigMissing` must surface before the worktree is touched. This signature is the **committer**
   for every replayed commit and for `rebase.finish()` (§3.6).

### 3.2 `rebase_branch` analysis + fast paths

```text
head_commit = repo.head()?.peel_to_commit()
onto_commit = onto_branch.get().peel_to_commit()

# up-to-date: onto == HEAD, or onto is an ancestor of HEAD.
mb = repo.merge_base(head_commit.id(), onto_commit.id())      # Result; unrelated histories -> Err
if onto_commit.id() == head_commit.id() || mb.ok() == Some(onto_commit.id()):
    return UpToDate

# fast-forward: HEAD is an ancestor of onto -> rebasing yields onto itself.
if mb.ok() == Some(head_commit.id()):
    # identical safe-FF recipe as merge §4.2 / remote.rs pull_ff (checkout BEFORE set_target):
    obj = repo.find_object(onto_commit.id(), None)
    checkout_tree(obj, CheckoutBuilder::new().safe())
        on ErrorCode::Conflict -> AppError::CheckoutConflict(
            "cannot rebase: local changes would be overwritten. Commit or discard them first.")
    repo.find_reference("refs/heads/<head_branch>")
        .set_target(onto_commit.id(), "rebase <onto_name>: fast-forward")
    return FastForwarded { branch: head_branch, to: onto_commit.id().to_string() }

# else: real rebase (§3.3). If merge_base errored (unrelated histories) fall through to
# repo.rebase(), which surfaces the git2 error as AppError::Git — do NOT special-case it.
```

### 3.3 `rebase_branch` real-replay (driving the git2 Rebase API)

```text
head_ac = repo.reference_to_annotated_commit(&repo.head()?)      # the branch being rebased
onto_ac = repo.reference_to_annotated_commit(onto_branch.get())  # target = upstream = onto
# Release every repo-lifetime borrow (index, *_commit, onto_branch) before the &mut drive.

opts = RebaseOptions::new()          # ON-DISK (do NOT set .inmemory(true)) — conflicts must land
                                     # in the worktree with libgit2's default <<<<<<< markers so
                                     # conflict.rs sees them, and state must persist across IPC calls.

match repo.rebase(Some(&head_ac), Some(&onto_ac), Some(&onto_ac), Some(&mut opts)):
    Err(e):
        # repo.rebase() may have written rebase-merge state before its initial
        # checkout failed. GUARANTEE: a failed START leaves state Clean.
        cleanup_failed_start(&repo, head_commit_id)               # §3.5
        if e.code() == Conflict -> CheckoutConflict("cannot rebase: local changes would be
                                     overwritten. Commit or discard them first.")   # same string as §3.2 FF
        else -> e.into()
    Ok(mut rebase):
        match run_rebase_loop(workdir, &repo, &mut rebase, &sig):   # §3.4
            Ok(Completed { head, steps }) -> Rebased { branch: head_branch, head, steps }
            Ok(Paused { paths, current_step, total_steps }) ->
                Conflicts { paths, current_step, total_steps }
            Err(e):
                let _ = rebase.abort();                             # failed START -> Clean
                if e.code() == Conflict -> CheckoutConflict(same string)
                else -> e.into()
```

**`repo.rebase` argument semantics (locked, matches plain `git rebase <onto_name>`):**
`branch = Some(head_ac)` (rebase HEAD's branch), `upstream = Some(onto_ac)` and `onto = Some(onto_ac)`
(replay the range `onto_name..HEAD` onto `onto_name`). Using the same annotated commit for both
`upstream` and `onto` is exactly what `git rebase <onto_name>` does.

### 3.4 Shared drive loop + step counters (locked)

```text
enum DriveResult { Completed { head: Oid, steps: u32 }, Paused { paths, current_step, total_steps } }

fn steps(rebase) -> (u32, u32):
    current = rebase.operation_current().map(|c| c as u32 + 1).unwrap_or(0)  # 1-based, == git msgnum
    total   = rebase.len() as u32
    (current, total)

fn run_rebase_loop(workdir, repo, rebase, committer) -> Result<DriveResult, AppError>:
    loop:
        match rebase.next():                       # applies the next patch to index+worktree
            None -> break                          # plan exhausted
            Some(Err(e)) -> return Err(e)          # caller decides abort-vs-keep (§3.3/§3.6)
            Some(Ok(_op)) ->                        # op.kind() is always Pick for plain rebase
                if repo.index()?.has_conflicts():
                    (cur, total) = steps(rebase)
                    paths = list_conflicts(workdir)?.map(|c| c.path)   # sorted, §P3c-3
                    return Ok(Paused { paths, current_step: cur, total_steps: total })
                match commit_current(rebase, committer):               # §3.6 empty-drop handling
                    Ok(()) -> continue
                    Err(e) -> return Err(e)
    # plan exhausted -> finalize
    rebase.finish(Some(committer))?                # reattaches HEAD to the branch, moves branch ref
    head = repo.head()?.peel_to_commit()?.id()
    Ok(Completed { head, steps: rebase.len() as u32 })
```

`Paused` KEEPS the on-disk rebase state — that is the whole point; it is NOT an error.

### 3.5 `cleanup_failed_start` (START-only; guarantee: state Clean, worktree untouched-or-restored)

```text
fn cleanup_failed_start(repo, head_oid):
    # Best-effort, in order; each step ignores its own error.
    if let Ok(mut r) = repo.open_rebase(None): let _ = r.abort();     # normal path: full restore
    if repo.state() != Clean:                                          # belt-and-suspenders
        let _ = repo.cleanup_state();
        if let Ok(tree) = repo.find_commit(head_oid)?.tree():
            let mut idx = repo.index()?; let _ = idx.read_tree(&tree); let _ = idx.write();
```

Document in the rustdoc: a failed `rebase_branch` (START) restores `RepositoryState::Clean` and does
not leave a half-initialized rebase. Because §3.1.5 allows a dirty worktree, the ONLY START failure
that can touch the worktree is the initial base checkout, which fails atomically as
`CheckoutConflict` before any commit is rewritten.

### 3.6 Committer/author identity during replay (locked, matches `git rebase`)

- **Author preserved** from each original commit: `rebase.commit(None /* author */, committer, None
  /* message */)` — `None` author tells libgit2 to reuse the operation's original author (name,
  email, AND author time).
- **Committer = the current signature** resolved in §3.1.6 (`resolve_signature`, `now()` timestamp).
- **Message preserved** (`None`).
- `rebase.finish(Some(committer))` for the reflog identity.

```text
fn commit_current(rebase, committer) -> Result<(), AppError>:
    match rebase.commit(None, committer, None):
        Ok(_)                                  -> Ok(())
        Err(e) if e.code() == Applied          -> Ok(())   # the pick became EMPTY -> DROP it
        Err(e)                                 -> Err(e.into())
```

**Empty/already-applied picks are dropped** (the `Applied` arm) — matching default `git rebase`,
which omits commits whose changes are already present on the new base. (Flagged §11.1.)

### 3.7 `rebase_continue`

```text
repo = open_workdir_repo(workdir)
if !is_rebase_state(repo.state()) -> NoOperationInProgress("no rebase in progress")   # §3.10
sig = resolve_signature(&repo.config()?.snapshot()?)?
if repo.index()?.has_conflicts():
    n = repo.index()?.conflicts()?.count()
    -> UnresolvedConflicts("cannot continue: <n> unresolved conflict(s) remain")
let mut rebase = repo.open_rebase(None)?          # valid: Bonsai STARTED this rebase (§4 note)
commit_current(&mut rebase, &sig)?                # commit the CURRENT resolved op (empty -> drop)
                                                  # HARD error here: return Err, do NOT abort (§3.9)
head_branch = read_head_from_rebase(&repo)        # head-name file minus refs/heads/, best-effort
match run_rebase_loop(workdir, &repo, &mut rebase, &sig):
    Ok(Completed { head, steps }) -> Rebased { branch: head_branch, head, steps }
    Ok(Paused { .. })             -> Conflicts { .. }
    Err(e)                        -> Err(e)        # leave state intact (§3.9)
```

`read_head_from_rebase`: read `rebase-merge/head-name` (strip `refs/heads/`); if unreadable, after a
completed `finish()` fall back to `read_head_info(&repo).branch_name.unwrap_or_default()`. The
`branch` field is display-only (toasts), so a best-effort value is acceptable — never error on it.

### 3.8 `rebase_skip` (`git rebase --skip` semantics, locked)

```text
repo = open_workdir_repo(workdir)
if !is_rebase_state(repo.state()) -> NoOperationInProgress("no rebase in progress")
sig = resolve_signature(&repo.config()?.snapshot()?)?
let mut rebase = repo.open_rebase(None)?
# Discard the current op's changes (conflicts + partial worktree) by hard-resetting to the
# in-progress rebase HEAD (identity on committed content — only clears the working state), so the
# next patch applies cleanly. This is the libgit2 equivalent of `git rebase --skip`: DO NOT commit
# the current op; just resume the plan.
head_commit = repo.head()?.peel_to_commit()?
repo.reset(head_commit.as_object(), Hard, None)?
match run_rebase_loop(workdir, &repo, &mut rebase, &sig):
    Ok(Completed { head, steps }) -> Rebased { branch: read_head_from_rebase, head, steps }
    Ok(Paused { .. })             -> Conflicts { .. }
    Err(e)                        -> Err(e)        # leave state intact (§3.9)
```

Semantics note (rustdoc): unlike `rebase_continue`, skip does NOT call `commit_current` for the
current operation — that operation is dropped from the result. The hard reset targets the current
(detached) rebase HEAD, so already-committed replayed commits are untouched; only the skipped op's
uncommitted/conflicted changes are discarded. (Validated against `git rebase --skip` in §9.5;
flagged §11.2.)

### 3.9 Partial-rebase safety (locked — the core "never corrupt" rule)

- **START** failures abort → state Clean (§3.3, §3.5). This is safe because no user resolution work
  exists yet.
- **CONTINUE / SKIP** hard errors **return the error and LEAVE the on-disk rebase state intact**
  (paused). They MUST NOT call `rebase.abort()` and MUST NOT call `repo.cleanup_state()` — the user
  has invested conflict-resolution work and can retry Continue or explicitly Abort. This honours
  "on-disk rebase state is re-opened per IPC call, never held in memory, no `cleanup_state`
  mid-rebase". Every continue/skip/abort re-opens via `repo.open_rebase(None)` and drops the handle
  before the command returns — no `Rebase` handle survives across IPC calls.

### 3.10 `rebase_abort` + helpers

```text
fn is_rebase_state(s: RepositoryState) -> bool:
    matches!(s, RebaseMerge | Rebase | RebaseInteractive)

pub fn rebase_abort(workdir):
    repo = open_workdir_repo(workdir)
    if !is_rebase_state(repo.state()) -> NoOperationInProgress("no rebase in progress")
    let mut rebase = repo.open_rebase(None)?      # git2 error (e.g. CLI apply-backend rebase it
                                                  # cannot load) surfaces as AppError::Git -> toast
    rebase.abort()?                               # restores original HEAD/branch + worktree
    Ok(())
```

`rebase_abort` is destructive (rewinds the branch and worktree to pre-rebase state, discarding
resolutions) → UI `ConfirmDialog` (§8) + the backend commit guard (§4.4) prevent an accidental plain
commit mid-rebase.

Register `pub mod rebase;` in `git/mod.rs`.

---

## 4. Commands (`src-tauri/src/commands.rs` + `lib.rs generate_handler!`)

All follow the established pattern verbatim (identical to P3c §6): `#[tauri::command] async fn
x(state: tauri::State<'_, AppState>, ...) -> Result<T, AppError>` delegating to a runtime-free
`x_inner(state: &AppState, ...)` that does `current_repo_path(state)?` then
`spawn_blocking(move || git::rebase::...)` with the standard join-error map. None emit
`repo-changed`; the frontend refetches imperatively after every successful mutation.

```rust
/// Starts a rebase of the current branch onto `onto` (local or remote-tracking
/// shorthand). P3d §3. Errors: operationInProgress | branchNotFound
/// | checkoutConflict | configMissing | git | noRepo.
pub async fn rebase_branch(.., onto: String) -> Result<RebaseOutcome, AppError>;   // rebase::rebase_branch

/// Resumes a paused rebase (commits the resolved op, replays on). P3d §3.7.
/// Errors: noOperationInProgress | unresolvedConflicts | configMissing | git | noRepo.
pub async fn rebase_continue(..) -> Result<RebaseOutcome, AppError>;               // rebase::rebase_continue

/// Skips the current operation and resumes. P3d §3.8.
/// Errors: noOperationInProgress | configMissing | git | noRepo.
pub async fn rebase_skip(..) -> Result<RebaseOutcome, AppError>;                   // rebase::rebase_skip

/// Aborts a paused rebase (worktree-destructive — the UI confirms first). P3d §3.10.
/// Errors: noOperationInProgress | git | noRepo.
pub async fn rebase_abort(..) -> Result<(), AppError>;                             // rebase::rebase_abort
```

- Arg key is `onto` (camelCase already). Tauri invoke: `invoke('rebase_branch', { onto })`.
- Add all four to `generate_handler![]` in `lib.rs` (after the P3c entries).
- Extend the `commands.rs` test module with `rebase_commands_require_an_open_repo` (exact shape of
  `merge_commands_require_an_open_repo`, P3c §6): assert all four `*_inner` return
  `AppError::NoRepo` with no repo open (`rebase_branch_inner(&state, "main".into())`,
  `rebase_continue_inner`, `rebase_skip_inner`, `rebase_abort_inner`).

### 4.4 Backend commit guard — already in place (confirm, no change)

`create_commit` in `git/commit.rs` already guards `repo.state() != Clean → OperationInProgress(...)`
as its first check (added P3c §4.5). Because `is_rebase_state(..)` states are all non-`Clean`, a
plain commit mid-rebase is already blocked at the backend — **no new guard needed.** Confirm this in
the §9 test matrix (a `create_commit` during a paused rebase → `OperationInProgress`). The frontend
also blocks CommitBox during a rebase (§8.5), mirroring merge's belt-and-suspenders reasoning.

---

## 5. `src-tauri/src/error.rs` — NO new variants

Reuse existing variants only. Mapping (locked):

| Situation | Variant | kind |
|---|---|---|
| START while state != Clean | `OperationInProgress` | `operationInProgress` |
| continue/skip/abort with no rebase | `NoOperationInProgress` | `noOperationInProgress` |
| continue while index still conflicted | `UnresolvedConflicts` | `unresolvedConflicts` |
| initial base checkout would overwrite local changes | `CheckoutConflict` | `checkoutConflict` |
| onto not found (local or remote) | `BranchNotFound` | `branchNotFound` |
| git identity unset | `ConfigMissing` | `configMissing` |
| unborn / detached / dirty-index / open_rebase load failure / unrelated histories | `Git` | `git` |
| any other git2::Error | `Git` (via `From`) | `git` |
| nothing open | `NoRepo` | `noRepo` |

No `error.rs` edit is expected. If a genuinely new case appears, STOP and flag the orchestrator
before adding a variant.

---

## 6. (reserved — see §4)

---

## 7. P3d-b — IPC mirror + mock

### 7.1 `src/ipc/types.ts` additions (verbatim)

```ts
export type RebaseOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string }
  | { kind: 'rebased'; branch: string; head: string; steps: number }
  | { kind: 'conflicts'; paths: string[]; currentStep: number; totalSteps: number };
```

No `AppError.kind` additions (all reused). `IpcApi` gains (mirror the Rust doc-comment error lists):

```ts
/** Start a rebase of the current branch onto `onto` (local or remote-tracking
 *  shorthand). Rejects operationInProgress | branchNotFound | checkoutConflict
 *  | configMissing | git | noRepo. */
rebaseBranch(onto: string): Promise<RebaseOutcome>;
/** Resume a paused rebase. Rejects noOperationInProgress | unresolvedConflicts
 *  | configMissing | git | noRepo. */
rebaseContinue(): Promise<RebaseOutcome>;
/** Skip the current operation and resume. Rejects noOperationInProgress
 *  | configMissing | git | noRepo. */
rebaseSkip(): Promise<RebaseOutcome>;
/** Abort a paused rebase (worktree-destructive). Rejects noOperationInProgress
 *  | git | noRepo. */
rebaseAbort(): Promise<void>;
```

`src/ipc/tauri.ts`: four invoke wrappers —
```ts
rebaseBranch(onto) { return invoke<RebaseOutcome>('rebase_branch', { onto }); }
rebaseContinue()   { return invoke<RebaseOutcome>('rebase_continue'); }
rebaseSkip()       { return invoke<RebaseOutcome>('rebase_skip'); }
rebaseAbort()      { return invoke<void>('rebase_abort'); }
```
Add `RebaseOutcome` to the type import block. Re-export `RebaseOutcome` from `src/ipc/index.ts`.

### 7.2 Mock (`src/ipc/mock.ts`) — stateful twin, `?op=rebase`

Extend the existing `?op=merge` seeding machinery. Compose with `?fixture=` (read from
`window.location.search` at module init, like `?op=merge`).

**Seed (`seedOpState` extension).** Add a branch for `op === 'rebase'`:
```ts
opState = {
  kind: 'rebase',
  headName: 'feature/topic',
  onto: '00'.repeat(20),           // fixture full oid of the onto tip (base row 0's oid)
  currentStep: 2,
  totalSteps: 3,
};
conflicts = [
  { path: 'src/auth.ts', kind: 'bothModified', hasBase: true, hasOurs: true, hasTheirs: true },
];
conflictTexts.set('src/auth.ts', { path:'src/auth.ts', kind:'bothModified',
  binary:false, tooLarge:false, missing:false, text: MERGE_AUTH_TEXT });  // reuse the marker fixture
mockStatus.conflicted = conflicts.map((c) => ({ path: c.path, origPath: null, status: 'conflicted' }));
```
Keep the existing `?op=merge` branch and the default `{ kind:'none' }` branch untouched. `?op=rebase`
starts pre-seeded in a paused conflicted rebase at step 2/3 (the "resolve → continue finishes" demo);
`rebaseBranch` is the clean-rebase demo path.

**New mock state:** none beyond the shared `opState`/`conflicts`/`conflictTexts`. Add a module-level
`let rebaseTotalSteps = 0;` set on start/seed so `rebaseContinue`/`rebaseSkip` can advance
`currentStep` toward it. (Or read `opState.totalSteps` when `opState.kind === 'rebase'`.)

**Methods (all with the standard `delay(150)`):**
- `rebaseBranch(onto)` → if `opState.kind !== 'none'` reject `operationInProgress`. Else the
  clean-rebase demo: set `mockHeadOid = randomOid()`, prepend 3 plain `MockCommit`s (summaries
  `pick: replayed 3/2/1` — no `mergeParentBase`) via `mockCommits.unshift(...)` so the rebased
  commits appear atop the graph, bump the head branch's `ahead` if it has an upstream, and resolve
  `{ kind: 'rebased', branch: mockHeadBranch, head: mockHeadOid, steps: 3 }`.
- `rebaseContinue()` → if `opState.kind !== 'rebase'` reject `noOperationInProgress`; if
  `conflicts.length > 0` reject `unresolvedConflicts` (message `cannot continue: <n> unresolved
  conflict(s) remain`). Else advance: `currentStep += 1`. If `currentStep > totalSteps` → **finish**:
  `opState = { kind:'none' }`, `mockStatus.conflicted = []`, `mockHeadOid = randomOid()`, prepend
  `totalSteps` plain `MockCommit`s (the replayed commits), and resolve `{ kind:'rebased', branch:
  mockHeadBranch, head: mockHeadOid, steps: totalSteps }`. Otherwise (still more steps and the demo
  seeds no further conflict) treat the remaining steps as clean and **finish** in this same call
  (single continue completes the seeded 2/3 → done story); resolve `rebased`. Update the stored
  `opState.currentStep` before finishing so a mid-call `getOpState` would reflect progress.
- `rebaseSkip()` → same guards as continue (minus the conflict guard — skip is allowed WITH
  conflicts present). Drop the current op: `conflicts = []`, `conflictTexts = new Map()`,
  `mockStatus.conflicted = []`, advance `currentStep`; then finish exactly like continue's finish
  branch (resolve `rebased`, prepend replayed commits, clear op).
- `rebaseAbort()` → if `opState.kind !== 'rebase'` reject `noOperationInProgress`. Else restore:
  `opState = { kind:'none' }`, `conflicts = []`, `conflictTexts = new Map()`,
  `mockStatus.conflicted = []`. Do NOT prepend any commit (abort rewinds).
- `getOpState`/`listConflicts`/`getConflict`/`resolveConflict` are **unchanged** — they already
  serve `opState`/`conflicts`/`conflictTexts` generically, so they work for rebase verbatim
  (confirming the operation-agnostic reuse). `resolveConflict`'s `deletedByThem`-specific branch is
  a no-op for the `bothModified` rebase seed.

`openRepo`'s path-change reset already calls `seedOpState()` — extend it only via the `seedOpState`
edit above (no new call site).

---

## 8. P3d-c — Frontend

### 8.1 `src/components/OpBanner.tsx` — actionable rebase mode (replace the info strip)

Replace the current non-actionable `op.kind !== 'merge'` info branch: **rebase becomes actionable**;
cherry-pick / revert keep the informational strip.

```ts
export interface OpBannerProps {
  op: RepoOpState;
  conflictCount: number;              // remaining conflicts (gates Continue/Commit merge)
  mutating: boolean;
  onCommitMerge(): void;              // merge mode
  onRebaseContinue(): void;           // rebase mode
  onRebaseSkip(): void;               // rebase mode
  onAbort(): void;                    // merge & rebase — opens the ConfirmDialog (App owns it)
}
```

Render logic:
- `op.kind === 'none'` → `null` (unchanged).
- `op.kind === 'merge'` → unchanged (title `Merging <incoming>`, `[Commit merge]` + `[Abort]`).
- `op.kind === 'rebase'` → actionable strip (same `op-banner` recipe, `role="status"`):
  - Title: `Rebasing {op.headName ?? 'HEAD'}`.
  - Subline: `step {op.currentStep}/{op.totalSteps} — {conflictCount > 0 ? `${conflictCount}
    conflict(s) remaining` : 'all conflicts resolved'}`. Guard `totalSteps === 0` (unreadable) by
    hiding the `step n/m` fragment.
  - Buttons (in `op-banner-actions`):
    - **[Continue]** `btn-primary op-banner-btn`, `disabled={conflictCount > 0 || mutating}`.
    - **[Skip]** `btn-secondary op-banner-btn`, `disabled={mutating}` (skip is allowed WITH
      conflicts present — dropping the offending commit is a valid resolution).
    - **[Abort]** `btn-danger op-banner-btn`, `disabled={mutating}`.
- `op.kind === 'cherryPick' | 'revert'` → the existing informational strip (drop `rebase` from
  `EXTERNAL_OP_LABEL`; keep `cherryPick`/`revert`).

### 8.2 Conflict rows + marker view — reuse verbatim (no component edits)

During a rebase the StatusPanel conflict section, the `[ours]`/`[theirs]`/`[resolved]` row buttons,
and the `conflict:<path>` DiffOverlay marker view are the **exact same** components and code paths as
P3c §8.2/§8.3. The only wiring change is that App populates `conflicts` during a rebase too (§8.4).
No edits to `StatusPanel.tsx`, `DiffOverlay.tsx`, or the `overlayMeta` `conflict:` branch.

### 8.3 (reserved — merged into §8.2)

### 8.4 App wiring (`src/App.tsx`)

- **`refetchOpState` (edit one line):** fetch `listConflicts()` when
  `op.kind === 'merge' || op.kind === 'rebase'` (currently merge-only). The existing
  `conflict:<path>` slot-lifecycle logic below it is unchanged (it keys off the conflict list, not
  the op kind).
- **Handlers** (standard `setMutating(true) → try ipc → await refreshAll() → pushToast → finally
  setMutating(false)` shape, exactly like `handleMergeBranch` P3c §8.4):
  - `handleRebaseBranch(onto: string)` — toast per outcome: `upToDate` → info `Already up to date
    with <onto>`; `fastForwarded` → success `Fast-forwarded onto <onto>`; `rebased` → success
    `Rebased onto <onto> (<steps> commit(s))`; `conflicts` → info (a normal pause, NOT error)
    `Rebase paused at step <currentStep>/<totalSteps>: <n> conflict(s) to resolve`. AppError → sticky
    error toast (existing `pushToast('error', …)` path). `await refreshAll()` after every branch.
  - `handleRebaseContinue()` — `rebased` → success `Rebase complete`; `conflicts` → info `Rebase
    paused at step <currentStep>/<totalSteps>`. Errors → error toast. `await refreshAll()`.
  - `handleRebaseSkip()` — same outcome→toast mapping as continue. `await refreshAll()`.
  - `handleRebaseAbort()` — called ONLY after the ConfirmDialog confirms; success toast `Rebase
    aborted`; `await refreshAll()`.
- **Generalized abort ConfirmDialog:** the existing single `abortConfirmOpen` dialog is reused for
  both ops; App picks copy + action from `opState.kind`:
  - merge → title `Abort merge?`, body `This restores the files touched by the merge to their
    pre-merge state. Conflict resolutions will be lost.`, confirm `Abort merge`, action
    `handleAbortMerge`.
  - rebase → title `Abort rebase?`, body `This restores your branch and working tree to their
    pre-rebase state. Replayed commits and conflict resolutions will be lost.`, confirm `Abort
    rebase`, action `handleRebaseAbort`.
  Wire `OpBanner.onAbort` → `setAbortConfirmOpen(true)` for both kinds; the dialog's `onConfirm`
  branches on `opState.kind`.
- **OpBanner props:** pass `onRebaseContinue={() => void handleRebaseContinue()}` and
  `onRebaseSkip={() => void handleRebaseSkip()}` alongside the existing `onCommitMerge`/`onAbort`.
- **CommitBox during rebase:** no change — the existing `blocked={opActive && opState.kind !==
  'merge'}` already blocks the box (placeholder `An operation is in progress`) during a rebase.
  Rebase commits happen via Continue, never the box.

### 8.5 Op-active gating — reuse P3c §8.5 verbatim

`opActive = opState.kind !== 'none'` already gates everything. During a rebase (an `opActive`
state): Sidebar checkout / delete / create-branch / **merge** / **rebase** actions disabled; Pull /
Push disabled (`canPullPush` already `&& !opActive`); plain commit blocked (§8.4). **Fetch stays
enabled** (always safe). **Stage/unstage of non-conflicted files stays enabled** (the resolve flow
relies on the same index mechanics — identical to merge). No new gating logic; the rebase affordance
(§8.6) threads the same `opActive`/`busy` used by the merge affordance.

### 8.6 Sidebar rebase affordance (edit `src/components/Sidebar.tsx`)

Mirror the merge affordance (`⇋`). Add a **fourth** hover icon-button on every non-HEAD local branch
row AND every remote branch row: glyph `⤵`, `title` = `Rebase <currentBranch> onto <name>`,
`aria-label` identical. New `SidebarProps` field `onRebaseBranch(name: string): void`, threaded to
`BranchRow`/`RemoteRow` (add `onRebase(name)` alongside `onMerge(name)`).

Gating (identical to the merge affordance): hidden when `currentBranch === null` (detached/unborn);
`disabled={actionsDisabled}` (`busy || opActive`). Click → `onRebaseBranch(name)` → `handleRebaseBranch`.
**No ConfirmDialog on rebase START** — start is non-destructive (preconditions guarantee nothing
user-authored is lost; the worst case is a paused state that Abort — which IS confirmed — undoes),
same reasoning as merge §11.3. App passes `onRebaseBranch={(name) => void handleRebaseBranch(name)}`.

---

## 9. Testing contract (`src-tauri/tests/rebase_cli.rs`; tester implements after P3d-a)

Conventions identical to P3c §9: scratch repos under `D:\Temp\bonsai-scratch` via the shared
`init_repo` helper (`core.autocrlf=false`, identity set), `TMP`/`TEMP` = `D:\Temp`, every Bonsai
result compared to a **git CLI twin repo** built by identical setup. **Commit-oid comparison rule
(locked, mirrors merge §9):** committer time = `now()`, so replayed commit OIDs differ from the twin.
Compare **tree oid, author (name/email/time — preserved), message, and parent topology** per
replayed commit, and the final HEAD tree oid — NOT commit oids.

1. **Clean linear rebase.** `main` linear; `topic` branched earlier with 2 commits touching disjoint
   files; `main` advanced. `rebase_branch("main")` → `Rebased { branch:"topic", steps:2, .. }`.
   Twin: `git checkout topic && git rebase main`. Assert: final HEAD tree oid identical; each
   replayed commit's tree oid + author identity/time + message identical to the twin's, in order;
   linear parent chain rooted at `main`'s tip; `repo.state()` Clean; no `rebase-merge` dir.
2. **Up-to-date.** onto is an ancestor of HEAD (`rebase_branch(<older-ref>)`) → `UpToDate`, HEAD
   unmoved (branch oid unchanged). Also: rebasing the current branch onto itself → `UpToDate`.
3. **Fast-forward.** `topic` is strictly behind `main` (HEAD ancestor of onto), linear →
   `FastForwarded { to }` == twin's post-`git rebase main` HEAD; no rewritten commits; worktree/tree
   == onto's tree.
4. **Conflict → paused.** `topic` and `main` edit the same line → guaranteed conflict on replay.
   `rebase_branch("main")` → `Conflicts { paths, current_step, total_steps }`. Assert: `paths` set
   == twin's `git rebase main` conflicted set (`git diff --name-only --diff-filter=U`);
   `repo.state()` is a rebase state; `read_op_state` returns `Rebase { head_name: Some("topic"),
   onto: Some(<main-tip-oid>), current_step, total_steps }` with `current_step`/`total_steps`
   matching the outcome (§2 assertion); the worktree file carries `<<<<<<<`/`=======`/`>>>>>>>`
   markers (`get_conflict` non-empty).
5. **continue after resolving.** From (4): resolve every conflict via `resolve_conflict` (Ours/
   Theirs/hand-edit+MarkResolved across cells), then `rebase_continue()` → `Rebased`. Twin: resolve
   identically + `git rebase --continue`. Assert final HEAD tree oid + per-commit tree/author/message
   match the twin; `repo.state()` Clean. Also assert `UnresolvedConflicts` when `rebase_continue` is
   called with a conflict still present.
6. **skip.** Multi-step rebase where the FIRST replayed commit conflicts; `rebase_skip()` drops it
   and completes the rest. Twin: `git rebase --skip`. Assert final HEAD tree oid + remaining
   commits' trees/messages match the twin (the skipped commit absent from both); `repo.state()`
   Clean.
7. **abort restores original HEAD byte-identically. AMENDED 2026-07-29 (tester, §11.11).** Rebase
   requires a clean worktree (§3.1.5 amendment), so the original "unstaged edit survives START" premise
   (copied from merge §9.8) is invalid for rebase. The suite pins the REAL contract in
   `dirty_start_is_rejected_like_the_cli_then_abort_restores_byte_identically`: (a) a dirty START
   (unstaged edit present) is rejected, `repo.state()` stays Clean, nothing is left behind, the
   unstaged edit is untouched, and the `git rebase` twin rejects identically; (b) abort from a CLEAN
   start restores HEAD/index/worktree byte-identically; (c) `NoOperationInProgress` when `rebase_abort`
   runs with no rebase.
8. **Remote-tracking onto.** Local bare `file://` remote (M6 pattern), fetch, `rebase_branch(
   "origin/main")` → replays onto the remote-tracking ref; parents/trees match twin `git rebase
   origin/main`. (No network.)
9. **Precondition matrix.** detached HEAD → `Git("…HEAD is detached")`; unborn HEAD →
   `Git("…no commits yet")`; staged/dirty index → `Git("…index contains uncommitted changes…")`;
   rebase-during-op (start a merge first, then `rebase_branch`) → `OperationInProgress`; unknown
   onto → `BranchNotFound`; missing identity → `ConfigMissing` (surfaces BEFORE the worktree is
   touched — assert state still Clean).
10. **Backend commit guard.** During a paused rebase, `create_commit(..)` → `OperationInProgress`
    (confirms §4.4). `rebase_continue`/`rebase_skip` with no rebase → `NoOperationInProgress`.
11. **Empty-pick drop.** `topic` contains a commit whose change is already on `main` (cherry-picked
    earlier). `rebase_branch("main")` completes; that commit is DROPPED from the result (assert the
    replayed-commit count == twin's `git rebase main`, which also drops it). (Confirms §3.6/§11.1.)

Module unit tests (senior-dev, P3d-a) in `rebase.rs`:
- `wire_shapes_are_camel_case_tagged` for all four `RebaseOutcome` variants (same recipe as
  `merge.rs`): `upToDate` → `{"kind":"upToDate"}`; `fastForwarded` → `{"kind":"fastForwarded",
  "branch":…,"to":…}`; `rebased` → `{"kind":"rebased","branch":…,"head":…,"steps":…}`; `conflicts` →
  `{"kind":"conflicts","paths":[…],"currentStep":…,"totalSteps":…}`.
- `rebase_preconditions_on_fresh_repo`: unborn → `Git`; `rebase_continue`/`rebase_skip`/
  `rebase_abort` on a Clean repo → `NoOperationInProgress`.

---

## 10. Acceptance

**AI gate (orchestrator verifies):**
- `cargo test` green incl. `rebase_cli.rs`; `cargo clippy -- -D warnings`; `pnpm build` green after
  every sub-increment.
- Byte-exact oracle assertions in §9 pass (per-commit tree oids, author identity/time, messages,
  conflicted sets, final HEAD tree, abort byte-identity).
- Harness (`VITE_MOCK_IPC=1`): `?op=rebase` shows the OpBanner (`Rebasing feature/topic`, `step 2/3`,
  Continue disabled while 1 conflict remains, Skip + Abort enabled); the shared conflict row shows
  the kind badge + ours/theirs/resolved buttons; clicking the row opens the read-only marker view
  with highlighted `<<<<<<<` lines; resolving the conflict enables Continue; Continue clears the
  banner and prepends the replayed commits atop the graph; Skip completes without requiring
  resolution; Abort shows the `Abort rebase?` ConfirmDialog and clears state; during the rebase,
  CommitBox is blocked and checkout/delete/create/merge/rebase/pull/push controls are disabled while
  Fetch + non-conflicted stage/unstage stay enabled; the Sidebar `⤵` affordance
  (`Rebase <current> onto <name>`) starts the clean-rebase demo (`rebaseBranch`) and prepends the
  rebased commits; plain (no `?op`) harness unchanged (regression).
- `src/ipc/mock.ts` compiles and implements all four new methods statefully.

**USER CHECKPOINT (native `pnpm tauri dev` — never self-declared):**
1. Clean rebase on a real scratch repo: rebase a linear topic onto an advanced main from the Sidebar
   `⤵` affordance → `git log` shows the topic replayed on main's tip, correct author dates preserved,
   new committer dates; `git status` clean.
2. Conflicting rebase: banner shows `step n/m`; resolve via ours/theirs/hand-edit; marker view is
   readable; Continue advances/completes; Skip drops the offending commit; final history matches
   expectation.
3. Abort a paused rebase: branch + worktree return to the pre-rebase state; an unrelated uncommitted
   edit made before the rebase survives.
4. Rebase onto `origin/<branch>` from the Remotes sidebar section works after a Fetch.

---

## 11. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Empty/already-applied picks are DROPPED** during replay (the `rebase.commit()` → `ErrorCode::
   Applied` arm, §3.6), matching default `git rebase`. The alternative (`--keep-empty`) is not v1.
   Oracle test §9.11 pins this against the twin.
2. **`rebase_skip` = hard-reset-to-current-HEAD + resume-without-commit** (§3.8), the libgit2
   equivalent of `git rebase --skip`. The reset is identity on committed content (only clears the
   skipped op's working/conflict state); already-replayed commits are untouched. Validated against
   `git rebase --skip` in §9.6. If the oracle reveals a discrepancy, the fallback is to expose Skip
   as disabled in v1 and flag — but the recipe is expected to hold.
3. **Merge commits in the replayed range are FLATTENED/DROPPED** exactly like default `git rebase`
   (no `--rebase-merges` in v1). The oracle's clean/conflict tests use linear ranges for
   byte-exactness; a range containing a merge is compared to the twin `git rebase` result only for
   completion + topology, not asserted commit-by-commit. **Recommend** confirming with the user that
   v1 rebase linearizes merges (GitKraken's default "rebase" does the same).
4. **Rebasing onto a remote-tracking branch does NOT auto-fetch** — it uses the local
   remote-tracking ref, identical to merge (Fetch first is the user's job). Consistent with the
   locked pull/merge philosophy.
5. **CONTINUE / SKIP never abort or `cleanup_state` on a hard error** (§3.9) — they return the error
   and leave the paused rebase intact so the user can retry or explicitly Abort. Only START failures
   restore Clean (no resolution work exists yet). This is the concrete realization of "a partially
   applied rebase is never corrupted".
6. **`open_rebase(None)` is used ONLY by the mutating commands**, never by `read_op_state` (§2) —
   the refresh-batch read stays on plain file reads so a CLI-started apply-backend rebase never
   errors the batch. A CLI apply-backend rebase that `open_rebase` cannot load surfaces as an
   `AppError::Git` toast when the user clicks Continue/Skip/Abort (honest, not silent).
7. **`RebaseOutcome::Rebased` carries `{ branch, head, steps }`, NOT `onto`** — `onto` is not
   cheaply available on the continue/skip paths, and the frontend already knows the onto name from
   the affordance click; `branch`/`head`/`steps` are available on all three entry points.
8. **`fastForwarded` / `upToDate` fast paths for rebase** mirror `git rebase`'s own fast-forward and
   "up to date" behaviors (§3.2) — a rebase whose result equals a fast-forward does not rewrite
   commits, and an already-based branch does nothing. `merge.ff`-style config is not consulted (v1).
9. **One generalized Abort ConfirmDialog** serves both merge and rebase (App branches copy + action
   on `opState.kind`, §8.4) — avoids a second near-identical dialog. Rebase START is not confirmed
   (§8.6), same reasoning as merge.
10. **No new `error.rs` variants** — every rebase failure maps onto an existing variant (§5). If a
    genuinely new case appears, STOP and flag before adding one.
11. **AMENDED 2026-07-29 (tester findings during P3d-a verification):**
    (a) **Rebase requires a clean worktree** (§3.1.5 / §9.7 amendments) — unlike merge, ANY unstaged
    change makes both libgit2 and `git rebase` refuse to start; it surfaces as `AppError::Git`, not
    `CheckoutConflict`. Bonsai matches the CLI. The permissive "unstaged allowed" prose copied from
    merge was retracted; the abort test now pins dirty-START rejection + clean-START abort restoration.
    (b) **`rebase_skip` recipe correction** — the original §3.8 `repo.reset(HEAD, Hard)` corrupted
    `.git/rebase-merge` when skipping the FIRST (not-yet-committed) op and returned an empty branch
    display name on the later-op path. Fixed to a lighter index-`read_tree(HEAD)` + force
    `checkout_index` that discards the current op's changes without rewriting HEAD/reflog or disturbing
    the rebase metadata; both first-op and later-op skip now match `git rebase --skip`, and the branch
    name is correct. The §3.9 safety rule is unchanged (skip still never aborts/cleanup_states on a
    hard error). The previously-ignored `skip_first_op_is_broken_known_bug` oracle test is un-ignored.
