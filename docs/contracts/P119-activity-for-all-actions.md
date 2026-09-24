# P119 — every repo-changing action is logged in the git activity dock

Status: contract (architect, 2026-09-24; **rev 2** reconciles the user rulings of 2026-09-24 and
`P119-ui.md` §9). UI copy/visuals: `docs/contracts/P119-ui.md` (ui-designer). **Wherever this file
gives copy, it is a placeholder and P119-ui wins**, except where §10 lists a P119-ui line that must
change to match a user ruling.

Builds on: `archive/P87-git-observability.md` (event model), `P87b-FU1-run-target.md` (target
funnel, §6 guarantees), `P87b-FU1-FU4-git-dock-ui.md` (dock).

## 0. Scope

1. 56 new `GitActivityCategory` variants (63 total). Every repo-changing command runs inside the
   `with_activity` bracket → `started` / `finished` on the one stream.
2. **Wire additions, all additive and optional** (still 7 kinds; no new command/event/channel):
   - `finished.outcome?: GitRunOutcome` — `conflicts` | `fastForwarded` | `merged` | `upToDate` (§2.6).
   - `started.targetCount?: number` — present iff the op acts on ≥2 items; `target` is then absent (§2.7).
   - On a failed run, the user-facing error message is emitted as a final `stderrLine` before
     `finished` (§2.8). No new field.
3. Targets for arg-carried commands come from a new core funnel (`TargetArg`, §2.3). src-tauri still
   cannot construct an `ActivityTarget`. Sanitizing + capping stay structural; for the free-text
   `Name`/`PathLeaf` variants, "never a phrase" is backed by tests and review (§2.4).
4. The toolbar `header-progress` bar is deleted; the determinate fetch/pull fraction moves onto
   `.git-dock-progress`. The `netBusy` prop is deleted. The Refresh icon spins while `refreshing`.
5. Mock IPC emits the same events, including outcome, count and failure line.

User rulings applied: stage/unstage/partial + conflict-resolution writes are **not** logged; a run
paused on conflicts ends as `! Conflicts`; fast-forward is one Merge row with an outcome detail;
clone/init are logged only when a repo workspace is already open (`initRepo` kept); Undo shows as
the Reset row.

Non-goals: repo-id on events, recorder threading into core for the new ops (no hook/output lines
for them), batching per-item frontend loops into one run (§10-7), clone rows from the empty state.

## 1. Final category list

`Src` = target source: **A** = arg-carried via `TargetArg` (no repo open), **N** = multi-item
(`targetCount` when ≥2, §2.7), **H** = HEAD branch via `resolve_activity_target`, **R** = branch of
the in-progress rebase, **C** = HEAD commit short oid (bisect midpoint), **—** = always `None`.
`Net` = command layer emits `phase(Network)` right after `started`. `Out` = outcome classifier (§2.6).
Copy is in P119-ui §4.3.

