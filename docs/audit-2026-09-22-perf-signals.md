# Perf-signal investigation — 2026-09-22 Dev-mode session

**Scope:** the two low-severity perf signals raised from the 2026-09-22 Dev-mode
session — (1) graph cache hit rate ~8%, (2) `redundant-refresh` × 23.
**Status:** diagnosed, read-only. No code changed by this investigation.

## Evidence base

| Source | Note |
|---|---|
| `%APPDATA%\com.bonsai.app\logs\bonsai-2026-09-22T04-52-03-scca8269e.jsonl` | 19,547 records, wall span 152.4 min, 5 repos open. **Primary source.** |
| `%APPDATA%\com.bonsai.app\metrics\usage.json` (day `2026-09-22`) | Day roll-up across **9 sessions** — a superset of the session above. |
| `usage.json.bak` (10:31) vs `usage.json` (10:42) | Used to verify counter-delta consistency. |

### Reconciling the three sets of numbers

The three figures in circulation are all correct for different windows:

| | walks/misses | verbatim hits | redecorates | hit rate |
|---|---|---|---|---|
| Reported in the signal | 60 | 5 | — | 8% |
| **This session (log spans)** | **75** | **6** | **0** | **7.4%** |
| Day `usage.json` (9 sessions) | 97 | 6 | 0 | 5.8% |

The reported 60/5 is an earlier snapshot of the same session. All analysis below
uses the session log. Note `perf.graph_redecorates` is absent from `usage.json`
(counters are minted only when non-zero) — i.e. **`HitRedecorate` fired zero
times all day**, which is itself part of finding 1.

Cost of the graph misses in this session, measured from the `graph.get` spans:

- 75 misses — 14,448 ms total, mean 193 ms; of that **12,220 ms is `revwalk` + `lane`** (mean 163 ms).
- 6 hits — 286 ms total, mean 48 ms.

---

## Finding 1 — graph cache: REAL DEFECT (low severity, design-level)

**Verdict: real defect.** 62 of this session's 81 graph requests (77%) were
**structurally guaranteed misses** — the cache was destroyed immediately before
each of them by design, not by a topology change.

### The cache is per-repo — the single-slot hypothesis is FALSE

`GraphCache = Mutex<Option<CachedGraph>>` lives in each `RepoEntry`
([graph_cache.rs:84](src-tauri/src/graph_cache.rs:84),
[state.rs](src-tauri/src/state.rs)), is cloned out per request as an `Arc` by
`repo_path_and_graph_cache` ([shared.rs:168](src-tauri/src/commands/shared.rs:168)),
and is looked up by `repo_id` in `stream_graph`
([status.rs:145](src-tauri/src/commands/status.rs:145)). Five open repos get five
independent cache slots. `graph_cache.rs` itself is correct and is **not** the
defect.

### Root cause: every `full` refresh round wipes the cache it is about to use

Two independently correct decisions compose into a guaranteed miss:

1. **`refreshScope.ts` puts `openRepo: true` and `graph: true` in the same
   scope.** The `full` slice matrix
   ([refreshScope.ts:81](src/components/repoWorkspace/refreshScope.ts:81)) calls
   `openRepo` (for the usability check + watcher self-heal) *and* refetches the
   graph in the same round.
2. **`open_repo_inner` unconditionally re-arms the `RepoEntry`.** The dedupe scan
   at [repo.rs:232](src-tauri/src/commands/repo.rs:232) only *reuses the key* of
   an already-open repo — there is no early return. Control always falls through
   to `repos.insert(...)` with a **fresh `graph_cache: Arc::new(Mutex::new(None))`**
   ([repo.rs:293](src-tauri/src/commands/repo.rs:293)), plus a
   `bump_repo_generation` ([repo.rs:305](src-tauri/src/commands/repo.rs:305)).

So: `full` round → `openRepo` → cache set to `None` → `streamGraph` → `Miss`.

The code comment justifies the wipe as *"a re-arm of an open repo must start
`None` — topology may have changed while closed."* That reasoning does not hold
for a repo that was **never closed**, and it is redundant even if it had been:
`classify` ([graph_cache.rs:167](src-tauri/src/graph_cache.rs:167)) compares the
**exact** `(tips, head, hide)` sets from a freshly probed seed, so any topology
change — including a repo swapped on disk — already forces a Miss. The wipe buys
no correctness; it only discards a valid cache.

### Confirming numbers (session log)

- **62 `full`-scope refresh rounds**, and **63 `openRepo` calls** — 62 rounds plus
  one user-initiated open. Every `full` round emits exactly one `openRepo`.
- **All 62** `full` rounds have both an `openRepo` and a `streamGraph` inside
  their execution window.
- Graph-touching scopes: `full` 62 + `remoteMeta` 15 + `stash` 3 = 80, plus the
  initial load = **81 `graph.get` spans**. Matches exactly.
- **Excluding the 62 forced misses, the cache served 6 hits in 19 eligible
  requests (32%)**, and the 13 eligible misses are almost fully accounted for by
  legitimate causes: 3 `stash` rounds (a hide-set change is a by-design Miss),
  ~6 new commits in `bonsai` itself (its node count climbs 4926 → 4932 across the
  session), and 7 `fetch` calls.
