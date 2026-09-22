# P113d — Watcher log volume (amendment to P91 §2.4 / §3.1)

**Status:** contract rev 2 (rev 1's evidence tally was wrong — see §0). Ready for senior-dev.
**Scope:** Rust only. No IPC command/event/channel change, no frontend behaviour change, no
mock-IPC change, **not a UI change** (`ui-reference.md` out of scope). One TS type file gains five
optional fields mirroring the Rust record. **`DEBOUNCE` (300 ms) is not touched** — 8,384 relevant
batches coalescing into 192 fires is a 44:1 ratio, i.e. the debounce is working; the defect is
purely in logging.

Amends `docs/contracts/P91-observability.md` §2.4 / §3.1; preserves
`docs/contracts/P110-watcher-burst-scoping.md` in full.

## 0. Problem (corrected figures)

`bonsai-2026-09-22T04-52-03-scca8269e.jsonl`, 152 min, 5 repos, 19,547 records / 3.65 MB:

| bucket | count | share |
|---|---|---|
| `kind:"watcher"` | 12,001 | **61.4% of the log** |
| non-firing (`fired:false`) | 11,809 | 98.4% of watcher records |
| — of which `relevant > 0` | **8,384** | **71% of the waste** |
| — of which `relevant == 0` | 3,425 | 29% |
| `fired:true` | 192 | 170 worktree / 22 refs |
| `suppressed:true` (frontend echo) | 11 | |
| `kind:"drop"` anywhere | **0** | the sink never fell behind |

Zero `drop` records ⇒ signal-to-noise, **not** data loss. Do not design for backpressure.

Cause: `watcher/mod.rs:204` logs **every** raw notify batch, before the relevance filter and before
the debounce.

**The redundancy that decides the design.** `mod.rs:205` forwards `relevant > 0` batches to the
debounce thread; `BurstAcc` sums `paths`/`relevant` over the whole burst (`mod.rs:224-226`); the
firing at `:235` logs those sums. So **every one of the 8,384 `relevant > 0, fired:false` records
is already re-reported, summed, by the fired record that closes its burst.** Confirmed numerically:
fired records sum to 7,623 relevant; all watcher records sum to 16,007; the 8,384 difference matches
the 8,384 non-fired relevant records one-for-one. They can be deleted with **no accumulator and no
information loss** — only their *count* and *intra-burst timing* need recovering, which two scalars
on the fired record provide.

Only `relevant == 0` batches are unrepresented anywhere (they never send a tick), so only they need
a counter.

## 1. Decisions asked for

**1. The per-batch `log_watcher` at `:204` is deleted outright** — not gated behind a trace level,
not reduced to a counter for the relevant case. Rationale above: the fired record already carries
the sums, so a level-gated variant would be a config surface producing records no analysis needs,
and a second "sometimes present" record shape for readers to special-case. What deletion *does*
cost is recovered by three fired-record fields (§3): `batches` (how many notify batches were
coalesced), `burstMs` (burst start = `ts − burstMs − debounceMs`), `errors` (§4.1). If per-batch
arrival timing is ever needed again it belongs in a temporary debug build, not a shipped level.

**2. The `relevant == 0` counters ride on the existing fired record** and therefore move
**off** the notify thread into a shared `NoiseCounters` behind an `Arc`. Rev 1's handler-local
`NoiseAcc` piggybacked onto the next `relevant > 0` record — which no longer exists, leaving
standalone summaries as the only carrier. With the shared counter the 192 fired records absorb the
noise, and a standalone summary is emitted only when noise accumulates without a fire.
`paths` is **kept** (3,425 batches could be 3,425 or 50,000 paths — the magnitude is the point).
`first_at` / `skippedMs` are **dropped**: a summary covers `(previous watcher record ts, this ts]`,
which the timestamps already give, bounded by `NOISE_FLUSH` (§2.2). Two atomics, not three.

**Consequence:** every watcher record is now emitted from the **debounce thread** (single-writer,
totally ordered), and the tail flush lands in that thread's `Disconnected` arms — which
`WatcherHandle::drop` already joins. See §6.

## 2. Design

### 2.1 State

```rust
const NOISE_FLUSH: Duration = Duration::from_secs(60);

/// `relevant == 0` notify batches not yet reported. Written by the notify
/// handler, drained by the debounce thread. Wait-free: two relaxed adds on the
/// producer, two relaxed swaps on the drain. No lock, no allocation, no IO —
/// the obs invariant "producers never block a git or UI path" holds by
/// construction (obs/mod.rs guarantee 1).
#[derive(Default)]
pub(crate) struct NoiseCounters {
    batches: AtomicU32,
    paths: AtomicU32,
}

#[derive(Clone, Copy)]
struct SkippedNoise { batches: u32, paths: u32 }

impl NoiseCounters {
    /// Notify thread. `fetch_add(_, Relaxed)` on both (see §9.5 on saturation).
    fn absorb(&self, paths: u32);
    /// Debounce thread. `swap(0, Relaxed)` on both; `None` when batches == 0.
    fn take(&self) -> Option<SkippedNoise>;
}
```

`BurstAcc` (existing) gains three fields, the first two folded in `new`/`absorb` with
`saturating_add`:

```rust
struct BurstAcc {
    paths: u32,
    relevant: u32,
    refs: bool,
    batches: u32,          // notify batches coalesced into this burst
    errors: u32,           // notify Err(_) ticks in this burst (§4.1)
    started: Instant,      // set in `new`; burst_ms = started.elapsed() at fire
}
```
`WatchTick` gains `error: bool` (set only on the `Err(_)` arm) so `absorb` can count it.
`accumulate()` (the `#[cfg(test)]` pure fold) returns `batches` and `errors` as well; `started` is
loop-level state and stays out of the pure fold — `burstMs` is covered by a loop test instead.

### 2.2 Emit sites (replace the single `log_watcher`)

```rust
/// Debounce thread. One debounce firing: the burst sums plus any noise drained
/// with it. The ONLY record kind that carries burstClass/batches/burstMs/errors.
fn log_watch_fired(acc: &BurstAcc, skipped: Option<SkippedNoise>);

/// Debounce thread. Noise that accumulated with no fire to carry it:
/// `paths: 0, relevant: 0, fired: false, skipped* > 0`. Emitted on the
/// NOISE_FLUSH tick and on shutdown.
fn log_watch_noise(skipped: SkippedNoise);
```

Both keep `TraceMeta::root("watcher")`, `LogLevel::Debug`, `debounce_ms = DEBOUNCE`,
`suppressed: false`, `suppress_reason: None` (§4.3). **The notify thread no longer emits at all.**

Both must **drain before checking the sink**: `active_sink()` returning `None` (Dev mode off) still
consumes the counters, so a Dev-mode-on transition never reports batches from an unlogged period.

### 2.3 Pseudocode

```
notify handler (unchanged except: no logging):
    tick = classify_as_today(res)          // Err(_) => paths 1, relevant 1, refs true, error true
    if tick.relevant == 0:
        noise.absorb(tick.paths)           // two relaxed adds; returns
    else:
        let _ = tx.send(tick)              // unchanged
```

```
debounce thread:
    loop:
        match rx.recv_timeout(NOISE_FLUSH):          // OUTER only — see note
            Err(Timeout) ->
                if let Some(s) = noise.take() { log_watch_noise(s) }
                continue
            Err(Disconnected) ->
                if let Some(s) = noise.take() { log_watch_noise(s) }   // tail, §6
                return
            Ok(first) ->
                acc = BurstAcc::new(first)
                loop:                                 // INNER LOOP: unchanged control flow
                    match rx.recv_timeout(DEBOUNCE):
                        Ok(t)        -> acc.absorb(t); continue
                        Err(Timeout) -> log_watch_fired(&acc, noise.take());
                                        on_change(acc.class()); break
                        Err(Disconnected) ->
                                        // IDENTICAL to today's `mod.rs:239`: no fire,
                                        // no on_change, the in-flight burst is dropped.
                                        // Flush noise only, exactly like the outer arm.
                                        if let Some(s) = noise.take() { log_watch_noise(s) }
                                        return
```

**The inner `Disconnected` arm must NOT emit a fired record.** Today it is a bare `return`: a burst
in flight at teardown never dispatches `on_change`, so logging `fired: true` for it would (a) lie
about a refresh that never happened, (b) break criteria 3/4 (firing count identical, exactly one
fired record per error), and (c) — because `anomaly/window.rs:234` keys `watcher-storm` on a single
global `"watcher"` key, not per repo — make `stop_all_watchers()` with 5 repos emit 5 firings
within milliseconds and stamp a spurious `watcher-storm` on **every app exit**. Both `Disconnected`
arms therefore read identically: flush noise, return.

**Only the OUTER `recv()` becomes `recv_timeout(NOISE_FLUSH)`.** It runs only when no burst is in
flight, so it cannot affect fire timing; the inner drain loop, `DEBOUNCE`, the conservative `refs`
fold and `on_change` are byte-for-byte the existing behaviour, and the P110 timing tests are
unaffected. Cost: one idle wake per minute per open repo (5/min at the measured 5 repos).

**Rejected alternatives** (one line each): *level-gated per-batch logging* — a config surface for
redundant records (§1.1). *Sending noise ticks down the channel* — they would extend the debounce
window unless the inner loop switched to a fixed deadline, perturbing P110-tested fire timing.
*Handler-local `NoiseAcc` with a `Drop` tail* (rev 1) — viable but now carrier-less, so every noise
report becomes a standalone record; it also depends on Rust 2021 upvar drop order (see §6).

## 3. Record schema — additive, **no `OBS_SCHEMA_VERSION` bump** (stays `2`)

`record.rs:18` and P91 line 248: a bump is required only for a **retype, removal, or meaning change
of an existing field**. No existing field changes meaning — `paths`/`relevant` still describe the
record's own batch/burst — five optional fields are added, and any pre-P113d reader parses every
new record. The record *population* changes, which a record-schema version cannot express anyway,
so the change is instead made self-describing per record (§3.2): the raw batch count, path count
and error count all stay exactly recoverable by summation.

```rust
// obs/record.rs — LogPayload::Watcher, appended after `burst_class`.
// The first three are `fired` records only; the last two appear on `fired`
// records AND on noise summaries. All omitted when absent/zero.
/// P113d: notify batches coalesced into this burst (replaces the per-batch
/// records deleted at watcher/mod.rs:204).
#[serde(default, skip_serializing_if = "Option::is_none")] batches: Option<u32>,
/// P113d: ms from the first batch of the burst to this record, so burst start
/// = ts − burstMs − debounceMs.
#[serde(default, skip_serializing_if = "Option::is_none")] burst_ms: Option<u32>,
/// P113d: notify Err(_) events in this burst (§4.1). Omitted when 0.
#[serde(default, skip_serializing_if = "Option::is_none")] errors: Option<u32>,
/// P113d: notify batches the `.git`-internals filter rejected whole
/// (`relevant == 0`) since this watcher's previous record. Omitted when 0.
#[serde(default, skip_serializing_if = "Option::is_none")] skipped_batches: Option<u32>,
/// Total paths in those batches.
#[serde(default, skip_serializing_if = "Option::is_none")] skipped_paths: Option<u32>,
```

```ts
// src/obs/types.ts — WatcherPayload, appended
/** P113d — `fired` records only. */
batches?: number;
burstMs?: number;
errors?: number;
/** P113d — irrelevant notify batches folded into this record (Rust only;
 *  never on frontend echo records). */
skippedBatches?: number;
skippedPaths?: number;
```

**Naming.** `skipped*`, deliberately not `suppressed*`: `suppressed`/`suppressReason` are reserved
by P91 §2.4 for the frontend echo path, which `record.rs:289-308` spends 20 lines defending against
exactly this collision.

### 3.2 Reader rule (add to P91 §3.2)

Rust-sourced (`source: "rust"`) `watcher` records of one session:

```
raw_notify_batches = Σ batches      + Σ skippedBatches
raw_notify_paths   = Σ paths        + Σ skippedPaths     // `paths` on fired records
relevant_paths     = Σ relevant
notify_errors      = Σ errors
debounce_firings   = count(fired == true)
```

Exactness holds over **completed** bursts. A burst still in flight when its watcher is torn down is
dropped without a record — unchanged from pre-P113d behaviour (`mod.rs:239`), bounded to at most
one burst per watcher per shutdown, at a moment when nothing is refreshing anyway.

Rev 1's rule (`count(fired == false && relevant > 0) + Σ skippedBatches`) is **void**: no such
record exists any more. Record taxonomy:

| record | source | `fired` | `relevant` | carries |
|---|---|---|---|---|
| debounce firing | rust | true | ≥ 1 | `paths`, `relevant`, `burstClass`, `batches`, `burstMs`, `errors?`, `skipped*?` |
| noise summary | rust | false | 0 | `paths: 0`, `relevant: 0`, `skippedBatches > 0`, `skippedPaths` |
| frontend echo | ui | false | 0 | `suppressed: true`, `suppressReason: "echo"`, `causedBy`; **never** `skipped*` |

`paths`/`relevant` describe the carrier burst only; skipped totals live solely in `skipped*`. The
two sets are disjoint (a `relevant == 0` batch never sends a tick, so it can never enter a
`BurstAcc`) — never add them without the rule above.

## 4. What must NOT be lost

1. **Watcher errors.** `Err(_)` stays `WatchTick { paths: 1, relevant: 1, refs: true, error: true }`
   → always fires (relevant ≥ 1 guarantees a burst), always `Refs`, so the wide refresh still runs.
   It no longer gets its own immediate record; it is reported ≤ (burst length + 300 ms) later by
   `errors` on the fired record that closes its burst. **Guarantee: every notify error is counted
   by exactly one fired record.** Test it (§7.5) — this is dropped-event recovery, the one place a
   lost signal is a correctness bug, not a diagnostic one.
2. **Burst classification (P110).** `burstClass`, the conservative `refs` fold, `on_change`,
   `DEBOUNCE` and the inner drain loop are untouched.
3. **Frontend echo** (`suppressReason: "echo"` + `causedBy`, 11 records): a different producer on
   the other side of the boundary, untouched, and distinguishable from noise summaries by
   `source`/`suppressed`.
4. **"Did the watcher see anything at all in this window?"** stays answerable. Every batch is
   represented: relevant ones by the fired record's `batches`/`paths`/`relevant`, irrelevant ones by
   `skipped*`. Drains are (a) the fire, (b) the `NOISE_FLUSH` tick — a genuine wall-clock bound, not
   a lazy one, because the debounce thread wakes on its own, (c) shutdown. A summary covers
   `(previous watcher record ts, this ts]`. Zero watcher records in a window means zero fs events in
   it — the honest answer, distinguishable from "noise was hidden".
5. **`watcher-storm`.** `anomaly.rs:125` keys on `fired: true`; `anomaly/window.rs:233` counts
   firings. Firings are unchanged in number and timing (see the inner-`Disconnected` rule in §2.3,
   which exists to keep that true at shutdown), and the only remaining non-fired Rust records are
   summaries, which were never counted. Detection is provably identical — test it.

## 5. Guarantees preserved (obs/mod.rs module invariants)

- **Nothing blocks a git or UI path.** Per irrelevant batch: two relaxed `fetch_add`s. Per relevant
  batch: one channel `send`, as today. The notify thread no longer constructs, enqueues or
  serialises any record — **strictly less work than today in both paths**. Emission stays
  `sink.enqueue` = `try_send`, now from one thread only.
  *Ordering caveat:* the two counters are separate atomics, so a drain interleaved between a
  producer's two adds can attribute one batch's `paths` to the next record. Totals stay exact (both
  operations are atomic swaps, nothing is lost); per-record attribution may skew by one batch. State
  this in the field doc comment; a `Mutex` to avoid it would violate the non-blocking invariant for
  no diagnostic gain.
- **Nothing deletes a log file.** No change to §6 pruning, retention or the writer.
- The watcher stays Tauri-decoupled (no `AppHandle`), reaching the sink via
  `obs::trace::active_sink()`.

## 6. Tail semantics (session end) — deterministic

The debounce thread flushes the noise counters in **both** its exit arms (`Disconnected` in the
outer and inner loops) before returning, and `WatcherHandle::drop` already drops the watcher first
(disconnecting the channel) and then **joins that thread**. So `WatcherHandle::drop` returning
implies the tail record is enqueued — the existing join is the barrier, with no reliance on drop
order inside the handler closure.

> Rev 1 put the accumulator in the closure and needed `struct HandlerState { noise, tx, git_dir }`
> so that declaration order (= drop order) flushed `noise` before `tx` disconnected — Rust 2021
> captures disjoint fields as separate upvars in **unspecified** drop order, so three bare upvars
> would race the flush against `shutdown_on_exit`. That analysis is why the counters live behind an
> `Arc` drained on the debounce thread instead: the hazard disappears rather than being managed.

**Still required — `lib.rs:362` ordering.** `obs::shutdown_on_exit` runs inside `ExitRequested`, but
managed state (hence every `WatcherHandle`) drops only **after** the run loop returns, so the tail
would hit `active_sink() == None` and be silently lost. In the `ExitRequested` arm, **before**
`obs::shutdown_on_exit(...)`:

```rust
// state.rs
impl AppState {
    /// Drops every repo's `WatcherHandle` (collect the handles under the map
    /// lock, drop them AFTER releasing it — `Drop` joins the debounce thread and
    /// must not run under the mutex). Idempotent; leaves the `RepoEntry` in
    /// place with `watcher: None`.
    pub fn stop_all_watchers(&self);
}
```
```rust
// lib.rs, ExitRequested arm, immediately before obs::shutdown_on_exit
app.state::<state::AppState>().stop_all_watchers();
```

## 7. Tests

**Must change:**

| File | Why |
|---|---|
| `src-tauri/src/watcher/tests.rs` | `watcher_record_shape` (185-245) drives the removed `log_watcher` and asserts a non-fired raw record exists — rewrite against `log_watch_fired` / `log_watch_noise` (reuse `obs::trace::test_sink_lock()` + `set_active_sink`). `burst_accumulation_is_conservative` / `worktree_only_burst_*` / `mixed_burst_*` (247-320) gain `batches`/`errors`. |
| `src-tauri/src/obs/tests_anomaly_support.rs:184` | The only `LogPayload::Watcher` struct literal outside the emit site; add the 5 fields as `None`. |
| `src-tauri/src/obs/tests_anomaly.rs:154` | Uses that helper; add the assertion that summaries never feed `watcher-storm`. |
| `src/obs/types.ts` | Type mirror (not a test); optional fields only, no call-site change. |

**Must add — unit (deterministic, no wall clock):**
1. `no_record_until_the_burst_fires` — drive the debounce loop with N relevant ticks and no
   timeout: zero records emitted; the record appears only at the `Timeout` arm. (Assert on the
   loop, not the handler — the handler has no emit site left, so a handler-level assertion is a
   no-op test.)
2. `fired_record_carries_batches_and_burst_ms` — `batches == N`, `burstMs > 0`, `paths`/`relevant`
   are the existing sums.
3. `noise_rides_on_next_fired_record` — M irrelevant batches then a fire → one record with
   `skippedBatches == M`, `skippedPaths` summed; counters zero afterwards.
4. `noise_summary_when_no_fire` — drain with no burst → `paths: 0, relevant: 0, fired: false,
   skippedBatches > 0`.
5. `error_tick_counted_and_refs` — an `Err(_)` tick yields `errors >= 1` on exactly one fired record
   and `burstClass == "refs"`; N errors in one burst → `errors == N` (§4.1).
6. `raw_batch_count_recoverable` — §3.2's formula over a synthesised mixed sequence equals the true
   batch/path/error counts exactly.
7. `noise_counters_drained_when_sink_absent` — with `active_sink() == None`, a drain still zeroes
   the counters (§2.2).
8. `watcher_storm_detection_unchanged` — identical firing sequence in/out, identical anomalies.
9. `teardown_mid_burst_emits_no_fired_record` — disconnect while a burst is in flight: noise
   summary only, no `fired: true`, no `on_change` (§2.3, and the guard against the app-exit
   `watcher-storm` false positive).

**Must add — integration, alongside `drop_is_clean` (`watcher/tests.rs:157`, under the existing
`serialize_watcher_test()` guard):**
10. `tail_noise_record_on_handle_drop` — spawn on the fixture repo, write an irrelevant `.git`
    internal path (e.g. under `.git/objects/zz/`), drop the `WatcherHandle`, assert a
    `skippedBatches > 0` record is present once `drop` has returned.

**Must still pass unchanged (guards, do not edit):**
`src/components/repoWorkspace/useCoalescedRefresh.causality.test.tsx`,
`src/components/repoWorkspace/useCoalescedRefresh.test.tsx`.
`src/ipc/mock/handlers/session.ts` proxies `repo-changed` only and emits no watcher record;
`src/graph/replay/*` uses `'watcher'` solely as a refresh-origin string and reads no log records —
both must still compile, neither needs a change.

**Add to `src-tauri/src/obs/tests_record.rs`** (zero watcher coverage today): serialize
`LogPayload::Watcher` with all five new fields `None` and assert the JSON key set is byte-identical
to the pre-P113d shape — the record-snapshot discipline this repo uses for schema-visible text.

## 8. Acceptance criteria

Structural (unit-testable, no session needed):

1. **Zero** Rust-emitted watcher records with `fired == false` and `skippedBatches` absent. (The
   only non-fired Rust record is a noise summary, and it always carries the counter.)
2. `raw_notify_batches`, `raw_notify_paths` and `notify_errors` per §3.2 equal the true values
   **exactly** on a synthesised sequence mixing relevant batches, irrelevant batches, errors and
   fires (completed bursts — see the §3.2 teardown clause).
3. Firing count, firing timing, `burstClass` values and `on_change` calls are **identical** to
   pre-P113d for the same tick sequence, including at teardown (P110 tests pass untouched;
   `DEBOUNCE` unchanged).
4. Every notify error is counted by exactly one fired record's `errors`.
5. At most one noise summary per `NOISE_FLUSH` (60 s) per watcher.
6. After `stop_all_watchers()` returns, the tail record is on the sink (test 10 / §6).
7. `OBS_SCHEMA_VERSION` still `2`; a pre-P113d reader parses every new record; the JSON key set of
   a record with no new field set is unchanged.

Empirical, on a comparable Dev session (≥ 3 repos, similar length and activity), against the
corrected baseline (19,547 records / 3.65 MB / 12,001 watcher records = 61.4% / 192 fires):

8. Watcher records fall by **≥ 90%** (12,001 → ≤ 1,200; projected ≈ 192 fires + a few hundred noise
   summaries), while `debounce_firings` stays within normal variance of 192 for comparable activity.
9. `kind:"watcher"` is **≤ 10%** of total records (projected ≈ 6%, from 61.4%).
10. Total records **≥ 50%** smaller and file size **≥ 45%** smaller (projected ≈ 19,547 → ~8,000;
    3.65 MB → ~1.9 MB).
11. Still **zero** `kind:"drop"` records.
12. No `watcher-storm` anomaly appears at app exit (the §2.3 inner-`Disconnected` guard).
13. `raw_notify_batches` recovered per §3.2 from the new session is of the same order as the old
    session's raw batch count for comparable activity — i.e. the *events* remain recoverable even
    though the *records* no longer exist. (Sanity check, not a tolerance: two sessions never have
    identical fs activity.)

## 9. Flags for the orchestrator

1. **Pre-existing schema drift** — `src/ipc/types/obs.ts:16` says `OBS_SCHEMA_VERSION = 1`,
   `src-tauri/src/obs/record.rs:35` says `2`, with no parity test. Unrelated to P113d; keep it
   filed separately. Do **not** fold it into this increment.
2. **Watcher records carry no repo identity.** With 5 repos open, each watcher has its own counters,
   so per-record figures are correct but **unattributable** to a repo. Adding `repoId` is a real
   schema addition plus a redaction question (paths). Recommend deferring. Related:
   `watcher-storm` is keyed globally (`anomaly/window.rs:234`), not per repo — noted here because
   §2.3's rule exists to stop that becoming an exit-time false positive.
3. **`NOISE_FLUSH = 60 s` is a judgement call.** Lower = finer noise timeline, more records; higher
   = fewer records, coarser window. 60 s bounds a pure-noise stretch to ≈152 summaries per repo in
   a 152-min session, well inside criterion 8. Raise to 300 s if criterion 9 misses in practice — a
   one-constant change, no schema impact.
4. **Intra-burst timing is deliberately gone.** `batches` + `burstMs` recover the count and the
   burst extent but not individual batch arrival times. Nothing in the current analysis needs them;
   if a future investigation does, that is a temporary debug build, not a shipped log level.
5. **`u32` saturation.** `NoiseCounters` uses `AtomicU32` with plain `fetch_add`. 4.29 G batches
   between two drains is unreachable (a drain is at most 60 s away), so a saturating
   compare-exchange loop is optional; senior-dev may use plain `fetch_add` and note it. Flagged
   only so the choice is deliberate.
