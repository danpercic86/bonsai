# P30 — Background-Job Scheduler (Theme B, item B5)

Status: contract. Rust-side scheduler for strictly NON-DESTRUCTIVE background jobs:
(a) auto-fetch per open repo, (b) periodic read-only refresh signal (health/status).
**Hard invariants:** no auto-pull/push/prune/gc; nothing mutates worktree or refs beyond
what `fetch_all` inherently updates (remote-tracking refs). Jobs are suppressed while
`read_op_state != None`, never overlap their own previous run, and NEVER prompt for
credentials.

Precedent/style: P29-repo-health.md, P27-worktrees.md. This contract **subsumes** the
existing frontend auto-fetch timer (P11e, `RepoWorkspace.tsx:970–988`) — see §8 migration.

---

## 1. Decisions (accept-defaults mode — all resolved here)

| # | Decision | Choice + rationale |
|---|----------|--------------------|
| D1 | Scheduler location | **Rust**, `src-tauri/src/scheduler.rs`, a `SchedulerState` in `.manage()`. Rust owns all Git logic; a frontend timer dies with the webview, can't read opstate cheaply, and fires per-tab (the current P11e timer only fetches the ACTIVE tab — background repos never fetch; this fixes that). |
| D2 | One task vs per-repo tasks | **ONE global tokio task** (spawned in `lib.rs` setup via `tauri::async_runtime::spawn`) with a coarse tick loop (`TICK_SECONDS = 15`). Each tick it snapshots `AppState.repos` keys and evaluates due-ness per (repo, job). Rationale: zero task-lifecycle code on open/close (the repo map IS the membership source of truth — a closed repo simply stops appearing), one cancellation point (task ends with the runtime on app exit), trivial to test. Per-repo tasks would need spawn/abort wiring inside `open_repo`/`close_repo` and buy nothing at N ≤ a handful of tabs. |
| D3 | Job execution | Tick loop never blocks: when a job is due it flips `running = true` and `tauri::async_runtime::spawn`s the job future (which uses the existing command-inner pattern — `spawn_blocking(move || fetch_all(&path))`). Completion updates status + emits the event. |
| D4 | No-overlap guard | `running: bool` per (repo, job) in `SchedulerState`. If still true when next due, the tick records outcome `skipped` (no failure counted) and waits. |
| D5 | Suppression | Immediately before executing autoFetch, the job future calls `read_op_state(&path)` (inside its own `spawn_blocking`). If `!= RepoOpState::None` → outcome `suppressed`, no fetch, no failure counted, `nextRun = now + interval` (normal reschedule). Same check for healthRefresh (cheap, consistent). |
| D6 | Backoff | Per (repo, job): `consecutiveFailures` increments on `failed` only. Effective interval = `base` for failures 0–2; `base * 2^(failures-2)` for ≥ 3, capped at `8 * base`. Reset to 0 on any `success`. `inBackoff = failures >= 3`. The event carries `enteredBackoff: true` exactly on the 2→3 transition (frontend toasts ONLY then). |
| D7 | Config storage | **GLOBAL, in the existing `settings.json`** — reuse `Settings.auto_fetch` (`AutoFetch { enabled, interval_minutes }`) verbatim as the autoFetch job config; ADD sibling `Settings.health_refresh: HealthRefresh { enabled, interval_minutes }`. Rationale: autoFetch is already global there with clamping, patch plumbing, Settings UI, and mock round-trip; per-repo config would need a new persistence surface for marginal v1 value. Per-repo overrides are a documented future extension (add `Option<JobsOverride>` keyed by repoId later). NO new config commands: config flows through the existing `get_ui_settings`/`set_ui_settings` patch; `set_ui_settings_inner`, after persisting, calls `scheduler::apply_config(&scheduler_state, cfg)`. Startup: `lib.rs` setup loads settings and applies before spawning the loop. |
| D8 | Events | New event `job-status-changed` (compact payload §4). Additionally, a successful autoFetch with `updatedRefs > 0` emits the EXISTING `repo-changed` event with the exact existing `RepoChangedPayload` shape (as emitted at `commands.rs:110`) so the whole existing refresh pipeline (status, graph, health panel per P29 D11) reacts unchanged. healthRefresh job = pure signal: it does NO git work in Rust; it emits `repo-changed` on its interval and lets the frontend's existing on-event refetch logic recompute status/health (read-only by construction). |
| D9 | Credentials | autoFetch reuses `fetch_all` (M6 chain: git CredentialHelper → ssh agent). That chain never prompts interactively — its callbacks return `Err` when no credential is available, surfacing `AppError::AuthFailed`/`Git`. The scheduler treats that as an ordinary `failed` outcome into the backoff path. **The scheduler must not add any prompting, credential storage, or UI dialog. A background failure is silent except for the single backoff-entry toast.** |
| D10 | Manual "run now" | `run_job_now(repoId, job)` command: fire-and-forget (`Ok(())` immediately if the repo exists and the job isn't already running, else `AppError::Other("job already running")` / `NoRepo`); result arrives via `job-status-changed`. Suppression + backoff-reset rules apply as for a scheduled run, except run-now IGNORES backoff delay (it runs immediately) — a successful run-now clears backoff. |
| D11 | Status surface (UI) | `getJobStatus(repoId)` on workspace mount + live updates via `job-status-changed` listener. Rendered as a small muted "Fetched Xm ago" (or "Auto-fetch paused — retrying in Xm" when in backoff) next to the existing Fetch control in the repo toolbar in `RepoWorkspace.tsx`. No new panel. |
| D12 | Interval clamps | autoFetch: existing `AUTO_FETCH_INTERVAL_MIN/MAX` (1..120 min, default 5, disabled). healthRefresh: `HEALTH_REFRESH_INTERVAL_MIN = 1`, `MAX = 240`, default `30`, disabled by default. `clamp_health_refresh` mirrors `clamp_auto_fetch` (settings.rs:146). |
| D13 | First run after enable/open | `lastRunMs = None` ⇒ job is due at `now + interval` (NOT immediately) — opening a repo doesn't trigger an instant network fetch storm; users who want immediate use Fetch/run-now. |
| D14 | Time source | Planner is pure over `now_ms: i64` (unix ms). The loop passes real time; tests pass fake time. The tick period itself is a constructor param of the loop (`run_scheduler(app, tick: Duration)`) so the integration test can run a 250 ms tick with second-scale intervals (intervals internally stored in ms: `interval_ms = interval_minutes * 60_000`, but the planner takes ms so tests can inject arbitrary values). |
| D15 | Sub-increments | Two passes P30a/P30b (§9). |

---

## 2. Module boundaries

| File | Responsibility |
|------|----------------|
| `src-tauri/src/scheduler.rs` (NEW) | `SchedulerState`, pure planner (`plan`, `effective_interval_ms`), the tick loop `run_scheduler`, job execution futures, event emission, `apply_config`, all consts, unit tests for the planner state machine. Uses only existing core fns (`fetch_all`, `read_op_state`) — **no new git logic**. |
| `src-tauri/src/lib.rs` | `.manage(SchedulerState::default())`; in `setup`: load settings → `apply_config` → `tauri::async_runtime::spawn(run_scheduler(handle, Duration::from_secs(TICK_SECONDS)))`. Register new commands. |
| `src-tauri/src/settings.rs` | `HealthRefresh` struct + default + clamps; add `health_refresh` to `Settings`, `Default`, load-clamp (additive/backward-compatible like P11 autoFetch). |
| `src-tauri/src/commands.rs` | `get_job_status`, `run_job_now` (+ `_inner`s); extend `UiSettings`/`UiSettingsPatch` with `health_refresh`; `set_ui_settings` pushes config into `SchedulerState`. |
| `src/ipc/types.ts` | §5 TS mirrors; `getJobStatus`, `runJobNow` on the IPC interface; `HealthRefreshSettings` on `UiSettings`/patch. |
| `src/ipc/tauri.ts` | `invoke('get_job_status', { repoId })`, `invoke('run_job_now', { repoId, job })`. |
| `src/ipc/mock.ts` | §7 mock ticks + status + config round-trip (must keep compiling — mandatory harness). |
| `src/components/SettingsPanel.tsx` | EXTEND the existing "Auto-fetch" section (checkbox + interval, lines ~226–244): retitle section "Background jobs", add the healthRefresh checkbox + interval below with identical binding pattern. No duplicate autoFetch UI. |
| `src/components/RepoWorkspace.tsx` | DELETE the P11e frontend interval (lines 964–988). Add job-status readout (D11) + `job-status-changed` subscription + backoff toast. |
| `src/App.tsx` | No structural change; keeps passing `autoFetch` (and now `healthRefresh`) settings down for the Settings UI only — the timer behavior moves to Rust. |

---

## 3. Rust interfaces (implement verbatim)

```rust
// src-tauri/src/scheduler.rs
pub const TICK_SECONDS: u64 = 15;
pub const BACKOFF_THRESHOLD: u32 = 3;      // failures at which backoff starts
pub const BACKOFF_MAX_FACTOR: i64 = 8;     // cap: 8 * base interval

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobKind { AutoFetch, HealthRefresh }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum JobOutcome { Success, Failed, Suppressed, Skipped } // skipped = overlap guard

/// Per-(repo, job) runtime record. All timestamps unix ms.
#[derive(Debug, Clone, Default)]
pub struct JobRuntime {
    pub running: bool,
    pub last_run_ms: Option<i64>,
    pub last_outcome: Option<JobOutcome>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
}

/// Global scheduler config snapshot (mirrors Settings fields, D7).
#[derive(Debug, Clone, Copy, Default)]
pub struct JobsConfig {
    pub auto_fetch: crate::settings::AutoFetch,
    pub health_refresh: crate::settings::HealthRefresh,
}

#[derive(Default)]
pub struct SchedulerState {
    pub cfg: std::sync::Mutex<JobsConfig>,
    /// keyed by (repoId, JobKind); entries pruned when repo no longer in AppState.
    pub jobs: std::sync::Mutex<std::collections::HashMap<(String, JobKind), JobRuntime>>,
}

pub fn apply_config(state: &SchedulerState, cfg: JobsConfig);

// ---- pure planner (unit-testable, no IO, no Tauri types) ----
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanDecision { Run, SkipOverlap, Wait { next_run_ms: i64 } }

/// base * 2^(failures - (BACKOFF_THRESHOLD - 1)) for failures >= BACKOFF_THRESHOLD,
/// else base; capped at BACKOFF_MAX_FACTOR * base.
pub fn effective_interval_ms(base_ms: i64, consecutive_failures: u32) -> i64;

/// Due when last_run_ms is None => first due at (enabled_since… simplification: treat
/// None as "ran at now" on the tick a job is first seen — D13) or
/// now >= last_run + effective_interval. Disabled => Wait{ i64::MAX }.
pub fn plan(enabled: bool, base_interval_ms: i64, now_ms: i64,
            last_run_ms: Option<i64>, running: bool,
            consecutive_failures: u32) -> PlanDecision;

pub fn next_run_estimate_ms(enabled: bool, base_interval_ms: i64,
                            last_run_ms: Option<i64>, consecutive_failures: u32) -> Option<i64>;

/// The loop. Never returns; ends with the runtime. `tick` injectable for tests (D14).
pub async fn run_scheduler(app: tauri::AppHandle, tick: std::time::Duration);
```

Job execution (inside `run_scheduler`'s spawned futures — signatures for clarity):
- autoFetch: `spawn_blocking(read_op_state)` → if `!= None` record `Suppressed`; else
  `spawn_blocking(fetch_all)` → `Success` (emit `repo-changed` iff sum(updatedRefs) > 0)
  or `Failed` with `AppError.to_string()` in `last_error`. Always emit `job-status-changed`.
- healthRefresh: opstate check as above; on pass, record `Success` and emit `repo-changed`
  (existing payload) + `job-status-changed`. No git work.

Commands (`commands.rs`, existing `_inner` pattern):

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub job: JobKind,
    pub enabled: bool,
    pub last_run_ms: Option<i64>,
    pub last_outcome: Option<JobOutcome>,
    pub last_error: Option<String>,
    pub consecutive_failures: u32,
    pub in_backoff: bool,
    pub next_run_ms: Option<i64>,   // estimate; None when disabled
}

#[tauri::command] pub async fn get_job_status(state: tauri::State<'_, AppState>,
    sched: tauri::State<'_, SchedulerState>, repo_id: String)
    -> Result<Vec<JobStatus>, AppError>;   // exactly 2 entries; NoRepo if unknown repoId

#[tauri::command] pub async fn run_job_now(app: tauri::AppHandle,
    state: tauri::State<'_, AppState>, sched: tauri::State<'_, SchedulerState>,
    repo_id: String, job: JobKind) -> Result<(), AppError>;  // D10