- The clinching evidence is node-count stability. Repos whose walked node count
  **never changes** still miss on nearly every request:
  `items=1041` → 14 requests, 1 hit · `items=5376` → 14 requests, 1 hit ·
  `items=5374` → 8 requests, 1 hit · `items=1009` → 6 requests, 0 hits ·
  `items=409` → 4 requests, 0 hits. Identical topology, repeated full re-walks.
- `HitRedecorate` never fired (0 all day) for the same reason: a redecorate needs
  a populated cache, and the cache was empty at almost every request.

### Recoverable cost

~62 × (193 − 48) ms ≈ **9.0 s of blocking-pool time in a 152-min session**, and
~10.1 s of that is pure `revwalk` + `lane` work. Real, and consistent with the
signal's "not dominant" framing.

### Secondary effect: `perf.repo_opens` = 564/day

Same root cause. Each `openRepo` also calls `bump_repo_generation`, which evicts
the pool's cached `git2::Repository` handle for that repo, so the commands
following each `full` round re-open it. 62 evictions/session is why 564 opens
appear against ~1,000 commands. Follow-up, not part of the fix below.

### Correcting the `cache-collapse` framing

The signal attributed the `cache-collapse` anomaly to *"the user switching
between 5 repos in ~100 seconds."* The log does not support that. Both firings
(there are **two**, at ts 1790059701129 and ts 1790061507559 — the second was not
reported) have five contributing spans with `items` = 1041, 1009, 5376, 4926,
4927 — **five different repos**, spanning **217 ms**, not 100 s. This is all five
open repos doing one `full` fan-out at once (focus/activation), each forced to
miss by its own wipe.

Two consequences:

- The *misses* the rule caught are real — they are the wipe.
- The rule's **window is cross-repo** (see finding 2): it pooled one first-walk
  from each of five repos and read it as one repo's collapsed cache. Five repos
  each walking once is not a collapse.

---

## Finding 2 — `redundant-refresh`: MOSTLY CORRECT BEHAVIOUR + a monitoring defect

**Verdict: the app is behaving correctly. The detector is wrong.** Of 26 firings
in this session, **at least ~17 are false positives**, and **at most ~9** are genuine same-repo
double rounds costing **≤ ≈0.8 s in total over 152 minutes**. The 300 ms debounce needs no
change. (The 17/9 split was originally stated as exact; see the CORRECTION under
"Separating the two populations" — the mutation clause used to classify them is
itself broken by a casing mismatch, so 9 is a ceiling and the false-positive
share can only be larger. The verdict is unaffected, and is in fact reinforced.)

### The detector is repo-blind

`detect_redundant_refresh`
([window.rs:152](src-tauri/src/obs/anomaly/window.rs:152)) finds the previous
refresh event with a matching **`scope` and nothing else**:

```rust
.find(|e| e.key == scope)
```

It cannot do better, because **`RefreshPayload` carries no repo id**
([types.ts:108](src/obs/types.ts:108) — `round`, `scope`, `origins`,
`contributingTraces`, `collapsed`, `ms`). `useCoalescedRefresh` *receives*
`repoId` ([useCoalescedRefresh.ts:67](src/components/repoWorkspace/useCoalescedRefresh.ts:67))
but never emits it. With 5 repos open — 5 watchers, 5 coalescers — two *different*
repos each doing one legitimate `worktree` refresh within 1000 ms is flagged as
one repo refreshing twice. The same blindness affects `cache-collapse`
(`graph.get` spans carry no repo id either) and the `mutations` list, which can
suppress a genuine finding in repo B because repo A mutated.

### Separating the two populations

`round` is a `useRef` inside `useCoalescedRefresh`, i.e. **per repo**
([useCoalescedRefresh.ts:84](src/components/repoWorkspace/useCoalescedRefresh.ts:84)).
Consecutive round numbers ⇒ same repo; a jump or a decrease ⇒ two repos.

**Cross-repo (false positives) — ~17 of 26.** Round pairs 9→28, 21→17
(*decreasing*), 29→41, 38→50, 40→53, 41→54, 42→30, 43→56, 44→57, 58→45, 59→28,
34→48, 57→67, 35→58, 36→63 …

**This includes all five `full`-scope pairs** — the ones carrying the real cost.
Attributing each side by the `openRepo` `args.path` in its window confirms
multiple distinct repos in every case. **The headline "the `full`-scope pair cost
541 ms + 684 ms" is a false positive**: that is round 9 of one repo and round 28
of another, 141 ms apart, each doing one legitimate full refresh. The A-side and
B-side rounds even form two strictly increasing series (A: 34, 35, 36 / B: 48, 58,
63) — two distinct repos, tracked separately.

**Same-repo (genuine) — 9 pairs**, all `worktree` scope, rounds 9→10, 24→25,
11→12, 13→14, 18→19, 35→36, 50→51, 60→61, 62→63; Δt = 879, 381, 857, 477, 417,
796, 384, 607, 935 ms. Second-round cost: 16, 20, 247, 199, 88, 45, 23, 60,
107 ms = **805 ms total**.

