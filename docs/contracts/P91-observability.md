# P91 — Observability: Dev mode, structured logs, local telemetry & metrics

**Goal (drives every choice):** produce a JSONL file the user can hand to an AI reviewer that makes
**double triggers, redundant IPC, effects firing on unchanged deps, unintended side effects and
never-meant-to-happen actions** *mechanically visible* — via correlation ids + machine-emitted
anomaly records, not via prose log lines.

**All §13 decisions are RESOLVED (user, 2026-08-27).** §13 is a decision record, not a question
list. Nothing in this contract is pending an answer.

**Two distinct systems, do not conflate:**

| | Logs | Metrics |
|---|---|---|
| Gate | Dev mode ON only | always on |
| Volume | verbose, per-event | tiny, aggregated |
| Lifetime | rotating session files, prunable, user-deletable | durable, retained (future Statistics page) |
| Sink | `logs/*.jsonl` | `metrics/usage.json` |
| Network egress | **none, ever** | **none, ever** |

---

## 1. Module map (each file its own concern, ~500-line cap)

### Rust — `src-tauri/src/obs/`
| File | Responsibility |
|---|---|
| `obs/mod.rs` | re-exports; `ObsState` held in `AppState` |
| `obs/record.rs` | `LogRecord` + all payload enums; schema version constant |
| `obs/redact.rs` | session salt, `Redactor` (stable-within-session id assignment), token scrubber |
| `obs/sink.rs` | bounded MPSC → writer thread; `try_send`, drop counter, flush; `RollAndPurge` control message |
| `obs/writer.rs` | file naming, JSONL append, rotation, pruning, header record, **purge** (§6.1) |
| `obs/anomaly.rs` | streaming detectors (§5) over the unified record stream |
| `obs/trace.rs` | `TraceId` type, minting, `TraceMeta`, `emit_logged` event helper |
| `obs/invoke_shim.rs` | `invoke_handler` wrapper logging every command dispatch |
| `obs/metrics.rs` | `MetricsStore` (counters/histograms), aggregation cadence |
| `obs/metrics_file.rs` | atomic load/save of `metrics/usage.json`, daily buckets, retention |
| `commands/obs.rs` | `log_append`, `log_session_info`, `log_reveal_dir`, `log_export_session`, `logs_delete_all`, `metrics_snapshot`, `metrics_reset` |

`perf.rs` is **kept** as the hot-path atomic tally and is *absorbed*: `MetricsStore` reads
`PerfState::snapshot()` at each flush and folds the delta into durable counters. No parallel counters.

### Frontend — `src/obs/`
| File | Responsibility |
|---|---|
| `obs/types.ts` | TS mirror of `LogRecord` + `TraceId` |
| `obs/enabled.ts` | `obsEnabled()` / `obsLevel()` — single boolean read, set once at boot + on settings change |
| `obs/trace.ts` | `newTrace()`, `withTrace()`, `currentTrace()` (sync ambient), `bindTrace()` |
| `obs/redact.ts` | frontend mirror of `Redactor` (same scheme, same session salt, fetched at boot) |
| `obs/log.ts` | `logRecord(r)` — no-op fast path when disabled; enqueues to batcher |
| `obs/batcher.ts` | ring buffer, flush on 500 ms / 100 records / `visibilitychange` / `beforeunload` |
| `obs/ipcProxy.ts` | `instrumentIpc(api: IpcApi): IpcApi` — the frontend choke point |
| `obs/react.ts` | `useRenderCount()`, `useTracedEffect()`, `useStateTransitionLog()` (dev-mode-only) |
| `obs/renderTally.ts` | aggregate-mode render accumulator (§9.2) — one record per window, not per render |
| `src/components/settings/catalog/dev.ts` | Dev-mode catalog rows |
| `src/components/settings/categories/DevPage.tsx` | Dev-mode page (ui-designer owns visuals) |

---

## 2. Trace model (the centerpiece)

### 2.1 Types

```ts
// src/obs/types.ts
export type TraceId = string;   // 12-char base36, monotonic-prefixed: `${t36}-${rand4}`
export type SpanId  = string;   // 6-char base36, unique within a trace

export type TraceOrigin =
  | 'click' | 'keybinding' | 'palette' | 'menu' | 'route'
  | 'timer' | 'watcher' | 'forge-poll' | 'boot' | 'backend';

export interface TraceRoot {
  trace: TraceId;
  origin: TraceOrigin;
  /** Stable label of the gesture, NOT free text: 'commit.submit', 'sidebar.branch.checkout'. */
  gesture: string;
  startedAt: number;      // epoch ms
}
```

```rust
// src-tauri/src/obs/trace.rs
pub type TraceId = String;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceMeta {
    pub trace: TraceId,
    /// Set when this backend work was CAUSED by a prior trace (event fan-out,
    /// a scheduler job seeded by a user action).
    pub caused_by: Option<TraceId>,
}

impl TraceMeta {
    pub fn root(origin: &str) -> Self;          // mints a backend-origin trace
    pub fn child_of(parent: &TraceId) -> Self;  // new trace, caused_by = parent
}
```

### 2.2 Crossing the IPC boundary — **DECIDED: extra arg field, not a wrapper**

The frontend proxy injects two reserved keys into the args object of every `invoke`:

```
__trace: TraceId        __span: SpanId
```

Tauri v2 deserializes command parameters key-by-key from the payload map, so **unknown keys are
ignored** by existing commands — zero signature churn. Rule: **no command signature changes in P91.**

**There is deliberately no ambient backend trace.** The shim reads `__trace` *synchronously, before
delegating*, and uses it only to stamp its own `ipc.recv` record. It does **not** stash a
task-local: the shim closure is synchronous, the generated handler spawns the command future
outside its scope, and heavy work then runs on `spawn_blocking` — a task-local would be empty
exactly where it is needed. Any Rust site that wants causality passes a `TraceMeta` **explicitly**.
Per-command traces and durations come from the frontend `ipc.call`/`ipc.result` pair, which is
where the user-visible latency actually lives.

### 2.3 Rust dispatch choke point — **DECIDED: ship the shim**

