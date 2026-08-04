# P33 — Auto-stash branch switch with auto fast-forward

Status: contract ready for senior-dev.

## Goal

Replace the hard failure on `checkout_branch` when the worktree is dirty
("cannot switch to '<name>': local changes would be overwritten") with a
GitKraken-style safe switch that carries work across and catches the branch up
to its upstream:

1. stash all changes (include untracked),
2. safe-checkout the target branch,
3. auto fast-forward the just-switched branch to its upstream tracking ref
   **without fetching** (local ref math only), when behind and not diverged,
4. re-apply the stash; drop on clean apply, **retain** on conflict (never lossy).

This mirrors `create_branch_here` (branches.rs:242) minus the branch creation,
plus the auto-FF step. `checkout_branch` (branches.rs:325) is REUSED unchanged.

---

## 1. Core: `checkout_branch_autostash`

File: `crates/bonsai-core/src/git/branches.rs` (append after `checkout_branch`).

### 1.1 New result struct (lives in `branches.rs`, next to `CreateBranchHereResult` at line 222)

```rust
/// Result of `checkout_branch_autostash`. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckoutResult {
    /// true when uncommitted work was auto-stashed and carried across.
    pub stashed: bool,
    /// true when the switched-to branch was fast-forwarded to its upstream
    /// (behind>0 && ahead==0). false when no upstream, up-to-date, ahead, or
    /// diverged.
    pub fast_forwarded: bool,
    /// Present only when `stashed`; the outcome of re-applying the stash on the
    /// (possibly fast-forwarded) target branch. `Applied` = clean carry-over
    /// (stash dropped); `Conflicts{paths}` = carried with markers, stash
    /// RETAINED at stash@{0}. `None` when the worktree was clean.
    pub apply: Option<stash::ApplyStashOutcome>,
}
```

### 1.2 Signature

```rust
pub fn checkout_branch_autostash(
    workdir: &Path,
    name: &str,
) -> Result<CheckoutResult, AppError>
```

Errors (mirror `create_branch_here` / `checkout_branch`):
`branchNotFound` | `operationInProgress` (via `create_stash`) |
`configMissing` (via `create_stash`, authors the stash commit) |
`checkoutConflict` (defensive — worktree is clean post-stash) | `git` | `noRepo`.

### 1.3 Ordered algorithm

```
checkout_branch_autostash(workdir, name):
    // 0. Resolve up-front, zero side effects on failure.
    repo = open_repo_at(workdir)
    find LOCAL branch `name`
        NotFound -> return Err(BranchNotFound)                // matches checkout_branch
    if branch.is_head(): return Ok(CheckoutResult{false,false,None})   // no-op, guard the race

    // 1. Auto-stash (owns the dirty-vs-clean + op-state decision).
    stashed = stash::create_stash(workdir, None, /*include_untracked*/ true)?.created

    // 2. SAFE checkout. On ANY failure, restore stash (best-effort) then return.
    //    Post-stash the worktree is clean, so a real conflict here is defensive.
    if let Err(e) = checkout_branch(workdir, name):
        if stashed: let _ = stash::pop_stash(workdir, 0)
        return Err(e)

    // 3. AUTO FAST-FORWARD (no fetch). Skip silently on any non-FF condition.
    fast_forwarded = try_ff_to_upstream(&repo, name)?   // see 1.4; returns bool

    // 4. Re-apply carried work iff stashed. Conflicts is a SUCCESS return.
    if stashed:
        outcome = stash::pop_stash(workdir, 0)?          // drops on clean, retains on conflict
        return Ok(CheckoutResult{ stashed:true, fast_forwarded, apply:Some(outcome) })

    // 5. Clean case.
    return Ok(CheckoutResult{ stashed:false, fast_forwarded, apply:None })
```

**Ordering rationale (must hold):** the FF (step 3) runs **after** the checkout
(step 2) and **before** the stash re-apply (step 4). That way the stash lands on
the fast-forwarded tip, exactly matching what the user would get from
`switch` + `pull --ff-only` with their WIP restored on top. Doing FF before the
switch would move the wrong branch; doing it after re-apply could conflict the
FF checkout against the just-restored dirty worktree.

### 1.4 The no-fetch fast-forward helper

Resolve the upstream oid from the **already-present remote-tracking ref** — no
network. Use the same primitives `list_refs` uses to compute ahead/behind
(branches.rs:103-117): `Branch::upstream()` resolves the tracking ref locally,
`Reference::target()` gives its oid, `repo.graph_ahead_behind(local, upstream)`
gives `(ahead, behind)`.

