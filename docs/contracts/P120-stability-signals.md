# P120 (part b) — Dev-mode stability field signals: `layout.shift`, `focus.lost`

**Status:** contract (architect, 2026-09-24). Ratifies + amends `docs/specs/008-perf-render-stability/plan.md`
"Part b". Spec: `docs/specs/008-perf-render-stability/spec.md` ACs 9–14. Parent: `docs/contracts/P91-observability.md`
(§3 schema, §3.2 versioning, §5 anomalies, §7 redaction). Part a (lab) is out of scope except the
shared detector interface in §3.

Locked (do not re-open): no new switch — capture iff `obsEnabled()`; additive kinds, **no
`OBS_SCHEMA_VERSION` bump** (stays 3, `record.rs:18` policy); Rust↔TS kind-parity test; closed region
vocabulary via `data-obs-region`; `layout.shift` Chromium-only (Windows in v1), `focus.lost` all OSes.

---

## 0. Amendments to the plan (binding; the plan text is superseded where it disagrees)

| # | Plan said | Contract says | Why |
|---|---|---|---|
| A1 | `focus-steal` = 1000 ms look-back for refresh/watcher | Look-back **plus a bounded pending hold**; refresh matched by **round-interval containment** | The `refresh` record is emitted in `.finally()` when the round settles (`useCoalescedRefresh.ts:119`), so the loss it causes is usually logged **before** it. Pure look-back misses the main case. |
| A2 | AC 10 "same trace" | Time correlation (+ trace equality when a trace happens to exist) | `refresh` is an unbound aggregate (`trace: undefined`), and `focusout`/observer callbacks run outside `withTrace`. Trace-id joins can't be done. **Flagged.** |
| A3 | `focusout` + microtask re-check | `focusout` **and** a `MutationObserver` detached-element path; re-check after `FOCUS_SETTLE_MS` (50 ms) | When a refresh unmounts the focused element, Chromium doesn't reliably fire `focusout`. A microtask is also too early: React dialogs autofocus in `useEffect`, after paint. |
| A4 | Classifier had a "not inside a dialog/toast/palette" condition | Dropped | Once "activeElement is body" holds, focus can't be inside an overlay, so the condition adds nothing. It would only hide real steals of a dialog's own input. |
| A5 | `FocusLostPayload.to: 'body' \| 'none'` | Replaced by `via: 'focusout' \| 'detached'` | By the time we check, `activeElement` is effectively never `null`. `via` tells the reviewer whether the element was *unmounted*, which is the refresh-remount signature. |
| A6 | Phase read at emit time (`currentPhase()`) | Phase from **`entry.startTime` / loss time** against a transition log; `markBoot()` re-arms on repo open/switch | Observer callbacks are async, and a buffered replay would label old boot shifts as `interactive`. A repo switch re-streams the graph, so it is boot-like. |
| A7 | Buffered observer always on | `buffered: true` **only on the first attach while in boot**; later attaches drop entries with `startTime < attachedAt` | Turning dev mode on mid-session would otherwise replay the ≤150 buffered shifts as fresh ones. |
| A8 | `hadRecentInput` = the API flag | `hadRecentInput = entry.hadRecentInput \|\| inputWithin(startTime, 500)`, using our own tracker (pointerdown, keydown, wheel, touchstart) | The API excludes scroll/wheel, but AC 11 counts scroll as a gesture. The DOM `scroll` event is **not** tracked: an app-initiated `scrollIntoView` would otherwise mask a real shift. |
| A9 | `value` = "summed score for this region in this entry" | `value` = sum of **whole-entry** scores for the entries this region took part in. It is not exclusive to the region; never sum it across regions | The API gives no per-source score. |
| A10 | Coalesce per region per animation frame; Rust keeps a 1 s window with all refs | The frontend dev sink coalesces into **1 s buckets per (region, phase, hadRecentInput, resizing)**. Boot buckets flush once, at `markInteractive()`. The Rust `unprompted-shift` rule is **stateless, one anomaly per qualifying record** | The sink detector has no timer, so a Rust-side window could only close on some later record. Bucketing at the source also bounds log volume, and one record carries `shifts`/`spanMs`. The flags are part of the bucket key, so buckets never mix prompted and unprompted shifts. |
| A11 | "emit when `value > 0` during boot" | Dropped (a real entry always has `value > 0`). Boot shifts are emitted as the boot summary above, at `debug` | The plan's condition was always true. |
| A12 | Disclosure copy goes in the `dev.capture-react` ("Screen updates") help text | Copy goes in the fixed **"What Bonsai records"** statement (`dev.ts:100`, P91 §7.3), with a Windows-only caveat for layout. `capture-react` help stays unchanged | Capture is gated on `obsEnabled()`, not `captureReact`. Hanging the copy on that toggle would make the disclosure false whenever the toggle is off. |
| A13 | Harness check only looked for records reaching `log_append` | The two rules are **mirrored in the mock analyzer** (`src/ipc/mock/obsAnomalyStability.ts`) | Without this, the browser-harness AI gate can't show AC 9/10 anomalies (mock-IPC invariant). |
| A14 | `region: String` in Rust | Closed Rust enum `StabilityRegion` with a lenient `From<String>`: any unknown value becomes `Other` | Writer-side enforcement of AC 13. A forged path can never reach disk, and one bad value can't reject a whole batch. |
| A15 | Split out "the small enums near the bottom" of `record.rs` | Move exactly `record.rs:49–135` and `FrameDim` (`:201–209`), see §2.3 | Makes the split concrete: ~390 lines, and the baseline entry is **removed**. |
| A16 | Both kinds default to `info` | `focus.lost` → `info`. `layout.shift` → `info` only when unprompted-interactive, else `debug`. `DEFAULT_LEVEL` gains both keys | Keeps `info`-level logs to actionable records. A missing `DEFAULT_LEVEL` key falls back to `debug` (`log.ts:36`). |

