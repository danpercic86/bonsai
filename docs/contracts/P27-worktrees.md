# P27 — Worktree management (list + create/remove/lock/unlock + open-in-tab)

Roadmap Theme C, item C1. Let a user check out multiple branches into sibling directories of one
repo, managed from a **Worktrees** sidebar section that mirrors **Submodules (P19)** exactly: a
read-first list + a small set of ops + open-in-tab, all harness-verifiable with a stateful mock.

New Rust module `crates/bonsai-core/src/git/worktree.rs`, five commands, a `WorktreeInfo` wire type,
the IPC triple, and a **Worktrees** sidebar section. "Open in new tab" reuses the existing multi-repo
tab flow — **no new command** (identical to P19 §6.5).

**Structural precedent (mirror verbatim):** `docs/contracts/P19-submodules.md`. This contract is its
sibling: same module shape, same command/`_inner`/`spawn_blocking` template, same IPC triple + stateful
mock, same sidebar-section + context-menu + open-in-tab wiring.

Source files to mirror (exact patterns):
- `crates/bonsai-core/src/git/submodule.rs` — canonical core module: pure git2, blocking, inline serde
  wire types (`rename_all = "camelCase"`), `AppError` returns, `open_workdir_repo` open, `#[cfg(test)]`
  wire-shape + classification unit tests. **Structural template.**
- `crates/bonsai-core/src/git/stage.rs:14` — `open_workdir_repo(&Path)` (rejects bare); `:33`
  `validate_rel_path` (path-safety idiom reused by the name sanitizer).
- `crates/bonsai-core/src/git/mod.rs` — module declaration list (alphabetical).
- `src-tauri/src/commands.rs:1997-2087` — the P19 `#[tauri::command] async fn` + `_inner` +
  `repo_path(state, repo_id)?` + `spawn_blocking` template (copy shape exactly).
- `src-tauri/src/lib.rs:87-90` — `generate_handler!` registration list.
- `src/ipc/{types.ts:368-378,1032-1040 , tauri.ts:391-404 , mock.ts:917-963,3071-3107}` — the IPC
  triple + stateful `MockRepoState.submodules` slice.
- `src/components/Sidebar.tsx:66-69,288-320,691-706` — Submodules section, `SUBMODULE_BADGE`,
  `SubmoduleRow`, `SectionHeader` (with `extra` action slot at `:84-109`).
- `src/components/RepoWorkspace.tsx:120-122,178,318,644-658,1587-1628,2256-2295,2742-2743` — submodule
  state + reqId guard + `refetchSubmodules` + handlers + `submoduleMenuItems` +
  `handleSubmoduleContextMenu` + Sidebar wiring; `:2988-3078` ConfirmDialog pattern (branch delete).
- `src/App.tsx:205,753` — `openTab(path)` + `onOpenRepoPath={(p) => void openTab(p)}` thread.

git2 version in the workspace: **0.20.4** (`Cargo.lock:1368`). All API names below are verified against
that version's rustdoc (§2 cites each).

---

## OPEN DECISIONS (recommended defaults chosen; implementation is NOT blocked)

1. **Create-path derivation (product-shaping — flag to user).** → **Recommend
   `<main_parent>/.worktrees/<slug>`**, where `<main_parent>` is the parent directory of the **main**
   worktree's workdir and `<slug>` is the sanitized branch name (§2.4). Justification: the roadmap
   (C1) literally says "suggest `.worktrees/<branch>` paths"; a single tidy `.worktrees/` container
   next to the repo avoids sprinkling many `repo-branch` siblings into the parent, and confining every
   created worktree under one controlled directory makes the path-escape check a trivial prefix
   assertion (§2.4). *(Alternative: `<main_parent>/<repo-name>-<slug>` flat siblings — rejected:
   clutters the parent and complicates the containment check. If the user prefers flat siblings, only
   §2.4's `derive_path` changes.)*
2. **`add_worktree` return type.** → **Recommend it returns the created `WorktreeInfo`** (not `()`),
   because the backend OWNS the derived path/name and the UI must show it (success toast) and be able
   to offer "open it now". This is a deliberate, justified deviation from P19's "return `()` + refetch"
   (there the caller already knew the key). The frontend still refetches the full list afterward for
   consistency; the mock returns the new row too. *(Alternative: return `()` — rejected: the UI would
   not know the derived path.)*
3. **Removing a worktree with uncommitted changes (product-shaping — flag to user).** → **Recommend
   REFUSE when the worktree is dirty** (staged/unstaged/untracked), returning a clear `Git` error
   ("worktree has uncommitted changes; commit or stash them first"). This matches `git worktree
   remove`'s default (which refuses dirty without `--force`) and prevents silent data loss, since
   libgit2's prune does NOT check dirtiness. *(Alternative: allow force-remove of dirty/stale
   worktrees — deferred behind a future `force: bool` param; the confirm dialog already names the path.
   Revisit if users find the refusal annoying for stale trees.)*
