# P3c — Merge + Conflict Handling: Implementation Contract

Status: authoritative for P3c. Scope: merge a chosen local or remote-tracking branch into the
current branch; auto-commit clean merges; file-level conflict resolution (ours / theirs / mark
resolved) with a read-only marker view; commit-the-merge or abort. Builds on `M3-commit.md`
(`resolve_signature`, commit step order), `M5-branches.md` (branch resolution, safe checkout),
`M6-remotes.md` (`pull_ff` FF pattern in `git/remote.rs`), `P3a` (diffSlot / DiffOverlay
center-pane machinery), `P3b` (tree-grouped lists — conflict rows reuse the flat row recipe, NOT
tree grouping; conflicts are few and flat is clearer).

**Reuse mandate:** `opstate.rs` and `conflict.rs` are designed as SHARED modules — P3d rebase will
drive the exact same `RepoOpState` wire type, conflict list, marker view, and resolution commands.
Nothing in them may be merge-specific except documented behavior notes.

Invariants (unchanged): Rust owns all Git logic; IPC carries compact precomputed data; commands =
req/resp; git2 under `spawn_blocking` via the established `*_inner` runtime-free pattern in
`commands.rs`; `src/ipc/mock.ts` updated with EVERY IpcApi change; destructive ops (abort merge)
require explicit `ConfirmDialog` confirmation.

---

## 1. Scope split (sub-increments)

| # | Increment | Content |
|---|---|---|
| 1 | **P3c-a** | Backend: `git/opstate.rs`, `git/conflict.rs`, `git/merge.rs`, `error.rs` variants, 7 new commands in `commands.rs` + `lib.rs` registration, module unit tests (wire shapes, matrix cells). Read §2–§6. |
| 2 | **P3c-b** | IPC mirror: `src/ipc/types.ts`, `src/ipc/tauri.ts`, `src/ipc/index.ts`, stateful mock (`?op=merge`). Read §7. |
| 3 | **P3c-c** | Frontend: `OpBanner.tsx`, StatusPanel conflict rows + `conflict:<path>` overlay, Sidebar merge action, App wiring + op-active gating. Read §8. |

Each is a self-contained fresh-context senior-dev pass (this file + the exact source paths).
Tester (after P3c-a lands): §9 CLI-oracle suites `src-tauri/tests/merge_cli.rs` +
`src-tauri/tests/conflict_cli.rs`.

---

## 2. `src-tauri/src/git/opstate.rs` — repository operation state (SHARED with P3d)

```rust
//! Detects an in-progress repository operation (merge / rebase / cherry-pick /
//! revert) from repo.state() + on-disk metadata. Pure git2, no Tauri types.

use std::path::Path;
use crate::error::AppError;

/// Wire: `{ "kind": "none" } | { "kind": "merge", "incoming": ..., "message": ... } | ...`
/// The Rebase variant is fully shaped NOW so P3d does not change the wire type;
/// P3c only populates its fields best-effort (see below).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RepoOpState {
    None,
    Merge {
        /// Human name of what is being merged, e.g. "feature/login" or
        /// "origin/main" — parsed from MERGE_MSG (see derivation below);
        /// falls back to the 7-char short oid of MERGE_HEAD.
        incoming: String,
        /// Full prepared merge message (MERGE_MSG contents, trailing
        /// whitespace trimmed). The frontend prefills the commit box with it.
        message: String,
    },
    Rebase {
        head_name: Option<String>,   // .git/rebase-merge/head-name minus "refs/heads/", best-effort
        onto: Option<String>,        // .git/rebase-merge/onto (full oid), best-effort
        current_step: u32,           // msgnum, 0 when unreadable
        total_steps: u32,            // end, 0 when unreadable
    },
    CherryPick,
    Revert,
}

/// Blocking. Maps repo.state():
///   Clean                              -> None
///   Merge                              -> Merge { .. } (see derivation)
///   RebaseMerge | RebaseInteractive
///     | Rebase                         -> Rebase { .. }
///   CherryPick | CherryPickSequence    -> CherryPick
///   Revert | RevertSequence            -> Revert
///   anything else (Bisect, ApplyMailbox, ...) -> None  (Bonsai has no UI for
///     them; get_op_state must never error the refresh batch for exotic states)
pub fn read_op_state(workdir: &Path) -> Result<RepoOpState, AppError>;
```

**Merge-variant derivation (locked):**
1. `message` = `fs::read_to_string(git_path("MERGE_MSG"))`, `\r\n`→`\n`, `trim_end()`. Missing/
   unreadable file → empty string (never an error — a foreign tool may have removed it).
2. `incoming`: first line of `message` matched against the two prefixes Bonsai itself writes
   (§4.3): `Merge branch '<name>'` and `Merge remote-tracking branch '<name>'` — extract
   `<name>` between the first pair of single quotes on that line (works for CLI-started merges
   too, which use the same phrasing). No quoted name found → fall back to the short (7-char) oid
   of the FIRST `MERGE_HEAD` entry via `repo.mergehead_foreach` (first callback wins, return
   `false` to stop). No MERGE_HEAD readable → `"(unknown)"`.
3. `git_path(x)` = `repo.path().join(x)` (repo.path() is the `.git` dir — works with worktrees
   where these files live in the per-worktree gitdir).

**Rebase-variant derivation (P3c best-effort, P3d refines):** read
`repo.path().join("rebase-merge")/{head-name,onto,msgnum,end}` (then `rebase-apply` as fallback
for msgnum/end only); any unreadable file → the documented default. Do NOT use
`repo.open_rebase()` in P3c (it errors on rebases not started by libgit2; plain file reads never
do).