> **CORRECTION (2026-09-22, from the P117 increment-2 review — read this with the 9).**
> Treat **9 as an upper bound, not a count.** The classification above leans on the rule's
> "no intervening mutation" clause, and that clause is far weaker than this report assumed:
> `is_mutation_cmd`'s table is **snake_case** (`anomaly.rs:326-366`) while the only producer of
> `ipc.call` is `wrapMethod`, which logs the JS property name verbatim — **camelCase**
> (`ipcProxy.ts:112,129`) — and Rust's snake_case `ipc.recv` is consumed by no rule
> (`anomaly.rs:173` is `_ => {}`). So only the **13 single-word prefixes** ever match: `commit`,
> `stage`, `unstage`, `discard`, `checkout`, `merge`, `rebase`, `cherrypick`, `revert`, `reset`,
> `fetch`, `pull`, `push`. **34 plausible mutations never register at all**, including `forcePush`,
> `cloneRepo`, `initRepo`, `createBranch`, `deleteBranch`, `createTag`, `createStash`, `applyStash`,
> `addRemote`, `add`/`removeWorktree`, `bisect*` and `autoSyncTags`. Some of the 9 may therefore be
> legitimate post-mutation refreshes that the rule simply failed to recognise as such.
>
> This **strengthens** the finding's verdict rather than weakening it — the false-positive share is
> larger than the ~17 estimated, not smaller — but the specific figure 9 is no longer defensible as
> a true-positive count, and the cost figure (805 ms) is correspondingly an upper bound too.
> Independently: **scheduled auto-fetch mutations are invisible to the clause in every mode**, before
> and after P117 — periodic fetch runs entirely in Rust (`scheduler/exec.rs:151` calls `fetch_all`
> directly) and `LogPayload::IpcCall` has no Rust producer, so a `cache-collapse` immediately after
> an auto-fetch tick is an *unsuppressable* false positive. Filed as its own task; fixing it needs a
> Rust-side `repo` producer, which `record.rs`'s three-producer allow-list forbids without a
> contract change.

### The genuine 9 are also correct behaviour

Every Δt exceeds the 300 ms `DEBOUNCE`
([watcher/mod.rs:89](src-tauri/src/watcher/mod.rs:89)), which is a **trailing-edge,
quiet-period** debounce. A second round means the watcher observed *new*
filesystem events after a ≥300 ms quiet gap — the debounce did precisely what it
is specified to do. The rule's "no intervening mutation" clause only tracks
*Bonsai-originated* mutations; a watcher-origin refresh is by definition external
(another tool writing in the worktree), so that clause can never be satisfied for
this population and contributes nothing.

**Recommendation: do not touch `DEBOUNCE` and do not add a post-fire quiet
window.** 805 ms over 152 minutes does not justify changing an architecture
invariant, and a quiet window would trade a real correctness property (prompt
reaction to external writes) for that. The fix belongs in the observability rule.

---

## Recommended actions

| # | Item | Kind | Owner |
|---|---|---|---|
| 1 | Preserve the existing `graph_cache` `Arc` when `open_repo_inner`'s dedupe scan finds an already-open entry at the same path; still replace the watcher (self-heal). Contradicts the P86 B1 contract line *"reset to `None` on `open_repo` re-arm"* ⇒ **contract change**. | fix | `architect` → `senior-dev` |
| 2 | Add `repoId` to `RefreshPayload` and to the `graph.get` span record; key `detect_redundant_refresh` / `cache-collapse` / the `mutations` list on `(repoId, scope)`. P91 §2.5 + §5.1 ⇒ **schema change**. | fix | `architect` → `senior-dev` |
| 3 | `perf.repo_opens` = 564/day: `bump_repo_generation` on every `full` round evicts the pool handle cache. Same root cause as #1; re-measure after #1 lands before changing anything. | follow-up | — |
| 4 | Raising `DEBOUNCE` / adding a quiet window. | **rejected** | — |

### Open question (not a claimed defect)

Day-wide `perf.graph_walks` = 103 but only 97 `graph.get` spans ran a `revwalk`
phase; `op.graph.get` = 103 = 97 misses + 6 hits, which is internally consistent.
The counter therefore over-reports by exactly the hit count (6). The offset is
constant — it is already +6 in `usage.json.bak`, and the delta across the
observed session was consistent (+5 calls / +5 misses / +5 walks / +0 hits) — so
it accrued in an earlier session of the day and cannot be attributed from static
reading. The only production site that could do it, `get_graph_inner`
([status.rs:110](src-tauri/src/commands/status.rs:110)), bumps `graph_walks` and
`repo_opens` without emitting a span, but it has no production caller and no
`cmd.getGraph` duration was recorded; the MCP `bonsai_get_graph` tool calls
`bonsai_core::graph::compute_graph` directly and touches no counters. Flagged for
a future session; **do not fix blind**. Practical note: use
`hits / (hits + misses)` from the `graph.get` spans as the hit rate, not
`hits / graph_walks`.
