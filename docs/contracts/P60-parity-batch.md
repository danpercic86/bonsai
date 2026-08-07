# P60 — Parity batch: branch rename · non-FF pull · one-click undo · submodule add/deinit/remove

Four small table-stakes items, each an independent sub-increment. Roadmap P60 (Phase 3 — Correctness
& parity). Every item **maximises reuse of already-shipped primitives** — the only genuinely new git
logic is `rename_branch` (thin git2 wrapper), a READ-ONLY undo classifier, and three submodule ops
(add via git2, deinit/remove via shell-out).

**Command count: 129 → 134** (`rename_branch`, `describe_last_undo`, `add_submodule`,
`deinit_submodule`, `remove_submodule`). Non-FF pull adds **no** command (reuses `merge_branch` /
`rebase_branch`; adds one field to `PullResult`).

References read (verified, not guessed):
`crates/bonsai-core/src/git/branches.rs` (`BranchInfo`, `validate_branch_name`, `create_branch`,
`checkout_branch`, `delete_branch`; git2 0.21 `Branch` API), `git/remote.rs`
(`pull_ff` §432-523: fetch-then-FF, `PullResult` enum), `git/merge.rs` (`merge_branch(workdir,name)`
→ `MergeOutcome`, accepts remote-tracking shorthand, own autostash), `git/rebase.rs`
(`rebase_branch(workdir,onto_name)` → `RebaseOutcome`, same shorthand + autostash), `git/reflog.rs`
(`read_reflog` READ-ONLY, `ReflogEntry{index,oldOid,newOid,message,...}`, index 0 = newest),
`git/reset.rs` (`reset_branch(workdir,target_oid,ResetMode)`, `ResetMode = soft|mixed|hard`),
`git/submodule.rs` (`SubmoduleInfo`, `list/init/update/sync`, `acquire_cred` credential reuse),
`git/search.rs` (`GitRunner`/`SpawnGitRunner` injected-runner idiom — reused for shell-out ops),
`src-tauri/src/commands/{branches,submodules,merge,diff}.rs` (command triple
`X → X_inner → repo_path → spawn_blocking`), `commands/remotes.rs` (pull command),
`src-tauri/src/lib.rs` (`generate_handler!`, 129 cmds), `src/ipc/types.ts` (`PullResult`,
`ResetMode`, `BranchInfo`, `SubmoduleInfo`), `error.rs` (kinds: invalidName, branchExists,
branchNotFound, checkoutConflict, operationInProgress, noUpstream, noRemote, git, noRepo).
House format: `docs/contracts/{P38-reflog,M6-remotes,P50-search-command-palette}.md`.

---

## 0. Key decisions (with rationale)

**D1 — Non-FF pull adds ZERO new git logic; it reuses `merge_branch` / `rebase_branch`.** The FF-only
`pull` stays the default happy path and is unchanged except that its `WouldNotFastForward` result now
also carries the resolved `upstream` shorthand (e.g. `"origin/main"`). On that result the frontend
opens a confirm dialog offering **Merge** / **Rebase** / **Cancel**; Merge → the existing
`merge_branch(repoId, upstream)`, Rebase → the existing `rebase_branch(repoId, upstream)`. Those
commands already own autostash, conflict recording, op-state, and their full conflict UX — so non-FF
pull is (a) one Rust field + (b) a frontend dialog. Rejected alternative: a monolithic
`pull(strategy)` backend that internally branches merge/rebase — it would duplicate every
`MergeOutcome`/`RebaseOutcome` variant into a new `PullOutcome` union and re-own conflict paths for no
gain. The fetch has already happened during the FF attempt, so the remote-tracking ref the reused
commands resolve is current.

