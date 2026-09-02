# P91 — Observability: Dev mode, structured logs, local telemetry & metrics

**Goal (drives every choice):** produce a JSONL file the user can hand to an AI reviewer that makes
**double triggers, redundant IPC, effects firing on unchanged deps, unintended side effects and
never-meant-to-happen actions** *mechanically visible* — via correlation ids + machine-emitted
anomaly records, not via prose log lines.

**Secondary goal (amendment 2026-08-27, §3.1/§5.1/§8.1):** the same file must make **performance
problems** mechanically visible — *where* an operation spent its time, whether it was slow relative
to its own baseline, whether it was queued rather than computing, and whether a cache stopped
working.

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
| `obs/redact.rs` | session salt, `Redactor` (salt-seeded counter ordinals, §7.2), token scrubber (§7.2.1) |
| `obs/sink.rs` | bounded MPSC → writer thread; `try_send`, drop counter, flush; `RollAndPurge` control message |
| `obs/writer.rs` | file naming, JSONL append, rotation (§6.3), pruning, header record, **purge** (§6.1) |
| `obs/anomaly.rs` | streaming detectors (§5) over the unified record stream |
| `obs/trace.rs` | `TraceId` type, minting, `TraceMeta`, `emit_logged` event helper |
| `obs/phase.rs` | **(inc. 3)** `PhaseRecorder` — explicit sub-span timing + `span` record emission (§3.1) |
| `obs/invoke_shim.rs` | `invoke_handler` wrapper logging every command dispatch |
| `obs/metrics.rs` | `MetricsStore` (counters/histograms), aggregation cadence |
| `obs/metrics_file.rs` | atomic load/save of `metrics/usage.json`, daily buckets, retention |
| `obs/metrics_keys.rs` | **The metrics key privacy guard.** Sole home of the `<domain>.<action>` / `cmd.<name>` / error-code shape predicates that decide whether a string may become a key in `usage.json`. Separate from `metrics.rs` **by contract, not by size**: `usage.json` is durable, is not covered by `logs_delete_all` and has no redaction pass, so a user-derived key there is permanent, un-deletable repo content. |
| `commands/obs.rs` | `log_append`, `log_session_info`, `log_reveal_dir`, `log_export_session`, `logs_delete_all`, `metrics_snapshot`, `metrics_reset` |

`perf.rs` is **kept** as the hot-path atomic tally and is *absorbed*: `MetricsStore` reads
`PerfState::snapshot()` at each flush and folds the delta into durable counters. No parallel counters.

### Frontend — `src/obs/`
| File | Responsibility |
|---|---|
| `obs/types.ts` | TS mirror of `LogRecord` + `TraceId` |
| `obs/enabled.ts` | `obsEnabled()` / `obsLevel()` — single boolean read, set once at boot + on settings change |
| `obs/trace.ts` | `newTrace()`, `withTrace()`, `currentTrace()` (sync ambient), `bindTrace()` |
| `obs/redact.ts` | frontend `Redactor` — **`ui:`-namespaced** ordinals + **the sole `argsHash` producer** (§7.2) |
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

**`ipc.recv` carries `cmd` + trace ids and NOTHING derived from the arguments** — see §7.2 and
§12 row 3. It is an arrival stamp, not a second description of the call.

**Known limit:** the shim cannot observe *completion* (the resolver is consumed downstream), so
**command duration is measured on the frontend side**. Backend-internal timings come from explicit
spans in heavy commands (§3.1 `PhaseRecorder`), added only where `perf.rs` already instruments.

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
explicitly and never depended on the shim), echo/double-trigger causality (§2.4, frontend), the
`span` phase records (§3.1 — they are emitted by explicit call sites, not by the shim), and
every anomaly rule except the ability to distinguish "never sent" from "sent but never answered"
— `orphan-trace` degrades from that distinction to a plain unanswered-call signal. **`dup-ipc` is
unaffected, because it keys exclusively on frontend `ipc.call` records (§5).**
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

> **Amendment rule (2026-08-27).** Everything added after increment 1 shipped is **ADDITIVE ONLY**:
> new `LogKind` variants plus new fields that are **all optional with `#[serde(default)]`** on the
> Rust side and `?:` on the TS side. No existing field is renamed, retyped, or redefined;
> `OBS_SCHEMA_VERSION` stays `1`. A reader of a v1 record without these fields stays valid.
> Additive since increment 1: `'span'` (§3.1), `'truncate'` (§6.3), and the two optional
> `SessionPayload` truncation fields.
>
> **§3 is authoritative for record shape.** Where prose elsewhere in this contract disagreed with a
> payload definition here, §3 won and the prose was corrected (see §13 row 20).
>
> **Pre-release carve-out (architect, `8da1291` ratification).** The all-optional clause binds from
> the moment P91 merges to `dev`. While P91 is unmerged, a **required** field may be added to an
> existing payload without a version bump, because no shipped build has written the old shape and no
> reader parses records from disk (§3.2). Exactly one field has used this carve-out:
> `FramePayload.dim` (`FrameDim = 'paint' | 'gap'`, required on both sides). **The carve-out expires
> on merge to `dev`.** After that, any required-field addition to an existing payload, any rename,
> retype or removal, requires an `OBS_SCHEMA_VERSION` bump **and** a §3.2 reader rule for the older
> version. Purely additive optional fields never need either.

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
  | 'span'                                  // ← added, §3.1
  | 'truncate'                              // ← added, §6.3
  | 'drop';
```

Payload unions (each record is `LogRecordBase & { kind: K } & PayloadK`):

```ts
interface SessionPayload  { schema: number; app: string; os: string; sessionId: string;
                            devMode: true; level: LogLevel; redaction: RedactionMode;
                            /** Human-readable one-liner restating §7 for the reviewer. */
                            redactionNote: string;
                            /** True when this header opens a file created by a purge roll (§6.1). */
                            afterPurge?: boolean;
                            /** §6.3 — ADDITIVE. True when one or more EARLIER parts of this
                             *  session have already been deleted, i.e. this file is NOT the
                             *  beginning of the session. */
                            truncated?: boolean;
                            /** §6.3 — ADDITIVE. Count of earlier parts deleted so far. */
                            droppedParts?: number; }
interface GesturePayload  { origin: TraceOrigin; gesture: string; }
/** The ONLY record kind that carries argsHash/argsShape. Emitted by the frontend proxy only. */
interface IpcCallPayload  { cmd: string; argsHash: string; argsShape?: ArgShape;
                            /** raw mode only; keyed by PARAM NAME, allow-listed scalars only
                             *  (A26). Never positional keys — see writer rule W3. */
                            args?: Record<string, unknown>;
                            /** raw mode only, emitted only when > 0: positions the policy elided. */
                            argsOmitted?: number;
                            /** WRITER-SET ONLY. Producers must never emit it; Rust's struct has no
                             *  such field, so a forged one is dropped at deserialisation. */
                            argsPolicyViolation?: boolean; }
```

Rust mirror (`obs/record.rs`, `LogPayload::IpcCall`) — **`args_omitted` is REQUIRED to exist on the
struct and is OPTIONAL on the wire**:

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
args_omitted: Option<u32>,
```

`OBS_SCHEMA_VERSION` stays **1**: the field is optional and additive, inside the §13 row-23
pre-release carve-out.

**Why it was missing, recorded so the class of bug is not repeated.** `argsOmitted` was
contract-declared (A26 §B.6) and producer-emitted, but absent from the Rust `IpcCall` variant, so
`serde` silently dropped it when `log_append` deserialised the record. Writer rule **W6**
(`argsOmitted` must be a non-negative integer, else remove) was therefore **dead in production** —
it could never see the field it validates — while its unit test passed, because that test called
`raw_args::enforce` on a hand-built `serde_json::Value` and so never crossed the
`LogRecord` → `append_record` boundary that W1–W5 exercised.

> **Binding test rule (applies to every current and future writer rule).** A writer rule is
> considered covered only by a test that goes **`LogRecord` → `append_record` → read the file
> back**. A test that hands a synthetic `Value` straight to an enforcement function proves the
> function, not the pipeline, and cannot detect a field that serde drops on the way in. Synthetic-
> `Value` tests are permitted **in addition to**, never **instead of**, a round-trip test.

```ts
interface IpcResultPayload{ cmd: string; argsHash: string; ms: number;
                            outcome: 'ok'|'err'|'aborted'|'superseded';
                            errCode?: string; resultShape?: ArgShape; }
/** Rust-side arrival stamp. Deliberately carries NO argsHash and NO argsShape — adding one would
 *  introduce a SECOND canonical form for the same call and silently break `dup-ipc`. See §7.2
 *  and the §12 row-3 prohibition. */
interface IpcRecvPayload  { cmd: string; }
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
interface FramePayload    { dim: 'paint' | 'gap';    // which quantity this window measured
                            paintMs: number; gapMs: number; over33: number; over100: number;
                            worstMs: number; }
// `dim` (ADDITIVE, increment-4 follow-up): the paint and gap recorders are separate (§4.7), so
// exactly one of paintMs/gapMs is a measurement and the other is a filler 0. Without the
// discriminator a consumer reads `gapMs: 0` on a paint record as "zero gap measured".
// REQUIRED on both sides: the only producer is the frontend's own graphObs.ts and Rust never
// re-parses log files from disk, so there is no older writer to stay compatible with.
interface ErrorPayload    { where: string; code?: string; message: string; stackHash?: string; }
interface AnomalyPayload  { rule: AnomalyRule; severity: 'info'|'warn'|'error';
                            detail: string; refs: number[];  // seq numbers of implicated records
                            traces: TraceId[]; }
interface DropPayload     { dropped: number; sinceSeq: number; } // sink backpressure
/** §6.3 — ADDITIVE. Emitted into the SURVIVING (newest) part immediately after an earlier part
 *  of the same session group is deleted to honour the part cap. Distinct from `drop`, which is
 *  in-memory sink backpressure: `truncate` is loss of records ALREADY WRITTEN to disk. */
interface TruncatePayload  { reason: 'max-parts';
                             /** How many parts have now been deleted for this session. */
                             droppedParts: number;
                             /** Redacted part label, e.g. `part#0` — never a path. */
                             droppedPart: string;
                             bytes: number;
                             /** Lowest `seq` still present on disk, when known. */
                             firstRetainedSeq?: number; }
```

`ArgShape` = `Record<string, 'str'|'num'|'bool'|'null'|`arr:${number}`|`obj:${number}`>` — key
names + type + length only, **never values**. Because `IpcApi` methods are **positional**, the
frontend keys it `"0"`, `"1"`, … (increment 2), matching the positional-array canonical form used
for `argsHash` (§7.2).

Rust mirrors these as `#[serde(tag = "kind", rename_all = "camelCase")] enum LogPayload`.

### 3.1 Backend operation spans — phases, queueing, saturation (ADDITIVE; **increment 3**)

**Shape decision: one `span` record per completed operation**, carrying a `phases` array — *not*
one record per phase. A slow `get_graph` therefore costs exactly one extra line, and its breakdown
is on that line.

```ts
export interface PhaseTiming {
  /** Allow-listed, dotted for nesting: 'revwalk', 'decorate', 'lane', 'serialize'. */
  name: string;
  ms: number;
  /** Optional unit count for the phase (commits walked, files scanned). */
  n?: number;
}

export interface SpanPayload {
  /** Allow-listed operation id, `<domain>.<action>`: 'graph.get' | 'status.scan' | 'diff.compute'. */
  op: string;
  /** Total wall time of the operation, measured at the src-tauri call site. */
  ms: number;
  /** Ordered, ≤16 entries. Sum may be < ms; the remainder is unattributed time. */
  phases?: PhaseTiming[];
  /** ms spent QUEUED before the spawn_blocking closure started running (§3.1.2). */
  queuedMs?: number;
  /** Blocking-pool tasks in flight when this one started, and the pool cap. */
  poolInflight?: number;
  poolMax?: number;
  /** elapsed / git-timeout deadline, 0..1+ — watchdog pressure (§3.1.3). */
  deadlineFrac?: number;
  /** Graph-cache outcome for this op, emitted only by graph_cache.rs (§5.1 `cache-collapse`). */
  cache?: 'hit' | 'redecorate' | 'miss';
  /** Primary unit count for the whole op (commits in the layout, files in the status). */
  items?: number;
  outcome?: 'ok' | 'err' | 'timeout';
}
```

Rust mirror — every new field `#[serde(default, skip_serializing_if = "Option::is_none")]`.
**`SpanPayload` carries no `argsHash` either**, for the same reason as `ipc.recv`.

#### 3.1.1 Mechanism — explicit recorder, no ambient stack

Consistent with §2.2 (no task-local trace): the recorder is a **value threaded explicitly**.

```rust
// src-tauri/src/obs/phase.rs
pub struct PhaseRecorder { /* op, start Instant, Vec<PhaseTiming> (cap 16), optional fields */ }

pub struct PhaseGuard<'a>;   // Drop → pushes {name, ms} onto the parent recorder

impl PhaseRecorder {
    /// No-op recorder when Dev mode is off: all methods compile to a branch + return.
    pub fn start(op: &'static str) -> Self;
    /// Opens a phase; nesting is expressed by the caller using dotted names
    /// ("decorate", "decorate.pills") — there is NO implicit parent stack.
    pub fn phase(&mut self, name: &'static str) -> PhaseGuard<'_>;
    pub fn note_queue(&mut self, queued_ms: u32, inflight: u32, max: u32);
    pub fn note_deadline(&mut self, frac: f32);
    pub fn note_cache(&mut self, outcome: CacheOutcome);
    pub fn note_items(&mut self, n: u64);
    /// Emits the single `span` record. Trace causality is explicit, like emit_logged.
    pub fn finish(self, meta: &TraceMeta, outcome: SpanOutcome);
}
```

Rules:
- `name` and `op` are `&'static str` from an allow-list in `obs/phase.rs` — no user-derived string
  can reach a span record (same guard as the metrics key namespace, §8).
