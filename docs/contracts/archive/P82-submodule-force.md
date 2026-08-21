# P82 — Submodule deinit/remove force (backend contract) — F-A7-7

Safety gap **F-A7-7**: `deinit_submodule` / `remove_submodule` in
`crates/bonsai-core/src/git/submodule.rs` always shell out with `-f`
(`deinit_args` hard-codes `-f`; `remove_submodule` also runs `git rm -f`). That
silently destroys uncommitted work inside a dirty submodule behind only the
generic UI confirm. Fix: make `-f` an explicit opt-in; the plain op **refuses**
(returns a typed outcome, mutating nothing) when the submodule worktree is dirty.

Pairs with the UI contract `docs/contracts/P82-submodule-force-ui.md` (Flow A —
attempt-then-offer-force). Orchestrator decision: model the refusal as a **typed
outcome enum** (mirroring `ApplyStashOutcome` / `stashPopConflicts`), NOT a new
`AppError` variant — keeps the P75 `AppErrorKind` codegen set untouched.

---

## 1. Module boundaries (files touched)

| File | Responsibility change |
|---|---|
| `crates/bonsai-core/src/git/submodule.rs` | add 2 outcome enums; thread `force: bool`; add `is_submodule_dirty`; make `-f` conditional in `deinit_args` + new `rm_args` |
| `crates/bonsai-core/src/git/submodule_tests.rs` | update `deinit_args_exact` + add rm-args/force/dirty tests (tester) |
| `src-tauri/src/commands/submodules.rs` | `force` param + new return types on both commands + `_inner` |
| `src-tauri/src/commands/shared.rs` (L96) | re-export the two new outcome types |
| `src/ipc/types.ts` | 2 new outcome types; 2 method sigs gain `force`, new return types |
| `src/ipc/tauri.ts` (L730–736) | pass `force` in both invoke wrappers; new return types |
| `src/ipc/index.ts` (near L34) | re-export the 2 new types |
| `src/ipc/mock/handlers/submodules.ts` | accept `force`; dirty-refusal seam; return outcome objects |

No new files. No `AppError` variant. No new Tauri command / event / channel.

---

## 2. Outcome enums

Data-less tagged enums (both refusal variants share the name `dirtyNeedsForce`).

### 2.1 Rust (`submodule.rs`)

```rust
/// Result of `deinit_submodule`. Wire: tagged "kind", camelCase (same recipe as
/// `ApplyStashOutcome`). `DirtyNeedsForce` is returned WITHOUT mutating anything
/// when `force == false` and the submodule worktree is dirty (F-A7-7).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubmoduleDeinitOutcome {
    /// Plain (`force=false`, clean) or forced (`force=true`) deinit succeeded.
    Deinitialized,
    /// `force=false` and the worktree is dirty; nothing was changed. The UI
    /// re-invokes with `force=true` after an explicit danger confirm.
    DirtyNeedsForce,
}

/// Result of `remove_submodule`. Wire: tagged "kind", camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SubmoduleRemoveOutcome {
    /// Full teardown succeeded (deinit → `git rm` → drop `.git/modules/<name>`).
    Removed,
    /// `force=false` and the worktree is dirty; nothing was changed.
    DirtyNeedsForce,
}
```

### 2.2 TypeScript (`src/ipc/types.ts`, place beside `SubmoduleInfo`)

```ts
/** Result of `deinitSubmodule` (P82). Mirrors Rust `SubmoduleDeinitOutcome`
 *  (serde tagged "kind", camelCase). `dirtyNeedsForce` = the plain op refused
 *  because the submodule worktree is dirty; re-invoke with `force=true`. */
export type SubmoduleDeinitOutcome =
  | { kind: 'deinitialized' }
  | { kind: 'dirtyNeedsForce' };

/** Result of `removeSubmodule` (P82). Mirrors Rust `SubmoduleRemoveOutcome`. */
export type SubmoduleRemoveOutcome =
  | { kind: 'removed' }
  | { kind: 'dirtyNeedsForce' };
```

---

## 3. Dirtiness check (BEFORE any mutation)

Use libgit2 status, never `git` stderr matching. Add one private helper reusing
the exact bitflags that `classify_status` step 3 already treats as "the
submodule's OWN worktree/index is dirty":

