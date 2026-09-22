# P117 — Two perf-signal fixes: graph-cache preserve-on-re-arm + repo-keyed anomaly rules

Diagnosis (read-only, already done, do NOT redo): `docs/audit-2026-09-22-perf-signals.md`.
Board entry: `TODO.md` `## P117`.

**Two independent sections. Two separate senior-dev increments.** Section 1 is Rust-only and
touches no IPC surface. Section 2 changes the obs record schema on both sides. Neither depends on
the other; either may land first.

---

# Section 1 — graph cache: preserve the slot on a same-path re-arm

One-line summary of the defect: `open_repo_inner` always inserts a fresh `RepoEntry` with
`graph_cache: None`, and the `full` refresh scope calls `openRepo` and refetches the graph in the
same round, so 62 of 81 graph requests in the measured session were structurally guaranteed misses
and `HitRedecorate` fired 0 times all day.

## 1.1 Superseded prior-contract clauses

**`docs/contracts/archive/P86-refresh-caching.md:116-117`** (quoted verbatim):

> Cleared implicitly when the `RepoEntry` is removed on `close_repo`; re-`open_repo` (idempotent
> re-arm) must **reset** it to `None` (topology may have changed while closed).

**`docs/contracts/archive/P86-refresh-caching.md:146`** (verbatim):

> `close_repo` → entry dropped. `open_repo` re-arm → set cache `None`.

**Both are superseded by §1.3 below.** The `close_repo` half of each sentence stands unchanged.

The same rule is restated in three doc comments that MUST be updated in this increment (they are
the durable copies of the superseded clause; leaving them is a contract lie):

| Site | Text to replace |
|---|---|
| `src-tauri/src/commands/repo.rs:291-292` | `// Fresh empty layout cache (P86 B1): a re-arm of an open repo must start None — topology may have changed while closed.` |
| `src-tauri/src/state.rs:63-67` | `… a new RepoEntry on open_repo re-arm starts None again (topology may have changed while closed).` |
| `src-tauri/src/graph_cache.rs:82-85` | `/// Per-repo cache slot. None until the first walk; reset to None on open_repo re-arm (a fresh RepoEntry is inserted) …` |

Also `grep -n "re-arm" docs/architecture-reference.md` and fix any copy of the rule there.

## 1.2 What "same path" means here — the exact comparison

Three distinct values are in play in `open_repo_inner`:

- `info.path` — the candidate `repoId`. **CORRECTION (2026-09-22, from the inc-1 review): this is
  the RAW caller string, not a canonical path.** `read_repo_info` returns `path.to_string_lossy()`
  verbatim (`crates/bonsai-core/src/git/repo.rs:43`, `:64`) — it never calls `canonicalize` and
  never consults `repo.workdir()` — and nothing canonicalises before `open_repo_inner`. So the map
  key is the raw string of whichever open came first. The pre-existing comment at `repo.rs:225`
  ("repoId == canonical workdir path string") carries the same inaccuracy. The *comparison*
  described below is what the code does; only this description of the value was wrong, and the
  fix's safety does not depend on it.
- the existing map **key** `k` found by the dedupe scan, via
  `same_repo_path(k, &candidate)` (`repo.rs:253-260`), which canonicalises both sides. On a match,
  `repo_id` is reassigned to that existing key.
- `workdir` = `PathBuf::from(&info.path)`, stored as the new entry's `path`.

**The carry-over test is NOT the dedupe-scan result.** It is: *does an entry already exist under
the FINAL `repo_id` key at the moment of the insert, read under the same map-lock acquisition as
the insert?* That is strictly better than keying off the scan, because:

- it closes the scan's documented TOCTOU (a tab closed between the snapshot and the insert ⇒ `get`
  returns `None` ⇒ fresh cache, which is exactly the correct cold start);
- it needs no extra state threaded from the scan;
- it is a single `HashMap::get` under a lock already being taken.

## 1.3 New rule (normative)

`open_repo_inner`, inside the existing `repos` lock scope at `repo.rs:280-296`:

```rust
let previous = {
    let mut repos = state.repos.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    // P117: a re-arm of an entry ALREADY PRESENT under this exact key carries its
    // layout cache over. See §1.4 for why this cannot serve a stale layout.
    let carried = repos
        .get(&repo_id)
        .map(|e| std::sync::Arc::clone(&e.graph_cache));
    repos.insert(
        repo_id.clone(),
        RepoEntry {
            path: workdir,
            watcher,
            graph_cache: carried
                .unwrap_or_else(|| std::sync::Arc::new(std::sync::Mutex::new(None))),
        },
    )
};
drop(previous);
```

Everything else in `open_repo_inner` is **unchanged**, specifically:

1. The watcher is still built fresh outside the lock and installed on every re-arm; the replaced
   entry is still dropped off-lock so the old watcher's debounce thread joins there. The self-heal
   is preserved verbatim — it is the reason `full` calls `openRepo` at all.
