# P8 — Merge with autostash

Increment: extend the existing merge (P3c) so that a merge triggered from the branch
context menu **stashes dirty local changes, merges (fast-forwarding when possible), then
re-applies the stash** — i.e. `git merge --autostash` semantics.

Scope: one focused `senior-dev` pass (Rust in `merge.rs`, wire type in `types.ts`, mock in
`mock.ts`, toast mapping in `RepoWorkspace.tsx`) plus a `tester` pass. **No command-surface
change** — same `merge_branch(repoId, name)` command; only the `MergeOutcome` wire shape grows.

---

## OPEN QUESTIONS FOR USER (recommendations given; contract proceeds with the safe defaults)

1. **Restore staged vs unstaged split?** `git merge --autostash` re-applies with
   `stash apply --index` (best-effort restore of the staged/unstaged split, falling back to
   worktree-only). git2's `StashApplyOptions` can do this via `StashApplyFlags::REINSTATE_INDEX`.
   **Recommendation for v1: do NOT reinstate the index** (plain apply). Previously-staged changes
   come back as *unstaged* worktree changes. Simpler, fewer conflict cases, nothing is lost, and
   the user re-stages trivially. Easy to flip later. → *This contract assumes no-reinstate.*
2. **Deferred re-apply on a paused (conflicted) merge.** Real git records the autostash and
   re-applies it automatically when you later `git merge --continue` / `--abort`. Wiring the
   stash into our separate `commit_merge` / `abort_merge` commands is more machinery than this
   increment warrants. **Recommendation for v1: leave the autostash on the stash stack and tell
   the user** (see §3 matrix row 5). The stash is never dropped; the user re-applies it after
   finishing the merge. Flagged so the orchestrator/user can confirm this UX is acceptable for v1.

Both defaults are SAFE (never lose data). Neither blocks implementation.

---

## 1. Overview & invariants held

- Rust owns all Git logic; git2 only; no network (merging `origin/x` uses the already-fetched
  remote-tracking ref, unchanged from P3c).
- Blocking git2 runs under the existing `spawn_blocking` in `merge_branch_inner`
  (`commands.rs:861`) — unchanged.
- `merge_branch` stays runtime-free / Tauri-free → unit-testable directly (the "test" Tauri
  feature is avoided on this machine).
- **Safety:** stashing is fully recoverable, so autostash does **not** require the destructive-op
  UI confirmation that Abort has. A failed autostash leaves the repo in a well-defined, non-lossy
  state, mirroring P3c's existing guarantee that *"a failed `merge_branch` leaves state Clean"*
  (`merge.rs:146-164`). The autostash is **never silently dropped**.

---

## 2. Rust: signature, flow, and exact git2 calls

Signature is **unchanged**:

```rust
pub fn merge_branch(workdir: &Path, branch_name: &str) -> Result<MergeOutcome, AppError>
```

### 2.1 What changes in `merge.rs`

- **Remove** the hard dirty-INDEX refusal at `merge.rs:88-97` (the
  `"commit or unstage them first"` error). It is replaced by the autostash logic below.
- **Reorder** so `merge_analysis` runs *before* deciding to stash (a no-op `UpToDate` must never
  create a stash). `resolve_signature` (`merge.rs:101`, defined `commit.rs:39`) stays early — it
  is required both as the merge auto-commit identity **and** as the stash *stasher* signature.

### 2.2 Definition of "dirty" (when to autostash)

Dirty = any **tracked** change, staged or unstaged. **Untracked files are excluded** (see §2.4).

```rust
// After analysis, only on the FF / normal-merge paths:
let mut so = git2::StatusOptions::new();
so.include_untracked(false).include_ignored(false);
let dirty = !repo.statuses(Some(&mut so))?.is_empty();
```

This subsumes the old `index.write_tree_to() != head_tree` check (staged) **and** additionally
catches pure-unstaged edits (which P3c allowed through, only to fail later as `CheckoutConflict`).
`index.has_conflicts()` cannot occur here — state is `Clean`.

### 2.3 Ordered flow

