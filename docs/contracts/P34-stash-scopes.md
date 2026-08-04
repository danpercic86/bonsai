# P34 — Stash Scopes + Staging-Area Affordance

Extends stash creation with a **scope** (`all` | `allWithUntracked` | `staged`) and adds a stash
control to the RIGHT-hand staging panel. Builds directly on the shipped P9 stash stack
(`crates/bonsai-core/src/git/stash.rs`, `create_stash`/`apply_stash`/`pop_stash`/`list_stashes`).

Invariant recap: Rust owns all git logic (including the new `staged` plumbing); IPC carries the
compact scope tag; the mock IPC must serve all three scopes so the browser harness keeps working.

---

## A. Scope plumbing (all layers)

### A.1 Rust — scope enum + core signature (`stash.rs`)

Replace the `include_untracked: bool` parameter with a scope enum. The enum lives in `stash.rs`
next to `CreateStashResult`.

```rust
/// Which changes a `create_stash` call captures. Wire: camelCase (matches the TS union).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StashScope {
    /// Staged + unstaged tracked changes; untracked left in place. → StashFlags::DEFAULT.
    All,
    /// Adds untracked files. → StashFlags::DEFAULT | INCLUDE_UNTRACKED.
    AllWithUntracked,
    /// ONLY the staged (index-vs-HEAD) changes; unstaged edits stay in the worktree.
    /// No native libgit2 flag — hand-rolled (see §B).
    Staged,
}

pub fn create_stash(
    workdir: &Path,
    message: Option<&str>,
    scope: StashScope,
) -> Result<CreateStashResult, AppError>;
```

Dispatch inside `create_stash` (keep the existing `require_clean` + `resolve_signature` up front,
unchanged for all scopes):

- `All` → `stash_save2(&sig, message, Some(StashFlags::DEFAULT))` (existing NotFound→`created:false`).
- `AllWithUntracked` → `stash_save2(&sig, message, Some(DEFAULT | INCLUDE_UNTRACKED))` (same
  NotFound handling). This is byte-for-byte the current `include_untracked:true` behavior.
- `Staged` → private helper `create_staged_stash(&mut repo, &sig, message)` (§B).

`CreateStashResult { created: bool }` is unchanged.

### A.2 Rust — Tauri command (`src-tauri/src/commands.rs:1879`)

```rust
#[tauri::command]
pub async fn create_stash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    message: Option<String>,
    scope: stash::StashScope,     // deserialized from the camelCase wire string
) -> Result<CreateStashResult, AppError>;
```

`create_stash_inner` forwards `scope` into `spawn_blocking(move || stash::create_stash(&path,
message.as_deref(), scope))`. `StashScope` is `Copy`, so no clone needed. Import `StashScope`
alongside the existing `stash::` uses. Registration in `src-tauri/src/lib.rs:102` is unchanged
(same command name). Still does NOT emit `repo-changed`.

### A.3 TypeScript types (`src/ipc/types.ts`)

New union (place next to `ApplyStashOutcome` / `CreateStashResult`, ~line 529):

```ts
/** Which changes a createStash call captures. Mirrors Rust `StashScope` (camelCase). */
export type StashScope = 'all' | 'allWithUntracked' | 'staged';
```

Update the `IpcApi` entry (currently `src/ipc/types.ts:1237`):

```ts
/** Stash the worktree per `scope`. message=null → git default. created:false == nothing
 *  in that scope to stash (NOT an error). `scope: 'staged'` captures only index-vs-HEAD
 *  changes, leaving unstaged edits in the worktree. Rejects
 *  operationInProgress | configMissing | git | noRepo. */
createStash(
  repoId: string,
  message: string | null,
  scope: StashScope,
): Promise<CreateStashResult>;
```

Export `StashScope` from `src/ipc/index.ts` (add to the type re-export block alongside
`CreateStashResult`).

### A.4 tauri.ts (`src/ipc/tauri.ts:359`)