**D2 — One-click undo = READ-ONLY classifier + reuse `reset_branch`** (mirrors P38's "read-only
`reflog.rs` + reuse existing mutation" invariant). New `describe_last_undo` reads HEAD reflog[0],
classifies the last op, and returns an `UndoPlan` naming the target oid + reset mode + safety flags.
Execution is the ALREADY-SHIPPED `reset_branch(repoId, targetOid, mode)` behind an explicit confirm
dialog. No new mutation primitive.

**D3 — Undo safety is classified per op-class, never "just hard-reset".** Two reversal shapes:
- **Ref-restore-only (mixed reset, worktree untouched):** `commit`, `commit (amend)`, `reset` →
  target = reflog `oldOid`, mode `mixed`. Safe even with a dirty worktree (mixed does not touch files).
- **Full-revert (hard reset, worktree restored):** `merge`, `rebase (finish)`, FF `pull`/`merge`,
  `cherry-pick`, `revert` → target = `oldOid`, mode `hard`. **Requires a clean worktree** (else new
  uncommitted work would be clobbered) — `describe_last_undo` reports `worktreeDirty` and the UI
  refuses/​warns.
- **Not undoable in v1:** branch switch (`checkout: moving from…` — not a tip move; different
  mechanism, no data-loss risk), initial-commit undo (target would be the 40-zero oid → HEAD unborn,
  which `reset_branch` cannot express), and any unrecognized message. `undoable=false` + a reason.
  (OQ1: optionally support branch-switch undo via `checkout_branch`.)

**D4 — Submodule add via git2; deinit + remove via shell-out.** libgit2 supports submodule *add*
well (`Repository::submodule` → clone → `add_finalize`, credentials via the existing `acquire_cred`
chain, uniform with `update_submodule`). libgit2 has **no** deinit/remove primitive, and hand-rolling
config+worktree surgery is dangerous; the `git` binary is already a hard dependency (clone / fetch /
credentials / search) and is authoritative for submodule teardown. So `deinit_submodule` /
`remove_submodule` shell out via the `search.rs` `GitRunner`/`SpawnGitRunner` idiom (testable,
injection-safe: path after `--`). (OQ2: do `add` via shell-out too, for uniformity.)

**D5 — No new `AppError` variants.** rename → invalidName | branchExists | branchNotFound | git;
submodule ops → invalidName | git (+ shell stderr tail); undo classifier → git | noRepo. All fit
existing kinds.

**D6 — repo-changed / refresh: follow the branches/merge command precedent — mutations do NOT emit
`repo-changed`; the frontend refetches imperatively** (rename, submodule add/deinit/remove). Undo's
`reset_branch` inherits its existing refresh. Read-only `describe_last_undo` emits nothing.

---

## P60a — Branch rename

### Module boundaries
- `crates/bonsai-core/src/git/branches.rs` — add `rename_branch` (beside `create_branch` /
  `delete_branch`) + in-module tests.
- `src-tauri/src/commands/branches.rs` — `rename_branch` command + `_inner`.
- `src-tauri/src/lib.rs` — register (after `delete_branch`).
- `src/ipc/{types.ts, tauri.ts}` + `src/ipc/mock/handlers/branches.ts` (or wherever branch mocks
  live) — wire + mock.
- Frontend: a **"Rename…"** item in `branchMenuItems` (`src/components/workspaceMenus.ts`) →
  the shared `PromptDialog` prefilled with the current name (reuse the create-branch prompt idiom).

### Rust
```rust
/// Result of `rename_branch`. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameBranchResult {
    /// true when the renamed branch was the checked-out branch (HEAD followed the
    /// rename — libgit2 rewrites the HEAD symref). Tells the frontend to refetch
    /// HEAD/status, not just the branch list.
    pub was_head: bool,
    /// The upstream shorthand still configured after the rename (e.g. "origin/main"),
    /// or None. libgit2 renames the `branch.<name>.*` config section, so tracking
    /// is PRESERVED; surfaced so the UI can confirm it in a toast.
    pub upstream: Option<String>,
}

/// Blocking. Renames LOCAL branch `old_name` → `new_name` (git `branch -m`,
/// non-force). Validates `new_name` (reuse `validate_branch_name`); resolves
/// `old_name` (NotFound → BranchNotFound); refuses if `new_name` already exists
/// (git2 `Branch::rename(.., force=false)` → ErrorCode::Exists → BranchExists).
/// libgit2 moves the ref, its reflog, and the `branch.<name>.*` config section,
/// and rewrites HEAD when `old_name` is checked out — so upstream/tracking survive
/// and no manual config surgery is needed.
///
/// Errors: `invalidName` | `branchNotFound` | `branchExists` | `git` | `noRepo`.
pub fn rename_branch(workdir: &Path, old_name: &str, new_name: &str)
    -> Result<RenameBranchResult, AppError>;
```
Algorithm (normative): `validate_branch_name(new_name)?` → open repo → `was_head =
repo.find_branch(old,Local)?.is_head()` → capture `upstream` shorthand via `.upstream().ok()` →
`branch.rename(new_name, false)` mapping `Exists → BranchExists`, `NotFound → BranchNotFound` →
re-read upstream from the renamed branch → `Ok(RenameBranchResult{ was_head, upstream })`.

### Command
```rust
#[tauri::command]
pub async fn rename_branch(
    state: tauri::State<'_, AppState>, repo_id: String, old_name: String, new_name: String,
) -> Result<RenameBranchResult, AppError>;
// _inner: repo_path → spawn_blocking(branches::rename_branch). Does NOT emit repo-changed.
```

### TypeScript
```ts
export interface RenameBranchResult { wasHead: boolean; upstream: string | null; }
/** Rename a local branch (git branch -m). Preserves upstream + reflog. Rejects
 *  invalidName | branchNotFound | branchExists | git | noRepo. */
renameBranch(repoId: string, oldName: string, newName: string): Promise<RenameBranchResult>;
```
`tauri.ts`: `renameBranch: (repoId, oldName, newName) => invoke('rename_branch', { repoId, oldName, newName })`.

### Mock
`renameBranch(repoId, oldName, newName)`: `requireRepo`; reject `{kind:'invalidName'}` on blank/`-`
leading; reject `{kind:'branchExists'}` if `newName` already in `mockBranches.local`; reject
`{kind:'branchNotFound'}` if `oldName` absent. Else rename the entry in `mockBranches.local`
(preserve its `upstream`/`ahead`/`behind`/`tip`), update `mockHeadBranch` if it equalled `oldName`,
and return `{ wasHead: oldName===mockHeadBranch(before), upstream: entry.upstream }`.

### Acceptance
1. `cargo test -p bonsai-core branches` green incl. new tests: rename moves the ref; **upstream
   survives** (`branch.<new>.remote`/`.merge` present via a git-config or `Branch::upstream()`
   assertion); renaming the checked-out branch sets `wasHead=true` and HEAD now resolves to
   `refs/heads/<new>`; new-name-exists → `BranchExists`; unknown old → `BranchNotFound`; invalid new →
   `InvalidName`. CLI-oracle: state after `rename_branch` matches `git branch -m old new`
   (`git rev-parse`, `git config --get branch.new.remote`).
2. `cargo clippy -- -D warnings` clean; `generate_handler!` = 130; `tsc`/`pnpm build` clean.
3. Harness: branch context-menu **Rename…** opens the prefilled PromptDialog; renaming a branch with
   ≥6 rows updates the sidebar; renaming the current branch refreshes the HEAD pill.

---

## P60b — Non-fast-forward pull (offer Merge or Rebase)

### Module boundaries
- `crates/bonsai-core/src/git/remote.rs` — add one field to `PullResult::WouldNotFastForward`.
- `src/ipc/types.ts` — mirror the field.
- `src/ipc/mock/handlers/remote.ts` (or the remote mock) — include `upstream` in the
  `wouldNotFastForward` result.
- Frontend: a new confirm dialog + wiring in the pull handler (`RepoWorkspace` / `WorkspaceDialogs`).
  **No new command** — Merge/Rebase reuse the existing `merge_branch` / `rebase_branch` handlers.

### Rust (only change)
```rust
// remote.rs — WouldNotFastForward gains `upstream`:
WouldNotFastForward {
    branch: String,
    ahead: u32,
    behind: u32,
    /// Upstream tracking shorthand ("origin/main") resolved AFTER the fetch —
    /// the exact `name` the frontend passes to merge_branch/rebase_branch.
    upstream: String,
},
```
`pull_ff` §491-498 already has `branch`, `ahead`, `behind` in scope; add
`upstream: format!("{remote_name}/{}", <upstream short branch>)` — derive from the re-resolved
`branch.upstream()?.name()` (already fetched at §477) rather than re-computing. FF-only remains the
sole thing `pull` does; nothing merges/rebases inside the backend.

### TypeScript
```ts
// PullResult union — extend the one member:
| { kind: 'wouldNotFastForward'; branch: string; ahead: number; behind: number; upstream: string };
```

### Frontend flow (confirm-gated)
1. `handlePull()` → `ipc.pull()`. On `wouldNotFastForward`, instead of only toasting, open
   `NonFfPullDialog` (new small presentational file, e.g.
   `src/components/dialogs/NonFfPullDialog.tsx`):
   > **"'{branch}' has diverged from '{upstream}'"** — {ahead} local / {behind} upstream commit(s).
   > Fast-forward isn't possible. Reconcile by: **[Merge]** **[Rebase]** **[Cancel]**.
   Copy each option's effect ("Merge creates a merge commit"; "Rebase replays your {ahead} commit(s)
   on top — rewrites local history"). This dialog **is** the confirm gate for the (history-changing)
   operation.
2. **Merge** → `ipc.mergeBranch(repoId, upstream)`; **Rebase** → `ipc.rebaseBranch(repoId, upstream)`.
   Route the returned `MergeOutcome`/`RebaseOutcome` through the EXISTING result handlers (conflict
   overlay, op-state banner, toasts) — no new handling. Then the standard post-pull full refresh.
3. Other `PullResult` variants (`upToDate`, `fastForwarded`) are unchanged.

### Mock
`pull()` `wouldNotFastForward` branch (e.g. `feature/sidebar` ahead 2 / behind 1): add
`upstream: entry.upstream ?? 'origin/'+branch`. The follow-up Merge/Rebase reuse the existing
`mergeBranch`/`rebaseBranch` mocks (already stateful — no change). Add a mock seam so the harness can
exercise a rebase-conflict path (e.g. `?remote=rebaseconflict` → `rebaseBranch` returns
`{kind:'conflicts',...}`) if not already reachable.

### Acceptance
1. `cargo test -p bonsai-core remote` green; `pull_ff` diverged case now returns `upstream` matching
   `git rev-parse --abbrev-ref @{u}`. `clippy` clean; `tsc`/`build` clean.
2. Harness: on `feature/sidebar`, Pull → the diverged dialog appears; **Merge** routes through the
   merge outcome path (up-to-date/merged/conflicts as the merge mock dictates); **Rebase** routes
   through the rebase outcome path; **Cancel** changes nothing. FF-able branches still fast-forward
   with no dialog.
3. Reviewer confirms the backend still performs **only** fetch+FF — no merge/rebase logic added to
   `remote.rs`; the merge/rebase run exclusively via the existing commands under an explicit confirm.

---

## P60c — One-click undo (undo last operation via reflog)

### Module boundaries
- `crates/bonsai-core/src/git/undo.rs` — **NEW, READ-ONLY**: `UndoKind`, `UndoPlan`,
  `describe_last_undo` (reads HEAD reflog[0] via the shipped `read_reflog` or `repo.reflog("HEAD")`;
  classifies; checks worktree dirtiness). ZERO mutation code (P38 invariant).
- `crates/bonsai-core/src/git/mod.rs` — `pub mod undo;`.
- `src-tauri/src/commands/undo.rs` (or fold into `history.rs`) — `describe_last_undo` command.
- `src-tauri/src/lib.rs` — register (after `read_reflog`).
- `src/ipc/{types.ts, tauri.ts}` + `src/ipc/mock/handlers/undo.ts`.
- Frontend: a prominent **Undo** toolbar button (`WorkspaceToolbar`) + a new
  `src/components/dialogs/UndoDialog.tsx` (confirm). Execution reuses the shipped `resetBranch`
  handler + its ConfirmDialog gating.

### Rust
```rust
/// Classified last-operation kind (drives the undo verb + reset mode). Wire: camelCase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UndoKind {
    Commit, Amend, Merge, Rebase, FastForward, CherryPick, Revert, Reset,
    BranchSwitch,   // not undoable in v1 (see reason); classified for a clear message
    Unknown,        // unrecognized reflog message
}

/// Plan for reversing the last HEAD-moving operation. Wire: camelCase.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoPlan {
    pub kind: UndoKind,
    /// Human summary from the reflog message, e.g. "commit: add feature".
    pub summary: String,
    /// Where undo would move the current branch (reflog oldOid). Full 40-hex.
    /// Empty string when there is nothing to undo / target is the 40-zero root.
    pub target_oid: String,
    /// short(target_oid) for the confirm copy; "" when target_oid is empty.
    pub target_short: String,
    /// Reset mode to reverse this op: mixed (ref-restore, worktree kept) for
    /// Commit/Amend/Reset; hard (full revert) for Merge/Rebase/FastForward/
    /// CherryPick/Revert. `None` when !undoable.
    pub reset_mode: Option<crate::git::reset::ResetMode>,
    /// true only for reset_mode==Hard classes — the frontend must refuse/warn
    /// while `worktree_dirty` (a hard reset would clobber new uncommitted work).
    pub requires_clean_worktree: bool,
    /// Current worktree dirtiness (staged/unstaged/untracked), for the UI gate.
    pub worktree_dirty: bool,
    /// Whether v1 can undo this op via `reset_branch`. false for BranchSwitch,
    /// Unknown, empty reflog, and initial-commit (target is the root/zero oid).
    pub undoable: bool,
    /// Why not, when `!undoable` (shown as a disabled-button tooltip). None when undoable.
    pub reason: Option<String>,
}

/// Blocking. READ-ONLY. Inspects HEAD reflog entry 0 and returns how to reverse it.
/// Never mutates. Empty reflog / unborn HEAD → UndoPlan{ undoable:false, kind:Unknown,
/// reason:"nothing to undo" }. Errors: `git` | `noRepo`.
pub fn describe_last_undo(workdir: &Path) -> Result<UndoPlan, AppError>;
```
Classification (normative — match the reflog-message PREFIX, first match wins; git writes these):
| reflog message starts with | UndoKind | reset_mode | requires_clean_wt |
|---|---|---|---|
| `commit: ` / `commit (initial): ` | Commit | Mixed | no |
| `commit (amend): ` | Amend | Mixed | no |
| `merge ` (and `pull:` that produced a merge) | Merge | Hard | yes |
| `rebase ` / `rebase (finish): ` / `rebase -i ` | Rebase | Hard | yes |
| `pull: Fast-forward` / `merge: Fast-forward` / `pull ` (ff) | FastForward | Hard | yes |
| `cherry-pick: ` | CherryPick | Hard | yes |
| `revert: ` | Revert | Hard | yes |
| `reset: ` | Reset | Mixed | no |
| `checkout: moving from ` | BranchSwitch | — | — (undoable=false, reason) |
| anything else | Unknown | — | — (undoable=false) |
Then: `target_oid = entry0.oldOid`; if `target_oid == "0"*40` → `undoable=false, reason="cannot undo
the initial commit"`. `worktree_dirty` via the shipped status/`autostash::is_dirty`. For a Hard class
with `worktree_dirty`, keep `undoable=true` but `requires_clean_worktree=true` (the UI blocks the
action and explains) — OR set `undoable=false` with a stash-first reason (OQ3: recommend the former —
show the plan, block the button, tell them to stash).

### Command
```rust
#[tauri::command]
pub async fn describe_last_undo(
    state: tauri::State<'_, AppState>, repo_id: String,
) -> Result<UndoPlan, AppError>;
// _inner: repo_path → spawn_blocking(undo::describe_last_undo). Read-only; no repo-changed.
```

### TypeScript
```ts
export type UndoKind =
  | 'commit' | 'amend' | 'merge' | 'rebase' | 'fastForward'
  | 'cherryPick' | 'revert' | 'reset' | 'branchSwitch' | 'unknown';
export interface UndoPlan {
  kind: UndoKind; summary: string; targetOid: string; targetShort: string;
  resetMode: ResetMode | null; requiresCleanWorktree: boolean;
  worktreeDirty: boolean; undoable: boolean; reason: string | null;
}
/** Describe how to reverse the last HEAD-moving op (read-only). Rejects git | noRepo. */
describeLastUndo(repoId: string): Promise<UndoPlan>;
```

### Frontend
- **Undo** button in `WorkspaceToolbar` (prominent, left group). On click → `describeLastUndo` → open
  `UndoDialog`:
  > **"Undo {kind}?"** — "{summary}". This will {mixed:"move your branch back to"/hard:"reset your
  > branch and working tree to"} **{targetShort}". [Undo] [Cancel]**.
  Disable **Undo** when `!undoable` (show `reason`) or when `requiresCleanWorktree && worktreeDirty`
  (show "Commit or stash your changes first"). For a Hard class the dialog carries the destructive
  styling already used by the reset ConfirmDialog.
- On confirm → `ipc.resetBranch(repoId, plan.targetOid, plan.resetMode!)` (the shipped handler +
  refresh). Reuse the existing reset error/toast handling verbatim.

### Mock
`describeLastUndo(repoId)`: `requireRepo`; read `MOCK_HEAD_REFLOG[0]` (the P38 fixture), classify with
the same prefix table, return a matching `UndoPlan` (`worktreeDirty` from the mock status,
`targetOid = entry0.oldOid`). Execution reuses the existing `resetBranch` mock (already updates the
mock graph/HEAD). Provide fixtures whose reflog[0] exercises a Commit (mixed) and a Merge (hard) case
(`?fixture=` or a small seam).

### Acceptance
1. `cargo test -p bonsai-core undo` green: classifier truth-table (each prefix → kind/mode) as pure
   unit tests over synthetic messages; a CLI-oracle test builds a scratch repo doing
   commit→merge→reset and asserts `describe_last_undo` picks the right kind/target for each latest op
   (target == `git rev-parse HEAD@{1}`). `undo.rs` contains no mutation calls (reviewer greps).
2. `clippy` clean; `generate_handler!` = 132 (after P60a); `tsc`/`build` clean.
3. Harness: after a mock commit, **Undo** → dialog "Undo commit? …move your branch back to <short>"
   → confirm → `resetBranch` mixed fires and the graph updates; a Merge fixture shows the hard-reset
   wording and blocks when the worktree is dirty; a BranchSwitch/empty reflog disables Undo with the
   reason.

---

## P60d — Submodule add / deinit / remove

### Module boundaries
- `crates/bonsai-core/src/git/submodule.rs` — add `add_submodule` (git2 + `acquire_cred`),
  `deinit_submodule` + `remove_submodule` (shell-out via injected `GitRunner`), + arg-builder units.
- `src-tauri/src/commands/submodules.rs` — three commands + `_inner`.
- `src-tauri/src/lib.rs` — register (after `sync_submodule`).
- `src/ipc/{types.ts, tauri.ts}` + the submodule mock handler.
- Frontend: three items on the submodule row menu / panel
  (`src/components/SubmodulePanel.tsx` or the submodule section) — **Add submodule…** (prompt: url +
  path), **Deinit** (confirm), **Remove** (destructive confirm). Reuse the shared Prompt/Confirm
  dialogs.

### Rust
```rust
/// Blocking. Adds a submodule at repo-relative `path` from `url`: git2
/// `Repository::submodule(url, Path::new(path), /*use_gitlink*/ true)` → clone the
/// subrepo with the shared M6 credential callback (`acquire_cred`, exactly as
/// `update_submodule`) → `Submodule::add_finalize()`. Writes .gitmodules + stages
/// the gitlink. `path` validated with `validate_rel_path`; blank url → InvalidName.
/// Errors: `invalidName` | `git` (incl. network/auth via `map_remote_err`) | `noRepo`.
pub fn add_submodule(workdir: &Path, url: &str, path: &str) -> Result<SubmoduleInfo, AppError>;

/// Blocking. `git submodule deinit -f -- <path>` via `runner` (no libgit2
/// primitive; D4). Clears `submodule.<name>` from .git/config and empties the
/// submodule worktree; KEEPS the .gitmodules entry (re-init-able). `name` resolved
/// to its path via `find_submodule`. Errors: `invalidName` | `git` (stderr tail) | `noRepo`.
pub fn deinit_submodule(workdir: &Path, runner: &dyn GitRunner, name: &str)
    -> Result<(), AppError>;

/// Blocking. Full removal (git's documented sequence): `git submodule deinit -f --
/// <path>` → `git rm -f -- <path>` (drops the gitlink + .gitmodules entry, stages
/// the removal) → best-effort `remove_dir_all(.git/modules/<name>)`. DESTRUCTIVE
/// (deletes the worktree, edits the index). Errors: `invalidName` | `git` | `noRepo`.
pub fn remove_submodule(workdir: &Path, runner: &dyn GitRunner, name: &str)
    -> Result<(), AppError>;

// pure, unit-testable argv builders (no git):
fn deinit_args(path: &str) -> Vec<String>;  // ["submodule","deinit","-f","--", path]
fn rm_args(path: &str)     -> Vec<String>;   // ["rm","-f","--", path]
```
Injection-safety: `path` is always the final token after `--` (never interpolated); reuse
`SpawnGitRunner` (`CREATE_NO_WINDOW`, `GIT_TERMINAL_PROMPT=0`, `current_dir(workdir)`, capture
stderr for the error tail).

### Commands
```rust
#[tauri::command] pub async fn add_submodule(
    state: tauri::State<'_, AppState>, repo_id: String, url: String, path: String,
) -> Result<SubmoduleInfo, AppError>;
#[tauri::command] pub async fn deinit_submodule(
    state: tauri::State<'_, AppState>, repo_id: String, name: String,
) -> Result<(), AppError>;
#[tauri::command] pub async fn remove_submodule(
    state: tauri::State<'_, AppState>, repo_id: String, name: String,
) -> Result<(), AppError>;
// _inner: repo_path → spawn_blocking(submodule::…). deinit/remove pass &SpawnGitRunner.
// None emit repo-changed; the frontend refetches submodules + status + graph.
```

### TypeScript
```ts
/** Add a submodule from `url` at repo-relative `path` (clones it). Rejects invalidName | git | noRepo. */
addSubmodule(repoId: string, url: string, path: string): Promise<SubmoduleInfo>;
/** Deinit (clear config + empty worktree; keep .gitmodules). Rejects invalidName | git | noRepo. */
deinitSubmodule(repoId: string, name: string): Promise<void>;
/** Remove entirely (deinit + git rm + drop .git/modules). DESTRUCTIVE. Rejects invalidName | git | noRepo. */
removeSubmodule(repoId: string, name: string): Promise<void>;
```

### Mock
- `addSubmodule`: reject `{kind:'invalidName'}` on blank url/path; push a new `SubmoduleInfo`
  (`status:'upToDate'`, derived `absPath`, `url`, oids from a random 40-hex) into the mock submodule
  list; return it.
- `deinitSubmodule`: flip the row's `status` to `'uninitialized'`, null its `wtOid`.
- `removeSubmodule`: drop the row from the mock list.
- All `requireRepo`; a `#fail`-in-name / `?submodule=fail` seam → `{kind:'git', message:'Mock: …'}`.

