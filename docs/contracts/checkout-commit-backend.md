# Checkout commit → detached HEAD — backend + IPC contract

Scope: add a dirty-safe "checkout an arbitrary commit → detached HEAD" command, its IPC surface
(tauri + mock), and the frontend handler. Consumed by `docs/contracts/checkout-commit-ui.md` (§9).
Reuses the existing `CheckoutResult` shape and the P33 autostash pattern verbatim — no new type.
Branch checkout keeps using `checkoutBranch`; only the detached path uses this new command.

## 1. Rust core — `crates/bonsai-core/src/git/branches/checkout.rs`

New sibling to `checkout_branch_autostash`. Detaching is the intended outcome, not an error.

```rust
/// Blocking. Checks out arbitrary commit `oid` as a DETACHED HEAD, dirty-safe:
/// auto-stash -> safe checkout_tree -> set_head_detached -> re-apply stash.
/// NO auto-FF (detached HEAD tracks nothing). A conflicted re-apply is a SUCCESS
/// carrying apply: Some(Conflicts{..}) (stash retained, never lossy). SAFE
/// checkout only — never force; a conflict before set_head leaves worktree+HEAD
/// untouched.
///
/// Errors: `invalidName` (oid not a 40-hex/parseable oid) | `git` (oid not found
/// or not a commit) | `operationInProgress` (via create_stash) | `configMissing`
/// (via create_stash) | `checkoutConflict` (defensive, post-stash) | `noRepo`.
pub fn checkout_commit_detached(workdir: &Path, oid: &str) -> Result<CheckoutResult, AppError>;
```

Algorithm (mirror `checkout_branch_autostash`, drop branch/worktree/FF steps):
1. `open_repo_at`; parse `oid` via `git2::Oid::from_str` → `invalidName` on parse error.
2. `repo.find_commit(oid)` → `git` error if missing / not a commit (peel is not required; the UI
   only ever passes commit oids).
3. **No-op guard:** if HEAD is already detached AND `repo.head().target() == oid`, return
   `CheckoutResult { stashed:false, fast_forwarded:false, apply:None }` with no side effects.
   (UI omits the item in this case; this guards the race.)
4. `stash::create_stash(.., AllWithUntracked)?.created` → `stashed` (owns clean/dirty +
   mid-merge `operationInProgress` guard).
5. SAFE `checkout_tree(&commit_obj, CheckoutBuilder::new().safe())`. On any error: if `stashed`,
   best-effort `pop_stash(0)`, then return the error (map `Conflict` → `checkoutConflict`).
6. `repo.set_head_detached(oid)` — this is what produces detached HEAD (vs `set_head` for branch).
7. `fast_forwarded` is ALWAYS `false` (a detached HEAD has no upstream — do not attempt FF).
8. If `stashed`: `pop_stash(0)` → `apply: Some(outcome)`; else `apply: None`.