```
try_ff_to_upstream(repo, name) -> Result<bool, AppError>:
    branch = repo.find_branch(name, Local)?
    upstream = match branch.upstream():
        Ok(u) => u
        Err(NotFound) => return Ok(false)          // no upstream -> skip silently
        Err(e) => return Ok(false)                 // best-effort: never fail the switch on this
    upstream_oid = upstream.get().target()  ?? return Ok(false)
    local_oid    = branch.get().target()    ?? return Ok(false)

    (ahead, behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?
    if behind == 0 { return Ok(false) }            // up-to-date or ahead-only
    if ahead  > 0  { return Ok(false) }            // diverged -> do NOT touch (no merge in v1)

    // Fast-forward (behind>0 && ahead==0). SAFE-FF recipe: checkout_tree BEFORE
    // set_target, identical to remote.rs pull_ff:356-371 and merge.rs:141-172.
    {
        let obj = repo.find_object(upstream_oid, None)?;   // scoped so its borrow ends
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.safe();                                       // NEVER .force()
        match repo.checkout_tree(&obj, Some(&mut opts)):
            Ok(()) => {}
            Err(Conflict) => return Ok(false)   // defensive: worktree is clean post-stash; skip FF
            Err(e) => return Err(e.into())
    }
    repo.find_reference(&format!("refs/heads/{name}"))?
        .set_target(upstream_oid, &format!("checkout: fast-forward {name} to {upstream_oid}"))?;
    Ok(true)
```

Notes for the implementer:
- `Branch::upstream()` reads the configured `branch.<name>.remote/merge` and the
  local remote-tracking ref only; it performs **no** network I/O. This is the
  no-fetch requirement.
- Keep the `git2::Object` borrow of `repo` in an inner scope so it is dropped
  before `set_target` (see merge.rs:144 comment) — otherwise a borrow-check
  failure.
- FF failure modes degrade to `Ok(false)` (switch still succeeded); only a
  genuine libgit2 error on `checkout_tree`/`set_target` propagates. Rationale:
  the branch is already switched; a missing/odd upstream must not turn a
  successful switch into an error.

### 1.5 Upstream shorthand for the toast

The frontend toast wants "(fast-forwarded to origin/main)". `CheckoutResult`
intentionally does **not** carry the upstream name — the frontend already has it
from `BranchInfo.upstream` (types.ts:23, populated by `list_refs`) for the
switched-to branch after `refreshAll`. Recommendation: the toast reads the
upstream string from the refreshed branches snapshot, not from the result.
**FLAG (minor):** if the orchestrator prefers the backend to be authoritative,
add `upstream: Option<String>` to `CheckoutResult` and set it from
`upstream.name()` when `fast_forwarded`. Default (recommended): keep the struct
as three fields; derive the label frontend-side.

---

## 2. Tauri command

File: `src-tauri/src/commands.rs` — add after `checkout_branch` (line 971-990),
mirroring that wrapper and the `create_branch_here` result-returning pattern
(line 946-967).

```rust
/// Dirty-safe checkout of a LOCAL branch: auto-stash -> switch -> auto FF to
/// upstream (no fetch) -> re-apply stash. Conflicted re-apply is a SUCCESS
/// carrying `apply: Some(conflicts)` (stash retained). Errors: `branchNotFound`
/// | `operationInProgress` | `configMissing` | `checkoutConflict` | `git` |
/// `noRepo`. Does NOT emit `repo-changed` (frontend calls refreshAll).
#[tauri::command]
pub async fn checkout_branch_autostash(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<CheckoutResult, AppError> {
    checkout_branch_autostash_inner(state.inner(), &repo_id, name).await
}

async fn checkout_branch_autostash_inner(
    state: &AppState,
    repo_id: &str,
    name: String,
) -> Result<CheckoutResult, AppError> {
    let path = repo_path(state, repo_id)?;
    tauri::async_runtime::spawn_blocking(move || {
        branches::checkout_branch_autostash(&path, &name)
    })
    .await
    .map_err(|e| AppError::Other(format!("task join error: {e}")))?
}
```

Import: add `CheckoutResult` to the existing `use ...branches::{...}` group in
commands.rs (same place `CreateBranchHereResult` is imported).

## 3. Registration

File: `src-tauri/src/lib.rs`, `invoke_handler![...]` (line 62 area). Add
`commands::checkout_branch_autostash,` next to `commands::checkout_branch`.

---

## 4. Frontend surface

### 4.1 Types — `src/ipc/types.ts`

Add near `CreateBranchHereResult` (line 540). Reuse existing `ApplyStashOutcome`
(line 529) and `StashEntry` (line 360) — do NOT redeclare them.

