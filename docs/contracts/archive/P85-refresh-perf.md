# P85 — Refresh Perf: kill the double-refresh (Workstream A)

Status: contract (design). Extends P81 (commit be01422, archived
`docs/contracts/archive/P81-refetch-coalescing.md`). **No change to the graph-walk math.**

Root cause (2026-08-22 perf investigation): a branch create takes 15-20s though `git branch`
is instant. Several mutation handlers call **raw** `refetchGraph()`/`refetchBranches()` instead of
`refresh('mutation')`, so they never `armEcho`. Their own `.git/refs/**` write then trips the
notify watcher ~300 ms later; that watcher round is **not** echo-suppressed and runs a SECOND full
`runRefreshRound` (repo re-open + full graph re-walk + ~9 refetches). Two O(all-commits) walks
contend in `spawn_blocking`, each bounded only by the 30s git-timeout. Also: the 600 ms wall-clock
echo window is fragile on large repos, and `fetch` serially awaits a hidden 2nd network fetch.

Three fixes, all landable independently.

---

## A1 — Route bypass handlers through the echo-armed round

Replace every raw post-mutation `refetch*` with the canonical armed round. Each handler already
`await`s its `ipc.*` git op first (that is where the FS write happens); only the **refresh** call
changes.

### `src/components/repoWorkspace/useBranchActions.ts`

| Handler | ~line | Today | Change to |
|---|---|---|---|
| `handleCreateBranch` | 31 | `await refetchBranches(); void refetchGraph();` | `await refreshAll('refsOnly')` |
| `handleDeleteBranch` | 100 | `await Promise.all([refetchBranches(), refetchGraph()])` | `await refreshAll('full')` * |
| `handleRenameBranch` (non-head path) | 132 | `await Promise.all([refetchBranches(), refetchGraph()])` | `await refreshAll('refsOnly')` |
| `handleDeleteRemoteTracking` | 164 | `await Promise.all([refetchBranches(), refetchGraph()])` | `await refreshAll('refsOnly')` |

\* a branch delete can drop commits from the reachable set (real topology change) → `'full'`.
The `refsOnly` scope is defined in P86; **until P86 lands, all four call `refreshAll()` (== full).**
The scope arg is forward-compatible: `refreshAll(scope: RefreshScope = 'full')` (P86 §B3).

### `src/components/repoWorkspace/useRemoteOps.ts`

| Handler | ~line | Today | Change to |
|---|---|---|---|
| `handleFetch` | 61 | `await Promise.all([refetchBranches(), refetchGraph()])` | `await refreshAll('remoteMeta')` |
| `pushCurrentBranch` | 117 | `await Promise.all([refetchBranches(), refetchGraph()])` | `await refreshAll('refsOnly')` |
| `doForcePush` | 151 | `await Promise.all([refetchBranches(), refetchGraph()])` | `await refreshAll('refsOnly')` |

Rules:
- Keep each handler's existing `try/catch/finally`, toasts, and `ipc.*` calls **verbatim**. Only the
  refetch statement changes. `refreshAll` never throws (P81), so the `push`/`forcePush` hook-gate
  `attempt` bodies keep the same success semantics.
- Drop `refetchGraph`/`refetchBranches` from these two hooks' `deps` objects **iff** no longer
  referenced (both become unused after A1). `handleCheckoutBranch`/`handleCreateBranchHere`/
  `handleCheckoutRemote`/`handlePull` already use `refreshAll` — leave them.
- No IPC change; `refreshAll` is the existing `RepoWorkspace.tsx:1314` callback.

**Result:** the handler's own watcher echo is now armed and dropped ⇒ **one** round, not two.

---

## A2 — Round-anchored echo suppression (hardening)

The P81 window is `armEcho(now) → suppressUntil = now + 600`. On a large repo the round
(re-open + full walk) plus the 300 ms debounce can exceed 600 ms, so the self-echo lands **after**
the window and double-refreshes even for commit/checkout/pull.

**Fix: anchor the tail to round completion, not to arm time, with a nesting count.** A mutation
keeps suppression armed for the entire span it is writing/refreshing, then a fixed tail after the
round settles. Round duration becomes irrelevant.

### `src/components/repoWorkspace/echoSuppression.ts` (rewrite the registry)

