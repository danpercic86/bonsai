# P9 — Stash management

User request (2026-07-29): *"add support for stashes, to view stashes in graph, apply/pop/delete
stash and so on."*

Add stash support: a Sidebar **Stashes** section (list + "Stash changes" + per-row Apply / Pop /
Drop), and **view stashes in the graph** as a left-column pill on each stash's base commit —
**without** touching the perf-gated graph walk. New Rust module `git/stash.rs`, five commands, one
new `RefLabel` kind (`stash`), and a `StashEntry` wire type.

Reference contracts: `docs/contracts/P8-merge-autostash.md` (stash primitives + apply/pop conflict
semantics — reused verbatim), `docs/contracts/P7-gitkraken-layout.md` (ref-column pill machinery),
`docs/contracts/P6-unified-context-menus.md` (shared ContextMenu + Drop confirm pattern),
`docs/contracts/M2-graph.md` (layout invariants).

---

## OPEN QUESTIONS FOR USER (recommended defaults chosen; implementation is NOT blocked)

1. **Does "Stash changes" include untracked files?** → **Recommend YES**: stash tracked + untracked,
   exclude ignored (`StashFlags::INCLUDE_UNTRACKED`, not `INCLUDE_IGNORED`). Matches the GitKraken
   "stash all my changes" mental model (the worktree comes back clean). Exposed as a command param
   (`includeUntracked`) so a future checkbox can override; the button passes `true`.
2. **Is Pop-with-conflict allowed?** → **Recommend YES** (git- and P8-consistent): pop applies,
   conflict markers land in the worktree, index gets conflict entries, and the stash is **retained**
   (libgit2 only drops on a clean pop). Never lossy. Surface via `ApplyStashOutcome::Conflicts`.
3. **Do Apply / Pop require a clean repo state (no in-progress merge/rebase)?** → **Recommend YES**:
   reject with `operationInProgress` when `repo.state() != Clean`. **Drop is allowed in any state**
   (it only edits the stash reflog, never the worktree).