---

## 1. Boundary

- No new command, event or channel. Records travel `logRecord()` → `batcher.ts` → `log_append` (existing).
- The frontend owns **detection and classification** (DOM-only knowledge). Rust owns **cross-record
  correlation** (the `anomaly` rules in the sink, P91 §5).
- Neither kind sets the base `repo` field. The three-producer allow-list at `record.rs:188` is **not** widened.
  Both rules are repo-blind.
- **Zero cost when off:** with no sink registered, no observer, listener or MutationObserver is attached (§3.2).
- **No layout reads:** the detector must never call `getBoundingClientRect`, `offset*`, `client*`,
  `getComputedStyle` or `scroll*` getters. Allowed reads: `closest`, `isConnected`,
  `document.activeElement`, `hasFocus()`, `visibilityState`, entry fields. This is the spec's overhead
  edge case, enforced by review and by T-F13.

---

## 2. Rust

### 2.1 `src-tauri/src/obs/record_stability.rs` (new, ≤110 lines)

```rust
use serde::{Deserialize, Serialize};

/// Closed region vocabulary (P120). NEVER a path or user string.
/// `from = "String"` ⇒ `rename_all` governs Serialize only; the `From<String>` match MUST use the
/// same lowercase strings as `as_str()` (pinned by T-R11's valid-region round trip).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", from = "String")]
pub enum StabilityRegion { Sidebar, Graph, Details, Diff, Dock, Other }

impl From<String> for StabilityRegion { /* exact lowercase match, else Other */ }
impl StabilityRegion { pub fn as_str(self) -> &'static str; }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StabilityPhase { Boot, Interactive }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FocusVia { Focusout, Detached }

/// One coalesced bucket: ≤1 record per (region, phase, hadRecentInput, resizing) per 1 s.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutShiftPayload {
    pub region: StabilityRegion,
    pub phase: StabilityPhase,
    pub had_recent_input: bool,
    pub resizing: bool,
    /// Entries coalesced into this record (≥1).
    pub shifts: u32,
    /// Sum of whole-entry scores (A9). Finite, ≥0, 4 dp.
    pub value: f64,
    /// Epoch ms of the first coalesced entry (the record's `ts` is the flush time).
    pub first_at: i64,
    /// last entry − first entry, ms.
    pub span_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusLostPayload {
    /// Region of the element that HAD focus (captured at focusin, A3).
    pub from_region: StabilityRegion,
    pub phase: StabilityPhase,
    pub via: FocusVia,
    /// Epoch ms the loss was observed (record `ts` = classify time, ≈ +50 ms).
    pub lost_at: i64,
}
```

Only `StabilityRegion` is lenient. `phase`/`via` are same-build closed enums, and an unknown value
rejects the record as usual (P91 §3.2 posture: `Deserialize` serves `log_append` only).

### 2.2 `LogPayload` variants (`record.rs`)

```rust
    /// P120 — frontend layout-shift bucket (Chromium/WebView2 only).
    #[serde(rename = "layout.shift")]
    LayoutShift(LayoutShiftPayload),
    /// P120 — unprompted focus loss to <body>.
    #[serde(rename = "focus.lost")]
    FocusLost(FocusLostPayload),
```
These are newtype variants in an internally tagged enum, so the struct's fields flatten beside `kind`.
`rename_all_fields` on `LogPayload` does **not** reach the inner structs, which is why each struct
carries its own `rename_all = "camelCase"`. Wire:
`{"kind":"layout.shift","region":"graph","phase":"interactive","hadRecentInput":false,"resizing":false,"shifts":3,"value":0.0412,"firstAt":…,"spanMs":412}`
`{"kind":"focus.lost","fromRegion":"details","phase":"interactive","via":"detached","lostAt":…}`

### 2.3 `record.rs` split (same increment, behaviour-preserving)

- Move to **`record_enums.rs`** (new, ~110 lines): `LogLevel` (`:49–61`), `RedactionMode` **with its
  `impl … note()`** (`:63–108`), `LogSource`, `IpcOutcome`, `AnomalySeverity` (`:110–135`), `FrameDim`
  (`:201–209`). Doc comments move with them. `ArgShape` and `PhaseTiming` stay.
- `record.rs` adds `pub use super::record_enums::{AnomalySeverity, FrameDim, IpcOutcome, LogLevel, LogSource, RedactionMode};`
  and `pub use super::record_stability::{FocusLostPayload, FocusVia, LayoutShiftPayload, StabilityPhase, StabilityRegion};`
  so every existing `record::X` path still resolves.