| # | Rust variant | wire | Src / `TargetArg` | Net | Out | Wrapped `*_inner` (file) |
|---|---|---|---|---|---|---|
| 1 | `CheckoutBranch` | `checkoutBranch` | A `Branch(name)` | | checkout | `checkout_branch_inner` (branches.rs) |
| 2 | `CheckoutCommit` | `checkoutCommit` | A `Commit(oid)` | | checkout | `checkout_commit_inner` (branches.rs) |
| 3 | `CheckoutRemote` | `checkoutRemote` | A `Ref(name)` | | | `checkout_remote_inner` (branches.rs) |
| 4 | `CreateBranch` | `createBranch` | A `Branch(name)` | | createHere | `create_branch_inner` (none), `create_branch_here_inner` (branches.rs) |
| 5 | `DeleteBranch` | `deleteBranch` | A `Branch(name)` | | | `delete_branch_inner` (branches.rs) |
| 6 | `DeleteBranches` | `deleteBranches` | N `Branch` over `names` | | | `delete_branches_inner` (branches.rs) |
| 7 | `RenameBranch` | `renameBranch` | A `Branch(new_name)` | | | `rename_branch_inner` (branches.rs) |
| 8 | `DeleteRemoteTracking` | `deleteRemoteTracking` | A `Ref(name)` | | | `delete_remote_tracking_inner` (branches.rs) |
| 9 | `Merge` | `merge` | A `Ref(name)` (incoming) | | merge | `merge_branch_inner` (merge.rs) |
| 10 | `AbortMerge` | `abortMerge` | H | | | `abort_merge_inner` (merge.rs) |
| 11 | `Rebase` | `rebase` | A `Ref(onto)` | | rebase | `rebase_branch_inner` (rebase.rs) |
| 12 | `InteractiveRebase` | `interactiveRebase` | A `Commit(onto_oid)` | | rebase | `start_interactive_rebase_inner` (rebase.rs) |
| 13 | `RebaseContinue` | `rebaseContinue` | R | | rebase | `rebase_continue_inner` (rebase.rs) |
| 14 | `RebaseSkip` | `rebaseSkip` | R | | rebase | `rebase_skip_inner` (rebase.rs) |
| 15 | `RebaseAbort` | `rebaseAbort` | R | | | `rebase_abort_inner` (rebase.rs) |
| 16 | `CherryPick` | `cherryPick` | A `Commit(oid)` | | pick | `cherrypick_commit_inner` (cherrypick.rs) |
| 17 | `CherryPickContinue` | `cherryPickContinue` | H | | pick | `cherrypick_continue_inner` (cherrypick.rs) |
| 18 | `CherryPickAbort` | `cherryPickAbort` | H | | | `cherrypick_abort_inner` (cherrypick.rs) |
| 19 | `Revert` | `revert` | A `Commit(oid)` | | revert | `revert_commit_inner` (revert.rs) |
| 20 | `RevertContinue` | `revertContinue` | H | | revert | `revert_continue_inner` (revert.rs) |
| 21 | `RevertAbort` | `revertAbort` | H | | | `revert_abort_inner` (revert.rs) |
| 22 | `ResetSoft` | `resetSoft` | A `Commit(oid)` | | | `reset_branch_command_inner` when `mode == Soft` (reset.rs) |
| 23 | `ResetMixed` | `resetMixed` | A `Commit(oid)` | | | … `Mixed` |
| 24 | `ResetHard` | `resetHard` | A `Commit(oid)` | | | … `Hard` (also the Undo execution path) |
| 25 | `StashCreate` | `stashCreate` | H | | | `create_stash_inner` (stash.rs) |
| 26 | `StashApply` | `stashApply` | A `Stash(index)` | | stash | `apply_stash_inner` (stash.rs) |
| 27 | `StashPop` | `stashPop` | A `Stash(index)` | | stash | `pop_stash_inner` (stash.rs) |
| 28 | `StashDrop` | `stashDrop` | A `Stash(index)` | | | `drop_stash_inner` (stash.rs) |
| 29 | `CreateTag` | `createTag` | A `Tag(name)` | | | `create_tag_inner` (tags.rs) |
| 30 | `DeleteTag` | `deleteTag` | A `Tag(name)` | | | `delete_tag_inner` (tags.rs) |
| 31 | `PushTag` | `pushTag` | A `Tag(tag_name)` | ✓ | | `push_tag_inner` (tags.rs) |
| 32 | `DeleteRemoteTag` | `deleteRemoteTag` | A `Tag(tag_name)` | ✓ | | `delete_remote_tag_inner` (tags.rs) |
| 33 | `ForceRefreshTag` | `forceRefreshTag` | A `Tag(tag_name)` | ✓ | | `force_refresh_tag_inner` (tags.rs) |
| 34 | `SubmoduleAdd` | `submoduleAdd` | A `Name(path)` | ✓ | | `add_submodule_inner` (submodules.rs) |
| 35 | `SubmoduleInit` | `submoduleInit` | A `Name(name)` | | | `init_submodule_inner` |
| 36 | `SubmoduleUpdate` | `submoduleUpdate` | A `Name(name)` | ✓ | | `update_submodule_inner` |
| 37 | `SubmoduleSync` | `submoduleSync` | A `Name(name)` | | | `sync_submodule_inner` |
| 38 | `SubmoduleDeinit` | `submoduleDeinit` | A `Name(name)` | | | `deinit_submodule_inner` |
| 39 | `SubmoduleRemove` | `submoduleRemove` | A `Name(name)` | | | `remove_submodule_inner` |
| 40 | `WorktreeAdd` | `worktreeAdd` | A `Name(name)` | | | `add_worktree_inner`, `add_worktree_with_changes_inner` (worktree.rs) |
| 41 | `WorktreeRemove` | `worktreeRemove` | A `Name(name)` | | | `remove_worktree_inner` |
| 42 | `WorktreeLock` | `worktreeLock` | A `Name(name)` | | | `lock_worktree_inner` |
| 43 | `WorktreeUnlock` | `worktreeUnlock` | A `Name(name)` | | | `unlock_worktree_inner` |
| 44 | `Discard` | `discard` | N `Name` over `paths`; `discard_partial` = A `Name(path)` | | | `discard_paths_inner`, `discard_paths_force_inner`, `discard_partial_inner` (discard.rs) |
| 45 | `BisectStart` | `bisectStart` | A `Commit(bad)` | | | `start_bisect_inner` (bisect.rs) |
| 46 | `BisectGood` | `bisectGood` | C | | | `bisect_mark_inner` when `is_good` |
| 47 | `BisectBad` | `bisectBad` | C | | | `bisect_mark_inner` when `!is_good` |
| 48 | `BisectSkip` | `bisectSkip` | C | | | `bisect_skip_inner` |
| 49 | `BisectReset` | `bisectReset` | — | | | `bisect_reset_inner` |
| 50 | `ComposeCommits` | `composeCommits` | H | | | `apply_composed_commits_inner` (compose.rs) |
| 51 | `CloneRepo` | `cloneRepo` | A `PathLeaf(dest)` — **never the URL** | ✓ + progress | | `clone_repo` (repo.rs), gains `state` |
| 52 | `InitRepo` | `initRepo` | A `PathLeaf(path)` | | | `init_repo` (repo.rs), gains `state` |
| 53 | `AddRemote` | `addRemote` | A `Remote(name)` — never URL | | | `add_remote_inner` (remotes.rs) |
| 54 | `RemoveRemote` | `removeRemote` | A `Remote(name)` | | | `remove_remote_inner` |
| 55 | `RenameRemote` | `renameRemote` | A `Remote(new_name)` | | | `rename_remote_inner` |
| 56 | `SetRemoteUrl` | `setRemoteUrl` | A `Remote(name)` — never URL | | | `set_remote_url_inner` |

Notes:
- Row 51 is `CloneRepo`, not `Clone`: the enum derives `Clone`, and a variant with that name would
  shadow the trait under `use GitActivityCategory::*`.
- Reset mode (P119-ui §9-3): **three categories**, not a mode field. The call site picks the variant
  from its `mode: ResetMode` argument; no extra wire field is needed and the noun can say `Hard reset`.
- The P87 categories are unchanged: `Commit`, `Amend`, `MergeCommit`, `Push`, `ForcePush`, `Fetch`,
  `Pull` (7), which gives **63 total**.