```rust
// obs/invoke_shim.rs
pub fn instrumented_handler<R: tauri::Runtime>(
    inner: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static;
```
Wraps the closure returned by `generate_handler![...]` in `lib.rs`. Reads
`invoke.message.command()` and the payload map (non-consuming) to extract `__trace`/`__span`,
emits `ipc.recv`, then delegates.

**Known limit:** the shim cannot observe *completion* (the resolver is consumed downstream), so
**command duration is measured on the frontend side**. Backend-internal timings come from explicit
spans in heavy commands (`obs::span!("graph.walk")`), added only where `perf.rs` already
instruments.

Commands excluded from instrumentation by name (would self-amplify):
`log_append`, `log_session_info`, `logs_delete_all`, `metrics_snapshot`, `debug_perf_counters`.

#### 2.3.1 Contingency — executing the fallback without re-deriving it

`tauri::ipc::Invoke` is documented upstream as "explicitly NOT stable". **If a Tauri upgrade breaks
`instrumented_handler`, delete `obs/invoke_shim.rs` and its `lib.rs` wiring. Do not attempt to
repair it and do not add `TraceMeta` parameters to command signatures.**

**Exactly what is lost:** only the `ipc.recv` record — i.e. the *arrival stamp on the Rust side*
(proof that a command actually reached the backend, and the Rust-side receive timestamp). Retained
in full: per-command traces, spans, args hashes, durations and outcomes (frontend
`ipc.call`/`ipc.result`), all event/channel/watcher records (`emit_logged` takes its `TraceMeta`
explicitly and never depended on the shim), echo/double-trigger causality (§2.4, frontend), and
every anomaly rule except the ability to distinguish "never sent" from "sent but never answered"
— `orphan-trace` degrades from that distinction to a plain unanswered-call signal.
**No increment other than 3 is affected, and the milestone's core evidence (increment 4) is
untouched.**

### 2.4 Events, channels & watcher — causal ids

```rust
// obs/trace.rs
pub fn emit_logged<P: serde::Serialize + Clone>(
    app: &tauri::AppHandle, event: &str, payload: P, meta: &TraceMeta,
);
```
Every existing `app.emit(...)` call site in `src-tauri` is migrated to `emit_logged`. `TraceMeta`
is always **explicit** at the call site: a command that already threads a caller-supplied trace
passes `child_of(that)`; everywhere else `TraceMeta::root("backend")` — which is honest rather
than guessed.

`watcher.rs` logs raw notify batches and each debounce *firing* (`paths`, `relevant`,
`debounceMs`, `fired`) with a `root("watcher")` meta. It does **not** know about echo suppression.

**Echo/self-trigger causality is produced on the frontend**, where the knowledge lives:
`armEcho(repoId, trace)` records the arming mutation's `TraceId` in the echo registry. When
`useCoalescedRefresh` drops a `'watcher'`-origin refresh (`useCoalescedRefresh.ts:82`), it emits a
`watcher` record with `suppressed: true`, `suppressReason: 'echo'` and
`causedBy = <the arming mutation's trace>`. **This is the primary double-trigger evidence** and it
is delivered in increment 4, not increment 3.

**Channels are never logged per chunk.** One `ChannelOpen` and one `ChannelClose`
(`chunks`, `bytes`, `durationMs`, outcome).

### 2.5 Frontend ambient trace

JS has no async-context propagation. Mechanism is deliberately synchronous and honest about it:

```ts
// src/obs/trace.ts
export function withTrace<T>(origin: TraceOrigin, gesture: string, fn: () => T): T;
export function currentTrace(): TraceRoot | undefined;   // sync ambient, cleared on fn return
export function bindTrace<F extends (...a:any[])=>any>(fn: F): F; // captures now, restores on call
```
`withTrace` sets a module-level ambient for the *synchronous* extent of `fn`. The IPC proxy
captures `currentTrace()` at call time. Work resumed after an `await` must be re-entered with
`bindTrace` (used by the coalescer and channel callbacks). **Documented limitation:** unbound async
continuations log `trace: undefined` rather than a wrong trace — never guess.

`useCoalescedRefresh` extension: alongside `pendingScopesRef`, a `pendingTracesRef: Set<TraceId>`.
An executed round emits `RefreshRound { round, scope, origin[], contributingTraces[], collapsed }`.
A round with >1 contributing trace *is* the collapse evidence; a round with 1 contributing trace
that repeats the same scope with no intervening mutation is a `redundant-refresh` anomaly.

---

## 3. Record schema (v1)

```ts
export const OBS_SCHEMA_VERSION = 1;

export type LogLevel = 'error' | 'warn' | 'info' | 'debug' | 'trace';
export type LogSource = 'ui' | 'rust';

export interface LogRecordBase {
  /** monotonic per-session sequence, assigned by the SINK (ordering is authoritative). */
  seq: number;
  ts: number;                 // epoch ms, wall clock
  mono: number;               // ms since session start (jitter-free ordering aid)
  src: LogSource;
  lvl: LogLevel;
  trace?: TraceId;
  span?: SpanId;
  causedBy?: TraceId;
  kind: LogKind;
}

export type LogKind =
  | 'session'      | 'gesture'
  | 'ipc.call'     | 'ipc.result' | 'ipc.recv'
  | 'event'        | 'channel'
  | 'watcher'      | 'refresh'
  | 'render'       | 'render.tally' | 'effect' | 'state'
  | 'frame'        | 'error'      | 'anomaly'
  | 'drop';
```

Payload unions (each record is `LogRecordBase & { kind: K } & PayloadK`):