- `obs/mod.rs`: `pub mod record_enums; pub mod record_stability;`, plus the `#[cfg(test)]` decl of
  `tests_kind_parity` next to `tests_schema_parity`.
- Target: `record.rs` ≈ 390 lines. **Delete** the `"src-tauri/src/obs/record.rs": 503` entry from
  `scripts/file-size-baseline.json` with a one-line hand edit. Refactorer rule: identical `cargo nextest`
  test count before and after the move, measured *before* the new tests are added.

### 2.4 `record_kind.rs` (42 → ≤80)

```rust
impl LogPayload {
    /// Every wire `kind`, in declaration order. Drift-guarded against TS `LogKind` (tests_kind_parity).
    pub const ALL_KINDS: &'static [&'static str] = &[
        "session","gesture","ipc.call","ipc.result","ipc.recv","event","channel","watcher",
        "refresh","render","render.tally","effect","state","frame","error","anomaly","span",
        "drop","truncate","layout.shift","focus.lost",
    ];
    pub fn kind(&self) -> &'static str; // + LayoutShift(_) => "layout.shift", FocusLost(_) => "focus.lost"
}
```
Known limit: nothing at compile time forces `ALL_KINDS` to change when a variant is added. That
direction is covered by T-R10's sample-per-variant list plus review. TS↔Rust drift is covered by T-R9.

### 2.5 `anomaly.rs` (399 → ≤415) + `anomaly/stability.rs` (new, ≤230)

`anomaly.rs` changes: `mod stability;`, a field `stability: stability::StabilityState`, and these
arms in `observe()`:
```rust
LogPayload::LayoutShift(p) => self.stability.on_shift(ts, seq, rec, p, &mut out),
LogPayload::FocusLost(p)   => self.stability.on_focus_lost(ts, seq, rec, p, &mut out),
// Refresh arm gains:        self.stability.on_refresh(ts, seq, rec, &mut out);
// Watcher arm gains:        self.stability.on_watcher(ts, seq, rec, &mut out);  (all watcher records; filter inside)
```
- `prune(now)` becomes `prune(now, &mut out)` and calls `self.stability.prune(now, out)`. It already
  runs before `bump_anomaly_count`, so held losses that expire are counted. **Senior-dev check:** confirm
  `prune` has no caller other than `observe()` (`anomaly.rs:179`).
- `on_session_end()` appends `self.stability.drain_pending()`.
- The existing `Refresh`/`Watcher` rules are otherwise untouched.
- `tests_anomaly::non_consumed_kinds_emit_nothing` needs **no change**: it feeds Gesture, IpcRecv and
  Render, and `_ => {}` still covers the rest. Do not add the new kinds to it; its purpose is the
  opposite.

```rust
// anomaly/stability.rs
pub(super) const FOCUS_WATCHER_LOOKBACK_MS: i64 = 1_000;
pub(super) const REFRESH_PRE_SLACK_MS: i64 = 50;    // loss just before the round's measured start
pub(super) const REFRESH_POST_MS: i64 = 250;        // React commit lands after `.finally()`
pub(super) const FOCUS_HOLD_MS: i64 = 5_000;        // max wait for the settling `refresh`
pub(super) const FOCUS_PENDING_CAP: usize = 64;
pub(super) const CAUSE_HISTORY_MS: i64 = 6_000;
pub(super) const CAUSE_HISTORY_CAP: usize = 256;

#[derive(Default)]
pub(super) struct StabilityState {
    causes: VecDeque<Cause>,          // pruned to CAUSE_HISTORY_MS, FIFO-capped
    pending: VecDeque<PendingLoss>,   // FIFO-capped; overflow ⇒ emit oldest as info
}
struct Cause { kind: CauseKind, seq: u64, start: i64, end: i64, traces: Vec<String> }
enum CauseKind { Refresh, Watcher }
struct PendingLoss { seq: u64, lost_at: i64, ts: i64, p: FocusLostPayload, trace: Option<String> }

impl StabilityState {
    pub(super) fn on_shift(&mut self, ts: i64, seq: u64, rec: &LogRecord, p: &LayoutShiftPayload, out: &mut Vec<LogRecord>);
    pub(super) fn on_focus_lost(&mut self, ts: i64, seq: u64, rec: &LogRecord, p: &FocusLostPayload, out: &mut Vec<LogRecord>);
    pub(super) fn on_refresh(&mut self, ts: i64, seq: u64, rec: &LogRecord, out: &mut Vec<LogRecord>);
    pub(super) fn on_watcher(&mut self, ts: i64, seq: u64, rec: &LogRecord, out: &mut Vec<LogRecord>);
    pub(super) fn prune(&mut self, now: i64, out: &mut Vec<LogRecord>);
    pub(super) fn drain_pending(&mut self) -> Vec<LogRecord>;
    #[cfg(test)] pub(super) fn pending_len(&self) -> usize;
    #[cfg(test)] pub(super) fn causes_len(&self) -> usize;
}
```

#### Rule `unprompted-shift` (stateless)
```
on_shift(ts, seq, rec, p):
  if p.phase == Interactive && !p.had_recent_input && !p.resizing:
     out += build_anomaly("unprompted-shift", Warn,
        detail = "{region}: {shifts} shift(s), score {value:.4}, over {span_ms} ms",
        refs = [seq], traces = rec.trace.into_iter().collect(), ts)
```
Bounded by input: one anomaly per qualifying record, and the source emits ≤1 qualifying record per
region per second.

