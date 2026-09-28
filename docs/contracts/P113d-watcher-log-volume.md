# P113d — Watcher log volume (amendment to P91 §2.4 / §3.1)

**Status:** contract **rev 3** (2026-09-28). Ready for senior-dev.

**Rev history.**
- rev 1 — evidence tally wrong; handler-local `NoiseAcc`. Superseded by rev 2.
- rev 2 — corrected tally (§0); delete per-batch record; shared `NoiseCounters`. User-approved goal.
- **rev 3** — re-baselined on the current tree: (a) `OBS_SCHEMA_VERSION` is now **3** (P117) — the
  "stays 2" statements are superseded, decision in §3; (b) P124b (`841566f`) added the all-ignored
  drop + 5-min heartbeat on the same fire path — §2.4 Timeout arm rewritten, §2.5 new, record
  taxonomy updated; (c) `BurstAcc`/`WatchTick` moved to `watcher/burst.rs`; (d) all `file:line`
  citations refreshed; (e) rev 2 §9 flag 1 resolved; (f) test list re-derived (5 payload literals
  + 5 `WatchTick` literals, not 1; new tests in new files).

**Scope:** Rust only, plus one TS type mirror. No IPC command/event/channel change, no frontend
behaviour change, no mock-IPC change, **not a UI change**. **`DEBOUNCE` (300 ms) is not touched** —
8,384 relevant batches coalescing into 192 fires is 44:1, i.e. the debounce works; the defect is
purely logging.

Amends `docs/contracts/P91-observability.md` §2.4 / §3.1; preserves
`docs/contracts/P110-watcher-burst-scoping.md` and the P124b drop rule (`watcher/burst.rs:142-179`)
in full.

## 0. Problem (figures unchanged from rev 2)

`bonsai-2026-09-22T04-52-03-scca8269e.jsonl`, 152 min, 5 repos, 19,547 records / 3.65 MB:

| bucket | count | share |
|---|---|---|
| `kind:"watcher"` | 12,001 | **61.4% of the log** |
| non-firing (`fired:false`) | 11,809 | 98.4% of watcher records |
| — of which `relevant > 0` | **8,384** | 71% of the waste |
| — of which `relevant == 0` | 3,425 | 29% |
| `fired:true` | 192 | 170 worktree / 22 refs |
| `suppressed:true` (frontend echo) | 11 | |
| `kind:"drop"` anywhere | **0** | the sink never fell behind |

Zero `drop` records ⇒ signal-to-noise, **not** data loss. Do not design for backpressure.

Cause: `watcher/mod.rs:185` logs **every** raw notify batch (`log_watcher(tick.paths,
tick.relevant, false, None, None, false)`), before the relevance send and before the debounce.
~~`mod.rs:204`~~ (rev 2 citation, stale).

**The redundancy that decides the design.** `mod.rs:186-188` forwards `relevant > 0` batches to the
debounce thread; `BurstAcc::absorb` (`burst.rs:72-80`) sums `paths`/`relevant` over the burst; the
fire at `mod.rs:232-239` logs those sums. So every `relevant > 0, fired:false` record is already
re-reported, summed, by the fired record closing its burst (verified on the baseline: fired sum
7,623 relevant, all-records sum 16,007, difference 8,384 = the non-fired relevant records). They can
be deleted with no information loss; only their *count* and burst *extent* need recovering — two
scalars on the fired record. Only `relevant == 0` batches are unrepresented anywhere (never sent),
so only they need a counter.

## 1. Decisions (unchanged from rev 2)

1. **The per-batch `log_watcher` call at `mod.rs:185` is deleted outright** — not level-gated.
   A gated variant is a config surface producing redundant records and a second "sometimes present"
   shape. Recovered by fired-record fields `batches`, `burstMs`, `errors` (§3). Per-batch arrival
   timing, if ever needed, belongs in a temporary debug build.
2. **`relevant == 0` counters ride on the next fired record**, held in a shared `NoiseCounters`
   behind an `Arc` (notify thread writes, debounce thread drains). A standalone noise summary is
   emitted only when noise accumulates with no fire. `paths` kept; rev 1's `first_at`/`skippedMs`
   dropped (a summary covers `(previous watcher record ts, this ts]`, bounded by `NOISE_FLUSH`).

**Consequence:** every Rust watcher record is emitted from the **debounce thread** (single writer,
totally ordered); the tail flush lands in its `Disconnected` arms, which `WatcherHandle::drop`
(`mod.rs:107-117`) already joins. See §6.

