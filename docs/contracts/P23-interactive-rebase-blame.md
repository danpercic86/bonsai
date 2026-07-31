# P23 — Interactive rebase + Blame / File-history: Implementation Contract

Status: authoritative for P23. This is the largest remaining milestone. It ships **two independent
features** that share almost no code, split into four fresh-context `senior-dev` sub-increments
(§12):

1. **Interactive rebase** — a rebase-plan editor (reorder / pick / reword / squash / fixup / drop),
   executed with pause-on-conflict, driven by a **Bonsai-owned cherry-pick replay engine** on a
   detached HEAD with on-disk state under `.git/bonsai-rebase/`.
2. **Blame + file history** — per-line blame (line → commit / author / oid) and a per-file commit
   history.

**Reuse mandate (locked).** Interactive rebase MUST flow its pause / continue / skip / abort through
the EXISTING machinery — `opstate.rs` (`RepoOpState::Rebase`, wire-**unchanged**), `conflict.rs`
(`list_conflicts` / `get_conflict` / `resolve_conflict` / `resolve_conflict_text`), and the actionable
`OpBanner` rebase branch (`OpBanner.tsx:73-114`). Nothing rebase-interactive-specific may leak into
`conflict.rs` or `OpBanner.tsx`. The plain-rebase Continue/Skip/Abort **commands and frontend
handlers are reused verbatim** — the backend `rebase::rebase_{continue,skip,abort}` gain a
**delegation branch** that routes to the interactive engine when `.git/bonsai-rebase/` is present.

Invariants held (unchanged from P3d/P20 §0): Rust owns all git2 logic + layout math; IPC carries
compact precomputed data; commands = request/response, no events, no channels; cores stay
Tauri-free / runtime-free and directly unit-testable; git2 runs under the established
`spawn_blocking` + runtime-free `*_inner(state, repo_id, …)` template resolving the path via
`repo_path(state, &repo_id)?`; `src/ipc/mock.ts` is updated with EVERY `IpcApi` change and kept a
faithful stateful twin; destructive ops (abort, hard restore) require explicit `ConfirmDialog`
confirmation AND a backend state guard.

Read first: `P3d-rebase.md` (rebase engine / opstate / OpBanner conventions, the drive-loop and
partial-safety discipline this contract mirrors) and `P20-daily-essentials.md` (the cherry-pick
finalize + `CHERRY_PICK_HEAD` reuse, the OpBanner actionable pattern, the command/IPC/mock house
style).

---

## 0. OPEN DECISIONS (recommended default in brackets; the contract proceeds on the default)

1. **Interactive-rebase execution model.** [**(a) Bonsai-owned cherry-pick replay on a detached
   HEAD, with an on-disk JSON todo/state file under `.git/bonsai-rebase/`**, NOT git2's `Rebase`
   op-iteration.] Justification in §2. Rejected: (b) git2 `Rebase` with pre-reordered onto/commit
   selection — cannot express squash/fixup (combine into predecessor) or reorder cleanly, and offers
   no todo cursor we can persist across IPC calls.
2. **Reword / squash message editing across IPC.** [**Messages are supplied UP FRONT in the todo
   list** (`newMessage` per op); the plan editor collects them before Start. **No editor-pause
   state** for reword/squash in v1.] This removes an entire pause mode; the only pause is
   conflict-on-apply (which reuses the existing conflict flow). Stated as a v1 simplification.
3. **Drag-to-reorder vs up/down buttons.** [**Up/Down buttons** for v1] — deterministic, keyboard-
   and test-friendly, and avoids an HTML5-DnD dependency. Drag is a Polish item. (The reorder *feel*
   is a USER CHECKPOINT regardless.)
4. **opstate detection during an interactive pause.** [**A new file-existence probe in
   `read_op_state`** that runs BEFORE the `repo.state()` switch: if `.git/bonsai-rebase/state.json`
   exists, return the EXISTING `RepoOpState::Rebase { … }` populated from that file.] No wire change,
   no new `RepoOpState` variant. During a conflict pause `repo.state()` is `CherryPick` (we use
   `repo.cherrypick` to materialize markers, §2.4); the probe must win over that.
5. **Continue/Skip/Abort command surface.** [**Reuse the existing `rebase_continue` /
   `rebase_skip` / `rebase_abort` commands**; the core fns delegate to the interactive engine when
   `.git/bonsai-rebase/` is present.] The frontend `OpBanner` + `handleRebase{Continue,Skip,Abort}`
   need ZERO changes. Rejected: three new `interactive_*` commands (would force an OpBanner routing
   flag).
6. **Empty pick during replay (result tree == parent tree).** [**Drop it silently**, matching
   default `git rebase` / plain-rebase §3.6.] No `--keep-empty` in v1.
7. **Merge commits inside the replay range.** [**Flattened / dropped**, like default `git rebase`
   and plain rebase §11.3. The plan editor only lists first-parent commits `onto..HEAD`.] No
   `--rebase-merges` in v1.
8. **Blame target for a dirty worktree.** [**Blame is against a committed version** — `atOid=None`
   means "as of HEAD". Uncommitted worktree edits are NOT attributed in v1.] `blame_buffer` for the
   dirty worktree is a later item.
9. **Blame size cap.** [**Hard cap `MAX_BLAME_LINES = 50_000`**; a larger file returns
   `AppError::Git("file too large to blame (> 50000 lines)")`.] Streaming blame over a channel is a
   later item; request/response with a cap is fine for v1.
10. **File-history rename following.** [**Best-effort `--follow` across a single rename** via
    per-commit rename detection (§9.2); the oracle fixture uses one mid-history rename and compares
    to `git log --follow`.] If exact `--follow` parity proves fragile, degrade to no-follow and
    compare to `git log --oneline -- <path>` (flagged in §9.2). `limit == 0` → the built-in cap
    `MAX_HISTORY = 1000`; the frontend passes a positive limit.
11. **`start_interactive_rebase` outcome type.** [**Reuse the existing `RebaseOutcome`**] — Start
    returns `Rebased` or `Conflicts`; there is no `UpToDate`/`FastForwarded` fast path for an
    interactive rebase (it always rewrites). Continue/Skip return the same `RebaseOutcome`, so the
    frontend handlers are reused verbatim.

None of these block implementation; all defaults are safe.

---

## 1. Module boundaries & file responsibilities

New / extended core modules under `crates/bonsai-core/src/git/` (each new module registered with a
`pub mod` line in `git/mod.rs`, alphabetical with the existing block):

| File | Responsibility | Increment |
|------|----------------|-----------|
| `rebase_interactive.rs` (new) | Todo types (`RebaseAction`, `RebaseTodoOp`), on-disk state (`InteractiveState` + `.git/bonsai-rebase/` read/write), `get_interactive_plan`, `start_interactive_rebase`, `interactive_continue`, `interactive_skip`, `interactive_abort`, the cherry-pick replay drive loop + `commit_current_op` + `finish_interactive` | P23a |
| `rebase.rs` (extend) | `rebase_continue` / `rebase_skip` / `rebase_abort` gain a **delegation branch**: if `interactive_in_progress(&repo)` → call the matching `rebase_interactive::interactive_*`; else the existing plain path unchanged | P23a |
| `opstate.rs` (extend) | `read_op_state` probes `.git/bonsai-rebase/state.json` BEFORE the `repo.state()` switch → `RepoOpState::Rebase` from the state file (§4). Wire type UNCHANGED | P23a |
| `blame.rs` (new) | `BlameLine`, `blame_file`; `FileHistoryEntry`, `file_history` | P23c |