### 1.1 Excluded (verified against `src-tauri/src/lib.rs`)

| Command(s) | Why |
|---|---|
| `stage`, `unstage`, `stage_partial`, `unstage_partial` | **User ruling 2026-09-24: not logged.** |
| `resolve_conflict`, `resolve_conflict_text`, `ai_apply_resolution` | **User ruling: not logged** (same class as stage). |
| `auto_sync_tags` | No component calls it; in production it runs inside `fetch` (`remotes.rs:43`, core call), so the Fetch row covers it. |
| `describe_last_undo` | Read-only; Undo executes via `reset_branch` → a `resetHard`/`resetMixed`/`resetSoft` row (user ruling). |
| config/identity/profile commands | Config, not repository content. |
| `forge_*` | Host API, not the local repo. |
| MCP write tools | Separate process, no hub. |
| `commit`, `commit_amend`, `commit_merge`, `fetch`, `pull`, `push`, `force_push` | Already wrapped (P87); they gain the §2.8 failure line automatically. |
| Refresh | User ruling: not logged; the icon spins. |

## 2. Rust — `bonsai-core`

### 2.1 New file `crates/bonsai-core/src/git/activity_category.rs`

Holds `GitActivityCategory` (moved out of `activity.rs`, which is 438 lines) and the new
`GitRunOutcome`. `activity.rs` re-exports both (`pub use crate::git::activity_category::{GitActivityCategory, GitRunOutcome};`),
so existing import paths are unchanged. Register it in `git/mod.rs`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitActivityCategory {
    Commit, Amend, MergeCommit, Push, ForcePush, Fetch, Pull,          // P87, order preserved
    // P119 — exactly §1 rows 1–56, in table order
    CheckoutBranch, CheckoutCommit, CheckoutRemote, CreateBranch, DeleteBranch, DeleteBranches,
    RenameBranch, DeleteRemoteTracking, Merge, AbortMerge, Rebase, InteractiveRebase,
    RebaseContinue, RebaseSkip, RebaseAbort, CherryPick, CherryPickContinue, CherryPickAbort,
    Revert, RevertContinue, RevertAbort, ResetSoft, ResetMixed, ResetHard, StashCreate, StashApply,
    StashPop, StashDrop, CreateTag, DeleteTag, PushTag, DeleteRemoteTag, ForceRefreshTag,
    SubmoduleAdd, SubmoduleInit, SubmoduleUpdate, SubmoduleSync, SubmoduleDeinit, SubmoduleRemove,
    WorktreeAdd, WorktreeRemove, WorktreeLock, WorktreeUnlock, Discard, BisectStart, BisectGood,
    BisectBad, BisectSkip, BisectReset, ComposeCommits, CloneRepo, InitRepo, AddRemote,
    RemoveRemote, RenameRemote, SetRemoteUrl,
}
impl GitActivityCategory { pub const ALL: [GitActivityCategory; 63] = [/* declaration order */]; }

/// How a SUCCESSFUL run ended, when that is more than "done". Carried on `finished` only.
/// Absent on failed runs and on runs with nothing to add.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GitRunOutcome { Conflicts, FastForwarded, Merged, UpToDate }
```

### 2.2 `ActivityTarget`: new `pub(crate)` constructors (in `activity.rs`)

All of them go through the private `new` funnel (strip controls/bidi/zero-width → trim → 255-char cap).

```rust
pub(crate) fn tag(tag: &str) -> Option<Self>;        // strips refs/tags/
pub(crate) fn any_ref(r: &str) -> Option<Self>;      // strips ONE of refs/heads/, refs/remotes/, refs/tags/
pub(crate) fn commit(oid: &str) -> Option<Self>;     // Some(first 7) iff len>=7 && all ASCII hex; else None
pub(crate) fn stash(index: usize) -> Option<Self>;   // "stash@{N}"
pub(crate) fn name(raw: &str) -> Option<Self>;       // raw name/path identifier; MAY contain spaces
```

**File size:** `activity.rs` ends up around 438 − 11 (enum) + 30 (constructors) + 35 (§2.6–2.8
emitter changes) ≈ **~490**. If it crosses 500, move the `ActivityTarget` block (currently lines
~373–433) into a child module (`#[path = "activity_target_type.rs"] mod target_type;` +
`pub use`). A child module can call the parent's private `strip_control_chars`/`truncate_chars`, so
the one-funnel property holds.

### 2.3 `activity_target.rs`: arg funnel, subject, resolver extension (109 → ~250)

```rust
/// P119: the ONLY cross-crate input for an arg-carried target. Core maps the raw argument
/// through the one sanitize+cap funnel; src-tauri still cannot build an ActivityTarget.
#[derive(Debug, Clone, Copy)]
pub enum TargetArg<'a> {
    Branch(&'a str), Ref(&'a str), Remote(&'a str), Tag(&'a str),
    Commit(&'a str), Stash(usize), Name(&'a str),
    PathLeaf(&'a str), // last path component, either separator, trailing separators ignored
}
/// PURE, infallible, no repo open, no I/O.
pub fn arg_activity_target(arg: TargetArg<'_>) -> Option<ActivityTarget>;

/// What a run is about: at most one target, or a count of ≥2 items (never both).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunSubject { pub(crate) target: Option<ActivityTarget>, pub(crate) count: Option<u32> }
impl From<Option<ActivityTarget>> for RunSubject { /* count: None */ }
impl RunSubject {
    /// 0 items → empty; 1 → that item's target; ≥2 → count = n (saturating u32), target None.
    pub fn many<'a>(items: impl ExactSizeIterator<Item = TargetArg<'a>>) -> RunSubject;
}
```