```rust
/// True when submodule `name`'s own worktree/index holds uncommitted work that a
/// force deinit/rm would destroy: staged (WD_INDEX_MODIFIED), unstaged
/// (WD_WD_MODIFIED), or untracked (WD_UNTRACKED) changes inside it. Uses the
/// same status path as `list_submodules` (`submodule_status(name, Ignore::None)`).
/// NOT dirty: uninitialized, absent workdir, or merely out-of-sync (a different
/// but committed pinned commit — no uncommitted work is lost).
fn is_submodule_dirty(repo: &git2::Repository, name: &str) -> Result<bool, AppError> {
    use git2::SubmoduleStatus as S;
    let flags = repo.submodule_status(name, git2::SubmoduleIgnore::None)?;
    Ok(flags.intersects(S::WD_INDEX_MODIFIED | S::WD_WD_MODIFIED | S::WD_UNTRACKED))
}
```

This is the `WD_*` dirty set — exactly what `classify_status` maps to
`ModifiedWorkdir`. It deliberately EXCLUDES `WD_UNINITIALIZED` and the
`OutOfSync` pointer flags (`INDEX_*` / `WD_MODIFIED`), because those carry no
uncommitted work to lose.

### 3.1 Edge-case table

| Situation | `submodule_status` flags | `is_submodule_dirty` | `force=false` behaviour |
|---|---|---|---|
| Clean, up-to-date | none of WD_* dirty | false | proceed, plain op (no `-f`) succeeds |
| Uninitialized / not checked out | `WD_UNINITIALIZED` | false | proceed (plain deinit/rm is a safe no-op/clean) |
| Workdir dir missing | no WD_* dirty flags set | false | proceed |
| Out-of-sync only (diff pinned commit, clean tree) | `WD_MODIFIED`/`INDEX_*` | false | proceed — no uncommitted work at risk |
| Dirty tree (staged/unstaged/untracked) | `WD_INDEX_MODIFIED`/`WD_WD_MODIFIED`/`WD_UNTRACKED` | true | **`dirtyNeedsForce`, zero mutation** |
| Out-of-sync AND dirty tree | `WD_MODIFIED` + `WD_WD_MODIFIED` | true | `dirtyNeedsForce` (dirty flags still match) |

**Flagged residuals (for orchestrator):**
- **Nested-submodule dirtiness** is not reliably reflected by the immediate
  `submodule_status` of the parent, so a submodule whose only dirtiness lives in
  its OWN sub-submodule may report `is_submodule_dirty=false`; the plain shell-out
  then either succeeds (git also ignores it) or git refuses → surfaces as the
  existing generic `git` error toast (not a dead-end for the common case).
  Recommendation: accept for v1 — matches the "uncommitted work in THIS worktree"
  scope of F-A7-7; deeper recursion is out of scope.
- If our verdict is "clean" but plain `git` still refuses (rare divergence), the
  op returns a generic `git` error rather than `dirtyNeedsForce`, so the UI shows
  the standard error toast instead of the escalation dialog. Acceptable; noted.

---

## 4. `deinit_args` / `rm_args` change

`-f` becomes conditional on `force` in BOTH shell-outs the remove teardown uses.

```rust
/// argv for `git submodule deinit [-f] -- <path>`. `-f` only when `force`.
/// `path` is ALWAYS the final token, after `--`.
fn deinit_args(path: &str, force: bool) -> Vec<String> {
    let mut v = vec!["submodule".into(), "deinit".into()];
    if force { v.push("-f".into()); }
    v.push("--".into());
    v.push(path.into());
    v
}

/// argv for `git rm [-f] -- <path>`. `-f` only when `force`.
fn rm_args(path: &str, force: bool) -> Vec<String> {
    let mut v = vec!["rm".into()];
    if force { v.push("-f".into()); }
    v.push("--".into());
    v.push(path.into());
    v
}
```

Byte-exact argv (tester updates `deinit_args_exact` + adds `rm_args_exact`):
- `deinit_args(p, true)`  → `["submodule","deinit","-f","--",p]`
- `deinit_args(p, false)` → `["submodule","deinit","--",p]`
- `rm_args(p, true)`      → `["rm","-f","--",p]`
- `rm_args(p, false)`     → `["rm","--",p]`

---

## 5. Core function signatures (`submodule.rs`)