Unchanged but REUSED verbatim: `conflict.rs`, `stage.rs` (`open_workdir_repo`, `validate_rel_path`),
`repo.rs` (`read_head_info`), `commit.rs` (`resolve_signature`), `diff.rs` (`CommitDetails` shape
reference).

Command layer: `src-tauri/src/commands.rs` gains new `#[tauri::command]` + `_inner` fns (§7);
`src-tauri/src/lib.rs` registers each in `generate_handler!`.

Frontend: IPC triple (`src/ipc/{types.ts,tauri.ts,mock.ts}` + `index.ts` re-exports); new components
`RebasePlanEditor.tsx`, `BlameView.tsx`, `FileHistoryView.tsx`; wiring in `RepoWorkspace.tsx`,
`Sidebar.tsx`, and the diff/status file-row entry points.

---

# PART A — INTERACTIVE REBASE

## 2. Execution model (LOCKED — read this before implementing)

git2's `Rebase` API iterates a **fixed linear** operation list and natively supports neither reorder,
squash, fixup, reword, nor drop. Plain rebase (`rebase.rs`) uses it because plain rebase is exactly
"replay `onto..HEAD` in order, all picks". Interactive rebase needs an editable todo list, so P23
uses a **Bonsai-owned engine**:

- **Replay onto a DETACHED HEAD.** Start detaches HEAD at `onto` and replays each todo by
  cherry-picking onto the moving detached HEAD. The **original branch ref is never moved until
  `finish`** — this makes Abort trivial and safe (just re-attach HEAD; the branch still points at
  its original tip).
- **On-disk todo + progress lives in `.git/bonsai-rebase/state.json`** (Bonsai-owned; deliberately
  NOT `.git/rebase-merge`, which is libgit2/`git`'s sequencer — colliding there would confuse
  `repo.open_rebase()` and `git` itself). Every command re-opens the repo, re-reads this file, drops
  it on finish/abort. **No engine state is held in memory across IPC calls** (same discipline as
  plain rebase).
- **Conflicts are materialized with `repo.cherrypick`** (the worktree-touching variant) so libgit2
  writes real `<<<<<<< ======= >>>>>>>` markers into the worktree + index — the SAME representation
  `conflict.rs` already reads. This transiently sets `repo.state() == CherryPick` and writes
  `CHERRY_PICK_HEAD`; that is harmless because opstate detects the interactive rebase from the
  Bonsai state file first (§4), and `commit_current_op` calls `repo.cleanup_state()` after each
  commit.

This reuses P20's cherry-pick finalize pattern (read the resolved index → build a tree → commit →
`cleanup_state`) and P3d's partial-safety discipline (Start failures restore Clean; Continue/Skip
hard errors leave the paused state intact).

### 2.1 On-disk state shape (`.git/bonsai-rebase/state.json`)

Serialized with `serde_json` (already a runtime dep of `bonsai-core`, Cargo.toml:12). camelCase.

```rust
/// Persisted interactive-rebase progress. Re-read on every IPC call; deleted on
/// finish/abort. NEVER held across calls.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractiveState {
    version: u32,             // = 1
    head_name: String,        // original branch short name (refs/heads/<head_name>)
    original_tip: String,     // 40-hex branch tip before rebase (Abort restore + display)
    onto: String,             // 40-hex base commit the replay starts from
    todos: Vec<RebaseTodoOp>, // the editable plan, in execution order
    cursor: usize,            // index of the NEXT todo to apply (0-based)
    committed: u32,           // count of todos that produced a commit (for `steps`)
    paused: bool,             // true iff a conflict pause is active at todos[cursor]
}
```

Helpers (private to `rebase_interactive.rs`):

```text
bonsai_dir(repo)        = repo.path().join("bonsai-rebase")
state_path(repo)        = bonsai_dir(repo).join("state.json")
interactive_in_progress(repo) -> bool   = state_path(repo).exists()
read_state(repo)  -> Result<InteractiveState, AppError>   # missing/corrupt -> Git("...")
write_state(repo, &state) -> Result<(), AppError>         # create_dir_all + atomic write
remove_state(repo)                                        # remove_dir_all, best-effort
effective_total(state)  = state.todos.iter().filter(|t| t.action != Drop).count() as u32
```

### 2.2 Todo wire types (`rebase_interactive.rs`)

```rust
/// Per-op action. Wire: "pick" | "reword" | "squash" | "fixup" | "drop".
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RebaseAction { Pick, Reword, Squash, Fixup, Drop }

/// One todo-list entry. `oid` = the commit being replayed. `new_message` is
/// REQUIRED for Reword, OPTIONAL for Squash (None -> default concat, §2.5),
/// ignored otherwise. Serialize (for get_interactive_plan) + Deserialize (for
/// start input and the state file).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RebaseTodoOp {
    pub oid: String,
    pub action: RebaseAction,
    #[serde(default)]
    pub new_message: Option<String>,
}
```

### 2.3 `get_interactive_plan` (seed the editor)

```rust
/// Blocking. Returns the DEFAULT todo list (every commit `Pick`, in
/// execution order = OLDEST first) for the first-parent range base..HEAD, so the
/// plan editor can seed its rows. `base_oid` is the exclusive lower bound (the
/// commit the rebase will replay onto). Does NOT mutate anything.
pub fn get_interactive_plan(workdir: &Path, base_oid: &str)
    -> Result<Vec<RebaseTodoOp>, AppError>;
```

Flow: `open_workdir_repo`; born + attached HEAD (unborn → `Git("no commits yet")`, detached →
`Git("HEAD is detached")`); resolve `base = find_commit(base_oid)`; walk first-parent from HEAD to
(not including) `base`:

```text
walk = []                         # newest-first
c = head_commit
while c.id() != base.id():
    walk.push(RebaseTodoOp { oid: c.id().hex(), action: Pick, new_message: None })
    if c.parent_count() == 0: break     # base not an ancestor -> Git("<base> is not an ancestor of HEAD")
    c = c.parent(0)
walk.reverse()                    # execution order: OLDEST first
if walk.is_empty(): Git("nothing to rebase: <base> is HEAD")   # or return []? -> see note
return walk
```

Note: if `base == HEAD` the range is empty → `Git("nothing to rebase")`. If `base` is unreachable
via first-parent → `Git("<base> is not a first-parent ancestor of HEAD")`. The editor never opens on
an empty plan.

### 2.4 `start_interactive_rebase`

```rust
/// Blocking. Starts an interactive rebase: replays `todos` (in the given order)
/// onto `onto_oid` on a detached HEAD, persisting progress under
/// `.git/bonsai-rebase/`. Clean replay runs to completion (finish); a conflict
/// pauses (RepoOpState::Rebase via the state file). Preconditions mirror plain
/// rebase (§P3d-3.1): Clean repo state, born + attached HEAD, clean index AND
/// clean worktree, git identity configured.
pub fn start_interactive_rebase(workdir: &Path, onto_oid: &str, todos: Vec<RebaseTodoOp>)
    -> Result<RebaseOutcome, AppError>;
```

Preconditions (ALL before any mutation; cheap first):
1. `open_workdir_repo(workdir)`.
2. `interactive_in_progress(&repo)` → `OperationInProgress("an interactive rebase is already in
   progress — continue or abort it first")`.