```ts
interface SessionPayload  { schema: number; app: string; os: string; sessionId: string;
                            devMode: true; level: LogLevel; redaction: RedactionMode;
                            /** Human-readable one-liner restating §7 for the reviewer. */
                            redactionNote: string;
                            /** True when this header opens a file created by a purge roll (§6.1). */
                            afterPurge?: boolean; }
interface GesturePayload  { origin: TraceOrigin; gesture: string; }
interface IpcCallPayload  { cmd: string; argsHash: string; argsShape?: ArgShape;
                            args?: Record<string, unknown>; } // only when redaction==='raw'
interface IpcResultPayload{ cmd: string; argsHash: string; ms: number;
                            outcome: 'ok'|'err'|'aborted'|'superseded';
                            errCode?: string; resultShape?: ArgShape; }
interface IpcRecvPayload  { cmd: string; }                       // rust-side dispatch stamp
interface EventPayload    { name: string; reason?: string; delivered: boolean; listeners: number; }
interface ChannelPayload  { name: string; phase: 'open'|'close'; chunks?: number;
                            bytes?: number; ms?: number; outcome?: string; }
interface WatcherPayload  { paths: number; relevant: number; debounceMs: number;
                            fired: boolean; suppressed: boolean; suppressReason?: string; }
interface RefreshPayload  { round: number; scope: string; origins: string[];
                            contributingTraces: TraceId[]; collapsed: number; ms: number; }
interface RenderPayload   { component: string; count: number; sinceMs: number;
                            changedProps?: string[]; }
/** §9.2 aggregate mode — ONE record per component per window, not per render. */
interface RenderTallyPayload { component: string; windowMs: number; renders: number;
                               instances: number;          // distinct mounted instances seen
                               changedProps: string[];     // union over the window
                               traces: TraceId[]; }        // traces active during the window
interface EffectPayload   { component: string; effect: string; run: number;
                            changedDeps: string[];         // [] ⇒ ran with no semantic change
                            depCount: number; }
interface StatePayload    { store: string; field: string; from: string; to: string; }
interface FramePayload    { paintMs: number; gapMs: number; over33: number; over100: number;
                            worstMs: number; }
interface ErrorPayload    { where: string; code?: string; message: string; stackHash?: string; }
interface AnomalyPayload  { rule: AnomalyRule; severity: 'info'|'warn'|'error';
                            detail: string; refs: number[];  // seq numbers of implicated records
                            traces: TraceId[]; }
interface DropPayload     { dropped: number; sinceSeq: number; } // sink backpressure
```

`ArgShape` = `Record<string, 'str'|'num'|'bool'|'null'|`arr:${number}`|`obj:${number}`>` — key
names + type + length only, **never values**.

Rust mirrors these as `#[serde(tag = "kind", rename_all = "camelCase")] enum LogPayload`.

---

## 4. Frontend choke point

```ts
// src/obs/ipcProxy.ts
export function instrumentIpc(api: IpcApi): IpcApi;
```
- Applied in `src/ipc/index.ts`: `export const ipc = instrumentIpc(await resolveIpc())` — one line,
  covers **both** `tauriIpc` and `mockIpc` (mock-parity satisfied by construction).
- Implemented as a `Proxy` whose `get` trap **re-checks `obsEnabled()` on every access**: when off
  it returns the original method identity (so toggling Dev mode takes effect with no reload and
  costs one boolean read when off); when on it returns a memoized wrapped method.
- For each call: mint `SpanId`, read `currentTrace()`, inject `__trace`/`__span` into the last
  object argument (or append `{__trace,__span}` when args are positional), emit `ipc.call`,
  `await`, emit `ipc.result` with `ms` + outcome.
- **Callback arguments are wrapped too** (`onRepoChanged(cb)`, channel `onChunk`): the wrapper
  emits `event`/`channel` records on *delivery* and re-binds the subscribing trace, so a
  subscription created by trace A shows deliveries as `causedBy: A`.
- `superseded`: the proxy keeps a per-`cmd` latest-span map; if a call resolves while a newer call
  of the same `cmd` is in flight, its result is stamped `superseded`.

---

## 5. Anomaly detection

Site-local rules are emitted by the site that has the knowledge. Cross-record rules run in
`obs/anomaly.rs` on the unified stream in the Rust sink (the only place that sees both sides).

| Rule id | Where | Window | Condition | Severity |
|---|---|---|---|---|
| `dup-ipc` | sink | 300 ms | same `cmd` + `argsHash`, ≥2 calls, no intervening mutation cmd | warn |
| `redundant-refresh` | sink | 1 s | same `scope`, ≥2 executed rounds, no mutation record between | warn |
| `effect-no-change` | site (`useTracedEffect`) | — | effect re-ran with `changedDeps.length === 0` | warn |
| `effect-thrash` | sink | 1 s | same `component.effect` ran ≥5× | warn |
| `render-storm` | sink | 1 s | a `render.tally` window reports `renders > 3 × instances` | warn |
| `event-storm` | sink | 1 s | same event `name` ≥20 deliveries | warn |
| `watcher-storm` | sink | 1 s | ≥5 debounce firings | warn |
| `jank-trace` † | sink | — | `frame.worstMs > 100` ⇒ attribute to traces with an open span at that ts | warn |
| `superseded-result` | proxy | — | `outcome: 'superseded'` | info |
| `orphan-trace` | sink | session end | trace with `ipc.call` and no `ipc.result` | error |
| `unbatched-sink` | sink | 1 s | ≥10 `log_append` calls | info |
| `drop` | sink | — | records dropped by backpressure | error |

† `jank-trace` is **inert unless `dev.captureFrames` is on**; `level: 'trace'` force-enables frame
capture so the rule is always live at the highest verbosity.

Every anomaly record carries `refs: number[]` (the implicated `seq`s) so the AI reviewer can jump
straight to the evidence without scanning.

**Anomaly detection is redaction-independent by construction:** every rule keys off `cmd` names,
`argsHash`, scopes, component ids, counts and timings — never off repo content. A `strict` log has
exactly the same anomaly signal as a `raw` one.

---

## 6. On-disk format & lifecycle

```
<app_config_dir>/            # sibling to settings.json (com.bonsai.app)
  logs/
    bonsai-2026-08-27T14-03-11-<sessionId>.jsonl     # one file per session
    bonsai-...-1.jsonl                               # rotation part when size cap hit
  metrics/
    usage.json
```
- **Format:** JSONL, one `LogRecord` per line. First line is always the `session` header record
  (schema version, app version, OS, **redaction mode + `redactionNote`**).
