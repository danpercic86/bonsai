# P86 — Refresh Caching & Scoped Rounds (Workstream B)

Status: contract (design). Builds on P85. Even after P85 kills the *double* refresh, **every**
round re-walks the ENTIRE graph and re-opens the repo per command. This contract makes an
unchanged-topology refresh cheap and makes each trigger refetch only what its change touched.

Invariants held: Rust owns the graph math **and** the caches; the IPC boundary and wire shapes
(`GraphChunk`, `GraphLayout`, `StreamNode`) are **unchanged**; `MAX_COMMITS=100_000` /
`STREAM_MAX_COMMITS=1_000_000` caps stay intact.

Three sub-increments: **B1 graph-layout cache**, **B2 repo-handle cache**, **B3 scoped rounds +
auto-fetch fold-in**. B1 and B3 deliver most of the win; B2 is a follow-on lever.

---

## Background (verified)

- `crates/bonsai-core/src/graph.rs`: `compute_graph:129`, `collect_seed:160`, `collect_refs:260`
  (enumerates every local + remote-tracking branch + every tag + HEAD; §284-359), `seeded_revwalk:182`
  (`TOPOLOGICAL|TIME`), `layout_walk:390`; streaming twin `graph/stream.rs:110` (`stream_graph_core`).
  `StreamNode` (`stream.rs:34`) carries `id` (oid) + `refs` + lane/summary/author/ts. No cache anywhere.
- `src-tauri/src/state.rs:12-18`: `RepoEntry { path, watcher }`; the doc-comment flags a cached
  `git2::Repository` as an unimplemented perf lever. `AppState { repos: Mutex<HashMap<..,RepoEntry>>, active_repo }`.
- `src-tauri/src/commands/status.rs:66` `stream_graph` and `:33` `get_graph`, `:7` `get_status`
  (`read_status`, `git/status.rs:134`, `recurse_untracked_dirs(true)` = O(worktree)) each do
  `repo_path(state,id)` (`commands/shared.rs:127`, brief map lock) then `spawn_blocking` open-from-path.
- `RepoWorkspace.tsx` `runRefreshRound:1230-1251` = `openRepo` + 11 refetches regardless of cause.
- `src-tauri/src/scheduler.rs:412-441` auto-fetch tick emits `repo-changed` (updated>0).

---

## B1 — Graph-layout cache (Rust, per repo)

### Key idea

A branch create/delete/rename at an **existing** commit leaves the walk (nodes, lanes, edges,
ordering) identical — only the ref pills differ. Separate the **expensive walk** from the **cheap
decoration**, and key the cache on a topology fingerprint derived from the walk *seed*.

### Fingerprints (both derived from `collect_seed`, no walk)

- `seed_fp` — the WALK identity: hash of `(sorted tip oids, head oid, sorted hide oids)`.
- `deco_fp` — the DECORATION identity: hash of the full `RefMap` (each `oid → sorted RefLabels`)
  plus `head_branch` + `detached`.

`collect_seed` (refs + tips + head + hide) is already computed before every walk and is
O(refs), not O(commits) — cheap enough to run as the cache probe on each request.

### Classification (given a fresh seed vs the cached entry)

```
if cache is None                                            -> Miss
let new_tips = set(new.tips)
if new_tips == cache.tips && new.head == cache.head && new.hide == cache.hide:
    if new.deco_fp == cache.deco_fp                         -> HitVerbatim   # identical layout
    else                                                    -> HitRedecorate # e.g. rename at same oids
if new.hide == cache.hide
   && cache.tips ⊆ new_tips                                 # no tip removed (no commit dropped)
   && new_tips ⊆ cache.node_oids:                           # every new tip is already a node (no new commit)
    -> HitRedecorate                                        # e.g. branch create at an existing commit
otherwise                                                   -> Miss
```

