# P123 — git activity per repo + unsubscribe on unmount

Parents: `P119-activity-for-all-actions.md` (stream, brackets, mock), `P119-ui.md` (dock; no visual
change here). Source: `TODO.md` "P122 follow-ups" + user rulings 2026-09-25 (items 1 and 3).
No UI contract needed: the dock renders the same rows, just fewer of them (no ui-designer step).

## 0. Scope

1. Each tab's dock shows only its own repo's runs. Routing is **backend-side** (per-repo
   subscriptions): a hidden tab receives **zero** channel messages for another repo's run, so it
   pays no deserialisation, no `noteDelivery`, no `onEvent`. This removes the P122c cost.
   Frontend-side filtering was rejected: every channel still deserialises its own copy.
2. Unsubscribe on unmount. The live channel count equals the number of mounted `RepoWorkspace`s,
   including under StrictMode's double mount.

Non-goals: no change to categories, targets, outcomes, line caps, dock UI, or `repo-changed`.

## 1. Decisions

| # | Topic | Decision |
|---|---|---|
| D1 | Where the repo id comes from | The **bracket**, not core. `logged_blocking` and the 7 direct `with_activity` callers already hold `repo_id`; they pass it through a `ScopedActivityHub` (§3.2). Core never looks up repos. |
| D2 | Routing key vs wire field | Kept separate. The routing key lives in the emitter closure (`move \|ev\| scoped.emit(ev)`), so the hub never inspects events and needs no id→repo map. On the wire, `repoId` rides on **`started` only**, like `target`. It is used only for a defensive frontend check (§4.5). |
| D3 | Runs with no repo (clone, init) | **Recommended: broadcast** (route `None` → every subscriber). This keeps P119 §7 visible behaviour (the row appears in the open tabs' docks). It has no dependency on `active_repo` going stale, and these runs are rare, so the cost is negligible. **FLAG Q1** (§8). |
| D4 | Unsubscribe mechanism | Explicit `git_activity_unsubscribe(subscriptionId)`. Channel-drop semantics cannot work: a JS `Channel` has no close that makes Rust `send` fail, so prune-on-send-failure fires only on reload. It stays as the safety net. |
| D5 | Subscription id | Backend-minted **string** `gas-<n>` from a per-hub counter. Frontend-minted ids were rejected: Tauri invokes are unordered, so an early unsubscribe could miss a late subscribe. A **string** rather than `u64`: `obs/ipcProxy.ts` `supersedeScope` keys non-callback calls by their first string arg. A numeric id would give every tab's unsubscribe the same `''` scope, and concurrent unmounts would log false `superseded` results. |
| D6 | Subscribe validation | `repo_id` is **not** checked against `AppState.repos`. A tab may mount before `open_repo` resolves, and a stale subscription is harmless: it receives nothing. |
| D7 | Hub file | Move `GitActivityHub` out of `state.rs` into `src-tauri/src/activity_hub.rs`, with its tests in their own file. |

## 2. Rust — `bonsai-core` (`crates/bonsai-core/src/git/activity.rs`, 429 → ≤450)

```rust
pub struct GitActivityEvent {
    // …existing fields unchanged…
    /// `Started` ONLY: the repoId (canonical workdir path string, P3e §2) of the
    /// repo this run acts on. Absent for an unscoped run (clone/init, P123 D3).
    /// Opaque to core, which never resolves or opens it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_id: Option<String>,          // wire: "repoId"
    pub elapsed_ms: u64,
}

pub struct ActivityEmitter {
    id: String,
    /// P123: set once here, read only by `started()`. Same rules as `target`:
    /// no setter, no `&mut self`, and `GitActivityRecorder` gains no method.
    repo_id: Option<String>,
    target: Option<String>,
    // …unchanged…
}

impl ActivityEmitter {
    /// Unchanged signature; passes `repo_id: None`. Used only by tests
    /// (`activity_tests.rs`, and `hooks_stream.rs:385`, which is inside
    /// `#[cfg(test)]` at L156, so it is not a production emit site).
    pub fn new(id: String, target: Option<ActivityTarget>,
               emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>) -> Self;

    /// P123: gains `repo_id` (2nd param). Call sites: `commands/activity.rs:90`,
    /// `activity_tests.rs:308`.
    pub fn with_subject(id: String, repo_id: Option<String>, subject: RunSubject,
                        emit: Box<dyn Fn(GitActivityEvent) + Send + Sync>) -> Self;
}
```
- `base()` sets `repo_id: None` (hard-coded, like `target`). `started()` sets
  `ev.repo_id = self.repo_id.clone()`. This is the only read.
- The only production emitter is `commands/activity.rs:90`. `hooks_stream.rs` changes only through
  the `new()` default.

## 3. Rust — `src-tauri`

### 3.1 New `src-tauri/src/activity_hub.rs` (~130 lines) + `activity_hub_tests.rs` (~180)

This is `GitActivityHub`, moved from `state.rs` L11-52 and extended. `state.rs` keeps
`pub git_activity: GitActivityHub` and imports it from `crate::activity_hub`.

```rust
/// One registered channel. `repo_id: None` = wildcard (receives every run). It is
/// reachable only from Rust test helpers; the command always passes `Some`.
struct Sub {
    id: String,                                   // "gas-<n>"
    repo_id: Option<String>,
    ch: tauri::ipc::Channel<GitActivityEvent>,
}