- **File granularity — DECIDED: one file per session** (matches "send me the log from when it
  flickered", and keeps one file to one redaction mode). Not daily, not per-repo.
- **Rotation:** 16 MB per part, max 8 parts per session; **pruning:** keep the 10 most recent
  session files, total cap 256 MB, oldest deleted first at session start.
- **Retention — DECIDED: no AUTOMATIC deletion beyond the caps above.** Turning Dev mode off closes
  the sink and leaves every existing file on disk (the user's whole workflow is exporting *after*
  the fact). Implementations must not add delete-on-disable, delete-on-uninstall, or age-based
  expiry. **This prohibition covers automatic deletion only** — the explicit, user-initiated
  `logs_delete_all` (§6.1) is in scope for v1 and is not a violation of it.
- **Sink:** `std::sync::mpsc::sync_channel(4096)` → one dedicated writer thread with a
  `BufWriter`. All producers use `try_send`; on full, increment a drop counter and emit one `drop`
  record when it drains. **Never blocks the git or UI paths — no lock is held across a write.**
- **Flush:** every 1 s, on 64 KB buffered, on `RunEvent::Exit`/`ExitRequested` (join the writer with
  a 2 s timeout), and on panic via a hook that flushes before unwinding.
- **Frontend → file:** `log_append(records: Vec<LogRecord>)`, called by `obs/batcher.ts` at
  500 ms / 100 records / page-hide. The batcher drops oldest on overflow (cap 5000) and reports
  the drop as a `drop` record. `log_append` is on the instrumentation exclusion list.
- **Mock mode:** the sink client writes to an in-memory ring buffer; `window.__bonsaiDumpLogs()`
  returns the JSONL string so the browser harness can assert schema + anomalies with no Tauri.

### 6.1 "Delete all log files" — **DECIDED: in v1** (roll-then-purge)

**Why it ships:** logs survive Dev mode being switched off (§6 retention), and pruning only fires
once 10 *newer* sessions exist. An occasional debugger who enables `raw` names once and never
produces 10 more sessions would otherwise leave real branch / tag / file / repo names on disk
indefinitely. Manual folder deletion is not an acceptable remedy for a privacy-relevant artifact
the app itself created.

```rust
#[tauri::command]
async fn logs_delete_all(
    app: AppHandle, state: State<'_, AppState>,
) -> Result<LogsDeleteResult, AppError>;

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogsDeleteResult {
    /// Files actually removed from disk.
    pub deleted_files: u32,
    /// Bytes reclaimed (sum of the sizes of the removed files, measured before removal).
    pub deleted_bytes: u64,
    /// Files that could not be removed (locked by another process, permission denied, ...).
    /// Redacted names: `log#<n>.jsonl` — never absolute paths.
    pub failed_files: u32,
    /// Present only when Dev mode was ON: the fresh, empty file logging continues into.
    pub active_file: Option<String>,     // file NAME only, not a path
    /// True when the writer was rolled to a new file as part of this operation.
    pub rolled: bool,
}
```
```ts
export interface LogsDeleteResult {
  deletedFiles: number; deletedBytes: number; failedFiles: number;
  activeFile: string | null; rolled: boolean;
}
```

**Behaviour when Dev mode is ON and the writer holds the current file open — DECIDED:
*roll, then purge*.** The command sends a `RollAndPurge` control message to the writer thread and
awaits its completion (`spawn_blocking`, so the UI never blocks). The writer, on its own thread:
1. flushes and **closes** the current file (releasing the Windows handle);
2. opens a **new** session file with a fresh `session` header record carrying `afterPurge: true`;
3. enumerates every other `*.jsonl` in `logs/`, records each file's size, and deletes it;
4. returns counts, with `rolled: true` and `active_file: Some(<new file name>)`.

Rationale: it is the only option that satisfies the privacy intent (**every byte written before the
click is gone, including the current session's — that is what "delete all" must mean for a
privacy-relevant artifact**), works identically on Windows, macOS and Linux, and does not force the
user to disable Dev mode and lose their in-progress debugging session. "Keep the live file" would
leave the very `raw`-names session the user is trying to erase; "refuse while active" makes the
feature unavailable in the exact state where it matters most.

When Dev mode is OFF, no writer exists: the command deletes every `*.jsonl` in `logs/` and returns
`rolled: false`, `active_file: None`.

Other rules:
- **Partial results are reported honestly, never as success.** A file that cannot be removed
  increments `failed_files`; the command still returns `Ok` with the partial counts, and the UI
  must state "Deleted N files (X MB); M could not be removed." A `failed_files > 0` result is a
  visible warning state, not a silent no-op.
- `deleted_bytes` is summed from `metadata().len()` read **immediately before** each successful
  removal, so the number the UI shows is what was actually reclaimed.
- Scope is exactly `<app_config_dir>/logs/*.jsonl` (plus any `*.jsonl.tmp`). The command never
  touches `metrics/`, `settings.json`, or anything outside `logs/`.
- `logs_delete_all` is on the IPC-instrumentation exclusion list (§2.3) — deleting logs must not
  itself write a log record into the file that survives.
- The UI **must confirm before invoking** (destructive-operation guardrail), and the confirm copy
  states the file count and total bytes obtained from `log_session_info` beforehand.
- **Mock IPC:** `src/ipc/mock/obs.ts` clears the in-memory ring buffer and returns a plausible
  `LogsDeleteResult` (`deletedFiles: 3, deletedBytes: 1_248_130, failedFiles: 0, rolled: true,
  activeFile: 'bonsai-…-mock.jsonl'`), so the browser harness exercises both the success and the
  `failedFiles > 0` copy path (a fixture flag toggles the latter).

### Commands
```rust
#[tauri::command] async fn log_append(state: State<'_, AppState>, records: Vec<LogRecord>) -> Result<(), AppError>;
#[tauri::command] async fn log_session_info(state: State<'_, AppState>) -> Result<LogSessionInfo, AppError>;
#[tauri::command] async fn log_reveal_dir(app: AppHandle) -> Result<(), AppError>;
#[tauri::command] async fn log_export_session(app: AppHandle, dest: Option<String>) -> Result<String, AppError>; // zips current session parts, returns path
#[tauri::command] async fn logs_delete_all(app: AppHandle, state: State<'_, AppState>) -> Result<LogsDeleteResult, AppError>; // §6.1
#[tauri::command] async fn metrics_snapshot(state: State<'_, AppState>) -> Result<MetricsSnapshot, AppError>;
#[tauri::command] async fn metrics_reset(state: State<'_, AppState>) -> Result<(), AppError>;
```
```ts
export interface LogSessionInfo {
  sessionId: string; dir: string; files: string[]; bytes: number;
  records: number; anomalies: number; dropped: number;
  redaction: RedactionMode;
  /** Total across ALL log files on disk, not just this session — the confirm copy needs it. */
  totalFiles: number; totalBytes: number;
}
```
All seven added to `IpcApi`, `src/ipc/tauri/obs.ts`, `src/ipc/mock/obs.ts`, and
`generate_handler![...]` in `src-tauri/src/lib.rs`.

**`metrics_reset` — DECIDED:** the command ships (tests need it) but **no UI exposes it in P91**.
No catalog row, no button. It becomes user-reachable only when a Statistics page exists, so there
is no way to destroy history from a surface that cannot yet display it. (Contrast `logs_delete_all`,
which *is* user-exposed — logs carry privacy-relevant content; metrics structurally cannot.)

---

## 7. Redaction & privacy — **DECIDED: conservative by default, opt-in escape hatch**

```ts
export type RedactionMode = 'strict' | 'raw';   // default 'strict'
```

A `strict` log file is **safe to hand to a third party without reading it first**. Structure,
timings, trace ids, counts, command/event names and every anomaly signal survive; nothing that
identifies the repo, its people or its contents does.

### 7.1 What is captured vs. redacted

| Data | `strict` (default) | `raw` (opt-in) |
|---|---|---|
| Command / event / channel names, `kind`, `outcome`, `errCode` | kept | kept |
| Trace ids, span ids, seq, timings, counts, scopes | kept | kept |
| Component / effect / state-field names (source symbols, not user data) | kept | kept |
| Argument **values** | **elided** → `argsHash` + `argsShape` | included as `args` |
| Repo path | `repo#<n>` | absolute path |
| File paths | `path#<n>` (extension kept: `path#7.ts`) | real path |
| Branch / tag / ref names | `ref#<n>` (kind kept: `ref#3(branch)`) | real name |
| Commit messages, diff / file contents, blame text, search queries | **never, in either mode** | **never** |
| Author / committer name + email | **never, in either mode** | **never** |
| Remote URLs | `remote#<n>` (scheme+forge kind kept: `remote#1(https,github)`) | full URL, userinfo stripped |
| Commit SHAs | first 7 chars kept (needed to correlate; not identifying on their own) | full SHA |
| Tokens, passwords, `Authorization` headers, PATs, SSH keys | **NEVER, under any setting** | **NEVER** |
| Error messages | scrubbed of paths, refs, URLs and tokens | kept, tokens still scrubbed |