3. `repo.state() != Clean` → `OperationInProgress("an operation is already in progress — commit or
   abort it first")` (refuses a git-native rebase / merge / cherry-pick / revert).
4. `read_head_info`: unborn → `Git("cannot rebase: the repository has no commits yet")`; detached →
   `Git("cannot rebase: HEAD is detached")`. Keep `head_branch`.
5. **Clean index AND clean worktree** (identical to plain rebase §3.1.5, AMENDED): `index.has_conflicts()
   || index.write_tree_to(&repo)? != head_commit.tree_id()` → `Git("cannot rebase: your index
   contains uncommitted changes — commit or unstage them first")`; then any tracked unstaged change
   (a non-empty `repo.statuses` with untracked+ignored excluded) → `Git("cannot rebase: you have
   unstaged changes — commit or stash them first")`.
6. `sig = resolve_signature(&repo.config()?.snapshot()?)?` (`ConfigMissing` before any mutation —
   replay creates commits).
7. Resolve `onto = repo.find_commit(Oid::from_str(onto_oid).map_err(|_| Git("invalid commit id"))?)?`.
8. **Validate the todo list** (`validate_todos`, §2.6). Any violation → `Git(<message>)` BEFORE
   mutation.

Mutation (start the replay):
```text
original_tip = head_commit.id()
state = InteractiveState { version:1, head_name, original_tip: original_tip.hex(),
                           onto: onto.id().hex(), todos, cursor:0, committed:0, paused:false }
write_state(&repo, &state)                       # create .git/bonsai-rebase/state.json
repo.set_head_detached(onto.id())?               # detach; branch ref untouched
checkout_res = repo.checkout_tree(onto.as_object(), CheckoutBuilder::new().force())
    # worktree verified clean in step 5 -> force is safe; brings worktree+index to onto's tree.
    on Err(Conflict) -> restore_start_failure(&repo, &state, original_tip); CheckoutConflict("cannot
        rebase: local changes would be overwritten. Commit or discard them first.")
    on Err(e)        -> restore_start_failure(&repo, &state, original_tip); e.into()
return drive(&repo, &mut state, &sig)            # §2.5 drive loop
```

`restore_start_failure(repo, state, original_tip)`: best-effort, guarantee state Clean —
`repo.cleanup_state()`; `repo.set_head("refs/heads/<head_name>")`; `checkout_tree(original_tip,
force)`; `remove_state(repo)`. Because the worktree was verified clean, the only START mutation that
can fail is the initial detach-checkout, which fails atomically before any commit is rewritten.

### 2.5 The drive loop, `commit_current_op`, `finish_interactive`

```text
fn drive(repo, state, sig) -> Result<RebaseOutcome, AppError>:
    loop:
        if state.cursor >= state.todos.len():
            return finish_interactive(repo, state)
        op = state.todos[state.cursor].clone()
        match op.action:
            Drop:
                state.cursor += 1; write_state(repo, state); continue
            Pick | Reword | Squash | Fixup:
                pick = repo.find_commit(Oid::from_str(&op.oid)?)?
                # Materialize the pick onto HEAD (== current detached tip) INTO the worktree+index.
                match repo.cherrypick(&pick, None):
                    Err(e):
                        # Could not even apply (e.g. checkout conflict). Abort the whole rebase.
                        abort_restore(repo, state)          # §2.7 abort body
                        return Err(map_pick_err(e))
                    Ok(()): {}
                if repo.index()?.has_conflicts():
                    state.paused = true; write_state(repo, state)   # cursor stays at this op
                    paths = list_conflicts(workdir)?.map(|c| c.path)
                    return Ok(RebaseOutcome::Conflicts {
                        paths, current_step: state.committed + 1, total_steps: effective_total(state) })
                commit_current_op(repo, state, &op, &pick, sig)?    # clean apply -> commit
                # commit_current_op advances cursor + committed + writes state + set_head_detached
                continue
```

`commit_current_op(repo, state, op, pick, committer)` — reads the RESOLVED repo index (works for both
the clean path and Continue after resolution):

```text
tree = repo.find_tree(repo.index()?.write_tree_to(repo)?)?     # resolved/clean result tree
head = repo.head()?.peel_to_commit()?                           # current detached tip (= "last")
# Parent + author + message depend on the action:
match op.action:
    Pick | Reword:
        parent = head
        author = pick.author().to_owned()                       # PRESERVED (name/email/author-time)
        message = match Pick   -> pick.message()
                        Reword -> op.new_message (required, §2.6)
    Squash | Fixup:
        parent = head.parent(0)?                                 # replace `head`, keep ITS parent
        author = head.author().to_owned()                        # base pick's author (matches git)
        message = match Fixup  -> head.message()                 # DISCARD op's message
                        Squash -> op.new_message
                                  .unwrap_or_else(|| concat(head.message(), pick.message()))
                                  # concat = "<head msg>\n\n<pick msg>"
# Empty-drop guard (§0 #6): a pick/squash producing no net change is dropped.
if tree.id() == parent.tree_id():
    repo.cleanup_state()?                                        # clear CHERRY_PICK_HEAD
    # For squash/fixup an "empty" combine means the fixup added nothing; keep `head` as-is.
    state.cursor += 1; write_state(repo, state); return Ok(())
new = repo.commit(None, &author, committer, &normalize(message), &tree, &[&parent])?  # dangling
repo.set_head_detached(new)?                                    # advance the detached tip
repo.cleanup_state()?                                           # remove CHERRY_PICK_HEAD -> worktree/index intact
state.committed += 1
state.cursor += 1
state.paused = false
write_state(repo, state)
Ok(())
```

`normalize` = the shared CRLF/CR→`\n` + trim + single trailing `\n` recipe (cherrypick.rs
`normalize_message`, commit.rs). `map_pick_err(e)` = `Conflict → CheckoutConflict("cannot rebase:
local changes would be overwritten…")`, else `e.into()`.

`finish_interactive(repo, state)`:
```text
final_tip = repo.head()?.peel_to_commit()?.id()               # detached tip after all ops
repo.reference(&format!("refs/heads/{}", state.head_name), final_tip, /*force*/ true,
               "rebase -i (finish)")?                          # move the branch ref
repo.set_head(&format!("refs/heads/{}", state.head_name))?     # re-attach HEAD
repo.checkout_head(Some(CheckoutBuilder::new().force()))?       # worktree already matches; ensure
remove_state(repo)
Ok(RebaseOutcome::Rebased { branch: state.head_name.clone(), head: final_tip.hex(),
                            steps: state.committed })
```

### 2.6 `validate_todos` (locked)

Reject BEFORE mutation (all → `AppError::Git(<msg>)`):
- Empty (`todos.is_empty()`) or all-`Drop` → `"nothing to rebase: the plan drops every commit"`.
- The FIRST non-`Drop` op MUST be `Pick` or `Reword` → else `"a squash/fixup must follow a pick"`
  (squash/fixup need a predecessor commit to combine into).
- Every `Reword` MUST have a non-empty `new_message` → `"reword requires a message"`.
- Every `oid` must parse (`Oid::from_str`) and resolve (`find_commit`) → `"invalid commit id"` /
  propagate. (Range/ancestry validation is best-effort; a bad oid surfaces at apply time as `Git`.)
- Duplicate oids are allowed (a commit may legitimately appear once; the editor never duplicates).

### 2.7 Continue / Skip / Abort (interactive engine)