`PathLeaf` pseudocode (platform-independent):
`s.trim_end_matches(['/', '\\']).rsplit(['/', '\\']).next()`; empty → `None`.

`resolve_activity_target(workdir, category)`: the final `match` names every variant, with **no `_`
arm**:

```
  // 1. Before any repo open. (A/N rows never call this; the guard keeps it free if one does.)
  if matches!(category, Fetch | <every A/N row> | BisectReset): return None
  // 2. Rebase in progress. MUST come before the detached check: HEAD is detached mid-rebase.
  if matches!(category, RebaseContinue | RebaseSkip | RebaseAbort):
      return match read_op_state(workdir).ok()? { Rebase { head_name: Some(h), .. } => ActivityTarget::branch(&h), _ => None }
  repo = open_repo_at(workdir).ok()?; head = read_head_info(&repo).ok()?
  // 3. Bisect midpoint: HEAD is detached on the commit being judged.
  if matches!(category, BisectGood | BisectBad | BisectSkip):
      return ActivityTarget::commit(&repo.head().ok()?.peel_to_commit().ok()?.id().to_string())
  // 4. HEAD branch.
  if head.unborn || head.detached → None
  branch = head.branch_name?
  match category {
    Commit | Amend | MergeCommit | AbortMerge | CherryPickContinue | CherryPickAbort | RevertContinue
      | RevertAbort | StashCreate | ComposeCommits => ActivityTarget::branch(&branch),
    Pull | ForcePush | Push => (unchanged P87b-FU1 logic),
    <every variant handled in steps 1–3> => None,   // unreachable; None, never panic
  }
```

### 2.4 FU-1 guarantees after P119

| FU-1 §6 guarantee | After P119 |
|---|---|
| 2. Set on `started`, immutable | Unchanged, structural. `targetCount` follows the same rule: set at emitter construction, `started` only. |
| 3. Sanitized / 4. ≤255 chars | Unchanged, structural (one funnel). |
| 1. Raw identifier, never a phrase | **Structural** for `Branch/Ref/Remote/Tag/Commit/Stash` and for the H/R/C resolver. **Test + review-backed** for `Name`/`PathLeaf`, which take any string. Mitigations: §1 fixes each call site's argument, `tests_activity_coverage.rs` asserts the exact `target`, and review rejects any `Name`/`PathLeaf` call site fed a URL or prose. Multi-item ops send a **count**, never a phrase (§2.7). |

The "no space" assertion of `no_target_contains_prose` covers ref-typed outputs only; `Name` and
`PathLeaf` may contain spaces (a worktree called `my wt`).
**Security:** `CloneRepo`, `AddRemote` and `SetRemoteUrl` never put the URL on the wire (it can
carry `user:token@`).

### 2.5 Clone progress

In `remote_activity.rs` (or a new `git/clone_activity.rs` if that file would cross ~500, with
`progress_should_fire` made `pub(crate)`):

```rust
/// Throttled CloneProgress → recorder bridge (same §14.3 coalescing as fetch/pull).
pub struct CloneActivityForwarder<'a> { /* rec, last: Option<Instant>, done_emitted: bool */ }
impl<'a> CloneActivityForwarder<'a> {
    pub fn new(rec: Option<&'a dyn GitActivityRecorder>) -> Self;
    /// CloneProgress → GitTransferProgress (deltas Some iff total_deltas > 0; indexed_objects :=
    /// received_objects); calls rec.progress iff progress_should_fire. No-op with None.
    pub fn tick(&mut self, p: &CloneProgress);
}
```

### 2.6 Outcome (user rulings 2 + 3)

`GitActivityEvent` gains:
```rust
/// `Finished` ONLY, and only when `success == true`. P119.
#[serde(skip_serializing_if = "Option::is_none")]
pub outcome: Option<GitRunOutcome>,
```
`base()` sets `None`. A run paused on conflicts is `finished{code:0, success:true, outcome:"conflicts"}`:
the command returned `Ok`, and the third status is derived on the frontend (§4.3). Keeping
`success` truthful keeps older readers correct.

New pure classifiers in a new file `crates/bonsai-core/src/git/activity_outcome.rs`. Each is
`fn(&T) -> Option<GitRunOutcome>`, and **conflicts wins**:

| fn | Input | Mapping |
|---|---|---|
| `merge_outcome` | `MergeOutcome` | `UpToDate`→`UpToDate`; `FastForwarded`→`FastForwarded`; `Merged`→`Merged`; `Conflicts`/`StashPopConflicts`→`Conflicts` |
| `rebase_outcome` | `RebaseOutcome` | `UpToDate`→`UpToDate`; `FastForwarded`→`FastForwarded`; `Rebased`→`None`; every conflict-carrying variant→`Conflicts` |
| `cherrypick_outcome` / `revert_outcome` | `CherrypickOutcome` / `RevertOutcome` | `Conflicts`/`StashPopConflicts`→`Conflicts`; else `None` |
| `stash_apply_outcome` | `ApplyStashOutcome` | `Conflicts`→`Conflicts`; else `None` |
| `checkout_outcome` | `CheckoutResult` | `apply == Some(Conflicts)`→`Conflicts`; else `fast_forwarded`→`FastForwarded`; else `None` |
| `create_here_outcome` | `CreateBranchHereResult` | `apply == Some(Conflicts)`→`Conflicts`; else `None` |
| `no_outcome<T>` | any | `None` |

"Paused on conflicts" is therefore detected from the command's own typed result. There is no extra
repo read and no string matching; the table in `activity_outcome_tests.rs` pins it (match
exhaustively on every variant, no `_`, so a new variant fails to compile).