`raw` mode is precisely why §6.1 exists: it is the only mode that puts real names on disk, and the
user must be able to remove them on demand.

### 7.2 Hashing scheme

```rust
// obs/redact.rs
pub struct Redactor { salt: [u8; 16], ids: DashMap<(Kind, String), u32>, next: AtomicU32 }
impl Redactor {
    /// Returns e.g. "path#7". First sight of a value assigns the next ordinal for its Kind.
    pub fn tag(&self, kind: Kind, value: &str) -> String;
    pub fn hash_args(&self, canonical_json: &str) -> String;   // 8-hex, salted
}
pub enum Kind { Repo, Path, Ref, Remote, Other }
```
- **Salt: 16 random bytes generated per session, held in memory only, never persisted.**
  Consequence — the same branch is `ref#3` in every record of one file (so the AI reviewer can
  correlate) and is a *different* ordinal in tomorrow's file (so files cannot be cross-linked or
  dictionary-attacked). This is the explicitly required property. A purge roll (§6.1) keeps the
  session salt: the new file continues the same session's ordinals.
- The frontend gets the same salt at boot via `log_session_info` and applies the identical scheme
  in `src/obs/redact.ts`, so UI-side and Rust-side records agree on `ref#3`.
- `argsHash` is a salted 8-hex digest of the canonicalized JSON args. It powers `dup-ipc` without
  storing content — one mechanism serving both §5 and §7.
- **Token scrubber runs last, on every string field, in both modes**, against a fixed pattern set
  (`ghp_`, `github_pat_`, `gh[pousr]_`, `xox[baprs]-`, `AZDO`/base64 PAT shape, `Bearer …`,
  `://user:pass@`, `-----BEGIN … PRIVATE KEY-----`, any value under a key matching
  `/token|secret|password|passphrase|auth/i`). Matches become `<redacted:token>`.

### 7.3 Mode disclosure & UI statement

- The `session` header record carries `redaction` and a `redactionNote` string, so a reviewer
  opening the file **immediately knows what they are looking at** without external context.
- `dev.include-raw-names` (the `raw` toggle) requires an explicit confirm dialog and shows a
  persistent warning row while on. Turning it on starts a **new** log file (a single file never
  mixes modes).
- The Settings Dev page shows a fixed, always-visible statement of log contents. Copy is
  ui-designer's; the *content* is exactly the §7.1 table plus: "Logs are written only to this
  computer. Bonsai never uploads them." plus a pointer to the delete action.

---

## 8. Metrics — **DECIDED: rolled-up JSON file, not SQLite**

Justification retained so a future session does not relitigate it:

1. **Dependency set.** `src-tauri/Cargo.toml` has no SQLite today. `rusqlite` bundles a C
   amalgamation — a second vendored C build alongside libgit2, on three platforms, for data that is
   a few kilobytes. `serde_json` is already present and costs nothing.
2. **Access pattern.** The future Statistics page reads **one aggregate snapshot**. There are no
   joins, no ad-hoc queries, no per-event rows to index — the design deliberately stores daily
   rollups, not an event log. SQL buys query power the data model does not have.
3. **Write pattern.** Exactly one writer, a dirty-flag flush every 60 s and on exit. No concurrency
   story is needed; SQLite's main advantage is moot.
4. **Crash safety.** `write tmp → fsync → rename` (atomic on all three targets), plus a
   `usage.json.bak` retained from the previous successful write. A torn write costs **at most the
   last 60 s of counters**; a corrupt file is detected on load and falls back to `.bak` rather than
   losing history. This is the only dimension where SQLite is genuinely stronger, and the loss
   window is bounded and trivial.
5. **Portability / inspectability.** A JSON file is user-readable, diffable and trivially
   exportable. SQLite would need a viewer.

**Revisit trigger (the only one):** a Statistics page that needs **per-event drill-down** (e.g.
"show me every commit I made in March"). That is a different data model and would justify a
migration, which the versioned `schema` field makes possible. Aggregate charts alone are *not* a
revisit trigger.