```ts
createStash(
  repoId: string,
  message: string | null,
  scope: StashScope,
): Promise<CreateStashResult> {
  return invoke<CreateStashResult>('create_stash', { repoId, message, scope });
},
```

Import `StashScope` in the type-import block (`src/ipc/tauri.ts:38` area).

### A.5 mock (`src/ipc/mock.ts:3019`)

Signature `async createStash(repoId, _message, scope: StashScope)`. Behavior per scope, mutating
`state.status` (`staged` / `unstaged` / `untracked` arrays) and the `state.stashes` stack. The stack
push + `baseOid = headNodeId` derivation is unchanged from today; only the "nothing to stash" test
and the post-state cleanup differ:

| scope | created:false when | worktree post-state |
|-------|--------------------|---------------------|
| `all` | `staged.length===0 && unstaged.length===0` | `staged=[]`, `unstaged=[]`, `untracked` unchanged |
| `allWithUntracked` | `staged`, `unstaged`, `untracked` all empty | `staged=[]`, `unstaged=[]`, `untracked=[]` |
| `staged` | `staged.length===0` | `staged=[]`, `unstaged` unchanged, `untracked` unchanged |

Import `StashScope` in the mock type-import block (`src/ipc/mock.ts:76` area). The mock is
file-level coarse: it cannot split a single path that appears in both `staged` and `unstaged`; for
`staged` it simply clears `staged` and leaves `unstaged` intact. Acceptable for the harness (flagged
§E).

---

## B. The `staged` algorithm (data-safety critical)

### B.0 Constraint

libgit2 exposes no `--staged` stash flag (only DEFAULT / KEEP_INDEX / INCLUDE_UNTRACKED /
INCLUDE_IGNORED). We must produce a **real, libgit2-compatible stash entry** so the EXISTING
`apply_stash` / `pop_stash` (which call `repo.stash_apply` with the SAFE default — **no
REINSTATE_INDEX**) restore the staged changes correctly, and `list_stashes` (which reads the
`refs/stash` reflog) lists it.

Because apply/pop never reinstate the index, popping a `staged` stash restores its changes to the
worktree as **unstaged** edits — exactly like `git stash pop` without `--index`. The stash entry is
therefore built so that `diff(base_tree → stash_tree)` == the staged changes; apply replays that
diff into the worktree. See flagged ambiguity F-1.

### B.1 Approaches evaluated

**Approach 1 — hand-roll the stash commit (RECOMMENDED).** Build the git-standard stash object
graph directly, then transform index+worktree ourselves. Produces a clean staged-only entry;
full control over rollback. Cost: we reproduce a slice of libgit2's stash format.

**Approach 2 — native-stash swap.** `stash_save2(DEFAULT)` (stashes staged+unstaged, cleans the
tree), then re-apply only the unstaged portion back and rewrite the entry to staged-only. Rejected:
the resulting entry still records staged+unstaged, so "rewrite to staged-only" is as much work as
Approach 1 *plus* a wider mutation window (the worktree is fully cleaned mid-operation, so a crash
between phases loses the unstaged edits until pop). Strictly less safe.

**Recommendation: Approach 1.** The worktree-restore step (a 3-way merge) is identical work in both;
Approach 1 has the smaller mutation window and a trivial rollback (`stash_drop(0)`).

### B.2 Algorithm (pseudocode)

