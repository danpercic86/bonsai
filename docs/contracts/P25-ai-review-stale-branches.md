# P25 — AI review of worktree/branch (B1) + stale-branch cleanup (B4)

Roadmap Theme B, "cheap AI-automation wins" (items **B1** + **B4**). Both extend already-shipped
primitives, so this contract specifies **reuse, not reinvention**.

- **B1** adds two review *scopes* to the EXISTING `ai_explain::analyze_diff` Review path: the whole
  **working-tree** change set, and a **branch** (its diff vs the merge-base with a base branch). It
  reuses the existing `ai_analyze_diff` command, the Review prompt, the consent gate, `run_claude`,
  and the `AiAnalysis` result verbatim — B1 is **purely additive**: two new `AiDiffTarget` variants,
  two `build_payload` arms, one base-branch resolver, one new pure `diff.rs` multi-file collector,
  and one payload byte-cap. **NO new command, NO new result type.** WRITES NOTHING.
- **B4** is pure git2 (no AI, no consent gate) in a new `git/stale.rs`: a read-only classifier
  (`list_stale_branches`) and a confirm-gated batch deleter (`delete_branches`) with server-side
  safety refusal. Destructive → the UI requires explicit confirmation listing exactly what is
  deleted.

Rust owns ALL logic; React only renders and confirms.

Sub-increments (see §10): **P25a** B1 core + IPC (no new command) · **P25b** B1 UI ·
**P25c** B4 core (detect + batch-delete) + two commands + IPC triple · **P25d** B4 UI.

---

## 0. Invariants held

- git2 is blocking; every command wraps the core in `spawn_blocking` (same template as
  `ai_analyze_diff` / `delete_branch`), resolving `repo_path(state, &repo_id)?` first.
- Cores stay Tauri-free / runtime-free → directly unit-testable (same rule as `branches.rs`,
  `ai_summary.rs`).
- **No `error.rs` change, no new `AppError` variant.** Every failure maps to an existing kind (§8).
  The TS `AppError` union in `types.ts` is unchanged.
- **B1 is consent-gated**: reuses `ai_analyze_diff_inner`, which already enforces
  `ai_enabled && ai_consented` BEFORE `repo_path` (commands.rs:1364). **B4 is NOT AI** — pure git,
  no gate.
- serde is `rename_all = "camelCase"`; all TS wire types are camelCase.
- Commands do **NOT** emit `repo-changed`; the frontend refetches imperatively after every
  mutation. B4's `delete_branches` mutation triggers the watcher too; the sidebar's existing
  request-id guards absorb it.
- **Destructive op (B4 delete)** requires an explicit UI confirm listing the exact branch set, per
  the repo guardrails. The backend independently re-verifies safety (defense-in-depth, §4.3).

---

## OPEN DECISIONS (recommended default in brackets; contract proceeds on the default)

1. **B1 shape: new `AiDiffTarget` variants vs a new function.** [**New variants
   `Worktree` + `Branch { name, base }`** feeding the existing `analyze_diff` Review path.] The
   Review prompt, consent gate, `has_analyzable_content` guard, `render_file_diffs`, `run_claude`,
   and `AiAnalysis` are identical for every target; a separate function would duplicate all of that
   for zero benefit. The only genuinely new logic is diff-gathering (two `build_payload` arms) +
   base resolution — pure additions. Rejected alternative: a standalone `ai_review_branch` command.
2. **B1 branch base resolution.** [**explicit `base` → branch's configured upstream → `origin/HEAD`
   target → local `main` → local `master` → error**, §2.3.] The default matches the roadmap
   ("upstream, else the repo default branch"). The `base` field is optional so a future UI can
   override; v1 UI always sends `null` (auto).
3. **B1 worktree scope.** [**HEAD-tree vs working directory, index-aware, incl. untracked**
   (`diff_tree_to_workdir_with_index`, `include_untracked`).] This is the single net "everything
   since my last commit" change set — the exact thing a pre-commit review wants — in ONE git2 diff,
   collected in ONE pass. Rejected: unioning the separate staged + unstaged file lists (duplicates
   partially-staged files and needs N per-file round-trips).
4. **B1 payload size cap.** [**Reuse the existing `MAX_PAYLOAD_LINES` (6000) + `MAX_PAYLOAD_FILES`
   (300) render caps, PLUS a new `MAX_REVIEW_PAYLOAD_BYTES` = 256 KiB hard byte-cap** applied to the
   assembled `payload_text` in `analyze_diff`, truncating on a char boundary and appending a
   model-visible note, §2.4.] Belt-and-suspenders for pathological long-line diffs; harmless for the
   small commit/file targets (never triggers).