- **Overhead budget:** ≤ 2 `Instant::now()` calls per phase and one `Vec` push; ≤ 16 phases per op;
  ≤ **5 µs** total added per operation. Dev mode off ⇒ zero allocation (the `Vec` is not created).
- Phases are **not nested structurally**; a dotted name is just a label. This keeps the array flat
  and the serializer trivial.

#### 3.1.2 Crate boundary — where the timing lives

`crates/bonsai-core` must **not** depend on `obs/`. All phase timing is taken at the **src-tauri
caller layer**, which already orchestrates these calls and already increments `perf.rs`:

| `op` | Call site | Phases (v1, exactly these) |
|---|---|---|
| `graph.get` | `src-tauri/src/graph_cache.rs` (the walk/decorate/lane orchestration + cache arms) | `revwalk`, `decorate`, `lane`, `filter`, `serialize` |
| `status.scan` | the status command's `spawn_blocking` body | `statuses`, `index`, `map` |
| `diff.compute` | the diff command's `spawn_blocking` body | `tree`, `hunks`, `serialize` |

Three operations, ~11 phase labels total. Nothing else is instrumented in v1; this is a diagnostic
tool, not a profiler. Adding a fourth `op` requires a §13 entry.

> **As-built ratification (D3, increment 3 — commit `a351d36`, reviewer-approved).** The
> per-phase splits above are realised in full **only for `graph.get`**, whose walk/decorate/lane
> orchestration and cache arms are visible at the `graph_cache.rs` src-tauri boundary; it emits
> `revwalk`, `decorate`, `lane`, `serialize` phases plus the `cache` outcome field. **`status.scan`
> and `diff.compute` collapse to a single caller-visible phase each** — `status.scan → statuses`,
> `diff.compute → hunks` — because their finer sub-steps (`index`/`map`, `tree`/`serialize`) happen
> *inside* `crates/bonsai-core`, and instrumenting them there would require bonsai-core to depend on
> `obs/`, breaching the boundary invariant enforced by the `bonsai_core_has_no_obs_reference`
> compile-test. The src-tauri caller sees each as one opaque call. **Op-level timing is unaffected:**
> all three ops still emit a `span` with total `ms` plus the §3.1.3 `queuedMs`/`poolInflight`/
> `poolMax`/`deadlineFrac` fields; only `graph.get` carries a multi-element `phases[]`. Row-3
> acceptance (multi-phase split required **only** for `graph.get`) is therefore satisfied.
>
> **Finer status/diff phasing is PERMANENTLY REJECTED, not deferred:** the `bonsai-core`↔`obs`
> boundary is non-negotiable and a per-phase split of two operations does not justify carving a
> timing seam through it — the op-level `ms` already localises slowness to the operation, which is
> the diagnostic granularity P91 needs. **v1 scope note:** within `diff.compute`, only
> `get_workdir_file_diff` is instrumented; the commit-vs-parent diff is uninstrumented in v1.

#### 3.1.3 Queue delay, saturation, watchdog

- `queuedMs`: capture `Instant::now()` **before** `spawn_blocking`; the first statement inside the
  closure computes the delta. Wired in the `repo_handle.rs` helpers so every git op gets it for free.
- `poolInflight` / `poolMax`: one process-wide `AtomicUsize` gauge incremented on closure entry and
  decremented on exit (added in `obs/phase.rs`, read at span start). `poolMax` is the configured
  blocking-pool cap.
- `deadlineFrac`: `run_with_git_timeout*` already knows its deadline (`effective_deadline`); on
  completion it reports `elapsed / deadline` into the recorder. A near-timeout is
  `deadlineFrac ≥ 0.8`; a real timeout is `outcome: 'timeout'`.

All three are optional fields on the **same** `span` record — one operation, one line.

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
- **This proxy is the single producer of `argsHash`** for the whole system (§7.2).
- **Callback arguments are wrapped too** (`onRepoChanged(cb)`, channel `onChunk`): the wrapper
  emits `event`/`channel` records on *delivery* and re-binds the subscribing trace, so a
  subscription created by trace A shows deliveries as `causedBy: A`.
- `superseded`: the proxy keeps a per-`cmd` latest-span map; if a call resolves while a newer call
  of the same `cmd` is in flight, its result is stamped `superseded`.
- **Mock IPC:** `src/ipc/mock/obs.ts` synthesises plausible `span` records (fixture phase arrays for
  `graph.get`, incl. one deliberately slow fixture) so the harness can exercise §5.1 rules with no
  Tauri.

---

## 5. Anomaly detection

Site-local rules are emitted by the site that has the knowledge. Cross-record rules run in
`obs/anomaly.rs` on the unified stream in the Rust sink (the only place that sees both sides).

| Rule id | Where | Window | Condition | Severity |
|---|---|---|---|---|
| `dup-ipc` | sink | 300 ms | **`kind === 'ipc.call'` only** (explicit precondition, see below) — same `cmd` + `argsHash`, ≥2 calls, no intervening mutation cmd | warn |
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
| **`slow-command`** | sink | — | §5.1 — duration beyond the command's own rolling baseline | warn |
| **`slow-phase`** | sink | — | §5.1 — one phase is ≥70 % of a `slow-command` span's `ms` | info |
| **`queue-delay`** | sink | 5 s | ≥3 `span` records with `queuedMs > 100` | warn |
| **`pool-saturation`** | sink | 5 s | ≥3 spans with `poolInflight >= poolMax` | warn |
| **`watchdog-pressure`** | sink | — | `deadlineFrac ≥ 0.8`, or `outcome: 'timeout'` (error) | warn |
| **`cache-collapse`** | sink | 10 s | §5.1 — graph-cache hit rate collapse | warn |

† `jank-trace` is **inert unless `dev.captureFrames` is on**; `level: 'trace'` force-enables frame
capture so the rule is always live at the highest verbosity.

**`dup-ipc` states its own precondition — REQUIRED, not optional.** The detector must filter on
`kind === 'ipc.call'` **explicitly in code**, not rely on `argsHash` happening to be absent from
other payload types. Rationale: correctness that emerges from a *missing field elsewhere* is exactly
the kind that a later additive change breaks silently, and this contract has been making additive
changes continuously. An explicit kind filter makes the rule locally verifiable and immune to any
future payload gaining an `argsHash`. A unit test asserts that a synthetic non-`ipc.call` record
carrying an `argsHash` does **not** contribute to `dup-ipc`. (Increment 5.)

Every anomaly record carries `refs: number[]` (the implicated `seq`s) so the AI reviewer can jump
straight to the evidence without scanning.

**Anomaly detection is redaction-independent by construction:** every rule keys off `cmd` names,
`argsHash`, scopes, component ids, counts and timings — never off repo content, **and never off a
redaction ordinal** (§7.2 (c)). A `strict` log has exactly the same anomaly signal as a `raw` one.

**`truncate` is not an anomaly rule** (§6.3) — it is a factual record of on-disk loss. It carries no
severity beyond `warn` and no detector consumes it; its only consumer is the reviewing AI's
completeness check.

**`unbatched-sink` mechanism (RATIFIED, increment 5).** The rule (≥10 `log_append` calls / 1 s)
needs per-`log_append` **call boundaries**, which individual records cannot express. This is carried
by a sink-internal `SinkMsg::BatchMark` channel variant, enqueued best-effort by `log_append` before
its records; the single `SyncSender` preserves mark-before-records FIFO ordering. A dropped mark only
softens this one info-severity rule, so best-effort enqueue is acceptable.

### 5.1 Duration & saturation rules (ADDITIVE; **increment 5**)

**`slow-command` — self-calibrating, per command.** A single global threshold is wrong (`get_graph`
on 20k commits vs `stage_file`), so the detector keeps an **in-memory per-`cmd` rolling
distribution reusing the §8 `Histogram` bucket shape** (same 8 boundaries — no new summary type):

```
on ipc.result(cmd, ms):
    h = baseline[cmd]                      # in-memory, session-scoped, ≤200 cmd keys
    if h.count >= MIN_SAMPLES(=20):
        p95 = h.percentile_ms(0.95)        # §8.1, interpolated within the containing bucket
        if ms > max(FLOOR_MS(=150), K(=3.0) * p95):
            emit slow-command{cmd, ms, p95, samples: h.count}
    if ms > HARD_MS(=10_000):              # catch-all, fires even before MIN_SAMPLES
        emit slow-command{severity: 'error', ...}
    h.observe(ms)                          # observe AFTER comparing
```

Constants live in **one table in `obs/anomaly.rs`** (`SLOW_RULES: &[(cmd_prefix, floor_ms, k)]`),
with a default row and per-command overrides (`get_graph` floor 1200 ms; `commit_create` floor
2000 ms). Anti-noise properties, stated so they are testable:
- baseline is *per command*, so a big repo's normal `get_graph` cost becomes the baseline and does
  not fire;
- `MIN_SAMPLES` suppresses the cold-start burst (a repo open fires nothing);
- **rate limit: at most 1 `slow-command` per `cmd` per 10 s**;
- the p95 is bucket-derived and therefore coarse by construction — the rule fires on step changes,
  not on jitter.

**Percentile method — see §8.1 (authoritative).** The inline "interpolated within the containing
bucket" note on the `p95 = h.percentile_ms(0.95)` line above is superseded by §8.1: the method is
**linear interpolation clamped to `max_ms`**, identical to the durable-metrics path, so `slow-command`
(inc. 5) and `metrics_snapshot` (inc. 6) never disagree on the same histogram.

**`slow-phase`:** when a `slow-command` fires and the correlated `span` (same `trace`) has a phase
≥70 % of `ms`, emit `slow-phase{op, phase, ms, share}` referencing both seqs. This is the record
that answers "was it the revwalk or the lane assignment".

**`cache-collapse`:** over a 10 s window of `span{op:'graph.get'}` records, let
`hits / (hits + redecorates + misses)`. Fire when the window has ≥5 spans and the hit rate is
`< 0.2` **while no repo-mutating command appeared in the window** (a real mutation legitimately
invalidates). Detail carries the counts; `refs` point at the offending spans. Cross-checked against
`perf.graph_cache_hits` / `perf.graph_redecorates` deltas at flush.

---

### 3.2 Schema version & reader compatibility

`OBS_SCHEMA_VERSION` (currently **1**) is stamped on the `session` header, which is line 1 of
**every** part, including rotation parts — so any file, and any part handed over on its own, is
self-describing.

**Bump the version when, and only when:** an existing field is renamed, retyped or removed, or a
**required** field is added to an existing payload after P91 has merged to `dev`. Adding a new
`LogKind` variant, or an optional field, never bumps it.

**Reader rules — binding on every consumer of a `.jsonl` file** (the in-app path, any future log
viewer, the gate's line-by-line parse test, and the reviewing AI's preamble):

1. **Unknown `kind` ⇒ skip the record**, do not fail the file. Forward compatibility is by
   construction: `LogKind` grows.
2. **Unknown field ⇒ ignore it.** No consumer may reject on extra keys.
3. **Missing required field ⇒ reject that LINE, count it, continue.** A malformed record never
   invalidates the rest of the file; a truncated tail line at a crash boundary is expected.
4. **Never infer a missing discriminator.** A `frame` record without `dim` is unattributed: report
   it as such, never default it to `paint`, and never average `paintMs`/`gapMs` across records of
   different `dim`.
5. A reader encountering `schema` **greater** than the version it knows parses on a best-effort
   basis under rules 1-3 and says so in its output; it does not refuse the file.

**Rust deserialization posture (as built, intentional):** `LogRecord`'s `Deserialize` exists for the
`log_append` IPC path only — a same-build producer. It is **not** a compatibility surface for old
files, and no increment may make it one without first satisfying the reader rules above.

## 6. On-disk format & lifecycle

```
<app_config_dir>/            # sibling to settings.json (com.bonsai.app)
  logs/
    bonsai-2026-08-27T14-03-11-<sessionId>.jsonl     # one file per session
    bonsai-...-1.jsonl                               # rotation part when size cap hit
  exports/                                           # §6.2 — default target for log_export_session
    bonsai-...-<sessionId>.zip
  metrics/
    usage.json
```
- **Format:** JSONL, one `LogRecord` per line. First line is always the `session` header record
  (schema version, app version, OS, **redaction mode + `redactionNote`**).
