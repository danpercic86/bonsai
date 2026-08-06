# P47 — Cherry-pick enhancements + commit-action menu consolidation

Status: contract (architect). Implements the approved plan
`i-need-you-to-replicated-goose.md`. Extends P20 (`docs/contracts/P20-daily-essentials.md`
§5/§6) and mirrors the merge autostash of P3c (`crates/bonsai-core/src/git/merge.rs`).

## 0. Overview & invariants

Single-commit cherry-pick / revert already ship end-to-end (P20b). P47 adds three
user-requested capabilities and one bug fix, in locked scope:

- **A. Menu consolidation** — a shared `commitActionItems(oid)` sub-builder, spread into the
  branch pill menu AND (graph) tag pill menu, so oid-based commit actions are reachable from
  refs, not just commit rows.
- **B1. Autostash** a dirty (tracked) worktree for cherry-pick AND revert, mirroring merge.
- **B2. Editable commit message** for cherry-pick (revert keeps its deterministic message).
- **B3. Fix** the paused cherry-pick/revert conflict-fetch bug at `RepoWorkspace.tsx:555`.

**Out of scope (do not implement):** detached-HEAD cherry-pick/revert (keep the existing
attached-born-HEAD gate), multi-commit / range picks, `--allow-empty`.

Invariants held (non-negotiable):
- Rust owns ALL git logic. React only renders + dispatches IPC.
- `bonsai-core` is runtime-free (no Tauri types); heavy calls wrapped in `spawn_blocking` at
  the `src-tauri/src/commands.rs` layer.
- IPC carries compact precomputed data; commands = request/response.
- `src/ipc/mock.ts` MUST keep compiling and drive every new flow with fixture data so the
  browser harness (`VITE_MOCK_IPC=1`) runs the whole feature without Tauri.
- File-size / single-responsibility: the message dialog is a NEW file
  (`src/components/CherrypickMessageDialog.tsx`), never appended to `WorkspaceDialogs.tsx`;
  the autostash helpers live in a NEW focused module `crates/bonsai-core/src/git/autostash.rs`.

### git2 0.21.0 API (verified vs `Cargo.lock`: git2 line 1400 = 0.21.0, libgit2-sys line 2129
### = 0.18.7+1.9.6). No new crate; all calls already used by `merge.rs`:
- `Repository::stash_save2(&mut self, stasher: &Signature, message: Option<&str>, flags:
  Option<StashFlags>) -> Result<Oid>`
- `Repository::stash_apply(&mut self, index: usize, opts: Option<&mut StashApplyOptions>)
  -> Result<()>`
- `Repository::stash_drop(&mut self, index: usize) -> Result<()>`
- `Repository::statuses(Option<&mut StatusOptions>) -> Result<Statuses>` (dirty probe)
- `Repository::cherrypick`, `revert`, `commit`, `cleanup_state`, `reset` (unchanged from P20)
- `StashFlags::DEFAULT` (tracked index+worktree, NOT untracked — matches git autostash).

---

## 1. On-disk state

No new on-disk schema. Existing sequencer files are reused:
- Cherry-pick pause: `.git/CHERRY_PICK_HEAD` (picked oid, author source) +, for the message
  override, `.git/MERGE_MSG` (see §2.2). State `RepositoryState::CherryPick`.
- Revert pause: `.git/REVERT_HEAD`. State `RepositoryState::Revert`.
- Autostash: a normal stash entry at `stash@{0}`, labelled
  `"bonsai: autostash before cherry-pick"` / `"bonsai: autostash before revert"`. RETAINED on
  any conflict pause or pop-conflict (never silently dropped), popped only after a clean
  finalize — identical discipline to merge (§2.7/§2.8 of P3c).

---

## 2. Rust core

### 2.1 New shared module `crates/bonsai-core/src/git/autostash.rs`