```
fn create_staged_stash(repo: &mut Repository, sig: &Signature, message: Option<&str>) -> CreateStashResult:
    # ---- read-only analysis; NOTHING mutated in this block ----
    head_commit = repo.head()?.peel_to_commit()?          # errors on unborn HEAD → AppError::Git
    head_tree   = head_commit.tree()?
    index       = repo.index()?
    index_tree  = repo.find_tree(index.write_tree()?)?     # HEAD + staged  (== "B")

    staged_deltas = repo.diff_tree_to_index(Some(&head_tree), Some(&index), None)?
    if staged_deltas.deltas().len() == 0:
        return CreateStashResult { created: false }        # AC (v): nothing staged

    unstaged_deltas = repo.diff_index_to_workdir(Some(&index), None)?   # excludes untracked by default
    changed = union(paths(staged_deltas), paths(unstaged_deltas))       # tracked paths only

    # Build an in-memory tree of the current WORKTREE content for `changed` paths (== "C").
    # Untracked paths are never added → untracked stays out of every tree.
    wt = Index::new()?
    wt.read_tree(&head_tree)?
    for p in changed:
        if worktree_file_exists(p): wt.add_path(p)?        # reads workdir bytes
        else:                       wt.remove_path(p)?     # deletion in worktree
    worktree_tree = repo.find_tree(wt.write_tree_to(repo)?)?

    # Desired final worktree = HEAD + unstaged  ==  merge(base=index_tree, ours=head_tree, theirs=worktree_tree).
    # base=B (staged), ours=A (HEAD), theirs=C (worktree): result = A + (C − B) = unstaged applied to HEAD.
    merged = repo.merge_trees(&index_tree, &head_tree, &worktree_tree, None)?
    if merged.has_conflicts():
        # Staged and unstaged edits overlap on some path → cannot cleanly split. Bail BEFORE any
        # mutation. No stash entry created, index/worktree untouched.
        return Err(AppError::Git("staged and unstaged changes overlap on <path>; \
                                  unstage or commit first"))
    merged_tree = repo.find_tree(merged.write_tree_to(repo)?)?

    # ---- build stash objects (unreferenced commits; still no index/worktree mutation) ----
    branch = current_branch_name(repo)                     # "main", or "(no branch)" if detached
    short  = head_commit.id() short-hex
    i_msg  = format!("index on {branch}: {short} {summary}")
    w_msg  = message.unwrap_or(&format!("WIP on {branch}: {short} {summary}"))
    i = repo.commit(None, sig, sig, &i_msg, &index_tree, &[&head_commit])?              # index commit
    w = repo.commit(None, sig, sig,  w_msg, &index_tree, &[&head_commit, &i_commit])?   # stash commit
    #   NOTE: w.tree == index_tree (staged), NOT the dirty worktree → base→w diff == staged changes.

    # ---- MUTATION WINDOW (rollback on any failure below) ----
    prev_stash = repo.find_reference("refs/stash").ok().map(|r| r.target())   # for rollback
    push_stash_ref(repo, w, w_msg)?          # append refs/stash reflog entry → w becomes stash@{0}

    result = (|| {
        # 1. worktree: drop staged changes, keep unstaged (path-scoped → untracked untouched)
        repo.checkout_tree(merged_tree.as_object(),
            CheckoutBuilder::new().safe().update_index(false).path(each changed path))?
        # 2. index: reset to HEAD (nothing staged)
        let mut idx = repo.index()?; idx.read_tree(&head_tree)?; idx.write()?;
        Ok(())
    })();

    if result.is_err():
        # rollback: remove the entry we pushed, restore index to the staged state
        repo.stash_drop(0).ok();                            # pops our w back off the stack
        let mut idx = repo.index()?; idx.read_tree(&index_tree)?; idx.write()?;
        return result_err

    return CreateStashResult { created: true }
```

`push_stash_ref` = ensure the `refs/stash` reflog exists, then update the ref to `w` with force and
`w_msg` so a reflog entry is appended (this is what makes it stash@{0} and shifts existing entries).
Preferred: `repo.reference("refs/stash", w, true, w_msg)?` after `repo.reflog("refs/stash")` (which
creates the log if absent). If a smoke test shows `list_stashes` does not see the entry, fall back to
explicit `reflog.append(w, sig, Some(w_msg))?; reflog.write()?` plus a forced ref set. See F-2.

### B.3 Preconditions / postconditions

- Precondition: `require_clean` (reject mid-merge/rebase) and `resolve_signature` run in the outer
  `create_stash` before dispatch — unchanged, applies to `Staged` too. Unborn HEAD →
  `head()?.peel_to_commit()?` errors as `AppError::Git` (staged stash needs a base commit).