5. **B1 empty scope.** [**`AiFailed("no changes to analyze")`** before any CLI call, via the existing
   `has_analyzable_content` guard — consistent with the `Commit`/`WorkdirFile` targets. (The legacy
   `Staged` target keeps its `NothingToCommit`.)]
6. **B1 branch name accepts any ref.** [**`revparse_single(name).peel_to_commit()`** — accepts a
   local branch, a remote-tracking shorthand, or an oid. Upstream auto-resolution only applies when
   `name` resolves to a local branch; otherwise base falls through to the default-branch chain.]
7. **B4 "stale" definition.** [**merged into base OR upstream-gone**, §4.1.] `merged` = base contains
   every commit of the branch (`base_oid == tip || graph_descendant_of(base_oid, tip)`).
   `goneUpstream` = an upstream is *configured* (`branch.<name>.merge` set) but the remote-tracking
   ref is missing. A branch qualifying on either is listed; `reason` = `Merged` when merged, else
   `GoneUpstream`; both raw flags are also carried.
8. **B4 base for merged-detection.** [**explicit `base` → `origin/HEAD` target → local `main` →
   local `master` → current HEAD (attached) → error**, §4.1.] The CLI oracle pins `base` explicitly
   and compares to `git branch --merged <base>` (§9.2), so the default chain is UX-only, never
   test-load-bearing.
9. **B4 never-deletable set.** [**Exclude the base branch AND the current HEAD branch from
   `entries` entirely.**] `isCurrent` is retained on the wire (roadmap-requested) but is always
   `false` in returned entries; it exists so the UI can hard-disable a row if a race ever surfaces
   one, and the batch-delete's server-side refusal is the real guard.
10. **B4 batch delete uses a direct git2 delete, NOT `branches::delete_branch`.** [**`branch.delete()`
    directly**, gated on membership in a freshly-recomputed safe set + not-current + not-base, §4.3.]
    `branches::delete_branch` refuses anything not merged into **HEAD**; a branch merged into the
    **base** (e.g. `main`) but not into the currently-checked-out branch would be wrongly blocked.
    Since P25 has already verified merged-into-base OR gone-upstream, the direct delete is the
    correct primitive (gone-upstream branches are intentionally force-deleted; the UI confirm
    covers it).
11. **Deferred (per roadmap):** B1 auto/scheduled review (→ B5 scheduler); a base-override picker in
    the B1 branch-review UI; echoing the resolved base in the review panel title. B4 remote-branch
    pruning ON the remote (`git push --delete` / `remote prune`), reflog-based recovery/undo, and
    remote-tracking-ref cleanup. All out of scope for P25.

None of these block implementation; all defaults are safe.

---

## 1. Module boundaries & file responsibilities

| File | Responsibility | Increment |
|------|----------------|-----------|
| `crates/bonsai-core/src/git/diff.rs` (extend) | add `pub(crate) fn collect_file_diffs` — multi-file generalization of `collect_file_diff` (§2.1) | P25a |
| `crates/bonsai-core/src/git/ai_explain.rs` (extend) | two new `AiDiffTarget` variants; two `build_payload` arms; `gather_worktree`, `gather_branch`, `resolve_branch_base` helpers; `MAX_REVIEW_PAYLOAD_BYTES` byte-cap in `analyze_diff` (§2) | P25a |
| `crates/bonsai-core/src/git/stale.rs` (new) | `StaleReason`, `StaleBranch`, `StaleReport`, `BranchDeleteStatus`, `BranchDeleteResult`; `find_stale_branches`, `delete_branches` (§4). Register `pub mod stale;` in `git/mod.rs` | P25c |

Unchanged but REUSED verbatim: `ai::run_claude` + `ai::payload::render_file_diffs`; the existing
`ai_analyze_diff` command + `AiAnalysis`; `branches.rs` helpers (`open_repo_at`, ci-sort, upstream
reading); `repo::read_head_info`; the `REVIEW_SYSTEM_PROMPT`/`REVIEW_PROMPT` consts.

Command layer: `commands.rs` gains **two** commands for B4 (§5); **zero** for B1. `lib.rs` registers
both B4 commands in `generate_handler!`.

Frontend: IPC triple (`src/ipc/{types.ts,tauri.ts,mock.ts}`) — B1 extends the existing
`AiDiffTarget` union + `aiAnalyzeDiff` mock; B4 adds two methods + a stateful mock slice. UI:
B1 review actions wire into the existing `runAnalyze`/`AiOutputPanel`; B4 adds `StaleBranchesDialog`.

---

## 2. B1 — AI review of worktree / branch (P25a)

### 2.1 New pure collector — `diff.rs`