Register `pub mod opstate;` in `git/mod.rs`.

---

## 3. `src-tauri/src/git/conflict.rs` — conflict listing + resolution (SHARED with P3d)

```rust
//! Index-conflict listing, read-only marker view, and file-level resolution.
//! Operation-agnostic: works identically during merge (P3c) and rebase (P3d).

use std::path::Path;
use crate::error::AppError;

/// Byte cap for the marker view. Above it: too_large=true, text="".
pub const MAX_CONFLICT_BYTES: u64 = 1_048_576; // 1 MiB — same all-or-nothing spirit as diff.rs MAX_FILE_DIFF_LINES

/// Derived from which index stages exist (see matrix §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictKind {
    BothModified, BothAdded, DeletedByUs, DeletedByThem,
    AddedByUs, AddedByThem, BothDeleted,
}

/// One conflicted path. `path` is repo-relative, forward slashes: prefer the
/// OURS side's path, else THEIRS, else ANCESTOR (rename conflicts can differ;
/// v1 surfaces one row per index conflict record under that preferred path).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictEntry {
    pub path: String,
    pub kind: ConflictKind,
    pub has_base: bool,
    pub has_ours: bool,
    pub has_theirs: bool,
}

/// Read-only working-tree view of one conflicted file, markers included.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConflictFile {
    pub path: String,
    pub kind: ConflictKind,
    /// NUL byte within the first 8000 bytes -> binary, text="".
    pub binary: bool,
    /// File size > MAX_CONFLICT_BYTES -> too_large, text="".
    pub too_large: bool,
    /// Worktree file missing (e.g. deletedBy* kinds) -> true, text="".
    pub missing: bool,
    /// Lossy UTF-8 of the worktree file WITH the <<<<<<< ======= >>>>>>> markers.
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ConflictResolution { Ours, Theirs, MarkResolved }

/// Blocking. All current index conflicts via Index::conflicts(), sorted by
/// path ascending (byte-wise). Empty vec when none / when state is Clean.
pub fn list_conflicts(workdir: &Path) -> Result<Vec<ConflictEntry>, AppError>;

/// Blocking. Marker view of one CURRENTLY CONFLICTED path. Non-conflicted
/// path -> AppError::Git("path '<p>' has no conflict").
pub fn get_conflict(workdir: &Path, path: &str) -> Result<ConflictFile, AppError>;

/// Blocking. Resolves ONE path per the matrix in §3.2, leaving the index
/// entry at stage 0 (or removed) and the worktree consistent with it.
/// Non-conflicted path -> AppError::Git("path '<p>' has no conflict").
/// Validates `path` with stage.rs::validate_rel_path first (same guard as
/// stage/unstage — no absolute/.. escapes).
pub fn resolve_conflict(workdir: &Path, path: &str, resolution: ConflictResolution)
    -> Result<(), AppError>;
```

### 3.1 `ConflictKind` derivation (locked)

From stage presence `(base, ours, theirs)` of the index conflict record:

| base | ours | theirs | kind |
|---|---|---|---|
| ✓ | ✓ | ✓ | `BothModified` |
| ✗ | ✓ | ✓ | `BothAdded` |
| ✓ | ✗ | ✓ | `DeletedByUs` |
| ✓ | ✓ | ✗ | `DeletedByThem` |
| ✗ | ✓ | ✗ | `AddedByUs` |
| ✗ | ✗ | ✓ | `AddedByThem` |
| ✓ | ✗ | ✗ | `BothDeleted` |
| ✗ | ✗ | ✗ | impossible per libgit2 — `debug_assert!` + treat as `BothDeleted` |

### 3.2 Resolution matrix (locked — every cell)

Let *write(side)* = read the side's blob from the ODB, `fs::write` it to the worktree path
(creating parent dirs), set the file mode from the side's `IndexEntry.mode` (on Windows: mode is
recorded in the index by `add_path` per core.filemode — no chmod call; document, don't fight it),
then `index.add_path(path)` (clears all conflict stages, records stage 0), `index.write()`.
Let *delete()* = `fs::remove_file` if the worktree file exists (missing file is fine), then
`index.remove_path(path)` (also clears conflict stages), `index.write()`.

| kind | `Ours` | `Theirs` | `MarkResolved` |
|---|---|---|---|
| `BothModified` | write(ours) | write(theirs) | add worktree file (below) |
| `BothAdded` | write(ours) | write(theirs) | add worktree file |
| `DeletedByUs` | **delete()** (keep our deletion) | write(theirs) | add-or-delete |
| `DeletedByThem` | write(ours) | **delete()** (accept their deletion) | add-or-delete |
| `AddedByUs` | write(ours) | **delete()** (theirs has no file) | add-or-delete |
| `AddedByThem` | **delete()** | write(theirs) | add-or-delete |
| `BothDeleted` | delete() | delete() | add-or-delete |

`MarkResolved` semantics ("add-or-delete", locked): if the worktree file exists →
`index.add_path` (stages the hand-edited content as the resolution — the user is trusted; leftover
`<<<<<<<` markers are NOT rejected, same as `git add`); if it does not exist →
`index.remove_path` (the user resolved by deleting the file by hand). Never an error for a missing
file.

`write(side)` when that side does not exist in the matrix is impossible by construction (the table
only writes existing sides); `debug_assert!` it.