4. **Removing a LOCKED worktree.** → **Recommend REFUSE** with a clear `Git` error ("worktree is
   locked; unlock it first"). No `force`/`--force` in v1. The user unlocks explicitly, then removes.
   *(Alternative: a force flag that sets `WorktreePruneOptions::locked(true)` — deferred.)*
5. **Removing main / current worktree.** → **Always refuse, server-side** (defense-in-depth), even
   though the UI disables the action. `Git` errors: "cannot remove the main worktree" / "cannot remove
   the worktree you currently have open". Non-negotiable safety, not really open.
6. **New-branch-at-create.** → **DEFERRED** (v1 requires an EXISTING local branch). Noted in §Scope.
7. **`AppError` variants.** → **Recommend NO new variant.** Reuse `NoRepo` | `InvalidName` |
   `BranchNotFound` | `Git` | `Io`. Every refusal (main/current/locked/dirty/not-found) is a `Git`
   error with a precise message; a blank name is `InvalidName`; a missing branch is `BranchNotFound`.
   *(Mirrors P19 §OPEN-3.)*

None of these block P27a (read-only list). Defaults 1 and 3 are the only product-shaping ones — both
are the conservative/standard-git choice; the orchestrator may accept them as-is.

---

## Scope

**INCLUDE (v1):**
- **List** all worktrees (main + linked) with: name, absolute path, repo-relative path (if under the
  main workdir), checked-out branch (or `null` when detached) + full HEAD oid, `locked` (+ reason),
  `isMain`, `isCurrent`, `prunable` (stale), `valid`. The **main** working tree is a synthesized row
  so the list is complete.
- **Create** a worktree for an **existing local branch** at a **derived** path (§2.4). No native
  folder picker. No new-branch creation.
- **Remove** a worktree — confirm-gated; refuses main / current / locked / dirty (§2.6, §OPEN 3-5).
- **Lock / unlock** a worktree.
- **Open a worktree in a new tab** via the existing `openRepo`/tab flow (no new command).

**DEFER (explicit):** custom-path selection via native dialog (USER-CHECKPOINT dependency);
new-branch-at-create; per-worktree AI context profiles (Theme A, later); worktree move/repair;
force-remove of dirty/locked worktrees; "prune all stale worktrees" one-shot action.

---

## 1. Overview & invariants held

- **Rust owns all Git logic.** `worktree.rs` wraps every git2 worktree call, the path derivation, the
  classification (main/current/locked/prunable), and the safe remove sequence. React only renders
  `WorktreeInfo` and dispatches the five commands + a confirm.
- **IPC carries compact precomputed data.** `list_worktrees` returns a small `Vec<WorktreeInfo>` with
  everything already resolved (branch, oid, badges) — no raw libgit2 objects, no per-worktree
  round-trips. Commands = request/response; **no new events or channels** (the `notify` watcher +
  existing refetch cover liveness, exactly like P19).
- **git2 is blocking → `spawn_blocking`.** Every command wraps its blocking core exactly like the P19
  inners (`commands.rs:2013`). Commands carry `repoId` first, resolve `repo_path`, **no consent gate**
  (pure git).
- **Runtime-free core.** `worktree.rs` functions take `&Path` / `&str`, no Tauri types → unit/CLI
  testable without the Tauri "test" feature (same rule as submodule/stash/remote).
- **The perf-gated graph walk is untouched.** Worktrees never seed the walk and add no `RefLabel`.
- **Destructive op is confirm-gated + defended server-side.** `remove_worktree` deletes a
  working-directory tree → explicit UI confirm naming the absolute path, AND the backend independently
  refuses main/current/locked/dirty (§2.6).
- **Path safety.** The derived create path is sanitized (§2.4) and asserted to stay inside the
  controlled `.worktrees/` container; the remove fallback `remove_dir_all` is guarded to the worktree's
  own path. No `..` / separator injection reaches libgit2 or the filesystem.
- **Mock-implementable.** Every command has a `mock.ts` implementation over a stateful
  `MockRepoState.worktrees` slice (seeded main + linked + locked), so `VITE_MOCK_IPC=1` runs the whole
  feature — list, create, remove, lock, unlock, open-in-tab — in a plain browser.

---

## 2. New Rust module `crates/bonsai-core/src/git/worktree.rs`

Register in `crates/bonsai-core/src/git/mod.rs`: add `pub mod worktree;` at the end of the list
(alphabetical: `tags` < `worktree`). Open the repo with the existing `open_workdir_repo(&Path)`
(`stage.rs:14`) — it rejects bare repos and uses `NO_SEARCH`, exactly like P19.

git2 0.20.4 API surface relied on (all verified against rustdoc):
- `Repository::worktrees(&self) -> Result<StringArray>` — **names** of linked worktrees (the main
  worktree is NOT included; we synthesize it, §2.3).
- `Repository::find_worktree(&self, name: &str) -> Result<Worktree>`.
- `Repository::worktree(&self, name: &str, path: &Path, opts: Option<&WorktreeAddOptions>) -> Result<Worktree>`
  — creates a linked worktree.
- `Repository::open_from_worktree(worktree: &Worktree) -> Result<Repository>` — open a worktree's own
  repo to read its HEAD/status.
- `Repository::{is_worktree, workdir, commondir}` — main-worktree derivation (§2.3).
- `Worktree::{name, path, validate, is_locked, is_prunable, lock, unlock, prune}`.
- `WorktreeLockStatus::{Unlocked, Locked(Option<String>)}` (Copy).
- `WorktreeAddOptions::{new, reference(Option<&Reference>), lock(bool), checkout_existing(bool)}`.
- `WorktreePruneOptions::{new, valid(bool), locked(bool), working_tree(bool)}`.

> Note for senior-dev: confirm the exact arg count of `Repository::worktree` against the local
> `git2 0.20.4` rustdoc before coding (`name, path, Option<&WorktreeAddOptions>` is the stable shape).

### 2.1 Wire type

```rust
/// One worktree row (main or linked). Wire: camelCase. `head_oid` is full
/// 40-hex or null; the frontend shortens for display.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeInfo {
    /// Worktree NAME (stable key for remove/lock/unlock). For linked worktrees
    /// this is `Worktree::name()`; for the synthesized main row it is the main
    /// workdir's directory basename. Destructive actions key off `is_main`/
    /// `is_current`, NOT the name (§2.3).
    pub name: String,
    /// ABSOLUTE path to the worktree's working directory, forward slashes on the
    /// wire. Fed verbatim to the open-repo/tab flow (§6.6).
    pub abs_path: String,
    /// Repo-relative path (forward slashes) IF the worktree lives under the main
    /// workdir, else None (worktrees are usually siblings → usually None).
    pub rel_path: Option<String>,
    /// Short branch name checked out in this worktree (e.g. "feature/x"), or None
    /// when HEAD is detached / the worktree is invalid.
    pub branch: Option<String>,
    /// Full HEAD commit oid, or None when the worktree is invalid/unreadable.
    pub head_oid: Option<String>,
    /// True if `Worktree::is_locked()` == Locked. Always false for the main row.
    pub locked: bool,
    /// The optional lock reason from `WorktreeLockStatus::Locked(Some(reason))`.
    pub lock_reason: Option<String>,
    /// True for the synthesized main working tree.
    pub is_main: bool,
    /// True for the worktree whose workdir == the repo the app currently has open.
    pub is_current: bool,
    /// True when `Worktree::is_prunable(None)` — i.e. stale (its working dir is
    /// gone / administratively removable). Always false for the main row.
    pub prunable: bool,
    /// `Worktree::validate().is_ok()` — the working tree + admin files are intact.
    /// Always true for the main row.
    pub valid: bool,
}
```

### 2.2 Function signatures

```rust
/// Blocking. List the main worktree (synthesized, first) followed by every
/// linked worktree in `Repository::worktrees()` order. Never empty (always ≥ the
/// main row for a non-bare repo).
pub fn list_worktrees(workdir: &Path) -> Result<Vec<WorktreeInfo>, AppError>;

/// Blocking. Create a linked worktree checking out the EXISTING local branch
/// `branch` at a derived path (§2.4). Returns the created row (§OPEN-2).
/// Errors: InvalidName (blank) | BranchNotFound | Git (branch already checked out
/// elsewhere / path or name collision exhausted / libgit2).
pub fn add_worktree(workdir: &Path, branch: &str) -> Result<WorktreeInfo, AppError>;

/// Blocking. Remove linked worktree `name`: refuse main/current/locked/dirty
/// (§2.6, §OPEN 3-5), then prune (deletes admin files + the working directory).
/// Errors: InvalidName (blank) | Git (refusals / not found / libgit2) | Io.
pub fn remove_worktree(workdir: &Path, name: &str) -> Result<(), AppError>;

/// Blocking. Lock linked worktree `name` with an optional reason.
/// git2: `Worktree::lock(reason)`. Errors: InvalidName | Git (not found / main).
pub fn lock_worktree(workdir: &Path, name: &str, reason: Option<&str>) -> Result<(), AppError>;

/// Blocking. Unlock linked worktree `name`. git2: `Worktree::unlock()`.
/// Errors: InvalidName | Git (not found / main).
pub fn unlock_worktree(workdir: &Path, name: &str) -> Result<(), AppError>;
```

### 2.3 `list_worktrees` internals

```rust
let repo = open_workdir_repo(workdir)?;                        // rejects bare
let cur = repo.workdir()                                       // current app workdir
    .map(canonical)                                            // §2.7 canonical()
    .ok_or_else(|| AppError::Git("repository has no working directory".into()))?;
let main_dir = main_workdir(&repo)?;                           // helper below

let mut out = Vec::new();
out.push(build_main_row(&main_dir, &cur)?);                    // synthesized main row, FIRST

for name in repo.worktrees()?.iter().flatten() {              // StringArray → &str, skip non-UTF-8
    let wt = repo.find_worktree(name)?;
    out.push(build_linked_row(&wt, &main_dir, &cur)?);
}
Ok(out)
```

Helpers:

```rust
/// Path of the MAIN worktree's workdir, regardless of which worktree the app has
/// open. If the current repo is itself a linked worktree, derive it from the
/// shared common dir (`<main>/.git` → parent). Non-bare assumed (open_workdir_repo).
fn main_workdir(repo: &git2::Repository) -> Result<PathBuf, AppError> {
    if repo.is_worktree() {
        // commondir == "<main>/.git" (possibly trailing sep) → parent is <main>.
        let cd = repo.commondir();
        strip_dotgit_parent(cd)                                 // trim trailing sep, .parent()
            .ok_or_else(|| AppError::Git("cannot locate main worktree".into()))
    } else {
        repo.workdir()
            .map(Path::to_path_buf)
            .ok_or_else(|| AppError::Git("repository has no working directory".into()))
    }
}

/// Synthesize the main row. Opens the main workdir to read its HEAD.
fn build_main_row(main_dir: &Path, cur: &Path) -> Result<WorktreeInfo, AppError> {
    let (branch, head_oid) = read_head(&git2::Repository::open(main_dir)?);   // §2.5 read_head
    Ok(WorktreeInfo {
        name: dir_basename(main_dir),                           // display only
        abs_path: to_fwd(main_dir),
        rel_path: None,
        branch, head_oid,
        locked: false, lock_reason: None,
        is_main: true,
        is_current: canonical(main_dir) == *cur,
        prunable: false, valid: true,
    })
}

fn build_linked_row(wt: &git2::Worktree, main_dir: &Path, cur: &Path)
    -> Result<WorktreeInfo, AppError>
{
    let name = wt.name().ok_or_else(|| AppError::Git("worktree has non-UTF-8 name".into()))?;
    let path = wt.path();                                       // absolute
    let valid = wt.validate().is_ok();
    let prunable = wt.is_prunable(None).unwrap_or(false);
    let (locked, lock_reason) = match wt.is_locked()? {
        git2::WorktreeLockStatus::Locked(r) => (true, r),
        git2::WorktreeLockStatus::Unlocked  => (false, None),
    };
    // branch/oid: only readable when the working tree is intact.
    let (branch, head_oid) = if valid {
        match git2::Repository::open_from_worktree(wt) {
            Ok(r) => read_head(&r),
            Err(_) => (None, None),
        }
    } else { (None, None) };
    Ok(WorktreeInfo {
        name: name.to_string(),
        abs_path: to_fwd(path),
        rel_path: path.strip_prefix(main_dir).ok().map(to_fwd_rel),
        branch, head_oid,
        locked, lock_reason,
        is_main: false,
        is_current: canonical(path) == *cur,
        prunable, valid,
    })
}
```

### 2.4 Create path + name derivation, sanitization, collision (§OPEN-1)

```rust
/// Derived worktree name/path for `branch`. Slug = branch with every char outside
/// [A-Za-z0-9._-] replaced by '-', runs of '-' collapsed, leading/trailing '-'
/// and '.' trimmed. (Git branch names cannot contain "..", but reject defensively.)
/// name == slug; path == <main_parent>/.worktrees/<slug>. On collision (dir exists
/// OR find_worktree(name) succeeds) append "-2", "-3", … up to "-99", else Git error.
fn derive_worktree(main_dir: &Path, repo: &git2::Repository, branch: &str)
    -> Result<(String, PathBuf), AppError>
{
    let base = sanitize_slug(branch)?;                         // Err(InvalidName) if empty/".."
    let container = main_dir.parent()
        .ok_or_else(|| AppError::Git("repo has no parent directory".into()))?
        .join(".worktrees");
    for n in std::iter::once(base.clone())
        .chain((2..=99).map(|i| format!("{base}-{i}")))
    {
        let path = container.join(&n);
        let name_taken = repo.find_worktree(&n).is_ok();
        if !path.exists() && !name_taken {
            // Containment defense: `path` MUST stay under `container`.
            debug_assert!(path.starts_with(&container));
            return Ok((n, path));
        }
    }
    Err(AppError::Git(format!("could not derive a free worktree path for '{branch}'")))
}
```

`sanitize_slug` rejects an empty/`..`-containing result with `AppError::InvalidName`. Because we build
the leaf from a sanitized slug and join it onto a fixed container, no separators or `..` reach
libgit2 (mirrors the `validate_rel_path` intent, `stage.rs:33`).

### 2.5 `add_worktree` internals

```rust
let repo = open_workdir_repo(workdir)?;
if branch.trim().is_empty() {
    return Err(AppError::InvalidName("branch name is empty".into()));
}
let br = repo.find_branch(branch, git2::BranchType::Local).map_err(|e| match e.code() {
    git2::ErrorCode::NotFound => AppError::BranchNotFound(format!("branch '{branch}' not found")),
    _ => e.into(),
})?;
let reference = br.get();                                      // &Reference (borrows repo)

let main_dir = main_workdir(&repo)?;
let (name, path) = derive_worktree(&main_dir, &repo, branch)?;
std::fs::create_dir_all(path.parent().unwrap())?;             // ensure `.worktrees/` exists (Io on fail)

let mut opts = git2::WorktreeAddOptions::new();
opts.reference(Some(reference));                              // check out THIS existing branch
// (do NOT set checkout_existing: we pass an explicit reference)

// libgit2 errors if `branch` is already checked out in another worktree → Git.
repo.worktree(&name, &path, Some(&opts))
    .map_err(|e| AppError::Git(e.message().to_string()))?;

let wt = repo.find_worktree(&name)?;
let cur = canonical(repo.workdir().unwrap());
build_linked_row(&wt, &main_dir, &cur)                        // return the created row (§OPEN-2)
```

### 2.6 `remove_worktree` — the safe sequence

git2 CAN delete the working directory: `WorktreePruneOptions::working_tree(true)` makes
`Worktree::prune` recursively remove the working tree on disk, and `valid(true)` allows pruning a
worktree that still exists (default prune only touches stale/invalid ones). So the primary deletion is
libgit2's; the Rust side adds a guarded fallback only if the directory somehow survives.

```rust
let repo = open_workdir_repo(workdir)?;
if name.trim().is_empty() {
    return Err(AppError::InvalidName("worktree name is empty".into()));
}
let main_dir = main_workdir(&repo)?;
let cur = canonical(repo.workdir().unwrap());

// 1. Refuse the MAIN worktree (its basename is not a real linked name; also
//    guard by path in case a linked name collides).
if name == dir_basename(&main_dir) && repo.find_worktree(name).is_err() {
    return Err(AppError::Git("cannot remove the main worktree".into()));
}
let wt = repo.find_worktree(name).map_err(|e| match e.code() {
    git2::ErrorCode::NotFound => AppError::Git(format!("worktree '{name}' not found")),
    _ => e.into(),
})?;
let wt_path = wt.path().to_path_buf();

// 2. Refuse current / main by PATH (defense-in-depth).
if canonical(&wt_path) == cur {
    return Err(AppError::Git("cannot remove the worktree you currently have open".into()));
}
if canonical(&wt_path) == canonical(&main_dir) {
    return Err(AppError::Git("cannot remove the main worktree".into()));
}
// 3. Refuse LOCKED (no force in v1, §OPEN-4).
if matches!(wt.is_locked()?, git2::WorktreeLockStatus::Locked(_)) {
    return Err(AppError::Git("worktree is locked; unlock it first".into()));
}
// 4. Refuse DIRTY (§OPEN-3): open the worktree, scan statuses.
if wt.validate().is_ok() {
    if let Ok(wt_repo) = git2::Repository::open_from_worktree(&wt) {
        if is_dirty(&wt_repo)? {                              // any non-CURRENT/IGNORED status entry
            return Err(AppError::Git(
                "worktree has uncommitted changes; commit or stash them first".into()));
        }
    }
}
// 5. Prune: remove admin files AND the working directory recursively.
let mut opts = git2::WorktreePruneOptions::new();
opts.valid(true).working_tree(true);                          // NOT locked(true): we refused locked
wt.prune(Some(&mut opts))?;

// 6. Guarded fallback: if libgit2 left the dir, remove it (only if it is exactly
//    the worktree path we just pruned and still exists).
if wt_path.exists() {
    std::fs::remove_dir_all(&wt_path)?;                       // Io on failure
}
Ok(())
```

`is_dirty(repo)`: `repo.statuses(Some(StatusOptions::new().include_untracked(true).include_ignored(false)))`
and return `true` if any entry's status is not `Status::CURRENT`. (Runtime-free; no dependency on
`status.rs`.)