4. **Reinstate the staged/unstaged split on apply/pop?** → **Recommend NO** (mirror P8 OPEN Q#1): plain
   apply, no `REINSTATE_INDEX`; previously-staged hunks return as **unstaged**. Nothing is lost;
   fewer conflict cases; trivial to re-stage.
5. **Stash pill label text.** → **Recommend `stash@{n}`** (stable, compact, CLI-matching). The stash
   message is shown in the Sidebar row, not on the canvas pill.

All defaults are SAFE (never lose data) and match `git stash` / the existing P8 autostash behavior.

---

## 1. Overview & invariants held

- **Rust owns all Git logic AND the graph layout math.** `stash.rs` wraps every git2 stash call;
  React only renders and dispatches commands.
- **The perf-gated graph walk is untouched.** Stashes are **not** seeded as walk tips and never
  become synthetic multi-parent commits. Each stash contributes at most one `RefLabel` attached to
  an **already-walked** base node in O(stashes) post-processing (§5). The 20k-commit gate is unaffected.
- **IPC carries compact precomputed data.** `list_stashes` returns a small `Vec<StashEntry>`; no raw
  libgit2 objects, no per-stash round-trips for the graph (pills ride inside the existing
  `GraphLayout`). Commands = request/response; no new events or channels.
- **git2 is blocking → `spawn_blocking`.** Every stash command wraps its blocking core exactly like
  `merge_branch_inner` (`commands.rs:861`).
- **Runtime-free cores.** `stash.rs` functions take `&Path` / `&mut Repository`, no Tauri types →
  unit-testable without the Tauri "test" feature (same rule as P8).
- **Destructive Drop needs UI confirmation** (guardrail): Drop routes through a `ConfirmDialog`
  mirroring the branch-delete dialog (`RepoWorkspace.tsx:1428`). Apply/Pop are recoverable (stash is
  retained on conflict) → no confirm, like the P8 autostash.
- **Mock-implementable.** Every command has a `mock.ts` implementation and the graph pill renders
  from static fixture data, so `VITE_MOCK_IPC=1` runs the whole feature in a plain browser.

---

## 2. New Rust module `src-tauri/src/git/stash.rs`

Register in `src-tauri/src/git/mod.rs` (add `pub mod stash;` beside the existing modules, alphabetical
slot after `remote`/`repo` — place `pub mod stash;` before `pub mod status;`).

Open the repo with the existing helper `open_workdir_repo(&Path)` (`git/stage.rs:14`) — it already
rejects bare repos with a clear `AppError::Git`. `stash_foreach` / `stash_apply` / `stash_pop` /
`stash_drop` / `stash_save2` all require **`&mut Repository`**, so bind `let mut repo = …`.

### 2.1 Wire type

```rust
/// One stash stack entry. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StashEntry {
    /// Stack index; 0 == most recent (== stash@{0}). SHIFTS after any drop/pop.
    pub index: usize,
    /// Full stash message, e.g. "WIP on main: 1a2b3c4 summary" or a custom message.
    pub message: String,
    /// Full 40-hex oid of the stash commit itself.
    pub oid: String,
    /// Full 40-hex oid of the stash's FIRST parent = the base commit it was
    /// created from (what the graph pill attaches to).
    pub base_oid: String,
    /// Stash commit author time, seconds since epoch (UTC) — drives relative age.
    pub ts: i64,
}

/// Result of apply/pop. Wire: tagged "kind", camelCase (same recipe as MergeOutcome).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ApplyStashOutcome {
    /// Clean apply/pop. (Pop additionally dropped the entry.)
    Applied,
    /// Worktree has <<<<<<< markers, index has conflict entries, and the stash
    /// entry is RETAINED (libgit2 does not drop on GIT_ECONFLICT). `paths` =
    /// sorted conflicted paths (the set `list_conflicts` returns).
    Conflicts { paths: Vec<String> },
}

/// Result of create_stash.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateStashResult {
    /// false == nothing to stash (clean worktree) → NOT an error.
    pub created: bool,
}
```

### 2.2 Function signatures

```rust
/// Blocking. Enumerate the stash stack, index 0 (most recent) first.
/// `stash_foreach` is the ONLY enumeration API; its callback receives
/// (index, message, &oid). Empty stack → Ok(vec![]).
pub fn list_stashes(workdir: &Path) -> Result<Vec<StashEntry>, AppError>;

/// Blocking. Stash the dirty worktree. `message: None` → git default
/// ("WIP on <branch>: <short> <summary>"). Precondition: state Clean else
/// OperationInProgress. Nothing to stash → Ok(CreateStashResult{created:false}).
pub fn create_stash(
    workdir: &Path,
    message: Option<&str>,
    include_untracked: bool,
) -> Result<CreateStashResult, AppError>;

/// Blocking. Apply stash `index` WITHOUT dropping it. Precondition: state Clean
/// else OperationInProgress. Conflicts → Ok(Conflicts{paths}) (stash retained).
pub fn apply_stash(workdir: &Path, index: usize) -> Result<ApplyStashOutcome, AppError>;

/// Blocking. Apply stash `index` and drop it on clean success only.
/// Precondition: state Clean else OperationInProgress. Conflicts →
/// Ok(Conflicts{paths}) and the entry is RETAINED (libgit2 does not drop).
pub fn pop_stash(workdir: &Path, index: usize) -> Result<ApplyStashOutcome, AppError>;

/// Blocking. Permanently discard stash `index`. Allowed in ANY repo state
/// (touches only the stash reflog). UI confirms first (destructive).
pub fn drop_stash(workdir: &Path, index: usize) -> Result<(), AppError>;
```

### 2.3 Exact git2 calls & internals

- **list_stashes** (mut repo needed for `stash_foreach`; but the closure cannot re-borrow `repo`):
  ```rust
  let mut raw: Vec<(usize, String)> = Vec::new();
  repo.stash_foreach(|index, message, oid| {
      raw.push((index, message.to_string()));
      // NOTE: we intentionally do NOT capture oid here — resolving base/ts needs
      // an immutable repo borrow, impossible inside this &mut closure. Re-resolve below.
      true
  })?;
  ```
  `stash_foreach`'s `&oid` cannot be used to look up parents inside the closure (repo is mutably
  borrowed). After the closure returns, resolve each entry via the stash **reflog**:
  `repo.reflog("refs/stash")?` gives entries in index order (entry 0 == stash@{0}); for entry `i`,
  `id_new()` is the stash commit oid. Then `let c = repo.find_commit(oid)?;` →
  `oid = c.id().to_string()`, `base_oid = c.parent_id(0)?.to_string()`,
  `ts = c.author().when().seconds()`. Pair by index with the `raw` message list.
  (Equivalent alternative: capture `*oid` into `raw` in the closure, then resolve after — either is
  fine; the point is the parent/ts lookup happens **after** the `&mut` foreach.)
- **create_stash**:
  ```rust
  if repo.state() != git2::RepositoryState::Clean {
      return Err(AppError::OperationInProgress(
          "an operation is already in progress — finish or abort it first".to_string()));
  }
  let sig = resolve_signature(&repo.config()?.snapshot()?)?; // commit.rs:39 — stasher identity
  let mut flags = git2::StashFlags::DEFAULT;                 // stash index + worktree, reset both
  if include_untracked { flags |= git2::StashFlags::INCLUDE_UNTRACKED; }
  match repo.stash_save2(&sig, message, Some(flags)) {
      Ok(_oid) => Ok(CreateStashResult { created: true }),
      // libgit2 returns GIT_ENOTFOUND when there is nothing to stash.
      Err(e) if e.code() == git2::ErrorCode::NotFound => Ok(CreateStashResult { created: false }),
      Err(e) => Err(e.into()),
  }
  ```
  `resolve_signature` surfaces `ConfigMissing` early (identity is required to author the stash
  commit), consistent with commit/merge.
