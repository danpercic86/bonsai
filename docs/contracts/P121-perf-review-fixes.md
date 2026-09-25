# P121 — Fixes from the first /perf-review (session `s229d042a`)

Evidence (read-only, do NOT redo): `D:\Data\Temp\bonsai-perf-review\s229d042a.md`.
Board: `TODO.md` `## P121`. No new IPC commands/channels. One event-payload value added
(`repo-changed` reason `"health"`), one additive obs field set (watcher `fired` records).

**Order by value:** a → b → c → e → d.
**Senior-dev passes (≤2):**

| Pass | Sub-increments | Why grouped |
|---|---|---|
| 1 | P121a + P121b | Both kill the forge/graph cascade; b's Rust parity test reads a TS file, so b's Rust + TS halves land together. |
| 2 | P121c + P121e + P121d | c and e share the "flag/clock to a leaf" pattern in the same sidebar files; d is a small Rust measurement. |

**StrictMode in vitest:** NOT on by default (no global wrapper; `src/main.tsx:24` is dev-app only).
Tests opt in with `{ wrapper: StrictMode }`, and `useRenderCount` then reports doubled counts
(`Sidebar.churn.test.tsx:249` convention). Every test below states its mode.

**P120 lab ratchet (`e2e/perf/`, `scripts/perf-baseline.json`):** no new lab interactions are
proposed — all failing checks are vitest / cargo tests. If an EXISTING interaction's `ipcCalls` /
`reactCommits` falls after this work, the ratchet reports a reclaim: regenerate the baseline
deliberately in the same increment (P120 rule). Any rise is a regression. (The report's proposed
`health-tick` / `clock-tick` lab siblings would be deliberate ratchet bumps — not in scope.)

---

## P121a — `forgeCommitStatuses` cost (error severity)

### Root cause (verified)

1. **Uniform 60 s TTL for every CI verdict.** `collectCiShas` (`useForgeSignals.ts:57-77`)
   skips a sha only while `isFresh(entry.tsMs, now, TTL_MS=60_000)`. A `success` verdict for an
   immutable sha is refetched on the first graph change after 60 s. Autofetch (5 min) → new graph
   identity → `forgeSignals.refresh('graph')` (`RepoWorkspace.tsx:1042`) → TTL expired → all
   ≤100 tips refetched. `force` (manual/remote op) bypasses the cache entirely (unchanged).
2. **No in-flight coalescing.** `runFetch` (`:184-297`) bumps `reqIdRef` and starts a new batch;
   the earlier batch still runs to completion in Rust and its result is discarded (`:260`).
   `ciCacheRef` is only rebuilt when a batch lands (`:280`), so the duplicate asks for the same
   100 shas. 8 of 36 calls in the session, 57 s.
3. **Rust side:** `batch_commit_statuses` (`rollup.rs:125-157`) is serial; GitHub's
   `combined_status` (`github/mod.rs:185-194`) makes **2** blocking GETs per sha. The command
   already runs in `spawn_blocking` (`commands/forge.rs:413`); transport is blocking. Correct as is.

Session arithmetic (estimate from the report): 36 calls ≈ 6 boot + 12 health-tick (P121b removes)
+ 8 in-flight duplicates (coalescing removes) + ~10 TTL-expired graph refreshes (tiered TTL removes
most). Expected residual: ~1 batch per repo at boot plus pending-tip polls.

### Decision

**Frontend-only. No Rust change. No parallelism.**
- Parallelizing the per-sha loop is rejected: P113a exists because the host limit was tripped, and
  GitHub secondary-rate-limit guidance discourages concurrent requests.
- A batched GraphQL request is **deferred** (follow-up, see Open questions): GitHub GraphQL
  requires a token (REST here works unauthenticated — `self.token.as_deref()` is `Option`), it
  covers 1 of 4 providers so `batch_commit_statuses` stays per-sha regardless, and it needs a new
  DTO for the `CheckRun | StatusContext` union. Its measurable target is the residual boot batch.

### Design — tiered TTL (`useForgeSignals.ts`)