```rust
pub struct MetricsSnapshot {          // camelCase serde
    pub schema: u32,
    pub first_seen: String,           // ISO date
    pub sessions: u64,
    pub days: Vec<DayBucket>,         // retained: 400 days, then folded into `lifetime`
    pub lifetime: MetricTotals,
}
pub struct DayBucket { pub date: String, pub totals: MetricTotals }
pub struct MetricTotals {
    pub counters: BTreeMap<String, u64>,        // 'repo.open', 'commit.create', 'graph.walk', ...
    pub durations: BTreeMap<String, Histogram>, // op → histogram
    pub errors: BTreeMap<String, u64>,          // errCode → count
    pub session_ms: u64,
}
pub struct Histogram { pub count: u64, pub sum_ms: u64, pub max_ms: u64,
                       pub buckets: [u64; 8] }  // 1,5,10,50,100,500,2000,+inf ms
```
- **Absorption:** at each flush, `PerfState::snapshot()` deltas fold into `counters` under
  `perf.repo_opens`, `perf.graph_walks`, `perf.graph_cache_hits`, `perf.graph_redecorates`,
  `perf.status_scans`. `perf.rs` is unchanged; no second hot-path counter is introduced.
- **Key namespace:** `<domain>.<action>` only, from a fixed allow-list in `obs/metrics.rs` — no
  user-derived string can ever become a key (privacy + unbounded-growth guard). Metrics therefore
  need no redaction: **they structurally cannot contain repo content**, which is why they are not
  covered by `logs_delete_all`.
- **No network sink exists.** No HTTP client is reachable from `obs/*`; a test asserts it.
- Read API for the future Statistics page: `metrics_snapshot()`. **No UI is designed in P91.**

---

## 9. React causality instrumentation (dev-mode only, zero-cost off)

### 9.1 Hooks

```ts
// src/obs/react.ts
export type RenderLogMode = 'each' | 'aggregate';

export function useRenderCount(
  component: string,
  props?: Record<string, unknown>,
  mode?: RenderLogMode,          // default 'each'
): void;

export function useTracedEffect(
  component: string, effect: string, fn: React.EffectCallback,
  deps: React.DependencyList, depNames: string[],
): void;

export function useStateTransitionLog(store: string, values: Record<string, unknown>): void;
```
- All three begin with `if (!obsEnabled()) { /* delegate raw */ return; }`. `useTracedEffect`
  falls through to `useEffect(fn, deps)` with **no** extra allocation, no prev-deps array. Hook
  order is unconditional (a stable `useRef(null)` is always created; nothing is written when off).
- `changedDeps` / `changedProps` are computed by `Object.is` against the previous values and mapped
  through names — **names only, never values** (dep values are frequently repo content).

### 9.2 Aggregate render mode (required for list-heavy surfaces)

`mode: 'aggregate'` routes through `obs/renderTally.ts`: renders are accumulated in a module-level
map keyed by `component` and flushed as **one `render.tally` record per component per 500 ms
window** (`renders`, `instances`, union of `changedProps`, active traces). No record is emitted for
a window with zero renders. Per-render `render` records are *not* produced in this mode.

This keeps a surface that re-renders once per row per ref change inside the §11 volume budget while
still exposing the storm signal — `render-storm` (§5) fires on `renders > 3 × instances`, which is
exactly the "re-rendered far more than it was mounted" evidence a flicker produces.

### 9.3 The six instrumented surfaces (v1) — **DECIDED**

| # | Surface | Files | Mode |
|---|---|---|---|
| 1 | Workspace container | `src/components/repoWorkspace/*` container + `useCoalescedRefresh.ts`, `useRepoChangeSubscription.ts` | `each` |
| 2 | Diff browser | `DiffBrowser` container | `each` |
| 3 | Graph canvas | `src/graph/GraphCanvas.tsx` (+ `frameStats` routing) | `each` |
| 4 | Right-panel tab container | the tab host component | `each` |
| 5 | PR panel | PR panel container | `each` |
| 6 | **Left sidebar (branches / remotes / tags)** — added by user decision; an observed flicker site | `src/components/Sidebar.tsx` (container, `each`); `BranchesSection.tsx`, `RemotesSection.tsx`, `TagsSection.tsx` (`aggregate`); `rows.tsx` (`aggregate`, one shared tally key per row component — **never per row instance**) | mixed, see left |

Sidebar rules, stated explicitly because this is the one surface that can flood the log:
- `Sidebar.tsx` (the container) uses `each`: it mounts once, so per-render records are bounded and
  are the most useful signal (which prop changed).
- The three section components and `rows.tsx` use `aggregate`. `rows.tsx` renders once per branch /
  remote / tag — on a repo with hundreds of refs, `each` mode would emit hundreds of records per ref
  change and would breach both the 100-records-per-batch and the ≤1-`log_append`-per-500 ms budgets
  on its own. Aggregate mode collapses that to one record per row component per window.
- Effects in the sections use `useTracedEffect` normally (effects run per section, not per row).
- `useSidebarTreeItem.ts` / `useSidebarTreeNav.ts` are **not** instrumented in v1 (per-item hooks;
  their signal is already covered by the row tally).

Nothing outside these six surfaces is instrumented in v1. App-wide instrumentation is a follow-up,
justified only if these six prove insufficient.

- `frameStats.ts`: `createFrameRecorder()` gains an optional `onWindow(stats)` callback; when Dev
  mode is on, `GraphCanvas` routes it to `logRecord({kind:'frame'})` instead of `console.log`.

---

## 10. Settings surface (behaviour only; visuals = ui-designer)

New category `'dev'` (rail last, `dividerBefore: true`), rows:

| Row id | Control | Effect |
|---|---|---|
| `dev.enabled` | switch | master Dev-mode gate; OFF ⇒ no instrumentation active, no file writes, **no automatic log deletion** |
| `dev.level` | segmented | `info` / `debug` / `trace` (`trace` force-enables frame capture) |
| `dev.capture-ipc` | switch | ipc/event/channel records (default on) |
| `dev.capture-react` | switch | render/render.tally/effect/state records (default on) |
| `dev.capture-frames` | switch | frame records (default off — high volume) |
| `dev.include-raw-names` | switch | `strict` → `raw` (§7). Default **off**. Confirm dialog on enable; persistent warning row while on; starts a new log file |
| `dev.reveal-logs` | button | `log_reveal_dir()` |
| `dev.export-session` | button | `log_export_session()` → save dialog; re-shows the content statement first |
| `dev.delete-logs` | button (destructive) | **§6.1** — confirm dialog first, stating `totalFiles` / `totalBytes` from `log_session_info`; then `logs_delete_all()`. Reports the result honestly: success count + bytes, and a warning state when `failedFiles > 0`. When Dev mode is ON, the copy states that logging continues into a new, empty file |
| `dev.session-info` | readonly | `LogSessionInfo` summary (files, size, records, anomalies, dropped, redaction mode); refreshes after a delete |
| `dev.privacy-note` | readonly | the fixed §7.3 statement of what a log file contains |

**No row for `metrics_reset` in P91** (§6).

**Persistence must change in lockstep across exactly these files:**
1. `src-tauri/src/settings.rs` — `#[serde(default)] pub dev: DevSettings` on `Settings` (+ struct)
2. `src-tauri/src/settings/prefs.rs` — patch/merge arm for `dev`
3. `src/ipc/types/settings.ts` — `DevSettings` on `UiSettings` + `UiSettingsPatch`
4. `src/settings/defaults.ts`
5. `src/settings/uiSettingsDefaults.json`
6. `src/components/settings/catalog/dev.ts` (+ register in the catalog barrel + `SettingsCategoryId`)
7. `src/components/settings/categories/DevPage.tsx` + `CATEGORY_PAGES`
8. `src/ipc/mock/settings.ts` (mock defaults) and `src/ipc/mock/obs.ts`

```ts
export interface DevSettings {
  enabled: boolean;          // default false
  level: LogLevel;           // default 'debug'
  captureIpc: boolean;       // true
  captureReact: boolean;     // true
  captureFrames: boolean;    // false
  includeRawNames: boolean;  // false  → RedactionMode 'raw' when true
}
```
Toggling `dev.enabled` takes effect **immediately** (no restart): the flag flips `obsEnabled()`
(which the IPC `get` trap re-reads per access), and Rust opens/closes the sink. A `session` record
is written on each enable.

---

## 11. Performance budget

| State | Budget | Enforcement |
|---|---|---|
| Dev mode **OFF** | ≤ **1 %** on graph scroll frame time; **zero** allocations per IPC call beyond today; no writer thread spawned; no `useEffect` dep copies; no render tally allocated | perf_gate case asserting sink-disabled paths do no work; vitest asserts `instrumentIpc(api).someCmd === api.someCmd` **while disabled at access time** |
| Dev mode **ON** | ≤ **5 %** frame-time regression on a 20k-commit scroll; ≤ 2 ms added per IPC call; ≤ 1 `log_append` per 500 ms | perf_gate case with the sink enabled + `unbatched-sink` must not fire in the harness run |
| Dev mode **ON, sidebar** | a single ref change on a **500-ref** repo produces **≤ 8** react records total (container `each` + 4 aggregate tallies), never one per row | vitest: mount the sidebar with a 500-ref fixture, trigger one ref change, count records |
| Sink | never blocks a caller (bounded `try_send`), writer thread only | test: fill the channel, assert producers return immediately and a `drop` record appears |
| `logs_delete_all` | runs on `spawn_blocking`; UI never blocks; no log record is lost between the flush and the new file opening | test: enqueue records concurrently with a purge, assert none are lost after the roll |
| Redactor | ≤ 5 µs per record (`DashMap` hit + one salted hash) | bench in `obs/redact.rs` tests |

---

## 12. Delivery decomposition

Seven increments, dependency-ordered; each sized for one fresh-context senior-dev pass. 1–3 build
the pipe, **4 is the milestone's payload**, 5 makes it self-analysing, 6–7 are the durable-metrics
and user-surface tails.