- Postcondition (success): index == HEAD tree (nothing staged); worktree contains the previously
  **unstaged** edits and none of the staged-only changes; untracked and ignored files untouched;
  one new `stash@{0}` whose `base_oid` == HEAD, applyable via the existing `apply_stash`/`pop_stash`.
- Postcondition (nothing staged): `created:false`, no entry, no mutation.
- Postcondition (overlap conflict / any mutation-step failure): error returned, stash stack + index
  + worktree left exactly as they were on entry.

### B.4 Data-loss risks and mitigations

1. **Losing unstaged edits when clearing staged changes.** Mitigated: the worktree is set to
   `merge_trees` output (HEAD + unstaged), never blindly reset to HEAD or to `index_tree`.
2. **Overlapping staged+unstaged edits on one path (mixed file).** Detected by
   `merged.has_conflicts()` and rejected **before** any mutation — no lossy guess. (Alternative
   considered: fold the whole file's worktree content into the stash — lossless but violates
   "only staged portion stashed"; rejected. Flagged F-3.)
3. **Untracked/ignored files swept away.** Mitigated: they are never added to any tree, and the
   `checkout_tree` is **path-scoped** to the tracked `changed` set with SAFE checkout (won't
   overwrite untracked collisions).
4. **Crash between ref-push and worktree/index reset.** Mutation window is two git2 calls on
   pre-computed objects; on error we `stash_drop(0)` + restore the index, so a caught failure is
   fully reverted. An uncatchable crash leaves a valid stash entry plus the original dirty tree —
   recoverable (worst case: the staged changes exist in both the stash and the worktree; no loss).
5. **`require_clean` bypass.** Kept — mid-merge/rebase is rejected up front.

---

## C. UI affordance (`WorkspaceRightPanel.tsx` + new `StashSplitButton.tsx`)

New presentational component `src/components/StashSplitButton.tsx` (single responsibility, keeps the
right-panel file small):

```ts
export interface StashSplitButtonProps {
  disabled: boolean;               // mutating || no changes at all
  stagedCount: number;             // enables the 'staged' option
  hasTrackedChanges: boolean;      // staged||unstaged → enables 'all'
  hasUntracked: boolean;           // → enables 'allWithUntracked' (also true if tracked changes)
  onStash(scope: StashScope): void;
}
```

Renders a split/menu button: primary action `Stash all` → `onStash('all')`; a caret opens a small
menu with `Stash all` (`all`), `Stash all + untracked` (`allWithUntracked`), `Stash staged only`
(`staged`). Disable each item when its scope has nothing to capture (mirror the mock's created:false
rule): `staged` disabled when `stagedCount===0`; `all` disabled when `!hasTrackedChanges`;
`allWithUntracked` disabled when `!hasTrackedChanges && !hasUntracked`. Reuse existing menu/toast
styling.

Wire-up in `WorkspaceRightPanel`:
- Add prop `onCreateStash(scope: StashScope): void` to `WorkspaceRightPanelProps`.
- Render `<StashSplitButton>` inside the status-state fragment (the `<>` at
  `WorkspaceRightPanel.tsx:191`), gated exactly like the amend affordance
  (`opState.kind === 'none' && head !== null && !head.unborn`), placed just above the amend block
  (`:219`). Derive counts from the already-threaded `status` prop
  (`status?.staged/unstaged/untracked`). `disabled = mutating || no changes`.

`RepoWorkspace.tsx`:
- Generalize `handleCreateStash()` (`:1646`) → `handleCreateStash(scope: StashScope)`; replace the
  hardcoded call with `await ipc.createStash(repoId, null, scope)`. Toast/refresh pattern unchanged.
  Success toast copy per scope (e.g. `Stashed staged changes` / `Changes stashed`), keep the
  `res.created ? … : 'Nothing to stash …'` info branch.
