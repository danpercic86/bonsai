# P36 — UX / safety fixes (backend + IPC): work items 1 & 2

Scope: **only** the backend + IPC surface for two changes. Frontend-only items (3 "Discard all"
UI, folder hover buttons, tab reorder, brand-label removal, "+" placement) are handled separately
and are out of scope here. Plan of record: `C:\Users\DPercic\.claude\plans\i-have-the-following-playful-koala.md`.

Invariants honored: Rust owns all Git logic; IPC carries compact request/response; both new fns are
blocking git2 and run via `spawn_blocking` in the command layer; both new IPC surfaces are
mock-implementable so `VITE_MOCK_IPC=1` keeps working.

---

## Work item 1 — Worktree-collision guard on checkout

### 1.1 New error variant — `crates/bonsai-core/src/error.rs`

Add a variant mirroring `CheckoutConflict` (error.rs:31,71,95) exactly:

```rust
#[error("{0}")]
BranchCheckedOutElsewhere(String),
```

- Placement: put it directly after `CheckoutConflict(String)` (after line 31) so the git-family
  variants stay grouped.
- `kind()` arm (add after the `checkoutConflict` arm, error.rs:71):
  ```rust
  AppError::BranchCheckedOutElsewhere(_) => "branchCheckedOutElsewhere",
  ```
- `message()` accessor: add `| AppError::BranchCheckedOutElsewhere(m)` to the shared `=> m` arm
  (the group at error.rs:89–107, next to `CheckoutConflict(m)`).
- Update the doc-comment union at the top of the file (error.rs:3–8) to include
  `| "branchCheckedOutElsewhere"`.
- **Code string (exact): `"branchCheckedOutElsewhere"`.**
- **Payload semantics:** the string is the OTHER worktree's absolute working-dir path (forward
  slashes, matching `WorktreeInfo.abs_path`). The full user-facing message is composed at the call
  site (see 1.3) so it can name both the branch and the path, git-style.

### 1.2 New guard helper — `crates/bonsai-core/src/git/worktree.rs`

Add a public helper alongside the existing listers. It reuses `list_worktrees` (worktree.rs:62) —
no new libgit2 traversal.

```rust
/// Blocking. Returns the ABSOLUTE working-dir path (forward slashes) of a
/// *different* worktree that currently has local branch `name` checked out, or
/// `None` if `name` is free to check out from `workdir`. Reuses
/// `list_worktrees`; never flags the caller's own worktree.
pub fn branch_checked_out_elsewhere(
    workdir: &Path,
    name: &str,
) -> Result<Option<String>, AppError>;
```