```ts
/** Tail after the last self-caused write + round settle: 300 ms watcher debounce
 *  + 300 ms dispatch/render slack. Named tunable. (Renames P81 ECHO_TTL_MS.) */
export const ECHO_TAIL_MS = 600;

/** Begin a self-caused-write span for `repoId` (call BEFORE enqueuing the round).
 *  Nesting-counted: overlapping mutations each arm once. While count > 0 every
 *  watcher event for the repo is suppressed, with NO expiry. */
export function armEcho(repoId: string): void;

/** End a span (call in the round's `finally`). When the count reaches 0, start the
 *  tail: suppress until `now + ECHO_TAIL_MS`. `now` injectable for tests. */
export function disarmEcho(repoId: string, now?: number): void;

/** True iff a span is open (count > 0) OR `now` is inside the post-settle tail. */
export function isEchoSuppressed(repoId: string, now?: number): boolean;

/** Drop `repoId` (tab close / unmount). */
export function clearEchoSuppression(repoId: string): void;

/** Test-only: wipe the registry. */
export function __resetEchoSuppression(): void;
```

State (module-level singletons, keyed by `repoId`): `armedCount: Map<string, number>` and
`disarmUntil: Map<string, number>`. `armEcho` → `count++`, delete `disarmUntil[repoId]`.
`disarmEcho` → `count--` (floor 0); if now 0, `disarmUntil[repoId] = now + ECHO_TAIL_MS`.
`isEchoSuppressed` → `(count>0) || now < (disarmUntil[repoId] ?? 0)`. `clearEchoSuppression`
deletes both.

**Ordering invariant (why this is sound):** the git op's FS write completes inside the backend
call, and the handler `await`s that call *before* invoking `refreshAll` → `armEcho`. The watcher
debounce needs 300 ms of quiet *after* that write, i.e. the echo arrives ≥ 300 ms after arm — far
longer than the JS gap between the op resolving and `armEcho`. So arm always precedes the echo.
While the round runs, `count>0` suppresses; after it settles the echo (≤ write+300+dispatch ≤
settle+600) still falls inside the tail. Duration-independent by construction.

### `src/components/repoWorkspace/useCoalescedRefresh.ts` (bracket the round)

`refresh(origin)` becomes:

```
if (origin === 'watcher' && isEchoSuppressed(repoId)) return Promise.resolve();
if (origin !== 'mutation') return coalescer.request();
armEcho(repoId);
return coalescer.request().finally(() => disarmEcho(repoId));
```

The `finally` disarms after **this caller's serving round** (leading or the trailing it collapsed
into) settles. Nesting count handles overlapping mutations. `clearEchoSuppression(repoId)` on
unmount is unchanged. Panels (`AiAssetsPanel`, `RepoHealthPanel`) still gate on `isEchoSuppressed`
only — no change.

Backward-compat with P81: the coalescer, origins, panel gates, and the one-tab-per-`repoId`
invariant are untouched. Only the suppression window semantics change (arm→count/tail).

---

## A3 — Take `fetch`'s 2nd network fetch off the critical path

`src-tauri/src/commands/remotes.rs:11` (`fetch`) `.await`s `auto_sync_tags`
(`crates/bonsai-core/src/git/tag_auto_sync.rs:122` → `fetch_temp_tags:92` = a 2nd
`remote.fetch("+refs/tags/*:refs/bonsai-tagsync/*")`) **before returning**, serializing a whole
extra network round-trip, and its temp-ref churn under `.git/refs/bonsai-tagsync/**` trips the
watcher.

### Backend changes

1. **`fetch` command** (`remotes.rs:11`): drop the `.await` on tag sync. Mirror the existing
   fire-and-forget commit-graph write (`remotes.rs:33-39`) — `spawn_blocking` the tag sync, return
   the `FetchResult` immediately. Add `app: tauri::AppHandle` to the command signature (as
   `open_repo` has). On the spawned task's completion, if the report shows any change
   (`added|updated|deleted > 0`), emit both:
   - `repo-changed { repoId, reason: "tags" }` (so the tag list refreshes), and
   - `tag-auto-sync { repoId, report }` (so the P84 count toast survives — see below).
   `FetchResult.tagAutoSync` becomes always absent (the field stays on the type, always `None`).

2. **Watcher** (`src-tauri/src/watcher.rs:55-71` `is_relevant`): add Bonsai's scratch namespace to
   the ignore list so the temp-fetch churn no longer triggers a refresh:
   ```
   if rel.starts_with("refs/bonsai-tagsync") { return false; }
   ```
   (Place before the `starts_with("refs")` clause.) Extend `is_relevant_rules` test accordingly.
   This is the piece that prevents the async tag sync from itself double-refreshing: the only
   watcher-visible writes it makes are to real `refs/tags/*` (a genuine change we *want* one
   refresh for), which the coalescer collapses with the explicit `reason:"tags"` emit.

Preserves tag-drift behavior: real tag adoption/deletion is unchanged; only its *delivery* moves
from the response body to a completion event.

