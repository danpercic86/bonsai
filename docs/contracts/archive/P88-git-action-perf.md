# P88 — Git-action perf round 2

Status: contract (design). Extends **P85** (echo-armed refresh) and **P86** (scoped rounds `refreshScope.ts`
+ graph-layout cache `graph_cache.rs` + perf counters). Branch off `main` @ `c0825a3`. **Backend-only +
frontend refresh-routing — no user-visible UI change, so no `ui-designer` step.**

Two sub-increments ship separately: **P88a** (frontend refresh-scope cluster; mechanical) then **P88b**
(backend repo-handle cache + cache memory cap; careful).

**No new IPC surface.** No command / event / channel is added or changed; no wire shape changes. The perf
counters (`debugPerfCounters` / `debugResetPerfCounters`) already exist from P86. `src/ipc/mock.ts` needs
**no** change — the `stash` `RefreshScope` value and `RefreshAll` alias are frontend-internal to the refresh
layer, and P88b is internal caching behind byte-identical output.

---

## P88a — Frontend refresh-scope cluster

**Bug class.** A set of action handlers never adopted the P85 pattern: they refetch via raw `refetchX()` /
`Promise.all([...])` instead of `refreshAll(scope)`, so they (a) bypass the coalescer and (b) never
`armEcho` — the op's own `.git` write then trips the notify watcher ~300 ms later and runs a **second,
unsuppressed `full` round** (the exact P85 double-refresh). A second cohort routes through `refreshAll` but
passes an over-broad `full` scope. P88a routes every listed handler through `refreshAll(scope)` at its
minimal-correct scope.

### A0 — New `stash` scope + `RefreshAll` alias (`refreshScope.ts`)

Add a `stash` scope = **status + graph + stashes** (confirmed slot names from `refreshScope.ts` /
`RepoWorkspace.tsx:1210-1237`). Stash push/apply/pop change the worktree+index (→ `status`), the
`refs/stash` reflog + the synthetic stash node in the walk (→ `graph`, correctly a B1 Miss on the hide-set
change), and the stash list (→ `stashes`). No HEAD move ⇒ `openRepo:false` (consistent with the other
non-`full` scopes). No `opState` (a stash pop-with-conflicts sets no MERGE_HEAD/rebase state).

```ts
// refreshScope.ts — extend the union + the Record (both are exhaustive; adding to the type FORCES the entry)
export type RefreshScope = 'full' | 'refsOnly' | 'remoteMeta' | 'worktree' | 'stash';

// add to the SLICES record:
  stash: { ...NONE, status: true, graph: true, stashes: true },

// new shared alias so every touched hook narrows identically (the runtime callback in
// RepoWorkspace.tsx:1277 already has this exact signature):
export type RefreshAll = (scope?: RefreshScope) => Promise<void>;
```

`unionScopes` needs **no** change — it already widens any two distinct non-`full` scopes to `full`
generically, so `stash` participates automatically.

### A1 — Type-widening (unblocks narrowing)

Hooks currently type their dep as `refreshAll: () => Promise<void>` (no scope param) and so cannot call
`refreshAll('worktree')` etc. Widen the dep type to the new `RefreshAll` alias in the hooks that pass a
scope, and **add** a `refreshAll: RefreshAll` dep to the three hooks that currently take only `refetchX`:

| Hook | change |
|---|---|
| `useMergeActions.ts:17` | `refreshAll: () => Promise<void>` → `refreshAll: RefreshAll` |
| `useStashActions.ts:9` | `refreshAll: () => Promise<void>` → `refreshAll: RefreshAll` |
| `useTagRemoteActions.ts` | **add** `refreshAll: RefreshAll` to deps; `RepoWorkspace.tsx:1695` passes `refreshAll` |
| `useSubmoduleActions.ts` | **add** `refreshAll: RefreshAll`; `RepoWorkspace.tsx:1663` passes `refreshAll` |
| `useCommitComposer.ts` | **add** `refreshAll: RefreshAll`; `RepoWorkspace.tsx` composer call passes `refreshAll` |

**Decision (always-`full` hooks):** `useCherrypickRevertActions` / `useRebaseActions` / `useBisectActions` /
`useCommitActions` (reset) already receive `refreshAll` and only ever call `refreshAll()` with no arg — a
`() => Promise<void>` param type accepts the wider runtime callback, so they compile unchanged.
**Recommend leaving them as-is** (velocity: zero churn; a `() => Promise<void>` dep is not a bug, just
narrower). Optionally re-type them to `RefreshAll` in a later cleanup; not required here.