### 2.7 `targetCount` (P119-ui §9-1)

`GitActivityEvent` gains:
```rust
/// `Started` ONLY; present iff the run acts on ≥2 items (then `target` is absent). A count, not
/// copy — the frontend owns "Delete 3 branches". P119.
#[serde(skip_serializing_if = "Option::is_none")]
pub target_count: Option<u32>,
```
`ActivityEmitter` stores `target_count` next to `target`, set at construction:
`ActivityEmitter::with_subject(id, subject: RunSubject, emit)`. The existing
`ActivityEmitter::new(id, Option<ActivityTarget>, emit)` stays and delegates with `count: None`, so
existing call sites and tests are unchanged. Only `started()` reads it.

### 2.8 Failure reason line (P119-ui §9-4)

```rust
impl ActivityEmitter {
    /// Terminal event, P119 form. Order: flush the truncation marker → if `reason` is Some, emit ONE
    /// `StderrLine` with `activity_line(reason)` (exempt from MAX_ACTIVITY_LINE_EVENTS, so it is
    /// always delivered) → `Finished{code, success, outcome}`.
    pub fn finish(&self, code: Option<i32>, success: bool, outcome: Option<GitRunOutcome>, reason: Option<&str>);
}
```
`finished(code, success)` stays and delegates to `finish(code, success, None, None)`.
The reason is `AppError::message()`, the same string serialized as `message` for the toast (make it
`pub` if it is not). It is **skipped for `AppError::HookRejected`**, whose hook output is already on
the stream as lines and whose message is the whole multi-line hook body.

## 3. Rust — `src-tauri`

### 3.1 `commands/activity.rs` (204 → ~300)

```rust
/// P119 full bracket. `with_activity` (unchanged signature) now delegates here with
/// `RunSubject::from(target)` and `no_outcome`, so the P87 call sites compile untouched and gain
/// the failure line.
pub(crate) async fn with_activity_ex<T, F, Fut>(
    hub: GitActivityHub,
    category: GitActivityCategory,
    subject: RunSubject,
    classify: fn(&T) -> Option<GitRunOutcome>,
    run: F,
) -> Result<T, AppError>
where F: FnOnce(Option<Arc<ActivityEmitter>>) -> Fut, Fut: Future<Output = Result<T, AppError>>;
//  inactive → run(None).await (passthrough, unchanged)
//  started → res = run(Some(em)).await →
//    Ok(v)  → em.finish(Some(0), true, classify(&v), None)
//    Err(e) → em.finish(activity_exit_code(&e), false, None,
//                       (!matches!(e, HookRejected(_))).then(|| e.message()))

pub(crate) fn arg_target(state: &AppState, arg: TargetArg<'_>) -> Option<ActivityTarget>; // None if inactive
pub(crate) fn arg_subject_many<'a>(state: &AppState, items: impl ExactSizeIterator<Item = TargetArg<'a>>) -> RunSubject;
#[derive(Clone, Copy)] pub(crate) enum LoggedPhase { Local, Network }

/// Bracket for a command with no recorder-aware core fn: repo_path INSIDE the bracket (a
/// `noRepo` still yields a failed row), optional phase(Network), job on the blocking pool, join
/// error → AppError::Other("task join error: {e}") exactly as today.
pub(crate) async fn logged_blocking<T, F>(
    state: &AppState, repo_id: &str, category: GitActivityCategory,
    subject: impl Into<RunSubject>, phase: LoggedPhase,
    classify: fn(&T) -> Option<GitRunOutcome>, job: F,
) -> Result<T, AppError>
where T: Send + 'static, F: FnOnce(std::path::PathBuf) -> Result<T, AppError> + Send + 'static;
```
**Re-exports:** command files import through `use super::shared::*;` (`branches.rs:3`, `merge.rs:3`).
Add `arg_target`, `arg_subject_many`, `logged_blocking`, `LoggedPhase`, `TargetArg`, `RunSubject`
and the §2.6 classifiers to `commands/shared.rs`.

### 3.2 Per-command wrap

```rust
pub(crate) async fn merge_branch_inner(state: &AppState, repo_id: &str, name: String, skip_hooks: Option<bool>)
    -> Result<MergeOutcome, AppError> {
    let target = arg_target(state, TargetArg::Ref(&name));
    let skip = skip_hooks.unwrap_or(false);
    logged_blocking(state, repo_id, GitActivityCategory::Merge, target, LoggedPhase::Local,
        merge_outcome, move |path| merge::merge_branch(&path, &name, skip)).await
}
```
- H/R/C rows: `let target = activity_target(state, repo_id, <cat>).await;`, then the same pattern.
- N rows: `let subject = arg_subject_many(state, names.iter().map(|n| TargetArg::Branch(n)));`.
- Reset: `let cat = match mode { Soft => ResetSoft, Mixed => ResetMixed, Hard => ResetHard };`.
- Bisect mark: `let cat = if is_good { BisectGood } else { BisectBad };`.
- `clone_repo` / `init_repo` gain `state: tauri::State<'_, AppState>` (Tauri-injected, no TS
  change). They call `with_activity_ex` directly, because they are not repo-scoped. For clone:
  emit `phase(Network)`, then run `clone_repo_core` with
  `move |p| { fwd.tick(&p); let _ = on_progress.send(p); }`, where `fwd` is a
  `CloneActivityForwarder` built from the emitter inside `spawn_blocking`. Move the
  `Arc<ActivityEmitter>` into the closure if the `&dyn` lifetime fights `Send`.
- Every `*_inner` / `#[tauri::command]` signature is otherwise unchanged. No op gains or loses an
  error, and no `repo-changed` emission changes.