1. `open_workdir_repo(workdir)` → `let mut repo` (mutable; `stash_save2`/`stash_pop` need `&mut`).
2. Precondition: `repo.state() == Clean` else `OperationInProgress` (unchanged, `merge.rs:54`).
3. `read_head_info` → reject unborn / detached; obtain `head_branch` (unchanged, `merge.rs:60-71`).
4. Resolve `incoming` (local then remote-tracking) → `(incoming, incoming_is_remote)`
   (unchanged, `merge.rs:75-86`).
5. `let sig = resolve_signature(&repo.config()?.snapshot()?)?;` (unchanged position;
   now also the stasher).
6. `annotated = repo.reference_to_annotated_commit(incoming.get())?;`
   `let (analysis, _) = repo.merge_analysis(&[&annotated])?;`
7. `if analysis.is_up_to_date() { return Ok(MergeOutcome::UpToDate); }` — **no stash created**.
8. `let dirty = /* §2.2 */;`
   `let stashed = if dirty { stash_save(&mut repo, &sig)?; true } else { false };`
9. Branch on analysis:
   - **Fast-forward** (`analysis.is_fast_forward()`): §2.5.
   - **Normal** (else): §2.6.

### 2.4 `stash_save` helper — exact call & flags

```rust
fn stash_save(repo: &mut git2::Repository, sig: &git2::Signature) -> Result<(), AppError> {
    repo.stash_save2(sig, Some("bonsai: autostash before merge"),
                     Some(git2::StashFlags::DEFAULT))?;
    Ok(())
}
```

- `StashFlags::DEFAULT` = stash **index + worktree** tracked changes and reset both to HEAD →
  the tree is clean, so the subsequent SAFE checkout/merge can never conflict with the user's own
  tracked edits (the whole point).
- **NOT `KEEP_INDEX`**: keeping staged changes in the index would leave the tree dirty and defeat
  the merge.
- **NOT `INCLUDE_UNTRACKED`**: matches git's default autostash — untracked files are left in
  place, and stashing them would risk removing files the merge never touches. Consequence: an
  untracked file physically in the way of the checkout still fails (recovered per §2.7,
  git-consistent).

### 2.5 Fast-forward path (dirty-aware)

Same SAFE-checkout-before-`set_target` recipe as today (`merge.rs:110-132`) and `pull_ff`
(`remote.rs:344-366`).

```
obj = repo.find_object(annotated.id(), None)?
checkout SAFE (never .force()):
  match repo.checkout_tree(&obj, safe):
    Ok  => proceed
    Err(Conflict) =>                         // e.g. an untracked file in the way
        if stashed { rollback_stash(&mut repo)?; }   // §2.7 — restore original state
        return Err(CheckoutConflict("cannot merge: local changes would be overwritten..."))
    Err(e) => { if stashed { rollback_stash()?; } return Err(e.into()) }
repo.find_reference("refs/heads/{head_branch}")?.set_target(annotated.id(), "merge {branch}: fast-forward")?
// ref now moved. Re-apply the stash on top:
if stashed { return pop_after_success(&mut repo, workdir, MergeKind::Ff{branch,to}); }
return Ok(FastForwarded { branch: head_branch, to: annotated.id().to_string(), stashed: false })
```

### 2.6 Normal-merge path (dirty-aware)

Same `repo.merge` + conflict/auto-commit logic as today (`merge.rs:135-195`), with these deltas:

```
repo.merge(&[&annotated], merge_opts, checkout(safe, allow_conflicts, conflict_style_merge)):
    Err(e) =>                                // libgit2 may have written MERGE_* then failed checkout
        repo.cleanup_state(); reset index to HEAD (unchanged best-effort block merge.rs:147-155)
        if stashed { rollback_stash(&mut repo)?; }        // §2.7
        return CheckoutConflict / e.into()   (unchanged mapping)

index = repo.index()?
if index.has_conflicts():
    write deterministic MERGE_MSG (unchanged merge.rs:167-182)
    // PAUSE. Do NOT pop — reapplying into a conflicted worktree is unsafe (OPEN Q #2).
    // The stash (if any) is RETAINED on the stack.
    return Ok(Conflicts { paths, stashed })

// clean merge → auto-commit (unchanged merge.rs:186-195 finalize_merge_commit)
oid = finalize_merge_commit(&mut repo, &message)?.oid
if stashed { return pop_after_success(&mut repo, workdir, MergeKind::Merged{oid}); }
return Ok(Merged { oid, stashed: false })
```