### A2 — Per-handler scope matrix (this is the acceptance checklist)

Only the refetch statement changes in each handler; keep every `try/catch/finally`, toast, `setMutating`,
and `ipc.*` call verbatim. `refreshAll` never throws (P81).

| # | Handler | file:line | Today | New | Why |
|---|---|---|---|---|---|
| 1 | `refreshAfterTagOp` (create/delete/force-refresh/fetch-remote-tag/delete-remote-tag/force-move) | `useTagRemoteActions.ts:31` | `Promise.all([refetchBranches, refetchGraph, refetchTagSync({force})])` | `Promise.all([refreshAll('refsOnly'), refetchTagSync({ force: true })])` | tag write → ref echo; `refsOnly` = graph+branches(tag list)+compare; keep the **forced** tagSync verdict (no scope forces it) |
| 2 | `handleAddRemote` | `useTagRemoteActions.ts:153` | `Promise.all([refetchRemotes, refetchBranches, refetchGraph])` | `refreshAll('remoteMeta')` | `remoteMeta` = graph+branches+remotes+compare+tagSync(non-forced) |
| 3 | `handleRemoveRemote` | `:166` | same | `refreshAll('remoteMeta')` | removes `refs/remotes/<n>/*` (pills+branches change) |
| 4 | `handleRenameRemote` | `:179` | same | `refreshAll('remoteMeta')` | moves remote-tracking refs |
| 5 | `handleSetRemoteUrl` | `:192` | `refetchRemotes()` | **keep `refetchRemotes()`** (see OD-P88-1) | config-only write; watcher ignores `.git/config` (`watcher.rs:75-78`) ⇒ no echo, nothing to fix |
| 6 | `handleCreateStash` | `useStashActions.ts:28` | `refreshAll()` | `refreshAll('stash')` | narrow the full round to status+graph+stashes |
| 7 | `handleApplyStash` | `:71` | `refreshAll()` | `refreshAll('stash')` | " |
| 8 | `handlePopStash` | `:105` | `refreshAll()` | `refreshAll('stash')` | " |
| 9 | `handleDropStash` | `:119` | `Promise.all([refetchStashes, refetchGraph])` | `refreshAll('stash')` | `refs/stash` write → echo; arm it. (`stash` over-fetches one status scan vs the ideal graph+stashes — accepted; not worth a 6th scope) |
| 10 | `handleResolveConflict` | `useMergeActions.ts:81` | `refreshAll()` (armed) | `refreshAll('worktree')` | index-only stage; no HEAD move; keep `opState` (merge still in progress) |
| 11 | `stageResolvedText` | `:112` | `refreshAll()` (armed) | `refreshAll('worktree')` | same (per-file resolved-text writer) |
| 12 | commit-composer `apply` | `useCommitComposer.ts:256` | `refetchStatus(); refetchGraph();` | `void refreshAll();` (`full`) | HEAD moves; raw pair omitted `branches` ⇒ stale ahead/behind. Drop `refetchStatus`/`refetchGraph` from deps + the `apply` `useCallback` deps iff unused |
| 13 | `handleDeleteBranch` (local) | `useBranchActions.ts:112` | `refreshAll()` (`full`) | `refreshAll('refsOnly')` | match `handleDeleteRemoteTracking:200`; no HEAD move, no worktree change. Graph slice still re-walks correctly (B1 Miss on tip removal — see P88b limitation) |
| 14 | `refreshAfterChange` (submodule add/deinit/remove) | `useSubmoduleActions.ts:48` | `Promise.all([refetchSubmodules, refetchStatus, refetchGraph])` | `Promise.all([refreshAll('worktree'), refetchSubmodules()])` | index+`.gitmodules` write → echo; `worktree` arms it + covers status; keep the submodules refetch (no scope carries it). Drop now-unused `refetchStatus`/`refetchGraph` deps |

**`useMergeActions` unchanged rows:** `handleMergeBranch:69`, `handleCommitMerge:214`, `handleAbortMerge:229`
keep `refreshAll()` (`full`) — all move HEAD / reset the worktree.
**`useSubmoduleActions` init/update/sync** keep `refetchSubmodules` only (they touch no superproject state).