```rust
pub fn deinit_submodule(
    workdir: &Path,
    runner: &dyn GitRunner,
    name: &str,
    force: bool,
) -> Result<SubmoduleDeinitOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    let path = submodule_path(&repo, name)?;            // validates name
    if !force && is_submodule_dirty(&repo, name)? {
        return Ok(SubmoduleDeinitOutcome::DirtyNeedsForce);   // zero mutation
    }
    runner.run(&deinit_args(&path, force), workdir)?;
    Ok(SubmoduleDeinitOutcome::Deinitialized)
}

pub fn remove_submodule(
    workdir: &Path,
    runner: &dyn GitRunner,
    name: &str,
    force: bool,
) -> Result<SubmoduleRemoveOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    validate_modules_name(name)?;                        // F-A7-2, before any step
    let path = submodule_path(&repo, name)?;
    if !force && is_submodule_dirty(&repo, name)? {
        return Ok(SubmoduleRemoveOutcome::DirtyNeedsForce);   // zero mutation
    }
    runner.run(&deinit_args(&path, force), workdir)?;
    runner.run(&rm_args(&path, force), workdir)?;
    remove_cached_git_dir(&repo, name);                  // best-effort, unchanged
    Ok(SubmoduleRemoveOutcome::Removed)
}
```

The dirty check runs AFTER name validation/`submodule_path` (so unknown/blank
name still errors first with `invalidName`/`git`) and BEFORE any `runner.run`.

---

## 6. Command layer (`src-tauri/src/commands/submodules.rs`)

Add `force: bool` param and change return types (both public command + `_inner`).
Everything else (spawn_blocking, error mapping, no `repo-changed` emit) unchanged.

```rust
#[tauri::command]
pub async fn deinit_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    force: bool,
) -> Result<SubmoduleDeinitOutcome, AppError> {
    deinit_submodule_inner(state.inner(), &repo_id, name, force).await
}

pub(crate) async fn deinit_submodule_inner(
    state: &AppState, repo_id: &str, name: String, force: bool,
) -> Result<SubmoduleDeinitOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        submodule::deinit_submodule(&path, &SpawnGitRunner, &name, force)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}

#[tauri::command]
pub async fn remove_submodule(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
    force: bool,
) -> Result<SubmoduleRemoveOutcome, AppError> {
    remove_submodule_inner(state.inner(), &repo_id, name, force).await
}

pub(crate) async fn remove_submodule_inner(
    state: &AppState, repo_id: &str, name: String, force: bool,
) -> Result<SubmoduleRemoveOutcome, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        submodule::remove_submodule(&path, &SpawnGitRunner, &name, force)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

`shared.rs` L96 re-export:
```rust
pub(crate) use bonsai_core::git::submodule::{
    self, SubmoduleDeinitOutcome, SubmoduleInfo, SubmoduleRemoveOutcome,
};
```

Tauri arg-name note: `force` (camelCase-neutral) matches the JS payload key
`force` verbatim — no rename needed.

---

## 7. IPC lockstep (pre-P75 — hand-written facade, all four spots)

**7.1 `src/ipc/types.ts`**
- Add the two outcome types from §2.2.
- Replace the two method signatures:
```ts
/** P60d/P82: deinit — clear config + empty worktree; keep .gitmodules.
 *  `force=false` refuses (`dirtyNeedsForce`) when the submodule worktree is
 *  dirty, mutating nothing; re-invoke with `force=true` to discard.
 *  Rejects noRepo | invalidName | git. */
deinitSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleDeinitOutcome>;
/** P60d/P82: remove entirely (deinit + git rm + drop .git/modules). DESTRUCTIVE.
 *  `force` semantics as `deinitSubmodule`. Rejects noRepo | invalidName | git. */
removeSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleRemoveOutcome>;
```

**7.2 `src/ipc/tauri.ts` (L730–736)**
```ts
deinitSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleDeinitOutcome> {
  return invoke<SubmoduleDeinitOutcome>('deinit_submodule', { repoId, name, force });
},
removeSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleRemoveOutcome> {
  return invoke<SubmoduleRemoveOutcome>('remove_submodule', { repoId, name, force });
},
```

**7.3 `src/ipc/index.ts` (type re-export block, near L34)**
Add `SubmoduleDeinitOutcome,` and `SubmoduleRemoveOutcome,` to the exported type
list (keep alphabetical grouping consistent with neighbours).

**7.4 `src/ipc/mock/handlers/submodules.ts`**
Both handlers accept `force` and return the outcome object; add the dirty-refusal
seam (UI contract §8) so the escalation + force retry are browser-verifiable:

```ts
// dirty := a modifiedWorkdir fixture row OR the `?submodule=dirty` seam.
function submoduleDirty(sub: SubmoduleInfo | undefined): boolean {
  return sub?.status === 'modifiedWorkdir' || query('submodule') === 'dirty';
}

async deinitSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleDeinitOutcome> {
  await delay(200);
  const state = requireRepo(repoId);
  failSeam(name);
  const sub = state.submodules.find((s) => s.name === name);
  if (!force && submoduleDirty(sub)) return { kind: 'dirtyNeedsForce' };  // zero mutation
  if (sub !== undefined) { sub.status = 'uninitialized'; sub.wtOid = null; }
  return { kind: 'deinitialized' };
},

async removeSubmodule(repoId: string, name: string, force: boolean): Promise<SubmoduleRemoveOutcome> {
  await delay(200);
  const state = requireRepo(repoId);
  failSeam(name);
  const sub = state.submodules.find((s) => s.name === name);
  if (!force && submoduleDirty(sub)) return { kind: 'dirtyNeedsForce' };  // zero mutation
  const idx = state.submodules.findIndex((s) => s.name === name);
  if (idx !== -1) state.submodules.splice(idx, 1);
  return { kind: 'removed' };
},
```
Also ensure `fixtures/submodules.ts` seeds at least one `modifiedWorkdir` row in
the default repo (UI §8). Mock caveat: `SubmoduleInfo.status` only carries the
classified enum, so the mock cannot reproduce the "outOfSync AND dirty" case
(which the backend still treats as dirty); the `?submodule=dirty` seam covers
forcing that path in the harness.

---

## 8. UI-designer dependency — CONFIRMED SATISFIED (with one wiring note)

The UI (§7) requires the dirty refusal to be **structurally distinguishable** so
it can branch without string-matching libgit2. The outcome-enum approach
satisfies this: the UI branches on `outcome.kind === 'dirtyNeedsForce'`.

**Wiring note for orchestrator / UI implementer:** the refusal is now a
**resolved value, not a thrown error**. UI contract §4 describes branching inside
`useSubmoduleActions.ts` `runRowOp`'s *catch*. That must change: inspect the
**resolved** outcome instead —
```ts
const outcome = await ipc.deinitSubmodule(repoId, name, force);
if (outcome.kind === 'dirtyNeedsForce' && !force) { deps.onSubmoduleDirtyRefused(name, 'deinit'); return; }
// success path (kind 'deinitialized'/'removed') → existing success toast
```
The `catch` keeps handling every *genuine* error (generic toast) unchanged. This
is strictly better than the §7 `AppError`-based plan (no error is thrown for the
expected refusal) and keeps the P75 error-code set untouched. No UI copy, token,
component, or geometry change is implied — the UI contract otherwise stands.

---

## 9. Acceptance criteria (measurable)

1. `deinit_args(p, true)` / `rm_args(p, true)` include `-f`; `(p, false)` omit it
   — byte-exact (§4). `deinit_args_exact` updated; `rm_args_exact` added.
2. **`force=false` + dirty ⇒ `dirtyNeedsForce`, ZERO mutation.** Test: dirty a
   submodule worktree, call `deinit_submodule(.., false)` / `remove_submodule(.., false)`;
   assert the returned `kind`, and assert the submodule config, worktree contents,
   index gitlink, and `.gitmodules` are byte-identical to before (no runner call).
3. **`force=false` + clean ⇒ succeeds without `-f`.** Test with a fake/recording
   `GitRunner`: assert the recorded argv contains NO `-f`, and the outcome is
   `Deinitialized` / `Removed`.
4. **`force=true` ⇒ discards and succeeds.** Dirty submodule + `force=true`
   returns `Deinitialized`/`Removed` and the recorded argv includes `-f`.
5. Edge cases (§3.1): uninitialized, absent workdir, and out-of-sync-only all
   report `is_submodule_dirty=false` and proceed under `force=false`.
6. Name validation still precedes the dirty check: blank name → `invalidName`,
   unknown name → `git`, unsafe `.git/modules` name (remove) → `git` refusal —
   all before any status read or mutation.
7. Serde round-trip: `SubmoduleDeinitOutcome::DirtyNeedsForce` →
   `{"kind":"dirtyNeedsForce"}`; `Deinitialized` → `{"kind":"deinitialized"}`;
   `SubmoduleRemoveOutcome::Removed` → `{"kind":"removed"}`.
8. `cargo check` + `clippy -D warnings` clean; `tsc` clean; mock compiles.
9. **Browser harness:** with the `modifiedWorkdir` fixture (or `?submodule=dirty`),
   Deinitialize/Remove opens the force-escalation dialog (no mutation); confirming
   re-invokes with `force=true` and the row updates. Clean rows succeed directly.
   No USER CHECKPOINT (UI §8).