Extract the three merge autostash helpers into a shared, message-parameterized module so
cherry-pick, revert AND merge share ONE implementation (file-size / DRY invariant; the plan's
"preferred" option). Register `pub mod autostash;` in `git/mod.rs` (alphabetical, after
`ai_summary` — i.e. line ~4/5, before `bisect`). Merge migrates to these helpers in the same
increment (see §11 flag F1).

```rust
//! Shared autostash for operations that need a clean tree (merge / cherry-pick /
//! revert). Stashes TRACKED changes (index + worktree), reset to HEAD, so the
//! subsequent checkout cannot clobber the user's edits. On any failure the stash
//! is RETAINED at stash@{0} — never silently dropped (no data loss).

use std::path::Path;
use crate::error::AppError;

/// Result of re-applying the autostash after a successful operation.
pub enum PopResult {
    /// Clean re-apply; the stash was dropped (equivalent to a clean pop).
    Restored,
    /// Re-apply produced conflict markers; the stash is RETAINED at stash@{0}.
    Conflicted(Vec<String>),
}

/// True iff the working tree has any TRACKED change (staged or unstaged).
/// Untracked and ignored files are excluded (mirrors git's autostash default).
pub fn is_dirty(repo: &git2::Repository) -> Result<bool, AppError>;

/// Autostash tracked changes with `label`, resetting the tree to HEAD.
/// Uses `StashFlags::DEFAULT` (NOT KEEP_INDEX, NOT INCLUDE_UNTRACKED).
pub fn stash_save(
    repo: &mut git2::Repository,
    sig: &git2::Signature,
    label: &str,
) -> Result<(), AppError>;

/// On a mutation failure AFTER `stash_save` but BEFORE the terminal outcome:
/// try to restore the user's original dirty state, then return the ORIGINAL
/// error. Drops the stash ONLY on a genuinely clean restore; on a conflicted /
/// failed restore the stash is RETAINED and `err` is augmented with
/// " (your changes are safe at stash@{0})". No-op passthrough when `!stashed`.
/// Uses `stash_apply` (never `stash_pop`) — see the merge.rs rationale (libgit2
/// applies a content conflict as Ok(()) and stash_pop would then silently drop).
pub fn rollback_and_map(
    repo: &mut git2::Repository,
    stashed: bool,
    err: AppError,
) -> AppError;

/// Re-apply the autostash after a SUCCESSFUL finalize. `stash_apply` + inspect
/// the index: clean → `stash_drop` + `Restored`; conflicted → RETAIN + return
/// `Conflicted(paths)` (sorted, from `conflict::list_conflicts`).
pub fn pop_after_success(
    repo: &mut git2::Repository,
    workdir: &Path,
) -> Result<PopResult, AppError>;
```

Bodies are moved verbatim from `merge.rs:296-395` with two changes: `stash_save` takes a
`label: &str` (was hard-coded `"bonsai: autostash before merge"`), and `is_dirty` is factored
out of `merge.rs:120-126`. Merge's call sites update to the new signatures.

### 2.2 `crates/bonsai-core/src/git/cherrypick.rs` — signature + outcome + autostash + message