```rust
/// Blocking. Resumes a paused interactive rebase: commits the resolved current
/// op from the index, then replays on. Reused via rebase::rebase_continue (§3).
pub fn interactive_continue(workdir: &Path) -> Result<RebaseOutcome, AppError>;
/// Blocking. Drops the current (conflicted or not) op and replays on.
pub fn interactive_skip(workdir: &Path) -> Result<RebaseOutcome, AppError>;
/// Blocking. Aborts: re-attach HEAD to the original branch (its ref never moved),
/// restore the worktree to the original tip, remove .git/bonsai-rebase/.
pub fn interactive_abort(workdir: &Path) -> Result<(), AppError>;
```

`interactive_continue`:
```text
repo = open_workdir_repo(workdir)
state = read_state(repo)?                          # missing -> caller (rebase.rs) never delegated here
if repo.index()?.has_conflicts():
    n = repo.index()?.conflicts()?.count()
    return UnresolvedConflicts("cannot continue: <n> unresolved conflict(s) remain")
sig = resolve_signature(...)?
op = state.todos[state.cursor].clone()             # the paused op
pick = repo.find_commit(Oid::from_str(&op.oid)?)?
commit_current_op(repo, &mut state, &op, &pick, &sig)?   # HARD error -> Err, leave state intact (§P3d-3.9)
return drive(repo, &mut state, &sig)
```

`interactive_skip` (lightweight discard, mirrors plain rebase_skip’s corrected recipe §P3d-11.11b):
```text
repo = open_workdir_repo(workdir)
state = read_state(repo)?
sig = resolve_signature(...)?
head_tree = repo.head()?.peel_to_commit()?.tree()?
idx = repo.index()?; idx.read_tree(&head_tree)?; idx.write()?        # clear conflict stages
checkout_index(force) so the worktree drops markers
repo.cleanup_state()?                                                # clear CHERRY_PICK_HEAD
state.cursor += 1; state.paused = false; write_state(repo, &state)
return drive(repo, &mut state, &sig)
```

`interactive_abort`:
```text
repo = open_workdir_repo(workdir)
state = read_state(repo)?
repo.cleanup_state().ok()                                            # clear any CHERRY_PICK_HEAD
repo.set_head(&format!("refs/heads/{}", state.head_name))?          # branch ref never moved
orig = repo.find_commit(Oid::from_str(&state.original_tip)?)?
repo.checkout_tree(orig.as_object(), CheckoutBuilder::new().force())?
let mut idx = repo.index()?; idx.read_tree(&orig.tree()?)?; idx.write()?
remove_state(repo)
Ok(())
```

Partial-safety (locked, = P3d §3.9): a START failure fully restores Clean (§2.4). A
CONTINUE/SKIP hard error **returns the error and leaves the on-disk state intact** (no abort, no
`remove_state`) — the user retries or explicitly Aborts. No engine handle survives across IPC calls
(each call re-reads + rewrites the JSON).

---

## 3. `rebase.rs` delegation (extend — no behavior change to the plain path)

`rebase::rebase_continue`, `rebase_skip`, `rebase_abort` each gain a FIRST branch:

```rust
pub fn rebase_continue(workdir: &Path) -> Result<RebaseOutcome, AppError> {
    let repo = open_workdir_repo(workdir)?;
    if crate::git::rebase_interactive::interactive_in_progress(&repo) {
        return crate::git::rebase_interactive::interactive_continue(workdir);
    }
    // ...existing plain path unchanged (re-opens repo as today)...
}
// identical guard at the top of rebase_skip -> interactive_skip
// identical guard at the top of rebase_abort -> interactive_abort
```

`start_interactive_rebase` has NO plain-rebase equivalent — it is its own command (§7). The plain
`rebase_branch` is untouched. This keeps the OpBanner rebase branch and `handleRebase*` handlers
working verbatim for both plain and interactive rebases.

Register `pub mod rebase_interactive;` in `git/mod.rs`.

---

## 4. `opstate.rs` (extend — wire UNCHANGED)

`read_op_state` gains a probe BEFORE the `repo.state()` switch:

```text
pub fn read_op_state(workdir) -> Result<RepoOpState, AppError>:
    let repo = open_workdir_repo(workdir)?;      # (mut as today)
    # NEW: interactive rebase wins over any transient CherryPick state.
    if rebase_interactive::interactive_in_progress(&repo):
        if let Ok(s) = rebase_interactive::read_state(&repo):
            return Ok(RepoOpState::Rebase {
                head_name: Some(s.head_name),
                onto: Some(s.onto),
                current_step: s.committed + 1,           # 1-based "current step"
                total_steps: rebase_interactive::effective_total(&s),
            });
        # corrupt/missing state -> fall through to the normal switch (belt-and-suspenders)
    match repo.state() { ...unchanged... }
```

`effective_total`/`read_state` are exposed `pub(crate)` from `rebase_interactive.rs`. No TS change,
no new `RepoOpState` variant — the OpBanner renders `Rebasing <head_name>` / `step c/t` from the
existing `{kind:'rebase', headName, onto, currentStep, totalSteps}` shape.

---

## 5. Error mapping (NO new `error.rs` variants)

| Situation | Variant | TS kind |
|---|---|---|
| interactive already in progress, or git-native op in progress (start) | `OperationInProgress` | `operationInProgress` |
| continue/skip/abort with no rebase | `NoOperationInProgress` | `noOperationInProgress` |
| continue while index still conflicted | `UnresolvedConflicts` | `unresolvedConflicts` |
| detach-checkout would overwrite (rare, worktree verified clean) | `CheckoutConflict` | `checkoutConflict` |
| git identity unset | `ConfigMissing` | `configMissing` |
| unborn / detached / dirty / invalid oid / bad plan / not-an-ancestor / corrupt state / any git2 error | `Git` | `git` |
| nothing open | `NoRepo` | `noRepo` |

If a genuinely new case appears, STOP and flag the orchestrator before adding a variant.

---

## 6. (reserved)

---

## 7. Command surface — interactive rebase (`commands.rs` + `lib.rs`)

Standard template (`pub async fn NAME(state, repo_id, …) -> Result<T, AppError>` + runtime-free
`NAME_inner` + `spawn_blocking(move || core_fn(&path, …))`, path via `repo_path(state, &repo_id)?`).
No events, no channels.

| Command (snake) | IPC method (camel) | Args | Returns | Error kinds |
|---|---|---|---|---|
| `get_interactive_plan` | `getInteractivePlan` | `repoId, baseOid` | `RebaseTodoOp[]` | `git \| noRepo` |
| `start_interactive_rebase` | `startInteractiveRebase` | `repoId, ontoOid, todos` | `RebaseOutcome` | `operationInProgress \| checkoutConflict \| configMissing \| git \| noRepo` |

`rebase_continue` / `rebase_skip` / `rebase_abort` commands are **already registered** (P3d) and gain
the interactive path purely via the core delegation (§3) — no command-layer change beyond confirming
they still compile against the extended core signatures (unchanged signatures).

`todos` deserializes `Vec<RebaseTodoOp>` (Tauri passes the camelCase array). Tauri invoke:
`invoke('start_interactive_rebase', { repoId, ontoOid, todos })`,
`invoke('get_interactive_plan', { repoId, baseOid })`.

Extend the `commands.rs` test module with `interactive_rebase_commands_require_an_open_repo` (shape of
`rebase_commands_require_an_open_repo`): both `*_inner` return `AppError::NoRepo` with no repo open.

### 7.1 TypeScript wire types (`src/ipc/types.ts`)