**Algorithm (spec, no body):**
1. `let cur = canonical(workdir);` — reuse the crate-private `canonical` (worktree.rs:200), the same
   best-effort canonicalization `list_worktrees` uses, so `\\?\` prefixes / symlinks compare
   consistently. This is the canonicalization rule that guarantees the caller's own worktree is
   never flagged.
2. `for wt in list_worktrees(workdir)?`:
   - Skip when `canonical(Path::new(&wt.abs_path)) == cur` (the current worktree — never a
     collision, even if it has `name` checked out; that path is the `is_head` no-op's job).
   - Skip when `!wt.valid` (invalid/unreadable/stale worktree). An invalid worktree already reports
     `branch == None` from `build_linked_row`, so this is belt-and-suspenders; do NOT error on it.
   - If `wt.branch.as_deref() == Some(name)` → return `Ok(Some(wt.abs_path.clone()))` (first match
     wins).
3. Fall through → `Ok(None)`.

**Edge cases:**
- `list_worktrees` returning an `Err` (e.g. bare repo) propagates unchanged — the caller
  (`checkout_branch_autostash`) already opened a non-bare repo, so this is not expected to fire.
- Detached-HEAD worktrees carry `branch == None` and are naturally skipped.
- `name` is the short branch name (e.g. `feature/x`), matching how `WorktreeInfo.branch` is stored.

### 1.3 Call site — `crates/bonsai-core/src/git/branches.rs`

In `checkout_branch_autostash` (branches.rs:394), insert the guard **after** the `is_head` no-op
return (branches.rs:408–414) and **before** step 1 `create_stash` (branches.rs:419). This ordering
guarantees a refusal mutates nothing: no stash created, HEAD unchanged, worktree untouched.

```rust
// 0b. Refuse if the branch is checked out in ANOTHER worktree (git-like:
//     "fatal: '<b>' is already checked out at '<path>'"). Runs before any
//     side effect, so a refusal changes nothing.
if let Some(other) = worktree::branch_checked_out_elsewhere(workdir, name)? {
    return Err(AppError::BranchCheckedOutElsewhere(format!(
        "branch '{name}' is already checked out at '{other}'"
    )));
}
```

- Add `use crate::git::worktree;` to the imports (branches.rs:10-11 area; the module already uses
  `use crate::git::stash;`).
- Update the `Errors:` doc line (branches.rs:391–393) to add
  `| branchCheckedOutElsewhere`.

### 1.4 Command layer — no signature change

`checkout_branch` (`src-tauri/src/commands.rs:1028`) calls the core fn and already maps `AppError`
through serde; the new variant serializes as `{ kind: "branchCheckedOutElsewhere", message }` and
propagates unchanged. **No Rust command edit, no registration change.**

### 1.5 Frontend IPC (types + mock only — surfacing UI is out of scope)

- `src/ipc/types.ts`: if an error-`kind` string union exists, add `'branchCheckedOutElsewhere'` to
  it. (If errors are typed only as `{ kind: string; message: string }`, no change.)
- `src/ipc/mock.ts` `checkoutBranch`: add a deterministic trigger so the harness can exercise the
  refusal path — when the requested branch name equals a reserved fixture value
  **`"__wt_locked__"`** (or a branch flagged in fixtures as checked-out-elsewhere), throw
  `{ kind: 'branchCheckedOutElsewhere', message: "branch '<name>' is already checked out at '<path>'" }`
  instead of switching. Keep the existing happy path otherwise. (Exact trigger value is a mock-only
  convention; senior-dev may align it with an existing worktree fixture if one is more natural —
  flagged below.)

---

## Work item 2 — `discard_paths_force` (bulk / folder discard incl. untracked deletion)

### 2.1 New core fn — `crates/bonsai-core/src/git/discard.rs`

Add alongside the existing tracked-only `discard_paths` (discard.rs:19). **Do not modify
`discard_paths`.**

```rust
/// Blocking. Force-discard a mixed set of paths:
///   - TRACKED paths (present in the index) are restored to their INDEX content
///     via `checkout_index` + per-path `CheckoutBuilder` (identical mechanism to
///     `discard_paths`, discard.rs:46-52) — reverts unstaged edits, recreates
///     unstaged deletions. Staged content is untouched.
///   - UNTRACKED paths (absent from the index) are DELETED from disk
///     (`std::fs::remove_file`).
/// All-or-nothing validation up-front (like `discard_paths`): every path is
/// `validate_rel_path`-checked before any mutation. Empty `paths` is a no-op
/// `Ok(())` — `checkout_index` is NEVER reached with a zero-`.path()` (match-all)
/// pathspec. Destructive — the UI confirms first.
pub fn discard_paths_force(workdir: &Path, paths: &[String]) -> Result<(), AppError>;
```

**Algorithm (spec, no body):**
1. **Empty guard:** `if paths.is_empty() { return Ok(()); }` — FIRST, before any repo open. This is
   the same match-all-clobber guarantee as `discard_paths` (discard.rs:20).
2. **Validate all:** `for p in paths { validate_rel_path(p)?; }` — reuse
   `crate::git::stage::validate_rel_path`. Any escaping/invalid path rejects the whole batch before
   touching the repo or filesystem.
3. `let repo = open_workdir_repo(workdir)?;` then `let index = repo.index()?;`.
4. **Partition** by index membership (`index.get_path(Path::new(p), 0).is_some()`):
   - `tracked: Vec<&String>` — present in the index.
   - `untracked: Vec<&String>` — absent from the index.
5. **Delete untracked first, then restore tracked** (ordering is fixed — see rationale below).
   - **Untracked deletion:** for each untracked `p`, `std::fs::remove_file(workdir.join(p))`. If the
     file is already gone, **tolerate it** (a missing untracked file is the desired end state): map
     `ErrorKind::NotFound` to `Ok(())` / skip; propagate any other io error as `AppError::Io`.
     (Untracked entries here are files, not dirs — the frontend expands folder selections to leaf
     paths; a directory path is not expected. If `remove_file` fails because the target is a
     directory, propagate the io error rather than recursing — flagged below.)
   - **Tracked restore:** only if `!tracked.is_empty()`, build one `CheckoutBuilder`:
     `cb.force().remove_untracked(false);` then `cb.path(p)` for each tracked `p`; call
     `repo.checkout_index(None, Some(&mut cb))?`. The `!tracked.is_empty()` guard preserves the
     "at least one `.path()`" invariant so this branch also can never match-all-clobber.

**Ordering rationale:** untracked deletes and tracked restores touch disjoint path sets, so order is
functionally independent; deleting untracked first keeps the destructive-but-simple step before the
libgit2 call and matches "remove new files, then revert modified files" mental model. Fixed for test
determinism.

**Edge cases:**
- Empty `paths` → `Ok(())`, no repo open, no `checkout_index` (match-all guard).
- All-untracked batch → no `checkout_index` call at all.
- All-tracked batch → behaves like `discard_paths` (minus the defensive "not a tracked file" error).
- Duplicate paths in the input are harmless (idempotent per side).
- Invalid/escaping path anywhere → whole batch rejected (`AppError::Other` from `validate_rel_path`),
  nothing mutated.

### 2.2 Command layer — `src-tauri/src/commands.rs`

Mirror `discard_paths` (commands.rs:2091–2110) exactly:

```rust
use bonsai_core::git::discard::discard_paths_force as discard_paths_force_core; // near commands.rs:28