New outcome (mirror `MergeOutcome`; adds `stashed` + a `StashPopConflicts` arm — a WIRE
change, propagated to TS/mock in §4):

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CherrypickOutcome {
    /// Clean pick, auto-committed. `oid` = the new commit.
    /// `stashed` = an autostash was created AND restored for this pick.
    Committed { oid: String, stashed: bool },
    /// Conflict markers written; CHERRY_PICK_HEAD (+ MERGE_MSG when a message
    /// override was supplied) written; paused in state CherryPick. `paths` =
    /// sorted conflicted paths. `stashed` = an autostash was created and is
    /// RETAINED on the stack (deferred re-apply, same as merge).
    Conflicts { paths: Vec<String>, stashed: bool },
    /// The pick committed cleanly, but re-applying the autostash conflicted.
    /// The stash is RETAINED at stash@{0}. `head` = the new commit oid; `paths`
    /// = conflicted paths from the stash apply.
    StashPopConflicts { head: String, paths: Vec<String> },
}
```

New public signature (adds `message`; keeps all preconditions):

```rust
/// Blocking. Cherry-picks `oid` onto the current branch.
///
/// `message`: `None` → reuse the picked commit's message verbatim (P20 behavior,
/// no regression). `Some(m)` → commit with the normalized `m` instead, PRESERVING
/// the original author and using a fresh committer (P20 identity rules).
///
/// A dirty TRACKED worktree is autostashed first (mirrors merge); the stash is
/// restored after a clean finalize, RETAINED on any conflict/pop-conflict.
///
/// Preconditions (checked BEFORE any mutation, unchanged from P20 except the
/// dirty-index guard is REPLACED by autostash):
///   state Clean; HEAD born (else `Git("no commits yet")`); HEAD attached (else
///   `Git("HEAD is detached")`); `oid` resolvable; git identity configured.
///
/// Errors: `OperationInProgress` | `Git` | `CheckoutConflict` | `ConfigMissing`
/// | `NothingToCommit` | any git2 error.
pub fn cherrypick_commit(
    workdir: &Path,
    oid: &str,
    message: Option<&str>,
) -> Result<CherrypickOutcome, AppError>;

/// Blocking. Finalizes a paused (resolved) cherry-pick. Reads the message from
/// MERGE_MSG if present+non-empty (honoring an override persisted at pick time),
/// else the picked commit's message. Does NOT auto-pop a retained autostash
/// (mirrors merge OPEN Q #2 — the user re-applies stash@{0} manually).
/// Returns `Committed { oid, stashed: false }`.
/// Errors: `NoOperationInProgress` | `UnresolvedConflicts` | `ConfigMissing`
/// | `NothingToCommit` | `Git`.
pub fn cherrypick_continue(workdir: &Path) -> Result<CherrypickOutcome, AppError>;

/// Unchanged from P20. reset --hard HEAD + cleanup_state. Does NOT pop a
/// retained autostash (git `cherry-pick --abort` leaves stashes untouched).
pub fn cherrypick_abort(workdir: &Path) -> Result<(), AppError>;
```

`finalize_cherrypick` gains a message parameter and a message-resolution precedence:

```rust
fn finalize_cherrypick(
    repo: &git2::Repository,
    committer: &git2::Signature,
    message: Option<&str>,   // clean-path override (may be None)
) -> Result<CherrypickOutcome, AppError>;
// Message precedence:
//   1. `message` Some(m)                          -> normalize_message(m)
//   2. else .git/MERGE_MSG present & non-empty     -> that (continue path)
//   3. else the picked commit's original message   -> normalize_message(pick.message)
// Author ALWAYS the picked commit's author. Committer = supplied `committer`.
// Empty-tree guard + cleanup_state + Committed{ stashed:false } unchanged.
```

Control flow of `cherrypick_commit` (pseudocode — replaces the dirty-index guard):

```
repo = open_workdir_repo(workdir)
if repo.state() != Clean: return OperationInProgress
head = read_head_info(repo)
if head.unborn: return Git("cannot cherry-pick: no commits yet")
if head.detached: return Git("cannot cherry-pick: HEAD is detached")
pick = repo.find_commit(parse(oid))
if repo.index().has_conflicts(): return Git("index has conflicts")   # cannot stash safely
sig = resolve_signature(...)                                         # ConfigMissing early
stashed = autostash::is_dirty(repo)
if stashed: autostash::stash_save(repo, sig, "bonsai: autostash before cherry-pick")
match repo.cherrypick(pick, None):
    Err(e):
        repo.cleanup_state()
        mapped = if e is Conflict { CheckoutConflict(...) } else { e.into() }
        return Err(autostash::rollback_and_map(repo, stashed, mapped))
    Ok: pass
if repo.index().has_conflicts():
    paths = list_conflicts(workdir)
    if let Some(m) = message: write normalize_message(m) -> .git/MERGE_MSG   # survive continue
    return Conflicts { paths, stashed }
