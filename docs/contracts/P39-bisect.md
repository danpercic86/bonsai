# P39 — git bisect (contract)

Binary-search for the commit that introduced a regression. Bonsai-owned on-disk
state machine (git2/libgit2 has **no** bisect sequencer), modelled directly on
the P23a interactive-rebase engine (`rebase_interactive.rs`): a versioned JSON
state file under `.git/bonsai-bisect/`, atomic writes, re-read on every IPC call,
never held in memory, and a force-restore of the ORIGINAL HEAD/branch on reset.

---

## 0. Overview & invariants

- **Rust owns all git logic**: the state machine, the midpoint (candidate-count)
  math, the midpoint checkout, and the convergence detection. React only renders
  the banner and issues commands.
- `bisect.rs` is **runtime-free** (`&Path` / `&str` signatures, pure git2, no
  Tauri types, no network) → CLI-testable without the tauri `test` feature.
- git2 is blocking → every command wraps the core call in `spawn_blocking`.
- **On-disk state file = trivially safe abort**: the ORIGINAL branch ref is never
  moved during bisect (we only move a DETACHED HEAD across midpoints), so reset
  just re-attaches HEAD to `original_branch` (or force-detaches to
  `original_head` when originally detached) and deletes the state dir.
- **Dirty-worktree guard**: start and every step check out a commit → refuse
  when the index differs from HEAD or the worktree has tracked unstaged changes
  (mirror `start_interactive_rebase`'s two-part clean check). Unborn/detached
  handling per §3.
- **Bonsai-only state**: we do **not** read or write native `.git/BISECT_*`
  files. A concurrent terminal `git bisect` is an accepted, unsupported edge
  (mirrors P23's "coexisting terminal git rebase" decision). `read_op_state`
  probes the Bonsai file FIRST (§6).
- Reset is **confirm-gated in the UI** (worktree-mutating). Scratch repos live
  only under `D:\Temp\bonsai-scratch`; `TMP`/`TEMP=D:\Temp`; run test + clippy
  sequentially.
- Keep `src/ipc/mock.ts` compiling on every IPC change.

### git2 0.21.0 API (verified vs `Cargo.lock` line 1368-1370)

- `Revwalk`: `repo.revwalk()`, `walk.push(oid)`, `walk.hide(oid)`,
  `walk.set_sorting(git2::Sort::TOPOLOGICAL)`, iterate `Result<Oid>`.
- Checkout: `repo.set_head_detached(oid)` + `repo.checkout_tree(obj, force)` —
  the exact pattern already used in `rebase_interactive::start_interactive_rebase`.
- `repo.head()`, `repo.head_detached()`, `repo.find_commit`, `repo.statuses`,
  `repo.index()`, `index.write_tree_to`.

---

## 1. On-disk state schema

Location: `.git/bonsai-bisect/state.json`. Atomic write = `create_dir_all` +
write `state.json.tmp` + `rename` (copy `write_state`/`remove_state` verbatim
from `rebase_interactive.rs`, renaming the dir to `bonsai-bisect`).

```rust
/// Persisted bisect progress. Re-read on every IPC call; deleted on reset.
/// NEVER held across calls. Wire-internal only (not sent to the frontend as-is;
/// the frontend sees the RepoOpState::Bisect projection, §5/§6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BisectState {
    /// schema version = 1
    pub version: u32,
    /// 40-hex HEAD commit before bisect started (abort restore of the worktree).
    pub original_head: String,
    /// Short branch name HEAD pointed at before bisect (`refs/heads/<name>` minus
    /// prefix). `None` when bisect started from a detached HEAD → reset detaches
    /// back to `original_head`.
    pub original_branch: Option<String>,
    /// The known-BAD commit that bounds the search (40-hex).
    pub bad: String,
    /// Known-GOOD commits (ancestors excluded from the candidate set). ≥1 after
    /// start; grows as the user marks midpoints good.
    pub good: Vec<String>,
    /// Commits the user marked SKIP — excluded as answers but NOT as ancestors.
    pub skipped: Vec<String>,
    /// The midpoint currently checked out and awaiting a verdict (40-hex). None
    /// only in the terminal `found` phase (nothing left to test).
    pub current: Option<String>,
    /// Terminal result: the first-bad commit once the range converges. Set when
    /// phase becomes `found`; the branch is NOT restored until reset.
    pub first_bad: Option<String>,
}
```

**Phase is derived, not stored**: `first_bad.is_some()` ⇒ found; else in
progress. `current` is the commit under test in the in-progress phase.

---

## 2. Rust module `crates/bonsai-core/src/git/bisect.rs`

All fns blocking, runtime-free. Helpers (`bonsai_dir`, `state_path`,
`bisect_in_progress`, `read_state`, `write_state`, `remove_state`) mirror
`rebase_interactive.rs`.

```rust
/// Outcome of start / mark / skip — drives the banner and any auto-checkout.
/// Wire: tagged "kind", camelCase (mirrored in TS, §5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum BisectOutcome {
    /// Still searching. `current` is now checked out (detached HEAD); the UI
    /// shows the banner with these counts.
    Testing {
        current: String,
        revisions_remaining: u32,
        estimated_steps: u32,
    },
    /// Range converged. `first_bad` is the culprit; HEAD stays detached at it
    /// until the user resets. `revisions_remaining` == 0.
    Found { first_bad: String },
    /// Every remaining candidate is skipped → cannot determine. State is kept so
    /// the user can reset (or unskip by future extension). No new checkout.
    CannotDetermine { skipped: Vec<String> },
}

/// Start a bisect. `bad` = known-bad commit (UI default = HEAD), `good` = one or
/// more known-good ancestors. Detaches HEAD onto the first midpoint.
/// Errors: OperationInProgress (bisect or other op already active),
/// Git (unborn HEAD / bad oid / good not an ancestor of bad / dirty worktree),
/// per §4.
pub fn start_bisect(
    workdir: &Path,
    bad: &str,
    good: &[String],
) -> Result<BisectOutcome, AppError>;

/// Mark the currently-checked-out midpoint. `is_good == true` → the midpoint and
/// all its ancestors are good; `false` → it and its descendants (up to bad) are
/// suspect and it becomes the new upper bound. Recomputes + checks out the next
/// midpoint, or converges. Errors: NoOperationInProgress, Git (dirty worktree,
/// current oid mismatch), per §4.
pub fn bisect_mark(workdir: &Path, is_good: bool) -> Result<BisectOutcome, AppError>;

/// Skip the current midpoint (untestable). Picks an adjacent candidate; if all
/// remaining are skipped → CannotDetermine. Errors as bisect_mark.
pub fn bisect_skip(workdir: &Path) -> Result<BisectOutcome, AppError>;

/// Abort/finish: force-restore the ORIGINAL HEAD/branch and worktree, delete
/// `.git/bonsai-bisect/`. Idempotent-safe. Errors: NoOperationInProgress,
/// Git (checkout failure).
pub fn bisect_reset(workdir: &Path) -> Result<(), AppError>;

/// Read-only projection of the current state for opstate/banner. Returns None
/// when no bisect is in progress. (Convenience for tests + the opstate probe.)
pub fn get_bisect_state(workdir: &Path) -> Result<Option<BisectProgress>, AppError>;

/// Flattened progress the banner needs (matches RepoOpState::Bisect fields, §6).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BisectProgress {
    pub bad: String,
    pub good: Vec<String>,
    pub skipped: Vec<String>,
    pub current: Option<String>,
    pub first_bad: Option<String>,
    pub revisions_remaining: u32,
    pub estimated_steps: u32,
}
```

### 2.1 Candidate set + midpoint pseudocode (count-based)

The candidate set = commits reachable from `bad` but NOT from any `good`
(exclusive of the good commits, inclusive of `bad`). `bad` itself stays a
candidate until proven good's-child (standard: the answer can be `bad`).

```
fn candidates(repo, bad, good[]):
    walk = repo.revwalk()
    walk.set_sorting(TOPOLOGICAL)
    walk.push(bad)
    for g in good: walk.hide(g)          # hide g AND all its ancestors
    return list(walk)                    # includes bad, excludes good & ancestors

fn pick_midpoint(repo, bad, good[], skipped[]):
    cand = candidates(repo, bad, good)
    cand = [c for c in cand if c not in good]     # defensive
    testable = [c for c in cand if c != bad and c not in skipped]
        # exclude bad itself (already known bad) and skipped from being *tested*,
        # but they still count toward revisions_remaining bounds.
    if testable is empty:
        remaining_non_skipped = [c for c in cand if c != bad and c not in skipped]
        if remaining_non_skipped is empty and cand-minus-bad all skipped-or-empty:
            # nothing left to test
            if cand has exactly one non-good element (== bad's oldest suspect):
                return Converged(first_bad = that element)
            else:
                return AllSkipped(skipped)
    # count-based midpoint: the candidate whose "distance from bad" is ~half.
    # Order `testable` by the topological walk (bad-first); the element at
    # index floor((len)/2) is the midpoint. This halves the search each step
    # (git's own default is count-weighted; a simple positional split is the
    # accepted v1 approximation — §4 decision 4).
    mid = testable[len(testable) / 2]
    remaining = len(testable)
    steps = ceil(log2(max(remaining, 1)))
    return Testing(mid, remaining, steps)
```

**Convergence rule**: after a `bad`/`good` mark narrows the set, if `testable`
becomes empty the first-bad is the *unique* remaining suspect: the child of the
newest good boundary on the path to `bad` — concretely the single element of
`candidates(bad, good)` minus `bad` when that set has size 1, else `bad` itself
when every ancestor is good. Implementation: after updating good/bad bounds,
recompute `candidates`; if `pick_midpoint` yields no testable commit, the
first-bad = the oldest candidate not in `good` (i.e. the boundary commit). Unit
tests (§7) pin this against a scripted real `git bisect` (oracle).

### 2.2 mark good / mark bad semantics

- **mark good** (`is_good=true`): push `current` into `state.good`. Its ancestors
  drop out of the candidate set on the next `candidates()` walk (via `hide`).
- **mark bad** (`is_good=false`): set `state.bad = current`. `current` becomes
  the new upper bound; commits between the new bad and the good boundary remain
  candidates. (The previous `bad` is now a descendant of the new bad and leaves
  the reachable set.)
- After updating bounds, call `pick_midpoint`; `Testing` → checkout the new
  midpoint (§2.3) and persist; `Converged` → set `first_bad`, clear `current`,
  persist, leave HEAD where it is (detached at first-bad), return `Found`.

### 2.3 midpoint checkout (each step)

Clean detached checkout onto the midpoint tree (worktree already verified clean):
```
repo.set_head_detached(mid)?;
let mut co = CheckoutBuilder::new(); co.force();
repo.checkout_tree(mid_commit.as_object(), Some(&mut co))?;
```
On checkout failure mid-step: return `AppError::Git`, leaving the state file and
prior HEAD intact (the user can `bisect_reset` to fully recover — reset force-
restores `original_head`).

### 2.4 skip semantics

`bisect_skip`: push `current` into `state.skipped`, DO NOT change good/bad
bounds, then `pick_midpoint` (which excludes skipped from `testable`). If a
testable commit remains → checkout + `Testing`. If none remain but the range has
not converged to a single unambiguous suspect → `CannotDetermine { skipped }`
(state kept; UI offers only Reset). Mirrors git's "there are only skipped commits
left to test" message.

### 2.5 reset / restore (the ONE recovery helper)

```
fn restore_to_original(repo, state):
    orig = Oid(state.original_head)
    match state.original_branch:
        Some(name):
            repo.set_head("refs/heads/{name}")?      # ref itself never moved
        None:
            repo.set_head_detached(orig)?
    checkout_tree(orig, force)                        # hard-restore worktree
    index.read_tree(orig.tree); index.write()
    remove_state(repo)
```
`bisect_reset` = `read_state` + `restore_to_original`. Because the branch ref is
never moved during bisect, this is a pure re-attach + worktree refresh — simpler
than rebase's abort (which must un-move a possibly-advanced branch).

---

## 3. Preconditions (checked in `start_bisect`, before any mutation)

1. `bisect_in_progress` → `OperationInProgress("a bisect is already in progress…")`.
2. `repo.state() != Clean` → `OperationInProgress` (another op active).
3. Unborn HEAD → `Git("cannot bisect: the repository has no commits yet")`.
4. `bad`/each `good` must resolve to a commit → `Git("invalid commit id")`.
5. Each `good` must be an ancestor of `bad` (else the range is meaningless) →
   `Git("good commit <x> is not an ancestor of the bad commit")`. Check via
   `repo.graph_descendant_of(bad, good)` or presence in `candidates` walk.
6. `bad == good` or candidate set (minus bad) empty → `Git("nothing to bisect:
   good and bad are the same commit")`.
7. Dirty guard (index vs HEAD tree + no tracked unstaged change), copied from
   `start_interactive_rebase`. Detached-HEAD start is ALLOWED (record
   `original_branch = None`); unborn is not.

`bisect_mark`/`bisect_skip` guards: `!bisect_in_progress` →
`NoOperationInProgress`; dirty worktree → `Git` (the user edited files while
testing — refuse to check out the next midpoint until clean).

---

## 4. Decisions (recommended defaults — non-blocking)

1. **Entry point** — RECOMMEND commit context-menu flow, no dialog. Two items on
   the graph commit menu (`commitMenuItems`, gated like cherry-pick: attached
   born HEAD + idle): **"Mark as bad & start bisect"** (calls
   `start_bisect(oid, good=[])`… but good is required) → refine to a two-click
   flow: **"Start bisect: mark this BAD"** stores the pending-bad oid in
   frontend state and shows a hint toast; then **"Mark as GOOD (start bisect)"**
   on an older commit calls `startBisect(pendingBad, [thisOid])`. Simpler and
   fully harness-verifiable. If a pending-bad exists, the good item is enabled;
   otherwise disabled. RECOMMEND this over a modal for v1. *(Flag: a small
   BisectStartDialog with two commit pickers is the alternative — more discover-
   able but more UI surface; defer.)*
2. **Dirty-worktree guard** — refuse start AND each step when dirty (§3).
   Recommended: same two-part clean check as interactive rebase.
3. **`RepoOpState::Bisect` variant** — ADD it (recommended). The banner needs
   bisect-specific progress that no existing variant carries. Wire shape §6.
   This is a serde change the frontend mirrors in `types.ts` + `mock.ts`.
4. **Midpoint algorithm** — count/position-based split (§2.1), NOT libgit2's
   weighted bisect. Skip → adjacent candidate; all-skipped → `CannotDetermine`.
   Accepted v1 approximation; the oracle test asserts the *final first-bad*
   matches real `git bisect` (the intermediate sequence may differ by the split
   heuristic — the oracle asserts first-bad equality + that each Bonsai midpoint
   is a member of git's candidate set, §7).
5. **Finish semantics** — on convergence set phase `found`, surface `first_bad`
   in the banner, KEEP HEAD detached at it, and REQUIRE an explicit Reset to
   leave (mirrors rebase's explicit finish). No auto-reset.
6. **AppError** — NO new variant. Reuse `Git` (validation/dirty/checkout),
   `OperationInProgress` (start while an op is active — confirmed existing at
   `error.rs:50`), `NoOperationInProgress` (mark/skip/reset with none active,
   `error.rs:52`). `InvalidName` not needed (commits, not refnames).

---

## 5. IPC surface

### Commands (`src-tauri/src/commands.rs`; register in `lib.rs` handler list)

Each = thin wrapper over the core fn via `spawn_blocking`, resolving `repo_id` →
path exactly like `rebase_continue`/`start_interactive_rebase`.

| Command | Signature | Core call |
|---|---|---|
| `start_bisect` | `(state, repo_id: String, bad: String, good: Vec<String>) -> Result<BisectOutcome, AppError>` | `bisect::start_bisect` |
| `bisect_mark` | `(state, repo_id: String, is_good: bool) -> Result<BisectOutcome, AppError>` | `bisect::bisect_mark` |
| `bisect_skip` | `(state, repo_id: String) -> Result<BisectOutcome, AppError>` | `bisect::bisect_skip` |
| `bisect_reset` | `(state, repo_id: String) -> Result<(), AppError>` | `bisect::bisect_reset` |

`get_bisect_state` is NOT a separate command — bisect progress reaches the UI via
the existing `get_op_state` refresh (§6). (Optional: expose it later if the
banner needs richer data than the opstate projection.)

No new events/channels: bisect data is tiny (a handful of oids + two counts) →
request/response only, surfaced through the existing op-state refresh that
already runs after every mutation + on focus/watcher.

### TypeScript (`src/ipc/types.ts`)

```ts
export type BisectOutcome =
  | { kind: 'testing'; current: string; revisionsRemaining: number; estimatedSteps: number }
  | { kind: 'found'; firstBad: string }
  | { kind: 'cannotDetermine'; skipped: string[] };

// added to the RepoOpState union (§6)
```

Add to the `BonsaiIpc` interface:
```ts
startBisect(repoId: string, bad: string, good: string[]): Promise<BisectOutcome>;
bisectMark(repoId: string, isGood: boolean): Promise<BisectOutcome>;
bisectSkip(repoId: string): Promise<BisectOutcome>;
bisectReset(repoId: string): Promise<void>;
```
Tauri impl (`src/ipc/tauri.ts`): `invoke('start_bisect', { repoId, bad, good })`,
etc. (snake_case command names, camelCase args — matches existing convention).

---

## 6. `RepoOpState::Bisect` — opstate probe change

### Rust (`opstate.rs`)

Add a variant to `RepoOpState`:
```rust
Bisect {
    /// oid under test now; None in the terminal `found` phase.
    current: Option<String>,
    /// the bounding known-bad commit.
    bad: String,
    /// known-good boundary commits.
    good: Vec<String>,
    /// skipped (untestable) commits.
    skipped: Vec<String>,
    /// culprit once converged; None while still searching.
    first_bad: Option<String>,
    /// testable candidates left.
    revisions_remaining: u32,
    /// ~log2(revisions_remaining).
    estimated_steps: u32,
},
```
Probe FIRST in `read_op_state` (before the `repo.state()` switch), mirroring the
interactive-rebase probe at `opstate.rs:125`:
```rust
if bisect::bisect_in_progress(&repo) {
    if let Ok(Some(p)) = bisect::get_bisect_state(workdir) {
        return Ok(RepoOpState::Bisect { current: p.current, bad: p.bad,
            good: p.good, skipped: p.skipped, first_bad: p.first_bad,
            revisions_remaining: p.revisions_remaining,
            estimated_steps: p.estimated_steps });
    }
}
```
(Native `git bisect` writes `.git/BISECT_*`; `repo.state()` does not report a
bisect state, and the current `_ => RepoOpState::None` arm already swallows it —
Bonsai-only, per §0.) Add a wire-shape unit test asserting the tagged/camelCase
JSON, matching the existing `wire_shapes_are_camel_case_tagged` test.

### TypeScript (`types.ts` — extend `RepoOpState`)

```ts
  | {
      kind: 'bisect';
      current: string | null;
      bad: string;
      good: string[];
      skipped: string[];
      firstBad: string | null;
      revisionsRemaining: number;
      estimatedSteps: number;
    }
```

---

## 7. Tests

### `#[cfg(test)]` unit tests in `bisect.rs`

- `bisect_outcome_wire_shape_is_camel_case` — round-trip each variant.
- `bisect_state_round_trips_on_disk` — write/read/remove; `bisect_in_progress`.
- `start_rejects_unborn / dirty / non_ancestor_good / same_good_bad`.
- `linear_bisect_converges` — build a linear repo where commit K introduces a
  regression (a marker file changes); drive mark-good/mark-bad from the seeded
  good..bad range and assert `Found{first_bad == K}`, plus that HEAD stays
  detached at K until reset.
- `skip_picks_adjacent / all_skipped_cannot_determine`.
- `reset_restores_original_branch` and `reset_restores_detached_start`.
- `mark_without_start_is_no_op_in_progress`.

### CLI oracle `crates/bonsai-core/tests/bisect_cli.rs`

Mirror `rebase_interactive_cli.rs`: `require_git!`, scratch repos under
`D:\Temp\bonsai-scratch`, skip-if-no-git. Build a fixture history with the `git`
CLI (fixed dates → deterministic oids), pick a known culprit commit, then:
- Script real `git bisect start <bad> <good>` + `git bisect run <script>` (a
  test predicate that greps the marker) to obtain git's authoritative first-bad.
- Run Bonsai's engine over the SAME range, auto-answering each midpoint with the
  same predicate, and assert: (a) Bonsai's `Found.first_bad` == git's first-bad;
  (b) every Bonsai midpoint is a member of git's candidate set for that step
  (`git rev-list bad ^good`); (c) step count ≤ `ceil(log2(N))+1`.
- A skip-path fixture: mark one midpoint skip, assert convergence still matches
  git (or `CannotDetermine` when git reports "only skipped commits left").

---

## 8. Frontend

### `src/components/OpBanner.tsx` — add a `bisect` arm

Bisect banner (not conflict-driven, so it ignores `conflictCount`):
- **in-progress** (`op.current !== null`, `op.firstBad === null`): title
  `Bisecting`, sub `${revisionsRemaining} revisions left, ~${estimatedSteps} steps`.
  Actions: **Good**, **Bad**, **Skip**, **Reset** (Reset = danger, opens the
  existing Abort ConfirmDialog owned by the container).
- **found** (`op.firstBad !== null`): title `Bisect found first bad commit`, sub
  `${shortOid(firstBad)}` + summary if available. Single action: **Reset/Finish**.
- **cannot-determine** surfaces via a toast from the command result
  (`kind === 'cannotDetermine'`); the banner stays in-progress with only Reset.

New `OpBannerProps` handlers: `onBisectGood()`, `onBisectBad()`, `onBisectSkip()`
(Reset reuses `onAbort`). The container (`RepoWorkspace.tsx`) wires them to
`ipc.bisectMark(id, true/false)` / `ipc.bisectSkip(id)` / `ipc.bisectReset(id)`,
each followed by the standard post-mutation refresh (`getOpState` + graph +
status). Bisect controls are NOT gated on `conflictCount`.

### Entry point (`workspaceMenus.ts` `commitMenuItems`, `RepoWorkspace.tsx`)

Two-click flow (decision 1): add, under the existing cherry-pick block (attached
born HEAD + idle gate), **"Start bisect: mark this BAD"** (sets
`pendingBisectBad = oid`, toast "Now mark a known-good older commit") and
**"Mark GOOD & start bisect"** (enabled only when `pendingBisectBad !== null`,
calls `startBisect(pendingBisectBad, [oid])`, clears pending on success). While a
bisect is in progress (`opState.kind === 'bisect'`) these items are hidden and
the commit menu instead offers nothing bisect-specific (the banner drives it).

### Mock (`src/ipc/mock.ts`) — stateful walk

Add a `bisect` field to the per-repo mock state and implement the four methods so
the harness walks a seeded good..bad range to a `found` result:
- `startBisect`: reject if `opState.kind !== 'none'`; seed a small linear
  candidate list `[bad … good)` from the mock graph, pick the middle as
  `current`, set `opState = { kind: 'bisect', … }`, return `{ kind: 'testing' }`.
- `bisectMark(isGood)`: narrow the candidate window (good → drop the lower half
  incl. current; bad → drop the upper half, current becomes new bad). When the
  window collapses to one → set `firstBad`, `current = null`, return
  `{ kind: 'found', firstBad }` and update `opState`.
- `bisectSkip`: mark current skipped, pick an adjacent candidate; empty → return
  `{ kind: 'cannotDetermine', skipped }`.
- `bisectReset`: `opState = { kind: 'none' }`, clear the mock bisect field.
Keep the deterministic seed so a screenshot/`get_page_text` check shows the
banner counts decreasing to a found result.

---

## 9. Sub-increments

- **P39a — engine + IPC + oracle.** `bisect.rs` (state machine, midpoint math,
  checkout, reset, `#[cfg(test)]` units); `RepoOpState::Bisect` variant + opstate
  probe + wire test; four commands + `lib.rs` registration; IPC triple
  (`types.ts` `BisectOutcome` + `RepoOpState` extension + `BonsaiIpc` methods,
  `tauri.ts`, stateful `mock.ts`); `tests/bisect_cli.rs` oracle cross-checked vs
  real `git bisect run`. Gate: `cargo test`/`clippy` green (sequential),
  oracle passes, `tsc`/`vite build` green.
- **P39b — UI.** OpBanner bisect arm (Good/Bad/Skip/Reset + found + counts);
  `commitMenuItems` two-click entry + `pendingBisectBad` state + Reset confirm
  dialog wiring in `RepoWorkspace.tsx`; `cannotDetermine` toast. Gate: browser
  harness (`VITE_MOCK_IPC=1`) shows start → mark → found flow, banner counts, and
  Reset returns to `none`.

---

## 10. AI gate vs USER CHECKPOINT

**AI gate (orchestrator verifies):**
- `cargo test -p bonsai-core` incl. `bisect_cli` oracle: Bonsai first-bad == git
  first-bad on ≥1 linear fixture; midpoint-membership + step-bound assertions.
- `cargo clippy` clean; `tsc` + `vite build` clean; `mock.ts` compiles.
- Browser harness: start bisect from a commit, click Bad/Good to convergence,
  banner shows decreasing "N revisions left, ~K steps" then the found culprit;
  Reset clears the banner. Screenshot as final proof.

**USER CHECKPOINT (native Tauri window):**
- Real repo: start bisect (mark bad/good via context menu), confirm each midpoint
  is actually checked out (files on disk change), mark good/bad a few rounds,
  confirm the reported first-bad matches expectation, and Reset returns to the
  original branch with a clean worktree.
- Reset confirm dialog appears and is honored.

---

## 11. Flagged ambiguities (for the orchestrator)

- **Entry-point UX (decision 1).** Recommended two-click context-menu flow
  (mark-bad → mark-good). A `BisectStartDialog` with two pickers is the
  alternative (more discoverable, more surface). Recommend context-menu for v1;
  confirm.
- **Midpoint heuristic (decision 4).** Positional split, not libgit2 weighted
  bisect. The oracle asserts *final first-bad* equality, not the exact
  intermediate midpoint sequence (which may differ from git's). Confirm this
  weaker-but-correct equivalence is acceptable for the AI gate.
- **`get_bisect_state` command.** Omitted for now (progress rides on
  `get_op_state`). Add later only if the banner needs commit *summaries* for the
  good/bad/first-bad oids (would require resolving summaries backend-side).