- **File granularity — DECIDED: one file per session** (matches "send me the log from when it
  flickered", and keeps one file to one redaction mode). Not daily, not per-repo.
- **Rotation:** 16 MB per part, max 8 parts per session. **Rotation is never refused** — see §6.3
  for what happens at the part cap.
- **Pruning:** keep the 10 most recent session files, total cap 256 MB, oldest deleted first.
  Pruning runs at session start **and after every rotation** (§6.3) — running it only at
  `LogWriter::open` left the total cap unenforced for the whole life of a long session.
- **Retention — DECIDED: no AUTOMATIC deletion beyond the caps above.** Turning Dev mode off closes
  the sink and leaves every existing file on disk (the user's whole workflow is exporting *after*
  the fact). Implementations must not add delete-on-disable, delete-on-uninstall, or age-based
  expiry. **This prohibition covers automatic deletion only** — the explicit, user-initiated
  `logs_delete_all` (§6.1) and the part-cap eviction (§6.3) are the two in-scope exceptions, both
  bounded by an explicit cap.
- **Sink:** `std::sync::mpsc::sync_channel(4096)` → one dedicated writer thread with a
  `BufWriter`. All producers use `try_send`; on full, increment a drop counter and emit one `drop`
  record when it drains. **Never blocks the git or UI paths — no lock is held across a write.**
- **Flush:** every 1 s, on 64 KB buffered, on `RunEvent::Exit`/`ExitRequested` (join the writer with
  a 2 s timeout), and on panic via a hook that flushes before unwinding.
- **Frontend → file:** `log_append(records: Vec<LogRecord>)`, called by `obs/batcher.ts` at
  500 ms / 100 records / page-hide. The batcher drops oldest on overflow (cap 5000) and reports
  the drop as a `drop` record. `log_append` is on the instrumentation exclusion list.
- **The session salt is NEVER written to any log file, export or metrics file** (§7.2). It exists
  in memory and crosses IPC exactly once, to the frontend, via `log_session_info`.
- **Mock mode:** the sink client writes to an in-memory ring buffer; `window.__bonsaiDumpLogs()`
  returns the JSONL string so the browser harness can assert schema + anomalies with no Tauri.
  The authoritative anomaly detector (`obs/anomaly.rs`) runs only on the Rust sink writer thread and
  is absent in mock mode, so those harness anomalies are produced by a **mock-only, dump-time batch
  analyzer** that scans the ring at `__bonsaiDumpLogs()` time (increment 5b). It is scoped to
  **exactly the three gate-named rules** (`dup-ipc`, `slow-command`, `slow-phase`), is a harness-only
  diagnostic that never ships in a production bundle, and does not replace or duplicate the Rust
  detector, which remains the sole authoritative one.

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
    /// Files actually removed from disk (log parts AND export zips — see `deleted_exports`).
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
    /// §6.2 — how many of `deleted_files` were export zips. Optional/additive; the UI uses it
    /// to say "4 log files and 1 export" instead of an undifferentiated count.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deleted_exports: Option<u32>,
}
```
```ts
export interface LogsDeleteResult {
  deletedFiles: number; deletedBytes: number; failedFiles: number;
  activeFile: string | null; rolled: boolean;
  deletedExports?: number;
}
```

**Behaviour when Dev mode is ON and the writer holds the current file open — DECIDED:
*roll, then purge*.** The command sends a `RollAndPurge` control message to the writer thread and
awaits its completion (`spawn_blocking`, so the UI never blocks). The writer, on its own thread:
1. flushes and **closes** the current file (releasing the Windows handle);
2. opens a **new** session file with a fresh `session` header record carrying `afterPurge: true`;
3. enumerates every other `*.jsonl` in `logs/`, records each file's size, and deletes it;
4. deletes every export artifact in scope per §6.2;
5. returns counts, with `rolled: true` and `active_file: Some(<new file name>)`.

Rationale: it is the only option that satisfies the privacy intent (**every byte written before the
click is gone, including the current session's — that is what "delete all" must mean for a
privacy-relevant artifact**), works identically on Windows, macOS and Linux, and does not force the
user to disable Dev mode and lose their in-progress debugging session. "Keep the live file" would
leave the very `raw`-names session the user is trying to erase; "refuse while active" makes the
feature unavailable in the exact state where it matters most.

When Dev mode is OFF, no writer exists: the command deletes every in-scope file and returns
`rolled: false`, `active_file: None`.

Other rules:
- **Partial results are reported honestly, never as success.** A file that cannot be removed
  increments `failed_files`; the command still returns `Ok` with the partial counts, and the UI
  must state "Deleted N files (X MB); M could not be removed." A `failed_files > 0` result is a
  visible warning state, not a silent no-op.
- `deleted_bytes` is summed from `metadata().len()` read **immediately before** each successful
  removal, so the number the UI shows is what was actually reclaimed. Export zips are included in
  both `deleted_files` and `deleted_bytes`, and additionally counted in `deleted_exports`.
- **Purge scope — exactly these, and nothing else:**
  `<app_config_dir>/logs/*.jsonl`, `<app_config_dir>/logs/*.jsonl.tmp`,
  `<app_config_dir>/logs/*.zip` (stray/legacy exports), and `<app_config_dir>/exports/*.zip`.
  The command never touches `metrics/`, `settings.json`, or anything outside those two directories.
- `logs_delete_all` is on the IPC-instrumentation exclusion list (§2.3) — deleting logs must not
  itself write a log record into the file that survives.
- The UI **must confirm before invoking** (destructive-operation guardrail), and the confirm copy
  states the file count and total bytes obtained from `log_session_info` beforehand, **and the
  §6.2 out-of-scope caveat**.
- **Mock IPC:** `src/ipc/mock/obs.ts` clears the in-memory ring buffer and returns a plausible
  `LogsDeleteResult` (`deletedFiles: 4, deletedBytes: 1_248_130, failedFiles: 0, rolled: true,
  activeFile: 'bonsai-…-mock.jsonl', deletedExports: 1`), so the browser harness exercises both the
  success and the `failedFiles > 0` copy path (a fixture flag toggles the latter).

### 6.2 Export artifacts are in the delete scope — **DECIDED (2026-08-27), do not re-open**

**The defect this closes.** `log_export_session` previously defaulted its zip into `logs/`, while
the purge scope was `*.jsonl` only. A user could export a **`raw`-names** session, click "Delete all
log files", get a success toast, and still have a zip full of real branch / tag / file / repo names
on disk. That is precisely the scenario decision 7 exists to prevent, so a "delete" that leaves it
behind is a privacy bug, not a scoping nicety.

**Resolution — both halves, because either alone leaves a hole:**
1. **Exports ALWAYS land in `exports/`; the command takes no destination.**
   `log_export_session()` writes to `<app_config_dir>/exports/` and has **no `dest` parameter**.
   `logs/` holds only the writer's own files, so rotation/pruning never reasons about foreign file
   types — and, because the destination is not caller-supplied, the §6.2 delete-all scope is
   exhaustive by construction rather than by convention.

   **Why `dest` was removed (audit F4, 2026-09-03).** The parameter was justified by a comment
   claiming the path came from an OS save dialog. **No such dialog exists** — no `tauri-plugin-dialog`
   save call was ever wired, and the only caller (`DevCategory.tsx`) passed nothing. A
   `Option<String>` destination reachable from the webview is therefore an **unmediated
   arbitrary-directory-create and file-write primitive**, and it simultaneously punched a hole in the
   delete-all scope this very section exists to close: a zip steered anywhere else is unreachable by
   `logs_delete_all`.

   **RULE, binding on every future increment.** Any "Save as…" for a log/export artifact must obtain
   its path from a **backend-invoked Tauri dialog** (the path is produced inside Rust, by a dialog the
   backend opened) and **never** from a string supplied by the webview. If such a dialog is added,
   §6.2's honest-reporting paragraph below must be extended to cover the new out-of-scope location
   before the feature ships, not after.
2. **The purge covers Bonsai-created export zips** in *both* `exports/` and `logs/` (the latter for
   files written by builds predating this rule, and for a user who steered the save dialog back into
   `logs/`). "Delete all log files" must mean *every log artifact Bonsai put in its own config
   directory*.

**A zip the user deliberately saved into `logs/` or `exports/` is deleted.** That is intended: those
two directories are app-managed, the confirm dialog states the count and bytes before anything is
removed, and the alternative — silently retaining a raw-names archive — is the worse failure. A user
who wants to keep an export saves it **anywhere else** via the dialog.

**Honest reporting, and the limit of the guarantee.** Exports the user saved outside the app config
directory (Desktop, Downloads, a chat upload) are physically unreachable and are **not** deleted.
The confirm copy and the post-export content statement must both say so in one line — e.g. "Exports
you saved elsewhere are not removed." Claiming a completeness the command cannot deliver would be
the same class of defect as the one this section fixes.

### 6.3 Rotation at the part cap — **RATIFIED (2026-08-27): evict the oldest part, never refuse rotation**

**The defect this closes** (found by reviewer during increment 1). The original §6 wording implied
that once `max_parts` files existed the writer would keep appending to the last part. Because
`prune` runs only from `LogWriter::open` — once, at session start — and never prunes the last
remaining group, a single event-storm session had **no size bound at all** and could grow past the
256 MB total cap until the next app launch. That defeats the entire point of having caps.

**Ratified behaviour** (as directed by the orchestrator and implemented in increment 1 — this
section makes it contract, the code stands unchanged):

- Rotation is **never refused**. When the part count for the current session group is already at
  `max_parts`, the writer deletes the **oldest part of that same session group** (scoped by
  file-name prefix, so no other session is ever touched) and then opens the new part.
- Pruning additionally runs **after every rotation**, not only at open, so the 256 MB total cap is
  enforced continuously rather than once per launch.
- Bound restored: a session occupies at most `max_parts × 16 MB` = **128 MB**, always.

**Why evict-oldest rather than the alternatives.** Refusing rotation is what created the unbounded
case. Stopping logging at the cap would silently blind the tool at exactly the moment something is
going wrong. Between "lose the oldest evidence" and "lose the newest evidence", the newest is worth
more: the user's workflow is "it just flickered — here is the log", and the anomaly they are chasing
is at the end of the file. **Never discard the newest evidence** is the invariant; discarding the
oldest is the only remaining lever.

**User-visible consequence, stated plainly because it is a real loss.** A truncated log can no
longer show the **first** occurrence of a bug. For the double-trigger investigation this milestone
exists to serve, that matters: "the effect ran twice on the very first repo open" is unprovable from
a file whose beginning is gone. Two mitigations, both required:

1. **The truncation is recorded in-band, never silent.** Immediately after deleting a part, the
   writer emits a `truncate` record (§3) into the surviving newest part, and every subsequent part
   header carries `truncated: true` + `droppedParts: n`. **The reviewing AI must treat the absence
   of an early record in a `truncated` file as inconclusive, not as evidence of absence** — this
   sentence belongs in `redactionNote`/the reviewer preamble.
2. **Hitting the cap is itself a finding.** 128 MB of JSONL is an event storm by definition, so a
   truncated session will also contain `event-storm` / `watcher-storm` / `render-storm` / `drop`
   records. The remedy the UI should suggest is narrowing capture (`dev.level`, turning off
   `dev.capture-frames` / `dev.capture-react`) and reproducing in a shorter session — not a bigger
   cap.

`truncate` is distinct from `drop`: `drop` is in-memory backpressure (records that never reached
disk); `truncate` is loss of records that **were** on disk. Conflating them would mislead the
reviewer about where the loss happened.

**Increment routing:** the eviction + prune-on-rotation behaviour **shipped in increment 1 and is
ratified as-is**. The additive `truncate` record and the two optional `SessionPayload` fields land in
**increment 7** (the increment that already reopens `writer.rs` for `RollAndPurge`), so nothing
disturbs the in-flight increment 2.

### 6.4 `writeFailed` — the "stopped writing" state machine (RATIFIED, increment 7c follow-up)

One bool, three transitions. It is writer-owned, shared with the sink as an `AtomicBool` so
`log_session_info` reads it without touching the writer thread.

| Event | Effect |
|---|---|
| any failed `write_all` / `flush` | `writeFailed = true` |
| failed `open_part` (rotation blocked: permission loss, path taken, disk full) | `writeFailed = true` **and latch `rotationBlocked = true`** |
| successful `open_part` | `rotationBlocked = false` (clearing is permitted again) |
| successful `flush` **while `rotationBlocked == false`** | `writeFailed = false` |
| successful `flush` while `rotationBlocked == true` | **no clear** — the buffer being flushed is the full previous part |

Consequences, all intended:

- A transient failure on a disk that recovers clears within ~1 s (the writer loop flushes at least
  every 1 s).
- A **persistent** rotation block stays `true` for the rest of the session unless a part opens
  again. It does not flap.
- `rotationBlocked` is **writer-local and never crosses IPC.** Only the bool is exported.
- **Privacy:** the errno, the `io::Error` and the path are dropped at the writer. The UI shows
  generic copy with **no `{reason}` interpolated** — this architecture contract is authoritative
  over the UI contract on that point.

**Consistency rule for any future "sticky until success" flag in `obs/`:** the clear condition must
name the *specific* operation that proves the failure is over, not merely "the next success". A
success on a resource the failure did not touch is not evidence.

### Commands
```rust
#[tauri::command] async fn log_append(state: State<'_, AppState>, records: Vec<LogRecord>) -> Result<(), AppError>;
#[tauri::command] async fn log_session_info(state: State<'_, AppState>) -> Result<LogSessionInfo, AppError>;
#[tauri::command] async fn log_reveal_dir(app: AppHandle) -> Result<(), AppError>;
/// Zips the current session's parts into `<app_config_dir>/exports/` and returns the path.
/// Takes NO destination — see §6.2. A webview-supplied path would be an unmediated
/// directory-create + file-write primitive and would escape the §6.2 delete-all scope.
#[tauri::command] async fn log_export_session(app: AppHandle) -> Result<String, AppError>;
#[tauri::command] async fn logs_delete_all(app: AppHandle, state: State<'_, AppState>) -> Result<LogsDeleteResult, AppError>; // §6.1
#[tauri::command] async fn metrics_snapshot(state: State<'_, AppState>) -> Result<MetricsSnapshot, AppError>;
#[tauri::command] async fn metrics_reset(state: State<'_, AppState>) -> Result<(), AppError>;
```
```ts
export interface LogSessionInfo {
  sessionId: string; dir: string; files: string[]; bytes: number;
  records: number; anomalies: number; dropped: number;
  redaction: RedactionMode;
  /** Session salt, hex. In-process only — never written to a log file or export (§7.2). */
  salt: string;
  /** Total across ALL log files on disk, not just this session — the confirm copy needs it. */
  totalFiles: number; totalBytes: number;
  /** §6.2 — export zips inside the purge scope, so the confirm copy can name them. */
  exportFiles?: number; exportBytes?: number;
  /** §6.3 — parts of THIS session already evicted at the part cap; >0 ⇒ the session is truncated. */
  droppedParts?: number;
  /** §6.4 — true while the writer cannot persist to disk. **A BOOL, never an error string**
   *  (an `io::Error` Display embeds the log path, which must not cross IPC). */
  writeFailed: boolean;
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
| **`span` `op` / `phase` names, phase ms, queue/pool/deadline numbers** | kept (allow-listed symbols, never user data) | kept |
| Component / effect / state-field names (source symbols, not user data) | kept | kept |
| Argument **values** | **elided** → `argsHash` + `argsShape` | `argsHash` + `argsShape`, **plus** an `args` object carrying only the positions named by the per-command allow-list in `src/obs/rawArgPolicy.json` — **default DENY**, name-keyed, scalars only. See `docs/contracts/P91-raw-args-privacy.md`, which is authoritative for this row. |
| Repo path | `repo#<n>` | absolute path |
| File paths | `path#<n>` (extension kept: `path#7.ts`) | real path |
| Branch / tag / ref names | `ref#<n>` (kind kept: `ref#3(branch)`) | real name |
| Commit messages, diff / file contents, blame text, search queries | **never, in either mode** | **never** |
| Author / committer name + email | **never, in either mode** | **never** |
| Remote URLs | `remote#<n>` (scheme+forge kind kept: `remote#1(https,github)`) | full URL, userinfo stripped |
| Commit SHAs | first 7 chars kept (needed to correlate; not identifying on their own) | full SHA |
| Tokens, passwords, `Authorization` headers, PATs, SSH keys | **NEVER, under any setting** | **NEVER** |
| Error messages | scrubbed of paths, refs, URLs and tokens | kept, tokens still scrubbed |
| **Session salt** | **never written to a file or export, in either mode** (§6, §7.2) | **never** |