outcome = finalize_cherrypick(repo, sig, message)?      # Committed{ oid, stashed:false }
if stashed:
    match autostash::pop_after_success(repo, workdir):
        Restored          -> return Committed { oid, stashed: true }
        Conflicted(paths) -> return StashPopConflicts { head: oid, paths }
return Committed { oid, stashed: false }
```

### 2.3 `crates/bonsai-core/src/git/revert.rs` — autostash parity (NO message override)

Revert adopts the identical autostash flow but KEEPS its deterministic
`Revert "<subject>"` message (P20 §6; no editable message — see §11 flag F2).

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RevertOutcome {
    Committed { oid: String, stashed: bool },
    Conflicts { paths: Vec<String>, stashed: bool },
    StashPopConflicts { head: String, paths: Vec<String> },
}

/// Signature UNCHANGED (no message param). Adds autostash exactly as cherry-pick
/// (label "bonsai: autostash before revert"); message stays the byte-exact
/// `git revert --no-edit` text.
pub fn revert_commit(workdir: &Path, oid: &str) -> Result<RevertOutcome, AppError>;
pub fn revert_continue(workdir: &Path) -> Result<RevertOutcome, AppError>; // Committed{stashed:false}
pub fn revert_abort(workdir: &Path) -> Result<(), AppError>;               // unchanged
```

Same control flow as §2.2 minus the message branch: no MERGE_MSG override write;
`finalize_revert` unchanged except it returns `Committed { oid, stashed:false }`.

---

## 3. `src-tauri/src/commands.rs` — command layer

Thin async wrapper + runtime-free `_inner` + `spawn_blocking` (pattern at
`commands.rs:2413`). Only `cherrypick_commit` changes shape (adds `message`); the rest change
only their return type (the outcome enum gained fields — automatic). Registration in
`src-tauri/src/lib.rs:148` list is unchanged (same command names).

```rust
#[tauri::command]
pub async fn cherrypick_commit(
    state: tauri::State<'_, AppState>,
    repo_id: String,
    oid: String,
    message: Option<String>,          // NEW
) -> Result<CherrypickOutcome, AppError>;
// _inner: spawn_blocking(move || cherrypick::cherrypick_commit(&path, &oid, message.as_deref()))
```

`revert_commit`, `*_continue`, `*_abort` command signatures unchanged.

---

## 4. IPC surface

### Commands (no name changes; one added arg, three enriched return shapes)

| Command (Rust `snake_case` / TS `camelCase`) | Args change | Return change |
|---|---|---|
| `cherrypick_commit` / `cherrypickCommit` | `+ message: Option<String>` / `message?: string \| null` | `CherrypickOutcome` gains `stashed` + `stashPopConflicts` |
| `revert_commit` / `revertCommit` | none | `RevertOutcome` gains `stashed` + `stashPopConflicts` |
| `cherrypick_continue`/`revert_continue`/`*_abort` | none | outcome shape only |

No new events, no channels. (Cherry-pick/revert do not emit `repo-changed`; the UI calls
`refreshAll()` — unchanged from P20.)

### TypeScript triple

`src/ipc/types.ts` (`CherrypickOutcome` ~:705, `RevertOutcome` ~:494 region, `IpcApi.cherrypickCommit` ~:1453):

```ts
export type CherrypickOutcome =
  | { kind: 'committed'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

export type RevertOutcome =
  | { kind: 'committed'; oid: string; stashed: boolean }
  | { kind: 'conflicts'; paths: string[]; stashed: boolean }
  | { kind: 'stashPopConflicts'; head: string; paths: string[] };

// IpcApi:
cherrypickCommit(repoId: string, oid: string, message?: string | null): Promise<CherrypickOutcome>;
// revertCommit signature UNCHANGED.
```

`src/ipc/tauri.ts:482`:

```ts
cherrypickCommit(repoId, oid, message = null) {
  return invoke<CherrypickOutcome>('cherrypick_commit', { repoId, oid, message });
},
```