Soundness: `cache.tips ⊆ new_tips` ⇒ `reachable(new) ⊇ reachable(old)`; `new_tips ⊆ node_oids`
⇒ every new tip (incl. HEAD, which `collect_refs` pushes as a tip) already lies in the walked set
⇒ `reachable(new) ⊆ reachable(old)`. Together ⇒ equal reachable set + same deterministic
`TOPOLOGICAL|TIME` order ⇒ identical `nodes/lanes/edges`. A branch **delete** shrinks `tips`
(fails ⊆) ⇒ Miss ⇒ correct re-walk. A commit / HEAD advance introduces a tip oid ∉ `node_oids`
⇒ Miss ⇒ correct re-walk. A truncated cache (prefix of history) can only *fail* the ⊆ test for a
tip beyond the cap ⇒ safe Miss.

### Cached representation

Cache the exact streamed bytes so a hit re-emits with zero re-serialization risk:

```rust
// src-tauri/src/graph_cache.rs  (new; command-layer, owns AppState-adjacent state)
pub struct CachedGraph {
    pub seed_fp: u64,
    pub deco_fp: u64,
    pub tips: std::collections::BTreeSet<git2::Oid>,
    pub head: Option<git2::Oid>,
    pub hide: std::collections::BTreeSet<git2::Oid>,
    pub node_oids: std::collections::HashSet<git2::Oid>,
    pub chunks: Vec<bonsai_core::graph::GraphChunk>, // Meta … Batch* … Done, exact wire order
}
```

- **HitVerbatim** — replay `chunks` through the channel; no git work.
- **HitRedecorate** — rewrite `StreamNode.refs` in-place across `chunks` from the fresh `RefMap`
  (O(nodes), no revwalk, no object reads), recompute `Meta.head_oid` + `Done.head_index` from the
  fresh head, update `deco_fp`, then replay. Add a bonsai-core helper:
  ```rust
  // graph/stream.rs — pure, no repo
  pub fn redecorate_chunks(chunks: &mut [GraphChunk], refs: &RefMap, head: Option<git2::Oid>);
  ```
  (`RefMap` = `HashMap<git2::Oid, Vec<RefLabel>>`; expose the alias `pub type RefMap` from `graph.rs`.)
- **Miss** — walk via `stream_graph_core`, **tee** each emitted chunk into both the channel and a
  local `Vec<GraphChunk>` buffer; on the terminal `Done`, build+store the `CachedGraph`. Cold-path
  streaming stays fully incremental (no server-side buffering delay before first paint).

### Where it lives / concurrency

Add to `RepoEntry` (state.rs):
```rust
pub struct RepoEntry {
    pub path: PathBuf,
    pub watcher: Option<WatcherHandle>,
    pub graph_cache: std::sync::Arc<std::sync::Mutex<Option<CachedGraph>>>, // NEW
}
```
`stream_graph`/`get_graph` clone the `Arc` out under the brief `repos` map lock (same pattern as
`path`), then lock the per-repo cache mutex **only** for the classify + replay/store — never across
a cold walk (the walk streams while holding nothing but the channel). Serialized graph requests
per repo are fine (the frontend issues one at a time via `graphReqId`). Cleared implicitly when the
`RepoEntry` is removed on `close_repo`; re-`open_repo` (idempotent re-arm) must **reset** it to
`None` (topology may have changed while closed).

### Command wiring (status.rs)

`stream_graph` and `get_graph` gain a private cache-aware path. Signatures on the wire are
unchanged. Sketch:
```
seed = bonsai_core::graph::graph_seed(&path)?     // NEW public: collect_seed w/o walking
match classify(&cache, &seed) {
  HitVerbatim   => replay(cache.chunks)
  HitRedecorate => { redecorate_chunks(&mut chunks, &seed.refs, seed.head); replay; store }
  Miss          => { tee stream_graph_core into channel + buffer; store on Done }
}
```
New bonsai-core surface (small, additive):
```rust
// graph.rs
pub struct GraphSeed { pub refs: RefMap, pub tips: Vec<git2::Oid>,
                       pub head: Option<git2::Oid>, pub hide: Vec<git2::Oid> }
pub fn graph_seed(workdir: &std::path::Path) -> Result<GraphSeed, AppError>; // opens, collect_seed, returns
```
`compute_graph`/`stream_graph_core` keep working unchanged (they can call `graph_seed` internally
to avoid duplicating `collect_seed`).