File sizes: each wrap nets about ±0–3 lines; `repo.rs` 410 → ~430. No src-tauri file crosses 500.

## 4. TypeScript

### 4.1 `src/ipc/types/activity.ts`

```ts
export type GitActivityCategory = /* 7 P87 */ | /* 56 §1 wire strings, table order */;
export type GitRunOutcome = 'conflicts' | 'fastForwarded' | 'merged' | 'upToDate';
export interface GitActivityEvent {
  // …existing…
  /** `finished` only, success runs only. P119. */
  outcome?: GitRunOutcome;
  /** `started` only; present iff ≥2 items (then `target` is absent). P119. */
  targetCount?: number;
}
```

### 4.2 New `src/components/gitActivityCategories.ts` (table file)

Move `CATEGORY_META` (+ `CategoryMeta`) and `TARGET_PREPOSITION` out of `gitActivityFormat.ts`
(320 lines). Both stay `Record<GitActivityCategory, …>` so tsc enforces coverage. The P119-ui fields
(`countNoun`, `blockedNoun`, the preposition set incl. `null`) live here, and their shape is owned by
P119-ui §4.2. Export `GIT_ACTIVITY_CATEGORIES` (declaration order). P119-1 lands placeholder copy
(noun = verb = the operation name); P119-4 replaces it with the final table.

### 4.3 Store + formatter (P119-4)

- `gitActivityState.ts`: `GitActivityRun` gains `outcome: GitRunOutcome | null` and
  `targetCount: number | null`. The status union gains `'conflicts'`.
  `isTerminalGitStatus('conflicts') === true`, so pruning/`hasTerminalRuns` treat it as terminal.
- `useGitActivity.ts` reducer: `started` → `targetCount = ev.targetCount ?? null`; `finished` →
  `status = !success ? 'failed' : ev.outcome === 'conflicts' ? 'conflicts' : 'success'`,
  `outcome = ev.outcome ?? null`.
- `gitActivityFormat.ts`: `statusPill('conflicts')`, the row detail for
  `fastForwarded`/`merged`/`upToDate`, `countNoun` rendering when `targetCount !== null`, and the
  conflicts sentence all follow P119-ui §4.4–4.5. `runTarget` is unchanged.

### 4.4 `useGitDock.ts`

Remove `gitProgress` from `GitToolbarProps`; keep `gitCategory`, `gitPhase`, `onShowGitActivity`.
`isRemoteCategory` and `commitPhase` are unchanged.

### 4.5 `GitActivityDock.tsx`: determinate bar

```tsx
const fraction = activeRun !== null ? progressFraction(activeRun) : null;
{activeRun !== null && (
  <div className="git-dock-progress" aria-hidden="true"
       data-determinate={fraction !== null ? 'true' : undefined}
       style={fraction !== null ? ({ '--progress': fraction } as CSSProperties) : undefined} />
)}
```
`activeRun` = the first running run; with concurrent runs the bar reflects that one (accepted).

### 4.6 `WorkspaceToolbar.tsx` + `RepoWorkspace.tsx` + CSS

- Delete the `header-progress` block (`WorkspaceToolbar.tsx:290-303`), the `netBusy` and
  `gitProgress` props, and the `CSSProperties` import that becomes unused. `remoteOp` and
  `refreshing` stay. The refresh spin marker and CSS are per P119-ui §3.
- `RepoWorkspace.tsx:1825`: delete `netBusy={submoduleBusy !== null}` (baselined file shrinks;
  add nothing). `submoduleBusy` stays (sidebar).
- `graph-banners.css`: delete `.header-progress` and `::after` (23-39); **keep
  `@keyframes header-progress-sweep`**.
- `git-dock.css:46-52` and `reduced-motion.css:30`: the selector becomes
  `.git-dock-progress[data-determinate]::after`. `reduced-motion.css:10-14`: delete.
- Grep gate: `header-progress` survives only as the keyframe name and its `animation:` uses.

## 5. Mock IPC

### 5.1 `src/ipc/mock/gitActivity.ts` (352 → ~420)

- `runMockActivity(category, target, fn, opts?: { classify?: (r: T) => GitRunOutcome | null; count?: number })`.
  `count >= 2` → `started` carries `targetCount` and no `target` (mirror of `RunSubject::many`).
- `GitSequencer.finished` gains `outcome` (key dropped when null). A failure emits
  `stderr(appError.message)` before `finished`, except for `hookRejected` (mirror of §2.8). This
  applies to the existing scripts as well.
- New `runPlain` (default for every non-push/fetch/commit category): `start` → `phase('network')`
  if `MOCK_NETWORK_CATEGORIES` has it (`pushTag, deleteRemoteTag, forceRefreshTag, submoduleAdd,
  submoduleUpdate, cloneRepo`) → for `cloneRepo` with `?fetchSlow`, `emitProgressRamp(s)` →
  `?gitSlowLocal` → `delay(800)` → `fn()` → `finished(0, true, classify(result))`.
- Dispatch: push family → `runPush`; fetch/pull → `runFetch`; **explicit** `commit|amend|mergeCommit`
  → `runCommit`; everything else → `runPlain`.
- Seams (P119-ui §8): `?gitSlowLocal`; `?gitConflicts` forces `outcome: 'conflicts'` for the §2.6
  conflict-capable categories; `?gitMultiTarget` forces `count: 3` on `deleteBranches`/`discard`.
- Target mirrors: `mockCommitTarget` (7-hex), `mockStashTarget`, `mockPathLeaf`; mock
  classifiers `mockMergeOutcome` etc. mirror §2.6 over the mock result types.