`src/ipc/mock.ts` (`cherrypickCommit` ~:3560, `seedPickRevertConflict` ~:644): see §7.3.

---

## 5. opstate

**No change.** `RepoOpState` already carries `{ kind:'cherryPick' }` and `{ kind:'revert' }`
(`types.ts:323-324`; Rust `opstate.rs`). The paused-conflict bug (§6.3) is a frontend
fetch-gating bug, not an opstate schema gap.

---

## 6. Frontend entry points

### 6.1 `src/components/workspaceMenus.ts` — shared `commitActionItems(oid)` (Part A)

New sub-builder, placed beside `resetMenuItems` (:458). Owns the five oid-based commit
actions currently inline in `commitMenuItems` (:476). Gating semantics preserved:

```ts
// Returns [] when there is no usable HEAD; callers spread unconditionally.
function commitActionItems(oid: string): ContextMenuItem[] {
  if (head === null || head.unborn) return [];
  const gate = mutating || opActive;
  const items: ContextMenuItem[] = [
    { label: 'Create branch here', icon: BranchIcon, disabled: gate,
      onSelect: () => setPendingCreateBranch({ oid }) },
    { label: 'Create tag here', icon: TagIcon, disabled: gate,
      onSelect: () => setPendingCreateTag({ oid }) },
    // 'Compare with HEAD' always available once HEAD is born (never gated).
    { label: 'Compare with HEAD', icon: CompareIcon, disabled: false,
      onSelect: () => handleCompareWithHead(oid) },
  ];
  // Cherry-pick / revert excluded on detached HEAD (backend rejects it; mirrors
  // resetMenuItems). handleCherrypick opens the message dialog (§6.2).
  if (!head.detached) {
    items.push(
      { label: 'Cherry-pick onto current…', icon: RebaseIcon, disabled: gate,
        onSelect: () => handleCherrypick(oid) },
      { label: 'Revert commit', icon: RebaseIcon, disabled: gate,
        onSelect: () => void handleRevert(oid) },
    );
  }
  return items;
}
```

`commitMenuItems(oid)` becomes: `[...commitActionItems(oid), <detached? [] : interactive-rebase-from-here + bisect items>, ...resetMenuItems(oid)]`. Interactive-rebase-from-here
and the two bisect items STAY commit-only (not part of the shared set).

`branchMenuItems`: **remove** its inline "Create branch here" (:184-189) and "Compare with
HEAD" (:269-276); spread `...commitActionItems(entry.tip)` in their place — recommended
insertion right after the Merge/Rebase block and before "Delete", so the commit actions form
one contiguous group. The current-HEAD-branch pill already returns `[]` (:172) → self
cherry-pick naturally excluded. No double "Create branch"/"Compare".

### Fork 1 RESOLVED — tag-menu oid source

`BranchesSnapshot.tags` is `string[]` (verified `types.ts:235`) — no oids — so a **sidebar**
tag row cannot cheaply resolve its target commit. A **graph** tag pill, however, sits on a
concrete `GraphNode`, whose `id` IS the tag's (peeled) target oid, and that node is in scope
at the context dispatch (`GraphCanvas.tsx:833/851`). Resolution:

1. Thread the oid through the ref target (frontend-only, correct, cheap):
   ```ts
   // GraphCanvas.tsx (GraphContextTarget):
   export type GraphContextTarget =
     | { kind: 'ref'; ref: RefLabel; oid: string }   // + oid = node.id
     | { kind: 'commit'; index: number; oid: string };
   // Both onContextMenu ref emissions (:833, :851) add `oid: node.id`.
   ```
2. `tagMenuItems(name: string, oid: string | null)`: after the existing delete/copy/push
   items, `if (oid !== null) items.push(...commitActionItems(oid));`.
3. `buildContextItems` (:568): `return tagMenuItems(r.name, target.oid);`. The sidebar tag row
   (`RepoWorkspace.tsx:2756`) calls `menus.tagMenuItems(name, null)` → delete/copy/push only.