| # | Increment | UI? | Scope | Acceptance |
|---|---|---|---|---|
| 1 | **Log core (Rust)** — `obs/record.rs`, `redact.rs`, `sink.rs`, `writer.rs`, `commands/obs.rs` (`log_append`, `log_session_info`, `log_reveal_dir`, `log_export_session`), `DevSettings` + all 8 lockstep files, mock IPC stubs | no | schema, redaction, sink, rotation, pruning, flush-on-exit, settings plumbing | `cargo test`: rotation at cap; pruning keeps N; backpressure emits `drop`; exit flush loses 0 records; `Redactor` assigns stable ordinals within a session and different ones across sessions; token scrubber catches every fixture pattern; **turning Dev mode off deletes no file**. Enabling Dev mode creates a `logs/*.jsonl` whose first line is a valid `session` record naming the redaction mode |
| 2 | **Frontend pipeline** — `obs/{types,enabled,trace,redact,log,batcher,ipcProxy}.ts`, wire into `src/ipc/index.ts`, mock ring buffer + `__bonsaiDumpLogs()` | no | proxy, trace minting, redaction mirror, batching | vitest: a mock-IPC call produces paired `ipc.call`/`ipc.result` sharing one trace + one span; disabled ⇒ method identity preserved; ≤1 `log_append` per 500 ms; a call with a path argument logs `argsHash`+`argsShape` and **no path substring** |
| 3 | **Rust dispatch + events + watcher** — `invoke_shim.rs` (§2.3, incl. the §2.3.1 contingency note in the module doc), `trace.rs`, migrate every `emit(` site to `emit_logged` with an explicit `TraceMeta`, watcher batch/debounce records | no | backend choke point | every dispatch yields an `ipc.recv` stamped with the frontend-injected `__trace`; no `emit` site remains unmigrated (grep test); a git-op burst yields watcher `fired` records with correct `paths`/`relevant`/`debounceMs`. **If `tauri::ipc::Invoke` will not compile, execute §2.3.1 (drop the shim) and drop only the `ipc.recv` criterion — do not modify command signatures** |
| 4 | **Refresh + echo causality + React causality across the SIX surfaces** — `armEcho(repoId, trace)`, suppressed-watcher record at `useCoalescedRefresh.ts:82`, `pendingTracesRef` + `refresh` records, `obs/react.ts` + `obs/renderTally.ts`, applied per the §9.3 table (incl. the sidebar), `frameStats` routing | **yes** | **the flicker evidence** | (a) a mutation followed by its fs echo yields a `watcher` record with `suppressed:true` and `causedBy` = the mutation's trace; (b) one mutation ⇒ exactly one `refresh` record listing every collapsed contributing trace; (c) an effect re-run with unchanged deps emits `effect-no-change`; (d) **sidebar: one ref change on a 500-ref fixture yields ≤8 react records — one `each` record for `Sidebar.tsx` plus one `render.tally` per section/row component — and zero per-row records**; (e) a sidebar flicker (repeated re-render with no ref change) shows as a `render.tally` with `renders > 3 × instances` |
| 5 | **Anomaly detector** — `obs/anomaly.rs`, every sink-side rule in §5 incl. `render-storm` | no | derived records | scripted double-click in the harness produces a `dup-ipc` anomaly whose `refs` point at the two `ipc.call` seqs; each rule has a unit test with a true-positive and a true-negative case |
| 6 | **Metrics** — `obs/metrics.rs`, `metrics_file.rs`, `perf.rs` absorption, `metrics_snapshot`/`metrics_reset` + mock | no | durable local aggregates | counters survive restart; `.bak` recovery on a corrupt file; daily bucketing correct across a simulated date change; `perf` deltas appear as `perf.*`; test asserts `obs/` reaches no HTTP dependency and that no metric key is user-derived; **`metrics_reset` exists as a command and appears in no catalog row** |
| 7 | **Settings Dev page + `logs_delete_all`** — the `RollAndPurge` writer path + `logs_delete_all` command + mock, `LogSessionInfo.totalFiles/totalBytes`, catalog rows, `DevPage.tsx`, reveal/export/delete actions, raw-names confirm + warning, privacy statement | **yes** (ui-designer first) | user surface + purge | catalog parity test passes; toggling `dev.enabled` starts/stops writing **and deletes nothing**; enabling raw names confirms and starts a new file; export produces a zip of the session parts. **`logs_delete_all`:** (a) Dev mode OFF ⇒ every `*.jsonl` removed, `rolled:false`, `activeFile:null`; (b) **Dev mode ON ⇒ the writer rolls to a new file with an `afterPurge:true` header, every prior file (including the one just closed) is removed, `rolled:true`, `activeFile` names the new file, and logging continues with no lost record**; (c) `deletedFiles` / `deletedBytes` **exactly match** the files and byte sizes removed, asserted against a pre-seeded fixture directory; (d) a file made undeletable increments `failedFiles`, the command still returns `Ok`, and the UI shows the partial-result warning; (e) nothing outside `logs/` is touched (assert `metrics/` and `settings.json` survive); (f) the delete is confirm-gated and writes no record into the surviving file |

Increments 4 and 7 require a `ui-designer` pass (`docs/contracts/P91-observability-ui.md`) before
senior-dev.

### Milestone gate
**AI gate:** `cargo test` + vitest green; `pnpm gate`; browser harness with `VITE_MOCK_IPC=1` runs a
scripted flicker scenario (including a sidebar ref change) and `__bonsaiDumpLogs()` output is
inspected for correct traces and at least one true-positive anomaly; §11 budgets hold, incl. the
sidebar record-count bound; a real `logs/*.jsonl` from a `pnpm tauri dev` session is parsed
line-by-line by a test asserting schema validity and **zero redaction violations** (regex scan for
path separators, `@`, `http`, known token shapes, and any branch name present in the fixture repo).

**USER CHECKPOINT:** open a real repo in the native window with Dev mode ON, perform the actions
that flicker (**including the sidebar interactions where flickering was observed**), then (a)
"Reveal logs folder" opens the correct directory, (b) "Export session" produces a zip, (c) the user
reads the exported JSONL and confirms it identifies the misbehaviour **and contains nothing they
would not send to a third party**, (d) **"Delete all log files" with Dev mode still ON empties the
folder down to one fresh file, the reported count/size look right, and logging visibly continues**,
(e) with Dev mode OFF, graph scroll on a 20k-commit repo feels unchanged.

---

## 13. Decision record (all resolved — user, 2026-08-27)

| # | Decision | Outcome | Where it lands |
|---|---|---|---|
| 1 | Trace transport | **APPROVED as specced** — injected `__trace`/`__span` args key + the Rust `ipc.recv` shim. Contingency retained in §2.3.1 with the exact visibility lost if the shim is dropped | §2.2, §2.3, §2.3.1, §12 inc. 3 |
| 2 | Metrics storage | **APPROVED — rolled-up JSON**, not SQLite. Justification and the sole revisit trigger (per-event drill-down) retained | §8 |
| 3 | React instrumentation scope | **CHANGED — SIX surfaces**, adding the **left sidebar** (an observed flicker site). Sidebar sections + `rows.tsx` use the new **aggregate** render mode so a per-ref re-render storm cannot flood the log | §9.2, §9.3, §11, §12 inc. 4 |
| 4 | Log retention | **APPROVED — no automatic deletion.** Turning Dev mode off deletes nothing; the §6 size/count caps are the only *automatic* deletion path | §6, §10, §12 inc. 1 & 7 |
| 5 | Log file granularity | **APPROVED — one file per session** | §6 |
| 6 | `metrics_reset` | **APPROVED — ship the command, expose no UI** until a Statistics page exists | §6, §10, §12 inc. 6 |
| 7 | **"Delete all log files"** | **PULLED INTO v1** (was a deferred follow-up under decision 4; raised by ui-designer, accepted by the user). Rationale: logs survive Dev mode being disabled, and pruning only fires after 10 newer sessions — so one `raw`-names session can leave real names on disk indefinitely for an occasional debugger. Manual folder deletion is not an acceptable remedy for a privacy-relevant artifact the app created. Behaviour with the writer active: **roll to a new file, then purge everything else** | §6.1, §6 Commands, §7.1, §10 (`dev.delete-logs`), §11, §12 inc. 7 |

**Deferred follow-ups (explicitly out of P91):** app-wide React instrumentation beyond the six
surfaces; the Statistics page and any UI for `metrics_reset`; selective/per-file log deletion (v1
is all-or-nothing).