### 2.7 `rollback_stash` (failure AFTER stash, BEFORE terminal outcome)

Guarantee: never lose the stash. On a mutation failure after `stash_save`, try to restore the
user's original dirty state so it is as if nothing happened (FF `set_target` has not run yet, so
the ref is untouched; the normal-merge error path already `cleanup_state()`'d + reset the index).

```rust
fn rollback_stash(repo: &mut git2::Repository) -> Result<(), AppError> {
    // Tree is clean here (we just stashed), so this pop should apply cleanly.
    match repo.stash_pop(0, Some(&mut git2::StashApplyOptions::new())) {
        Ok(()) => Ok(()),
        Err(_) => Ok(()),   // could not auto-restore: LEAVE the stash on the stack (never drop);
                            // the surfaced error message tells the user it is at stash@{0}.
    }
}
```

The caller returns the original `CheckoutConflict` / git error; when `rollback_stash` could not
restore, wrap/augment the message to include *"your changes are safe at stash@{0}"* (use
`AppError::Git` if augmenting).

### 2.8 `pop_after_success` — re-apply the autostash after a successful FF / merge-commit

```rust
enum PopResult { Restored, Conflicted(Vec<String>) }

fn pop_after_success(repo: &mut git2::Repository, workdir: &Path)
    -> Result<PopResult, AppError>
{
    // No REINSTATE_INDEX (OPEN Q #1): staged changes return as unstaged. SAFE checkout (default).
    match repo.stash_pop(0, Some(&mut git2::StashApplyOptions::new())) {
        Ok(()) => Ok(PopResult::Restored),
        // libgit2 git_stash_pop applies then drops ONLY on success; on GIT_ECONFLICT it returns
        // early and the stash entry is RETAINED. So a Conflict here means: FF/merge already
        // landed, local changes partially applied with markers, stash still on the stack.
        Err(e) if e.code() == git2::ErrorCode::Conflict => {
            let paths = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
            Ok(PopResult::Conflicted(paths))
        }
        Err(e) => Err(e.into()),   // rare; stash is retained (pop did not drop) — safe
    }
}
```

Mapping at the call sites:
- FF: `Restored` → `FastForwarded { branch, to, stashed: true }`;
  `Conflicted(paths)` → `StashPopConflicts { head: to, paths }`.
- Merge: `Restored` → `Merged { oid, stashed: true }`;
  `Conflicted(paths)` → `StashPopConflicts { head: oid, paths }`.

After `StashPopConflicts` the repo state is `Clean` (FF makes no merge state; a clean merge is
already committed; a conflicted stash-apply is not a merge op). The index holds conflict entries
and the worktree has `<<<<<<<` markers — a legitimate git state the user resolves manually, then
drops the retained stash.

---

## 3. Edge-case matrix (the crux)