`CheckoutResult` is unchanged (`{ stashed, fast_forwarded, apply }`); `fast_forwarded` is always
false here — the UI ignores it for detached (§7 toasts don't mention FF).

## 2. Tauri command — `src-tauri/src/commands/branches.rs`

Mirror `checkout_branch` + `checkout_branch_inner` exactly (spawn_blocking, `_inner` split for
unit tests). Register in `src-tauri/src/lib.rs` invoke_handler beside `checkout_branch` (~line 129).

```rust
#[tauri::command]
pub async fn checkout_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
) -> Result<CheckoutResult, AppError> {
    checkout_commit_inner(state.inner(), &repo_id, oid).await
}

pub(crate) async fn checkout_commit_inner(
    state: &AppState,
    repo_id: &str,
    oid: String,
) -> Result<CheckoutResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || branches::checkout_commit_detached(&path, &oid))
        .await
        .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Does NOT emit `repo-changed` — the frontend calls `refreshAll` (matches `checkout_branch`).
Command name over the wire: `checkout_commit`; param key: `oid` (camelCase-safe, single word).

## 3. Frontend IPC surface

`CheckoutResult` already exists in `src/ipc/types/branches.ts` — reuse, no new type.

**`src/ipc/types` (the `IpcApi` interface, where `checkoutBranch` is declared):**
```ts
checkoutCommit(repoId: string, oid: string): Promise<CheckoutResult>;
```

**`src/ipc/tauri/branches.ts`** (beside `checkoutBranch`):
```ts
checkoutCommit(repoId: string, oid: string): Promise<CheckoutResult> {
  return invoke<CheckoutResult>('checkout_commit', { repoId, oid });
},
```

**`src/ipc/mock/handlers/branches.ts`** (beside `checkoutBranch`) — stateful, so the graph HEAD
pill moves to detached on the next `refreshAll`:
```ts
async checkoutCommit(repoId: string, oid: string): Promise<CheckoutResult> {
  await delay(150);
  const state = requireRepo(repoId);
  // no-op: already detached at this oid → item is omitted by UI; guard the race.
  if (state.kind === 'detached' && state.headOid === oid) {
    return { stashed: false, fastForwarded: false, apply: null };
  }
  const dirty = /* staged|unstaged|untracked|conflicted non-empty, as checkoutBranch */;
  // Transition to detached HEAD:
  for (const b of state.branches.local) b.isHead = false;
  state.kind = 'detached';
  state.headOid = oid;
  // headBranch left as-is/ignored; listBranches recomputes head from state.kind.
  state.branches.head = { branchName: null, oid, detached: true, unborn: false };
  if (!dirty) return { stashed: false, fastForwarded: false, apply: null };
  // conflict seam: query('checkout') === 'detachconflict' → retained stash + conflict
  if (query('checkout') === 'detachconflict') {
    upsert(s.conflicted, { path: 'src/app.ts', origPath: null, status: 'conflicted' });
    return { stashed: true, fastForwarded: false, apply: { kind: 'conflicts', paths: ['src/app.ts'] } };
  }
  return { stashed: true, fastForwarded: false, apply: { kind: 'applied' } };
}
```
Note: `state.kind` must be settable to `'detached'` (RepoState already models a `'detached'`
kind — see `listBranches`). If the mock `RepoState` union field is not mutable in place, set the
discriminant + `headOid`; `listBranches` already derives `head` + clears `isHead` for detached.

## 4. Frontend handler — `src/components/repoWorkspace/useBranchActions.ts`

Add beside `handleCheckoutBranch` / `handleCheckoutRemote`; `refreshAll()` full (HEAD moves).
Toast wording is fixed by UI contract §7 (uses `<shortOid>` = `oid.slice(0,7)`).

```ts
// Checkout arbitrary commit → detached HEAD. Non-destructive (no confirm),
// dirty-safe (auto-stash/re-apply). HEAD moves → refreshAll full.
async function handleCheckoutCommit(oid: string) {
  const short = oid.slice(0, 7);
  setMutating(true);
  try {
    const res = await ipc.checkoutCommit(repoId, oid);
    await refreshAll();
    if (res.apply?.kind === 'conflicts') {
      pushToast('warning',
        `Detached HEAD at ${short}; your changes were carried over with conflicts and kept safe at stash@{0} — resolve them in the status panel`);
    } else if (res.stashed) {
      pushToast('success', `Detached HEAD at ${short} (stashed & re-applied)`);
    } else {
      pushToast('success', `Detached HEAD at ${short}. Commit or create a branch to keep new work.`);
    }
  } catch (e) {
    pushToast('error', errorMessage(e));
  } finally {
    setMutating(false);
  }
}
```
- Return/expose `handleCheckoutCommit` from the hook and thread it into the `workspaceMenus` deps
  bundle alongside `handleCheckoutBranch` / `handleCheckoutRemote`.
- Add `handleCheckoutCommit: vi.fn()` to `src/test/workspaceMenusFixtures.ts` (UI contract §9).

## 5. Edge cases
- **Unborn HEAD:** no reachable commits, so the UI never offers checkout (UI §1 returns `[]`).
  Backend not called; no special handling required (a bad oid would return `git`).
- **oid == current detached HEAD:** backend + mock return the clean no-op result; UI omits the
  item anyway (UI §1: `head.detached && head.oid === oid` → `[]`).
- **oid == current attached-branch tip:** valid, non-no-op — detaches from the branch onto the
  same commit (real state change; UI keeps the item, §2b case e). Backend proceeds normally.
- **Dirty tree conflict on re-apply:** SUCCESS with `apply.kind==='conflicts'`, stash retained at
  `stash@{0}` — warning toast, not an error.
- **checkoutConflict:** only defensive (post-stash the tree is clean); if it occurs, error toast
  via `errorMessage(e)`, HEAD/worktree unchanged.
- **Invalid / missing oid:** `invalidName` / `git` → error toast. Never surface raw libgit2 text.

## 6. Acceptance criteria
- `checkout_commit` registered; `cargo check` + `tsc` clean; mock keeps `satisfies IpcApi`.
- Core unit tests (in `checkout_autostash_tests.rs` or a sibling): clean detach on a non-tip
  commit → HEAD detached at oid, worktree matches; dirty detach → stash created + re-applied;
  conflicted re-apply → `apply.kind==Conflicts`, stash retained; no-op when already detached at
  oid; bad oid → error, HEAD unchanged.
- Harness (`VITE_MOCK_IPC=1`): triggering `Checkout commit (detached)` from a commit row detaches
  and the red `HEAD` detached pill renders on the target row; `?checkout=detachconflict` seam
  produces the warning toast with a retained conflict entry.
- USER CHECKPOINT: real detached-HEAD checkout + re-attach in the native window.

## 7. Flagged to orchestrator
- Mock `RepoState`: confirm `state.kind` can be flipped to `'detached'` in place; if the union is
  constructed immutably elsewhere, the mock handler may need a small `repoState.ts` helper
  (`setDetached(state, oid)`). Recommendation: add the helper next to `requireRepo`.
- No new `AppError` variant needed — `invalidName`/`git`/`checkoutConflict` cover all cases.