### P88a acceptance criteria

- **AC-a1:** each row 1–4, 6–14 fires **exactly 1** refresh round on success (assert
  `window.__bonsaiRefreshRounds` delta == 1); its own watcher echo within the window adds **0** rounds.
- **AC-a2:** each round runs at the matrix scope (assert via `window.__bonsaiRefreshScopes`): rows 1/13 →
  `refsOnly`; 2–4 → `remoteMeta`; 6–9 → `stash`; 10–11 → `worktree`; 12 → `full`; 14 → `worktree` (+ a
  standalone `refetchSubmodules`).
- **AC-a3 (consistency):** after commit-composer `apply`, `branches` is refetched ⇒ ahead/behind not stale.
  After a tag op, the tag list/pills and the sync verdict both update. After a local branch delete that drops
  commits, the graph shrinks (B1 Miss).
- **AC-a4:** `slicesForScope('stash')` == `{ status, graph, stashes }` (all else false); `refreshScope.test.ts`
  extended.
- **AC-a5:** `tsc` clean — no hook types `refreshAll` too narrowly to pass its scope.

---

## P88b — Backend repo-handle cache (B2) + PB-1 cache cap

Today **no** `git2::Repository` is cached (`state.rs:58` `RepoEntry` has none). Every command re-opens from
path (`repo_path` → `spawn_blocking` → `open_repo_at`): ~9–11 opens per full round; a multi-step mutation
re-opens ~5× in one op (`checkout_branch_autostash` `checkout.rs:98` then `create_stash`/`checkout_branch`/
`try_ff`/`pop_stash` each re-open). Goal: drive `repo_opens` (`perf.rs:18`) toward ~1 per round and ~1 per
composite op, **byte-identical** output, no test-count change.

### B2 — handle-strategy decision

`git2::Repository` is `Send` but **not `Sync`**, and a full round fires ~11 **concurrent** `invoke`s
(`Promise.all`), each in its own `spawn_blocking` task on the tokio blocking pool.

| Option | opens/round | concurrency | `Sync`? | verdict |
|---|---|---|---|---|
| `Mutex<Repository>` per `RepoEntry` | ~1 | **serializes** the round's 11 tasks onto one handle — the slow `status` (O(worktree)) and a `graph` Miss (O(commits)) run back-to-back instead of parallel ⇒ latency regression on big repos | n/a (Mutex) | reject as default (kills the fan-out the round relies on) |
| Small handle **pool** per repo | ~1 (warm) | preserved | n/a | over-engineered for a one-active-repo desktop app; checkout/return plumbing |
| **Thread-local**, keyed `(repo_id, generation)` | ~1 per pool thread per gen | **preserved** (each task uses its own thread's handle; no contention) | not needed (never shared) | **RECOMMENDED** — matches the shipped P86 §B2 recommendation |

**Recommendation: thread-local handle cache keyed `(repo_id, generation)`.** Each blocking-pool thread keeps
its own `Repository`; nothing is shared across threads (so no `Sync`), no mutex serializes the round, and a
handle is reused across rounds and across the sequential sub-primitives of one composite op. Bound: ≤
(pool threads × open repos) handles (tens of MB worst case) — negligible for a desktop app; evicted on
generation bump.

### B2 migration shape

Core fns keep their `&Path` entry points (public API, tests, mock, `get_graph` all unchanged); add
`&Repository`-taking `*_with` twins by extracting the body:

```rust
// pattern, per hot core fn (bonsai-core::git::*)
pub fn read_status(path: &Path) -> Result<StatusSnapshot, AppError> {
    let repo = open_repo_at(path)?;            // the ONLY open
    read_status_with(&repo)
}
pub fn read_status_with(repo: &git2::Repository) -> Result<StatusSnapshot, AppError> { /* moved body */ }
```

Single command-layer seam that resolves+reuses a handle:

```rust
// src-tauri/src/repo_handle.rs (new, ~80 lines)
thread_local! { static HANDLES: RefCell<HashMap<(String, u64), git2::Repository>> = /* empty */; }

/// Run `f` against a per-thread cached open Repository for (repo_id, generation).
/// Opens (NO_SEARCH) on first use per thread (bumps `perf.repo_opens`) and reuses
/// thereafter; a generation bump drops the stale entry and reopens.
pub fn with_repo<R>(
    repo_id: &str, generation: u64, path: &Path, perf: &PerfState,
    f: impl FnOnce(&git2::Repository) -> Result<R, AppError>,
) -> Result<R, AppError>;
```

Generation source — add to `AppState` (or an `AtomicU64` in `RepoEntry`):

```rust
pub repo_generations: Mutex<HashMap<String, u64>>, // bumped on open_repo re-arm + close_repo
```

`repo.rs:269` (open re-arm) and `close_repo_inner:312` bump the generation so a re-open/close evicts every
thread's stale handle lazily on next `with_repo`. Extend `repo_path` → `repo_path_and_gen` (clone
`(path, generation)` out under the one map lock, same pattern as `repo_path_and_graph_cache`).