### 2.7 lock / unlock + shared helpers

```rust
fn open_linked<'r>(repo: &'r git2::Repository, name: &str) -> Result<git2::Worktree<'r>, AppError> {
    if name.trim().is_empty() {
        return Err(AppError::InvalidName("worktree name is empty".into()));
    }
    repo.find_worktree(name).map_err(|e| match e.code() {
        git2::ErrorCode::NotFound => AppError::Git(format!("worktree '{name}' not found")),
        _ => e.into(),
    })
}

pub fn lock_worktree(workdir: &Path, name: &str, reason: Option<&str>) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let wt = open_linked(&repo, name)?;
    // Empty reason string → None (git treats "" as no reason).
    let reason = reason.filter(|r| !r.trim().is_empty());
    wt.lock(reason)?;                                          // already-locked → libgit2 Git error
    Ok(())
}

pub fn unlock_worktree(workdir: &Path, name: &str) -> Result<(), AppError> {
    let repo = open_workdir_repo(workdir)?;
    let wt = open_linked(&repo, name)?;
    wt.unlock()?;                                             // not-locked → libgit2 Git error
    Ok(())
}
```

`canonical(p)` = `dunce::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())` if `dunce` is already a
dep, else `std::fs::canonicalize` with the same fallback (senior-dev: check `Cargo.toml`; a lexical
fallback is fine — comparison only needs to be consistent). `to_fwd` / `to_fwd_rel` = `to_string_lossy`
with `\\` → `/`. `dir_basename` = final path component as `String`.