Register `pub mod conflict;` in `git/mod.rs`.

---

## 4. `src-tauri/src/git/merge.rs` — merge engine

```rust
//! Merge a local or remote-tracking branch into the current branch.
//! Clean merges auto-commit; conflicts pause into RepoOpState::Merge.
//! Pure git2, no Tauri types, no network (merging origin/x uses the local
//! remote-tracking ref — Fetch first is the user's job, same as GitKraken).

use std::path::Path;
use crate::error::AppError;
use crate::git::commit::CommitResult;

/// Wire: tagged "kind", camelCase (same recipe as PullResult).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MergeOutcome {
    /// Incoming is already reachable from HEAD. Nothing changed.
    UpToDate,
    /// HEAD branch fast-forwarded to `to` (full oid). No merge commit.
    FastForwarded { branch: String, to: String },
    /// Clean merge, auto-committed. `oid` = the new 2-parent merge commit.
    Merged { oid: String },
    /// Conflicts recorded in index + worktree; MERGE_HEAD/MERGE_MSG written;
    /// repo paused in state Merge. Sorted conflicted paths (same set
    /// list_conflicts returns).
    Conflicts { paths: Vec<String> },
}

/// Blocking. Merges `branch_name` (local shorthand "feature/x" OR
/// remote-tracking shorthand "origin/main") into the current branch.
pub fn merge_branch(workdir: &Path, branch_name: &str) -> Result<MergeOutcome, AppError>;

/// Blocking. Finalizes a paused merge as a 2(+)-parent commit.
pub fn commit_merge(workdir: &Path, message: &str) -> Result<CommitResult, AppError>;

/// Blocking. Aborts a paused merge; restores pre-merge index + the worktree
/// files the merge touched (approximate `git reset --merge` — see §4.5).
pub fn abort_merge(workdir: &Path) -> Result<(), AppError>;
```

### 4.1 `merge_branch` preconditions (exact order, cheap first — all checked BEFORE anything mutates)

1. `open_repo_at` — same `NO_SEARCH` open as remote.rs (extract a shared helper or duplicate the
   3-liner; duplication is the current codebase norm, keep it).
2. `repo.state() != RepositoryState::Clean` → `AppError::OperationInProgress("an operation is
   already in progress — commit or abort it first")`.
3. `read_head_info`: `unborn` → `AppError::Git("cannot merge: the repository has no commits
   yet")`; `detached` → `AppError::Git("cannot merge: HEAD is detached")` (same phrasing family
   as pull/push in remote.rs).
4. Resolve incoming: `repo.find_branch(name, Local)`, else `repo.find_branch(name, Remote)`,
   else → `AppError::BranchNotFound("branch '<name>' not found (local or remote-tracking)")`.
   Merging the current branch by name → falls out as `UpToDate` naturally (no special case).
5. **Dirty-index guard (git2 merge semantics, locked):** staged changes present — i.e.
   `index.write_tree_to(&repo)?` != HEAD tree id, or `index.has_conflicts()` — →
   `AppError::Git("cannot merge: your index contains uncommitted changes — commit or unstage
   them first")`. This mirrors `git merge`'s refusal on an index that differs from HEAD.
   **Unstaged worktree changes and untracked files are ALLOWED** (git-like): they only fail
   later if the merge would overwrite them, surfacing as `CheckoutConflict` in §4.2 step 3 —
   in which case nothing is left behind (cleanup guarantee below).
6. Identity check EARLY: `resolve_signature(&repo.config()?.snapshot()?)?` — a clean merge
   auto-commits, so `ConfigMissing` must surface before the worktree is touched, not after.

### 4.2 `merge_branch` algorithm

```text
annotated = repo.reference_to_annotated_commit(incoming_branch.get())
(analysis, _pref) = repo.merge_analysis(&[&annotated])

if analysis.is_up_to_date():        return UpToDate

if analysis.is_fast_forward():      # merge.ff config NOT consulted in v1 — FF whenever possible
    # identical safe-FF recipe as remote.rs pull_ff (checkout BEFORE set_target):
    obj = repo.find_object(annotated.id(), None)
    checkout_tree(obj, CheckoutBuilder::safe())
        on ErrorCode::Conflict -> AppError::CheckoutConflict(
            "cannot merge: local changes would be overwritten. Commit or discard them first.")
    repo.find_reference("refs/heads/<head_branch>")
        .set_target(annotated.id(), "merge <name>: fast-forward")
    return FastForwarded { branch: head_branch, to: annotated.id().to_string() }

# analysis.is_normal():
message = prepared_merge_message(name, incoming_is_remote)          # §4.3
merge_opts  = MergeOptions::new()                                    # defaults: find_renames on
checkout    = CheckoutBuilder::safe(); checkout.allow_conflicts(true)
              .conflict_style_merge(true)                            # <<<<<<< ======= >>>>>>> markers
repo.merge(&[&annotated], Some(&mut merge_opts), Some(&mut checkout))
    on ANY Err e:
        # libgit2 may have written MERGE_HEAD/MERGE_MSG/MERGE_MODE before the
        # checkout failed. Guarantee: a failed merge_branch leaves state Clean.
        repo.cleanup_state();  index.read_tree(HEAD tree); index.write()
        if e.code == Conflict -> AppError::CheckoutConflict(same message as FF path)
        else -> e.into()

index = repo.index()
if index.has_conflicts():
    paths = list_conflicts(workdir) paths                            # §3
    message += "\n\nConflicts:\n" + paths.map(|p| "\t" + p).join("\n")
    fs::write(git_path("MERGE_MSG"), message + "\n")                 # overwrite libgit2's — deterministic (§4.3)
    return Conflicts { paths }

# clean: auto-commit (git-like), then cleanup
fs::write(git_path("MERGE_MSG"), message + "\n")                     # keep on-disk msg == committed msg until cleanup
result = commit_merge_inner(repo, &message)                          # §4.4 core, shared
return Merged { oid: result.oid }
```