### 5.2 Handler wraps (body → `xInner`; public = `runMockActivity(cat, target, () => xInner(...), opts)`)

| Mock file | Handlers |
|---|---|
| `handlers/branches.ts` | checkoutBranch, checkoutCommit, checkoutRemote, createBranch, createBranchHere, deleteBranch, deleteBranches (count), renameBranch, deleteRemoteTracking |
| `handlers/merge.ts` | mergeBranch (classify), abortMerge |
| `handlers/rebase.ts` | rebaseBranch, startInteractiveRebase, rebaseContinue, rebaseSkip (classify), rebaseAbort |
| `handlers/resetRevert.ts` | resetBranch (mode → category), discardPaths/Force (count), cherrypick*/revert* (classify) |
| `handlers/status.ts` | discardPartial only (**not** stage/unstage) |
| `handlers/stash.ts` | createStash, applyStash/popStash (classify), dropStash |
| `handlers/repoMeta.ts` | createTag, deleteTag, pushTag, addRemote, removeRemote, renameRemote, setRemoteUrl |
| `handlers/tagSync.ts` | forceRefreshTag, deleteRemoteTag (**not** autoSyncTags: mock fetch calls it at `remotesSync.ts:30`) |
| `handlers/submodules.ts` | add/init/update/sync/deinit/remove |
| `handlers/worktrees.ts` | addWorktree, addWorktreeWithChanges, removeWorktree, lockWorktree, unlockWorktree |
| `handlers/bisectHistory.ts` | startBisect, bisectMark (good/bad), bisectSkip, bisectReset |
| `handlers/compose.ts` | applyComposedCommits |
| `handlers/repo.ts` | cloneRepo, initRepo |

H/R/C mock targets: the mock repo's HEAD branch/commit if its state exposes one, else `'main'` /
a fixture short sha. Largest mock file: `branches.ts` 353 → ~395.

## 6. Tests

Rust (bonsai-core):
- **T-R1** `activity_category_tests.rs`: all 63 `ALL` entries → exact wire strings; `GitRunOutcome`
  → `conflicts|fastForwarded|merged|upToDate`.