## 2. Design

### 2.1 Files

| File | Change |
|---|---|
| `src-tauri/src/watcher/noise.rs` | **NEW.** `NoiseCounters`, `SkippedNoise`, `NOISE_FLUSH`. |
| `src-tauri/src/watcher/emit.rs` | **NEW.** `log_watch_fired`, `log_watch_noise`. `log_watcher` (`mod.rs:54-97`) is **removed**, replaced by these two. |
| `src-tauri/src/watcher/burst.rs` | `WatchTick.error`; `BurstAcc.{batches, errors, started}`. |
| `src-tauri/src/watcher/mod.rs` | Handler: no logging, noise → counters. Debounce thread: §2.4. Module doc gains a P113d bullet. |
| `src-tauri/src/obs/record.rs` | 5 optional fields on `LogPayload::Watcher` (§3). |
| `src/obs/types.ts` | 5 optional fields on `WatcherPayload` (§3). |
| `src-tauri/src/state.rs` | `AppState::stop_all_watchers` (§6). |
| `src-tauri/src/lib.rs` | One call in the `ExitRequested` arm (§6). |

Not touched: `src/ipc/types/obs.ts` (`OBS_SCHEMA_VERSION = 3` stays), `src/ipc/mock.ts` and
handlers, `obs/anomaly*`, `useCoalescedRefresh.ts`.

### 2.2 State

```rust
// watcher/noise.rs
pub(super) const NOISE_FLUSH: Duration = Duration::from_secs(60);

/// `relevant == 0` notify batches not yet reported. Written by the notify
/// handler, drained by the debounce thread. Wait-free: two relaxed adds on the
/// producer, two relaxed swaps on the drain. No lock, no allocation, no IO
/// (obs/mod.rs guarantee 1: producers never block a git or UI path).
/// Ordering caveat: the two atomics are independent, so a drain interleaved
/// between a producer's two adds may attribute one batch's paths to the next
/// record. Totals stay exact.
#[derive(Default)]
pub(super) struct NoiseCounters {
    batches: AtomicU32,
    paths: AtomicU32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SkippedNoise { pub(super) batches: u32, pub(super) paths: u32 }

impl NoiseCounters {
    /// Notify thread. `batches.fetch_add(1)`, `paths.fetch_add(paths)`, Relaxed (§9.5).
    pub(super) fn absorb(&self, paths: u32);
    /// Debounce thread. `swap(0, Relaxed)` on both; `None` when batches == 0.
    pub(super) fn take(&self) -> Option<SkippedNoise>;
}
```

```rust
// watcher/burst.rs — additions only
pub(super) struct WatchTick {
    /* existing: paths, relevant, refs, worktree_paths, worktree_count */
    /// P113d: this tick is a notify `Err(_)` (true only on that arm).
    pub(super) error: bool,
}

pub(super) struct BurstAcc {
    /* existing: paths, relevant, refs, worktree_paths, worktree_total */
    /// P113d: notify batches coalesced (saturating, +1 per absorb incl. `new`).
    pub(super) batches: u32,
    /// P113d: `Err(_)` ticks coalesced (saturating, +1 per `t.error`).
    pub(super) errors: u32,
    /// P113d: `Instant::now()` in `new`; `burst_ms = started.elapsed()` at fire.
    pub(super) started: Instant,
}
```

`accumulate()` (`burst.rs:91-100`, `#[cfg(test)]`) **keeps its signature** `(u32, u32, BurstClass)`
so `burst_accumulation_is_conservative` (`tests.rs:252-293`) stays untouched. *(Supersedes rev 2
§2.1 "accumulate returns batches and errors as well".)* New tests read `acc.batches`/`acc.errors`
off a `BurstAcc` directly.

P124b note: noise never enters `BurstAcc` (`relevant == 0` ticks are not sent, `mod.rs:186`), so
`drop_eligible`'s `acc.paths <= DROP_MAX_PATHS` (`burst.rs:145-150`) is unaffected by P113d.

### 2.3 Emit sites (replace `log_watcher`)