#[derive(Clone, Default)]
pub struct GitActivityHub {
    subs: Arc<Mutex<Vec<Sub>>>,
    next_id: Arc<AtomicU64>,
}

impl GitActivityHub {
    /// Registers a channel and returns its subscription id (unique per hub).
    pub fn subscribe(&self, repo_id: Option<String>, ch: Channel<GitActivityEvent>) -> String;
    /// Removes exactly the matching sub. IDEMPOTENT: an unknown or already-removed id
    /// returns `false` and does nothing else.
    pub fn unsubscribe(&self, sub_id: &str) -> bool;
    /// Fans out to the subs matching `route` (see `matches`), pruning any whose `send`
    /// errors. `route: None` = every sub.
    pub fn emit_to(&self, route: Option<&str>, ev: GitActivityEvent);
    /// True if any sub matches `route`.
    pub fn is_active_for(&self, route: Option<&str>) -> bool;
    /// True if any sub exists. Kept for the pure, repo-less gates (`arg_target`,
    /// `arg_subject_many`), which only save work; the bracket makes the real decision.
    pub fn is_active(&self) -> bool;
    /// Test/diagnostic count of live subs.
    pub fn subscriber_count(&self) -> usize;
}

/// route None       → true for every sub (unscoped run: broadcast, D3)
/// route Some(r)    → sub.repo_id == Some(r) || sub.repo_id == None
fn matches(sub: &Sub, route: Option<&str>) -> bool;

/// A hub handle bound to one route. This is what the brackets take instead of the
/// bare hub, so a bracket cannot emit unscoped by accident.
#[derive(Clone)]
pub struct ScopedActivityHub {
    hub: GitActivityHub,
    repo_id: Option<String>,
}
impl ScopedActivityHub {
    pub fn new(hub: GitActivityHub, repo_id: Option<String>) -> Self;
    pub fn repo_id(&self) -> Option<&str>;
    pub fn is_active(&self) -> bool;                 // hub.is_active_for(self.repo_id())
    pub fn emit(&self, ev: GitActivityEvent);        // hub.emit_to(self.repo_id(), ev)
}
```
The lock is recovered on poison, as it is today. `emit_to` holds the lock for the `retain` only,
as it does today.

### 3.2 `state.rs` (116 → ~85)

- Remove the hub body; `use crate::activity_hub::{GitActivityHub, ScopedActivityHub};`.
- Replace `git_activity_hub()` with:
```rust
impl AppState {
    /// The hub bound to one repo's subscribers.
    pub fn git_activity_for(&self, repo_id: &str) -> ScopedActivityHub;
    /// P123 D3: the hub for a run with no repo (clone/init). Broadcast.
    pub fn git_activity_unscoped(&self) -> ScopedActivityHub;
}
```
`git_activity_hub()` is **deleted**. Every caller migrates (§3.4), and the compiler lists them.

### 3.3 `commands/activity.rs` (344 → ≤400)

```rust
/// P123: registers a channel for ONE repo's runs and returns its subscription id.
#[tauri::command]
pub fn git_activity_subscribe(
    state: tauri::State<'_, AppState>,
    repo_id: String,                                   // TS arg: repoId
    on_event: tauri::ipc::Channel<GitActivityEvent>,   // TS arg: onEvent
) -> String;                                           // → hub.subscribe(Some(repo_id), on_event)