#### Rule `focus-steal` (warn = caused, info = no cause found)
```
on_watcher(ts, seq, rec):
  if payload.fired && !payload.suppressed:           # a suppressed echo ran no round
     causes.push(Cause{Watcher, seq, start: ts, end: ts, traces: rec.trace ∪ rec.caused_by})

on_refresh(ts, seq, rec):                             # ts = settle time, ms = round duration
  c = Cause{Refresh, seq, start: ts - round(ms), end: ts, traces: contributing_traces}
  causes.push(c)
  for p in pending (in order) where matches(c, p):
     emit_warn(p, [c] ∪ causes.filter(Watcher && matches(_, p))); remove p

on_focus_lost(ts, seq, rec, p):
  loss = PendingLoss{seq, lost_at: p.lost_at, ts, p, trace: rec.trace}
  hits = causes.filter(|c| matches(c, loss))
  if hits non-empty: emit_warn(loss, hits)
  else pending.push(loss); if pending.len() > CAP: emit_info(pending.pop_front())

matches(c, loss):
  trace_hit = loss.trace.is_some() && c.traces.contains(loss.trace)
  window_hit = match c.kind {
     Watcher => c.start <= loss.lost_at <= c.start + FOCUS_WATCHER_LOOKBACK_MS,
     Refresh => c.start - REFRESH_PRE_SLACK_MS <= loss.lost_at <= c.end + REFRESH_POST_MS }
  trace_hit || window_hit

prune(now):
  drop causes with end < now - CAUSE_HISTORY_MS; FIFO-cap at CAUSE_HISTORY_CAP
  for p in pending where now > p.lost_at + FOCUS_HOLD_MS: emit_info(p); remove

drain_pending(): emit_info for every pending loss (session end)

emit_warn(loss, hits):  build_anomaly("focus-steal", Warn,
    detail = "focus lost from {from_region} ({via}, {phase}); causes: {n_refresh} refresh, {n_watcher} watcher",
    refs = [loss.seq] ++ hits.seq sorted ascending, traces = dedup(∪ hits.traces ∪ loss.trace),
    ts = loss.ts)                                      # anchored at the loss record, not the refresh
emit_info(loss):        build_anomaly("focus-steal", Info,
    detail = "focus lost from {from_region} ({via}, {phase}); no refresh or watcher round found",
    refs = [loss.seq], traces = loss.trace.into_iter().collect(), ts = loss.ts)
```
Every `focus.lost` produces **exactly one** `focus-steal` anomaly, either warn or info. The frontend
already filtered out prompted losses, so the Rust rule doesn't look at input.

**Time base.** `lost_at` and `refresh.ts` are both UI `Date.now()`, and `watcher.ts` is the Rust
clock. `ts` is the only cross-side clock (anomaly.rs:20), and the 1 s watcher window absorbs skew.
Expiry is keyed on any observed record's `ts`. A UI batch lands within ~500 ms, well inside
`FOCUS_HOLD_MS`. A round longer than about 4.5 s degrades to `info`: a missed warn, never a false one.

**Redaction-independence invariant (P91 §5, extended).** Both rules key only on `kind`, the closed
region/phase/via enums, booleans, `ts`/`lost_at`/`ms`, and trace ids. They never key on `repo`,
`scope` text, origins or any ordinal. The detector runs on the in-memory, pre-redaction record
(`record.rs:181–183`, `obs/sink.rs`), so identical anomalies across modes hold by construction. T-R8
pins the *record lines*. `detail` strings are built only from enum `as_str()` values and
integers/floats, and **must contain no `/`, `\`, or `://`**, so `strict::enforce` leaves them
byte-identical.

### 2.6 P91 amendments (recorded here; do NOT edit P91-observability.md, docs-curator folds them in on archive)

**§3 `LogKind`:** append `| 'layout.shift' | 'focus.lost'  // ← added, P120`, and add the two payload
interfaces from §3 of this contract.

**§5 table — rows to append:**

| Rule id | Where | Window | Condition | Severity |
|---|---|---|---|---|
| **`unprompted-shift`** | sink | — (source buckets 1 s) | `layout.shift` with `phase:'interactive'`, `!hadRecentInput`, `!resizing`; refs `[seq]` | warn |
| **`focus-steal`** | sink | watcher +1 s; refresh `[start−50, end+250]`; hold ≤5 s | `focus.lost` whose `lostAt` falls in a `watcher{fired,!suppressed}` or `refresh` round window (or shares a trace); refs `[loss, causes…]`, traces = causes' `contributingTraces`/trace | warn |
| **`focus-steal`** (no cause) | sink | hold ≤5 s / session end | `focus.lost` with no matching round | info |

**§7.1 table — row to add:** `| UI region names, layout/focus phase, shift scores & counts (closed vocabulary, P120) | kept | kept |`

**§11 bounds:** `StabilityState` ≤256 causes (6 s) and ≤64 pending losses. No per-key maps.

---

## 3. Frontend — `src/obs/stability/` (shared detector; part a consumes the same events)

### 3.1 Types