- Pass `onCreateStash={(scope) => void handleCreateStash(scope)}` to `WorkspaceRightPanel`.
- Sidebar wiring (`:2595`) keeps calling `handleCreateStash('allWithUntracked')` so the existing
  sidebar `⊟` button preserves today's behavior. Update `Sidebar` prop `onCreateStash(): void` →
  the sidebar button calls the parent-provided no-arg handler, and `RepoWorkspace` binds it to
  `() => handleCreateStash('allWithUntracked')`. (No signature change needed inside `Sidebar.tsx`
  beyond the callsite comment — it already takes a `onCreateStash(): void`.)

---

## D. Acceptance criteria

Rust (core, scratch-repo fixtures in `stash.rs` tests, mirroring the P9 `s9_*` pattern):

- **AC-1** `staged`: file with ONLY staged changes → `created:true`; worktree clean for that file
  (reverts to HEAD content); index == HEAD (nothing staged); one `stash@{0}`.
- **AC-2** `staged`: file with staged **and** non-overlapping unstaged changes → only the staged
  portion is stashed; the unstaged portion remains in the worktree; index == HEAD.
- **AC-3** `staged`: an untracked file present → untouched on disk, absent from the stash (pop does
  not create/alter it).
- **AC-4** `staged` round-trip: stash `staged` then `pop_stash(0)` → the staged changes are restored
  to the worktree (as unstaged edits — no index reinstate); clean pop drops the entry.
- **AC-5** `staged`: nothing staged → `created:false`, no entry, index/worktree unchanged.
- **AC-6** `staged`: overlapping staged+unstaged edits on one path → `Err(AppError::Git)`, no entry,
  index/worktree unchanged (rollback proven).
- **AC-7** `staged` respects `require_clean`: mid-merge → `OperationInProgress`.
- **AC-8** `all` == today's `include_untracked:false` behavior; `allWithUntracked` == today's
  `include_untracked:true` behavior (existing `s9_1`/`s9_4` re-expressed against the enum).
- **AC-9** Wire test: `serde_json::from_value::<StashScope>` maps `"all"|"allWithUntracked"|"staged"`
  to the three variants.

Frontend (harness + tsc):

- **AC-10** `pnpm dev` + `VITE_MOCK_IPC=1`: the staging panel shows the stash split button; each of
  the three scopes updates the mock status arrays per §A.5; sidebar `⊟` still stashes
  (`allWithUntracked`).
- **AC-11** `tsc` clean; mock and tauri IPC both compile against the new `StashScope` signature.

---

## E. Notes on mock fidelity

The mock cannot split hunks within a file (`staged` just clears the `staged` array and keeps
`unstaged`/`untracked`). This is sufficient for visual/harness verification; the real hunk-level
correctness is covered by the Rust ACs. No mock change is needed to prove the split behavior.

---

## F. Flagged ambiguities (orchestrator decisions)

- **F-1 (recommend accept).** Popping a `staged` stash restores its changes as **unstaged** worktree
  edits, not re-staged into the index — because the shipped `apply_stash`/`pop_stash` use the SAFE
  default (no `REINSTATE_INDEX`, P9 OPEN Q#4). This matches `git stash pop` without `--index`.
  Re-staging on pop would require a new REINSTATE path and is out of scope. AC-4 is written to this
  semantic.
- **F-2 (verify in impl).** The exact git2 call that appends a `refs/stash` reflog entry so
  `list_stashes` sees the hand-rolled entry needs a smoke check: primary
  `repo.reference("refs/stash", w, true, msg)` after ensuring the reflog; fallback explicit
  `Reflog::append` + forced ref set. Flagging because git2-rs reflog-on-ref-update behavior depends
  on `core.logAllRefUpdates`/special-casing of `refs/stash`.
- **F-3 (recommend the reject-on-overlap design in §B).** For a path with overlapping staged and
  unstaged edits, the contract rejects with a clear error rather than silently folding the whole
  file into the stash. If product prefers "stash the whole file when it overlaps" (lossless, but
  captures more than the staged hunks), that is a one-line change in the `has_conflicts()` branch —
  confirm the preferred UX.
- **F-4.** Detached-HEAD branch label in the stash message: pseudocode uses `"(no branch)"`.
  Confirm acceptable (git uses the short oid form `WIP on (no branch): …`).