Invalidation rules (all automatic via the fingerprint, plus explicit resets):
- commit / merge / rebase / cherry-pick / revert / reset / pull / any HEAD move → new head or tip
  oid ∉ node set → **Miss** (auto).
- branch create/rename at existing oid → **HitRedecorate** (auto).
- branch/remote-tracking delete, stash push/pop (hide change) → **Miss** (auto).
- `close_repo` → entry dropped. `open_repo` re-arm → set cache `None`.

---

## B2 — Repo-handle cache (Rust)

`git2::Repository` is `Send` but **not `Sync`**, and commands run on the `spawn_blocking` pool —
that is exactly why `state.rs:12` never cached it: it cannot be shared behind a plain `&`.

Trade-off of the three options:
- **`Mutex<Repository>` per repoId** — sound, simplest, but serializes the round's parallel
  refetches (they run as concurrent `spawn_blocking` tasks) onto one handle → can be *slower* than
  N parallel opens. Rejected as the default.
- **Small pool per repoId** — more parallelism, more memory + checkout/return plumbing. Overkill.
- **Thread-local per (repoId, generation)** — RECOMMENDED. Each blocking-pool thread keeps its own
  handle, so there is no cross-thread sharing (no `Sync` needed) and no mutex contention; the
  existing "one open per task" parallelism is preserved, just amortized.

### Design (thread-local + generation)

```rust
// AppState — monotonic per-repo generation, bumped on close/re-open to evict stale handles.
pub repo_generations: Mutex<HashMap<String, u64>>,  // NEW; or an AtomicU64 inside RepoEntry

// src-tauri/src/repo_handle.rs (new)
thread_local! { static HANDLES: RefCell<HashMap<(String,u64), git2::Repository>> = …; }

/// Run `f` against a cached open Repository for (repo_id, generation), opening
/// (NO_SEARCH) on first use per thread and reusing thereafter. On a generation
/// bump the stale entry is dropped and reopened. Returns f's result.
pub fn with_cached_repo<R>(
    repo_id: &str, gen: u64, path: &Path,
    f: impl FnOnce(&git2::Repository) -> Result<R, AppError>,
) -> Result<R, AppError>;
```
Soundness: entries are per-thread (never shared), keyed by generation so a close/re-open (or a
detected repo-dir move) evicts them. A held handle re-reads refs/odb lazily on each call, so it is
never "stale" for reads within a generation. Bound: ≤ (pool threads × open repos) handles; evicted
on generation bump; capped/LRU only if a leak shows up (flag).

### Scope caveat (why B2 is staged last)

The core git fns take `&Path` and open internally (`read_status(&path)`, `compute_graph(&path)`,
`branches::list_refs(&path)`, …). To reuse a cached handle they need `&Repository`-taking variants.
That is a broad but mechanical refactor. **Land B2 incrementally**: introduce `with_cached_repo`
and convert the hottest per-round callers first — `graph_seed` (runs on every graph request incl.
cache hits), `read_status`, `list_refs` — leaving the rest on open-from-path. Do **not** block B1/B3
on it. If the refactor proves large, ship B1+B3 and file B2 as a follow-up.

---

## B3 — Scoped refresh rounds (frontend) + auto-fetch fold-in

Today every trigger runs the full 11-slice round. Scope each round to the change reason so a
ref-only mutation never pays the O(worktree) `get_status` scan or (with B1) a graph walk.

### Scope type + slice matrix

```ts
// src/components/repoWorkspace/useCoalescedRefresh.ts (or a sibling refreshScope.ts)
export type RefreshScope = 'full' | 'refsOnly' | 'remoteMeta' | 'worktree';
```