- **apply_stash / pop_stash** (mirror P8 §2.8 `pop_after_success`):
  ```rust
  if repo.state() != git2::RepositoryState::Clean { return Err(OperationInProgress(...)); }
  let mut opts = git2::StashApplyOptions::new(); // SAFE checkout default; NO REINSTATE_INDEX (OPEN Q#4)
  let res = repo.stash_apply(index, Some(&mut opts));   // apply
  // (pop uses repo.stash_pop(index, Some(&mut opts)) instead)
  match res {
      Ok(()) => Ok(ApplyStashOutcome::Applied),
      Err(e) if e.code() == git2::ErrorCode::Conflict => {
          let paths = list_conflicts(workdir)?.into_iter().map(|c| c.path).collect();
          Ok(ApplyStashOutcome::Conflicts { paths })
      }
      Err(e) => Err(e.into()),
  }
  ```
  Reuse `crate::git::conflict::list_conflicts` (`git/conflict.rs`) exactly as P8 does (`merge.rs:11`).
  **`stash_pop` drops the entry only on clean success**; on `GIT_ECONFLICT` it returns early with the
  entry retained — so `Conflicts` from either apply or pop leaves the stash on the stack (documented
  to the user in the toast, §6.3).
- **drop_stash**: `repo.stash_drop(index)?;` (no state precondition). An out-of-range `index`
  surfaces as the underlying `git2::Error` → `AppError::Git` (acceptable; the UI always calls with a
  freshly-listed index).

### 2.4 INDEX-SHIFTING CAVEAT (must be honored by callers)

Stash indices are **positional into a mutating stack**. After `drop_stash(i)` or a clean
`pop_stash(i)`, every entry with index `> i` shifts **down by one**; entries `< i` are unchanged.
`apply_stash` does **not** shift indices. Rule: **the frontend refetches `list_stashes` after every
create/pop/drop** and never reuses a stale index across operations. Each command call takes exactly
one index and returns; batching multiple index-based mutations without a refetch in between is
forbidden.

### 2.5 Error mapping (→ `AppError`, `error.rs`)

| Situation | AppError |
|---|---|
| repo not usable / bare | `Git` (via `open_workdir_repo`, `stage.rs:20`) |
| create/apply/pop while state != Clean | `OperationInProgress` |
| identity unset (create) | `ConfigMissing` (via `resolve_signature`) |
| nothing to stash | **not an error** → `CreateStashResult{created:false}` |
| apply/pop conflict | **not an error** → `ApplyStashOutcome::Conflicts{paths}` |
| invalid/out-of-range index; other libgit2 | `Git` |

No new `AppError` variant is required (reuses `git` | `operationInProgress` | `configMissing`).

---

## 3. Commands (`src-tauri/src/commands.rs`) + registration

Add `use crate::git::stash::{self, ApplyStashOutcome, CreateStashResult, StashEntry};` to the import
block (`commands.rs:1-21`). Follow the `merge_branch` / `merge_branch_inner` template
(`commands.rs:851-870`) exactly: thin `#[tauri::command] pub async fn` delegating to a runtime-free
`_inner` that resolves the path via `repo_path(state, repo_id)` (`commands.rs:423`) and runs the
blocking core under `tauri::async_runtime::spawn_blocking`. None emit `repo-changed` (the frontend
refetches imperatively, same as merge/rebase).

```rust
// list_stashes(repoId) -> Vec<StashEntry>            errors: noRepo | git
#[tauri::command] pub async fn list_stashes(state, repo_id: String) -> Result<Vec<StashEntry>, AppError>;
async fn list_stashes_inner(state, repo_id) { spawn_blocking(|| stash::list_stashes(&path)) }

// create_stash(repoId, message, includeUntracked) -> CreateStashResult
//                                                    errors: noRepo | operationInProgress | configMissing | git
#[tauri::command] pub async fn create_stash(state, repo_id: String, message: Option<String>, include_untracked: bool) -> Result<CreateStashResult, AppError>;
async fn create_stash_inner(...) { spawn_blocking(move || stash::create_stash(&path, message.as_deref(), include_untracked)) }

// apply_stash(repoId, index) -> ApplyStashOutcome    errors: noRepo | operationInProgress | git
#[tauri::command] pub async fn apply_stash(state, repo_id: String, index: usize) -> Result<ApplyStashOutcome, AppError>;
async fn apply_stash_inner(...) { spawn_blocking(move || stash::apply_stash(&path, index)) }

// pop_stash(repoId, index) -> ApplyStashOutcome      errors: noRepo | operationInProgress | git
#[tauri::command] pub async fn pop_stash(state, repo_id: String, index: usize) -> Result<ApplyStashOutcome, AppError>;
async fn pop_stash_inner(...) { spawn_blocking(move || stash::pop_stash(&path, index)) }

// drop_stash(repoId, index) -> ()                    errors: noRepo | git
#[tauri::command] pub async fn drop_stash(state, repo_id: String, index: usize) -> Result<(), AppError>;
async fn drop_stash_inner(...) { spawn_blocking(move || stash::drop_stash(&path, index)) }
```

`spawn_blocking` join errors map with `AppError::Other(format!("task join error: {e}"))` (verbatim as
the existing inners). Register all five in `src-tauri/src/lib.rs` `generate_handler!`
(`lib.rs:18-57`), appended after `commands::rebase_abort`:

```rust
        commands::rebase_abort,
        commands::list_stashes,
        commands::create_stash,
        commands::apply_stash,
        commands::pop_stash,
        commands::drop_stash
```