- **T-R2** `activity_target_arg_tests.rs` (new): the `TargetArg` table (prefix strips; `Commit`
  40-hex→7, `HEAD~2`/`abc`→None; `stash@{2}`; `Name("my wt")`; `PathLeaf` for `C:\a\repo\`, `/a/b/`,
  `""`, `https://tok@host/r.git`→`r.git`; bidi stripped; 400 chars → 255 ending `…`) and
  `RunSubject::many` (0/1/2+ items; count set ⇒ target None).
- **T-R3** extend `resolves_table`: mid-rebase fixture → R rows = `topic`, `AbortMerge` = None;
  bisect fixture → C rows = HEAD short oid; on-branch → every H row = the branch; unborn → None.
- **T-R4** `no_target_contains_prose` / `resolver_never_panics_on_broken_repo` iterate `ALL`.
- **T-R5** `activity_outcome_tests.rs`: every variant of every §2.6 input type.
- **T-R6** emitter: `target_count` only on `started`; `outcome` only on `finished`; the reason line
  is delivered after 5000+ suppressed lines and just before `finished`; `finish(..)` with `None`
  everywhere is byte-identical to today's `finished`.
- **T-R7** `CloneActivityForwarder`: coalescing within 50 ms, first terminal tick always fires,
  no-op with None.

Rust (src-tauri), `tests_activity_coverage.rs` (new; capture via `tauri::ipc::Channel::new`, or a
`#[cfg(test)]` hub sink if that needs a runtime):
- One test per family: one `started` with the exact category + `target`/`targetCount`, and one
  `finished`.
- Merge **FF → `outcome:"fastForwarded"`**; merge with conflicts → `success:true,
  outcome:"conflicts"`; rebase paused → conflicts; stash pop conflict → conflicts.
- `delete_branches` of 3 → `targetCount:3`, no `target`; reset `Hard` → `resetHard`.
- Failure: `delete_branch_inner` on a missing branch → last two events are `stderrLine(<message>)`
  then `finished{success:false}`, and the returned error is unchanged. A push `HookRejected` gets
  no reason line.
- The existing `tests_*.rs` suites (inactive hub) stay green unmodified.

Frontend (vitest):
- `gitActivityCategories.test.ts`: the key set equals the 63-string list.
- `useGitActivity` / `gitActivityState` tests: `finished{success, outcome:'conflicts'}` →
  status `conflicts` (terminal, prunable); `targetCount` stored; FF outcome stored.
- `gitActivityFormat.test.ts`: conflicts pill + sentence, the count noun, the FF/merged detail
  (copy from P119-ui).
- `GitActivityDock.test.tsx`: determinate `--progress: 0.5` at 50/100; none without progress.
- `WorkspaceToolbar.test.tsx:83` inverted: no `.header-progress` in any state; spin marker iff
  `refreshing`.
- `src/ipc/mock/handlers/activityCoverage.test.ts`: table over §5.2 (category, target/count, outcome
  under `?gitConflicts`); `stage`, `unstage`, `resolveConflict`, `autoSyncTags` emit nothing; a
  throwing handler emits the message line.

## 7. Resolved (was open in rev 1)

| Topic | Resolution |
|---|---|
| Stage/unstage, conflict-resolution writes | Not logged (user). |
| Conflicts | `finished.outcome = 'conflicts'` with `success: true` → frontend status `conflicts` (user). |
| Fast-forward | One `merge` row; the detail comes from `outcome` (`fastForwarded`/`merged`/`upToDate`) (user). Checkout's auto-FF and rebase FF/up-to-date use the same field. |
| Clone/init | Logged only when a workspace is mounted (no subscriber otherwise); `initRepo` kept (user). |
| Undo | Shows as the reset row of its mode (user). |
| Multi-target | `started.targetCount`. |
| Failure reason | Final `stderrLine` = `AppError::message()`, except `hookRejected`. |
| Reset mode | Three categories. |

Remaining note (no action): events carry no repo id, so a background op in another tab shows in the
mounted workspace's dock. This is pre-existing and unchanged.

## 8. Sub-increments

1. **P119-1: core model + TS types + mock dispatch.** §2.1–2.8 (enum, outcome, `RunSubject`,
   `TargetArg`, resolver, classifiers, emitter `finish`/`with_subject`, clone forwarder), §4.1, the
   §4.2 table with placeholder copy, and the §5.1 `runMockActivity`/sequencer/`runPlain` changes.
   The Rust enum, the TS union and the category tables must land together. Tests T-R1…T-R7 and
   `gitActivityCategories.test.ts`. Checks: `cargo nextest -p bonsai-core activity`, `tsc`.
2. **P119-2: src-tauri wrapping.** §3 (`with_activity_ex`, helpers, `shared.rs`, every §1 row, clone/init
   `state`) + `tests_activity_coverage.rs`. Checks: `cargo check`, targeted nextest, clippy.
3. **P119-3: mock handlers** §5.2 + seams + `activityCoverage.test.ts`. Can run in parallel with P119-2
   once P119-1 has landed.
4. **P119-4: UI.** §4.3–4.6 plus the final P119-ui copy (after ui-designer applies §10);
   store/format/dock/toolbar tests; harness checks with `?fetchSlow`, `?gitSlowLocal`,
   `?gitConflicts`, `?gitMultiTarget`, and a failing op.

## 9. Acceptance criteria

- AC1 No `.header-progress` element in any toolbar state; `ToolbarPhaseReadout` unchanged; the
  keyframes are kept and still used.
- AC2 `.git-dock-progress` is determinate at `received/total` during a counted fetch/pull or clone,
  indeterminate otherwise; reduced motion is honoured.
- AC3 Every §1 command emits exactly one `started` + one `finished` when subscribed, zero events
  otherwise; results and errors are unchanged.
- AC4 A run paused on conflicts (merge, rebase*, cherry-pick*, revert*, stash apply/pop, checkout's
  stash re-apply) ends with `outcome:"conflicts"` and renders `! Conflicts`, neither success nor
  failure.
- AC5 A merge row states fast-forwarded / merged / already up to date from `outcome`.
- AC6 Multi-item ops carry `targetCount`, never a phrase; no URL is ever a target.
- AC7 A failed run's log ends with the user-facing error message (not for hook rejections).
- AC8 Stage/unstage/conflict-resolution writes, refresh and autoSyncTags produce no row. Submodule ops
  appear in the dock; `netBusy` is gone. Refresh spins.
- AC9 Mock IPC mirrors all of the above; the harness shows every state.
- AC10 No touched file crosses ~500 lines; `RepoWorkspace.tsx` does not grow.

## 10. P119-ui.md lines needing ui-designer changes (architect does not edit that file)

1. **§4.3 Merge table, line 284 `fast-forward` row:** remove it (ruling 3). Add merge-row detail copy
   for `outcome` `fastForwarded` / `merged` / `upToDate` (also usable for checkout auto-FF and rebase
   FF/up-to-date).
2. **§4.3 Working tree table, lines 376-377 `stage`/`unstage`:** remove them (ruling 1); `StageIcon`/
   `UnstageIcon` (§5, lines 463-464) are then unused.
3. **§6 (lines 470-486), `quiet` categories + staging-eviction rationale:** moot (no quiet categories
   remain); drop, or keep only as a reserved mechanism. §9-7 as well.
4. **§4.3 Other, line 396 `undo` row:** remove it (ruling 5: Undo = the reset row of its mode).
5. **§4.3 Other, line 401 + §9-5 `clone` excluded:** reverse it (ruling 4). Add copy for `cloneRepo`
   (target = destination folder name, `network` label, determinate progress) and `initRepo`; note the
   row appears only when a workspace is open.
6. **§4.3 Tag table, line 345 `sync` row:** remove it (`auto_sync_tags` is not logged; the Fetch row
   covers it).
7. **Count column entries `Cherry-pick {n} commits`, `Revert {n} commits`, `Delete {n} tags`,
   `Push {n} tags`, `Initialize/Update/Sync {n} submodules` (lines 303, 311, 342-343, 353-355):** those
   commands take ONE item. If the frontend loops, each call is its own run, so no count ever
   arrives. Only `deleteBranches` and `discard` carry `targetCount`. Drop the others or mark them
   future.
8. **§4.3 Reset (lines 315-322):** confirmed as three categories, `resetSoft` / `resetMixed` /
   `resetHard`; replace provisional ids with §1's.
9. **Category identifiers throughout §4.3:** replace provisional names with §1 wire strings (e.g.
   `cherryPick`, `forceRefreshTag` for "Update tag", `submoduleInit` etc.). Submodule target = the
   submodule **name** argument (usually equal to its path).
10. **§4.5 (line 428) fallback clause** ("success plus a final output line"): obsolete; the outcome
    field exists. Also add `'conflicts'` handling of checkout/create-branch stash re-apply to the ⚑
    list.
11. **§4.6 / §9-4:** confirmed as specified; note the exception that `hookRejected` gets no reason
    line (its hook output is already streamed).
12. **§9-1 / §4.4:** confirmed: `targetCount?: number` on `started`, absent (not `null`) when <2.