2. `bump_repo_generation(state, &repo_id)` (`repo.rs:305`) still runs unconditionally.
3. The non-usable-open path (non-repo / bare) still inserts nothing and touches no entry.
4. `GraphCache`, `CachedGraph`, `classify`, `stream_graph_cached*` and
   `repo_path_and_graph_cache` are **not modified at all**. No signature in `graph_cache.rs`
   changes. No IPC surface changes. `src/ipc/mock.ts` needs no change.

## 1.4 Soundness argument (this is what justifies superseding P86 B1)

The wipe was never a correctness mechanism. `classify` (`graph_cache.rs:164-194`) is an
**exact-set** comparison of `(tips, head, hide)` against a seed probed fresh from the live
repository on every single request, gated first by `filter.walk_eq`. A hit additionally requires
`new_tips ⊆ cache.node_oids`. Therefore any topology difference — including one that happened
while the app was not looking — already forces a Miss. The wipe discards a cache whose validity is
re-proved on every use; it buys nothing and costs a full re-walk.

Residual cases, each ruled:

- **(a) repo closed, then re-opened.** `close_repo` removes the entry, so `repos.get(&repo_id)`
  returns `None` and the new entry starts `None`. **Ruling: still cold, by construction, not by a
  special case.** Nothing to carry — the `Arc` was dropped with the entry. This is why the ruling
  needs no code: the P86 justification ("topology may have changed while closed") is satisfied
  automatically.
- **(b) a DIFFERENT repo now at the same path, never closed in between.** The entry persists, so
  the cache is carried. Safe: the oid sets are disjoint, so `tips == c.tips` fails and
  `c.tips.is_subset(tips)` fails, giving a Miss. The one degenerate overlap is a cached
  empty/unborn walk (`tips` and `node_oids` both empty): then a hit requires the new `tips` to be
  empty too, i.e. both repos are empty, and replaying an empty layout for an empty repo is
  correct.
- **(c) an IDENTICAL clone at the same path.** Identical tips/head/hide ⇒ HitVerbatim, and the
  walk is a deterministic function of the seed, so the replayed layout is byte-identical to what a
  re-walk would produce. If the refs differ, `deco_fp` differs ⇒ HitRedecorate re-pills from the
  live `RefMap`. **Ruling: a correct hit, not a false hit.**
- **(d) the path resolves DIFFERENTLY than an existing key.** Then `repo_id` is the new canonical
  string, no entry exists under it, `get` returns `None`, and a fresh cache is minted. **Ruling:
  this is not a same-path re-arm and a fresh cache is correct.** The old entry (and its cache)
  stays under its own key, exactly as today.
- **(e) `bump_repo_generation`.** It evicts pooled `git2::Repository` handles for the id
  (`repo_handle::with_repo`); it holds, reads and invalidates **no graph state**. It is orthogonal
  and stays unconditional. A re-opened `Repository` handle re-probes the seed, which is the
  very check that keeps (b)/(c) sound.
- **(f) a `stream_graph` in flight across the re-arm.** Today it holds an `Arc` to a slot that is
  about to be orphaned, so its store is silently lost. After this change it stores into the
  **live** slot. Safe: `stream_graph_cached` brackets the cold walk with a second seed probe and
  stores only when the topology is observably unchanged across it. This is a strict improvement.
- **(g) memory.** A carried cache now lives until `close_repo` instead of until the next `full`
  round. The bound is unchanged: `GRAPH_CACHE_MAX_NODES = 50_000` caps one repo's retained
  `Vec<GraphChunk>` at ~10–25 MB, and the wipe only ever freed memory that the immediately
  following walk re-allocated. **Ruling: accepted, no new cap.**

Per-repo isolation is untouched: the carry is keyed on the exact `repo_id` and can only ever
resurrect that same id's own slot.

## 1.5 Explicitly OUT of scope

