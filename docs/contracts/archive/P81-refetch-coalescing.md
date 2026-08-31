# P81 — Refetch Coalescing & Self-Event Suppression (frontend-only)

Status: contract (design). Fixes the "refetch storm" — audit #1 §3.10 / audit #2 §5.3.
Scope: pure React frontend. **No backend change.** The `notify` watcher and its 300 ms
debounce stay exactly as-is.

---

## 1. Problem statement (verified in current source)

`src/components/RepoWorkspace.tsx`:

- **`refreshAll`** — `1145-1208`. Does `ipc.openRepo(repoPath)` then `Promise.all([...])` of
  **10** refetches: status, graph, branches, stashes, submodules, worktrees, remotes, opState,
  compare, `tagSync({force:true})`. Never throws (errors → sticky toast).
- **Mutation handlers** (commit, stage, stash, merge, rebase, checkout, cherry-pick/revert,
  submodule, worktree, tag/remote, bulk-AI-resolve, pending-op apply) all receive `refreshAll`
  and call it directly for immediate feedback. Call sites depending on `refreshAll`:
  `1477, 1519, 1556, 1591, 1611, 1625, 1640, 1705, 1723, 1732, 2016`.
- **`repo-changed` subscription** — `1298-1340`. Filters to this `repoId`, then re-runs **9**
  refetches (everything in `refreshAll` except `openRepo` + `tagSync`). Refetches regardless of
  `active` so background tabs stay fresh.
- The **same filesystem write** that a mutation performs also trips the backend watcher, which
  debounces **300 ms trailing-edge** (`src-tauri/src/watcher.rs:29`, `DEBOUNCE = 300ms`) then
  emits `repo-changed`. So every mutation fires **~2× (≈9-10 parallel fetches)** ≈ 300 ms apart:
  once eagerly, once from the watcher echo.

Other refresh entry points that MUST keep working:

- **Initial mount load** — `1213-1225`. Fires the 8 refetches once, no `openRepo`, active + bg.
- **Activation self-heal** — `1230-1239`. On flip TO `active` after mount → `refreshAll`.
- **Window-focus rescan** — `1344-1387`. Active tab only; inlines the 9 refetches + forge signals.
- **Sibling panels with their OWN `onRepoChanged`**: `AiAssetsPanel.tsx:143-161` and
  `RepoHealthPanel.tsx:357-375` (each does a single lightweight `refresh()` on same-`repoId`
  events while `open`). These must not regress.

**Tab/repo invariant that makes the fix safe:** `App.tsx:275-282` — opening an
already-open repo focuses the existing tab; tabs are deduped by `repoId`. **Therefore there is at
most one `RepoWorkspace` per `repoId`.** A background tab is always a *different* `repoId`, so a
suppression keyed by `repoId` can never swallow another tab's genuine refresh.

---

## 2. Design overview

Two independent, composable pieces, both pure and unit-testable, wrapped by one hook:

1. **Coalescer** (`refreshCoalescer.ts`) — a leading + single-trailing, **in-flight-based**
   dedup around the canonical refresh round. No timers.
2. **Echo suppression registry** (`echoSuppression.ts`) — a module-level, **per-`repoId`,
   time-window** gate. A mutation arms it; watcher events inside the window are dropped.
3. **`useCoalescedRefresh.ts`** — React hook binding a coalescer instance + the registry to a
   `run()` round for a `repoId`, exposing `refresh(origin)`.

The canonical **refresh round** = today's `refreshAll` body verbatim (openRepo + the 10 refetches,
or the clear-branch when the repo is unusable). All origins funnel through the coalescer to this
one round, so the watcher path stops being a divergent near-duplicate.

No new IPC surface. Mock-implementable trivially: the hook only consumes existing
`ipc.onRepoChanged` (mock: `src/ipc/mock/handlers/session.ts:25`) and the existing refetch
callbacks; nothing new crosses the boundary.

---

## 3. Coalescing state machine (requirement 1)

**Policy: leading + at-most-one trailing, in-flight-based** (the "window" is the duration of the
round itself — no arbitrary timer). Chosen because it exactly matches the requirement — "a refetch
already running should not be stacked; a request arriving mid-flight schedules at most one trailing
round" — without introducing a second tunable.