### 4.3 Prepared MERGE_MSG (locked, byte-exact for the oracle tests)

- Local branch: `Merge branch '<name>'`
- Remote-tracking branch: `Merge remote-tracking branch '<name>'` (e.g. `'origin/main'`)
- Conflicted merges append `\n\nConflicts:\n\t<path>` per path (sorted), exactly like git.
- No `into <branch>` suffix (git only adds it when merging into a non-default branch; matching
  that heuristic requires guessing the default branch — skipped in v1, flagged in §11.4).

### 4.4 `commit_merge`

Step order (cheap checks first, mirrors `create_commit`):
1. `repo.state() != RepositoryState::Merge` → `AppError::NoOperationInProgress("no merge in
   progress")`.
2. `index.has_conflicts()` → `AppError::UnresolvedConflicts("cannot commit: <n> unresolved
   conflict(s) remain")`.
3. Normalize message exactly like `create_commit` (CRLF/CR → `\n`, trim); empty →
   `AppError::EmptyMessage`.
4. `sig = resolve_signature(&repo.config()?.snapshot()?)?` (reuse from `git::commit`).
5. Parents: HEAD commit first, then EVERY `MERGE_HEAD` oid in file order via
   `repo.mergehead_foreach` (v1 UI only produces one, but octopus state written by the CLI must
   not be silently truncated). Zero mergeheads → `AppError::Git("MERGE_HEAD missing")`.
6. `tree = repo.find_tree(index.write_tree()?)`; NO nothing-to-commit check (an empty-diff merge
   commit is legitimate — it records ancestry).
7. `repo.commit(Some("HEAD"), &sig, &sig, &format!("{msg}\n"), &tree, &parents)`.
8. `repo.cleanup_state()?` (removes MERGE_HEAD/MERGE_MSG/MERGE_MODE, returns state to Clean).
9. Return `CommitResult` (reuse the git::commit struct: oid, summary = first line, branch).

The clean-merge auto-commit path in §4.2 calls the same core (steps 3–9) with the prepared
message, skipping step 1–2 (state IS Merge at that point, conflicts already known zero).
Implement as a private `fn finalize_merge_commit(repo, msg) -> Result<CommitResult, AppError>`
shared by both entry points.

### 4.5 `abort_merge` (destructive — UI confirms; approximate `git reset --merge`, locked)

Plain `reset --hard` would also nuke pre-merge unstaged edits to files the merge never touched
(which §4.1.5 deliberately allows). Instead:

```text
1. repo.state() != Merge -> AppError::NoOperationInProgress("no merge in progress")
2. head_tree = HEAD commit tree
3. touched = every path where the current index differs from head_tree:
     diff_tree_to_index(head_tree, index, DiffOptions default) -> old+new paths,
     UNION all conflicted paths from Index::conflicts() (ours/theirs/base paths)
4. cb = CheckoutBuilder::force(); for p in touched: cb.path(p)
   cb.remove_untracked(false)                    # only listed paths are forced
   repo.checkout_tree(head_tree.as_object(), cb) # restores/deletes exactly the merge-touched files
5. index.read_tree(&head_tree); index.write()    # drop all conflict + merged entries
6. repo.cleanup_state()
```

Guarantee to document in the rustdoc: files with pre-merge unstaged edits that the merge did NOT
touch survive an abort byte-identically. Files the merge touched are restored to HEAD (a pre-merge
unstaged edit to a merge-touched file cannot exist — it would have failed §4.2 with
`CheckoutConflict` before any state was written).

**Backend commit guard (in-scope one-liner):** `create_commit` in `git/commit.rs` gains, as its
FIRST check, `repo.state() != Clean → AppError::OperationInProgress("an operation is in progress
— use 'Commit merge' or abort it")`. The frontend also disables CommitBox (§8.5), but a
frontend-only gate would let a race create a 1-parent commit mid-merge and silently drop
MERGE_HEAD ancestry. Everything else in the existing commit flow is untouched. (Flagged §11.1.)

Register `pub mod merge;` in `git/mod.rs`.

---

## 5. `src-tauri/src/error.rs` — additive variants

```rust
// Add to AppError (with matching kind()/message() arms and doc-comment kind list):
#[error("{0}")] OperationInProgress(String),   // kind "operationInProgress"
#[error("{0}")] NoOperationInProgress(String), // kind "noOperationInProgress"
#[error("{0}")] UnresolvedConflicts(String),   // kind "unresolvedConflicts"
```

Reused (NOT duplicated): `CheckoutConflict` (merge/FF would overwrite local changes),
`BranchNotFound`, `ConfigMissing`, `EmptyMessage`, `Git`, `NoRepo`.

---

## 6. Commands (`src-tauri/src/commands.rs` + `lib.rs generate_handler!`)

All follow the established pattern verbatim: `#[tauri::command] async fn x(state:
tauri::State<'_, AppState>, ...) -> Result<T, AppError>` delegating to a runtime-free
`x_inner(state: &AppState, ...)` that does `current_repo_path(state)?` then
`tauri::async_runtime::spawn_blocking(move || git::...)` with the standard join-error map. None
emit `repo-changed` — the frontend refetches imperatively after every successful mutation.