- **Changing the `full` slice matrix** (`src/components/repoWorkspace/refreshScope.ts:79-92`) so
  `full` stops setting `openRepo: true`. Deliberately deferred: `openRepo` in `full` carries the
  usability check *and* the watcher self-heal, and the P99 invariant at
  `refreshScope.ts:27-30` ("every scope with `openRepo: true` also has `branches: true`, and every
  `openRepo: false` scope never moves HEAD") is load-bearing. With §1.3 landed, the composition is
  no longer harmful, so there is nothing left to force this change.
- **`perf.repo_opens` = 564/day** (the `bump_repo_generation` handle-churn follow-up). Same root
  cause, but it must be **re-measured after this lands** before anything is changed.

## 1.6 Acceptance criteria (Section 1)

Numbered for reviewer check-off. Rust-only; no frontend, no harness.

- **AC1-1 — the headline regression.** In `src-tauri/src/commands/` (new test in
  `tests_repo_session_misc.rs`, near the existing `open_repo_inner` block at line 15): open a
  scratch repo via the `tests_support::open` helper, run one graph pass through
  `stream_graph_cached*` against the entry's `graph_cache` + a `PerfState`, call `open` a **second
  time for the same path**, run a second graph pass, and assert
  `perf.snapshot().graph_cache_hits == 1`. This is the assertion that fails today (it is `0`).
  Mirror the observation seam used by `graph_cache/tests.rs:180-190`
  (`hit_verbatim_on_unchanged_repo`).
- **AC1-2 — structural proof.** The `Arc<GraphCache>` cloned out of the map before the second
  `open` and the one cloned out after it satisfy `Arc::ptr_eq`. (Cheap, and pins the mechanism
  rather than the symptom.)
- **AC1-3 — close then re-open starts cold.** `open` → graph pass (populates) →
  `close_repo_inner(&state, &id)` (runtime-free, `repo.rs:324`; used the same way at
  `tests_repo_session_misc.rs:93`) → `open` again → graph pass ⇒ `graph_cache_hits` does **not**
  increase on the post-reopen pass (it is a Miss from an empty slot). Ruling per §1.4(a): cold,
  because the entry was dropped.
- **AC1-4 — path variant that does NOT match gets a fresh slot.** An open whose canonical id
  differs from every existing key inserts an entry whose `graph_cache` is not `Arc::ptr_eq` to any
  pre-existing entry's, and starts `None`.
  **CORRECTION (2026-09-22): this AC originally named "the same path-variant construction as
  `tests_repo_isolation.rs:128`", which is UNIMPLEMENTABLE** — confirmed independently by
  senior-dev and the reviewer. The `to_uppercase()` construction can only yield (a) on
  Windows/macOS a canonicalize-equal path, so `same_repo_path` matches and it becomes a *same-path
  re-arm* — the opposite of what this AC asks — or (b) on a case-sensitive filesystem a
  nonexistent directory, which `read_repo_info`'s `is_dir()` precheck rejects before any insert.
  Symlinks and junctions canonicalise back too. **A second distinct repo is the only construction
  that satisfies this AC**; the case-variant belongs in a `cfg`-gated complement asserting the
  slot IS carried, which is the §1.4(d) branch the real `full` scope hits.
- **AC1-5 — the watcher is still replaced on re-arm.** Two `open_repo_inner` calls for the same
  path invoke the `make_on_change` factory **twice** (count it in the closure), the installed
  entry's `watcher` is `Some` after each, and the first arm's callback is **dropped** by the second
  arm (observe via a drop-counting value moved into the first callback). The point of the test is
  that the self-heal survives the cache carry-over.
- **AC1-6 — per-repo isolation does not regress.** Two distinct scratch repos, interleaved
  open/graph rounds; each repo's hits/misses are unaffected by the other's re-arms, and neither
  slot is ever `Arc::ptr_eq` to the other.
- **AC1-7 — existing suites green with NO edits.** `src-tauri/src/graph_cache/tests.rs`,
  `tests_filter.rs`, `tests_fold.rs`, `src-tauri/src/commands/tests_repo_isolation.rs`,
  `tests_open_repo_guards.rs`, `tests_repo_session_misc.rs`.
  **Verified while writing this contract:** `grep -n graph_cache src-tauri/src/commands/tests_*.rs`
  returns **no matches**, i.e. no existing command-level test asserts the old None-on-re-arm rule,
  so this AC does not conflict with AC1-1/AC1-2. If senior-dev nonetheless finds a test asserting
  the superseded rule, that test is the one legitimate edit — report it rather than working around
  it.
- **AC1-8 — the three doc comments in §1.1 (plus any in `docs/architecture-reference.md`) now
  state the new rule**, each citing P117 and the §1.4 reason (exact-set classify against a live
  seed), not just "carried over".
- **AC1-9** — `cargo clippy --tests -D warnings` and `cargo fmt --check` clean.

---

# Section 2 — observability: a repo dimension on the refresh + graph spans

One-line summary of the defect: `detect_redundant_refresh` selects the previous refresh by `scope`
and nothing else, and cannot do better because no refresh or span record carries a repo id. With
5 repos open, ~17 of 26 firings in the measured session were cross-repo false positives (including
all five `full`-scope pairs), both `cache-collapse` firings pooled one first-walk from each of five
different repos, and a mutation in repo A can suppress a genuine finding in repo B.

## 2.1 Superseded / amended prior-contract clauses

**`docs/contracts/P91-observability.md:223-226` (§2.5)** — verbatim:

> An executed round emits `RefreshRound { round, scope, origin[], contributingTraces[], collapsed }`.
> A round with >1 contributing trace *is* the collapse evidence; a round with 1 contributing trace
> that repeats the same scope with no intervening mutation is a `redundant-refresh` anomaly.

*Amended:* "repeats the same scope" ⇒ "repeats the same scope **for the same repo**"; the record
gains the repo dimension (§2.2).

**`docs/contracts/P91-observability.md:555` (§5 rule table)** — verbatim:

> | `redundant-refresh` | sink | 1 s | same `scope`, ≥2 executed rounds, no mutation record between | warn |

*Amended:* → `same (repo, scope)`, ≥2 executed rounds, **no mutation record between that is
attributed to this repo or unattributed**.

**`docs/contracts/P91-observability.md:638-642` (§5.1 `cache-collapse`)** — verbatim:

> **`cache-collapse`:** over a 10 s window of `span{op:'graph.get'}` records, let
> `hits / (hits + redecorates + misses)`. Fire when the window has ≥5 spans and the hit rate is
> `< 0.2` **while no repo-mutating command appeared in the window** (a real mutation legitimately
> invalidates).

*Amended:* the window is **partitioned by repo**; the ≥5-span minimum and the rate are computed
**per repo**; the mutation suppression considers this repo's (or unattributed) mutations.
`docs/contracts/P91-observability.md:571` (the table row) is amended to match.

**Not superseded, restated as a ruling:** the 300 ms watcher `DEBOUNCE`
(`src-tauri/src/watcher/mod.rs:89`) does **not** change and **no post-fire quiet window is added**
— see §2.7.

## 2.2 The repo dimension — where it lives, and its representation

### Shape: ONE optional field on the record base, not three per-payload fields

```rust
// src-tauri/src/obs/record.rs — LogRecord, next to `caused_by` (~line 166)
/// P117 — the repo this record is about, as the canonical `repoId` (the
/// `AppState::repos` key / `open_repo`'s returned id). In memory this is the RAW
/// string: the anomaly detector keys on it (see `obs/sink.rs` — the detector
/// observes the in-memory record, never the redacted copy). The WRITER decides
/// what reaches disk (§7.1): strict ⇒ `repo#N`, raw ⇒ the home-masked path.
///
/// Emitted by exactly three producers in v1 (allow-list, do not widen without a
/// contract change): the UI `refresh` record, the Rust `span{op:"graph.get"}`
/// record, and `ipc.call` for repo-scoped commands. Every other kind leaves it
/// `None`.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub repo: Option<String>,
```

```ts
// src/ipc/types/obs.ts — LogRecordBase (~line 99, after `causedBy`)
/** P117 — canonical repoId this record is about. Absent ⇒ not repo-scoped, or
 *  unattributed. Never rendered in the UI; it is a correlation key. */
repo?: string;

// src/obs/types.ts — UiRecordInput (~line 261)
repo?: string;
```

`RefreshPayload` and the span payload are therefore **unchanged** — the dimension is a base field,
which is what makes the `ipc.call` attribution in §2.4 free rather than a third schema change.

**Rejected alternative (record it, do not implement it):** a `repoId` field on each of
`RefreshPayload`, `Span` and `IpcCall`. Three fields, three serde sites, three TS mirrors and three
redaction rules for one concept.

### Producers

| Record | Site | Value |
|---|---|---|
| `refresh` | `src/components/repoWorkspace/useCoalescedRefresh.ts:111-119` — add `repo: repoId` to the `logRecord({ kind: 'refresh', … })` call | the hook's `repoId` argument (`:67`), already in scope |
| `span{op:'graph.get'}` | `src-tauri/src/commands/status.rs:184-215` — `PhaseRecorder::start(OP_GRAPH_GET)` then `recorder.note_repo(&repo_id)` before `finish` | `stream_graph`'s `repo_id` param (`:137`) |
| `ipc.call` | `src/obs/ipcProxy.ts` — see the CORRECTION below; lift the `repoId` **positional** argument, else omit | the invoke argument |

**CORRECTION (2026-09-22) — the row above originally said "lift `args.repoId` when the invoke args
object has a string `repoId`". There is no args object at the proxy.** Every `IpcApi` method is
**positional** (`streamGraph(repoId, filter, onChunk)`); only `src/ipc/tauri/invoke.ts` ever sees a
keyed payload, long after the record is emitted. As implemented, `src/obs/repoArg.ts` resolves the
*position* of the `repoId` parameter from `rawArgPolicy.json` (whose non-null slots are
name-checked against the real signatures by the A26 drift guard), falling back to a private
`REPO_PARAM_FALLBACK` map for the 10 recognised mutations the policy omits. Unknown command or
unknown name ⇒ omit, never guess. Note the coupling this creates, called out in
`rawArgPolicy.ts`'s module doc: a **privacy** allow-list is now also read as a **name** map, so
nulling a `repoId` slot to tighten raw-mode exposure would silently degrade attribution to `None`
— which widens suppression to every repo. Guarded by the two tests in
`rawArgPolicy.test.ts`'s `P117 repo-attribution guard`.

New `PhaseRecorder` method (the only signature added in Section 2):

```rust
// src-tauri/src/obs/phase.rs
impl PhaseRecorder {
    /// P117 — attribute this span to a repo. The raw canonical `repoId`; the
    /// writer redacts it. Call before `finish`.
    pub fn note_repo(&mut self, repo_id: &str);
}
```

### Ruling on the representation: raw in memory, writer-redacted on disk

**Raw canonical `repoId` in the in-memory record; `repo#N` on disk in strict mode; the
home-masked path on disk in raw mode.**

Why this is the right answer, point by point:

1. **The detector never sees the redacted form.** `obs/sink.rs:310-314` writes the record and then
   feeds the *same in-memory `LogRecord`* to `detector.observe`; redaction happens in
   `writer.rs:263-280` on a separate `serde_json::Value` copy. So the rules key on the raw string
   and are **bit-identical in both redaction modes** — which preserves the existing invariant that
   strict and raw sessions produce identical anomaly output (`obs/tests_anomaly.rs:235`).
2. **Both sides already produce the identical string.** The frontend's `repoId` is
   `open_repo`'s returned id, which is the `AppState::repos` key that `stream_graph` receives. No
   normalisation, no case folding, no separator rewriting is permitted on either side.
3. **NORMATIVE — no UI-side redaction of `repo`, on any path.** This is the one rule whose absence
   the unit tests in §2.9 cannot catch, and breaking it produces a *false positive* in production:
   if `repo` were passed through `tagPath`/`tagValue` (`src/obs/react.ts:17,160-189`) it would
   become `ui:path#3`, would never equal the Rust span's raw path, and a `cache-collapse` would
   fire immediately after a real mutation — the exact inversion of §2.4's "suppression is the safe
   direction" argument. Concretely:
   - `src/obs/log.ts:39-49` **spreads** `input`, so a `repo` on `UiRecordInput` reaches the wire
     with no per-field change needed — do not convert that spread into an explicit field pick.
   - `src/obs/batcher.ts` forwards records verbatim; leave it that way.
   - Neither `react.ts`'s state-value redactor nor any new pass may touch `repo`. **The Rust
     writer is the only redaction point for this field.**
4. **On disk, strict mode must not carry a path.** Use the **already-defined but currently unused**
   `redact::Kind::Repo` (prefix `repo`, `redact.rs:52,62`) via `r.tag(Kind::Repo, raw)`. Do **not**
   rely on `strict::redact_names`' shape heuristic reaching it: `is_run_char` splits runs on
   whitespace, so `D:\Repos\my project` would ordinalise `D:\Repos\my` and leave `project` in the
   clear. This must be a **field-name rule**, applied to the top-level `repo` member (the payload
   is `#[serde(flatten)]`, so `repo` is a top-level key) **before** the generic
   `strict::walk`, inside `strict::enforce` (`strict.rs:167-175`).
5. **Raw mode** writes the home-masked path (`redact::scrub_value` with `home_mask` runs in both
   modes), which is exactly what `openRepo`'s `args.path` already shows in raw mode. Nothing new
   is exposed.
6. **Not an ordinal across sides — and not a promised join key.** A *counter* cannot agree across
   sides (hence `ui:ref#3` and the header's "join on trace, span or argsHash, never on ordinal
   equality", `record.rs:82-83`). This `repo#N` is minted by the **single writer-side `Redactor`**
   from an identical raw input, so both sides' records do land on the same token — but that is
   **incidental**. **Ruling: `repo#N` is NOT a promised cross-side join key, and the pinned
   `redactionNote` string (`record.rs:74-91`) is NOT to be edited.** Correlation of the Rust span
   with the UI refresh record remains `trace`/`span`/`argsHash`, as the header says.
7. **Rejected: a producer-side salted digest** (`Redactor::hash_args` / TS `hashCanonical`). It
   would agree across sides by construction, but no command-layer Rust site can reach the
   `Redactor` today (its only non-test callers are the redaction pipeline), so it means plumbing
   the session redactor into `status.rs`. Not worth it for a correlation we are not promising.
8. **Anomaly `detail` strings must never contain the repo value** (raw or redacted). The detector
   has no `Redactor`, and interpolating the raw value would leak a path into `detail` in raw mode
   and break the strict/raw byte-identity of point 1. The `refs` seqs already point at the records
   that carry the dimension.

## 2.3 Rule changes

```rust
// src-tauri/src/obs/anomaly/window.rs — composite keys
// redundant-refresh: key = format!("{repo}\u{0}{scope}") with repo = rec.repo
//                    .as_deref().unwrap_or("");  "" is its own bucket (see §2.5).
pub(super) fn detect_redundant_refresh(
    &mut self,
    ts: i64,
    seq: u64,
    repo: Option<&str>,   // NEW
    scope: &str,
    out: &mut Vec<LogRecord>,
);
```

- The `Sliding` key, the `arm(...)` debounce key and `refs_for(...)` (`window.rs:92,171`) all take
  the **composite** key. Missing any one of them reintroduces the bug in a different place
  (`arm` keyed on `scope` alone would rate-limit repo B out because repo A just fired).
- The `detail` string keeps naming only `{scope}` (§2.2 point 8). Recommended wording:
  `"{scope}: repeated refresh for the same repo within {W_REFRESH_MS}ms, no intervening mutation"`.
- Dispatch at `anomaly.rs:114-116` passes `rec.repo.as_deref()`.

```rust
// src-tauri/src/obs/anomaly/slow.rs — cache-collapse partitioned by repo
struct CacheSample { ts: i64, seq: u64, kind: String, repo: Option<String> } // + repo
```

Pseudocode (replaces `detect_cache_collapse`, `slow.rs:316-348`):

```
on_span(rec) where op == "graph.get" and cache.is_some():
    push CacheSample { ts, seq, kind, repo: rec.repo.clone() }
    retain samples with ts >= now - W_CACHE_MS          # unchanged
    this_repo = rec.repo.as_deref()                     # only the arriving repo is evaluated
    group = samples.filter(|c| c.repo.as_deref() == this_repo)
    if group.len() < CACHE_COLLAPSE_MIN: return         # 5, now PER REPO
    if any mutation m in [now - W_CACHE_MS, now] with attribution_matches(m, this_repo): return
    hit_rate = group.count(kind == "hit") / group.len()
    if hit_rate < CACHE_HIT_FLOOR and rate_ok(cache_last_fire[this_repo], now, W_CACHE_MS):
        emit cache-collapse { detail: "graph cache hit rate {r}% over {n} spans for one repo,
                                       no intervening mutation",
                              refs: group.map(seq) }
```

- `cache_last_fire: Option<i64>` becomes a **per-repo** map (`HashMap<String, i64>`), pruned to
  `W_CACHE_MS` like every other debounce map, so the §11 boundedness claim
  (`anomaly.rs:47-55`) still holds. Cap it at the same order as the other maps; the key set is
  bounded by open repos in practice.
- **Data shape produced** (unchanged wire shape, new semantics): one `anomaly` record with
  `rule: "cache-collapse"`, `severity: "warn"`, `refs` = the seqs of the ≥5 spans **of one repo**,
  `traces: []`.

## 2.4 `mutations` — the mirror-image bug (scope widening; flag to the orchestrator)

`self.mutations: Vec<i64>` (`anomaly.rs:58-60`) is a bare timeline, shared by `dup-ipc`,
`redundant-refresh` and `cache-collapse`. A mutation in repo A currently suppresses a genuine
finding in repo B. Fixing it needs the mutation to be *attributable*, and `IpcCall.args` is
**raw-mode-only** (`record.rs:234-236`) — so in strict mode there is today no repo on an `ipc.call`
at all. That is why §2.2 puts `repo` on the record **base** (present in both modes) rather than
inside `args`.

```rust
// anomaly.rs — attributed mutation timeline
mutations: Vec<(i64, Option<String>)>,   // (ts, repo)
```

Attribution rule (normative):

- `is_mutation_cmd(cmd)` (`anomaly.rs:285`) is unchanged; the push becomes
  `self.mutations.push((ts, rec.repo.clone()))`.
- A **repo-keyed** rule (`redundant-refresh`, `cache-collapse`) treats a mutation as intervening
  when `m.repo == this_repo` **OR `m.repo.is_none()`**. Unattributed suppresses everywhere —
  deliberately conservative: suppression is the *safe* direction (a missed anomaly, never a false
  one), and it keeps today's behaviour exactly for any producer that does not set `repo`.
- ~~Consequence to expect, not a defect: mutation commands with **no `repoId` argument** (e.g.
  `clone`, `init`) land in the `None` bucket and therefore suppress everywhere for their window.~~
  **CORRECTION (2026-09-22): this example was wrong twice over and is withdrawn.** First,
  `cloneRepo`/`initRepo` never enter `mutations` **at all** — `is_mutation_cmd` is keyed on
  snake_case while the only producer of `ipc.call` logs the camelCase JS property name verbatim, so
  they match nothing and suppress nothing. Second, the follow-up pass added `REPO_PARAM_FALLBACK`
  (`src/obs/repoArg.ts`), so **all 29 recognised mutations now attribute** and the "no `repoId`
  argument" route does not exist. What actually still reaches the `None` bucket: a `schema: 2` line
  replayed from an older log (§2.6/AC2-10), a lift that failed its non-empty-string check, and any
  future producer that omits the field.
- `dup-ipc` treats **all** mutations as intervening (unchanged behaviour) — its
  `mutation_between` closure at `window.rs:136` must destructure the new tuple and ignore the
  attribution.
- Pruning to `MAX_HISTORY_MS` is unchanged.

**Flag:** this is a third record shape (`ipc.call`) beyond the two the task named. It is not
optional — without it, AC2-5 is unmeetable in strict mode, which is the default mode.

## 2.5 Rules that explicitly do NOT change

- **`dup-ipc`.** No repo dimension needed. Its key is `cmd\0argsHash`, and `argsHash` is the
  salted hash of the *whole* canonicalised args object — which contains `repoId` for every
  repo-scoped command. It is **already repo-discriminating**; adding the dimension would be
  redundant and would change no outcome. (It does adopt the attributed `mutations` tuple, but
  ignores the attribution — see §2.4.)
- **`effect-thrash`, `render-storm`, `event-storm`, `watcher-storm`, `unbatched-sink`,
  `orphan-trace`, `slow-command`, `slow-phase`, `queue-delay`, `pool-saturation`,
  `watchdog-pressure`, `jank-trace`.** Unchanged. They key on component/effect/event/cmd or on
  process-wide resources (the blocking pool, the frame clock) where pooling across repos is the
  *correct* reading: three repos saturating the pool together IS pool saturation.
- **Records with `repo: None`** form their own bucket (`""`) for `redundant-refresh` and
  `cache-collapse`, i.e. today's repo-blind behaviour for any unattributed producer. They are not
  merged into any repo's bucket.

## 2.6 `OBS_SCHEMA_VERSION` 2 → 3, and v2 backward compatibility

**Requirement: bump to 3 on both sides in this increment.**

- `src-tauri/src/obs/record.rs:35` — `pub const OBS_SCHEMA_VERSION: u32 = 3;`
- `src/ipc/types/obs.ts:22` — `export const OBS_SCHEMA_VERSION = 3;`
- `src-tauri/src/obs/tests_schema_parity.rs::ts_mirror_matches_the_rust_schema_version` (P116)
  enforces the pair and will fail red if either side is missed. It asserts the literal
  `export const OBS_SCHEMA_VERSION = 3;` appears **exactly once** in the TS source with comment
  lines stripped — so the TS doc block must not contain that literal in prose.
- Add a **v2 → v3** paragraph to the `record.rs` module note (after the v1 → v2 one at `:21-30`)
  stating the justification below.

**Justification, because the documented rule reads the other way.** `record.rs:18-19` says
*"`OBS_SCHEMA_VERSION` stays put for additive changes; it is bumped only when an EXISTING field
changes shape or meaning"*, and `record.rs:242-244` cites that rule to keep an additive optional
field from bumping. The new `repo` field alone would therefore **not** bump it. The bump is
justified by an **existing field changing meaning**: in a v3 file, a `redundant-refresh` or
`cache-collapse` `anomaly` record means *one repo* did the thing; in a v2 file the identical
`rule`/`detail`/`refs` shape means *any set of repos* did. A reader cannot tell those apart except
by the header's `schema` — which is precisely the v1 → v2 argument. Write that sentence into the
module note so the next reader does not "fix" the bump away.

**Backward compatibility (ruling: degrade, never refuse).**

- **There is no external replay/parse tool to gate.** Checked while writing this contract:
  `scripts/**` contains only `gate.mjs`, `check-file-size.mjs`, the e2e server/reporter and
  `lib/spawn-tool.mjs`; there is no `tools/` log parser. **Serde is the only reader**, plus the
  in-app Dev log viewer.
- **Serde:** `repo` is `#[serde(default, skip_serializing_if = "Option::is_none")]`, so a
  `schema: 2` line deserialises with `repo: None`. Every v2 record lands in the `""` bucket, which
  by §2.5 is exactly today's repo-blind behaviour. **A v2 file replays repo-blind and is never
  rejected**; its `redundant-refresh`/`cache-collapse` firings keep their v2 meaning.
- The `Dev` log viewer / export path renders records generically (`LogRecord = LogRecordBase &
  Record<string, unknown>`) and needs no change.

## 2.7 Explicitly OUT of scope, with the reason

**The 300 ms watcher `DEBOUNCE` (`src-tauri/src/watcher/mod.rs:89`) does not change, and no
post-fire quiet window is added.** This is a ruling, not an omission. The 9 genuine same-repo
double-rounds cost **805 ms in total across 152 minutes**, and every inter-round gap (381–935 ms)
exceeded the 300 ms **trailing-edge quiet period** — so the watcher genuinely observed new external
filesystem writes after a quiet gap and the debounce did exactly what it is specified to do.
Suppressing them would trade a real correctness property (prompt reaction to another tool writing
in the worktree) for a sub-second saving. The architecture invariant
("`notify` events are debounced ~300 ms, always paired with manual refresh + focus rescan") stands
unchanged. The `redundant-refresh` rule's "no intervening mutation" clause also tracks only
*Bonsai-originated* mutations, so for a watcher-origin refresh — external by definition — it can
never be satisfied and contributes nothing; that is not a defect either, and it is not changed here.

Also out of scope: the `perf.graph_walks` +6 over-report (audit "Open question" — **do not fix
blind**), and Section 1's follow-ups.

## 2.8 Mock-IPC / browser-harness requirement

`log_append` / `log_session_info` are the only commands involved and their **signatures do not
change** — the new field is additive and optional inside the record payload. `src/ipc/mock.ts`
(and `src/ipc/mock/handlers/obs.ts`) must still typecheck against the updated `LogRecordBase`;
accepting records that now carry `repo` requires no handler change. The `schema: 1` literals in
`src/ipc/mock/handlers/obs.ts` and `handlers/history.ts` are `METRICS_SCHEMA_VERSION` /
`IndexStatus.schema` and are **not** this counter — do not touch them (see `TODO.md:596-598`).

## 2.9 Acceptance criteria (Section 2)

Rust ACs are unit tests in `src-tauri/src/obs/` using the existing harness
(`tests_anomaly_support.rs` — `det.observe(&rec, seq)` + record builders), extended with a `repo`
argument.

- **AC2-1 — the 17 false positives.** Two `refresh` records with the **same `scope`** and
  **different `repo`**, 141 ms apart, inside `W_REFRESH_MS` ⇒ **no** `redundant-refresh`. Repeat
  for `scope: 'full'` specifically (the population carrying the reported cost).
- **AC2-2 — the true positive still fires.** Two `refresh` records with the same `repo` **and**
  same `scope`, 400 ms apart, no mutation between ⇒ exactly one `redundant-refresh`.
- **AC2-3 — the `arm` debounce is repo-keyed.** Repo A fires a `redundant-refresh`, then repo B
  produces its own same-repo pair inside the same 1 s window ⇒ repo B **also** fires (proves
  `arm` and `refs_for` took the composite key, not just the lookup).
- **AC2-4 — `cache-collapse` across five repos.** Five `span{op:'graph.get', cache:'miss'}`
  records with five **distinct** `repo` values inside 217 ms ⇒ **no** `cache-collapse` (today: it
  fires). Then five misses with the **same** `repo` in the same window ⇒ exactly one
  `cache-collapse`, whose `refs` contain **only** that repo's five seqs.
- **AC2-5 — cross-repo mutation suppression is gone.** A mutation `ipc.call` with `repo: A`,
  then a genuine same-repo `redundant-refresh` pair for `repo: B` ⇒ B **still** fires. The same
  pair for `repo: A` ⇒ suppressed. An `ipc.call` with `repo: None` ⇒ suppresses **both**
  (conservative, §2.4). Repeat the A/B pair for `cache-collapse`.
- **AC2-6 — `dup-ipc` unchanged.** Its existing tests pass untouched; one added test shows two
  calls of the same `cmd` for different repos do not collide (because `argsHash` differs), with no
  rule change.
- **AC2-7 — redaction.** (a) Strict mode: a record with `repo: "D:\\Repos\\my project"` reaches
  disk as `repo#N` matching `^repo#\d+$` — with **no path fragment anywhere in the line** (this is
  the assertion that catches the `is_run_char` whitespace gap). (b) The same repo string gets the
  **same** `repo#N` twice in one session. (c) Raw mode: the value reaches disk home-masked
  (`<home>\Repos\my project`). (d) A record's `repo` value is the **raw** string when the detector
  observes it (assert via a repo-keyed rule firing correctly in strict mode).
- **AC2-8 — no repo value in any anomaly `detail`.** For every rule changed here, the emitted
  `detail` contains neither the raw repoId nor a `repo#` token; strict and raw sessions produce
  **byte-identical** anomaly records (extend `obs/tests_anomaly.rs:235`).
- **AC2-9 — schema parity at 3.** `tests_schema_parity.rs` green with both sides at `3`;
  mutation-prove it (revert the TS literal to `2` ⇒ red).
- **AC2-10 — v2 replay degrades.** A `schema: 2` record line (no `repo` key) deserialises to
  `repo: None` and produces the pre-P117 firing for the same input sequence; no error, no drop.
- **AC2-11 — boundedness.** The per-repo `cache_last_fire` map and the composite-key `last_fire`
  maps are still pruned to their windows; a test feeds ≥200 distinct repo ids and asserts the maps
  stay bounded (mirror the existing `last_fire_len` / `baseline_len` caps at `window.rs:86`,
  `anomaly.rs:172`).
- **AC2-12 — frontend, asserted at the wire boundary.** With the mock `logAppend` spied on: a
  refresh round emits a record whose `repo` is the **raw** `repoId` string, byte-identical to the
  hook's `repoId` prop — not `ui:path#N`, not masked, not absent. This is the AC that pins §2.2
  point 3; a test that builds both records itself would pass even if the UI redacted the field.
  Also: `ipcProxy` sets `repo` from a string `args.repoId` and omits it otherwise; `tsc --noEmit`
  and `eslint` clean; the harness still boots with `VITE_MOCK_IPC=1` with no new console errors.

---

## Flagged for the orchestrator

1. **§2.4 is scope widening** beyond the two record shapes named in the task: attributing the
   `mutations` timeline requires `repo` on `ipc.call` too, because `args` is raw-mode-only. Not
   optional — AC2-5 is otherwise unmeetable in the default (strict) mode.
2. **§2.6 contradicts the codebase's own documented versioning rule** (`record.rs:18-19`: additive
   ⇒ no bump). I rule **bump to 3 anyway**, justified by the changed *meaning* of existing
   `anomaly` records rather than by the new field. If the orchestrator prefers to honour the
   additive rule literally, the alternative is to stay at 2 and accept that a reader cannot tell a
   repo-blind `redundant-refresh` from a repo-keyed one — I recommend against it.
3. **§2.2 puts the dimension on `LogRecordBase`**, not on the two payloads. This is a deliberate
   deviation from the task's wording ("add a repo dimension to both record types"); it is one
   field instead of three and is what makes flag 1 cheap.
4. **The task's "works in BOTH redaction modes" is only true via writer-side enforcement**, and
   the writer's existing shape heuristic has a known whitespace gap — hence the field-name rule in
   §2.2 point 4 and the specific assertion in AC2-7(a). Do not let a reviewer relax that to
   "strict::enforce already redacts paths".