**Stage B2 for safety** (same `*_with` refactor serves both):
- **B2a (safe, biggest single-command win, land first):** thread `&Repository` through the composite
  mutations (`checkout_branch_autostash` and peers at `checkout.rs:98/128/132/144/150`) so each composite
  command opens **once**. Single thread, sequential, sole writer = the command itself ⇒ trivially
  byte-identical, no shared-state/freshness concern. Fixes the ~5× → ~1×.
- **B2b (round handle cache, land after B2a green):** route the round's hot read commands
  (`graph_seed`, `read_status`, `list_refs`, then the rest) through `with_repo`. Fixes the ~9–11 → ~1
  per pool thread. Carries the freshness contract below; gate on the byte-identical status/graph tests.

### B2 freshness / invalidation contract

A reused handle must never serve stale data. libgit2 behavior for Bonsai's read paths:

- **Re-read on demand (safe to reuse):** ref enumeration (`references()` re-stats loose refs + reloads
  `packed-refs` on mtime change), the odb (new loose/pack objects found per lookup), and every `Revwalk`
  (fresh per `revwalk()`). ⇒ **`graph_seed` stays byte-identical** through a cached handle — its ref/tip/HEAD
  probe reads current on-disk state. This is the load-bearing correctness point for the B1 classify.
- **Cached, MUST force fresh:** the `Index` object (`repo.index()` returns a cached handle) and `Config`.
  Any `*_with` fn that reads the index (chiefly `read_status_with`) MUST call `index.read(true)` before use
  if the handle can outlive an external index write. **senior-dev must verify with a test** (status through a
  reused handle after an external index change via a second handle == status through a fresh open).
- **Invalidation:** generation bump on `close_repo`/`open_repo` re-arm evicts all per-thread handles for the
  id. Watcher-detected external changes need **no** eviction (refs/odb/worktree re-read on demand; index
  forced via `read(true)` at the seam). Our own mutations (fetch/checkout/commit) run through the same
  handle in-process and see their own writes; the following round reads fresh.

### PB-1 — cache memory cap (`graph_cache.rs`)

`stream_graph_cached` retains the whole `Vec<GraphChunk>` up to `STREAM_MAX_COMMITS` (1M) — ~150–250 MB/repo
at cap. Cap the store by node count:

```rust
// graph_cache.rs
const GRAPH_CACHE_MAX_NODES: usize = 50_000; // 2.5× the 20k jank-free target; ~10–25 MB/repo at cap
```

In the Miss tee loop: track accumulated node count; once it exceeds `GRAPH_CACHE_MAX_NODES`, **drop `buf`
(free it) and stop buffering** (set a `too_big` flag) so the huge Vec is never even transiently retained; on
`Done`, skip the store when `too_big`. The channel emit path is unchanged (every chunk still streams to the
frontend); the walk is still counted (`inc_graph_walks`). A repo above the cap simply re-walks each refresh
(correct, uncached). **Justify 50k:** covers the 20k target with 2.5× headroom while bounding per-repo cache
memory to ~10–25 MB.

### KNOWN limitation (document, do not fix) + retirements

- **Delete-branch cache miss:** `classify` (`graph_cache.rs:139-162`) requires `c.tips.is_subset(new_tips)`
  for a `HitRedecorate`; a tip removal (branch/remote-tracking delete, stash pop) always fails ⊆ ⇒ **Miss ⇒
  full re-walk**, even when the deleted branch was fully merged (no node actually drops). This is a
  **conservative, correct** limitation. The P88a `refsOnly` routing (row 13) is the cheap win — it skips
  status/remotes/stashes/etc.; only the (unavoidable-for-now) graph re-walk remains. The hard fix — a
  reachability check after tip removal to prove no emitted node became unreachable, enabling a redecorate —
  is **out of scope** for P88.