```rust
// watcher/emit.rs
/// Debounce thread. One debounce firing — dispatched OR P124b-dropped — with
/// the burst sums plus any noise drained with it. Sets batches = Some(acc.batches),
/// burst_ms = Some(u32::try_from(acc.started.elapsed().as_millis()).unwrap_or(u32::MAX)),
/// errors = (acc.errors > 0).then_some(acc.errors), skipped_* from `skipped`.
/// `suppressed`/`suppress_reason: "ignored"` and the P121d ignore fields exactly
/// as `mod.rs:79-93` today.
pub(super) fn log_watch_fired(
    acc: &BurstAcc,
    class: BurstClass,
    ignore: Option<&IgnoreStats>,
    dropped: bool,
    skipped: Option<SkippedNoise>,
);

/// Debounce thread. Noise with no fire to carry it:
/// `paths: 0, relevant: 0, fired: false, suppressed: false`, every optional
/// field None except skipped_batches (> 0) / skipped_paths.
pub(super) fn log_watch_noise(skipped: SkippedNoise);
```

Both: `TraceMeta::root("watcher")`, `LogLevel::Debug`, `debounce_ms = DEBOUNCE`, `repo: None`
(unchanged, see §9.2). The notify thread no longer emits at all.

**Drain before checking the sink.** Callers call `noise.take()` unconditionally and pass the result
in; the emit fns return early on `active_sink() == None`. So a Dev-mode-on transition never reports
batches from an unlogged period.

### 2.4 Pseudocode

```
notify handler (mod.rs:141-189; only change: no logging):
    tick = classify_as_today(res)        // Ok arm literal gains `error: false`;
                                         // Err(_) => paths 1, relevant 1, refs true, error true
    if tick.relevant == 0:
        noise.absorb(tick.paths)         // two relaxed adds; return
    else:
        let _ = tx.send(tick)            // unchanged
```

```
debounce thread (mod.rs:198-251):
    last_fire = Instant::now()                        // P124b heartbeat clock, unchanged
    loop:
        match rx.recv_timeout(noise_flush):           // OUTER only (was rx.recv())
            Err(Timeout)      -> if let Some(s) = noise.take() { log_watch_noise(s) }; continue
            Err(Disconnected) -> if let Some(s) = noise.take() { log_watch_noise(s) }; return
            Ok(first) ->
                acc = BurstAcc::new(first)
                loop:                                 // INNER: unchanged control flow
                    match rx.recv_timeout(DEBOUNCE):
                        Ok(t) -> acc.absorb(t); continue
                        Err(Timeout) ->               // = mod.rs:216-245 + noise
                            class   = acc.class()
                            dev     = active_sink().is_some()
                            stats   = if drop_eligible(&acc) || dev { ignore_stats(&workdir, &acc) } else { None }
                            verdict = burst_verdict(&acc, stats.as_ref(), last_fire.elapsed())
                            dropped = verdict == DropIgnored
                            log_watch_fired(&acc, class, stats.as_ref(), dropped, noise.take())
                            if !dropped { on_change(class); last_fire = Instant::now() }
                            break
                        Err(Disconnected) ->          // today's mod.rs:246: bare return
                            if let Some(s) = noise.take() { log_watch_noise(s) }
                            return                    // NO fired record, NO on_change
```

For deterministic testing, extract the thread body into
`fn debounce_loop(rx: Receiver<WatchTick>, noise: Arc<NoiseCounters>, workdir: PathBuf,
on_change: Box<dyn Fn(BurstClass) + Send>, noise_flush: Duration)` (in `mod.rs`, or a new
`watcher/debounce.rs` if `mod.rs` would pass ~350 lines). Production passes `NOISE_FLUSH`; tests pass
a short value. `DEBOUNCE` stays a `const`, not a parameter.

**The inner `Disconnected` arm must NOT emit a fired record** (rev 2 trap, preserved). A burst in
flight at teardown never dispatches `on_change`; logging `fired: true` would lie about a refresh,
break criteria 4/5, and — because `detect_watcher_storm` keys on the single global `"watcher"` key
(`anomaly/window.rs:262-277`; `W_WATCHER_MS = 1000` `:17`, `WATCHER_STORM_MIN = 5` `:22`) — make
`stop_all_watchers()` with ≥ 5 repos stamp a spurious `watcher-storm` on every app exit.

**Only the OUTER receive becomes `recv_timeout(noise_flush)`.** It runs only with no burst in flight,
so it cannot move fire timing. It does not touch `last_fire`, so the P124b heartbeat is unaffected.
Cost: one idle wake per minute per open repo.

**Rejected** (one line each): level-gated per-batch logging (§1.1); noise ticks down the channel
(would extend the debounce window → perturbs P110 timing); handler-local `NoiseAcc` with `Drop`
tail (rev 1: carrier-less, and depends on unspecified upvar drop order, §6).

### 2.5 P124b interaction (new in rev 3)

Derived from the current `watcher/{mod,burst}.rs` source.