Slices (from `runRefreshRound`): `openRepo, status, graph, branches, stashes, submodules,
worktrees, remotes, opState, compare, tagSync`.

| scope | openRepo | status | graph | branches | remotes | compare | opState | stashes | submodules | worktrees | tagSync |
|---|---|---|---|---|---|---|---|---|---|---|---|
| `full` | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | ✓(forced per origin) |
| `refsOnly` | – | – | ✓* | ✓ | – | ✓ | – | – | – | – | – |
| `remoteMeta` | – | – | ✓* | ✓ | ✓ | ✓ | – | – | – | – | ✓(non-forced) |
| `worktree` | – | ✓ | – | – | – | – | ✓ | – | – | – | – |

\* the `graph` slice is now cheap under B1 (Redecorate/Verbatim hit for ref-only changes).
`refsOnly`/`remoteMeta`/`worktree` skip `openRepo` because they never move HEAD (HEAD-moving ops go
through `full`) — the header HEAD label and watcher self-heal are unaffected. Matrix is
conservative: when a handler is unsure, it uses `full`.

### Trigger → scope map

- **Mutation handlers** pass an explicit scope (P85 A1 already threads it):
  - `refsOnly`: create/rename branch (non-head), delete remote-tracking, push/force-push, tag
    create/delete.
  - `remoteMeta`: fetch.
  - `worktree`: stage/unstage/discard (index-only; no commit, no HEAD move).
  - `full`: commit, merge, rebase, cherry-pick, revert, reset, pull, checkout, create-branch-here,
    branch delete, rename-of-HEAD, stash apply/pop/drop, submodule/worktree ops, pending-op apply.
- **`watcher`** — mapped by `RepoChangedPayload.reason`: `"fetch"` → `remoteMeta`, `"tags"` →
  `refsOnly`, `"fs"`/unknown → `full` (the watcher can't know the cause; full is safe and cheap
  under B1).
- **`manual` / `focus` / `activation`** → `full` (unchanged self-heal guarantees; these also keep
  the P81 forced `tagSync`).

### API + `runRefreshRound` change

```ts
// P85 already made refreshAll accept a scope:
const refreshAll = useCallback(
  (scope: RefreshScope = 'full') => refresh('mutation', scope), [refresh]);
// refresh gains scope; watcher derives it from reason:
refresh(origin: RefreshOrigin, scope?: RefreshScope): Promise<void>;
```
`runRefreshRound(scope)` runs only the matrix's slices; `else`-branch (unusable repo) clear-all is
unchanged. The coalescer collapses overlapping requests as in P81 — **but** because different
scopes can collapse, a trailing round must run the **union** of pending scopes (track a pending
scope set; the trailing round widens to their union; `full` dominates). Specify: `request(scope)`
accumulates `pendingScope`; the round reads and clears it.

### Auto-fetch fold-in (backend, tiny)

`scheduler.rs` auto-fetch tick already emits `repo-changed` on `updated>0`. Emit it with
`reason: "fetch"` so the frontend maps it to `remoteMeta` instead of `full`. `HealthRefresh`
emits keep `reason:"fs"` (→ full) unless a lighter scope is wanted (flag). Backward-compatible:
old frontends treat any reason as full.

---

## Instrumentation (how the tester asserts caching + scoping)

Backend `PerfCounters` (atomics) in `AppState`, incremented at the seams:

```rust
// src-tauri/src/perf.rs (new)
#[derive(Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerfCounters {
    pub repo_opens: u64,        // open_no_search / with_cached_repo miss
    pub graph_walks: u64,       // stream_graph_core / compute_graph actually walked
    pub graph_cache_hits: u64,  // HitVerbatim
    pub graph_redecorates: u64, // HitRedecorate
    pub status_scans: u64,      // read_status entered
}
pub struct PerfState { /* AtomicU64 fields */ }  // in AppState

#[tauri::command] pub async fn debug_perf_counters(state) -> Result<PerfCounters, AppError>;
#[tauri::command] pub async fn debug_reset_perf_counters(state) -> Result<(), AppError>;
```