```rust
/// Current operation state. In the frontend refresh batch (§8.4).
/// Errors: noRepo | git.
pub async fn get_op_state(..) -> Result<RepoOpState, AppError>;          // opstate::read_op_state

/// Merge a local or remote-tracking branch into the current branch.
/// Errors: operationInProgress | branchNotFound | checkoutConflict
/// | configMissing | git | noRepo.
pub async fn merge_branch(.., name: String) -> Result<MergeOutcome, AppError>;

/// Finalize a paused merge. Errors: noOperationInProgress
/// | unresolvedConflicts | emptyMessage | configMissing | git | noRepo.
pub async fn commit_merge(.., message: String) -> Result<CommitResult, AppError>;

/// Abort a paused merge (worktree-destructive for merge-touched files).
/// Errors: noOperationInProgress | git | noRepo.
pub async fn abort_merge(..) -> Result<(), AppError>;

/// All current index conflicts, path-ascending. Errors: noRepo | git.
pub async fn list_conflicts(..) -> Result<Vec<ConflictEntry>, AppError>;

/// Read-only marker view of one conflicted file. Errors: noRepo | git.
pub async fn get_conflict(.., path: String) -> Result<ConflictFile, AppError>;

/// Resolve one path (§3.2 matrix). Errors: noRepo | git | invalidName
/// (validate_rel_path).
pub async fn resolve_conflict(.., path: String, resolution: ConflictResolution)
    -> Result<(), AppError>;
```

Add all seven to `generate_handler![]` in `lib.rs`. Extend the existing `commands.rs` test module
with a `merge_commands_require_an_open_repo` test (same shape as the M3–M6 NoRepo tests) covering
all seven inners.

---

## 7. P3c-b — IPC mirror + mock

### 7.1 `src/ipc/types.ts` additions (verbatim)

```ts
export type RepoOpState =
  | { kind: 'none' }
  | { kind: 'merge'; incoming: string; message: string }
  | {
      kind: 'rebase';
      headName: string | null;
      onto: string | null;
      currentStep: number;
      totalSteps: number;
    }
  | { kind: 'cherryPick' }
  | { kind: 'revert' };

export type ConflictKind =
  | 'bothModified'
  | 'bothAdded'
  | 'deletedByUs'
  | 'deletedByThem'
  | 'addedByUs'
  | 'addedByThem'
  | 'bothDeleted';

export interface ConflictEntry {
  path: string;
  kind: ConflictKind;
  hasBase: boolean;
  hasOurs: boolean;
  hasTheirs: boolean;
}

export interface ConflictFile {
  path: string;
  kind: ConflictKind;
  binary: boolean;
  tooLarge: boolean;
  /** Worktree file missing (deletion conflicts). text is '' when true. */
  missing: boolean;
  /** Worktree contents INCLUDING <<<<<<< ======= >>>>>>> markers. */
  text: string;
}

export type ConflictResolution = 'ours' | 'theirs' | 'markResolved';

export type MergeOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string }
  | { kind: 'merged'; oid: string }
  | { kind: 'conflicts'; paths: string[] };
```

`AppError.kind` union gains `'operationInProgress' | 'noOperationInProgress' |
'unresolvedConflicts'`.

`IpcApi` gains (mirror the Rust doc-comment error lists):

```ts
getOpState(): Promise<RepoOpState>;
mergeBranch(name: string): Promise<MergeOutcome>;
commitMerge(message: string): Promise<CommitResult>;
abortMerge(): Promise<void>;
listConflicts(): Promise<ConflictEntry[]>;
getConflict(path: string): Promise<ConflictFile>;
resolveConflict(path: string, resolution: ConflictResolution): Promise<void>;
```

`src/ipc/tauri.ts`: seven invoke wrappers (`invoke('merge_branch', { name })`, etc. —
snake_case command names, camelCase arg keys per Tauri convention already in the file). Re-export
new types from `src/ipc/index.ts`.

### 7.2 Mock (`src/ipc/mock.ts`) — stateful twin, `?op=merge`

- New module-level mutable state (same pattern as the existing mutable status/graph state):
  ```ts
  let opState: RepoOpState = { kind: 'none' };
  let conflicts: ConflictEntry[] = [];
  let conflictTexts: Map<string, ConflictFile>;
  ```
- `?op=merge` (read once at module init, composable with `?fixture=`): seeds
  `opState = { kind: 'merge', incoming: 'feature/login', message: "Merge branch
  'feature/login'\n\nConflicts:\n\tsrc/auth.ts\n\tREADME.md" }`, `conflicts` = two entries
  (`src/auth.ts` → `bothModified`, `README.md` → `deletedByThem`), matching
  `status.conflicted` entries, and `conflictTexts` with realistic marker text for `src/auth.ts`
  (a ~20-line file with one `<<<<<<< HEAD / ======= / >>>>>>> feature/login` block) and
  `missing: false`; `README.md` view has `missing: false`, ours-side text without markers
  (deletedByThem: worktree keeps ours).