```ts
export type RebaseAction = 'pick' | 'reword' | 'squash' | 'fixup' | 'drop';

export interface RebaseTodoOp {
  oid: string;
  action: RebaseAction;
  newMessage: string | null;   // required for reword; optional for squash; null otherwise
}
```

`RebaseOutcome` already exists (P3d §7.1) — reused. `IpcApi` additions:

```ts
/** Default todo list (all `pick`, oldest-first) for base..HEAD, seeding the plan
 *  editor. Rejects git | noRepo. */
getInteractivePlan(repoId: string, baseOid: string): Promise<RebaseTodoOp[]>;
/** Start an interactive rebase of the current branch onto `ontoOid`, replaying
 *  `todos` in order. Rejects operationInProgress | checkoutConflict |
 *  configMissing | git | noRepo. */
startInteractiveRebase(repoId: string, ontoOid: string, todos: RebaseTodoOp[]): Promise<RebaseOutcome>;
```

`src/ipc/tauri.ts`: two invoke wrappers; import `RebaseTodoOp`/`RebaseAction`; re-export both from
`src/ipc/index.ts`. Continue/Skip/Abort wrappers are unchanged (they already exist).

### 7.2 Mock (`src/ipc/mock.ts`) — stateful interactive-rebase twin

Extend the existing per-repo mock state + the `?op=rebase` machinery (P3d §7.2). Add module-level
interactive state mirroring `InteractiveState` (`todos`, `cursor`, `committed`, `headName`, `onto`,
`paused`). Behaviors:

- `getInteractivePlan(baseOid)` → return a canned all-`pick` list of the top N (e.g. 3) mock commits
  above `baseOid` (oldest-first): `[{oid, action:'pick', newMessage:null}, …]`. If `baseOid` matches
  HEAD → reject `git` ("nothing to rebase").