```rust
/// Walks a MULTI-FILE diff and collects one `FileDiff` (with hunks) per delta,
/// in delta order. The plural of `collect_file_diff`: each new file_cb starts a
/// new file (pushing the previous), hunk_cb/line_cb append to the current file,
/// and the per-file `MAX_FILE_DIFF_LINES` budget resets per file (an overflowing
/// file is flagged `too_large` with empty hunks, exactly like the singular fn).
/// Binary files come back with `binary:true`, empty hunks. Never fails on a
/// too-large file (all-or-nothing per file). Empty diff => empty Vec.
pub(crate) fn collect_file_diffs(diff: &git2::Diff) -> Result<Vec<FileDiff>, AppError>;
```

### 2.2 New `AiDiffTarget` variants + `build_payload` arms — `ai_explain.rs`

```rust
// Added to the existing #[serde(tag="kind", rename_all="camelCase")] enum
// (Deserialize only; command INPUT):
    /// The whole working-tree change set: HEAD tree vs working directory,
    /// index-aware, including untracked additions. The natural pre-commit
    /// Review target (B1).
    Worktree,
    /// A branch (or any ref/oid) vs the merge-base with `base`. `base=None`
    /// => auto-resolve (§2.3). The natural pre-push Review target (B1).
    Branch {
        name: String,
        #[serde(default)]
        base: Option<String>,
    },
```

`build_payload` gains two arms (both return `(prefix, Vec<FileDiff>)` like the existing arms):

- **`Worktree`** → `gather_worktree(workdir)`:
  1. `open_workdir_repo`; `head_tree(&repo)?` (None on unborn HEAD → empty-tree diff, all Added).
  2. `let mut opts = build_diff_options(&[], false);` then `opts.include_untracked(true).recurse_untracked_dirs(true);`
  3. `let mut diff = repo.diff_tree_to_workdir_with_index(head_tree.as_ref(), Some(&mut opts))?;`
  4. `apply_find_similar(&mut diff)?;` → `collect_file_diffs(&diff)?`.
  5. prefix = `""` (no header). Empty diff → empty Vec → `has_analyzable_content` false →
     `AiFailed("no changes to analyze")` in `analyze_diff` (§2 flow unchanged).

- **`Branch { name, base }`** → `gather_branch(workdir, name, base.as_deref())`:
  1. `open_workdir_repo`; resolve head: `repo.revparse_single(name)?.peel_to_commit()?` (bad ref →
     `Git`).
  2. `let (base_name, base_commit) = resolve_branch_base(&repo, name, base)?;` (§2.3).
  3. `let mb = repo.merge_base(base_commit.id(), head.id()).ok();` — `None` => unrelated histories:
     compare vs the empty tree and prepend an "unrelated histories" note to the prefix (mirrors
     `ai_summary`).
  4. `mb_tree` = mb's tree (or empty tree); `diff_tree_to_tree(Some(&mb_tree), Some(&head.tree()?), opts)`;
     `apply_find_similar` → `collect_file_diffs`.
  5. prefix = `format!("BRANCH {name} vs {base_name} (merge-base)\n\n")` (+ the unrelated note when
     applicable). Empty diff → `AiFailed("no changes to analyze")`.

### 2.3 Base resolution — `resolve_branch_base`

```rust
/// Resolves the comparison base for a branch review. Returns (shorthand, commit).
/// Precedence (OPEN #2): explicit `base` (revparse) → the branch's configured
/// upstream (only when `name` is a local branch) → `origin/HEAD` target →
/// local `main` → local `master` → Err(Git("cannot determine a base branch to
/// review against; specify one explicitly")).
fn resolve_branch_base<'r>(
    repo: &'r git2::Repository,
    name: &str,
    base: Option<&str>,
) -> Result<(String, git2::Commit<'r>), AppError>;
```

- explicit: `repo.revparse_single(b)?.peel_to_commit()?`, shorthand = `b`.
- upstream: `repo.find_branch(name, Local).ok().and_then(|br| br.upstream().ok())` → its shorthand +
  tip commit.
- `origin/HEAD`: `repo.find_reference("refs/remotes/origin/HEAD").ok()` → resolve symbolic → target
  commit; shorthand = that ref's shorthand (e.g. `origin/main`).
- `main` / `master`: `repo.find_branch("main"|"master", Local)`.

### 2.4 Payload byte-cap — `analyze_diff`

After assembling `payload_text` (step 3 of the existing flow), before `run_claude`:

```rust
pub const MAX_REVIEW_PAYLOAD_BYTES: usize = 256 * 1024;
// if payload_text.len() > MAX_REVIEW_PAYLOAD_BYTES: truncate to the largest char
// boundary <= the cap and append "\n... (payload truncated at 256 KiB for review) ...\n"
```