```
state:
  phase        : 'idle' | 'running'        # start 'idle'
  trailing     : boolean                    # start false
  runningTail  : Promise<void> | resolved   # resolves when current run settles
  trailingTail : Deferred<void>             # resolves when the queued trailing run settles

request():                       # returns a Promise that resolves when THIS caller's round settles
  if phase == 'idle':
    phase = 'running'
    start()                      # returns runningTail
    return runningTail
  else:                          # phase == 'running'
    trailing = true              # collapse ALL mid-flight requests into one trailing round
    return trailingTail.promise

start():
  runningTail = run()            # the canonical round; never throws (already try/caught)
  runningTail.finally(onSettle)

onSettle():
  if trailing:
    trailing = false
    prev = trailingTail; trailingTail = newDeferred()
    start()                      # chain the single trailing round
    runningTail.finally(() => prev.resolve())   # resolve the batch that requested it
  else:
    phase = 'idle'
```

Consequences:
- K requests arriving during one in-flight round ⇒ **exactly 2 rounds total** (1 leading + 1
  trailing), regardless of K.
- Requests arriving when idle ⇒ run immediately (leading edge → instant user feedback).
- `run` is the shared round; its own internal per-slice request-id guards (`*ReqId`) already make
  a superseded round's late responses no-ops, so overlap is safe.

---

## 4. Self-event suppression (requirement 2)

**Mechanism: per-`repoId` time-window (arm-and-check, NOT consume-on-read).**

```
registry: Map<repoId, suppressUntilMs>          # module-level singleton

armEcho(repoId, now):        suppressUntil[repoId] = now + TTL_MS
isSuppressed(repoId, now):   return now < (suppressUntil[repoId] ?? 0)
clearRepo(repoId):           suppressUntil.delete(repoId)     # on tab close / unmount
```

- A **mutation**-origin refresh calls `armEcho(repoId, now)` *before* enqueuing its round.
- A **watcher**-origin refresh checks `isSuppressed(repoId, now)`; if true it is a **no-op**
  (resolves immediately, no round). Otherwise it enqueues a round.
- `manual` / `activation` / `focus` origins **never** arm and are **never** gated — they always
  enqueue a round. This preserves the activation self-heal and focus rescan guarantees
  (requirement 3).

**Why time-window, not consume-on-read one-shot:** suppression must be honored by *multiple*
subscribers of the same `repoId` (RepoWorkspace + the two panels). A one-shot token would be
consumed by whichever subscriber's callback ran first, leaving the others un-suppressed. A shared
time-window is checked (not consumed) by every subscriber, so one arm suppresses the single
self-echo everywhere. Because a mutation produces exactly one debounced `repo-changed`, a plain
window is sufficient; a rapid second mutation simply re-arms (extends) the window.

### Exact timing

| Constant | Value | Rationale |
|---|---|---|
| Backend watcher debounce | **300 ms** (fixed, `watcher.rs:29`) | trailing-edge quiet period after the last FS write |
| `TTL_MS` (suppression window) | **600 ms** | see below |

**Justification of `TTL_MS = 600 ms` against the 300 ms debounce.** The mutation calls
`refreshAll` synchronously *after* the git2 op returns, i.e. after its FS writes are complete, so
`armEcho` fires at ≈ (last-write time). The watcher's trailing edge fires 300 ms after that last
write, then the `repo-changed` event must cross the Tauri IPC boundary and be dispatched on a
frontend event loop that is busy rendering the just-completed round. `600 ms = 2×` the debounce
budgets: 300 ms debounce + ~300 ms slack for event dispatch, `spawn_blocking` scheduling, and
render contention. Operation *duration* does not inflate the gap — the debounce measures quiet
time after the **last** write, not total op time, so even a multi-second rebase echoes ≈ 300 ms
after it finishes. `TTL` is a single named constant, tunable; it may be lowered toward ~450 ms if
telemetry shows echoes always land < 350 ms.

**Correctness / residual-risk analysis (requirement 3).**
- A genuine external change with **no** preceding local mutation → no arm → not suppressed →
  refetches normally.