- TS: `debugPerfCounters(): Promise<PerfCounters>` + `debugResetPerfCounters(): Promise<void>` on
  the ipc API; **mock** returns a zeroed/echoing fixture so the harness compiles.
- Frontend round/scope tally: extend P85's `window.__bonsaiRefreshRounds` with a per-scope map
  `window.__bonsaiRefreshScopes: Record<RefreshScope, number>` for e2e assertions; vitest uses the
  `run`/spy pattern.

---

## Acceptance criteria

- **AC-B1a:** create a branch at an existing commit → `graph_walks` unchanged, `graph_redecorates`
  +1, and the emitted layout's `nodes/lanes/edges` are byte-identical to the prior layout except
  the new pill (fixture assertion in bonsai-core + a command-level counter assertion).
- **AC-B1b:** commit / HEAD advance → `graph_walks` +1 (Miss), counters prove no false hit.
- **AC-B1c:** two consecutive identical graph requests with no repo change → 2nd is `HitVerbatim`
  (`graph_walks` unchanged, `graph_cache_hits` +1); output identical.
- **AC-B1d:** branch delete that drops commits → Miss (`graph_walks` +1); output correctly shrinks.
- **AC-B2:** with the handle cache, a `full` round performs **≤1** `repo_opens` per pool thread per
  generation (not 11); a generation bump forces a reopen.
- **AC-B3a:** a `refsOnly` mutation → `status_scans` unchanged (no worktree scan) and (with B1)
  `graph_walks` unchanged.
- **AC-B3b:** a `worktree` mutation (stage) → `status_scans` +1, `graph_walks` unchanged,
  `branches` not refetched.
- **AC-B3c:** auto-fetch tick with `updated>0` → frontend runs a `remoteMeta` (not `full`) round.
- **AC-B4 (end-to-end):** the P85 branch-create fixture, now with B1+B3, does **0** graph walks and
  **0** status scans (down from 2 walks pre-P85), asserted via `PerfCounters`.
- **Caps intact:** `MAX_COMMITS`/`STREAM_MAX_COMMITS` unchanged; a truncated cache never yields a
  wrong hit (AC covers the ⊆-with-truncation safe-Miss).

---

## File-size / split notes

- New: `src-tauri/src/graph_cache.rs` (~150), `src-tauri/src/repo_handle.rs` (~80),
  `src-tauri/src/perf.rs` (~60). `graph_seed` + `redecorate_chunks` add ~60 lines to
  `graph.rs`/`graph/stream.rs` — both already near their budgets; if either crosses ~500, move the
  seed/decoration helpers into a new `graph/seed.rs` / `graph/decorate.rs`.
- `status.rs` gains the cache-aware graph path (~40 lines); if it crosses ~500, split the graph
  commands into `commands/graph.rs`.
- Frontend: `RefreshScope` + matrix live in a new `refreshScope.ts` (~40) so `useCoalescedRefresh.ts`
  and `RepoWorkspace.tsx` stay small.

---

## Flags for the orchestrator

- **OD-P86-1 (B2 approach):** thread-local (recommended) vs `Mutex<Repository>`. Thread-local
  preserves the round's parallel fan-out; confirm before the core-fn `&Repository` refactor.
- **OD-P86-2 (B3 openRepo skip):** the scoped rounds skip `openRepo` (header/self-heal). Confirmed
  safe because HEAD-moving ops use `full`; flag if you want `openRepo` retained everywhere (cheap
  once B2 lands).
- **Sequencing:** B1 (biggest win, self-contained) → B3 (scoping + auto-fetch) → B2 (handle cache,
  staged refactor). B1 depends on the P85 round being single (else two walks still race). B3's
  `refsOnly`/`remoteMeta`/`worktree` scopes are the ones A1 already passes.
- **Open question:** should `worktree` scope also refetch `graph` for the future WIP row (Polish)?
  Deferred — WIP row is not MVP; add `graph` to the `worktree` matrix when it lands.