- **PB-2 (cold-walk store fingerprint): RESOLVED — retire.** The post-walk `seed_unchanged` re-probe
  (`graph_cache.rs:251-290`) already guards the TOCTOU.

### P88b acceptance criteria

- **AC-b1 (round opens):** a full refresh round drops `repo_opens` (via `debugPerfCounters`) from ~9–11 toward
  ~1 per pool thread per generation.
- **AC-b2 (composite opens):** a dirty checkout (`checkout_branch_autostash`) drops from ~5 opens to ~1.
- **AC-b3 (generation evict):** a `close_repo`+`open_repo` re-arm forces a reopen on the next command.
- **AC-b4 (byte-identical):** graph chunk stream and `StatusSnapshot` are byte-identical to pre-P88b for the
  same repo state; **no test-count change**.
- **AC-b5 (freshness):** status/graph_seed through a reused handle after an external index/ref change equal
  the fresh-open result (the index `read(true)` guard test).
- **AC-b6 (PB-1):** a repo above `GRAPH_CACHE_MAX_NODES` is not stored (cache stays `None`; `graph_walks`
  increments each refresh); a ≤50k repo (incl. the 20k target) still stores and serves HitVerbatim/Redecorate.

---

## File-size / split notes

- `refreshScope.ts` gains ~2 lines (stay < 100). No hook file grows materially (dep-type + one-line body edits).
- New `src-tauri/src/repo_handle.rs` (~80). `graph_cache.rs` gains the PB-1 cap (~10 lines; stays < 500).
- The `*_with` twins add a few lines each across `status.rs` / `branches.rs` / `checkout.rs` / `graph.rs`;
  if any crosses ~500, extract the twin into a sibling module — do not let it grow.

## Flags for the orchestrator

- **OD-P88-1 (set-url, row 5):** set-url writes only `.git/config`, which the watcher ignores
  (`watcher.rs:75-78`) ⇒ no echo, no double-refresh to fix. **Recommend keep `refetchRemotes()`** (already
  minimal-correct; there is no "remotes-only" scope and `remoteMeta` would over-fetch graph+branches+tagSync).
  If uniform `refreshAll` routing is preferred for consistency, use `remoteMeta` (graph/branches would be B1
  cache hits). Decide.