/// P123: drops a subscription. IDEMPOTENT, and never errors.
#[tauri::command]
pub fn git_activity_unsubscribe(
    state: tauri::State<'_, AppState>,
    subscription_id: String,                           // TS arg: subscriptionId
);

pub(crate) async fn with_activity<T, F, Fut>(
    hub: ScopedActivityHub,            // was GitActivityHub
    category: GitActivityCategory,
    target: Option<ActivityTarget>,
    run: F,
) -> Result<T, AppError>;

pub(crate) async fn with_activity_ex<T, F, Fut>(
    hub: ScopedActivityHub,            // was GitActivityHub
    category: GitActivityCategory,
    subject: RunSubject,
    classify: fn(&T) -> Option<GitRunOutcome>,
    run: F,
) -> Result<T, AppError>;
```
The `with_activity_ex` body change, as pseudocode:
```
if !hub.is_active() { return run(None).await }            // scoped: no sub for THIS repo ⇒ passthrough
let repo = hub.repo_id().map(str::to_owned)
let h2 = hub.clone()
emitter = Arc::new(ActivityEmitter::with_subject(new_activity_id(), repo, subject,
                                                 Box::new(move |ev| h2.emit(ev))))
…unchanged…
```
- `logged_blocking`: `let hub = state.git_activity_for(repo_id);`. The signature is unchanged.
- `activity_target`: its gate becomes `state.git_activity.is_active_for(Some(repo_id))`. This skips
  the repo open when no tab of that repo listens.
- `arg_target` / `arg_subject_many`: unchanged (global `is_active()`).
- Existing tests: `GitActivityHub::default()` → `ScopedActivityHub::new(GitActivityHub::default(), None)`.

### 3.4 Call-site migration (every change is net 0 lines)

| File:line | Before | After |
|---|---|---|
| `merge.rs:94` | `state.git_activity_hub()` | `state.git_activity_for(repo_id)` |
| `remotes.rs:92,136,178,223` | same | same |
| `staging.rs:81,189` | same | same |
| `repo.rs:423,462` (clone, init) | `state.git_activity_hub()` | `state.git_activity_unscoped()` |
| `tests_activity_coverage_misc.rs:284` | `state.git_activity_hub()` | `state.git_activity_unscoped()` |

`repo.rs` is at **490 lines**: its edit must be net 0. Pass `&repo_id` wherever the local is a
`String`.

### 3.5 Registration + allow-lists

- `lib.rs:247`: add `commands::git_activity_unsubscribe` beside `git_activity_subscribe`. Export it
  from `commands/mod.rs`.
- `obs/metrics_cmds.rs`: `KNOWN_CMDS: [&str; 201]`, with `"gitActivityUnsubscribe"` inserted after
  `"gitActivitySubscribe"` (sorted). `tests_metrics_cmds.rs` re-derives the list from
  `ipc-api.ts` and fails on drift.
- `src/obs/rawArgPolicy.json` has no row for either method (verified), and unlisted means DENY
  (`rawArgPolicy.test.ts:67`), so neither needs a row. Do not add one.
- No capability or permission file lists activity commands (verified), so there is no change there.

### 3.6 Test support (`tests_activity_support.rs`, 106 lines)

```rust
/// Wildcard capture (receives every run). Keeps the 13 existing `subscribe(&state)`
/// call sites unchanged.
pub(super) fn subscribe(state: &AppState) -> Log;            // hub.subscribe(None, ch)
/// P123: capture only `repo_id`'s runs (the production shape).
pub(super) fn subscribe_for(state: &AppState, repo_id: &str) -> (Log, String /*sub id*/);
```

## 4. TypeScript

### 4.1 `src/ipc/types/activity.ts`

```ts
export interface GitActivityEvent {
  // …existing…
  /** `started` only: the repoId this run acts on. Absent = unscoped run
   *  (clone/init), broadcast to every tab. P123. */
  repoId?: string;
}
/** The `gitActivitySubscribe` callback. P123. */
export type GitActivityListener = (e: GitActivityEvent) => void;
```
The subscription id is a plain `string`, opaque and backend-minted, with no alias type.

### 4.2 `src/ipc/types/ipc-api.ts` (baselined at 760: net ≤ 0 lines)

Replace L576-580 (5 lines) with exactly these 4 lines, all ≤ 100 chars. There is no prettier
config, and the file is hand-wrapped at about 100 chars. If a formatter ever wraps a line, drop the
unsubscribe doc line so the block stays ≤ 5 lines.
```ts
  /** P87/P123. One repo's activity channel; resolves to its subscription id. */
  gitActivitySubscribe(repoId: string, onEvent: GitActivityListener): Promise<string>;
  /** P123. Drop a subscription. IDEMPOTENT: an unknown id resolves. */
  gitActivityUnsubscribe(subscriptionId: string): Promise<void>;