- Behavior (all with the standard `delay(150)`):
  - `getOpState` → current `opState`.
  - `listConflicts` → current `conflicts`. `getConflict(p)` → map lookup, else reject
    `{ kind: 'git', message: ... }`.
  - `resolveConflict(p, r)` → remove `p` from `conflicts` + `status.conflicted`; for `theirs`
    on `deletedByThem` also mark it deleted from the mock file lists; resolves void.
  - `commitMerge(msg)` → if `conflicts.length > 0` reject `{ kind: 'unresolvedConflicts', ... }`;
    else set `opState = { kind: 'none' }`, clear conflicted status, and PREPEND a new merge node
    to the mock graph fixture (2 parents = previous head node + a fixture feature tip; summary =
    first line of `msg`) so the harness visibly shows the merge commit — faithful-twin rule.
  - `abortMerge` → `opState = none`, clear `conflicts` + `status.conflicted`.
  - `mergeBranch(name)` → when `opState.kind !== 'none'` reject `operationInProgress`; else
    resolve `{ kind: 'merged', oid: <fixture oid> }` and prepend a merge node (clean-merge demo
    path); `?op=merge` is the conflicted demo, started pre-seeded rather than via mergeBranch.

---

## 8. P3c-c — Frontend

### 8.1 `src/components/OpBanner.tsx` (new)

```ts
export interface OpBannerProps {
  op: RepoOpState;              // render null when kind === 'none' (parent may also skip mounting)
  conflictCount: number;        // remaining conflicts (drives Commit-merge enablement)
  mutating: boolean;
  onCommitMerge(): void;        // parent opens the merge-commit flow (§8.3)
  onAbort(): void;              // parent opens the ConfirmDialog
}
export function OpBanner(props: OpBannerProps): JSX.Element | null;
```

- Rendered ABOVE the right panel's content (first child inside the right-panel column in
  `App.tsx`, above StatusPanel/CommitPanel), full panel width. Recipe: `--bg-1` background,
  1px `--accent`-tinted top/bottom border, 8px padding — visually a status strip, not a toast.
- Merge mode content: bold line `Merging feature/login` (from `op.incoming`); subline
  `<n> conflict(s) remaining` when `conflictCount > 0`, else `All conflicts resolved`; two
  buttons: **[Commit merge]** (primary; `disabled` while `conflictCount > 0 || mutating`) and
  **[Abort]** (danger-styled secondary, disabled while `mutating`).
- Rebase/cherryPick/revert kinds (started externally via the CLI): render an informational
  banner `A <op> is in progress — finish or abort it in your terminal.` with NO buttons (P3d
  makes rebase actionable). This keeps the app honest instead of pretending state is clean.

### 8.2 StatusPanel conflicts section (edit `src/components/StatusPanel.tsx`)

- The existing Conflicts section rows become actionable. New props threaded from App:
  ```ts
  conflicts: ConflictEntry[];                    // authoritative kind per path
  onResolveConflict(path: string, r: ConflictResolution): void;
  onToggleConflictView(path: string): void;      // diffSlot key `conflict:<path>`
  diffSlot / mutating: already threaded
  ```
- Per row: path (existing recipe), a small kind badge (`both modified`, `deleted by them`, ... —
  lowercase spaced text of `ConflictKind`), then three 22px text-buttons in the row-hover action
  slot (same affordance as stage/unstage buttons): `[ours]` `[theirs]` `[resolved]` with
  `title` tooltips ("Take our version", "Take their version", "Mark resolved (I edited the
  file)"). All disabled while `mutating`. No confirm dialog — resolution is re-doable until
  Commit merge (re-running the merge after abort restores conflicts), and per-file confirms
  would make a 10-file conflict unbearable.
- Row click (not on a button) → `onToggleConflictView(path)` — expands the read-only marker
  view exactly like workdir diff rows expand diffs, routed through the SAME diffSlot/DiffOverlay
  machinery with key `conflict:<path>`. Conflict rows always render FLAT (no P3b tree grouping)
  — conflicts are few; keep the section simple.

### 8.3 Conflict marker view in DiffOverlay (edit `src/App.tsx`, `src/components/DiffOverlay.tsx`)

- `diffSlot` key namespace gains `conflict:<path>`. `overlayMeta` in App handles the prefix:
  `{ path, origPath: null, status: 'conflicted', kind: 'conflict' }`.
- Content decision (locked): **plain `<pre class="conflict-view">`**, NOT DiffView — marker text
  is a single file body, not hunks; reusing DiffView would require fabricating fake hunks/line
  numbers. Styling: monospace, `--bg-0`, 12px, horizontal scroll; lines starting with `<<<<<<<`,
  `=======`, `>>>>>>>` get a `--accent`-tinted background via cheap line-split rendering (one
  `<div>` per line; conflict files are ≤ 1 MiB capped and typically small — acceptable).