- `startInteractiveRebase(ontoOid, todos)` → reject `operationInProgress` if `opState.kind !== 'none'`.
  Else **apply the plan to the mock commit list deterministically** so the harness shows a real
  result: drop `drop` rows; reorder per array order; combine `squash`/`fixup` into the preceding row
  (summary = squash uses `newMessage` first line, fixup keeps the predecessor's summary); apply
  `reword`’s `newMessage` first line as the new summary. **Conflict-demo trigger**: if any op’s oid
  is the designated conflicting fixture oid (document a constant, reuse the `?op=rebase` seed), set
  `opState = { kind:'rebase', headName, onto:ontoOid, currentStep:1, totalSteps }`, seed `conflicts`
  + `status.conflicted` + `conflictTexts` (reuse the merge-conflict fixture), persist the pending
  plan/cursor, and resolve `{ kind:'conflicts', paths:['src/auth.ts'], currentStep:1, totalSteps }`.
  Otherwise finish immediately: `mockHeadOid = randomOid()`, replace the top rows with the rewritten
  commits, resolve `{ kind:'rebased', branch: headName, head: mockHeadOid, steps }`.
- `rebaseContinue()` / `rebaseSkip()` / `rebaseAbort()` are **reused verbatim** — extend their
  existing `?op=rebase` bodies so that, when an interactive plan is pending, Continue advances the
  cursor (finishing the remaining clean ops and prepending the rewritten commits), Skip drops the
  current op then finishes, Abort restores the original commit list + clears `opState`. `getOpState`
  already returns `opState` and the OpBanner renders unchanged.

The mock must let the harness reach: seed plan → edit rows → Start clean (commits rewritten) AND
Start conflicting → OpBanner → resolve → Continue → done, entirely in the browser.

---

## 8. Frontend — the rebase-plan editor

### 8.1 `RebasePlanEditor.tsx` (new)

A modal/panel that opens from a branch/commit context action, lists the commits `base..HEAD`
(oldest-first, matching execution order), and collects the plan.

```ts
export interface RebasePlanEditorProps {
  open: boolean;
  /** Human label for the base the plan replays onto (short oid or ref name). */
  ontoLabel: string;
  ontoOid: string;
  /** Seed rows from getInteractivePlan (all `pick`, oldest-first). */
  initialTodos: RebaseTodoOp[];
  /** Per-row commit summaries for display (parallel to initialTodos by oid). */
  summaries: Record<string, string>;   // oid -> first line
  mutating: boolean;
  onCancel(): void;
  /** Fired with the FINAL edited plan (order + actions + messages). */
  onStart(todos: RebaseTodoOp[]): void;
}
```

Row UI (one per todo, in list order = execution order):
- **Up / Down** buttons (OPEN #3) to reorder (disabled at the ends). Reorder mutates local state
  order.
- **Action `<select>`**: pick / reword / squash / fixup / drop.
- **Commit label**: short oid + `summaries[oid]` (struck-through when action === 'drop').
- **Inline message editor** (a `<textarea>`): shown only when action is `reword` or `squash`.
  - reword: prefilled with the commit’s current summary/message; REQUIRED (Start disabled while any
    reword row has an empty message).
  - squash: prefilled with the concatenation of the predecessor’s + this commit’s messages
    (optional; empty → backend default concat).
- A validation banner mirrors `validate_todos` (§2.6): the first non-drop row must be pick/reword;
  Start is disabled otherwise with an inline hint.
- Footer: **[Start rebase]** (`btn-primary`, disabled while `mutating` or invalid) → `onStart(todos)`;
  **[Cancel]** (`btn-secondary`).

The editor is presentation-only; it holds local draft state and emits the final `todos` array. No
IPC inside the component.

### 8.2 Entry points + `RepoWorkspace.tsx` wiring

- **Commit context menu** (`commitMenuItems(oid)`, RepoWorkspace.tsx:2203): add
  **"Interactive rebase from here…"**, gated exactly like the existing cherry-pick item (attached
  born HEAD, `!mutating && !opActive`, and the target commit is an ancestor of HEAD — it becomes the
  `onto` base). On select → `openRebasePlan({ ontoOid: oid, ontoLabel: shortOid })`.
- **Branch/remote context menu** (`branchMenuItems`): add **"Rebase onto… (interactive)"** on
  non-current local + remote rows — the picked branch’s tip is the `onto`. Gated like the existing
  `⤵` plain-rebase affordance (§P3d-8.6). On select → `openRebasePlan({ ontoOid: branchTip,
  ontoLabel: name })`.
- `openRebasePlan(target)` → `setMutating(true)` guard off; fetch the seed via
  `ipc.getInteractivePlan(repoId, target.ontoOid)` + build the `summaries` map from the graph nodes
  (each node already carries its summary) → `setRebasePlan({ …target, initialTodos, summaries })`.
  On error → error toast, no editor.
- **`handleStartInteractiveRebase(ontoOid, todos)`** (standard `setMutating → try ipc → await
  refreshAll() → toast → finally`): call `ipc.startInteractiveRebase(repoId, ontoOid, todos)`.
  Outcome → toast: `rebased` → success `Rebased onto <ontoLabel> (<steps> commit(s))`; `conflicts` →
  info `Rebase paused at step <currentStep>/<totalSteps>: <n> conflict(s) to resolve`. AppError →
  sticky error toast. Close the editor on any resolved outcome; `await refreshAll()`.
- **Continue / Skip / Abort during an interactive pause** reuse the EXISTING OpBanner + `handleRebase
  {Continue,Skip,Abort}` + the generalized Abort ConfirmDialog (P3d §8.4) verbatim — the op is a
  `RepoOpState::Rebase`, so the banner, gating, and abort dialog copy already apply. **No OpBanner
  edit.** (The abort dialog body "restores your branch and working tree to their pre-rebase state" is
  accurate for the interactive engine too.)
- **Op-active gating** (P3d §8.5) is inherited unchanged: while the interactive rebase is paused,
  checkout/create/delete/merge/rebase/pull/push are disabled, plain commit is blocked, Fetch +
  non-conflicted stage/unstage stay enabled.

---

# PART B — BLAME + FILE HISTORY

## 9. Core — `blame.rs`

### 9.1 `blame_file`

```rust
/// One blamed line (contract §9). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlameLine {
    pub oid: String,          // 40-hex of the commit that last touched this line
    pub author_name: String,  // lossy UTF-8
    pub author_email: String,
    pub author_ts: i64,       // author time, seconds since epoch (UTC)
    pub summary: String,      // first line of that commit's message (gutter hover)
    pub orig_line_no: u32,    // 1-based line number in the introducing commit
    pub final_line_no: u32,   // 1-based line number in the blamed version
    pub line_text: String,    // content w/o trailing newline, lossy UTF-8
}

/// Blocking. Per-line blame of `path` as of `at_oid` (None -> HEAD, OPEN #8).
/// Rejects paths that traverse (`validate_rel_path`); caps at MAX_BLAME_LINES.
pub fn blame_file(workdir: &Path, path: &str, at_oid: Option<&str>)
    -> Result<Vec<BlameLine>, AppError>;
```

`pub const MAX_BLAME_LINES: usize = 50_000;`

Flow:
1. `validate_rel_path(path)?` (reuse stage.rs; `..`/abs/backslash → `AppError::Other`).
2. `open_workdir_repo(workdir)`.
3. `let mut opts = git2::BlameOptions::new();` (rename/copy tracking OFF for v1). If `at_oid` is
   `Some(o)` → `opts.newest_commit(Oid::from_str(o).map_err(|_| Git("invalid commit id"))?);`
4. `let blame = repo.blame_file(Path::new(path), Some(&mut opts))?;` (unknown path / never-committed
   → git2 error → `Git`).
5. Read the file content for the blamed version:
   - `at_oid` None → the HEAD blob at `path`: `repo.head()?.peel_to_commit()?.tree()?.get_path(
     Path::new(path))?.to_object(&repo)?.as_blob()` content (NOT the worktree — OPEN #8).
   - `at_oid` Some → the same, from that commit’s tree.
   - Binary blob (`blob.is_binary()`) → `Git("cannot blame a binary file")`.
6. Split content on `\n` (strip a trailing `\r` per line, lossy UTF-8). Count > `MAX_BLAME_LINES` →
   `Git("file too large to blame (> 50000 lines)")`.
7. Iterate `blame.iter()` hunks; per hunk of `n` lines starting at `final_start_line` /
   `orig_start_line`, emit one `BlameLine` per line, resolving the commit once per hunk
   (`hunk.final_commit_id()`), caching `Oid -> (author fields, summary)` in a `HashMap` to avoid
   O(lines) `find_commit`. `line_text` = the corresponding content line. `final_line_no` =
   `final_start_line + i`; `orig_line_no` = `orig_start_line + i` (1-based).

### 9.2 `file_history`

```rust
/// One commit that touched a file (contract §9). Serialize camelCase.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHistoryEntry {
    pub oid: String,
    pub summary: String,
    pub author_name: String,
    pub author_email: String,
    pub author_ts: i64,
}

/// Blocking. Commits that modified `path`, newest-first, best-effort following a
/// single rename (OPEN #10). `limit` caps the result (0 -> MAX_HISTORY).
pub fn file_history(workdir: &Path, path: &str, limit: u32)
    -> Result<Vec<FileHistoryEntry>, AppError>;
```

`pub const MAX_HISTORY: usize = 1000;`

Flow: `validate_rel_path`; `open_workdir_repo`; `revwalk` from HEAD with
`TOPOLOGICAL | TIME` sort (matches the graph walk). Track `current_path` (starts = `path`). For each
commit `c` (until `limit`/`MAX_HISTORY`):
```text
parent = c.parent(0).ok()                       # None for the root commit
old_tree = parent.map(|p| p.tree())             # None -> empty tree
diff = repo.diff_tree_to_tree(old_tree, Some(&c.tree()?),
          DiffOptions::new().pathspec(current_path).... )
enable diff.find_similar(find_renames) so a rename delta is detected
touched = diff.deltas().any(|d| d.new_file().path() == current_path
                             || d.old_file().path() == current_path)
if touched:
    push FileHistoryEntry { oid, summary=first line, author fields }
    # follow a rename: if a delta renamed current_path, retarget for older commits
    if let Some(d) = rename delta where d.new_file().path() == current_path:
        current_path = d.old_file().path()
```
Stop at `limit`. Root commit’s add counts as a touch. If `path` is unknown at HEAD → return `[]`
(empty history, not an error) — the frontend shows "No history".

Rename-follow is best-effort (OPEN #10): the oracle fixture uses one mid-history rename and asserts
oids equal `git log --follow --oneline -- <path>`; if parity proves fragile, degrade to no-follow +
`git log --oneline -- <path>` and note it in the test.

### 9.3 Unit tests (in `blame.rs #[cfg(test)]`)

`wire_shapes_are_camel_case_tagged` for `BlameLine` + `FileHistoryEntry` (assert exact JSON keys:
`oid`, `authorName`, `authorEmail`, `authorTs`, `summary`, `origLineNo`, `finalLineNo`, `lineText`).
`blame_rejects_bad_path` (`..` → `Other`). `history_empty_for_unknown_path` → `[]`.

---

## 10. Command surface — blame + history (`commands.rs` + `lib.rs`)

| Command (snake) | IPC method (camel) | Args | Returns | Error kinds |
|---|---|---|---|---|
| `blame_file` | `blameFile` | `repoId, path, atOid` | `BlameLine[]` | `other \| git \| noRepo` |
| `file_history` | `fileHistory` | `repoId, path, limit` | `FileHistoryEntry[]` | `other \| git \| noRepo` |

`atOid` is `Option<String>` (camelCase key `atOid`, omitted/`null` → None). Standard `_inner` +
`spawn_blocking` template. Register both in `lib.rs`. No events, no channels.

### 10.1 TypeScript wire types + `IpcApi` (`types.ts`, `tauri.ts`, `index.ts`)

```ts
export interface BlameLine {
  oid: string;
  authorName: string;
  authorEmail: string;
  authorTs: number;
  summary: string;
  origLineNo: number;
  finalLineNo: number;
  lineText: string;
}
export interface FileHistoryEntry {
  oid: string;
  summary: string;
  authorName: string;
  authorEmail: string;
  authorTs: number;
}
// IpcApi:
blameFile(repoId: string, path: string, atOid: string | null): Promise<BlameLine[]>;
fileHistory(repoId: string, path: string, limit: number): Promise<FileHistoryEntry[]>;
```
`tauri.ts`: `invoke('blame_file', { repoId, path, atOid })`, `invoke('file_history', { repoId, path,
limit })`. Re-export the two interfaces from `index.ts`.

### 10.2 Mock (`src/ipc/mock.ts`)

- `blameFile(path, atOid)` → a canned multi-author `BlameLine[]` for a designated fixture path (e.g.
  `src/app.ts`): a handful of lines attributed to 2–3 fixture commits already present in
  `mockCommits` (reuse their oids/authors/summaries so a click can resolve to a real graph node).
  Unknown path → reject `git`.
- `fileHistory(path, limit)` → a canned `FileHistoryEntry[]` referencing existing mock commit oids,
  newest-first, capped by `limit`. Unknown path → `[]`.

---

## 11. Frontend — Blame view + File history

Reuse the DiffOverlay full-panel overlay lifecycle (`RepoWorkspace.tsx` `diffSlot`/`overlayMeta`
pattern) and the commit-selection-by-oid remap already in `refetchGraph` (RepoWorkspace.tsx:534-551).

### 11.1 `BlameView.tsx` (new)

Presentation-only, given `lines: BlameLine[]`, `path`, `mutating`, `onClose`, and
`onRevealCommit(oid: string): void`. Renders a scrollable list; each row = a left **gutter**
(short-oid pill + author + relative date, styled like the graph author cells) + the `lineText` in a
monospace column with `finalLineNo`. **Grouping:** consecutive lines with the same `oid` collapse the
gutter to a single labelled block (git-blame look). Clicking a gutter block → `onRevealCommit(oid)`.

### 11.2 `FileHistoryView.tsx` (new)

Given `entries: FileHistoryEntry[]`, `path`, `onClose`, `onRevealCommit(oid)`,
`onShowCommitDiff(oid)`. A list of commit rows (short-oid, summary, author, date). Row click →
`onRevealCommit(oid)`; a secondary action → `onShowCommitDiff(oid)` (reuse the existing commit-diff
overlay path). Empty `entries` → "No history for this file".

### 11.3 Entry points + `RepoWorkspace.tsx` wiring

- **Diff file rows** (`DiffFileTree.tsx` / `DiffBrowser.tsx`) and **StatusPanel rows** gain a context
  action (right-click menu item or hover button) **"Blame"** and **"File history"**, threaded via a
  new `onBlame(path)` / `onFileHistory(path)` prop up to `RepoWorkspace`.
- `RepoWorkspace` owns `blame: { path, lines } | null` and `history: { path, entries } | null` state.
  - `handleBlame(path)` → `ipc.blameFile(repoId, path, selectedCommitOid ?? null)` → set state (error
    → toast). When a commit is selected in the graph, blame as of that commit; else HEAD (`null`).
  - `handleFileHistory(path)` → `ipc.fileHistory(repoId, path, MAX_HISTORY_UI /* e.g. 200 */)`.
- **`onRevealCommit(oid)`** → a new `revealCommitByOid(oid)` helper: find the graph node index by
  oid (`graph.nodes.findIndex(n => n.id === oid)`), `setSelectedIndex(idx)` when found, and reveal
  the row. **Requires a small graph API addition** — a `revealRow(index)` imperative handle (or a
  `revealIndex` prop) on the commit-graph canvas component to scroll a row into the virtualized
  viewport. If found in a collapsed/off-screen region, scroll; if the oid is not in the current walk,
  toast info `Commit not in the current view`. (Flag: confirm the graph component exposes or can add
  `revealRow`; the selection-by-oid remap machinery already exists.)
- Blame/history open as overlays layered like the diff overlay; `onClose` clears the state. Only one
  of blame / history / diff overlay is visible at a time (share the overlay slot or stack with a
  clear precedence — recommend a single `rightOverlay` discriminated state).

---

## 12. Sub-increment breakdown (each = one fresh-context `senior-dev` pass)

- **P23a — Interactive-rebase backend engine + tests.**
  - Rust: new `rebase_interactive.rs` (todo types, `InteractiveState` + `.git/bonsai-rebase/`
    read/write, `get_interactive_plan`, `start_interactive_rebase`, drive loop + `commit_current_op`
    + `finish_interactive`, `interactive_continue` / `interactive_skip` / `interactive_abort`,
    `validate_todos`); `rebase.rs` delegation branch; `opstate.rs` probe; `mod.rs` registration;
    module unit tests (wire shapes + preconditions + `validate_todos`).
  - Tests: `crates/bonsai-core/tests/rebase_interactive_cli.rs` (§13.1).
- **P23b — Interactive-rebase IPC + plan-editor UI.**
  - IPC triple: `RebaseAction`/`RebaseTodoOp` types, `getInteractivePlan` + `startInteractiveRebase`
    in `types.ts`/`tauri.ts`/`index.ts`, stateful mock (§7.2).
  - Commands: `get_interactive_plan`, `start_interactive_rebase` (+ `lib.rs`); confirm reused
    continue/skip/abort compile.
  - UI: `RebasePlanEditor.tsx`; `RepoWorkspace` entry points + `handleStartInteractiveRebase` +
    `openRebasePlan`; reuse OpBanner/abort-dialog verbatim (§8.2).
- **P23c — Blame + file-history backend + tests.**
  - Rust: new `blame.rs` (`BlameLine`/`blame_file`, `FileHistoryEntry`/`file_history`); `mod.rs`;
    unit tests (§9.3).
  - Tests: `crates/bonsai-core/tests/blame_cli.rs` (§13.2).
- **P23d — Blame + file-history IPC + UI.**
  - IPC triple: `BlameLine`/`FileHistoryEntry` types, `blameFile` + `fileHistory`; mock (§10.2).
  - Commands: `blame_file`, `file_history` (+ `lib.rs`).
  - UI: `BlameView.tsx`, `FileHistoryView.tsx`; diff/status file-row entry points; `revealCommitByOid`
    + the graph `revealRow` handle.

Commit each approved sub-increment as `wip(P23a): …` … `wip(P23d): …` (orchestrator owns commits).

---

## 13. Tests (AI gate)

Conventions (all suites): scratch repos under `D:\Temp\bonsai-scratch`; `TMP`/`TEMP=D:\Temp` (USER
MANDATE); Bash uses forward-slash paths; `cargo test` + `clippy` run **sequentially** (target-dir
race); degrade gracefully (skip, not fail) when `git` is absent (like `p8_git_cli_autostash_ff_oracle`
/ the `require_git!` macro in `rebase_cli.rs`). **Commit-oid comparison rule (locked, = P3d §9):**
committer time = `now()`, so replayed commit OIDs differ from the twin — compare **tree oids,
author identity (name/email/author-time — preserved), messages, and parent topology**, plus final
HEAD tree oid, NOT commit oids.

### 13.1 `crates/bonsai-core/tests/rebase_interactive_cli.rs`

For a fixture history built by identical scripted setup on a Bonsai repo and a `git` twin, assert each
todo action produces the tree/topology matching the hand-run `git rebase -i` equivalent:

1. **Reorder.** 3 linear commits on `topic` touching disjoint files; plan swaps the top two. Twin:
   `git rebase -i` reordering the same two lines. Assert final HEAD tree identical; per-commit trees
   + preserved authors match in the new order; linear topology.
2. **Squash two into one.** Plan: `pick A`, `squash B` (`newMessage` = a combined message). Twin:
   `git rebase -i` `pick A` / `squash B` with the same final message. Assert the resulting single
   commit’s tree == twin’s, message byte-exact, parent == A’s parent, and the commit count dropped by
   one.
3. **Fixup.** Plan: `pick A`, `fixup B`. Assert combined tree == twin’s and the message == A’s
   message (B’s discarded); count dropped by one.
4. **Reword.** Plan: `pick A`, `reword B` (`newMessage`). Assert B’s new commit has the new message
   and the SAME tree as before; author preserved.
5. **Drop.** Plan drops the middle commit. Assert it is absent from the result and the surviving
   commits’ trees match the twin `git rebase -i` with a `drop` line; final HEAD tree matches.
6. **Conflict → resolve → continue.** An op conflicts on apply → `Conflicts { paths, currentStep,
   totalSteps }`, `read_op_state` returns `Rebase { head_name: Some("topic"), onto: Some(<40hex>),
   … }` (the §4 probe), the worktree carries `<<<<<<<`/`=======`/`>>>>>>>` markers, and
   `.git/bonsai-rebase/state.json` exists with `paused=true`. Resolve via `conflict::resolve_conflict`
   /`resolve_conflict_text`, then `rebase_continue()` (delegates to `interactive_continue`) →
   `Rebased`. Twin: `git rebase -i` hand-resolved + `--continue`. Assert final HEAD tree == twin’s;
   `repo.state()` Clean; `.git/bonsai-rebase/` gone. Also assert `UnresolvedConflicts` if
   `rebase_continue` runs with a conflict still present.
7. **Skip.** A multi-op plan where the first op conflicts; `rebase_skip()` drops it and completes the
   rest. Assert the skipped commit absent and remaining trees match `git rebase -i` with that op
   removed.
8. **Abort restores the original tip byte-identically.** From a paused conflict, `rebase_abort()` →
   `repo.state()` Clean, HEAD re-attached to `topic`, branch tip == `original_tip`,
   worktree/index byte-identical to pre-rebase, `.git/bonsai-rebase/` gone.
9. **Precondition matrix.** interactive-in-progress start → `OperationInProgress`; dirty
   worktree/index → `Git`; unborn → `Git`; detached → `Git`; bad plan (all drop / squash-first /
   reword-without-message) → `Git`; missing identity → `ConfigMissing` (assert state still Clean, no
   `.git/bonsai-rebase/`). `interactive_continue`/`skip`/`abort` with no rebase →
   `NoOperationInProgress`.
10. **Empty-pick drop.** A picked commit whose change is already present on `onto` → dropped from the
    result (§0 #6), count matches the twin.

Module unit tests (P23a) in `rebase_interactive.rs`: `wire_shapes_*` for `RebaseAction` (round-trips
`"pick"|"reword"|"squash"|"fixup"|"drop"`) and `RebaseTodoOp` (`{oid,action,newMessage}`);
`validate_todos` rejections; `get_interactive_plan` returns oldest-first all-pick for a linear range;
preconditions on a fresh repo.

### 13.2 `crates/bonsai-core/tests/blame_cli.rs`

1. **Blame matches `git blame --porcelain`.** A multi-author fixture file edited across several
   commits (fixed authors/dates so the twin is deterministic). Assert, per line, `blame_file`’s
   `oid` + `author_name`/`author_email` match the porcelain output’s commit + author for that line;
   `final_line_no` is 1-based contiguous; `line_text` matches the file content.
2. **Blame at an older commit** (`at_oid = Some(<older>)`) attributes lines as of that revision (fewer
   lines / different authors than HEAD).
3. **Blame errors:** binary file → `Git`; `..` path → `Other`; unknown path → `Git`.
4. **File history matches `git log --follow --oneline -- <path>`.** A fixture with several edits and
   ONE mid-history rename. Assert the returned oids (in order) equal the `--follow` oids (OPEN #10:
   degrade to non-follow `git log --oneline -- <path>` if parity is fragile — note which in the test).
5. **History `limit`** caps the result; `0` → up to `MAX_HISTORY`. Unknown path → `[]`.

### 13.3 Frontend AI gate (browser harness, `VITE_MOCK_IPC=1`)

- **Interactive rebase:** the commit/branch menu opens `RebasePlanEditor`; rows reorder via Up/Down;
  action dropdown switches pick/reword/squash/fixup/drop; reword/squash reveal the message textarea;
  Start with a clean plan rewrites the top commits in the graph; Start with the conflict-demo oid
  shows the OpBanner (`Rebasing …`, `step 1/N`, Continue disabled while a conflict remains); resolving
  via the shared conflict rows enables Continue; Continue clears the banner and prepends the rewritten
  commits; Skip completes without resolution; Abort shows the `Abort rebase?` dialog and restores.
  Plain (no `?op`) harness unchanged (regression).
- **Blame / history:** the diff/status file-row "Blame" opens `BlameView` (multi-author gutter,
  grouped blocks); clicking a gutter block selects + reveals that commit in the graph. "File history"
  opens `FileHistoryView`; a row selects + reveals the commit; empty path shows "No history".
- `pnpm build` + `tsc` clean; `src/ipc/mock.ts` compiles and implements every new method statefully.

---

## 14. Acceptance criteria — AI gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable, no network, no native window):**
- `cargo check` + `cargo clippy -- -D warnings` clean; `pnpm build` + `tsc` clean after every
  sub-increment.
- `crates/bonsai-core/tests/rebase_interactive_cli.rs` + `blame_cli.rs` green (degrade to git2-only
  when `git` absent); the byte-exact oracle assertions in §13.1/§13.2 pass.
- Module unit tests (§13.1/§13.3) pass.
- Browser-harness screenshots per §13.3 (plan editor, conflict pause + OpBanner reuse, blame gutter,
  file history).

**USER CHECKPOINT (native `pnpm tauri dev`, real scratch repo — never self-declared):**
1. **Drag/reorder feel** — reordering rows in the plan editor with Up/Down (and, if added later,
   drag) feels responsive; the summaries/message editors read correctly.
2. **Real-repo interactive rebase** — reorder + squash + fixup + reword + drop on a real topic
   branch; `git log` shows the rewritten history with preserved authors + new committer dates;
   `git status` clean.
3. **Conflicting interactive rebase** — the OpBanner shows `step n/m`; resolve via ours/theirs/
   hand-edit; Continue advances/completes; Skip drops the offending op; Abort restores the branch +
   worktree to the pre-rebase state; a git-native `git rebase --continue` is NOT needed (Bonsai owns
   the sequencer).
4. **Blame** — open Blame on a real multi-author file; per-line authors/oids look correct; clicking a
   line reveals that commit in the graph.
5. **File history** — open File history on a file (including one that was renamed); the list matches
   expectation; a row reveals the commit.

---

## 15. Notes / risks flagged to the orchestrator

- **`.git/bonsai-rebase/` is a Bonsai-proprietary sequencer.** A user who runs `git rebase` /
  `git cherry-pick` in a terminal WHILE a Bonsai interactive rebase is paused would create a
  git-native state alongside ours; opstate’s probe (§4) still reports our Rebase, and our
  continue/abort operate on our state. This is an accepted v1 edge (document in the banner tooltip if
  cheap). The Start guard (§2.4 step 3) prevents the reverse (starting ours over a git-native op).
- **`serde_json` is already a runtime dep** (Cargo.toml:12) — no Cargo change for the state file.
- **Graph `revealRow` API** (§11.3) is the one frontend piece that may need a new imperative handle
  on the canvas graph component; the selection-by-oid remap already exists. Confirm feasibility in
  P23d; if scrolling proves hard, v1 can select-without-scroll and flag.
- **Rename-follow parity** (§9.2 / OPEN #10) is the one blame/history spot with oracle risk; the
  fallback (no-follow) is specified.
- **squash default message concat** when `newMessage` is None is a convenience; the plan editor
  should pre-fill it so the backend default is rarely hit.
</content>
</invoke>