```ts
import type { CheckRollup } from '../../ipc';
export const TTL_MS = 60_000;                 // pending / none — a fresh push gains checks soon
export const TTL_TERMINAL_MS = 30 * 60_000;   // success | failure | error | neutral

/** PURE. Terminal verdicts for an immutable sha rarely change (a CI re-run can flip one,
 *  hence finite). `force` still bypasses every TTL. */
export function ciTtlFor(rollup: CheckRollup): number;
//   'success' | 'failure' | 'error' | 'neutral' → TTL_TERMINAL_MS
//   'pending' | 'none'                          → TTL_MS

// SIGNATURE CHANGE: `ttlMs: number` → `ttlOf: (rollup: CheckRollup) => number`.
export function collectCiShas(
  tips: readonly string[], prHeadShas: readonly string[],
  cached: ReadonlyMap<string, CiEntry>, now: number,
  ttlOf: (rollup: CheckRollup) => number, force: boolean, max: number,
): string[];
// skip rule: !force && entry !== undefined && isFresh(entry.tsMs, now, ttlOf(entry.badge.rollup))
```
The PR-list TTL (`prTsRef`, `TTL_MS`) is unchanged. `rebuildCiCache` is unchanged (carried-over
entries keep their original `tsMs`, so a terminal entry ages correctly).

### Design — in-flight coalescing (`useForgeSignals.ts`)

Mirrors `useTagSync`'s P113b `inFlight` ref, but **coalesces instead of dropping**.

```
refs:  inFlightRef: number | null      // reqId of the running fetch, null when idle
       rerunRef: { pending: boolean; force: boolean }

runFetch(force):
  clearDebounce()
  if !enabledRef.current: return
  if inFlightRef.current !== null:            // a batch is on the wire
    rerunRef.pending = true
    rerunRef.force ||= force                  // sticky, like pendingForceRef
    return
  layout = graphDataRef.current; if layout === null: return
  reqId = ++reqIdRef.current
  inFlightRef.current = reqId
  try { …body unchanged (every early return still passes through finally)… }
  catch { …unchanged… }
  finally:
    if inFlightRef.current === reqId:
      inFlightRef.current = null
      if rerunRef.pending && reqIdRef.current === reqId:
        f = rerunRef.force; rerunRef = { pending: false, force: false }
        void runFetch(f)                      // reads the LATEST graphDataRef
```
Invalidation sites — the enable/repo effect (`:323-331`) and the disabled branch of `refresh`
(`:301-307`) — additionally do `inFlightRef.current = null; rerunRef = {false,false}` next to
their existing `reqIdRef.current += 1`, so a stale batch never blocks the new repo's first fetch
and its `finally` becomes a no-op. The rerun normally finds every sha cached-fresh (the first batch
just rebuilt the cache) → 0 `forgeCommitStatuses` calls; it only costs the local
`forgeRepoContext`. P113a semantics untouched: `isForgeSuppressed` is still checked per run, and
`noteForgeRateLimit` still fires on `stoppedBy` / catch.

### Files

| File | Change |
|---|---|
| `src/components/repoWorkspace/useForgeSignals.ts` | `ciTtlFor`, `TTL_TERMINAL_MS`, `collectCiShas` signature, coalescing guard. Stays <400 lines. |
| existing pure-function test file for `collectCiShas` | Update call sites; add TTL cases. |
| `src/components/repoWorkspace/useForgeSignals.coalesce.test.tsx` (new) | Hook tests below. |

### Failing tests (fail today)