```ts
export interface CheckoutResult {
  /** true when uncommitted work was auto-stashed and carried across. */
  stashed: boolean;
  /** true when the switched-to branch was fast-forwarded to its upstream. */
  fastForwarded: boolean;
  /** Present only when `stashed`; null otherwise (serde None -> null). */
  apply: ApplyStashOutcome | null;
}
```

Change the `IpcApi` method (currently line 1152) from returning `void` to:

```ts
checkoutBranch(repoId: string, name: string): Promise<CheckoutResult>;
```

**FLAG (naming):** the task names the new IPC method `checkoutBranchAutostash`,
but the existing UI already calls `ipc.checkoutBranch` and the switch is ALWAYS
dirty-safe now (no reason to keep a failing variant). Two options:
- **(A, recommended)** Repurpose `checkoutBranch` to call the new command and
  return `CheckoutResult`. One method, one behavior; the old error-banner path is
  fully replaced. Wire name still `checkout_branch_autostash`.
- **(B)** Add a distinct `checkoutBranchAutostash` method, leaving the old
  `checkoutBranch: Promise<void>` in place. More surface, two code paths, dead
  old path. Only pick this if some caller must keep hard-fail semantics.

The rest of this contract assumes **(A)**. If the orchestrator picks (B), add a
sibling method/command instead of repurposing and keep both mocks.

### 4.2 Tauri impl — `src/ipc/tauri.ts` (line 215)

```ts
checkoutBranch(repoId: string, name: string): Promise<CheckoutResult> {
  return invoke<CheckoutResult>('checkout_branch_autostash', { repoId, name });
},
```

### 4.3 Mock — `src/ipc/mock.ts` (replace current `checkoutBranch`, line 2171)

Mirror the `createBranchHere` mock (line 2119) for the stash/apply simulation
plus a fast-forward simulation. Must stay implementable with fixture data only.

Behavior:
- `branchNotFound` when `name` not in `state.branches.local` (unchanged).
- Remove the old hard `checkoutConflict` throw for `fix/watcher-debounce`; the
  switch now succeeds. Repurpose a designated branch name to exercise the
  conflicted re-apply path (recommendation: keep a `cbhconflict`-style trigger,
  e.g. switching to a branch named `fix/watcher-debounce` yields
  `apply: { kind:'conflicts', paths:['src/app.ts'] }` and leaves the worktree
  dirty — parity with the createBranchHere mock at line 2161-2166).
- Compute `dirty` from `state.status` exactly as createBranchHere does
  (line 2138-2143).
- Move HEAD to the branch (set `isHead`, `state.headBranch`, `state.headOid`,
  `state.branches.head`) as the current mock does (line 2189-2192).
- `fastForwarded`: simulate when the switched-to `BranchInfo` has an `upstream`
  and `behind > 0 && ahead === 0`; then set `branch.tip` to the upstream tip and
  zero its `behind`. For a simple deterministic fixture, gate on a designated
  branch name (e.g. `feature/behind-upstream`) so the harness can screenshot the
  FF toast. Return `fastForwarded: true` in that case, else `false`.
- Return shape:
  - clean, no FF: `{ stashed:false, fastForwarded:false, apply:null }`
  - clean, FF:    `{ stashed:false, fastForwarded:true, apply:null }`
  - stashed clean:`{ stashed:true, fastForwarded:<bool>, apply:{kind:'applied'} }`
  - stashed conflict: `{ stashed:true, fastForwarded:<bool>, apply:{kind:'conflicts', paths:['src/app.ts']} }`

### 4.4 Handler — `src/components/RepoWorkspace.tsx` (`handleCheckoutBranch`, line 1318)

Rewrite to consume `CheckoutResult` and drive toasts, mirroring
`handleCreateBranchHere` (line 1333-1354). This **replaces** the
`checkoutConflict` error-banner path for the switch (the banner path is gone;
dirty switches now succeed). Keep `setBranchesError(null)` + `setMutating` frame.

```
async function handleCheckoutBranch(name):
  setBranchesError(null); setMutating(true)
  try:
    res = await ipc.checkoutBranch(repoId, name)
    await refreshAll()
    upstreamLabel = branches.local.find(b => b.name===name)?.upstream   // post-refresh
    if res.apply?.kind === 'conflicts':
      pushToast('warning',
        `Switched to ${name}; your changes were carried over with conflicts and kept safe at stash@{0} — resolve them in the status panel`)
    else:
      let msg = `Switched to ${name}`
      const extras = []
      if (res.stashed) extras.push('stashed & re-applied')
      if (res.fastForwarded) extras.push(`fast-forwarded to ${upstreamLabel ?? 'upstream'}`)
      if (extras.length) msg += ` (${extras.join(', ')})`
      pushToast('success', msg)
  catch e:
    // real errors only now (branchNotFound / operationInProgress / configMissing / git)
    pushToast('error', errorMessage(e))       // or keep setBranchesError — see FLAG
  finally:
    setMutating(false)
```