### 2.8 Error mapping (→ `AppError`)

| Situation | AppError |
|---|---|
| repo not usable / bare | `Git` (via `open_workdir_repo`) |
| blank `name`/`branch` arg | `InvalidName` |
| `add`: branch does not exist | `BranchNotFound` |
| `add`: branch already checked out elsewhere / collision exhausted | `Git` |
| `remove`: main / current / locked / dirty / not found | `Git` (precise message) |
| `remove`/`add` filesystem (`create_dir_all`, `remove_dir_all`) | `Io` |
| lock already-locked / unlock not-locked / any other libgit2 | `Git` |

**No new `AppError` variant** (reuses `noRepo` | `invalidName` | `branchNotFound` | `git` | `io`).

### 2.9 `#[cfg(test)]` unit tests (in-module, no repo)

Mirror `submodule.rs:181-301`:
- `worktree_info_serializes_camel_case_keys` — `serde_json` asserts keys
  `{name,absPath,relPath,branch,headOid,locked,lockReason,isMain,isCurrent,prunable,valid}`.
- `sanitize_slug_table` — `feature/x` → `feature-x`; `a//b` → `a-b`; `--weird--` → `weird`; `""`,
  `".."`, `"/"` → `Err(InvalidName)`; collapses runs; trims leading/trailing `-`/`.`.