**Net:** branch pills (graph + sidebar) AND graph tag pills get the full commit-action set;
**sidebar tag rows are scoped OUT** (no cheap oid) — flagged F3. Branch-pill inclusion, the
plan's primary demo, ships fully.

### A3 reverse-audit (documented complete)

Deliberately NOT commit-generalized (genuinely ref-specific): **Checkout** (needs detached
HEAD, out of scope), **Copy branch/tag name**, **View reflog**, **Summarize / Review branch**
(AI, base-relative), **Merge/Rebase into/onto**, **Delete**, tag **Push to <remote>**. The
"no arbitrary split" audit is therefore complete.

### 6.2 Fork 2 RESOLVED — `src/components/CherrypickMessageDialog.tsx` (new file)

Default: `handleCherrypick(oid)` opens a prefilled, editable message dialog. The container
(`RepoWorkspace`) fetches the source commit's FULL message via the existing
`ipc.getCommitDiff(repoId, oid)` → `details.message` (verified selector: `CommitDetails.message`
= "Full message, trailing whitespace trimmed", `types.ts:159`). The graph node only carries
`summary` (first line), so a fetch is required for a faithful prefill.

New presentational component (small, single-responsibility — NOT in `WorkspaceDialogs.tsx`):

```ts
export interface CherrypickMessageDialogProps {
  oid: string;                       // source commit (for the title, short oid)
  initialMessage: string;            // prefilled full message
  loading: boolean;                  // fetching the source message
  busy: boolean;                     // pick invoke in flight
  onConfirm(message: string): void;  // edited message
  onCancel(): void;
}
```

Container wiring in `RepoWorkspace.tsx` (replaces the immediate-invoke `handleCherrypick` at
:2495):

```
state pendingCherrypick: { oid: string; initialMessage: string; loading: boolean } | null
handleCherrypick(oid):
  set pendingCherrypick = { oid, initialMessage: '', loading: true }; open dialog
  try { m = (await ipc.getCommitDiff(repoId, oid)).details.message }
  catch { m = '' /* fall back to empty; user can type */ }
  set pendingCherrypick.initialMessage = m, loading = false   (guard against stale/cancel)
confirmCherrypick(message):
  setMutating(true)
  res = await ipc.cherrypickCommit(repoId, oid, message)     // message may equal original
  handle res (see §6.4 outcome toasts); await refreshAll(); close dialog
```

Rendered from `WorkspaceDialogs.tsx`'s composition site OR directly in `RepoWorkspace`'s dialog
region — but the component file itself is standalone. Alternative (one-click, no dialog) is
noted only as a fallback; **default is the dialog** (F4 non-blocking).

### 6.3 Fork/Part B3 — paused conflict fetch bug (`RepoWorkspace.tsx:555`)

```ts
const list =
  op.kind === 'merge' || op.kind === 'rebase' || op.kind === 'cherryPick' || op.kind === 'revert'
    ? await ipc.listConflicts(repoId)
    : [];
```

Effect: a paused cherry-pick/revert now reports its real `conflictCount`, so the OpBanner
"Continue" button stays DISABLED until conflicts are resolved and the conflicted files list
renders. Verify against the mock (§7.3) drive.

### 6.4 Outcome handling (both handlers)

- `committed`: success toast; if `stashed` append " · stash restored".
- `conflicts`: info toast "N conflict(s) to resolve"; if `stashed` append " · your changes are
  stashed (stash@{0})".
- `stashPopConflicts`: info/warning toast "Cherry-picked <short head>; re-applying your stashed
  changes conflicted — resolve them (stash@{0})". `refreshAll()` in all cases.

---

## 7. Tests

### 7.1 Rust unit (`#[cfg(test)]` in `cherrypick.rs` / `revert.rs`)
- `wire_shapes_are_camel_case_tagged`: update for `stashed` + add a `StashPopConflicts` case
  (`{ "kind": "stashPopConflicts", "head": "...", "paths": [...] }`).