**FLAG (error surface):** current `handleCheckoutBranch` shows errors via
`setBranchesError` (sidebar banner); `handleCreateBranchHere` uses
`pushToast('error', ...)`. Recommendation: switch to `pushToast` for consistency
with the new toast-driven success/warning paths. If the orchestrator wants to
preserve the sidebar banner for hard errors, keep `setBranchesError(errorMessage(e))`
in the catch — either is acceptable; pick one and be consistent.

---

## 5. Acceptance criteria

Backend (cargo test in `bonsai-core`, scratch repos built with git2 like the
stash.rs `s9_*` fixtures):

- **AC1 clean switch, no upstream:** dirty=false, no upstream →
  `{stashed:false, fast_forwarded:false, apply:None}`; HEAD moved; worktree
  matches target tree.
- **AC2 dirty switch, clean carry-over:** tracked edit, target has no
  relevant upstream lag → `{stashed:true, fast_forwarded:false, apply:Some(Applied)}`;
  edit present on target branch; stash stack empty.
- **AC3 dirty switch, conflicted carry-over:** target tip conflicts with the
  stashed edit → `apply:Some(Conflicts{paths})`, worktree has `<<<<<<<`
  markers, **stash retained** (`list_stashes().len()==1`), repo state Clean
  (not Merge). Return is `Ok`, not `Err`.
- **AC4 auto-FF when behind & not diverged:** target branch behind its
  remote-tracking ref by N (ahead 0), clean worktree → `fast_forwarded:true`;
  local ref now equals the upstream oid; no network performed (fixture has no
  remote reachable — upstream oid comes from the tracking ref only).
- **AC5 no FF when diverged:** target ahead>0 && behind>0 → `fast_forwarded:false`;
  local ref unchanged.
- **AC6 no FF when up-to-date or ahead-only:** behind==0 → `fast_forwarded:false`,
  ref unchanged.
- **AC7 no FF when no upstream:** `fast_forwarded:false`, no error.
- **AC8 already checked out:** `is_head` → `{false,false,None}`, no side effects.
- **AC9 branch not found:** `Err(BranchNotFound)`.
- **AC10 op in progress (mid-merge):** dirty tree, mid-merge → `create_stash`
  rejects with `OperationInProgress`; nothing switched.
- **AC11 ordering:** in an AC2+AC4 combined fixture (dirty AND behind), the
  restored edit sits on the fast-forwarded tip (assert both the FF oid AND the
  edit content present).
- **AC12 wire shape:** `serde_json::to_value(CheckoutResult{...})` →
  `{"stashed":_, "fastForwarded":_, "apply":_}` (camelCase; `apply` null when
  None, tagged `{"kind":...}` otherwise). Matches the TS `CheckoutResult`.

Frontend (browser harness, `VITE_MOCK_IPC=1`):

- **AC13** switching a clean branch → success toast "Switched to <name>".
- **AC14** switching the FF fixture branch → success toast includes
  "(fast-forwarded to origin/…)".
- **AC15** switching the conflict fixture branch → warning toast mentioning
  "kept safe at stash@{0}"; status panel shows the conflicted file.
- **AC16** `tsc` clean; mock and tauri `checkoutBranch` return `CheckoutResult`;
  no remaining `void` callers break.

---

## 6. Invariants honored

- Rust owns all git logic + the FF math; React only renders toasts off the
  returned struct.
- IPC carries one compact precomputed result; no per-commit round-trips.
- Command = request/response; no new events/channels needed (frontend already
  calls `refreshAll`).
- `git2` blocking call wrapped in `spawn_blocking`.
- `checkout_branch` (branches.rs:325) is REUSED verbatim — not modified — so no
  double-stash.
- Mock IPC fully implements the surface with fixture data.

## 7. Flagged ambiguities (for orchestrator)

- **F1 IPC method name:** repurpose `checkoutBranch` (recommended, §4.1 option A)
  vs. add `checkoutBranchAutostash` (option B). Task text says the latter; the
  recommendation is A because the switch is now always dirty-safe and B leaves a
  dead hard-fail path. Orchestrator to confirm.
- **F2 upstream label source:** derive from refreshed `BranchInfo.upstream`
  (recommended) vs. add `upstream: Option<String>` to `CheckoutResult` (§1.5).
- **F3 error surface in handler:** `pushToast('error')` (recommended) vs. keep
  `setBranchesError` sidebar banner (§4.4).