`raw` mode is precisely why §6.1/§6.2 exist: it is the only mode that puts real names on disk, and
the user must be able to remove them — including from an export zip — on demand.

**Limit of the writer-side guarantee — stated plainly, because it is a real and accepted design
limit.** `obs/raw_args.rs` enforces **shape + vocabulary, not semantics**. It can prove that an
`args` object is keyed by identifier-shaped parameter names, that no key matches the free-text or
credential vocabulary, and that every value is a scalar ≤ `RAW_ARG_MAX_STR` (512) with no newline.
It **cannot** prove that the *content* under `{"targetOid": "…"}` is actually an oid. Any content
that fits an identifier-named key, ≤512 chars and single-line, survives the writer.

The writer is therefore a **backstop against the producer being wrong about shape**, not against the
producer being wrong about meaning. The meaning half has exactly one defence, and it is not in the
writer: the **positional drift guard** in `src/obs/rawArgPolicy.test.ts`, which re-parses the
`IpcApi` declarations and asserts that policy position *i* names the parameter actually declared at
position *i*. A refactor that reorders a signature — relabelling a `message` as a `targetOid` — is
structurally invisible to `raw_args.rs` and is caught only there. **Neither guard may be removed on
the grounds that the other exists.**

### 7.2 Ordinal scheme — **DECIDED: salt-seeded counters; ordinals are side-local**

The three properties previously demanded here were not jointly achievable, and the contradiction is
resolved explicitly:

| | Property | Holds? |
|---|---|---|
| (a) | An ordinal is **stable within a session**: the same value is always `ref#3` in every record emitted by that side | **YES** |
| (b) | Ordinals **differ across sessions**, so two files cannot be cross-linked or dictionary-attacked | **YES** |
| (c) | Frontend and Rust **independently agree** on the same ordinal for the same value | **NO — struck** |

**Why (c) is struck rather than engineered.** A counter assigns ordinals in first-sight order, and
the two sides observe values in different orders, so agreement is impossible without either a shared
mapping (a synchronous IPC round-trip per value — unacceptable on the render path) or a pure
`hash(salt, value) → ordinal`. The hash option was **rejected on privacy grounds**: to be mirrored
synchronously in TS it must be a non-cryptographic hash (FNV-1a-64), and the input space for branch,
tag and remote names is small and highly guessable. Anyone who learned the salt could dictionary-
attack every ordinal in the file, and salt-free structural attacks on a non-PRF are cheap. A counter
leaks **strictly less**: it reveals only first-sight order, never anything about the value — which is
what makes the §7 "safe to hand to a third party" claim true. A cryptographic keyed hash would
restore secrecy but cannot be mirrored synchronously in the frontend fast path.

**Consequences, specified so the two sides can never be confused:**
- **Rust ordinals are bare** (`ref#3`, `path#7`, `repo#1`). **Frontend ordinals are `ui:`-prefixed**
  (`ui:ref#3`, `ui:path#7`). The two namespaces are disjoint by construction, so a merged file can
  never imply that a UI `ref#3` and a Rust `ref#3` are the same branch. This prefix is the whole
  mitigation for the "silently implies two different branches" failure mode, and it is mandatory.
- **The reviewing AI does not need cross-side ordinal identity, and must not attempt to infer it.**
  Correlation across the boundary runs on `trace` / `span` / `seq` — every §5 rule already keys off
  exactly those. A UI `ipc.call` and its Rust `ipc.recv` share a `trace`; that is a stronger join
  than a name match would be.
- `redactionNote` (§7.3) states this in one sentence: ordinals are per-side and per-session; join on
  `trace`/`span`, never on ordinal equality.

```rust
// obs/redact.rs
pub struct Redactor { salt: [u8; 16], ids: DashMap<(Kind, String), u32>, next: AtomicU32 }
impl Redactor {
    /// Returns e.g. "path#7". First sight of a value assigns the next ordinal for its Kind.
    /// The counter's starting point is SEEDED FROM THE SALT, which is what delivers (b).
    pub fn tag(&self, kind: Kind, value: &str) -> String;
    /// Takes ALREADY-CANONICALISED text. There is no Rust canonicaliser and none is to be added
    /// (see the argsHash bullet below).
    pub fn hash_args(&self, canonical_json: &str) -> String;   // 8-hex, salted FNV-1a-64
}
pub enum Kind { Repo, Path, Ref, Remote, Other }
```
- **Salt: 16 random bytes generated per session, held in memory only, never persisted** — not to a
  log file, not into an export zip, not to `usage.json`. The frontend receives it once at boot via
  `log_session_info` (in-process IPC) and uses it to seed its own counter and its `argsHash`.
- A purge roll (§6.1) keeps the session salt: the new file continues the same session's ordinals.
- **`argsHash` — one producer, one canonical form. CORRECTED 2026-08-27 (§13 row 20).**
  `argsHash` is produced by the **frontend only** (`src/obs/ipcProxy.ts` via `src/obs/redact.ts`), as
  a salted FNV-1a-64 digest (8 hex) over a **positional JSON array** canonical form — positional
  because `IpcApi` methods are positional, which is also why `argsShape` is keyed `"0"`, `"1"`, ….
  It appears on `ipc.call` and `ipc.result` and **nowhere else**.
  **Cross-side agreement is neither required nor implemented**, and an earlier revision of this
  section wrongly claimed "a UI call and its Rust arrival hash alike" — §3 has always defined
  `IpcRecvPayload` as `{ cmd }` with no `argsHash`, §3 is authoritative, and that sentence is struck.
  `Redactor::hash_args` takes an already-canonicalised `&str`; **no Rust canonicaliser exists.**
  - **PROHIBITION (binding on increment 3 and every later increment): `ipc.recv` must NOT gain an
    `argsHash` (or `argsShape`) field.** A Rust canonicaliser would naturally serialise a *named
    payload map*, producing different canonical text for the same logical call. That second canonical
    form would make `dup-ipc` **silently stop matching real double triggers** — the precise failure
    this milestone exists to detect, and invisible because the rule would simply go quiet.
  - **If a future increment genuinely needs a Rust-side `argsHash`, BOTH of the following are
    required — this is not an either/or:** (1) it must adopt the **identical positional-array
    canonical form** as `src/obs/redact.ts`, pinned by the existing cross-side vectors at
    `src-tauri/src/obs/tests_redact.rs:203-211`; **and** (2) `dup-ipc` must already be filtering
    explicitly on `kind === 'ipc.call'` (§5), which is mandated now and independently of any such
    change, so the rule cannot be broken by a payload gaining a hash.
  - `argsHash` powers `dup-ipc` without storing content, and it remains the only mechanism serving
    both §5 and §7.
  **FNV-1a-64 is adequate here and only here**, for two stated reasons: equality is the only
  property required of it, and the salt never leaves the process, so a file on its own carries no
  key with which to brute-force the digest. It is **not** a concealment primitive — no future change
  may reintroduce a value into the log on the grounds that "it is only a hash".

#### 7.2.1 Token scrubber — **RATIFIED (2026-08-27)**, constants named

The scrubber runs **last, on every string field, in both redaction modes**. Two independent layers;
a miss in one must not depend on the other.

**Layer A — keyword/prefix-established patterns (length-independent where the keyword is
unambiguous). Increment 1's MUST-FIX round already built this layer out; the table below is the
authoritative status so no later increment re-adds working code.**

| Pattern | Status | Where |
|---|---|---|
| `TOKEN_PREFIXES` as **`(prefix, min_len)` pairs** — the mechanism that lets short tokens be caught under the global floor | **SHIPPED (inc. 1)** | `obs/redact.rs:190` (doc comment names the `glpat-` 26-char case), table at `:213` |
| `glpat-` (12), plus `gldt-`, `glrt-`, `ghp_`/`gho_`/`ghu_`/`ghs_`/`ghr_`, `github_pat_`, `npm_`, `AKIA`/`ASIA`, `AIza`, `sk-`/`sk_live_`/`rk_live_`, `dckr_pat_`, `xox[baprs]-`, `xoxe-` | **SHIPPED (inc. 1)** | `obs/redact.rs:213` |
| `Bearer <token>` **and `Basic <token>`** | **SHIPPED (inc. 1)** | `obs/redact.rs:382` |
| Looser length rule for keyword-established credentials (a `Basic` credential is only ~24 chars) | **SHIPPED (inc. 1)** | `obs/redact.rs:304-305` |
| `://user:pass@` URL userinfo; `-----BEGIN … PRIVATE KEY-----`; `is_sensitive_key` on JSON keys matching `/token|secret|password|passphrase|auth/i` | **SHIPPED (inc. 1)** | `obs/redact.rs:232-243` for the key rule |
| **`eyJ[A-Za-z0-9_-]{10,}\.` — JWT header prefix** | **OPEN → increment 3** | new |
| **`key=value` / `key: value` credential pairs matched INSIDE a plain-text string body**, using the same sensitive-key vocabulary as `is_sensitive_key` | **OPEN → increment 3** | new |

**Do not disturb the `basic` guard.** `redact.rs:377-394` deliberately treats `bearer` and `basic`
asymmetrically: `basic` additionally requires the following word to look like credential material,
because "basic" occurs in ordinary prose ("basic auth is disabled") where swallowing the next word
would corrupt an error message for zero privacy gain. Both directions are tested. Any increment-3
work on Layer A must leave that asymmetry intact.

**Why the two open items are genuinely uncaught, and why they are worth the pass:**
- **JWTs.** A `.` disqualifies a Layer-B candidate and no shipped Layer-A prefix matches, so a JWT
  in an error body or an `Authorization` value that lost its `Bearer ` keyword passes through today.
  The `eyJ` prefix is base64 of `{"`, so it is high-precision — near-zero false-positive risk.
- **In-string `key=value` pairs.** `is_sensitive_key` (`redact.rs:232-243`) is **key-name-based**, so
  it only fires on structured JSON keys. Credential-helper and `.netrc` output is line-oriented text
  that arrives inside a *single string value*, where nothing currently inspects it. This is the
  highest-value remaining gap for a Git client, because it is the shape a real credential actually
  takes when git hands it back to us.

**Layer B — generic high-entropy heuristic** (the ratified base64/PAT shape, for credentials with no
recognisable keyword or prefix). Increment 1's original form rejected any candidate containing `/`,
which made base64 secrets containing `/` structurally uncatchable while §7.2 explicitly demanded the
"base64 PAT shape". The implemented replacement is **ratified with these named constants**:

| Constant | Value | Role |
|---|---|---|
| `MIN_SECRET_LEN` | **32** | total candidate length floor (was 40; lowered to reach Azure DevOps / base64 shapes) |
| `MIN_B64_RUN` | **24** | a candidate containing `/` qualifies only if some `/`-free run is ≥ this |

plus: an **alpha + digit mix** is required, and **pure hex is excluded** (so 40-char SHAs survive).
Note that Layer B is *not* the path by which short keyword-established credentials are caught —
`redact.rs:304-305` handles those — so its 32-char floor is not a recall gap.

**Why the run test is sound.** Random base64 hits `/` about once per 64 characters, so a real secret
almost always contains a ≥24-char `/`-free run; path segments are human-named and rarely reach 24
characters, and when they do (`useRepoChangeSubscription` is 25) they are word-shaped with no digits
and so fail the alpha+digit requirement. Verified both directions in increment 1: a base64 secret
with an embedded `/` is caught; `/home/developer/projects/bonsai/src/components/settings/categories`
is untouched. **32 and 24 are ratified as the right constants** — 32 is the shortest *unkeyworded*
credential shape Bonsai handles (Bitbucket app passwords, Azure DevOps PATs), and dropping
`MIN_B64_RUN` below 24 starts colliding with long camelCase path segments for no recall gain.

**Accepted consequences, all fail-safe.** scp-style remotes (`git@host:o/r.git`) ordinalise as
`path#` rather than `remote#`; UUID-shaped strings ≥32 chars over-redact; a long camelCase path
segment that happens to contain a digit may over-redact. **This trade is deliberate: a missed
credential is unrecoverable, while an over-redacted path costs only reviewer legibility.** Precision
is subordinate to recall here, permanently.

### 7.3 Mode disclosure & UI statement

- The `session` header record carries `redaction` and a `redactionNote` string, so a reviewer
  opening the file **immediately knows what they are looking at** without external context. The
  note includes the §7.2 sentence about side-local ordinals **and, when `truncated` is set, the
  §6.3 sentence that early records are missing and absence is not evidence of absence**.
- `dev.include-raw-names` (the `raw` toggle) requires an explicit confirm dialog and shows a
  persistent warning row while on. Turning it on starts a **new** log file (a single file never
  mixes modes).