- Preconditions-on-fresh-repo tests unchanged (unborn/detached still refuse before mutation).

### 7.2 CLI oracle (extend `crates/bonsai-core/tests/essentials_cli.rs` — the P20 pick/revert
### oracle; 37 existing refs). Assert against real `git`:
- **autostash-clean parity:** dirty tracked worktree + a non-conflicting pick →
  `Committed{stashed:true}`; resulting HEAD tree + committed message + author match
  `git stash push` → `git cherry-pick <oid>` → `git stash pop`; worktree changes restored;
  `git stash list` empty.
- **custom-message parity:** `Some(m)` on a clean pick → new commit message == `normalize(m)`,
  author == source author, committer == configured identity, tree == plain pick tree.
- **custom-message survives conflict:** conflicting pick with `Some(m)` writes `m` to
  `.git/MERGE_MSG`; after resolving + `cherrypick_continue`, the commit message == `normalize(m)`.
- **autostash conflict retains stash:** conflicting pick on a dirty tree →
  `Conflicts{stashed:true}` and `git stash list` still shows the entry.
- **revert autostash parity:** dirty tree + clean revert → `Committed{stashed:true}`, message ==
  byte-exact `git revert --no-edit`, stash restored.
- **stash-pop-conflict:** pick that commits cleanly but whose stash re-apply conflicts →
  `StashPopConflicts{head,paths}`, stash retained.

Windows temp mandate: scratch repos under `D:\Temp\bonsai-scratch`; set `TMP`/`TEMP` to
`D:\Temp`. Never run `cargo test` and `cargo clippy` concurrently (target-dir race).

### 7.3 Mock (`src/ipc/mock.ts`)
- `cherrypickCommit(repoId, oid, message?)`: accept the new arg; use `message` (when provided)
  as the new top-node summary's first line. Return `stashed: <state has tracked dirty>` on the
  clean path (derive from `state.status.unstaged`/`staged` non-empty). Add a
  `STASH_POP_CONFLICT_OID_SUFFIX` so a chosen oid returns `{ kind:'stashPopConflicts', head,
  paths:['src/app.ts'] }`; keep the existing `PICK_REVERT_CONFLICT_OID_SUFFIX` path but extend
  its return to `{ kind:'conflicts', paths, stashed:true }`.
- `revertCommit`: mirror (stashed + stashPopConflicts driver).
- `seedPickRevertConflict` (~:644) unchanged (opstate/conflict seeding) — it already backs the
  §6.3 fix drive; confirm `listConflicts` returns the seeded path so Continue disables.
- `getCommitDiff` (~:2161): ensure at least one fixture commit's `details.message` is
  MULTI-LINE (summary + body) so the dialog prefill is visibly non-trivial in the harness.

### 7.4 tsc / build
`pnpm build` (tsc) clean; mock and real IPC both compile against the new signatures/types.

---

## 8. Module boundaries

| File | Responsibility | P47 change |
|---|---|---|
| `crates/bonsai-core/src/git/autostash.rs` | shared autostash (is_dirty/save/rollback/pop) | NEW |
| `crates/bonsai-core/src/git/cherrypick.rs` | pick core | outcome variants, `message` param, autostash, MERGE_MSG override |
| `crates/bonsai-core/src/git/revert.rs` | revert core | outcome variants, autostash |
| `crates/bonsai-core/src/git/merge.rs` | merge core | migrate to `autostash::*` (F1) |
| `crates/bonsai-core/src/git/mod.rs` | module registry | `pub mod autostash;` |
| `src-tauri/src/commands.rs` | Tauri command layer | `cherrypick_commit` gains `message` |
| `src/ipc/types.ts` / `tauri.ts` / `mock.ts` | IPC triple | outcome union + `message` arg + mock drives |
| `src/components/workspaceMenus.ts` | context menus | `commitActionItems`, branch/tag spread |
| `src/graph/GraphCanvas.tsx` | graph canvas + ctx dispatch | `oid` on the ref target |
| `src/components/CherrypickMessageDialog.tsx` | editable-message dialog | NEW |
| `src/components/RepoWorkspace.tsx` | container | dialog wiring, outcome toasts, `:555` fix, `tagMenuItems(name,null)` |