1. **Pure (no React):** entry `{rollup:'success', tsMs: now-61_000}` → `collectCiShas(..., ciTtlFor,
   force=false, ...)` excludes it (today's 60 s TTL includes it). Same entry with `force=true` →
   included. `pending` aged 61 s → included. `success` aged `TTL_TERMINAL_MS+1` → included.
2. **Hook, non-StrictMode, fake timers (the report's check):** `vi.spyOn(mockIpc,
   'forgeCommitStatuses')` returning a deferred promise; mount with a 3-tip layout; advance 300 ms
   → enable batch A on the wire (held). Advance 1 s, `refresh('graph')` (non-forced), advance
   300 ms. Resolve A, flush. Assert `forgeCommitStatuses` called exactly **1** time (today 2 — the
   second run requests the identical set while A is held); `forgeRepoContext` called 2 times (the
   rerun ran and found everything cached-fresh).
3. **Hook, forced variant:** same, but the second trigger is `refresh('manual', true)` → exactly 2
   calls, and the second starts only after A resolved (max concurrent pending = 1).
4. **Hook:** disable (`showCiStatus=false`) while A is held, re-enable → the new enable fetch is
   issued without waiting for A; resolving A afterwards publishes nothing.

### Acceptance

- Identical sha set is never requested twice concurrently from one hook instance.
- Terminal CI verdicts are not refetched by focus/graph triggers within 30 min; manual refresh and
  fetch/pull/push (`force`) still refetch.
- `useForgeSignals.backoff.test.tsx` passes unchanged (P113a suppression intact).
- Mock IPC unchanged (no surface change).

---

## P121b — health refresh routed as a full refresh

### Root cause (verified)

- `scheduler/exec.rs:242-251` emits reason `"fs"` for every non-`AutoFetch` job, i.e.
  `HealthRefresh`. `useRepoChangeSubscription.ts:77-80` maps `"fs"`/unknown →
  `refresh('watcher','full')`: graph re-stream + branches + tag sync + forge cascade, ×6 repos.
- **"Is `repo_changed` always true?" — yes, by design, not a bug.** `exec.rs:142-148` sets
  `emit_repo_changed: true` for every successful health run: P30 D8 defines the job as a pure
  signal that does no git work, so there is nothing to compare. Not changed here.
- Consumers of `repo-changed` besides the workspace: `RepoHealthPanel.tsx:368` (refreshes itself
  while open, any reason) and `AiAssetsPanel.tsx:105`. Both keep working with a new reason.
- No Rust/TS parity test on reason strings exists today.

### Design

Rust — `src-tauri/src/commands/repo.rs` (next to `RepoChangedPayload`):
```rust
/// Wire values of `RepoChangedPayload.reason`. Rust is authoritative; the TS mirror is
/// `REPO_CHANGED_REASONS` in `src/ipc/types/common.ts` (parity-tested).
pub mod repo_changed_reason {
    pub const FS: &str = "fs";
    pub const FS_WORKTREE: &str = "fsWorktree";
    pub const FETCH: &str = "fetch";
    pub const TAGS: &str = "tags";
    pub const HEALTH: &str = "health"; // P121b: scheduled healthRefresh tick
    pub const ALL: [&str; 5] = [FS, FS_WORKTREE, FETCH, TAGS, HEALTH];
}
```
Use the constants at all emit sites: `commands/repo.rs:~74-78`, `commands/remotes.rs:57`,
`scheduler/exec.rs:245`. In `exec.rs`, replace the `if matches!` with an **exhaustive** match (no
wildcard, so a new `JobKind` fails to compile):
```rust
reason: match job { JobKind::AutoFetch => repo_changed_reason::FETCH,
                    JobKind::HealthRefresh => repo_changed_reason::HEALTH }.to_string(),
```
Update the doc lists at `commands/repo.rs:11-21`.

TS — `src/ipc/types/common.ts`:
```ts
/** MIRROR of Rust `commands::repo::repo_changed_reason::ALL` (parity-tested). */
export const REPO_CHANGED_REASONS = ['fs', 'fsWorktree', 'fetch', 'tags', 'health'] as const;
export type RepoChangedReason = (typeof REPO_CHANGED_REASONS)[number];
```
`RepoChangedPayload.reason` stays `string` (unknown → full stays the safe default); add `"health"`
to its doc list.

Routing — `useRepoChangeSubscription.ts`, new explicit case:
```ts
case 'health':
  // P121b: scheduled self-heal tick (P30 D8), not an fs echo → `external` origin (never
  // echo-suppressed). Narrow `worktree` scope ({status, opState, submodules}): D8's intent
  // is "recompute status/health"; RepoHealthPanel refreshes itself on any repo-changed.
  // worktree has openRepo:false — sound, a tick never moves HEAD (P99 invariant).
  void refresh('external', 'worktree');
  break;
```
`default:` stays `refresh('watcher','full')`. (Alternative `health` → no workspace round: see
Open questions.)

Mock — `src/ipc/mock/scheduler.ts:105-110`: reason `job === 'autoFetch' ? 'fetch' : 'health'`;
update the comment.

Parity test — new `src-tauri/src/commands/tests_repo_changed_reason_parity.rs` (registered
`#[cfg(test)] mod` in `commands/mod.rs`), same pattern as `obs/tests_schema_parity.rs`:
`include_str!("../../../src/ipc/types/common.ts")`, strip comment lines, remove all whitespace,
build the expected literal from `ALL` (`export const REPO_CHANGED_REASONS=['fs','fsWorktree',…]as
const;`, whitespace-free) and assert it occurs exactly once. Failure message: edit TS, not Rust.

### Files

| File | Change |
|---|---|
| `src-tauri/src/commands/repo.rs` | `repo_changed_reason` module; use `FS`/`FS_WORKTREE`; docs. |
| `src-tauri/src/commands/remotes.rs` | `TAGS` constant. |
| `src-tauri/src/scheduler/exec.rs` | Exhaustive reason match. |
| `src-tauri/src/scheduler/test_support.rs` | `repo_changed_reasons(&events) -> Vec<String>`. |
| `src-tauri/src/scheduler/run_now_tests.rs` | Extend the `:103-106` health test. |
| `src-tauri/src/commands/tests_repo_changed_reason_parity.rs` (new) + `commands/mod.rs` | Parity. |
| `src/ipc/types/common.ts` | `REPO_CHANGED_REASONS`, `RepoChangedReason`, docs. |
| `src/components/repoWorkspace/useRepoChangeSubscription.ts` (+ `.test.tsx`) | `health` case. |
| `src/ipc/mock/scheduler.ts` (+ `scheduler.test.tsx:102-109`) | Emit `health`; assert reason. |

### Failing tests (fail today)

1. Rust `run_now_tests.rs`: health run-now → `repo_changed_reasons(&events) == ["health"]`
   (today `["fs"]`). Existing `repo_changed_count == 1` assertion kept.
2. Rust parity test (fails until the TS constant exists).
3. Vitest, non-StrictMode, `useRepoChangeSubscription.test.tsx`: dispatch
   `{repoId: REPO, reason: 'health'}` → `refresh` called once with `('external','worktree')`
   (today `('watcher','full')`). Plus: dispatch every `REPO_CHANGED_REASONS` member and assert
   none except `'fs'` reaches `('watcher','full')` (guards future reasons falling into `default`).
4. Vitest `scheduler.test.tsx`: healthRefresh completion emits `reason: 'health'`.

### Acceptance

- A health tick never triggers `streamGraph`, `listBranches`, `listTagSync` or
  `forgeCommitStatuses`; it runs `getStatus` / op state / submodules only.
- Unknown reasons still → full. Parity test red if either side's list drifts.

---

## P121c — 30 s job clock re-renders whole workspaces

### Root cause (verified)

`useJobStatus.ts:111-115` holds `jobNow` state with `setInterval(…, 30_000)`; the hook runs inside
`RepoWorkspace` (`:1046`), so each tick re-renders the whole container in all 6 mounted workspaces.
Consumers of the clock: `WorkspaceToolbar` (`jobNow` prop, `WorkspaceToolbar.tsx:136,144`) and
`Sidebar` `now` prop (`RepoWorkspace.tsx:1879` → `TagsSection:228` "Last checked" title,
`StashesSection:74` → `StashRow:294` age badge). No other consumer (grepped).

### Design — one shared, aligned clock store (no provider)

New `src/hooks/useNow.ts`. A module-level external store read via `useSyncExternalStore`, chosen
over a context provider so App.tsx (at its size baseline) is not touched and components rendered
outside the workspace (shared rows) need no default.

```ts
export const NOW_PERIOD_MS = 30_000;
/** Current time quantized to NOW_PERIOD_MS boundaries (ms). Stable within a period, so
 *  getSnapshot is consistent across React's double reads. */
function snapshot(): number;              // Math.floor(Date.now() / P) * P
function subscribe(cb: () => void): () => void;
//   first subscriber → arm setTimeout(tick, P - (Date.now() % P)); tick → notify all, re-arm
//   for the next boundary (setTimeout chain, not setInterval — stays boundary-aligned)
//   last unsubscribe → clearTimeout
/** Epoch ms, re-rendering ONLY the calling component once per period. */
export function useNow(): number;         // useSyncExternalStore(subscribe, snapshot)
/** Test-only: reset module state between tests. */
export function __resetNowForTests(): void;
```
Label error ≤ 30 s, same as today; `relativeDate`'s finest bucket is 1 min.

New leaf `src/components/RelativeAge.tsx`:
```tsx
/** Re-renders itself on the shared clock; parents never see the tick. */
export function RelativeAge(props: { tsSec: number }): ReactElement; // relativeDate(tsSec, floor(useNow()/1000))
```
For the TagsSection `title` attribute (a string prop, not a child), extract the offline line into
a leaf `TagSyncOfflineNote` (`src/components/sidebar/TagSyncOfflineNote.tsx`) that calls
`useNow()` for its `title`; TagsSection renders it only when `tagSyncState === 'unavailable'`.

Removals: `jobNow` from `UseJobStatus`, the interval effect in `useJobStatus`, `jobNow` from
`WorkspaceToolbar` props (it calls `useNow()` itself), `now` from `SidebarProps`, `TagsSection`,
`StashesSection`, `StashRow` (`rows.tsx:243-257`; replace `:294` with `<RelativeAge tsSec={ts}/>`
and drop the "KNOWN CONSEQUENCE" note at `:300-306`, now resolved). `RepoWorkspace.tsx` loses the
`jobNow` binding and both props (net ≤ −2 lines) — regenerate the size baseline
(`pnpm lint:size -- --update-baseline`) so the reclaim is locked (TODO rule).

### Files

| File | Change |
|---|---|
| `src/hooks/useNow.ts` (+ `useNow.test.tsx`) | New store + hook. |
| `src/components/RelativeAge.tsx` | New leaf. |
| `src/components/sidebar/TagSyncOfflineNote.tsx` | New leaf (offline line + title). |
| `src/components/repoWorkspace/useJobStatus.ts` | Drop clock. |
| `src/components/WorkspaceToolbar.tsx` | `useNow()` instead of prop. |
| `src/components/Sidebar.tsx`, `sidebar/TagsSection.tsx`, `sidebar/StashesSection.tsx`, `sidebar/rows.tsx` | Drop `now`. |
| `src/components/RepoWorkspace.tsx` | Drop `jobNow` wiring. |
| Existing tests passing `now=`/`jobNow=` | Remove the prop (test-only edits). |

### Failing tests (fail today)

1. **Non-StrictMode, fake timers:** `renderHook(() => { renders++; return useJobStatus(...) })`,
   flush mount, record `renders`, advance 30 s → `renders` unchanged (today +1).
2. **`useNow.test.tsx`:** three components using `useNow` → `vi.getTimerCount() === 1`; unmount
   all → 0; advancing to the next boundary re-renders each consumer exactly once.
3. **StrictMode (`wrapper: StrictMode`), fake timers, tally helpers from
   `Sidebar.churn.test.tsx`:** render `StashesSection` with 3 stashes, discard mount tallies,
   advance 60 s → `StashRow` and `StashesSection` tallies 0, and the badge text advanced (e.g.
   "1 minute ago" → "2 minutes ago"). Compiles only after `now` is removed — the failing-today
   form is test 1.

### Acceptance

- A clock tick re-renders only `RelativeAge` / `TagSyncOfflineNote` / `WorkspaceToolbar`
  instances; `RepoWorkspace`, `Sidebar`, `GraphCanvas`, `WorkspaceRightPanel`, sections and rows
  do not re-render.
- Exactly one timer for the whole app regardless of mounted workspaces.
- Labels still advance ("Fetched 1m ago" → "2m ago").

---

## P121e — Tags section renders twice per check

### Root cause (verified)

`useTagSync.ts:101` `setState('checking')`, then `:107-109` `setReport` + `setState('ready')` +
`setCheckedAt` (batched → one render). `TagsSection` consumes `tagSyncState` in its body:
`ready` gates `byName`/`remoteOnly`/`divergedCount` (`:136-162`), so during a re-check the body
re-renders with badges REMOVED, then again with badges back (a 5-min flicker), and the rollup
badge reads `busy={tagSyncState === 'checking'}` (`:198`). Same shape as P118's busy flag;
P118's fix = split the flag, carry the rendered part by context to a leaf
(`sidebar/sidebarBusyContext.ts:1-26`).

### Design

`useTagSync` (`useTagSync.ts`):
```ts
export interface UseTagSync { /* existing fields */ checking: boolean; }
// state: 'checking' now ONLY when there is no settled verdict yet (from 'idle'); a re-check keeps
// the last settled 'ready' | 'unavailable' (stale-while-revalidate).
refetch:  …guards unchanged…
          setChecking(true); if (stateRef.current === 'idle') setState('checking')
  success (latest id): setReport(r); setState('ready'); setCheckedAt(…); setChecking(false)
  failure (latest id): setState('unavailable'); setChecking(false); lastFetch = 0
  remoteCount===0 path and clear(): also setChecking(false)
```
New `src/components/sidebar/tagSyncCheckingContext.ts` (mirror `sidebarBusyContext.ts`):
```ts
export const TagSyncCheckingContext = createContext<boolean>(false);
/** Leaf controls only — consuming re-renders on every flip. */
export function useTagSyncChecking(): boolean;
```
New leaf `src/components/sidebar/TagSyncRollup.tsx` wrapping `SectionRollupBadge` with
`busy = useTagSyncChecking() || state === 'checking'` (props: `count`, `remote`, `state`).
`TagsSection` renders `<TagSyncRollup …/>` as `extra` and never reads the context.
`Sidebar` gains prop `tagSyncChecking: boolean` and wraps `TagsSection` in
`<TagSyncCheckingContext.Provider value={tagSyncChecking}>`; `RepoWorkspace` passes
`checking` from `useTagSync` (net growth offset by P121c's removals).

The container still re-renders on the `checking` flip (state lives in `RepoWorkspace`) — out of
scope; the memoised section body and rows no longer do.

### Files

| File | Change |
|---|---|
| `src/components/repoWorkspace/useTagSync.ts` (+ `.test.tsx`) | `checking`, settled-state rule. |
| `src/components/sidebar/tagSyncCheckingContext.ts`, `sidebar/TagSyncRollup.tsx` | New. |
| `src/components/sidebar/TagsSection.tsx` | Use leaf; drop `busy` read. |
| `src/components/Sidebar.tsx`, `src/components/RepoWorkspace.tsx` | Provider + prop. |

### Failing tests (fail today)

1. **Non-StrictMode**, harness component wiring the real `useTagSync` (mock IPC `listTagSync`
   deferred) into `TagSyncCheckingContext.Provider` + `TagsSection`; settle a first check; reset
   tallies; `refetch({force:true})`, resolve with a changed report → `TagsSection` tally **1**
   (today 2); while pending, the rollup shows "checking…" and row badges from the previous report
   remain (today they vanish).
2. `useTagSync.test.tsx`: during a re-check `state` stays `'ready'` and `checking === true`; first
   check from idle → `state === 'checking'`.

### Acceptance

- ≤ 1 body render of `TagsSection` per check (StrictMode: ≤ 2); the busy leaf may render twice.
- Existing `useTagSync` in-flight floor (P113b) and cache-window tests pass.

---

## P121d — watcher double worktree refresh (measurement only)

### Root cause analysis (verified against code; writer still unknown)

- Fixed 300 ms trailing debounce (`watcher/mod.rs:89`, loop `:218-243`); `.git/index` is classed
  `Refs` (`classify.rs:69-76`), so the redundant **worktree** rounds had no index path — the
  second burst was working-tree paths only.
- `classify` returns `Worktree` for **anything outside `.git`, including gitignored paths**
  (`classify.rs:33`). Leading hypothesis: IDE save (round 1) followed ~300–1000 ms later by tooling
  writing ignored output (`dist/`, `.angular/cache`, `target/`) → round 2 with identical status.
  Fits ham-digi-web (Angular) topping the list and the 55k pre-debounce records.
- **Why no fix now:** a cooldown/max-wait *spaces* the two rounds but does not merge them (write
  at F+400 still fires, just later); merging needs ≥1 s quiet, breaking the ~300 ms invariant and
  adding latency to every change. `keepIfUnchanged(setStatus)` (`RepoWorkspace.tsx:691`) already
  dedups the React side, and the open-diff refetch (`:693-719`) is legitimate — equal status
  counts ≠ equal content (report's own `hunks 4→3` case). Dropping all-ignored bursts is
  semantically safe for status but **not yet**: on Windows an event-lossy burst might keep only
  its ignored path while the tracked-file event was dropped — the same belt-and-braces argument
  as the index classification. Measure first.

### Design — count ignored paths per fired burst (dev mode only)

- `WatchTick` stops being `Copy`; it carries `worktree_paths: Vec<PathBuf>` (absolute paths whose
  class is `Worktree`), collected only when `crate::obs::trace::active_sink().is_some()`.
- `BurstAcc` gains `worktree_paths: Vec<PathBuf>` capped at `IGNORE_SAMPLE_CAP = 512` (dedup not
  required) and `worktree_total: u32`. Paths never leave the debounce thread and are never logged.
- At fire time, if the sink is active and `worktree_total > 0`: strip the canonical `workdir`
  prefix, call core:
```rust
// crates/bonsai-core/src/git/ignore.rs (new; `pub mod ignore;` in git/mod.rs)
/// How many of `rel_paths` (workdir-relative) are gitignored. None if the repo cannot be
/// opened. Opens the repo per call (ignore rules may have changed). Blocking; called on the
/// watcher debounce thread, never on a tokio worker.
pub fn count_ignored(workdir: &Path, rel_paths: &[PathBuf]) -> Option<u32>;
```
- `LogPayload::Watcher` (`obs/record.rs:221-251`) gains, `fired` records only,
  `#[serde(skip_serializing_if = "Option::is_none")]`:
  `worktree_paths: Option<u32>` (= `worktree_total`), `ignore_checked: Option<u32>` (≤ cap),
  `ignored: Option<u32>`. TS mirror `WatcherPayload` (`src/obs/types.ts:101-111`):
  `worktreePaths?`, `ignoreChecked?`, `ignored?: number`. Additive optional fields: follow
  `record.rs:18-19` (no `OBS_SCHEMA_VERSION` bump) unless that rule's current text says otherwise.
- `on_change` behaviour is **unchanged** — no burst is dropped.

### Files

| File | Change |
|---|---|
| `crates/bonsai-core/src/git/ignore.rs` (new) + `git/mod.rs` | `count_ignored` + tests. |
| `src-tauri/src/watcher/mod.rs` (+ `tests.rs`) | Tick/acc path sampling, fired-record fields. Stays <400 lines; if not, move `BurstAcc` to `watcher/burst.rs`. |
| `src-tauri/src/obs/record.rs`, `src/obs/types.ts` | Additive fields. |

### Failing tests (fail today — symbols do not exist)

1. `count_ignored`: temp repo (git2), `.gitignore` = `dist/\n*.log\n`, create `dist/a.js`,
   `src/a.ts`, `x.log` → `Some(2)`; non-repo dir → `None`.
2. `BurstAcc`: absorb 600 worktree paths → `worktree_total == 600`, retained == 512; refs-only
   ticks add no paths.
3. Record serde: a fired record with the three fields serializes camelCase; a non-fired record
   omits them.

### Acceptance

- Behaviour identical (no dropped/added `repo-changed`); zero extra work when dev mode is off.
- **USER CHECKPOINT:** next dev-mode session + `/perf-review` reports, for the second burst of
  each `redundant-refresh` pair, whether `ignored == ignoreChecked`. If yes → file "drop
  all-ignored worktree bursts" with the Windows event-loss risk addressed; if no → the rounds are
  real content churn and P121d closes as not-a-bug.

---

## Open questions (flag to orchestrator / user ruling)

1. **P121e visible behaviour:** during a tag re-check the row badges now persist
   (stale-while-revalidate) instead of vanishing; this supersedes the tag-sync contract's §2.2
   "badge-less while checking" for re-checks (first check unchanged). Recommend accept. Small
   visible change despite the board's "no ui-designer pass".
2. **P121b routing:** `health` → `('external','worktree')` (recommended; keeps D8 "recompute
   status") vs no workspace round at all (health panel self-refreshes; cheapest). Neither heals
   a missed refs event — focus rescan + manual refresh remain the refs safety net.
3. **P121a `TTL_TERMINAL_MS` = 30 min** (a CI re-run flipping failure→success shows late unless
   the user refreshes or pushes). Recommend 30 min; 10 min if re-runs matter.
4. **P121a GraphQL batching** deferred to a follow-up (token-only, GitHub-only, new DTO); target =
   the residual boot batch. File it only if a post-P121 session still shows `forgeCommitStatuses`
   as a top cost.
5. **P121d** ships as measurement only — no redundant-refresh reduction expected from P121.

**User rulings 2026-09-25:** Q1 accepted (badges persist during re-checks). Q2 `health` →
`('external','worktree')`. Q3 `TTL_TERMINAL_MS` = 30 min. Q4/Q5 accepted as recommended
(GraphQL deferred; P121d measurement only).