- `blank_name_is_invalid` / `blank_branch_is_invalid` — `open_linked` / `add_worktree` reject blank.

---

## 3. Commands (`src-tauri/src/commands.rs`) + registration

Add `use bonsai_core::git::worktree::{self, WorktreeInfo};` to the import block. Follow the P19
`#[tauri::command] pub async fn` → runtime-free `_inner` → `repo_path(state, repo_id)?` →
`spawn_blocking` template exactly (`commands.rs:1997-2087`). None emit `repo-changed`; the frontend
refetches imperatively (identical to P19). `spawn_blocking` join errors →
`AppError::Other(format!("task join error: {e}"))` verbatim.

```rust
// list_worktrees(repoId) -> Vec<WorktreeInfo>            errors: noRepo | git
#[tauri::command] pub async fn list_worktrees(state, repo_id: String) -> Result<Vec<WorktreeInfo>, AppError>;
async fn list_worktrees_inner(state, repo_id) { spawn_blocking(|| worktree::list_worktrees(&path)) }

// add_worktree(repoId, branch) -> WorktreeInfo   errors: noRepo | invalidName | branchNotFound | git
#[tauri::command] pub async fn add_worktree(state, repo_id: String, branch: String) -> Result<WorktreeInfo, AppError>;
async fn add_worktree_inner(...) { spawn_blocking(move || worktree::add_worktree(&path, &branch)) }

// remove_worktree(repoId, name) -> ()                    errors: noRepo | invalidName | git | io
#[tauri::command] pub async fn remove_worktree(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn remove_worktree_inner(...) { spawn_blocking(move || worktree::remove_worktree(&path, &name)) }

// lock_worktree(repoId, name, reason?) -> ()             errors: noRepo | invalidName | git
#[tauri::command] pub async fn lock_worktree(state, repo_id: String, name: String, reason: Option<String>) -> Result<(), AppError>;
async fn lock_worktree_inner(...) { spawn_blocking(move || worktree::lock_worktree(&path, &name, reason.as_deref())) }

// unlock_worktree(repoId, name) -> ()                    errors: noRepo | invalidName | git
#[tauri::command] pub async fn unlock_worktree(state, repo_id: String, name: String) -> Result<(), AppError>;
async fn unlock_worktree_inner(...) { spawn_blocking(move || worktree::unlock_worktree(&path, &name)) }
```

Register all five in `src-tauri/src/lib.rs` `generate_handler!`, appended after
`commands::sync_submodule` (add a trailing comma to that line):

```rust
        commands::sync_submodule,
        commands::list_worktrees,
        commands::add_worktree,
        commands::remove_worktree,
        commands::lock_worktree,
        commands::unlock_worktree
```

---

## 4. Wire types (TS mirror — `src/ipc/types.ts`)

New type (place near `SubmoduleInfo`, `types.ts:370`):

```ts
export interface WorktreeInfo {
  name: string;               // stable key for remove/lock/unlock
  absPath: string;            // absolute workdir path — feed to open-in-tab
  relPath: string | null;     // repo-relative if under the main workdir, else null
  branch: string | null;      // short branch name; null if detached/invalid
  headOid: string | null;     // full 40-hex; UI shortens to 7
  locked: boolean;
  lockReason: string | null;
  isMain: boolean;
  isCurrent: boolean;
  prunable: boolean;          // stale
  valid: boolean;
}
```

`IpcApi` additions (near `listSubmodules`, `types.ts:1032`; mirror the JSDoc style):

```ts
/** All worktrees (main first) with resolved branch/oid/badges. Rejects noRepo | git. */
listWorktrees(repoId: string): Promise<WorktreeInfo[]>;
/** Create a worktree for the EXISTING local branch `branch` at a derived path.
 *  Returns the created row. Rejects noRepo | invalidName | branchNotFound | git. */
addWorktree(repoId: string, branch: string): Promise<WorktreeInfo>;
/** Remove worktree `name` (refuses main/current/locked/dirty). Rejects noRepo | invalidName | git | io. */
removeWorktree(repoId: string, name: string): Promise<void>;
/** Lock worktree `name` with an optional reason. Rejects noRepo | invalidName | git. */
lockWorktree(repoId: string, name: string, reason?: string): Promise<void>;
/** Unlock worktree `name`. Rejects noRepo | invalidName | git. */
unlockWorktree(repoId: string, name: string): Promise<void>;
```

`src/ipc/tauri.ts` (add beside the submodule wrappers, `tauri.ts:391-404`), snake_case command +
camelCase arg keys:

```ts
listWorktrees(repoId: string): Promise<WorktreeInfo[]> {
  return invoke<WorktreeInfo[]>('list_worktrees', { repoId });
},
addWorktree(repoId: string, branch: string): Promise<WorktreeInfo> {
  return invoke<WorktreeInfo>('add_worktree', { repoId, branch });
},
removeWorktree(repoId: string, name: string): Promise<void> {
  return invoke<void>('remove_worktree', { repoId, name });
},
lockWorktree(repoId: string, name: string, reason?: string): Promise<void> {
  return invoke<void>('lock_worktree', { repoId, name, reason: reason ?? null });
},
unlockWorktree(repoId: string, name: string): Promise<void> {
  return invoke<void>('unlock_worktree', { repoId, name });
},
```

Import `WorktreeInfo` alongside `SubmoduleInfo` in `tauri.ts` (`:61`).

---

## 5. Stateful mock (`src/ipc/mock.ts`)

- Import `WorktreeInfo` (with `SubmoduleInfo`, `mock.ts:94`).
- Add `worktrees: WorktreeInfo[]` to `MockRepoState` (near `submodules`, `mock.ts:668`).
- Add `seedWorktrees(kind, graphFixture)` (mirror `seedSubmodules`, `mock.ts:917`) wired into
  `createRepoState` (`mock.ts:985`). Only the **default** repo seeds worktrees (others → `[]`), covering
  every badge: main+current, a clean linked, a locked linked, a stale/prunable linked:

```ts
function seedWorktrees(kind: RepoKind, graphFixture: GraphFixture): WorktreeInfo[] {
  if (kind !== 'default' || graphFixture !== 'default') return [];
  return [
    { name: 'repo', absPath: '/mock/repo', relPath: null, branch: 'main',
      headOid: fixtureOid(1), locked: false, lockReason: null,
      isMain: true, isCurrent: true, prunable: false, valid: true },
    { name: 'feature-login', absPath: '/mock/.worktrees/feature-login', relPath: null,
      branch: 'feature/login', headOid: fixtureOid(3), locked: false, lockReason: null,
      isMain: false, isCurrent: false, prunable: false, valid: true },
    { name: 'release-1.2', absPath: '/mock/.worktrees/release-1.2', relPath: null,
      branch: 'release/1.2', headOid: fixtureOid(4), locked: true, lockReason: 'pinned for QA',
      isMain: false, isCurrent: false, prunable: false, valid: true },
    { name: 'hotfix-stale', absPath: '/mock/.worktrees/hotfix-stale', relPath: null,
      branch: null, headOid: null, locked: false, lockReason: null,
      isMain: false, isCurrent: false, prunable: true, valid: false },
  ];
}
```

- Command methods (add beside the submodule methods, `mock.ts:3071`):
  - `listWorktrees(repoId)` → `structuredClone(state.worktrees)`.
  - `addWorktree(repoId, branch)` → build the derived name/path in TS mirroring §2.4
    (`slug = branch.replace(/[^A-Za-z0-9._-]+/g, '-').replace(/^[-.]+|[-.]+$/g,'')`; collision-suffix
    against existing names), push a new `{ isMain:false, isCurrent:false, locked:false, valid:true,
    prunable:false, branch, headOid: randomOid(), absPath: '/mock/.worktrees/'+name }`, return
    `structuredClone(row)`. If `branch` matches no seeded/known branch, still create (mock list is
    authoritative; the real backend enforces existence).
  - `removeWorktree(repoId, name)` → find; **reject** (throw the shaped error, mirror `mockError`) if
    the row is `isMain` / `isCurrent` / `locked`; else splice it out. (Dirty is not modeled in the
    mock — the seeds are clean.)
  - `lockWorktree(repoId, name, reason?)` → find; set `locked=true`, `lockReason = reason ?? null`.
  - `unlockWorktree(repoId, name)` → find; set `locked=false`, `lockReason=null`.
- Throw shaped errors via the existing mock-error helper (same one the submodule/branch mocks use) so
  the harness exercises the toast path for the main/current/locked refusals.
- No new events/channels.

---

## 6. Frontend

### 6.1 Sidebar "Worktrees" section (`src/components/Sidebar.tsx`)

Add after the Submodules section, matching its styling and the `submodulesCollapsed` collapse pattern
(`worktreesCollapsed` local state). New props on `SidebarProps` (near `:66-69`):

```ts
/** P27 §6.1: worktrees (main first) with resolved branch/badges. */
worktrees: WorktreeInfo[];
/** Right-click a worktree row → open the shared context menu at the cursor. */
onWorktreeContextMenu(name: string, clientX: number, clientY: number): void;
/** Click the section "+" → open the new-worktree branch picker. */
onNewWorktree(anchorX: number, anchorY: number): void;
```

- **Header**: `SectionHeader label="Worktrees"` with an `extra` "+" button (mirror the `extra` slot,
  `Sidebar.tsx:88,106`) whose `onClick` calls `onNewWorktree(e.clientX, e.clientY)`. Unlike Submodules,
  the Worktrees section is **always shown** when a repo is open (the main row is always present).
- **Row** (`WorktreeRow`, sibling of `SubmoduleRow`, `:295`): a `branch-row` showing `branch ?? name`
  as the primary label (glyph `'⌥'` or reuse `'⎇'`), with `title={absPath}`, and a right-aligned badge
  cluster (§6.2). `onContextMenu` → `onWorktreeContextMenu(name, e.clientX, e.clientY)` with
  `e.preventDefault()`, exactly like `SubmoduleRow`.

### 6.2 Badges

Small pills reusing the `branch-badge` CSS + the P19 intent classes (`submodule-badge-*`), display-only:

| condition | label | intent |
|---|---|---|
| `isCurrent` | "current" | ok / green |
| `isMain` (and not current) | "main" | muted |
| `locked` | "locked" (`title={lockReason}`) | warn / amber |
| `prunable` / `!valid` | "stale" | warn / amber |

A row may show more than one (e.g. current+main). `headOid` shortened to 7 chars may render as a muted
suffix (optional, matches commit-row style).

### 6.3 RepoWorkspace state + handlers (`src/components/RepoWorkspace.tsx`)

Mirror the P19 wiring (`:178,318,644-658`): add `worktrees` state, `worktreesReqId` guard,
`refetchWorktrees()` (`ipc.listWorktrees(repoId)`), and `clearWorktrees()`, included in `refreshAll`
and the `repo-changed` / window-focus refresh batch. Pass `worktrees` + `onWorktreeContextMenu` +
`onNewWorktree` into `<Sidebar>`.

Reuse the existing `onOpenRepoPath` prop (already present, `:120-122`) for open-in-tab.

Handlers (mirror `handleUpdateSubmodule` — `setMutating(true)` / `try` / toast / refresh / `finally`):

```ts
async function handleAddWorktree(branch: string) {
  setMutating(true);
  try {
    const wt = await ipc.addWorktree(repoId, branch);
    pushToast('success', `Created worktree for ${branch} at ${wt.absPath}`);
    await refetchWorktrees();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}
async function handleLockWorktree(name: string) { /* ipc.lockWorktree(repoId, name); toast; refetch */ }
async function handleUnlockWorktree(name: string) { /* ipc.unlockWorktree(repoId, name); toast; refetch */ }
async function handleRemoveWorktree(name: string) { // called AFTER the ConfirmDialog
  setMutating(true);
  try {
    await ipc.removeWorktree(repoId, name);
    pushToast('success', `Removed worktree ${name}`);
    await refetchWorktrees();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}
```

`refetchWorktrees` suffices for lock/unlock/add. `remove` deletes a directory but does not change the
CURRENT repo's status/graph (it can never remove the open worktree), so `refetchWorktrees` is enough.
Lock v1 uses no reason prompt (pass `undefined`); a reason field is a Polish add.

### 6.4 Worktree context menu (shared `ContextMenu`)