- The Settings Dev page shows a fixed, always-visible statement of log contents. Copy is
  ui-designer's; the *content* is exactly the §7.1 table plus: "Logs are written only to this
  computer. Bonsai never uploads them." plus a pointer to the delete action **and the §6.2 line
  that exports saved outside Bonsai's folder are not removed**.

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
- **Key namespace — three independent gates, see §8.2.** Key *names* are enumerated in
  `obs/metrics.rs`; the *shape* predicates live in `obs/metrics_keys.rs`; the *`cmd.<name>`
  allow-list* lives in `obs/metrics_cmds.rs`; the *cardinality cap* lives in `obs/metrics_map.rs`.
  No user-derived string can become a key (privacy + unbounded-growth guard). Metrics therefore need
  no redaction: they **structurally cannot contain repo content**, which is why they are not covered
  by `logs_delete_all`.
- **No network sink exists.** No HTTP client is reachable from `obs/*`; a test asserts it.
- Read API for the future Statistics page: `metrics_snapshot()`. **No UI is designed in P91.**
- **"Always on" clarified (RATIFIED, increment 6).** Only the **counters** are truly always-on:
  `perf.*` (folded from `PerfState::snapshot()` every launch) plus `sessions`/`first_seen`. The
  **duration histograms** (`op.*`, `cmd.*`, `queue.blocking`) accumulate **only during Dev-mode
  sessions**, because their source records (`span`, `ipc.result`) exist only then — de-gating them
  would require emitting spans with Dev mode off, violating §11's zero-cost-off budget. This is the
  correct consequence of the architecture, not a defect: the always-on aggregate is usage/error
  frequency; latency distributions are a Dev-mode artefact.

### 8.1 Duration percentiles — bounded summaries, no sample retention (ADDITIVE; **increment 6**)

**No new summary type.** `Histogram` *is* the durable summary: 8 counters + `count`/`sum_ms`/
`max_ms` per key per day — fixed size, bounded by the allow-listed key set. Bucket boundaries are
**frozen** (changing them would break existing files) and no raw sample is ever stored.

Additive changes only:

```rust
impl Histogram {
    /// Linear interpolation inside the containing bucket; the top bucket returns `max_ms`.
    /// p in 0.0..=1.0. Returns None when count == 0.
    pub fn percentile_ms(&self, p: f32) -> Option<u32>;
    pub fn mean_ms(&self) -> Option<u32>;   // sum_ms / count
}
```

**Authoritative percentile method (RATIFIED, increment 5).** `percentile_ms` uses **linear
interpolation inside the containing bucket, then clamps the result to `max_ms`**. This is the single
method both §5.1's in-memory `slow-command` baseline and §8.1's durable `metrics_snapshot`
percentiles use — they must agree. The clamp only moves the estimate toward truth (a percentile can
never exceed the observed maximum), so §8.1's "coarse but within one bucket width" bound is
preserved. Worked case from the increment-5 review: 50 observations of 900 ms all land in the coarse
`(500, 2000]` bucket; unclamped interpolation yields p95 ≈ 1925 ms (so a `slow-command` threshold of
`3 × p95 = 5775` would swallow the §12(a) 4 s acceptance outlier), whereas clamping to `max_ms = 900`
gives p95 = 900 and threshold 2700, which fires correctly.

```rust
// New OPTIONAL, DERIVED fields — computed at snapshot time, never persisted to usage.json.
#[serde(default, skip_serializing_if = "Option::is_none")] pub p50_ms: Option<u32>,
#[serde(default, skip_serializing_if = "Option::is_none")] pub p95_ms: Option<u32>,
```
```ts
export interface Histogram {
  count: number; sumMs: number; maxMs: number; buckets: number[]; // 8
  p50Ms?: number; p95Ms?: number;   // present only on metrics_snapshot() results
}
```

**Empty-bucket skip (increment-5 follow-up, non-ratified surface).** `percentile_ms` skips buckets
with a zero count when choosing the containing bucket, except the top bucket, which is the
guaranteed terminus. This is **inert for every `p` in `(0, 1]`** — the first bucket reaching the
target is necessarily non-empty — so the ratified p50/p95 method is unchanged. It fixes `p == 0.0`,
which previously stopped in an empty bucket 0 and fell through to the `max_ms` fallback, reporting
the **maximum** as p0. The remaining `bucket_count == 0` path is reachable only from a torn or
hand-edited `usage.json` that deserialized with `count > 0` and no bucket set; it returns `max_ms`
rather than dividing by zero. p0 is not part of any rule or snapshot field; this is a bug fix
outside the ratified surface, recorded so a later session does not read the guard as drift.

**Which keys get a duration histogram** (names in `obs/metrics.rs`, shape guard in
`obs/metrics_keys.rs`, folded at flush from
`span` and `ipc.result` records):
- `cmd.<name>` for every non-excluded command — end-to-end IPC duration;
- `op.graph.get`, `op.status.scan`, `op.diff.compute` — total operation time;
- `op.graph.get.<phase>` for the five §3.1.2 graph phases, plus `op.status.scan.statuses` and
  `op.diff.compute.hunks` — the sub-keys that answer "which stage got slower";
- `queue.blocking` — `queuedMs` across all spans.

Ceiling: ~60 histogram keys × ~50 bytes ≈ 3 KB per day bucket; 400 days ≈ 1.2 MB worst case, and
`lifetime` folding keeps the tail flat. "Did this get slower over the last week" is answered by
comparing `p95_ms` across `days[]` — no new storage, no new file.

### 8.2 Metric-key admission — shape, allow-list, cardinality (RATIFIED 2026-09-03)

Three gates, applied in this order. Each is independently necessary; none is a size split.

| Gate | Home | Decides |
|---|---|---|
| **G1 shape** | `obs/metrics_keys.rs` | Is the string *shaped* like a key at all? Cheap first filter. **Never sufficient alone** — it accepts unlimited well-shaped strings. |
| **G2 vocabulary** | `obs/metrics_cmds.rs` | For `cmd.<name>`: is `<name>` an actual `IpcApi` method? Exact membership, default deny. |
| **G3 cardinality** | `obs/metrics_map.rs` | Has this map already minted `MAX_KEYS_PER_MAP` keys? If so, fold into `meta.overflow`. |

#### G1 — shape predicates (`obs/metrics_keys.rs`)

```rust
pub(super) fn is_valid_cmd_name(name: &str) -> bool;    // non-empty, <=40, starts ascii-lowercase,
                                                        // [a-zA-Z0-9_]* — camelCase ADMITTED
pub(super) fn is_valid_counter_key(key: &str) -> bool;  // non-empty, <=60, MUST contain '.',
                                                        // no leading/trailing/doubled '.',
                                                        // chars in [a-z0-9_.]
pub(super) fn is_valid_err_code(code: &str) -> bool;    // non-empty, <=48, [a-zA-Z0-9_.-]
```

**`is_valid_counter_key` requires the `<domain>.<action>` dot literally** — tightened beyond the
audit finding. Without it a bare lowercase token (an oid, an id, a `ghp_…` token) is shape-valid,
and would have been persisted the instant the guard began running in a release build.

**`bump_validated` is the single sink.** Both counter writers — `bump_counter` (the public door for
future `<domain>.<action>` counters) and `fold_perf` (the one production writer today) — pass
through it, and it is a **runtime `if`, not a `debug_assert`** (release builds compile those out).
Only with the single sink in place is "shape-valid decides persistence" true of the path that
actually persists; before it, `fold_perf` wrote unvalidated.

```rust
// obs/metrics.rs — drops the observation and returns false when the key is rejected.
#[must_use] fn bump_validated(t: &mut MetricTotals, key: &str, n: u64) -> bool;
```

#### G2 — the `cmd.*` allow-list (`obs/metrics_cmds.rs`)

```rust
/// Every `IpcApi` method name, sorted. Exact bijection with the interface declarations.
pub(super) const KNOWN_CMDS: [&str; 199] = [ /* generated */ ];
pub(super) fn is_known_cmd(name: &str) -> bool;   // binary search; once per `ipc.result`
```

- Generated from the `IpcApi` declarations in `src/ipc/types/ipc-api.ts`, `ipc-api-forge.ts`,
  `ipc-api-obs.ts`. `obs/ipcProxy.ts` uses the **method name** as `cmd`, so these are **camelCase**,
  not the snake_case Tauri command names.
- `tests_metrics_cmds.rs` re-derives the list from those `.ts` files at test time and fails on any
  drift, so adding an IPC method without regenerating the table is caught by `cargo test`, not by a
  silently-missing histogram.
- The §2.3-excluded methods (`logAppend`, `metricsSnapshot`, …) are present for the exact-mirror
  property; the proxy never instruments them, so they can never arrive.
- `metrics::observe_ipc_result` requires **G1 and G2**. An unknown name drops the observation.

**FAILURE MODE, recorded because the fix alone does not teach it.** `is_valid_cmd_name` originally
required **all-lowercase**, while the producer has always sent **camelCase** method names. Result:
**the entire `cmd.*` histogram family recorded nothing in production** — every real observation was
rejected by the shape gate — and the feature was silently, completely inert. It was green the whole
time because the Rust tests fed the validator **snake_case** names (`get_status`, `commit`), which
pass all-lowercase. *Tests and production fed different-shaped inputs to the same validator.*

> **Binding rule.** A validator's tests must use inputs **produced by the real producer**, not
> hand-written plausible ones. Where the producer lives on the other side of the IPC boundary, pin
> the vocabulary with a **drift test that re-derives it from the producer's own source**
> (`tests_metrics_cmds.rs` is the model), so the two sides cannot diverge silently. A predicate that
> rejects everything is indistinguishable from a predicate that works, unless something asserts a
> real input is **accepted**.

#### G3 — cardinality cap (`obs/metrics_map.rs`) — RATIFIED as built

```rust
pub const MAX_KEYS_PER_MAP: usize = 512;
pub const OVERFLOW_KEY: &str = "meta.overflow";   // <domain>.<action> shaped: passes every G1
                                                  // predicate, cannot collide with a real key
```

Applied to **`counters`, `durations`, `errors`** in every `MetricTotals` — day buckets **and** the
400-day→`lifetime` roll-up. Past the cap a map stops minting keys and folds every further
observation into `meta.overflow`, so the **count is never lost** (an operator reading `usage.json`
can tell a cap was hit) while the key set stays bounded at `MAX_KEYS_PER_MAP + 1` per map per bucket.
An existing key always keeps recording under its own name.

**DECISION — no global cap. The per-map-per-bucket cap is the whole mechanism.** The reviewer is
right that worst-case cardinality is `400 days × 3 maps × 513` ≈ **616k keys**, not 513. That is
accepted, for three reasons:

1. **The cap is a runaway stop, not a sizing parameter.** Realistic bound is
   `199 cmd.* + ~11 op/phase + queue.blocking + meta.overflow` ≈ **212** duration keys, plus a few
   dozen `counters` and `errors` — every one of them allow-listed. 512 sits deliberately *above* the
   reachable set so a healthy build never overflows and `meta.overflow` is itself a signal.
2. **A global cap would be strictly worse.** It makes *today's* recording depend on *history*: once
   the global budget is spent, a long-lived install stops minting new keys forever and dumps the
   current day's real activity into `meta.overflow`, destroying exactly the week-over-week comparison
   §8.1 exists to serve. Per-bucket capping keeps every day independently readable.
3. **Total file size is a different problem with a different lever.** If a real `usage.json` ever
   exceeds ~8 MB, the remedy is a **size-triggered early roll-up** — fold the oldest `days[]` into
   `lifetime` before the 400-day age trigger, reusing the roll-up that already exists — not a key
   cap. This is a **revisit trigger, not work in P91**; nothing is to be built for it now.

**§8.1 ceiling corrected.** §8.1's "~60 histogram keys ≈ 3 KB per day bucket; 400 days ≈ 1.2 MB"
predates the `cmd.<name>` family being enumerated. The bound is ~212 keys ≈ **~11 KB/day**, ≈ **4.3 MB**
over 400 days if a user invoked every command every day in Dev mode; in practice far lower, since
duration histograms accumulate **only during Dev-mode sessions** (§8, "always on clarified"). The
`lifetime` fold keeps the tail flat either way.

---

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
- **`briefString` gates on field NAME in BOTH modes (corrected 2026-09-03).** `useStateTransitionLog`
  writes `from`/`to` through `briefString(field, value)`. Its raw branch previously returned up to 48
  characters **verbatim for any field**, while its doc claimed the hook was "safe to wire to a repo
  store BY CONSTRUCTION" — true in `strict` (which ordinalises) and false in `raw`. It now applies
  the shared vocabulary in **both** modes: if `isFreeTextParam(field) || isSensitiveParam(field)`
  (`message`, `msg`, `query`, `search`, `text`, `token`, `password`, …), the value is ordinalised or
  reported as `str:<len>` — never echoed — regardless of redaction mode. Otherwise `raw` returns at
  most 48 chars and `strict` ordinalises. The 48-char cap applies either way.
- **Why the gate must be NAME-based here specifically.** `state` records ride **outside** the
  `raw_args` backstop: writer rule W1 scopes `args` enforcement to `kind == "ipc.call"`, so a
  `StatePayload.from`/`to` is never inspected by `obs/raw_args.rs`. `briefString` is the *sole* gate
  on that path, which is why it uses the same vocabulary as A26 rather than a value heuristic —
  over-classification (`origin/main` reported as a shape) is the deliberate failure direction.

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

### 9.4 Interaction latency (gesture → visible result) — **DEFERRED, with reason**

Considered per the amendment and **not shipped in P91**. A true gesture→paint measure needs a
`requestAnimationFrame`-after-commit probe wired into every instrumented surface plus a way to know
which paint *is* the result — new plumbing in all six surfaces, and unreliable in the headless
harness (the Browser pane is 0×0, so rAF never fires; see the frame-timing note in §11).

The existing triangulation is sufficient for the complaints that motivated P91: `gesture` (t0) →
`ipc.call`/`ipc.result.ms` (backend cost) → `span.phases` (where inside it) → `render.tally`
(did the UI churn) → `frame.worstMs` + `jank-trace` (did a frame drop). If, after increment 5, a
real complaint cannot be explained by that chain, revisit as a follow-up.

---

## 10. Settings surface (behaviour only; visuals = ui-designer)

New category `'dev'` (rail last, `dividerBefore: true`), rows:

| Row id | Control | Effect |
|---|---|---|
| `dev.enabled` | switch | master Dev-mode gate; OFF ⇒ no instrumentation active, no file writes, **no automatic log deletion** |
| `dev.level` | segmented | `info` / `debug` / `trace` (`trace` force-enables frame capture) |
| `dev.capture-ipc` | switch | ipc/event/channel/span records (default on) |
| `dev.capture-react` | switch | render/render.tally/effect/state records (default on) |
| `dev.capture-frames` | switch | frame records (default off — high volume) |
| `dev.include-raw-names` | switch | `strict` → `raw` (§7). Default **off**. Confirm dialog on enable; persistent warning row while on; starts a new log file |
| `dev.reveal-logs` | button | `log_reveal_dir()` |
| `dev.export-session` | button | `log_export_session()` → writes a zip into `exports/` (§6.2). **No save dialog and no path argument**; re-shows the content statement first, including that exports the user later copies elsewhere are not covered by the delete action |
| `dev.delete-logs` | button (destructive) | **§6.1/§6.2** — confirm dialog first, stating `totalFiles` / `totalBytes` **and `exportFiles` / `exportBytes`** from `log_session_info`; then `logs_delete_all()`. Reports the result honestly: success count + bytes (naming exports separately via `deletedExports`), and a warning state when `failedFiles > 0`. When Dev mode is ON, the copy states that logging continues into a new, empty file |
| `dev.session-info` | readonly | `LogSessionInfo` summary (files, size, records, anomalies, dropped, redaction mode). **When `droppedParts > 0`, shows a warning line: the session hit its 128 MB cap and its earliest records were discarded (§6.3), with the suggestion to narrow capture and reproduce in a shorter session.** Refreshes after a delete |
| `dev.privacy-note` | readonly | the fixed §7.3 statement of what a log file contains |

**No new setting row for spans.** `span` records ride the existing `dev.capture-ipc` gate (they are
backend operation records, ≤1 per heavy op — see §11).

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
| Dev mode **OFF** | ≤ **1 %** on graph scroll frame time; **zero** allocations per IPC call beyond today; no writer thread spawned; no `useEffect` dep copies; no render tally allocated; **`PhaseRecorder::start` allocates nothing and `phase()` takes no `Instant`** | perf_gate case asserting sink-disabled paths do no work; vitest asserts `instrumentIpc(api).someCmd === api.someCmd` **while disabled at access time**; `cargo test` asserts a disabled recorder emits no record and holds an empty `Vec` |
| Dev mode **ON** | ≤ **5 %** frame-time regression on a 20k-commit scroll; ≤ 2 ms added per IPC call; ≤ 1 `log_append` per 500 ms | perf_gate case with the sink enabled + `unbatched-sink` must not fire in the harness run |
| Dev mode **ON, spans** | ≤ **5 µs** per operation; ≤ **16** phases per span; **exactly 1 `span` record per completed operation** — a 20k-commit repo open produces ≤ 5 span records (~1 KB total), a graph scroll session ≤ 1 per served `get_graph`. Phase timing must never appear on a per-commit or per-file path | `cargo test`: a `graph.get` produces exactly one `span` with ≤16 phases; a bench asserts recorder overhead < 5 µs; grep test asserts no `PhaseRecorder` use inside a loop over commits/files |
| Dev mode **ON, anomaly detector** | **"Bounded" is literal and every piece pays for it:** (a) `mutations` and each `Sliding` event list are pruned to their window; (b) **each `Sliding::last_fire` debounce map is pruned to the same window as its events** — mandatory, because `dup-ipc` keys on `cmd\0argsHash`, an unbounded space where every distinct argument set mints an entry; (c) `open_calls` is FIFO-capped at 1024; (d) the §5.1 per-`cmd` baseline map is LRU-capped at **200 keys**; (e) `slow_last_fire` is keyed by `cmd`, a finite catalogue, and needs no prune. Total ≈ tens of KB, independent of session length. | `cargo test`: 10k distinct cmd names keep the baseline map at the cap; **2000 distinct arg hashes over 200 s of session time leave the `dup-ipc` debounce map within one window's worth of keys**; a long synthetic session leaves every window list at window size |
| Dev mode **ON, sidebar** | a single ref change on a **500-ref** repo produces **≤ 8** react records total (container `each` + 4 aggregate tallies), never one per row | vitest: mount the sidebar with a 500-ref fixture, trigger one ref change, count records |
| **Disk, per session** | **hard bound `max_parts × 16 MB` = 128 MB**, enforced continuously by §6.3 eviction + prune-on-rotation — never only at launch | `cargo test`: drive a writer past `max_parts` and assert the group never exceeds the bound, the oldest part is the one deleted, the newest part survives, and no other session group is touched |
| Sink | never blocks a caller (bounded `try_send`), writer thread only | test: fill the channel, assert producers return immediately and a `drop` record appears |
| `logs_delete_all` | runs on `spawn_blocking`; UI never blocks; no log record is lost between the flush and the new file opening | test: enqueue records concurrently with a purge, assert none are lost after the roll |
| Redactor | ≤ 5 µs per record (`DashMap` hit + one salted FNV-1a-64 hash) | bench in `obs/redact.rs` tests |
| Metrics percentiles | `percentile_ms` is O(8); no sample buffer exists; a day bucket stays ≤ ~3 KB | `cargo test`: 1M observations leave the histogram byte-size unchanged |

---

## 12. Delivery decomposition

Seven increments, dependency-ordered; each sized for one fresh-context senior-dev pass. 1–3 build
the pipe, **4 is the milestone's payload**, 5 makes it self-analysing, 6–7 are the durable-metrics
and user-surface tails.

**Amendment routing (2026-08-27):** increment 1 is **unchanged** in scope — it does not implement
§3.1. The `span` kind, `SpanPayload` and `obs/phase.rs` land in **increment 3**; the
duration/saturation rules in **increment 5**; the percentile API in **increment 6**.

**Post-increment-1 ratifications (§13 rows 16–19).** Increment 1 is committed (`1b94529`); nothing
below asks for it to be rewritten:
- §7.2 **confirms** its salt-seeded counters. §7.2.1 **ratifies** its Layer-B heuristic and constants
  (`MIN_SECRET_LEN = 32`, `MIN_B64_RUN = 24`) **and records its Layer-A prefix/keyword table — incl.
  `glpat-` and `Basic` — as already shipped.** Only two Layer-A items remain open.
- §6.3 **ratifies** its rotation-eviction + prune-on-rotation behaviour.
- Remaining deltas are all **additions**: the `ui:` ordinal prefix (increment 2), the **two** open
  Layer-A patterns (increment 3), the `truncate` record + optional `SessionPayload`/
  `LogSessionInfo` truncation fields (increment 7), and the widened purge scope (increment 7).
  `log_export_session`'s `exports/` default and `LogSessionInfo.salt` were folded into increment 1.

**Post-increment-2 correction (§13 row 20).** `argsHash` has **one** producer (the frontend proxy)
and **one** canonical form (positional JSON array). §7.2's old "hash alike" sentence was wrong and is
struck; §3's `IpcRecvPayload { cmd }` was always authoritative. Two binding consequences below:
increment 3 must **not** add `argsHash` to `ipc.recv`, and increment 5 must make `dup-ipc` filter
explicitly on `kind === 'ipc.call'`.

| # | Increment | UI? | Scope | Acceptance |
|---|---|---|---|---|
| 1 | **Log core (Rust)** — `obs/record.rs`, `redact.rs`, `sink.rs`, `writer.rs`, `commands/obs.rs` (`log_append`, `log_session_info`, `log_reveal_dir`, `log_export_session`), `DevSettings` + all 8 lockstep files, mock IPC stubs. **No §3.1 work.** | no | schema, redaction, sink, rotation, pruning, flush-on-exit, settings plumbing | `cargo test`: rotation at cap **evicts the oldest part of the current group and never refuses rotation (§6.3), prune runs after each rotation, and the session stays ≤128 MB**; pruning keeps N; backpressure emits `drop`; exit flush loses 0 records; `Redactor` assigns stable ordinals within a session and different ones across sessions (§7.2 (a)+(b); **(c) is explicitly NOT required — do not test for cross-side agreement**); the §7.2.1 scrubber catches every shipped Layer-A pattern **and every fixture in the `(prefix, min_len)` table**, keeps the `basic`-in-prose negative case, and leaves a long real path untouched; **turning Dev mode off deletes no file**. Enabling Dev mode creates a `logs/*.jsonl` whose first line is a valid `session` record naming the redaction mode. **§6.2:** `log_export_session(None)` writes into `<app_config_dir>/exports/` and a test asserts **no `.zip` is ever created inside `logs/`**; `log_session_info` returns the session `salt` and, when exports exist, `exportFiles`/`exportBytes`; a test asserts the salt appears in **no** file on disk |
| 2 | **Frontend pipeline** — `obs/{types,enabled,trace,redact,log,batcher,ipcProxy}.ts`, wire into `src/ipc/index.ts`, mock ring buffer + `__bonsaiDumpLogs()` | no | proxy, trace minting, redaction mirror, batching, **the system's sole `argsHash` producer** | vitest: a mock-IPC call produces paired `ipc.call`/`ipc.result` sharing one trace + one span; disabled ⇒ method identity preserved; ≤1 `log_append` per 500 ms; a call with a path argument logs `argsHash`+`argsShape` and **no path substring**. **§7.2:** every frontend ordinal is `ui:`-prefixed (regex test over a full fixture run — a bare `ref#`/`path#`/`repo#` from `src: 'ui'` is a failure); the canonical form is a **positional JSON array** with `argsShape` keyed `"0"`,`"1"`,…; ordinals are stable within the session |
| 3 | **Rust dispatch + events + watcher + §3.1 spans + the TWO open §7.2.1 Layer-A patterns** — `invoke_shim.rs` (§2.3, incl. the §2.3.1 contingency note in the module doc), `trace.rs`, **`obs/phase.rs` + the `span` `LogKind`/`SpanPayload` (additive to `record.rs`) + the three §3.1.2 call sites + `queuedMs`/pool gauge in `repo_handle.rs` + `deadlineFrac` from `run_with_git_timeout*` + `cache` from `graph_cache.rs`**, migrate every `emit(` site to `emit_logged` with an explicit `TraceMeta`, watcher batch/debounce records, mock `span` fixtures, **the `eyJ…` JWT prefix + in-string `key=value` credential pairs** | no | backend choke point + intra-operation breakdown | every dispatch yields an `ipc.recv` stamped with the frontend-injected `__trace`; **`ipc.recv` carries `cmd` + trace ids ONLY — adding `argsHash`/`argsShape` is PROHIBITED (§7.2), and a test asserts the emitted `ipc.recv` JSON has no `argsHash` key**; no Rust args canonicaliser is introduced; no `emit` site remains unmigrated (grep test); a git-op burst yields watcher `fired` records with correct `paths`/`relevant`/`debounceMs`. **Spans:** (a) a `get_graph` on a fixture repo emits exactly **one** `span{op:'graph.get'}` whose `phases` cover `revwalk`/`decorate`/`lane` and whose phase sum ≤ `ms`; (b) `queuedMs` is present and ≥0 on every git span, and a test that saturates the blocking pool shows `queuedMs > 0` and `poolInflight >= poolMax`; (c) a forced near-timeout yields `deadlineFrac ≥ 0.8`; (d) a cache-served graph emits `cache:'hit'` with no `revwalk` phase; (e) recorder overhead bench < 5 µs; (f) `bonsai-core` gains **no** dependency on `obs` (compile test). **§7.2.1 — exactly two additions, nothing else in Layer A is touched:** a JWT (`eyJ…`) and a plain-text `password=…` credential-helper line are both redacted; **`glpat-` and `Basic` are ALREADY SHIPPED — do not re-add them**, and the `redact.rs:377-394` `basic`-in-prose asymmetry must survive unchanged (its existing negative test still passes); a 40-char SHA and a long real path still untouched. **If `tauri::ipc::Invoke` will not compile, execute §2.3.1 (drop the shim) and drop only the `ipc.recv` criterion — spans are unaffected — do not modify command signatures** |
| 4 | **Refresh + echo causality + React causality across the SIX surfaces** — `armEcho(repoId, trace)`, suppressed-watcher record at `useCoalescedRefresh.ts:82`, `pendingTracesRef` + `refresh` records, `obs/react.ts` + `obs/renderTally.ts`, applied per the §9.3 table (incl. the sidebar), `frameStats` routing | **yes** | **the flicker evidence** | (a) a mutation followed by its fs echo yields a `watcher` record with `suppressed:true` and `causedBy` = the mutation's trace; (b) one mutation ⇒ exactly one `refresh` record listing every collapsed contributing trace; (c) an effect re-run with unchanged deps emits `effect-no-change`; (d) **sidebar: one ref change on a 500-ref fixture yields ≤8 react records — one `each` record for `Sidebar.tsx` plus one `render.tally` per section/row component — and zero per-row records**; (e) a sidebar flicker (repeated re-render with no ref change) shows as a `render.tally` with `renders > 3 × instances` |
| 5 | **Anomaly detector** — `obs/anomaly.rs`, every sink-side rule in §5 incl. `render-storm` **and the §5.1 performance rules** (`slow-command`, `slow-phase`, `queue-delay`, `pool-saturation`, `watchdog-pressure`, `cache-collapse`) + the `SLOW_RULES` constants table | no | derived records | scripted double-click in the harness produces a `dup-ipc` anomaly whose `refs` point at the two `ipc.call` seqs; each rule has a unit test with a true-positive and a true-negative case; **`dup-ipc` filters EXPLICITLY on `kind === 'ipc.call'` and a test feeds a synthetic non-`ipc.call` record carrying an `argsHash` and asserts it does not contribute (§5)**; tests assert no rule reads a redaction ordinal (§7.2) and no rule consumes `truncate` (§6.3). **§5.1 specifically:** (a) a synthetic stream of 50 normal `get_graph` results at 900 ms followed by one at 4 s fires **exactly one** `slow-command`; (b) the same 50 results at a *uniformly* high 900 ms fire **none** (large-repo non-firing test); (c) fewer than `MIN_SAMPLES` results fire nothing except the >10 s catch-all; (d) the rate limit caps repeats at 1 per cmd per 10 s; (e) a slow span dominated by `lane` fires `slow-phase{phase:'lane'}` referencing both seqs; (f) 5 spans with `cache:'redecorate'` and no mutation fire `cache-collapse`, and the same 5 *with* an intervening mutation fire nothing; (g) 3 spans with `queuedMs 150` in 5 s fire `queue-delay`; (h) baseline map stays at the 200-key cap |
| 6 | **Metrics** — `obs/metrics.rs`, `metrics_file.rs`, `perf.rs` absorption, `metrics_snapshot`/`metrics_reset` + mock, **§8.1 `percentile_ms`/`mean_ms` + the derived `p50Ms`/`p95Ms` snapshot fields + folding `span` phase durations into the allow-listed `op.*` histogram keys** | no | durable local aggregates | counters survive restart; `.bak` recovery on a corrupt file; daily bucketing correct across a simulated date change; `perf` deltas appear as `perf.*`; test asserts `obs/` reaches no HTTP dependency and that no metric key is user-derived; **`metrics_reset` exists as a command and appears in no catalog row**. **§8.1:** (a) `percentile_ms` matches a brute-force reference within one bucket width on 10k synthetic samples; (b) 1M observations leave `usage.json` byte-size unchanged (no sample retention); (c) `p50Ms`/`p95Ms` appear on `metrics_snapshot()` output and are **absent** from `usage.json` on disk; (d) `op.graph.get.lane` accumulates from `span` phases across two days and the two days' `p95Ms` are independently comparable; (e) the histogram key set stays within the allow-list |
| 7 | **Settings Dev page + `logs_delete_all` + §6.3 truncation record** — the `RollAndPurge` writer path + `logs_delete_all` command + mock, **the `truncate` record + `SessionPayload.truncated`/`droppedParts` + `LogSessionInfo.droppedParts`**, `LogSessionInfo.totalFiles/totalBytes` (+ export counts), catalog rows, `DevPage.tsx`, reveal/export/delete actions, raw-names confirm + warning, privacy statement | **yes** (ui-designer first) | user surface + purge + truncation disclosure | catalog parity test passes; toggling `dev.enabled` starts/stops writing **and deletes nothing**; enabling raw names confirms and starts a new file; export produces a zip of the session parts **in `exports/`**. **`logs_delete_all`:** (a) Dev mode OFF ⇒ every in-scope file removed, `rolled:false`, `activeFile:null`; (b) **Dev mode ON ⇒ the writer rolls to a new file with an `afterPurge:true` header, every prior file (including the one just closed) is removed, `rolled:true`, `activeFile` names the new file, and logging continues with no lost record**; (c) `deletedFiles` / `deletedBytes` **exactly match** the files and byte sizes removed, asserted against a pre-seeded fixture directory; (d) a file made undeletable increments `failedFiles`, the command still returns `Ok`, and the UI shows the partial-result warning; (e) nothing outside `logs/` and `exports/` is touched (assert `metrics/` and `settings.json` survive); (f) the delete is confirm-gated and writes no record into the surviving file. **§6.2:** (g) **export a session, then delete: the zip in `exports/` is gone**, `deletedExports` counts it, and its bytes are included in `deletedBytes`; (h) a `.zip` seeded into `logs/` is also removed; (i) a zip saved *outside* both directories survives and the confirm copy says so. **§6.3:** (j) driving a writer past `max_parts` emits exactly one `truncate` record per evicted part **into the surviving newest part**, with `droppedParts` incrementing; (k) every part header opened after an eviction carries `truncated:true` + `droppedParts`; (l) `log_session_info` reports `droppedParts` and the Dev page shows the truncation warning line |