---

## 4. Wire types (TS mirror — `src/ipc/types.ts`)

Add the `stash` `RefKind` and grow the union at `types.ts:48`:

```ts
export type RefKind = 'localBranch' | 'remoteBranch' | 'tag' | 'head' | 'stash';
```

`RefLabel` (`types.ts:50`) is unchanged in shape; a stash pill is
`{ name: 'stash@{0}', kind: 'stash', isHead: false }`.

New types (place near `MergeOutcome`, `types.ts:271`):

```ts
export interface StashEntry {
  index: number;      // 0 == stash@{0}; SHIFTS after drop/pop — always refetch
  message: string;
  oid: string;        // stash commit oid
  baseOid: string;    // first-parent = base commit the pill attaches to
  ts: number;         // seconds since epoch (UTC)
}

export type ApplyStashOutcome =
  | { kind: 'applied' }
  | { kind: 'conflicts'; paths: string[] };

export interface CreateStashResult { created: boolean }
```

`IpcApi` additions (`types.ts:356`, mirror JSDoc style):

```ts
/** Stash stack, index 0 (most recent) first. Rejects noRepo | git. */
listStashes(repoId: string): Promise<StashEntry[]>;
/** Stash the dirty worktree. message=null → git default. Rejects
 *  operationInProgress | configMissing | git | noRepo. created:false == nothing to stash. */
createStash(repoId: string, message: string | null, includeUntracked: boolean): Promise<CreateStashResult>;
/** Apply stash `index` WITHOUT dropping. Rejects operationInProgress | git | noRepo. */
applyStash(repoId: string, index: number): Promise<ApplyStashOutcome>;
/** Apply + drop on clean success (retained on conflict). Rejects operationInProgress | git | noRepo. */
popStash(repoId: string, index: number): Promise<ApplyStashOutcome>;
/** Permanently discard stash `index` (UI confirms). Rejects git | noRepo. */
dropStash(repoId: string, index: number): Promise<void>;
```

`src/ipc/tauri.ts` (add beside the rebase wrappers, `tauri.ts:181`), snake_case command names +
camelCase arg keys (Tauri auto-converts, matching every existing wrapper):

```ts
listStashes(repoId) { return invoke('list_stashes', { repoId }); },
createStash(repoId, message, includeUntracked) { return invoke('create_stash', { repoId, message, includeUntracked }); },
applyStash(repoId, index) { return invoke('apply_stash', { repoId, index }); },
popStash(repoId, index) { return invoke('pop_stash', { repoId, index }); },
dropStash(repoId, index) { return invoke('drop_stash', { repoId, index }); },
```

---

## 5. Graph: attach stash pills to base commits (`src-tauri/src/graph.rs`)

Add `Stash` to `RefKind` (`graph.rs:25`) and give it the last pill rank:

```rust
pub enum RefKind { LocalBranch, RemoteBranch, Tag, Head, Stash }   // serde "stash"
// pill_rank (graph.rs:127): RefKind::Stash => 4   (after Tag=3)
```

`compute_graph` (`graph.rs:111`): change `let repo` → `let mut repo` and, **before** `collect_refs`,
collect stash bases (needs `&mut` for `stash_foreach`):

```rust
let stash_bases = collect_stash_bases(&mut repo)?;   // Vec<(usize index, git2::Oid base_oid)>
let (refs, tips, head_oid) = collect_refs(&repo)?;   // unchanged immutable borrow
if tips.is_empty() { return Ok(GraphLayout::empty()); }  // stashes never SEED the walk
layout_walk(&repo, &tips, refs, head_oid, &stash_bases)
```

```rust
/// O(stashes). Enumerate the stack; resolve each stash commit's FIRST parent
/// (= the base commit it was created from). Does NOT touch tips/seeds.
fn collect_stash_bases(repo: &mut git2::Repository) -> Result<Vec<(usize, git2::Oid)>, AppError> {
    let mut idxs: Vec<usize> = Vec::new();
    repo.stash_foreach(|index, _msg, _oid| { idxs.push(index); true })?;
    let reflog = match repo.reflog("refs/stash") {
        Ok(r) => r,
        Err(_) => return Ok(Vec::new()),   // no stash ref → nothing to attach
    };
    let mut out = Vec::with_capacity(idxs.len());
    for &index in &idxs {
        if let Some(entry) = reflog.get(index) {
            let stash_oid = entry.id_new();
            if let Ok(commit) = repo.find_commit(stash_oid) {
                if let Ok(base) = commit.parent_id(0) {
                    out.push((index, base));
                }
            }
        }
    }
    Ok(out)   // ascending by index (stash@{0} first)
}
```

`layout_walk` (`graph.rs:293`) — add the `stash_bases: &[(usize, git2::Oid)]` param and, in the
resolve pass (after step 6, `graph.rs:402`, once `index_of` is populated), append stash labels:

```rust
// Step 6.5: attach stash pills to base commits present in the walk.
// index_of maps every emitted commit oid → its row. Stash bases outside the
// loaded/truncated window are simply omitted (no pill). O(stashes).
for &(idx, base_oid) in stash_bases {
    if let Some(&row) = index_of.get(&base_oid) {
        nodes[row as usize].refs.push(RefLabel {
            name: format!("stash@{{{idx}}}"),
            kind: RefKind::Stash,
            is_head: false,
        });
    }
}
```

Ordering: `stash_bases` is ascending by index, and `Stash` is the highest `pill_rank`, so appending
after the already-sorted branch/tag/head labels keeps a valid pill order **without re-sorting** —
and multiple stashes on one base append as `stash@{0}, stash@{1}, …` in stack order. This is the
"multiple stashes on the same base commit → multiple pills" case; the P7 ref-column budget collapses
overflow to a `+n` chip automatically (`draw.ts:449`).

Invariants preserved: tips/seeds and the revwalk are unchanged; work is O(stashes) with no re-walk;
`determinism` (`graph.rs:812`) still holds; the 20k perf gate is unaffected (a handful of reflog +
`find_commit` lookups). A stash whose base is not an ancestor of any branch/remote/tag tip (orphaned)
is simply not in `index_of` → **pill omitted** (correct, no error).

---

## 6. Frontend

### 6.1 Graph pill rendering (`src/graph/draw.ts`)

Add a `stash` entity kind to `RefEntity` (`draw.ts:249`), NOT collapsed with branches (it is its own
entity, like `tag`/`head`):

```ts
export type RefEntity =
  | { kind: 'branch'; /* …unchanged… */ }
  | { kind: 'tag'; name: string; ref: RefLabel }
  | { kind: 'head'; name: string; ref: RefLabel }
  | { kind: 'stash'; name: string; ref: RefLabel };   // name = "stash@{n}"
```

`groupRefs` (`draw.ts:265`): add a `case 'stash':` that pushes to a local `stashes: RefEntity[]` and
return order `[...heads, ...branches.values(), ...tags, ...stashes]` (stashes last — matches the Rust
pill rank). Stash entities never merge with branches.

`entityStyle` (`draw.ts:309`): add a `case 'stash':` returning a distinct, theme-aware pill —
recommend a muted violet: add sibling constants next to `TAG_BG` / `TAG_COLOR` in `draw.ts`
(`STASH_BG`, `STASH_COLOR`) and return `{ fill: STASH_BG, text: STASH_COLOR, border: STASH_COLOR,
label: e.name }` (label already `stash@{n}`; the glyph is the drawn icon below, not a text prefix).