Add `worktreeMenuItems(wt: WorktreeInfo)` beside `submoduleMenuItems` (`:2260`):

```ts
function worktreeMenuItems(wt: WorktreeInfo): ContextMenuItem[] {
  const gate = mutating || opActive;
  return [
    { label: 'Open in new tab', disabled: wt.isCurrent || !wt.valid,
      onSelect: () => onOpenRepoPath(wt.absPath) },
    { label: 'Lock',   disabled: gate || wt.isMain || wt.locked,
      onSelect: () => void handleLockWorktree(wt.name) },
    { label: 'Unlock', disabled: gate || !wt.locked,
      onSelect: () => void handleUnlockWorktree(wt.name) },
    { label: 'Remove…', disabled: gate || wt.isMain || wt.isCurrent || wt.locked,
      onSelect: () => setPendingWorktreeRemove({ name: wt.name, absPath: wt.absPath }) },
  ];
}
```

`handleWorktreeContextMenu(name, x, y)` looks up the `WorktreeInfo` by name and
`setMenu({ x, y, items: worktreeMenuItems(wt) })` (reuses the single `<ContextMenu>`). Remove is
disabled for main/current/locked in the UI; the backend refuses independently (§2.6).

### 6.5 "New worktree" affordance

The "+" in the section header opens the shared `<ContextMenu>` anchored at the click, listing the
**eligible local branches** — local branches from `branches` state that are NOT already the `branch`
of any row in `worktrees` (a branch can be checked out in only one worktree). Each item's `onSelect`
calls `handleAddWorktree(branch.name)`; the derived path is shown in the resulting success toast (the
backend owns the derivation, §OPEN-1/§OPEN-2). If no branch is eligible, show a single disabled
"No available branches" item.

```ts
function newWorktreeMenuItems(): ContextMenuItem[] {
  const used = new Set(worktrees.map((w) => w.branch).filter(Boolean));
  const eligible = branches.filter((b) => b.kind === 'local' && !used.has(b.name)); // adapt to BranchInfo shape
  if (eligible.length === 0) return [{ label: 'No available branches', disabled: true, onSelect: () => {} }];
  return eligible.map((b) => ({ label: b.name, disabled: mutating || opActive,
    onSelect: () => void handleAddWorktree(b.name) }));
}
```