- A concurrent external write that lands *before* the mutation-round's reads → already captured
  by that round (the round reads current FS state ≈ 0 ms after arm).
- A concurrent external write coalesced into the *same* debounce window as the self-write → its
  data is in the FS the mutation-round already read; suppressing the combined echo loses nothing.
- **Residual (accepted):** an external write landing in the 300-600 ms tail *after* the
  mutation-round's reads but whose `repo-changed` still arrives inside the window would be dropped.
  In a single-user desktop client this is astronomically rare, and it self-heals on the next
  focus rescan, manual Refresh, activation, or the user's next action. Documented, not mitigated.
- Background-tab freshness is preserved because suppression is keyed by `repoId` and there is one
  tab per `repoId` (§1); repo B's watcher is unaffected by repo A's arm.

---

## 5. Granularity — OPEN DECISION (requirement 4)

**OD-P81-1: Where does suppression live — RepoWorkspace-only vs a shared cross-container gate?**

- **Option A (per-instance, local to RepoWorkspace).** Simplest; but the two sibling panels keep
  refetching on the self-echo (each does 1 lightweight fetch). Storm's main cost (the 9-10 parallel
  RepoWorkspace fetches) is killed, panels' 1-fetch echo is minor and pre-existing.
- **Option B (shared module-level per-`repoId` registry; RECOMMENDED).** `armEcho`/`isSuppressed`
  live in `echoSuppression.ts` as a singleton keyed by `repoId`. RepoWorkspace arms on mutation;
  RepoWorkspace *and* both panels consult `isSuppressed` in their `onRepoChanged` handlers. One
  arm suppresses the single self-echo across the whole container tree for that repo. Safe because
  of the one-tab-per-`repoId` invariant (§1). Cost: module-level shared state needs an explicit
  reset hook in tests (`__resetEchoSuppression()`), and `clearRepo(repoId)` must run on tab close
  to avoid unbounded map growth.