```
Note for obs: the callback moves to index 1, so its delivery record name becomes
`gitActivitySubscribe#1`. Nothing in `src/` references `#0` (verified).

### 4.3 `src/ipc/tauri/activity.ts`

```ts
gitActivitySubscribe(repoId: string, onEvent: GitActivityListener): Promise<string> {
  const channel = new Channel<GitActivityEvent>(); channel.onmessage = onEvent;
  return invoke<string>('git_activity_subscribe', { repoId, onEvent: channel });
},
gitActivityUnsubscribe(subscriptionId: string): Promise<void> {
  return invoke<void>('git_activity_unsubscribe', { subscriptionId });
},
```

### 4.4 New `src/components/repoWorkspace/useGitActivitySubscription.ts` (~60 lines)

This owns the subscription lifecycle alone, so the StrictMode race has its own test.
```ts
/** Subscribes `onEvent` to `repoId`'s runs for the lifetime of the mount. */
export function useGitActivitySubscription(repoId: string, onEvent: GitActivityListener): void;
```
Pseudocode (the effect's deps are `[repoId, onEvent]`):
```
effect:
  disposed = false; subId = null
  deliver = (ev) => { if (!disposed) onEvent(ev) }       // nothing lands after cleanup
  ipc.gitActivitySubscribe(repoId, deliver)
     .then(id => { if (disposed) void ipc.gitActivityUnsubscribe(id).catch(noop)
                   else subId = id })
     .catch(noop)                                          // observability: never throw
  cleanup:
     disposed = true
     if (subId !== null) void ipc.gitActivityUnsubscribe(subId).catch(noop)
```
Under StrictMode (mount → cleanup → mount), the first subscribe resolves after its cleanup ran, so
the `.then` unsubscribes it. The result is exactly 1 live subscription.

### 4.5 `useGitActivity.ts` (260 → ≤280)

- Signature: `export function useGitActivity(repoId: string): GitActivityApi`.
- Replace the effect at L210-219 with `useGitActivitySubscription(repoId, onEvent)`. Keep a separate
  small effect for `mounted.current` and `flushTimer` cleanup.
- Defensive drop, first line of the `started` branch:
  `if (ev.repoId !== undefined && ev.repoId !== repoId) return;` (`repoId` joins `onEvent`'s
  deps). Non-started events of a dropped run already fall out through `existing === undefined`.
- Update the module doc (L5-6): per-repo subscription, unsubscribed on unmount.

### 4.6 `useGitDock.ts` + `RepoWorkspace.tsx`

- `useGitDock(deps: { density: PanelDensity; repoId: string })` → `useGitActivity(repoId)`.
- `RepoWorkspace.tsx:1386`: `useGitDock({ density: panelDensity, repoId })`. This is a
  **same-line edit; the file does not grow** (baseline 2261). `App.tsx` is untouched: tabs are
  already keyed by `repoId` (L410).

## 5. Mock IPC

### 5.1 New `src/ipc/mock/gitActivityHub.ts` (~70 lines)

This moves the subscribe/fan-out seam out of `gitActivity.ts` (471; L71-89 removed) and mirrors
§3.1 exactly.
```ts
export function subscribeGitActivity(repoId: string | null, onEvent: GitActivityListener): string; // "gas-mock-<n>"
export function unsubscribeGitActivity(id: string): void;                            // idempotent
/** route null → all subs; route r → subs with repoId === r or repoId === null. */
export function emitGitActivity(route: string | null, event: GitActivityEvent): void;
export function gitActivityActive(route: string | null): boolean;
export function gitActivitySubscriberCount(): number;
```
State is a `Map<id, { repoId: string | null; cb }>` (insertion order = fan-out order). `null`
repoId subs are test-only, as in Rust. Callers import from `gitActivityHub` directly (no
re-export from `gitActivity.ts`). Tests that `import('../gitActivity')` after `vi.resetModules()`
also import `../gitActivityHub`.

### 5.2 `src/ipc/mock/gitActivity.ts` (471 → ≤465)

- `GitSequencer` constructor: `(id: string, route: string | null)`. `emit` calls
  `emitGitActivity(this.route, …)`. `start()` adds `...(route !== null ? { repoId: route } : {})`,
  on started only.
- The signature gains a required first param (it mirrors the Rust scoped hub):
```ts
export function runMockActivity<T>(
  repoId: string | null,               // null = unscoped (clone/init) → broadcast
  category: GitActivityCategory,
  target: string | null,
  fn: () => Promise<T>,
  opts: MockActivityOpts<T> = {},
): Promise<T>;                          // gate: if (!gitActivityActive(repoId)) return fn();
```

### 5.3 Handlers

- `handlers/activity.ts`: `gitActivitySubscribe: async (repoId, onEvent) =>
  subscribeGitActivity(repoId, onEvent)`, plus `gitActivityUnsubscribe: async (id) =>
  unsubscribeGitActivity(id)`.
- The 14 handler files with `runMockActivity(` (bisectHistory, branches, compose, merge, rebase,
  remotesSync, stash, repo, resetRevert, submodules, tagSync, repoMeta, status, worktrees; about
  65 sites) prepend `repoId,`. Each handler already takes `repoId` as its first arg.
  `handlers/repo.ts` (clone/init) passes `null`. The largest file, `branches.ts`, is 398, so there
  is no 500-line risk.

## 6. File map + budgets

| File | Now | After (≤) | Change |
|---|---|---|---|
| `crates/bonsai-core/src/git/activity.rs` | 429 | 450 | `repo_id` field on event + emitter; `with_subject` param |
| `crates/bonsai-core/src/git/activity_repo_tests.rs` (new, `#[path]` from `activity.rs`) | — | 120 | §7 R1-R2 |
| `crates/bonsai-core/src/git/activity_tests.rs` | 426 | 430 | `with_subject` call |
| `src-tauri/src/activity_hub.rs` (new) | — | 150 | hub + `ScopedActivityHub` |
| `src-tauri/src/activity_hub_tests.rs` (new) | — | 220 | §7 R3-R8 |
| `src-tauri/src/state.rs` | 116 | 100 | hub removed; `git_activity_for` / `_unscoped` |
| `src-tauri/src/lib.rs` | 434 | 436 | `mod activity_hub;` + handler |
| `src-tauri/src/commands/activity.rs` | 344 | 400 | commands, scoped brackets |
| `src-tauri/src/commands/{merge,remotes,staging,repo}.rs` | — | net 0 | §3.4 |
| `src-tauri/src/commands/mod.rs` | 204 | 205 | export |
| `src-tauri/src/commands/tests_activity_support.rs` | 106 | 125 | `subscribe_for` |
| `src-tauri/src/commands/tests_activity_scope.rs` (new) | — | 200 | §7 R9-R12 |
| `src-tauri/src/obs/metrics_cmds.rs` | — | +1 | allow-list |
| `src/ipc/types/activity.ts` | 140 | 150 | `repoId`, `GitActivityListener` |
| `src/ipc/types/ipc-api.ts` | 760 | **759** | baselined; §4.2 |
| `src/ipc/tauri/activity.ts` | 18 | 25 | |
| `src/components/repoWorkspace/useGitActivitySubscription.ts` (new) | — | 60 | §4.4 |
| `src/components/repoWorkspace/useGitActivity.ts` | 260 | 280 | |
| `src/components/repoWorkspace/useGitDock.ts` | — | +1 | |
| `src/components/RepoWorkspace.tsx` | 2261 | **2261** | same-line edit |
| `src/ipc/mock/gitActivityHub.ts` (new) | — | 70 | §5.1 |
| `src/ipc/mock/gitActivity.ts` | 471 | 465 | |
| `src/ipc/mock/handlers/*.ts` (14 + `activity.ts`) | — | +0-2 each | |
| New vitest files (§7) | — | ≤250 each | |

Existing tests needing mechanical updates: `useGitActivity.test.tsx` and
`useGitActivity.p119.test.tsx` (`useGitActivity(REPO)`, `emitGitActivity(REPO, ev)`), plus the
mock suites `gitActivityTarget`, `gitActivityPlain`, `gitActivitySeams`,
`handlers/activityCoverage`, and `handlers/amendActivity` (new subscribe/emit/`runMockActivity`
signatures). The existing assertions are otherwise unchanged.

## 7. Tests

**Rust**
- R1 (core): `started` from `with_subject(.., Some("repo-a"), ..)` serialises `"repoId":"repo-a"`.
  With `None`, the key is absent.
- R2 (core): drive every kind (`phase`, both line streams, `hookDone`, `progress`, the truncation
  marker, the reason line, `finished`). No non-started event JSON has a `repoId` key.
- R3 (hub): `subscribe` returns distinct ids. `subscriber_count` tracks sub/unsub.
- R4 (hub): with subs A1, A2, B, `emit_to(Some("a"))` reaches A1 and A2 once each, and B gets 0.
  `emit_to(None)` reaches all 3. A wildcard (`None`) sub receives both.
- R5 (hub): `is_active_for(Some("b"))` is false with only A subscribed. `is_active_for(None)` is
  true if any sub exists.
- R6 (hub): `unsubscribe` removes exactly one sub, and later emits don't reach it. A second call,
  or an unknown id, returns `false` and changes nothing.
- R7 (hub): StrictMode sequence `sub(A)=s1, sub(A)=s2, unsub(s1)` → count 1, and an emit delivers
  once.
- R8 (hub): a channel whose closure returns `Err` is pruned on emit (existing safety net, kept).
- R9 (bracket): repo A and B each subscribed via `subscribe_for`. `fetch_inner`/`logged_blocking`
  on A logs one run in A's log with `started.repoId == A`, and B's log is empty.
- R10 (bracket): only B subscribed, op on A → `run` sees `em == None` (passthrough), and B's log is
  empty.
- R11 (clone/init): `init_repo_inner` with A and B subscribed → one run in **both** logs, and
  `started` has no `repoId` (D3).
- R12 (command surface): `git_activity_unsubscribe` with an unknown id is a no-op (no panic).
- Existing: `tests_metrics_cmds` drift test passes with 201 entries. All
  `tests_activity_coverage*` suites stay green through the wildcard `subscribe`.

**Vitest**
- V1 `useGitActivitySubscription.test.tsx`: under a `<StrictMode>` wrapper, after settle,
  `gitActivitySubscriberCount() === 1`. Unmount → 0.
- V2: subscribe held pending (spy returns a deferred), unmount, then resolve → unsubscribe called
  with that id; count 0; a later emit does not call `onEvent`.
- V3: after unmount, an event emitted before the unsubscribe lands is not delivered (`disposed`
  guard).
- V4 `useGitActivity.scope.test.tsx`: two hooks (A, B). A run emitted to route A shows only in A's
  `runs`. `onEvent` for B is never invoked (spy).
- V5 (defensive path only; build the event by hand with `emitGitActivity`, because
  `runMockActivity` never produces it): a broadcast (`route null`) `started` carrying `repoId: 'B'`
  is dropped by A's hook and kept by B's hook. A broadcast with no `repoId` is kept by both.
- V6 `gitActivityHub.test.ts`: routing, idempotent unsubscribe, and `gitActivityActive(route)`, as
  mirrors of R4-R6.
- V7 mock: `runMockActivity('repo-a', …)` → `started.repoId === 'repo-a'`, with no `repoId` on
  other kinds. `cloneRepo` / `initRepo` emit with no `repoId`. With only B subscribed, a run on A is
  a passthrough (no events).

## 8. Flags for the orchestrator / user

- **Q1 (user):** runs with no repo (clone, init) have three options.
  - (a) **Broadcast to every open tab** (recommended). This is today's visible behaviour, and the
    rows are rare and cheap.
  - (b) Route to the focused tab (`AppState.active_repo` at start). Tidier, but it couples to a
    frontend-set value, and switching tabs mid-clone leaves the row in the old tab.
  - (c) Drop them. This is the strictest reading of "only its own repo". Not recommended: a clone
    started from the app would then leave no trace anywhere.

  The contract implements (a). Switching to (b) changes only `git_activity_unscoped` and the mock's
  `null` in `handlers/repo.ts`.
- Size constraints: `ipc-api.ts` (baseline 760) must net ≤ 0 lines (§4.2). `repo.rs` is at 490
  (net 0). `RepoWorkspace.tsx` gets a same-line edit, and `App.tsx` is untouched.
- **Security triggers: none.** No file touched is under `crates/bonsai-mcp/src/server/tools_*.rs`,
  `crates/bonsai-core/src/tools/*.rs`, `commands/external.rs`, or `commands/tools.rs`. `repoId` on
  the wire is the canonical workdir path that every repo-scoped command already carries, so it is
  not a new exposure. The metrics allow-list gains one exact name, and the audit-F3 membership rule
  still holds.

## 9. Acceptance criteria

1. With N tabs open, one op in repo A produces exactly **one** channel message per event, delivered
   to A's subscription only (R4, R9, V4). Hidden tabs of other repos run no `onEvent`.
2. `started.repoId` equals the run's repoId for every repo-scoped op, and is absent for clone/init.
   No other kind carries it (R1, R2, R11, V7).
3. The live subscription count equals the number of mounted `RepoWorkspace`s, including under
   StrictMode and unmount-before-resolve (R7, V1, V2). Closing a tab drops its subscription.
4. With no subscriber for repo A, an op on A takes the buffered passthrough (R10).
5. Mock parity: the browser harness (`VITE_MOCK_IPC=1`) shows a run only in the tab of its repo.
   Clone/init show in every tab.
6. `pnpm lint:size` passes with no baseline growth; `cargo test`, `tsc`, and vitest are green.
7. USER CHECKPOINT: in `pnpm tauri dev` with 2+ repos, a push in tab A does not appear in tab B's
   dock, and closing tab A leaves no stray activity in the dev log.

## 10. Sub-increments

- **P123a** (one senior-dev pass; the pieces are coupled by signatures): §2, §3, §4, §5 together.
  Splitting Rust from TS would leave `ipc-api.ts` and the mock uncompilable in between.
- **P123b** (tester): §7 tests not written in P123a, plus the smoke check for AC5.