### New event surface (mock-implementable)

```ts
// src/ipc/types/common.ts — extend RepoChangedPayload.reason domain (already `string`):
//   "fs" | "fetch" | "tags"  (unknown reasons treated as full refresh — safe)

// src/ipc/types — new event payload
export interface TagAutoSyncEvent { repoId: string; report: TagAutoSyncReport; }

// ipc-api.ts
onTagAutoSync(cb: (e: TagAutoSyncEvent) => void): Promise<Unsubscribe>;
```

- Tauri impl: `listen('tag-auto-sync', …)` (mirror `onRepoChanged`, `src/ipc/tauri/app.ts:16`).
- Mock impl: after `fetch`, `setTimeout(…)` emit a `repo-changed{reason:"tags"}` + a `tag-auto-sync`
  from a fixture report through the existing `repoChangedListeners` + a new `tagAutoSyncListeners`
  (mirror `src/ipc/mock/scheduler.ts:108`). Keeps the browser harness self-consistent.
- Frontend: subscribe in `RepoWorkspace.tsx` alongside `onRepoChanged`; on event, toast the counts
  (reuse the P84 toast copy currently driven by `FetchResult.tagAutoSync` in `useRemoteOps.handleFetch`).

**Optionality (flag OD-P85-1):** if the orchestrator prefers minimal surface, drop the
`tag-auto-sync` event and emit only `repo-changed{reason:"tags"}` — tags still refresh, but the
count toast is lost. Recommendation: keep the event (preserves P84 UX; ~15 lines).

---

## Measurement (how the tester asserts A landed)

**Frontend round counter (deterministic, no backend).** In `RepoWorkspace.tsx`, wrap
`runRefreshRound` so it increments a test-visible tally before the body:
```ts
if (import.meta.env.DEV || import.meta.env.MODE === 'test')
  (globalThis as any).__bonsaiRefreshRounds = ((globalThis as any).__bonsaiRefreshRounds ?? 0) + 1;
```
Vitest asserts via the existing `run` spy pattern (P81 §9); e2e/harness reads
`window.__bonsaiRefreshRounds`. This is the same instrument P86 reuses.

The native "15-20s → instant" branch-create timing is a **USER CHECKPOINT** (headless harness
pauses rAF; wall-clock timing is not AI-verifiable).

---

## Acceptance criteria

- **AC-A1:** each of the 7 bypass handlers, on success, runs **exactly 1** refresh round; its own
  watcher echo within the window adds **0** rounds (regression of P81 AC1 to these handlers).
- **AC-A2a:** a mutation whose round takes T ms (mock a slow `run`), with the echo arriving at
  T+ε, is suppressed for **any** T (arm stays while `count>0`; tail begins at settle). The old
  fixed-600-from-arm miss no longer occurs.
- **AC-A2b:** overlapping mutations (count 2) resolve to `count 0` only after both settle; the tail
  then applies once. External change after the tail → a round (not swallowed).
- **AC-A3a:** `fetch` returns before `auto_sync_tags` completes (assert `fetch_inner` result is not
  gated on tag sync; unit test on the command core).
- **AC-A3b:** `is_relevant` returns `false` for `.git/refs/bonsai-tagsync/**` and `true` for
  `.git/refs/tags/*`.
- **AC-A3c:** a fetch that changes tags emits one `repo-changed{reason:"tags"}` + one
  `tag-auto-sync`; a no-change tag sync emits neither.
- **AC-A4 (integration):** a single branch-create fixture drives exactly **1** `runRefreshRound`
  (was 2). Asserted via the round counter with the watcher echo dispatched through
  `repoChangedListeners`.

---

## File-size / split notes

- `echoSuppression.ts` stays < 60 lines. `useCoalescedRefresh.ts` stays < 80.
- `remotes.rs` gains ~15 lines (the fire-and-forget block already has the pattern). If it crosses
  ~500 lines, extract the tag-sync-spawn helper into `commands/remotes_tagsync.rs`.
- No new large files.

---

## Flags for the orchestrator

- **OD-P85-1** (A3): keep the `tag-auto-sync` event (preserve the P84 count toast) vs
  repo-changed-only. Recommend **keep**.
- **Behavior delta:** fetch's tag counts now arrive slightly *after* the fetch toast (async), not
  in the same toast. Acceptable; flag if a single combined toast is required.
- **Sequencing:** A1/A2/A3 are independent. Land A1+A2 together (the primary bug + the hardening
  that makes it robust); A3 can follow. The `'refsOnly'`/`'remoteMeta'` scope args in A1 are inert
  (== full) until P86; ship A1 passing plain `refreshAll()` if P86 is not yet merged.