- **OD-P88-2 (B2 strategy):** thread-local (recommended) vs `Mutex<Repository>` (the TODO's phrasing). The
  concurrency argument (don't serialize the parallel round) favors thread-local and matches the shipped P86
  §B2. Confirm before the `*_with` refactor, since both feed the same refactor.
- **OD-P88-3 (B2 staging):** land B2a (composite single-open, zero freshness risk) independently and treat
  B2b (round handle cache + index-freshness guard) as a separately-verified follow-on. Recommend yes.
- **Sequencing:** P88a is self-contained (depends only on shipped P85/P86a). P88b B2a → B2b → PB-1 (PB-1 is
  independent and can land with either B2 step). P88a's `refsOnly` on branch delete depends on B1 (shipped)
  to re-walk correctly.

---

## FU-B2c — status/graph handle reuse across the corrupt-object watchdog

**Problem (recap).** `get_status` and `stream_graph` are the two hottest refresh-round reads, but they
run `with_repo*` INSIDE `run_with_git_timeout` (`timeout.rs`), which spawns a fresh watchdog OS thread
per call. That thread's `HANDLES` thread-local is always empty ⇒ status/graph open once per call, no
cross-round reuse (the shipped B2b limitation, pinned by `get_status_opens_once_per_call` /
`stream_graph_path_opens_once_per_call`). FU-B2c makes them reuse a cached handle across rounds while
KEEPING the watchdog's abandon-on-hang safety. **Backend-only. No IPC/TS/mock change** — byte-identical
wire output; the perf counters (`debugPerfCounters`) already exist.

### Strategy decision

| Option | steady-state reuse | hang safety | complexity | verdict |
|---|---|---|---|---|
| **1. Move-in/move-back through the watchdog** | pool thread owns the handle, MOVES it into the watchdog thread, watchdog MOVES it back on the result channel; re-cached per (repo_id, gen) | unchanged — watchdog still abandons the thread on inactivity; the moved handle leaks with it (ONE handle, self-heals) | small (one owned-value timeout variant + one wrapper) | **RECOMMENDED** |
| 2. Persistent watchdog worker(s) with warm thread-locals | reuse without any move | **broken by design** — a hung/corrupt call POISONS a persistent worker permanently; needs poison-detection + worker-respawn, and a respawned worker's warm handle is lost anyway (so no steady-state gain over opt 1) | high | reject |

**Recommendation: Option 1.** Option 2 fights the watchdog's core contract (abandon a wedged thread);
its poison/respawn machinery is strictly more code for no better warm-path outcome. `git2::Repository`
is `Send` (movable across threads) but `!Sync` (no shared `&` across threads) — Option 1 keeps the
handle owned by EXACTLY ONE thread at every instant (pool → watchdog via channel/capture → pool via
channel), so `Send` alone is sound; no `Sync`, no shared `&mut`, no UB. On timeout the pool thread has
already relinquished ownership (`val` moved in, `None` back), so the abandoned thread solely owns the
leaked handle — no aliasing.

### Rust — `crates/bonsai-core/src/git/timeout.rs` (owned-value variant)

Add an owned-value twin of the existing wrapper (existing `run_with_git_timeout` / `_with` callers stay
UNCHANGED). `effective_deadline` / `GitProgress` / `POLL` / error strings are reused verbatim.

```rust
/// Like `run_with_git_timeout`, but MOVES an owned `val: T` into the worker and
/// back out. `f` receives `val` by value and returns it alongside its result, so
/// the handle survives an inner git error and is re-cachable by the caller.
/// Return contract:
///   (Some(val), Ok(r))   — f completed; git op ok.        caller RE-CACHES val.
///   (Some(val), Err(e))  — f completed; inner git error.  caller RE-CACHES val.
///   (None, Err(Git ..))  — inactivity timeout; worker abandoned, val leaked.
///   (None, Err(Other ..))— worker panicked / spawn failed; val gone.
pub fn run_with_git_timeout_owned<T, R, F>(op: &str, val: T, f: F) -> (Option<T>, Result<R, AppError>)
where
    T: Send + 'static,
    R: Send + 'static,
    F: FnOnce(&GitProgress, T) -> (T, Result<R, AppError>) + Send + 'static;

/// Explicit-deadline twin (tests / short deadlines), mirroring `_with`.
pub fn run_with_git_timeout_owned_with<T, R, F>(
    op: &str, deadline: Duration, val: T, f: F,
) -> (Option<T>, Result<R, AppError>)
where /* same bounds */;
```

Recv-loop delta vs `run_with_git_timeout_with` (everything else identical):
- channel type becomes `mpsc::channel::<(T, Result<R, AppError>)>()`; worker body
  `let _ = tx.send(f(&worker_progress, val));` (moves `val` into the spawn closure).
- `Ok((val, res))` → `return (Some(val), res)` (was `return result`).
- `Disconnected` (panic) → `return (None, Err(AppError::Other(".. panicked")))`.
- inactivity deadline reached → `return (None, Err(AppError::Git(".. operation timed out ..")))`.
- spawn failure → `return (None, Err(AppError::Other(".. failed to spawn ..")))` (val was consumed by
  the failed spawn closure — unrecoverable, catastrophic, rare).

**De-dup (recommended, optional):** reimplement `run_with_git_timeout_with` as a thin adapter —
`let (_u, res) = run_with_git_timeout_owned_with(op, deadline, (), move |p, ()| ((), f(p))); res` — so
one copy of the recv loop remains. Same error strings ⇒ the 6 existing timeout tests pass unchanged.
senior-dev may keep a second copy of the loop if it reads cleaner; behavior must be identical either way.

### Rust — `src-tauri/src/repo_handle.rs` (`_timed` wrappers)

Add timeout-aware twins that run on the POOL thread (called INSIDE `spawn_blocking`, OUTSIDE the
watchdog). The direct `with_repo`/`with_repo_mut` stay as-is for the list trio.

```rust
use bonsai_core::git::timeout::{run_with_git_timeout_owned, run_with_git_timeout_owned_with, GitProgress};

/// `&mut` handle reuse ACROSS the corrupt-object watchdog. Runs on the caller's
/// (blocking-pool) thread: takes the cached handle out of HANDLES for
/// (repo_id, generation) — opening + `perf.inc_repo_opens()` only on a miss /
/// stale generation — moves it THROUGH `run_with_git_timeout_owned`, and puts it
/// back on success. On timeout/panic the handle is abandoned with the worker and
/// the entry stays absent ⇒ the next call reopens.
pub fn with_repo_mut_timed<R>(
    op: &str, repo_id: &str, generation: u64, path: &Path, perf: &PerfState,
    f: impl FnOnce(&GitProgress, &mut git2::Repository) -> Result<R, AppError> + Send + 'static,
) -> Result<R, AppError>
where R: Send + 'static;

/// Shared twin (narrows `&mut` → `&`); delegates to `with_repo_mut_timed`.
pub fn with_repo_timed<R>(
    op: &str, repo_id: &str, generation: u64, path: &Path, perf: &PerfState,
    f: impl FnOnce(&GitProgress, &git2::Repository) -> Result<R, AppError> + Send + 'static,
) -> Result<R, AppError>
where R: Send + 'static;

/// Explicit-deadline internal variant so timeout behavior is testable env-free
/// (delegates to `run_with_git_timeout_owned_with`); `with_repo_mut_timed`
/// delegates to it with `effective_deadline()`.
fn with_repo_mut_timed_with<R>(op, deadline: Duration, repo_id, generation, path, perf, f) -> ...;
```

Body of `with_repo_mut_timed_with` (pseudocode):
```
// 1. TAKE the handle out of THIS pool thread's thread-local.
let repo = HANDLES.with(|cell| {
    let mut map = cell.borrow_mut();
    let key = (repo_id.to_string(), generation);
    if let Some(r) = map.remove(&key) { return Ok(r); }   // exact-gen hit → take ownership
    map.retain(|(id, _), _| id != repo_id);               // evict any stale-gen entry for the id
    let r = open_no_search(path)?;
    perf.inc_repo_opens();                                 // count the ACTUAL open only
    Ok(r)
})?;
// 2. Move it through the watchdog; f gets it by &mut, returns it by value.
let (returned, result) = run_with_git_timeout_owned_with(op, deadline, repo,
    move |progress, mut repo| { let res = f(progress, &mut repo); (repo, res) });
// 3. Re-cache iff it came back (Ok OR inner Err). None ⇒ abandoned ⇒ leave absent.
if let Some(repo) = returned {
    HANDLES.with(|cell| { cell.borrow_mut().insert((repo_id.to_string(), generation), repo); });
}
result
```
`repo_id` / `path` / `perf` are borrowed on the pool thread only (steps 1 & 3); only `f` and `repo`
cross to the watchdog thread. `with_repo_mut_timed` blocks until the watchdog returns, so those borrows
stay valid. The remove-then-insert window has no reentrancy risk (single pool thread, synchronous; the
watchdog thread has a separate thread-local).

**Leak-on-timeout semantics.** On inactivity timeout `returned == None`: the one `git2::Repository`
(its libgit2 odb/mmap handles) leaks with the already-abandoned worker thread — an ADDITIVE one-handle
leak on top of the existing one-thread leak, same bound (corrupt-object hits × user retries), not the
warm path. The map entry stays absent, so the next call reopens cleanly (self-healing; no stale entry
ever points at the moved-away handle). The watchdog still returns the timeout `AppError::Git` PROMPTLY
(the recv loop is unchanged) — abandonment safety preserved.

### Call-site rewrites — `src-tauri/src/commands/status.rs`

`get_status_inner`: move `with_repo` out of the timeout closure onto the pool thread.
```rust
tauri::async_runtime::spawn_blocking(move || {
    crate::repo_handle::with_repo_timed("read_status", &repo_id, generation, &path, &perf,
        move |_progress, repo| read_status_with(repo))   // single-shot: no tick, deadline bounds whole scan
}).await.map_err(..)?
```
`stream_graph`: same move; `perf` is needed on both threads, so clone it (it is `Arc<PerfState>` —
shared atomics, so both clones bump the same counters).
```rust
let perf_seam = state.inner().perf.clone();      // pool thread: open-count
let perf_walk = perf_seam.clone();               // captured into f (watchdog thread)
tauri::async_runtime::spawn_blocking(move || {
    crate::repo_handle::with_repo_mut_timed("stream_graph", &repo_id, generation, &path, &perf_seam,
        move |progress, repo| {
            crate::graph_cache::stream_graph_cached_with(repo, &cache, &perf_walk, |chunk| {
                progress.tick(); on_chunk.send(chunk).is_ok()
            })
        })
}).await.map_err(..)?
```
`get_graph_inner` is OUT of scope (uncached, off the hot path; keeps its own open + explicit
`inc_repo_opens`).

### Invariants carried over (unchanged)

- **Generation eviction:** `remove(&key)` matches only the exact `(id, gen)`; a bump misses → the
  `retain(id != repo_id)` branch drops the stale-gen handle and reopens. Identical to `with_repo_mut`.
- **`index.read(true)` freshness:** unchanged — it lives inside `read_status_with`, which still runs
  through the reused handle. Re-applied verbatim; now actually load-bearing (the handle IS reused).
- **Refs/odb re-read on demand:** unchanged — `graph_seed_with` / `stream_graph_cached_with` re-probe
  the reused handle, so topology stays current.
- **`repo_opens` counting:** bumped ONLY in step 1 on a real open; `stream_graph_cached_with` never
  bumps it. ⇒ a warm status+graph round on the same pool thread + gen = 0 opens.

### Acceptance criteria (FU-B2c)

- **(a) Cross-round reuse:** calling `with_repo_timed`(read_status) then `with_repo_mut_timed`(graph) on
  the SAME thread + generation → cold round `repo_opens == 1` (both share ONE handle keyed
  `(repo_id, gen)`), warm round `repo_opens == 0`. (Test directly on the test thread — do NOT go through
  `get_status_inner`'s `spawn_blocking`, whose pool-thread choice is nondeterministic.)
- **(b) Hang safety + acceptable leak:** `run_with_git_timeout_owned_with(_, 300ms, marker, hang)` →
  `(None, Err(AppError::Git(".. operation timed out ..")))` returned promptly (test finishes long before
  the hang); marker not returned. At the wrapper level (`with_repo_mut_timed_with` tiny deadline + hanging
  `f`): timeout Err returned, and the NEXT fast call on the same thread reopens (`repo_opens` bumps) —
  proves the abandoned handle was not re-cached. No shared-`&mut`.
- **(c) Generation evict:** `_timed` call at gen 1 opens+reuses (1 then 0); a gen-2 call reopens (1).
- **(d) Byte-identical:** `StatusSnapshot` from `with_repo_timed` == `read_status(path)`; the graph
  `Vec<GraphChunk>` collected through `with_repo_mut_timed` == the stream from a fresh
  `stream_graph_cached(path, ..)`. `GraphChunk` is `Serialize`-only (no `PartialEq`) ⇒ compare via
  `serde_json::to_value` per chunk.
- **(e) No test-count regression:** REPURPOSE the two now-false limitation tests —
  `get_status_opens_once_per_call` → `fu_b2c_status_reuses_across_rounds` and
  `stream_graph_path_opens_once_per_call` → `fu_b2c_graph_reuses_across_rounds` (both assert (a)) — and
  ADD the (b)/(c-timed)/(d) tests. Net count ≥ before; the `ac_b1`/`ac_b3`/`ac_b5` direct-path tests stay.

### File-size impact

- `timeout.rs` 231 → ~285 lines (two owned fns; existing `_with` shrinks if delegated) — under 500.
- `repo_handle.rs` 345 → ~450 lines (three `_timed` fns + swapped/added tests). Nears the ~500 ratchet:
  if it would cross ~480, extract `#[cfg(test)] mod tests;` into a sibling `repo_handle/tests.rs`
  (mirrors `graph_cache.rs`), in this same increment. `status.rs` net-neutral (~129).

### Flags for the orchestrator

- **OD-FU-B2c-1 (leak bound):** the one-handle leak-on-timeout is unbounded in principle (accumulates
  only per corrupt-object hit × retries, same bound as the already-accepted one-thread leak).
  **Recommend accept, no cap** — it self-heals (entry stays absent → reopen) and bounding it adds a
  counter for a pathological-only path. Confirm.
- **OD-FU-B2c-2 (de-dup):** delegate `run_with_git_timeout_with` to the owned variant (one recv loop) vs.
  keep two copies. **Recommend delegate.** Cosmetic; either is correct.
- **OD-FU-B2c-3 (get_graph):** left un-routed (uncached, off hot path). Confirm out of scope.