`src/obs/types.ts` (+≈40, ≤315):
```ts
export type StabilityRegion = 'sidebar' | 'toolbar' | 'graph' | 'details' | 'diff' | 'dock' | 'other';
// 'toolbar' added by P122b (2026-09-25). Adding a region is a closed-vocabulary change: extend
// the Rust StabilityRegion (A14) and the TS union together, keep the round-trip/redaction tests
// green, and update the Dev privacy copy in the same increment (P120-ui.md amendment).
export type StabilityPhase = 'boot' | 'interactive';
export type FocusVia = 'focusout' | 'detached';
export interface LayoutShiftPayload { region: StabilityRegion; phase: StabilityPhase; hadRecentInput: boolean;
  resizing: boolean; shifts: number; value: number; firstAt: number; spanMs: number; }
export interface FocusLostPayload { fromRegion: StabilityRegion; phase: StabilityPhase; via: FocusVia; lostAt: number; }
// UiPayload gains:
//   | ({ kind: 'layout.shift' } & LayoutShiftPayload) | ({ kind: 'focus.lost' } & FocusLostPayload)
// and both kinds are added to the Exclude<LogKind, …> catch-all list.
```
`src/ipc/types/obs.ts`: add `| 'layout.shift' | 'focus.lost'` to `LogKind` (`:32`), one literal per
line in the existing style so the T-R9 parser stays trivial. `OBS_SCHEMA_VERSION` stays `3`.
`src/obs/log.ts` `DEFAULT_LEVEL`: add `'layout.shift': 'debug', 'focus.lost': 'info'`. The dev sink
passes `lvl` explicitly for unprompted shifts.

`src/obs/stability/types.ts` (new, ≤50): the detector events, used by **both** part a and part b.
```ts
export interface ShiftEvent { type: 'shift'; region: StabilityRegion; phase: StabilityPhase;
  hadRecentInput: boolean; resizing: boolean; value: number; at: number /* epoch ms */; }
export interface FocusLostEvent { type: 'focus-lost'; fromRegion: StabilityRegion; phase: StabilityPhase;
  via: FocusVia; at: number /* epoch ms */; }
export type StabilityEvent = ShiftEvent | FocusLostEvent;
export type StabilitySink = (e: StabilityEvent) => void;
```

### 3.2 Modules and public surface

| File | Lines | Exports |
|---|---|---|
| `regions.ts` | ≤50 | `REGIONS: readonly StabilityRegion[]` (with a compile-time exhaustiveness check against the type); `regionOf(node: Node \| null): StabilityRegion` (`closest('[data-obs-region]')`, nearest wins, unknown or missing → `'other'`) |
| `classify.ts` | ≤30 | `isUnpromptedShift(e: Pick<ShiftEvent,'phase'\|'hadRecentInput'\|'resizing'>): boolean` is the **single** predicate, used by the dev sink (level), the mock analyzer, and part a's `layoutShiftsUnprompted` counter |
| `input.ts` | ≤70 | `attachInputTracker(): () => void` (window, capture, passive: `pointerdown keydown wheel touchstart`); `inputWithin(t: number, ms: number): boolean` (an input at `τ ∈ [t−ms, t]`, ring of the last 8 `event.timeStamp`s); `attachResizeTracker(): () => void` (`resize` + `matchMedia` dppx change); `resizeWithin(t, ms): boolean` |
| `phase.ts` | ≤60 | `markBoot(): void`; `markInteractive(): void` (no-op if already interactive); `phaseAt(t: DOMHighResTimeStamp): StabilityPhase`; `onPhaseChange(fn: (p: StabilityPhase) => void): () => void`; `resetPhaseForTests()` |
| `layoutShift.ts` | ≤110 | `layoutShiftSupported(): boolean`; `attachLayoutShift(emit: StabilitySink, first: boolean): () => void` |
| `focusLoss.ts` | ≤130 | `attachFocusLoss(emit: StabilitySink): () => void` |
| `sinks.ts` | ≤60 | `addStabilitySink(fn: StabilitySink): () => void`. The **0→1 transition attaches** all observers/trackers and **1→0 detaches** them. `emit` fans out to every sink |
| `devSink.ts` | ≤120 | `createDevSink(): { sink: StabilitySink; flushAll(): void }` (coalescing, §3.5) |
| `install.ts` | ≤50 | `startStability(): void`, idempotent. Registers or unregisters the dev sink on `onObsConfigChange` according to `obsEnabled()`. On disable it calls `flushAll()` **before** unregistering. That leaves the sink clean for re-enable. The flushed records are **dropped by design** by the `logRecord` obs gate, which is already off, and Rust closes its session on disable (`mod.rs:209-214`). *Amended 2026-09-24, reviewer ruling.* While registered it also listens for `pagehide` and `visibilitychange`→hidden, and runs `flushAll(); flushNow()` so the buckets ship in the same tick. |

Wiring: call `startStability()` once, next to the first `configureObs` call (senior-dev locates it via
`enabled.ts` consumers). Part a's mock counter sink registers through `addStabilitySink` from
`useMockDevHooks.ts` and never touches `devSink`.

**Constants (one place, `stability/types.ts`):** `INPUT_WINDOW_MS = 500`, `RESIZE_WINDOW_MS = 500`,
`FOCUS_SETTLE_MS = 50`, `SHIFT_BUCKET_MS = 1000`, `BOOT_BUCKET_MAX_MS = 30_000`.