/// Force-discards a mixed set: tracked paths restored to index, untracked paths
/// deleted from disk. Destructive — the UI confirms first. Errors: `other`
/// (invalid path) | `io` | `git` | `noRepo`. Does NOT emit `repo-changed`.
#[tauri::command]
pub async fn discard_paths_force(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    discard_paths_force_inner(state.inner(), &repo_id, paths).await
}

async fn discard_paths_force_inner(
    state: &AppState,
    repo_id: &str,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || discard_paths_force_core(&path, &paths))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

### 2.3 Registration — `src-tauri/src/lib.rs`

Add `commands::discard_paths_force,` in the `invoke_handler` list immediately after
`commands::discard_paths,` (lib.rs:108).

### 2.4 Frontend IPC binding — `discardPathsForce(repoId, paths)`

Mirror the existing `discardPaths` binding in all three files.

- `src/ipc/types.ts` (next to line 1291):
  ```ts
  discardPathsForce(repoId: string, paths: string[]): Promise<void>;
  ```
- `src/ipc/tauri.ts` (next to line 389):
  ```ts
  discardPathsForce(repoId: string, paths: string[]): Promise<void> {
    return invoke<void>('discard_paths_force', { repoId, paths });
  }
  ```
- `src/ipc/mock.ts` (next to line 3200 `discardPaths`): implement the split — remove the given
  paths from `status.untracked` (deleted) AND from `status.unstaged` (reverted), so the mocked
  Changes panel reflects a bulk force-discard:
  ```ts
  async discardPathsForce(repoId: string, paths: string[]): Promise<void> {
    await delay(150);
    const state = requireRepo(repoId);
    const drop = new Set(paths);
    state.status.unstaged = state.status.unstaged.filter((e) => !drop.has(e.path));
    state.status.untracked = state.status.untracked.filter((e) => !drop.has(e.path));
  }
  ```
  (Adjust field names to the actual mock `StatusSnapshot` shape — senior-dev matches whatever the
  fixture uses for untracked entries.)

---

## Acceptance criteria

**Work item 1 (cargo test, scratch repo):**
- A linked worktree on branch `feature` exists; calling `checkout_branch_autostash(main_workdir,
  "feature")` returns `Err(AppError::BranchCheckedOutElsewhere(p))` where `p` is the linked
  worktree's abs path.
- The refusal mutated nothing: no new stash entry, HEAD unchanged, the main worktree's dirty state
  preserved.
- Checking out a branch that is NOT checked out elsewhere still succeeds.
- Checking out the CURRENT branch still hits the `is_head` no-op (guard never flags self).
- The new variant serializes as `{ "kind": "branchCheckedOutElsewhere", "message": ... }`.

**Work item 2 (cargo test, scratch repo):**
- Modified tracked file → reverted to index content.
- Untracked file → deleted from disk.
- Mixed set (one modified tracked + one untracked) → both effects applied.
- Empty `paths` → `Ok(())`, and a pre-existing unstaged edit is untouched (proves no match-all
  clobber).
- Untracked path already absent on disk → tolerated (`Ok`), not an error.
- Invalid/escaping path → rejected, nothing mutated.

**Harness (`VITE_MOCK_IPC=1`):** `discardPathsForce` is callable from the mock and the reserved
`branchCheckedOutElsewhere` trigger throws the typed error, so the frontend builds and runs in a
plain browser.

---

## Flagged ambiguities (for the orchestrator)

1. **Mock refusal trigger (item 1.5):** I specified a reserved branch-name sentinel
   (`"__wt_locked__"`) for the mock `checkoutBranch` to throw `branchCheckedOutElsewhere`. If the
   mock fixtures already carry a worktree list with a branch marked checked-out-elsewhere, aligning
   the trigger to that fixture is cleaner. Recommendation: use whatever worktree fixture already
   exists; fall back to the sentinel if none. Low impact (mock-only).
2. **Untracked *directory* paths (item 2.1):** the contract assumes the frontend expands folder
   selections to leaf file paths, so `discard_paths_force` only ever receives files, and a directory
   path would surface an io error rather than recursively delete. Recommendation: keep it
   file-only in the backend (safer — no recursive delete of an unexpected dir); if product later
   wants true recursive folder discard, that is a follow-up with its own confirm-count semantics.
   Flagging because the plan's UI item 3 discards "folder" leaves — confirm the frontend sends leaf
   files, not dir paths.