| # | Situation | git2 sequence | Repo state after | Stash disposition | `MergeOutcome` |
|---|-----------|---------------|------------------|-------------------|----------------|
| 0 | Not dirty | no stash; FF or merge as P3c | as P3c | — | `UpToDate` / `FastForwarded{stashed:false}` / `Merged{stashed:false}` / `Conflicts{stashed:false}` — **identical to today** |
| 1 | Up to date (any tree) | analysis only, **before** any stash | Clean, unchanged | none created | `UpToDate` |
| 2 | Dirty + FF + clean pop | stash → SAFE checkout → set_target → pop OK | Clean, at target; local edits restored (staged→unstaged per OPEN Q #1) | created, applied, dropped | `FastForwarded{stashed:true}` |
| 3 | Dirty + clean normal merge + clean pop | stash → merge → auto-commit → pop OK | Clean, merge commit on HEAD; local edits restored | created, applied, dropped | `Merged{stashed:true}` |
| 4 | Dirty + FF/merge OK but **pop conflicts** | …→ pop → `GIT_ECONFLICT` | **Clean** (not Merge); worktree has markers; index has conflict entries | **RETAINED at stash@{0}** (libgit2 does not drop on conflict) | `StashPopConflicts{head,paths}` |
| 5 | Dirty + **normal merge pauses on conflicts** | stash → merge → `index.has_conflicts()` | **Merge** (MERGE_HEAD/MERGE_MSG written), paused | **RETAINED at stash@{0}** — deferred (OPEN Q #2); the user re-applies after `commit_merge`/`abort_merge` | `Conflicts{paths,stashed:true}` |
| 6 | Failure AFTER stash, BEFORE terminal (e.g. untracked file blocks FF checkout) | stash → checkout `Err` → `rollback_stash` | Clean, original dirty state restored | rolled back (dropped); if rollback itself fails → RETAINED at stash@{0}, said in error | `Err(CheckoutConflict…)` (message notes stash if rollback failed) |
| 7 | `stash_save` itself fails | propagate | unchanged, nothing mutated | none created | `Err` (same safety as today) |

**Justification vs `git merge --autostash`:**
- Rows 2/3 are the canonical autostash success — dirty tree merges and comes back.
- Row 4 mirrors git: a conflicting autostash re-apply leaves markers and keeps the stash entry;
  git prints *"Applying autostash resulted in conflicts… stash is kept"*. We surface the same via
  a dedicated outcome kind.
- Row 5 is the deliberate v1 simplification (OPEN Q #2): git defers the re-apply to
  continue/abort; we defer it to the user, keeping the stash on the stack. SIMPLE + SAFE, never
  drops a stash.
- Row 6 keeps git-consistency (untracked files still block) while guaranteeing recovery.

---

## 4. Wire type & TS mirror

### 4.1 Rust (`merge.rs` — extend the existing enum)

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum MergeOutcome {
    UpToDate,
    FastForwarded { branch: String, to: String, stashed: bool },
    Merged { oid: String, stashed: bool },
    Conflicts { paths: Vec<String>, stashed: bool },
    /// FF / merge-commit landed, but re-applying the autostash conflicted.
    /// The stash entry is RETAINED at stash@{0}. `head` = FF target or new
    /// merge-commit oid; `paths` = conflicted paths from the stash apply.
    StashPopConflicts { head: String, paths: Vec<String> },
}
```

`stashed` uniform meaning: *"an autostash was created for this operation."* The **kind** conveys
its disposition — `FastForwarded`/`Merged` imply it was restored cleanly; `Conflicts` implies it
is retained on the stack (paused merge); `StashPopConflicts` always implies a retained stash.

### 4.2 TypeScript (`src/ipc/types.ts:271`)

```ts
export type MergeOutcome =
  | { kind: 'upToDate' }
  | { kind: 'fastForwarded'; branch: string; to: string; stashed: boolean }
  | { kind: 'merged'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };
```

### 4.3 Command / event / channel surface

**Unchanged.** Command `merge_branch(repoId, name) -> MergeOutcome` (`commands.rs:851`,
`tauri.ts:145`). No new events; no channels. Does not emit `repo-changed` (frontend refetches
imperatively, as today).

---

## 5. Frontend — `handleMergeBranch` toast mapping (`RepoWorkspace.tsx:775`)

`stash@{0}` shown literally so the user can find it via CLI.

```ts
switch (res.kind) {
  case 'upToDate':
    pushToast('info', `Already up to date with ${name}`); break;
  case 'fastForwarded':
    pushToast('success', `Fast-forwarded to ${name}` +
      (res.stashed ? ' (local changes stashed and restored)' : '')); break;
  case 'merged':
    pushToast('success', `Merged ${name}` +
      (res.stashed ? ' (local changes stashed and restored)' : '')); break;
  case 'conflicts':
    pushToast('info', `Merge paused: ${res.paths.length} conflict(s) to resolve` +
      (res.stashed
        ? '. Your local changes are safe on the stash (stash@{0}) — apply them after finishing the merge.'
        : '')); break;
  case 'stashPopConflicts':
    pushToast('error',
      `Merge done, but re-applying your stashed changes hit ${res.paths.length} conflict(s). ` +
      'Your changes are still on the stash (stash@{0}); resolve the conflicts, then drop the stash.');
    break;
}
await refreshAll();
```

Tone rationale: `stashPopConflicts` uses the `error` tone (of the available
`success`|`info`|`error`) purely for visibility — no data is lost, but the user must act.

---

## 6. Mock IPC (`src/ipc/mock.ts:1013`) — keep browser harness implementable

- The existing clean-merge return becomes `{ kind: 'merged', oid, stashed: false }`.
- Add a lightweight demo trigger so the harness can exercise every new shape without a real repo,
  keyed on the branch `name` (mirrors the existing `?op=` fixture convention):
  - name contains `"autostash"` → `{ kind: 'merged', oid, stashed: true }`.
  - name contains `"stash-conflict"` → `{ kind: 'stashPopConflicts', head: randomOid(),
    paths: ['src/app.ts'] }` (do not mutate `opState`; repo stays "clean").
  - name contains `"conflict"` (existing paused-merge demo, if present) → include `stashed: true`
    on the `conflicts` result.
- No new mock methods; signature of `mergeBranch` is unchanged. `tauri.ts` needs no change.

---

## 7. Acceptance criteria & AI gate

**AI gate (orchestrator-verifiable, no network):**

- `cargo check` + `clippy` clean; `pnpm build` + `tsc` clean; browser harness renders each new
  outcome toast from the mock triggers in §6.
- Rust unit tests in `merge.rs` `#[cfg(test)]` using `crate::testutil::scratch_dir()` (tempfile).
  **Tester env:** set `TMP`/`TEMP` to `D:\Temp` (USER MANDATE); run `cargo test` and `clippy`
  **sequentially**, never concurrently (target-dir race).
- Extend `wire_shapes_are_camel_case_tagged` (`merge.rs:346`) to assert the new fields/variant,
  e.g. `{"kind":"fastForwarded","branch":"main","to":…,"stashed":true}` and
  `{"kind":"stashPopConflicts","head":…,"paths":[…]}`.

**Required test matrix (one test per row; assert outcome + on-disk state):**

1. **Not-dirty FF unchanged** — FF-able upstream, clean tree → `FastForwarded{stashed:false}`;
   identical to P3c.
2. **Dirty (unstaged) FF round-trip** (matrix #2) — unrelated tracked file edited but unstaged;
   FF-able → `FastForwarded{stashed:true}`; assert HEAD moved to target **and** the edit is
   present in the worktree afterwards.
3. **Dirty (staged) FF round-trip** — stage an unrelated change; FF → `FastForwarded{stashed:true}`;
   assert the change content survives (as *unstaged*, per OPEN Q #1 — document in the assertion).
4. **Dirty clean normal merge** (matrix #3) — unrelated dirty edit + non-FF mergeable branch →
   `Merged{stashed:true}`; assert 2-parent commit **and** dirty edit preserved.
5. **Stash-pop conflict** (matrix #4) — locally edit file X (unstaged); FF target also modifies X
   → `StashPopConflicts{paths:["X"]}`; assert `repo.state()==Clean`, `X` contains conflict
   markers, and `stash_foreach` count `== 1` (stash retained).
6. **Normal-merge paused + dirty** (matrix #5) — conflicting merge on file X **plus** an unrelated
   dirty file Y → `Conflicts{stashed:true}`; assert `repo.state()==Merge`, MERGE_HEAD present,
   and `stash_foreach` count `== 1` (Y's change is on the stash, worktree Y at HEAD version).
7. **Rollback on blocked FF** (matrix #6) — dirty tracked edit + an **untracked** file the FF
   would create → `Err(CheckoutConflict)`; assert `repo.state()==Clean`, the dirty tracked edit
   is restored, and `stash_foreach` count `== 0` (rolled back, nothing left behind).

Optional CLI oracle (where feasible): compare test 2's final HEAD + worktree to a scratch repo run
through real `git merge --autostash`.

**USER CHECKPOINT (native app):** in a real repo with uncommitted changes, right-click a branch →
Merge; confirm the FF/merge lands and the local changes reappear; confirm the paused-merge and
pop-conflict toasts read clearly and the stash is findable via `git stash list`.

---

## 8. File touch list for senior-dev

- `src-tauri/src/git/merge.rs` — remove refusal `88-97`; add `stash_save`/`rollback_stash`/
  `pop_after_success` helpers; reorder analysis-before-stash; extend `MergeOutcome`; add tests.
- `src/ipc/types.ts:271` — extend `MergeOutcome`.
- `src/ipc/mock.ts:1013` — add `stashed`/demo triggers (§6).
- `src/components/RepoWorkspace.tsx:775` — toast mapping (§5).
- No change to `commands.rs`, `tauri.ts`, `error.rs`, or the command/event/channel surface.