**Recommendation: Option B.** It is the only option that makes the self-echo fully dead
(the requirement's spirit) and it answers the global-vs-per-container question decisively:
**global, per-`repoId`.** Wiring the two panels is a one-line gate each (see §7); if the
orchestrator wants the smallest possible first landing, ship RepoWorkspace wiring first and add
the two panel gates in the same increment (they are trivial and covered by their existing tests).

---

## 6. New files (requirement: keep RepoWorkspace.tsx from growing)

All three are new, small, single-responsibility, well under the ~500-line limit. No React state
math is added to `RepoWorkspace.tsx`; it only swaps call-site bodies.

### `src/components/repoWorkspace/refreshCoalescer.ts` (pure, no React)

```ts
export interface RefreshCoalescer {
  /** Enqueue a round. Resolves when the round serving this call (the leading
   *  round, or the single trailing round it collapsed into) settles. */
  request(): Promise<void>;
  /** True while a round is in flight (test/inspection helper). */
  readonly isRunning: boolean;
}

/** `run` MUST NOT throw (the canonical round is already try/caught). */
export function createRefreshCoalescer(run: () => Promise<void>): RefreshCoalescer;
```

### `src/components/repoWorkspace/echoSuppression.ts` (pure, module-level singleton)

```ts
/** Suppression window after a self-initiated (mutation) refresh, in ms. */
export const ECHO_TTL_MS = 600;

/** Arm the window for `repoId`. `now` defaults to Date.now() (injectable for tests). */
export function armEcho(repoId: string, now?: number): void;

/** True iff a watcher event for `repoId` at `now` falls inside the armed window. */
export function isEchoSuppressed(repoId: string, now?: number): boolean;

/** Drop `repoId`'s entry (call on tab close / RepoWorkspace unmount). */
export function clearEchoSuppression(repoId: string): void;

/** Test-only: wipe the registry between vitest cases. */
export function __resetEchoSuppression(): void;
```

### `src/components/repoWorkspace/useCoalescedRefresh.ts` (React hook)

```ts
export type RefreshOrigin =
  | 'mutation'    // a git write op just completed → arms echo suppression
  | 'manual'      // Refresh button — always runs
  | 'activation'  // tab flip to active — always runs
  | 'focus'       // window focus — always runs
  | 'watcher';    // repo-changed event — gated by echo suppression

export interface UseCoalescedRefresh {
  /** Run a coalesced refresh round for `origin`. 'watcher' resolves immediately
   *  (no round) while the self-echo window is active; all other origins always
   *  enqueue. Resolves when the serving round settles. */
  refresh(origin: RefreshOrigin): Promise<void>;
}

/** Binds one coalescer instance + the shared echo registry to `run` for `repoId`.
 *  `run` is the canonical refresh round (today's refreshAll body). */
export function useCoalescedRefresh(
  repoId: string,
  run: () => Promise<void>,
): UseCoalescedRefresh;
```

Hook internals (spec, not code): create the coalescer once (`useRef`) bound to a stable `run`
(kept current via a ref so identity churn doesn't rebuild the coalescer). `refresh(origin)`:
`if (origin === 'watcher' && isEchoSuppressed(repoId)) return Promise.resolve();`
`if (origin === 'mutation') armEcho(repoId);` then `return coalescer.request();`. On unmount,
`clearEchoSuppression(repoId)`.

---

## 7. Integration edits in RepoWorkspace.tsx (and panels)

Behavior-preserving swaps; net line count roughly flat or lower.

1. **Extract** the current `refreshAll` body (`1146-1184`) into a stable
   `runRefreshRound = useCallback(async () => { ...same body... }, [same deps])`.
2. `const { refresh } = useCoalescedRefresh(repoId, runRefreshRound);`
3. `const refreshAll = useCallback(() => refresh('mutation'), [refresh]);` — keep the name so the
   ~11 mutation call sites and hook props are untouched (mutations cause writes → arm).
4. **Manual refresh** (`~1477`): call `refresh('manual')` (still awaits, still followed by
   `verification.refresh` / `forgeSignals.refresh` / `refetchSigningStatus`).
5. **Activation self-heal** (`1238`): `void refresh('activation')` (via a ref, as today).
6. **repo-changed subscription** (`1302-1313`): replace the 9 inline refetches with
   `void refresh('watcher')`. Keep the `p.repoId !== repoId` filter.
7. **Focus rescan** (`1350-1359`): replace the 9 inline refetches with `void refresh('focus')`;
   keep `forgeSignals.refresh('focus')`.
8. **Pending-op apply** (`~2016`): `await refresh('mutation')` (it applied an op = a write).
9. **Mount load** (`1213-1225`): **leave unchanged** — raw refetches, no `openRepo`, first paint.
   (Not routed through the coalescer; documented exception.)
10. **Panels (Option B):** in `AiAssetsPanel.tsx:149` and `RepoHealthPanel.tsx:363`, change
    `if (p.repoId === repoId) void refresh();` to
    `if (p.repoId === repoId && !isEchoSuppressed(repoId)) void refresh();`
    (import from `echoSuppression.ts`). Their `open`/activation refreshes are unchanged.

The dependency arrays of the subscription (`1329-1340`) and focus (`1375-1387`) effects shrink to
`[repoId, refresh]` (+ `active`, `forgeSignals.refresh` for focus), reducing re-subscription churn.

---

## 8. Acceptance criteria (measurable)

- **AC1 — no echo double-fetch:** 1 mutation + its watcher echo within `TTL` ⇒ **exactly 1** round.
- **AC2 — N mutations ⇒ N rounds:** N sequential mutations (each settles before the next; each
  echo inside `TTL`) ⇒ **exactly N** rounds; the N echoes are all suppressed.
- **AC3 — genuine external change fires:** a `repo-changed` with no preceding mutation ⇒ 1 round.
- **AC4 — post-window external change fires:** mutation (1 round), advance clock past `TTL`, then
  `repo-changed` ⇒ a 2nd round (not swallowed).
- **AC5 — burst coalescing:** while a round is in flight, K≥1 further requests ⇒ exactly 1 trailing
  round (2 total), independent of K.
- **AC6 — forced origins bypass suppression:** arm via mutation, then before `TTL` expiry call
  `refresh('activation')` / `'manual'` / `'focus'` ⇒ each runs a round.
- **AC7 — per-repo isolation:** arming repo A does not suppress a `watcher` refresh for repo B.
- **AC8 — panels (Option B):** within the window, same-`repoId` `onRepoChanged` in
  AiAssetsPanel/RepoHealthPanel does NOT refetch; outside the window (or on `open`/activation) it
  does.
- **AC9 — no user-visible latency regression:** the leading edge still runs immediately, so
  post-mutation feedback timing is unchanged.

---

## 9. Vitest test plan (requirement 5 — fake timers, no real 300 ms waits)

Pure modules take an injectable `now` and a controllable `run`, so tests are deterministic.

- **`refreshCoalescer.test.ts`** (no timers needed; control `run` via deferred promises):
  - idle `request()` runs immediately (`run` called once).
  - K requests mid-flight ⇒ resolve leading ⇒ exactly one trailing ⇒ `run` called twice (AC5).
  - each caller's returned promise resolves when its serving round settles.
  - a `run` rejection is impossible by contract; add a guard test that a resolved-but-late slice
    doesn't start extra rounds.
- **`echoSuppression.test.ts`** (inject `now`; `__resetEchoSuppression()` in `beforeEach`):
  - `armEcho(repo, t0)` ⇒ `isEchoSuppressed(repo, t0+599)` true, `t0+600` false (boundary).
  - `clearEchoSuppression` drops the window.
  - per-`repoId` isolation (AC7).
- **`useCoalescedRefresh.test.tsx`** (`@testing-library/react` + `vi.useFakeTimers` +
  `vi.setSystemTime`; `run` = a `vi.fn` returning a resolved/deferred promise):
  - AC1: `refresh('mutation')` then `refresh('watcher')` within `TTL` ⇒ `run` called once.
  - AC2: loop N mutation+echo pairs ⇒ `run` called N times.
  - AC3: `refresh('watcher')` with no arm ⇒ `run` once.
  - AC4: mutation, advance system time past `TTL`, `refresh('watcher')` ⇒ `run` twice.
  - AC6: mutation then `refresh('activation'|'manual'|'focus')` within `TTL` ⇒ each runs.
  - unmount ⇒ `clearEchoSuppression` called for `repoId`.
- **Panel regression (Option B):** extend `AiAssetsPanel.test.tsx` / `RepoHealthPanel.test.tsx`
  to arm suppression then dispatch a same-`repoId` `repo-changed` and assert `refresh` is NOT
  called; after `TTL`, assert it is (AC8).
- **RepoWorkspace integration smoke** (existing harness): a mutation spy asserts the round runs
  once per mutation and the watcher echo (dispatched via `repoChangedListeners`) adds no second
  round.

---

## 10. OPTIONAL backend alternative (documented, NOT recommended)

Add an origin/nonce field to `RepoChangedPayload` so the frontend can *positively* identify
self-caused events instead of guessing by time. **Cost:** the watcher only sees files, not causes;
to tag origin, each mutation command would have to register an "expected change" token with the
watcher before returning, and the debounce (which coalesces multiple unrelated causes into one
event) would have to carry/merge a set of tokens — real backend plumbing plus a TOCTOU race
between token registration and the debounce flush. It also breaks the invariant that
`repo-changed` is a small, dumb push signal. The frontend time-window (§4) achieves the same
outcome with zero backend change and bounded, self-healing residual risk. **Recommend the
frontend-only design; keep this only if a future audit shows the residual miss is real.**

---

## 11. Flags for the orchestrator

- **OD-P81-1** (§5): granularity — recommend **Option B** (shared per-`repoId` registry, wire
  RepoWorkspace + both panels). Confirm before senior-dev if you prefer the minimal
  RepoWorkspace-only Option A.
- **Behavior delta**: routing the watcher path through the canonical round means watcher events
  now also `openRepo` + `refetchTagSync({force:true})` + `refetchCompare` (a superset of today's 9).
  Recommended (consistency + HEAD/self-heal on external change); flag if the added forced tagSync
  per external event is unwanted — a non-forced round variant is a trivial parameter.
- **`TTL_MS = 600`** is a named tunable; revisit against real dispatch telemetry.