---

## 9. Sub-increments (commit each after review)

- **P47a — Rust core.** `autostash.rs` (extract from merge; migrate merge) + cherry-pick
  outcome/message/autostash + revert autostash. Unit tests updated; compiles + clippy clean.
- **P47b — IPC + commands.** `commands.rs` `message` arg; TS triple (`types.ts`/`tauri.ts`/
  `mock.ts`) outcome union + mock drives. `pnpm build` clean; harness loads.
- **P47c — Menu consolidation (Part A).** `commitActionItems` + branch/tag spread + the
  `GraphContextTarget.oid` thread + `tagMenuItems(name, oid)` + sidebar `null` call site.
- **P47d — Dialog + bug fix (Part B UI).** `CherrypickMessageDialog.tsx`, `handleCherrypick`
  rewrite + outcome toasts, and the `:555` paused-conflict fix.

---

## 10. AI gate vs USER CHECKPOINT

**AI gate (orchestrator verifies):**
- `cargo test -p bonsai-core` green incl. the new autostash + custom-message oracle tests.
- `cargo clippy` + `pnpm build` (tsc) clean.
- Browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`): right-click a **branch pill** → Cherry-pick /
  Revert / Create-tag / Create-branch / Compare appear; right-click a **graph tag pill** → same;
  **sidebar tag row** → delete/copy/push only (expected). Drive the message dialog → confirm the
  edited message reaches the mock (new top node summary). Drive a paused pick (conflict-suffix
  oid) → OpBanner "Continue" DISABLED until resolved and conflicted files listed (B3). Drive the
  stash-pop-conflict oid → correct toast. Console clean; one final screenshot.

**USER CHECKPOINT (native `pnpm tauri dev`, real repo — user confirms):**
- Dirty worktree + cherry-pick from a branch pill: autostashes, commits with the edited message,
  restores the stash.
- A genuinely conflicting pick pauses; resolve + Continue completes (edited message preserved);
  Abort restores HEAD cleanly (retained autostash recoverable via stash list).
- Menu placement feels right on real branch/tag/commit right-clicks.

---

## 11. Flagged ambiguities (for the orchestrator)

- **F1 — merge migrates to `autostash.rs`.** Default: extract the three helpers as the SINGLE
  source and update `merge.rs` to call them (DRY / file-size invariant, the plan's "preferred").
  Cost: `merge.rs` diff + re-run `merge_cli.rs` oracle. Fallback if the orchestrator wants a
  minimal P47a diff: leave `merge.rs` untouched and let `autostash.rs` serve only
  cherry-pick/revert (temporary duplication). **Recommend F1 default (migrate).**
- **F2 — revert has NO editable message.** Revert keeps its deterministic
  `Revert "<subject>"` text (git `--no-edit` parity); only autostash is added. A revert message
  override is a trivial future parallel to cherry-pick if requested. **Recommend: no override.**
- **F3 — sidebar tag rows scoped OUT of commit actions** (no cheap oid in `BranchesSnapshot.tags`).
  Graph tag pills DO get them (via `node.id`). To include sidebar tag rows later, the backend
  would add tag→oid to the snapshot (out of P47 scope). **Recommend: ship as scoped.**
- **F4 — message UX = dialog (default).** One-click immediate pick + a separate "Cherry-pick &
  edit message…" item is the noted alternative. **Recommend the dialog.**
- **F5 — continue/abort do NOT auto-pop a retained autostash** (mirrors merge OPEN Q #2). The
  user re-applies stash@{0} manually; surfaced via the `stashed` flag + toast. Confirm this
  parity is acceptable (it is the existing merge behavior).