```

`settings.rs` additions:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthRefresh { pub enabled: bool, pub interval_minutes: u32 }
// Default { enabled: false, interval_minutes: 30 }
pub const HEALTH_REFRESH_INTERVAL_MIN: u32 = 1;
pub const HEALTH_REFRESH_INTERVAL_MAX: u32 = 240;
pub fn clamp_health_refresh(h: HealthRefresh) -> HealthRefresh;
// Settings: pub health_refresh: HealthRefresh (+ Default, + clamp on load — P11 pattern)
```

---

## 4. Event surface

Existing (reused, unchanged shape): `repo-changed` with the exact `RepoChangedPayload`
already emitted by `open_repo`/watcher (`commands.rs:110`).

New event `job-status-changed` (small push signal — command/event/channel invariant holds;
no channel needed, payloads are tiny):

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatusChangedPayload {
    pub repo_id: String,
    pub job: JobKind,
    pub outcome: JobOutcome,
    pub updated_refs: Option<u32>,      // autoFetch success only
    pub error: Option<String>,          // failed only
    pub consecutive_failures: u32,
    pub in_backoff: bool,
    pub entered_backoff: bool,          // true exactly on the 2→3 transition (D6)
    pub ts_ms: i64,
    pub next_run_ms: Option<i64>,
}
```

---

## 5. TypeScript mirrors (`src/ipc/types.ts`)

```ts
export type JobKind = 'autoFetch' | 'healthRefresh';
export type JobOutcome = 'success' | 'failed' | 'suppressed' | 'skipped';
export interface JobStatus {
  job: JobKind; enabled: boolean;
  lastRunMs: number | null; lastOutcome: JobOutcome | null; lastError: string | null;
  consecutiveFailures: number; inBackoff: boolean; nextRunMs: number | null;
}
export interface JobStatusChangedPayload {
  repoId: string; job: JobKind; outcome: JobOutcome;
  updatedRefs?: number; error?: string;
  consecutiveFailures: number; inBackoff: boolean; enteredBackoff: boolean;
  tsMs: number; nextRunMs: number | null;
}
export interface HealthRefreshSettings { enabled: boolean; intervalMinutes: number; }
// UiSettings += healthRefresh: HealthRefreshSettings
// UiSettingsPatch += healthRefresh?: HealthRefreshSettings
// IPC interface += getJobStatus(repoId: string): Promise<JobStatus[]>
//                  runJobNow(repoId: string, job: JobKind): Promise<void>
```

Frontend subscription: same `listen()` pattern used for `repo-changed` in
`RepoWorkspace.tsx` — filter on `payload.repoId === repoId`, update local `JobStatus`
state; if `enteredBackoff`, `pushToast('warning', 'Auto-fetch failing — backing off')`.
No toast for individual background failures (replaces the current per-failure toast at
`RepoWorkspace.tsx:983`, which is deleted with the timer).

---

## 6. Migration of the P11e frontend timer (§ CRITICAL)

1. DELETE the `useEffect` interval at `RepoWorkspace.tsx:964–988` (P30b).
2. The "Fetched N refs" quiet info toast + `refreshAllRef` refresh move to the
   `job-status-changed` handler (`outcome === 'success' && updatedRefs > 0`); the graph/
   status refresh itself also arrives via the emitted `repo-changed` — the handler must
   NOT double-refresh (rely on `repo-changed` for data, use `job-status-changed` only for
   toast + status readout).
3. Settings UI and persistence for `autoFetch` are UNCHANGED (same struct, same patch
   path); behavior change: auto-fetch now runs for ALL open repos, not only the active
   tab, and while the settings window is anywhere. Document in the Settings help text.

---

## 7. Mock behavior (`src/ipc/mock.ts`, VITE_MOCK_IPC=1)

- Config: `healthRefresh` joins the existing localStorage-backed `UiSettings`
  round-trip with the same clamp pattern as `clampAutoFetch` (mock.ts:1320).
- Status: mock keeps an in-memory `Map<string, JobStatus[]>`; `getJobStatus` returns it
  (seeded: autoFetch success 2 min ago, healthRefresh disabled).
- Ticks: when `autoFetch.enabled` (resp. healthRefresh), the mock runs a
  `setInterval` treating **intervalMinutes as SECONDS** (documented test-speed shim) that
  updates the status and dispatches `job-status-changed` then `repo-changed` through the
  mock's existing listener registry. `runJobNow` fires the same synthetic completion
  immediately. One mock repo (fixture) simulates failure escalation when localStorage key
  `bonsaiMockJobFail=1` is set, to exercise the backoff toast in the harness.

---

## 8. Error taxonomy

Reuse existing `AppError` kinds only: `NoRepo` (unknown repoId), `Other("job already
running")` for run-now overlap. Job-internal errors (`Git`, `AuthFailed`, `Io`) are NOT
returned to the frontend — they land in `lastError`/event `error` as strings via the
existing `to_string()` convention. **No new git logic; `fetch_all` and `read_op_state`
are called as-is.**

---

## 9. Sub-increments

- **P30a — Rust core.** `scheduler.rs` (planner + loop + state), settings additions,
  `get_job_status`/`run_job_now` commands + registration, `set_ui_settings` →
  `apply_config` hook. Tests: planner unit tests (due/wait/first-run D13, overlap skip,
  suppression outcome bookkeeping, backoff growth 1×/2×/4×/8× cap, reset on success,
  entered-backoff transition, disabled ⇒ never due); integration test with
  `run_scheduler(handle-less harness or inner tick fn)` — extract the per-tick body as
  `async fn tick_once(...)` callable without a real AppHandle event sink (inject an
  `impl Fn(Event)` emitter) so tests drive ticks with fake `now_ms`; plus one real-time
  test with second-scale intervals: scratch repo + local bare `file://` remote under
  `D:\Temp\bonsai-scratch`, commit pushed to bare from a second clone, scheduler tick →
  remote-tracking ref updated; repeat with a staged merge-conflict opstate → `suppressed`,
  ref NOT updated; slow-fetch overlap simulated via a long-running flag → `skipped`.
- **P30b — IPC + UI + mock.** types.ts/tauri.ts/mock.ts (§5, §7), SettingsPanel
  "Background jobs" section, RepoWorkspace timer removal + status readout + subscriptions
  + backoff toast, App.tsx healthRefresh plumb-through.

---

## 10. Acceptance criteria

**AI gate**
- `cargo test` green (run sequentially with clippy per repo memory; `TMP`/`TEMP` =
  `D:\Temp`; scratch repos only under `D:\Temp\bonsai-scratch`): planner state-machine
  unit tests (§9 list) + integration tests (fetch updates remote-tracking refs on a local
  bare `file://` remote; suppressed during merge-conflict opstate; no overlap under slow
  fetch; failure → backoff after 3, event `enteredBackoff` once).
- `cargo clippy` clean; `pnpm tsc`/`pnpm build` clean; `mock.ts` compiles.
- Browser harness (`pnpm dev` + `VITE_MOCK_IPC=1`): Settings shows Background-jobs
  section, autoFetch + healthRefresh round-trip through reload; enabling autoFetch fires
  mock ticks → status readout updates and `repo-changed` refresh runs; failure shim
  produces exactly one backoff toast.
- Code review confirms: no new git primitives, no credential prompting path, frontend
  timer deleted, `repo-changed` payload unchanged.

**USER CHECKPOINT** (native `pnpm tauri dev` — orchestrator must not self-declare)
- Real background fetch against a network remote using the configured credential helper;
  refs update with NO credential prompt and no prompt storms on failure.
- Backoff behaves sanely when the network is disconnected (one toast, quiet retries).
- CPU/battery sanity: idle app with jobs enabled shows no measurable busy-loop.

---

## 11. Flag for orchestrator

- D7 (global vs per-repo config) deviates from the task's literal "per-repo job config
  commands": recommended GLOBAL via existing settings for v1 (justified in D7, per-repo
  is a documented additive extension). Confirm before P30a if per-repo is a hard
  requirement.
- Behavior change (D1/§6.3): auto-fetch now covers all open tabs, not just the active
  one. Intentional; mention at USER CHECKPOINT.