This reuses `ContextMenu` (already the app's menu primitive) → fully harness-verifiable with no new UI
component. (A richer "New worktree" dialog previewing the derived path is a Polish enhancement.)

### 6.6 Remove confirm + open-in-tab

- **Confirm**: add `pendingWorktreeRemove: { name: string; absPath: string } | null` state (mirror the
  branch-delete pending state, `:2988-3078`) and a `<ConfirmDialog>` with `confirmLabel="Remove
  worktree"`, body naming the **absolute path** and warning the directory will be deleted from disk
  (per §OPEN-3 the backend refuses if it has uncommitted changes). `onConfirm` → `handleRemoveWorktree`.
- **Open-in-tab**: reuse the existing `onOpenRepoPath` thread (`App.tsx:753` already passes
  `onOpenRepoPath={(p) => void openTab(p)}` to `<RepoWorkspace>` for P19). A worktree is an ordinary
  repo directory → `openRepo(absPath)` opens it as its own tab. **No new command/event/tab type.**

---

## 7. Testing (AI gate) — `crates/bonsai-core/tests/worktree_cli.rs`

CLI-oracle suite mirroring `tests/submodule_cli.rs` (`require_git!` skip when `git` is absent).
**Env (tester):** `TMP`/`TEMP` → `D:\Temp`; run `cargo test` and `clippy` **sequentially** (per
memory); scratch repos under `D:\Temp\bonsai-scratch`; forward slashes in Bash-tool paths. NEVER touch
a real repo.

### 7.1 Fixture (built with git2 / the `git` CLI)

A scratch repo with ≥2 commits on `main` and a second local branch `feature`. All worktrees created
under a temp parent so the derived `.worktrees/` container is inside the scratch area.

### 7.2 Assertions (cross-check with `git worktree list --porcelain`)

1. **`list_worktrees` on a plain repo** returns exactly one row: `isMain==true`, `isCurrent==true`,
   `branch=="main"`, `valid==true`, `prunable==false`. Cross-check `git worktree list --porcelain`
   shows a single `worktree`/`bare`? no — a single main entry.
2. **`add_worktree("feature")`** creates a worktree: the returned row has `isMain==false`,
   `branch=="feature"`, `absPath` under `<parent>/.worktrees/feature`, `valid==true`. Cross-check
   `git worktree list --porcelain` now lists two worktrees and the new path/branch match; the
   directory exists and `git -C <path> rev-parse --abbrev-ref HEAD` == `feature`.
3. **derivation + collision**: a second `add_worktree` for a differently-named branch pointing to a
   colliding slug yields `-2` suffix; assert the path/name. Sanitizer: a `feat/x` branch → `feat-x`.
4. **`add_worktree` on a non-existent branch** → `BranchNotFound`; on a branch already checked out
   (e.g. `main`, the current HEAD) → `Git` (cross-check `git worktree add` refuses likewise).
5. **`lock_worktree` / `unlock_worktree`**: after lock, `list_worktrees` shows `locked==true` (+ reason
   if given) and `git worktree list --porcelain` shows `locked`; after unlock, both clear.
6. **`remove_worktree` refusals**: main by name → `Git`; the current worktree → `Git`; a locked
   worktree → `Git` ("unlock it first"); a dirty worktree (touch an untracked file inside it) → `Git`
   ("uncommitted changes"). None of these delete anything (assert the dir still exists).
7. **`remove_worktree` happy path**: on a clean, unlocked, non-current worktree → `Ok(())`; assert the
   working directory is gone from disk AND `git worktree list --porcelain` no longer lists it (prune
   removed the admin entry too).
8. **Wire shape + sanitizer unit tests** (§2.9) run under `cargo test` as well.

### 7.3 Browser-harness (orchestrator-verifiable)

- `pnpm build` + `tsc` clean.
- Sidebar shows a **Worktrees** section listing the four seeded rows with correct badges:
  current+main, a clean linked, a locked linked (title shows the reason), a stale linked (screenshot
  evidence).
- Right-click rows → menu with correct disabled states: main/current cannot Remove or Lock; locked
  shows Unlock enabled + Lock/Remove disabled; Open-in-tab disabled on current and on the stale
  (`!valid`) row.
- "+" opens the branch picker listing eligible local branches (excluding branches already checked out
  by a worktree); selecting one adds a row + toasts the derived path.
- Lock/Unlock flip the badge; Remove opens the ConfirmDialog naming the absolute path, and confirming
  removes the row; the mock rejects removing main/current/locked with an error toast.
- "Open in new tab" on the clean linked worktree opens a second tab (mock `openRepo(absPath)`).

### 7.4 USER CHECKPOINT (native `pnpm tauri dev`, real repo)

The Worktrees section lists real worktrees with correct badges; "+" → pick a branch creates a real
worktree at `<parent>/.worktrees/<branch>` (cross-check `git worktree list`); Lock/Unlock reflect in
`git worktree list --porcelain`; Remove deletes the directory and prunes it (cross-check
`git worktree list`); "Open in new tab" opens the worktree as its own repo tab; main/current cannot be
removed.

---

## 8. Sub-increments (each a single fresh-context senior-dev pass; read-only list first)

### P27a — Rust core (read) + list command + IPC list + mock list
- New `crates/bonsai-core/src/git/worktree.rs` (§2): `WorktreeInfo`, `list_worktrees` + all helpers
  (`main_workdir`, `build_main_row`, `build_linked_row`, `read_head`, `canonical`/`to_fwd`), plus the
  `sanitize_slug` + `derive_worktree` scaffolding (used by P27b but land it here) and the §2.9 unit
  tests. `git/mod.rs` `pub mod worktree;`.
- `commands.rs`: `list_worktrees` + `_inner`; `lib.rs` registration (list only).
- IPC: `types.ts` `WorktreeInfo` + `listWorktrees`; `tauri.ts` wrapper; `mock.ts` `worktrees` state +
  `seedWorktrees` + `listWorktrees`.
- **Acceptance**: `cargo check`/`clippy` clean; unit tests pass; `pnpm build`/`tsc` clean; harness shows
  the four seeded rows with correct badges (list is read-only; no ops yet).

### P27b — Rust mutating ops + commands + IPC + stateful mock
- `worktree.rs`: `add_worktree` (§2.5), `remove_worktree` (§2.6, incl. `is_dirty`), `lock_worktree` /
  `unlock_worktree` (§2.7), `open_linked`.
- `commands.rs`: four commands + `_inner`s; `lib.rs` registration.
- IPC: four `IpcApi` methods + `tauri.ts` wrappers; `mock.ts` `addWorktree`/`removeWorktree`/
  `lockWorktree`/`unlockWorktree` mutating the slice, with shaped-error rejections for main/current/
  locked remove.
- **Acceptance**: `cargo check`/`clippy` clean; the §7.2 CLI-oracle suite (`worktree_cli.rs`) passes
  (create/remove/lock/unlock cross-checked with `git worktree list --porcelain`, all refusals);
  `pnpm build`/`tsc` clean; harness ops mutate the mock list.

### P27c — Frontend section + menu + New affordance + confirm + open-in-tab
- `Sidebar.tsx`: Worktrees section + `WorktreeRow` + badges + props (`worktrees`,
  `onWorktreeContextMenu`, `onNewWorktree`) (§6.1-6.2).
- `RepoWorkspace.tsx`: `worktrees` state + `refetchWorktrees` (into `refreshAll` + repo-changed batch),
  four handlers, `worktreeMenuItems`, `newWorktreeMenuItems`, `handleWorktreeContextMenu`,
  `pendingWorktreeRemove` + `<ConfirmDialog>`, Sidebar wiring (§6.3-6.6). Reuse the existing
  `onOpenRepoPath` prop for open-in-tab.
- **Acceptance**: `pnpm build`/`tsc` clean; harness passes §7.3 (badges, menu disabled-states, branch
  picker, lock/unlock/add/remove mutations, remove-refusal toast, open-in-tab second tab).

---

## 9. File touch list

- `crates/bonsai-core/src/git/worktree.rs` (**new**), `crates/bonsai-core/src/git/mod.rs`
  (`pub mod worktree;`).
- `src-tauri/src/commands.rs` (import + 5 command/_inner pairs), `src-tauri/src/lib.rs` (register 5).
- `crates/bonsai-core/tests/worktree_cli.rs` (**new**; reuse `tests/common`).
- `src/ipc/types.ts` (`WorktreeInfo` + 5 `IpcApi` methods), `src/ipc/tauri.ts` (5 wrappers + import),
  `src/ipc/mock.ts` (`worktrees` state + `seedWorktrees` + 5 methods + shaped-error rejections).
- `src/components/Sidebar.tsx` (Worktrees section + `WorktreeRow` + badges + 3 props),
  `src/components/RepoWorkspace.tsx` (state, `refetchWorktrees`, 4 handlers, 2 menu builders,
  context-menu handler, `pendingWorktreeRemove` + ConfirmDialog, Sidebar wiring).
- `src/components/*.css` — reuse existing `branch-badge` / `submodule-badge-*` pill classes; add a
  `worktree-badge-*` alias only if a new intent is needed (avoid).
- **No new `AppError` variant; no new events/channels; `notify` watcher, `status.rs`, and the graph
  walk unchanged.**

---

## AI gate (orchestrator-verifiable)

- `cargo check` + `cargo clippy` clean on `bonsai-core` + `src-tauri` (run sequentially).
- `cargo test` green: `worktree.rs` unit tests (wire shape, sanitizer) + `worktree_cli.rs` CLI-oracle
  suite (§7.2 #1-8) cross-checked against `git worktree list --porcelain` on scratch repos.
- `pnpm build` + `tsc` clean.
- Browser harness (`VITE_MOCK_IPC=1`): Worktrees section renders the four seeded rows with correct
  badges; context menu disabled-states correct; "+" branch picker lists eligible branches; add/lock/
  unlock/remove mutate the mock; remove-of-main/current/locked toasts an error; open-in-tab opens a
  second tab (screenshots + DOM checks per §7.3).

## USER CHECKPOINT (native `pnpm tauri dev`, real repo)

- Worktrees section lists real worktrees (main + linked) with correct branch/badges.
- "+" → pick a branch creates a real worktree at the derived `.worktrees/<branch>` path — cross-checked
  with `git worktree list`.
- Lock / Unlock reflected in `git worktree list --porcelain`.
- Remove (confirm dialog naming the path) deletes the directory and prunes it — cross-checked with
  `git worktree list`; a dirty/locked/current/main worktree is refused with a clear message.
- "Open in new tab" opens the worktree as its own repo tab in the existing multi-repo flow.