Applied universally (harmless for small targets; only large `Worktree`/`Branch` payloads trip it).
`run_claude` already streams arbitrarily large stdin without deadlock (tested >128 KiB).

### 2.5 Command surface — UNCHANGED

B1 reuses `ai_analyze_diff(repoId, target, mode)` verbatim (commands.rs:1341). No signature change.
The command's documented error surface simply grows by `git` (bad ref) for the `Branch` target —
already an existing kind. The frontend sends `mode: 'review'`.

### 2.6 TS wire (`types.ts`) — extend the existing union

```ts
export type AiDiffTarget =
  | { kind: 'commit'; oid: string }
  | { kind: 'workdirFile'; path: string; origPath: string | null; staged: boolean }
  | { kind: 'staged' }
  | { kind: 'worktree' }                                   // B1
  | { kind: 'branch'; name: string; base?: string | null }; // B1
```

`aiAnalyzeDiff` in `IpcApi`/`tauri.ts` is unchanged. `tauri.ts` already does
`invoke('ai_analyze_diff', { repoId, target, mode })` — the new variants flow through untouched.

---

## 3. B1 frontend (P25b)

Reuses the existing `runAnalyze(target, mode, title)` + `AiOutputPanel` plumbing (RepoWorkspace).

- **Review working tree** — an action near the status/commit area (e.g. a header button on the
  Changes panel, beside the existing review-staged affordance). Gated identically to the current AI
  review actions (AI availability/consent). Calls
  `runAnalyze({ kind: 'worktree' }, 'review', 'Review working tree')`.
- **Review branch** — an item on a branch's context menu in the sidebar
  (`branchMenuItems(name, kind)`). Calls
  `runAnalyze({ kind: 'branch', name }, 'review', \`Review branch ${name}\`)` (base auto). The panel
  shows the returned prose in `AiOutputPanel`; title carries the branch name.
- Errors surface in the existing `AiOutputPanel` error banner (`aiUnavailable`/`aiFailed`/`git`);
  `git` (bad ref / no base) reads e.g. "cannot determine a base branch…".

### 3.1 Mock (`mock.ts`) — extend `aiAnalyzeDiff`