- Data flow mirrors the workdir-file-diff slot: on slot open App calls `ipc.getConflict(path)`
  (request-id guarded like `fileDiffReqId`); `binary`/`tooLarge`/`missing` render the same
  placeholder pattern DiffView uses ("Binary file", "File too large to display", "File was
  deleted"). After a successful `resolveConflict` on the open path, App collapses the slot
  (entry no longer conflicted — same rule as the existing stage/unstage slot-collapse logic).

### 8.4 App wiring (`src/App.tsx`)

- New state `const [opState, setOpState] = useState<RepoOpState>({ kind: 'none' });` and
  `const [conflicts, setConflicts] = useState<ConflictEntry[]>([]);` with a `refetchOpState`
  helper (request-id guarded) that fetches `getOpState()` and, when `kind === 'merge'`,
  `listConflicts()` in the same pass (else sets `[]`).
- `refreshAll` and `openPath` add `refetchOpState()` to the existing `Promise.all` batch;
  the clear-path calls a `clearOpState()` (`{kind:'none'}`, `[]`).
- Handlers (standard `setMutating(true) → try ipc → await refreshAll() → pushToast → finally
  setMutating(false)` shape, exactly like `handlePull`):
  - `handleMerge(name)` — toast per outcome: upToDate `Already up to date with <name>` (info);
    fastForwarded `Fast-forwarded to <name>` (success); merged `Merged <name>` (success);
    conflicts `Merge paused: <n> conflict(s) to resolve` (info, NOT error — it's a normal
    pause). AppError → sticky error toast (existing `errorMessage` path).
  - `handleResolveConflict(path, r)`, `handleCommitMerge()` (sends the CURRENT merge-message
    text — see below), `handleAbortMerge()` behind a `ConfirmDialog`: title `Abort merge?`,
    body `This restores the files touched by the merge to their pre-merge state. Conflict
    resolutions will be lost.`, confirm label `Abort merge` (danger). Reuse the Sidebar's
    dialog-open lift (`onDialogOpenChange`) so global shortcuts go inert.
- **Merge message editing (locked):** while `opState.kind === 'merge'`, the existing CommitBox
  is repurposed, not hidden: it is prefilled ONCE per merge (when opState transitions into
  merge) with `opState.message`, its button label becomes `Commit merge`, and submitting calls
  `commitMerge(text)` instead of `commit(text)`; disabled while `conflictCount > 0`. This gives
  message editing for free and is why OpBanner's [Commit merge] simply focuses/triggers the
  same submit path.

### 8.5 Op-active gating (frontend; backend guard per §4.5)

While `opState.kind !== 'none'`: disable normal commit submission (CommitBox is in merge mode or,
for non-merge ops, disabled with placeholder `An operation is in progress`), disable Sidebar
checkout, delete, create-branch, merge actions, and Pull/Push buttons (Fetch stays enabled —
fetching is always safe). Stage/unstage of non-conflicted files stays ENABLED during a merge
(git allows amending the merge's index; the resolve buttons rely on the same index mechanics).

### 8.6 Sidebar merge affordance (edit `src/components/Sidebar.tsx`)

- Every non-HEAD local branch row and every remote branch row gains a third hover icon-button
  (same 22px icon-button recipe as the existing checkout/delete actions): glyph `⇋`, `title`
  = `Merge <name> into <currentBranch>`. Hidden/disabled when: no current branch (detached/
  unborn), `mutating`, or `opState.kind !== 'none'` (thread an `opActive: boolean` prop).
- Click → `onMergeBranch(name)` (new prop) → `handleMerge`. No ConfirmDialog on start (§11.3).

---

## 9. Testing contract (tester implements; P3c-a must make these possible)

Conventions: existing `src-tauri/tests/` style (see `remote_cli.rs`, `m6_adversarial.rs`) —
scratch repos under `D:\Temp\bonsai-scratch` via the shared `init_repo` helper
(`core.autocrlf=false`, identity set), `TMP`/`TEMP` = `D:\Temp`, every Bonsai result compared
against a `git` CLI **twin repo** built by identical setup commands.

`src-tauri/tests/merge_cli.rs`:
1. **Clean merge** — diverged branches touching disjoint files. Assert `Merged{oid}`; twin runs
   `git merge <name>`; compare: HEAD tree oid byte-identical, both parents identical (order:
   HEAD first), commit message identical (`Merge branch '<name>'` + `\n`), `repo.state()` Clean,
   MERGE_HEAD absent.
2. **Fast-forward** — assert `FastForwarded{to}` == twin's post-`git merge` HEAD; no new commit.
3. **Up-to-date** — merging an ancestor → `UpToDate`, HEAD unmoved.
4. **Remote-tracking merge** — local bare `file://` remote (M6 pattern), fetch, merge
   `origin/topic`: message is `Merge remote-tracking branch 'origin/topic'`, parents match twin.
5. **Guaranteed conflict** (same line edited both sides) — assert `Conflicts{paths}` set ==
   twin's `git merge` conflicted set (`git diff --name-only --diff-filter=U`); `repo.state()` ==
   Merge; MERGE_MSG contains the `Conflicts:` block; `read_op_state` returns
   `Merge{incoming: "<name>"}`.
6. **Preconditions** — detached HEAD, unborn HEAD, staged change, merge-during-merge
   (`OperationInProgress`), unknown branch (`BranchNotFound`), unstaged edit to a merge-touched
   file (`CheckoutConflict`, then state still Clean and worktree byte-identical to before).
7. **commit_merge** — after resolving all conflicts, commit; compare tree oid + parents +
   message to the twin resolved identically (twin: same resolutions via `git checkout --ours/
   --theirs` + `git add`, then `git commit --no-edit` with the same message text). Also:
   `UnresolvedConflicts` when one conflict remains; `NoOperationInProgress` with no merge.
8. **abort_merge** — start conflicted merge with an additional pre-merge unstaged edit to an
   untouched file; abort; assert state Clean, index tree == HEAD tree, conflicted files restored
   to pre-merge worktree bytes, the unrelated unstaged edit SURVIVES byte-identically;
   `NoOperationInProgress` when nothing in progress.
9. **create_commit gate** — plain `commit` during a paused merge → `OperationInProgress`.

`src-tauri/tests/conflict_cli.rs`:
1. Kind derivation fixtures: bothModified, bothAdded (same new path both sides), deletedByUs,
   deletedByThem, and a rename/delete conflict — assert `ConflictKind` + `has_*` flags against
   the stage presence the CLI twin's `git ls-files -u` shows.
2. `get_conflict` — marker text equals the twin's post-`git merge` worktree file bytes (lossy
   UTF-8); binary fixture (NUL bytes) → `binary`; > 1 MiB fixture → `tooLarge`; deletedByUs
   with ours-deleted worktree → `missing` handling.
3. Resolution matrix — for EVERY cell of §3.2 exercisable by a merge fixture: apply, then assert
   (a) path no longer in `Index::conflicts()`, (b) stage-0 blob oid + presence matches the twin
   after the equivalent `git checkout --ours|--theirs -- p; git add p` / `git rm p` /
   hand-edit + `git add p`, (c) worktree file bytes match the twin.
4. `resolve_conflict` on a non-conflicted path → `AppError::Git`; `../escape` → `invalidName`.

Unit tests inside the new modules (senior-dev, P3c-a): wire-shape serde tests for
`RepoOpState` / `MergeOutcome` / `ConflictEntry` / `ConflictFile` (camelCase tagged JSON, same
recipe as `remote.rs::wire_shapes_are_camel_case_tagged`); `ConflictKind` derivation from the
§3.1 truth table (pure fn over three bools); MERGE_MSG `incoming` parsing (both prefixes, quoted
names with slashes, fallback path).

---

## 10. Acceptance

**AI gate (orchestrator verifies):**
- `cargo test` green incl. §9 suites; `cargo clippy -- -D warnings`; `pnpm build` green after
  every sub-increment.
- Byte-identical oracle assertions in §9 pass (tree oids, parents, messages, conflicted sets).
- Harness (`VITE_MOCK_IPC=1`): `?op=merge` shows the OpBanner ("Merging feature/login",
  2 conflicts, Commit merge disabled); conflict rows show kind badges + ours/theirs/resolved
  buttons; clicking a row opens the read-only marker view with highlighted `<<<<<<<` lines;
  resolving both rows enables Commit merge; committing clears the banner and a new 2-parent
  merge node appears at the top of the graph; Abort shows the ConfirmDialog and clears state;
  during the op, CommitBox is in merge mode and checkout/delete/merge/pull/push controls are
  disabled; plain (no `?op`) harness is unchanged (regression).
- `src/ipc/mock.ts` compiles and implements all seven new methods statefully.

**USER CHECKPOINT (native `pnpm tauri dev` — never self-declared):**
1. On a real scratch repo: merge a clean branch → merge commit appears in the graph with two
   parent edges; merge an ancestor → "Already up to date".
2. Conflicted merge: banner appears; take ours on one file, take theirs on another, hand-edit +
   Mark resolved on a third; marker view is readable; Commit merge produces a commit `git log`
   shows with 2 parents; `git status` clean afterwards.
3. Abort a conflicted merge: repo returns to pre-merge state; an unrelated uncommitted edit made
   before the merge survives.
4. Merge `origin/<branch>` from the Remotes sidebar section works.

---

## 11. Ambiguities resolved here (flag to orchestrator if disagreed)

1. **Backend `create_commit` op-state guard added** despite "existing commit flow untouched":
   one early-return line; without it a race between refresh and click can create a 1-parent
   commit mid-merge, silently discarding MERGE_HEAD ancestry — judged within the spirit of
   "gating while op active". Revert to frontend-only gating is a one-line removal.
2. **`abort_merge` = approximate `git reset --merge`** (force-checkout only merge-touched +
   conflicted paths, then read HEAD tree into the index) — NOT `reset --hard`, so pre-merge
   unstaged edits to untouched files survive, matching `git merge --abort`. Costs ~25 lines vs
   a one-line hard reset; the hard reset silently destroys user work, which is unacceptable
   given §4.1.5 deliberately permits a dirty worktree.
3. **No ConfirmDialog on merge START and on per-file resolutions** — merge start is
   non-destructive (preconditions guarantee nothing user-authored can be lost; worst case is a
   paused state that Abort — which IS confirmed — undoes), and per-file confirms would make
   multi-file conflicts miserable. Only Abort confirms.
4. **MERGE_MSG has no `into <branch>` suffix** — git's own heuristic depends on knowing the
   remote default branch; skipped for determinism. Oracle tests compare against `git merge` on
   a repo whose current branch IS the init default, where git also omits the suffix.
5. **`merge.ff` / `--no-ff` config not consulted** — FF whenever `merge_analysis` allows, always
   auto-commit otherwise. Honoring config is a P4 candidate, not v1.
6. **CommitBox is repurposed as the merge-message editor** (prefilled from `opState.message`,
   submit → `commitMerge`) instead of a separate message field in OpBanner — one editor, message
   editing for free, no duplicate commit-box styling.
7. **Marker view is a plain highlighted `<pre>`, not DiffView** — marker text is one file body,
   not hunks; faking hunks to reuse DiffView would misrender line numbers.
8. **Stage/unstage stays enabled during a merge** — git permits augmenting a merge commit's
   index, and the resolve flow uses the same mechanics; only branch-switching, plain commit,
   pull/push, and nested merges are gated.
9. **Rebase/cherry-pick/revert states render an informational, non-actionable banner** — the app
   must acknowledge externally-started operations rather than showing a false-clean UI; P3d
   makes rebase actionable using this same `RepoOpState` wire type unchanged.
10. **`MarkResolved` accepts leftover conflict markers** (same trust model as `git add`); a
    marker-detection warning toast is a possible P4 nicety, not a gate.