Stash **icon** — extend the icon record so the pill carries a distinct glyph (the direction's "stash
icon"):
- `LaidRefLabel.icons` (`draw.ts:387`) becomes `{ laptop: boolean; cloud: boolean; stash: boolean }`.
- `iconsFor` (`draw.ts:392`): `if (e.kind === 'stash') return { laptop:false, cloud:false, stash:true };`
  branch case adds `stash:false`; tag/head return all-false.
- `iconsWidth` (`draw.ts:398`): add `(icons.stash ? METRICS.iconSize : 0)` (a stash pill has exactly
  one icon, so no inter-icon gap term is needed for it).
- `drawStashIcon(ctx, bx, by, S)`: monochrome (caller sets `ctx.strokeStyle = style.text`), same
  convention as `drawLaptopIcon` (`draw.ts:333`). Draw a small **tray/box**: a rounded rect
  `[bx+0.1S, by+0.32S, 0.8S, 0.5S]` plus a short horizontal "slot" line across its upper third —
  reads as a drawer/stash. (Exact geometry at senior-dev's discretion; must fit the `S×S` box.)
- `drawRefLabelAt` (`draw.ts:484`) and the width path draw/measure the stash icon in the icon slot
  when `icons.stash`, exactly parallel to laptop/cloud.

Add `METRICS` — no new metric needed (reuse `iconSize`/`iconGap`, `metrics.ts:40`).

Stash pills are **display-only**: they carry no context menu (§6.4). In `GraphCanvas.tsx`
`targetRefOf` (`draw`/`GraphCanvas.tsx:98`) a stash entity returns its own `ref`, whose
`branchMenuItems` resolves to `[]` — so no menu opens (identical to tag/head today).

### 6.2 Sidebar "Stashes" section (`src/components/Sidebar.tsx`)

Add after the Tags section (`Sidebar.tsx:413-437`), matching the existing section styling
(`SectionHeader`, `branch-list`, `branch-row`, `branch-muted`). New props on `SidebarProps`:

```ts
stashes: StashEntry[];
onCreateStash(): void;                 // "Stash changes" action
onStashContextMenu(index: number, clientX: number, clientY: number): void;
```

- **Header**: `SectionHeader label="Stashes"` with an `extra` button "Stash changes" (title +
  aria-label; a box/`⊟`-style glyph or `+`), disabled when `busy || opActive`, calling `onCreateStash`.
  (Enabled even when `data.head.unborn`? No — stashing needs commits; hide/disable when unborn.)
- **Empty state**: `<p className="branch-muted">No stashes</p>`.
- **Row** (`StashRow`, sibling of `TagRow`): a `branch-row` with a stash glyph, the label
  `stash@{index}` (mono), the message (truncated, `branch-name-muted`, `title={message}` for the full
  text), and a right-aligned relative age from `ts` (reuse the graph's `relativeDate` helper or a
  small local formatter). `onContextMenu` → `onStashContextMenu(index, e.clientX, e.clientY)`
  (`e.preventDefault()`), exactly like `BranchRow` (`Sidebar.tsx:120`). Collapsible via local
  `stashesCollapsed` state (mirror `tagsCollapsed`, `Sidebar.tsx:203`).

Stashes are a flat list (no tree grouping — indices, not paths), so the `listView` tree branch does
not apply.

### 6.3 RepoWorkspace handlers (`src/components/RepoWorkspace.tsx`)

State + fetching (mirror the branches wiring): add `stashes: StashEntry[]` state and
`refetchStashes()` (`ipc.listStashes(repoId)`), included in `refreshAll` and in the `repo-changed` /
window-focus refresh batch. Pass `stashes`, `onCreateStash`, `onStashContextMenu` into `<Sidebar>`
(`RepoWorkspace.tsx:1290` area).

Handlers (mirror `handleMergeBranch` `:775` and `handleDeleteBranch` `:653` — `setMutating(true)` /
`try` / toast / refresh / `finally`):

```ts
async function handleCreateStash() {
  setMutating(true);
  try {
    const res = await ipc.createStash(repoId, null, /* includeUntracked */ true);
    pushToast(res.created ? 'success' : 'info',
      res.created ? 'Changes stashed' : 'Nothing to stash — working tree is clean');
    await refreshAll();                     // status + graph (pills) + stashes
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}

async function handleApplyStash(index: number) {
  setMutating(true);
  try {
    const res = await ipc.applyStash(repoId, index);
    if (res.kind === 'applied') pushToast('success', `Applied stash@{${index}}`);
    else pushToast('info',
      `Stash applied with ${res.paths.length} conflict(s) to resolve — the stash is kept (stash@{${index}}).`);
    await refreshAll();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}

async function handlePopStash(index: number) {
  setMutating(true);
  try {
    const res = await ipc.popStash(repoId, index);
    if (res.kind === 'applied') pushToast('success', `Popped stash@{${index}}`);
    else pushToast('error',
      `Pop hit ${res.paths.length} conflict(s); your changes are still on the stash (stash@{${index}}). ` +
      'Resolve the conflicts, then drop it.');
    await refreshAll();
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}

async function handleDropStash(index: number) {   // called after ConfirmDialog
  setMutating(true);
  try {
    await ipc.dropStash(repoId, index);
    pushToast('success', `Dropped stash@{${index}}`);
    await Promise.all([refetchStashes(), refetchGraph()]);   // pills change; worktree does not
  } catch (e) { pushToast('error', errorMessage(e)); }
  finally { setMutating(false); }
}
```

`refreshAll` is used for create/apply/pop because they change worktree + status + graph pills.
`handleDropStash` only refetches stashes + graph (Drop never touches the worktree). After any
create/pop/drop the refetched list re-indexes — honoring §2.4.

### 6.4 Stash context menu (shared `ContextMenu`)

Add `stashMenuItems(index)` beside `branchMenuItems` (`RepoWorkspace.tsx:1003`), returning
`ContextMenuItem[]`:

```ts
function stashMenuItems(index: number): ContextMenuItem[] {
  const gate = mutating || opActive;   // Apply/Pop need a clean, idle repo
  return [
    { label: 'Apply', disabled: gate, onSelect: () => void handleApplyStash(index) },
    { label: 'Pop',   disabled: gate, onSelect: () => void handlePopStash(index) },
    { label: 'Drop',  disabled: mutating, onSelect: () => setPendingDropStash(index) },  // Drop allowed mid-op
  ];
}
```

`onStashContextMenu(index, x, y)` → `setMenu({ x, y, items: stashMenuItems(index) })` (reuses the
existing single `<ContextMenu>` at `RepoWorkspace.tsx:1466`).

**Drop confirm**: add `pendingDropStash: number | null` state and a fourth `<ConfirmDialog>` beside
the branch-delete one (`RepoWorkspace.tsx:1428`):

```tsx
<ConfirmDialog
  open={pendingDropStash !== null}
  title="Drop stash"
  confirmLabel="Drop stash"
  busy={mutating}
  onConfirm={() => { const i = pendingDropStash; setPendingDropStash(null); if (i !== null) void handleDropStash(i); }}
  onCancel={() => setPendingDropStash(null)}>
  <div>Drop <span className="mono">stash@{`{${pendingDropStash ?? 0}}`}</span>?</div>
  <div className="dialog-body-note">This permanently discards the stashed changes and cannot be undone.</div>
</ConfirmDialog>
```

### 6.5 Mock IPC (`src/ipc/mock.ts`) — keep the browser harness whole

- Add `stashes: StashEntry[]` to `MockRepoState` (`mock.ts:117`). Seed the default repo with 2–3
  entries whose `baseOid` matches a **visible** default-fixture node id so a pill renders (see §6.6),
  e.g. `{ index:0, message:'WIP on main: polish sidebar', oid:randomOid(), baseOid:oid(3), ts:now-3600 }`,
  and a **second + third** stash sharing `baseOid:oid(6)` (`index:1`,`index:2`) to exercise the
  multi-stash / `+n` path. Non-default kinds (detached/unborn/20k) seed `stashes: []`.
- Command methods (add beside `mergeBranch`, `mock.ts:1013`):
  - `listStashes` → `structuredClone(state.stashes)`.
  - `createStash(repoId, _msg, _incl)` → if `state.status` is empty (clean) return `{created:false}`;
    else `unshift` a new `stash@{0}` entry, **re-index** the rest (`+1`), clear the mock dirty status,
    return `{created:true}`.
  - `applyStash(index)` → `{kind:'applied'}`; a demo conflict trigger (path/message contains
    `"conflict"`, mirroring the P8 `mergeBranch` convention `mock.ts:1013`) →
    `{kind:'conflicts', paths:['src/app.ts']}`. List unchanged.
  - `popStash(index)` → on success `{kind:'applied'}` and **remove** the entry + re-index; conflict
    trigger → `{kind:'conflicts', …}` and **keep** the entry.
  - `dropStash(index)` → remove entry at `index` + re-index remaining; resolve `void`.
- `tauri.ts` gets the five wrappers (§4). No new events/channels, so `onRepoChanged` etc. unchanged.

### 6.6 Graph fixture pill (`src/ipc/fixtures/graph.ts`)

In `buildMockGraph` (`fixtures/graph.ts:21`), add stash `RefLabel`s so the harness renders pills
without a backend: give row 3 (`push('core work 4', …)`, `graph.ts:67`) a
`{ name:'stash@{0}', kind:'stash', isHead:false }`, and give one node (e.g. row 6) TWO stash labels
(`stash@{1}`, `stash@{2}`) to prove multi-stash + `+n` collapse. Keep `baseOid`s in the mock
`stashes` array (§6.5) consistent with these node ids (`oid(3)`, `oid(6)`) so the Sidebar list and the
graph pills tell the same story. (The `20k`/`detached` fixtures get no stash labels.)

---

## 7. Sub-increments (each a single fresh-context senior-dev pass)

### P9a — Rust: `stash.rs` + commands + registration + tests
- New `git/stash.rs` (§2): `StashEntry`, `ApplyStashOutcome`, `CreateStashResult`, and the five
  functions with exact git2 calls. `git/mod.rs` `pub mod stash;`.
- `commands.rs`: five `#[tauri::command]` + `_inner` pairs (§3); `lib.rs` registration.
- Rust unit tests in `stash.rs` `#[cfg(test)]` (§8).
- **Acceptance**: `cargo check`/`clippy` clean; the §8 test matrix passes; wire-shape test asserts
  camelCase (`{"kind":"applied"}`, `{"kind":"conflicts","paths":[…]}`, StashEntry `baseOid`/`ts`).
  No frontend change required to compile.

### P9b — Graph: stash-ref attachment + canvas pill + fixtures + self-test
- `graph.rs`: `RefKind::Stash` + `pill_rank` + `collect_stash_bases` + `layout_walk` step 6.5 (§5).
- `types.ts`: `RefKind` gains `'stash'`.
- `draw.ts`: `RefEntity` stash kind, `groupRefs`/`entityStyle`/`iconsFor`/`iconsWidth`/`drawStashIcon`/
  `drawRefLabelAt`, `STASH_BG`/`STASH_COLOR` (§6.1); `GraphCanvas.tsx` stash → no-menu path.
- `fixtures/graph.ts`: stash pills on rows 3 and 6 (§6.6).
- `GraphCanvas.tsx` `p7SelfTest` (`GraphCanvas.tsx:447`): add checks that
  `groupRefs([{name:'stash@{0}',kind:'stash',isHead:false},{name:'main',kind:'localBranch',isHead:false}])`
  yields **two** entities with the branch NOT absorbing the stash, and that the stash entity's kind is
  `'stash'` and sorts **after** the branch.
- **Acceptance**: `pnpm build`/`tsc` clean; a Rust graph test (§8) attaches a stash pill to the correct
  base row and omits an orphaned stash; harness screenshot shows the violet `stash@{n}` pill (with the
  drawer icon) in the left ref column on the base commit, and the `+n` collapse on the multi-stash row;
  `window.__bonsai.p7SelfTest()` reports `fail:0`.

### P9c — Sidebar section + handlers + confirm + mock commands
- `Sidebar.tsx`: Stashes section, `StashRow`, empty state, "Stash changes" action, new props (§6.2).
- `RepoWorkspace.tsx`: `stashes` state + `refetchStashes` (into `refreshAll` + repo-changed batch),
  the four handlers, `stashMenuItems`, `pendingDropStash` + the Drop `ConfirmDialog`, Sidebar wiring
  (§6.3–§6.4).
- `types.ts` `IpcApi` + `StashEntry`/`ApplyStashOutcome`/`CreateStashResult`; `tauri.ts` wrappers;
  `mock.ts` state seed + five command methods (§4, §6.5).
- **Acceptance**: `pnpm build`/`tsc` clean; harness shows the Stashes section listing the seeded
  stashes (index, message, age); right-click → Apply/Pop/Drop menu opens; Drop shows the confirm
  dialog; "Stash changes" toasts; apply/pop conflict triggers toast correctly.

---

## 8. Acceptance criteria & AI gate

**Env mandates (tester):** set `TMP`/`TEMP` → `D:\Temp`; run `cargo test` and `clippy`
**sequentially** (target-dir race); use `crate::testutil::scratch_dir()` (tempfile) + runtime-free
inner fns (avoid the Tauri "test" feature).

**Rust test matrix (P9a — `stash.rs` `#[cfg(test)]`; assert outcome + on-disk state via a scratch
repo, echoing the `merge.rs` fixtures `merge.rs:407`):**
1. **Round-trip** — commit a base; edit a tracked file; `create_stash(None,false)` →
   `created:true`; worktree clean; `list_stashes` len 1 with correct `base_oid`(==HEAD) + non-empty
   message; `apply_stash(0)` → `Applied`; file edit is back; list still len 1.
2. **Pop drops** — as (1) but `pop_stash(0)` → `Applied`; edit back; `list_stashes` len 0.
3. **Nothing to stash** — clean tree → `create_stash` → `created:false`; list len 0.
4. **Include untracked** — an untracked file present; `create_stash(None,true)` removes it from the
   worktree; `pop_stash(0)` restores it.
5. **Pop conflict retains** — base has file X; stash an edit to X; commit a different change to X;
   `pop_stash(0)` → `Conflicts{paths:["X"]}`; `repo.state()==Clean`; X has `<<<<<<<` markers;
   `list_stashes` len 1 (retained).
6. **Drop** — create two stashes; `drop_stash(0)`; `list_stashes` len 1 and the **surviving entry
   re-indexed to 0** (index-shift, §2.4).
7. **Op-state guard** — put the repo in `Merge` (reuse a P3c/P8 conflict fixture); `create_stash` /
   `apply_stash` / `pop_stash` → `Err(operationInProgress)`; `drop_stash` still succeeds.
8. **Wire shapes** — `serde_json` asserts `{"kind":"applied"}`, `{"kind":"conflicts","paths":[…]}`,
   and `StashEntry` → `{index,message,oid,baseOid,ts}`.

**Rust graph test (P9b — `graph.rs` `#[cfg(test)]`):** build a repo, create a stash on a commit that
is a branch ancestor → its base node's `refs` contains one `RefKind::Stash` label `stash@{0}` (after
any branch/tag labels); a stash on an **orphaned** commit (base not reachable from any tip) → **no**
pill anywhere; two stashes on one base → two labels `stash@{0}`,`stash@{1}` in order. Determinism
(`graph.rs:812`) still holds.

**Browser-harness (orchestrator-verifiable):**
- `pnpm build` + `tsc` clean.
- Graph pane: the seeded default fixture shows a violet `stash@{n}` pill (drawer icon) on the base
  commit, and the multi-stash row collapses to a `+n` chip — screenshot evidence.
- Sidebar: the **Stashes** section lists the seeded stashes; right-click opens Apply/Pop/Drop; Drop
  opens the confirm dialog; "Stash changes" and the apply/pop(-conflict) toasts fire from the mock.
- `window.__bonsai.p7SelfTest()` → `fail:0` including the new stash-not-collapsed checks.

**USER CHECKPOINT (native `pnpm tauri dev`, real repo):** with real uncommitted changes, "Stash
changes" clears the worktree and a `stash@{0}` pill appears on HEAD; Apply/Pop restore the changes;
Pop-with-conflict leaves markers and keeps the stash (findable via `git stash list`); Drop (after
confirm) removes it; the sidebar + graph stay in sync after each op.

---

## 9. File touch list

- `src-tauri/src/git/stash.rs` (**new**), `src-tauri/src/git/mod.rs` (`pub mod stash;`).
- `src-tauri/src/commands.rs` (imports + 5 command/_inner pairs), `src-tauri/src/lib.rs` (register 5).
- `src-tauri/src/graph.rs` (`RefKind::Stash`, `pill_rank`, `collect_stash_bases`, `layout_walk` param
  + step 6.5, `compute_graph` `let mut repo`).
- `src/ipc/types.ts` (`RefKind` + `StashEntry`/`ApplyStashOutcome`/`CreateStashResult` + `IpcApi`).
- `src/ipc/tauri.ts` (5 wrappers), `src/ipc/mock.ts` (state seed + 5 methods),
  `src/ipc/fixtures/graph.ts` (stash pills).
- `src/graph/draw.ts` (RefEntity + style + icon + `STASH_BG`/`STASH_COLOR`), `src/graph/GraphCanvas.tsx`
  (no-menu path + `p7SelfTest`).
- `src/components/Sidebar.tsx` (Stashes section + props), `src/components/RepoWorkspace.tsx`
  (state, handlers, menu, Drop confirm, Sidebar wiring).
- No new `AppError` variant; no new events/channels; `notify` watcher unchanged (worktree changes
  already fire `repo-changed`, which triggers the refresh batch that now includes `refetchStashes`).
```