Increments 4 and 7 require a `ui-designer` pass (`docs/contracts/P91-observability-ui.md`) before
senior-dev.

### Milestone gate
**AI gate:** `cargo test` + vitest green; `pnpm gate`; browser harness with `VITE_MOCK_IPC=1` runs a
scripted flicker scenario (including a sidebar ref change) and `__bonsaiDumpLogs()` output is
inspected for correct traces and at least one true-positive anomaly; **the mock slow-`graph.get`
fixture produces a `slow-command` + `slow-phase` pair whose `refs` resolve to the span**; §11
budgets hold, incl. the sidebar record-count bound, the ≤1-span-per-operation bound and the
**128 MB per-session disk bound**; a real `logs/*.jsonl` from a `pnpm tauri dev` session is parsed
line-by-line by a test asserting schema validity, **that `argsHash` appears on no record kind other
than `ipc.call`/`ipc.result`**, and **zero redaction violations** (regex scan for path separators,
`@`, `http`, known token shapes, the session salt, and any branch name present in the fixture repo).

**USER CHECKPOINT:** open a real repo in the native window with Dev mode ON, perform the actions
that flicker (**including the sidebar interactions where flickering was observed**), then (a)
"Reveal logs folder" opens the correct directory, (b) "Export session" produces a zip, (c) the user
reads the exported JSONL and confirms it identifies the misbehaviour **and contains nothing they
would not send to a third party**, (d) **"Delete all log files" with Dev mode still ON empties the
folder down to one fresh file — including the zip exported in step (b) — the reported count/size
look right, and logging visibly continues**, (e) with Dev mode OFF, graph scroll on a 20k-commit
repo feels unchanged, (f) **on a large repo, an operation that feels slow produces a `span` record
whose phases plausibly explain where the time went**.

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
| 8 | **Performance dimension added** (user, 2026-08-27) | **ACCEPTED.** P91 must diagnose *slowness*, not only redundancy. All schema work is **ADDITIVE ONLY** (new `LogKind` variants + optional `#[serde(default)]` fields); `OBS_SCHEMA_VERSION` stays 1 so the in-flight implementation absorbs it without rework | §3.1, §5.1, §8.1, §11, §12 |
| 9 | Intra-operation breakdown shape | **DECIDED — one `span` record per operation with a `phases[]` array**, not one record per phase (volume: ~1 line per heavy op). Mechanism is an **explicit `PhaseRecorder` value**, never a task-local — consistent with §2.2's rejection of an ambient backend trace. Nesting is expressed by dotted labels, not a structural tree | §3.1, §3.1.1 |
| 10 | Where phase timing lives | **DECIDED — at the `src-tauri` caller layer** (`graph_cache.rs`, the status and diff command bodies), **not** inside `crates/bonsai-core`. bonsai-core must not depend on `obs/`; the src-tauri layer already orchestrates these calls and already increments `perf.rs`. **Exactly three ops** (`graph.get`, `status.scan`, `diff.compute`) and ~11 phase labels in v1; a fourth op needs a new decision row | §3.1.2, §12 inc. 3 |
| 11 | Contention / saturation records | **DECIDED — optional fields on the same `span` record** (`queuedMs`, `poolInflight`, `poolMax`, `deadlineFrac`), not new record kinds. Queue delay is captured around `spawn_blocking` in `repo_handle.rs`; watchdog pressure comes from the existing `run_with_git_timeout*` deadline. One operation ⇒ one line | §3.1.3, §5, §12 inc. 3 |
| 12 | Slow-operation thresholds | **DECIDED — per-command, self-calibrating.** `ms > max(floor, k × rolling_p95)` over an in-memory per-`cmd` histogram (reusing the §8 bucket shape), gated by `MIN_SAMPLES`, rate-limited to 1 per cmd per 10 s, plus a >10 s absolute catch-all. Constants in one `SLOW_RULES` table in `obs/anomaly.rs`. This is why a 20k-commit repo does not fire continuously: the baseline is that repo's own normal cost | §5.1, §12 inc. 5 |
| 13 | Cache-effectiveness rule | **DECIDED — `cache-collapse`**, driven by an optional `cache` field on the `graph.get` span emitted by `graph_cache.rs` (site-local knowledge, per §5's philosophy) and suppressed when a mutation occurred in the window | §3.1, §5.1, §12 inc. 5 |
| 14 | Durable percentiles | **DECIDED — no new storage type.** The existing `Histogram` is the bounded summary; `percentile_ms()` derives p50/p95 from the frozen 8 buckets at snapshot time, and `p50Ms`/`p95Ms` are **derived, never persisted**. Phase durations fold into allow-listed `op.*` sub-keys so week-over-week regression is answerable per phase. Bucket boundaries are frozen — changing them would break existing `usage.json` files | §8.1, §11, §12 inc. 6 |
| 15 | Interaction-latency (gesture → paint) | **DEFERRED — reason recorded.** Needs rAF-after-commit plumbing in all six surfaces and is unverifiable in the headless harness (0×0 pane ⇒ no rAF). The `gesture → ipc.result.ms → span.phases → render.tally → frame/jank-trace` chain already triangulates the motivating complaints. Revisit only if a real complaint resists that chain after increment 5 | §9.4 |
| 16 | **§7.2 contradiction — ordinal scheme** (raised by senior-dev during increment 1) | **RESOLVED — keep salt-seeded counters; (a) and (b) hold, (c) cross-side agreement is STRUCK.** A counter cannot agree across sides (different first-sight order), and the only synchronous-mirrorable alternative, `FNV-1a-64(salt, value) → ordinal`, was **rejected on privacy grounds**: branch/tag/remote names are a small, guessable input space and FNV is not a PRF, so anyone with the salt could dictionary-attack every ordinal — while a counter leaks only first-sight order. A cryptographic keyed hash would restore secrecy but cannot be mirrored synchronously on the render path. **Mitigation for the "two different branches look identical" failure mode: frontend ordinals are `ui:`-prefixed**, so the namespaces are disjoint and conflation is impossible. Cross-side correlation uses `trace`/`span`/`seq`; **no rule may read an ordinal**. **Increment 1's implementation stands — no reimplementation**; §12 inc. 1 tests are correct as written and must NOT assert (c) | §7.2, §7.1, §7.3, §6, §5, §12 inc. 1 & 2 & 5 |
| 17 | **Export/delete privacy hole** (raised by senior-dev during increment 1) | **RESOLVED — both halves.** `log_export_session` defaulted its zip *into* `logs/`, whose purge scope was `*.jsonl` only, so "Delete all log files" reported success while a **`raw`-names** zip survived — defeating decision 7 outright. Fix: (1) exports default to `<app_config_dir>/exports/`, never `logs/`; (2) the purge scope widens to `logs/*.jsonl`, `logs/*.jsonl.tmp`, `logs/*.zip` and `exports/*.zip`. A zip the user deliberately saved into either app-managed directory **is** deleted — that is intended, the confirm dialog states counts and bytes first, and silently retaining a raw-names archive is the worse failure. Exports saved elsewhere are unreachable and are **not** deleted; the confirm copy and the export content statement must both say so, because claiming completeness the command cannot deliver is the same class of defect. Counts stay honest: export zips appear in `deletedFiles`/`deletedBytes` and are additionally broken out in the new optional `deletedExports` | §6, §6.1, §6.2, §7.1, §10, §12 inc. 1 & 7 |
| 18 | **Rotation past `max_parts`** (found by reviewer, directed by orchestrator during increment 1) | **RATIFIED AS IMPLEMENTED — evict the oldest part of the current session group; never refuse rotation.** The original "keep appending to the last part" wording left a session unbounded, because `prune` ran only at `LogWriter::open` and never pruned the last group — a single event-storm session could exceed the 256 MB cap until the next launch. Eviction restores a hard **128 MB** per-session bound while preserving the **never discard the newest evidence** invariant (the user's workflow puts the anomaly at the end of the file). Pruning also now runs **after every rotation**, not only at open, so the total cap is enforced continuously. **Accepted cost, disclosed not hidden:** a truncated log cannot prove the *first* occurrence of a bug, which matters for double-trigger work — so truncation is recorded in-band (new additive `truncate` record + `SessionPayload.truncated`/`droppedParts` + `LogSessionInfo.droppedParts`), the reviewing AI must treat missing early records in a truncated file as **inconclusive rather than absent**, and the Dev page surfaces a warning suggesting narrower capture instead of a bigger cap. `truncate` ≠ `drop`: one is on-disk loss, the other in-memory backpressure. **Increment 1's code stands; the additive record lands in increment 7** | §6, §6.3, §3, §5, §7.3, §10, §11, §12 inc. 1 & 7 |
| 19 | **Token scrubber — Layer B heuristic + Layer A status** (implemented in increment 1 incl. its MUST-FIX round; ratification requested) | **RATIFIED. Layer B constants named: `MIN_SECRET_LEN = 32`, `MIN_B64_RUN = 24`**, alpha+digit mix required, pure hex excluded. The original `/`-rejecting form made §7.2's own "base64 PAT shape" structurally uncatchable. The run test is sound because random base64 hits `/` about once per 64 chars while path segments are human-named and short — and the few that reach 24 chars are word-shaped and fail the digit requirement. Accepted, fail-safe consequences: scp-style remotes ordinalise as `path#` not `remote#`; UUID-shaped strings ≥32 chars and digit-bearing long camelCase segments over-redact. **Precision is permanently subordinate to recall: a missed credential is unrecoverable, an over-redacted path costs only legibility.** **Layer A is largely SHIPPED and must not be re-implemented** — increment 1's MUST-FIX round converted `TOKEN_PREFIXES` to `(prefix, min_len)` pairs (`redact.rs:190`, `:213`) precisely so short tokens escape the global floor, and shipped `glpat-` (+ `gldt-`/`glrt-`/`npm_`/`AKIA`/`ASIA`/`AIza`/`sk-`/`dckr_pat_`/`xoxe-` …), **`Basic` alongside `Bearer`** (`:382`) with a deliberate **`basic`-in-prose guard** (`:377-394`, must survive untouched) and a looser keyword-established length rule (`:304-305`) that already covers ~24-char `Basic` credentials. **Only TWO Layer-A items remain open, both additive in increment 3:** the `eyJ…` **JWT** prefix (a `.` disqualifies a Layer-B candidate and no prefix matches, so JWTs pass through today) and **`key=value` / `key: value` pairs matched inside plain-text string bodies** — `is_sensitive_key` (`:232-243`) is key-name-based and never sees line-oriented credential-helper / `.netrc` output, which is the shape a real credential takes when git hands it back. An earlier revision of this row listed `glpat-` and `Basic` as open; that assessed the pre-MUST-FIX state and is **corrected here** | §7.2.1, §12 inc. 1 & 3 |
| 20 | **`argsHash` has ONE producer and ONE canonical form** (contradiction found by the increment-2 reviewer) | **CORRECTED — §7.2's "a UI call and its Rust arrival hash alike" sentence is STRUCK; §3 was and is authoritative.** `IpcRecvPayload` is `{ cmd }` with no `argsHash`, so the log stream has exactly one canonical form — the frontend's **positional JSON array** (positional because `IpcApi` methods are, which is also why `argsShape` is keyed `"0"`,`"1"`,…). `argsHash` appears only on `ipc.call`/`ipc.result`. **Cross-side agreement is neither required nor implemented**, and `Redactor::hash_args` takes already-canonicalised `&str` — **no Rust canonicaliser exists**. **PROHIBITION binding on increment 3 and later: `ipc.recv` must NOT gain `argsHash`/`argsShape`.** A Rust canonicaliser would serialise a *named payload map*, yielding different canonical text for the same logical call; the resulting second canonical form would make `dup-ipc` **silently stop matching real double triggers** — the exact failure this milestone exists to detect, and invisible because the rule just goes quiet. **If a future increment truly needs a Rust-side `argsHash`, BOTH are required (not either/or):** it must adopt the **identical positional-array canonical form** as `src/obs/redact.ts`, pinned by the existing cross-side vectors at `src-tauri/src/obs/tests_redact.rs:203-211`; **and** `dup-ipc` must already filter explicitly on `kind`. **Item 3 decision — `dup-ipc` states its own precondition, mandated NOW:** the rule must filter on `kind === 'ipc.call'` **explicitly in code**, with a unit test feeding a synthetic non-`ipc.call` record bearing an `argsHash`. Reason: correctness that emerges from a field being *absent from another payload type* is fragile in a contract making continuous additive changes — any future additive field breaks it silently. An explicit filter makes the rule locally verifiable and independent of every other payload's shape. Documentation-only; no shipped code changes | §3 (`IpcRecvPayload`, `ArgShape`), §2.3, §2.3.1, §4, §5 (`dup-ipc` row + precondition), §7.2, §12 inc. 2/3/5 + gate |
| 21 | **Sub-phase split unrealisable for status/diff** (D3, ratified during increment 3) | **RATIFIED AS BUILT — commit `a351d36`, reviewer-approved.** `graph.get` is fully phased (`revwalk`/`decorate`/`lane`/`serialize` + `cache`) because its orchestration is visible at `graph_cache.rs`. `status.scan` collapses to a single `statuses` phase and `diff.compute` to a single `hunks` phase: their finer steps (`index`/`map`, `tree`/`serialize`) live inside `crates/bonsai-core`, and hooking them there would breach the `bonsai-core`↔`obs` boundary invariant (`bonsai_core_has_no_obs_reference`). Op-level `ms` + `queuedMs`/`poolInflight`/`poolMax`/`deadlineFrac` retained for all three. Finer status/diff phasing is **permanently rejected** (invariant non-negotiable; op-level `ms` is sufficient granularity), not deferred. **v1 scope note:** only `get_workdir_file_diff` is instrumented; commit-vs-parent diff is uninstrumented | §3.1.2 addendum, §12 inc. 3 |
| 22 | **Mock-mode anomaly source split** (D5b, ratified during increment 5) | **RATIFIED AS BUILT.** The authoritative anomaly detector (`obs/anomaly.rs`) runs only on the Rust sink writer thread, so in mock mode (`VITE_MOCK_IPC=1`, no Tauri) the ring buffer never yields anomalies — yet §6's mock-mode assertion and the §12 gate + row 5 require the browser harness to show them. Resolution: a **mock-only, dump-time batch analyzer** over the ring at `__bonsaiDumpLogs()` time, scoped to **exactly the three gate-named rules** (`dup-ipc`, `slow-command`, `slow-phase`). `obs/anomaly.rs` stays the **sole authoritative detector**; the mock analyzer is a harness-only diagnostic that never reaches a production bundle. Documentation-only; ratifies increment-5b code | §6, §5, §5.1, §12 inc. 5 + gate |
| 23 | **`FramePayload.dim` is REQUIRED at schema 1** (edit by senior-dev in `8da1291`, ratified by architect) | **RATIFIED with a stated carve-out.** The discriminator is required, not optional-with-default: `gapMs: 0` on a paint record is a fabricated datum, and a default reintroduces exactly the ambiguity the field removes. It is nonetheless a **breaking change to the v1 `frame` shape**, permitted only because P91 is **pre-release** (branch-only, absent from `dev`) and **no Rust reader parses records from disk** — `LogRecord::Deserialize` serves the same-build `log_append` IPC path alone. `OBS_SCHEMA_VERSION` stays **1**. **The carve-out expires on merge to `dev`**; after that a required-field addition needs a version bump **and** a §3.2 reader rule. New §3.2 states the reader rules (skip unknown `kind`, ignore unknown fields, reject the malformed LINE not the file, never infer a missing discriminator, best-effort on a higher `schema`). **Orchestrator decision 2026-09-02: option A of the architect's three** — schema stays 1 with an expiring carve-out, rather than bumping to 2 now, because no v1 corpus exists to protect and a bump would manufacture a phantom version that future readers write compatibility code for | §3 amendment rule, §3.2 (new), §11 |
| 24 | **`writeFailed` clears only while rotation is healthy** (edit by senior-dev in `8da1291`, ratified by architect) | **RATIFIED — the deviation from the literal old wording is the correct contract.** "Clears on any flush that reaches disk" let a persistent rotation block report healthy, because the writer still held the previous part's working `BufWriter`; the Dev status row and `DevModePill` flapped every idle flush. The flag means **"records are not reaching disk"**, and no clear rule may return `false` while that holds. State machine moved out of the `LogSessionInfo` doc comment into **§6.4**; `rotationBlocked` is writer-local and never crosses IPC; the exported surface stays a bare bool for privacy | §6 Commands, §6.4 (new), UI §8.4 |
| 25 | **Metrics key predicates are a privacy guard, not a size split** (extended 2026-09-03) | **RATIFIED, with two corrections.** `obs/metrics_keys.rs` is contractually separate from `metrics.rs` because `usage.json` is durable, uncovered by `logs_delete_all` and unredacted — a user-derived key there is permanent repo content. (a) **`is_valid_counter_key` now literally requires the `<domain>.<action>` dot**, tightened beyond the original finding: a bare lowercase token (an oid, an id, a `ghp_…`) was otherwise shape-valid and would have been persisted the moment the guard began running in release. (b) **`bump_validated` is the single sink** both `bump_counter` and `fold_perf` pass through, as a **runtime `if`, not a `debug_assert`** — only now is "shape-valid decides persistence" true of the path that actually persists (`fold_perf` previously wrote unvalidated). §8/§8.1's "allow-list in `obs/metrics.rs`" pointers are corrected: names in `metrics.rs`, shape in `metrics_keys.rs`, `cmd.*` membership in `metrics_cmds.rs`, cardinality in `metrics_map.rs` — see new §8.2 | §1, §8, §8.1, §8.2 |
| 26 | **Raw mode widens IDENTIFIER fidelity, never CONTENT fidelity** (contradiction found by the 2026-09-02 security audit; ruled by architect) | **RULED: line 910 was the defect; §7.1's two absolute "never" rows stand.** The table said argument values are "included as `args`" in raw, while the same table said commit messages and search queries are "never, in either mode" and tokens are "NEVER, under any setting". The implementation resolved the conflict in the leaking direction and wrote commit messages, search text and forge PATs to disk — while the consent dialog promised it would not. Grounds for the ruling: two absolute "never"s outrank one mechanism description that never mentions them; consent is bounded by what the dialog promised at the moment of consent; §7.1 line 922 already defines raw's purpose as *real names*; and the failure is unrecoverable in one direction only (a PAT in a mailed zip) versus mere reviewer legibility in the other. **Mechanism:** sparse per-command allow-list, **default DENY**, keyed by parameter NAME never position, scalars only — so `is_sensitive_key` becomes meaningful inside `args` for the first time. **Independent writer-side enforcement** (`obs/raw_args.rs`) deliberately does NOT consult the table: it enforces a shape+vocabulary invariant it can decide alone, and its key rule kills the leak even if the producer is never fixed. `OBS_SCHEMA_VERSION` stays 1 under the row-23 pre-release carve-out | §7.1 (row corrected), `P91-raw-args-privacy.md` |
| 27 | **`log_export_session` loses its `dest` parameter** (audit F4, fixed `120cadd`) | **RATIFIED.** The parameter was justified by a comment claiming the path came from an OS save dialog **that does not exist** — none was ever wired, and the only caller passed nothing. An `Option<String>` destination reachable from the webview is an unmediated arbitrary-directory-create + file-write primitive, and it silently escaped the §6.2 delete-all scope that decision 17 exists to guarantee. The command is now zero-arity on both sides and always writes to `exports/`, which restores the delete scope **by construction**. **RULE:** any future "Save as…" must take its path from a **backend-invoked** Tauri dialog, never a webview-supplied string, and must extend §6.2's honest-reporting paragraph before shipping | §6.2, §6 Commands, §10 |
| 28 | **`cmd.*` metric keys recorded NOTHING in production** (audit F3, fixed `120cadd`) | **RATIFIED — and the failure mode is the durable part.** `is_valid_cmd_name` required all-lowercase; the producer (`obs/ipcProxy.ts`) sends **camelCase `IpcApi` method names**. The entire `cmd.*` histogram family was therefore silently empty in every real session, while the Rust tests stayed green because they fed the validator **snake_case** names. *Tests and production fed different-shaped inputs to the same validator, so everything was green while the feature did nothing.* Fix: authority moves to **exact membership in a generated 199-name allow-list** (`obs/metrics_cmds.rs`, an exact bijection with the `IpcApi` declarations, pinned by a drift test that re-derives it from the `.ts` sources); the shape predicate merely bounds it. **Binding rule:** a validator's tests must use inputs produced by the real producer, and a cross-boundary vocabulary must be pinned by a drift test — a predicate that rejects everything is indistinguishable from one that works unless something asserts a real input is **accepted** | §8, §8.2 |
| 29 | **`MAX_KEYS_PER_MAP = 512` + `meta.overflow`** (new surface, `120cadd`) | **RATIFIED AS BUILT; NO global cap.** `obs/metrics_map.rs` caps `counters`/`durations`/`errors` in every `MetricTotals` — day buckets and the 400-day→`lifetime` roll-up — folding overflow into a `<domain>.<action>`-shaped `meta.overflow` bucket so counts are bounded but never lost. Worst-case cardinality is per-map-per-bucket, i.e. `400 × 3 × 513` ≈ **616k keys**, acknowledged. A global cap is **rejected**: it would make today's recording depend on history, so a long-lived install would stop minting keys and dump current activity into overflow, destroying the week-over-week comparison §8.1 exists for. The reachable key set is ~212 (allow-listed), so 512 is a runaway stop, not a sizing parameter. Total-file-size pressure has a different lever — a **size-triggered early roll-up** of the oldest `days[]` into `lifetime` — recorded as a **revisit trigger only** (threshold ~8 MB), not work in P91. §8.1's "~60 keys / 1.2 MB" ceiling is corrected to ~212 keys / ~4.3 MB worst case | §8, §8.1, §8.2 |
| 30 | **`LogPayload::IpcCall` gains `args_omitted: Option<u32>`** (fixed `120cadd`) | **RATIFIED; `OBS_SCHEMA_VERSION` stays 1** under the row-23 pre-release carve-out (optional + `skip_serializing_if`). **Why it was missing:** the field was contract-declared (A26 §B.6) and producer-emitted but absent from the Rust struct, so serde dropped it at `log_append` deserialisation — writer rule **W6 was dead in production** while its test passed, because that test alone bypassed the `LogRecord`→`append_record` round-trip W1–W5 used. **Binding test rule:** a writer rule is covered only by a round-trip test (`LogRecord` → `append_record` → read the file back); a synthetic-`Value` test is permitted in addition, never instead | §3, `P91-raw-args-privacy.md` §C |
| 31 | **`briefString` gates on field name in BOTH modes** (fixed `120cadd`) | **RATIFIED.** The raw branch returned 48 characters verbatim for any field, while the hook's doc claimed it was safe to wire "BY CONSTRUCTION" — true only in `strict`. It now applies the shared free-text/credential vocabulary (`isFreeTextParam`/`isSensitiveParam`) in both modes, so a `message`/`query`/`token`-shaped field name is ordinalised or reported as `str:<len>` regardless of mode. The gate must be **name-based here specifically** because `state` records ride **outside** the `raw_args` backstop (W1 scopes it to `kind == "ipc.call"`), making `briefString` the sole gate on that path | §7.1, §9.1 |

**Deferred follow-ups (explicitly out of P91):** app-wide React instrumentation beyond the six
surfaces; the Statistics page and any UI for `metrics_reset`; selective/per-file log deletion (v1
is all-or-nothing); **interaction-latency (gesture→paint) measurement (§9.4)**; **phase
instrumentation of any operation beyond the three in §3.1.2** (e.g. fetch/push, blame, search);
**deletion of exports the user saved outside Bonsai's config directory** (physically unreachable —
§6.2 requires disclosing this, not solving it); **a user-configurable part/size cap** (§6.3 chose
disclosure + narrower capture over a bigger cap); **a Rust-side `argsHash`** (§13 row 20 states the
two conditions any such change must meet).