### 3.3 Phase (`phase.ts`)

```
transitions: ring(8) of {at: performance.now(), phase}; initial = boot at 0
markBoot():        push {now, boot}           # call at repo open/switch START
markInteractive(): if last.phase != interactive: push {now, interactive}; notify
phaseAt(t):        phase of the last transition with at ≤ t (default boot)
```
**`markInteractive()` definition.** Fire when, for the *current* repo, the first graph `meta` **and**
the first status round have both been handled. A handled error or an empty result counts as landed.
Also fire on the no-repo empty state (folder-picker prompt mounted), so the phase can't stay boot
forever. **`markBoot()`** fires when a repo open or switch begins. Senior-dev picks the call sites and
must not grow `RepoWorkspace.tsx` (2262, baselined). A one-line call in an existing hook is fine.
Note: `e2e`'s `waitForGraphSettled` waits for the *full* graph height plus the status heading, which
is later than "first meta". The lab (part a) keeps using it, and the two don't need to coincide.

**Accepted deviations (2026-09-24, part-a reviewer ruling):**
- **Readiness is detected in the IPC layer.** It lives in `stability/readiness.ts`, via
  `tapReadiness()` wrapping the IPC object in `src/ipc/index.ts`. The only call sites for
  `getStatus`/`streamGraph` are in `RepoWorkspace.tsx`, which may not grow.
  - The cost with Dev mode off: one Proxy `get` trap per IPC call, plus one `.finally` per
    `getStatus`/`streamGraph`.
  - That breaks the letter of P91 §11 ("zero allocations per IPC call beyond today") but stays well
    within its frame budget. Accepted.
- **Where the mock counter sink registers.** It registers in `install.ts` at module level, not from
  `useMockDevHooks.ts`. StrictMode's 1→0→1 remount would otherwise lose the A7 buffered replay.
- **The empty state only marks interactive after session restore has settled.** Otherwise shifts
  during restore would be labelled interactive.

### 3.4 Layout-shift capture (`layoutShift.ts`)

```
layoutShiftSupported(): typeof PerformanceObserver !== 'undefined'
                        && PerformanceObserver.supportedEntryTypes?.includes('layout-shift') === true
attachLayoutShift(emit, first):
  if !supported → return noop                         # WebKit: silently absent, never throws
  attachedAt = performance.now()
  buffered = first && phaseAt(attachedAt) == 'boot'   # A7
  obs = new PerformanceObserver(list => list.getEntries().forEach(handle))
  obs.observe({ type: 'layout-shift', buffered })
  handle(e):
    if !buffered && e.startTime < attachedAt → return
    regions = distinct(e.sources.map(s => regionOf(s.node)))  ; if empty → ['other']
    ev = { phase: phaseAt(e.startTime),
           hadRecentInput: e.hadRecentInput || inputWithin(e.startTime, INPUT_WINDOW_MS),
           resizing: resizeWithin(e.startTime, RESIZE_WINDOW_MS),
           value: Number.isFinite(e.value) ? e.value : 0,
           at: Math.round(performance.timeOrigin + e.startTime) }
    regions.forEach(r => emit({ type: 'shift', region: r, ...ev }))
  return () => obs.disconnect()
```
`first` is true only for the first attach in the page's life (`sinks.ts` tracks this).

### 3.5 Dev-sink coalescing (`devSink.ts`)

```
key = region|phase|hadRecentInput|resizing
on shift e:
  b = buckets[key] ?? open(key, first=e.at, timer = e.phase=='interactive'
                                   ? setTimeout(flush(key), SHIFT_BUCKET_MS)
                                   : setTimeout(flush(key), BOOT_BUCKET_MAX_MS))
  b.shifts++; b.value += e.value; b.last = e.at
onPhaseChange(→ interactive): flush every boot bucket          # the boot summary (A11)
flush(key): clearTimeout; delete; logRecord({ kind: 'layout.shift',
     lvl: isUnpromptedShift(b) ? 'info' : 'debug', region, phase, hadRecentInput, resizing,
     shifts, value: round4(b.value), firstAt: b.first, spanMs: max(0, b.last - b.first) })
on focus-lost e: logRecord({ kind: 'focus.lost', lvl: 'info', fromRegion, phase, via, lostAt: e.at })
flushAll(): flush every bucket
```
`logRecord` already no-ops when `!obsEnabled()` and applies the level filter (AC 12, belt and braces).

### 3.6 Focus-loss classifier (`focusLoss.ts`)

