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