### Acceptance
1. `cargo test -p bonsai-core submodule` green: `deinit_args`/`rm_args` exact vecs (path after `--`,
   space/`;`-bearing path stays one token); CLI-oracle (guarded by `have_git()`): build a superproject
   with a local (`file://`) submodule, `add_submodule` → `.gitmodules` + staged gitlink match
   `git submodule add`; `deinit_submodule` → `git config --get submodule.<n>.url` gone, worktree
   empty, `.gitmodules` retained; `remove_submodule` → gitlink + `.gitmodules` entry gone, worktree
   deleted (parity with the real `git submodule deinit`/`git rm` sequence). Scratch under
   `D:\Temp\bonsai-scratch`; TMP/TEMP=`D:\Temp`; cargo/clippy sequential.
2. `clippy` clean; `generate_handler!` = 135; `tsc`/`build` clean.
3. Harness: **Add submodule…** prompt adds a row; **Deinit** flips it to uninitialized; **Remove**
   (destructive confirm) drops it; console clean.

---

## Sub-increment order
`P60a` rename → `P60b` non-FF pull → `P60c` undo → `P60d` submodules. All four are independent; this
order front-loads the two smallest (a, b) and matches the roadmap listing. Each is one fresh-context
senior-dev pass; commit after each reviewer approval.

## Open questions (flag to orchestrator)
- **OQ1 — Branch-switch undo.** v1 marks `checkout: moving from…` as `undoable:false`. Recommend
  keeping it out (no data loss; user can switch back) rather than parsing the old branch name out of
  the reflog message (fragile) to drive `checkout_branch`. Confirm, or ask to support switch-back.
- **OQ2 — Submodule `add` transport.** Recommend git2 (`submodule`+clone+`add_finalize`) so the
  credential chain matches `update_submodule`; the alternative is a `git submodule add` shell-out
  (more robust to libgit2 edge cases, but a second credential path). Confirm git2.
- **OQ3 — Hard-reset undo on a dirty worktree.** Recommend `undoable:true` +
  `requiresCleanWorktree:true` so the UI *shows* the plan but blocks the button with "stash first",
  rather than `undoable:false` (which hides what happened). Confirm. (An autostash-wrapped undo is
  possible but adds lossy-recovery surface — defer.)
- **OQ4 — Undo of `amend` loses the amended message.** A mixed reset to the pre-amend commit returns
  the changes to the worktree but discards the amended commit's message. Acceptable for v1 (it is what
  the user is undoing); the dialog copy should say so. Confirm the wording is enough.