```
tracked: { el: Element, region } | null ; checking: WeakSet<Element> ; windowBlurAt = -∞
focusin (document, capture):  if target is Element && target !== body: tracked = {target, regionOf(target)}
focusout(document, capture):  if tracked?.el === target && relatedTarget == null → schedule('focusout')
MutationObserver(body, {childList, subtree}): if tracked && !tracked.el.isConnected → schedule('detached')
window 'blur':                windowBlurAt = performance.now()
schedule(via0): t = tracked; at = performance.now(); if checking.has(t.el) return; checking.add(t.el)
                setTimeout(() => classify(t, at, via0), FOCUS_SETTLE_MS)
classify(t, at, via0):
  A = document.activeElement == null || document.activeElement === document.body
  W = document.hasFocus() && document.visibilityState === 'visible'
      && windowBlurAt < at - INPUT_WINDOW_MS          # no window blur in [at−500, classify]
  I = inputWithin(at, INPUT_WINDOW_MS)
  via = t.el.isConnected ? via0 : 'detached'
  if A && W && !I → emit({ type: 'focus-lost', fromRegion: t.region, phase: phaseAt(at), via,
                           at: Math.round(performance.timeOrigin + at) })
  if A && tracked === t → tracked = null      # element lost either way; no double report
  checking.delete(t.el)
```
The MutationObserver callback does O(1) work: one `isConnected` read, and nothing else.

**Truth table** (T = loss emitted; every row is a vitest case):

| # | Focus at settle (A) | Window focused (W) | Input ≤500 ms (I) | Result | Scenario |
|---|---|---|---|---|---|
| 1 | body | yes | no | **T** | refresh remounted the focused row |
| 2 | body | yes | yes | – | user clicked empty space / pressed Esc |
| 3 | body | no (blur ≤500 ms / hidden) | no | – | user switched apps |
| 4 | other element | yes | no | – | focus moved programmatically to a real target |
| 5 | other element | yes | yes | – | Tab navigation |
| 6 | other element (dialog input autofocused in `useEffect`, briefly body at `focusout`) | yes | no | – | dialog opened; A is read at +50 ms, not at `focusout` |
| 7 | body, element detached, no `focusout` fired | yes | no | **T**, `via:'detached'` | Chromium removal path |
| 8 | nothing tracked (last focus was `body`) | — | — | – | no element to lose |

---

## 4. Region attributes

Put `data-obs-region` on an **existing** element (no net new lines) per the plan's table: `sidebar`
goes on `Sidebar.tsx`; `graph` on `WorkspaceGraphPane.tsx` (505, baselined, **same line**); `details`
on `StatusPanel.tsx` and `CommitPanel.tsx`; `diff` on `DiffView.tsx`; `dock` on `GitActivityDock.tsx`
and the AI dock root. When regions nest, the nearest ancestor wins, so a diff inside the details
panel reports as `diff`.

## 5. Mock parity (`VITE_MOCK_IPC=1`)

- `src/ipc/mock/obsAnomalyStability.ts` (new, ≤140): `analyzeStability(records: readonly LogRecord[]): LogRecord[]`.
  It is a batch mirror of §2.5 with the same constants and detail strings. Since it sees the whole
  ring, pending becomes "search the refreshes that follow". Called with **one line** from
  `analyzeAnomalies` in `obsAnomaly.ts`.
- Mock `log_append` needs no change: the new kinds flow into `obsRing` as-is.
- Real Chromium in the harness produces genuine `layout.shift` entries. Part a's
  `?perfInjectShift` / `?perfInjectFocusSteal` flags are the deterministic triggers for the
  harness demonstration of AC 9/10.

## 6. Disclosure copy (A12; ui-designer owns the wording)

- Extend the fixed "What Bonsai records" statement (`dev.ts:100` / `SettingsDevCaptureSection.tsx`)
  with one sentence: layout movement (by window area: sidebar, graph, details, diff, dock) **on
  Windows**, plus focus that moves to nowhere without input, on all platforms. No names, paths or
  content.
- The copy must not claim layout capture on macOS/Linux. Either use static "On Windows, …" phrasing,
  or pick the copy at runtime with `layoutShiftSupported()`. ui-designer chooses.
- `dev.capture-react` help text is **unchanged**.

---

## 7. Tests

**Rust**
- T-R1 `tests_anomaly_stability.rs` (declared from `anomaly.rs` in the same way as `tests_anomaly_slow`): an interactive, unprompted
  shift → one `unprompted-shift` warn with refs `[seq]` (AC 9).
- T-R2 `hadRecentInput:true` → none; `resizing:true` → none; `phase:boot` → none (AC 11).
- T-R3 `focus.lost` arriving **before** the `refresh` whose `[ts−ms−50, ts+250]` contains `lostAt` →
  one warn, refs `[loss, refresh]`, traces = `contributingTraces`, anomaly `ts` = the loss `ts` (AC 10, A1).
- T-R4 `watcher{fired:true, suppressed:false}` 400 ms before the loss → warn with the watcher ref.
  With `suppressed:true` → no cause.
- T-R5 `focus.lost` **after** the refresh settled, inside +250 ms → warn, found by look-back.
- T-R6 no cause → no anomaly until a record with `ts > lostAt + 5000` arrives, then one `info`.
  Session end drains pending ones as `info`.
- T-R7 bounds: 70 unmatched losses → `pending_len() ≤ 64`, and the overflow is emitted as info; 300
  causes → `causes_len() ≤ 256`.
- T-R8 (AC 13, P91 §3 binding rule) `LogRecord → append_record → read back` for both kinds plus both
  anomaly records, in **strict** and **raw**. Record lines are byte-identical across modes (header
  excluded), and `detail` contains no `/`, `\`, or `://`.