| P124b element | Effect of P113d |
|---|---|
| Dropped all-ignored burst → `fired:true, suppressed:true, suppressReason:"ignored"` record, no `on_change` (`mod.rs:229-243`) | Unchanged. Still exactly one fired record per burst; it carries `batches`/`burstMs`/`errors?`/`skipped*?` like any fired record. |
| Heartbeat (`IGNORED_HEARTBEAT = 300 s`, `burst.rs:24`) | A verdict **input** (`last_fire.elapsed()`), **not a record**. A heartbeat-forced fire is an ordinary fired record; no separate carrier, no new field. |
| Ignore check (`drop_eligible || dev`) | Unchanged; runs before the emit. |
| Error tick | `refs: true` ⇒ `drop_eligible == false` ⇒ an error burst is **never** dropped; its `errors` always lands on a `suppressed:false` fired record. |
| Record count | P124b adds/removes no records (dropped bursts are still logged). The ≤ 1,200 target and "≈ 192 fired records" still hold; `on_change` count = fired ∧ ¬suppressed ≤ fired count. |

## 3. Record schema — additive, **`OBS_SCHEMA_VERSION` stays `3`**

~~"no bump (stays `2`)"~~ — rev 2, stale: P117 moved it to 3 (`record.rs:32-40, :54`); the TS mirror
`src/ipc/types/obs.ts:22` is 3 and `obs/tests_schema_parity.rs` guards parity. P113d does **not**
bump to 4.

**Why.** The rule (`record.rs:18-19`; P91 `:247-249`): bump only when an EXISTING field changes
shape or meaning such that a reader can tell old from new **only by the header**. P117 bumped
because an identical `anomaly` record changed meaning with no per-record discriminator. P113d:
- no existing field is retyped, removed, or changes meaning (`paths`/`relevant` still describe the
  record's own burst; `fired`/`suppressed` unchanged);
- the population change is **self-describing per record**: every post-P113d Rust fired record
  carries `batches` (always `Some`, ≥ 1); every noise summary carries `skippedBatches` (> 0); no
  pre-P113d record carries either. A reader discriminates by field presence (§3.2);
- precedent on this same variant: P124b gave `fired:true` a new "no refresh ran" reading via
  `suppressed`/`suppressReason: "ignored"` with no bump, and `render.tally.strictModeDoubling`
  (`record.rs:301-308`) is additive-optional with no bump.

Hence `batches` and `burstMs` are **always present on Rust fired records** (not "omitted when 0") —
they are the discriminator. *(Tightens rev 2, which allowed them absent.)*

```rust
// obs/record.rs — LogPayload::Watcher, appended after `ignored` (:265).
/// P113d: notify batches coalesced into this burst. ALWAYS present on a Rust
/// `fired` record written by a P113d build (≥ 1) — its presence is how a
/// reader knows the per-batch `fired:false` records no longer exist (§3.2).
#[serde(default, skip_serializing_if = "Option::is_none")]
batches: Option<u32>,
/// P113d: ms from the burst's first batch to the fire (burst start =
/// ts − burstMs − debounceMs). Always present with `batches`.
#[serde(default, skip_serializing_if = "Option::is_none")]
burst_ms: Option<u32>,
/// P113d: notify `Err(_)` events in this burst. Fired records only; omitted when 0.
#[serde(default, skip_serializing_if = "Option::is_none")]
errors: Option<u32>,
/// P113d: notify batches the `.git`-internals filter rejected whole
/// (`relevant == 0`) since this watcher's previous record. On fired records
/// AND noise summaries; omitted when 0. Never set by the frontend.
#[serde(default, skip_serializing_if = "Option::is_none")]
skipped_batches: Option<u32>,
/// P113d: total paths in those batches (see NoiseCounters ordering caveat).
#[serde(default, skip_serializing_if = "Option::is_none")]
skipped_paths: Option<u32>,
```

```ts
// src/obs/types.ts — WatcherPayload (:101-123), appended after `ignored?`
/** P113d — Rust `fired` records only. `batches`/`burstMs` are always present on
 *  a P113d-era Rust firing; `errors` only when > 0. */
batches?: number;
burstMs?: number;
errors?: number;
/** P113d — irrelevant notify batches folded into this record (Rust fired
 *  records and noise summaries; never on frontend echo records). */
skippedBatches?: number;
skippedPaths?: number;
```

No TS code constructs Rust watcher records; the echo producer (`useCoalescedRefresh.ts:165-174`)
needs no change. The `OBS_SCHEMA_VERSION` mirror is untouched.

**Naming.** `skipped*`, not `suppressed*`: `suppressed`/`suppressReason` now mean "a firing that ran
no refresh round" and have two producers (`record.rs:226-247`: frontend `"echo"`, Rust P124b
`"ignored"`). Noise is not a firing. *(rev 2 said "reserved for the frontend echo path" — superseded
by P124b; the conclusion stands.)*