Add `worktree` and `branch` prefixes + canned Review prose:
- `target.kind === 'worktree'` → prefix `Working tree: ` + review text (e.g. "Review: 3 files
  changed; the new error path in commands.rs lacks a test; otherwise LGTM.").
- `target.kind === 'branch'` → prefix `Branch ${target.name} vs main: ` + review text.
Keep the `AI_OFF` (`?ai=off`) → `aiFailed` path. No new mock method (reuses `aiAnalyzeDiff`).

---

## 4. B4 — stale-branch cleanup (P25c core)

### 4.1 Data + detection — `stale.rs`

```rust
/// Why a branch is safe to delete. Field-less enum → serializes to the bare
/// camelCase string ("merged" | "goneUpstream").
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum StaleReason { Merged, GoneUpstream }

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleBranch {
    pub name: String,
    /// 40-hex tip oid.
    pub tip: String,
    /// First line of the tip commit's message (lossy).
    pub last_commit_summary: String,
    /// Tip commit author name (lossy).
    pub last_commit_author: String,
    /// Tip committer time, epoch seconds.
    pub last_commit_time: i64,
    /// Primary reason: Merged when merged (even if also gone), else GoneUpstream.
    pub reason: StaleReason,
    /// Raw flags (a branch may be both).
    pub merged: bool,
    pub gone_upstream: bool,
    /// Configured upstream shorthand (e.g. "origin/feature"), if any — present
    /// even when gone.
    pub upstream: Option<String>,
    /// Ahead/behind the BASE (best-effort; None on any lookup error). ahead =
    /// commits on the branch not in base (0 when merged); behind = base commits
    /// not on the branch.
    pub ahead: Option<u32>,
    pub behind: Option<u32>,
    /// Always false in returned entries (the current branch is excluded, OPEN #9);
    /// defensive wire field.
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaleReport {
    /// Resolved base shorthand (e.g. "main" / "origin/main").
    pub base: String,
    /// 40-hex base commit oid.
    pub base_oid: String,
    /// Stale candidates, case-insensitively sorted by name. Excludes the base
    /// branch and the current HEAD branch.
    pub branches: Vec<StaleBranch>,
}

/// Blocking. Classifies local branches safe to delete against `base`
/// (`None` => auto-resolve, OPEN #8). Read-only; touches nothing. Errors:
/// `git` (bad base / bare / no resolvable base) | `noRepo` (command layer).
pub fn find_stale_branches(workdir: &Path, base: Option<&str>)
    -> Result<StaleReport, AppError>;
```

Algorithm (pseudocode):

```
repo = open_repo_at(workdir)
(base_name, base_commit) = resolve_stale_base(repo, base)?    // OPEN #8 precedence
base_oid = base_commit.id()
current = read_head_info(repo).branch_name   // Some(name) attached; None detached/unborn
cfg = repo.config()?

out = []
for (branch, _) in repo.branches(Local)?:
    name = branch.name()?      // skip non-UTF-8 with eprintln (as list_refs does)
    if name == base_name: continue                 // never the base
    if Some(name) == current: continue             // never current HEAD (OPEN #9)
    tip = branch.get().target()?                   // skip symbolic/targetless
    merged = (tip == base_oid) || repo.graph_descendant_of(base_oid, tip)?
    (upstream, gone) = upstream_state(repo, cfg, name, &branch)  // §4.2
    if !(merged || gone): continue
    (ahead, behind) = repo.graph_ahead_behind(tip, base_oid) best-effort -> (Option,Option)
    reason = if merged { Merged } else { GoneUpstream }
    commit = repo.find_commit(tip)?
    push StaleBranch { name, tip: tip.hex(), last_commit_* from commit,
                       reason, merged, gone_upstream: gone, upstream, ahead, behind,
                       is_current: false }
sort out by name (ci_cmp)
StaleReport { base: base_name, base_oid: base_oid.hex(), branches: out }
```

### 4.2 Upstream-gone detection — `upstream_state`

```
// configured iff `branch.<name>.merge` is set in config.
configured = cfg.get_string(&format!("branch.{name}.merge")).is_ok()
if !configured: return (None, gone=false)
match branch.upstream() {
    Ok(u)  => (Some(u.name shorthand), gone=false),   // tracking ref exists
    Err(_) => (Some(configured shorthand or "<remote>/<merge-branch>"), gone=true), // configured but missing
}
```

The shorthand when gone is reconstructed from `branch.<name>.remote` + the short of
`branch.<name>.merge` (strip `refs/heads/`); on any read hiccup, `upstream = None`, `gone` still
true. Distinguishes "configured but the remote deleted it" (gone) from "never had an upstream"
(`configured == false` → not gone).

### 4.3 Batch delete — `stale.rs`

```rust
/// Per-branch outcome. Field-less enum → bare camelCase string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BranchDeleteStatus {
    Deleted,
    SkippedCurrent,   // is the checked-out branch
    SkippedBase,      // is the resolved base branch
    SkippedNotStale,  // not in the freshly-recomputed safe set
    SkippedNotFound,  // no such local branch
    Failed,           // git2 delete error (message carries detail)
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchDeleteResult {
    pub name: String,
    pub status: BranchDeleteStatus,
    /// Human detail for skipped/failed rows; None when Deleted.
    pub message: Option<String>,
}

/// Blocking. Deletes each caller-supplied name that is STILL safe, refusing the
/// current branch, the base branch, and anything not in a freshly-recomputed
/// stale set (defense-in-depth — never trusts the client). Deletes directly via
/// git2 `Branch::delete()` (OPEN #10). Returns a per-branch result; a per-branch
/// failure is reported, never a whole-call error. `base` mirrors
/// `find_stale_branches` so the safe set is recomputed against the same base.
/// Errors (whole-call): `git` (bad base) | `noRepo` (command layer).
pub fn delete_branches(workdir: &Path, names: &[String], base: Option<&str>)
    -> Result<Vec<BranchDeleteResult>, AppError>;
```

Algorithm:

```
report = find_stale_branches(workdir, base)?         // recompute safe set + base_name
safe = { b.name for b in report.branches }
repo = open_repo_at(workdir)
current = read_head_info(repo).branch_name
results = []
for name in names:
    if Some(name) == current:  push (name, SkippedCurrent, "checked-out branch"); continue
    if name == report.base:    push (name, SkippedBase, "base branch"); continue
    if name not in safe:       push (name, SkippedNotStale, "not detected as stale"); continue
    match repo.find_branch(name, Local) {
        Err(NotFound) => push (name, SkippedNotFound, "not found"),
        Err(e)        => push (name, Failed, e.message),
        Ok(mut b)     => match b.delete() {
            Ok(())  => push (name, Deleted, None),
            Err(e)  => push (name, Failed, e.message),
        }
    }
return results
```

`b.delete()` is force-delete semantics; safe because `safe` only holds merged-into-base or
gone-upstream branches (OPEN #10).

---

## 5. Command surface (`commands.rs` + `lib.rs`) — B4 only

Each: `pub async fn NAME(state, repo_id, …) -> Result<T, AppError>` + `spawn_blocking(move || stale::…)`,
resolving `repo_path(state, &repo_id)?`. **No consent gate** (pure git). Register both in `lib.rs`.
**No events, no channels.**

| Command (snake) | IPC method (camel) | Args | Returns | Error kinds |
|---|---|---|---|---|
| `list_stale_branches` | `listStaleBranches` | `repoId, base?` | `StaleReport` | `git \| noRepo` |
| `delete_branches` | `deleteBranches` | `repoId, names, base?` | `BranchDeleteResult[]` | `git \| noRepo` |
| *(B1: reuses)* `ai_analyze_diff` | `aiAnalyzeDiff` | `repoId, target, mode` | `AiAnalysis` | `aiUnavailable \| aiFailed \| nothingToCommit \| git \| invalidName \| noRepo` |

`base` is `Option<String>` (Tauri passes `undefined`/omitted → `None`). `names: Vec<String>`.

### 5.1 TS wire types (`types.ts`)

```ts
export type StaleReason = 'merged' | 'goneUpstream';

export interface StaleBranch {
  name: string;
  tip: string;
  lastCommitSummary: string;
  lastCommitAuthor: string;
  lastCommitTime: number;   // epoch seconds
  reason: StaleReason;
  merged: boolean;
  goneUpstream: boolean;
  upstream: string | null;
  ahead: number | null;
  behind: number | null;
  isCurrent: boolean;
}

export interface StaleReport {
  base: string;
  baseOid: string;
  branches: StaleBranch[];
}

export type BranchDeleteStatus =
  | 'deleted' | 'skippedCurrent' | 'skippedBase'
  | 'skippedNotStale' | 'skippedNotFound' | 'failed';

export interface BranchDeleteResult {
  name: string;
  status: BranchDeleteStatus;
  message: string | null;
}
```

### 5.2 `IpcApi` additions + `tauri.ts`

```ts
// IpcApi:
listStaleBranches(repoId: string, base?: string): Promise<StaleReport>;
deleteBranches(repoId: string, names: string[], base?: string): Promise<BranchDeleteResult[]>;
```

`tauri.ts` — one thin `invoke` each: `invoke('list_stale_branches', { repoId, base })`,
`invoke('delete_branches', { repoId, names, base })`.

---

## 6. B4 frontend (P25d)

### 6.1 `StaleBranchesDialog.tsx` (new)

- Opened by a **"Clean up branches…"** action in the sidebar Branches-section header (a small
  header button/overflow item). On open, calls `listStaleBranches(repoId)` (base auto).
- Renders `report.base` ("Comparing against `main`") + one checkbox row per `StaleBranch`: name,
  short tip, `lastCommitSummary`, relative `lastCommitTime`, and a reason chip
  (`merged` = neutral/green, `goneUpstream` = amber "upstream gone"). Rows with `merged` are
  **pre-checked**; `goneUpstream`-only rows are **unchecked** by default (force-delete — the user
  opts in). A "Select all / none" toggle.
- Empty report → a friendly "No stale branches — everything is merged or tracked." state.
- Footer **Delete selected (N)** button, disabled when 0 selected.

### 6.2 Confirm + delete (SAFETY GATE)

- **Delete selected** opens a `ConfirmDialog` (reuse) whose body lists the EXACT names to be
  deleted: *"Delete N local branch(es)? This cannot be undone.\n\n- feature/x\n- old/y"*. Title
  "Delete branches".
- On confirm → `deleteBranches(repoId, selectedNames)` then refresh the branches snapshot and
  re-run `listStaleBranches` (or close). Toast summary from the results:
  - all `deleted` → `success` `Deleted N branch(es)`.
  - some skipped/failed → `info`/`error` `Deleted N, skipped M` with the skipped reasons available
    per row (re-render the dialog showing residual rows + their `message`).
- `git` (bad base) / `noRepo` → `error` toast.

### 6.3 Mock (`mock.ts`) — stateful

Add `listStaleBranches` + `deleteBranches` to `mockIpc`, reusing `requireRepo`/`delay` and the
existing `state.branches.local`.

- Seed (in the per-repo fixture, or derived): classify a few fixture locals — e.g.
  `feature/merged-a` (merged), `feature/merged-b` (merged), `experiment-unmerged` (NEITHER →
  excluded), and a `feature/gone` (goneUpstream, `upstream:'origin/feature/gone'`,
  `goneUpstream:true`). `base` = `'main'`, `baseOid` = the fixture head oid. Never include the
  current branch or `main`.
- `listStaleBranches(repoId, base?)`: return the classified `StaleReport` from state (recomputed
  from `state.branches.local` so a prior `deleteBranches` shrinks it).
- `deleteBranches(repoId, names, base?)`: for each name, produce a `BranchDeleteResult` mirroring
  §4.3 (skip current → `skippedCurrent`; skip `main` → `skippedBase`; not in stale set →
  `skippedNotStale`; else `deleted` and **remove it from `state.branches.local`**). Return the
  results so the harness shows the deleted rows disappear and the toast summary.

---

## 7. Frontend behavior summary

- B1: two Review actions → existing `AiOutputPanel`; no new component.
- B4: `StaleBranchesDialog` + `ConfirmDialog` + toasts; branches snapshot refetched after delete.

---

## 8. Error mapping (no `error.rs` change)

| Situation | Variant | TS kind |
|---|---|---|
| B1 bad branch ref / no resolvable base / unrelated-history git errors | `Git` | `git` |
| B1 empty scope (no changes) | `AiFailed("no changes to analyze")` | `aiFailed` |
| B1 AI disabled / CLI missing | `AiUnavailable` / `AiUnavailable`(gate) | `aiUnavailable` |
| B1 CLI failed / timed out | `AiFailed` | `aiFailed` |
| B4 bad base / bare repo / git2 error (whole call) | `Git` | `git` |
| B4 per-branch skip/fail | *(in `BranchDeleteResult`, not an error)* | — |
| Unknown `repoId` (any command) | `NoRepo` | `noRepo` |

B4 per-branch outcomes are DATA (`BranchDeleteResult`), never thrown errors — a partial batch always
returns `Ok(results)`.

---

## 9. Tests (AI gate)

### 9.1 Rust unit tests (`#[cfg(test)]` in each module)

Fixtures use `crate::testutil::scratch_dir()` + git2 builders (deterministic identity,
`core.autocrlf=false`), mirroring `branches.rs`/`diff.rs`. `TMP`/`TEMP=D:\Temp`; run `cargo test`
and `clippy` **sequentially**.

**B1 (`ai_explain.rs` / `diff.rs`):**
1. **`AiDiffTarget` deserializes the new variants** — `{"kind":"worktree"}` → `Worktree`;
   `{"kind":"branch","name":"feature","base":null}` and `{"kind":"branch","name":"feature"}` (base
   omitted) → `Branch{ name:"feature", base:None }`; `{"kind":"branch","name":"f","base":"main"}`
   → `base:Some("main")`. (Extends the existing `diff_target_deserializes_each_variant` test.)
2. **`collect_file_diffs` multi-file** — a two-file tree-to-tree diff yields two `FileDiff`s with
   correct paths/status/hunks, in delta order; a too-large file is flagged `too_large` with empty
   hunks while its siblings collect normally.
3. **`gather_worktree`** — stage one file, leave another modified-unstaged, add an untracked file;
   assert the gathered `Vec<FileDiff>` covers all three (staged + unstaged + untracked) and a clean
   worktree gathers empty (→ `AiFailed` "no changes to analyze" via `analyze_diff`, using the
   `claude_stub.cmd` harness or a direct `has_analyzable_content` assert).
4. **`resolve_branch_base` precedence** — explicit base wins; else configured upstream; else
   `origin/main` via `origin/HEAD`; else local `main`; else `master`; else `Git`. Build refs to
   exercise each rung.
5. **Byte-cap** — assert `MAX_REVIEW_PAYLOAD_BYTES` truncation appends the note and stays a valid
   char boundary for a synthetic oversize payload (unit-test the cap helper directly).

**B4 (`stale.rs`):**
6. **Wire shapes** — `serde_json::to_value` on `StaleReport` / `BranchDeleteResult` asserts
   camelCase keys and that `StaleReason`/`BranchDeleteStatus` serialize to bare strings
   (`"merged"`, `"goneUpstream"`, `"skippedCurrent"`).
7. **merged detection** — base `main`, branch `feat` fully merged into `main` → listed `merged`,
   `ahead:0`; branch `wip` with a commit not in `main` → NOT listed. `main` (base) and the current
   HEAD branch are never listed.
8. **gone-upstream detection** — configure `branch.gone.remote/merge` with NO matching
   `refs/remotes/...` ref → listed `goneUpstream`, `merged:false`; a branch with a live upstream is
   not gone.
9. **`delete_branches` safety** — a set mixing a stale name, the current branch, the base, a
   non-stale branch, and a missing name → results are `deleted` / `skippedCurrent` / `skippedBase`
   / `skippedNotStale` / `skippedNotFound` respectively; only the stale branch ref is actually gone
   afterward; a fabricated non-stale name is NEVER deleted (defense-in-depth).

### 9.2 CLI-oracle — `crates/bonsai-core/tests/stale_cli.rs`

Scratch repos under `D:\Temp\bonsai-scratch`; `TMP`/`TEMP=D:\Temp`; degrade-skip when `git` is
absent (like `p8_git_cli_autostash_ff_oracle`). Build a repo with merged + unmerged branches; assert
`find_stale_branches(base="main").branches (reason==Merged).names` == the set from
`git branch --merged main` minus `main` and the current branch. (gone-upstream has no clean CLI
oracle — covered by the unit test.)

### 9.3 B1 core via the stub harness

Reuse `tests/fixtures/claude_stub.cmd` (as the other AI features do): with `BONSAI_CLAUDE_BIN`
pointed at the stub, `analyze_diff(Worktree, Review)` and `analyze_diff(Branch{..}, Review)` over a
scratch repo return the stub's canned text; the empty-scope path returns `AiFailed` before spawning.

### 9.4 Frontend AI gate

`pnpm build` + `tsc` clean; browser harness (`VITE_MOCK_IPC=1`) renders: the **Review working tree**
and **Review branch** actions → `AiOutputPanel` prose (mock); the **Clean up branches** dialog with
pre-checked merged rows + an amber gone-upstream row, the confirm dialog listing exact names, and —
after confirming — the deleted rows disappearing with a summary toast (mock mutation).

---

## 10. Sub-increment breakdown (each = one fresh-context `senior-dev` pass)

- **P25a — B1 core + IPC (no new command).**
  - Rust: `collect_file_diffs` in `diff.rs`; two `AiDiffTarget` variants + `gather_worktree` /
    `gather_branch` / `resolve_branch_base` + `MAX_REVIEW_PAYLOAD_BYTES` cap in `ai_explain.rs`;
    unit tests §9.1(1–5) + stub test §9.3.
  - IPC: extend the `AiDiffTarget` TS union (§2.6); extend `aiAnalyzeDiff` mock (§3.1). No command,
    no new type.
  - Acceptance: `analyze_diff` reviews a scratch worktree and a scratch branch via the stub; empty
    scope → `AiFailed`; `tsc`/`pnpm build` clean.
- **P25b — B1 UI.** Review-working-tree action (Changes panel) + Review-branch context-menu item,
  both routed through the existing `runAnalyze`/`AiOutputPanel`. Acceptance: harness screenshots of
  both review actions producing prose from the mock.
- **P25c — B4 core + commands + IPC triple.**
  - Rust: `stale.rs` (structs + `find_stale_branches` + `delete_branches` + `upstream_state`);
    `git/mod.rs` registration; unit tests §9.1(6–9) + oracle §9.2.
  - Commands: `list_stale_branches`, `delete_branches` (+ `lib.rs`).
  - IPC triple: §5.1 types + two methods in `types.ts`/`tauri.ts`/`mock.ts` (stateful mock §6.3).
  - Acceptance: detection matches `git branch --merged`; batch delete refuses current/base/non-stale
    server-side; `tsc`/`pnpm build` clean.
- **P25d — B4 UI.** `StaleBranchesDialog` + Branches-header action + `ConfirmDialog` (exact-name
  list) + toasts + branches refetch. Acceptance: harness screenshots of the dialog, the confirm, and
  the post-delete shrink.

Commit each approved sub-increment as `wip(P25a): …` etc. (orchestrator owns commits).

---

## 11. Acceptance criteria — AI gate vs USER CHECKPOINT

**AI gate (orchestrator-verifiable, no network, no native window):**
- `cargo check` + `clippy` clean on `bonsai-core`; `pnpm build` + `tsc` clean.
- All §9.1 unit tests + §9.2 CLI-oracle (degrades to git2-only when `git` absent) + §9.3 stub tests
  green.
- Browser-harness screenshots per §9.4: B1 worktree + branch review prose; B4 dialog (pre-checked
  merged rows, amber gone row), the exact-name confirm, and the post-delete shrink + toast.

**USER CHECKPOINT (native `pnpm tauri dev`, real repo + real `claude` CLI):**
- With AI enabled/consented, **Review working tree** on a repo with uncommitted changes returns a
  sane review; the same with AI disabled is blocked with a clear message.
- **Review branch** on a feature branch returns a review of its diff vs the resolved base
  (upstream/`main`); an unrelated-history or no-base case degrades gracefully.
- **Clean up branches** lists exactly the merged/gone-upstream locals (cross-check with
  `git branch --merged` and a branch whose remote was deleted), never the current or base branch;
  confirming deletes exactly the checked set (verify with `git branch`), Cancel deletes nothing, and
  a non-stale/base/current name is refused even if forced.
</content>
</invoke>