- T-R9 `tests_kind_parity.rs` (AC 14): `include_str!("../../../src/ipc/types/obs.ts")`, strip
  `//` and `/* */` comments, extract the `'…'` literals of the `export type LogKind =` block up to `;`,
  and assert set equality with `LogPayload::ALL_KINDS` in both directions. `OBS_SCHEMA_VERSION` stays
  3 (the existing `tests_schema_parity` already pins TS == Rust).
- T-R10 `tests_kind_parity.rs`: one sample value per variant, each `.kind()` is in `ALL_KINDS`, and the set of
  sample kinds == `ALL_KINDS`.
- T-R11 `tests_record.rs` addition: serde round trip of both payloads matching the §2.2 wire JSON (every
  valid region). `region:"C:\\x"` / `"foo"` deserialises to `Other` without rejecting a batch that also
  holds valid records.
- T-R12 refactor proof: `cargo nextest` count unchanged across the §2.3 move (reported in the increment).

**Frontend (vitest; observer, `document` and timers faked)**
- T-F1 `regionOf`: nested (nearest wins), no attribute → `other`, `null` node → `other`.
- T-F2 `phaseAt`: boot → interactive → `markBoot` → interactive. An entry timestamped before
  `markInteractive` is `boot` even when delivered after it (A6).
- T-F3 unsupported path: no `PerformanceObserver`, and a missing `'layout-shift'` in `supportedEntryTypes`
  → `attachLayoutShift` returns a noop and never throws.
- T-F4 buffered replay: a second attach drops entries whose `startTime < attachedAt` (A7).
- T-F5 `hadRecentInput` ORs the API flag with the `wheel` tracker (A8). An entry with two sources in
  two regions → two events carrying the same `value` (A9).
- T-F6 focus truth-table rows 1–8 (§3.6).
- T-F7 dev sink: 3 interactive shifts in one region within 1 s → **one** `layout.shift` record,
  `shifts:3`, `lvl:'info'`. Boot shifts → one record per key at `markInteractive`, `lvl:'debug'`.
- T-F8 dev sink with `obsEnabled()` false → zero `logRecord` output. With no sinks registered, no
  observers or listeners are attached (spy on `PerformanceObserver`, `addEventListener`,
  `MutationObserver`) (AC 12).
- T-F9 turning dev mode off flushes the open buckets once, then detaches. It asserts that nothing is
  enqueued, because the obs gate drops them (amended 2026-09-24).
- T-F10 `analyzeStability` mirrors T-R1–T-R6 on the same fixtures (the shared fixture JSON lives in
  `src/ipc/mock/fixtures/`).
- T-F11 the `UiPayload` type test: `logRecord({kind:'layout.shift', …})` type-checks, and a payload
  missing `region` does not.
- T-F12 `isUnpromptedShift` truth table (all 8 combinations).
- T-F13 no-layout-read guard: in the T-F7 and T-F6 runs, spy on `Element.prototype.getBoundingClientRect` and
  `window.getComputedStyle` and assert zero calls.

**Harness / USER CHECKPOINT:** see the plan's "Browser harness" and "USER CHECKPOINT" bullets,
amended so the harness step also confirms that an `unprompted-shift` / `focus-steal` anomaly appears
in the mock ring when triggered by the part-a inject flags.

---

## 8. Acceptance criteria → verification

| Spec AC | Satisfied by | Verified by |
|---|---|---|
| 9 | §3.4–3.5 capture + `unprompted-shift` | T-R1, T-F7, T-F10, harness |
| 10 | §3.6 classifier + `focus-steal` warn with refs `[loss, cause…]` | T-R3–T-R5, T-F6, T-F10, harness |
| 11 | `hadRecentInput` (A8), classifier `I`, rule preconditions | T-R2, T-F5, T-F6 rows 2 and 5 |
| 12 | `sinks.ts` 0↔1 attach, `install.ts` gating, `logRecord` gate | T-F8, T-F9 |
| 13 | closed enums (A14), no-separator `detail`, pre-redaction detector input | T-R8, T-R11 |
| 14 | `ALL_KINDS` ↔ `LogKind` parity; version unchanged | T-R9, T-R10 |

Size discipline: no touched file crosses 500 lines. `record.rs` leaves the baseline, and
`WorkspaceGraphPane.tsx` / `RepoWorkspace.tsx` don't grow. No `security-auditor` path trigger applies.

## 9. Flags for the orchestrator

1. **A2 (spec wording — needs a user ruling):** AC 10's "within the same trace" is implemented as
   time-window correlation, plus trace equality when a trace exists. Pure trace joins aren't possible
   with today's trace model. Recommend accepting; otherwise the refresh round would need to carry a
   bound trace, which is a P91 §2.5 change.
2. **A6 phase re-arm:** recommended `markBoot()` on every repo open/switch. The alternative, one boot
   per app launch, would make every repo switch's streaming shifts `interactive`, and so anomalies.
3. **Unborn-HEAD repos:** senior-dev must confirm an empty repo still delivers a graph `meta`. If it
   doesn't, the empty-graph render path calls `markInteractive()` too (§3.3 "empty result counts").
4. **A3 verification:** the `detached` path assumes Chromium may skip `focusout` on removal. It is
   correct either way (`via` is taken from `isConnected`), and part a's `?perfInjectFocusSteal` must
   exercise the **removal** path, not `.blur()`.