### 3.2 Reader rule (add to P91 §3.2)

A session is **P113d-era** iff any Rust (`source:"rust"`) `watcher` record has `batches` or
`skippedBatches` present (a session with zero Rust watcher records is trivially both). For a
P113d-era session:

```
raw_notify_batches = Σ batches      + Σ skippedBatches
raw_notify_paths   = Σ paths[fired] + Σ skippedPaths
relevant_paths     = Σ relevant
notify_errors      = Σ errors
debounce_firings   = count(fired == true)                    // incl. P124b-dropped
refresh_firings    = count(fired == true && !suppressed)     // what ran on_change
```

Pre-P113d files: `raw_notify_batches = count(source=rust, fired=false)`, as before.

Exactness holds over **completed** bursts; a burst in flight at teardown is dropped without a record
(unchanged from today's `mod.rs:246`), bounded to one burst per watcher per shutdown.

Record taxonomy (rev 3 — adds the P124b row):

| record | source | `fired` | `suppressed` | `relevant` | carries |
|---|---|---|---|---|---|
| debounce firing | rust | true | false | ≥ 1 | `paths`, `relevant`, `burstClass`, `batches`, `burstMs`, `errors?`, `skipped*?`, P121d `worktreePaths?`/`ignoreChecked?`/`ignored?` |
| dropped firing (P124b) | rust | true | true (`"ignored"`) | ≥ 1 | same as above; `errors` never (error ⇒ refs ⇒ not droppable) |
| noise summary | rust | false | false | 0 | `paths: 0`, `relevant: 0`, `skippedBatches > 0`, `skippedPaths` |
| frontend echo | ui | false | true (`"echo"`) | 0 | `causedBy?`; never `skipped*`/`batches` |

`paths`/`relevant` describe the carrier burst only; the skipped set is disjoint (a `relevant == 0`
batch can never enter a `BurstAcc`) — never add them without the rule above.

## 4. What must NOT be lost

1. **Watcher errors.** `Err(_)` stays `WatchTick { paths: 1, relevant: 1, refs: true, error: true,
   .. }` → always a burst, always `Refs`, never P124b-dropped, so the wide refresh runs. Reported
   ≤ (burst length + 300 ms) later by `errors` on the fired record closing its burst. **Every notify
   error is counted by exactly one fired record** — dropped-event recovery; a lost signal here is a
   correctness bug.
2. **Burst classification (P110)** and **the P124b drop rule**: `burstClass`, the conservative
   `refs` fold, `burst_verdict`, `last_fire`, `on_change`, `DEBOUNCE`, the inner drain loop — all
   untouched.
3. **Frontend echo** (`suppressReason: "echo"`): other producer, untouched, distinguishable by
   `source`.
4. **"Did the watcher see anything in this window?"** stays answerable: relevant batches via fired
   records, irrelevant via `skipped*`. Drains: the fire, the `NOISE_FLUSH` tick (a genuine
   wall-clock bound — the thread wakes itself), shutdown. Zero watcher records in a window means
   zero fs events in it.
5. **Anomaly consumers.**
   - `watcher-storm`: `anomaly.rs:161-164` calls `detect_watcher_storm` on `fired == true` only;
     `window.rs:262-277` needs only `ts`/`seq`. Fired records are unchanged in number and timing
     (the §2.4 inner-`Disconnected` rule keeps that true at shutdown); noise summaries are
     `fired:false` ⇒ never counted.
   - `stability.on_watcher` (`anomaly/stability.rs:162-186`): needs `fired && !suppressed` plus
     `rec.trace`/`rec.caused_by`. Both unchanged on fired records (`TraceMeta::root("watcher")`);
     noise summaries return early at `:176`.
   - **Pre-existing, not P113d:** `detect_watcher_storm` also counts P124b-dropped
     (`suppressed:true`) firings, and `tests_anomaly.rs:194-199` feeds `fired:false` for its "not
     fired" negative. Do **not** change either in this increment (§9.6).

## 5. Guarantees preserved (obs/mod.rs module invariants)

- **Nothing blocks a git or UI path.** Irrelevant batch: two relaxed `fetch_add`s. Relevant batch:
  one channel `send`, as today. The notify thread no longer builds, enqueues or serialises any
  record — strictly less work than today. Emission stays `sink.enqueue` (`try_send`), one thread.
- **Nothing deletes a log file.** No change to pruning, retention or the writer.
- Watcher stays Tauri-decoupled (no `AppHandle`), reaching the sink via `obs::trace::active_sink()`.

## 6. Tail semantics (session end)

The debounce thread flushes noise in **both** `Disconnected` arms before returning;
`WatcherHandle::drop` (`mod.rs:107-117`) drops the watcher first (disconnect) and then joins that
thread. So `drop` returning ⇒ the tail record is enqueued. No reliance on closure upvar drop order
(rev 1's hazard, eliminated by the `Arc` design).

**Required — exit ordering.** Managed state (every `WatcherHandle`) drops only after the run loop
returns, i.e. after `obs::shutdown_on_exit` — the tail would hit `active_sink() == None` and be
lost. In the `ExitRequested` arm (`lib.rs:418-434`), insert immediately **before**
`obs::shutdown_on_exit(...)` at `lib.rs:427` (after `cancel_all()` at `:424`):

```rust
app.state::<state::AppState>().stop_all_watchers();
```

The arm has no `api.prevent_exit()` branch — exit is never cancelled after this point, so stopping
watchers cannot strand a still-running app without them.

```rust
// state.rs (77 lines today; method does not yet exist)
impl AppState {
    /// P113d §6: drops every repo's `WatcherHandle`. Collect via
    /// `entry.watcher.take()` under the `repos` lock (poison-recover like
    /// commands/repo.rs:301-305), RELEASE the lock, then drop the Vec —
    /// `Drop` joins the debounce thread and must not run under the mutex
    /// (the thread's `on_change` may itself touch app state). Idempotent;
    /// leaves every `RepoEntry` in place with `watcher: None`.
    pub fn stop_all_watchers(&self);
}
```

Join cost at exit: each debounce thread returns on `Disconnected` within at most one in-progress
`ignore_stats` / `on_change` call; no `DEBOUNCE` wait (the inner `recv_timeout` returns
`Disconnected` immediately once the sender is gone).

## 7. Tests

All new tests go in **new files**; no touched file may exceed ~500 lines. Current sizes:
`watcher/tests.rs` 391, `watcher/tests_ignore.rs` 342, `obs/tests_record.rs` 161,
`obs/tests_anomaly.rs` 389, `obs/tests_anomaly_support.rs` 344, `obs/tests_kind_parity.rs` 264,
`obs/record.rs` 441 (→ ~465 after §3).

**Must change (compile/shape fixes):**

| File:line | Change |
|---|---|
| `watcher/tests.rs:188-247` `watcher_record_shape` | Drives the removed `log_watcher`, and `:213` emits the deleted non-fired raw record. Rewrite against `log_watch_fired` (assert `paths`/`relevant`/`debounceMs`/`suppressed:false`/`burstClass`, plus `batches`/`burstMs` present) and `log_watch_noise` (assert `burstClass` absent, `skippedBatches` present). Keep `test_sink_lock()` + `set_active_sink`. |
| `watcher/tests_ignore.rs:52`, `:69`, `:325` | `LogPayload::Watcher` literals: add the 5 fields as `None`. `:69` (`raw`, `fired:false`) stays as a legacy-shape serde check; extend its omitted-key loop (`:83`) with the 5 new camelCase keys. |
| `watcher/tests_ignore.rs:12`, `:96`, `:198`, `:291`, `:305` | Exhaustive `WatchTick { .. }` literals (no `..WatchTick::default()`): add `error: false`. Mechanical only — no assertion changes. (`BurstAcc` is only ever built via `BurstAcc::new`, so its new fields need no test edits.) |
| `obs/tests_anomaly_support.rs:195` | `watcher()` helper literal: add the 5 fields as `None`. |
| `obs/tests_kind_parity.rs:140` | Literal: add the 5 fields as `None`. |
| `src/obs/types.ts:101-123` | Type mirror only (not a test). |

`burst_accumulation_is_conservative` (`tests.rs:252-293`) and the P110/P124b integration tests
(`tests.rs:79-186, :297-391`; `tests_ignore.rs:131-194`) must pass **with no assertion edits**
(`tests.rs` uses `..WatchTick::default()`; `tests_ignore.rs` gets only the `error: false` literal
fix above). The production `Ok` arm literal (`mod.rs:167-173`) likewise gains `error: false`.

**Must add — `src-tauri/src/watcher/tests_noise.rs` (NEW; `#[cfg(test)] mod tests_noise;` in
`mod.rs`).** Loop tests drive `debounce_loop` directly over a hand-fed `mpsc` channel + a fresh
`Arc<NoiseCounters>`, with `test_sink_lock()` + a temp `Sink` (the `tests.rs:193-209` pattern),
short `noise_flush` (e.g. 50 ms), no notify watch:

1. `no_record_until_the_burst_fires` — N relevant ticks sent back-to-back: zero watcher records
   while ticks keep arriving; exactly one `fired:true` record after the quiet period.
2. `fired_record_carries_batches_and_burst_ms` — `batches == N`, `burstMs` present, `paths` /
   `relevant` equal the tick sums.
3. `noise_rides_on_next_fired_record` — `absorb` M times, then one relevant tick → one fired
   record with `skippedBatches == M`, `skippedPaths` = sum; `take()` afterwards is `None`.
4. `noise_summary_when_no_fire` — `absorb` only, wait > `noise_flush` → one record `paths:0,
   relevant:0, fired:false, suppressed:false, skippedBatches == M`.
5. `error_tick_counted_and_refs` — one `error: true` tick → exactly one fired record with
   `errors == 1`, `burstClass == "refs"`, `suppressed == false`; N error ticks in one burst →
   `errors == N`.
6. `raw_batch_count_recoverable` — mixed sequence (relevant, irrelevant, errors, several fires);
   §3.2 formulas equal the true batch / path / error counts exactly.
7. `noise_counters_drained_when_sink_absent` — `set_active_sink(None)`, absorb, let a fire / flush
   happen → counters zero; then enable the sink, fire once → no `skipped*` on that record.
8. `teardown_mid_burst_emits_no_fired_record` — send a tick, absorb noise, drop the sender before
   `DEBOUNCE` elapses → one noise summary, zero `fired:true`, `on_change` never called.
9. `dropped_burst_carries_aggregates` — P124b: build a drop-eligible `BurstAcc` (re-declare the
   `tests_ignore.rs:196-218` `wt_acc`/`stats` shapes locally) and call
   `log_watch_fired(.., dropped = true, Some(noise))` → `suppressed:true, suppressReason:"ignored"`
   with `batches`/`burstMs`/`skipped*` present.
10. `noise_counters_absorb_take` — pure: `take()` on fresh = `None`; absorb(3), absorb(0) →
    `Some { batches: 2, paths: 3 }`; second `take()` = `None`.

Integration, same file, under `serialize_watcher_test()` (`tests.rs:20`; reuse `fixture_repo`
`:27`):

11. `tail_noise_record_on_handle_drop` — `spawn_watcher` on the fixture repo with an active sink,
    write under `.git/objects/zz/` (irrelevant), drop the `WatcherHandle`, then `sink.shutdown()` →
    a record with `skippedBatches > 0` is on disk. (Windows may coalesce: assert ≥ 1 such record,
    never an exact count.)

**Must add — `src-tauri/src/obs/tests_anomaly_watcher.rs` (NEW; register in `obs/mod.rs` next to
the `tests_anomaly` declaration; reuse `tests_anomaly_support`'s harness / `watcher()` helper):**

12. `noise_summaries_never_feed_watcher_rules` — feed 20 noise-summary-shaped records (`fired:false,
    skippedBatches: Some(k)`) within 1 s: zero `watcher-storm`, and no watcher cause credited by a
    following stability loss.
13. `watcher_storm_detection_unchanged` — the same ts sequence of fired records with and without
    the P113d fields set → identical anomaly output.

**Must add — `obs/tests_record.rs`** (161 lines; room):
`watcher_record_key_set_unchanged_when_new_fields_absent` — serialise `LogPayload::Watcher` with the
5 new fields `None` and assert the JSON key set equals the pre-P113d set; with all 5 `Some`, assert
the camelCase keys `batches`, `burstMs`, `errors`, `skippedBatches`, `skippedPaths` appear.

**`AppState::stop_all_watchers`:** a unit test in a NEW `src-tauri/src/state_tests.rs`
(`#[cfg(test)] #[path]`-registered from `state.rs`) — two entries with `watcher: None` and one with
a real handle (`fixture_repo` is `pub(super)` inside `watcher::tests`, unreachable here: build a
`tempfile::TempDir` + `git2::Repository::init` inline); call twice → no panic, all `watcher`
`None`, entries kept.

**Must still pass unchanged (guards, do not edit):**
`src/components/repoWorkspace/useCoalescedRefresh.causality.test.tsx`,
`src/components/repoWorkspace/useCoalescedRefresh.test.tsx`, `obs/tests_anomaly_stability.rs`,
`obs/tests_stability_fixture.rs`, `obs/tests_schema_parity.rs`. `src/ipc/mock/handlers/session.ts`
and `src/graph/replay/*` need no change.

## 8. Acceptance criteria

Structural (unit-testable):

1. **Zero** Rust watcher records with `fired == false` and `skippedBatches` absent.
2. Every Rust `fired == true` record has `batches` and `burstMs` present.
3. §3.2 `raw_notify_batches`, `raw_notify_paths`, `notify_errors` equal the true values exactly on a
   synthesised mixed sequence (completed bursts).
4. Fire count, fire timing, `burstClass`, P124b verdicts and `on_change` calls are **identical** to
   pre-P113d for the same tick sequence, including at teardown; P110/P124b tests pass with no
   assertion edits; `DEBOUNCE` unchanged.
5. Every notify error is counted by exactly one fired record's `errors`, and that record is
   `suppressed:false`.
6. At most one noise summary per `NOISE_FLUSH` per watcher.
7. After `stop_all_watchers()` returns, the tail record is on the sink (test 11).
8. `OBS_SCHEMA_VERSION` still **3** on both sides (`record.rs:54`, `src/ipc/types/obs.ts:22`); a
   pre-P113d reader parses every new record; the key set with no new field set is unchanged.
9. `watcher/mod.rs` no longer references `log_watcher`; the notify closure contains no obs call.

Empirical, on a comparable Dev session (≥ 3 repos, similar length/activity) vs the §0 baseline:

10. Watcher records fall **≥ 90%** (12,001 → ≤ 1,200; projected ≈ 192 fired + a few hundred noise
    summaries); `debounce_firings` (incl. P124b-dropped) within normal variance of 192.
11. `kind:"watcher"` **≤ 10%** of total records (projected ≈ 6%).
12. Total records **≥ 50%** fewer and file size **≥ 45%** smaller (projected ≈ 8,000 / ~1.9 MB).
13. Still **zero** `kind:"drop"`.
14. No `watcher-storm` anomaly at app exit.
15. §3.2 `raw_notify_batches` is of the same order as the baseline's raw batch count for comparable
    activity (sanity check, not a tolerance).

## 9. Flags for the orchestrator

1. ~~**Pre-existing schema drift** (`src/ipc/types/obs.ts` = 1 vs Rust = 2, no parity test).~~
   **RESOLVED** (rev 3): both are 3 (`src/ipc/types/obs.ts:22`, `record.rs:54`) and
   `obs/tests_schema_parity.rs` guards it.
2. **Watcher records carry no repo identity.** P117 added `LogRecord::repo` (`record.rs:98-113`),
   but its doc comment is an explicit three-producer allow-list ("do not widen without a contract
   change"), and `make_record` sets `repo: None` (`trace.rs:129`). P113d keeps `repo: None`.
   Setting it for watcher records would make `watcher-storm` per-repo-keyable — a separate
   contract change; recommend deferring. With ≥ 5 repos the §2.4 inner-`Disconnected` rule is what
   keeps exit storm-free.
3. **`NOISE_FLUSH = 60 s`** is a judgement call; raise to 300 s if criterion 11 misses — one
   constant, no schema impact.
4. **Intra-burst timing is deliberately gone** (`batches` + `burstMs` give count and extent only).
5. **`u32` saturation.** Plain `fetch_add` on `AtomicU32` is acceptable (4.29 G batches between
   drains ≤ 60 s apart is unreachable); senior-dev notes the choice in the doc comment.
6. **Pre-existing, out of scope:** `detect_watcher_storm` counts P124b-dropped (`suppressed:true`)
   firings; arguably it should count dispatched firings only. P113d leaves it alone (criterion 4
   requires identical detection). File as a follow-up if wanted.
7. **Schema decision for confirmation:** rev 3 decides **no bump** (§3). Fallback if the
   orchestrator/user prefers a header-level signal: bump to 4 in `record.rs:54` +
   `src/ipc/types/obs.ts:22`, add a v3→v4 module note; `tests_schema_parity.rs` covers the pair.
   Recommendation: no bump — per-record discrimination (`batches` always present) is strictly more
   informative than a header, and matches the P124b / `strictModeDoubling` precedent.
